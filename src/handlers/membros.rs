use axum::{
    body::Body,
    extract::{Query, State},
    http::Response,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    csrf::gerar_token,
    error::AppError,
    models::{EditarMembroForm, LoginMembroForm, NovoMembroForm, PerfilMembroForm},
    repositories::{membros::MembrosRepo, usuarios::UsuariosRepo},
    services::{auth::hash_senha, membros::MembrosService},
    state::AppState,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct QueryNext {
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QuerySucesso {
    pub sucesso: Option<String>,
}

/// Valida o parâmetro `next` para evitar open redirect: só aceita paths
/// relativos começando com exatamente uma barra (`/algo`), nunca `//algo`
/// (que navegadores tratam como URL absoluta protocol-relative) nem URLs
/// com esquema (`http://`, `javascript:`, etc). Path inválido ou ausente
/// cai no destino padrão da área de membros.
fn next_seguro(next: Option<&str>) -> &str {
    match next {
        // Barra invertida é tratada como '/' pelos navegadores ao resolver
        // URLs (WHATWG URL spec) — "/\evil.com" normaliza para "//evil.com"
        // e vira redirect para domínio externo. Rejeitar '\' em qualquer
        // posição, não só no prefixo.
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.contains('\\') => n,
        _ => "/membros/area",
    }
}

fn ctx_membros(state: &AppState, titulo: &str) -> tera::Context {
    let mut ctx = tera::Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_descricao", &state.config.site_descricao);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("tema", &state.config.tema);
    ctx.insert("titulo", titulo);

    // providers SSO disponíveis — para exibir botões no login
    ctx.insert(
        "google_habilitado",
        &state.config.google_client_id.is_some(),
    );
    ctx.insert(
        "microsoft_habilitado",
        &state.config.microsoft_client_id.is_some(),
    );
    ctx.insert(
        "github_habilitado",
        &state.config.github_client_id.is_some(),
    );
    ctx.insert(
        "oidc_generico_habilitado",
        &state.config.oidc_client_id.is_some(),
    );
    ctx.insert("oidc_botao_label", &state.config.oidc_botao_label);

    ctx
}

// ─── Login ───────────────────────────────────────────────────────────────────

/// GET /membros/login
pub async fn form_login(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryNext>,
) -> Result<impl IntoResponse, AppError> {
    let destino = next_seguro(query.next.as_deref());

    // já está logado como membro → redireciona para o destino original
    let membro_id: Option<i64> = session.get("membro_id").await.unwrap_or(None);
    if membro_id.is_some() {
        return Ok(Redirect::to(destino).into_response());
    }

    // já está logado como usuário CMS → redireciona para o destino também
    let usuario_id: Option<String> = session.get("usuario_id").await.unwrap_or(None);
    if usuario_id.is_some() {
        return Ok(Redirect::to(destino).into_response());
    }

    let mut ctx = ctx_membros(&state, "Login — Área de Membros");
    ctx.insert("erro", &Option::<String>::None);
    ctx.insert("next", destino);

    let html = state
        .tera
        .render("membros/login.html", &ctx)
        .map_err(AppError::Template)?;

    Ok(Html(html).into_response())
}

/// POST /membros/login
pub async fn processar_login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginMembroForm>,
) -> Result<impl IntoResponse, AppError> {
    // Revalida o next vindo do form — nunca confiar em dado de entrada sem
    // checar de novo, mesmo que já tenha sido validado no GET que gerou o link.
    let destino = next_seguro(form.next.as_deref()).to_string();
    let repo = MembrosRepo::novo(&state.db);

    match MembrosService::login(&repo, &form.email, &form.senha).await {
        Ok(membro) => {
            session.insert("membro_id", membro.id).await.unwrap();
            Ok(Redirect::to(&destino).into_response())
        }
        Err(AppError::NaoAutorizado) | Err(AppError::Interno(_)) => {
            let mut ctx = ctx_membros(&state, "Login — Área de Membros");
            ctx.insert("erro", &Some("E-mail ou senha inválidos."));
            ctx.insert("next", &destino);

            let html = state
                .tera
                .render("membros/login.html", &ctx)
                .map_err(AppError::Template)?;

            Ok(Html(html).into_response())
        }
        Err(e) => Err(e),
    }
}

/// POST /membros/logout
pub async fn logout(session: Session) -> impl IntoResponse {
    session.remove::<i64>("membro_id").await.unwrap();
    Redirect::to("/membros/login")
}

// ─── Cadastro público ────────────────────────────────────────────────────────

/// GET /membros/cadastro
pub async fn form_cadastro(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QueryNext>,
) -> Result<impl IntoResponse, AppError> {
    let destino = next_seguro(query.next.as_deref());

    let membro_id: Option<i64> = session.get("membro_id").await.unwrap_or(None);
    if membro_id.is_some() {
        return Ok(Redirect::to(destino).into_response());
    }

    let csrf = gerar_token(&session).await;
    let mut ctx = ctx_membros(&state, "Cadastro — Área de Membros");
    ctx.insert("erro", &Option::<String>::None);
    ctx.insert("next", destino);
    ctx.insert("csrf_token", &csrf);

    let html = state
        .tera
        .render("membros/cadastro.html", &ctx)
        .map_err(AppError::Template)?;

    Ok(Html(html).into_response())
}

/// POST /membros/cadastro
pub async fn processar_cadastro(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<NovoMembroForm>,
) -> Result<impl IntoResponse, AppError> {
    let destino = next_seguro(form.next.as_deref()).to_string();

    let permitido = MembrosService::email_permitido(
        &form.email,
        state.config.membros_permitir_cadastro,
        state.config.membros_dominio_permitido.as_deref(),
    );

    if !permitido {
        let csrf = gerar_token(&session).await;
        let mut ctx = ctx_membros(&state, "Cadastro — Área de Membros");
        ctx.insert("erro", &Some("Cadastro não disponível para este e-mail."));
        ctx.insert("next", &destino);
        ctx.insert("csrf_token", &csrf);
        let html = state
            .tera
            .render("membros/cadastro.html", &ctx)
            .map_err(AppError::Template)?;
        return Ok(Html(html).into_response());
    }

    let repo = MembrosRepo::novo(&state.db);

    match MembrosService::cadastrar(&repo, form).await {
        Ok(membro) => {
            session.insert("membro_id", membro.id).await.unwrap();
            Ok(Redirect::to(&destino).into_response())
        }
        Err(AppError::Interno(msg)) => {
            let csrf = gerar_token(&session).await;
            let mut ctx = ctx_membros(&state, "Cadastro — Área de Membros");
            ctx.insert("erro", &Some(msg));
            ctx.insert("next", &destino);
            ctx.insert("csrf_token", &csrf);
            let html = state
                .tera
                .render("membros/cadastro.html", &ctx)
                .map_err(AppError::Template)?;
            Ok(Html(html).into_response())
        }
        Err(e) => Err(e),
    }
}

// ─── Área restrita ───────────────────────────────────────────────────────────

/// GET /membros/area  ← protegida pelo middleware requer_membro
pub async fn area(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let membro_id: Option<i64> = session.get("membro_id").await.unwrap_or(None);
    let usuario_id: Option<String> = session.get("usuario_id").await.unwrap_or(None);

    let nome_exibido = if let Some(id) = membro_id {
        let repo = MembrosRepo::novo(&state.db);
        repo.buscar_por_id(id)
            .await?
            .map(|m| m.nome)
            .unwrap_or_else(|| "Membro".into())
    } else if usuario_id.is_some() {
        session
            .get::<String>("usuario_nome")
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| "Usuário".into())
    } else {
        return Err(AppError::NaoAutorizado);
    };

    let mut ctx = ctx_membros(&state, "Área de Membros");
    ctx.insert("nome_exibido", &nome_exibido);

    let html = state
        .tera
        .render("membros/area.html", &ctx)
        .map_err(AppError::Template)?;

    Ok(Html(html))
}

