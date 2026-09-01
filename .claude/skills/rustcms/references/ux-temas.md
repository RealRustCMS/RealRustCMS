# UX e Temas — RustCMS

## Visão geral

O RustCMS tem **dois contextos visuais** completamente independentes:

| Contexto | Base | Tema | Fontes |
|---|---|---|---|
| **Admin** | `templates/base.html` + `static/css/admin.css` | variável via `{{ tema }}` | DM Sans + DM Mono |
| **Público `default`** | `templates/publico/base.html` | variável via `{{ tema }}` | DM Sans + DM Mono |
| **Público `deco`** | `templates/publico/deco/base.html` | CSS próprio (hardcoded dark) | Cinzel + Lora + JetBrains Mono |

`deco` **não usa as variáveis de tema do admin** — tem `:root` próprio embutido no `<style>` da sua `base.html`, com toggle claro/escuro via `[data-theme="light"]` + localStorage (independente dos 6 temas do admin). Nunca tente aplicar os 6 temas admin a ele.

Ativado via `TEMPLATE_PUBLICO` no `.env` (vazio = `default`, ou `"deco"`). Selecionar UM tema ativo por vez — não há mistura.

> O tema `institucional` existiu entre 2026-08-19 e 2026-08-25 e foi removido — `deco` o substituiu como padrão público. Não recriar.

---

## Os 6 temas admin

Cada tema é um arquivo `static/css/temas/<nome>.css` que define as variáveis `:root`. O `base.html` carrega com:

```html
<link rel="stylesheet" href="{{ asset(path='css/temas/' ~ tema ~ '.css') | safe }}">
<link rel="stylesheet" href="{{ asset(path='css/admin.css') | safe }}">
```

A função Tera `asset(path=...)` (registrada em `main.rs`) prefixa `/static/` e
anexa `?v=<asset_ver>` para cache-busting — `/static` é servido com
`Cache-Control: immutable` de 1 ano em produção (`routes/mod.rs`), então a URL
precisa mudar a cada release. `asset_ver` vem de `ASSET_VER` (env) →
`option_env!("ASSET_VER")` (gravado pelo CI no build) → `CARGO_PKG_VERSION`.
Nunca referencie `/static/...` direto num template — sempre via `asset()`.

A ordem importa: o tema vem antes do `admin.css`. O `admin.css` usa as variáveis, nunca as redefine.

### Mapa de temas

| Arquivo | Personalidade | `--accent` | Sidebar |
|---|---|---|---|
| `verde.css` | Padrão, sóbrio | `#415D43` (verde musgo) | `#111D13` (escuro) |
| `escuro.css` | Dark mode | `#6C757D` (cinza médio) | `#131619` (quase preto) |
| `azul.css` | Corporativo | `#1a56cc` (azul forte) | `#0f1729` (azul naval) |
| `cinza.css` | Neutro | `#495057` (cinza escuro) | `#212529` (carvão) |
| `dourado.css` | Premium | `#C9A227` (dourado) | `#1c1708` (marrom escuro) |
| `roxo.css` | Criativo | `#7c3aed` (violeta) | `#16082e` (roxo escuro) |

**Todos os temas claros** (azul, cinza, dourado, roxo, verde) têm sidebar escura — o contraste sidebar/conteúdo é estrutural, não opcional.

---

## Variáveis CSS completas

Todas as variáveis que o `admin.css` consome. Todo template admin deve usar **apenas estas variáveis** — nunca cores hardcoded.

