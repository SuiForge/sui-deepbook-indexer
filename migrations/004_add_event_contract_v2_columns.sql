-- Add v2 data-contract foundation columns for DeepBook fact tables.
-- Keeps legacy `ts` for backward compatibility while introducing explicit timestamp semantics.
-- This migration is intended for the current self-hosted rollout path and assumes a maintenance window for large tables.
-- Zero-downtime large-table rollouts should split schema add, backfill, and concurrent index creation into separate steps.

ALTER TABLE db_events
    ADD COLUMN IF NOT EXISTS checkpoint_ts TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS event_ts TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS ingested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS package_id TEXT,
    ADD COLUMN IF NOT EXISTS module TEXT,
    ADD COLUMN IF NOT EXISTS event_name TEXT;

UPDATE db_events
SET checkpoint_ts = COALESCE(checkpoint_ts, ts),
    event_ts = COALESCE(event_ts, ts)
WHERE checkpoint_ts IS NULL
   OR event_ts IS NULL;

CREATE INDEX IF NOT EXISTS idx_db_events_pool_event_ts
ON db_events (pool_id, event_ts);

ALTER TABLE db_order_events
    ADD COLUMN IF NOT EXISTS checkpoint_ts TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS event_ts TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS ingested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS package_id TEXT,
    ADD COLUMN IF NOT EXISTS module TEXT,
    ADD COLUMN IF NOT EXISTS event_name TEXT;

UPDATE db_order_events
SET checkpoint_ts = COALESCE(checkpoint_ts, ts),
    event_ts = COALESCE(event_ts, ts)
WHERE checkpoint_ts IS NULL
   OR event_ts IS NULL;

CREATE INDEX IF NOT EXISTS idx_db_order_events_pool_event_ts
ON db_order_events (pool_id, event_ts DESC);
