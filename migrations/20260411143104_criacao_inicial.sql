CREATE TABLE usuarios (
    id            CHAR(36)      NOT NULL PRIMARY KEY,
    nome          VARCHAR(120)  NOT NULL,
    email         VARCHAR(200)  NOT NULL UNIQUE,
    senha_hash    VARCHAR(255)  NOT NULL,
    papel         VARCHAR(20)   NOT NULL DEFAULT 'editor',
    criado_em     TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

CREATE TABLE artigos (
    id            CHAR(36)      NOT NULL PRIMARY KEY,
    titulo        VARCHAR(255)  NOT NULL,
    slug          VARCHAR(255)  NOT NULL UNIQUE,
    corpo         TEXT          NOT NULL,
    status        VARCHAR(20)   NOT NULL DEFAULT 'rascunho',
    autor_id      CHAR(36)      NOT NULL REFERENCES usuarios(id),
    criado_em     TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    publicado_em  TIMESTAMPTZ   NULL
);

CREATE TABLE albuns (
    id            CHAR(36)      NOT NULL PRIMARY KEY,
    titulo        VARCHAR(255)  NOT NULL,
    descricao     TEXT          NULL,
    criado_em     TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

CREATE TABLE fotos (
    id            CHAR(36)      NOT NULL PRIMARY KEY,
    url           VARCHAR(500)  NOT NULL,
    legenda       VARCHAR(255)  NULL,
    album_id      CHAR(36)      NOT NULL REFERENCES albuns(id),
    criado_em     TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);
