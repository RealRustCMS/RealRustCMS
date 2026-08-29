use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::Result,
    models::{Artigo, ArtigoCompleto, Categoria, NovaCategoria, Tag},
    repositories::usuarios::UsuariosRepo,
};

pub struct CategoriasRepo<'a> {
    pub db: &'a PgPool,
}

impl<'a> CategoriasRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    pub async fn listar(&self) -> Result<Vec<Categoria>> {
        let cats = sqlx::query_as!(Categoria, "SELECT * FROM categorias ORDER BY nome ASC")
            .fetch_all(self.db)
            .await?;
        Ok(cats)
    }

    pub async fn buscar_por_slug(&self, slug: &str) -> Result<Categoria> {
        let cat = sqlx::query_as!(Categoria, "SELECT * FROM categorias WHERE slug = $1", slug)
            .fetch_optional(self.db)
            .await?
            .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(cat)
    }

    pub async fn buscar_por_id(&self, id: &str) -> Result<Categoria> {
        let cat = sqlx::query_as!(Categoria, "SELECT * FROM categorias WHERE id = $1", id)
            .fetch_optional(self.db)
            .await?
            .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(cat)
    }

    pub async fn criar(&self, dados: NovaCategoria) -> Result<Categoria> {
        let id = Uuid::new_v4().to_string();
        let slug = gerar_slug(&dados.nome);
        sqlx::query!(
            "INSERT INTO categorias (id, nome, slug) VALUES ($1, $2, $3)",
            id,
            dados.nome,
            slug
        )
        .execute(self.db)
        .await?;
        self.buscar_por_id(&id).await
    }

    pub async fn deletar(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE artigos SET categoria_id = NULL WHERE categoria_id = $1",
            id
        )
        .execute(self.db)
        .await?;
        sqlx::query!("DELETE FROM categorias WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    pub async fn artigos_da_categoria(
        &self,
        slug: &str,
        pagina: i64,
        por_pagina: i64,
        ocultar_restritos: bool,
    ) -> Result<(Vec<Artigo>, i64, Categoria)> {
        let cat = self.buscar_por_slug(slug).await?;

        let offset = (pagina - 1) * por_pagina;

        let (artigos, total) = if ocultar_restritos {
            let artigos = sqlx::query_as!(
                Artigo,
                "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id, comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas, resumo, imagem_capa, titulo_seo, destaque, ordem_destaque, notificar_comentarios, restrito, criado_em, publicado_em FROM artigos WHERE categoria_id = $1 AND status = 'publicado' AND restrito = false
                 ORDER BY publicado_em DESC LIMIT $2 OFFSET $3",
                cat.id,
                por_pagina,
                offset
            )
            .fetch_all(self.db)
            .await?;

            let total = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM artigos WHERE categoria_id = $1 AND status = 'publicado' AND restrito = false",
                cat.id
            )
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);

            (artigos, total)
        } else {
            let artigos = sqlx::query_as!(
                Artigo,
                "SELECT id, titulo, slug, corpo, status, autor_id, categoria_id, comentarios_habilitados, moderacao_habilitada, avaliacoes_habilitadas, resumo, imagem_capa, titulo_seo, destaque, ordem_destaque, notificar_comentarios, restrito, criado_em, publicado_em FROM artigos WHERE categoria_id = $1 AND status = 'publicado'
                 ORDER BY publicado_em DESC LIMIT $2 OFFSET $3",
                cat.id,
                por_pagina,
                offset
            )
            .fetch_all(self.db)
            .await?;

            let total = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM artigos WHERE categoria_id = $1 AND status = 'publicado'",
                cat.id
            )
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);

            (artigos, total)
        };

        Ok((artigos, total, cat))
    }
}

pub struct TagsRepo<'a> {
    pub db: &'a PgPool,
}

