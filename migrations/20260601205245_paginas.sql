-- Tabela de páginas estáticas gerenciáveis pelo admin.
-- Usada para conteúdo que não é artigo (Sobre, Contato, Estatuto, etc.)
-- e para popular o menu público dinamicamente via cache no AppState.
CREATE TABLE paginas (
    id            VARCHAR(36)  NOT NULL PRIMARY KEY,
    titulo        VARCHAR(255) NOT NULL,
    slug          VARCHAR(255) NOT NULL UNIQUE,
    corpo         TEXT         NOT NULL DEFAULT '',
    publicada     BOOLEAN      NOT NULL DEFAULT FALSE,
    ordem         INTEGER      NOT NULL DEFAULT 0,
    criado_por    VARCHAR(36)  NOT NULL REFERENCES usuarios(id),
    criado_em     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    atualizado_em TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Índice para a rota pública /paginas/:slug
CREATE INDEX idx_paginas_slug ON paginas (slug);

-- Índice para a query do menu (só publicadas, ordenadas)
CREATE INDEX idx_paginas_menu ON paginas (publicada, ordem);
