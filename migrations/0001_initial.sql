CREATE TABLE users (id uuid PRIMARY KEY, email text UNIQUE NOT NULL, password_hash text NOT NULL, role text NOT NULL CHECK(role IN ('owner','admin','viewer')), created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE sessions (token_hash text PRIMARY KEY, user_id uuid NOT NULL REFERENCES users ON DELETE CASCADE, expires_at timestamptz NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE auth_audit_events (id bigserial PRIMARY KEY, event text NOT NULL, user_id uuid, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE login_attempts (key text PRIMARY KEY, attempts integer NOT NULL DEFAULT 0, window_start timestamptz NOT NULL DEFAULT now());
CREATE TABLE servers (id uuid PRIMARY KEY, name text NOT NULL, token_hash text UNIQUE NOT NULL, last_seen timestamptz, version text, inventory jsonb NOT NULL DEFAULT '{}', created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE monitors (
 id uuid PRIMARY KEY, name text NOT NULL, kind text NOT NULL, target text NOT NULL,
 interval_seconds integer NOT NULL, timeout_seconds integer NOT NULL, failure_threshold integer NOT NULL, recovery_threshold integer NOT NULL,
 group_name text NOT NULL DEFAULT '', description text NOT NULL DEFAULT '', tags jsonb NOT NULL DEFAULT '[]', public boolean NOT NULL DEFAULT false,
 server_id uuid REFERENCES servers ON DELETE RESTRICT, config jsonb NOT NULL DEFAULT '{}', enabled boolean NOT NULL DEFAULT true,
 state text NOT NULL DEFAULT 'PENDING', failures integer NOT NULL DEFAULT 0, successes integer NOT NULL DEFAULT 0,
 next_check_at timestamptz NOT NULL DEFAULT now(), last_checked_at timestamptz, last_latency double precision, last_error text,
 push_token_hash text UNIQUE, last_push timestamptz, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX monitors_due ON monitors(next_check_at) WHERE enabled;
CREATE TABLE checks (id bigserial PRIMARY KEY, monitor_id uuid NOT NULL REFERENCES monitors ON DELETE CASCADE, ok boolean NOT NULL, state text NOT NULL, latency_ms double precision NOT NULL, error text, observed jsonb NOT NULL DEFAULT '{}', created_at timestamptz NOT NULL DEFAULT now());
CREATE INDEX checks_monitor_time ON checks(monitor_id,created_at DESC);
CREATE TABLE incidents (id uuid PRIMARY KEY, monitor_id uuid NOT NULL REFERENCES monitors ON DELETE CASCADE, started_at timestamptz NOT NULL DEFAULT now(), ended_at timestamptz, cause text, note text NOT NULL DEFAULT '');
CREATE UNIQUE INDEX one_open_incident ON incidents(monitor_id) WHERE ended_at IS NULL;
CREATE TABLE incident_events (id bigserial PRIMARY KEY, incident_id uuid NOT NULL REFERENCES incidents ON DELETE CASCADE, message text NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE notification_channels (id uuid PRIMARY KEY, name text NOT NULL, provider text NOT NULL CHECK(provider IN ('discord','webhook','telegram')), secret text NOT NULL, enabled boolean NOT NULL DEFAULT true, created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE notification_deliveries (id uuid PRIMARY KEY, channel_id uuid REFERENCES notification_channels ON DELETE SET NULL, incident_id uuid REFERENCES incidents ON DELETE SET NULL, event text NOT NULL, payload jsonb NOT NULL, state text NOT NULL DEFAULT 'pending', attempts integer NOT NULL DEFAULT 0, due_at timestamptz NOT NULL DEFAULT now(), last_error text, created_at timestamptz NOT NULL DEFAULT now(), UNIQUE(channel_id,incident_id,event));
CREATE INDEX delivery_due ON notification_deliveries(due_at) WHERE state='pending';
CREATE TABLE maintenance_windows (id uuid PRIMARY KEY, reason text NOT NULL, starts_at timestamptz NOT NULL, ends_at timestamptz NOT NULL, monitor_ids uuid[] NOT NULL, CHECK(ends_at>starts_at));
CREATE TABLE status_pages (id uuid PRIMARY KEY, slug text UNIQUE NOT NULL, title text NOT NULL, description text NOT NULL DEFAULT '', accent text NOT NULL DEFAULT '#9a85ff', monitor_ids uuid[] NOT NULL DEFAULT '{}');
CREATE TABLE agent_submissions (server_id uuid NOT NULL REFERENCES servers ON DELETE CASCADE, submission_id uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), PRIMARY KEY(server_id,submission_id));
