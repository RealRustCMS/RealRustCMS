# Security Checklist — RustCMS

## Input validation

### next_seguro() — use everywhere
Any handler or OAuth flow that accepts a redirect destination:
- `handlers/membros.rs` — form_login, processar_login, form_cadastro, processar_cadastro
- `handlers/oauth.rs` — all `*_redirect` handlers (4 providers)
- `handlers/oauth.rs` — all `*_callback` handlers (reading from oauth_states)

### File uploads
- Magic bytes check (not just extension)
- Allowlist of permitted MIME types
- Max size from config (`UPLOAD_TAMANHO_MAXIMO_MB`)

### Comments / ratings
- Rate limiting via DashMap (`rate_limit.rs`)
- Turnstile captcha (optional, configured via env)
- Rating uniqueness: UNIQUE constraint on `(artigo_id, ip)`

## Authentication

### Password hashing
- Argon2id only
- `hash_senha` and `verificar_senha` are free functions in `services/auth.rs`
- Constant-time comparison to mitigate timing attacks
- `senha_hash` is `Option<String>` — OIDC-only users and SSO members have no local password

### Sessions
- `SameSite::Lax` (not Strict — Strict breaks OIDC callbacks)
- `Secure` flag only when `PRODUCAO=true`
- Session secret from `SESSION_SECRET` env var
- Schema `tower_sessions` separate from `public`

### MFA
- TOTP via `totp-rs v5`
- `MFA_OBRIGATORIO=true` enforces for all non-OIDC users
- OIDC users are exempt from MFA
- Two session states: `mfa_pendente_id` (login flow) and `mfa_setup_id` (setup flow)

## Authorization

### Middleware layers
```
requer_login    → checks usuario_id in session
requer_editor   → checks usuario_id + papel in {editor, admin}
requer_admin    → checks usuario_id + papel == admin
requer_membro   → checks usuario_id OR membro_id in session,
                  then calls esta_ativo() in DB (real-time deactivation)
```

### Conteúdo restrito — two-tier check
1. **Visibility** (listing, menu): `MembrosService::sessao_membro_ativa(session)`
   - No DB hit, just reads session keys
   - Used for: article listing filter, menu recursive filter, config flag
   
2. **Access block** (direct view): `requer_membro` middleware
   - DB validated via `esta_ativo()`
   - Used for: `/artigos/:slug` if `restrito=true`, `/paginas/:slug` if `restrito=true`

### Always block before side effects
```rust
// CORRECT order in ver_artigo handler:
// 1. Fetch article
// 2. Check if restrito and no session
// 3. THEN record page view / render
if artigo.restrito && !sessao_ativa {
    return redirect_to_login_with_next(slug);
}
// page view recording happens here, not before
```

## CSRF

### Protected
All POST routes except the exclusion list.

### Excluded (intentional)
- `/api/` — Bearer token auth instead
- `/login` — pre-auth, no session yet
- `/login/mfa*` — mid-auth flow
- `/admin/upload` — multipart, token in header
- `*/salvar` — drag-and-drop menu, AJAX POST
- `*/destaque` — quick toggle, AJAX POST
- `/membros/logout` — logout should always work

### Implementation
CSRF token generated per session (`csrf.rs`), injected into all admin
POST forms via JS in `base.html`. Handlers validate via `verificar_csrf`
middleware.

## HTTP headers

CSP is built dynamically in `config.rs` from base sources + env overrides:
```
CSP_EXTRA_SCRIPT_SRC=domain1.com domain2.com  # space-separated
CSP_EXTRA_STYLE_SRC=
CSP_EXTRA_IMG_SRC=
CSP_EXTRA_CONNECT_SRC=
```

Phase 2 (not yet implemented): per-request nonces to replace `unsafe-inline`.

## OAuth security

- State token as CSRF protection for OAuth flow
- State persisted in `oauth_states` DB table (not session — session dies)
- State deleted on consumption (DELETE ... RETURNING)
- `next` validated with `next_seguro()` before storing AND before using

## Member deactivation

Real-time blocking: `requer_membro` calls `MembrosRepo::esta_ativo(id)`
on every request. A deactivated member is blocked immediately without
waiting for session expiry.

## API security

- Bearer token auth (`api_token` table)
- No session, no CSRF
- Treats caller as anonymous visitor for content restrictions
- Returns 404 (not 403) for restricted articles — doesn't reveal existence
- `ocultar_restritos_api(state)` helper reads `mostrar_artigos_restritos_listagem` config
