# DeepBook V2 Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 `sui-deepbook-indexer` 从当前 v1/v1.5 状态推进到 `DATA_CONTRACT.md` 所定义的 v2 基础能力：统一配置、补齐 raw_event、落地双时间字段、建立 metadata 脚手架，并保持当前 API 尽量兼容。

**Architecture:** 采用渐进式改造，不做大规模重写。先保留现有表和 API 形状，在数据库层追加 `checkpoint_ts` / `event_ts` / metadata 支撑列，在 Rust indexer 中补齐写入，在 Go API 中优先使用新字段但保留旧字段兼容，然后再进入 normalized data 与更高级分析层。

**Tech Stack:** Rust (`sqlx`, `chrono`, `serde_json`), Go (`gin`, `pgx`, `shopspring/decimal`), PostgreSQL migrations, Docker Compose

---

### Task 1: 对齐配置模型与运行文档

**Files:**
- Modify: `../sui-deepbook-indexer/docker/docker-compose.yml`
- Modify: `../sui-deepbook-indexer/README.md`
- Modify: `../sui-deepbook-indexer/docs/USAGE.md`
- Modify: `../sui-deepbook-indexer/docs/ARCHITECTURE.md`
- Modify: `../sui-deepbook-indexer/indexer/README.md`
- Modify: `../sui-deepbook-indexer/indexer/.env.local.example`
- Modify: `../sui-deepbook-indexer/api-go/.env.local.example`

**Step 1: 将 Docker Compose 改成当前真实配置模型**

把 indexer 环境变量从旧版 RPC / package 直配，改成当前代码真实读取的配置：

```yaml
environment:
  DATABASE_URL: postgresql://sui:sui@postgres:5432/deepbook_indexer
  DEEPBOOK_ENV: testnet
  INDEXER_POLL_INTERVAL_MS: "1000"
  INDEXER_REQUEST_TIMEOUT_MS: "30000"
  INDEXER_BACKOFF_BASE_MS: "100"
  INDEXER_BACKOFF_MAX_MS: "30000"
  RUST_LOG: info
```

删除或注释误导性的：
- `RPC_API_URL`
- `DEEPBOOK_PACKAGE_ID`
- `DEEPBOOK_EVENT_TYPE`

**Step 2: 同步 README / USAGE / indexer README**

统一改成：
- 使用 `DEEPBOOK_ENV=mainnet|testnet`
- 数据源默认是 Remote Store，不再写成 RPC 主流程
- package 列表由代码内置维护，不要求用户自己传 `DEEPBOOK_PACKAGE_ID`

**Step 3: 修正 `api-go/.env.local.example`**

当前文件把两行环境变量粘在了一起，修正为：

```dotenv
DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer
API_LISTEN_ADDR=127.0.0.1:8080
LOG_LEVEL=debug
WS_PING_INTERVAL_SEC=15
API_SINGLE_KEY=
```

**Step 4: 验证 Compose 配置展开**

Run:

```bash
docker compose -f docker/docker-compose.yml config
```

Expected:
- 命令成功
- 输出中不再出现旧版 indexer 环境变量

**Step 5: Commit**

```bash
git add docker/docker-compose.yml README.md docs/USAGE.md docs/ARCHITECTURE.md indexer/README.md indexer/.env.local.example api-go/.env.local.example
git commit -m "docs: align runtime config with remote-store indexer"
```

---

### Task 2: 为成交表和生命周期表追加 v2 基础列

**Files:**
- Create: `../sui-deepbook-indexer/migrations/004_add_event_contract_v2_columns.sql`
- Modify: `../sui-deepbook-indexer/docs/DATA_CONTRACT.md`

**Step 1: 新建迁移文件，给 `db_events` 增加 v2 基础列**

创建迁移：

```sql
ALTER TABLE db_events
    ADD COLUMN IF NOT EXISTS checkpoint_ts TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS event_ts TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS ingested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS package_id TEXT,
    ADD COLUMN IF NOT EXISTS module TEXT,
    ADD COLUMN IF NOT EXISTS event_name TEXT;

UPDATE db_events
SET checkpoint_ts = COALESCE(checkpoint_ts, ts),
    event_ts = COALESCE(event_ts, ts)
WHERE checkpoint_ts IS NULL OR event_ts IS NULL;

CREATE INDEX IF NOT EXISTS idx_db_events_pool_event_ts
ON db_events (pool_id, event_ts);
```

