use crate::{
    error::AppError,
    models::{Membro, MembroListagem},
};
use sqlx::PgPool;

pub struct MembrosRepo<'a> {
    db: &'a PgPool,
}

impl<'a> MembrosRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    // ─── Criação ─────────────────────────────────────────────────────────────

    /// Cria membro com senha local (cadastro manual ou auto-cadastro público)
    pub async fn criar(
        &self,
        nome: &str,
        email: &str,
        senha_hash: &str,
    ) -> Result<Membro, AppError> {
        let membro = sqlx::query_as!(
            Membro,
            r#"
            INSERT INTO membros (nome, email, senha_hash)
            VALUES ($1, $2, $3)
            RETURNING
                id, nome, email, senha_hash, ativo,
                oauth_provider, oauth_sub, criado_em
            "#,
            nome,
            email,
            senha_hash,
        )
        .fetch_one(self.db)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_err)
                if db_err.constraint() == Some("membros_email_key") =>
            {
                AppError::Interno("E-mail já cadastrado.".into())
            }
            _ => AppError::Database(e),
        })?;

        Ok(membro)
    }

    /// Cria membro via SSO (sem senha local)
    pub async fn criar_via_oauth(
        &self,
        nome: &str,
        email: &str,
        provider: &str,
        sub: &str,
    ) -> Result<Membro, AppError> {
        let membro = sqlx::query_as!(
            Membro,
            r#"
            INSERT INTO membros (nome, email, oauth_provider, oauth_sub)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id, nome, email, senha_hash, ativo,
                oauth_provider, oauth_sub, criado_em
            "#,
            nome,
            email,
            provider,
            sub,
        )
        .fetch_one(self.db)
        .await
        .map_err(AppError::Database)?;

        Ok(membro)
    }

    /// Vincula provider SSO a um membro já existente (criado localmente)
    pub async fn vincular_oauth(&self, id: i64, provider: &str, sub: &str) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE membros
            SET oauth_provider = $1, oauth_sub = $2
            WHERE id = $3
            "#,
            provider,
            sub,
            id,
        )
        .execute(self.db)
        .await
        .map_err(AppError::Database)?;

        Ok(())
    }

    // ─── Buscas ──────────────────────────────────────────────────────────────

    pub async fn buscar_por_id(&self, id: i64) -> Result<Option<Membro>, AppError> {
        let membro = sqlx::query_as!(
            Membro,
            r#"
            SELECT id, nome, email, senha_hash, ativo,
                   oauth_provider, oauth_sub, criado_em
            FROM membros
            WHERE id = $1
            "#,
            id,
        )
        .fetch_optional(self.db)
        .await
        .map_err(AppError::Database)?;

        Ok(membro)
    }

    pub async fn buscar_por_email(&self, email: &str) -> Result<Option<Membro>, AppError> {
        let membro = sqlx::query_as!(
            Membro,
            r#"
            SELECT id, nome, email, senha_hash, ativo,
                   oauth_provider, oauth_sub, criado_em
            FROM membros
            WHERE email = $1
            "#,
            email,
        )
        .fetch_optional(self.db)
        .await
        .map_err(AppError::Database)?;

        Ok(membro)
    }

    pub async fn buscar_por_oauth(
        &self,
        provider: &str,
        sub: &str,
    ) -> Result<Option<Membro>, AppError> {
        let membro = sqlx::query_as!(
            Membro,
            r#"
            SELECT id, nome, email, senha_hash, ativo,
                   oauth_provider, oauth_sub, criado_em
            FROM membros
            WHERE oauth_provider = $1 AND oauth_sub = $2
            "#,
            provider,
            sub,
        )
        .fetch_optional(self.db)
        .await
        .map_err(AppError::Database)?;

        Ok(membro)
    }

    // ─── Verificações ────────────────────────────────────────────────────────

    /// Usado pelo middleware — evita carregar todos os campos só para checar ativo
    pub async fn esta_ativo(&self, id: i64) -> Result<bool, AppError> {
        let row = sqlx::query!("SELECT ativo FROM membros WHERE id = $1", id,)
            .fetch_optional(self.db)
            .await
            .map_err(AppError::Database)?;

        Ok(row.map(|r| r.ativo).unwrap_or(false))
    }

    pub async fn email_existe(&self, email: &str) -> Result<bool, AppError> {
        let row = sqlx::query!("SELECT id FROM membros WHERE email = $1", email,)
            .fetch_optional(self.db)
            .await
            .map_err(AppError::Database)?;

        Ok(row.is_some())
    }

    // ─── Admin ───────────────────────────────────────────────────────────────

    pub async fn listar(&self) -> Result<Vec<MembroListagem>, AppError> {
        let membros = sqlx::query_as!(
            MembroListagem,
            r#"
            SELECT id, nome, email, ativo, oauth_provider, criado_em
            FROM membros
            ORDER BY criado_em DESC
            "#,
        )
        .fetch_all(self.db)
        .await
        .map_err(AppError::Database)?;

        Ok(membros)
    }

    pub async fn atualizar(
        &self,
        id: i64,
        nome: &str,
        email: &str,
        ativo: bool,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            UPDATE membros
            SET nome = $1, email = $2, ativo = $3
            WHERE id = $4
            "#,
            nome,
            email,
            ativo,
            id,
        )
        .execute(self.db)
        .await
        .map_err(AppError::Database)?;

        Ok(())
    }

    pub async fn atualizar_perfil(&self, id: i64, nome: &str, email: &str) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE membros SET nome = $1, email = $2 WHERE id = $3",
            nome,
            email,
            id,
        )
        .execute(self.db)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_err)
                if db_err.constraint() == Some("membros_email_key") =>
            {
                AppError::Interno("E-mail já está em uso.".into())
            }
            _ => AppError::Database(e),
        })?;
        Ok(())
    }

    pub async fn atualizar_senha(&self, id: i64, senha_hash: &str) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE membros SET senha_hash = $1 WHERE id = $2",
            senha_hash,
            id,
        )
        .execute(self.db)
        .await
        .map_err(AppError::Database)?;
        Ok(())
    }

    pub async fn atualizar_nome(&self, id: i64, nome: &str) -> Result<(), AppError> {
        sqlx::query!("UPDATE membros SET nome = $1 WHERE id = $2", nome, id)
            .execute(self.db)
            .await
            .map_err(AppError::Database)?;
        Ok(())
    }

    pub async fn deletar(&self, id: i64) -> Result<(), AppError> {
        sqlx::query!("DELETE FROM membros WHERE id = $1", id)
            .execute(self.db)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }
}
