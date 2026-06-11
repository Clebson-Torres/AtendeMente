-- Add payment_id and kind to record_files
ALTER TABLE record_files ADD COLUMN payment_id TEXT REFERENCES payments(id) ON DELETE RESTRICT;
ALTER TABLE record_files ADD COLUMN kind TEXT NOT NULL DEFAULT 'session_attachment'
    CHECK(kind IN ('session_attachment', 'payment_receipt'));

CREATE INDEX IF NOT EXISTS idx_record_files_payment_id ON record_files(payment_id);
