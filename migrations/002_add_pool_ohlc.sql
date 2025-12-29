-- Add OHLC fields to per-minute pool metrics.
-- Idempotent: safe to run multiple times.

ALTER TABLE pool_metrics_1m
    ADD COLUMN IF NOT EXISTS open_price NUMERIC(38, 18),
    ADD COLUMN IF NOT EXISTS high_price NUMERIC(38, 18),
    ADD COLUMN IF NOT EXISTS low_price  NUMERIC(38, 18);

