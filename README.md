# DeepBook Data Service

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Self-hosted data backend for DeepBook v3 on Sui blockchain.

> 中文文档：见 [README.zh-CN.md](README.zh-CN.md)

## Overview

Checkpoint-driven indexer that reads Sui checkpoint data from Remote Store, extracts DeepBook execution data (fills + order lifecycle), computes 1-minute rollups (including OHLC), and serves data via REST/WebSocket APIs.

**Features:**
- ✅ DeepBook trade fact storage (`db_events`)
- ✅ Dual timestamp + raw event persistence (`checkpoint_ts`, `event_ts`, `raw_event`)
- ✅ 1-minute rollup metrics (pool & BalanceManager dimensions)
- ✅ Idempotent ingestion with replay capability
- ✅ Metadata scaffolding (`asset_metadata`, `pool_metadata`) with startup seed
- ✅ Builder-facing metadata discovery APIs (`/assets`, `/pools`, `/pools/:pool_id/metadata`)
- ✅ Market discovery + service status APIs (`/markets/top`, `/status`)
- ✅ Remote Store-first ingestion with environment-based package selection
- ✅ REST API + WebSocket streaming
- ✅ Docker Compose one-click deployment

## Quick Start

```bash
docker compose -f docker/docker-compose.yml up -d --build
```

Service will automatically:
1. Start PostgreSQL database
2. Run schema migrations
3. Index DeepBook trades (default: Testnet in `docker/docker-compose.yml`)
4. Serve API on http://localhost:8080

Current v2 foundation notes:
- `execution/fills`, `execution/lifecycle`, `execution/summary`, and WS trade `ts_ms` now prefer `COALESCE(event_ts, checkpoint_ts, ts)`
- new indexer writes persist `checkpoint_ts`, `event_ts`, `package_id`, `module`, `event_name`, and `raw_event`
- every indexer startup seeds minimal metadata scaffolding for `asset_metadata` / `pool_metadata`
- metadata REST endpoints expose the currently seeded/scaffolded asset + pool catalog for builder discovery
- `/v1/deepbook/status` now enriches DB-observed service state with Remote Store latest checkpoint / lag when source probing succeeds

## API Usage

```bash
# Service status
curl "http://localhost:8080/v1/deepbook/status"

# Asset catalog
curl "http://localhost:8080/v1/deepbook/assets"

# Top markets (default: 24h by quote volume)
curl "http://localhost:8080/v1/deepbook/markets/top"

# Pool catalog
curl "http://localhost:8080/v1/deepbook/pools"

# Pool metadata detail
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/metadata"

# Pool metrics (1h window)
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/metrics?window=1h"

# Execution summary (1h/24h/7d)
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/execution/summary?window=24h"

# Order lifecycle stream snapshot (placed/canceled/modified, cursor pagination)
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/execution/lifecycle?window=24h&event_type=order_canceled&limit=200&cursor=<ts_ms|checkpoint|event_seq>"

# Execution fills (cursor pagination)
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/execution/fills?window=24h&limit=200&cursor=<ts_ms|checkpoint|event_seq>"

# OHLCV candles (24h window, 1m interval)
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/candles?window=24h&interval=1m"

# BalanceManager volume (24h window)
curl "http://localhost:8080/v1/deepbook/bm/{bm_id}/volume?window=24h"

# WebSocket trade stream
wscat -c "ws://localhost:8080/v1/deepbook/trades?pool={pool_id}"
```

WebSocket with auth (if enabled):

```bash
wscat -H "Authorization: Bearer <API_SINGLE_KEY>" -c "ws://localhost:8080/v1/deepbook/trades?pool={pool_id}"
```

### Parameters

- **status semantics**: `/v1/deepbook/status` now returns DB counts plus `latest_checkpoint`, `checkpoint_lag`, `source_status`, and `source_url` when the Remote Store probe succeeds.
- **source probe config**: `DEEPBOOK_ENV=testnet|mainnet`, `SOURCE_STATUS_TIMEOUT_MS` (default `5000`), `SOURCE_STATUS_CACHE_SEC` (default `15`).
- **metadata coverage**: current `/assets` + `/pools` responses reflect the repo-local seeded catalog, not full chain-wide discovery.
- **top markets**: `window=1h|24h|7d`, `sort=volume_quote|trades`, `limit=1..100` (default `20`).
- **window (pool metrics)**: allowed `1h`, `24h`; default `1h`.
- **window (pool candles)**: allowed `1h`, `24h`, `7d`; default `1h`.
- **interval (pool candles)**: allowed `1m`, `5m`, `15m`, `1h`; default `1m`.
- **window (BM volume)**: allowed `24h`, `7d`; default `24h`.
- **pool filter**: optional, comma-separated pool IDs. Supported by BM volume (`?pool=POOL1,POOL2`) and WebSocket trades (`?pool=POOL1,POOL2`).
- **auth (optional)**: when enabled, send `Authorization: Bearer <API_SINGLE_KEY>`. Errors return `{ "error": "unauthorized" }` with HTTP 401.

