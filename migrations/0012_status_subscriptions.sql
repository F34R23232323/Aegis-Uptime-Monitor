ALTER TABLE status_pages ADD COLUMN subscriptions_enabled boolean NOT NULL DEFAULT true;

CREATE TABLE status_subscriptions (
    id uuid PRIMARY KEY,
    page_id uuid NOT NULL REFERENCES status_pages(id) ON DELETE CASCADE,
    address_hash text NOT NULL,
    address_cipher text NOT NULL,
    confirm_hash text NOT NULL,
    confirmed boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(page_id, address_hash)
);
CREATE TABLE status_subscription_deliveries (
    id uuid PRIMARY KEY,
    subscription_id uuid NOT NULL REFERENCES status_subscriptions(id) ON DELETE CASCADE,
    incident_id uuid REFERENCES incidents(id) ON DELETE CASCADE,
    payload_cipher text NOT NULL,
    state text NOT NULL DEFAULT 'pending',
    attempts integer NOT NULL DEFAULT 0,
    due_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX status_subscription_delivery_due ON status_subscription_deliveries(due_at) WHERE state='pending';
