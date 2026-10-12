-- N-16: HPKE/signing secrets move to a 0600 identity file. The row keeps
-- public keys and host metadata; secret columns may be NULL.
ALTER TABLE server_identity ALTER COLUMN hpke_secret DROP NOT NULL;
ALTER TABLE server_identity ALTER COLUMN sign_secret DROP NOT NULL;
