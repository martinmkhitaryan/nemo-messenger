-- Credential secrets are stored as SHA-256; bound-invite salt lives with the
-- invite/pending rows so any admitter can verify without trusting the joiner.
ALTER TABLE groups ADD COLUMN IF NOT EXISTS creator BYTEA;
ALTER TABLE group_invites ADD COLUMN IF NOT EXISTS salt BYTEA;
ALTER TABLE group_pending ADD COLUMN IF NOT EXISTS invitee_binding BYTEA;
ALTER TABLE group_pending ADD COLUMN IF NOT EXISTS salt BYTEA;
