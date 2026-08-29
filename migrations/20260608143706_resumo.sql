-- Adiciona coluna resumo na tabela artigos.
-- NULL = artigo sem resumo (exibição omitida nas listagens).
ALTER TABLE artigos ADD COLUMN resumo TEXT;
