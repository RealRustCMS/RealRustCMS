-- Adiciona coluna imagem_capa na tabela eventos.
-- Uma única imagem opcional, exibida só no detalhe do evento
-- (não na home nem na listagem /eventos) — mesmo padrão de artigos.
ALTER TABLE eventos ADD COLUMN imagem_capa TEXT;
