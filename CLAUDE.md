# RustCMS — CLAUDE.md

Guia para o Claude Code trabalhar neste projeto. Leia antes de qualquer tarefa.

---

## O que é este projeto

CMS genérico em Rust, projeto de aprendizado, estruturado para reutilização via `.env`.
Versão atual: **v4**, feature-complete. Open source sob a org `RealRustCMS`.

Repositório: `github.com/RealRustCMS/RealRustCMS`

---

## Stack

| Componente | Tecnologia |
|---|---|
| Web framework | Axum 0.8 |
| Banco de dados | PostgreSQL via SQLx 0.8 |
| Templates | Tera (SSR) |
| Autenticação local | Argon2id + tower-sessions 0.14 |
| Session store | tower-sessions-sqlx-store-chrono 0.14 |
| OAuth/OIDC | reqwest 0.12 + serde_json (implementação própria em `services/oidc.rs`) |
| MFA | totp-rs v5 |
| Editor rich text | Quill.js 2 |
| Menu drag-and-drop | SortableJS 1.15 |
| Captcha | Cloudflare Turnstile (opcional) |
| Rate limiting | dashmap + once_cell |
| E-mail | lettre 0.11 |
| Runtime | Tokio |

---

## Ambiente de desenvolvimento

- **OS:** Windows com PowerShell
- **Editor:** Zed com Claude Code via ACP
- **Banco:** PostgreSQL local

**Zed — obrigatório em `settings.json`:**
```json
"languages": {
  "HTML": { "format_on_save": false }
}
```
O Zed formata HTML destrutivamente, quebrando a sintaxe Tera.

---

## Arquitetura

```
Router → Handlers → Services → Repositories → Database
```

```
src/
  main.rs            → inicializa servidor, popula menu_cache, executa migrations
  config.rs          → ÚNICA fonte de env vars — NUNCA usar std::env::var fora daqui
  state.rs           → AppState { db, config, tera, menu_cache }
  error.rs           → AppError: Database, Template, NaoEncontrado, NaoAutorizado, Interno
  csrf.rs            → gerar_token() / validar_token() via sessão
  rate_limit.rs      → DashMap em memória, chave "acao:ip"

  models/mod.rs      → todas as structs (Artigo, Membro, MenuItem, etc.)
  repositories/      → acesso ao banco, uma struct por domínio
  services/          → lógica de negócio
  handlers/          → Axum handlers (admin, auth, membros, oauth, publico, upload, api)
  routes/            → montagem do Router
  bin/seed.rs        → cria admin + token de API
```

---

## Regras críticas — leia antes de escrever qualquer código

### SQLx

**`SELECT *` é proibido em `artigos`, `albuns` e `paginas`.**
Essas tabelas têm coluna `busca_fts tsvector GENERATED ALWAYS AS STORED`.
SQLx falha em tempo de compilação. Sempre liste colunas explicitamente.

**`sqlx::query!` gera tipo anônimo distinto por call site.**
Se um `if/else` tem dois branches com `sqlx::query!`, o `.map()` para a struct
final deve estar **dentro de cada branch**, antes do `if/else` fechar. Nunca
deixe `Vec<Record>` cru ser o tipo de retorno do `if/else`.

```rust
// ERRADO — não compila
let rows = if flag {
    sqlx::query!("SELECT ...").fetch_all(&db).await?
} else {
    sqlx::query!("SELECT ...").fetch_all(&db).await?
};
let result: Vec<Artigo> = rows.into_iter().map(...).collect();

// CERTO
let result: Vec<Artigo> = if flag {
    sqlx::query!("SELECT ...").fetch_all(&db).await?
        .into_iter().map(|r| Artigo { ... }).collect()
} else {
    sqlx::query!("SELECT ...").fetch_all(&db).await?
        .into_iter().map(|r| Artigo { ... }).collect()
};
```

**`COUNT()` / `AVG()` retornam nullable ou NUMERIC.**
Use `CAST(AVG(col) AS FLOAT8)` para f64. Use `.unwrap_or(0)` para contagens.

### Axum 0.8

