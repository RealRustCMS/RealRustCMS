use crate::config::Config;
use crate::models::MenuItemArvore;
use sqlx::PgPool;
use std::sync::Arc;
use tera::Tera;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    // Config e Tera atrás de Arc: o extractor `State<AppState>` clona o
    // AppState inteiro a cada request. Sem o Arc, isso significava deep-clone
    // dos ~50 templates Tera e de todas as Strings do Config por request.
    // Com Arc, o clone é só um bump de refcount. Acesso continua transparente
    // via Deref (`state.config.x`, `state.tera.render(...)`).
    pub config: Arc<Config>,
    pub tera: Arc<Tera>,
    // Cache da árvore do menu principal.
    // Substituiu o Vec<PaginaMenu> — agora suporta submenus ilimitados.
    // Invalidado toda vez que o menu é salvo no admin.
    pub menu_cache: Arc<RwLock<Vec<MenuItemArvore>>>,
}

impl AppState {
    // Substitui o cache inteiro pela nova árvore.
    // Chamado no startup e após cada save no editor de menu.
    pub async fn atualizar_menu_cache(&self, itens: Vec<MenuItemArvore>) {
        let mut cache = self.menu_cache.write().await;
        *cache = itens;
    }

    // Retorna uma cópia da árvore para injetar no contexto do Tera.
    // RwLock permite múltiplos leitores simultâneos — escrita é exclusiva.
    pub async fn ler_menu_cache(&self) -> Vec<MenuItemArvore> {
        self.menu_cache.read().await.clone()
    }
}
