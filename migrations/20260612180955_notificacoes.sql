-- Migration 1: tabela de configurações genérica chave-valor
CREATE TABLE configuracoes (
    chave TEXT PRIMARY KEY,
    valor TEXT NOT NULL
);

-- Seeds iniciais para notificações
INSERT INTO configuracoes (chave, valor) VALUES
    ('notif_ativa', 'false'),
    ('notif_email_fallback', '');

-- Migration 2: flag de notificação por artigo
-- O valor padrão false respeita o opt-in — o handler injeta NOTIF_PADRAO do .env
ALTER TABLE artigos
    ADD COLUMN notificar_comentarios BOOLEAN NOT NULL DEFAULT false;
