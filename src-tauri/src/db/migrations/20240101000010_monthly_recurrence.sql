-- Allow monthly frequency in recurring_series
-- SQLite cannot ALTER CHECK constraints; recreate table
CREATE TABLE IF NOT EXISTS recurring_series_v2 (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
    frequency TEXT NOT NULL CHECK(frequency IN ('weekly', 'biweekly', 'monthly')),
    starts_on TEXT NOT NULL,
    ends_on TEXT,
    occurrences_count INTEGER,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    cancelled_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO recurring_series_v2 SELECT * FROM recurring_series;

DROP TABLE IF EXISTS recurring_series;

ALTER TABLE recurring_series_v2 RENAME TO recurring_series;

CREATE INDEX IF NOT EXISTS idx_recurring_series_user_id ON recurring_series(user_id);
CREATE INDEX IF NOT EXISTS idx_recurring_series_patient_id ON recurring_series(patient_id);
