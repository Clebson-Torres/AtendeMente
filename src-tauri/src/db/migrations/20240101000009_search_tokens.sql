CREATE TABLE IF NOT EXISTS patient_search_tokens (
    patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
    token_type TEXT NOT NULL,
    token_text TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_search_tokens_patient ON patient_search_tokens(patient_id);
CREATE INDEX IF NOT EXISTS idx_search_tokens_text ON patient_search_tokens(token_text);