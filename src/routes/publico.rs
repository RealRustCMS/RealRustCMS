use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handlers::publico::{
        artigos_por_categoria, artigos_por_tag, avaliar_artigo, buscar, home, listar_artigos,
        listar_eventos, listar_galeria, postar_comentario, rss, sitemap, ver_album_publico,
        ver_artigo, ver_evento, ver_pagina,
    },
    state::AppState,
};

pub fn rotas(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/artigos", get(listar_artigos))
        .route("/artigos/{slug}", get(ver_artigo))
        .route("/artigos/{slug}/comentarios", post(postar_comentario))
        .route("/artigos/{slug}/avaliar", post(avaliar_artigo))
        .route("/categoria/{slug}", get(artigos_por_categoria))
        .route("/tag/{slug}", get(artigos_por_tag))
        .route("/galeria", get(listar_galeria))
        .route("/galeria/{id}", get(ver_album_publico))
        .route("/busca", get(buscar))
        .route("/paginas/{slug}", get(ver_pagina))
        .route("/eventos", get(listar_eventos))
        .route("/eventos/{slug}", get(ver_evento))
        .route("/rss", get(rss))
        .route("/sitemap.xml", get(sitemap))
        .with_state(state)
}
