CREATE TABLE IF NOT EXISTS backup_config (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    frequency TEXT NOT NULL DEFAULT 'never' CHECK(frequency IN ('never', 'daily', 'weekly')),
    last_backup_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO backup_config (user_id, frequency)
SELECT id, 'never' FROM users;
