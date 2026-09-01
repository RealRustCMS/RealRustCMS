// ─── SMTP — configuração de envio de e-mail ──────────────
// None em Config.smtp significa SMTP desabilitado — nenhum e-mail é enviado.
// Todos os 4 campos são obrigatórios para habilitar.
#[derive(Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub usuario: String,
    pub senha: String,
}

// ─── OIDC — configuração de um provedor ──────────────────
// Cada provedor (Google, Microsoft, provedor genérico) é representado
// por essa struct. Se client_id estiver vazio, o provedor está desabilitado
// e o botão não aparece no login — mesma lógica do Turnstile.
#[derive(Clone)]
pub struct OidcProviderConfig {
    pub client_id: String,
    pub client_secret: String,
}

// GitHub usa OAuth2 puro (sem id_token), mas a configuração
// é idêntica aos demais — só o fluxo no handler muda.
#[derive(Clone)]
pub struct GitHubConfig {
    pub client_id: String,
    pub client_secret: String,
}

// Provedor genérico (Keycloak, RHSSO, Authentik, etc).
// Além das credenciais, precisa da URL de discovery — o endpoint
// .well-known/openid-configuration que o provedor expõe.
#[derive(Clone)]
pub struct OidcGenericoConfig {
    pub client_id: String,
    pub client_secret: String,
    pub discovery_url: String,
    pub botao_label: String, // texto do botão: "Entrar com Empresa X"
}

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    // Teto de conexões do pool SQLx. Env DB_MAX_CONEXOES, padrão 20.
    // Não passar de ~25-30 por instância sem PgBouncer — mais conexão que
    // core do Postgres não acelera, só consome RAM no servidor.
    pub db_max_conexoes: u32,
    pub site_nome: String,
    pub site_descricao: String,
    pub porta: u16,
    pub base_url: String,
    pub session_secret: String,
    pub artigos_por_pagina: i64,
    pub artigos_relacionados: usize,
    pub artigos_destaque: usize,
    pub site_logo: Option<String>,
    pub views_no_dashboard: i64,
    pub turnstile_site_key: Option<String>,
    pub turnstile_secret_key: Option<String>,
    pub upload_tamanho_maximo_mb: usize,
    pub tema: String,
    pub template_publico: Option<String>,

    // ─── Versão dos assets estáticos ─────────────────────
    // Usada como query string `?v=<asset_ver>` nos <link> de CSS servidos de
    // /static, para cache-busting: com a URL mudando a cada release, o
    // Cache-Control `immutable` de longa duração em /static (ver routes/mod.rs)
    // não deixa o navegador preso num CSS velho.
    // Cascata: ASSET_VER (env, runtime) → option_env!("ASSET_VER") (gravado em
    // tempo de compilação pelo CI, github.sha) → CARGO_PKG_VERSION.
    pub asset_ver: String,

    // ─── Ambiente ────────────────────────────────────────
    // Se true: cookie de sessão só trafega em HTTPS (Secure flag).
    // Deve ser false em desenvolvimento local (HTTP) e true em produção.
    pub producao: bool,

    // ─── Content-Security-Policy ─────────────────────────
    // String CSP completa, montada no carregar() a partir das fontes
    // base (hardcoded) + fontes extras opcionais do .env.
    // Separado por diretiva para facilitar extensão futura.
    pub csp: String,

    // ─── MFA ─────────────────────────────────────────────
    // Se true: todos os usuários são obrigados a configurar MFA.
    // Se false (padrão): MFA é opcional — cada usuário decide no perfil,
    // ou o admin pode exigir individualmente via mfa_obrigatorio no banco.
    pub mfa_obrigatorio: bool,

    // ─── OIDC ────────────────────────────────────────────
    // Option<T>: None significa desabilitado (variáveis vazias ou ausentes)
    pub oidc_google: Option<OidcProviderConfig>,
    pub oidc_microsoft: Option<OidcProviderConfig>,
    pub oidc_microsoft_tenant: String, // "common" por padrão
    pub oidc_github: Option<GitHubConfig>,
    pub oidc_generico: Option<OidcGenericoConfig>,

    // Se false (padrão): só entra quem o admin já cadastrou.
    // Se true: cria usuário novo automaticamente ao logar via OIDC
    // (papel padrão: visualizador).
    pub oidc_permitir_cadastro: bool,
    pub oidc_generico_permitir_cadastro: bool,

    // ─── E-mail / Notificações ───────────────────────────
    // None: SMTP não configurado, nenhuma notificação é enviada.
    pub smtp: Option<SmtpConfig>,
    // Valor padrão para notificar_comentarios ao criar novo artigo.
    pub notif_padrao: bool,

    // ─── Área de membros ─────────────────────────────────
    // Se true: qualquer pessoa pode se cadastrar como membro publicamente
    // ou ser criada automaticamente ao logar via SSO.
    // Se false (padrão): só membros cadastrados manualmente pelo admin entram.
    pub membros_permitir_cadastro: bool,

    // Se definido: só e-mails desse domínio são criados automaticamente
    // como membros via SSO. Tem precedência sobre membros_permitir_cadastro.
    // Exemplo: "empresa.com.br" → só @empresa.com.br vira membro no primeiro SSO.
    // None: sem restrição de domínio (depende só de membros_permitir_cadastro).
    pub membros_dominio_permitido: Option<String>,

    // ─── Helpers para templates de login ─────────────────
    // Expõe se cada provider SSO está configurado, sem expor credenciais.
    // Usados em ctx_membros() para exibir/ocultar botões SSO.
    pub google_client_id: Option<String>,
    pub microsoft_client_id: Option<String>,
    pub github_client_id: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_botao_label: Option<String>,
}

