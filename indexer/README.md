# DeepBook Indexer (Rust)

Checkpoint-driven indexer that reads Sui checkpoint data from Remote Store, extracts DeepBook events, computes 1-minute metrics, and stores data in PostgreSQL.

## Configuration

Required environment variables:

```dotenv
# Database connection
DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer

# Network selection: determines Remote Store URL + supported DeepBook package list
DEEPBOOK_ENV=testnet
# DEEPBOOK_ENV=mainnet

# Optional tuning parameters
INDEXER_POLL_INTERVAL_MS=1000
INDEXER_REQUEST_TIMEOUT_MS=30000
INDEXER_BACKOFF_BASE_MS=100
INDEXER_BACKOFF_MAX_MS=30000

# Optional checkpoint control
# INDEXER_START_CHECKPOINT=12345
# INDEXER_STOP_CHECKPOINT=67890

RUST_LOG=info
```

Notes:
- The current indexer uses Remote Store by default rather than direct RPC checkpoint polling
- The DeepBook package list is built into the code for each environment
- You no longer need to set `RPC_API_URL` or `DEEPBOOK_PACKAGE_ID`

## Local Development

```bash
# Install dependencies
cargo build

# Run indexer
export DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
export DEEPBOOK_ENV=testnet
export INDEXER_POLL_INTERVAL_MS=1000
export INDEXER_REQUEST_TIMEOUT_MS=30000

cargo run --release -- run
```

## Architecture

1. **Checkpoint Loop**: Polls the Remote Store for the next checkpoint
2. **Event Ingestion**: Filters DeepBook events by the built-in package list for the selected environment
3. **Fact Storage**:
   - `db_events` for fills
   - `db_order_events` for lifecycle events
4. **Metric Computation**: Recomputes affected 1-minute rollup buckets:
   - `pool_metrics_1m`
   - `bm_metrics_1m`
5. **State Tracking**: Updates `indexer_state` with the latest processed checkpoint for replay support

## Key Modules

- `main.rs`: entry point, orchestration, replay, status
- `config.rs`: environment config and package selection
- `remote_store.rs`: Remote Store client and backoff logic
- `events.rs`: BCS event structs and row mapping
- `storage` crate: database queries and models

## Troubleshooting

- **Missing DATABASE_URL**: Ensure environment variable is set before running
- **Wrong network data**: Verify `DEEPBOOK_ENV` matches the intended network
- **Slow / stalled indexer**: Check Remote Store reachability and logs with `RUST_LOG=debug`
- **Unexpected gap error**: Check `INDEXER_START_CHECKPOINT` vs persisted `indexer_state`

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
