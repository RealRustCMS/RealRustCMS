use crate::{
    csrf::gerar_token,
    error::{AppError, Result},
    models::{
        calcular_tempo_leitura, ArtigoListagem, MenuItemArvore, NovaAvaliacao, NovoComentario,
        Paginacao, QueryAvaliado, QueryPagina,
    },
    rate_limit,
    repositories::{
        artigos::ArtigosRepo,
        avaliacoes::AvaliacoesRepo,
        busca::BuscaRepo,
        categorias::{artigo_completo, CategoriasRepo, TagsRepo},
        comentarios::ComentariosRepo,
        configuracoes::ConfiguracoesRepo,
        eventos::EventosRepo,
        galeria::GaleriaRepo,
        page_views::PageViewsRepo,
        paginas::PaginasRepo,
        usuarios::UsuariosRepo,
    },
    services::{comentarios::ComentariosService, membros::MembrosService},
    state::AppState,
};

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use tera::Context;
use tower_sessions::Session;

/// Serve o HTML de uma listagem pública a partir do cache quando:
///  - o admin habilitou o cache (`cache_ttl` > 0), e
///  - o visitante é anônimo (`sessao_membro_ativa` == false) — logado vê menu
///    filtrado e artigos restritos, o conteúdo varia por sessão.
/// `try_get_with` faz coalescing: sob stampede, `render` roda uma vez só por
/// chave. `render` só é criado/avaliado no miss; no hit é descartado sem custo.
async fn pagina_cacheada<F>(
    state: &AppState,
    session: &Session,
    chave: String,
    render: F,
) -> Result<Html<String>>
where
    F: Future<Output = Result<String>> + Send,
{
    if state.cache_ttl_segundos() == 0 || MembrosService::sessao_membro_ativa(session).await {
        return Ok(Html(render.await?));
    }

    let html = state
        .pagina_cache
        .try_get_with(chave, async move { render.await.map(Arc::<str>::from) })
        .await
        .map_err(|e: Arc<AppError>| {
            AppError::Interno(format!("falha ao renderizar página cacheada: {e}"))
        })?;

    Ok(Html(html.to_string()))
}

// Injeta as variáveis globais do site em todos os templates públicos.
// menu vem do cache em memória — árvore com submenus ilimitados, sem query por request.
// Itens restritos são removidos da árvore (cascata: filho some se o pai sumir)
// quando não há sessão de membro/usuário ativa. Checagem só de sessão, sem ida
// ao banco — ver doc de MembrosService::sessao_membro_ativa.
async fn ctx_base(state: &AppState, session: &Session) -> Context {
    let logado = MembrosService::sessao_membro_ativa(session).await;
    let menu = state.ler_menu_cache().await;
    let menu = if logado {
        menu
    } else {
        filtrar_menu_restrito(menu)
    };

    let mut ctx = Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_descricao", &state.config.site_descricao);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("tema", &state.config.tema);
    ctx.insert("menu", &menu);
    ctx
}

// Remove recursivamente itens com restrito = true. Quando um item pai some,
// seus filhos somem junto (cascata) — não há como um filho aparecer "órfão"
// sem o pai no menu renderizado.
fn filtrar_menu_restrito(itens: Vec<MenuItemArvore>) -> Vec<MenuItemArvore> {
    itens
        .into_iter()
        .filter(|item| !item.restrito)
        .map(|mut item| {
            item.filhos = filtrar_menu_restrito(item.filhos);
            item
        })
        .collect()
}

// Decide se as listagens públicas (home, /artigos, categoria, tag, RSS,
// relacionados) devem ocultar artigos com restrito = true.
// Quem está logado (membro ou usuário CMS) sempre vê tudo, independente da
// configuração — a flag só afeta visitantes anônimos. Sem sessão ativa,
// respeita a configuração `mostrar_artigos_restritos_listagem` do admin.
async fn deve_ocultar_restritos(state: &AppState, session: &Session) -> bool {
    if MembrosService::sessao_membro_ativa(session).await {
        return false;
    }
    let mostrar = ConfiguracoesRepo::novo(&state.db)
        .get_listagem_restrita()
        .await
        .unwrap_or_default()
        .mostrar_artigos_restritos_listagem;
    !mostrar
}

