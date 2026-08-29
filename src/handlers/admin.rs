use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tera::Context;
use tower_sessions::Session;

use crate::csrf::gerar_token;
use crate::error::Result;
use crate::{
    models::{
        AlterarSenha, AlterarSenhaPerfil, ArtigoAvaliadoView, ConfiguracoesGeraisForm,
        EditarArtigo, EditarPerfil, EditarUsuario, MenuItemForm, NovaCategoria, NovaTag,
        NovoAlbum, NovoArtigo, NovoUsuario, PaginaForm, Paginacao, SalvarMenuPayload,
    },
    repositories::{
        artigos::ArtigosRepo,
        avaliacoes::AvaliacoesRepo,
        busca::BuscaRepo,
        categorias::{CategoriasRepo, TagsRepo},
        comentarios::ComentariosRepo,
        configuracoes::ConfiguracoesRepo,
        galeria::GaleriaRepo,
        menus::MenusRepo,
        page_views::PageViewsRepo,
        paginas::PaginasRepo,
        usuarios::UsuariosRepo,
    },
    services::auth::{hash_senha, verificar_senha, AuthService},
    state::AppState,
};
use axum::extract::Json;
use serde_json::json;

const SESSION_USER_ID: &str = "usuario_id";

// ─── HELPERS ─────────────────────────────────────────────

async fn usuario_nome_e_papel(state: &AppState, session: &Session) -> (String, String, String) {
    let id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    match UsuariosRepo::novo(&state.db).buscar_por_id(&id).await {
        Ok(Some(u)) => (u.nome, u.papel, u.id),
        _ => ("Administrador".into(), "visualizador".into(), String::new()),
    }
}

fn ctx_base(
    state: &AppState,
    nome: &str,
    papel: &str,
    uid: &str,
    pagina: &str,
    csrf: &str,
) -> Context {
    let mut ctx = Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("usuario_nome", nome);
    ctx.insert("usuario_papel", papel);
    ctx.insert("usuario_id", uid);
    ctx.insert("pagina_ativa", pagina);
    ctx.insert("total_pendentes_global", &0i64);
    ctx.insert("csrf_token", csrf);
    ctx.insert("tema", &state.config.tema);
    ctx
}

// ─── DASHBOARD ───────────────────────────────────────────

pub async fn dashboard(State(state): State<AppState>, session: Session) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;
    let artigos_repo = ArtigosRepo::novo(&state.db);

    let todos = artigos_repo.listar_todos().await.unwrap_or_default();
    let total_artigos = todos.len();
    let total_publicados = todos.iter().filter(|a| a.status == "publicado").count();
    let total_rascunhos = todos.iter().filter(|a| a.status == "rascunho").count();
    let artigos_recentes: Vec<_> = todos.into_iter().take(5).collect();
    let total_usuarios = UsuariosRepo::novo(&state.db).total().await.unwrap_or(0);
    let total_views = PageViewsRepo::novo(&state.db)
        .total_geral()
        .await
        .unwrap_or(0);
    let mais_visitadas = PageViewsRepo::novo(&state.db)
        .mais_visitadas(state.config.views_no_dashboard)
        .await
        .unwrap_or_default();
    let views_paginas = PageViewsRepo::novo(&state.db)
        .mais_visitadas_paginas(state.config.views_no_dashboard)
        .await
        .unwrap_or_default();
    let total_pendentes = ComentariosRepo::novo(&state.db)
        .total_pendentes()
        .await
        .unwrap_or(0);

    let mais_avaliados: Vec<ArtigoAvaliadoView> = AvaliacoesRepo::novo(&state.db)
        .mais_bem_avaliados(5, 1)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(ArtigoAvaliadoView::from)
        .collect();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "dashboard", &csrf);
    ctx.insert("total_artigos", &total_artigos);
    ctx.insert("total_publicados", &total_publicados);
    ctx.insert("total_rascunhos", &total_rascunhos);
    ctx.insert("total_usuarios", &total_usuarios);
    ctx.insert("total_views", &total_views);
    ctx.insert("mais_visitadas", &mais_visitadas);
    ctx.insert("views_paginas", &views_paginas);
    ctx.insert("artigos_recentes", &artigos_recentes);
    ctx.insert("total_pendentes_global", &total_pendentes);
    ctx.insert("mais_avaliados", &mais_avaliados);

    Ok(Html(state.tera.render("admin/dashboard.html", &ctx)?))
}

pub async fn pagina_views(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryPagina>,
) -> Result<Html<String>> {
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = state.config.artigos_por_pagina;
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let (views, total) = PageViewsRepo::novo(&state.db)
        .listar_todas(pagina, por_pagina)
        .await
        .unwrap_or_default();

    let total_geral = PageViewsRepo::novo(&state.db)
        .total_geral()
        .await
        .unwrap_or(0);
    let paginacao = Paginacao::calcular(pagina, total, por_pagina);

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "views", &csrf);
    ctx.insert("views", &views);
    ctx.insert("total_geral", &total_geral);
    ctx.insert("paginacao", &paginacao);

    Ok(Html(state.tera.render("admin/views.html", &ctx)?))
}

// ─── LISTAGEM DE ARTIGOS ─────────────────────────────────

#[derive(Deserialize)]
pub struct QueryPagina {
    pub pagina: Option<i64>,
}

#[derive(Deserialize)]
pub struct QueryArtigos {
    pub pagina: Option<i64>,
    pub status: Option<String>,
    pub categoria_id: Option<String>,
    pub busca: Option<String>,
    pub ordenar: Option<String>,
}

