-- Full-Text Search com colunas tsvector geradas automaticamente.
-- O Postgres mantém os vetores atualizados a cada INSERT/UPDATE — sem triggers.
-- Configuração 'portuguese' aplica stemming e stopwords em português.
-- Pesos: A = título (mais relevante), B = corpo/descrição.

-- ─── ARTIGOS ────────────────────────────────────────────────────────────────

ALTER TABLE artigos
    ADD COLUMN IF NOT EXISTS busca_fts tsvector
        GENERATED ALWAYS AS (
            setweight(to_tsvector('portuguese', coalesce(titulo, '')), 'A') ||
            setweight(to_tsvector('portuguese', coalesce(resumo, '')), 'B') ||
            setweight(to_tsvector('portuguese', coalesce(
                -- remove tags HTML do corpo antes de indexar
                regexp_replace(coalesce(corpo, ''), '<[^>]+>', ' ', 'g')
            , '')), 'B')
        ) STORED;

CREATE INDEX IF NOT EXISTS artigos_busca_fts_idx ON artigos USING GIN (busca_fts);

-- ─── ÁLBUNS ─────────────────────────────────────────────────────────────────

ALTER TABLE albuns
    ADD COLUMN IF NOT EXISTS busca_fts tsvector
        GENERATED ALWAYS AS (
            setweight(to_tsvector('portuguese', coalesce(titulo, '')), 'A') ||
            setweight(to_tsvector('portuguese', coalesce(descricao, '')), 'B')
        ) STORED;

CREATE INDEX IF NOT EXISTS albuns_busca_fts_idx ON albuns USING GIN (busca_fts);

-- ─── PÁGINAS ESTÁTICAS ──────────────────────────────────────────────────────

ALTER TABLE paginas
    ADD COLUMN IF NOT EXISTS busca_fts tsvector
        GENERATED ALWAYS AS (
            setweight(to_tsvector('portuguese', coalesce(titulo, '')), 'A') ||
            setweight(to_tsvector('portuguese', coalesce(
                regexp_replace(coalesce(corpo, ''), '<[^>]+>', ' ', 'g')
            , '')), 'B')
        ) STORED;

CREATE INDEX IF NOT EXISTS paginas_busca_fts_idx ON paginas USING GIN (busca_fts);