/// Junta Vec<Artigo> com as stats de avaliação, devolvendo Vec<ArtigoListagem>.
/// O HashMap fica aqui no Rust — o Tera acessa artigo.avaliacao diretamente,
/// sem precisar fazer lookup por chave (que o Tera não suporta bem).
fn artigos_com_avaliacao(
    artigos: Vec<crate::models::Artigo>,
    stats: Vec<(String, crate::models::AvaliacaoStats)>,
) -> Vec<ArtigoListagem> {
    let mut mapa: HashMap<String, crate::models::AvaliacaoStats> = stats.into_iter().collect();
    artigos
        .into_iter()
        .map(|a| {
            let av = mapa.remove(&a.id);
            ArtigoListagem::from_artigo(a, av, None)
        })
        .collect()
}

pub async fn home(State(state): State<AppState>, session: Session) -> Result<Html<String>> {
    pagina_cacheada(
        &state,
        &session,
        "pub:home".to_string(),
        home_render(&state, &session),
    )
    .await
}

async fn home_render(state: &AppState, session: &Session) -> Result<String> {
    let artigos_repo = ArtigosRepo::novo(&state.db);
    let ocultar_restritos = deve_ocultar_restritos(state, session).await;

    // Artigos em destaque — separados em hero (primeiro) e secundários (resto)
    let mut destaques = artigos_repo
        .listar_destaques(state.config.artigos_destaque, ocultar_restritos)
        .await
        .unwrap_or_default();

    let destaque_hero = if !destaques.is_empty() {
        Some(destaques.remove(0))
    } else {
        None
    };
    let destaques_secundarios = destaques;

    // IDs dos destaques para excluir da lista de recentes
    let ids_destaque: Vec<String> = destaque_hero
        .iter()
        .map(|a| a.id.clone())
        .chain(destaques_secundarios.iter().map(|a| a.id.clone()))
        .collect();

    let artigos = artigos_repo
        .listar_publicados(ocultar_restritos)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|a| !ids_destaque.contains(&a.id))
        .collect::<Vec<_>>();

    let proximos_eventos = EventosRepo::novo(&state.db)
        .listar_proximos(3)
        .await
        .unwrap_or_default();

    let mut ctx = ctx_base(state, session).await;
    ctx.insert("destaque_hero", &destaque_hero);
    ctx.insert("destaques_secundarios", &destaques_secundarios);
    ctx.insert("artigos", &artigos);
    ctx.insert("proximos_eventos", &proximos_eventos);

    Ok(state
        .tera
        .render(&state.config.template_pub("home.html"), &ctx)?)
}

pub async fn listar_artigos(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryPagina>,
) -> Result<Html<String>> {
    let pagina = query.pagina.unwrap_or(1).max(1);
    pagina_cacheada(
        &state,
        &session,
        format!("pub:artigos:{pagina}"),
        listar_artigos_render(&state, &session, pagina),
    )
    .await
}

async fn listar_artigos_render(
    state: &AppState,
    session: &Session,
    pagina: i64,
) -> Result<String> {
    let por_pagina = state.config.artigos_por_pagina;
    let ocultar_restritos = deve_ocultar_restritos(state, session).await;

    let (artigos, total) = ArtigosRepo::novo(&state.db)
        .listar_publicados_paginado(pagina, por_pagina, ocultar_restritos)
        .await
        .unwrap_or_default();

    let ids: Vec<String> = artigos.iter().map(|a| a.id.clone()).collect();
    let stats = AvaliacoesRepo::novo(&state.db)
        .buscar_stats_multiplos(&ids)
        .await
        .unwrap_or_default();

    let artigos = artigos_com_avaliacao(artigos, stats);
    let paginacao = Paginacao::calcular(pagina, total, por_pagina);

    let mut ctx = ctx_base(state, session).await;
    ctx.insert("artigos", &artigos);
    ctx.insert("paginacao", &paginacao);

    Ok(state
        .tera
        .render(&state.config.template_pub("artigos.html"), &ctx)?)
}

