use sqlx::PgPool;

pub async fn conectar(database_url: &str) -> PgPool {
    PgPool::connect(database_url)
        .await
        .expect("Falha ao conectar no banco de dados")
}
