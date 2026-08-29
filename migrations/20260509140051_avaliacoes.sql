CREATE TABLE avaliacoes (
    id        VARCHAR(36)  NOT NULL PRIMARY KEY,
    artigo_id VARCHAR(36)  NOT NULL,
    -- SMALLINT em vez de TINYINT UNSIGNED — Postgres não tem tipos unsigned.
    -- Range SMALLINT: -32768..32767 — mais que suficiente para notas 1-5.
    -- A constraint CHECK abaixo garante o range válido.
    nota      SMALLINT     NOT NULL,
    ip        VARCHAR(45)  NOT NULL,
    criado_em TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

    CONSTRAINT chk_nota CHECK (nota BETWEEN 1 AND 5),
    -- UNIQUE KEY vira UNIQUE constraint — mesma semântica, sintaxe diferente
    CONSTRAINT unico_voto UNIQUE (artigo_id, ip)
);
