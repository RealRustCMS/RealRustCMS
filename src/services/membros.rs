use crate::{
    error::AppError,
    models::{Membro, NovoMembroForm},
    repositories::membros::MembrosRepo,
    services::auth::{hash_senha, verificar_senha},
};
use tower_sessions::Session;

pub struct MembrosService;

impl MembrosService {
    // ─── Cadastro local ──────────────────────────────────────────────────────

    pub async fn cadastrar(
        repo: &MembrosRepo<'_>,
        form: NovoMembroForm,
    ) -> Result<Membro, AppError> {
        let nome = form.nome.trim().to_string();
        let email = form.email.trim().to_lowercase();

        if nome.is_empty() {
            return Err(AppError::Interno("Nome não pode ser vazio.".into()));
        }
        if email.is_empty() || !email.contains('@') {
            return Err(AppError::Interno("E-mail inválido.".into()));
        }
        if form.senha.len() < 8 {
            return Err(AppError::Interno(
                "Senha deve ter no mínimo 8 caracteres.".into(),
            ));
        }

        if repo.email_existe(&email).await? {
            return Err(AppError::Interno("E-mail já cadastrado.".into()));
        }

        let hash = hash_senha(&form.senha)?;
        let membro = repo.criar(&nome, &email, &hash).await?;

        Ok(membro)
    }

    // ─── Login local ─────────────────────────────────────────────────────────

    pub async fn login(
        repo: &MembrosRepo<'_>,
        email: &str,
        senha: &str,
    ) -> Result<Membro, AppError> {
        let email = email.trim().to_lowercase();

        let membro = repo.buscar_por_email(&email).await?;

        // Hash fictício — Argon2 sempre roda (anti timing attack)
        let hash_dummy = "$argon2id$v=19$m=19456,t=2,p=1$\
            cmFuZG9tc2FsdHZhbHVl$\
            RG9lc05vdEV4aXN0SGFzaFZhbHVlRm9yVGltaW5n";

        let hash = membro
            .as_ref()
            .and_then(|m| m.senha_hash.as_deref())
            .unwrap_or(hash_dummy);

        // Sempre roda — tempo constante
        let _ = verificar_senha(senha, hash);

        match membro {
            Some(m) if m.ativo => {
                let hash_real = m.senha_hash.as_deref().ok_or(AppError::NaoAutorizado)?;
                verificar_senha(senha, hash_real)?;
                Ok(m)
            }
            Some(_) => Err(AppError::Interno("Conta desativada.".into())),
            None => Err(AppError::NaoAutorizado),
        }
    }

    // ─── Fluxo OAuth ─────────────────────────────────────────────────────────

    /// Decide o que fazer quando um provider SSO retorna email + sub.
    /// Retorna o membro autenticado ou None se não deve criar/logar.
    pub async fn resolver_oauth(
        repo: &MembrosRepo<'_>,
        provider: &str,
        sub: &str,
        email: &str,
        nome: &str,
        permitir_cadastro: bool,
        dominio_permitido: Option<&str>,
    ) -> Result<Option<Membro>, AppError> {
        // 1. já existe membro com esse provider+sub → loga direto
        if let Some(membro) = repo.buscar_por_oauth(provider, sub).await? {
            if membro.ativo {
                return Ok(Some(membro));
            } else {
                return Err(AppError::Interno("Conta desativada.".into()));
            }
        }

        // 2. existe membro com mesmo email (cadastrado manualmente) → vincula e loga
        if let Some(membro) = repo.buscar_por_email(email).await? {
            if !membro.ativo {
                return Err(AppError::Interno("Conta desativada.".into()));
            }
            repo.vincular_oauth(membro.id, provider, sub).await?;
            return Ok(Some(membro));
        }

        // 3. não existe → verificar se pode criar automaticamente
        if Self::email_permitido(email, permitir_cadastro, dominio_permitido) {
            let membro = repo.criar_via_oauth(nome, email, provider, sub).await?;
            return Ok(Some(membro));
        }

        // 4. não pode criar → não loga como membro
        Ok(None)
    }

    // ─── Helpers ─────────────────────────────────────────────────────────────

    /// Verifica se o email é permitido para auto-cadastro.
    /// Se `dominio_permitido` está configurado → só emails desse domínio.
    /// Se não → depende de `permitir_cadastro`.
    pub fn email_permitido(
        email: &str,
        permitir_cadastro: bool,
        dominio_permitido: Option<&str>,
    ) -> bool {
        if let Some(dominio) = dominio_permitido {
            email.ends_with(&format!("@{}", dominio))
        } else {
            permitir_cadastro
        }
    }

    /// Checagem rápida de "há uma sessão de membro/usuário ativa?", sem ida
    /// ao banco. Usada apenas para decisões de VISIBILIDADE (mostrar/ocultar
    /// conteúdo restrito em listagens e no menu) — nunca para bloqueio de
    /// acesso de fato.
    ///
    /// Diferença importante em relação ao middleware `requer_membro`:
    /// `requer_membro` consulta `esta_ativo()` no banco a cada request porque
    /// está decidindo se BLOQUEIA o acesso (e precisa refletir desativação em
    /// tempo real). Este helper só decide se um badge ou item de menu aparece;
    /// errar por uma fração de segundo após uma desativação não é falha de
    /// segurança, já que o middleware real ainda barra o acesso de verdade.
    /// Por isso aqui não vale o custo de uma query extra em todo request
    /// público.
    pub async fn sessao_membro_ativa(session: &Session) -> bool {
        let tem_membro = session
            .get::<i64>("membro_id")
            .await
            .unwrap_or(None)
            .is_some();

        let tem_usuario = session
            .get::<String>("usuario_id")
            .await
            .unwrap_or(None)
            .is_some();

        tem_membro || tem_usuario
    }
}