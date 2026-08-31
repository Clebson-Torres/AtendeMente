-- Indice de busca com tokens derivados.
--
-- A migration 9 criou `patient_search_tokens` guardando telefone e e-mail em texto
-- claro: a migration 8 tinha acabado de mover a PII do paciente para um blob
-- AES-256-GCM, e o indice devolvia os dois campos de contato mais identificaveis ao
-- disco, legiveis por qualquer cliente SQLite. O `identity_key` carregava o telefone
-- junto do nome normalizado, entao nem cifrar os outros dois resolveria.
--
-- Esta tabela guarda apenas HMAC-SHA256 truncado, com chave derivada do pepper.
--
-- O conteudo antigo e descartado sem cerimonia: o indice e derivado dos pacientes,
-- nao fonte de verdade. Quem o reconstroi e `reindex_search_tokens`, em Rust, no
-- proximo login — regenerar os tokens exige decifrar `pii_encrypted`, e a chave nao
-- existe dentro do SQL.
DROP TABLE IF EXISTS patient_search_tokens;

CREATE TABLE patient_search_tokens (
    user_id TEXT NOT NULL,
    patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
    token_type TEXT NOT NULL,
    token_text TEXT NOT NULL
);

-- A busca passou de `LIKE '%x%'` para igualdade, entao o indice finalmente serve
-- para alguma coisa: com o curinga a esquerda era sempre full scan.
CREATE INDEX idx_search_tokens_lookup
    ON patient_search_tokens(user_id, token_type, token_text);
CREATE INDEX idx_search_tokens_patient
    ON patient_search_tokens(patient_id);

-- Marca qual chave produziu o indice de cada usuario. Quando o fingerprint muda
-- (upgrade que altera o esquema de token, ou restore num computador com outro
-- pepper) o indice inteiro perdeu a validade e e reconstruido.
CREATE TABLE IF NOT EXISTS search_index_state (
    user_id TEXT PRIMARY KEY,
    key_fingerprint TEXT NOT NULL,
    updated_at TEXT NOT NULL
);