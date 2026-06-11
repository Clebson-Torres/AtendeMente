-- Rate limiting table
CREATE TABLE IF NOT EXISTS request_limits (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    identifier TEXT NOT NULL,
    hits INTEGER NOT NULL DEFAULT 0,
    window_starts_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_request_limits_scope_id ON request_limits(scope, identifier);
CREATE INDEX IF NOT EXISTS idx_request_limits_window ON request_limits(window_starts_at);
