ALTER TABLE patients ADD COLUMN pii_encrypted TEXT;
ALTER TABLE patients ADD COLUMN pii_iv TEXT;
ALTER TABLE patients ADD COLUMN pii_auth_tag TEXT;