use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use tera::Context;
use tower_sessions::Session;

use crate::csrf::gerar_token;
use crate::error::Result;
use crate::models::EventoForm;
use crate::repositories::{eventos::EventosRepo, usuarios::UsuariosRepo};
use crate::state::AppState;

const SESSION_USER_ID: &str = "usuario_id";

async fn usuario_nome_e_papel(state: &AppState, session: &Session) -> (String, String, String) {
    let id = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();
    match UsuariosRepo::novo(&state.db).buscar_por_id(&id).await {
        Ok(Some(u)) => (u.nome, u.papel, u.id),
        _ => ("Administrador".into(), "visualizador".into(), String::new()),
    }
}

fn ctx_base(
    state: &AppState,
    nome: &str,
    papel: &str,
    uid: &str,
    pagina: &str,
    csrf: &str,
) -> Context {
    let mut ctx = Context::new();
    ctx.insert("site_nome", &state.config.site_nome);
    ctx.insert("site_logo", &state.config.site_logo);
    ctx.insert("usuario_nome", nome);
    ctx.insert("usuario_papel", papel);
    ctx.insert("usuario_id", uid);
    ctx.insert("pagina_ativa", pagina);
    ctx.insert("total_pendentes_global", &0i64);
    ctx.insert("csrf_token", csrf);
    ctx.insert("tema", &state.config.tema);
    ctx
}

// Parseia o valor bruto de <input type="datetime-local"> ("YYYY-MM-DDTHH:MM"),
// tratado diretamente como UTC — mesma simplicidade usada no resto do projeto,
// sem infraestrutura de fuso horário.
fn parsear_data_hora(valor: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::NaiveDateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M")
        .ok()
        .map(|dt| dt.and_utc())
}

// Mesma lógica de gerar_slug_pagina (admin.rs) — função local para não criar
// dependência cruzada entre módulos.
fn gerar_slug(titulo: &str) -> String {
    titulo
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'a'..='z' | '0'..='9' => c,
            ' ' | '-' => '-',
            _ => '_',
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub async fn listar_eventos(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let eventos = EventosRepo::novo(&state.db)
        .listar()
        .await
        .unwrap_or_default();

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "eventos", &csrf);
    ctx.insert("eventos", &eventos);

    Ok(Html(state.tera.render("admin/eventos.html", &ctx)?))
}

pub async fn form_novo_evento(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "eventos", &csrf);
    ctx.insert("form_titulo", "Novo Evento");
    ctx.insert("form_action", "/admin/eventos");
    ctx.insert("form_botao", "Criar Evento");
    ctx.insert("evento_id", &Option::<String>::None);
    ctx.insert("titulo", &"");
    ctx.insert("descricao", &"");
    ctx.insert("data_hora", &"");
    ctx.insert("local", &"");
    ctx.insert("link_detalhes", &"");
    ctx.insert("imagem_capa", &Option::<String>::None);
    ctx.insert("publicado", &false);
    ctx.insert("erro", &Option::<String>::None);

    Ok(Html(state.tera.render("admin/evento_form.html", &ctx)?))
}

pub async fn criar_evento(
    State(state): State<AppState>,
    session: Session,
    Form(dados): Form<EventoForm>,
) -> Result<impl IntoResponse> {
    let criado_por = session
        .get::<String>(SESSION_USER_ID)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    let Some(data_hora) = parsear_data_hora(&dados.data_hora) else {
        return Ok(Redirect::to("/admin/eventos/novo"));
    };

    let repo = EventosRepo::novo(&state.db);
    let slug_base = gerar_slug(&dados.titulo);
    let slug = repo.slug_unico(&slug_base, None).await;
    let descricao = dados.descricao.as_deref().filter(|s| !s.is_empty());
    let local = dados.local.as_deref().filter(|s| !s.is_empty());
    let link_detalhes = dados.link_detalhes.as_deref().filter(|s| !s.is_empty());
    let imagem_capa = dados.imagem_capa.as_deref().filter(|s| !s.is_empty());
    let publicado = dados.publicado.is_some();

    repo.criar(
        &dados.titulo,
        &slug,
        descricao,
        data_hora,
        local,
        link_detalhes,
        imagem_capa,
        publicado,
        &criado_por,
    )
    .await?;

    tracing::info!(criado_por = %criado_por, "Evento criado");
    Ok(Redirect::to("/admin/eventos"))
}

pub async fn form_editar_evento(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let (nome, papel, uid) = usuario_nome_e_papel(&state, &session).await;
    let csrf = gerar_token(&session).await;

    let evento = EventosRepo::novo(&state.db).buscar_por_id(&id).await?;

    let mut ctx = ctx_base(&state, &nome, &papel, &uid, "eventos", &csrf);
    ctx.insert("form_titulo", "Editar Evento");
    ctx.insert("form_action", &format!("/admin/eventos/{}/editar", id));
    ctx.insert("form_botao", "Salvar");
    ctx.insert("evento_id", &Some(&id));
    ctx.insert("titulo", &evento.titulo);
    ctx.insert("descricao", &evento.descricao.unwrap_or_default());
    ctx.insert(
        "data_hora",
        &evento.data_hora.format("%Y-%m-%dT%H:%M").to_string(),
    );
    ctx.insert("local", &evento.local.unwrap_or_default());
    ctx.insert("link_detalhes", &evento.link_detalhes.unwrap_or_default());
    ctx.insert("imagem_capa", &evento.imagem_capa);
    ctx.insert("publicado", &evento.publicado);
    ctx.insert("erro", &Option::<String>::None);

    Ok(Html(state.tera.render("admin/evento_form.html", &ctx)?))
}

pub async fn salvar_evento(
    State(state): State<AppState>,
    _session: Session,
    Path(id): Path<String>,
    Form(dados): Form<EventoForm>,
) -> Result<impl IntoResponse> {
    let Some(data_hora) = parsear_data_hora(&dados.data_hora) else {
        return Ok(Redirect::to(&format!("/admin/eventos/{id}/editar")));
    };

    let repo = EventosRepo::novo(&state.db);
    let slug_base = gerar_slug(&dados.titulo);
    let slug = repo.slug_unico(&slug_base, Some(&id)).await;
    let descricao = dados.descricao.as_deref().filter(|s| !s.is_empty());
    let local = dados.local.as_deref().filter(|s| !s.is_empty());
    let link_detalhes = dados.link_detalhes.as_deref().filter(|s| !s.is_empty());
    let imagem_capa = dados.imagem_capa.as_deref().filter(|s| !s.is_empty());
    let publicado = dados.publicado.is_some();

    repo.atualizar(
        &id,
        &dados.titulo,
        &slug,
        descricao,
        data_hora,
        local,
        link_detalhes,
        imagem_capa,
        publicado,
    )
    .await?;

    tracing::info!(evento_id = %id, "Evento atualizado");
    Ok(Redirect::to("/admin/eventos"))
}

pub async fn deletar_evento(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    EventosRepo::novo(&state.db).deletar(&id).await.ok();
    tracing::info!(evento_id = %id, "Evento deletado");
    Redirect::to("/admin/eventos")
}
