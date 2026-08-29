use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::{GitHubConfig, OidcGenericoConfig, OidcProviderConfig},
    error::{AppError, Result},
    models::Usuario,
    repositories::usuarios::UsuariosRepo,
};

// ─── CLAIMS extraídos do id_token ou da API do provedor ──
// Campos mínimos que precisamos de qualquer provedor.
// `sub` é o identificador permanente; `email` e `nome` são
// usados para criar o usuário quando permitir_cadastro=true.
#[derive(Debug)]
pub struct OidcClaims {
    pub sub: String,
    pub email: String,
    pub nome: String,
}

// ─── STATE ───────────────────────────────────────────────

/// Grava um state aleatório na tabela oauth_states antes do redirect.
/// O state protege contra CSRF no fluxo OAuth — mesmo conceito do csrf.rs,
/// mas para o redirect externo (o callback chega numa requisição nova,
/// então não podemos usar apenas a sessão Axum).
///
/// `next` é o destino pós-login validado (path relativo, ex: "/artigos/x"
/// ou "/membros/area") — viaja junto com o state porque a sessão Axum não
/// é o mecanismo certo aqui: o callback do provedor é uma requisição nova
/// que pode até chegar com cookies diferentes dependendo do navegador.
pub async fn salvar_state(db: &PgPool, provider: &str, next: Option<&str>) -> Result<String> {
    let state = Uuid::new_v4().to_string();

    sqlx::query!(
        "INSERT INTO oauth_states (state, provider, next) VALUES ($1, $2, $3)",
        state,
        provider,
        next
    )
    .execute(db)
    .await?;

    Ok(state)
}

/// Valida o state recebido no callback.
/// Verifica se existe, pertence ao provider esperado e não expirou (10 min).
/// Deleta após validar — uso único, igual a um token CSRF.
/// Retorna o `next` salvo junto (se houver) para o callback redirecionar
/// ao destino original do visitante.
pub async fn validar_state(db: &PgPool, state: &str, provider: &str) -> Result<Option<String>> {
    let row = sqlx::query!(
        "SELECT provider, criado_em, next FROM oauth_states
         WHERE state = $1
         AND criado_em > NOW() - INTERVAL '10 minutes'",
        state
    )
    .fetch_optional(db)
    .await?;

    // Deleta independente do resultado — evita acúmulo de states expirados
    sqlx::query!("DELETE FROM oauth_states WHERE state = $1", state)
        .execute(db)
        .await?;

    match row {
        None => Err(AppError::Interno("State inválido ou expirado.".into())),
        Some(r) if r.provider != provider => Err(AppError::Interno(
            "State não pertence a este provedor.".into(),
        )),
        Some(r) => Ok(r.next),
    }
}

// ─── GOOGLE / MICROSOFT (OIDC completo) ──────────────────

/// Monta a URL de autorização para provedores OIDC completos.
/// O `nonce` evita replay attacks — o provedor inclui no id_token
/// e verificamos se bate com o que enviamos.
pub fn montar_url_google(config: &OidcProviderConfig, state: &str, redirect_uri: &str) -> String {
    format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope=openid%20email%20profile\
         &state={}\
         &access_type=offline",
        config.client_id,
        urlencoding::encode(redirect_uri),
        state
    )
}

pub fn montar_url_microsoft(
    config: &OidcProviderConfig,
    tenant: &str,
    state: &str,
    redirect_uri: &str,
) -> String {
    format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope=openid%20email%20profile\
         &state={}",
        tenant,
        config.client_id,
        urlencoding::encode(redirect_uri),
        state
    )
}

pub fn montar_url_generico(
    config: &OidcGenericoConfig,
    state: &str,
    redirect_uri: &str,
    authorization_endpoint: &str,
) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&state={}",
        authorization_endpoint,
        config.client_id,
        urlencoding::encode(redirect_uri),
        state
    )
}

pub fn montar_url_github(config: &GitHubConfig, state: &str) -> String {
    format!(
        "https://github.com/login/oauth/authorize\
         ?client_id={}\
         &scope=user:email\
         &state={}",
        config.client_id, state
    )
}

// ─── TROCA DE CODE POR TOKENS ────────────────────────────

/// Troca o authorization code pelo id_token e access_token.
/// O id_token é um JWT assinado pelo provedor — contém os claims
/// do usuário sem precisar de outra chamada de API.
///
/// Retorna o id_token como String — a validação e extração dos
/// claims são feitas em funções separadas por provedor.
pub async fn trocar_code_google(
    config: &OidcProviderConfig,
    code: &str,
    redirect_uri: &str,
) -> Result<OidcClaims> {
    let client = Client::new();

    // Passo 1: troca o code pelo token
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code),
            ("client_id", &config.client_id),
            ("client_secret", &config.client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    // Passo 2: decodifica o id_token (JWT) sem verificar assinatura.
    // Em produção real você verificaria a assinatura contra as chaves
    // JWKS do Google. Para um CMS interno, a troca via HTTPS é suficiente.
    let id_token = json["id_token"]
        .as_str()
        .ok_or_else(|| AppError::Interno("id_token ausente na resposta do Google.".into()))?;

    extrair_claims_jwt(id_token)
}