impl<'a> TagsRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    pub async fn listar(&self) -> Result<Vec<Tag>> {
        let tags = sqlx::query_as!(Tag, "SELECT * FROM tags ORDER BY nome ASC")
            .fetch_all(self.db)
            .await?;
        Ok(tags)
    }

    pub async fn buscar_por_id(&self, id: &str) -> Result<Tag> {
        let tag = sqlx::query_as!(Tag, "SELECT * FROM tags WHERE id = $1", id)
            .fetch_optional(self.db)
            .await?
            .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(tag)
    }

    pub async fn buscar_por_slug(&self, slug: &str) -> Result<Tag> {
        let tag = sqlx::query_as!(Tag, "SELECT * FROM tags WHERE slug = $1", slug)
            .fetch_optional(self.db)
            .await?
            .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(tag)
    }

    pub async fn criar(&self, dados: crate::models::NovaTag) -> Result<Tag> {
        let id = Uuid::new_v4().to_string();
        let slug = gerar_slug(&dados.nome);
        sqlx::query!(
            "INSERT INTO tags (id, nome, slug) VALUES ($1, $2, $3)",
            id,
            dados.nome,
            slug
        )
        .execute(self.db)
        .await?;
        self.buscar_por_id(&id).await
    }

    pub async fn deletar(&self, id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM artigo_tags WHERE tag_id = $1", id)
            .execute(self.db)
            .await?;
        sqlx::query!("DELETE FROM tags WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    pub async fn tags_do_artigo(&self, artigo_id: &str) -> Result<Vec<Tag>> {
        // "at" é palavra reservada no Postgres — usar alias explícito na join
        let tags = sqlx::query_as!(
            Tag,
            "SELECT t.id, t.nome, t.slug, t.criado_em FROM tags t
             INNER JOIN artigo_tags at2 ON at2.tag_id = t.id
             WHERE at2.artigo_id = $1
             ORDER BY t.nome ASC",
            artigo_id
        )
        .fetch_all(self.db)
        .await?;
        Ok(tags)
    }

    pub async fn sincronizar_tags(&self, artigo_id: &str, tag_ids: &[&str]) -> Result<()> {
        sqlx::query!("DELETE FROM artigo_tags WHERE artigo_id = $1", artigo_id)
            .execute(self.db)
            .await?;

        for tag_id in tag_ids {
            sqlx::query!(
                "INSERT INTO artigo_tags (artigo_id, tag_id) VALUES ($1, $2)",
                artigo_id,
                tag_id
            )
            .execute(self.db)
            .await?;
        }
        Ok(())
    }

    pub async fn artigos_da_tag(
        &self,
        slug: &str,
        pagina: i64,
        por_pagina: i64,
        ocultar_restritos: bool,
    ) -> Result<(Vec<Artigo>, i64, Tag)> {
        let tag = self.buscar_por_slug(slug).await?;

        let offset = (pagina - 1) * por_pagina;

        let (artigos, total) = if ocultar_restritos {
            let artigos = sqlx::query_as!(
                Artigo,
                "SELECT a.id, a.titulo, a.slug, a.corpo, a.status, a.autor_id, a.categoria_id, a.comentarios_habilitados, a.moderacao_habilitada, a.avaliacoes_habilitadas, a.resumo, a.imagem_capa, a.titulo_seo, a.destaque, a.ordem_destaque, a.notificar_comentarios, a.restrito, a.criado_em, a.publicado_em FROM artigos a
                 INNER JOIN artigo_tags at2 ON at2.artigo_id = a.id
                 WHERE at2.tag_id = $1 AND a.status = 'publicado' AND a.restrito = false
                 ORDER BY a.publicado_em DESC LIMIT $2 OFFSET $3",
                tag.id,
                por_pagina,
                offset
            )
            .fetch_all(self.db)
            .await?;

            let total = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM artigos a
                 INNER JOIN artigo_tags at2 ON at2.artigo_id = a.id
                 WHERE at2.tag_id = $1 AND a.status = 'publicado' AND a.restrito = false",
                tag.id
            )
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);

            (artigos, total)
        } else {
            let artigos = sqlx::query_as!(
                Artigo,
                "SELECT a.id, a.titulo, a.slug, a.corpo, a.status, a.autor_id, a.categoria_id, a.comentarios_habilitados, a.moderacao_habilitada, a.avaliacoes_habilitadas, a.resumo, a.imagem_capa, a.titulo_seo, a.destaque, a.ordem_destaque, a.notificar_comentarios, a.restrito, a.criado_em, a.publicado_em FROM artigos a
                 INNER JOIN artigo_tags at2 ON at2.artigo_id = a.id
                 WHERE at2.tag_id = $1 AND a.status = 'publicado'
                 ORDER BY a.publicado_em DESC LIMIT $2 OFFSET $3",
                tag.id,
                por_pagina,
                offset
            )
            .fetch_all(self.db)
            .await?;

            let total = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM artigos a
                 INNER JOIN artigo_tags at2 ON at2.artigo_id = a.id
                 WHERE at2.tag_id = $1 AND a.status = 'publicado'",
                tag.id
            )
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);

            (artigos, total)
        };

        Ok((artigos, total, tag))
    }
}

pub async fn artigo_completo(db: &PgPool, artigo: Artigo) -> ArtigoCompleto {
    let categoria = match &artigo.categoria_id {
        Some(cat_id) => CategoriasRepo::novo(db).buscar_por_id(cat_id).await.ok(),
        None => None,
    };

    let tags = TagsRepo::novo(db)
        .tags_do_artigo(&artigo.id)
        .await
        .unwrap_or_default();

    // autor_id sempre resolve (FK RESTRICT em usuarios) — fallback só por segurança.
    let autor_nome = UsuariosRepo::novo(db)
        .buscar_por_id(&artigo.autor_id)
        .await
        .ok()
        .flatten()
        .map(|u| u.nome)
        .unwrap_or_else(|| "Equipe".to_string());

    ArtigoCompleto {
        artigo,
        categoria,
        tags,
        autor_nome,
    }
}

fn gerar_slug(nome: &str) -> String {
    nome.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'a'..='z' | '0'..='9' => c,
            ' ' | '-' => '-',
            _ => '_',
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}