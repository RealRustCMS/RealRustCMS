-- Adiciona titulo_seo na tabela artigos e na tabela paginas.
-- NULL = usa o titulo padrão como fallback nos templates.
ALTER TABLE artigos ADD COLUMN titulo_seo TEXT;
ALTER TABLE paginas ADD COLUMN titulo_seo TEXT;
