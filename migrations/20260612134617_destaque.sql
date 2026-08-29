-- Migration: artigo em destaque
-- Adiciona flag destaque e ordem_destaque à tabela artigos

ALTER TABLE artigos ADD COLUMN destaque BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE artigos ADD COLUMN ordem_destaque INTEGER;

-- Índice para a query da home (só artigos em destaque, ordenados)
CREATE INDEX idx_artigos_destaque ON artigos (destaque, ordem_destaque ASC NULLS LAST)
    WHERE destaque = true;
