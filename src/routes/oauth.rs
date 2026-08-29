use axum::{routing::get, Router};

use crate::{
    handlers::oauth::{
        github_callback, github_redirect, google_callback, google_redirect, microsoft_callback,
        microsoft_redirect, oidc_callback, oidc_redirect,
    },
    state::AppState,
};

pub fn rotas(state: AppState) -> Router {
    Router::new()
        // Google
        .route("/auth/google/redirect", get(google_redirect))
        .route("/auth/google/callback", get(google_callback))
        // Microsoft
        .route("/auth/microsoft/redirect", get(microsoft_redirect))
        .route("/auth/microsoft/callback", get(microsoft_callback))
        // GitHub
        .route("/auth/github/redirect", get(github_redirect))
        .route("/auth/github/callback", get(github_callback))
        // Provedor genérico (Keycloak, RHSSO, etc.)
        .route("/auth/oidc/redirect", get(oidc_redirect))
        .route("/auth/oidc/callback", get(oidc_callback))
        .with_state(state)
}
