# Usage Guide

This guide shows how to run the DeepBook Data Service with a minimal setup.

## Prerequisites
- Docker + Docker Compose (recommended), or
- Go 1.21+ (API local run)
- Rust 1.75+ (Indexer local run)

## Quick Start (Docker)

```powershell
# From repo root
docker compose -f docker/docker-compose.yml up -d --build

# API available at http://localhost:8080
# Postgres at localhost:5432 (user: sui, pass: sui, db: deepbook_indexer)
```

## Configuration

Minimal env variables:

```
DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
# Choose one network
# RPC_API_URL=https://fullnode.mainnet.sui.io:443
RPC_API_URL=https://fullnode.testnet.sui.io:443
# Optional RPC failover
# RPC_API_FALLBACK=https://fullnode.testnet.sui.io:443
# Match network
# Mainnet: 0x00c1a56ec8c4c623a848b2ed2f03d23a25d17570b670c22106f336eb933785cc
# Testnet: 0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc
DEEPBOOK_PACKAGE_ID=... # supports multiple IDs (comma/space-separated)
DEEPBOOK_EVENT_TYPE=OrderFilled

# API optional auth / WS ping
# API_SINGLE_KEY=...
# WS_PING_INTERVAL_SEC=15
```

## Run Locally (no Docker)

```powershell
# Start Postgres yourself, then apply migrations:
psql "postgresql://sui:sui@localhost:5432/deepbook_indexer" -f migrations/001_init.sql
psql "postgresql://sui:sui@localhost:5432/deepbook_indexer" -f migrations/002_add_pool_ohlc.sql

# Run API
$env:DATABASE_URL = "postgresql://sui:sui@localhost:5432/deepbook_indexer"
cd api-go
go run cmd/api/main.go

# Run Indexer (choose network)
cd ../indexer
$env:DATABASE_URL = "postgresql://sui:sui@localhost:5432/deepbook_indexer"
$env:RPC_API_URL = "https://fullnode.testnet.sui.io:443"
$env:DEEPBOOK_PACKAGE_ID = "0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc"
cargo run --package deepbook-indexer-indexer --bin deepbook-indexer-indexer -- run
```

## API Endpoints

- GET /health
- GET /v1/deepbook/pools/:pool_id/metrics?window=1h
- GET /v1/deepbook/pools/:pool_id/candles?window=24h&interval=1m
- GET /v1/deepbook/bm/:bm_id/volume?window=24h
- WS /v1/deepbook/trades?pool={pool_id}

See architecture details in docs/ARCHITECTURE.md.
Field semantics: docs/DATA_CONTRACT.md.
