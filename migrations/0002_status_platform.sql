-- Public presentation is separate from raw monitoring evidence.
ALTER TABLE notification_channels ADD COLUMN template jsonb NOT NULL DEFAULT '{}';
ALTER TABLE notification_channels ADD COLUMN events text[] NOT NULL DEFAULT ARRAY['DOWN','RECOVERED','DEGRADED'];
ALTER TABLE notification_channels ADD COLUMN monitor_ids uuid[] NOT NULL DEFAULT '{}';
ALTER TABLE notification_channels ADD COLUMN cooldown_seconds integer NOT NULL DEFAULT 0 CHECK(cooldown_seconds BETWEEN 0 AND 86400);
ALTER TABLE notification_channels ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE monitors ADD COLUMN secret_config text;
ALTER TABLE status_pages ADD COLUMN resource_ids uuid[] NOT NULL DEFAULT '{}';
ALTER TABLE status_pages ADD COLUMN tabs jsonb NOT NULL DEFAULT '[{"id":"overview","label":"Overview","kind":"overview"},{"id":"incidents","label":"Incidents","kind":"incidents"},{"id":"maintenance","label":"Maintenance","kind":"maintenance"},{"id":"metrics","label":"Metrics","kind":"metrics"}]';
ALTER TABLE status_pages ADD COLUMN theme text NOT NULL DEFAULT 'dark' CHECK(theme IN ('dark','light','auto'));
ALTER TABLE status_pages ADD COLUMN logo_url text NOT NULL DEFAULT '';
ALTER TABLE status_pages ADD COLUMN support_url text NOT NULL DEFAULT '';
ALTER TABLE status_pages ADD COLUMN announcement text NOT NULL DEFAULT '';
ALTER TABLE status_pages ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();
CREATE TABLE status_resources (
 id uuid PRIMARY KEY, name text NOT NULL, description text NOT NULL DEFAULT '', group_name text NOT NULL DEFAULT '',
 monitor_ids uuid[] NOT NULL DEFAULT '{}', public boolean NOT NULL DEFAULT false,
 override_state text CHECK(override_state IN ('operational','degraded','partial_outage','major_outage','maintenance','unknown')),
 override_reason text NOT NULL DEFAULT '', override_until timestamptz,
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE resource_status_events (
 id uuid PRIMARY KEY, resource_id uuid NOT NULL REFERENCES status_resources ON DELETE CASCADE,
 state text NOT NULL, reason text NOT NULL DEFAULT '', actor_id uuid REFERENCES users ON DELETE SET NULL,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX resource_status_events_time ON resource_status_events(resource_id,created_at DESC);
ALTER TABLE incidents ALTER COLUMN monitor_id DROP NOT NULL;
ALTER TABLE incidents ADD COLUMN title text NOT NULL DEFAULT '';
ALTER TABLE incidents ADD COLUMN severity text NOT NULL DEFAULT 'major' CHECK(severity IN ('minor','major','critical'));
ALTER TABLE incidents ADD COLUMN phase text NOT NULL DEFAULT 'investigating' CHECK(phase IN ('investigating','identified','monitoring','resolved'));
ALTER TABLE incidents ADD COLUMN public_message text NOT NULL DEFAULT '';
ALTER TABLE incidents ADD COLUMN published boolean NOT NULL DEFAULT false;
ALTER TABLE incidents ADD COLUMN resource_ids uuid[] NOT NULL DEFAULT '{}';
ALTER TABLE incidents ADD COLUMN status_page_ids uuid[] NOT NULL DEFAULT '{}';
ALTER TABLE incidents ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();
UPDATE incidents SET phase='resolved' WHERE ended_at IS NOT NULL;
CREATE TABLE incident_updates (
 id uuid PRIMARY KEY, incident_id uuid NOT NULL REFERENCES incidents ON DELETE CASCADE,
 phase text NOT NULL CHECK(phase IN ('investigating','identified','monitoring','resolved')),
 message text NOT NULL, actor_id uuid REFERENCES users ON DELETE SET NULL,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX incident_updates_timeline ON incident_updates(incident_id,created_at);
ALTER TABLE maintenance_windows ADD COLUMN title text NOT NULL DEFAULT '';
ALTER TABLE maintenance_windows ADD COLUMN public_message text NOT NULL DEFAULT '';
ALTER TABLE maintenance_windows ADD COLUMN status_page_ids uuid[] NOT NULL DEFAULT '{}';
CREATE TABLE admin_audit_events (
 id bigserial PRIMARY KEY, actor_id uuid REFERENCES users ON DELETE SET NULL,
 action text NOT NULL, target_id uuid, details jsonb NOT NULL DEFAULT '{}', created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX admin_audit_events_time ON admin_audit_events(created_at DESC);
ALTER TABLE sessions ADD COLUMN id uuid NOT NULL DEFAULT gen_random_uuid();
CREATE UNIQUE INDEX sessions_id ON sessions(id);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE INDEX public_incidents_pages ON incidents USING gin(status_page_ids) WHERE published;
ALTER TABLE maintenance_windows ADD COLUMN resource_ids uuid[] NOT NULL DEFAULT '{}';

UPDATE incidents i SET title=m.name FROM monitors m WHERE i.monitor_id=m.id AND i.title='';