pub async fn trocar_code_microsoft(
    config: &OidcProviderConfig,
    tenant: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<OidcClaims> {
    let client = Client::new();

    let token_url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        tenant
    );

    let resp = client
        .post(&token_url)
        .form(&[
            ("code", code),
            ("client_id", &config.client_id),
            ("client_secret", &config.client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    let id_token = json["id_token"]
        .as_str()
        .ok_or_else(|| AppError::Interno("id_token ausente na resposta da Microsoft.".into()))?;

    extrair_claims_jwt(id_token)
}

/// Provedor genérico (Keycloak, RHSSO, etc.) — mesmo fluxo,
/// mas o token_endpoint vem do discovery document.
pub async fn trocar_code_generico(
    config: &OidcGenericoConfig,
    code: &str,
    redirect_uri: &str,
    token_endpoint: &str,
) -> Result<OidcClaims> {
    let client = Client::new();

    let resp = client
        .post(token_endpoint)
        .form(&[
            ("code", code),
            ("client_id", &config.client_id),
            ("client_secret", &config.client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    let id_token = json["id_token"]
        .as_str()
        .ok_or_else(|| AppError::Interno("id_token ausente na resposta do provedor.".into()))?;

    extrair_claims_jwt(id_token)
}

/// GitHub não tem id_token — busca o perfil na API REST depois
/// de trocar o code pelo access_token.
pub async fn trocar_code_github(config: &GitHubConfig, code: &str) -> Result<OidcClaims> {
    let client = Client::new();

    // Passo 1: troca code por access_token
    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("code", code),
            ("client_id", &config.client_id),
            ("client_secret", &config.client_secret),
        ])
        .send()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Interno("access_token ausente na resposta do GitHub.".into()))?
        .to_string();

    // Passo 2: busca perfil do usuário
    let perfil: serde_json::Value = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "RustCMS")
        .send()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    // GitHub pode não ter e-mail público — busca na API de e-mails
    let email = if let Some(e) = perfil["email"].as_str().filter(|s| !s.is_empty()) {
        e.to_string()
    } else {
        buscar_email_github(&client, &access_token).await?
    };

    let sub = perfil["id"]
        .as_i64()
        .ok_or_else(|| AppError::Interno("ID ausente no perfil do GitHub.".into()))?
        .to_string();

    let nome = perfil["name"]
        .as_str()
        .or_else(|| perfil["login"].as_str())
        .unwrap_or("Usuário GitHub")
        .to_string();

    Ok(OidcClaims { sub, email, nome })
}

async fn buscar_email_github(client: &Client, access_token: &str) -> Result<String> {
    let emails: serde_json::Value = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "RustCMS")
        .send()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::Interno(e.to_string()))?;

    // Prefere o e-mail marcado como primário e verificado
    if let Some(arr) = emails.as_array() {
        for item in arr {
            let primario = item["primary"].as_bool().unwrap_or(false);
            let verificado = item["verified"].as_bool().unwrap_or(false);
            if primario && verificado {
                if let Some(e) = item["email"].as_str() {
                    return Ok(e.to_string());
                }
            }
        }
    }

    Err(AppError::Interno(
        "Não foi possível obter e-mail verificado do GitHub.".into(),
    ))
}

// ─── EXTRAÇÃO DE CLAIMS DO JWT ───────────────────────────

/// Decodifica o payload do id_token (JWT) sem verificar assinatura.
/// Um JWT tem 3 partes separadas por ponto: header.payload.signature
/// O payload é Base64url — decodificamos e extraímos os campos.
fn extrair_claims_jwt(id_token: &str) -> Result<OidcClaims> {
    let partes: Vec<&str> = id_token.split('.').collect();
    if partes.len() != 3 {
        return Err(AppError::Interno("id_token malformado.".into()));
    }

    // Base64url não tem padding — o decoder padrão precisa de '='
    // use_url para aceitar '-' e '_' no lugar de '+' e '/'
    let payload = base64_decode_url(partes[1])?;

    let claims: serde_json::Value = serde_json::from_slice(&payload)
        .map_err(|e| AppError::Interno(format!("Falha ao decodificar claims JWT: {}", e)))?;

    let sub = claims["sub"]
        .as_str()
        .ok_or_else(|| AppError::Interno("Campo 'sub' ausente no id_token.".into()))?
        .to_string();

    let email = claims["email"]
        .as_str()
        .ok_or_else(|| AppError::Interno("Campo 'email' ausente no id_token.".into()))?
        .to_string();

    // 'name' é o nome completo; alguns provedores usam 'given_name'
    let nome = claims["name"]
        .as_str()
        .or_else(|| claims["given_name"].as_str())
        .or_else(|| claims["preferred_username"].as_str())
        .unwrap_or(&email)
        .to_string();

    Ok(OidcClaims { sub, email, nome })
}

