ALTER TABLE notification_channels ADD COLUMN reminder_seconds integer NOT NULL DEFAULT 0
    CHECK (reminder_seconds = 0 OR reminder_seconds BETWEEN 300 AND 86400);
ALTER TABLE notification_deliveries ADD COLUMN reminder_slot integer NOT NULL DEFAULT 0
    CHECK (reminder_slot >= 0);
ALTER TABLE notification_deliveries DROP CONSTRAINT notification_deliveries_channel_id_incident_id_event_key;
ALTER TABLE notification_deliveries ADD CONSTRAINT notification_deliveries_channel_incident_event_slot_key
    UNIQUE (channel_id, incident_id, event, reminder_slot);
