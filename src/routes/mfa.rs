use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{
    handlers::{
        mfa::{
            admin_exigir_mfa, admin_remover_exigencia_mfa, admin_resetar_mfa, ativar_mfa_perfil,
            desativar_mfa_perfil, pagina_mfa_login, pagina_mfa_setup, pagina_perfil_mfa,
            processar_mfa_login, processar_mfa_setup,
        },
        middleware::{requer_admin, requer_login, requer_mfa_pendente, requer_mfa_setup},
    },
    state::AppState,
};

pub fn rotas(state: AppState) -> Router {
    // ─── Segunda etapa do login (código TOTP) ─────────────
    // Protegidas por requer_mfa_pendente: só acessíveis quando
    // mfa_pendente_id existe na sessão (usuário passou na senha
    // mas ainda não passou no MFA).
    let rotas_mfa_login = Router::new()
        .route(
            "/login/mfa",
            get(pagina_mfa_login).post(processar_mfa_login),
        )
        .route_layer(middleware::from_fn(requer_mfa_pendente));

    // ─── Setup obrigatório de MFA (redirecionado no login) ─
    // Protegidas por requer_mfa_setup: só acessíveis quando
    // mfa_setup_id existe na sessão (MFA exigido mas não configurado).
    let rotas_mfa_setup = Router::new()
        .route(
            "/login/mfa/configurar",
            get(pagina_mfa_setup).post(processar_mfa_setup),
        )
        .route_layer(middleware::from_fn(requer_mfa_setup));

    // ─── Perfil — ativação voluntária ─────────────────────
    // Protegidas por requer_login: usuário já está autenticado.
    let rotas_mfa_perfil = Router::new()
        .route("/admin/perfil/mfa", get(pagina_perfil_mfa))
        .route("/admin/perfil/mfa/ativar", post(ativar_mfa_perfil))
        .route("/admin/perfil/mfa/desativar", post(desativar_mfa_perfil))
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_login));

    // ─── Admin — gerenciar MFA de outros usuários ─────────
    // Protegidas por requer_admin — mfa::rotas é mesclada direto no router
    // raiz em routes/mod.rs, não aninhada dentro de admin::rotas, então
    // precisa da própria camada de autorização.
    // Os handlers admin_* são chamados a partir da tela de edição de usuário.
    let rotas_mfa_admin = Router::new()
        .route("/admin/usuarios/{id}/mfa/resetar", post(admin_resetar_mfa))
        .route("/admin/usuarios/{id}/mfa/exigir", post(admin_exigir_mfa))
        .route(
            "/admin/usuarios/{id}/mfa/remover-exigencia",
            post(admin_remover_exigencia_mfa),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_admin));

    Router::new()
        .merge(rotas_mfa_login)
        .merge(rotas_mfa_setup)
        .merge(rotas_mfa_perfil)
        .merge(rotas_mfa_admin)
        .with_state(state)
}
