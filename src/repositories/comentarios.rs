use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::Result, models::Comentario};

pub struct ComentariosRepo<'a> {
    pub db: &'a PgPool,
}

impl<'a> ComentariosRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    pub async fn listar_por_url(&self, url: &str) -> Result<Vec<Comentario>> {
        let comentarios = sqlx::query_as!(
            Comentario,
            "SELECT * FROM comentarios WHERE url = $1 AND status = 'aprovado'
             ORDER BY criado_em ASC",
            url
        )
        .fetch_all(self.db)
        .await?;
        Ok(comentarios)
    }

    pub async fn listar_pendentes(&self) -> Result<Vec<Comentario>> {
        let comentarios = sqlx::query_as!(
            Comentario,
            "SELECT * FROM comentarios WHERE status = 'pendente'
             ORDER BY criado_em ASC"
        )
        .fetch_all(self.db)
        .await?;
        Ok(comentarios)
    }

    pub async fn listar_pendentes_por_url(&self, url: &str) -> Result<Vec<Comentario>> {
        let comentarios = sqlx::query_as!(
            Comentario,
            "SELECT * FROM comentarios WHERE url = $1 AND status = 'pendente'
             ORDER BY criado_em ASC",
            url
        )
        .fetch_all(self.db)
        .await?;
        Ok(comentarios)
    }

    pub async fn listar_todos(
        &self,
        pagina: i64,
        por_pagina: i64,
    ) -> Result<(Vec<Comentario>, i64)> {
        let offset = (pagina - 1) * por_pagina;

        let comentarios = sqlx::query_as!(
            Comentario,
            "SELECT * FROM comentarios ORDER BY criado_em DESC LIMIT $1 OFFSET $2",
            por_pagina,
            offset
        )
        .fetch_all(self.db)
        .await?;

        let total = sqlx::query_scalar!("SELECT COUNT(*) FROM comentarios")
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);

        Ok((comentarios, total))
    }

    pub async fn criar(
        &self,
        url: &str,
        dados: &crate::models::NovoComentario,
        status: &str,
    ) -> Result<Comentario> {
        let id = Uuid::new_v4().to_string();
        sqlx::query!(
            "INSERT INTO comentarios (id, url, autor_nome, autor_email, corpo, status)
             VALUES ($1, $2, $3, $4, $5, $6)",
            id,
            url,
            dados.autor_nome,
            dados.autor_email,
            dados.corpo,
            status
        )
        .execute(self.db)
        .await?;

        let comentario = sqlx::query_as!(Comentario, "SELECT * FROM comentarios WHERE id = $1", id)
            .fetch_one(self.db)
            .await?;

        Ok(comentario)
    }

    pub async fn aprovar(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE comentarios SET status = 'aprovado' WHERE id = $1",
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn rejeitar(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE comentarios SET status = 'rejeitado' WHERE id = $1",
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn deletar(&self, id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM comentarios WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    pub async fn total_pendentes(&self) -> Result<i64> {
        let total =
            sqlx::query_scalar!("SELECT COUNT(*) FROM comentarios WHERE status = 'pendente'")
                .fetch_one(self.db)
                .await?
                .unwrap_or(0);
        Ok(total)
    }
}
