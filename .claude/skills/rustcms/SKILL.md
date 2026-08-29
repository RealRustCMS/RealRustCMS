---
name: rustcms
description: >
  Expert skill for developing RustCMS — a CMS built with Axum 0.8, SQLx 0.8,
  PostgreSQL, and Tera templates. Use this skill for ANY task involving the
  RustCMS codebase: adding features, fixing bugs, writing migrations, editing
  templates, debugging compile errors, or reviewing architecture decisions.
  Always consult this skill when the user mentions RustCMS, ASSER-CMS, or any
  of the stack components (Axum, SQLx, Tera, tower-sessions, lettre, totp-rs)
  in the context of this project. Also triggers for: "área de membros", "conteúdo
  restrito", "admin handler", "ctx_base", "migration", "middleware", "FTS",
  "OAuth/OIDC flow" — even if the user doesn't say "RustCMS" explicitly.
---

# RustCMS — Development Skill

## Project overview

Generic CMS in Rust, learning project, structured for reuse across deployments
via `.env`. v4 is feature-complete. Stack:

| Layer | Technology |
|---|---|
| Web framework | Axum 0.8 |
| Database | PostgreSQL via SQLx 0.8 |
| Templates | Tera (SSR) |
| Auth | Argon2id + tower-sessions 0.14 |
| OAuth/OIDC | reqwest 0.12 + serde_json (own impl in `services/oidc.rs`) |
| MFA | totp-rs v5 |
| Email | lettre 0.11 |
| Rich text | Quill.js 2 |
| Drag-and-drop | SortableJS 1.15 |
| Captcha | Cloudflare Turnstile (optional) |
| Rate limiting | dashmap + once_cell |

Dev environment: Windows + PowerShell, Zed editor with Claude Code via ACP.

---

## Architecture

```
Router → Handlers → Services → Repositories → Database
```

Always follow this layering. Handlers call services and repos. Services
call repos. Repos talk to the DB. Never call repos directly from routes.

**Always read `CLAUDE.md`** at the start of any session that touches
multiple files — it is the canonical project state.

---

## Critical rules — read before writing any code

### SQLx

1. **`SELECT *` is permanently banned on `artigos`, `albuns`, and `paginas`.**
   These tables have a `tsvector GENERATED ALWAYS AS STORED` column (`busca_fts`).
   SQLx will fail at compile time. Always list columns explicitly.

2. **`sqlx::query!` generates a distinct anonymous type per call site.**
   Even two calls with identical SQL produce different types. If you have an
   `if/else` that branches on a condition and both branches call `sqlx::query!`,
   you MUST map to the final struct (`.into_iter().map(...).collect()`) **inside**
   each branch, before the `if/else` closes. Never let the raw `Vec<Record>` be
   the type returned by the `if/else` expression.

   ```rust
   // WRONG — won't compile
   let rows = if flag {
       sqlx::query!("SELECT ...").fetch_all(&db).await?
   } else {
       sqlx::query!("SELECT ...").fetch_all(&db).await?
   };
   let result: Vec<Artigo> = rows.into_iter().map(|r| ...).collect();

   // CORRECT
   let result: Vec<Artigo> = if flag {
       sqlx::query!("SELECT ...").fetch_all(&db).await?
           .into_iter().map(|r| Artigo { ... }).collect()
   } else {
       sqlx::query!("SELECT ...").fetch_all(&db).await?
           .into_iter().map(|r| Artigo { ... }).collect()
   };
   ```

3. **`COUNT()`, `AVG()`, `SUM()` return nullable or NUMERIC.**
   Use `CAST(AVG(col) AS FLOAT8)` for f64. Use `.unwrap_or(0)` for counts.

4. **Config is always read from `config.rs`.** Never use `std::env::var()`
   directly outside that module. Add new env vars to `Config` first.

### Axum 0.8

5. **Route params use `{id}`, not `:id`.** Using `:id` panics at startup:
   *"Path segments must not start with `:`"*.

6. **Three extractors (`State` + `Session` + `Form`) together may exceed
   trait bound inference.** If you get a cryptic extractor error, remove
   `Session` if it's unused in that handler.

7. **Async recursive functions require `Box::pin`.**
   Example: `salvar_arvore` in `menus.rs`.

### Tera templates

8. **CSS inside `{% block estilos %}` must be wrapped in `<style>...</style>`.**
   Bare CSS with `{` and `}` is parsed as Tera delimiters.

9. **Never interpolate user content (article body, `html_bruto`, any DB text)
   inside a JS string or `<script>` block.**
   Even a single `</script>` substring in the content closes the tag prematurely
   — the HTML parser doesn't understand JS context. Use `{% if %}` to output
   only `true`/`false`, never arbitrary text.

10. **Boolean PostgreSQL fields use `{% if campo %}`, not `{% if campo == 1 %}`.**

11. **Recursive macros must be defined outside the `<html>` tag.**

12. **`{% set %}` inside `{% block %}` does not work.** Set variables before
    the block, or restructure.

