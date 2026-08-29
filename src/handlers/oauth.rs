use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    repositories::membros::MembrosRepo,
    services::membros::MembrosService,
    services::oidc::{
        buscar_discovery, montar_url_generico, montar_url_github, montar_url_google,
        montar_url_microsoft, resolver_usuario, salvar_state, trocar_code_generico,
        trocar_code_github, trocar_code_google, trocar_code_microsoft, validar_state,
    },
    state::AppState,
};

const SESSION_USER_ID: &str = "usuario_id";
const SESSION_MEMBRO_ID: &str = "membro_id";

// ─── QUERY PARAMS DO REDIRECT INICIAL ────────────────────
// O link "Entrar com Google/Microsoft/GitHub/OIDC" pode trazer ?next=
// quando a pessoa chegou ao login a partir de um conteúdo restrito.
#[derive(Deserialize)]
pub struct QueryNext {
    pub next: Option<String>,
}

/// Mesma validação usada em handlers/membros.rs — paths relativos
/// começando com exatamente uma barra, nunca URLs absolutas ou
/// protocol-relative (`//evil.com`). Evita open redirect via OAuth também.
fn next_seguro(next: Option<&str>) -> Option<String> {
    match next {
        // Ver comentário equivalente em handlers/membros.rs — barra invertida
        // é normalizada para '/' pelos navegadores, permitindo bypass do
        // filtro de "//" (open redirect via "/\evil.com").
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.contains('\\') => {
            Some(n.to_string())
        }
        _ => None,
    }
}

// ─── QUERY PARAMS DO CALLBACK ────────────────────────────
// O provedor envia `code` e `state` como query params na URL de callback.
// Exemplo: /auth/google/callback?code=ABC&state=XYZ
#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    // Alguns provedores enviam `error` quando o usuário cancela
    pub error: Option<String>,
}

// ─── HELPER: URL base do site ────────────────────────────
fn redirect_uri(base_url: &str, provider: &str) -> String {
    format!("{}/auth/{}/callback", base_url, provider)
}

// ─── HELPER: resolve membro via SSO ──────────────────────
// Chamado em todos os callbacks após obter os claims.
// Insere membro_id na sessão se o email/sub corresponder a um membro
// (existente ou criado automaticamente conforme MEMBROS_PERMITIR_CADASTRO
// e MEMBROS_DOMINIO_PERMITIDO).
// Erros são apenas logados — nunca bloqueiam o fluxo de login do CMS.
async fn resolver_e_inserir_membro(
    state: &AppState,
    session: &Session,
    provider: &str,
    sub: &str,
    email: &str,
    nome: &str,
) {
    let repo = MembrosRepo::novo(&state.db);
    match MembrosService::resolver_oauth(
        &repo,
        provider,
        sub,
        email,
        nome,
        state.config.membros_permitir_cadastro,
        state.config.membros_dominio_permitido.as_deref(),
    )
    .await
    {
        Ok(Some(membro)) => {
            session.insert(SESSION_MEMBRO_ID, membro.id).await.ok();
            tracing::info!(
                membro_id = %membro.id,
                provider = %provider,
                "Membro autenticado via SSO"
            );
        }
        Ok(None) => {
            // email não corresponde a membro — normal, sem log
        }
        Err(e) => {
            // falha não bloqueia o login do CMS
            tracing::warn!(erro = %e, provider = %provider, "Falha ao resolver membro via SSO");
        }
    }
}

// ─── HELPER: decide o destino final pós-login ────────────
// Prioriza o `next` salvo junto ao state (se válido) — tanto para membro
// quanto para usuário CMS, como decidido: ambos voltam ao conteúdo que
// motivou o login quando há um destino específico. Sem next, cada tipo
// de sessão cai no seu destino padrão (usuário → /admin, membro → área).
fn destino_pos_login(next: &Option<String>, usuario_logado: bool) -> String {
    match next {
        Some(n) => n.clone(),
        None if usuario_logado => "/admin".to_string(),
        None => "/membros/area".to_string(),
    }
}

// ─── GOOGLE ──────────────────────────────────────────────

pub async fn google_redirect(
    State(state): State<AppState>,
    Query(query): Query<QueryNext>,
) -> impl IntoResponse {
    let config = match &state.config.oidc_google {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let next = next_seguro(query.next.as_deref());
    let oauth_state = match salvar_state(&state.db, "google", next.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao salvar state OAuth Google");
            return Redirect::to("/login").into_response();
        }
    };

    let uri = redirect_uri(&state.config.base_url, "google");
    let url = montar_url_google(&config, &oauth_state, &uri);

    tracing::info!("Iniciando fluxo OIDC Google");
    Redirect::to(&url).into_response()
}