// ─── Perfil do membro ────────────────────────────────────────────────────────

/// GET /membros/perfil  ← protegida pelo middleware requer_membro
pub async fn form_perfil(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<QuerySucesso>,
) -> Result<impl IntoResponse, AppError> {
    let membro_id: Option<i64> = session.get("membro_id").await.unwrap_or(None);
    let usuario_id: Option<String> = session.get("usuario_id").await.unwrap_or(None);

    let membro = if let Some(id) = membro_id {
        let repo = MembrosRepo::novo(&state.db);
        repo.buscar_por_id(id).await?.ok_or(AppError::NaoEncontrado)?
    } else if let Some(uid) = usuario_id {
        let _ = uid;
        return Err(AppError::NaoEncontrado);
    } else {
        return Err(AppError::NaoAutorizado);
    };

    let csrf = gerar_token(&session).await;
    let is_oauth = membro.oauth_provider.is_some();
    let sucesso = query.sucesso.as_deref() == Some("1");

    let mut ctx = ctx_membros(&state, "Meu Perfil");
    ctx.insert("membro", &membro);
    ctx.insert("is_oauth", &is_oauth);
    ctx.insert("csrf_token", &csrf);
    ctx.insert("sucesso", &sucesso);
    ctx.insert("erro", &Option::<String>::None);

    let html = state
        .tera
        .render("membros/perfil.html", &ctx)
        .map_err(AppError::Template)?;

    Ok(Html(html).into_response())
}

