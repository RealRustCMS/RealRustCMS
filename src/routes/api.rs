use axum::{middleware, routing::get, Router};

use crate::{
    handlers::{
        api::{
            artigos_por_categoria, artigos_por_tag, listar_artigos, listar_categorias,
            listar_comentarios_pendentes, listar_comentarios_pendentes_por_url,
            listar_comentarios_por_url, listar_galeria, listar_tags, ver_album, ver_artigo,
        },
        api_auth::requer_token,
    },
    state::AppState,
};

pub fn rotas(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/artigos", get(listar_artigos))
        .route("/api/v1/artigos/{slug}", get(ver_artigo))
        .route("/api/v1/categorias", get(listar_categorias))
        .route(
            "/api/v1/categorias/{slug}/artigos",
            get(artigos_por_categoria),
        )
        .route("/api/v1/tags", get(listar_tags))
        .route("/api/v1/tags/{slug}/artigos", get(artigos_por_tag))
        .route("/api/v1/galeria", get(listar_galeria))
        .route("/api/v1/galeria/{id}", get(ver_album))
        .route("/api/v1/comentarios", get(listar_comentarios_por_url))
        .route(
            "/api/v1/comentarios/pendentes",
            get(listar_comentarios_pendentes),
        )
        .route(
            "/api/v1/comentarios/pendentes/url",
            get(listar_comentarios_pendentes_por_url),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_token))
        .with_state(state)
}