pub async fn google_callback(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    if let Some(erro) = &params.error {
        tracing::warn!(erro = %erro, "Usuário cancelou autenticação Google");
        return Redirect::to("/login").into_response();
    }

    let code = match &params.code {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let oauth_state = match &params.state {
        Some(s) => s.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let next = match validar_state(&state.db, &oauth_state, "google").await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(erro = %e, "State inválido no callback Google");
            return Redirect::to("/login").into_response();
        }
    };

    let config = match &state.config.oidc_google {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let uri = redirect_uri(&state.config.base_url, "google");

    let claims = match trocar_code_google(&config, &code, &uri).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao trocar code Google");
            return Redirect::to("/login").into_response();
        }
    };

    // resolve usuario CMS e membro em paralelo
    let (resultado_usuario, _) = tokio::join!(
        resolver_usuario(
            &state.db,
            &claims,
            "google",
            state.config.oidc_permitir_cadastro,
        ),
        resolver_e_inserir_membro(
            &state,
            &session,
            "google",
            &claims.sub,
            &claims.email,
            &claims.nome,
        )
    );

    match resultado_usuario {
        Ok(usuario) => {
            session.insert(SESSION_USER_ID, &usuario.id).await.ok();
            tracing::info!(usuario_id = %usuario.id, "Login via Google bem-sucedido");
            Redirect::to(&destino_pos_login(&next, true)).into_response()
        }
        Err(e) => {
            tracing::warn!(erro = %e, email = %claims.email, "Acesso negado via Google");
            // remove membro_id inserido acima — login de usuário falhou
            session.remove::<i64>(SESSION_MEMBRO_ID).await.ok();
            Redirect::to("/login?erro=acesso_negado").into_response()
        }
    }
}

// ─── MICROSOFT ───────────────────────────────────────────

pub async fn microsoft_redirect(
    State(state): State<AppState>,
    Query(query): Query<QueryNext>,
) -> impl IntoResponse {
    let config = match &state.config.oidc_microsoft {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let next = next_seguro(query.next.as_deref());
    let oauth_state = match salvar_state(&state.db, "microsoft", next.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao salvar state OAuth Microsoft");
            return Redirect::to("/login").into_response();
        }
    };

    let uri = redirect_uri(&state.config.base_url, "microsoft");
    let url = montar_url_microsoft(
        &config,
        &state.config.oidc_microsoft_tenant,
        &oauth_state,
        &uri,
    );

    tracing::info!("Iniciando fluxo OIDC Microsoft");
    Redirect::to(&url).into_response()
}

pub async fn microsoft_callback(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    if let Some(erro) = &params.error {
        tracing::warn!(erro = %erro, "Usuário cancelou autenticação Microsoft");
        return Redirect::to("/login").into_response();
    }

    let code = match &params.code {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let oauth_state = match &params.state {
        Some(s) => s.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let next = match validar_state(&state.db, &oauth_state, "microsoft").await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(erro = %e, "State inválido no callback Microsoft");
            return Redirect::to("/login").into_response();
        }
    };

    let config = match &state.config.oidc_microsoft {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let uri = redirect_uri(&state.config.base_url, "microsoft");

    let claims = match trocar_code_microsoft(
        &config,
        &state.config.oidc_microsoft_tenant,
        &code,
        &uri,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao trocar code Microsoft");
            return Redirect::to("/login").into_response();
        }
    };

    let (resultado_usuario, _) = tokio::join!(
        resolver_usuario(
            &state.db,
            &claims,
            "microsoft",
            state.config.oidc_permitir_cadastro,
        ),
        resolver_e_inserir_membro(
            &state,
            &session,
            "microsoft",
            &claims.sub,
            &claims.email,
            &claims.nome,
        )
    );

    match resultado_usuario {
        Ok(usuario) => {
            session.insert(SESSION_USER_ID, &usuario.id).await.ok();
            tracing::info!(usuario_id = %usuario.id, "Login via Microsoft bem-sucedido");
            Redirect::to(&destino_pos_login(&next, true)).into_response()
        }
        Err(e) => {
            tracing::warn!(erro = %e, email = %claims.email, "Acesso negado via Microsoft");
            session.remove::<i64>(SESSION_MEMBRO_ID).await.ok();
            Redirect::to("/login?erro=acesso_negado").into_response()
        }
    }
}

// ─── GITHUB ──────────────────────────────────────────────

pub async fn github_redirect(
    State(state): State<AppState>,
    Query(query): Query<QueryNext>,
) -> impl IntoResponse {
    let config = match &state.config.oidc_github {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let next = next_seguro(query.next.as_deref());
    let oauth_state = match salvar_state(&state.db, "github", next.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao salvar state OAuth GitHub");
            return Redirect::to("/login").into_response();
        }
    };

    let url = montar_url_github(&config, &oauth_state);

    tracing::info!("Iniciando fluxo OAuth GitHub");
    Redirect::to(&url).into_response()
}

