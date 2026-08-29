-- TINYINT(1) → BOOLEAN
ALTER TABLE artigos ADD COLUMN comentarios_habilitados BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE artigos ADD COLUMN moderacao_habilitada    BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE comentarios (
    id          CHAR(36)     NOT NULL PRIMARY KEY,
    url         VARCHAR(500) NOT NULL,
    autor_nome  VARCHAR(120) NOT NULL,
    autor_email VARCHAR(200) NOT NULL,
    corpo       TEXT         NOT NULL,
    status      VARCHAR(20)  NOT NULL DEFAULT 'pendente',
    criado_em   TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_comentarios_url    ON comentarios(url);
CREATE INDEX idx_comentarios_status ON comentarios(status);
