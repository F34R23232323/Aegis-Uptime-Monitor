ALTER TABLE notification_channels DROP CONSTRAINT notification_channels_provider_check;
ALTER TABLE notification_channels ADD CONSTRAINT notification_channels_provider_check CHECK (provider IN ('discord','webhook','email'));
