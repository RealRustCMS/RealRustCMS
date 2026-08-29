-- Tabela de eventos/agenda institucional (assembleias, confraternizações,
-- palestras etc.). Usada no bloco "Agenda" da home pública e no CRUD do admin.
CREATE TABLE eventos (
    id             VARCHAR(36)  NOT NULL PRIMARY KEY,
    titulo         VARCHAR(255) NOT NULL,
    descricao      TEXT         NULL,
    data_hora      TIMESTAMPTZ  NOT NULL,
    local          VARCHAR(255) NULL,
    link_detalhes  VARCHAR(500) NULL,
    publicado      BOOLEAN      NOT NULL DEFAULT FALSE,
    criado_por     VARCHAR(36)  NOT NULL REFERENCES usuarios(id),
    criado_em      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    atualizado_em  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Índice para a query de "próximos eventos" (publicado = true, ordenado por data)
CREATE INDEX idx_eventos_agenda ON eventos (publicado, data_hora);
