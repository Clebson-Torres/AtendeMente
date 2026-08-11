PRAGMA foreign_keys=off;

CREATE TABLE IF NOT EXISTS audit_logs_new (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT,
    details TEXT NOT NULL DEFAULT '{}',
    ip_or_device TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO audit_logs_new
    (id, timestamp, user_id, action, entity_type, entity_id, details, ip_or_device, created_at)
SELECT
    id,
    COALESCE(created_at, datetime('now')),
    user_id,
    action,
    entity_type,
    entity_id,
    COALESCE(metadata, '{}'),
    COALESCE(ip_address, user_agent),
    COALESCE(created_at, datetime('now'))
FROM audit_logs;

DROP TABLE audit_logs;
ALTER TABLE audit_logs_new RENAME TO audit_logs;

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp);

PRAGMA foreign_keys=on;
