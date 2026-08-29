use totp_rs::{Algorithm, Secret, TOTP};

use crate::error::{AppError, Result};

// Constrói a struct TOTP com os parâmetros padrão do RFC 6238.
// Usada internamente por todas as funções deste módulo.
fn montar_totp(site_nome: &str, email: &str, secret_base32: &str) -> Result<TOTP> {
    TOTP::new(
        Algorithm::SHA1, // SHA1 é o padrão do RFC 6238 — compatível com todos os apps
        6,               // 6 dígitos
        1,               // tolerância de ±1 janela (30s antes e depois)
        30,              // janela de 30 segundos
        Secret::Encoded(secret_base32.to_string())
            .to_bytes()
            .map_err(|e| AppError::Interno(format!("MFA secret inválido: {e}")))?,
        Some(site_nome.to_string()), // emissor exibido no app autenticador
        email.to_string(),           // conta exibida no app autenticador
    )
    .map_err(|e| AppError::Interno(format!("Falha ao montar TOTP: {e}")))
}

/// Gera uma nova chave secreta aleatória em Base32.
/// Salve no banco SOMENTE após o usuário confirmar com um código válido.
pub fn gerar_secret() -> String {
    Secret::generate_secret().to_encoded().to_string()
}

/// Gera o QR Code como string Base64 de um PNG.
/// Use no template assim:
///   <img src="data:image/png;base64,{{ qr_base64 }}">
pub fn gerar_qr_base64(site_nome: &str, email: &str, secret_base32: &str) -> Result<String> {
    let totp = montar_totp(site_nome, email, secret_base32)?;
    totp.get_qr_base64()
        .map_err(|e| AppError::Interno(format!("Falha ao gerar QR Code: {e}")))
}

/// Valida o código de 6 dígitos digitado pelo usuário.
/// Retorna true se o código for válido para a janela atual (±30s).
pub fn validar_codigo(secret_base32: &str, codigo: &str, site_nome: &str, email: &str) -> bool {
    // Remove espaços caso o usuário digite "123 456"
    let codigo = codigo.replace(' ', "");

    match montar_totp(site_nome, email, secret_base32) {
        Ok(totp) => totp.check_current(&codigo).unwrap_or(false),
        Err(_) => false,
    }
}
