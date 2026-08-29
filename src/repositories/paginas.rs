use crate::{error::Result, models::Pagina, sanitize::sanitizar_html};
use sqlx::PgPool;
use uuid::Uuid;

pub struct PaginasRepo<'a> {
    db: &'a PgPool,
}

impl<'a> PaginasRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    // Busca todas as páginas para o painel admin (sem filtro de publicada).
    pub async fn listar(&self) -> Result<Vec<Pagina>> {
        let paginas = sqlx::query_as!(
            Pagina,
            "SELECT id, titulo, slug, corpo, publicada, ordem, titulo_seo, criado_por,
                    criado_em, atualizado_em, html_bruto, restrito
             FROM paginas
             ORDER BY ordem ASC, criado_em ASC"
        )
        .fetch_all(self.db)
        .await?;
        Ok(paginas)
    }

    // Lista páginas publicadas e não restritas — usada no sitemap.xml, que é
    // sempre consumido anonimamente (crawlers) e não deve indexar conteúdo
    // restrito a membros.
    pub async fn listar_publicadas(&self) -> Result<Vec<Pagina>> {
        let paginas = sqlx::query_as!(
            Pagina,
            "SELECT id, titulo, slug, corpo, publicada, ordem, titulo_seo, criado_por,
                    criado_em, atualizado_em, html_bruto, restrito
             FROM paginas
             WHERE publicada = TRUE AND restrito = false
             ORDER BY ordem ASC, criado_em ASC"
        )
        .fetch_all(self.db)
        .await?;
        Ok(paginas)
    }

    // Busca uma página pelo slug — usada na rota pública /paginas/:slug.
    // Só retorna páginas publicadas.
    pub async fn buscar_por_slug(&self, slug: &str) -> Result<Option<Pagina>> {
        let pagina = sqlx::query_as!(
            Pagina,
            "SELECT id, titulo, slug, corpo, publicada, ordem, titulo_seo, criado_por,
                    criado_em, atualizado_em, html_bruto, restrito
             FROM paginas
             WHERE slug = $1 AND publicada = TRUE",
            slug
        )
        .fetch_optional(self.db)
        .await?;
        Ok(pagina)
    }

    // Busca pelo id — usada no handler de edição do admin.
    pub async fn buscar_por_id(&self, id: &str) -> Result<Pagina> {
        let pagina = sqlx::query_as!(
            Pagina,
            "SELECT id, titulo, slug, corpo, publicada, ordem, titulo_seo, criado_por,
                    criado_em, atualizado_em, html_bruto, restrito
             FROM paginas
             WHERE id = $1",
            id
        )
        .fetch_optional(self.db)
        .await?
        .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(pagina)
    }

    // Cria uma nova página. O slug único é gerado antes de chamar este método.
    #[allow(clippy::too_many_arguments)]
    pub async fn criar(
        &self,
        titulo: &str,
        slug: &str,
        corpo: &str,
        publicada: bool,
        ordem: i32,
        titulo_seo: Option<&str>,
        criado_por: &str,
        html_bruto: Option<&str>,
        restrito: bool,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let corpo = sanitizar_html(corpo);
        sqlx::query!(
            "INSERT INTO paginas (id, titulo, slug, corpo, publicada, ordem, titulo_seo, criado_por, html_bruto, restrito)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            id,
            titulo,
            slug,
            corpo,
            publicada,
            ordem,
            titulo_seo,
            criado_por,
            html_bruto,
            restrito
        )
        .execute(self.db)
        .await?;
        Ok(id)
    }

    // Atualiza uma página existente.
    // atualizado_em é setado explicitamente — PostgreSQL não tem ON UPDATE automático.
    #[allow(clippy::too_many_arguments)]
    pub async fn atualizar(
        &self,
        id: &str,
        titulo: &str,
        slug: &str,
        corpo: &str,
        publicada: bool,
        ordem: i32,
        titulo_seo: Option<&str>,
        html_bruto: Option<&str>,
        restrito: bool,
    ) -> Result<()> {
        let corpo = sanitizar_html(corpo);
        sqlx::query!(
            "UPDATE paginas
             SET titulo = $1, slug = $2, corpo = $3,
                 publicada = $4, ordem = $5, titulo_seo = $6,
                 html_bruto = $7, restrito = $8, atualizado_em = NOW()
             WHERE id = $9",
            titulo,
            slug,
            corpo,
            publicada,
            ordem,
            titulo_seo,
            html_bruto,
            restrito,
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn deletar(&self, id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM paginas WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    // Mesmo padrão do slug_unico() dos artigos — privado, sem propagar erro.
    pub async fn slug_unico(&self, slug_base: &str, excluir_id: Option<&str>) -> String {
        let mut slug = slug_base.to_string();
        let mut contador = 1u32;

        loop {
            let existe: i64 = match excluir_id {
                Some(id) => sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM paginas WHERE slug = $1 AND id != $2",
                    slug,
                    id
                )
                .fetch_one(self.db)
                .await
                .unwrap_or(Some(0))
                .unwrap_or(0),

                None => sqlx::query_scalar!("SELECT COUNT(*) FROM paginas WHERE slug = $1", slug)
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