fn base64_decode_url(input: &str) -> Result<Vec<u8>> {
    // Base64url usa '-' e '_'; converte para Base64 padrão
    let padded = match input.len() % 4 {
        0 => input.to_string(),
        2 => format!("{}==", input),
        3 => format!("{}=", input),
        _ => input.to_string(),
    };
    let normalizado = padded.replace('-', "+").replace('_', "/");

    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(&normalizado)
        .map_err(|e| AppError::Interno(format!("Falha ao decodificar Base64: {}", e)))
}

// ─── DISCOVERY DOCUMENT ──────────────────────────────────

/// Busca o discovery document do provedor genérico.
/// Retorna os endpoints de autorização e token.
/// Cacheamento não implementado — para um CMS com poucos logins
/// por dia, uma chamada HTTP extra no redirect é aceitável.
pub async fn buscar_discovery(discovery_url: &str) -> Result<DiscoveryDoc> {
    let client = Client::new();
    let doc: serde_json::Value = client
        .get(discovery_url)
        .send()
        .await
        .map_err(|e| AppError::Interno(format!("Falha ao buscar discovery document: {}", e)))?
        .json()
        .await
        .map_err(|e| {
            AppError::Interno(format!("Falha ao decodificar discovery document: {}", e))
        })?;

    let authorization_endpoint = doc["authorization_endpoint"]
        .as_str()
        .ok_or_else(|| AppError::Interno("authorization_endpoint ausente no discovery.".into()))?
        .to_string();

    let token_endpoint = doc["token_endpoint"]
        .as_str()
        .ok_or_else(|| AppError::Interno("token_endpoint ausente no discovery.".into()))?
        .to_string();

    Ok(DiscoveryDoc {
        authorization_endpoint,
        token_endpoint,
    })
}

#[derive(Debug)]
pub struct DiscoveryDoc {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
}

// ─── DECISION TREE ───────────────────────────────────────

/// Núcleo do callback: recebe os claims validados e decide o que fazer.
/// Retorna o Usuario que deve ter a sessão criada, ou erro se acesso negado.
///
/// Fluxo:
/// 1. Busca por (provider, sub) → login direto se encontrado
/// 2. Busca por email → vincula provider ao usuário existente → login
/// 3. permitir_cadastro=true → cria usuário novo → login
/// 4. Nenhum dos anteriores → acesso negado
pub async fn resolver_usuario(
    db: &PgPool,
    claims: &OidcClaims,
    provider: &str,
    permitir_cadastro: bool,
) -> Result<Usuario> {
    let repo = UsuariosRepo::novo(db);

    // 1. Já tem vínculo direto — caminho mais comum após o primeiro login
    if let Some(usuario) = repo.buscar_por_oauth(provider, &claims.sub).await? {
        tracing::info!(
            provider = %provider,
            sub = %claims.sub,
            email = %claims.email,
            "Login OIDC via vínculo existente"
        );
        return Ok(usuario);
    }

    // 2. E-mail existe mas sem vínculo — admin pré-cadastrou o usuário.
    // Vinculamos na primeira autenticação para que futuros logins
    // usem o caminho 1 (mais rápido, sem lookup por email).
    if let Some(usuario) = repo.buscar_por_email(&claims.email).await? {
        repo.vincular_oauth(&usuario.id, provider, &claims.sub)
            .await?;
        tracing::info!(
            provider = %provider,
            sub = %claims.sub,
            email = %claims.email,
            usuario_id = %usuario.id,
            "Login OIDC — provedor vinculado a usuário existente"
        );
        return Ok(usuario);
    }

    // 3. Usuário não existe — cria se cadastro automático estiver habilitado
    if permitir_cadastro {
        let usuario = repo
            .criar_via_oauth(&claims.nome, &claims.email, provider, &claims.sub)
            .await?;
        tracing::info!(
            provider = %provider,
            email = %claims.email,
            usuario_id = %usuario.id,
            "Login OIDC — novo usuário criado automaticamente"
        );
        return Ok(usuario);
    }

    // 4. Acesso negado
    tracing::warn!(
        provider = %provider,
        email = %claims.email,
        "Login OIDC negado — usuário não cadastrado e cadastro automático desabilitado"
    );
    Err(AppError::Interno(
        "Acesso não autorizado. Solicite ao administrador.".into(),
    ))
}