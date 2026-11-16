CREATE TABLE server_metric_samples (
    server_id uuid NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    bucket timestamptz NOT NULL,
    cpu_percent double precision,
    memory_percent double precision,
    disk_percent double precision,
    PRIMARY KEY (server_id, bucket)
);
CREATE INDEX server_metric_samples_time ON server_metric_samples(bucket);
