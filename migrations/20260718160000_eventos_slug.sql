-- Adiciona slug único para permitir URLs públicas amigáveis (/eventos/{slug}),
-- igual ao padrão já usado em artigos e páginas.
ALTER TABLE eventos ADD COLUMN slug VARCHAR(255) NOT NULL DEFAULT '';

-- Backfill seguro para linhas existentes antes da constraint única.
UPDATE eventos SET slug = 'evento-' || id WHERE slug = '';

CREATE UNIQUE INDEX idx_eventos_slug ON eventos (slug);
ALTER TABLE eventos ALTER COLUMN slug DROP DEFAULT;