pub async fn listar_artigos(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryArtigos>,
) -> Result<Html<String>> {
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = state.config.artigos_por_pagina;
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    // Normaliza filtros: strings vazias viram None para não gerar WHERE desnecessário
    let filtro_status = query.status.filter(|s| !s.is_empty());
    let filtro_categoria = query.categoria_id.filter(|s| !s.is_empty());
    let filtro_busca = query.busca.filter(|s| !s.is_empty());
    let filtro_ordenar = query.ordenar.filter(|s| !s.is_empty());

    let filtros = crate::repositories::artigos::FiltrosArtigos {
        status: filtro_status.clone(),
        categoria_id: filtro_categoria.clone(),
        busca: filtro_busca.clone(),
        ordenar: filtro_ordenar.clone(),
        pagina,
        por_pagina,
    };

    let (artigos, total) = ArtigosRepo::novo(&state.db)
        .listar_filtrados(filtros)
        .await
        .unwrap_or_default();

    let paginacao = Paginacao::calcular(pagina, total, por_pagina);

    // Carrega categorias para popular o select de filtro
    let categorias = CategoriasRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "artigos", &csrf);
    ctx.insert("artigos", &artigos);
    ctx.insert("total_artigos", &total);
    ctx.insert("paginacao", &paginacao);
    ctx.insert("categorias", &categorias);
    // Reinjeta filtros ativos para o template manter o estado dos selects
    ctx.insert("filtro_status", &filtro_status);
    ctx.insert("filtro_categoria", &filtro_categoria);
    ctx.insert("filtro_busca", &filtro_busca);
    ctx.insert("filtro_ordenar", &filtro_ordenar);

    Ok(Html(state.tera.render("admin/artigos.html", &ctx)?))
}

// ─── CRIAÇÃO DE ARTIGO ───────────────────────────────────

pub async fn form_novo_artigo(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;
    let categorias = CategoriasRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();
    let tags = TagsRepo::novo(&state.db).listar().await.unwrap_or_default();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "novo_artigo", &csrf);
    ctx.insert("form_titulo", "Novo Artigo");
    ctx.insert("form_action", "/admin/artigos");
    ctx.insert("form_botao", "Publicar");
    ctx.insert("status", "rascunho");
    ctx.insert("artigo_id", &Option::<String>::None);
    ctx.insert("autor_id", &Option::<String>::None);
    ctx.insert("categoria_id", &Option::<String>::None);
    ctx.insert("resumo", &Option::<String>::None);
    ctx.insert("imagem_capa", &Option::<String>::None);
    ctx.insert("titulo_seo", &Option::<String>::None);
    ctx.insert("categorias", &categorias);
    ctx.insert("tags_disponiveis", &tags);
    ctx.insert("tags_selecionadas", &Vec::<String>::new());
    ctx.insert("notificar_comentarios", &state.config.notif_padrao);
    ctx.insert("restrito", &false);
    ctx.insert("erro", &Option::<String>::None);

    Ok(Html(state.tera.render("admin/artigo_form.html", &ctx)?))
}

pub async fn criar_artigo(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<NovoArtigo>,
) -> Result<impl IntoResponse> {
    let autor_id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    match ArtigosRepo::novo(&state.db)
        .criar(dados.clone(), &autor_id)
        .await
    {
        Ok(artigo) => {
            if let Some(tags_str) = &dados.tags {
                let tag_ids: Vec<&str> = tags_str.split(',').filter(|s| !s.is_empty()).collect();
                TagsRepo::novo(&state.db)
                    .sincronizar_tags(&artigo.id, &tag_ids)
                    .await
                    .ok();
            }
            tracing::info!(
                usuario_id = %autor_id,
                artigo_id = %artigo.id,
                titulo = %artigo.titulo,
                "Artigo criado"
            );
            Ok(Redirect::to("/admin/artigos").into_response())
        }
        Err(e) => {
            tracing::error!(usuario_id = %autor_id, erro = %e, "Falha ao criar artigo");
            let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
            let csrf = gerar_token(&session).await;
            let categorias = CategoriasRepo::novo(&state.db)
                .listar()
                .await
                .unwrap_or_default();
            let tags = TagsRepo::novo(&state.db).listar().await.unwrap_or_default();

            let mut ctx = ctx_base(&state, &nome, &papel, &uid, "novo_artigo", &csrf);
            ctx.insert("form_titulo", "Novo Artigo");
            ctx.insert("form_action", "/admin/artigos");
            ctx.insert("form_botao", "Publicar");
            ctx.insert("status", "rascunho");
            ctx.insert("artigo_id", &Option::<String>::None);
            ctx.insert("autor_id", &Option::<String>::None);
            ctx.insert("categoria_id", &Option::<String>::None);
            ctx.insert("resumo", &Option::<String>::None);
            ctx.insert("imagem_capa", &Option::<String>::None);
            ctx.insert("titulo_seo", &Option::<String>::None);
            ctx.insert("categorias", &categorias);
            ctx.insert("tags_disponiveis", &tags);
            ctx.insert("tags_selecionadas", &Vec::<String>::new());
            ctx.insert(
                "notificar_comentarios",
                &dados.notificar_comentarios.is_some(),
            );
            ctx.insert("restrito", &dados.restrito.is_some());
            ctx.insert("erro", &Some(e.to_string()));
            Ok(Html(state.tera.render("admin/artigo_form.html", &ctx)?).into_response())
        }
    }
}

// ─── EDIÇÃO DE ARTIGO ────────────────────────────────────

