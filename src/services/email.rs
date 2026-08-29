use crate::{config::SmtpConfig, error::AppError};
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};

pub struct EmailService<'a> {
    smtp: &'a SmtpConfig,
}

impl<'a> EmailService<'a> {
    pub fn novo(smtp: &'a SmtpConfig) -> Self {
        Self { smtp }
    }

    pub async fn enviar(
        &self,
        destinatario: &str,
        assunto: &str,
        corpo: &str,
    ) -> Result<(), AppError> {
        let mensagem = Message::builder()
            .from(
                self.smtp
                    .usuario
                    .parse()
                    .map_err(|e: lettre::address::AddressError| {
                        AppError::Interno(format!("E-mail remetente inválido: {}", e))
                    })?,
            )
            .to(destinatario
                .parse()
                .map_err(|e: lettre::address::AddressError| {
                    AppError::Interno(format!("E-mail destinatário inválido: {}", e))
                })?)
            .subject(assunto)
            .header(ContentType::TEXT_PLAIN)
            .body(corpo.to_string())
            .map_err(|e| AppError::Interno(format!("Erro ao montar e-mail: {}", e)))?;

        let creds = Credentials::new(self.smtp.usuario.clone(), self.smtp.senha.clone());

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.smtp.host)
            .map_err(|e| AppError::Interno(format!("Erro ao configurar SMTP: {}", e)))?
            .port(self.smtp.port)
            .credentials(creds)
            .build();

        transport
            .send(mensagem)
            .await
            .map_err(|e| AppError::Interno(format!("Falha ao enviar e-mail: {}", e)))?;

        Ok(())
    }
}
