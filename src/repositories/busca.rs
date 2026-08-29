use crate::error::Result;
use serde::Serialize;
use sqlx::PgPool;

/// Resultado unificado de busca — artigos, álbuns e páginas estáticas.
/// `rank` é o score do ts_rank (0.0–1.0). Útil para debug e para
/// ordenação em templates futuros.
/// `trecho` já vem com os termos destacados via ts_headline no banco.
#[derive(Debug, Clone, Serialize)]
pub struct ResultadoBusca {
    pub tipo: String,
    pub titulo: String,
    pub url: String,
    pub descricao: Option<String>,
    pub rank: f32,
}

pub struct BuscaRepo<'a> {
    pub db: &'a PgPool,
}

impl<'a> BuscaRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    /// Busca pública — retorna apenas artigos publicados e páginas publicadas.
    /// Ordena por relevância (ts_rank) descendente.
    /// `ocultar_restritos` segue a mesma regra das demais listagens públicas:
    /// quem está logado vê tudo; sem sessão, respeita a configuração do admin.
    pub async fn buscar_publico(
        &self,
        termo: &str,
        ocultar_restritos: bool,
    ) -> Result<Vec<ResultadoBusca>> {
        // websearch_to_tsquery aceita linguagem natural:
        //   "acordo coletivo"  → frase exata
        //   acordo coletivo    → AND implícito
        //   acordo OR coletivo → OR explícito
        //   -acordo            → NOT
        // Retorna NULL para queries vazias/inválidas — o WHERE col @@ NULL
        // não casa com nenhuma linha, então a query simplesmente retorna vazio.
        let resultados: Vec<ResultadoBusca> = if ocultar_restritos {
            sqlx::query!(
                r#"
                SELECT tipo, titulo, url, trecho, rank FROM (

                    -- Artigos publicados (exclui restritos quando ocultar_restritos)
                    SELECT
                        'artigo'                               AS tipo,
                        titulo,
                        '/artigos/' || slug                    AS url,
                        ts_headline(
                            'portuguese',
                            coalesce(resumo,
                                left(regexp_replace(coalesce(corpo,''), '<[^>]+>', ' ', 'g'), 400)
                            ),
                            websearch_to_tsquery('portuguese', $1),
                            'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                        )                                      AS trecho,
                        ts_rank(busca_fts,
                            websearch_to_tsquery('portuguese', $1)
                        )                                      AS rank
                    FROM artigos
                    WHERE status = 'publicado'
                      AND restrito = false
                      AND busca_fts @@ websearch_to_tsquery('portuguese', $1)

                    UNION ALL

                    -- Páginas estáticas publicadas
                    SELECT
                        'pagina'                               AS tipo,
                        titulo,
                        '/paginas/' || slug                    AS url,
                        ts_headline(
                            'portuguese',
                            left(regexp_replace(coalesce(corpo,''), '<[^>]+>', ' ', 'g'), 400),
                            websearch_to_tsquery('portuguese', $1),
                            'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                        )                                      AS trecho,
                        ts_rank(busca_fts,
                            websearch_to_tsquery('portuguese', $1)
                        )                                      AS rank
                    FROM paginas
                    WHERE publicada = true
                      AND busca_fts @@ websearch_to_tsquery('portuguese', $1)

                    UNION ALL

                    -- Álbuns (sem filtro de status — todos são públicos)
                    SELECT
                        'album'                                AS tipo,
                        titulo,
                        '/galeria/' || id                      AS url,
                        ts_headline(
                            'portuguese',
                            coalesce(descricao, titulo),
                            websearch_to_tsquery('portuguese', $1),
                            'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                        )                                      AS trecho,
                        ts_rank(busca_fts,
                            websearch_to_tsquery('portuguese', $1)
                        )                                      AS rank
                    FROM albuns
                    WHERE busca_fts @@ websearch_to_tsquery('portuguese', $1)

                ) sub
                ORDER BY rank DESC
                LIMIT 30
                "#,
                termo
            )
            .fetch_all(self.db)
            .await?
            .into_iter()
            .map(|r| ResultadoBusca {
                tipo: r.tipo.unwrap_or_default(),
                titulo: r.titulo.unwrap_or_default(),
                url: r.url.unwrap_or_default(),
                descricao: r.trecho,
                rank: r.rank.unwrap_or(0.0),
            })
            .collect()
        } else {
            sqlx::query!(
                r#"
                SELECT tipo, titulo, url, trecho, rank FROM (

                    -- Artigos publicados
                    SELECT
                        'artigo'                               AS tipo,
                        titulo,
                        '/artigos/' || slug                    AS url,
                        ts_headline(
                            'portuguese',
                            coalesce(resumo,
                                left(regexp_replace(coalesce(corpo,''), '<[^>]+>', ' ', 'g'), 400)
                            ),
                            websearch_to_tsquery('portuguese', $1),
                            'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                        )                                      AS trecho,
                        ts_rank(busca_fts,
                            websearch_to_tsquery('portuguese', $1)
                        )                                      AS rank
                    FROM artigos
                    WHERE status = 'publicado'
                      AND busca_fts @@ websearch_to_tsquery('portuguese', $1)

                    UNION ALL

                    -- Páginas estáticas publicadas
                    SELECT
                        'pagina'                               AS tipo,
                        titulo,
                        '/paginas/' || slug                    AS url,
                        ts_headline(
                            'portuguese',
                            left(regexp_replace(coalesce(corpo,''), '<[^>]+>', ' ', 'g'), 400),
                            websearch_to_tsquery('portuguese', $1),
                            'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                        )                                      AS trecho,
                        ts_rank(busca_fts,
                            websearch_to_tsquery('portuguese', $1)
                        )                                      AS rank
                    FROM paginas
                    WHERE publicada = true
                      AND busca_fts @@ websearch_to_tsquery('portuguese', $1)

                    UNION ALL

                    -- Álbuns (sem filtro de status — todos são públicos)
                    SELECT
                        'album'                                AS tipo,
                        titulo,
                        '/galeria/' || id                      AS url,
                        ts_headline(
                            'portuguese',
                            coalesce(descricao, titulo),
                            websearch_to_tsquery('portuguese', $1),
                            'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                        )                                      AS trecho,
                        ts_rank(busca_fts,
                            websearch_to_tsquery('portuguese', $1)
                        )                                      AS rank
                    FROM albuns
                    WHERE busca_fts @@ websearch_to_tsquery('portuguese', $1)

                ) sub
                ORDER BY rank DESC
                LIMIT 30
                "#,
                termo
            )
            .fetch_all(self.db)
            .await?
            .into_iter()
            .map(|r| ResultadoBusca {
                tipo: r.tipo.unwrap_or_default(),
                titulo: r.titulo.unwrap_or_default(),
                url: r.url.unwrap_or_default(),
                descricao: r.trecho,
                rank: r.rank.unwrap_or(0.0),
            })
            .collect()
        };

        Ok(resultados)
    }

    /// Busca admin — inclui artigos em qualquer status (rascunho, publicado).
    /// O `tipo` carrega o status para o template exibir o badge correto:
    /// "artigo:publicado", "artigo:rascunho", "pagina", "album".
    pub async fn buscar_admin(&self, termo: &str) -> Result<Vec<ResultadoBusca>> {
        let resultados = sqlx::query!(
            r#"
            SELECT tipo, titulo, url, trecho, rank FROM (

                -- Artigos (todos os status)
                SELECT
                    'artigo:' || status                    AS tipo,
                    titulo,
                    '/artigos/' || slug                    AS url,
                    ts_headline(
                        'portuguese',
                        coalesce(resumo,
                            left(regexp_replace(coalesce(corpo,''), '<[^>]+>', ' ', 'g'), 400)
                        ),
                        websearch_to_tsquery('portuguese', $1),
                        'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                    )                                      AS trecho,
                    ts_rank(busca_fts,
                        websearch_to_tsquery('portuguese', $1)
                    )                                      AS rank
                FROM artigos
                WHERE busca_fts @@ websearch_to_tsquery('portuguese', $1)

                UNION ALL

                -- Páginas estáticas (todas — publicadas e rascunhos)
                SELECT
                    CASE WHEN publicada THEN 'pagina:publicada' ELSE 'pagina:rascunho' END
                                                           AS tipo,
                    titulo,
                    '/paginas/' || slug                    AS url,
                    ts_headline(
                        'portuguese',
                        left(regexp_replace(coalesce(corpo,''), '<[^>]+>', ' ', 'g'), 400),
                        websearch_to_tsquery('portuguese', $1),
                        'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                    )                                      AS trecho,
                    ts_rank(busca_fts,
                        websearch_to_tsquery('portuguese', $1)
                    )                                      AS rank
                FROM paginas
                WHERE busca_fts @@ websearch_to_tsquery('portuguese', $1)

                UNION ALL

                -- Álbuns
                SELECT
                    'album'                                AS tipo,
                    titulo,
                    '/galeria/' || id                      AS url,
                    ts_headline(
                        'portuguese',
                        coalesce(descricao, titulo),
                        websearch_to_tsquery('portuguese', $1),
                        'MaxWords=30, MinWords=15, StartSel=«, StopSel=»'
                    )                                      AS trecho,
                    ts_rank(busca_fts,
                        websearch_to_tsquery('portuguese', $1)
                    )                                      AS rank
                FROM albuns
                WHERE busca_fts @@ websearch_to_tsquery('portuguese', $1)

            ) sub
            ORDER BY rank DESC
            LIMIT 50
            "#,
            termo
        )
        .fetch_all(self.db)
        .await?;

        Ok(resultados
            .into_iter()
            .map(|r| ResultadoBusca {
                tipo: r.tipo.unwrap_or_default(),
                titulo: r.titulo.unwrap_or_default(),
                url: r.url.unwrap_or_default(),
                descricao: r.trecho,
                rank: r.rank.unwrap_or(0.0),
            })
            .collect())
    }
}