```css
/* Fundos — hierarquia page > surface > elevated */
--bg-page          /* fundo da página (body) */
--bg-surface       /* cards, topbar, formulários */
--bg-elevated      /* inputs, badges, form-footer, hover states */

/* Sidebar (sempre dark, independente do tema) */
--sidebar-bg
--sidebar-border

/* Accent — cor de identidade do tema */
--accent           /* botão primário, logo-mark, nav-badge, stat-line */
--accent-hover     /* hover do botão primário */
--accent-muted     /* badge admin bg, stat-icon bg, hover de nav item público */
--accent-shadow    /* box-shadow do botão primário */
--accent-text      /* texto sobre accent (sempre #ffffff) */

/* Texto */
--text-primary     /* títulos, valores de tabela em destaque */
--text-secondary   /* texto padrão de tabelas, labels */
--text-muted       /* subtítulos, placeholders, datas, topbar-title */

/* Bordas */
--border           /* borda padrão (cards, inputs, tabelas) */
--border-subtle    /* separadores de linhas de tabela */
--border-strong    /* hover de inputs, badges */

/* Navegação (sidebar) */
--nav-hover        /* fundo hover de nav-item */
--nav-active-bg    /* fundo do item ativo */
--nav-active-text  /* texto/ícone do item ativo */

/* Utilitários */
--user-av-bg       /* fundo do avatar do usuário */
--row-hover        /* hover de linha de tabela */
--btn-secondary-hover /* hover do botão secundário */

/* Badges fixos — iguais em todos os temas claros */
--badge-pub-bg / --badge-pub-text / --badge-pub-bdr   /* verde — publicado */
--badge-draft-bg / --badge-draft-txt                  /* âmbar — rascunho */

/* Perigo */
--danger           /* cor de texto/borda de erro */
--danger-bg        /* fundo de mensagem de erro */
--danger-bdr       /* borda de mensagem de erro */
```

> **Nota:** `--danger`, `--badge-pub-*` e `--badge-draft-*` são idênticos em todos os temas claros. Apenas o tema `escuro` tem valores diferentes para `--danger` e badges.

---

## Componentes admin — padrões visuais

### Estrutura de página

```html
{% extends "base.html" %}

{% block estilos %}
<style>
  /* estilos específicos desta página */
</style>
{% endblock %}

{% block topbar_acoes %}
<a href="/admin/..." class="btn btn-secondary">← Voltar</a>
{% endblock %}

{% block conteudo %}
<div class="page-header">
  <div>
    <h1 class="page-title">Título</h1>
    <p class="page-sub">Subtítulo opcional</p>
  </div>
  <!-- ação principal opcional -->
  <a href="..." class="btn btn-primary">Nova ação</a>
</div>

<!-- conteúdo aqui -->
{% endblock %}
```

### Formulário — padrão `form-card`

```html
<div class="form-card">
  <form method="post" action="...">
    <input type="hidden" name="_csrf" value="{{ csrf_token }}">
    <div class="form-body">
      <!-- campos aqui -->
      <div class="form-group">
        <label>Rótulo</label>
        <input type="text" name="campo" value="{{ valor }}">
      </div>
    </div>
    <div class="form-footer">
      <a href="..." class="btn btn-secondary">Cancelar</a>
      <button type="submit" class="btn btn-primary">Salvar</button>
    </div>
  </form>
</div>
```

**Regra crítica:** `form-body` e `form-footer` são **irmãos diretos dentro do `<form>`**, nunca aninhados um dentro do outro. `form-footer` aninhado dentro de `form-body` herda o `gap` e fica grudado nos campos.

### Botões

| Classe | Uso |
|---|---|
| `.btn.btn-primary` | Ação principal (salvar, criar) |
| `.btn.btn-secondary` | Ação secundária (cancelar, voltar, editar) |
| `.btn.btn-danger` | Ação destrutiva (deletar, revogar) |
| `.btn.btn-sm` | Versão compacta para tabelas (combinar com primary/secondary/danger) |

### Badges

| Classe | Uso |
|---|---|
| `.badge.badge-pub` | Publicado — verde |
| `.badge.badge-draft` | Rascunho — âmbar |
| `.badge.badge-admin` | Papel admin — accent do tema |
| `.badge.badge-editor` | Papel editor — azul fixo |
| `.badge.badge-visualizador` | Papel visualizador — cinza fixo |

### Tabelas

Sempre dentro de `.card > .tw > table`:
```html
<div class="card">
  <div class="tw">
    <table>
      <thead><tr><th>Col</th>...</tr></thead>
      <tbody>
        {% for item in items %}
        <tr>
          <td>...</td>
          <td><div class="acts"><!-- botões de ação --></div></td>
        </tr>
        {% endfor %}
      </tbody>
    </table>
  </div>
</div>
```

`.acts` alinha os botões de ação à direita com `gap: 5px`.

### Stats (dashboard)

```html
<div class="stats">
  <div class="stat">
    <div class="stat-icon"><svg>...</svg></div>
    <div class="stat-val">{{ numero }}</div>
    <div class="stat-lbl">Rótulo</div>
  </div>
</div>
```