pub async fn form_editar_artigo(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    match ArtigosRepo::novo(&state.db).buscar_por_id(&id).await {
        Ok(artigo) => {
            let categorias = CategoriasRepo::novo(&state.db)
                .listar()
                .await
                .unwrap_or_default();
            let tags_disponiveis = TagsRepo::novo(&state.db).listar().await.unwrap_or_default();
            let tags_artigo = TagsRepo::novo(&state.db)
                .tags_do_artigo(&id)
                .await
                .unwrap_or_default();
            let tags_selecionadas: Vec<String> = tags_artigo.iter().map(|t| t.id.clone()).collect();

            let mut ctx = ctx_base(&state, &nome, &papel, &uid, "artigos", &csrf);
            ctx.insert("form_titulo", "Editar Artigo");
            ctx.insert("form_action", &format!("/admin/artigos/{}/editar", id));
            ctx.insert("form_botao", "Salvar alterações");
            ctx.insert("artigo_id", &Some(&id));
            ctx.insert("autor_id", &artigo.autor_id);
            ctx.insert("titulo", &artigo.titulo);
            ctx.insert("corpo", &artigo.corpo);
            ctx.insert("resumo", &artigo.resumo);
            ctx.insert("imagem_capa", &artigo.imagem_capa);
            ctx.insert("titulo_seo", &artigo.titulo_seo);
            ctx.insert("status", &artigo.status);
            ctx.insert("categoria_id", &artigo.categoria_id);
            ctx.insert("categorias", &categorias);
            ctx.insert("tags_disponiveis", &tags_disponiveis);
            ctx.insert("tags_selecionadas", &tags_selecionadas);
            ctx.insert("comentarios_habilitados", &(artigo.comentarios_habilitados));
            ctx.insert("moderacao_habilitada", &(artigo.moderacao_habilitada));
            ctx.insert("avaliacoes_habilitadas", &(artigo.avaliacoes_habilitadas));
            ctx.insert("notificar_comentarios", &(artigo.notificar_comentarios));
            ctx.insert("restrito", &(artigo.restrito));
            ctx.insert("erro", &Option::<String>::None);
            Ok(Html(state.tera.render("admin/artigo_form.html", &ctx)?).into_response())
        }
        Err(_) => Ok(Redirect::to("/admin/artigos").into_response()),
    }
}

pub async fn editar_artigo(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Form(dados): Form<EditarArtigo>,
) -> Result<impl IntoResponse> {
    match ArtigosRepo::novo(&state.db)
        .editar(&id, dados.clone())
        .await
    {
        Ok(_) => {
            let tag_ids_str = dados.tags.unwrap_or_default();
            let tag_ids: Vec<&str> = tag_ids_str.split(',').filter(|s| !s.is_empty()).collect();
            TagsRepo::novo(&state.db)
                .sincronizar_tags(&id, &tag_ids)
                .await
                .ok();
            tracing::info!(artigo_id = %id, "Artigo editado");
            Ok(Redirect::to("/admin/artigos").into_response())
        }
        Err(e) => {
            tracing::error!(artigo_id = %id, erro = %e, "Falha ao editar artigo");
            let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
            let csrf = gerar_token(&session).await;
            let categorias = CategoriasRepo::novo(&state.db)
                .listar()
                .await
                .unwrap_or_default();
            let tags_disponiveis = TagsRepo::novo(&state.db).listar().await.unwrap_or_default();

            let (titulo, corpo, resumo, imagem_capa, titulo_seo, status, categoria_id, autor_id) =
                match ArtigosRepo::novo(&state.db).buscar_por_id(&id).await {
                    Ok(a) => (
                        a.titulo,
                        a.corpo,
                        a.resumo,
                        a.imagem_capa,
                        a.titulo_seo,
                        a.status,
                        a.categoria_id,
                        a.autor_id,
                    ),
                    Err(_) => (
                        String::new(),
                        String::new(),
                        None,
                        None,
                        None,
                        "rascunho".into(),
                        None,
                        String::new(),
                    ),
                };

            let mut ctx = ctx_base(&state, &nome, &papel, &uid, "artigos", &csrf);
            ctx.insert("form_titulo", "Editar Artigo");
            ctx.insert("form_action", &format!("/admin/artigos/{}/editar", id));
            ctx.insert("form_botao", "Salvar alterações");
            ctx.insert("artigo_id", &Some(&id));
            ctx.insert("autor_id", &autor_id);
            ctx.insert("titulo", &titulo);
            ctx.insert("corpo", &corpo);
            ctx.insert("resumo", &resumo);
            ctx.insert("imagem_capa", &imagem_capa);
            ctx.insert("titulo_seo", &titulo_seo);
            ctx.insert("status", &status);
            ctx.insert("categoria_id", &categoria_id);
            ctx.insert("categorias", &categorias);
            ctx.insert("tags_disponiveis", &tags_disponiveis);
            ctx.insert("tags_selecionadas", &Vec::<String>::new());
            ctx.insert("restrito", &dados.restrito.is_some());
            ctx.insert("erro", &Some(e.to_string()));
            Ok(Html(state.tera.render("admin/artigo_form.html", &ctx)?).into_response())
        }
    }
}

// ─── DELEÇÃO DE ARTIGO ───────────────────────────────────

pub async fn deletar_artigo(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let usuario_id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    let (_, papel, _) = usuario_nome_e_papel(&state, &session).await;

    if papel == "admin" {
        ArtigosRepo::novo(&state.db).deletar(&id).await.ok();
        tracing::info!(usuario_id = %usuario_id, artigo_id = %id, papel = "admin", "Artigo deletado");
    } else if papel == "editor" {
        if let Ok(autor_id) = ArtigosRepo::novo(&state.db).buscar_autor(&id).await {
            if autor_id == usuario_id {
                ArtigosRepo::novo(&state.db).deletar(&id).await.ok();
                tracing::info!(usuario_id = %usuario_id, artigo_id = %id, papel = "editor", "Artigo deletado pelo autor");
            }
        }
    }

    Redirect::to("/admin/artigos")
}

// ─── GALERIA ─────────────────────────────────────────────

pub async fn listar_galeria(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;
    let albuns = GaleriaRepo::novo(&state.db)
        .listar_albuns_com_capa()
        .await
        .unwrap_or_default();
    let total = albuns.len();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "galeria", &csrf);
    ctx.insert("albuns", &albuns);
    ctx.insert("total_albuns", &total);

    Ok(Html(state.tera.render("admin/galeria.html", &ctx)?))
}

pub async fn criar_album(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<NovoAlbum>,
) -> impl IntoResponse {
    let criado_por = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    GaleriaRepo::novo(&state.db)
        .criar_album(dados, &criado_por)
        .await
        .ok();
    tracing::info!(usuario_id = %criado_por, "Álbum criado");
    Redirect::to("/admin/galeria")
}

