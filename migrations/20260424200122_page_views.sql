CREATE TABLE page_views (
    url            VARCHAR(500) NOT NULL PRIMARY KEY,
    visualizacoes  BIGINT       NOT NULL DEFAULT 0,
    ultima_visita  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);