**Parâmetros de rota usam `{id}`, não `:id`.**
`:id` causa panic no startup: *"Path segments must not start with `:`"*.

**Três extractors juntos (`State` + `Session` + `Form`) podem exceder inferência.**
Se ocorrer erro de trait bound em extractor, remova `Session` se não usado.

**Funções async recursivas requerem `Box::pin`.**

### Tera

**CSS em `{% block estilos %}` deve estar dentro de `<style>...</style>`.**
`{` e `}` sem a tag são interpretados como delimitadores Tera.

**Nunca interpole conteúdo do banco dentro de `<script>`.**
A substring `</script>` em qualquer conteúdo do usuário fecha a tag — o parser
HTML não entende contexto JS. Use `{% if %}` para gerar apenas `true`/`false`.

**Booleanos PostgreSQL:** `{% if campo %}`, não `{% if campo == 1 %}`.

**Macros recursivas** devem ser definidas fora da tag `<html>`.

**`{% set %}` dentro de `{% block %}`** não funciona.

**Páginas com HTML livre (`pagina.html_bruto`) exigem `{% block fullpage %}`
no `base.html` do tema.** Envolva header+conteudo+footer nesse block; em
`pagina.html`, sobrescreva com
`{% if pagina.html_bruto %}{{ pagina.html_bruto | safe }}{% else %}{{ super() }}{% endif %}`.
Sem isso o header/nav/footer do tema aparecem mesmo em modo HTML livre.
`super()` é suportado desde Tera 1.20 — renderiza o conteúdo original do
block do pai, evita duplicar o markup no `else`.

### Admin handlers

**Todo handler que renderiza template que estende `base.html` deve injetar
o conjunto completo `ctx_base`:**
`site_nome`, `site_logo`, `usuario_nome`, `usuario_papel`, `usuario_id`,
`pagina_ativa`, `total_pendentes_global`, `csrf_token`, `tema`.

Faltar qualquer chave = erro silencioso `Failed to render`.

### Segurança

**`next_seguro()` em todo `?next=` input.** Aceita apenas paths começando com
exatamente uma `/`. Rejeita `//evil.com`, URLs absolutas, `javascript:`.

**Conteúdo restrito: sempre bloquear no servidor antes de qualquer efeito colateral.**
Checar `restrito` antes de registrar page view ou renderizar.

**`SameSite::Lax`**, não `Strict` — Strict quebra callbacks OIDC.

**`PRODUCAO=false`** por padrão para não ativar flag Secure em dev HTTP.

**CSRF excluído em:** `/api/`, `/login`, `/login/mfa*`, `/admin/upload`,
`*/salvar`, `*/destaque`, `/membros/logout`.

### services/auth.rs

`hash_senha` e `verificar_senha` são **funções livres**, não métodos:
```rust
use crate::services::auth::{hash_senha, verificar_senha};
```

### OAuth / `?next=`

O `?next=` **não pode viajar na sessão Axum** pelo fluxo OAuth — a sessão não
sobrevive ao redirect externo. Persiste na tabela `oauth_states` (coluna `next`).

`next` tem prioridade sobre destino padrão para usuário CMS e membro igualmente.

---

## Convenções do projeto

### Workflow com Claude Code

- **Arquivo por arquivo**, com confirmação antes de cada um.
- **Arquivos completos**, não diffs, exceto patches cirúrgicos (< ~10 linhas).
- **`cargo check` só ao final** de um batch de features, não após cada arquivo.
- **Confirmar migration rodou** antes de prosseguir para o código que depende dela.
- **`deco` é o template público padrão** (`TEMPLATE_PUBLICO=deco`). `default` é a alternativa (`templates/publico/*.html` na raiz, sem subpasta).
- **Commit convencional** ao final de cada sessão.

### Banco de dados

- Nomes de colunas e tabelas em **português** (`resumo`, `imagem_capa`, `criado_em`).
- Migrations são **append-only** — nunca editar uma existente.
- Schema `tower_sessions` separado de `public` — intencional (backup seletivo).
- Multi-site via `search_path` na `DATABASE_URL`.

### Config