pub async fn ver_album(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;
    let repo = GaleriaRepo::novo(&state.db);

    match repo.buscar_album(&id).await {
        Ok(album) => {
            let fotos = repo.listar_fotos_do_album(&id).await.unwrap_or_default();
            let total = fotos.len();

            let mut ctx = ctx_base(&state, &nome, &papel, &uid, "galeria", &csrf);
            ctx.insert("album", &album);
            ctx.insert("fotos", &fotos);
            ctx.insert("total_fotos", &total);

            Ok(Html(state.tera.render("admin/album.html", &ctx)?).into_response())
        }
        Err(_) => Ok(Redirect::to("/admin/galeria").into_response()),
    }
}

pub async fn deletar_album(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let usuario_id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();
    let (_, papel, _) = usuario_nome_e_papel(&state, &session).await;

    if papel == "admin" {
        GaleriaRepo::novo(&state.db).deletar_album(&id).await.ok();
        tracing::info!(usuario_id = %usuario_id, album_id = %id, papel = "admin", "Álbum deletado");
    } else if papel == "editor" {
        if let Ok(album) = GaleriaRepo::novo(&state.db).buscar_album(&id).await {
            if album.criado_por.as_deref() == Some(&usuario_id) {
                GaleriaRepo::novo(&state.db).deletar_album(&id).await.ok();
                tracing::info!(usuario_id = %usuario_id, album_id = %id, papel = "editor", "Álbum deletado pelo autor");
            }
        }
    }

    Redirect::to("/admin/galeria")
}

pub async fn deletar_foto(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let usuario_id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();
    let (_, papel, _) = usuario_nome_e_papel(&state, &session).await;

    let repo = GaleriaRepo::novo(&state.db);

    let pode_deletar = if papel == "admin" {
        true
    } else if papel == "editor" {
        match sqlx::query_scalar!("SELECT criado_por FROM fotos WHERE id = $1", id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .flatten()
        {
            Some(criado_por) => criado_por == usuario_id,
            None => false,
        }
    } else {
        false
    };

    if pode_deletar {
        if let Ok(Some(url)) = repo.deletar_foto(&id).await {
            let caminho = url.trim_start_matches('/');
            tracing::info!(usuario_id = %usuario_id, foto_id = %id, caminho = %caminho, "Foto deletada");
            tokio::fs::remove_file(caminho).await.ok();
        }
    }

    Redirect::to("/admin/galeria")
}

// ─── TAXONOMIAS ──────────────────────────────────────────

pub async fn pagina_taxonomias(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;
    let categorias = CategoriasRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();
    let tags = TagsRepo::novo(&state.db).listar().await.unwrap_or_default();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "taxonomias", &csrf);
    ctx.insert("categorias", &categorias);
    ctx.insert("tags", &tags);

    Ok(Html(state.tera.render("admin/taxonomias.html", &ctx)?))
}

pub async fn criar_categoria(
    State(state): State<AppState>,
    Form(dados): Form<NovaCategoria>,
) -> impl IntoResponse {
    CategoriasRepo::novo(&state.db).criar(dados).await.ok();
    Redirect::to("/admin/taxonomias")
}

pub async fn deletar_categoria(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    CategoriasRepo::novo(&state.db).deletar(&id).await.ok();
    tracing::info!(categoria_id = %id, "Categoria deletada");
    Redirect::to("/admin/taxonomias")
}

pub async fn criar_tag(
    State(state): State<AppState>,
    Form(dados): Form<NovaTag>,
) -> impl IntoResponse {
    TagsRepo::novo(&state.db).criar(dados).await.ok();
    Redirect::to("/admin/taxonomias")
}

pub async fn deletar_tag(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    TagsRepo::novo(&state.db).deletar(&id).await.ok();
    tracing::info!(tag_id = %id, "Tag deletada");
    Redirect::to("/admin/taxonomias")
}

// ─── USUÁRIOS ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct QueryUsuarios {
    pub erro: Option<String>,
}

pub async fn listar_usuarios(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryUsuarios>,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;
    let usuario_id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();
    let usuarios = UsuariosRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();
    let total = usuarios.len();

    // Mensagem de erro vinda do redirect
    let erro_delete: Option<&str> = match query.erro.as_deref() {
        Some("usuario_com_conteudo") => Some(
            "Este usuário possui artigos publicados e não pode ser deletado. \
             Reatribua ou delete o conteúdo antes.",
        ),
        _ => None,
    };

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "usuarios", &csrf);
    ctx.insert("usuarios", &usuarios);
    ctx.insert("total", &total);
    ctx.insert("usuario_id", &usuario_id);
    ctx.insert("erro", &erro_delete.map(|s| s.to_string()));

    Ok(Html(state.tera.render("admin/usuarios.html", &ctx)?))
}

pub async fn criar_usuario(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<NovoUsuario>,
) -> Result<impl IntoResponse> {
    let service = AuthService::novo(&state.db);
    match service
        .criar_usuario(&dados.nome, &dados.email, Some(&dados.senha), &dados.papel)
        .await
    {
        Ok(_) => {
            tracing::info!(email = %dados.email, papel = %dados.papel, "Usuário criado");
            Ok(Redirect::to("/admin/usuarios").into_response())
        }
        Err(e) => {
            tracing::warn!(email = %dados.email, erro = %e, "Falha ao criar usuário");
            let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
            let csrf = gerar_token(&session).await;
            let usuario_id = session
                .get::<String>(SESSION_USER_ID)
                .await
                .unwrap_or(None)
                .unwrap_or_default();
            let usuarios = UsuariosRepo::novo(&state.db)
                .listar()
                .await
                .unwrap_or_default();
            let total = usuarios.len();

            let mut ctx = ctx_base(&state, &nome, &papel, &uid, "usuarios", &csrf);
            ctx.insert("usuarios", &usuarios);
            ctx.insert("total", &total);
            ctx.insert("usuario_id", &usuario_id);
            ctx.insert("erro", &Some(e.to_string()));
            Ok(Html(state.tera.render("admin/usuarios.html", &ctx)?).into_response())
        }
    }
}