**Step 2: 在同一个迁移里给 `db_order_events` 增加对应列**

```sql
ALTER TABLE db_order_events
    ADD COLUMN IF NOT EXISTS checkpoint_ts TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS event_ts TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS ingested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS package_id TEXT,
    ADD COLUMN IF NOT EXISTS module TEXT,
    ADD COLUMN IF NOT EXISTS event_name TEXT;

UPDATE db_order_events
SET checkpoint_ts = COALESCE(checkpoint_ts, ts),
    event_ts = COALESCE(event_ts, ts)
WHERE checkpoint_ts IS NULL OR event_ts IS NULL;

CREATE INDEX IF NOT EXISTS idx_db_order_events_pool_event_ts
ON db_order_events (pool_id, event_ts DESC);
```

**Step 3: 明确兼容策略**

不要删除旧列 `ts`。当前阶段：
- `ts` 继续保留
- `checkpoint_ts` / `event_ts` 作为新增精确语义列
- 后续 API 优先读新列并保留旧字段兼容

**Step 4: 手工验证迁移**

Run:

```bash
psql "postgresql://sui:sui@localhost:5432/deepbook_indexer" -f migrations/004_add_event_contract_v2_columns.sql
```

Expected:
- 迁移执行成功
- `db_events` / `db_order_events` 均存在新列

**Step 5: Commit**

```bash
git add migrations/004_add_event_contract_v2_columns.sql docs/DATA_CONTRACT.md
git commit -m "feat: add v2 timestamp and event metadata columns"
```

---

### Task 3: 更新 Rust storage model 与 SQL 查询

**Files:**
- Modify: `../sui-deepbook-indexer/storage/src/models.rs`
- Modify: `../sui-deepbook-indexer/storage/src/queries.rs`
- Test: `../sui-deepbook-indexer/storage/src/models.rs` (新增 `#[cfg(test)]` 小测试即可)

**Step 1: 给 `DbEventRow` 和 `DbOrderEventRow` 增加字段**

在 `storage/src/models.rs` 中补充：

```rust
pub checkpoint_ts: DateTime<Utc>,
pub event_ts: DateTime<Utc>,
pub ingested_at: DateTime<Utc>,
pub package_id: Option<String>,
pub module: Option<String>,
pub event_name: Option<String>,
```

如果担心迁移切换期兼容性，可先把新增字段定义为 `Option<DateTime<Utc>>` / `Option<String>`，等迁移稳定后再收紧。

**Step 2: 修改 `insert_db_events` SQL**

把插入语句扩展为：
- `checkpoint_ts`
- `event_ts`
- `ingested_at`
- `package_id`
- `module`
- `event_name`

`ON CONFLICT` 分支也要同步更新这些列。

**Step 3: 修改 `insert_db_order_events` SQL**

同样扩展生命周期事件插入和 UPSERT 逻辑。

**Step 4: 修改读查询**

更新以下查询的 `SELECT` 列表，保证模型与 SQL 一致：
- `list_events_in_checkpoint_range`
- `list_events_in_time_range`

当前仍可继续按 `ts` 查询桶，但读取出的模型必须带上 `checkpoint_ts` / `event_ts`，为后续切换做好准备。

**Step 5: 加一个最小编译测试**

在 `storage/src/models.rs` 或新增同文件测试模块中写一个最小序列化/构造测试，确保新增字段没有破坏 `serde` / `sqlx::FromRow` 派生。

Run:

```bash
cargo test -p deepbook-indexer-storage --lib
```

Expected:
- 编译通过
- 新增测试通过

**Step 6: Commit**

```bash
git add storage/src/models.rs storage/src/queries.rs
git commit -m "refactor: extend storage models for data contract v2"
```

---

### Task 4: 在 indexer 中真正写入 raw_event 与双时间字段

**Files:**
- Modify: `../sui-deepbook-indexer/indexer/src/events.rs`
- Modify: `../sui-deepbook-indexer/indexer/src/main.rs`
- Test: `../sui-deepbook-indexer/indexer/src/events.rs`

**Step 1: 在 `events.rs` 中增加时间转换辅助函数**

新增一个小函数，统一做毫秒时间戳到 `DateTime<Utc>` 的转换：

```rust
fn ts_from_ms(ms: i64) -> chrono::DateTime<chrono::Utc> {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}
```

如果事件里的 `timestamp` 字段类型是 `u64`，调用时用 `i64::try_from(...)`；溢出则 fallback 到 `checkpoint_ts`。

