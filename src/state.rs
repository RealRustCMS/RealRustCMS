use crate::config::Config;
use crate::models::MenuItemArvore;
use moka::future::Cache;
use sqlx::PgPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    // Cache de HTML já renderizado de páginas públicas de listagem, para
    // visitante anônimo. Chave: "pub:<rota>". Valor: o HTML pronto.
    // Habilitado só quando `cache_ttl` > 0 (config do admin). Qualquer mutação
    // no /admin chama `invalidate_all()` (middleware `invalidar_cache_publico`).
    pub pagina_cache: Cache<String, Arc<str>>,
    // TTL do `pagina_cache` em segundos, editável em runtime pelo admin.
    // 0 = cache desligado. Lido pelo `Expiry` a cada inserção — ver
    // `TtlDinamico` — então mudar aqui afeta as próximas entradas sem rebuild.
    pub cache_ttl: Arc<AtomicU64>,
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

    /// TTL atual do cache de páginas, em segundos. 0 = desligado.
    pub fn cache_ttl_segundos(&self) -> u64 {
        self.cache_ttl.load(Ordering::Relaxed)
    }

    /// Ajusta o TTL do cache de páginas em runtime (chamado ao salvar as
    /// configurações do admin). As entradas já no cache mantêm o TTL antigo
    /// até expirarem; as novas usam este valor.
    pub fn definir_cache_ttl(&self, segundos: u64) {
        self.cache_ttl.store(segundos, Ordering::Relaxed);
    }
}

/// Política de expiração do `pagina_cache` que lê o TTL de um `AtomicU64`
/// compartilhado — assim o admin muda o TTL sem reconstruir o cache.
/// `Duration::ZERO` (quando o TTL é 0) faz a entrada expirar de imediato;
/// combinado com o short-circuit no helper `pagina_cacheada`, nada chega a
/// ser inserido com o cache desligado.
pub struct TtlDinamico(pub Arc<AtomicU64>);

impl moka::Expiry<String, Arc<str>> for TtlDinamico {
    fn expire_after_create(
        &self,
        _chave: &String,
        _valor: &Arc<str>,
        _criado_em: Instant,
    ) -> Option<Duration> {
        match self.0.load(Ordering::Relaxed) {
            0 => Some(Duration::ZERO),
            segundos => Some(Duration::from_secs(segundos)),
        }
    }
}