pub async fn form_editar_usuario(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    match UsuariosRepo::novo(&state.db).buscar_por_id(&id).await {
        Ok(Some(usuario)) => {
            let mut ctx = ctx_base(&state, &nome, &papel, &uid, "usuarios", &csrf);
            ctx.insert("usuario", &usuario);
            ctx.insert("erro", &Option::<String>::None);
            Ok(Html(state.tera.render("admin/usuario_editar.html", &ctx)?).into_response())
        }
        _ => Ok(Redirect::to("/admin/usuarios").into_response()),
    }
}

pub async fn editar_usuario(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Form(dados): Form<EditarUsuario>,
) -> Result<impl IntoResponse> {
    match UsuariosRepo::novo(&state.db)
        .atualizar(&id, &dados.nome, &dados.email, &dados.papel)
        .await
    {
        Ok(_) => {
            tracing::info!(usuario_id = %id, "Usuário editado");
            Ok(Redirect::to("/admin/usuarios").into_response())
        }
        Err(e) => {
            tracing::error!(usuario_id = %id, erro = %e, "Falha ao editar usuário");
            let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
            let csrf = gerar_token(&session).await;
            match UsuariosRepo::novo(&state.db).buscar_por_id(&id).await {
                Ok(Some(usuario)) => {
                    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "usuarios", &csrf);
                    ctx.insert("usuario", &usuario);
                    ctx.insert("erro", &Some(e.to_string()));
                    Ok(Html(state.tera.render("admin/usuario_editar.html", &ctx)?).into_response())
                }
                _ => Ok(Redirect::to("/admin/usuarios").into_response()),
            }
        }
    }
}

pub async fn alterar_senha_usuario(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Form(dados): Form<AlterarSenha>,
) -> Result<impl IntoResponse> {
    if dados.senha_nova != dados.senha_confirm {
        let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
        let csrf = gerar_token(&session).await;
        if let Ok(Some(usuario)) = UsuariosRepo::novo(&state.db).buscar_por_id(&id).await {
            let mut ctx = ctx_base(&state, &nome, &papel, &uid, "usuarios", &csrf);
            ctx.insert("usuario", &usuario);
            ctx.insert("erro", &Some("As senhas não conferem."));
            return Ok(Html(state.tera.render("admin/usuario_editar.html", &ctx)?).into_response());
        }
    }

    match hash_senha(&dados.senha_nova) {
        Ok(hash) => {
            UsuariosRepo::novo(&state.db)
                .atualizar_senha(&id, &hash)
                .await
                .ok();
            tracing::info!(usuario_id = %id, "Senha de usuário alterada pelo admin");
            Ok(Redirect::to("/admin/usuarios").into_response())
        }
        Err(e) => {
            let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
            let csrf = gerar_token(&session).await;
            if let Ok(Some(usuario)) = UsuariosRepo::novo(&state.db).buscar_por_id(&id).await {
                let mut ctx = ctx_base(&state, &nome, &papel, &uid, "usuarios", &csrf);
                ctx.insert("usuario", &usuario);
                ctx.insert("erro", &Some(e.to_string()));
                return Ok(
                    Html(state.tera.render("admin/usuario_editar.html", &ctx)?).into_response()
                );
            }
            Ok(Redirect::to("/admin/usuarios").into_response())
        }
    }
}

