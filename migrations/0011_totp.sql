ALTER TABLE users ADD COLUMN totp_secret text;
ALTER TABLE users ADD COLUMN totp_pending_secret text;
ALTER TABLE users ADD COLUMN totp_last_step bigint NOT NULL DEFAULT -1;