### Behavior

- Invalid `window` values fall back to the default (no error returned).

## Response Examples

Pool metrics (1h):

```json
{
     "pool_id": "0xPOOL...",
     "window": "1h",
     "start_ts": "2025-12-25T09:00:00Z",
     "end_ts": "2025-12-25T10:00:00Z",
     "trades": 1234,
     "volume_base": "456.789",
     "volume_quote": "98765.4321",
     "maker_volume": "200.000",
     "taker_volume": "256.789",
     "vwap": "215.4321",
     "last_price": "217.00"
}
```

BalanceManager volume (24h):

```json
{
     "bm_id": "0xBM...",
     "window": "24h",
     "start_ts": "2025-12-24T10:00:00Z",
     "end_ts": "2025-12-25T10:00:00Z",
     "total_volume_quote": "123456.789",
     "breakdown": [
          { "pool_id": "0xPOOL1", "volume_quote": "50000.000", "trades": 321 },
          { "pool_id": "0xPOOL2", "volume_quote": "73456.789", "trades": 456 }
     ]
}
```

WebSocket trade event:

```json
{
     "type": "trade",
     "ts_ms": 1766640000000,
     "pool_id": "0xPOOL...",
     "side": "buy",
     "price": "215.43",
     "base_sz": "1.2345",
     "quote_sz": "265.89",
     "maker_bm": "0xMAKERBM...",
     "taker_bm": "0xTAKERBM...",
     "tx_digest": "5Vjk...",
     "event_seq": 42,
     "checkpoint": 1234567
}
```

## Documentation

- **[docs/README.md](docs/README.md)** - Documentation index
- **[docs/USAGE.md](docs/USAGE.md)** - Minimal usage guide
- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** - System architecture
- **[docs/DATA_CONTRACT.md](docs/DATA_CONTRACT.md)** - v2 target contract + current compatibility notes
- **[docs/DEEPBOOK_EVENTS.md](docs/DEEPBOOK_EVENTS.md)** - DeepBook v3 event list (from Move sources)
- **[docs/plans/2026-03-19-v2-foundation-implementation.md](docs/plans/2026-03-19-v2-foundation-implementation.md)** - Foundation rollout implementation plan

## Architecture

```
Sui Blockchain (Mainnet/Testnet)
        ↓
   Indexer (Rust)
   - Checkpoint-driven ingestion
   - Remote Store checkpoint ingestion
   - Environment-based package selection
   - 1-minute rollup computation
        ↓
   PostgreSQL
   - db_events (trade facts)
   - pool_metrics_1m
   - bm_metrics_1m
        ↓
   API Server (Go)
   - REST endpoints
   - WebSocket streaming
```

## Project Structure

```
├── api-go/          # Go API server
├── indexer/         # Rust indexer
├── storage/         # Database models & queries
├── common/          # Shared configuration
├── migrations/      # PostgreSQL migrations
├── docker/          # Docker configurations
└── docs/            # Documentation
```

## Requirements

- Docker & Docker Compose
- (Optional) Go 1.24+ for local API development
- (Optional) Rust 1.75+ for local indexer development

## Configuration

Default configuration connects to **Sui Testnet**. To customize the indexer, set:

```yaml
environment:
  DEEPBOOK_ENV: testnet
  INDEXER_POLL_INTERVAL_MS: "1000"
  INDEXER_REQUEST_TIMEOUT_MS: "30000"
  INDEXER_BACKOFF_BASE_MS: "100"
  INDEXER_BACKOFF_MAX_MS: "30000"
```

For Mainnet, change to:

```yaml
environment:
  DEEPBOOK_ENV: mainnet
```

`DEEPBOOK_ENV` controls both the Remote Store URL and the built-in DeepBook package list for that network.

## Replay & Data Correction

See docs/USAGE.md for minimal commands. Advanced replay instructions can be added as needed.

## Development

**Indexer:**
```bash
cd indexer
export DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
export DEEPBOOK_ENV=mainnet
export INDEXER_POLL_INTERVAL_MS=1000
export INDEXER_REQUEST_TIMEOUT_MS=30000
cargo run -- run
```

**API:**
```bash
cd api-go
export DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
go run cmd/main.go
```

## License

[Apache-2.0](LICENSE)

## Support

- **Issues**: [GitHub Issues](../../issues)
- **Documentation**: [docs/](docs/)
