# Builder Ops + OpenAPI Phase Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the Go serving plane from a builder MVP into a more operator-friendly and integration-friendly service by adding source-aware status/lag, Prometheus-compatible metrics, and OpenAPI + example client artifacts.

**Architecture:** Keep the current Rust indexer / Go API split. The Go API will add a small source checkpoint probe for the Sui Remote Store, enrich the existing `/v1/deepbook/status` response with latest-checkpoint / lag semantics, expose a scrape-friendly `/metrics` endpoint, and publish a static OpenAPI contract plus a minimal JS example client without introducing a new service.

**Tech Stack:** Go (`gin`, `net/http`, current pgx store), static YAML docs, minimal JS example under `examples/`

---

### Task 1: Add source-aware status and lag semantics

**Files:**
- Modify: `api-go/internal/config/config.go`
- Create: `api-go/internal/source/checkpoints.go`
- Modify: `api-go/internal/store/store.go`
- Modify: `api-go/internal/handlers/handlers.go`
- Modify: `api-go/internal/handlers/handlers_cursor_test.go`
- Test: `api-go/internal/source/checkpoints_test.go`
- Modify: `README.md`
- Modify: `docs/USAGE.md`
- Modify: `docs/DATA_CONTRACT.md`

**Step 1: Introduce source config and checkpoint probe**

Add API-side config for:
- `DEEPBOOK_ENV` (`testnet|mainnet`, default `testnet`)
- `SOURCE_STATUS_TIMEOUT_MS` (default `5000`)
- `SOURCE_STATUS_CACHE_SEC` (default `15`)

Implement a small source probe that can resolve the correct Remote Store base URL from `DEEPBOOK_ENV` and fetch the latest checkpoint with a cache-aware search strategy.

**Step 2: Expand service status model**

Extend the existing service status response to include:
- `latest_checkpoint`
- `checkpoint_lag`
- `source_status`
- `source_url`
- optional `source_error`

`source_status` should clearly distinguish:
- `ok`
- `error`
- `disabled` (if source probing is turned off or unavailable)

**Step 3: Add handler tests first**

Using the fake store and a fake source probe, add tests covering:
- status response with lag enrichment
- status response when source probe fails
- default/fallback source status behavior

**Step 4: Verify**

Run:

```bash
cd api-go && GOROOT=/usr/local/go go test ./internal/handlers ./internal/source
```

Expected:
- new status tests pass
- source probe package compiles and tests pass

**Step 5: Commit**

```bash
git add api-go/internal/config/config.go api-go/internal/source/checkpoints.go api-go/internal/source/checkpoints_test.go api-go/internal/store/store.go api-go/internal/handlers/handlers.go api-go/internal/handlers/handlers_cursor_test.go README.md docs/USAGE.md docs/DATA_CONTRACT.md
git commit -m "feat: add source-aware service status"
```

---

### Task 2: Expose Prometheus-compatible metrics

**Files:**
- Modify: `api-go/internal/handlers/handlers.go`
- Modify: `api-go/cmd/main.go`
- Modify: `api-go/internal/handlers/handlers_cursor_test.go`
- Modify: `README.md`
- Modify: `docs/USAGE.md`
- Modify: `docs/DATA_CONTRACT.md`

**Step 1: Add a scrape-friendly `/metrics` endpoint**

Implement a Prometheus text-format endpoint that exposes at least:
- processed checkpoint
- latest checkpoint
- checkpoint lag
- trade event count
- order event count
- asset metadata count
- pool metadata count
- distinct pool count
- source health state

Reuse the status composition path from Task 1 so metrics and `/status` stay aligned.

**Step 2: Add handler tests**

Using fake store + fake source probe, assert that:
- `/metrics` returns `200`
- output contains the expected metric names
- gauges reflect enriched lag values

**Step 3: Verify**

Run:

```bash
cd api-go && GOROOT=/usr/local/go go test ./internal/handlers
```

Expected:
- metrics endpoint tests pass
- existing handler tests remain green

**Step 4: Commit**

```bash
git add api-go/internal/handlers/handlers.go api-go/cmd/main.go api-go/internal/handlers/handlers_cursor_test.go README.md docs/USAGE.md docs/DATA_CONTRACT.md
git commit -m "feat: expose prometheus-compatible metrics"
```

---

### Task 3: Publish OpenAPI contract and example client

**Files:**
- Create: `docs/openapi/deepbook-v1.yaml`
- Create: `examples/js/deepbook-client.mjs`
- Modify: `api-go/cmd/main.go`
- Modify: `README.md`
- Modify: `docs/README.md`
- Modify: `docs/USAGE.md`

**Step 1: Add a static OpenAPI document**

Document the current public REST surface in a single YAML spec, covering:
- health
- status
- assets / pools / metadata
- markets/top
- metrics / candles / summary / lifecycle / fills / BM volume

Keep the spec honest to the current response fields and query params already implemented.

**Step 2: Add a minimal JS example client**

Provide a tiny example that fetches:
- `/v1/deepbook/status`
- `/v1/deepbook/markets/top`
- `/v1/deepbook/pools/:pool_id/execution/summary`

The example should be copy-paste friendly and clearly document base URL usage.

**Step 3: Optionally serve the spec from the API**

Expose the spec as a static route (for example `/openapi.yaml`) if it can be done without adding a new dependency or large framework.

**Step 4: Verify**

Run:

```bash
cd api-go && GOROOT=/usr/local/go go test ./...
test -f docs/openapi/deepbook-v1.yaml
node examples/js/deepbook-client.mjs --help || true
```

Expected:
- Go tests still pass
- OpenAPI file exists
- example client is present and readable

**Step 5: Commit**

```bash
git add docs/openapi/deepbook-v1.yaml examples/js/deepbook-client.mjs api-go/cmd/main.go README.md docs/README.md docs/USAGE.md
git commit -m "docs: add openapi contract and example client"
```

---

### Task 4: Final verification and rollout notes

**Files:**
- Modify: `docs/README.md`

**Step 1: Run final verification**

Run:

```bash
cargo test -p deepbook-indexer-storage --lib
cargo test -p deepbook-indexer-indexer
cd api-go && GOROOT=/usr/local/go go test ./...
docker compose -f docker/docker-compose.yml config
```

**Step 2: Manual acceptance**

Against a temporary Postgres + local API instance, validate:
- `/health`
- `/metrics`
- `/v1/deepbook/status`
- `/v1/deepbook/assets`
- `/v1/deepbook/markets/top`
- `/openapi.yaml` (if Task 3 serves it)
- `WS /v1/deepbook/trades`

**Step 3: Update docs index**

Add this plan and the new ops/openapi rollout state to `docs/README.md`.

**Step 4: Commit**

```bash
git add docs/README.md
git commit -m "docs: record builder ops and openapi rollout"
```
