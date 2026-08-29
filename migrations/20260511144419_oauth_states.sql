CREATE TABLE oauth_states (
    state      VARCHAR(128) NOT NULL PRIMARY KEY,
    provider   VARCHAR(50)  NOT NULL,
    criado_em  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Postgres usa CREATE INDEX em vez de INDEX dentro do CREATE TABLE
CREATE INDEX idx_oauth_states_criado_em ON oauth_states(criado_em);
