use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

/// Abre o pool de conexões com o PostgreSQL.
///
/// `max_conexoes` vem de `Config::db_max_conexoes` (env `DB_MAX_CONEXOES`,
/// padrão 20). O default silencioso do `PgPool::connect` é 10 — baixo demais
/// quando cada page-view faz 3-8 queries mais a leitura/escrita da sessão.
///
/// - `min_connections(2)`: mantém 2 conexões quentes mesmo ocioso, evita
///   latência de handshake na primeira request depois de um período parado.
/// - `acquire_timeout(8s)`: se o pool esgotar, a request falha rápido em vez
///   de pendurar os 30s do default segurando um worker do Tokio.
/// - `test_before_acquire(true)`: descarta conexões mortas (restart do
///   Postgres, timeout de firewall) antes de entregar pro handler.
pub async fn conectar(database_url: &str, max_conexoes: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(max_conexoes)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(600))
        .test_before_acquire(true)
        .connect(database_url)
        .await
        .expect("Falha ao conectar no banco de dados")
}
