-- Torna senha_hash opcional — usuários OIDC não têm senha local.
-- Postgres usa ALTER COLUMN ... DROP NOT NULL em vez de MODIFY COLUMN.
ALTER TABLE usuarios ALTER COLUMN senha_hash DROP NOT NULL;
ALTER TABLE usuarios ALTER COLUMN senha_hash SET DEFAULT NULL;

ALTER TABLE usuarios ADD COLUMN oauth_provider VARCHAR(50)  NULL DEFAULT NULL;
ALTER TABLE usuarios ADD COLUMN oauth_sub      VARCHAR(255) NULL DEFAULT NULL;

-- UNIQUE KEY vira UNIQUE constraint no Postgres
ALTER TABLE usuarios ADD CONSTRAINT uq_oauth UNIQUE (oauth_provider, oauth_sub);
