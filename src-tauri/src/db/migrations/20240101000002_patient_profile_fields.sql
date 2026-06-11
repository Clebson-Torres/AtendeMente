-- Add patient profile fields
ALTER TABLE patients ADD COLUMN health_history TEXT;
ALTER TABLE patients ADD COLUMN medications_in_use TEXT;
ALTER TABLE patients ADD COLUMN emergency_phone TEXT;
