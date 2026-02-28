-- Add DeepBook order lifecycle event table for non-fill events.

CREATE TABLE IF NOT EXISTS db_order_events (
    checkpoint BIGINT NOT NULL,
    ts TIMESTAMPTZ NOT NULL,
    pool_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    order_id TEXT,
    trader TEXT,
    is_bid BOOLEAN,
    price NUMERIC(38, 18),
    original_quantity NUMERIC(38, 18),
    new_quantity NUMERIC(38, 18),
    canceled_quantity NUMERIC(38, 18),
    tx_digest TEXT NOT NULL,
    event_seq INT NOT NULL,
    event_index INT,
    raw_event JSONB,
    PRIMARY KEY (tx_digest, event_seq)
);

CREATE INDEX IF NOT EXISTS idx_db_order_events_pool_ts ON db_order_events (pool_id, ts DESC);
CREATE INDEX IF NOT EXISTS idx_db_order_events_type_ts ON db_order_events (event_type, ts DESC);
