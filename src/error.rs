use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Database(sqlx::Error),
    Template(tera::Error),
    NaoEncontrado,
    NaoAutorizado,
    Interno(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, mensagem) = match self {
            AppError::NaoEncontrado => (StatusCode::NOT_FOUND, "Não encontrado".into()),
            AppError::NaoAutorizado => (StatusCode::UNAUTHORIZED, "Não autorizado".into()),
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Template(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Interno(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, mensagem).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Database(e)
    }
}

impl From<tera::Error> for AppError {
    fn from(e: tera::Error) -> Self {
        AppError::Template(e)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NaoEncontrado => write!(f, "Não encontrado"),
            AppError::NaoAutorizado => write!(f, "Não autorizado"),
            AppError::Database(e) => write!(f, "Erro de banco: {}", e),
            AppError::Template(e) => write!(f, "Erro de template: {}", e),
            AppError::Interno(msg) => write!(f, "{}", msg),
        }
    }
}