/// POST /membros/perfil  ← protegida pelo middleware requer_membro
pub async fn salvar_perfil(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<PerfilMembroForm>,
) -> Result<Response<Body>, AppError> {
    let membro_id: Option<i64> = session.get("membro_id").await.unwrap_or(None);
    let id = membro_id.ok_or(AppError::NaoAutorizado)?;

    let repo = MembrosRepo::novo(&state.db);
    let membro = repo.buscar_por_id(id).await?.ok_or(AppError::NaoEncontrado)?;
    let is_oauth = membro.oauth_provider.is_some();

    let nome = form.nome.trim().to_string();
    if nome.is_empty() {
        return render_perfil_erro(&state, &session, &membro, is_oauth, "Nome não pode ser vazio.").await;
    }

    if is_oauth {
        repo.atualizar_nome(id, &nome).await?;
    } else {
        let email = form.email.as_deref().unwrap_or("").trim().to_string();
        if email.is_empty() {
            return render_perfil_erro(&state, &session, &membro, is_oauth, "E-mail não pode ser vazio.").await;
        }

        match repo.atualizar_perfil(id, &nome, &email).await {
            Ok(_) => {}
            Err(AppError::Interno(msg)) => {
                return render_perfil_erro(&state, &session, &membro, is_oauth, &msg).await;
            }
            Err(e) => return Err(e),
        }

        let nova_senha = form.senha_nova.as_deref().unwrap_or("").trim().to_string();
        if !nova_senha.is_empty() {
            let confirm = form.senha_confirm.as_deref().unwrap_or("").trim().to_string();
            if nova_senha != confirm {
                return render_perfil_erro(&state, &session, &membro, is_oauth, "As senhas não coincidem.").await;
            }
            if nova_senha.len() < 8 {
                return render_perfil_erro(&state, &session, &membro, is_oauth, "A senha deve ter ao menos 8 caracteres.").await;
            }
            let hash = hash_senha(&nova_senha).map_err(|_| AppError::Interno("Erro ao processar senha.".into()))?;
            repo.atualizar_senha(id, &hash).await?;
        }
    }

    Ok(Redirect::to("/membros/perfil?sucesso=1").into_response())
}

async fn render_perfil_erro(
    state: &AppState,
    session: &Session,
    membro: &crate::models::Membro,
    is_oauth: bool,
    erro: &str,
) -> Result<Response<Body>, AppError> {
    let csrf = gerar_token(session).await;
    let mut ctx = ctx_membros(state, "Meu Perfil");
    ctx.insert("membro", membro);
    ctx.insert("is_oauth", &is_oauth);
    ctx.insert("csrf_token", &csrf);
    ctx.insert("sucesso", &false);
    ctx.insert("erro", &Some(erro.to_string()));

    let html = state
        .tera
        .render("membros/perfil.html", &ctx)
        .map_err(AppError::Template)?;

    Ok(Html(html).into_response())
}

// ─── Admin — gestão de membros ───────────────────────────────────────────────

/// GET /admin/membros
pub async fn admin_listar(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let repo = MembrosRepo::novo(&state.db);
    let membros = repo.listar().await?;

    let id = session
        .get::<String>("usuario_id")
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    let (nome, papel, uid) = match UsuariosRepo::novo(&state.db).buscar_por_id(&id).await {
        Ok(Some(u)) => (u.nome, u.papel, u.id),
        _ => ("Administrador".into(), "visualizador".into(), String::new()),
    };

    let csrf = gerar_token(&session).await;

    let mut ctx = tera::Context::new();
    ctx.insert("membros", &membros);
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("usuario_nome", &nome);
    ctx.insert("usuario_papel", &papel);
    ctx.insert("usuario_id", &uid);
    ctx.insert("pagina_ativa", "membros");
    ctx.insert("total_pendentes_global", &0i64);
    ctx.insert("csrf_token", &csrf);
    ctx.insert("tema", &state.config.tema);

    let html = state
        .tera
        .render("admin/membros.html", &ctx)
        .map_err(AppError::Template)?;

    Ok(Html(html))
}

