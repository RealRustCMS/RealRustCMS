use axum::{
    extract::{Multipart, State},
    response::Json,
};
use chrono::{Datelike, Utc};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{repositories::galeria::GaleriaRepo, state::AppState};

const SESSION_USER_ID: &str = "usuario_id";

/// Verifica os magic bytes do arquivo para confirmar o tipo real
fn verificar_magic_bytes(bytes: &[u8], extensao: &str) -> bool {
    if bytes.len() < 4 {
        return false;
    }

    match extensao {
        "jpg" | "jpeg" => {
            // FF D8 FF
            bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
        }
        "png" => {
            // 89 50 4E 47 0D 0A 1A 0A
            bytes[0] == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4E && bytes[3] == 0x47
        }
        "gif" => {
            // GIF87a ou GIF89a
            bytes[0] == 0x47 && bytes[1] == 0x49 && bytes[2] == 0x46 && bytes[3] == 0x38
        }
        "webp" => {
            // RIFF....WEBP
            if bytes.len() < 12 {
                return false;
            }
            bytes[0] == 0x52  // R
                && bytes[1] == 0x49  // I
                && bytes[2] == 0x46  // F
                && bytes[3] == 0x46  // F
                && bytes[8] == 0x57  // W
                && bytes[9] == 0x45  // E
                && bytes[10] == 0x42 // B
                && bytes[11] == 0x50 // P
        }
        _ => false,
    }
}

pub async fn upload_imagem(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Json<Value> {
    let usuario_id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    let mut url_final = None;
    let mut album_id_opt = None;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let nome_campo = field.name().unwrap_or("").to_string();

        if nome_campo == "album_id" {
            if let Ok(val) = field.text().await {
                album_id_opt = Some(val);
            }
            continue;
        }

        if nome_campo != "imagem" {
            continue;
        }

        let nome_original = field.file_name().unwrap_or("arquivo.jpg").to_string();
        let extensao = nome_original
            .rsplit('.')
            .next()
            .unwrap_or("jpg")
            .to_lowercase();

        if !["jpg", "jpeg", "png", "gif", "webp"].contains(&extensao.as_str()) {
            tracing::warn!(usuario_id = %usuario_id, extensao = %extensao, "Upload recusado: extensão não permitida");
            return Json(json!({ "erro": "Formato não permitido." }));
        }

        let dados = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                tracing::error!(usuario_id = %usuario_id, erro = %e, "Falha ao ler bytes do upload");
                return Json(json!({ "erro": "Falha ao ler arquivo." }));
            }
        };

        if !verificar_magic_bytes(&dados, &extensao) {
            tracing::warn!(usuario_id = %usuario_id, extensao = %extensao, "Upload recusado: magic bytes inválidos");
            return Json(
                json!({ "erro": "O conteúdo do arquivo não corresponde à extensão informada." }),
            );
        }

        let agora = Utc::now();
        let pasta = format!("static/uploads/{}/{:02}", agora.year(), agora.month());
        let nome_arquivo = format!("{}.{}", Uuid::new_v4(), extensao);
        let caminho = PathBuf::from(&pasta).join(&nome_arquivo);

        if fs::create_dir_all(&pasta).await.is_err() {
            tracing::error!(pasta = %pasta, "Falha ao criar diretório de upload");
            return Json(json!({ "erro": "Falha ao criar diretório." }));
        }

        if fs::write(&caminho, &dados).await.is_err() {
            tracing::error!(caminho = ?caminho, "Falha ao salvar arquivo de upload");
            return Json(json!({ "erro": "Falha ao salvar arquivo." }));
        }

        let url = format!(
            "/static/uploads/{}/{:02}/{}",
            agora.year(),
            agora.month(),
            nome_arquivo
        );

        tracing::info!(usuario_id = %usuario_id, url = %url, "Upload realizado com sucesso");
        url_final = Some(url);
    }

    let url = match url_final {
        Some(u) => u,
        None => {
            tracing::warn!(usuario_id = %usuario_id, "Upload chamado sem nenhuma imagem");
            return Json(json!({ "erro": "Nenhuma imagem recebida." }));
        }
    };

    if let Some(album_id) = album_id_opt {
        if !album_id.is_empty() {
            GaleriaRepo::novo(&state.db)
                .adicionar_foto(&url, None, &album_id, &usuario_id)
                .await
                .ok();
        }
    }

    Json(json!({ "url": url }))
}
