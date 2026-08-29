// routes/membros.rs

use axum::{
    extract::State,
    http::StatusCode,
    middleware,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};

use crate::{
    handlers::membros::{
        admin_ativar, admin_deletar, admin_desativar, admin_form_editar, admin_listar,
        admin_salvar_editar, area, form_cadastro, form_login, form_perfil, logout,
        processar_cadastro, processar_login, salvar_perfil,
    },
    handlers::middleware::{requer_admin, requer_membro},
    state::AppState,
};

/// Fallback 404 para rotas /membros/* restritas não encontradas.
async fn membros_404(State(state): State<AppState>) -> impl IntoResponse {
    let mut ctx = tera::Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("site_descricao", &state.config.site_descricao);
    ctx.insert("tema", &state.config.tema);

    match state.tera.render("membros/404.html", &ctx) {
        Ok(html) => (StatusCode::NOT_FOUND, Html(html)).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "404 — Página não encontrada").into_response(),
    }
}

/// Rotas públicas da área de membros — login, cadastro, logout.
/// Não exigem sessão.
pub fn rotas_membros_publicas(state: AppState) -> Router {
    Router::new()
        .route("/membros/login", get(form_login).post(processar_login))
        .route(
            "/membros/cadastro",
            get(form_cadastro).post(processar_cadastro),
        )
        .route("/membros/logout", post(logout))
        .with_state(state)
}

/// Rotas protegidas da área de membros.
/// Todas passam pelo middleware `requer_membro`.
/// Tem fallback próprio para não cair no 404 público (que precisa do menu).
pub fn rotas_membros_restritas(state: AppState) -> Router {
    Router::new()
        .route("/membros/area", get(area))
        .route("/membros/perfil", get(form_perfil).post(salvar_perfil))
        // adicionar aqui novas rotas restritas conforme o projeto crescer:
        // .route("/membros/conteudo/{slug}", get(conteudo_restrito))
        .fallback(membros_404)
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_membro))
        .with_state(state)
}

/// Rotas de gestão de membros no painel admin.
/// Protegidas por requer_admin (aplicado aqui — esta função não é aninhada
/// dentro de admin::rotas, é mesclada direto no router raiz em routes/mod.rs).
pub fn rotas_admin_membros(state: AppState) -> Router {
    Router::new()
        .route("/admin/membros", get(admin_listar))
        .route("/admin/membros/{id}/ativar", post(admin_ativar))
        .route("/admin/membros/{id}/desativar", post(admin_desativar))
        .route("/admin/membros/{id}/deletar", post(admin_deletar))
        .route("/admin/membros/{id}/editar", get(admin_form_editar).post(admin_salvar_editar))
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_admin))
        .with_state(state)
}