`.stats` usa `grid-template-columns: repeat(4, 1fr)`. Para 3 itens, o grid ainda funciona mas deixa espaço. Para 2, prefira `repeat(2, 1fr)` com estilo local.

---

## Template público `default`

Arquivo base: `templates/publico/base.html`

- Usa os mesmos 6 temas via `{{ tema }}.css`
- Max-width do conteúdo: `860px` centrado
- Header sticky com nav dinâmico (menu_cache) + submenus hover
- Fontes: DM Sans (corpo) + DM Mono (código/mono)
- Itens de menu condicionais: `{% if item.restrito and not sessao_membro_ativa %}` oculta o item

**Blocos disponíveis:**
```
{% block titulo %}
{% block meta_seo %}
{% block estilos %}
{% block conteudo %}
{% block scripts %}
```

## Template público `deco`

Arquivo base: `templates/publico/deco/base.html`. Criado em 2026-08-19 a
partir de um mockup estático (`tema-deco/` na raiz do repo, referência de
design — não é código servido). **Padrão público do RustCMS desde
2026-08-25** (`TEMPLATE_PUBLICO=deco`).

- **CSS próprio embutido — NÃO usa as variáveis de tema do admin**
- Identidade Art Deco: dourado/preto, bordas com cantos recortados
  (`.deco-border`), losangos e linhas decorativas (`.deco-diamond`,
  `.deco-line`, `.deco-heading`)
- Fontes: **Cinzel** (display, uppercase com letter-spacing largo) + **Lora**
  (serif, corpo) + **JetBrains Mono**
- Toggle claro/escuro (`[data-theme="light"]` + localStorage
  `rustcms-theme-deco`)
- Header de 72px, logo com SVG geométrico próprio (losangos concêntricos)
- Menu mobile: hambúrguer funcional (`.hamburger`, JS `toggleMenu()`) —
  `.nav-links.aberto` vira overlay full-screen abaixo de 768px

**Variáveis próprias:**
```css
--bg / --bg2 / --bg3 / --bg4
--border
--text / --text2 / --text3
--gold / --gold-light / --gold-dark / --gold-dim / --gold-bright
--green / --green-dim                  /* só o acento verde do prompt/terminal */
--card-bg / --card-border
--serif / --mono / --display
```

---

## Checklist de revisão de template

Ao criar ou revisar qualquer template admin:

- [ ] Estende `base.html` (admin) ou `publico/base.html` ou `publico/deco/base.html`?
- [ ] CSS está dentro de `<style>...</style>` no `{% block estilos %}`? (CSS nu quebra com delimitadores Tera)
- [ ] Usa apenas variáveis CSS do tema — nenhuma cor hardcoded fora de valores fixos de badge?
- [ ] `form-footer` é irmão de `form-body`, não filho?
- [ ] Handler injeta `ctx_base` completo? (`site_nome`, `site_logo`, `usuario_nome`, `usuario_papel`, `usuario_id`, `pagina_ativa`, `total_pendentes_global`, `csrf_token`, `tema`)
- [ ] `pagina_ativa` está com o valor correto para ativar o item de nav certo?
- [ ] Booleanos PostgreSQL usam `{% if campo %}`, não `{% if campo == true %}` nem `{% if campo == 1 %}`?
- [ ] Conteúdo do banco nunca interpolado dentro de `<script>`?
- [ ] `{% set %}` não está dentro de `{% block %}`?
- [ ] Macros recursivas definidas fora de `<html>`?
- [ ] Se template público: `default` e `deco` atualizados juntos?
- [ ] Se é `pagina.html` de tema público: sobrescreve `{% block fullpage %}` checando `pagina.html_bruto` (com `{{ super() }}` no `else`)? Sem isso o modo "HTML livre" renderiza com o tema em volta.

---

## Como o tema é selecionado

O `tema` vem da tabela `configuracoes` e é injetado no `ctx_base` de todos os handlers admin. O valor é o nome do arquivo CSS sem extensão (`"verde"`, `"escuro"`, `"azul"`, etc.).

Se `tema` estiver ausente ou inválido, o browser tentará carregar `/static/css/temas/.css` e falhará silenciosamente — o layout aparecerá sem variáveis CSS, quebrando tudo. Sempre validar no handler que `tema` é um dos 6 valores conhecidos antes de renderizar.
