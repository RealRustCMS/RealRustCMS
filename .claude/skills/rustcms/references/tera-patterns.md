# Tera Patterns — RustCMS

## Block inheritance

Templates extend `base.html` (admin) or `publico/base.html` (public).

```html
{% extends "base.html" %}

{% block titulo %}Minha Página{% endblock %}

{% block conteudo %}
  <div>...</div>
{% endblock conteudo %}

{% block estilos %}
<style>
  /* CSS must be inside <style> tags */
  .my-class { color: red; }
</style>
{% endblock estilos %}
```

**NEVER put bare CSS in a block.** `{` and `}` are Tera delimiters.

## Raw HTML pages (html_bruto)

Pages with `html_bruto` override the entire layout:

```html
{% block fullpage %}
  {{ pagina.html_bruto | safe }}
{% endblock fullpage %}
```

When `fullpage` block is non-empty, `base.html` skips the normal layout.

## Boolean fields

PostgreSQL booleans come through as Rust `bool`. In Tera:
```
{% if artigo.restrito %}🔒{% endif %}
{% if config.publicado %}Publicado{% endif %}
```

**NOT** `{% if artigo.restrito == 1 %}` (that's for i8 flags, not bool).

## i8 flag fields

For fields that are `i8` (some legacy flags):
```
{% if item.ativo == 1 %}
```

## CSRF token injection

The admin `base.html` injects CSRF into all POST forms via JS.
Handlers must pass `csrf_token` in context. Do not add CSRF tokens
manually to individual forms — the base template handles it.

## JS safety — NEVER interpolate DB content

```html
<!-- WRONG — </script> inside corpo breaks parsing -->
<script>
  const modoAtual = "{{ pagina.html_bruto }}"; // DANGEROUS
  const temCorpo = "{{ artigo.corpo }}"; // DANGEROUS
</script>

<!-- CORRECT — only output safe boolean primitives -->
<script>
  const temHtmlBruto = {% if pagina.html_bruto %}true{% else %}false{% endif %};
  const estaPublicado = {% if artigo.publicado %}true{% else %}false{% endif %};
</script>
```

The HTML parser closes `<script>` on the first `</script>` substring,
even inside a JS string or comment. This applies to any user-controlled
content from the database.

## Recursive macros (menu tree)

Recursive macros must be defined **outside the `<html>` tag**, before
`{% extends %}` or at the top of the file for included macros.

```html
{% macro renderizar_item(item, nivel) %}
  <li>
    {{ item.titulo }}
    {% if item.filhos %}
      <ul>
        {% for filho in item.filhos %}
          {{ self::renderizar_item(item=filho, nivel=nivel+1) }}
        {% endfor %}
      </ul>
    {% endif %}
  </li>
{% endmacro %}
```

## Option fields

Optional fields need `is defined` check or default:
```
{% if artigo.imagem_capa is defined and artigo.imagem_capa %}
  <img src="{{ artigo.imagem_capa }}">
{% endif %}

{{ artigo.resumo | default(value="") }}
```

## URL encoding for ?next= parameter

In login/cadastro templates, `next` must be URL-encoded when used in
hrefs:
```html
<a href="/auth/google/redirect?next={{ next | urlencode }}">
  Login com Google
</a>
```

## Paginação

Standard pagination block:
```html
{% if total_paginas > 1 %}
<nav>
  {% for p in range(start=1, end=total_paginas+1) %}
    <a href="?pagina={{ p }}"
       {% if p == pagina_atual %}class="active"{% endif %}>
      {{ p }}
    </a>
  {% endfor %}
</nav>
{% endif %}
```

## Admin ctx_base keys

Every admin template that extends `base.html` receives:
- `site_nome` — site name string
- `site_logo` — logo URL or empty string
- `usuario_nome` — logged-in user display name
- `usuario_papel` — "admin" | "editor" | "visualizador"
- `usuario_id` — UUID string
- `pagina_ativa` — string for sidebar highlight (e.g., "artigos")
- `total_pendentes_global` — count of pending comments (badge)
- `csrf_token` — CSRF token for JS injection
- `tema` — theme name (e.g., "verde")

Missing any of these = silent `Failed to render` error.

## Tema CSS classes

The `tema` variable controls admin color scheme. Values:
`verde`, `azul`, `roxo`, `laranja`, `vermelho`, `cinza`

Templates should not hardcode color names; they flow through CSS variables.

## MFA template context pattern

Use explicit `contexto` variable ("perfil" or "login") injected by handler
to control form action:
```html
{% if contexto == "perfil" %}
  <form action="/admin/perfil/mfa/verificar" method="post">
{% else %}
  <form action="/login/mfa" method="post">
{% endif %}
```

Never use `mfa_habilitado is defined` to infer context.
