mod config;
pub mod csrf;
mod db;
mod error;
mod handlers;
mod ip;
mod models;
pub mod rate_limit;
mod repositories;
mod routes;
mod sanitize;
mod services;
mod state;

use state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        std::env::set_current_dir(manifest_dir).unwrap();
    }

    // Carrega o .env antes do tracing para que RUST_LOG seja lido daqui
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = config::Config::carregar();
    let db = db::conectar(&config.database_url).await;

    // Roda as migrations automaticamente na inicialização.
    // O SQLx controla quais já foram aplicadas via tabela _sqlx_migrations,
    // então é seguro rodar sempre — migrations já aplicadas são ignoradas.
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Falha ao rodar migrations");

    let tera = tera::Tera::new("templates/**/*.html").expect("Falha ao carregar templates Tera");

    let porta = config.porta;
    let state = AppState {
        db,
        config,
        tera,
        menu_cache: Arc::new(RwLock::new(Vec::new())),
    };

    // Popula o cache do menu principal antes de aceitar requests.
    // Se a query falhar (ex: banco indisponível), sobe com menu vazio —
    // degradação silenciosa, o tracing já logou o erro de conexão antes.
    let menu_inicial = repositories::menus::MenusRepo::novo(&state.db)
        .arvore_menu_principal()
        .await
        .unwrap_or_default();
    state.atualizar_menu_cache(menu_inicial).await;

    rate_limit::iniciar_sweep_periodico();

    let app = routes::montar(state).await;

    let endereco = format!("0.0.0.0:{}", porta);
    let listener = tokio::net::TcpListener::bind(&endereco).await.unwrap();

    tracing::info!("Servidor rodando em http://{}", endereco);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
