-- DeepBook Data Service - PostgreSQL Schema (All Migrations Combined)
-- Created: 2025-12-24
-- 
-- This single migration file includes all schema setup for DeepBook Data Service Core.
-- Includes: indexer state, event tracking, and DeepBook-specific data models.

-- ============================================================================
-- Phase 1: Core Indexer State & Tracking Tables
-- ============================================================================

-- Track indexer progress (which checkpoint has been processed)
CREATE TABLE IF NOT EXISTS indexer_state (
  id SMALLINT PRIMARY KEY,
  processed_checkpoint BIGINT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Initialize indexer state
INSERT INTO indexer_state (id, processed_checkpoint)
VALUES (1, 0)
ON CONFLICT (id) DO NOTHING;

-- Track processed checkpoints
CREATE TABLE IF NOT EXISTS checkpoints (
  checkpoint BIGINT PRIMARY KEY,
  epoch BIGINT NOT NULL,
  timestamp_ms BIGINT NOT NULL,
  processed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================================
-- Phase 2: Generic Transaction & Event Tables (for future extensibility)
-- ============================================================================

-- All transactions on Sui (not just DeepBook)
CREATE TABLE IF NOT EXISTS transactions (
  digest TEXT PRIMARY KEY,
  checkpoint BIGINT NOT NULL,
  timestamp_ms BIGINT NOT NULL,
  sender TEXT NOT NULL,
  raw JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tx_sender ON transactions(sender);
CREATE INDEX IF NOT EXISTS idx_tx_checkpoint ON transactions(checkpoint);

-- Address-transaction mapping for faster lookups
CREATE TABLE IF NOT EXISTS address_transactions (
  address TEXT NOT NULL,
  digest TEXT NOT NULL,
  checkpoint BIGINT NOT NULL,
  timestamp_ms BIGINT NOT NULL,
  PRIMARY KEY (address, digest)
);

CREATE INDEX IF NOT EXISTS idx_addr_tx_checkpoint
ON address_transactions(address, checkpoint DESC);

-- Generic events (will be deprecated in favor of db_events)
CREATE TABLE IF NOT EXISTS events (
  id BIGSERIAL PRIMARY KEY,
  digest TEXT NOT NULL,
  checkpoint BIGINT NOT NULL,
  timestamp_ms BIGINT NOT NULL,
  sender TEXT,
  event_type TEXT NOT NULL,
  raw JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_event_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_event_checkpoint ON events(checkpoint);
CREATE INDEX IF NOT EXISTS idx_event_sender ON events(sender);

-- Current object state
CREATE TABLE IF NOT EXISTS objects (
  object_id TEXT PRIMARY KEY,
  owner TEXT,
  object_type TEXT,
  version BIGINT,
  raw JSONB NOT NULL,
  updated_checkpoint BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_objects_owner ON objects(owner);
CREATE INDEX IF NOT EXISTS idx_objects_updated_checkpoint ON objects(updated_checkpoint);

-- ============================================================================
-- Phase 3: Event Idempotency & Deduplication
-- ============================================================================

-- Add stable per-transaction event sequence for deduplication
-- This allows us to safely replay events without duplication
ALTER TABLE events
ADD COLUMN IF NOT EXISTS event_seq BIGINT;

-- Backfill event_seq for existing events
WITH ranked AS (
  SELECT id, (ROW_NUMBER() OVER (PARTITION BY digest ORDER BY id) - 1) AS seq
  FROM events
  WHERE event_seq IS NULL
)
UPDATE events e
SET event_seq = r.seq
FROM ranked r
WHERE e.id = r.id;

-- Make event_seq NOT NULL (only after backfill)
ALTER TABLE events
ALTER COLUMN event_seq SET NOT NULL;

-- Ensure deduplication via (digest, event_seq)
CREATE UNIQUE INDEX IF NOT EXISTS uq_events_digest_event_seq
ON events (digest, event_seq);

-- Fix initial checkpoint offset for clean databases
-- A fresh DB starts with processed_checkpoint = 0, which makes first run start at checkpoint 1
-- We fix it to -1 so the next checkpoint becomes 0
UPDATE indexer_state
SET processed_checkpoint = -1
WHERE id = 1
  AND processed_checkpoint = 0
  AND NOT EXISTS (SELECT 1 FROM checkpoints);

-- ============================================================================
-- Phase 4: DeepBook-Specific Schema (Core v1)
-- ============================================================================

-- DeepBook trade facts: individual events with extracted fields
-- Primary table for DeepBook Core v1
CREATE TABLE IF NOT EXISTS db_events (
    checkpoint      BIGINT      NOT NULL,
    ts              TIMESTAMPTZ NOT NULL,
    pool_id         TEXT        NOT NULL,
    side            TEXT        NOT NULL CHECK (side IN ('buy', 'sell')),
    price           NUMERIC(38, 18) NOT NULL,
    base_sz         NUMERIC(38, 18) NOT NULL,
    quote_sz        NUMERIC(38, 18) NOT NULL,
    maker_bm        TEXT,
    taker_bm        TEXT,
    tx_digest       TEXT        NOT NULL,
    event_seq       INT         NOT NULL,
    event_index     INT,
    raw_event       JSONB,
    PRIMARY KEY (tx_digest, event_seq)
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_db_events_pool_ts ON db_events (pool_id, ts);
CREATE INDEX IF NOT EXISTS idx_db_events_bm_ts ON db_events (maker_bm, ts) WHERE maker_bm IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_db_events_checkpoint ON db_events (checkpoint);

-- Pool-level 1-minute aggregated metrics
-- Used for /v1/deepbook/pools/{pool_id}/metrics endpoint
CREATE TABLE IF NOT EXISTS pool_metrics_1m (
    pool_id         TEXT        NOT NULL,
    bucket_start    TIMESTAMPTZ NOT NULL,
    trades          BIGINT      NOT NULL,
    volume_base     NUMERIC(38, 18) NOT NULL,
    volume_quote    NUMERIC(38, 18) NOT NULL,
    maker_volume    NUMERIC(38, 18) NOT NULL,
    taker_volume    NUMERIC(38, 18) NOT NULL,
    fees_quote      NUMERIC(38, 18),
    avg_price       NUMERIC(38, 18),
    vwap            NUMERIC(38, 18),
    last_price      NUMERIC(38, 18),
    PRIMARY KEY (pool_id, bucket_start)
);

CREATE INDEX IF NOT EXISTS idx_pool_metrics_bucket ON pool_metrics_1m (bucket_start);

-- Balance Manager 1-minute aggregated metrics
-- Used for /v1/deepbook/bm/{bm_id}/volume endpoint
-- Each BalanceManager's contribution to a pool per minute
CREATE TABLE IF NOT EXISTS bm_metrics_1m (
    bm_id           TEXT        NOT NULL,
    pool_id         TEXT        NOT NULL,
    bucket_start    TIMESTAMPTZ NOT NULL,
    trades          BIGINT      NOT NULL,
    volume_quote    NUMERIC(38, 18) NOT NULL,
    maker_volume    NUMERIC(38, 18) NOT NULL,
    taker_volume    NUMERIC(38, 18) NOT NULL,
    PRIMARY KEY (bm_id, pool_id, bucket_start)
);

CREATE INDEX IF NOT EXISTS idx_bm_metrics_bucket ON bm_metrics_1m (bucket_start);
CREATE INDEX IF NOT EXISTS idx_bm_metrics_bm_bucket ON bm_metrics_1m (bm_id, bucket_start);

-- ============================================================================
-- End of Schema
-- ============================================================================
-- 
-- Summary of tables:
-- - indexer_state: Tracks which checkpoint has been processed
-- - db_events: DeepBook trade facts (main table for Core v1)
-- - pool_metrics_1m: Pool-level 1-minute metrics
-- - bm_metrics_1m: BalanceManager-level 1-minute metrics
-- 
-- Legacy tables (for future extensibility):
-- - checkpoints, transactions, address_transactions, events, objects
