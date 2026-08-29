-- Adiciona o destino pós-login (?next=) ao registro de state OAuth.
-- Permite que o callback, que chega numa requisição nova após o passeio
-- pelo provedor externo, saiba para onde redirecionar o usuário/membro
-- de volta — sem isso, o next se perderia no meio do fluxo OAuth.
ALTER TABLE oauth_states ADD COLUMN next TEXT;