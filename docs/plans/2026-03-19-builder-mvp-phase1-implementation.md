# Builder-grade MVP Phase 1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Push `sui-deepbook-indexer` beyond the v2 foundation into a more builder-facing MVP by unifying execution time semantics, exposing metadata and market discovery APIs, and surfacing service status in the Go serving plane.

**Architecture:** Reuse the existing Rust/Go split without adding a new service. The API layer will read from the current PostgreSQL schema, prefer `COALESCE(event_ts, checkpoint_ts, ts)` where serving semantics need to reflect event time, and expose lightweight builder-facing endpoints on top of the metadata and rollup tables already introduced in foundation work.

**Tech Stack:** Go (`gin`, `pgx`, `shopspring/decimal`), PostgreSQL, Rust migration/indexer artifacts already present, repo docs under `docs/`

---

### Task 1: Add API handler test scaffolding via a store interface

**Files:**
- Modify: `api-go/internal/handlers/handlers.go`
- Modify: `api-go/cmd/main.go`
- Test: `api-go/internal/handlers/handlers_cursor_test.go`

**Step 1: Introduce a handler-facing store interface**

Refactor `Handler` so it depends on an interface instead of `*store.Store`. The interface should cover all current handler methods plus the new ones added in later tasks.

**Step 2: Keep `store.Store` compatibility**

Do not change the public `store.New(...)` signature. Update `handlers.New(...)` and `cmd/main.go` so the concrete store still plugs in without extra wiring.

**Step 3: Add fake-store test scaffolding**

Extend `handlers_cursor_test.go` with a minimal fake implementation that can satisfy the handler interface for validation-only tests added in later tasks.

**Step 4: Run tests**

Run:

```bash
cd api-go && GOROOT=/usr/local/go go test ./internal/handlers
```

Expected:
- Handler package compiles
- Existing cursor tests still pass

**Step 5: Commit**

```bash
git add api-go/internal/handlers/handlers.go api-go/cmd/main.go api-go/internal/handlers/handlers_cursor_test.go
git commit -m "refactor: decouple handlers from concrete store"
```

---

### Task 2: Unify execution summary and WebSocket `ts_ms` semantics

**Files:**
- Modify: `api-go/internal/store/store.go`
- Modify: `docs/DATA_CONTRACT.md`
- Modify: `README.md`
- Modify: `docs/USAGE.md`

**Step 1: Switch summary time filter/order to compatibility time**

In `GetExecutionSummary`, change the query to use `COALESCE(event_ts, checkpoint_ts, ts)` for:
- window filter
- first/last price ordering

This intentionally changes summary semantics to align with fills/lifecycle.

**Step 2: Switch WebSocket trade payload timestamps**

Update `StreamTrades` so backlog and live rows read `COALESCE(event_ts, checkpoint_ts, ts)` for the emitted `ts_ms`. Preserve the existing checkpoint-based live cursor for stable incremental delivery.

**Step 3: Document the serving-plane time rules**

Update docs so:
- `execution/summary` now uses compatibility time semantics
- WebSocket trade events expose `ts_ms` from `COALESCE(event_ts, checkpoint_ts, ts)`

**Step 4: Verify**

Run:

```bash
cd api-go && GOROOT=/usr/local/go go test ./...
```

And perform a minimal manual API/WS check against a temporary Postgres instance with seeded rows.

**Step 5: Commit**

```bash
git add api-go/internal/store/store.go docs/DATA_CONTRACT.md README.md docs/USAGE.md
git commit -m "refactor: align summary and ws timestamps"
```

---

### Task 3: Expose metadata read APIs for assets and pools

**Files:**
- Modify: `api-go/internal/store/store.go`
- Modify: `api-go/internal/handlers/handlers.go`
- Modify: `api-go/cmd/main.go`
- Modify: `docs/DATA_CONTRACT.md`
- Modify: `README.md`
- Modify: `docs/USAGE.md`
- Test: `api-go/internal/handlers/handlers_cursor_test.go`

