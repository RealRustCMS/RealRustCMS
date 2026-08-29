use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{ArtigoAvaliado, AvaliacaoStats};

pub struct AvaliacoesRepo<'a> {
    db: &'a PgPool,
}

impl<'a> AvaliacoesRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    /// Registra um voto. Retorna Ok(true) se foi o primeiro voto deste IP,
    /// Ok(false) se já tinha votado (duplicata ignorada silenciosamente).
    /// ON CONFLICT DO NOTHING é o equivalente Postgres do INSERT IGNORE do MySQL.
    pub async fn votar(&self, artigo_id: &str, nota: u8, ip: &str) -> Result<bool, sqlx::Error> {
        let nota = nota.clamp(1, 5) as i16;

        let resultado = sqlx::query!(
            "INSERT INTO avaliacoes (id, artigo_id, nota, ip)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (artigo_id, ip) DO NOTHING",
            Uuid::new_v4().to_string(),
            artigo_id,
            nota,
            ip,
        )
        .execute(self.db)
        .await?;

        Ok(resultado.rows_affected() > 0)
    }

    /// Busca média e total de votos para um artigo.
    /// No Postgres, AVG() retorna NUMERIC/DOUBLE diretamente — sem necessidade
    /// de CAST manual como era no MySQL para evitar DECIMAL.
    pub async fn buscar_stats(
        &self,
        artigo_id: &str,
    ) -> Result<Option<AvaliacaoStats>, sqlx::Error> {
        let row = sqlx::query!(
            "SELECT AVG(nota::float8) as media, COUNT(*) as total
             FROM avaliacoes
             WHERE artigo_id = $1",
            artigo_id,
        )
        .fetch_one(self.db)
        .await?;

        match row.media {
            Some(media) => Ok(Some(AvaliacaoStats::new(media, row.total.unwrap_or(0)))),
            None => Ok(None),
        }
    }

    /// Busca stats de múltiplos artigos — usado na listagem pública e nas páginas
    /// de categoria/tag. Retorna apenas artigos que têm pelo menos um voto.
    ///
    /// Usa `query_as` com struct auxiliar porque o SQL é construído dinamicamente
    /// (cláusula IN com N placeholders) — a macro `query!` exige string literal.
    /// No Postgres os placeholders são $1, $2... então geramos a lista dinamicamente.
    pub async fn buscar_stats_multiplos(
        &self,
        artigo_ids: &[String],
    ) -> Result<Vec<(String, AvaliacaoStats)>, sqlx::Error> {
        if artigo_ids.is_empty() {
            return Ok(vec![]);
        }

        // Postgres: $1, $2, $3... em vez de ?, ?, ?
        let placeholders = artigo_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "SELECT artigo_id,
                    AVG(nota::float8) as media,
                    COUNT(*) as total
             FROM avaliacoes
             WHERE artigo_id IN ({})
             GROUP BY artigo_id",
            placeholders
        );

        let mut q = sqlx::query_as::<_, AvaliacaoStatsRow>(&sql);
        for id in artigo_ids {
            q = q.bind(id);
        }

        let rows = q.fetch_all(self.db).await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.media.map(|media| {
                    (
                        r.artigo_id,
                        AvaliacaoStats::new(media, r.total.unwrap_or(0)),
                    )
                })
            })
            .collect())
    }

    /// Retorna os N artigos mais bem avaliados, com título e slug.
    /// Usado no dashboard admin.
    pub async fn mais_bem_avaliados(
        &self,
        limite: i64,
        minimo_votos: i64,
    ) -> Result<Vec<ArtigoAvaliado>, sqlx::Error> {
        // query! em vez de query_as! para controlar o mapeamento do COUNT manualmente.
        // O Postgres reporta COUNT como nullable mesmo com COALESCE — o query_as! não
        // consegue forçar i64 não-nullable via annotation nesse caso.
        let rows = sqlx::query!(
            r#"SELECT
                   a.id,
                   a.titulo,
                   a.slug,
                   AVG(av.nota::float8) AS media,
                   COUNT(av.id)         AS total
               FROM avaliacoes av
               JOIN artigos a ON a.id = av.artigo_id
               WHERE a.status = 'publicado'
               GROUP BY a.id, a.titulo, a.slug
               HAVING COUNT(av.id) >= $1
               ORDER BY AVG(av.nota::float8) DESC, COUNT(av.id) DESC
               LIMIT $2"#,
            minimo_votos,
            limite,
        )
        .fetch_all(self.db)
        .await?;

        let resultado = rows
            .into_iter()
            .map(|r| ArtigoAvaliado {
                id: r.id,
                titulo: r.titulo,
                slug: r.slug,
                media: r.media,
                total: r.total.unwrap_or(0),
            })
            .collect();

        Ok(resultado)
    }
}

/// Struct auxiliar para `query_as` no `buscar_stats_multiplos`.
#[derive(sqlx::FromRow)]
struct AvaliacaoStatsRow {
    artigo_id: String,
    media: Option<f64>,
    total: Option<i64>,
}
