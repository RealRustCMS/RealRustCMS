use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{
    handlers::{
        admin::{
            adicionar_item_menu, alterar_senha_perfil, alterar_senha_usuario, aprovar_comentario,
            bulk_artigos, buscar_admin, criar_album, criar_artigo, criar_categoria, criar_pagina,
            criar_tag, criar_usuario, dashboard, deletar_album, deletar_artigo, deletar_categoria,
            deletar_comentario, deletar_foto, deletar_item_menu, deletar_pagina, deletar_tag,
            deletar_usuario, editar_artigo, editar_usuario, editor_menu, form_editar_artigo,
            form_editar_pagina, form_editar_usuario, form_nova_pagina, form_novo_artigo,
            gerar_token_usuario, listar_artigos, listar_comentarios, listar_galeria,
            listar_paginas, listar_usuarios, pagina_configuracoes, pagina_perfil,
            pagina_taxonomias, pagina_views, rejeitar_comentario, revogar_token_usuario,
            salvar_configuracoes, salvar_menu, salvar_pagina, salvar_perfil, toggle_destaque,
            ver_album,
        },
        eventos::{
            criar_evento, deletar_evento, form_editar_evento, form_novo_evento, listar_eventos,
            salvar_evento,
        },
        middleware::{invalidar_cache_publico, requer_admin, requer_editor, requer_login},
    },
    state::AppState,
};

pub fn rotas(state: AppState) -> Router {
    // Somente admin
    let rotas_admin = Router::new()
        .route("/admin/usuarios", get(listar_usuarios).post(criar_usuario))
        .route(
            "/admin/usuarios/{id}/editar",
            get(form_editar_usuario).post(editar_usuario),
        )
        .route("/admin/usuarios/{id}/senha", post(alterar_senha_usuario))
        .route("/admin/usuarios/{id}/deletar", post(deletar_usuario))
        .route(
            "/admin/usuarios/{id}/gerar-token",
            post(gerar_token_usuario),
        )
        .route(
            "/admin/usuarios/{id}/revogar-token",
            post(revogar_token_usuario),
        )
        .route("/admin/galeria/{id}/deletar", post(deletar_album))
        .route("/admin/galeria/fotos/{id}/deletar", post(deletar_foto))
        .route("/admin/categorias", post(criar_categoria))
        .route("/admin/categorias/{id}/deletar", post(deletar_categoria))
        .route("/admin/tags", post(criar_tag))
        .route("/admin/tags/{id}/deletar", post(deletar_tag))
        // Páginas estáticas
        .route("/admin/paginas", get(listar_paginas).post(criar_pagina))
        .route("/admin/paginas/nova", get(form_nova_pagina))
        .route(
            "/admin/paginas/{id}/editar",
            get(form_editar_pagina).post(salvar_pagina),
        )
        .route("/admin/paginas/{id}/deletar", post(deletar_pagina))
        // Editor de menu
        .route("/admin/menu", get(editor_menu))
        .route("/admin/menu/{menu_id}/itens", post(adicionar_item_menu))
        .route(
            "/admin/menu/itens/{item_id}/deletar",
            post(deletar_item_menu),
        )
        .route("/admin/menu/{menu_id}/salvar", post(salvar_menu))
        // Configurações do sistema
        .route(
            "/admin/configuracoes",
            get(pagina_configuracoes).post(salvar_configuracoes),
        )
        // Eventos / Agenda
        .route("/admin/eventos", get(listar_eventos).post(criar_evento))
        .route("/admin/eventos/novo", get(form_novo_evento))
        .route(
            "/admin/eventos/{id}/editar",
            get(form_editar_evento).post(salvar_evento),
        )
        .route("/admin/eventos/{id}/deletar", post(deletar_evento))
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_admin));

    // Admin e editor
    let rotas_editor = Router::new()
        .route("/admin/artigos/novo", get(form_novo_artigo))
        .route(
            "/admin/artigos/{id}/editar",
            get(form_editar_artigo).post(editar_artigo),
        )
        .route("/admin/artigos/{id}/deletar", post(deletar_artigo))
        .route("/admin/artigos/{id}/destaque", post(toggle_destaque))
        .route("/admin/artigos/bulk", post(bulk_artigos))
        .route("/admin/taxonomias", get(pagina_taxonomias))
        .route("/admin/galeria", post(criar_album))
        .route("/admin/comentarios", get(listar_comentarios))
        .route("/admin/comentarios/{id}/aprovar", post(aprovar_comentario))
        .route(
            "/admin/comentarios/{id}/rejeitar",
            post(rejeitar_comentario),
        )
        .route("/admin/comentarios/{id}/deletar", post(deletar_comentario))
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_editor));

    // Qualquer usuário logado — inclui visualizador
    let rotas_login = Router::new()
        .route("/admin", get(dashboard))
        .route("/admin/artigos", get(listar_artigos).post(criar_artigo))
        .route("/admin/galeria", get(listar_galeria))
        .route("/admin/galeria/{id}", get(ver_album))
        .route("/admin/perfil", get(pagina_perfil))
        .route("/admin/perfil/dados", post(salvar_perfil))
        .route("/admin/perfil/senha", post(alterar_senha_perfil))
        .route("/admin/views", get(pagina_views))
        .route("/admin/busca", get(buscar_admin))
        .route_layer(middleware::from_fn_with_state(state.clone(), requer_login));

    Router::new()
        .merge(rotas_admin)
        .merge(rotas_editor)
        .merge(rotas_login)
        // Qualquer POST/PUT/DELETE bem-sucedido no /admin limpa o cache de
        // páginas públicas — ver `invalidar_cache_publico`.
        .layer(middleware::from_fn_with_state(
            state.clone(),
            invalidar_cache_publico,
        ))
        .with_state(state)
}
