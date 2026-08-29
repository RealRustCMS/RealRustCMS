-- Flag de conteúdo restrito a membros, mesma semântica de artigos.restrito.
-- Páginas estáticas não têm listagem pública (só acesso via URL direta ou
-- menu), então não há necessidade de uma flag de visibilidade em listagem
-- equivalente a mostrar_artigos_restritos_listagem — só o bloqueio de acesso.
ALTER TABLE paginas ADD COLUMN restrito BOOLEAN NOT NULL DEFAULT false;