use axum::body::Body;
use axum::http::Method;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::{
    repositories::{membros::MembrosRepo, usuarios::UsuariosRepo},
    state::AppState,
};

const SESSION_USER_ID: &str = "usuario_id";
const SESSION_MEMBRO_ID: &str = "membro_id";

// Chaves de sessão MFA — escritas em handlers/auth.rs durante o login,
// lidas aqui para bloquear acesso ao /admin enquanto o fluxo não terminou.
const SESSION_MFA_PENDENTE: &str = "mfa_pendente_id";
const SESSION_MFA_SETUP: &str = "mfa_setup_id";

pub async fn requer_login(
    State(_state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Response {
    // Só deixa passar quem tem SESSION_USER_ID — ou seja, quem completou
    // todo o fluxo de login (incluindo MFA, se aplicável).
    // Sessões com mfa_pendente_id ou mfa_setup_id (segunda etapa não concluída)
    // são tratadas como não autenticadas e redirecionadas para o login.
    match session.get::<String>(SESSION_USER_ID).await {
        Ok(Some(_)) => next.run(request).await,
        _ => Redirect::to("/login").into_response(),
    }
}

pub async fn requer_editor(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Response {
    let id = match session.get::<String>(SESSION_USER_ID).await {
        Ok(Some(id)) => id,
        _ => return Redirect::to("/login").into_response(),
    };

    let usuario = UsuariosRepo::novo(&state.db)
        .buscar_por_id(&id)
        .await
        .unwrap_or(None);

    match usuario {
        Some(u) if u.papel == "admin" || u.papel == "editor" => next.run(request).await,
        _ => Redirect::to("/admin").into_response(),
    }
}

pub async fn requer_admin(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Response {
    let id = match session.get::<String>(SESSION_USER_ID).await {
        Ok(Some(id)) => id,
        _ => return Redirect::to("/login").into_response(),
    };

    let usuario = UsuariosRepo::novo(&state.db)
        .buscar_por_id(&id)
        .await
        .unwrap_or(None);

    match usuario {
        Some(u) if u.papel == "admin" => next.run(request).await,
        _ => Redirect::to("/admin").into_response(),
    }
}

// Invalida o cache de páginas públicas depois de qualquer mutação no /admin.
// Em vez de espalhar `pagina_cache.invalidate_all()` pelos ~20 handlers que
// alteram conteúdo (artigo, página, evento, categoria, tag, menu, galeria,
// configurações...), um único ponto: toda request não-GET ao /admin que não
// termina em erro derruba o cache inteiro. Mutações de usuário/perfil/comentário
// também invalidam — inofensivo, só força um miss na próxima visita pública.
// `invalidate_all()` é O(1) no moka (marca um timestamp; entradas anteriores
// passam a ser ignoradas na leitura).
pub async fn invalidar_cache_publico(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let e_mutacao = !matches!(
        request.method(),
        &Method::GET | &Method::HEAD | &Method::OPTIONS
    );
    let res = next.run(request).await;
    if e_mutacao && !res.status().is_server_error() && !res.status().is_client_error() {
        state.pagina_cache.invalidate_all();
    }
    res
}

pub async fn verificar_csrf(session: Session, request: Request<Body>, next: Next) -> Response {
    if matches!(
        request.method(),
        &Method::POST | &Method::PUT | &Method::DELETE | &Method::PATCH
    ) {
        let path = request.uri().path().to_string();

        // Rotas excluídas do CSRF:
        // - /api/          → autenticadas por Bearer token
        // - /login         → não há sessão ainda
        // - /login/mfa*    → CSRF validado manualmente dentro do handler
        //                    (FormCodigo tem campo _csrf próprio)
        // - /admin/upload  → protegido por sessão + magic bytes
        // - */salvar       → fetch JSON (menu drag-and-drop), protegido por sessão + requer_admin
        // - */destaque     → fetch JSON (toggle estrela), protegido por sessão + requer_editor
        // - /membros/logout → sem body de form, protegido por sessão
        if path.starts_with("/api/")
            || path == "/login"
            || path == "/membros/login"
            || path == "/membros/cadastro"
            || path.starts_with("/login/mfa")
            || path.starts_with("/admin/upload")
            || path.ends_with("/salvar")
            || path.ends_with("/destaque")
            || path == "/membros/logout"
        {
            return next.run(request).await;
        }

        let (parts, body) = request.into_parts();
        let bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
            Ok(b) => b,
            Err(_) => return axum::http::StatusCode::BAD_REQUEST.into_response(),
        };

        let token_recebido = form_urlencoded::parse(&bytes)
            .find(|(k, _)| k == "_csrf")
            .map(|(_, v)| v.to_string());

        let token_valido = match token_recebido {
            Some(t) => crate::csrf::validar_token(&session, &t).await,
            None => false,
        };

        if !token_valido {
            tracing::warn!(path = %path, "Token CSRF inválido ou ausente");
            return (
                axum::http::StatusCode::FORBIDDEN,
                "Token CSRF inválido ou ausente.",
            )
                .into_response();
        }

        let request = Request::from_parts(parts, Body::from(bytes));
        next.run(request).await
    } else {
        next.run(request).await
    }
}

// ─── Middleware de proteção das rotas MFA de login ────────────────────────────
// Garante que /login/mfa e /login/mfa/configurar só sejam acessadas
// quando há uma pendência MFA legítima na sessão.
// Sem isso, qualquer pessoa poderia acessar essas URLs diretamente.
pub async fn requer_mfa_pendente(session: Session, request: Request, next: Next) -> Response {
    let tem_pendente = session
        .get::<String>(SESSION_MFA_PENDENTE)
        .await
        .unwrap_or(None)
        .is_some();

    if tem_pendente {
        next.run(request).await
    } else {
        Redirect::to("/login").into_response()
    }
}

pub async fn requer_mfa_setup(session: Session, request: Request, next: Next) -> Response {
    let tem_setup = session
        .get::<String>(SESSION_MFA_SETUP)
        .await
        .unwrap_or(None)
        .is_some();

    if tem_setup {
        next.run(request).await
    } else {
        Redirect::to("/login").into_response()
    }
}

// ─── Middleware de proteção da área de membros ────────────────────────────────
// Passa se:
//   1. `usuario_id` está na sessão (usuário CMS tem acesso automático)
//   2. `membro_id` está na sessão e o membro está ativo no banco
//
// Se o membro foi desativado após o login, a sessão é limpa e ele é
// redirecionado para /membros/login imediatamente.
pub async fn requer_membro(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Response {
    // caminho 1 — usuário CMS logado → acesso direto, sem consulta ao banco
    if let Ok(Some(_)) = session.get::<String>(SESSION_USER_ID).await {
        return next.run(request).await;
    }

    // caminho 2 — membro logado → verifica se ainda está ativo
    if let Ok(Some(id)) = session.get::<i64>(SESSION_MEMBRO_ID).await {
        let repo = MembrosRepo::novo(&state.db);
        match repo.esta_ativo(id).await {
            Ok(true) => return next.run(request).await,
            Ok(false) => {
                // foi desativado depois do login → limpa sessão
                let _ = session.remove::<i64>(SESSION_MEMBRO_ID).await;
            }
            Err(_) => {}
        }
    }

    Redirect::to("/membros/login").into_response()
}
