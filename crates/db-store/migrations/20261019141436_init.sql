-- Add migration script here
-- Ensure UUID extension is available for UUID generation
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE outbox_events (
    id UUID PRIMARY KEY,
    destination_url TEXT NOT NULL,
    payload JSONB NOT NULL,
    secret_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    attempt_count INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    next_retry_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_outbox_queue ON outbox_events (next_retry_at ASC)
WHERE status = 'PENDING';

CREATE TABLE delivery_attempts (
    id UUID PRIMARY KEY,
    event_id UUID NOT NULL REFERENCES outbox_events(id) ON DELETE CASCADE,
    http_status INT,
    response_body TEXT,
    latency_ms INT NOT NULL,
    error_message TEXT,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION notify_delivery_attempt()
RETURNS TRIGGER AS $$
DECLARE
    target_url TEXT;
BEGIN
    SELECT destination_url INTO target_url FROM outbox_events WHERE id = NEW.event_id;
    
    PERFORM pg_notify(
        'webhook_stream',
        json_build_object(
            'attempt_id', NEW.id,
            'event_id', NEW.event_id,
            'http_status', NEW.http_status,
            'latency_ms', NEW.latency_ms,
            'destination_url', target_url,
            'attempted_at', NEW.attempted_at
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_delivery_attempt
AFTER INSERT ON delivery_attempts
FOR EACH ROW EXECUTE FUNCTION notify_delivery_attempt();