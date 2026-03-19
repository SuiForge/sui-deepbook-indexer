# Architecture

The open-source version of this repository currently has three core components: **Rust Indexer** (consumes Sui checkpoint data and parses DeepBook events), **PostgreSQL** (fact tables + rollups), and **Go API** (REST + WebSocket).

Related docs:
- Field semantics and units: `docs/DATA_CONTRACT.md`
- DeepBook v3 event inventory: `docs/DEEPBOOK_EVENTS.md`
- Target design: `docs/ARCHITECTURE_V2.md`

## Components

### 1) Indexer (Rust)

- Entry point: `indexer/src/main.rs`
- Data source: Sui Remote Store checkpoint data
- Network selection: `DEEPBOOK_ENV=mainnet|testnet` selects both the Remote Store URL and the package list for that network
- Currently indexed events:
  - `OrderFilled` → `db_events`
  - `OrderPlaced` / `OrderCanceled` / `OrderModified` → `db_order_events`
- Writes facts and recomputes affected 1-minute buckets (`pool_metrics_1m` / `bm_metrics_1m`)
- Idempotency: fact tables use `(tx_digest, event_seq)` as the primary key, so replays are safe

### 2) Storage (PostgreSQL)

Migrations live in `migrations/`:
- `migrations/001_init.sql`
- `migrations/002_add_pool_ohlc.sql`
- `migrations/003_add_order_lifecycle_events.sql`

Core tables:
- `db_events`: fill facts
- `db_order_events`: order lifecycle events
- `pool_metrics_1m`: pool-level 1-minute rollups (including OHLC)
- `bm_metrics_1m`: BalanceManager-level 1-minute rollups
- `indexer_state`: indexer progress

### 3) API (Go / Gin)

Entry point: `api-go/cmd/main.go`

Current endpoints:
- `GET /health`
- `GET /v1/deepbook/pools/:pool_id/metrics?window=1h|24h`
- `GET /v1/deepbook/pools/:pool_id/candles?window=1h|24h|7d&interval=1m|5m|15m|1h`
- `GET /v1/deepbook/pools/:pool_id/execution/summary?window=1h|24h|7d`
- `GET /v1/deepbook/pools/:pool_id/execution/lifecycle?...`
- `GET /v1/deepbook/pools/:pool_id/execution/fills?...`
- `GET /v1/deepbook/bm/:bm_id/volume?window=24h|7d&pool=POOL1,POOL2`
- `WS /v1/deepbook/trades?pool=POOL1,POOL2`

## Data Flow

### 1) Live indexing (`run`)

1. Poll the next checkpoint from the Remote Store
2. Iterate through transaction events inside that checkpoint
3. Match DeepBook events using the built-in package list for the selected environment
4. Parse supported fill and lifecycle events
5. **In a single DB transaction**:
   - UPSERT `db_events`
   - UPSERT `db_order_events`
   - query the full event set for each affected minute bucket from `db_events`
   - recompute and UPSERT `pool_metrics_1m` / `bm_metrics_1m`
   - update `indexer_state.processed_checkpoint`

Benefits of this design:
- multiple checkpoints landing in the same minute do not overwrite each other incorrectly
- correction / replay can recompute only affected buckets and still match full rebuild semantics
- fact writes and rollup updates stay in the same transactional boundary

### 2) Replay (`replay`)

`replay --from-checkpoint A --to-checkpoint B`:
1. Query `db_events` in the `[A, B]` checkpoint range
2. Compute the set of affected minute buckets
3. For each bucket, reload the full bucket from `db_events`, recompute rollups, and UPSERT results

> Current replay only recomputes rollups. It does not delete facts from `db_events`.

## Reliability Notes

- **Idempotent writes**: fact-table UPSERTs and rollup UPSERTs allow repeated processing of the same checkpoint/event.
- **Atomicity**: writes, rollup recomputation, and progress updates happen in one DB transaction.
- **Backoff and retry**: Remote Store fetch failures use exponential backoff with jitter.
- **WebSocket behavior**: the server first sends the latest 100 trades, then streams new ones; reconnect / resume is currently handled client-side.
