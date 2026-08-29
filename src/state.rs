use crate::config::Config;
use crate::models::MenuItemArvore;
use sqlx::PgPool;
use std::sync::Arc;
use tera::Tera;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    pub tera: Tera,
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
