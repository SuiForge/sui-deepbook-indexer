# DeepBook Indexer (Rust)

Checkpoint-driven indexer that fetches DeepBook trade events from Sui blockchain, computes 1-minute metrics, and stores data in PostgreSQL.

## Configuration

Required environment variables:

```dotenv
# Database connection
DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer

# Sui RPC endpoint (default: Testnet)
RPC_API_URL=https://fullnode.testnet.sui.io:443
# RPC_API_URL=https://fullnode.mainnet.sui.io:443

# DeepBookV3 package ID (must match RPC network)
# Testnet (default)
DEEPBOOK_PACKAGE_ID=0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc
# Mainnet
# DEEPBOOK_PACKAGE_ID=0x00c1a56ec8c4c623a848b2ed2f03d23a25d17570b670c22106f336eb933785cc

# Optional: tuning parameters
INDEXER_POLL_INTERVAL_MS=1000          # Checkpoint poll interval
INDEXER_RPC_TIMEOUT_MS=10000           # RPC request timeout
RUST_LOG=info                          # Logging level
```

## Local Development

```bash
# Install dependencies
cargo build

# Run indexer
export DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
export RPC_API_URL=https://fullnode.testnet.sui.io:443
export DEEPBOOK_PACKAGE_ID=0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc

cargo run --release -- run
```

## Architecture

1. **Checkpoint Loop**: Polls Sui RPC for latest checkpoints (default interval: 1s)
2. **Event Ingestion**: Filters DeepBook `OrderFilled` events by package ID
3. **Data Storage**: Idempotent upsert into `db_events` (trade facts)
4. **Metric Computation**: Aggregates 1-minute rollup metrics:
   - `pool_metrics_1m`: Pool-level metrics (trades, volumes, VWAP, etc.)
   - `bm_metrics_1m`: BalanceManager-level metrics (volume breakdown)
5. **State Tracking**: Updates `indexer_state` with latest processed checkpoint for replay support

## Key Modules

- `main.rs`: Entry point, config loading, main checkpoint loop
- `queries.rs` (via storage crate): Database queries and upserts
- `models.rs` (via storage crate): Data structures (events, metrics)

## Troubleshooting

- **Missing DATABASE_URL**: Ensure environment variable is set before running
- **RPC timeout**: Increase `INDEXER_RPC_TIMEOUT_MS` if network is slow
- **No data ingested**: Check that package ID matches your RPC network (Mainnet vs Testnet)
- **Stalled indexer**: Verify RPC endpoint is responsive; check logs with `RUST_LOG=debug`

## Build

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

## Docker

Build and run via Docker Compose from repo root:

```bash
docker compose -f docker/docker-compose.yml up -d --build
```

Indexer is built with Rust 1.88+ and runs as a container service.
