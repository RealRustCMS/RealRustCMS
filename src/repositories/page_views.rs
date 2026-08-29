use crate::{error::Result, models::PageView};
use sqlx::PgPool;

pub struct PageViewsRepo<'a> {
    pub db: &'a PgPool,
}

impl<'a> PageViewsRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    /// Registra uma visita — cria o registro se não existir, incrementa se existir.
    /// ON CONFLICT ... DO UPDATE é o equivalente Postgres do ON DUPLICATE KEY UPDATE do MySQL.
    /// A constraint UNIQUE(url) na tabela page_views é o que aciona o ON CONFLICT.
    pub async fn registrar(&self, url: &str) -> Result<()> {
        sqlx::query!(
            "INSERT INTO page_views (url, visualizacoes, ultima_visita)
             VALUES ($1, 1, NOW())
             ON CONFLICT (url) DO UPDATE
               SET visualizacoes = page_views.visualizacoes + 1,
                   ultima_visita = NOW()",
            url
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    /// Retorna o total de visualizações de uma URL
    pub async fn contar(&self, url: &str) -> Result<i64> {
        let total = sqlx::query_scalar!("SELECT visualizacoes FROM page_views WHERE url = $1", url)
            .fetch_optional(self.db)
            .await?
            .unwrap_or(0);
        Ok(total)
    }

    /// Lista as URLs mais visitadas (todas as rotas)
    pub async fn mais_visitadas(&self, limite: i64) -> Result<Vec<PageView>> {
        let paginas = sqlx::query_as!(
            PageView,
            "SELECT url, visualizacoes, ultima_visita
             FROM page_views
             ORDER BY visualizacoes DESC
             LIMIT $1",
            limite
        )
        .fetch_all(self.db)
        .await?;
        Ok(paginas)
    }

    /// Lista apenas as páginas estáticas mais visitadas (/paginas/*)
    pub async fn mais_visitadas_paginas(&self, limite: i64) -> Result<Vec<PageView>> {
        let paginas = sqlx::query_as!(
            PageView,
            "SELECT url, visualizacoes, ultima_visita
             FROM page_views
             WHERE url LIKE '/paginas/%'
             ORDER BY visualizacoes DESC
             LIMIT $1",
            limite
        )
        .fetch_all(self.db)
        .await?;
        Ok(paginas)
    }

    /// Total geral de visualizações do site via SUM no banco.
    /// CAST para BIGINT porque SUM(bigint) retorna NUMERIC no Postgres,
    /// que o SQLx não mapeia para i64 sem a feature bigdecimal.
    /// COALESCE garante 0 em vez de NULL quando a tabela está vazia.
    pub async fn total_geral(&self) -> Result<i64> {
        let total =
            sqlx::query_scalar!("SELECT COALESCE(SUM(visualizacoes), 0)::BIGINT FROM page_views")
                .fetch_one(self.db)
                .await?
                .unwrap_or(0);
        Ok(total)
    }

    pub async fn listar_todas(&self, pagina: i64, por_pagina: i64) -> Result<(Vec<PageView>, i64)> {
        let offset = (pagina - 1) * por_pagina;

        let paginas = sqlx::query_as!(
            PageView,
            "SELECT url, visualizacoes, ultima_visita
             FROM page_views
             ORDER BY ultima_visita DESC
             LIMIT $1 OFFSET $2",
            por_pagina,
            offset
        )
        .fetch_all(self.db)
        .await?;

        let total = sqlx::query_scalar!("SELECT COUNT(*) FROM page_views")
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);

        Ok((paginas, total))
    }
}
