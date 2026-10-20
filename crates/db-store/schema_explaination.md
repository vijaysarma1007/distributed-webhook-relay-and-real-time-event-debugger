## CREATE EXTENSION IF NOT EXISTS "uuid-ossp"

- Enabling this extension allows to generate UUID's directly insdie raw SQL quieries.

## Auto Trigger when delivery attempt is created

```sql
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
```

- After each delivery attempt is created, PostgreSQL automatically executes _notify_delivery_attempt()_.
- **NEW** : refers to newly inserted _delivery_attempts_ row.
- _SELECT destination_url INTO target_url FROM outbox_events WHERE id = NEW.event_id;_ : Selects the destination_url from outbox event table and places it into newly created varaible 'target_url' where id from outbox event matches with event id in delivery attempt table, this will later be pushed to json object.
- _pg_notify_: packages a light weight json object, whose keys match with **LiveEvenetNotification** Struct from _core_types_ library crate. This respects the PostgreSQL's 8000 byte notify limit and prevents secrets to the debugger ui.
- _stream_bridge_ bin crate listens to _webhook_stream_ via **sqlx::postgres::PgListener**, deserializes this JSON into _LiveEventNotification_ struct and pushes it via server sent events (SSE) straight to debigger ui.