pub async fn ver_artigo(
    State(state): State<AppState>,
    session: Session,
    Path(slug): Path<String>,
    Query(query): Query<QueryAvaliado>,
) -> Result<impl IntoResponse> {
    match ArtigosRepo::novo(&state.db).buscar_por_slug(&slug).await {
        Ok(artigo) => {
            // Bloqueio de conteúdo restrito: sem sessão de membro/usuário,
            // redireciona para o login de membros preservando a URL de destino.
            // Feito antes de registrar page view / montar contexto — visitante
            // sem acesso não deve gerar efeitos colaterais nem ver o conteúdo.
            if artigo.restrito && !MembrosService::sessao_membro_ativa(&session).await {
                let next = format!("/artigos/{}", slug);
                return Ok(Redirect::to(&format!(
                    "/membros/login?next={}",
                    urlencoding::encode(&next)
                ))
                .into_response());
            }

            let url = format!("/artigos/{}", slug);
            PageViewsRepo::novo(&state.db).registrar(&url).await.ok();
            let views = PageViewsRepo::novo(&state.db)
                .contar(&url)
                .await
                .unwrap_or(0);
            let csrf = gerar_token(&session).await;

            let comentarios = if artigo.comentarios_habilitados {
                ComentariosRepo::novo(&state.db)
                    .listar_por_url(&url)
                    .await
                    .unwrap_or_default()
            } else {
                vec![]
            };

            // Só busca stats se avaliações estiverem habilitadas — evita query desnecessária
            let avaliacao = if artigo.avaliacoes_habilitadas {
                AvaliacoesRepo::novo(&state.db)
                    .buscar_stats(&artigo.id)
                    .await
                    .unwrap_or(None)
            } else {
                None
            };

            // true quando o redirect pós-voto trouxe ?avaliado=1
            // Permite ao template exibir "Obrigado!" sem estado no servidor
            let avaliado = query.avaliado.as_deref() == Some("1");

            let ocultar_restritos = deve_ocultar_restritos(&state, &session).await;
            let completo = artigo_completo(&state.db, artigo.clone()).await;
            let tempo_leitura = calcular_tempo_leitura(&completo.artigo.corpo);
            let data_formatada = completo
                .artigo
                .publicado_em
                .map(formatar_data_artigo)
                .unwrap_or_default();
            let relacionados = ArtigosRepo::novo(&state.db)
                .buscar_relacionados(
                    &artigo.id,
                    completo.artigo.categoria_id.as_deref(),
                    state.config.artigos_relacionados,
                    ocultar_restritos,
                )
                .await
                .unwrap_or_default();
            let mut ctx = ctx_base(&state, &session).await;
            ctx.insert("artigo", &completo.artigo);
            ctx.insert("categoria", &completo.categoria);
            ctx.insert("tags", &completo.tags);
            ctx.insert("autor_nome", &completo.autor_nome);
            ctx.insert("views", &views);
            ctx.insert("tempo_leitura", &tempo_leitura);
            ctx.insert("data_formatada", &data_formatada);
            ctx.insert("base_url", &state.config.base_url);
            ctx.insert("relacionados", &relacionados);
            ctx.insert("comentarios", &comentarios);
            ctx.insert("avaliacao", &avaliacao);
            ctx.insert("avaliado", &avaliado);
            ctx.insert("turnstile_site_key", &state.config.turnstile_site_key);
            ctx.insert("csrf_token", &csrf);
            Ok(Html(
                state
                    .tera
                    .render(&state.config.template_pub("artigo.html"), &ctx)?,
            )
            .into_response())
        }
        Err(_) => Ok(Redirect::to("/artigos").into_response()),
    }
}

