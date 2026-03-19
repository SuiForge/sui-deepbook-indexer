# Usage Guide

This guide shows how to run the DeepBook Data Service with the current Remote Store-first indexer setup.

## Prerequisites
- Docker + Docker Compose (recommended), or
- Go 1.24+ (API local run)
- Rust 1.75+ (Indexer local run)
- PostgreSQL 16+ (if not using Docker)

## Quick Start (Docker)

```bash
# From repo root
docker compose -f docker/docker-compose.yml up -d --build

# API available at http://localhost:8080
# Postgres at localhost:5432 (user: sui, pass: sui, db: deepbook_indexer)
```

Docker defaults:
- API listens on `0.0.0.0:8080`
- Indexer uses `DEEPBOOK_ENV=testnet`
- Data source is the Sui Remote Store checkpoint stream
- Indexer startup also seeds minimal metadata scaffolding (`asset_metadata`, `pool_metadata`)

## Configuration

### Indexer

Minimal indexer configuration:

```dotenv
DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
DEEPBOOK_ENV=testnet
INDEXER_POLL_INTERVAL_MS=1000
INDEXER_REQUEST_TIMEOUT_MS=30000
INDEXER_BACKOFF_BASE_MS=100
INDEXER_BACKOFF_MAX_MS=30000
RUST_LOG=info
```

Notes:
- `DEEPBOOK_ENV` accepts `testnet` or `mainnet`
- The current indexer uses `DEEPBOOK_ENV` to select both:
  - the Remote Store URL
  - the built-in DeepBook package list for that network
- current write-path also persists:
  - `checkpoint_ts`
  - `event_ts`
  - `package_id` / `module` / `event_name`
  - `raw_event`
- You no longer need to set `RPC_API_URL`, `DEEPBOOK_PACKAGE_ID`, or `DEEPBOOK_EVENT_TYPE`

### API

```dotenv
DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
API_LISTEN_ADDR=127.0.0.1:8080
LOG_LEVEL=info
WS_PING_INTERVAL_SEC=15
API_SINGLE_KEY=
```

## Run Locally (no Docker)

```bash
# Start Postgres yourself, then apply all migrations
for f in migrations/*.sql; do
  psql "postgresql://sui:sui@localhost:5432/deepbook_indexer" -f "$f"
done

# Run API
cd api-go
export DATABASE_URL="postgresql://sui:sui@localhost:5432/deepbook_indexer"
export API_LISTEN_ADDR="127.0.0.1:8080"
go run cmd/main.go

# Run Indexer (choose network)
cd ../indexer
export DATABASE_URL="postgresql://sui:sui@localhost:5432/deepbook_indexer"
export DEEPBOOK_ENV="testnet"
export INDEXER_POLL_INTERVAL_MS="1000"
export INDEXER_REQUEST_TIMEOUT_MS="30000"
cargo run --package deepbook-indexer-indexer --bin deepbook-indexer-indexer -- run
```

To switch to Mainnet:

```bash
export DEEPBOOK_ENV="mainnet"
```

## API Endpoints

- `GET /health`
- `GET /v1/deepbook/status`
- `GET /v1/deepbook/assets`
- `GET /v1/deepbook/markets/top?window=1h|24h|7d&sort=volume_quote|trades&limit=1..100`
- `GET /v1/deepbook/pools`
- `GET /v1/deepbook/pools/:pool_id/metadata`
- `GET /v1/deepbook/pools/:pool_id/metrics?window=1h|24h`
- `GET /v1/deepbook/pools/:pool_id/candles?window=1h|24h|7d&interval=1m|5m|15m|1h`
- `GET /v1/deepbook/pools/:pool_id/execution/summary?window=1h|24h|7d`
- `GET /v1/deepbook/pools/:pool_id/execution/lifecycle?...`
- `GET /v1/deepbook/pools/:pool_id/execution/fills?...`
- `GET /v1/deepbook/bm/:bm_id/volume?window=24h|7d`
- `WS /v1/deepbook/trades?pool={pool_id}`

See architecture details in `docs/ARCHITECTURE.md`.
Field semantics: `docs/DATA_CONTRACT.md`.

Current compatibility notes:
- `/status` currently exposes DB-observed processed checkpoint and table counts; remote latest-checkpoint lag is not yet surfaced
- `/assets`, `/pools`, `/pools/:pool_id/metadata` expose the current seeded metadata catalog for builder discovery; coverage is intentionally partial during rollout
- `/markets/top` is backed by `pool_metrics_1m` and joins seeded pool metadata when available
- `execution/fills` and `execution/lifecycle` return `ts_ms` from `COALESCE(event_ts, checkpoint_ts, ts)`
- `execution/summary` now uses `COALESCE(event_ts, checkpoint_ts, ts)` for window filtering and first/last price semantics
- WebSocket trade events now emit `ts_ms` from `COALESCE(event_ts, checkpoint_ts, ts)`, while live delivery still follows checkpoint order

## Builder API Quick Checks

```bash
curl "http://localhost:8080/v1/deepbook/status"
curl "http://localhost:8080/v1/deepbook/assets"
curl "http://localhost:8080/v1/deepbook/markets/top?window=24h&sort=volume_quote&limit=20"
curl "http://localhost:8080/v1/deepbook/pools"
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/metadata"
```
