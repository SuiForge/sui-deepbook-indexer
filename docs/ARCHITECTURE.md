# Architecture

本仓库的开源版由 3 个核心组件组成：**Rust Indexer**（抓链上事件）、**PostgreSQL**（事实表 + 聚合表）、**Go API**（REST + WebSocket）。

相关文档：
- 字段语义/单位：`docs/DATA_CONTRACT.md`
- DeepBook v3 事件清单：`docs/DEEPBOOK_EVENTS.md`

## Components

### 1) Indexer（Rust）

- 入口：`indexer/src/main.rs`
- 数据源：Sui RPC `get_checkpoint`
- 过滤：按 `DEEPBOOK_PACKAGE_ID` + `DEEPBOOK_EVENT_TYPE`（默认 `OrderFilled`）
- 落库：写入 `db_events`，并回算受影响的 1 分钟 bucket（`pool_metrics_1m` / `bm_metrics_1m`）
- 幂等：`db_events` 以 `(tx_digest, event_seq)` 为主键，允许安全重放

### 2) Storage（PostgreSQL）

迁移在 `migrations/`：
- `migrations/001_init.sql`
- `migrations/002_add_pool_ohlc.sql`

核心表：
- `db_events`：成交事实（带 `raw_event`，便于追溯）
- `pool_metrics_1m`：池维度 1 分钟聚合（含 OHLC）
- `bm_metrics_1m`：BalanceManager 维度 1 分钟聚合（maker/taker 归因）

### 3) API（Go / Gin）

入口：`api-go/cmd/main.go`

当前端点：
- `GET /health`
- `GET /v1/deepbook/pools/:pool_id/metrics?window=1h|24h`
- `GET /v1/deepbook/pools/:pool_id/candles?window=1h|24h|7d&interval=1m|5m|15m|1h`
- `GET /v1/deepbook/bm/:bm_id/volume?window=24h|7d&pool=POOL1,POOL2`
- `WS /v1/deepbook/trades?pool=POOL1,POOL2`

## Data Flow

### 1) 索引（Run）

1. 轮询拉取 checkpoint（支持主/备 RPC）
2. 遍历 checkpoint 内交易的 events
3. 解析并筛选 DeepBook 成交事件（`OrderFilled`）
4. **单事务**写入：
   - UPSERT `db_events`
   - 对受影响的 minute bucket：从 `db_events` 查询该 bucket 的全量事件并回算聚合
   - UPSERT `pool_metrics_1m` / `bm_metrics_1m`
   - 更新 `indexer_state.processed_checkpoint`

这样做的好处：
- 同一分钟内多个 checkpoint 进入时不会覆盖/丢失聚合数据
- 纠错/回放可以只重算受影响 bucket，结果与全量重建一致

### 2) 回放（Replay）

`replay --from-checkpoint A --to-checkpoint B`：
1. 查询 `[A,B]` 范围内的 `db_events`
2. 计算这些事件涉及到的 minute bucket 集合
3. 对每个 bucket：从 `db_events` 查询该 bucket 的全量事件并回算聚合后 UPSERT

> 当前 replay 只负责“聚合重算”，不会删除 `db_events`。

## Reliability Notes

- **幂等**：`db_events` UPSERT + rollup UPSERT，允许重复处理同一 checkpoint/事件。
- **原子性**：同一 checkpoint 的事件写入、聚合回算、进度更新在一个 DB 事务内完成。
- **退避重试**：RPC 拉取失败会指数退避（带 jitter），避免打爆节点。
- **WS 行为**：先推送最近 100 条成交，再持续推送新成交；断线重连由客户端负责。

