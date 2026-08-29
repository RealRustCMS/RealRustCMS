-- Proteção contra reuso de código TOTP dentro da mesma janela de 30s.
-- Armazena o último código aceito e o instante em que foi aceito.
-- O handler rejeita submissões onde o código é idêntico ao último
-- e o uso foi há menos de 30 segundos (UPDATE atômico — sem TOCTOU).
ALTER TABLE usuarios ADD COLUMN mfa_ultimo_codigo VARCHAR(6)     NULL;
ALTER TABLE usuarios ADD COLUMN mfa_ultimo_uso    TIMESTAMPTZ    NULL;
