use crate::{
    error::{AppError, Result},
    models::{Artigo, NovoComentario},
    repositories::{comentarios::ComentariosRepo, configuracoes::ConfiguracoesRepo},
    services::email::EmailService,
};
use sqlx::PgPool;

pub struct ComentariosService<'a> {
    db: &'a PgPool,
    secret_key: Option<String>,
}

impl<'a> ComentariosService<'a> {
    pub fn novo(db: &'a PgPool, secret_key: Option<String>) -> Self {
        Self { db, secret_key }
    }

    pub async fn validar_captcha(&self, token: Option<&str>) -> Result<()> {
        let secret = match &self.secret_key {
            Some(s) => s.clone(),
            None => return Ok(()),
        };

        let token = token.ok_or_else(|| AppError::Interno("Token de captcha ausente.".into()))?;

        let client = reqwest::Client::new();
        let resposta = client
            .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
            .form(&[("secret", &secret), ("response", &token.to_string())])
            .send()
            .await
            .map_err(|e| AppError::Interno(e.to_string()))?;

        let json: serde_json::Value = resposta
            .json()
            .await
            .map_err(|e| AppError::Interno(e.to_string()))?;

        if json["success"].as_bool().unwrap_or(false) {
            Ok(())
        } else {
            Err(AppError::Interno("Verificação de captcha falhou.".into()))
        }
    }

    pub async fn criar_comentario(
        &self,
        url: &str,
        dados: &NovoComentario,
        moderacao_habilitada: bool,
        // Artigo e e-mail do autor para notificação — None desabilita
        artigo: Option<&Artigo>,
        email_autor: Option<&str>,
        smtp: Option<&crate::config::SmtpConfig>,
        base_url: &str,
    ) -> Result<bool> {
        self.validar_captcha(dados.turnstile_token.as_deref())
            .await?;

        let status = if moderacao_habilitada {
            "pendente"
        } else {
            "aprovado"
        };

        ComentariosRepo::novo(self.db)
            .criar(url, dados, status)
            .await?;

        // Notificar apenas quando moderação estiver ativa e o artigo pedir notificação
        if moderacao_habilitada {
            if let Some(artigo) = artigo {
                if artigo.notificar_comentarios {
                    self.notificar(artigo, dados, email_autor, smtp, base_url)
                        .await;
                }
            }
        }

        Ok(!moderacao_habilitada)
    }

    /// Dispara o e-mail de notificação.
    /// Falhas são apenas logadas — nunca propagam erro para o visitante.
    async fn notificar(
        &self,
        artigo: &Artigo,
        comentario: &NovoComentario,
        email_autor: Option<&str>,
        smtp: Option<&crate::config::SmtpConfig>,
        base_url: &str,
    ) {
        let smtp = match smtp {
            Some(s) => s,
            None => {
                tracing::debug!("SMTP não configurado — notificação ignorada");
                return;
            }
        };

        // Destino: e-mail do autor se disponível; senão, fallback da tabela configuracoes
        let destinatario = match email_autor {
            Some(e) if !e.is_empty() => e.to_string(),
            _ => {
                match ConfiguracoesRepo::novo(self.db)
                    .get("notif_email_fallback")
                    .await
                    .unwrap_or_default()
                {
                    Some(v) if !v.is_empty() => v,
                    _ => {
                        tracing::warn!(
                            artigo_id = %artigo.id,
                            "Nenhum e-mail de destino para notificação"
                        );
                        return;
                    }
                }
            }
        };

        let link_artigo = format!("{}/artigos/{}", base_url, artigo.slug);
        let link_admin = format!("{}/admin/comentarios", base_url);

        let assunto = format!(
            "[{}] Novo comentário aguardando aprovação: {}",
            "RustCMS", artigo.titulo
        );

        let corpo = format!(
            "Novo comentário aguardando aprovação em \"{titulo}\".\n\n\
             De: {nome} <{email}>\n\
             Mensagem:\n{mensagem}\n\n\
             Ver artigo: {link_artigo}\n\
             Moderar: {link_admin}",
            titulo = artigo.titulo,
            nome = comentario.autor_nome,
            email = comentario.autor_email,
            mensagem = comentario.corpo,
            link_artigo = link_artigo,
            link_admin = link_admin,
        );

        match EmailService::novo(smtp)
            .enviar(&destinatario, &assunto, &corpo)
            .await
        {
            Ok(_) => tracing::info!(
                artigo_id = %artigo.id,
                destinatario = %destinatario,
                "Notificação de comentário enviada"
            ),
            Err(e) => tracing::warn!(
                artigo_id = %artigo.id,
                erro = %e,
                "Falha ao enviar notificação de comentário"
            ),
        }
    }
}