pub async fn deletar_usuario(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let session_id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    // Não permite deletar a si mesmo
    if id == session_id {
        return Redirect::to("/admin/usuarios").into_response();
    }

    // Verifica se o usuário tem artigos vinculados
    let total_artigos = sqlx::query_scalar!("SELECT COUNT(*) FROM artigos WHERE autor_id = $1", id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    if total_artigos > 0 {
        // Redireciona com erro em vez de silenciar
        tracing::warn!(
            usuario_id = %id,
            total_artigos = %total_artigos,
            "Tentativa de deletar usuário com conteúdo vinculado"
        );
        return Redirect::to("/admin/usuarios?erro=usuario_com_conteudo").into_response();
    }

    match UsuariosRepo::novo(&state.db).deletar(&id).await {
        Ok(_) => tracing::info!(usuario_id = %id, "Usuário deletado"),
        Err(e) => tracing::error!(usuario_id = %id, erro = %e, "Falha ao deletar usuário"),
    }

    Redirect::to("/admin/usuarios").into_response()
}

pub async fn gerar_token_usuario(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    UsuariosRepo::novo(&state.db).gerar_token(&id).await.ok();
    tracing::info!(usuario_id = %id, "Token de API gerado");
    Redirect::to(&format!("/admin/usuarios/{}/editar", id))
}

pub async fn revogar_token_usuario(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    UsuariosRepo::novo(&state.db).revogar_token(&id).await.ok();
    tracing::info!(usuario_id = %id, "Token de API revogado");
    Redirect::to(&format!("/admin/usuarios/{}/editar", id))
}

// ─── PERFIL ──────────────────────────────────────────────

pub async fn pagina_perfil(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    match UsuariosRepo::novo(&state.db).buscar_por_id(&uid).await {
        Ok(Some(usuario)) => {
            let mut ctx = ctx_base(&state, &nome, &papel, &uid, "perfil", &csrf);
            ctx.insert("usuario", &usuario);
            ctx.insert("erro_dados", &Option::<String>::None);
            ctx.insert("sucesso_dados", &Option::<String>::None);
            ctx.insert("erro_senha", &Option::<String>::None);
            ctx.insert("sucesso_senha", &Option::<String>::None);
            Ok(Html(state.tera.render("admin/perfil.html", &ctx)?).into_response())
        }
        _ => Ok(Redirect::to("/admin").into_response()),
    }
}

pub async fn salvar_perfil(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<EditarPerfil>,
) -> Result<impl IntoResponse> {
    let (_nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let resultado = UsuariosRepo::novo(&state.db)
        .atualizar(&uid, &dados.nome, &dados.email, &papel)
        .await;

    match UsuariosRepo::novo(&state.db).buscar_por_id(&uid).await {
        Ok(Some(usuario)) => {
            let mut ctx = ctx_base(&state, &dados.nome, &papel, &uid, "perfil", &csrf);
            ctx.insert("usuario", &usuario);
            ctx.insert("sucesso_senha", &Option::<String>::None);
            ctx.insert("erro_senha", &Option::<String>::None);
            match resultado {
                Ok(_) => {
                    ctx.insert("sucesso_dados", &Some("Dados atualizados com sucesso."));
                    ctx.insert("erro_dados", &Option::<String>::None);
                }
                Err(e) => {
                    ctx.insert("erro_dados", &Some(e.to_string()));
                    ctx.insert("sucesso_dados", &Option::<String>::None);
                }
            }
            Ok(Html(state.tera.render("admin/perfil.html", &ctx)?).into_response())
        }
        _ => Ok(Redirect::to("/admin").into_response()),
    }
}

pub async fn alterar_senha_perfil(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<AlterarSenhaPerfil>,
) -> Result<impl IntoResponse> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let usuario = match UsuariosRepo::novo(&state.db).buscar_por_id(&uid).await {
        Ok(Some(u)) => u,
        _ => return Ok(Redirect::to("/admin").into_response()),
    };

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "perfil", &csrf);
    ctx.insert("usuario", &usuario);
    ctx.insert("erro_dados", &Option::<String>::None);
    ctx.insert("sucesso_dados", &Option::<String>::None);

    // senha_hash é Option<String> — usuários OIDC-only não têm senha local.
    let hash_atual = match usuario.senha_hash.as_deref() {
        Some(h) => h,
        None => {
            ctx.insert(
                "erro_senha",
                &Some("Operação não disponível para este usuário."),
            );
            ctx.insert("sucesso_senha", &Option::<String>::None);
            return Ok(Html(state.tera.render("admin/perfil.html", &ctx)?).into_response());
        }
    };

    if verificar_senha(&dados.senha_atual, hash_atual).is_err() {
        ctx.insert("erro_senha", &Some("Senha atual incorreta."));
        ctx.insert("sucesso_senha", &Option::<String>::None);
        return Ok(Html(state.tera.render("admin/perfil.html", &ctx)?).into_response());
    }

    if dados.senha_nova != dados.senha_confirm {
        ctx.insert("erro_senha", &Some("As senhas não conferem."));
        ctx.insert("sucesso_senha", &Option::<String>::None);
        return Ok(Html(state.tera.render("admin/perfil.html", &ctx)?).into_response());
    }

    match hash_senha(&dados.senha_nova) {
        Ok(hash) => {
            UsuariosRepo::novo(&state.db)
                .atualizar_senha(&uid, &hash)
                .await
                .ok();
            tracing::info!(usuario_id = %uid, "Senha de perfil alterada");
            ctx.insert("sucesso_senha", &Some("Senha alterada com sucesso."));
            ctx.insert("erro_senha", &Option::<String>::None);
        }
        Err(e) => {
            ctx.insert("erro_senha", &Some(e.to_string()));
            ctx.insert("sucesso_senha", &Option::<String>::None);
        }
    }

    Ok(Html(state.tera.render("admin/perfil.html", &ctx)?).into_response())
}

// ─── COMENTÁRIOS ─────────────────────────────────────────

pub async fn listar_comentarios(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryPagina>,
) -> Result<Html<String>> {
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = state.config.artigos_por_pagina;
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let (comentarios, total) = ComentariosRepo::novo(&state.db)
        .listar_todos(pagina, por_pagina)
        .await
        .unwrap_or_default();

    let total_pendentes = ComentariosRepo::novo(&state.db)
        .total_pendentes()
        .await
        .unwrap_or(0);

    let paginacao = Paginacao::calcular(pagina, total, por_pagina);

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "comentarios", &csrf);
    ctx.insert("comentarios", &comentarios);
    ctx.insert("total_pendentes", &total_pendentes);
    ctx.insert("paginacao", &paginacao);

    Ok(Html(state.tera.render("admin/comentarios.html", &ctx)?))
}

pub async fn aprovar_comentario(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    ComentariosRepo::novo(&state.db).aprovar(&id).await.ok();
    tracing::info!(comentario_id = %id, "Comentário aprovado");
    Redirect::to("/admin/comentarios")
}

pub async fn rejeitar_comentario(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    ComentariosRepo::novo(&state.db).rejeitar(&id).await.ok();
    tracing::info!(comentario_id = %id, "Comentário rejeitado");
    Redirect::to("/admin/comentarios")
}

pub async fn deletar_comentario(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    ComentariosRepo::novo(&state.db).deletar(&id).await.ok();
    tracing::info!(comentario_id = %id, "Comentário deletado");
    Redirect::to("/admin/comentarios")
}

// ─── BUSCA ───────────────────────────────────────────────

pub async fn buscar_admin(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryBusca>,
) -> Result<Html<String>> {
    let termo = query.q.clone().unwrap_or_default();
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let resultados = if termo.len() >= 2 {
        BuscaRepo::novo(&state.db)
            .buscar_admin(&termo)
            .await
            .unwrap_or_default()
    } else {
        vec![]
    };

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "", &csrf);
    ctx.insert("termo", &termo);
    ctx.insert("resultados", &resultados);
    ctx.insert("total", &resultados.len());

    Ok(Html(state.tera.render("admin/busca.html", &ctx)?))
}

#[derive(Deserialize)]
pub struct QueryBusca {
    pub q: Option<String>,
}

// ─── PÁGINAS ESTÁTICAS ───────────────────────────────────

pub async fn listar_paginas(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let paginas = PaginasRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "paginas", &csrf);
    ctx.insert("paginas", &paginas);

    Ok(Html(state.tera.render("admin/paginas.html", &ctx)?))
}

