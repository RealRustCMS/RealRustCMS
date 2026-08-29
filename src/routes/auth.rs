use axum::routing::{get, post};
use axum::Router;

use crate::{
    handlers::auth::{pagina_login, processar_login, processar_logout},
    state::AppState,
};

pub fn rotas(state: AppState) -> Router {
    Router::new()
        .route("/login", get(pagina_login).post(processar_login))
        .route("/logout", post(processar_logout))
        .with_state(state)
}
