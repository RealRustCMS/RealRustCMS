# OAuth/OIDC Flow — RustCMS

## Providers

Four providers: `google`, `microsoft`, `github`, `oidc` (generic).

## Flow overview

```
User clicks "Login com Google"
  → GET /auth/google/redirect?next=/artigos/meu-artigo
      - validate next with next_seguro()
      - generate state token
      - persist in oauth_states: (state, provider, next)
      - redirect to Google authorization URL

Google redirects back
  → GET /auth/google/callback?code=...&state=...
      - validate state from oauth_states (retrieves next)
      - exchange code for tokens
      - extract user info (email, name, sub)
      - tokio::join! resolves CMS user AND member in parallel
      - set session: usuario_id and/or membro_id
      - redirect to next (if valid) OR default destination
```

## State persistence

The Axum session does NOT survive the external OAuth round-trip.
The `state` CSRF token and `next` redirect destination are persisted in DB:

```sql
-- oauth_states table
CREATE TABLE oauth_states (
    state TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    next TEXT,           -- added in *_oauth_states_next.sql migration
    criado_em TIMESTAMPTZ DEFAULT NOW()
);
```

```rust
// services/oidc.rs
pub async fn salvar_state(
    db: &PgPool,
    provider: &str,
    next: Option<&str>,
) -> Result<String> {
    let state = gerar_uuid();
    sqlx::query!(
        "INSERT INTO oauth_states (state, provider, next) VALUES ($1, $2, $3)",
        state, provider, next
    )
    .execute(db).await?;
    Ok(state)
}

pub async fn validar_state(
    db: &PgPool,
    state: &str,
    provider: &str,
) -> Result<Option<String>> { // Returns Option<next>
    let row = sqlx::query!(
        "DELETE FROM oauth_states WHERE state = $1 AND provider = $2
         RETURNING next",
        state, provider
    )
    .fetch_optional(db).await?;
    Ok(row.and_then(|r| r.next))
}
```

## Parallel session resolution

The callback resolves CMS user and member concurrently:

```rust
let (usuario_result, membro_result) = tokio::join!(
    UsuariosRepo::buscar_por_oauth(&db, provider, &sub),
    MembrosService::resolver_oauth(&db, &config, provider, &sub, &email, &nome),
);

// If CMS user resolution fails, clean up member from session too
if let Err(e) = usuario_result {
    session.remove::<i64>("membro_id").await?;
    return Err(e.into());
}
```

## Post-login redirect decision

```rust
fn destino_pos_login(next: Option<String>, tem_usuario_cms: bool) -> String {
    if let Some(destino) = next {
        destino  // next always wins, for both CMS users and members
    } else if tem_usuario_cms {
        "/admin".to_string()
    } else {
        "/membros/area".to_string()
    }
}
```

**Key decision:** `next` takes priority over default destinations for
both CMS users and members. A CMS admin who clicked a restricted article
wants to see that article, not be dumped into `/admin`.

## next_seguro() function

Every `?next=` input MUST pass through `next_seguro()`:

```rust
pub fn next_seguro(next: &str) -> Option<String> {
    let trimmed = next.trim();
    // Must start with exactly one '/'
    if trimmed.starts_with('/') && !trimmed.starts_with("//") {
        // No scheme (javascript:, data:, etc.)
        if !trimmed.contains(':') {
            return Some(trimmed.to_string());
        }
    }
    None
}
```

Rejects:
- `//evil.com` (protocol-relative URL)
- `https://evil.com` (absolute URL)
- `javascript:alert(1)` (scheme injection)
- Empty string, whitespace

## next propagation through login templates

```html
<!-- login.html -->
<form method="post" action="/membros/login">
  <input type="hidden" name="next" value="{{ next }}">
  <!-- SSO links must also carry next -->
  <a href="/auth/google/redirect?next={{ next | urlencode }}">
    Login com Google
  </a>
  <a href="/membros/cadastro?next={{ next | urlencode }}">
    Cadastre-se
  </a>
</form>
```

The `next` value travels:
1. Query param `?next=` on the initial redirect (e.g., blocked article)
2. Handler validates with `next_seguro()`, puts in Tera context
3. Template puts it in hidden form field AND SSO link query params
4. POST handler re-validates with `next_seguro()` before using

## MembrosService::resolver_oauth

Resolution order for members:
1. `buscar_por_oauth(provider, sub)` — already linked
2. `buscar_por_email(email)` — same email, link the OAuth
3. `criar_via_oauth(...)` — create new member (if allowed by config)
4. `None` — registration not allowed

## OIDC generic provider

Own implementation (`reqwest` + `serde_json`, no `openidconnect` crate),
driven by the discovery URL:
```
OIDC_DISCOVERY_URL=https://sso.example.com/realms/myrealm
OIDC_CLIENT_ID=rustcms
OIDC_CLIENT_SECRET=secret
OIDC_BOTAO_LABEL=Login Corporativo
```

JWT is decoded via Base64url without JWKS signature verification —
sufficient for internal CMS over server-to-server HTTPS.
