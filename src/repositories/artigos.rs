use crate::{
    error::Result,
    models::{Artigo, ArtigoListagem, ArtigoRelacionado, EditarArtigo, NovoArtigo},
    sanitize::sanitizar_html,
};
use sqlx::{PgPool, QueryBuilder};
use uuid::Uuid;

pub struct ArtigosRepo<'a> {
    pub db: &'a PgPool,
}

/// Parâmetros de filtragem e ordenação para a listagem admin.
/// Todos os campos são opcionais — ausente = sem filtro / padrão.
pub struct FiltrosArtigos {
    pub status: Option<String>,       // "publicado" | "rascunho"
    pub categoria_id: Option<String>, // UUID da categoria
    pub busca: Option<String>,        // ILIKE no título
    pub ordenar: Option<String>,      // "data_desc" | "data_asc" | "titulo"
    pub pagina: i64,
    pub por_pagina: i64,
}

impl<'a> ArtigosRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    async fn slug_unico(&self, slug_base: &str, excluir_id: Option<&str>) -> String {
        let mut slug = slug_base.to_string();
        let mut contador = 1u32;

        loop {
            let existe: i64 = match excluir_id {
                Some(id) => sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM artigos WHERE slug = $1 AND id != $2",
                    slug,
                    id
                )
                .fetch_one(self.db)
                .await
                .unwrap_or(Some(0))
                .unwrap_or(0),

                None => sqlx::query_scalar!("SELECT COUNT(*) FROM artigos WHERE slug = $1", slug)
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

    pub async fn listar_publicados(&self, ocultar_restritos: bool) -> Result<Vec<Artigo>> {
        let artigos = if ocultar_restritos {
            sqlx::query_as!(
                Artigo,
                "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id, comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas, resumo, imagem_capa, titulo_seo, destaque, ordem_destaque, notificar_comentarios, restrito, criado_em, publicado_em FROM artigos WHERE status = 'publicado' AND restrito = false ORDER BY publicado_em DESC"
            )
            .fetch_all(self.db)
            .await?
        } else {
            sqlx::query_as!(
                Artigo,
                "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id, comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas, resumo, imagem_capa, titulo_seo, destaque, ordem_destaque, notificar_comentarios, restrito, criado_em, publicado_em FROM artigos WHERE status = 'publicado' ORDER BY publicado_em DESC"
            )
            .fetch_all(self.db)
            .await?
        };
        Ok(artigos)
    }

    pub async fn listar_publicados_paginado(
        &self,
        pagina: i64,
        por_pagina: i64,
        ocultar_restritos: bool,
    ) -> Result<(Vec<Artigo>, i64)> {
        let offset = (pagina - 1) * por_pagina;

        let (artigos, total) = if ocultar_restritos {
            let artigos = sqlx::query_as!(
                Artigo,
                "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id,
                  comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas,
                  resumo, imagem_capa, titulo_seo, destaque, ordem_destaque,
                  notificar_comentarios, restrito, criado_em, publicado_em
                  FROM artigos WHERE status = 'publicado' AND restrito = false
                 ORDER BY publicado_em DESC
                 LIMIT $1 OFFSET $2",
                por_pagina,
                offset
            )
            .fetch_all(self.db)
            .await?;

            let total = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM artigos WHERE status = 'publicado' AND restrito = false"
            )
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);

            (artigos, total)
        } else {
            let artigos = sqlx::query_as!(
                Artigo,
                "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id,
                  comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas,
                  resumo, imagem_capa, titulo_seo, destaque, ordem_destaque,
                  notificar_comentarios, restrito, criado_em, publicado_em
                  FROM artigos WHERE status = 'publicado'
                 ORDER BY publicado_em DESC
                 LIMIT $1 OFFSET $2",
                por_pagina,
                offset
            )
            .fetch_all(self.db)
            .await?;

            let total =
                sqlx::query_scalar!("SELECT COUNT(*) FROM artigos WHERE status = 'publicado'")
                    .fetch_one(self.db)
                    .await?
                    .unwrap_or(0);

            (artigos, total)
        };

        Ok((artigos, total))
    }

    pub async fn listar_todos(&self) -> Result<Vec<Artigo>> {
        let artigos = sqlx::query_as!(Artigo, "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id, comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas, resumo, imagem_capa, titulo_seo, destaque, ordem_destaque, notificar_comentarios, restrito, criado_em, publicado_em FROM artigos ORDER BY criado_em DESC")
            .fetch_all(self.db)
            .await?;
        Ok(artigos)
    }

    /// Lista artigos com filtros e ordenação aplicados no banco.
    /// Usa QueryBuilder porque o WHERE é dinâmico — query_as! exige SQL estático.
    /// ORDER BY não aceita bind param no Postgres; o valor vem de um match interno
    /// que mapeia strings conhecidas para SQL fixo (sem risco de injection).
    pub async fn listar_filtrados(&self, filtros: FiltrosArtigos) -> Result<(Vec<Artigo>, i64)> {
        let offset = (filtros.pagina - 1) * filtros.por_pagina;

        // Colunas explícitas — SELECT * proibido pois artigos tem coluna busca_fts (tsvector)
        let colunas = "id, titulo, slug, corpo, status, autor_id, categoria_id,
                       comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas,
                       resumo, imagem_capa, titulo_seo, destaque, ordem_destaque,
                       notificar_comentarios, restrito, criado_em, publicado_em";

        let ordenacao = match filtros.ordenar.as_deref().unwrap_or("data_desc") {
            "data_asc" => "criado_em ASC",
            "titulo" => "titulo ASC",
            _ => "criado_em DESC", // padrão e "data_desc"
        };

        // ── Query principal ──────────────────────────────────────────────────
        let mut qb = QueryBuilder::new(format!("SELECT {colunas} FROM artigos WHERE 1=1"));

        if let Some(ref s) = filtros.status {
            qb.push(" AND status = ").push_bind(s.clone());
        }
        if let Some(ref cat) = filtros.categoria_id {
            qb.push(" AND categoria_id = ").push_bind(cat.clone());
        }
        if let Some(ref busca) = filtros.busca {
            let padrao = format!("%{}%", busca);
            qb.push(" AND titulo ILIKE ").push_bind(padrao);
        }

        // ORDER BY com valor controlado internamente (sem bind — Postgres não aceita bind em ORDER BY)
        qb.push(format!(" ORDER BY {ordenacao} LIMIT "));
        qb.push_bind(filtros.por_pagina);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let artigos = qb.build_query_as::<Artigo>().fetch_all(self.db).await?;

        // ── Contagem com os mesmos filtros (sem LIMIT/OFFSET) ────────────────
        let mut qb_count = QueryBuilder::new("SELECT COUNT(*) FROM artigos WHERE 1=1");

        if let Some(ref s) = filtros.status {
            qb_count.push(" AND status = ").push_bind(s.clone());
        }
        if let Some(ref cat) = filtros.categoria_id {
            qb_count.push(" AND categoria_id = ").push_bind(cat.clone());
        }
        if let Some(ref busca) = filtros.busca {
            let padrao = format!("%{}%", busca);
            qb_count.push(" AND titulo ILIKE ").push_bind(padrao);
        }

        let total: i64 = qb_count
            .build_query_scalar()
            .fetch_one(self.db)
            .await
            .unwrap_or(0);

        Ok((artigos, total))
    }

    pub async fn buscar_por_slug(&self, slug: &str) -> Result<Artigo> {
        let artigo = sqlx::query_as!(Artigo, "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id, comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas, resumo, imagem_capa, titulo_seo, destaque, ordem_destaque, notificar_comentarios, restrito, criado_em, publicado_em FROM artigos WHERE slug = $1", slug)
            .fetch_optional(self.db)
            .await?
            .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(artigo)
    }

    pub async fn buscar_por_id(&self, id: &str) -> Result<Artigo> {
        let artigo = sqlx::query_as!(Artigo, "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id, comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas, resumo, imagem_capa, titulo_seo, destaque, ordem_destaque, notificar_comentarios, restrito, criado_em, publicado_em FROM artigos WHERE id = $1", id)
            .fetch_optional(self.db)
            .await?
            .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(artigo)
    }

    pub async fn buscar_autor(&self, id: &str) -> Result<String> {
        let autor_id = sqlx::query_scalar!("SELECT autor_id FROM artigos WHERE id = $1", id)
            .fetch_optional(self.db)
            .await?
            .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(autor_id)
    }

    pub async fn criar(&self, dados: NovoArtigo, autor_id: &str) -> Result<Artigo> {
        let id = Uuid::new_v4().to_string();
        let slug_base = gerar_slug(&dados.titulo);
        let slug = self.slug_unico(&slug_base, None).await;

        let publicado_em = if dados.status == "publicado" {
            Some(chrono::Utc::now())
        } else {
            None
        };

        let categoria_id = dados.categoria_id.filter(|s| !s.is_empty());
        let comentarios_habilitados = dados.comentarios_habilitados.is_some();
        let moderacao_habilitada = dados.moderacao_habilitada.is_some();
        let avaliacoes_habilitadas = dados.avaliacoes_habilitadas.is_some();
        let destaque = dados.destaque.is_some();
        let notificar_comentarios = dados.notificar_comentarios.is_some();
        let restrito = dados.restrito.is_some();
        let resumo = dados.resumo.filter(|s| !s.is_empty());
        let imagem_capa = dados.imagem_capa.filter(|s| !s.is_empty());
        let titulo_seo = dados.titulo_seo.filter(|s| !s.is_empty());
        let corpo = sanitizar_html(&dados.corpo);

        sqlx::query!(
            "INSERT INTO artigos (id, titulo, slug, corpo, resumo, imagem_capa, titulo_seo,
             status, autor_id, categoria_id,
             comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas,
             destaque, notificar_comentarios, restrito, publicado_em)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
            id,
            dados.titulo,
            slug,
            corpo,
            resumo,
            imagem_capa,
            titulo_seo,
            dados.status,
            autor_id,
            categoria_id,
            comentarios_habilitados,
            moderacao_habilitada,
            avaliacoes_habilitadas,
            destaque,
            notificar_comentarios,
            restrito,
            publicado_em
        )
        .execute(self.db)
        .await?;

        self.buscar_por_id(&id).await
    }

    pub async fn editar(&self, id: &str, dados: EditarArtigo) -> Result<Artigo> {
        let publicado_em = if dados.status == "publicado" {
            let atual = self.buscar_por_id(id).await?;
            atual.publicado_em.or_else(|| Some(chrono::Utc::now()))
        } else {
            None
        };

        let slug_base = gerar_slug(&dados.titulo);
        let slug = self.slug_unico(&slug_base, Some(id)).await;
        let categoria_id = dados.categoria_id.filter(|s| !s.is_empty());
        let comentarios_habilitados = dados.comentarios_habilitados.is_some();
        let moderacao_habilitada = dados.moderacao_habilitada.is_some();
        let avaliacoes_habilitadas = dados.avaliacoes_habilitadas.is_some();
        let destaque = dados.destaque.is_some();
        let notificar_comentarios = dados.notificar_comentarios.is_some();
        let restrito = dados.restrito.is_some();
        let resumo = dados.resumo.filter(|s| !s.is_empty());
        let imagem_capa = dados.imagem_capa.filter(|s| !s.is_empty());
        let titulo_seo = dados.titulo_seo.filter(|s| !s.is_empty());
        let corpo = sanitizar_html(&dados.corpo);

        sqlx::query!(
            "UPDATE artigos SET titulo = $1, slug = $2, corpo = $3, resumo = $4, imagem_capa = $5,
             titulo_seo = $6, status = $7, categoria_id = $8,
             comentarios_habilitados = $9, moderacao_habilitada = $10,
             avaliacoes_habilitadas = $11, destaque = $12, notificar_comentarios = $13,
             restrito = $14, publicado_em = $15
             WHERE id = $16",
            dados.titulo,
            slug,
            corpo,
            resumo,
            imagem_capa,
            titulo_seo,
            dados.status,
            categoria_id,
            comentarios_habilitados,
            moderacao_habilitada,
            avaliacoes_habilitadas,
            destaque,
            notificar_comentarios,
            restrito,
            publicado_em,
            id
        )
        .execute(self.db)
        .await?;

        self.buscar_por_id(id).await
    }

    pub async fn deletar(&self, id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM artigos WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    /// Lista artigos em destaque ordenados por ordem_destaque.
    /// O primeiro item ([0]) é o hero; os demais são secundários.
    pub async fn listar_destaques(
        &self,
        limite: usize,
        ocultar_restritos: bool,
    ) -> Result<Vec<ArtigoListagem>> {
        let limite = limite as i64;
        let listagem: Vec<ArtigoListagem> = if ocultar_restritos {
            sqlx::query!(
                "SELECT a.id, a.titulo, a.slug, a.corpo, a.status, a.autor_id, a.categoria_id,
                  a.comentarios_habilitados, a.moderacao_habilitada, a.avaliacoes_habilitadas,
                  a.resumo, a.imagem_capa, a.titulo_seo, a.destaque, a.ordem_destaque,
                  a.notificar_comentarios, a.restrito, a.criado_em, a.publicado_em,
                  c.nome AS \"categoria_nome?\"
                  FROM artigos a
                  LEFT JOIN categorias c ON c.id = a.categoria_id
                 WHERE a.destaque = true AND a.status = 'publicado' AND a.restrito = false
                 ORDER BY a.ordem_destaque ASC NULLS LAST, a.publicado_em DESC
                 LIMIT $1",
                limite
            )
            .fetch_all(self.db)
            .await?
            .into_iter()
            .map(|r| {
                let artigo = Artigo {
                    id: r.id,
                    titulo: r.titulo,
                    slug: r.slug,
                    corpo: r.corpo,
                    status: r.status,
                    autor_id: r.autor_id,
                    categoria_id: r.categoria_id,
                    comentarios_habilitados: r.comentarios_habilitados,
                    moderacao_habilitada: r.moderacao_habilitada,
                    avaliacoes_habilitadas: r.avaliacoes_habilitadas,
                    resumo: r.resumo,
                    imagem_capa: r.imagem_capa,
                    titulo_seo: r.titulo_seo,
                    destaque: r.destaque,
                    ordem_destaque: r.ordem_destaque,
                    notificar_comentarios: r.notificar_comentarios,
                    restrito: r.restrito,
                    criado_em: r.criado_em,
                    publicado_em: r.publicado_em,
                };
                ArtigoListagem::from_artigo(artigo, None, r.categoria_nome)
            })
            .collect()
        } else {
            sqlx::query!(
                "SELECT a.id, a.titulo, a.slug, a.corpo, a.status, a.autor_id, a.categoria_id,
                  a.comentarios_habilitados, a.moderacao_habilitada, a.avaliacoes_habilitadas,
                  a.resumo, a.imagem_capa, a.titulo_seo, a.destaque, a.ordem_destaque,
                  a.notificar_comentarios, a.restrito, a.criado_em, a.publicado_em,
                  c.nome AS \"categoria_nome?\"
                  FROM artigos a
                  LEFT JOIN categorias c ON c.id = a.categoria_id
                 WHERE a.destaque = true AND a.status = 'publicado'
                 ORDER BY a.ordem_destaque ASC NULLS LAST, a.publicado_em DESC
                 LIMIT $1",
                limite
            )
            .fetch_all(self.db)
            .await?
            .into_iter()
            .map(|r| {
                let artigo = Artigo {
                    id: r.id,
                    titulo: r.titulo,
                    slug: r.slug,
                    corpo: r.corpo,
                    status: r.status,
                    autor_id: r.autor_id,
                    categoria_id: r.categoria_id,
                    comentarios_habilitados: r.comentarios_habilitados,
                    moderacao_habilitada: r.moderacao_habilitada,
                    avaliacoes_habilitadas: r.avaliacoes_habilitadas,
                    resumo: r.resumo,
                    imagem_capa: r.imagem_capa,
                    titulo_seo: r.titulo_seo,
                    destaque: r.destaque,
                    ordem_destaque: r.ordem_destaque,
                    notificar_comentarios: r.notificar_comentarios,
                    restrito: r.restrito,
                    criado_em: r.criado_em,
                    publicado_em: r.publicado_em,
                };
                ArtigoListagem::from_artigo(artigo, None, r.categoria_nome)
            })
            .collect()
        };
        Ok(listagem)
    }

    /// Alterna o flag destaque de um artigo.
    /// Quando ativa (true), atribui ordem_destaque = próximo número livre.
    /// Quando desativa (false), zera a ordem para não ocupar posição.
    pub async fn toggle_destaque(&self, id: &str) -> Result<bool> {
        let atual = sqlx::query_scalar!("SELECT destaque FROM artigos WHERE id = $1", id)
            .fetch_optional(self.db)
            .await?
            .ok_or(crate::error::AppError::NaoEncontrado)?;

        let novo = !atual;

        if novo {
            let proxima: i32 = sqlx::query_scalar!(
                "SELECT COALESCE(MAX(ordem_destaque), 0) + 1 FROM artigos WHERE destaque = true"
            )
            .fetch_one(self.db)
            .await?
            .unwrap_or(1);

            sqlx::query!(
                "UPDATE artigos SET destaque = true, ordem_destaque = $1 WHERE id = $2",
                proxima,
                id
            )
            .execute(self.db)
            .await?;
        } else {
            sqlx::query!(
                "UPDATE artigos SET destaque = false, ordem_destaque = NULL WHERE id = $1",
                id
            )
            .execute(self.db)
            .await?;
        }

        Ok(novo)
    }

    /// Busca artigos relacionados ao artigo dado.
    /// Estratégia: tags em comum primeiro; completa com categoria se necessário.
    /// Nunca inclui o artigo atual. Retorna no máximo `limite` artigos.
    /// Atualiza o status de múltiplos artigos em um único round-trip.
    /// Usa ANY($1) com array de strings — equivalente ao IN mas com bind param tipado.
    /// Retorna quantas linhas foram afetadas.
    /// Atualiza o status de múltiplos artigos em um único round-trip.
    /// Duas queries separadas evitam ambiguidade de tipo do SQLx com CASE/parâmetros mistos.
    pub async fn atualizar_status_bulk(&self, ids: &[String], status: &str) -> Result<u64> {
        let resultado = if status == "publicado" {
            // Publica: preenche publicado_em apenas se ainda era NULL
            sqlx::query!(
                "UPDATE artigos
                 SET status = 'publicado',
                     publicado_em = COALESCE(publicado_em, NOW())
                 WHERE id = ANY($1)",
                ids
            )
            .execute(self.db)
            .await?
        } else {
            // Despublica: zera publicado_em
            sqlx::query!(
                "UPDATE artigos
                 SET status = 'rascunho',
                     publicado_em = NULL
                 WHERE id = ANY($1)",
                ids
            )
            .execute(self.db)
            .await?
        };
        Ok(resultado.rows_affected())
    }

    pub async fn buscar_relacionados(
        &self,
        artigo_id: &str,
        categoria_id: Option<&str>,
        limite: usize,
        ocultar_restritos: bool,
    ) -> Result<Vec<ArtigoRelacionado>> {
        let limite = limite as i64;

        let mut resultado: Vec<ArtigoRelacionado> = if ocultar_restritos {
            sqlx::query!(
                r#"
                SELECT a.id, a.titulo, a.slug, a.resumo, a.imagem_capa, a.publicado_em,
                       COUNT(at2.tag_id) AS tags_comuns
                FROM artigos a
                JOIN artigo_tags at2 ON at2.artigo_id = a.id
                WHERE a.id != $1
                  AND a.status = 'publicado'
                  AND a.restrito = false
                  AND at2.tag_id IN (
                      SELECT tag_id FROM artigo_tags WHERE artigo_id = $1
                  )
                GROUP BY a.id, a.titulo, a.slug, a.resumo, a.imagem_capa, a.publicado_em
                ORDER BY tags_comuns DESC, a.publicado_em DESC
                LIMIT $2
                "#,
                artigo_id,
                limite
            )
            .fetch_all(self.db)
            .await?
            .into_iter()
            .map(|r| ArtigoRelacionado {
                id: r.id,
                titulo: r.titulo,
                slug: r.slug,
                resumo: r.resumo,
                imagem_capa: r.imagem_capa,
                publicado_em: r.publicado_em,
            })
            .collect()
        } else {
            sqlx::query!(
                r#"
                SELECT a.id, a.titulo, a.slug, a.resumo, a.imagem_capa, a.publicado_em,
                       COUNT(at2.tag_id) AS tags_comuns
                FROM artigos a
                JOIN artigo_tags at2 ON at2.artigo_id = a.id
                WHERE a.id != $1
                  AND a.status = 'publicado'
                  AND at2.tag_id IN (
                      SELECT tag_id FROM artigo_tags WHERE artigo_id = $1
                  )
                GROUP BY a.id, a.titulo, a.slug, a.resumo, a.imagem_capa, a.publicado_em
                ORDER BY tags_comuns DESC, a.publicado_em DESC
                LIMIT $2
                "#,
                artigo_id,
                limite
            )
            .fetch_all(self.db)
            .await?
            .into_iter()
            .map(|r| ArtigoRelacionado {
                id: r.id,
                titulo: r.titulo,
                slug: r.slug,
                resumo: r.resumo,
                imagem_capa: r.imagem_capa,
                publicado_em: r.publicado_em,
            })
            .collect()
        };

        if resultado.len() < limite as usize {
            if let Some(cat_id) = categoria_id {
                let ids_ja: Vec<String> = resultado.iter().map(|a| a.id.clone()).collect();
                let faltam = limite - resultado.len() as i64;

                if ocultar_restritos {
                    sqlx::query!(
                        r#"
                        SELECT id, titulo, slug, resumo, imagem_capa, publicado_em
                        FROM artigos
                        WHERE id != $1
                          AND status = 'publicado'
                          AND restrito = false
                          AND categoria_id = $2
                          AND id != ALL($3)
                        ORDER BY publicado_em DESC
                        LIMIT $4
                        "#,
                        artigo_id,
                        cat_id,
                        &ids_ja,
                        faltam
                    )
                    .fetch_all(self.db)
                    .await?
                    .into_iter()
                    .for_each(|r| {
                        resultado.push(ArtigoRelacionado {
                            id: r.id,
                            titulo: r.titulo,
                            slug: r.slug,
                            resumo: r.resumo,
                            imagem_capa: r.imagem_capa,
                            publicado_em: r.publicado_em,
                        });
                    });
                } else {
                    sqlx::query!(
                        r#"
                        SELECT id, titulo, slug, resumo, imagem_capa, publicado_em
                        FROM artigos
                        WHERE id != $1
                          AND status = 'publicado'
                          AND categoria_id = $2
                          AND id != ALL($3)
                        ORDER BY publicado_em DESC
                        LIMIT $4
                        "#,
                        artigo_id,
                        cat_id,
                        &ids_ja,
                        faltam
                    )
                    .fetch_all(self.db)
                    .await?
                    .into_iter()
                    .for_each(|r| {
                        resultado.push(ArtigoRelacionado {
                            id: r.id,
                            titulo: r.titulo,
                            slug: r.slug,
                            resumo: r.resumo,
                            imagem_capa: r.imagem_capa,
                            publicado_em: r.publicado_em,
                        });
                    });
                }
            }
        }

        Ok(resultado)
    }
}

fn gerar_slug(titulo: &str) -> String {
    titulo
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            'á' | 'à' | 'ã' | 'â' | 'ä' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' => Some('e'),
            'í' | 'ì' | 'î' | 'ï' => Some('i'),
            'ó' | 'ò' | 'õ' | 'ô' | 'ö' => Some('o'),
            'ú' | 'ù' | 'û' | 'ü' => Some('u'),
            'ç' => Some('c'),
            'ñ' => Some('n'),
            'a'..='z' | '0'..='9' => Some(c),
            ' ' | '-' => Some('-'),
            _ => None,
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}