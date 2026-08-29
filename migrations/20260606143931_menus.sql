-- Menu principal do site.
-- Por enquanto só um menu (menu-principal), mas a estrutura
-- suporta múltiplos menus no futuro sem alterar o schema.
CREATE TABLE menus (
    id         VARCHAR(36)  NOT NULL PRIMARY KEY,
    nome       VARCHAR(100) NOT NULL,
    slug       VARCHAR(100) NOT NULL UNIQUE,
    criado_em  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Itens do menu com suporte a aninhamento ilimitado via pai_id.
-- A árvore é montada no Rust após buscar todos os itens de um menu.
CREATE TABLE menu_itens (
    id             VARCHAR(36)  NOT NULL PRIMARY KEY,
    menu_id        VARCHAR(36)  NOT NULL REFERENCES menus(id) ON DELETE CASCADE,
    pai_id         VARCHAR(36)  REFERENCES menu_itens(id) ON DELETE CASCADE,
    rotulo         VARCHAR(255) NOT NULL,
    tipo           VARCHAR(20)  NOT NULL CHECK (tipo IN ('pagina','artigo','categoria','tag','externo')),
    referencia_id  VARCHAR(36),
    url_externa    VARCHAR(500),
    ordem          INTEGER      NOT NULL DEFAULT 0,
    abrir_nova_aba BOOLEAN      NOT NULL DEFAULT FALSE,
    criado_em      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Índices para as queries mais frequentes
CREATE INDEX idx_menu_itens_menu_id ON menu_itens (menu_id);
CREATE INDEX idx_menu_itens_pai_id  ON menu_itens (pai_id);

-- Insere o menu principal automaticamente.
-- O sistema sempre assume que existe um menu com slug 'menu-principal'.
-- O admin pode renomear mas não deletar (tratado no handler).
INSERT INTO menus (id, nome, slug)
VALUES ('00000000-0000-0000-0000-000000000001', 'Menu Principal', 'menu-principal');
