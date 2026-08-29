# Rodando o RustCMS localmente

Guia de desenvolvimento local. Para a referência completa de configuração,
rotas e segurança, veja o [`README.md`](README.md).

## Pré-requisitos

- [Rust](https://rustup.rs/) stable (edição 2021+)
- PostgreSQL 13+ rodando localmente
- [sqlx-cli](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli) para as migrations:

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

> O RustCMS usa `sqlx::query!` — as queries são validadas **em tempo de
> compilação** contra um banco real. Você precisa de um Postgres acessível
> (com as migrations aplicadas) para o projeto compilar.

## 1. Clone o repositório

```bash
git clone https://github.com/RealRustCMS/RealRustCMS
cd RealRustCMS
```

## 2. Crie o banco de dados

```bash
createdb rustcms
```

Ou via `psql`:

```sql
CREATE DATABASE rustcms;
```

## 3. Configure o `.env`

```bash
cp .env.exemplo .env
```

Variáveis mínimas para rodar localmente:

```env
DATABASE_URL=postgres://postgres:senha@localhost:5432/rustcms
SITE_NOME=RustCMS
PORTA=3000
BASE_URL=http://localhost:3000
SESSION_SECRET=<gere com o comando abaixo — mínimo 64 caracteres>
TEMA=verde
TEMPLATE_PUBLICO=deco
RUST_LOG=rustcms=debug,sqlx=warn
PRODUCAO=false
```

Gerando o `SESSION_SECRET`:

```bash
# Linux/Mac
openssl rand -base64 64

# PowerShell
[Convert]::ToBase64String((1..64 | ForEach-Object { [byte](Get-Random -Max 256) }))
```

## 4. Rode as migrations

```bash
sqlx migrate run
```

O binário também aplica as migrations sozinho ao subir
(`sqlx::migrate!("./migrations")` embutido no `main.rs`), mas rodar via
`sqlx-cli` aqui garante que o banco existe antes do primeiro `cargo build`.

## 5. Crie o usuário admin

```bash
cargo run --bin seed -- "Seu Nome" seu@email.com suasenha123
```

Cria um usuário com papel `admin` e imprime um token de API no terminal.
A senha precisa ter no mínimo 8 caracteres.

## 6. Inicie o servidor

```bash
cargo run
```

- Site público: `http://localhost:3000`
- Painel admin: `http://localhost:3000/admin`

---

## Configurações opcionais

### Trocar o template público

```env
TEMPLATE_PUBLICO=default  # template genérico, sem subpasta
TEMPLATE_PUBLICO=deco     # padrão atual do RustCMS
```

### Captcha nos comentários

Crie um widget no [Cloudflare Turnstile](https://www.cloudflare.com/products/turnstile/)
e preencha no `.env`:

```env
TURNSTILE_SITE_KEY=sua_site_key
TURNSTILE_SECRET_KEY=sua_secret_key
```

Deixar as duas vazias desabilita o captcha por completo.

### Login OIDC/OAuth2

Configure os providers desejados no `.env` (veja `.env.exemplo` para a lista
completa) e defina a `BASE_URL` — ela monta o `redirect_uri`:

```env
BASE_URL=http://localhost:3000
```

O callback de cada provider é `{BASE_URL}/auth/{provider}/callback`.

### MFA

Ativável por usuário em `/admin/perfil`. Para tornar obrigatório:

```env
MFA_OBRIGATORIO=true
```

Usuários que entram via OIDC são isentos.

---

## Observações para Windows

- Use PowerShell para os comandos acima.
- No `.env`, valores com espaços vão entre aspas: `SITE_DESCRICAO="Meu site"`.
- Editor **Zed**: desabilite o format-on-save para HTML — ele corrompe a
  sintaxe Tera dos templates:

```json
// settings.json do Zed
"languages": {
  "HTML": {
    "format_on_save": false
  }
}
```

## Fluxo de trabalho

- `cargo check` ao final de um lote de mudanças, não a cada arquivo.
- Migrations são **append-only** — nunca edite uma existente; crie uma nova.
- Confirme que a migration rodou antes de escrever o código que depende dela.
- Nomes de tabelas e colunas em português (`resumo`, `imagem_capa`, `criado_em`).
