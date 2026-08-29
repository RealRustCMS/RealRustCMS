use axum::{
    extract::{ConnectInfo, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use std::net::SocketAddr;
use tera::Context;
use tower_sessions::Session;

use crate::{rate_limit, services::auth::AuthService, state::AppState};

const SESSION_USER_ID: &str = "usuario_id";

const SESSION_MFA_PENDENTE: &str = "mfa_pendente_id";
const SESSION_MFA_SETUP: &str = "mfa_setup_id";

#[derive(Deserialize)]
pub struct FormLogin {
    pub email: String,
    pub senha: String,
}

#[derive(Deserialize)]
pub struct QueryLogin {
    pub erro: Option<String>,
}

pub async fn pagina_login(
    State(state): State<AppState>,
    Query(query): Query<QueryLogin>,
) -> Html<String> {
    let mut ctx = Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("tema", &state.config.tema);

    let erro: Option<String> = match query.erro.as_deref() {
        Some("acesso_negado") => Some("Acesso não autorizado. Solicite ao administrador.".into()),
        _ => None,
    };
    ctx.insert("erro", &erro);

    ctx.insert("oidc_google", &state.config.oidc_google.is_some());
    ctx.insert("oidc_microsoft", &state.config.oidc_microsoft.is_some());
    ctx.insert("oidc_github", &state.config.oidc_github.is_some());
    ctx.insert("oidc_generico", &state.config.oidc_generico.is_some());
    ctx.insert(
        "oidc_generico_label",
        &state.config.oidc_generico.as_ref().map(|c| &c.botao_label),
    );

    Html(state.tera.render("admin/login.html", &ctx).unwrap())
}

pub async fn processar_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    session: Session,
    Form(dados): Form<FormLogin>,
) -> impl IntoResponse {
    let ip = crate::ip::do_cliente(&headers, addr);

    if let Err(msg) = rate_limit::verificar_ip(&ip) {
        tracing::warn!(ip = %ip, "Rate limit atingido no login");
        return renderizar_erro_login(&state, &msg).into_response();
    }

    let service = AuthService::novo(&state.db);

    match service.login(&dados.email, &dados.senha).await {
        Ok(usuario) => {
            rate_limit::registrar_sucesso(&ip);
            tracing::info!(ip = %ip, email = %dados.email, usuario_id = %usuario.id, "Credenciais válidas");

            // Caminho 1: MFA ativo — pede o código
            if usuario.mfa_habilitado {
                session
                    .insert(SESSION_MFA_PENDENTE, &usuario.id)
                    .await
                    .unwrap();
                tracing::info!(usuario_id = %usuario.id, "MFA pendente — redirecionando para /login/mfa");
                return Redirect::to("/login/mfa").into_response();
            }

            // Caminho 2: MFA obrigatório mas não configurado — força o setup
            let mfa_exigido = state.config.mfa_obrigatorio || usuario.mfa_obrigatorio;
            if mfa_exigido {
                session
                    .insert(SESSION_MFA_SETUP, &usuario.id)
                    .await
                    .unwrap();
                tracing::info!(usuario_id = %usuario.id, "MFA obrigatório não configurado — redirecionando para setup");
                return Redirect::to("/login/mfa/configurar").into_response();
            }

            // Caminho 3: sem MFA — login completo
            session.insert(SESSION_USER_ID, &usuario.id).await.unwrap();
            tracing::info!(ip = %ip, email = %dados.email, "Login bem-sucedido");
            Redirect::to("/admin").into_response()
        }
        Err(_) => {
            tracing::warn!(ip = %ip, email = %dados.email, "Tentativa de login falhou");
            renderizar_erro_login(&state, "E-mail ou senha incorretos.").into_response()
        }
    }
}

pub async fn processar_logout(session: Session) -> impl IntoResponse {
    tracing::info!("Logout realizado");
    session.flush().await.unwrap();
    Redirect::to("/login")
}

fn renderizar_erro_login(state: &AppState, msg: &str) -> Html<String> {
    let mut ctx = Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("tema", &state.config.tema);
    ctx.insert("erro", &Some(msg));
    ctx.insert("oidc_google", &state.config.oidc_google.is_some());
    ctx.insert("oidc_microsoft", &state.config.oidc_microsoft.is_some());
    ctx.insert("oidc_github", &state.config.oidc_github.is_some());
    ctx.insert("oidc_generico", &state.config.oidc_generico.is_some());
    ctx.insert(
        "oidc_generico_label",
        &state.config.oidc_generico.as_ref().map(|c| &c.botao_label),
    );
    Html(state.tera.render("admin/login.html", &ctx).unwrap())
}
