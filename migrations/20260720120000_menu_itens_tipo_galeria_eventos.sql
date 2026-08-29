-- Adiciona 'galeria' e 'eventos' aos tipos válidos de item de menu.
-- 'galeria' já era tratado pelo código (resolver_urls, admin/menu.html)
-- mas nunca tinha sido incluído nesta constraint — INSERTs com esse tipo
-- estavam falhando silenciosamente até agora.
ALTER TABLE menu_itens DROP CONSTRAINT menu_itens_tipo_check;

ALTER TABLE menu_itens ADD CONSTRAINT menu_itens_tipo_check
    CHECK (tipo IN ('pagina','artigo','categoria','tag','externo','galeria','eventos'));
