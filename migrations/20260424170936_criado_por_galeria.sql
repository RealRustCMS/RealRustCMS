ALTER TABLE albuns ADD COLUMN criado_por CHAR(36) NULL REFERENCES usuarios(id);
ALTER TABLE fotos  ADD COLUMN criado_por CHAR(36) NULL REFERENCES usuarios(id);
