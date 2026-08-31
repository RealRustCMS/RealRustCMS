use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── USUÁRIO ─────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Usuario {
    pub id: String,
    pub nome: String,
    pub email: String,
    // Opcional: usuários OIDC-only não têm senha local
    pub senha_hash: Option<String>,
    pub papel: String,
    pub api_token: Option<String>,
    // Qual provedor autenticou: "google", "microsoft", "github", "oidc"
    // None = usuário local (login com senha)
    pub oauth_provider: Option<String>,
    // Identificador único do usuário no provedor (campo "sub" do id_token)
    // Permanente mesmo que o usuário troque de e-mail no provedor
    pub oauth_sub: Option<String>,
    // ─── MFA ─────────────────────────────────────────────
    // Chave secreta TOTP em Base32. None = MFA nunca configurado.
    pub mfa_secret: Option<String>,
    // true = MFA ativo e será exigido no login
    pub mfa_habilitado: bool,
    // true = admin exigiu que este usuário configure MFA no próximo login
    pub mfa_obrigatorio: bool,
    // Último código TOTP aceito — impede reuso dentro da mesma janela de 30s
    pub mfa_ultimo_codigo: Option<String>,
    pub mfa_ultimo_uso: Option<DateTime<Utc>>,
    // ─────────────────────────────────────────────────────
    // TIMESTAMPTZ no Postgres → DateTime<Utc> no Rust via SQLx
    pub criado_em: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct NovoUsuario {
    pub nome: String,
    pub email: String,
    pub senha: String,
    pub papel: String,
}

#[derive(Debug, Deserialize)]
pub struct EditarUsuario {
    pub nome: String,
    pub email: String,
    pub papel: String,
}

#[derive(Debug, Deserialize)]
pub struct AlterarSenha {
    pub senha_nova: String,
    pub senha_confirm: String,
}

// ─── Membro ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Membro {
    pub id: i64,
    pub nome: String,
    pub email: String,
    pub senha_hash: Option<String>,
    pub ativo: bool,
    pub oauth_provider: Option<String>,
    pub oauth_sub: Option<String>,
    pub criado_em: chrono::DateTime<chrono::Utc>,
}

/// Usado no formulário de cadastro público ou pelo admin
#[derive(Debug, serde::Deserialize)]
pub struct NovoMembroForm {
    pub nome: String,
    pub email: String,
    pub senha: String, // plain text — hasheado no service
    /// Path de retorno após cadastro bem-sucedido, mesmo padrão de
    /// LoginMembroForm::next — revalidado no handler antes de qualquer redirect.
    pub next: Option<String>,
}

/// Usado no formulário de login local de membros
#[derive(Debug, serde::Deserialize)]
pub struct LoginMembroForm {
    pub email: String,
    pub senha: String,
    /// Path de retorno após login bem-sucedido (campo hidden no form,
    /// originado do ?next= da URL de login). Revalidado no handler antes
    /// de qualquer redirect — nunca usado diretamente sem checagem.
    pub next: Option<String>,
}

/// Usado pelo admin para editar um membro existente
#[derive(Debug, serde::Deserialize)]
pub struct EditarMembroForm {
    pub nome: String,
    pub email: String,
    pub ativo: Option<String>, // checkbox: Some("on") ou None
}

/// Usado pelo próprio membro para editar seu perfil em /membros/perfil
#[derive(Debug, serde::Deserialize)]
pub struct PerfilMembroForm {
    pub nome: String,
    /// Só presente para membros locais (OAuth não alteram e-mail)
    pub email: Option<String>,
    /// Preenchido apenas quando o membro quer trocar a senha
    pub senha_nova: Option<String>,
    pub senha_confirm: Option<String>,
}

/// Resumo para listagem no admin
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct MembroListagem {
    pub id: i64,
    pub nome: String,
    pub email: String,
    pub ativo: bool,
    pub oauth_provider: Option<String>,
    pub criado_em: chrono::DateTime<chrono::Utc>,
}