pub async fn form_nova_pagina(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "paginas", &csrf);
    ctx.insert("form_titulo", "Nova Página");
    ctx.insert("form_action", "/admin/paginas");
    ctx.insert("form_botao", "Criar Página");
    ctx.insert("pagina_id", &Option::<String>::None);
    ctx.insert("titulo", &"");
    ctx.insert("corpo", &"");
    ctx.insert("publicada", &false);
    ctx.insert("ordem", &0i32);
    ctx.insert("titulo_seo", &Option::<String>::None);
    ctx.insert("html_bruto", &Option::<String>::None);
    ctx.insert("restrito", &false);
    ctx.insert("erro", &Option::<String>::None);

    Ok(Html(state.tera.render("admin/pagina_form.html", &ctx)?))
}

pub async fn criar_pagina(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<PaginaForm>,
) -> Result<impl IntoResponse> {
    let criado_por = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    let repo = PaginasRepo::novo(&state.db);
    let slug_base = gerar_slug_pagina(&dados.titulo);
    let slug = repo.slug_unico(&slug_base, None).await;
    let publicada = dados.publicada.is_some();
    let titulo_seo = dados.titulo_seo.as_deref().filter(|s| !s.is_empty());
    let html_bruto = dados.html_bruto.as_deref().filter(|s| !s.is_empty());
    let restrito = dados.restrito.is_some();

    repo.criar(
        &dados.titulo,
        &slug,
        &dados.corpo,
        publicada,
        dados.ordem,
        titulo_seo,
        &criado_por,
        html_bruto,
        restrito,
    )
    .await?;

    // Invalida o cache do menu para refletir a nova página (se publicada).

    tracing::info!(slug = %slug, criado_por = %criado_por, "Página estática criada");
    Ok(Redirect::to("/admin/paginas"))
}

pub async fn form_editar_pagina(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let pagina = PaginasRepo::novo(&state.db).buscar_por_id(&id).await?;

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "paginas", &csrf);
    ctx.insert("form_titulo", "Editar Página");
    ctx.insert("form_action", &format!("/admin/paginas/{}/editar", id));
    ctx.insert("form_botao", "Salvar");
    ctx.insert("pagina_id", &Some(&id));
    ctx.insert("titulo", &pagina.titulo);
    ctx.insert("corpo", &pagina.corpo);
    ctx.insert("publicada", &pagina.publicada);
    ctx.insert("ordem", &pagina.ordem);
    ctx.insert("titulo_seo", &pagina.titulo_seo);
    ctx.insert("html_bruto", &pagina.html_bruto);
    ctx.insert("restrito", &pagina.restrito);
    ctx.insert("erro", &Option::<String>::None);

    Ok(Html(state.tera.render("admin/pagina_form.html", &ctx)?))
}

pub async fn salvar_pagina(
    State(state): State<AppState>,
    _session: Session,
    Path(id): Path<String>,
    Form(dados): Form<PaginaForm>,
) -> Result<impl IntoResponse> {
    let repo = PaginasRepo::novo(&state.db);

    // Gera novo slug baseado no título, excluindo o próprio id para não colidir consigo mesmo.
    let slug_base = gerar_slug_pagina(&dados.titulo);
    let slug = repo.slug_unico(&slug_base, Some(&id)).await;
    let publicada = dados.publicada.is_some();
    let titulo_seo = dados.titulo_seo.as_deref().filter(|s| !s.is_empty());
    let html_bruto = dados.html_bruto.as_deref().filter(|s| !s.is_empty());
    let restrito = dados.restrito.is_some();

    repo.atualizar(
        &id,
        &dados.titulo,
        &slug,
        &dados.corpo,
        publicada,
        dados.ordem,
        titulo_seo,
        html_bruto,
        restrito,
    )
    .await?;

    // Invalida o cache — publicada pode ter mudado.

    tracing::info!(pagina_id = %id, "Página estática atualizada");
    Ok(Redirect::to("/admin/paginas"))
}

pub async fn deletar_pagina(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let repo = PaginasRepo::novo(&state.db);
    repo.deletar(&id).await.ok();

    // Invalida o cache após deleção.

    tracing::info!(pagina_id = %id, "Página estática deletada");
    Redirect::to("/admin/paginas")
}

// Mesma lógica de gerar_slug dos artigos — função local para não criar dependência cruzada.
fn gerar_slug_pagina(titulo: &str) -> String {
    titulo
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'a'..='z' | '0'..='9' => c,
            ' ' | '-' => '-',
            _ => '_',
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ─── MENU ─────────────────────────────────────────────────

pub async fn editor_menu(State(state): State<AppState>, session: Session) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;
    let repo = MenusRepo::novo(&state.db);

    let menu = repo.buscar_menu_principal().await?;
    let itens = repo.listar_itens(&menu.id).await.unwrap_or_default();

    // Busca os recursos disponíveis para adicionar ao menu
    let paginas = crate::repositories::paginas::PaginasRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();
    let categorias = crate::repositories::categorias::CategoriasRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();
    let tags = crate::repositories::categorias::TagsRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "menu", &csrf);
    ctx.insert("menu", &menu);
    ctx.insert("itens", &itens);
    ctx.insert("paginas", &paginas);
    ctx.insert("categorias", &categorias);
    ctx.insert("tags", &tags);

    Ok(Html(state.tera.render("admin/menu.html", &ctx)?))
}

pub async fn adicionar_item_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
    Form(dados): Form<MenuItemForm>,
) -> impl IntoResponse {
    MenusRepo::novo(&state.db)
        .adicionar_item(&menu_id, &dados)
        .await
        .ok();

    // Invalida o cache após adicionar item
    let arvore = MenusRepo::novo(&state.db)
        .arvore_menu_principal()
        .await
        .unwrap_or_default();
    state.atualizar_menu_cache(arvore).await;

    tracing::info!(menu_id = %menu_id, rotulo = %dados.rotulo, "Item de menu adicionado");
    Redirect::to("/admin/menu")
}

pub async fn deletar_item_menu(
    State(state): State<AppState>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    MenusRepo::novo(&state.db).deletar_item(&item_id).await.ok();

    // Invalida o cache após deletar item
    let arvore = MenusRepo::novo(&state.db)
        .arvore_menu_principal()
        .await
        .unwrap_or_default();
    state.atualizar_menu_cache(arvore).await;

    tracing::info!(item_id = %item_id, "Item de menu deletado");
    Redirect::to("/admin/menu")
}

