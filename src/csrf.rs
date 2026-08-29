use constant_time_eq::constant_time_eq;
use tower_sessions::Session;
use uuid::Uuid;

const CSRF_KEY: &str = "csrf_token";

/// Gera um token CSRF e salva na sessão
pub async fn gerar_token(session: &Session) -> String {
    // Reusa o token existente se já houver um
    if let Ok(Some(token)) = session.get::<String>(CSRF_KEY).await {
        return token;
    }

    let token = Uuid::new_v4().to_string();
    session.insert(CSRF_KEY, &token).await.ok();
    token
}

/// Valida o token CSRF enviado no formulário
pub async fn validar_token(session: &Session, token_recebido: &str) -> bool {
    match session.get::<String>(CSRF_KEY).await {
        Ok(Some(token_sessao)) => constant_time_eq(token_sessao.as_bytes(), token_recebido.as_bytes()),
        _ => false,
    }
}
