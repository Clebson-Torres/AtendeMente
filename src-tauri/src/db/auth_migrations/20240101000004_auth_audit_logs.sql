-- Authentication audit trail.
--
-- Auth events (login, logout, lock, unlock) are written against the auth
-- database, but this table only ever existed in the per-user application
-- database. Every one of those writes was failing and being swallowed by
-- `let _ = ...`, so the LGPD trail had no authentication events at all.
--
-- No FK on user_id: `auth_users` is the identity table here, and failed logins
-- for an unknown address must still be recorded (with a `unknown:<email>`
-- pseudo-id), which a foreign key would reject.
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT,
    details TEXT NOT NULL DEFAULT '{}',
    ip_or_device TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_auth_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_auth_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_auth_audit_logs_timestamp ON audit_logs(timestamp);