/// POST /admin/membros/:id/ativar
pub async fn admin_ativar(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let repo = MembrosRepo::novo(&state.db);
    if let Some(membro) = repo.buscar_por_id(id).await? {
        repo.atualizar(id, &membro.nome, &membro.email, true)
            .await?;
    }
    Ok(Redirect::to("/admin/membros"))
}

/// POST /admin/membros/:id/desativar
pub async fn admin_desativar(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let repo = MembrosRepo::novo(&state.db);
    if let Some(membro) = repo.buscar_por_id(id).await? {
        repo.atualizar(id, &membro.nome, &membro.email, false)
            .await?;
    }
    Ok(Redirect::to("/admin/membros"))
}

/// POST /admin/membros/:id/deletar
pub async fn admin_deletar(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let repo = MembrosRepo::novo(&state.db);
    repo.deletar(id).await?;
    Ok(Redirect::to("/admin/membros"))
}

/// GET /admin/membros/{id}/editar
pub async fn admin_form_editar(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(query): Query<QuerySucesso>,
) -> Result<Html<String>, AppError> {
    let repo = MembrosRepo::novo(&state.db);
    let membro = repo.buscar_por_id(id).await?.ok_or(AppError::NaoEncontrado)?;

    let uid = session.get::<String>("usuario_id").await.unwrap_or(None).unwrap_or_default();
    let (nome, papel, usuario_id) = match UsuariosRepo::novo(&state.db).buscar_por_id(&uid).await {
        Ok(Some(u)) => (u.nome, u.papel, u.id),
        _ => ("Administrador".into(), "visualizador".into(), String::new()),
    };

    let csrf = gerar_token(&session).await;
    let sucesso = query.sucesso.as_deref() == Some("1");

    let mut ctx = tera::Context::new();
    ctx.insert("membro", &membro);
    ctx.insert("sucesso", &sucesso);
    ctx.insert("erro", &Option::<String>::None);
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("usuario_nome", &nome);
    ctx.insert("usuario_papel", &papel);
    ctx.insert("usuario_id", &usuario_id);
    ctx.insert("pagina_ativa", "membros");
    ctx.insert("total_pendentes_global", &0i64);
    ctx.insert("csrf_token", &csrf);
    ctx.insert("tema", &state.config.tema);

    let html = state.tera.render("admin/membros_editar.html", &ctx).map_err(AppError::Template)?;
    Ok(Html(html))
}

/// POST /admin/membros/{id}/editar
pub async fn admin_salvar_editar(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Form(form): Form<EditarMembroForm>,
) -> Result<impl IntoResponse, AppError> {
    let repo = MembrosRepo::novo(&state.db);
    let membro = repo.buscar_por_id(id).await?.ok_or(AppError::NaoEncontrado)?;

    let nome = form.nome.trim().to_string();
    let email = form.email.trim().to_string();
    let ativo = form.ativo.as_deref() == Some("on");

    if nome.is_empty() || email.is_empty() {
        let uid = session.get::<String>("usuario_id").await.unwrap_or(None).unwrap_or_default();
        let (unome, papel, usuario_id) = match UsuariosRepo::novo(&state.db).buscar_por_id(&uid).await {
            Ok(Some(u)) => (u.nome, u.papel, u.id),
            _ => ("Administrador".into(), "visualizador".into(), String::new()),
        };
        let csrf = gerar_token(&session).await;
        let mut ctx = tera::Context::new();
        ctx.insert("membro", &membro);
        ctx.insert("sucesso", &false);
        ctx.insert("erro", &Some("Nome e e-mail são obrigatórios."));
        ctx.insert("site_nome", &state.config.site_nome);
        ctx.insert("site_logo", &state.config.site_logo);
        ctx.insert("usuario_nome", &unome);
        ctx.insert("usuario_papel", &papel);
        ctx.insert("usuario_id", &usuario_id);
        ctx.insert("pagina_ativa", "membros");
        ctx.insert("total_pendentes_global", &0i64);
        ctx.insert("csrf_token", &csrf);
        ctx.insert("tema", &state.config.tema);
        let html = state.tera.render("admin/membros_editar.html", &ctx).map_err(AppError::Template)?;
        return Ok(Html(html).into_response());
    }

    repo.atualizar(id, &nome, &email, ativo).await?;
    Ok(Redirect::to(&format!("/admin/membros/{}/editar?sucesso=1", id)).into_response())
}