13. **Conditional `{% block %}` overrides must be inside the block itself.**

14. **Zed's format-on-save MUST be disabled for HTML files** — it destroys
    Tera syntax. Keep in `settings.json`:
    ```json
    "languages": { "HTML": { "format_on_save": false } }
    ```

### Security invariants — never violate

15. **`next_seguro()` must be used on every `?next=` input.**
    It accepts only paths starting with exactly one `/`. Rejects `//evil.com`,
    absolute URLs, and `javascript:` schemes. Use it in handlers, OAuth
    redirects, and anywhere that reads a redirect target from user input.

16. **Conteúdo restrito: always check on the server before any side effect.**
    Block before recording page views, before rendering, before anything.
    Use `sessao_membro_ativa()` for visibility decisions (no DB hit);
    use `requer_membro` middleware for actual access blocking (DB validated).

17. **`SameSite::Lax`, not `Strict`.**
    Strict breaks OIDC callbacks (cross-site POST after provider redirect).

18. **`PRODUCAO=false` by default** so Secure cookie flag doesn't break local
    HTTP dev. Set `PRODUCAO=true` only in production.

19. **CSRF excludes specific routes:** `/api/`, `/login`, `/login/mfa*`,
    `/admin/upload`, `*/salvar`, `*/destaque`, `/membros/logout`.

### Admin handlers

20. **Every handler rendering a template that extends `base.html` must inject
    the full `ctx_base` key set:**
    `site_nome`, `site_logo`, `usuario_nome`, `usuario_papel`, `usuario_id`,
    `pagina_ativa`, `total_pendentes_global`, `csrf_token`, `tema`.
    Missing any one causes a silent `Failed to render` error.

### Sessions

21. **`usuario_id` (String/UUID) and `membro_id` (i64) coexist in the same
    tower-sessions session.** They are independent keys. The OAuth callback
    resolves both in parallel via `tokio::join!`. If `resolver_usuario` fails,
    remove `membro_id` from session before redirecting.

22. **`?next=` cannot travel through the Axum session across an OAuth redirect.**
    The callback is a new request after an external round-trip. Persist `next`
    in the `oauth_states` DB table (column `next TEXT`) alongside the CSRF state.

### services/auth.rs

23. **`hash_senha` and `verificar_senha` are free functions, not methods.**
    Import with: `use crate::services::auth::{hash_senha, verificar_senha};`

---

## Workflow conventions

- **File by file, confirm before each.** Deliver complete files, not diffs,
  unless it's a surgical patch (< ~10 lines).
- **`cargo check` only at end of a feature batch**, not after every file.
- **Confirm migrations run successfully** before proceeding to code that
  depends on the new schema.
- **`deco` is the default public template** (`TEMPLATE_PUBLICO=deco`). `default`
  is the alternative (`templates/publico/*.html` at the root, no subfolder).
- **Conventional commit messages** at end of session.

---

## Database conventions

- All column/table names in Portuguese (`resumo`, not `excerpt`; `imagem_capa`,
  not `cover_image`).
- Migrations are append-only (new `.sql` files, never edit existing ones).
- Schema `tower_sessions` is intentionally separate from `public` — for
  selective backup and independent cleanup.
- Multi-site via `search_path` in `DATABASE_URL`.

---

## Key architecture decisions (already made — don't re-litigate)

| Decision | Rationale |
|---|---|
| `moka` for caching, not Redis | Single-instance; Redis only if multi-instance |
| `SameSite::Lax` | Lax survives OIDC cross-site callbacks; Strict doesn't |
| `oauth_states.next` for redirect | Session doesn't survive external OAuth round-trip |
| `sessao_membro_ativa` is no-DB | Fast visibility check; `requer_membro` middleware does the real DB-validated block |
| `mostrar_artigos_restritos_listagem` in DB config, not `.env` | Runtime toggle by admin, not deploy-time |
| `next` has priority over default dest in OAuth | User clicked a restricted article → they want the article, not `/admin` |
| API treats Bearer token as anonymous visitor | API has no member session concept |

---

## Feature state (v4)

See `CLAUDE.md` → "Estado atual — v4" for the full ✅ list.

**Known open items:**
- `EditarMembroForm` exists in `models/mod.rs` but has no handler yet
  (dead_code warning is expected). OAuth members will edit name only;
  local members will edit name + email + password.
- CSP phase 2: per-request nonces (replacing `unsafe-inline`).

---

## Reference files

For deep dives, read the relevant reference file:

- `references/sqlx-patterns.md` — SQLx query patterns, FTS queries, type mapping
- `references/tera-patterns.md` — Tera template patterns, block inheritance, macros
- `references/oauth-flow.md` — Full OAuth/OIDC flow, state persistence, next handling
- `references/security-checklist.md` — Security invariants expanded with examples
- `references/ux-temas.md` — Os 6 temas admin (variáveis CSS, personalidades), templates públicos (default vs deco), componentes visuais (form-card, badges, tabelas, stats), checklist de revisão de template