/// Exibe uma página estática pelo slug.
/// Só renderiza páginas com publicada = true — rascunhos retornam 404.
pub async fn ver_pagina(
    State(state): State<AppState>,
    session: Session,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse> {
    match PaginasRepo::novo(&state.db).buscar_por_slug(&slug).await? {
        Some(pagina) => {
            // Mesmo bloqueio aplicado a artigos restritos: sem sessão de
            // membro/usuário, redireciona para o login preservando destino.
            if pagina.restrito && !MembrosService::sessao_membro_ativa(&session).await {
                let next = format!("/paginas/{}", slug);
                return Ok(Redirect::to(&format!(
                    "/membros/login?next={}",
                    urlencoding::encode(&next)
                ))
                .into_response());
            }

            let url = format!("/paginas/{}", slug);
            PageViewsRepo::novo(&state.db).registrar(&url).await.ok();

            let mut ctx = ctx_base(&state, &session).await;
            ctx.insert("pagina", &pagina);
            ctx.insert("base_url", &state.config.base_url);

            Ok(Html(
                state
                    .tera
                    .render(&state.config.template_pub("pagina.html"), &ctx)?,
            )
            .into_response())
        }
        // Página não encontrada ou não publicada — redireciona para home
        None => Ok(Redirect::to("/").into_response()),
    }
}

/// Listagem pública de eventos — só publicados, paginada.
pub async fn listar_eventos(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryPagina>,
) -> Result<Html<String>> {
    let pagina = query.pagina.unwrap_or(1).max(1);
    pagina_cacheada(
        &state,
        &session,
        format!("pub:eventos:{pagina}"),
        listar_eventos_render(&state, &session, pagina),
    )
    .await
}

async fn listar_eventos_render(
    state: &AppState,
    session: &Session,
    pagina: i64,
) -> Result<String> {
    let por_pagina = state.config.artigos_por_pagina;

    let (eventos, total) = EventosRepo::novo(&state.db)
        .listar_publicados_paginado(pagina, por_pagina)
        .await
        .unwrap_or_default();

    let paginacao = Paginacao::calcular(pagina, total, por_pagina);

    let mut ctx = ctx_base(state, session).await;
    ctx.insert("eventos", &eventos);
    ctx.insert("paginacao", &paginacao);

    Ok(state
        .tera
        .render(&state.config.template_pub("eventos.html"), &ctx)?)
}

/// Exibe um evento pelo slug. Só renderiza eventos com publicado = true.
pub async fn ver_evento(
    State(state): State<AppState>,
    session: Session,
    Path(slug): Path<String>,
) -> Result<impl IntoResponse> {
    match EventosRepo::novo(&state.db).buscar_por_slug(&slug).await? {
        Some(evento) => {
            let url = format!("/eventos/{}", slug);
            PageViewsRepo::novo(&state.db).registrar(&url).await.ok();

            let mut ctx = ctx_base(&state, &session).await;
            ctx.insert("evento", &evento);
            ctx.insert("base_url", &state.config.base_url);

            Ok(Html(
                state
                    .tera
                    .render(&state.config.template_pub("evento.html"), &ctx)?,
            )
            .into_response())
        }
        // Evento não encontrado ou não publicado — redireciona para a listagem
        None => Ok(Redirect::to("/eventos").into_response()),
    }
}

/// Recebe o voto por estrelas enviado pelo visitante.
/// IP resolvido via ip::do_cliente (X-Forwarded-For do Caddy, com fallback
/// pro ConnectInfo cru) — mesmo mecanismo do rate limiting no login.
/// Duplicatas (mesmo IP + mesmo artigo) são ignoradas pelo banco via INSERT IGNORE.
/// Redireciona com ?avaliado=1 para o template exibir feedback sem estado no servidor.
pub async fn avaliar_artigo(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(slug): Path<String>,
    Form(dados): Form<NovaAvaliacao>,
) -> impl IntoResponse {
    let ip = crate::ip::do_cliente(&headers, addr);

    // ─── Rate limiting ────────────────────────────────────
    // Evita flood de requisições por IP. A UNIQUE constraint no banco
    // já garante 1 voto por IP por artigo — o rate limit protege antes
    // de chegar ao banco.
    if let Err(_) = rate_limit::verificar_avaliacao(&ip) {
        tracing::warn!(ip = %ip, "Rate limit atingido em avaliações");
        return Redirect::to(&format!("/artigos/{}#avaliacao", slug)).into_response();
    }

    let artigo = match ArtigosRepo::novo(&state.db).buscar_por_slug(&slug).await {
        Ok(a) => a,
        Err(_) => return Redirect::to("/artigos").into_response(),
    };

    if !artigo.avaliacoes_habilitadas {
        return Redirect::to(&format!("/artigos/{}", slug)).into_response();
    }

    let repo = AvaliacoesRepo::novo(&state.db);
    match repo.votar(&artigo.id, dados.nota, &ip).await {
        Ok(true) => {
            tracing::info!(slug = %slug, ip = %ip, nota = dados.nota, "Avaliação registrada")
        }
        Ok(false) => tracing::debug!(slug = %slug, ip = %ip, "Voto duplicado ignorado"),
        Err(e) => tracing::error!(slug = %slug, erro = %e, "Erro ao registrar avaliação"),
    }

    // ?avaliado=1 sinaliza ao template que deve exibir a mensagem de obrigado.
    // A âncora #avaliacao garante que o visitante role direto para o bloco.
    Redirect::to(&format!("/artigos/{}?avaliado=1#avaliacao", slug)).into_response()
}

pub async fn postar_comentario(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(slug): Path<String>,
    Form(dados): Form<NovoComentario>,
) -> impl IntoResponse {
    let ip = crate::ip::do_cliente(&headers, addr);

    // ─── Rate limiting ────────────────────────────────────
    // 5 comentários por 5 minutos por IP.
    // O Turnstile (quando habilitado) já filtra bots — este limite
    // é uma camada adicional para quando o captcha está desabilitado.
    if let Err(_) = rate_limit::verificar_comentario(&ip) {
        tracing::warn!(ip = %ip, "Rate limit atingido em comentários");
        return Redirect::to(&format!("/artigos/{}#comentarios", slug)).into_response();
    }

    let artigo = match ArtigosRepo::novo(&state.db).buscar_por_slug(&slug).await {
        Ok(a) => a,
        Err(_) => return Redirect::to("/artigos").into_response(),
    };

    if !artigo.comentarios_habilitados {
        return Redirect::to(&format!("/artigos/{}", slug)).into_response();
    }

    let url = format!("/artigos/{}", slug);
    let moderado = artigo.moderacao_habilitada;

    // Busca e-mail do autor para notificação (apenas se necessário)
    let email_autor = if artigo.notificar_comentarios && moderado {
        UsuariosRepo::novo(&state.db)
            .buscar_por_id(&artigo.autor_id)
            .await
            .ok()
            .flatten()
            .and_then(|u| {
                if u.email.is_empty() {
                    None
                } else {
                    Some(u.email)
                }
            })
    } else {
        None
    };

    let service = ComentariosService::novo(&state.db, state.config.turnstile_secret_key.clone());

    match service
        .criar_comentario(
            &url,
            &dados,
            moderado,
            Some(&artigo),
            email_autor.as_deref(),
            state.config.smtp.as_ref(),
            &state.config.base_url,
        )
        .await
    {
        Ok(aprovado_imediatamente) => {
            if aprovado_imediatamente {
                tracing::info!(url = %url, ip = %ip, "Comentário postado e aprovado automaticamente");
            } else {
                tracing::info!(url = %url, ip = %ip, "Comentário postado, aguardando moderação");
            }
            let ancora = if aprovado_imediatamente {
                "#comentarios"
            } else {
                "#comentario-pendente"
            };
            Redirect::to(&format!("/artigos/{}{}", slug, ancora)).into_response()
        }
        Err(e) => {
            tracing::error!(url = %url, erro = %e, "Falha ao criar comentário");
            Redirect::to(&format!("/artigos/{}#comentarios", slug)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct QueryBusca {
    pub q: Option<String>,
}

pub async fn buscar(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryBusca>,
) -> Result<Html<String>> {
    let termo = query.q.unwrap_or_default();
    let ocultar_restritos = deve_ocultar_restritos(&state, &session).await;
    let resultados = if termo.len() >= 2 {
        BuscaRepo::novo(&state.db)
            .buscar_publico(&termo, ocultar_restritos)
            .await
            .unwrap_or_default()
    } else {
        vec![]
    };

    let mut ctx = ctx_base(&state, &session).await;
    ctx.insert("termo", &termo);
    ctx.insert("resultados", &resultados);
    ctx.insert("total", &resultados.len());

    Ok(Html(
        state
            .tera
            .render(&state.config.template_pub("busca.html"), &ctx)?,
    ))
}

pub async fn listar_galeria(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    pagina_cacheada(
        &state,
        &session,
        "pub:galeria".to_string(),
        listar_galeria_render(&state, &session),
    )
    .await
}

async fn listar_galeria_render(state: &AppState, session: &Session) -> Result<String> {
    let albuns = GaleriaRepo::novo(&state.db)
        .listar_albuns_com_capa()
        .await
        .unwrap_or_default();

    let mut ctx = ctx_base(state, session).await;
    ctx.insert("albuns", &albuns);

    Ok(state
        .tera
        .render(&state.config.template_pub("galeria.html"), &ctx)?)
}

pub async fn ver_album_publico(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let repo = GaleriaRepo::novo(&state.db);

    match repo.buscar_album(&id).await {
        Ok(album) => {
            let url = format!("/galeria/{}", id);
            PageViewsRepo::novo(&state.db).registrar(&url).await.ok();
            let views = PageViewsRepo::novo(&state.db)
                .contar(&url)
                .await
                .unwrap_or(0);

            let fotos = repo.listar_fotos_do_album(&id).await.unwrap_or_default();
            let mut ctx = ctx_base(&state, &session).await;
            ctx.insert("album", &album);
            ctx.insert("fotos", &fotos);
            ctx.insert("views", &views);
            Ok(Html(
                state
                    .tera
                    .render(&state.config.template_pub("album.html"), &ctx)?,
            )
            .into_response())
        }
        Err(_) => Ok(Redirect::to("/galeria").into_response()),
    }
}

pub async fn artigos_por_categoria(
    State(state): State<AppState>,
    session: Session,
    Path(slug): Path<String>,
    Query(query): Query<QueryPagina>,
) -> Result<impl IntoResponse> {
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = state.config.artigos_por_pagina;
    let ocultar_restritos = deve_ocultar_restritos(&state, &session).await;

    // Existência da categoria é checada sempre (fora do cache) para preservar
    // o redirect para /artigos em slug inválido; o resto do render é cacheado.
    let (artigos, total, categoria) = match CategoriasRepo::novo(&state.db)
        .artigos_da_categoria(&slug, pagina, por_pagina, ocultar_restritos)
        .await
    {
        Ok(t) => t,
        Err(_) => return Ok(Redirect::to("/artigos").into_response()),
    };

    let chave = format!("pub:categoria:{slug}:{pagina}");
    let html = pagina_cacheada(&state, &session, chave, async {
        let ids: Vec<String> = artigos.iter().map(|a| a.id.clone()).collect();
        let stats = AvaliacoesRepo::novo(&state.db)
            .buscar_stats_multiplos(&ids)
            .await
            .unwrap_or_default();
        let artigos = artigos_com_avaliacao(artigos, stats);

        let paginacao = Paginacao::calcular(pagina, total, por_pagina);
        let mut ctx = ctx_base(&state, &session).await;
        ctx.insert("artigos", &artigos);
        ctx.insert("paginacao", &paginacao);
        ctx.insert("titulo", &format!("Categoria: {}", categoria.nome));
        ctx.insert("descricao", &format!("{} artigos", total));
        Ok(state
            .tera
            .render(&state.config.template_pub("artigos.html"), &ctx)?)
    })
    .await?;
    Ok(html.into_response())
}

pub async fn artigos_por_tag(
    State(state): State<AppState>,
    session: Session,
    Path(slug): Path<String>,
    Query(query): Query<QueryPagina>,
) -> Result<impl IntoResponse> {
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = state.config.artigos_por_pagina;
    let ocultar_restritos = deve_ocultar_restritos(&state, &session).await;

    let (artigos, total, tag) = match TagsRepo::novo(&state.db)
        .artigos_da_tag(&slug, pagina, por_pagina, ocultar_restritos)
        .await
    {
        Ok(t) => t,
        Err(_) => return Ok(Redirect::to("/artigos").into_response()),
    };

    let chave = format!("pub:tag:{slug}:{pagina}");
    let html = pagina_cacheada(&state, &session, chave, async {
        let ids: Vec<String> = artigos.iter().map(|a| a.id.clone()).collect();
        let stats = AvaliacoesRepo::novo(&state.db)
            .buscar_stats_multiplos(&ids)
            .await
            .unwrap_or_default();
        let artigos = artigos_com_avaliacao(artigos, stats);

        let paginacao = Paginacao::calcular(pagina, total, por_pagina);
        let mut ctx = ctx_base(&state, &session).await;
        ctx.insert("artigos", &artigos);
        ctx.insert("paginacao", &paginacao);
        ctx.insert("titulo", &format!("Tag: {}", tag.nome));
        ctx.insert("descricao", &format!("{} artigos", total));
        Ok(state
            .tera
            .render(&state.config.template_pub("artigos.html"), &ctx)?)
    })
    .await?;
    Ok(html.into_response())
}

// ─── RSS FEED ─────────────────────────────────────────────
//
// Retorna os últimos 20 artigos publicados no formato RSS 2.0.
// Sem dependência nova — o XML é montado manualmente como String.
// Content-Type: application/xml com charset UTF-8.
//
// Padrão RSS 2.0: https://www.rssboard.org/rss-specification
// Leitores compatíveis: Feedly, NewsBlur, Firefox, qualquer agregador.

pub async fn rss(State(state): State<AppState>) -> impl IntoResponse {
    // RSS é sempre consumido anonimamente — não há sessão de membro num leitor
    // de feeds. Respeita apenas a configuração de visibilidade do admin.
    let mostrar = ConfiguracoesRepo::novo(&state.db)
        .get_listagem_restrita()
        .await
        .unwrap_or_default()
        .mostrar_artigos_restritos_listagem;

    let artigos = ArtigosRepo::novo(&state.db)
        .listar_publicados(!mostrar)
        .await
        .unwrap_or_default();

    // Pega a URL base do config ou usa localhost como fallback
    let base_url = state.config.base_url.trim_end_matches('/').to_string();

    // Data de build = data do artigo mais recente, ou agora se não houver artigos
    let last_build = artigos
        .first()
        .and_then(|a| a.publicado_em)
        .map(|d| formatar_data_rss(d))
        .unwrap_or_else(|| formatar_data_rss(chrono::Utc::now()));

    // Escapa caracteres XML para evitar XML inválido no conteúdo
    let site_nome = escapar_xml(&state.config.site_nome);
    let site_descricao = escapar_xml(&state.config.site_descricao);

    let mut itens = String::new();
    for artigo in artigos.iter().take(20) {
        let titulo = escapar_xml(&artigo.titulo);
        let link = format!("{}/artigos/{}", base_url, artigo.slug);
        let pub_date = artigo
            .publicado_em
            .map(formatar_data_rss)
            .unwrap_or_default();
        let guid = link.clone();

        itens.push_str(&format!(
            "    <item>\n\
                   <title>{titulo}</title>\n\
                   <link>{link}</link>\n\
                   <guid isPermaLink=\"true\">{guid}</guid>\n\
                   <pubDate>{pub_date}</pubDate>\n\
                 </item>\n"
        ));
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n\
           <channel>\n\
             <title>{site_nome}</title>\n\
             <link>{base_url}</link>\n\
             <description>{site_descricao}</description>\n\
             <language>pt-BR</language>\n\
             <lastBuildDate>{last_build}</lastBuildDate>\n\
             <atom:link href=\"{base_url}/rss\" rel=\"self\" type=\"application/rss+xml\"/>\n\
         {itens}\
           </channel>\n\
         </rss>"
    );

    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/xml; charset=utf-8",
        )],
        xml,
    )
}

pub async fn sitemap(State(state): State<AppState>) -> impl IntoResponse {
    let base_url = state.config.base_url.trim_end_matches('/').to_string();

    // Sitemap também é consumido anonimamente (crawlers) — mesma regra do RSS.
    let mostrar = ConfiguracoesRepo::novo(&state.db)
        .get_listagem_restrita()
        .await
        .unwrap_or_default()
        .mostrar_artigos_restritos_listagem;

    let artigos = ArtigosRepo::novo(&state.db)
        .listar_publicados(!mostrar)
        .await
        .unwrap_or_default();

    let paginas = PaginasRepo::novo(&state.db)
        .listar_publicadas()
        .await
        .unwrap_or_default();

    let mut urls = String::new();

    // Home
    urls.push_str(&format!(
        "  <url>
    <loc>{base_url}/</loc>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
  </url>
"
    ));

    // Artigos publicados
    for artigo in &artigos {
        let loc = format!("{}/artigos/{}", base_url, artigo.slug);
        let lastmod = artigo
            .publicado_em
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| artigo.criado_em.format("%Y-%m-%d").to_string());
        urls.push_str(&format!(
            "  <url>
    <loc>{loc}</loc>
    <lastmod>{lastmod}</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.8</priority>
  </url>
"
        ));
    }

    // Páginas estáticas publicadas
    for pagina in &paginas {
        let loc = format!("{}/paginas/{}", base_url, pagina.slug);
        let lastmod = pagina.atualizado_em.format("%Y-%m-%d").to_string();
        urls.push_str(&format!(
            "  <url>
    <loc>{loc}</loc>
    <lastmod>{lastmod}</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.6</priority>
  </url>
"
        ));
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n\
         {urls}\
         </urlset>"
    );

    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/xml; charset=utf-8",
        )],
        xml,
    )
}

// Formata DateTime<Utc> no formato RFC 2822 exigido pelo RSS.
// Exemplo: "Sat, 06 Jun 2026 12:00:00 +0000"
fn formatar_data_rss(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%a, %d %b %Y %H:%M:%S +0000").to_string()
}

// Formata DateTime<Utc> com mês abreviado em português.
// Exemplo: "25 AGO 2026". chrono::format usa locale inglês por padrão,
// então o mês é mapeado manualmente em vez de usar "%b".
fn formatar_data_artigo(dt: chrono::DateTime<chrono::Utc>) -> String {
    let mes = match dt.format("%m").to_string().as_str() {
        "01" => "JAN",
        "02" => "FEV",
        "03" => "MAR",
        "04" => "ABR",
        "05" => "MAI",
        "06" => "JUN",
        "07" => "JUL",
        "08" => "AGO",
        "09" => "SET",
        "10" => "OUT",
        "11" => "NOV",
        _ => "DEZ",
    };
    format!("{} {} {}", dt.format("%d"), mes, dt.format("%Y"))
}

// Escapa os 5 caracteres especiais do XML.
// Necessário para títulos e descrições que possam conter < > & ' "
fn escapar_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}