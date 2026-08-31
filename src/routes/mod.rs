pub mod admin;
pub mod api;
pub mod auth;
pub mod membros;
pub mod mfa;
pub mod oauth;
pub mod publico;
pub mod upload;

use crate::handlers::middleware::verificar_csrf;
use crate::state::AppState;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::Router;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_sessions::cookie::time::Duration;
use tower_sessions::session_store::ExpiredDeletion;
use tower_sessions::{cookie::Key, cookie::SameSite, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store_chrono::PostgresStore;

pub async fn montar(state: AppState) -> Router {
    // A tabela fica em `tower_sessions.sessoes` — o schema `tower_sessions` é
    // o default do PostgresStore (não passamos with_schema_name); só o nome da
    // tabela é sobrescrito (o default do crate seria `session`). Schema
    // separado de `public` é intencional — backup seletivo e limpeza independente.
    let session_store = PostgresStore::new(state.db.clone())
        .with_table_name("sessoes")
        .unwrap();

    session_store
        .migrate()
        .await
        .expect("Falha ao inicializar store de sessões");

    // Limpeza periódica de sessões expiradas. O PostgresStore não expira
    // sozinho — sem isto, cada linha de sessão que passa das 2h de inatividade
    // fica para sempre em `tower_sessions.sessoes` acumulando peso morto.
    // `delete_expired` roda `DELETE ... WHERE expiry_date < now()`. Loop
    // próprio (em vez do `continuously_delete_expired` do crate, que exige a
    // feature `deletion-task` e encerra de vez no primeiro erro) — o primeiro
    // tick do interval dispara imediato, então o backlog é limpo já no boot.
    {
        let store = session_store.clone();
        tokio::spawn(async move {
            let mut intervalo =
                tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                intervalo.tick().await;
                if let Err(e) = store.delete_expired().await {
                    tracing::warn!(erro = %e, "Falha ao limpar sessões expiradas");
                }
            }
        });
    }

    let secret = state.config.session_secret.as_bytes().to_vec();
    let key = Key::from(&secret);

    // ─── Configuração do cookie de sessão ─────────────────────────────
    // with_secure(true):    cookie só é enviado em HTTPS — protege contra
    //                       interceptação em redes abertas. Em desenvolvimento
    //                       local (HTTP) o browser ignora o cookie; para
    //                       desenvolvimento use with_secure(false) ou uma
    //                       variável de ambiente.
    //
    // with_same_site(Strict): o cookie não é enviado em requisições cross-site
    //                         (ex: clique em link de outro domínio). Isso
    //                         elimina uma classe de ataques CSRF mesmo sem o
    //                         token CSRF. Estamos usando os dois como defesa
    //                         em profundidade.
    //
    // with_expiry(OnInactivity): sessão expira após 2h sem atividade.
    //                            Protege contra sessões esquecidas abertas.
    let em_producao = state.config.producao;

    let session_layer = SessionManagerLayer::new(session_store)
        .with_signed(key)
        .with_secure(em_producao)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::hours(2)));

    // SameSite::Lax em vez de Strict porque Strict bloqueia o cookie
    // inclusive no redirecionamento de volta dos providers OIDC (Google,
    // GitHub, etc.) — o callback chegaria sem sessão e o state de validação
    // OAuth seria perdido. Lax permite o cookie em navegações top-level
    // (como o redirect do provider) mas bloqueia em requests cross-site
    // iniciados por JS ou iframes. É o balanço correto para este projeto.

    let csp = state.config.csp.clone();
    let limite_bytes = state.config.upload_tamanho_maximo_mb * 1024 * 1024;

    // Captura o state em uma closure para o fallback — evita conflito com with_state
    let state_404 = state.clone();
    let fallback = move || {
        let state = state_404.clone();
        async move {
            let mut ctx = tera::Context::new();
            ctx.insert("site_nome", &state.config.site_nome);
            ctx.insert("site_descricao", &state.config.site_descricao);
            ctx.insert("site_logo", &state.config.site_logo);
            ctx.insert("tema", &state.config.tema);
            // menu é necessário para o base.html público renderizar a nav
            ctx.insert("menu", &state.ler_menu_cache().await);

            match state
                .tera
                .render(&state.config.template_pub("404.html"), &ctx)
            {
                Ok(html) => (StatusCode::NOT_FOUND, Html(html)).into_response(),
                Err(_) => (StatusCode::NOT_FOUND, "404 — Página não encontrada").into_response(),
            }
        }
    };

    Router::new()
        .nest_service("/static", ServeDir::new("static"))
        .merge(publico::rotas(state.clone()))
        .merge(auth::rotas(state.clone()))
        .merge(oauth::rotas(state.clone()))
        .merge(mfa::rotas(state.clone()))
        .merge(admin::rotas(state.clone()))
        .merge(upload::rotas(state.clone()))
        .merge(api::rotas(state.clone()))
        // ─── Área de membros ──────────────────────────────────────────────
        .merge(membros::rotas_membros_publicas(state.clone()))
        .merge(membros::rotas_membros_restritas(state.clone()))
        .merge(membros::rotas_admin_membros(state))
        .fallback(fallback)
        .layer(axum::middleware::from_fn(verificar_csrf))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        // ─── Content-Security-Policy ──────────────────────────────────────
        // Fase 1: 'unsafe-inline' habilitado para scripts e estilos.
        // Isso permite o script inline de CSRF no admin/base.html e os
        // scripts de lightbox/avaliação nos templates públicos, sem quebrar
        // nenhuma funcionalidade existente.
        //
        // Fase 2 (futura): substituir 'unsafe-inline' por nonce gerado
        // por request, adicionado como atributo nos <script> inline.
        //
        // As fontes base cobre o que o CMS precisa por padrão.
        // Fontes extras podem ser adicionadas via .env sem recompilar:
        //   CSP_EXTRA_SCRIPT_SRC=https://www.googletagmanager.com
        //   CSP_EXTRA_STYLE_SRC=https://cdn.algolia.com
        //   CSP_EXTRA_IMG_SRC=https://images.unsplash.com
        //   CSP_EXTRA_CONNECT_SRC=https://api.algolia.com
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::try_from(csp)
                .expect("CSP header inválido — verifique as variáveis CSP_EXTRA_* no .env"),
        ))
        // Desabilita APIs do browser que o CMS não usa.
        // camera, microphone, geolocation: nunca usados pelo CMS.
        // payment: sem e-commerce.
        // usb, bluetooth: periféricos desnecessários num CMS web.
        .layer(SetResponseHeaderLayer::if_not_present(
            header::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static(
                "camera=(), microphone=(), geolocation=(), payment=(), usb=(), bluetooth=()",
            ),
        ))
        .layer(RequestBodyLimitLayer::new(limite_bytes))
        .layer(session_layer)
}