pub async fn github_callback(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    if let Some(erro) = &params.error {
        tracing::warn!(erro = %erro, "Usuário cancelou autenticação GitHub");
        return Redirect::to("/login").into_response();
    }

    let code = match &params.code {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let oauth_state = match &params.state {
        Some(s) => s.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let next = match validar_state(&state.db, &oauth_state, "github").await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(erro = %e, "State inválido no callback GitHub");
            return Redirect::to("/login").into_response();
        }
    };

    let config = match &state.config.oidc_github {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let claims = match trocar_code_github(&config, &code).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao trocar code GitHub");
            return Redirect::to("/login").into_response();
        }
    };

    let (resultado_usuario, _) = tokio::join!(
        resolver_usuario(
            &state.db,
            &claims,
            "github",
            state.config.oidc_permitir_cadastro,
        ),
        resolver_e_inserir_membro(
            &state,
            &session,
            "github",
            &claims.sub,
            &claims.email,
            &claims.nome,
        )
    );

    match resultado_usuario {
        Ok(usuario) => {
            session.insert(SESSION_USER_ID, &usuario.id).await.ok();
            tracing::info!(usuario_id = %usuario.id, "Login via GitHub bem-sucedido");
            Redirect::to(&destino_pos_login(&next, true)).into_response()
        }
        Err(e) => {
            tracing::warn!(erro = %e, email = %claims.email, "Acesso negado via GitHub");
            session.remove::<i64>(SESSION_MEMBRO_ID).await.ok();
            Redirect::to("/login?erro=acesso_negado").into_response()
        }
    }
}

// ─── PROVEDOR GENÉRICO ───────────────────────────────────

pub async fn oidc_redirect(
    State(state): State<AppState>,
    Query(query): Query<QueryNext>,
) -> impl IntoResponse {
    let config = match &state.config.oidc_generico {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let discovery = match buscar_discovery(&config.discovery_url).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao buscar discovery document");
            return Redirect::to("/login").into_response();
        }
    };

    let next = next_seguro(query.next.as_deref());
    let oauth_state = match salvar_state(&state.db, "oidc", next.as_deref()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao salvar state OAuth genérico");
            return Redirect::to("/login").into_response();
        }
    };

    let uri = redirect_uri(&state.config.base_url, "oidc");
    let url = montar_url_generico(
        &config,
        &oauth_state,
        &uri,
        &discovery.authorization_endpoint,
    );

    tracing::info!("Iniciando fluxo OIDC genérico");
    Redirect::to(&url).into_response()
}

pub async fn oidc_callback(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<CallbackParams>,
) -> impl IntoResponse {
    if let Some(erro) = &params.error {
        tracing::warn!(erro = %erro, "Usuário cancelou autenticação OIDC genérico");
        return Redirect::to("/login").into_response();
    }

    let code = match &params.code {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let oauth_state = match &params.state {
        Some(s) => s.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let next = match validar_state(&state.db, &oauth_state, "oidc").await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(erro = %e, "State inválido no callback OIDC genérico");
            return Redirect::to("/login").into_response();
        }
    };

    let config = match &state.config.oidc_generico {
        Some(c) => c.clone(),
        None => return Redirect::to("/login").into_response(),
    };

    let discovery = match buscar_discovery(&config.discovery_url).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao buscar discovery document no callback");
            return Redirect::to("/login").into_response();
        }
    };

    let uri = redirect_uri(&state.config.base_url, "oidc");

    let claims = match trocar_code_generico(&config, &code, &uri, &discovery.token_endpoint).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao trocar code OIDC genérico");
            return Redirect::to("/login").into_response();
        }
    };

    let (resultado_usuario, _) = tokio::join!(
        resolver_usuario(
            &state.db,
            &claims,
            "oidc",
            state.config.oidc_generico_permitir_cadastro,
        ),
        resolver_e_inserir_membro(
            &state,
            &session,
            "oidc",
            &claims.sub,
            &claims.email,
            &claims.nome,
        )
    );

    match resultado_usuario {
        Ok(usuario) => {
            session.insert(SESSION_USER_ID, &usuario.id).await.ok();
            tracing::info!(usuario_id = %usuario.id, "Login via OIDC genérico bem-sucedido");
            Redirect::to(&destino_pos_login(&next, true)).into_response()
        }
        Err(e) => {
            tracing::warn!(erro = %e, email = %claims.email, "Acesso negado via OIDC genérico");
            session.remove::<i64>(SESSION_MEMBRO_ID).await.ok();
            Redirect::to("/login?erro=acesso_negado").into_response()
        }
    }
}