**Step 2: 让 `to_db_row` / `to_order_event_row` 接受更多上下文**

把函数签名改成能接收：
- `checkpoint`
- `checkpoint_ts_ms`
- `event_ts_ms`（可选）
- `tx_digest`
- `event_seq`
- `package_id`
- `module`
- `event_name`
- `raw_event`

`raw_event` 最简单的做法：直接对解析出来的 Rust struct 使用 `serde_json::to_value(&parsed_event)`。

**Step 3: 正确填充 `checkpoint_ts` / `event_ts`**

推荐规则：
- `checkpoint_ts = ts_from_ms(checkpoint_ts_ms)`
- `event_ts = ts_from_ms(parsed.timestamp as i64)`，若该事件没有 timestamp 或转换失败，则 fallback 到 `checkpoint_ts`
- `ts` 暂时继续写 `checkpoint_ts`，保持老查询兼容

**Step 4: 在 `main.rs` 里把事件元数据和 raw_event 传进去**

在每个事件分支里补充：

```rust
let raw_event = serde_json::to_value(&order_filled)?;
let module = event.type_.module.to_string();
let event_name = event.type_.name.to_string();
let package_id = format!("0x{}", hex::encode(event.package_id));
```

然后传给对应 `to_db_row` / `to_order_event_row`。

**Step 5: 给 `events.rs` 写单元测试**

至少覆盖：
- `OrderFilled` 转换后 `raw_event` 非空
- `event_ts` 优先取事件 timestamp
- fallback 时 `event_ts == checkpoint_ts`

Run:

```bash
cargo test -p deepbook-indexer-indexer events -- --nocapture
```

Expected:
- 测试通过
- `raw_event` 不再默认是 `None`

**Step 6: Commit**

```bash
git add indexer/src/events.rs indexer/src/main.rs
git commit -m "feat: persist raw events and dual timestamps"
```

---

### Task 5: 让 Go API 优先使用 v2 时间语义，但不破坏现有输出

**Files:**
- Modify: `../sui-deepbook-indexer/api-go/internal/store/store.go`
- Modify: `../sui-deepbook-indexer/docs/DATA_CONTRACT.md`
- Test: `../sui-deepbook-indexer/api-go/internal/handlers/handlers_cursor_test.go`

**Step 1: 为 fills / lifecycle 查询统一时间读取口径**

在 SQL 中把当前：

```sql
EXTRACT(EPOCH FROM ts) * 1000 AS ts_ms
```

改成：

```sql
EXTRACT(EPOCH FROM COALESCE(event_ts, checkpoint_ts, ts)) * 1000 AS ts_ms
```

这样：
- 旧数据仍可工作
- 新数据优先使用 `event_ts`

**Step 2: 为 execution summary 明确口径**

短期先保持 `WHERE ts >= NOW() - ...` 不变，避免一次改太多。

在代码旁边加一个非啰嗦注释：
- 当前 summary 仍按兼容列 `ts` 过滤
- 后续切到 `event_ts` 时再统一做行为变更和回归测试

**Step 3: 保持 API 响应结构不变**

当前阶段不要重命名现有 JSON 字段：
- 继续保留 `ts_ms`
- 继续保留 `base_size` / `quote_size`
- 继续保留当前 cursor 格式

只改变 `ts_ms` 的优先数据来源。

**Step 4: 运行 Go 测试**

Run:

```bash
cd api-go && go test ./...
```

Expected:
- 现有 handler/cursor 测试通过
- 没有因为 store 代码调整引入编译错误

**Step 5: Commit**

```bash
git add api-go/internal/store/store.go api-go/internal/handlers/handlers_cursor_test.go docs/DATA_CONTRACT.md
git commit -m "refactor: prefer v2 timestamps in API queries"
```

---

### Task 6: 建立 metadata 表与最小脚手架

**Files:**
- Create: `../sui-deepbook-indexer/migrations/005_add_metadata_tables.sql`
- Modify: `../sui-deepbook-indexer/storage/src/models.rs`
- Modify: `../sui-deepbook-indexer/storage/src/queries.rs`
- Create: `../sui-deepbook-indexer/indexer/src/metadata.rs`
- Modify: `../sui-deepbook-indexer/indexer/src/main.rs`
- Modify: `../sui-deepbook-indexer/docs/DATA_CONTRACT.md`

**Step 1: 创建 metadata 表**

建议先最小化建表：

