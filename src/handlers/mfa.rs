use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tera::Context;
use tower_sessions::Session;

use crate::{
    csrf::{gerar_token, validar_token},
    error::{AppError, Result},
    repositories::usuarios::UsuariosRepo,
    services::mfa,
    state::AppState,
};

// Chaves de sessão — mesmas usadas em auth.rs e middleware.rs
const SESSION_USER_ID: &str = "usuario_id";

// Chave usada durante o fluxo de login de duas etapas.
// Armazena o ID do usuário que passou na senha mas ainda não passou no MFA.
// Enquanto essa chave existir sem SESSION_USER_ID, o acesso ao admin é bloqueado.
const SESSION_MFA_PENDENTE: &str = "mfa_pendente_id";

// Chave usada quando o usuário precisa configurar MFA antes de entrar.
// Pode vir de mfa_obrigatorio (por usuário) ou MFA_OBRIGATORIO (global).
const SESSION_MFA_SETUP: &str = "mfa_setup_id";

// ─── LOGIN — SEGUNDA ETAPA ───────────────────────────────

/// Exibe a página que pede o código de 6 dígitos do autenticador.
/// Só deve ser acessada quando SESSION_MFA_PENDENTE está na sessão —
/// o middleware de rota garante isso.
pub async fn pagina_mfa_login(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse> {
    let id_pendente = session
        .get::<String>(SESSION_MFA_PENDENTE)
        .await
        .unwrap_or(None);

    if id_pendente.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let csrf = gerar_token(&session).await;
    let mut ctx = Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("tema", &state.config.tema);
    ctx.insert("csrf", &csrf);
    ctx.insert("erro", &Option::<String>::None);

    Ok(Html(state.tera.render("admin/mfa_codigo.html", &ctx)?).into_response())
}

#[derive(Deserialize)]
pub struct FormCodigo {
    pub codigo: String,
    pub _csrf: String,
}

/// Valida o código digitado na segunda etapa do login.
/// Se correto: promove a sessão (insere SESSION_USER_ID, remove SESSION_MFA_PENDENTE).
/// Se errado: volta para a página do código com erro.
pub async fn processar_mfa_login(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<FormCodigo>,
) -> Result<impl IntoResponse> {
    if !validar_token(&session, &dados._csrf).await {
        return Err(AppError::NaoAutorizado);
    }

    let id = match session
        .get::<String>(SESSION_MFA_PENDENTE)
        .await
        .unwrap_or(None)
    {
        Some(id) => id,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let usuario = UsuariosRepo::novo(&state.db)
        .buscar_por_id(&id)
        .await?
        .ok_or(AppError::NaoAutorizado)?;

    let secret = usuario
        .mfa_secret
        .as_deref()
        .ok_or(AppError::NaoAutorizado)?;

    let codigo_valido = mfa::validar_codigo(
        secret,
        &dados.codigo,
        &state.config.site_nome,
        &usuario.email,
    );

    // Verificação em duas etapas: validade matemática (TOTP) e depois replay.
    // registrar_uso_mfa é atômico — rejeita se o mesmo código já foi aceito
    // nos últimos 30s, eliminando submissões concorrentes com o mesmo OTP.
    let aceito = if codigo_valido {
        UsuariosRepo::novo(&state.db)
            .registrar_uso_mfa(&id, &dados.codigo)
            .await?
    } else {
        false
    };

    if aceito {
        session.remove::<String>(SESSION_MFA_PENDENTE).await.ok();
        session.insert(SESSION_USER_ID, &id).await.unwrap();
        tracing::info!(usuario_id = %id, "MFA validado — login completo");
        Ok(Redirect::to("/admin").into_response())
    } else {
        if codigo_valido {
            tracing::warn!(usuario_id = %id, "Replay de código MFA bloqueado");
        } else {
            tracing::warn!(usuario_id = %id, "Código MFA incorreto");
        }
        let csrf = gerar_token(&session).await;
        let mut ctx = Context::new();
        ctx.insert("site_nome", &state.config.site_nome);
        ctx.insert("site_logo", &state.config.site_logo);
        ctx.insert("tema", &state.config.tema);
        ctx.insert("csrf", &csrf);
        ctx.insert("erro", &Some("Código incorreto. Tente novamente."));
        Ok(Html(state.tera.render("admin/mfa_codigo.html", &ctx)?).into_response())
    }
}

// ─── SETUP OBRIGATÓRIO (redirecionado no login) ──────────

/// Exibe a página de configuração de MFA quando o setup é obrigatório.
/// Gera um novo secret temporário e mostra o QR Code.
/// O secret só é salvo no banco depois que o usuário confirmar com um código válido.
pub async fn pagina_mfa_setup(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse> {
    let id = match session
        .get::<String>(SESSION_MFA_SETUP)
        .await
        .unwrap_or(None)
    {
        Some(id) => id,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let usuario = UsuariosRepo::novo(&state.db)
        .buscar_por_id(&id)
        .await?
        .ok_or(AppError::NaoAutorizado)?;

    let secret_temp = mfa::gerar_secret();
    session.insert("mfa_secret_temp", &secret_temp).await.ok();

    let qr = mfa::gerar_qr_base64(&state.config.site_nome, &usuario.email, &secret_temp)?;
    let csrf = gerar_token(&session).await;

    let mut ctx = Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("tema", &state.config.tema);
    ctx.insert("qr_base64", &qr);
    ctx.insert("secret", &secret_temp);
    ctx.insert("csrf", &csrf);
    ctx.insert("erro", &Option::<String>::None);
    // "login": form aponta para /login/mfa/configurar
    ctx.insert("contexto", "login");

    Ok(Html(state.tera.render("admin/mfa_setup.html", &ctx)?).into_response())
}

/// Confirma o código do setup obrigatório e salva o secret no banco.
/// Após salvar, promove a sessão para autenticada e redireciona para o admin.
pub async fn processar_mfa_setup(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<FormCodigo>,
) -> Result<impl IntoResponse> {
    if !validar_token(&session, &dados._csrf).await {
        return Err(AppError::NaoAutorizado);
    }

    let id = match session
        .get::<String>(SESSION_MFA_SETUP)
        .await
        .unwrap_or(None)
    {
        Some(id) => id,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let secret_temp = match session
        .get::<String>("mfa_secret_temp")
        .await
        .unwrap_or(None)
    {
        Some(s) => s,
        None => return Ok(Redirect::to("/login/mfa/configurar").into_response()),
    };

    let usuario = UsuariosRepo::novo(&state.db)
        .buscar_por_id(&id)
        .await?
        .ok_or(AppError::NaoAutorizado)?;

    if mfa::validar_codigo(
        &secret_temp,
        &dados.codigo,
        &state.config.site_nome,
        &usuario.email,
    ) {
        UsuariosRepo::novo(&state.db)
            .habilitar_mfa(&id, &secret_temp)
            .await?;

        session.remove::<String>(SESSION_MFA_SETUP).await.ok();
        session.remove::<String>("mfa_secret_temp").await.ok();
        session.insert(SESSION_USER_ID, &id).await.unwrap();

        tracing::info!(usuario_id = %id, "MFA configurado e ativado via setup obrigatório");
        Ok(Redirect::to("/admin").into_response())
    } else {
        tracing::warn!(usuario_id = %id, "Código MFA incorreto durante setup");
        let qr = mfa::gerar_qr_base64(&state.config.site_nome, &usuario.email, &secret_temp)?;
        let csrf = gerar_token(&session).await;

        let mut ctx = Context::new();
        ctx.insert("site_nome", &state.config.site_nome);
        ctx.insert("site_logo", &state.config.site_logo);
        ctx.insert("tema", &state.config.tema);
        ctx.insert("qr_base64", &qr);
        ctx.insert("secret", &secret_temp);
        ctx.insert("csrf", &csrf);
        ctx.insert("contexto", "login");
        ctx.insert(
            "erro",
            &Some("Código incorreto. Verifique o app e tente novamente."),
        );
        Ok(Html(state.tera.render("admin/mfa_setup.html", &ctx)?).into_response())
    }
}

// ─── PERFIL — ATIVAÇÃO VOLUNTÁRIA ───────────────────────

/// Exibe a página de configuração de MFA no perfil do usuário.
/// Acessada voluntariamente — o usuário já está logado.
pub async fn pagina_perfil_mfa(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse> {
    let id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .ok_or(AppError::NaoAutorizado)?;

    let usuario = UsuariosRepo::novo(&state.db)
        .buscar_por_id(&id)
        .await?
        .ok_or(AppError::NaoAutorizado)?;

    let secret_temp = mfa::gerar_secret();
    session.insert("mfa_secret_temp", &secret_temp).await.ok();

    let qr = mfa::gerar_qr_base64(&state.config.site_nome, &usuario.email, &secret_temp)?;
    let csrf = gerar_token(&session).await;

    let mut ctx = Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("tema", &state.config.tema);
    ctx.insert("qr_base64", &qr);
    ctx.insert("secret", &secret_temp);
    ctx.insert("csrf", &csrf);
    ctx.insert("erro", &Option::<String>::None);
    // "perfil": form aponta para /admin/perfil/mfa/ativar
    ctx.insert("contexto", "perfil");

    Ok(Html(state.tera.render("admin/mfa_setup.html", &ctx)?).into_response())
}

/// Ativa o MFA no perfil após confirmação com código válido.
pub async fn ativar_mfa_perfil(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<FormCodigo>,
) -> Result<impl IntoResponse> {
    if !validar_token(&session, &dados._csrf).await {
        return Err(AppError::NaoAutorizado);
    }

    let id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .ok_or(AppError::NaoAutorizado)?;

    let secret_temp = match session
        .get::<String>("mfa_secret_temp")
        .await
        .unwrap_or(None)
    {
        Some(s) => s,
        None => return Ok(Redirect::to("/admin/perfil/mfa").into_response()),
    };

    let usuario = UsuariosRepo::novo(&state.db)
        .buscar_por_id(&id)
        .await?
        .ok_or(AppError::NaoAutorizado)?;

    if mfa::validar_codigo(
        &secret_temp,
        &dados.codigo,
        &state.config.site_nome,
        &usuario.email,
    ) {
        UsuariosRepo::novo(&state.db)
            .habilitar_mfa(&id, &secret_temp)
            .await?;
        session.remove::<String>("mfa_secret_temp").await.ok();
        tracing::info!(usuario_id = %id, "MFA ativado pelo usuário no perfil");
        Ok(Redirect::to("/admin/perfil").into_response())
    } else {
        tracing::warn!(usuario_id = %id, "Código MFA incorreto ao ativar no perfil");
        let qr = mfa::gerar_qr_base64(&state.config.site_nome, &usuario.email, &secret_temp)?;
        let csrf = gerar_token(&session).await;

        let mut ctx = Context::new();
        ctx.insert("site_nome", &state.config.site_nome);
        ctx.insert("site_logo", &state.config.site_logo);
        ctx.insert("tema", &state.config.tema);
        ctx.insert("qr_base64", &qr);
        ctx.insert("secret", &secret_temp);
        ctx.insert("csrf", &csrf);
        ctx.insert("contexto", "perfil");
        ctx.insert(
            "erro",
            &Some("Código incorreto. Verifique o app e tente novamente."),
        );
        Ok(Html(state.tera.render("admin/mfa_setup.html", &ctx)?).into_response())
    }
}

/// Desativa o MFA no perfil.
/// Bloqueado se MFA_OBRIGATORIO global estiver ativo.
pub async fn desativar_mfa_perfil(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse> {
    if state.config.mfa_obrigatorio {
        return Ok(Redirect::to("/admin/perfil").into_response());
    }

    let id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .ok_or(AppError::NaoAutorizado)?;

    UsuariosRepo::novo(&state.db).desabilitar_mfa(&id).await?;

    tracing::info!(usuario_id = %id, "MFA desativado pelo usuário no perfil");
    Ok(Redirect::to("/admin/perfil").into_response())
}

// ─── ADMIN — GERENCIAR MFA DE OUTROS USUÁRIOS ───────────

/// Admin reseta o MFA de um usuário (emergência: perdeu o celular).
/// Remove o secret e desabilita — o usuário precisará configurar novamente.
pub async fn admin_resetar_mfa(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    UsuariosRepo::novo(&state.db).desabilitar_mfa(&id).await?;
    tracing::info!(usuario_id = %id, "MFA resetado pelo admin");
    Ok(Redirect::to(&format!("/admin/usuarios/{}/editar", id)).into_response())
}

/// Admin marca mfa_obrigatorio = 1 para um usuário específico.
/// O usuário será forçado a configurar MFA no próximo login.
pub async fn admin_exigir_mfa(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    UsuariosRepo::novo(&state.db).exigir_mfa(&id).await?;
    tracing::info!(usuario_id = %id, "MFA exigido pelo admin");
    Ok(Redirect::to(&format!("/admin/usuarios/{}/editar", id)).into_response())
}

/// Admin remove a exigência de MFA de um usuário.
pub async fn admin_remover_exigencia_mfa(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    UsuariosRepo::novo(&state.db)
        .remover_exigencia_mfa(&id)
        .await?;
    tracing::info!(usuario_id = %id, "Exigência de MFA removida pelo admin");
    Ok(Redirect::to(&format!("/admin/usuarios/{}/editar", id)).into_response())
}
