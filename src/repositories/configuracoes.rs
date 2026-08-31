use crate::{
    error::Result,
    models::{ConfigListagemRestrita, NotificacoesConfig},
};
use sqlx::PgPool;

pub struct ConfiguracoesRepo<'a> {
    db: &'a PgPool,
}

impl<'a> ConfiguracoesRepo<'a> {
    pub fn novo(db: &'a PgPool) -> Self {
        Self { db }
    }

    /// Lê uma chave da tabela configuracoes.
    /// Retorna None se a chave não existir.
    pub async fn get(&self, chave: &str) -> Result<Option<String>> {
        let valor = sqlx::query_scalar!("SELECT valor FROM configuracoes WHERE chave = $1", chave)
            .fetch_optional(self.db)
            .await?;
        Ok(valor)
    }

    /// Insere ou atualiza uma chave (upsert).
    pub async fn set(&self, chave: &str, valor: &str) -> Result<()> {
        sqlx::query!(
            "INSERT INTO configuracoes (chave, valor) VALUES ($1, $2)
             ON CONFLICT (chave) DO UPDATE SET valor = EXCLUDED.valor",
            chave,
            valor
        )
        .execute(self.db)
        .await?;
        Ok(())
    }

    /// Lê as configurações de notificação como struct tipada.
    /// Usa Default (notif_ativa=false, email_fallback="") se as chaves não existirem.
    pub async fn get_notificacoes(&self) -> Result<NotificacoesConfig> {
        let notif_ativa = self
            .get("notif_ativa")
            .await?
            .map(|v| v == "true")
            .unwrap_or(false);

        let notif_email_fallback = self.get("notif_email_fallback").await?.unwrap_or_default();

        Ok(NotificacoesConfig {
            notif_ativa,
            notif_email_fallback,
        })
    }

    /// TTL do cache de páginas públicas, em segundos.
    /// 0 (padrão) = cache desligado — preserva o comportamento de instalações
    /// que nunca tocaram nessa configuração.
    pub async fn get_cache_ttl(&self) -> Result<u64> {
        Ok(self
            .get("cache_ttl_segundos")
            .await?
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0))
    }

    /// Lê a configuração de visibilidade de artigos restritos nas listagens
    /// públicas. Usa Default (true — mostra com badge) se a chave não existir,
    /// o que preserva o comportamento atual em instalações já existentes.
    pub async fn get_listagem_restrita(&self) -> Result<ConfigListagemRestrita> {
        let mostrar_artigos_restritos_listagem = self
            .get("mostrar_artigos_restritos_listagem")
            .await?
            .map(|v| v == "true")
            .unwrap_or(true);

        Ok(ConfigListagemRestrita {
            mostrar_artigos_restritos_listagem,
        })
    }
}