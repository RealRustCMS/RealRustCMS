use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::Usuario,
    repositories::usuarios::UsuariosRepo,
};

pub struct AuthService<'a> {
    db: &'a PgPool,
}

impl<'a> AuthService<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    /// Cria um novo usuário.
    /// senha é Option<&str>: None para usuários OIDC-only (sem senha local).
    pub async fn criar_usuario(
        &self,
        nome: &str,
        email: &str,
        senha: Option<&str>,
        papel: &str,
    ) -> Result<Usuario> {
        let repo = UsuariosRepo::novo(self.db);

        if repo.buscar_por_email(email).await?.is_some() {
            return Err(AppError::Interno("E-mail já cadastrado".into()));
        }

        let id = Uuid::new_v4().to_string();

        let senha_hash: Option<String> = match senha {
            Some(s) => Some(hash_senha(s)?),
            None => None,
        };

        sqlx::query!(
            "INSERT INTO usuarios (id, nome, email, senha_hash, papel)
             VALUES ($1, $2, $3, $4, $5)",
            id,
            nome,
            email,
            senha_hash,
            papel
        )
        .execute(self.db)
        .await?;

        repo.buscar_por_id(&id).await?.ok_or(AppError::Interno(
            "Falha ao recuperar usuário criado".into(),
        ))
    }

    /// Valida e-mail e senha, retorna o usuário se correto.
    /// Bloqueia login local para usuários OIDC-only (senha_hash NULL).
    pub async fn login(&self, email: &str, senha: &str) -> Result<Usuario> {
        let repo = UsuariosRepo::novo(self.db);
        let usuario = repo.buscar_por_email(email).await?;

        // Hash fictício — garante que o Argon2 sempre roda mesmo quando
        // o usuário não existe ou é OIDC-only, evitando timing attacks.
        let hash_ficticio = "$argon2id$v=19$m=19456,t=2,p=1$\
            cmFuZG9tc2FsdHZhbHVl$\
            RG9lc05vdEV4aXN0SGFzaFZhbHVlRm9yVGltaW5n";

        let hash = match &usuario {
            Some(u) => u.senha_hash.as_deref().unwrap_or(hash_ficticio),
            None => hash_ficticio,
        };

        // Sempre roda o Argon2 — tempo de resposta constante.
        let _ = verificar_senha(senha, hash);

        match usuario {
            Some(u) => {
                let hash_real = u.senha_hash.as_deref().ok_or(AppError::NaoAutorizado)?;
                verificar_senha(senha, hash_real)?;
                Ok(u)
            }
            None => Err(AppError::NaoAutorizado),
        }
    }
}

// ─── HELPERS DE SENHA ────────────────────────────────────

pub fn hash_senha(senha: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(senha.as_bytes(), &salt)
        .map_err(|e| AppError::Interno(e.to_string()))?
        .to_string();
    Ok(hash)
}

pub fn verificar_senha(senha: &str, hash: &str) -> Result<()> {
    let parsed = PasswordHash::new(hash).map_err(|e| AppError::Interno(e.to_string()))?;
    Argon2::default()
        .verify_password(senha.as_bytes(), &parsed)
        .map_err(|_| AppError::NaoAutorizado)
}
