# DeepBook Data Service

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Self-hosted data backend for DeepBook v3 on Sui blockchain.

> 中文文档：见 [README.zh-CN.md](README.zh-CN.md)

## Overview

Checkpoint-driven indexer that extracts DeepBook trade events (`OrderFilled`), computes 1-minute rollups (including OHLC), and serves data via REST/WebSocket APIs.

**Features:**
- ✅ DeepBook trade fact storage (`db_events`)
- ✅ 1-minute rollup metrics (pool & BalanceManager dimensions)
- ✅ Idempotent ingestion with replay capability
- ✅ RPC failover support
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

## API Usage

```bash
# Pool metrics (1h window)
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/metrics?window=1h"

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
- **[docs/DATA_CONTRACT.md](docs/DATA_CONTRACT.md)** - Schema & field semantics (v1)
- **[docs/DEEPBOOK_EVENTS.md](docs/DEEPBOOK_EVENTS.md)** - DeepBook v3 event list (from Move sources)

## Architecture

```
Sui Blockchain (Mainnet/Testnet)
        ↓
   Indexer (Rust)
   - Checkpoint-driven ingestion
   - DeepBook event filtering
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
- (Optional) Go 1.21+ for local API development
- (Optional) Rust 1.75+ for local indexer development

## Configuration

Default configuration connects to **Sui Testnet**. To customize:

Edit `docker/docker-compose.yml`:
```yaml
environment:
  RPC_API_URL: https://fullnode.testnet.sui.io:443
  # DeepBook package id(s). You can pass multiple IDs (comma/space-separated).
  DEEPBOOK_PACKAGE_ID: "0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc"  # Testnet DeepBookV3
```

For Mainnet, change to:
```yaml
  RPC_API_URL: https://fullnode.mainnet.sui.io:443
  DEEPBOOK_PACKAGE_ID: "0x00c1a56ec8c4c623a848b2ed2f03d23a25d17570b670c22106f336eb933785cc"  # Mainnet DeepBookV3
```

## Replay & Data Correction

See docs/USAGE.md for minimal commands. Advanced replay instructions can be added as needed.

## Development

**Indexer:**
```bash
cd indexer
export DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
export RPC_API_URL=https://fullnode.mainnet.sui.io:443
export DEEPBOOK_PACKAGE_ID=0x...dee9 # or multiple IDs: "0xabc...,0xdef..."
cargo run -- run
```

**API:**
```bash
cd api-go
export DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
go run cmd/api/main.go
```

## License

[Apache-2.0](LICENSE)

## Support

- **Issues**: [GitHub Issues](../../issues)
- **Documentation**: [docs/](docs/)
