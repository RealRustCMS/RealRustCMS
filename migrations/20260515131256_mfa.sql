-- Adiciona suporte a MFA (TOTP) nos usuários.
-- TINYINT(1) → BOOLEAN; Postgres não tem cláusula AFTER — colunas são
-- sempre adicionadas ao final, o que é irrelevante para o funcionamento.
ALTER TABLE usuarios ADD COLUMN mfa_secret      VARCHAR(64) NULL;
ALTER TABLE usuarios ADD COLUMN mfa_habilitado  BOOLEAN     NOT NULL DEFAULT FALSE;
ALTER TABLE usuarios ADD COLUMN mfa_obrigatorio BOOLEAN     NOT NULL DEFAULT FALSE;
