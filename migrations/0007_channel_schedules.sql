ALTER TABLE notification_channels ADD COLUMN quiet_start_minute integer;
ALTER TABLE notification_channels ADD COLUMN quiet_end_minute integer;
ALTER TABLE notification_channels ADD COLUMN delay_seconds integer NOT NULL DEFAULT 0 CHECK (delay_seconds BETWEEN 0 AND 86400);
ALTER TABLE notification_channels ADD CONSTRAINT quiet_hours_pair CHECK (
    (quiet_start_minute IS NULL AND quiet_end_minute IS NULL) OR
    (quiet_start_minute BETWEEN 0 AND 1439 AND quiet_end_minute BETWEEN 0 AND 1439 AND quiet_start_minute <> quiet_end_minute)
);
