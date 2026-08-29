use axum::{middleware, routing::post, Router};

use crate::{
    handlers::{middleware::requer_login, upload::upload_imagem},
    state::AppState,
};

pub fn rotas(state: AppState) -> Router {
    Router::new()
        .route("/admin/upload/imagem", post(upload_imagem))
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_login))
        .with_state(state)
}
