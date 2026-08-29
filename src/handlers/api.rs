use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde_json::{json, Value};

use crate::{
    models::Paginacao,
    repositories::{
        artigos::ArtigosRepo,
        categorias::{artigo_completo, CategoriasRepo, TagsRepo},
        comentarios::ComentariosRepo,
        configuracoes::ConfiguracoesRepo,
        galeria::GaleriaRepo,
    },
    state::AppState,
};

#[derive(serde::Deserialize)]
pub struct QueryPagina {
    pub pagina: Option<i64>,
}

#[derive(serde::Deserialize)]
pub struct QueryComentarios {
    pub url: String,
}

// API REST não tem conceito de sessão de membro (consumida via Bearer token,
// não por um navegador com cookies) — equivalente a um visitante anônimo.
// Respeita a mesma configuração `mostrar_artigos_restritos_listagem` usada
// nas listagens públicas via navegador.
async fn ocultar_restritos_api(state: &AppState) -> bool {
    let mostrar = ConfiguracoesRepo::novo(&state.db)
        .get_listagem_restrita()
        .await
        .unwrap_or_default()
        .mostrar_artigos_restritos_listagem;
    !mostrar
}

// ─── ARTIGOS ─────────────────────────────────────────────

pub async fn listar_artigos(
    State(state): State<AppState>,
    Query(query): Query<QueryPagina>,
) -> (StatusCode, Json<Value>) {
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = state.config.artigos_por_pagina;
    let ocultar_restritos = ocultar_restritos_api(&state).await;

    let (artigos, total) = match ArtigosRepo::novo(&state.db)
        .listar_publicados_paginado(pagina, por_pagina, ocultar_restritos)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(erro = %e, "Falha ao listar artigos na API");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "erro": "Erro interno" })),
            );
        }
    };

    let paginacao = Paginacao::calcular(pagina, total, por_pagina);

    (
        StatusCode::OK,
        Json(json!({
            "artigos":   artigos,
            "paginacao": paginacao,
        })),
    )
}

pub async fn ver_artigo(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> (StatusCode, Json<Value>) {
    match ArtigosRepo::novo(&state.db).buscar_por_slug(&slug).await {
        Ok(artigo) => {
            // Mesma regra das listagens: sem conceito de sessão de membro na
            // API, então um artigo restrito só é exposto se a configuração
            // do admin permitir (mostrar_artigos_restritos_listagem = true).
            if artigo.restrito && ocultar_restritos_api(&state).await {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "erro": "Artigo não encontrado" })),
                );
            }
            let completo = artigo_completo(&state.db, artigo).await;
            (StatusCode::OK, Json(json!(completo)))
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "erro": "Artigo não encontrado" })),
        ),
    }
}

// ─── CATEGORIAS ──────────────────────────────────────────

pub async fn listar_categorias(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let cats = CategoriasRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();
    (StatusCode::OK, Json(json!({ "categorias": cats })))
}

pub async fn artigos_por_categoria(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<QueryPagina>,
) -> (StatusCode, Json<Value>) {
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = state.config.artigos_por_pagina;
    let ocultar_restritos = ocultar_restritos_api(&state).await;

    match CategoriasRepo::novo(&state.db)
        .artigos_da_categoria(&slug, pagina, por_pagina, ocultar_restritos)
        .await
    {
        Ok((artigos, total, categoria)) => {
            let paginacao = Paginacao::calcular(pagina, total, por_pagina);
            (
                StatusCode::OK,
                Json(json!({
                    "categoria": categoria,
                    "artigos":   artigos,
                    "paginacao": paginacao,
                })),
            )
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "erro": "Categoria não encontrada" })),
        ),
    }
}

// ─── TAGS ────────────────────────────────────────────────

pub async fn listar_tags(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let tags = TagsRepo::novo(&state.db).listar().await.unwrap_or_default();
    (StatusCode::OK, Json(json!({ "tags": tags })))
}

pub async fn artigos_por_tag(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<QueryPagina>,
) -> (StatusCode, Json<Value>) {
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = state.config.artigos_por_pagina;
    let ocultar_restritos = ocultar_restritos_api(&state).await;

    match TagsRepo::novo(&state.db)
        .artigos_da_tag(&slug, pagina, por_pagina, ocultar_restritos)
        .await
    {
        Ok((artigos, total, tag)) => {
            let paginacao = Paginacao::calcular(pagina, total, por_pagina);
            (
                StatusCode::OK,
                Json(json!({
                    "tag":       tag,
                    "artigos":   artigos,
                    "paginacao": paginacao,
                })),
            )
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "erro": "Tag não encontrada" })),
        ),
    }
}

// ─── GALERIA ─────────────────────────────────────────────

pub async fn listar_galeria(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let albuns = GaleriaRepo::novo(&state.db)
        .listar_albuns()
        .await
        .unwrap_or_default();
    (StatusCode::OK, Json(json!({ "albuns": albuns })))
}

pub async fn ver_album(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<Value>) {
    let repo = GaleriaRepo::novo(&state.db);
    match repo.buscar_album(&id).await {
        Ok(album) => {
            let fotos = repo.listar_fotos_do_album(&id).await.unwrap_or_default();
            (
                StatusCode::OK,
                Json(json!({
                    "album": album,
                    "fotos": fotos,
                })),
            )
        }
        Err(e) => {
            tracing::error!(id = %id, erro = %e, "Falha ao buscar álbum na API");
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "erro": "Álbum não encontrado" })),
            )
        }
    }
}

pub async fn listar_comentarios_por_url(
    State(state): State<AppState>,
    Query(query): Query<QueryComentarios>,
) -> (StatusCode, Json<Value>) {
    let comentarios = ComentariosRepo::novo(&state.db)
        .listar_por_url(&query.url)
        .await
        .unwrap_or_default();

    (StatusCode::OK, Json(json!({ "comentarios": comentarios })))
}

pub async fn listar_comentarios_pendentes(
    State(state): State<AppState>,
) -> (StatusCode, Json<Value>) {
    let comentarios = ComentariosRepo::novo(&state.db)
        .listar_pendentes()
        .await
        .unwrap_or_default();

    (StatusCode::OK, Json(json!({ "comentarios": comentarios })))
}

pub async fn listar_comentarios_pendentes_por_url(
    State(state): State<AppState>,
    Query(query): Query<QueryComentarios>,
) -> (StatusCode, Json<Value>) {
    let comentarios = ComentariosRepo::novo(&state.db)
        .listar_pendentes_por_url(&query.url)
        .await
        .unwrap_or_default();

    (StatusCode::OK, Json(json!({ "comentarios": comentarios })))
}