CREATE TABLE slo_alerts (
    id uuid PRIMARY KEY,
    monitor_id uuid NOT NULL REFERENCES monitors(id) ON DELETE CASCADE,
    alert_date date NOT NULL DEFAULT ((now() AT TIME ZONE 'UTC')::date),
    budget_consumed_percent double precision NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(monitor_id, alert_date)
);
