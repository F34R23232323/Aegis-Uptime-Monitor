ALTER TABLE status_pages ADD COLUMN layout text NOT NULL DEFAULT 'standard'
  CHECK (layout IN ('standard', 'provider'));
ALTER TABLE status_pages ADD COLUMN metrics_settings jsonb NOT NULL DEFAULT
  '{"show_uptime":true,"show_latency":true,"show_on_overview":true,"uptime_days":[1,7,30,90],"latency_samples":120}';

UPDATE status_pages SET layout='provider'
WHERE description LIKE 'Status of selected % components, observed by Aegis from the provider''s public API.';
