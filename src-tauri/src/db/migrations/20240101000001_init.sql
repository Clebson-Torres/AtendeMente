-- Users table (synced with Firebase Auth)
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    full_name TEXT,
    two_factor_enabled INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Patients
CREATE TABLE IF NOT EXISTS patients (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    full_name TEXT NOT NULL,
    phone TEXT,
    email TEXT,
    birth_date TEXT,
    admin_notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_patients_user_id ON patients(user_id);
CREATE INDEX IF NOT EXISTS idx_patients_deleted_at ON patients(deleted_at);
CREATE INDEX IF NOT EXISTS idx_patients_full_name ON patients(full_name);

-- Appointments
CREATE TABLE IF NOT EXISTS appointments (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
    starts_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'scheduled'
        CHECK(status IN ('scheduled', 'completed', 'cancelled', 'no_show')),
    session_price_cents INTEGER NOT NULL DEFAULT 0,
    quick_notes TEXT,
    cancel_reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_appointments_user_id ON appointments(user_id);
CREATE INDEX IF NOT EXISTS idx_appointments_patient_id ON appointments(patient_id);
CREATE INDEX IF NOT EXISTS idx_appointments_starts_at ON appointments(starts_at);
CREATE INDEX IF NOT EXISTS idx_appointments_status ON appointments(status);
CREATE INDEX IF NOT EXISTS idx_appointments_deleted_at ON appointments(deleted_at);

-- Payments
CREATE TABLE IF NOT EXISTS payments (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    appointment_id TEXT NOT NULL UNIQUE REFERENCES appointments(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'paid', 'cancelled')),
    method TEXT NOT NULL DEFAULT 'other'
        CHECK(method IN ('pix', 'cash', 'card', 'bank_transfer', 'other')),
    paid_at TEXT,
    amount_received_cents INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_payments_user_id ON payments(user_id);
CREATE INDEX IF NOT EXISTS idx_payments_status ON payments(status);
CREATE INDEX IF NOT EXISTS idx_payments_deleted_at ON payments(deleted_at);

-- Session Records (encrypted)
CREATE TABLE IF NOT EXISTS session_records (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
    appointment_id TEXT NOT NULL UNIQUE REFERENCES appointments(id) ON DELETE RESTRICT,
    encrypted_payload TEXT NOT NULL,
    iv TEXT NOT NULL,
    auth_tag TEXT NOT NULL,
    key_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_session_records_user_id ON session_records(user_id);
CREATE INDEX IF NOT EXISTS idx_session_records_patient_id ON session_records(patient_id);
CREATE INDEX IF NOT EXISTS idx_session_records_deleted_at ON session_records(deleted_at);

-- Record Files
CREATE TABLE IF NOT EXISTS record_files (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE RESTRICT,
    appointment_id TEXT NOT NULL REFERENCES appointments(id) ON DELETE RESTRICT,
    storage_path TEXT NOT NULL,
    original_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    uploaded_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_record_files_user_id ON record_files(user_id);
CREATE INDEX IF NOT EXISTS idx_record_files_patient_id ON record_files(patient_id);
CREATE INDEX IF NOT EXISTS idx_record_files_appointment_id ON record_files(appointment_id);
CREATE INDEX IF NOT EXISTS idx_record_files_deleted_at ON record_files(deleted_at);

-- Audit Logs
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action TEXT NOT NULL
        CHECK(action IN ('login', 'logout', 'file_upload', 'file_download', 'patient_export', 'delete', 'update')),
    entity_type TEXT NOT NULL,
    entity_id TEXT,
    ip_address TEXT,
    user_agent TEXT,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
