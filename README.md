# RustCMS

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://rustup.rs/)
[![Axum](https://img.shields.io/badge/axum-0.8-blue.svg)](https://github.com/tokio-rs/axum)

Um CMS (Content Management System) genérico e performático construído com **Rust**, utilizando o framework web **Axum** e banco de dados **PostgreSQL**, estruturado para ser reutilizável em qualquer instalação via `.env`.

![Dashboard do RustCMS](docs/screenshot-dashboard.png)

<details>
<summary>Mais screenshots</summary>

![Página de login](docs/screenshot-login.png)

![Artigo público](docs/screenshot-artigo.png)

</details>

## Funcionalidades

- **Artigos** — criação, edição e publicação com suporte a rascunhos, slugs únicos automáticos, editor rich text (Quill.js), resumo, imagem de capa, título SEO, tempo de leitura estimado e artigos relacionados
- **Artigo em destaque** — hero na home + grid de secundários, ordem configurável pelo admin
- **Categorias e tags** — taxonomia para organizar artigos, com páginas públicas por categoria e tag
- **Galeria de fotos** — upload de imagens organizadas em álbuns, com capa automática
- **Páginas estáticas com HTML livre** — além do editor Quill, páginas aceitam HTML bruto completo (com estilos, scripts e layout customizado); renderizadas em tela cheia sem o header/footer do site; úteis para landing pages, demos interativas e visualizações
- **Menu dinâmico** — editor drag-and-drop com submenus em qualquer profundidade; itens para páginas, artigos, categorias, tags, galeria, links externos ou títulos/agrupadores não clicáveis
- **Área de membros** — login local e SSO separados dos usuários do painel; auto-cadastro livre ou restrito por domínio de e-mail (ideal para portais corporativos); usuários do CMS acessam a área de membros automaticamente, sem segundo login
- **Conteúdo restrito a membros** — artigos, páginas e itens de menu podem ser marcados como exclusivos; visitante sem login é redirecionado ao login e volta automaticamente ao conteúdo que tentava acessar; admin decide se conteúdo restrito aparece nas listagens públicas (com indicador 🔒) ou fica totalmente oculto
- **Autenticação local** — login com sessões seguras (Argon2id + tower-sessions), persistidas no banco
- **Autenticação OIDC/OAuth2** — login via Google, Microsoft, GitHub e provedores genéricos (Keycloak, RHSSO, Authentik); botões condicionais aparecem conforme configuração no `.env`; mesmo fluxo de SSO resolve usuário do CMS e membro em paralelo
- **MFA (autenticação em dois fatores)** — TOTP compatível com Google Authenticator, Microsoft Authenticator e Authy; obrigatório globalmente (`MFA_OBRIGATORIO`) ou por usuário via painel admin; usuários OIDC são isentos (segurança delegada ao provedor)
- **Controle de acesso** — três papéis no painel: `admin`, `editor` e `visualizador`
- **Comentários** — moderação opcional e integração com Cloudflare Turnstile (captcha); notificação por e-mail configurável (global ou por artigo)
- **Avaliação por estrelas** — visitantes avaliam artigos de 1 a 5 estrelas; controle por IP (um voto por artigo), habilitável por artigo, exibida na listagem e no artigo individual; dashboard mostra os mais bem avaliados
- **Busca full-text** — pesquisa em português via PostgreSQL `tsvector`/`ts_rank`, com trechos destacados; cobre artigos, páginas e álbuns, no admin e na área pública
- **Page views** — rastreamento de visitas por URL com dashboard detalhado
- **RSS e Sitemap** — feed RSS 2.0 em `/rss` e sitemap XML em `/sitemap.xml`, ambos respeitando conteúdo restrito
- **API REST** — endpoints JSON autenticados via token Bearer
- **Painel administrativo** — interface web completa para gestão de conteúdo
- **Templates públicos** — sistema de templates intercambiáveis via `TEMPLATE_PUBLICO` no `.env`; `deco` é o template padrão atual, pronto para uso
- **Temas** — painel administrativo com 6 temas de cor configuráveis via `.env`
- **Multi-site** — múltiplas instalações independentes via schemas PostgreSQL separados, configurável só pela `DATABASE_URL`
- **Segurança** — rate limiting no login, avaliações e comentários; CSRF; cookie Secure/SameSite; Content-Security-Policy dinâmico; Permissions-Policy; MFA TOTP; headers HTTP; magic bytes e allowlist de extensões em uploads; timing attack mitigado; proteção contra open redirect em todos os fluxos de login

## Tecnologias

| Componente | Biblioteca |
|---|---|
| Web framework | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| Runtime assíncrono | [Tokio](https://tokio.rs/) 1 |
| Banco de dados | [SQLx](https://github.com/launchbadge/sqlx) 0.8 + PostgreSQL |
| Templates | [Tera](https://keats.github.io/tera/) 1 |
| Sessões | tower-sessions 0.14 + tower-sessions-sqlx-store-chrono |
| Hash de senhas | [Argon2id](https://docs.rs/argon2) |
| OIDC/OAuth2 | reqwest 0.12 + serde_json (implementação própria) |
| MFA/TOTP | [totp-rs](https://docs.rs/totp-rs) 5 |
| E-mail | [lettre](https://docs.rs/lettre) 0.11 (STARTTLS) |
| Editor rich text | [Quill.js](https://quilljs.com/) 2 |
| Menu drag-and-drop | [SortableJS](https://sortablejs.github.io/Sortable/) 1.15 |
| Logs | tracing + tracing-subscriber |
| Rate limiting | dashmap + once_cell |
| IDs únicos | UUID v4 |

## Pré-requisitos

- [Rust](https://rustup.rs/) (edição 2021 ou superior)
- PostgreSQL 13+
- [SQLx CLI](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli) para executar as migrações

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

## Configuração

Crie um arquivo `.env` na raiz do projeto com base no `.env.exemplo`.

```env
# Banco de dados (obrigatório)
# Multi-site: use search_path na querystring para isolar schemas
# postgres://usuario:senha@localhost:5432/rustcms?options=-csearch_path%3Dmeu_site
DATABASE_URL=postgres://usuario:senha@localhost:5432/rustcms

# Teto de conexões do pool (opcional, padrão 20)
# O default interno do SQLx é 10; suba para ~20-30 sob carga real
DB_MAX_CONEXOES=20

# Identidade do site (obrigatório)
SITE_NOME=Meu Site
SITE_DESCRICAO=Descrição do meu site

# Caminho do logo exibido no painel e no site público (opcional)
SITE_LOGO=/static/img/logo.png

# Porta do servidor — aceita PORT (Railway/Render/Fly.io) ou PORTA (local)
PORTA=3000

# URL base — usada para montar redirect_uri nos fluxos OIDC, RSS e sitemap
# Em produção: https://meusite.com (sem barra no final)
BASE_URL=http://localhost:3000

# Versão dos assets (opcional) — vira o `?v=` dos CSS em /static.
# Em produção /static tem Cache-Control imutável de 1 ano; mude a cada release
# (ou grave o hash do commit no build via ASSET_VER). Padrão: CARGO_PKG_VERSION.
# ASSET_VER=

# Chave de assinatura das sessões — mínimo 64 caracteres (obrigatório)
# Gere com: openssl rand -base64 64
SESSION_SECRET=uma-chave-secreta-longa-e-aleatoria-com-pelo-menos-64-caracteres

# Paginação e destaques
ARTIGOS_POR_PAGINA=10
ARTIGOS_RELACIONADOS=3
ARTIGOS_DESTAQUE=4
VIEWS_NO_DASHBOARD=5

# Tamanho máximo de upload em MB (padrão: 10)
UPLOAD_TAMANHO_MAXIMO_MB=10

# Tema do painel administrativo (padrão: verde)
# Opções: verde, cinza, azul, dourado, roxo, escuro
TEMA=verde

# Template da área pública (padrão: vazio = default)
# Opções: default, deco (recomendado — padrão atual do RustCMS)
TEMPLATE_PUBLICO=deco

# Nível de log
RUST_LOG=rustcms=info,sqlx=warn

# Ambiente — ativa Secure flag no cookie de sessão em produção
# false (padrão): desenvolvimento local sem HTTPS
# true: produção — definir no painel do Railway/Render/Fly.io ou no servidor
PRODUCAO=false

# MFA — autenticação em dois fatores (TOTP)
# false (padrão): MFA opcional — cada usuário decide no próprio perfil
# true: MFA obrigatório para todos os usuários locais
MFA_OBRIGATORIO=false

# Notificações de comentário por e-mail (opcional)
# Falha de envio é apenas logada — nunca propagada ao visitante
NOTIF_PADRAO=false
SMTP_HOST=
SMTP_PORT=587
SMTP_USUARIO=
SMTP_SENHA=

# Cloudflare Turnstile — captcha nos comentários (opcional)
TURNSTILE_SITE_KEY=
TURNSTILE_SECRET_KEY=

# CSP — fontes externas adicionais (opcional, sem recompilar)
# O RustCMS já inclui Google Fonts e Cloudflare Turnstile por padrão.
# Use estas variáveis para adicionar outras origens ao seu template.
# Múltiplas origens na mesma diretiva: separe por espaço dentro das aspas.
CSP_EXTRA_SCRIPT_SRC=
CSP_EXTRA_STYLE_SRC=
CSP_EXTRA_IMG_SRC=
CSP_EXTRA_CONNECT_SRC=

# Área de membros — auto-cadastro
# MEMBROS_DOMINIO_PERMITIDO tem precedência sobre MEMBROS_PERMITIR_CADASTRO:
# se definido, só e-mails desse domínio entram (portal corporativo).
MEMBROS_PERMITIR_CADASTRO=false
MEMBROS_DOMINIO_PERMITIDO=

# OIDC — Google (opcional, deixe vazio para desabilitar)
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=

# OIDC — Microsoft (opcional)
MICROSOFT_CLIENT_ID=
MICROSOFT_CLIENT_SECRET=
MICROSOFT_TENANT_ID=common

# OIDC — GitHub (opcional)
GITHUB_CLIENT_ID=
GITHUB_CLIENT_SECRET=

# OIDC — Provedor genérico: Keycloak, RHSSO, Authentik… (opcional)
OIDC_CLIENT_ID=
OIDC_CLIENT_SECRET=
OIDC_DISCOVERY_URL=https://sso.empresa.com/realms/org/.well-known/openid-configuration
OIDC_BOTAO_LABEL=Entrar com Empresa X

# Cadastro automático via OIDC (usuários do painel administrativo)
# false (padrão): só entra quem o admin já cadastrou previamente
# true: cria usuário novo automaticamente (papel: visualizador)
OIDC_PERMITIR_CADASTRO=false
OIDC_GENERICO_PERMITIR_CADASTRO=false
```

## Banco de Dados

Crie o banco de dados e execute as migrações:

```bash
sqlx database create
sqlx migrate run
```

As migrações criam as seguintes tabelas:

- `usuarios` — usuários do painel com papéis, token de API, campos OIDC e campos MFA
- `artigos` — artigos com slug, status, categoria, autor, flags de destaque/notificação/restrito e busca full-text
- `albuns` / `fotos` — galeria de imagens com autoria
- `categorias` / `tags` / `artigo_tags` — taxonomia dos artigos
- `paginas` — páginas estáticas com slug, corpo Quill, campo `html_bruto` para HTML livre e flag `restrito`
- `menus` / `menu_itens` — menu de navegação dinâmico com submenus ilimitados e flag `restrito` por item
- `membros` — membros da área restrita, separados dos usuários do painel; senha local opcional (membros só-SSO não têm)
- `page_views` — rastreamento de visitas por URL
- `comentarios` — comentários com estado de moderação
- `avaliacoes` — votos por estrelas (1-5), com unicidade por artigo + IP
- `configuracoes` — chave-valor genérica para configurações editáveis via painel (notificações, visibilidade de conteúdo restrito)
- `oauth_states` — states temporários para proteção CSRF dos fluxos OIDC, incluindo o destino pós-login (`next`)
- `sessoes` — sessões de login persistidas (criada automaticamente pelo tower-sessions, em schema próprio)

## Primeiro acesso

Após as migrações, crie o usuário administrador com o binário `seed`:

```bash
cargo run --bin seed -- "Nome Completo" email@exemplo.com senha123
```

A senha deve ter no mínimo 8 caracteres.

## Executando

```bash
cargo run
```

O servidor sobe em `http://localhost:3000` por padrão.

## Área de Membros

Sistema de autenticação separado dos usuários do painel administrativo, voltado para visitantes do site público.

### Cadastro

| Configuração | Comportamento |
|---|---|
| `MEMBROS_PERMITIR_CADASTRO=false` (padrão) | Auto-cadastro desabilitado — membros só entram via SSO já vinculado ou criados manualmente |
| `MEMBROS_PERMITIR_CADASTRO=true` | Qualquer visitante pode se cadastrar livremente |
| `MEMBROS_DOMINIO_PERMITIDO=empresa.com` | Tem precedência absoluta — só e-mails `@empresa.com` viram membros, via formulário ou SSO. Ideal para portais corporativos |

### Login

Usuários do painel (`admin`, `editor`, `visualizador`) acessam a área de membros automaticamente, sem precisar de um segundo cadastro — a mesma sessão cobre os dois.

Login local (e-mail/senha) e os mesmos provedores OIDC configurados para o painel (Google, Microsoft, GitHub, genérico) estão disponíveis também para membros.

### Conteúdo restrito

Artigos, páginas estáticas e itens de menu podem ser marcados como exclusivos para membros:

- **Acesso direto** (URL do artigo/página): visitante sem sessão é redirecionado ao login e retorna automaticamente ao conteúdo após autenticar — funciona com login local, cadastro e os 4 provedores OIDC
- **Listagens de artigos** (home, listagem, categoria, tag, busca, RSS, sitemap, relacionados): controlado em **Configurações → Conteúdo restrito a membros**
  - Ativado (padrão): artigos restritos aparecem com indicador 🔒, melhor para SEO e descoberta
  - Desativado: artigos restritos somem completamente das listagens para quem não está logado
- **Páginas estáticas**: não têm listagem pública, então só o bloqueio de acesso se aplica; sempre excluídas do sitemap quando restritas
- **Itens de menu**: somem do menu público para quem não está logado; se um item pai está restrito, os filhos somem junto (cascata)

Em todos os casos, quem está logado — membro ou usuário do painel — vê tudo, independente da configuração de visibilidade.

## MFA — Autenticação em Dois Fatores

O RustCMS suporta TOTP (Time-based One-Time Password), compatível com Google Authenticator, Microsoft Authenticator e Authy.

### Modos de operação

| Modo | Configuração |
|---|---|
| Opcional (padrão) | `MFA_OBRIGATORIO=false` — cada usuário ativa no próprio perfil |
| Obrigatório global | `MFA_OBRIGATORIO=true` — todos são forçados a configurar no próximo login |
| Obrigatório por usuário | Admin marca "Exigir MFA" na tela de edição do usuário |

### Fluxo do usuário

1. Acesse **Meu Perfil → Autenticação em dois fatores → Configurar MFA**
2. Escaneie o QR Code com o app autenticador
3. Confirme com o código de 6 dígitos gerado
4. A partir do próximo login, será pedido o código após a senha

### Gerenciamento pelo admin

Na tela de edição de cada usuário o admin pode:
- **Resetar MFA** — remove o secret (emergência: usuário perdeu o celular)
- **Exigir MFA** — força o usuário a configurar no próximo login
- **Cancelar exigência** — desfaz a exigência antes do próximo login

### Usuários OIDC

Usuários que fazem login via Google, Microsoft, GitHub ou provedor genérico são **isentos do MFA do RustCMS**. A segurança do segundo fator é responsabilidade do provedor externo.

## Autenticação OIDC

O RustCMS suporta login via provedores externos, tanto para usuários do painel quanto para membros. Os botões aparecem automaticamente conforme as variáveis estiverem configuradas no `.env`.

### Configurando o GitHub (exemplo mais simples)

1. Acesse `github.com/settings/developers` → **OAuth Apps** → **New OAuth App**
2. **Homepage URL**: `http://localhost:3000`
3. **Authorization callback URL**: `http://localhost:3000/auth/github/callback`
4. Copie o Client ID e gere um Client Secret
5. Adicione ao `.env`:
```env
GITHUB_CLIENT_ID=seu_client_id
GITHUB_CLIENT_SECRET=seu_client_secret
```

### Configurando Keycloak / RHSSO (provedor genérico)

```env
OIDC_CLIENT_ID=rustcms
OIDC_CLIENT_SECRET=seu_secret
OIDC_DISCOVERY_URL=https://sso.empresa.com/realms/minha-org/.well-known/openid-configuration
OIDC_BOTAO_LABEL=Entrar com SSO Corporativo
```

### Controle de acesso OIDC (usuários do painel)

Por padrão (`OIDC_PERMITIR_CADASTRO=false`), apenas usuários pré-cadastrados pelo admin conseguem entrar via OIDC. O admin cadastra o usuário com nome, e-mail e papel — na primeira autenticação OIDC o sistema vincula automaticamente o provedor ao usuário pelo e-mail.

Com `OIDC_PERMITIR_CADASTRO=true`, qualquer pessoa com conta no provedor configurado consegue entrar como `visualizador`.

Para membros, o controle de acesso via OIDC segue `MEMBROS_PERMITIR_CADASTRO`/`MEMBROS_DOMINIO_PERMITIDO`, independente das variáveis acima.

## Papéis de Usuário (painel)

| Papel | Permissões |
|---|---|
| `admin` | Tudo, incluindo menu, páginas, configurações, usuários e membros |
| `editor` | Criar e editar conteúdo; deletar apenas os próprios artigos, álbuns e fotos |
| `visualizador` | Apenas visualizar o painel, sem criar nem editar |

## Rotas

### Área Pública

```
GET  /                              → página inicial (hero + destaques + recentes)
GET  /artigos                       → listagem paginada de artigos
GET  /artigos/:slug                 → artigo individual (bloqueia se restrito sem sessão)
POST /artigos/:slug/comentarios     → postar comentário (com CSRF + rate limit)
POST /artigos/:slug/avaliar         → registrar avaliação por estrelas (com CSRF + rate limit)
GET  /categoria/:slug               → artigos por categoria
GET  /tag/:slug                     → artigos por tag
GET  /galeria                       → galeria de álbuns
GET  /galeria/:id                   → álbum individual
GET  /busca?q=termo                 → busca full-text (artigos, páginas e álbuns)
GET  /paginas/:slug                 → página estática pública (bloqueia se restrito sem sessão)
GET  /sitemap.xml                   → sitemap XML
GET  /rss                           → feed RSS 2.0
```

### Área de Membros

```
GET/POST /membros/login             → login local (?next= para retorno pós-login)
GET/POST /membros/cadastro          → auto-cadastro (controlado por flags, ?next= suportado)
POST     /membros/logout            → encerrar sessão de membro
GET      /membros/area              → área restrita (requer sessão de membro ou usuário do painel)
```

### Autenticação e MFA (painel)

```
GET/POST /login                         → login local (e-mail + senha)
POST     /logout                        → encerrar sessão
GET/POST /login/mfa                     → segunda etapa MFA (código TOTP)
GET/POST /login/mfa/configurar          → setup obrigatório de MFA no login
```

### OIDC/OAuth2 (resolve usuário do painel e membro em paralelo)

```
GET /auth/google/redirect          → iniciar login Google (?next= opcional)
GET /auth/google/callback          → callback Google
GET /auth/microsoft/redirect       → iniciar login Microsoft (?next= opcional)
GET /auth/microsoft/callback       → callback Microsoft
GET /auth/github/redirect          → iniciar login GitHub (?next= opcional)
GET /auth/github/callback          → callback GitHub
GET /auth/oidc/redirect            → iniciar login provedor genérico (?next= opcional)
GET /auth/oidc/callback            → callback provedor genérico
```

`?next=` aceita apenas caminhos relativos do próprio site (proteção contra
open redirect) e tem prioridade sobre o destino padrão — tanto para membro
quanto para usuário do painel — quando presente.

### Painel Admin

```
GET      /admin                                        → dashboard
GET      /admin/busca?q=termo                          → busca no admin
GET/POST /admin/artigos                                → listagem e criação de artigos
GET/POST /admin/artigos/:id/editar                     → edição de artigo
POST     /admin/artigos/:id/deletar                    → deleção de artigo
GET/POST /admin/galeria                                → galeria e novo álbum
GET      /admin/galeria/:id                            → ver álbum
POST     /admin/galeria/:id/deletar                    → deletar álbum
POST     /admin/galeria/fotos/:id/deletar              → deletar foto
GET      /admin/taxonomias                             → categorias e tags
GET      /admin/comentarios                            → moderação de comentários
POST     /admin/comentarios/:id/aprovar                → aprovar comentário
POST     /admin/comentarios/:id/rejeitar               → rejeitar comentário
POST     /admin/comentarios/:id/deletar                → deletar comentário
GET      /admin/usuarios                                → gestão de usuários (admin)
GET/POST /admin/perfil                                  → perfil do usuário logado
POST     /admin/perfil/senha                            → alterar senha
GET      /admin/perfil/mfa                              → configurar MFA (QR Code)
POST     /admin/perfil/mfa/ativar                       → confirmar código e ativar MFA
POST     /admin/perfil/mfa/desativar                    → desativar MFA
GET      /admin/views                                   → page views detalhadas
POST     /admin/upload/imagem                           → upload de imagem
GET      /admin/paginas                                 → listagem de páginas (admin)
GET      /admin/paginas/nova                            → nova página (admin)
POST     /admin/paginas                                 → criar página (admin)
GET/POST /admin/paginas/:id/editar                      → editar página (admin)
POST     /admin/paginas/:id/deletar                     → deletar página (admin)
GET      /admin/menu                                    → editor de menu drag-and-drop (admin)
POST     /admin/menu/:id/itens                          → adicionar item ao menu (admin)
POST     /admin/menu/itens/:id/deletar                  → deletar item do menu (admin)
POST     /admin/menu/:id/salvar                         → salvar árvore JSON (admin)
GET/POST /admin/configuracoes                           → configurações do sistema (admin)
GET      /admin/membros                                 → listagem de membros (admin)
POST     /admin/membros/:id/ativar                      → ativar membro (admin)
POST     /admin/membros/:id/desativar                   → desativar membro (admin)
POST     /admin/membros/:id/deletar                     → deletar membro (admin)
POST     /admin/usuarios/:id/mfa/resetar                → admin reseta MFA (emergência)
POST     /admin/usuarios/:id/mfa/exigir                 → admin exige MFA no próximo login
POST     /admin/usuarios/:id/mfa/remover-exigencia      → admin cancela exigência
```

### API REST

Todos os endpoints exigem o header `Authorization: Bearer <token>`. Tratam o
chamador como visitante anônimo — listagens respeitam a mesma configuração de
visibilidade de conteúdo restrito do site público.

```
GET /api/v1/artigos
GET /api/v1/artigos/:slug
GET /api/v1/categorias
GET /api/v1/categorias/:slug/artigos
GET /api/v1/tags
GET /api/v1/tags/:slug/artigos
GET /api/v1/galeria
GET /api/v1/galeria/:id
GET /api/v1/comentarios?url=/artigos/slug
GET /api/v1/comentarios/pendentes
GET /api/v1/comentarios/pendentes/url?url=
```

## Estrutura do Projeto

```
rustcms/
├── migrations/          # Migrações SQL incrementais
├── src/
│   ├── bin/
│   │   └── seed.rs      # Cria usuário admin e gera token de API
│   ├── main.rs          # Ponto de entrada — migrations + menu_cache no startup
│   ├── config.rs        # Variáveis de ambiente + CSP dinâmico + structs OIDC
│   ├── state.rs         # AppState (db + config + tera + menu_cache)
│   ├── error.rs         # AppError com IntoResponse
│   ├── csrf.rs          # Tokens CSRF via sessão
│   ├── rate_limit.rs    # Rate limiting por ação+IP (login, avaliações, comentários)
│   ├── models/          # Structs de domínio
│   ├── repositories/    # Acesso ao banco (artigos, páginas, menus, membros, busca…)
│   ├── services/        # Lógica de negócio (auth, comentários, oidc, mfa, membros)
│   ├── handlers/        # Handlers HTTP (admin, auth, mfa, oauth, membros, público, upload, api)
│   └── routes/          # Montagem das rotas + headers de segurança + cookie config
├── static/
│   ├── css/
│   │   ├── admin.css
│   │   └── temas/       # verde, cinza, azul, dourado, roxo, escuro
│   └── uploads/
└── templates/
    ├── admin/
    ├── membros/          # login, cadastro e área restrita de membros
    └── publico/
        ├── *.html        # Template "default" (genérico, sem subpasta)
        └── deco/         # Template "deco" — padrão atual do RustCMS
```

## Segurança

- **Argon2id** para hash de senhas com tempo constante (anti timing attack) — usuários do painel e membros locais
- **Mensagem genérica no login** — não distingue usuário/membro inexistente de senha errada
- **Sessões assinadas** com `SESSION_SECRET` via cookie; usuário do painel e membro coexistem na mesma sessão sem conflito
- **Cookie Secure** em produção (`PRODUCAO=true`), **SameSite=Lax**, expiração por inatividade
- **Rate limiting no login**: 5 tentativas por IP a cada 5 minutos, bloqueio de 15 minutos
- **Rate limiting em avaliações**: 10 por minuto por IP
- **Rate limiting em comentários**: 5 por 5 minutos por IP
- **MFA TOTP** — segundo fator obrigatório global ou por usuário; secret salvo apenas após código válido confirmado; sessões intermediárias bloqueiam acesso ao painel durante o fluxo; membros não têm MFA (escopo do painel)
- **CSRF** em todos os formulários POST (exceto `/api/`, `/login`, `/login/mfa*`, `/admin/upload`, rotas terminadas em `/salvar` ou `/destaque`, e `/membros/logout`)
- **OIDC state anti-CSRF** — token aleatório gravado antes do redirect, validado e deletado no callback; carrega também o destino pós-login (`next`) através do passeio pelo provedor externo
- **Proteção contra open redirect** — `?next=` (login local, cadastro e os 4 fluxos OIDC) só aceita caminhos relativos do próprio site, nunca URLs absolutas ou protocol-relative (`//evil.com`)
- **OIDC-only bloqueado no login local** — sem revelar existência ou tipo de autenticação do usuário/membro
- **Conteúdo restrito sempre verificado no servidor** — bloqueio de acesso nunca depende de estado de UI; checado antes de qualquer efeito colateral (registro de page view, render de conteúdo)
- **Magic bytes validation** no upload de imagens
- **Allowlist de extensões** no upload (jpg, jpeg, png, gif, webp)
- **Content-Security-Policy** dinâmico — fontes base hardcoded + extras via `CSP_EXTRA_*` no `.env` sem recompilar; múltiplos domínios por diretiva separados por espaço dentro de aspas duplas: `CSP_EXTRA_SCRIPT_SRC="https://cdn.a.com https://cdn.b.com"`
- **Permissions-Policy** — desabilita câmera, microfone, geolocalização, pagamento, USB e bluetooth
- **Headers HTTP**: `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy`
- **SQL Injection** impossível via SQLx query macros com bind parameters
- **Bearer token** na API REST
- **Anti-spam em avaliações** — unicidade por artigo + IP via UNIQUE constraint
- **Membros desativados** são barrados em tempo real, mesmo com sessão ativa

## Implantação em Produção

O RustCMS não serve HTTPS diretamente — use um reverse proxy na frente.

### Plataformas gerenciadas (Railway, Render, Fly.io)

Deploy direto com HTTPS automático. Configure as variáveis de ambiente no painel da plataforma. Defina `BASE_URL` com a URL pública do site e `PRODUCAO=true` para ativar o cookie Secure.

### VPS próprio (DigitalOcean, Hetzner, Oracle Cloud Free Tier…)

Use o **Caddy** como reverse proxy:

```
meusite.com {
    reverse_proxy localhost:3000
}
```

Serviço systemd em `/etc/systemd/system/rustcms.service`:

```ini
[Unit]
Description=RustCMS
After=network.target

[Service]
ExecStart=/caminho/para/rustcms
WorkingDirectory=/caminho/para/projeto
Restart=always
EnvironmentFile=/caminho/para/projeto/.env

[Install]
WantedBy=multi-user.target
```

### Multi-site

Cada instalação adicional só precisa de uma `DATABASE_URL` apontando para um
schema PostgreSQL diferente — sem precisar de uma cópia separada do binário:

```env
DATABASE_URL=postgres://usuario:senha@localhost:5432/rustcms?options=-csearch_path%3Dsite_dois
```

### Backup do banco de dados

```bash
pg_dump -U usuario rustcms > backup_$(date +%Y%m%d).sql
```

## Licença

Este projeto está sob a licença MIT.