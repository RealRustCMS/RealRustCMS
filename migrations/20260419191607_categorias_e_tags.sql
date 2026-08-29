CREATE TABLE categorias (
    id        CHAR(36)     NOT NULL PRIMARY KEY,
    nome      VARCHAR(100) NOT NULL UNIQUE,
    slug      VARCHAR(100) NOT NULL UNIQUE,
    criado_em TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE TABLE tags (
    id        CHAR(36)     NOT NULL PRIMARY KEY,
    nome      VARCHAR(100) NOT NULL UNIQUE,
    slug      VARCHAR(100) NOT NULL UNIQUE,
    criado_em TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE TABLE artigo_tags (
    artigo_id   CHAR(36) NOT NULL REFERENCES artigos(id),
    tag_id      CHAR(36) NOT NULL REFERENCES tags(id),
    PRIMARY KEY (artigo_id, tag_id)
);

ALTER TABLE artigos ADD COLUMN categoria_id CHAR(36) NULL REFERENCES categorias(id);
