-- Add chart number to patients
ALTER TABLE patients ADD COLUMN chart_number TEXT;

CREATE INDEX IF NOT EXISTS idx_patients_chart_number ON patients(chart_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_patients_user_chart_number
    ON patients(user_id, chart_number)
    WHERE chart_number IS NOT NULL AND chart_number != '';