// Recebe a árvore serializada pelo editor drag-and-drop como JSON.
// O JS serializa a ordem e o aninhamento após cada drag e envia via fetch.
pub async fn salvar_menu(
    State(state): State<AppState>,
    Path(menu_id): Path<String>,
    Json(payload): Json<SalvarMenuPayload>,
) -> impl IntoResponse {
    match MenusRepo::novo(&state.db)
        .salvar_arvore(&menu_id, &payload.itens)
        .await
    {
        Ok(_) => {
            // Invalida o cache com a nova ordem
            let arvore = MenusRepo::novo(&state.db)
                .arvore_menu_principal()
                .await
                .unwrap_or_default();
            state.atualizar_menu_cache(arvore).await;
            tracing::info!(menu_id = %menu_id, "Menu salvo");
            axum::http::StatusCode::OK
        }
        Err(e) => {
            tracing::error!(menu_id = %menu_id, erro = %e, "Falha ao salvar menu");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

// ─── BULK ACTIONS ────────────────────────────────────────

// serde_urlencoded (usado pelo Axum) não deserializa Vec quando há apenas
// um valor com chave "ids[]" — entrega string em vez de sequência.
// Solução: receber o body como String e parsear manualmente com form_urlencoded.
pub async fn bulk_artigos(
    State(state): State<AppState>,
    session: Session,
    axum::extract::RawForm(bytes): axum::extract::RawForm,
) -> Result<impl IntoResponse> {
    // RawForm extrai o body como bytes sem passar pelo serde_urlencoded,
    // que falha com Vec<String> quando há apenas 1 checkbox selecionado.
    // Parsing manual com split resolve o problema sem dependências extras.
    let body = String::from_utf8_lossy(&bytes);
    let mut ids: Vec<String> = Vec::new();
    let mut acao = String::new();

    for pair in body.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let k = k.trim();
            let v = urlencoding::decode(v)
                .unwrap_or(std::borrow::Cow::Borrowed(v))
                .into_owned();
            if k == "ids%5B%5D" || k == "ids[]" {
                ids.push(v);
            } else if k == "acao" {
                acao = v;
            }
        }
    }
    let acao = acao.as_str();

    if ids.is_empty() {
        return Ok(Redirect::to("/admin/artigos").into_response());
    }

    let (_, papel, uid) = usuario_nome_e_papel(&state, &session).await;

    let status = match acao {
        "publicar" => "publicado",
        "despublicar" => "rascunho",
        _ => return Ok(Redirect::to("/admin/artigos").into_response()),
    };

    let ids_permitidos: Vec<String> = if papel == "admin" {
        ids.clone()
    } else if papel == "editor" {
        // Editor só pode alterar artigos próprios
        let mut permitidos = Vec::new();
        for id in &ids {
            if let Ok(autor) = ArtigosRepo::novo(&state.db).buscar_autor(id).await {
                if autor == uid {
                    permitidos.push(id.clone());
                }
            }
        }
        permitidos
    } else {
        return Err(crate::error::AppError::NaoAutorizado);
    };

    if !ids_permitidos.is_empty() {
        let afetados = ArtigosRepo::novo(&state.db)
            .atualizar_status_bulk(&ids_permitidos, status)
            .await
            .unwrap_or(0);
        tracing::info!(
            usuario_id = %uid,
            papel = %papel,
            acao = %acao,
            total = %afetados,
            "Bulk action aplicada"
        );
    }

    Ok(Redirect::to("/admin/artigos").into_response())
}

// ─── TOGGLE DESTAQUE ─────────────────────────────────────

pub async fn toggle_destaque(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    // Só admin e editor podem alterar destaque
    let (_, papel, _) = usuario_nome_e_papel(&state, &session).await;
    if papel != "admin" && papel != "editor" {
        return Err(crate::error::AppError::NaoAutorizado);
    }

    let novo = ArtigosRepo::novo(&state.db).toggle_destaque(&id).await?;
    Ok(Json(json!({ "destaque": novo })))
}

// ─── CONFIGURAÇÕES ───────────────────────────────────────

pub async fn pagina_configuracoes(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let repo_config = ConfiguracoesRepo::novo(&state.db);
    let config_notif = repo_config.get_notificacoes().await.unwrap_or_default();
    let config_listagem = repo_config
        .get_listagem_restrita()
        .await
        .unwrap_or_default();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "configuracoes", &csrf);
    ctx.insert("config_notif", &config_notif);
    ctx.insert("config_listagem", &config_listagem);
    ctx.insert("smtp_configurado", &state.config.smtp.is_some());

    Ok(Html(state.tera.render("admin/configuracoes.html", &ctx)?))
}

pub async fn salvar_configuracoes(
    State(state): State<AppState>,
    Form(dados): Form<ConfiguracoesGeraisForm>,
) -> impl IntoResponse {
    let repo = ConfiguracoesRepo::novo(&state.db);

    let notif_ativa = dados.notif_ativa.is_some();
    let email_fallback = dados.notif_email_fallback.unwrap_or_default();
    let mostrar_restritos = dados.mostrar_artigos_restritos_listagem.is_some();

    if let Err(e) = repo
        .set("notif_ativa", if notif_ativa { "true" } else { "false" })
        .await
    {
        tracing::error!(erro = %e, "Falha ao salvar notif_ativa");
    }
    if let Err(e) = repo.set("notif_email_fallback", &email_fallback).await {
        tracing::error!(erro = %e, "Falha ao salvar notif_email_fallback");
    }
    if let Err(e) = repo
        .set(
            "mostrar_artigos_restritos_listagem",
            if mostrar_restritos { "true" } else { "false" },
        )
        .await
    {
        tracing::error!(erro = %e, "Falha ao salvar mostrar_artigos_restritos_listagem");
    }

    tracing::info!("Configurações gerais salvas");

    Redirect::to("/admin/configuracoes")
}