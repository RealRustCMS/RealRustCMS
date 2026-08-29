use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::Result,
    models::{Album, AlbumComCapa, Foto, NovoAlbum},
};

pub struct GaleriaRepo<'a> {
    pub db: &'a PgPool,
}

impl<'a> GaleriaRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    // ─── ÁLBUNS ──────────────────────────────────────────

    pub async fn listar_albuns(&self) -> Result<Vec<Album>> {
        let albuns = sqlx::query_as!(Album, "SELECT id, titulo, descricao, criado_em, criado_por FROM albuns ORDER BY criado_em DESC")
            .fetch_all(self.db)
            .await?;
        Ok(albuns)
    }

    pub async fn buscar_album(&self, id: &str) -> Result<Album> {
        let album = sqlx::query_as!(
            Album,
            "SELECT id, titulo, descricao, criado_em, criado_por FROM albuns WHERE id = $1",
            id
        )
        .fetch_optional(self.db)
        .await?
        .ok_or(crate::error::AppError::NaoEncontrado)?;
        Ok(album)
    }

    pub async fn criar_album(&self, dados: NovoAlbum, criado_por: &str) -> Result<Album> {
        let id = Uuid::new_v4().to_string();
        sqlx::query!(
            "INSERT INTO albuns (id, titulo, descricao, criado_por) VALUES ($1, $2, $3, $4)",
            id,
            dados.titulo,
            dados.descricao,
            criado_por
        )
        .execute(self.db)
        .await?;
        self.buscar_album(&id).await
    }

    pub async fn deletar_album(&self, id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM fotos WHERE album_id = $1", id)
            .execute(self.db)
            .await?;
        sqlx::query!("DELETE FROM albuns WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    // ─── FOTOS ───────────────────────────────────────────

    pub async fn listar_fotos_do_album(&self, album_id: &str) -> Result<Vec<Foto>> {
        let fotos = sqlx::query_as!(
            Foto,
            "SELECT * FROM fotos WHERE album_id = $1 ORDER BY criado_em DESC",
            album_id
        )
        .fetch_all(self.db)
        .await?;
        Ok(fotos)
    }

    pub async fn adicionar_foto(
        &self,
        url: &str,
        legenda: Option<String>,
        album_id: &str,
        criado_por: &str,
    ) -> Result<Foto> {
        let id = Uuid::new_v4().to_string();
        sqlx::query!(
            "INSERT INTO fotos (id, url, legenda, album_id, criado_por) VALUES ($1, $2, $3, $4, $5)",
            id,
            url,
            legenda,
            album_id,
            criado_por
        )
        .execute(self.db)
        .await?;

        let foto = sqlx::query_as!(Foto, "SELECT * FROM fotos WHERE id = $1", id)
            .fetch_one(self.db)
            .await?;
        Ok(foto)
    }

    pub async fn deletar_foto(&self, id: &str) -> Result<Option<String>> {
        let foto = sqlx::query_as!(Foto, "SELECT * FROM fotos WHERE id = $1", id)
            .fetch_optional(self.db)
            .await?;

        if let Some(f) = foto {
            sqlx::query!("DELETE FROM fotos WHERE id = $1", id)
                .execute(self.db)
                .await?;
            return Ok(Some(f.url));
        }
        Ok(None)
    }

    pub async fn listar_albuns_com_capa(&self) -> Result<Vec<AlbumComCapa>> {
        let albuns = self.listar_albuns().await?;
        let mut resultado = Vec::new();

        for album in albuns {
            let capa = sqlx::query_scalar!(
                "SELECT url FROM fotos WHERE album_id = $1 ORDER BY criado_em ASC LIMIT 1",
                album.id
            )
            .fetch_optional(self.db)
            .await?;

            resultado.push(AlbumComCapa {
                id: album.id,
                titulo: album.titulo,
                descricao: album.descricao,
                criado_em: album.criado_em,
                criado_por: album.criado_por,
                capa_url: capa,
            });
        }

        Ok(resultado)
    }
}