impl Config {
    pub fn template_pub(&self, arquivo: &str) -> String {
        match &self.template_publico {
            Some(nome) => format!("publico/{}/{}", nome, arquivo),
            None => format!("publico/{}", arquivo),
        }
    }

    pub fn carregar() -> Self {
        let env_path = std::env::var("CARGO_MANIFEST_DIR")
            .map(|dir| std::path::PathBuf::from(dir).join(".env"))
            .unwrap_or_else(|_| std::path::PathBuf::from(".env"));

        // Em plataformas como Railway o .env não existe — ok() evita panic
        dotenvy::from_path(&env_path).ok();

        // Temas válidos — fallback para "verde" se o valor for inválido
        let tema_raw = std::env::var("TEMA").unwrap_or_else(|_| "verde".into());
        let tema = if ["verde", "cinza", "azul", "dourado", "roxo", "escuro"]
            .contains(&tema_raw.as_str())
        {
            tema_raw
        } else {
            eprintln!(
                "AVISO: TEMA='{}' inválido. Usando 'verde'. Opções: verde, cinza, azul, dourado, roxo, escuro.",
                tema_raw
            );
            "verde".into()
        };

        // PORT é o nome padrão usado por Railway, Render e Fly.io.
        // PORTA é o nome usado localmente. Aceita os dois.
        let porta: u16 = std::env::var("PORT")
            .or_else(|_| std::env::var("PORTA"))
            .unwrap_or("3000".into())
            .parse()
            .expect("Porta inválida");

        // Teto do pool SQLx. Valor inválido ou ausente cai no padrão 20.
        let db_max_conexoes: u32 = std::env::var("DB_MAX_CONEXOES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);

        // Versão dos assets para o cache-busting de /static (ver campo asset_ver).
        // env_opt está definido logo abaixo — aqui usamos std::env::var direto
        // porque ainda não foi declarado.
        let asset_ver = std::env::var("ASSET_VER")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| option_env!("ASSET_VER").filter(|s| !s.is_empty()).map(String::from))
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

        // ─── Helper interno ───────────────────────────────
        // Lê uma variável de ambiente e retorna None se vazia ou ausente.
        // Mesmo padrão usado para TURNSTILE_SITE_KEY.
        let env_opt =
            |nome: &str| -> Option<String> { std::env::var(nome).ok().filter(|s| !s.is_empty()) };

        // ─── Ambiente ─────────────────────────────────────
        // Padrão false: seguro para desenvolvimento local sem HTTPS.
        // Em produção: definir PRODUCAO=true no painel da plataforma.
        let producao = std::env::var("PRODUCAO")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        // ─── Content-Security-Policy ──────────────────────
        // Fontes base cobre tudo que o CMS usa por padrão:
        //   Google Fonts, Cloudflare Turnstile, scripts inline (CSRF,
        //   lightbox, avaliações), QR code MFA (data:), preview upload (blob:)
        //
        // Fontes extras via .env — sem recompilar:
        //   CSP_EXTRA_SCRIPT_SRC=https://www.googletagmanager.com
        //   CSP_EXTRA_STYLE_SRC=https://cdn.algolia.com
        //   CSP_EXTRA_IMG_SRC=https://images.unsplash.com
        //   CSP_EXTRA_CONNECT_SRC=https://api.algolia.com
        //
        // Múltiplas origens na mesma diretiva: separar por espaço.
        //   CSP_EXTRA_SCRIPT_SRC=https://gtm.com https://analytics.com
        let extra_script = env_opt("CSP_EXTRA_SCRIPT_SRC").unwrap_or_default();
        let extra_style = env_opt("CSP_EXTRA_STYLE_SRC").unwrap_or_default();
        let extra_img = env_opt("CSP_EXTRA_IMG_SRC").unwrap_or_default();
        let extra_connect = env_opt("CSP_EXTRA_CONNECT_SRC").unwrap_or_default();

        let script_src = format!(
            "'self' 'unsafe-inline' https://challenges.cloudflare.com{}",
            if extra_script.is_empty() {
                String::new()
            } else {
                format!(" {}", extra_script)
            }
        );
        let style_src = format!(
            "'self' 'unsafe-inline' https://fonts.googleapis.com{}",
            if extra_style.is_empty() {
                String::new()
            } else {
                format!(" {}", extra_style)
            }
        );
        let img_src = format!(
            "'self' data: blob:{}",
            if extra_img.is_empty() {
                String::new()
            } else {
                format!(" {}", extra_img)
            }
        );
        let connect_src = format!(
            "'self'{}",
            if extra_connect.is_empty() {
                String::new()
            } else {
                format!(" {}", extra_connect)
            }
        );

        let csp = format!(
            "default-src 'self'; script-src {}; style-src {}; font-src 'self' https://fonts.gstatic.com; img-src {}; connect-src {}; frame-src https://challenges.cloudflare.com; frame-ancestors 'none';",
            script_src, style_src, img_src, connect_src
        );

        // ─── MFA ──────────────────────────────────────────
        // Padrão false: MFA opcional.
        // true: ninguém entra no painel sem configurar MFA.
        let mfa_obrigatorio = std::env::var("MFA_OBRIGATORIO")
            .unwrap_or("false".into())
            .to_lowercase()
            == "true";

        // ─── Google ───────────────────────────────────────
        // Só habilita se AMBOS client_id e client_secret estiverem presentes.
        let google_client_id = env_opt("GOOGLE_CLIENT_ID");
        let oidc_google = match (google_client_id.clone(), env_opt("GOOGLE_CLIENT_SECRET")) {
            (Some(client_id), Some(client_secret)) => Some(OidcProviderConfig {
                client_id,
                client_secret,
            }),
            _ => None,
        };

        // ─── Microsoft ────────────────────────────────────
        let microsoft_client_id = env_opt("MICROSOFT_CLIENT_ID");
        let oidc_microsoft = match (
            microsoft_client_id.clone(),
            env_opt("MICROSOFT_CLIENT_SECRET"),
        ) {
            (Some(client_id), Some(client_secret)) => Some(OidcProviderConfig {
                client_id,
                client_secret,
            }),
            _ => None,
        };

        // Tenant define o escopo de login da Microsoft:
        // "common"       → qualquer conta Microsoft (pessoal + corporativa)
        // "organizations"→ só contas corporativas/escola
        // "<uuid>"       → só o tenant específico da organização
        let oidc_microsoft_tenant =
            env_opt("MICROSOFT_TENANT_ID").unwrap_or_else(|| "common".into());

        // ─── GitHub ───────────────────────────────────────
        let github_client_id = env_opt("GITHUB_CLIENT_ID");
        let oidc_github = match (github_client_id.clone(), env_opt("GITHUB_CLIENT_SECRET")) {
            (Some(client_id), Some(client_secret)) => Some(GitHubConfig {
                client_id,
                client_secret,
            }),
            _ => None,
        };

        // ─── Provedor genérico ────────────────────────────
        // Requer os 4 campos: client_id, client_secret, discovery_url e label.
        // Se qualquer um faltar, o provedor inteiro é desabilitado.
        let oidc_client_id = env_opt("OIDC_CLIENT_ID");
        let oidc_botao_label = env_opt("OIDC_BOTAO_LABEL");
        let oidc_generico = match (
            oidc_client_id.clone(),
            env_opt("OIDC_CLIENT_SECRET"),
            env_opt("OIDC_DISCOVERY_URL"),
            oidc_botao_label.clone(),
        ) {
            (Some(client_id), Some(client_secret), Some(discovery_url), Some(botao_label)) => {
                Some(OidcGenericoConfig {
                    client_id,
                    client_secret,
                    discovery_url,
                    botao_label,
                })
            }
            _ => None,
        };

        // ─── Controle de cadastro automático ─────────────
        // Separado por tipo de provedor para permitir políticas diferentes.
        // Exemplo: Google/Microsoft/GitHub abertos, mas Keycloak só para
        // quem o admin cadastrou previamente.
        // Padrão false em ambos: ninguém entra sem estar pré-cadastrado.
        let oidc_permitir_cadastro = std::env::var("OIDC_PERMITIR_CADASTRO")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        let oidc_generico_permitir_cadastro = std::env::var("OIDC_GENERICO_PERMITIR_CADASTRO")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        // ─── URL base ─────────────────────────────────────
        // Usada para montar redirect_uri nos fluxos OIDC.
        // Em desenvolvimento: http://localhost:3000
        // Em produção: https://meusite.com (sem barra no final)
        let base_url =
            std::env::var("BASE_URL").unwrap_or_else(|_| format!("http://localhost:{}", porta));

        // ─── SMTP ─────────────────────────────────────────
        // Só habilita se TODOS os 4 campos estiverem presentes.
        // Porta padrão 587 (STARTTLS) se SMTP_PORT não for informada.
        let smtp = match (
            env_opt("SMTP_HOST"),
            env_opt("SMTP_USUARIO"),
            env_opt("SMTP_SENHA"),
        ) {
            (Some(host), Some(usuario), Some(senha)) => {
                let port: u16 = std::env::var("SMTP_PORT")
                    .unwrap_or("587".into())
                    .parse()
                    .unwrap_or(587);
                Some(SmtpConfig {
                    host,
                    port,
                    usuario,
                    senha,
                })
            }
            _ => None,
        };

        // ─── Notificações ─────────────────────────────────
        // Valor padrão para o checkbox "notificar por e-mail" ao criar artigo.
        // Padrão false: opt-in explícito por artigo.
        let notif_padrao = std::env::var("NOTIF_PADRAO")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        // ─── Área de membros ──────────────────────────────
        // membros_permitir_cadastro: false por padrão (opt-in explícito).
        // membros_dominio_permitido: None por padrão (sem restrição de domínio).
        // Se domínio definido, tem precedência sobre membros_permitir_cadastro.
        let membros_permitir_cadastro = std::env::var("MEMBROS_PERMITIR_CADASTRO")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        let membros_dominio_permitido = env_opt("MEMBROS_DOMINIO_PERMITIDO");

        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL não definida"),
            db_max_conexoes,
            site_nome: std::env::var("SITE_NOME").expect("SITE_NOME não definido"),
            site_descricao: std::env::var("SITE_DESCRICAO").expect("SITE_DESCRICAO não definida"),
            site_logo: env_opt("SITE_LOGO"),
            porta,
            base_url,
            session_secret: std::env::var("SESSION_SECRET").expect("SESSION_SECRET não definida"),
            artigos_por_pagina: std::env::var("ARTIGOS_POR_PAGINA")
                .unwrap_or("10".into())
                .parse()
                .expect("ARTIGOS_POR_PAGINA inválido"),
            artigos_destaque: std::env::var("ARTIGOS_DESTAQUE")
                .unwrap_or("4".into())
                .parse()
                .expect("ARTIGOS_DESTAQUE inválido"),
            artigos_relacionados: std::env::var("ARTIGOS_RELACIONADOS")
                .unwrap_or("3".into())
                .parse()
                .expect("ARTIGOS_RELACIONADOS inválido"),
            views_no_dashboard: std::env::var("VIEWS_NO_DASHBOARD")
                .unwrap_or("5".into())
                .parse()
                .expect("VIEWS_NO_DASHBOARD inválido"),
            turnstile_site_key: env_opt("TURNSTILE_SITE_KEY"),
            turnstile_secret_key: env_opt("TURNSTILE_SECRET_KEY"),
            upload_tamanho_maximo_mb: std::env::var("UPLOAD_TAMANHO_MAXIMO_MB")
                .unwrap_or("10".into())
                .parse()
                .expect("UPLOAD_TAMANHO_MAXIMO_MB inválido"),
            tema,
            template_publico: env_opt("TEMPLATE_PUBLICO"),
            asset_ver,
            producao,
            csp,
            mfa_obrigatorio,
            oidc_google,
            oidc_microsoft,
            oidc_microsoft_tenant,
            oidc_github,
            oidc_generico,
            oidc_permitir_cadastro,
            oidc_generico_permitir_cadastro,
            smtp,
            notif_padrao,
            membros_permitir_cadastro,
            membros_dominio_permitido,
            google_client_id,
            microsoft_client_id,
            github_client_id,
            oidc_client_id,
            oidc_botao_label,
        }
    }
}
