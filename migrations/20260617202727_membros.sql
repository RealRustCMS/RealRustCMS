CREATE TABLE membros (
    id          BIGSERIAL PRIMARY KEY,
    nome        TEXT NOT NULL,
    email       TEXT NOT NULL UNIQUE,
    senha_hash  TEXT,                           -- NULL para membros OIDC-only
    ativo       BOOLEAN NOT NULL DEFAULT true,
    oauth_provider  TEXT,                       -- "google", "microsoft", "github", "generico"
    oauth_sub       TEXT,                       -- subject claim do provider
    criado_em   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- índice para lookup por email (login local)
CREATE INDEX idx_membros_email ON membros (email);

-- índice para lookup por provider+sub (callback OIDC)
CREATE INDEX idx_membros_oauth ON membros (oauth_provider, oauth_sub);

-- flag de conteúdo restrito em artigos
ALTER TABLE artigos ADD COLUMN restrito BOOLEAN NOT NULL DEFAULT false;

-- flag de item de menu restrito a membros
ALTER TABLE menu_itens ADD COLUMN restrito BOOLEAN NOT NULL DEFAULT false;
