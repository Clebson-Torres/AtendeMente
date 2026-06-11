-- Add confirmation status to appointments
ALTER TABLE appointments ADD COLUMN confirmation_status TEXT NOT NULL DEFAULT 'unconfirmed'
    CHECK(confirmation_status IN ('unconfirmed', 'confirmed', 'cancelled'));

CREATE INDEX IF NOT EXISTS idx_appointments_confirmation_status ON appointments(confirmation_status);
