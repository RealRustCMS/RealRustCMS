use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

use crate::state::AppState;

pub async fn requer_token(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let token = match token {
        Some(t) => t.to_string(),
        None => {
            tracing::warn!("Requisição à API sem token de autenticação");
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "erro": "Token não fornecido" })),
            )
                .into_response();
        }
    };

    let valido: i64 =
        sqlx::query_scalar!("SELECT COUNT(*) FROM usuarios WHERE api_token = $1", token)
            .fetch_one(&state.db)
            .await
            .unwrap_or(Some(0))
            .unwrap_or(0);

    if valido == 0 {
        tracing::warn!("Requisição à API com token inválido");
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "erro": "Token inválido" })),
        )
            .into_response();
    }

    next.run(request).await
}
