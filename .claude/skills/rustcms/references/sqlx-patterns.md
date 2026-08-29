# SQLx Patterns — RustCMS

## Explicit column lists (FTS tables)

Never use `SELECT *` on `artigos`, `albuns`, or `paginas`.
The `busca_fts tsvector GENERATED ALWAYS AS STORED` column breaks SQLx.

Always list columns:
```rust
sqlx::query!(
    r#"SELECT id, titulo, slug, resumo, imagem_capa, publicado,
              criado_em, atualizado_em, destaque, restrito, autor_id
       FROM artigos
       WHERE slug = $1"#,
    slug
)
```

## Conditional queries with `if/else`

Each `sqlx::query!` call site generates a distinct anonymous type.
The `.map()` to struct must happen **inside each branch**.

```rust
// Pattern for ocultar_restritos conditional
let artigos: Vec<ArtigoListagem> = if ocultar_restritos {
    sqlx::query!(
        r#"SELECT id, titulo, slug, resumo, imagem_capa, restrito
           FROM artigos WHERE publicado = true AND restrito = false
           ORDER BY criado_em DESC LIMIT $1"#,
        limit
    )
    .fetch_all(&db).await?
    .into_iter()
    .map(|r| ArtigoListagem {
        id: r.id,
        titulo: r.titulo,
        // ... all fields
    })
    .collect()
} else {
    sqlx::query!(
        r#"SELECT id, titulo, slug, resumo, imagem_capa, restrito
           FROM artigos WHERE publicado = true
           ORDER BY criado_em DESC LIMIT $1"#,
        limit
    )
    .fetch_all(&db).await?
    .into_iter()
    .map(|r| ArtigoListagem {
        id: r.id,
        titulo: r.titulo,
        // ... all fields
    })
    .collect()
};
```

## COUNT / AVG / SUM

```rust
// COUNT — returns Option<i64>
let total = sqlx::query_scalar!(
    "SELECT COUNT(*) FROM artigos WHERE publicado = true"
)
.fetch_one(&db).await?
.unwrap_or(0);

// AVG — cast to FLOAT8 to get f64
let media: Option<f64> = sqlx::query_scalar!(
    "SELECT CAST(AVG(nota) AS FLOAT8) FROM avaliacoes WHERE artigo_id = $1",
    artigo_id
)
.fetch_one(&db).await?;
```

## AvaliacaoStats pattern

`AvaliacaoStats` has a `new(count, sum)` constructor that pre-calculates
`estrelas` and `media_formatada`. Tera only serializes Serde fields, not
methods — so pre-calculate in Rust, don't compute in templates.

```rust
pub struct AvaliacaoStats {
    pub total_votos: i64,
    pub media: f64,
    pub media_formatada: String, // "4.2"
    pub estrelas: i32,           // rounded to nearest int
}

impl AvaliacaoStats {
    pub fn new(total: i64, soma: i64) -> Self { ... }
}
```

## FTS queries

Full-text search uses `websearch_to_tsquery('portuguese')`:

```rust
sqlx::query!(
    r#"SELECT id, titulo, slug,
              ts_headline('portuguese', resumo, websearch_to_tsquery('portuguese', $1),
                'MaxWords=30, MinWords=15') AS headline,
              ts_rank(busca_fts, websearch_to_tsquery('portuguese', $1)) AS rank
       FROM artigos
       WHERE busca_fts @@ websearch_to_tsquery('portuguese', $1)
         AND publicado = true
       ORDER BY rank DESC
       LIMIT 20"#,
    termo
)
```

Applies to `artigos`, `albuns`, and `paginas` (all have `busca_fts`).

## Pagination pattern

```rust
let offset = (pagina - 1) * por_pagina;
sqlx::query!(
    "... LIMIT $1 OFFSET $2",
    por_pagina as i64,
    offset as i64,
)
```

## UUID vs i64

- Usuários CMS: `id UUID` → stored as `String` in session key `usuario_id`
- Membros: `id BIGSERIAL` → stored as `i64` in session key `membro_id`
- Always match the type when fetching from session.

## Migrations

Append-only. File name convention: `{timestamp}_{description}.sql`.
Never edit an existing migration. New schema change = new migration file.

Current migrations in order:
1. `*_criacao_inicial.sql`
2. `*_categorias_e_tags.sql`
3. `*_api_token.sql`
4. `*_criado_por_galeria.sql`
5. `*_page_views.sql`
6. `*_comentarios.sql`
7. `*_avaliacoes.sql`
8. `*_oidc.sql`
9. `*_mfa.sql`
10. `*_paginas.sql`
11. `*_menus.sql`
12. `*_destaque.sql`
13. `*_notificacoes.sql`
14. `*_fts.sql`
15. `*_membros.sql`
16. `*_oauth_states_next.sql`
17. `*_paginas_restrito.sql`