// ─── ARTIGO ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Artigo {
    pub id: String,
    pub titulo: String,
    pub slug: String,
    pub corpo: String,
    pub status: String,
    pub autor_id: String,
    pub categoria_id: Option<String>,
    // BOOLEAN no Postgres → bool no Rust
    pub comentarios_habilitados: bool,
    pub moderacao_habilitada: bool,
    pub avaliacoes_habilitadas: bool,
    pub resumo: Option<String>,
    pub imagem_capa: Option<String>,
    pub titulo_seo: Option<String>,
    pub destaque: bool,
    pub ordem_destaque: Option<i32>,
    pub notificar_comentarios: bool,
    // true = artigo só visível/acessível para membros (área de membros) ou usuários CMS logados
    pub restrito: bool,
    pub criado_em: DateTime<Utc>,
    pub publicado_em: Option<DateTime<Utc>>,
}

/// Dados recebidos ao criar um novo artigo (vindo do formulário)
#[derive(Debug, Clone, Deserialize)]
pub struct NovoArtigo {
    pub titulo: String,
    pub corpo: String,
    pub resumo: Option<String>,
    pub imagem_capa: Option<String>,
    pub titulo_seo: Option<String>,
    pub status: String,
    pub categoria_id: Option<String>,
    pub tags: Option<String>,
    pub comentarios_habilitados: Option<String>,
    pub moderacao_habilitada: Option<String>,
    pub avaliacoes_habilitadas: Option<String>,
    pub destaque: Option<String>,
    pub notificar_comentarios: Option<String>,
    pub restrito: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditarArtigo {
    pub titulo: String,
    pub corpo: String,
    pub resumo: Option<String>,
    pub imagem_capa: Option<String>,
    pub titulo_seo: Option<String>,
    pub status: String,
    pub categoria_id: Option<String>,
    pub tags: Option<String>,
    pub comentarios_habilitados: Option<String>,
    pub moderacao_habilitada: Option<String>,
    pub avaliacoes_habilitadas: Option<String>,
    pub destaque: Option<String>,
    pub notificar_comentarios: Option<String>,
    pub restrito: Option<String>,
}

// ─── COMENTÁRIO ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Comentario {
    pub id: String,
    pub url: String,
    pub autor_nome: String,
    pub autor_email: String,
    pub corpo: String,
    pub status: String,
    pub criado_em: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct NovoComentario {
    pub autor_nome: String,
    pub autor_email: String,
    pub corpo: String,
    #[serde(rename = "cf-turnstile-response")]
    pub turnstile_token: Option<String>,
}

// ─── ÁLBUM ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Album {
    pub id: String,
    pub titulo: String,
    pub descricao: Option<String>,
    pub criado_em: DateTime<Utc>,
    pub criado_por: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Foto {
    pub id: String,
    pub url: String,
    pub legenda: Option<String>,
    pub album_id: String,
    pub criado_em: DateTime<Utc>,
    pub criado_por: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NovoAlbum {
    pub titulo: String,
    pub descricao: Option<String>,
}

/// Álbum com a URL da primeira foto para uso na listagem pública
#[derive(Debug, Clone, Serialize)]
pub struct AlbumComCapa {
    pub id: String,
    pub titulo: String,
    pub descricao: Option<String>,
    pub criado_em: DateTime<Utc>,
    pub criado_por: Option<String>,
    pub capa_url: Option<String>,
}

/// Dados de paginação para uso nos templates
#[derive(Debug, Clone, Serialize)]
pub struct Paginacao {
    pub pagina_atual: i64,
    pub total_paginas: i64,
    pub total_itens: i64,
    pub tem_anterior: bool,
    pub tem_proxima: bool,
    pub pagina_anterior: i64,
    pub proxima_pagina: i64,
}

impl Paginacao {
    pub fn calcular(pagina_atual: i64, total_itens: i64, por_pagina: i64) -> Self {
        let total_paginas = ((total_itens as f64) / (por_pagina as f64)).ceil() as i64;
        let total_paginas = total_paginas.max(1);
        let pagina_atual = pagina_atual.clamp(1, total_paginas);

        Self {
            pagina_atual,
            total_paginas,
            total_itens,
            tem_anterior: pagina_atual > 1,
            tem_proxima: pagina_atual < total_paginas,
            pagina_anterior: pagina_atual - 1,
            proxima_pagina: pagina_atual + 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Categoria {
    pub id: String,
    pub nome: String,
    pub slug: String,
    pub criado_em: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tag {
    pub id: String,
    pub nome: String,
    pub slug: String,
    pub criado_em: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct NovaCategoria {
    pub nome: String,
}

#[derive(Debug, Deserialize)]
pub struct NovaTag {
    pub nome: String,
}

/// Artigo com categoria e tags para exibição
#[derive(Debug, Clone, Serialize)]
pub struct ArtigoCompleto {
    pub artigo: Artigo,
    pub categoria: Option<Categoria>,
    pub tags: Vec<Tag>,
    pub autor_nome: String,
}

#[derive(Debug, Deserialize)]
pub struct EditarPerfil {
    pub nome: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct AlterarSenhaPerfil {
    pub senha_atual: String,
    pub senha_nova: String,
    pub senha_confirm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PageView {
    pub url: String,
    pub visualizacoes: i64,
    pub ultima_visita: DateTime<Utc>,
}

// ─── QUERIES DE URL ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct QueryPagina {
    pub pagina: Option<i64>,
}

/// Parâmetro de query injetado pelo redirect após uma avaliação bem-sucedida.
#[derive(Debug, Deserialize)]
pub struct QueryAvaliado {
    pub avaliado: Option<String>,
}

// ─── AVALIAÇÃO ───────────────────────────────────────────

/// Form enviado pelo visitante ao votar
#[derive(Debug, Deserialize)]
pub struct NovaAvaliacao {
    pub nota: u8,
}

/// Estatísticas de avaliação — retornadas pelo repositório e injetadas no template.
#[derive(Debug, Clone, Serialize)]
pub struct AvaliacaoStats {
    pub media: f64,
    pub total: i64,
    pub media_formatada: String,
    pub estrelas: Vec<u8>, // 5 posições: 2=cheia, 1=meia, 0=vazia
}

impl AvaliacaoStats {
    pub fn new(media: f64, total: i64) -> Self {
        let media_formatada = if media == media.floor() {
            format!("{:.0}", media)
        } else {
            format!("{:.1}", media)
        };

        let mut estrelas = Vec::with_capacity(5);
        for i in 1..=5 {
            let val = i as f64;
            if media >= val - 0.25 {
                estrelas.push(2u8);
            } else if media >= val - 0.75 {
                estrelas.push(1u8);
            } else {
                estrelas.push(0u8);
            }
        }

        Self {
            media,
            total,
            media_formatada,
            estrelas,
        }
    }
}

/// Artigo com média de avaliação — resultado direto do JOIN no repositório.
/// `total` é Option<i64> porque COUNT() retorna Option no SQLx com Postgres.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArtigoAvaliado {
    pub id: String,
    pub titulo: String,
    pub slug: String,
    pub media: Option<f64>,
    pub total: i64,
}

impl ArtigoAvaliado {
    fn estrelas(media: f64) -> Vec<u8> {
        let mut resultado = Vec::with_capacity(5);
        for i in 1..=5 {
            let val = i as f64;
            if media >= val - 0.25 {
                resultado.push(2u8);
            } else if media >= val - 0.75 {
                resultado.push(1u8);
            } else {
                resultado.push(0u8);
            }
        }
        resultado
    }

    fn media_formatada(media: f64) -> String {
        if media == media.floor() {
            format!("{:.0}", media)
        } else {
            format!("{:.1}", media)
        }
    }
}

/// Versão de ArtigoAvaliado pronta para o Tera — campos pré-calculados.
#[derive(Debug, Clone, Serialize)]
pub struct ArtigoAvaliadoView {
    pub id: String,
    pub titulo: String,
    pub slug: String,
    pub media_formatada: String,
    pub total: i64,
    pub estrelas: Vec<u8>,
}

impl From<ArtigoAvaliado> for ArtigoAvaliadoView {
    fn from(a: ArtigoAvaliado) -> Self {
        let media = a.media.unwrap_or(0.0);
        Self {
            estrelas: ArtigoAvaliado::estrelas(media),
            media_formatada: ArtigoAvaliado::media_formatada(media),
            id: a.id,
            titulo: a.titulo,
            slug: a.slug,
            total: a.total,
        }
    }
}

/// Artigo relacionado — exibido no rodapé do artigo individual.
/// Subconjunto de campos: sem corpo, sem flags, sem avaliação.
#[derive(Debug, Clone, Serialize)]
pub struct ArtigoRelacionado {
    pub id: String,
    pub titulo: String,
    pub slug: String,
    pub resumo: Option<String>,
    pub imagem_capa: Option<String>,
    pub publicado_em: Option<DateTime<Utc>>,
}

/// Artigo com avaliação opcional — usado nas listagens públicas.
#[derive(Debug, Clone, Serialize)]
pub struct ArtigoListagem {
    pub id: String,
    pub titulo: String,
    pub slug: String,
    pub status: String,
    pub resumo: Option<String>,
    pub imagem_capa: Option<String>,
    pub publicado_em: Option<DateTime<Utc>>,
    pub avaliacao: Option<AvaliacaoStats>,
    pub tempo_leitura: u32,
    pub destaque: bool,
    pub restrito: bool,
    pub categoria_nome: Option<String>,
}

impl ArtigoListagem {
    pub fn from_artigo(
        artigo: Artigo,
        avaliacao: Option<AvaliacaoStats>,
        categoria_nome: Option<String>,
    ) -> Self {
        let tempo_leitura = calcular_tempo_leitura(&artigo.corpo);
        Self {
            id: artigo.id,
            titulo: artigo.titulo,
            slug: artigo.slug,
            status: artigo.status,
            resumo: artigo.resumo,
            imagem_capa: artigo.imagem_capa,
            publicado_em: artigo.publicado_em,
            avaliacao,
            tempo_leitura,
            destaque: artigo.destaque,
            restrito: artigo.restrito,
            categoria_nome,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Pagina {
    pub id: String,
    pub titulo: String,
    pub slug: String,
    pub corpo: String,
    pub publicada: bool,
    pub ordem: i32,
    pub titulo_seo: Option<String>,
    pub criado_por: String,
    pub criado_em: chrono::DateTime<chrono::Utc>,
    pub atualizado_em: chrono::DateTime<chrono::Utc>,
    // Conteúdo HTML bruto opcional — quando presente, é renderizado no lugar do corpo Quill.
    // Útil para páginas com layout customizado (demos, landing pages, etc.)
    pub html_bruto: Option<String>,
    // true = página só visível/acessível para membros (área de membros) ou usuários CMS logados
    pub restrito: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct PaginaForm {
    pub titulo: String,
    pub corpo: String,
    pub publicada: Option<String>, // checkbox — Some("on") ou None
    pub ordem: i32,
    pub titulo_seo: Option<String>,
    // Presente quando o modo "HTML livre" está ativo no form
    pub html_bruto: Option<String>,
    pub restrito: Option<String>, // checkbox
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Evento {
    pub id: String,
    pub titulo: String,
    pub slug: String,
    pub descricao: Option<String>,
    pub data_hora: chrono::DateTime<chrono::Utc>,
    pub local: Option<String>,
    pub link_detalhes: Option<String>,
    pub imagem_capa: Option<String>,
    pub publicado: bool,
    pub criado_por: String,
    pub criado_em: chrono::DateTime<chrono::Utc>,
    pub atualizado_em: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Deserialize)]
pub struct EventoForm {
    pub titulo: String,
    pub descricao: Option<String>,
    // valor bruto de <input type="datetime-local">: "YYYY-MM-DDTHH:MM" — parseado no handler
    pub data_hora: String,
    pub local: Option<String>,
    pub link_detalhes: Option<String>,
    pub imagem_capa: Option<String>,
    pub publicado: Option<String>, // checkbox — Some("on") ou None
}

// ─── TEMPO DE LEITURA ────────────────────────────────────

/// Calcula tempo estimado de leitura em minutos.
/// Remove tags HTML antes de contar palavras, sem regex e sem dependência nova.
/// 200 palavras/min é a média de leitura confortável em português.
pub fn calcular_tempo_leitura(corpo: &str) -> u32 {
    let mut texto = String::with_capacity(corpo.len());
    let mut dentro_tag = false;
    for c in corpo.chars() {
        match c {
            '<' => dentro_tag = true,
            '>' => dentro_tag = false,
            _ if !dentro_tag => texto.push(c),
            _ => {}
        }
    }
    let palavras = texto.split_whitespace().count();
    let minutos = (palavras as f64 / 200.0).ceil() as u32;
    minutos.max(1)
}

// ─── MENU ────────────────────────────────────────────────

/// Registro da tabela menus
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Menu {
    pub id: String,
    pub nome: String,
    pub slug: String,
    pub criado_em: DateTime<Utc>,
}

/// Registro da tabela menu_itens — espelho plano da tabela
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MenuItem {
    pub id: String,
    pub menu_id: String,
    pub pai_id: Option<String>,
    pub rotulo: String,
    pub tipo: String, // pagina | artigo | categoria | tag | externo
    pub referencia_id: Option<String>,
    pub url_externa: Option<String>,
    pub ordem: i32,
    pub abrir_nova_aba: bool,
    // true = item só visível no menu para membros (área de membros) ou usuários CMS logados
    pub restrito: bool,
}

/// Nó da árvore de menu — usado no cache e nos templates.
/// filhos é recursivo: cada item pode ter N filhos, cada filho N netos, etc.
/// url é resolvida no Rust antes de serializar — o template nunca precisa
/// calcular a URL a partir do tipo + referencia_id.
#[derive(Debug, Clone, Serialize)]
pub struct MenuItemArvore {
    pub id: String,
    pub rotulo: String,
    pub url: String,
    pub abrir_nova_aba: bool,
    pub restrito: bool,
    pub filhos: Vec<MenuItemArvore>,
}

/// Form para adicionar/editar um item de menu no admin
#[derive(Debug, Deserialize)]
pub struct MenuItemForm {
    pub rotulo: String,
    pub tipo: String,
    pub referencia_id: Option<String>,
    pub url_externa: Option<String>,
    pub abrir_nova_aba: Option<String>, // checkbox
    pub restrito: Option<String>,       // checkbox
}

/// Payload JSON enviado pelo editor drag-and-drop ao salvar o menu.
/// O JS serializa a árvore inteira como JSON e envia via fetch POST.
/// Cada item tem id e lista de filhos (também com seus filhos, etc.)
#[derive(Debug, Deserialize)]
pub struct SalvarMenuPayload {
    pub itens: Vec<SalvarMenuNo>,
}

#[derive(Debug, Deserialize)]
pub struct SalvarMenuNo {
    pub id: String,
    pub filhos: Vec<SalvarMenuNo>, // recursivo
}

// ─── CONFIGURAÇÕES ───────────────────────────────────────

/// Configurações de notificação lidas da tabela `configuracoes`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct NotificacoesConfig {
    pub notif_ativa: bool,
    pub notif_email_fallback: String,
}

/// Configuração de visibilidade de artigos restritos nas listagens públicas.
/// Quando false, artigos com `restrito = true` somem completamente das
/// listagens (home, /artigos, categoria, tag, busca, RSS) para quem não
/// está logado como membro/usuário — o acesso direto via slug continua
/// bloqueado independentemente desta flag.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigListagemRestrita {
    pub mostrar_artigos_restritos_listagem: bool,
}

impl Default for ConfigListagemRestrita {
    fn default() -> Self {
        // Padrão true — mais visibilidade/SEO; admin pode desativar explicitamente
        Self {
            mostrar_artigos_restritos_listagem: true,
        }
    }
}

/// Form combinado da página de configurações — um único POST cobre
/// notificações por e-mail, visibilidade de conteúdo restrito e o TTL do
/// cache de páginas públicas.
#[derive(Debug, Deserialize)]
pub struct ConfiguracoesGeraisForm {
    pub notif_ativa: Option<String>,
    pub notif_email_fallback: Option<String>,
    pub mostrar_artigos_restritos_listagem: Option<String>,
    /// TTL do cache de páginas em segundos; vazio ou não-numérico vira 0 (off).
    pub cache_ttl_segundos: Option<String>,
}