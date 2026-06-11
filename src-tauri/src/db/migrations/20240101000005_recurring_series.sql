-- Patient status
ALTER TABLE patients ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
    CHECK(status IN ('active', 'inactive'));

CREATE INDEX IF NOT EXISTS idx_patients_status ON patients(status);

-- Recurring series
CREATE TABLE IF NOT EXISTS recurring_series (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
    frequency TEXT NOT NULL CHECK(frequency IN ('weekly', 'biweekly')),
    starts_on TEXT NOT NULL,
    ends_on TEXT,
    occurrences_count INTEGER,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    cancelled_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_recurring_series_user_id ON recurring_series(user_id);
CREATE INDEX IF NOT EXISTS idx_recurring_series_patient_id ON recurring_series(patient_id);

-- Link appointment to series
ALTER TABLE appointments ADD COLUMN series_id TEXT REFERENCES recurring_series(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_appointments_series_id ON appointments(series_id);