```sql
CREATE TABLE IF NOT EXISTS asset_metadata (
    asset_id TEXT PRIMARY KEY,
    coin_type TEXT,
    symbol TEXT,
    name TEXT,
    decimals INT,
    status TEXT,
    source TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS pool_metadata (
    pool_id TEXT PRIMARY KEY,
    base_asset_id TEXT,
    quote_asset_id TEXT,
    package_id TEXT,
    status TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

第一版不必立刻把外键关系做得很重，先保证 schema 与查询接口稳定。

**Step 2: 添加 storage model / query**

在 `storage/src/models.rs` 新增：
- `AssetMetadataRow`
- `PoolMetadataRow`

在 `storage/src/queries.rs` 新增：
- `upsert_asset_metadata`
- `upsert_pool_metadata`
- `get_asset_metadata`
- `get_pool_metadata`

**Step 3: 创建 indexer metadata 模块**

新增 `indexer/src/metadata.rs`，第一版先提供：
- 静态或配置驱动的 metadata seed
- `fn seed_known_metadata(...) -> Result<()>`

不要一开始就做复杂链上发现；先让 normalized path 有可用的 metadata 来源。

**Step 4: 在启动流程中调用 metadata seed**

在 `main.rs` 中，数据库连接和迁移完成后，先执行一次 metadata seed，再进入 `run/replay/status` 逻辑。

**Step 5: 验证最小闭环**

Run:

```bash
cargo test -p deepbook-indexer-storage --lib
cargo test -p deepbook-indexer-indexer --lib
```

Expected:
- storage 与 indexer 均可通过编译/基础测试
- metadata 相关模型与查询不报错

**Step 6: Commit**

```bash
git add migrations/005_add_metadata_tables.sql storage/src/models.rs storage/src/queries.rs indexer/src/metadata.rs indexer/src/main.rs docs/DATA_CONTRACT.md
git commit -m "feat: add metadata scaffolding for normalized data"
```

---

### Task 7: 完成回归验证与文档收尾

**Files:**
- Modify: `../sui-deepbook-indexer/README.md`
- Modify: `../sui-deepbook-indexer/docs/README.md`
- Modify: `../sui-deepbook-indexer/docs/USAGE.md`

**Step 1: 跑基础验证命令**

Run:

```bash
cargo test -p deepbook-indexer-storage --lib
cargo test -p deepbook-indexer-indexer --lib
cd api-go && go test ./...
docker compose -f docker/docker-compose.yml config
```

Expected:
- Rust / Go 至少通过基础单测与编译
- Compose 配置成功展开

**Step 2: 做一轮最小手工验收**

本地启动后手工检查：
- `/health`
- `/v1/deepbook/pools/:pool_id/execution/fills`
- `/v1/deepbook/pools/:pool_id/execution/lifecycle`
- `WS /v1/deepbook/trades`

重点确认：
- 老接口没有被破坏
- `ts_ms` 行为未退化
- 新数据写入后数据库里 `raw_event` 非空

**Step 3: 更新文档索引**

把 `docs/README.md` 中的计划文档也列出来，方便后续执行。

**Step 4: Commit**

```bash
git add README.md docs/README.md docs/USAGE.md
git commit -m "docs: finalize v2 foundation rollout notes"
```

---

## 执行顺序建议

按以下顺序执行，不要并行改动共享文件：
1. Task 1 配置与文档对齐
2. Task 2 数据库迁移
3. Task 3 storage model / query 改造
4. Task 4 indexer 写入改造
5. Task 5 API 兼容改造
6. Task 6 metadata 脚手架
7. Task 7 回归验证与文档收尾

---

## 风险提醒

1. **不要先删 `ts`**：先追加新列，等 API 全切换后再考虑废弃
2. **不要直接把现有 `price/base_sz/quote_sz` 语义改成 normalized**：必须新增字段表达
3. **不要在 Task 6 一开始就做复杂链上 metadata 自动发现**：先用最小 seed 方案落地
4. **不要在同一提交里同时做 schema 重构和 API 结构重命名**：分开做，方便回归

---

## 完成定义

当以下条件同时满足，才算本阶段完成：
- 文档、docker、env 配置一致
- `db_events` / `db_order_events` 写入 `raw_event`
- 数据库中存在 `checkpoint_ts` / `event_ts`
- API `ts_ms` 能优先读取新时间字段
- metadata 表存在且可被 seed
- 基础 Rust / Go 测试和最小手工验收通过
