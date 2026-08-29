use rand;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::Result, models::Usuario};

pub struct UsuariosRepo<'a> {
    pub db: &'a PgPool,
}

impl<'a> UsuariosRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    pub async fn listar(&self) -> Result<Vec<Usuario>> {
        let usuarios = sqlx::query_as!(Usuario, "SELECT * FROM usuarios ORDER BY criado_em ASC")
            .fetch_all(self.db)
            .await?;
        Ok(usuarios)
    }

    pub async fn buscar_por_email(&self, email: &str) -> Result<Option<Usuario>> {
        // ILIKE garante busca case-insensitive — e-mails não diferenciam maiúsculas
        let usuario = sqlx::query_as!(
            Usuario,
            "SELECT * FROM usuarios WHERE email ILIKE $1",
            email
        )
        .fetch_optional(self.db)
        .await?;
        Ok(usuario)
    }

    pub async fn buscar_por_id(&self, id: &str) -> Result<Option<Usuario>> {
        let usuario = sqlx::query_as!(Usuario, "SELECT * FROM usuarios WHERE id = $1", id)
            .fetch_optional(self.db)
            .await?;
        Ok(usuario)
    }

    /// Busca usuário pela identidade do provedor OIDC.
    pub async fn buscar_por_oauth(&self, provider: &str, sub: &str) -> Result<Option<Usuario>> {
        let usuario = sqlx::query_as!(
            Usuario,
            "SELECT * FROM usuarios WHERE oauth_provider = $1 AND oauth_sub = $2",
            provider,
            sub
        )
        .fetch_optional(self.db)
        .await?;
        Ok(usuario)
    }

    /// Vincula um provedor OIDC a um usuário existente.
    pub async fn vincular_oauth(&self, id: &str, provider: &str, sub: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE usuarios SET oauth_provider = $1, oauth_sub = $2 WHERE id = $3",
            provider,
            sub,
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    /// Cria um usuário OIDC-only diretamente pelo repositório.
    pub async fn criar_via_oauth(
        &self,
        nome: &str,
        email: &str,
        provider: &str,
        sub: &str,
    ) -> Result<Usuario> {
        let id = Uuid::new_v4().to_string();

        sqlx::query!(
            "INSERT INTO usuarios (id, nome, email, senha_hash, papel, oauth_provider, oauth_sub)
             VALUES ($1, $2, $3, NULL, 'visualizador', $4, $5)",
            id,
            nome,
            email,
            provider,
            sub
        )
        .execute(self.db)
        .await?;

        self.buscar_por_id(&id)
            .await?
            .ok_or(crate::error::AppError::Interno(
                "Falha ao recuperar usuário criado via OAuth".into(),
            ))
    }

    pub async fn atualizar(&self, id: &str, nome: &str, email: &str, papel: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE usuarios SET nome = $1, email = $2, papel = $3 WHERE id = $4",
            nome,
            email,
            papel,
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn atualizar_senha(&self, id: &str, senha_hash: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE usuarios SET senha_hash = $1 WHERE id = $2",
            senha_hash,
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn deletar(&self, id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM usuarios WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    pub async fn total(&self) -> Result<i64> {
        let total = sqlx::query_scalar!("SELECT COUNT(*) FROM usuarios")
            .fetch_one(self.db)
            .await?
            .unwrap_or(0);
        Ok(total)
    }

    pub async fn gerar_token(&self, id: &str) -> Result<String> {
        let token: String = (0..32)
            .map(|_| format!("{:02x}", rand::random::<u8>()))
            .collect();

        sqlx::query!(
            "UPDATE usuarios SET api_token = $1 WHERE id = $2",
            token,
            id
        )
        .execute(self.db)
        .await?;

        Ok(token)
    }

    pub async fn revogar_token(&self, id: &str) -> Result<()> {
        sqlx::query!("UPDATE usuarios SET api_token = NULL WHERE id = $1", id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    // ─── MFA ─────────────────────────────────────────────────

    pub async fn habilitar_mfa(&self, id: &str, secret: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE usuarios SET mfa_secret = $1, mfa_habilitado = TRUE, mfa_obrigatorio = FALSE WHERE id = $2",
            secret,
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn desabilitar_mfa(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE usuarios SET mfa_secret = NULL, mfa_habilitado = FALSE WHERE id = $1",
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn exigir_mfa(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE usuarios SET mfa_obrigatorio = TRUE WHERE id = $1",
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    pub async fn remover_exigencia_mfa(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE usuarios SET mfa_obrigatorio = FALSE WHERE id = $1",
            id
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    /// Registra o uso de um código TOTP de forma atômica.
    /// Retorna `true` se o código foi aceito (novo), `false` se já foi usado
    /// dentro da janela de 30 segundos (replay bloqueado).
    pub async fn registrar_uso_mfa(&self, id: &str, codigo: &str) -> Result<bool> {
        let resultado = sqlx::query!(
            r#"
            UPDATE usuarios
            SET mfa_ultimo_codigo = $1,
                mfa_ultimo_uso    = NOW()
            WHERE id = $2
              AND (
                mfa_ultimo_codigo IS DISTINCT FROM $1
                OR mfa_ultimo_uso IS NULL
                OR mfa_ultimo_uso < NOW() - INTERVAL '30 seconds'
              )
            "#,
            codigo,
            id
        )
        .execute(self.db)
        .await?;

        Ok(resultado.rows_affected() > 0)
    }
}
