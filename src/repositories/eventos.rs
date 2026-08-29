use crate::{error::Result, models::Evento};
use sqlx::PgPool;
use uuid::Uuid;

pub struct EventosRepo<'a> {
    db: &'a PgPool,
}

impl<'a> EventosRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    // Busca todos os eventos para o painel admin (sem filtro de publicado).
    pub async fn listar(&self) -> Result<Vec<Evento>> {
        let eventos = sqlx::query_as!(
            Evento,
            "SELECT id, titulo, slug, descricao, data_hora, local, link_detalhes,
                    imagem_capa, publicado, criado_por, criado_em, atualizado_em
             FROM eventos
             ORDER BY data_hora ASC"
        )
        .fetch_all(self.db)
        .await?;
        Ok(eventos)
    }

    // Lista os próximos eventos publicados — usada no bloco "Agenda" da home pública.
    pub async fn listar_proximos(&self, limite: usize) -> Result<Vec<Evento>> {
        let limite = limite as i64;
        let eventos = sqlx::query_as!(
            Evento,
            "SELECT id, titulo, slug, descricao, data_hora, local, link_detalhes,
                    imagem_capa, publicado, criado_por, criado_em, atualizado_em
             FROM eventos
             WHERE publicado = true AND data_hora >= NOW()
             ORDER BY data_hora ASC
             LIMIT $1",
            limite
        )
        .fetch_all(self.db)
        .await?;
        Ok(eventos)
    }

    // Lista eventos publicados, paginado — usada na listagem pública /eventos.
    pub async fn listar_publicados_paginado(
        &self,
        pagina: i64,
        por_pagina: i64,
    ) -> Result<(Vec<Evento>, i64)> {
        let offset = (pagina - 1) * por_pagina;

        let eventos = sqlx::query_as!(
            Evento,
            "SELECT id, titulo, slug, descricao, data_hora, local, link_detalhes,
                    imagem_capa, publicado, criado_por, criado_em, atualizado_em
             FROM eventos
             WHERE publicado = true
             ORDER BY data_hora ASC
             LIMIT $1 OFFSET $2",
            por_pagina,
            offset
        )
        .fetch_all(self.db)
        .await?;

        let total = sqlx::query_scalar!("SELECT COUNT(*) FROM eventos WHERE publicado = true")
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);

        Ok((eventos, total))
    }

    // Busca pelo id — usada no handler de edição do admin.
    pub async fn buscar_por_id(&self, id: &str) -> Result<Evento> {
        let evento = sqlx::query_as!(
            Evento,
            "SELECT id, titulo, slug, descricao, data_hora, local, link_detalhes,
                    imagem_capa, publicado, criado_por, criado_em, atualizado_em
             FROM eventos
             WHERE id = $1",
            id
        )
        .fetch_optional(self.db)
        .await?
        .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(evento)
    }

    // Busca pelo slug — usada na rota pública /eventos/:slug. Só retorna publicados.
    pub async fn buscar_por_slug(&self, slug: &str) -> Result<Option<Evento>> {
        let evento = sqlx::query_as!(
            Evento,
            "SELECT id, titulo, slug, descricao, data_hora, local, link_detalhes,
                    imagem_capa, publicado, criado_por, criado_em, atualizado_em
             FROM eventos
             WHERE slug = $1 AND publicado = true",
            slug
        )
        .fetch_optional(self.db)
        .await?;
        Ok(evento)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn criar(
        &self,
        titulo: &str,
        slug: &str,
        descricao: Option<&str>,
        data_hora: chrono::DateTime<chrono::Utc>,
        local: Option<&str>,
        link_detalhes: Option<&str>,
        imagem_capa: Option<&str>,
        publicado: bool,
        criado_por: &str,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        sqlx::query!(
            "INSERT INTO eventos (id, titulo, slug, descricao, data_hora, local, link_detalhes, imagem_capa, publicado, criado_por)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            id,
            titulo,
            slug,
            descricao,
            data_hora,
            local,
            link_detalhes,
            imagem_capa,
            publicado,
            criado_por
        )
        .execute(self.db)
        .await?;
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn atualizar(
        &self,
        id: &str,
        titulo: &str,
        slug: &str,
        descricao: Option<&str>,
        data_hora: chrono::DateTime<chrono::Utc>,
        local: Option<&str>,
        link_detalhes: Option<&str>,
        imagem_capa: Option<&str>,
        publicado: bool,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE eventos
             SET titulo = $1, slug = $2, descricao = $3, data_hora = $4, local = $5,
                 link_detalhes = $6, imagem_capa = $7, publicado = $8, atualizado_em = NOW()
             WHERE id = $9",
            titulo,
            slug,
            descricao,
            data_hora,
            local,
            link_detalhes,
            imagem_capa,
            publicado,
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn deletar(&self, id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM eventos WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    // Mesmo padrão do slug_unico() de páginas e artigos.
    pub async fn slug_unico(&self, slug_base: &str, excluir_id: Option<&str>) -> String {
        let mut slug = slug_base.to_string();
        let mut contador = 1u32;

        loop {
            let existe: i64 = match excluir_id {
                Some(id) => sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM eventos WHERE slug = $1 AND id != $2",
                    slug,
                    id
                )
                .fetch_one(self.db)
                .await
                .unwrap_or(Some(0))
                .unwrap_or(0),

                None => sqlx::query_scalar!("SELECT COUNT(*) FROM eventos WHERE slug = $1", slug)
                    .fetch_one(self.db)
                    .await
                    .unwrap_or(Some(0))
                    .unwrap_or(0),
            };

            if existe == 0 {
                return slug;
            }

            slug = format!("{}-{}", slug_base, contador);
            contador += 1;
        }
    }
}