Toda variável de ambiente passa por `config.rs`. Nunca ler `std::env::var`
diretamente fora deste módulo.

---

## Sessões

| Chave | Tipo | Significado |
|---|---|---|
| `usuario_id` | String (UUID) | Usuário CMS autenticado |
| `membro_id` | i64 | Membro da área restrita autenticado |
| `mfa_pendente_id` | String | MFA em progresso (login) |
| `mfa_setup_id` | String | MFA em progresso (setup) |

`usuario_id` e `membro_id` coexistem na mesma sessão. O middleware `requer_membro`
aceita qualquer um dos dois. O callback OAuth resolve ambos via `tokio::join!`.

---

## Dois níveis de verificação de conteúdo restrito

| Verificação | Função | Vai ao banco? | Usado para |
|---|---|---|---|
| Visibilidade | `MembrosService::sessao_membro_ativa()` | Não | Filtro de listagem, menu |
| Bloqueio de acesso | `requer_membro` middleware | Sim (`esta_ativo()`) | Acesso direto ao conteúdo |

---

## Migrações (ordem atual)

```
*_criacao_inicial.sql
*_categorias_e_tags.sql
*_api_token.sql
*_criado_por_galeria.sql
*_page_views.sql
*_comentarios.sql
*_avaliacoes.sql
*_oidc.sql
*_mfa.sql
*_paginas.sql
*_menus.sql
*_destaque.sql
*_notificacoes.sql
*_fts.sql
*_membros.sql
*_oauth_states_next.sql
*_paginas_restrito.sql
```

---

## Estado atual — v4

Funcionalidades completas:
- Autenticação Argon2id + OIDC/OAuth2 (Google, Microsoft, GitHub, genérico)
- MFA/TOTP opcional ou obrigatório
- CRUD de artigos, páginas estáticas, galeria, categorias, tags
- Menu dinâmico com SortableJS, submenus ilimitados, Nav público dinâmico
- Busca full-text (FTS) com PostgreSQL
- Área de membros completa: login local + SSO, conteúdo restrito ponta a ponta,
  itens de menu condicionais, `?next=` em todos os 4 fluxos OAuth
- API REST com Bearer token
- 6 temas admin + 2 templates públicos (default, deco — padrão atual)
- CSP dinâmico + CSP_EXTRA_* vars
- Multi-site via PostgreSQL search_path

## Próximos passos

- [ ] CSP fase 2: nonces por request (remover `unsafe-inline`)
- [ ] Perfil do membro (`/membros/perfil`) — `EditarMembroForm` existe em
      `models/mod.rs` sem handler (dead_code warning esperado). OAuth: só nome.
      Local: nome + email + senha.

---

## Referência rápida de rotas

**Públicas:**
```
GET  /                    → home
GET  /artigos             → listagem
GET  /artigos/:slug       → artigo (bloqueia se restrito sem sessão)
GET  /categoria/:slug     → por categoria
GET  /tag/:slug           → por tag
GET  /galeria             → galeria
GET  /galeria/:id         → álbum
GET  /busca?q=            → busca FTS
GET  /paginas/:slug       → página estática (bloqueia se restrito sem sessão)
GET  /rss                 → RSS 2.0
GET  /sitemap.xml         → sitemap
```

**Membros:**
```
GET/POST /membros/login      → login local (?next= suportado)
GET/POST /membros/cadastro   → auto-cadastro (?next= suportado)
POST     /membros/logout     → logout
GET      /membros/area       → área restrita (requer_membro)
```

**OAuth:**
```
GET /auth/{provider}/redirect   → inicia fluxo (?next= salvo em oauth_states)
GET /auth/{provider}/callback   → processa retorno, redireciona para next ou padrão
```
provider ∈ {google, microsoft, github, oidc}

**Admin (principais):**
```
GET/POST /admin/artigos/**          → CRUD de artigos
GET/POST /admin/paginas/**          → CRUD de páginas
GET      /admin/menu                → editor de menu
GET/POST /admin/configuracoes       → configurações do sistema
GET      /admin/membros             → listagem de membros
POST     /admin/membros/{id}/ativar → ativar/desativar/deletar membro
```