**Step 1: Add metadata response models and store queries**

Add Go structs and store methods for:
- listing asset metadata
- listing pool metadata
- fetching a single pool metadata record with optional embedded base/quote asset info

Use the existing `asset_metadata` / `pool_metadata` tables. Keep the query surface small and builder-oriented.

**Step 2: Add REST endpoints**

Add endpoints:
- `GET /v1/deepbook/assets`
- `GET /v1/deepbook/pools`
- `GET /v1/deepbook/pools/:pool_id/metadata`

Keep responses simple and explicit.

**Step 3: Add handler tests**

Use the fake store from Task 1 to test:
- empty / missing `pool_id` validation for the pool metadata route
- successful JSON shape for the metadata list/detail handlers

**Step 4: Verify**

Run:

```bash
cd api-go && GOROOT=/usr/local/go go test ./...
```

Expected:
- New endpoints compile
- handler tests pass

**Step 5: Commit**

```bash
git add api-go/internal/store/store.go api-go/internal/handlers/handlers.go api-go/cmd/main.go api-go/internal/handlers/handlers_cursor_test.go docs/DATA_CONTRACT.md README.md docs/USAGE.md
git commit -m "feat: add deepbook metadata api"
```

---

### Task 4: Add market discovery and service status APIs

**Files:**
- Modify: `api-go/internal/store/store.go`
- Modify: `api-go/internal/handlers/handlers.go`
- Modify: `api-go/cmd/main.go`
- Modify: `docs/DATA_CONTRACT.md`
- Modify: `README.md`
- Modify: `docs/USAGE.md`
- Test: `api-go/internal/handlers/handlers_cursor_test.go`

**Step 1: Add top markets query**

Implement a `top markets` read model backed by `pool_metrics_1m`, with:
- window: `1h|24h|7d`
- sort: `volume_quote|trades`
- limit clamp

Prefer joining seeded metadata when available so the response can include base/quote asset ids or a derived pair label.

**Step 2: Add service status query**

Implement a small service status read model backed by current DB state:
- processed checkpoint
- indexer state timestamp
- event counts
- order event counts
- metadata counts
- distinct pool count

**Step 3: Add REST endpoints**

Add:
- `GET /v1/deepbook/markets/top`
- `GET /v1/deepbook/status`

**Step 4: Add handler tests**

Use the fake store to test:
- limit fallback/clamp handling for `markets/top`
- successful status response shape

**Step 5: Verify**

Run:

```bash
cd api-go && GOROOT=/usr/local/go go test ./...
```

Then run a manual API check against a temporary Postgres instance to confirm both endpoints return non-error JSON.

**Step 6: Commit**

```bash
git add api-go/internal/store/store.go api-go/internal/handlers/handlers.go api-go/cmd/main.go api-go/internal/handlers/handlers_cursor_test.go docs/DATA_CONTRACT.md README.md docs/USAGE.md
git commit -m "feat: add market discovery and status api"
```

---

### Task 5: Final verification and docs index update

**Files:**
- Modify: `docs/README.md`

**Step 1: Run end-to-end verification commands**

Run:

```bash
cargo test -p deepbook-indexer-storage --lib
cargo test -p deepbook-indexer-indexer
cd api-go && GOROOT=/usr/local/go go test ./...
docker compose -f docker/docker-compose.yml config
```

**Step 2: Perform minimal manual acceptance**

Against a temporary Postgres + local API instance, validate:
- `/health`
- `/v1/deepbook/status`
- `/v1/deepbook/assets`
- `/v1/deepbook/pools`
- `/v1/deepbook/pools/:pool_id/metadata`
- `/v1/deepbook/markets/top`
- `/v1/deepbook/pools/:pool_id/execution/summary`
- `WS /v1/deepbook/trades`

**Step 3: Update docs index**

Add this plan to `docs/README.md`.

**Step 4: Commit**

```bash
git add docs/README.md
git commit -m "docs: record builder mvp phase 1 rollout"
```
