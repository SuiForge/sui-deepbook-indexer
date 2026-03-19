# 数据契约（v2 目标契约 + 当前兼容说明）

- Status: Proposed / Partial Implementation
- Last Updated: 2026-03-19
- Related Docs:
  - `docs/PRODUCT_PRD.md`
  - `docs/ARCHITECTURE_V2.md`
  - `docs/FEATURE_BACKLOG.md`

## 1. 文档目的

本文档不再只描述旧版单一成交事件模型，而是统一定义：
- 当前服务已经对外暴露的字段语义
- 当前实现与目标契约之间的差距
- 后续 `v2` 需要落地的数据模型方向

> 重要：本文档是 **“目标契约 + 当前兼容说明”**。
>
> 也就是说：
> - 文中标记为 **Current** 的内容，表示当前代码/数据库/API 已基本具备
> - 文中标记为 **Target v2** 的内容，表示后续迭代目标，当前可能尚未完全实现

---

## 2. 版本边界

### 2.1 v1 的问题

旧版 `DATA_CONTRACT.md` 主要围绕 `OrderFilled` 展开，存在几个不足：
- 没有把生命周期事件纳入统一契约
- 没有明确区分 `checkpoint_ts` 与 `event_ts`
- 没有系统化描述 raw / normalized 双轨字段
- 没有说明当前实现中 `raw_event` 实际可能为空
- 没有给出 API cursor、时间、数值序列化的统一规则

### 2.2 v2 的目标

`v2` 的目标是把当前项目收敛成一套 **builder-facing、语义清晰、可演进** 的契约：
- canonical trade facts
- order lifecycle facts
- clear timestamp semantics
- raw + normalized values
- metadata-driven enrichment
- stable REST / WebSocket conventions

### 2.3 Migration Status

**Current after migration 005**
- `db_events` and `db_order_events` now introduce schema columns for:
  - `checkpoint_ts`
  - `event_ts`
  - `ingested_at`
  - `package_id`
  - `module`
  - `event_name`
- Existing rows are backfilled with `checkpoint_ts = ts` and `event_ts = ts` as a compatibility fallback
- Current write-path status for newly ingested rows:
  - `checkpoint_ts`, `event_ts`, `ingested_at`, `package_id`, `module`, `event_name`, and `raw_event` can already be populated from the current conversion layer
- `asset_metadata` and `pool_metadata` tables now exist as v2 scaffolding
- indexer startup seeds a repo-local static catalog:
  - shared asset rows for `SUI` and `USDC`
  - one known mainnet `SUI/USDC` pool mapping
  - testnet currently keeps only asset rows until verified pool ids are curated
- Readers should continue to treat `COALESCE(event_ts, checkpoint_ts, ts)` as the safe compatibility read path during the phased rollout
- The remaining writer and API rollout still continues incrementally in later tasks

---

## 3. 数据范围与事件覆盖

### 3.1 数据源范围

**Current**
- indexer 通过 `DEEPBOOK_ENV` 选择 `mainnet` / `testnet`
- package id 列表由代码内置维护，并按环境切换
- 默认数据源是 Sui Remote Store checkpoint 数据

### 3.2 当前已接入事件

**Current**
- `OrderFilled` → `db_events`
- `OrderPlaced` → `db_order_events`
- `OrderCanceled` → `db_order_events`
- `OrderModified` → `db_order_events`

**Target v2**
- `OrderExpired` → `db_order_events`（当前结构已预留方向，但主流程尚未完全接入）

### 3.3 契约范围内的核心实体

`v2` 统一围绕以下实体建模：
- Trade Fact
- Order Lifecycle Fact
- Pool Metrics 1m
- BalanceManager Metrics 1m
- Asset Metadata（Target v2）
- Pool Metadata（Target v2）

---

## 4. 全局契约约定

## 4.1 ID 字段

所有链上对象/地址相关字段统一按字符串表示：
- `pool_id`
- `maker_bm`
- `taker_bm`
- `trader`
- `order_id`
- `tx_digest`

说明：
- object id / address / digest 均按链上原始标识的字符串形式返回
- 不在 API 层做缩写或 checksum 风格变换

## 4.2 时间字段

### Current
当前数据库事实表中的兼容列 `ts` 仍来自：
- `checkpoint.checkpoint_summary.timestamp_ms`

因此：
- 表级兼容列 `ts` 的语义仍更接近 **checkpoint 时间代理值**
- `execution/fills`、`execution/lifecycle`、`execution/summary` 与当前 WebSocket trade stream 的 `ts_ms` / 时间窗口都已优先读取 `COALESCE(event_ts, checkpoint_ts, ts)`

### Target v2
后续统一收敛为三类时间：
- `checkpoint_ts`: checkpoint 的官方时间
- `event_ts`: event payload 自身携带的业务时间（如存在）
- `ingested_at`: 本服务实际写入数据库的时间

Compatibility note:
- For rows that existed before migration `004_add_event_contract_v2_columns.sql`, `ingested_at` is a bootstrap / synthetic value created during schema rollout, not a historically exact ingest timestamp

兼容策略：
- 在 `v2` 真正落地前，execution-serving 路径统一使用 `COALESCE(event_ts, checkpoint_ts, ts)`
- 文档与 API 描述必须明确：旧数据会 fallback 到 checkpoint 语义，新数据才会优先体现 `event_ts`

## 4.3 数值字段

### Current
当前数据库主要保存链上原始数值：
- `price`
- `base_sz`
- `quote_sz`
- `original_quantity`
- `new_quantity`
- `canceled_quantity`

这些值通常是协议事件里的原始单位，不等同于已经按 decimals 归一化后的展示值。

### Target v2
后续所有关键数值字段建议同时保留：
- `*_raw`: 链上原始值
- `*_norm`: 按 metadata / decimals 归一化后的值

## 4.4 JSON 序列化

### Current
REST / WebSocket 中的 decimal 字段按字符串输出，以避免精度丢失。

典型字段包括：
- `price`
- `base_sz` / `base_size`
- `quote_sz` / `quote_size`
- `volume_base`
- `volume_quote`
- `vwap`
- `last_price`

时间字段由 Go `time.Time` 序列化为 RFC3339 风格字符串；`ts_ms` 为毫秒级 Unix 时间戳整数。

## 4.5 Cursor 约定

### Current
`execution/lifecycle` 与 `execution/fills` 的 cursor 格式为：

```text
ts_ms|checkpoint|event_seq
```

语义：
- `ts_ms`: 当前记录的 `COALESCE(event_ts, checkpoint_ts, ts)` 毫秒值
- `checkpoint`: checkpoint 序号
- `event_seq`: 事件在交易内的序号

排序规则：
- `ORDER BY COALESCE(event_ts, checkpoint_ts, ts) DESC, checkpoint DESC, event_seq DESC`

---

## 5. Trade Fact：`db_events`

### 5.1 作用

`db_events` 是当前服务最核心的 DeepBook 成交事实表。

用途：
- fills 查询
- execution summary 统计
- pool / BM 聚合重算
- WebSocket 成交流输出
- replay 区间重建

### 5.2 主键与幂等性

**Current**
- 主键：`(tx_digest, event_seq)`

含义：
- 同一笔交易内用 `event_seq` 唯一标识该 event
- replay 时通过 UPSERT 保持幂等

### 5.3 当前字段语义

| 字段 | Current 语义 | 备注 |
|---|---|---|
| `checkpoint` | Sui checkpoint 序号 | 已实现 |
| `ts` | checkpoint 时间代理值 | 当前不是 `OrderFilled.timestamp` |
| `pool_id` | DeepBook pool object id | 已实现 |
| `side` | taker 方向：`buy`/`sell` | 由 `taker_is_bid` 推导 |
| `price` | 成交价格原始值 | 未做 decimals 归一化 |
| `base_sz` | base 成交量原始值 | 来自 `base_quantity` |
| `quote_sz` | quote 成交量原始值 | 来自 `quote_quantity` |
| `maker_bm` | maker BalanceManager id | 可为空 |
| `taker_bm` | taker BalanceManager id | 可为空 |
| `tx_digest` | 交易摘要 | 已实现 |
| `event_seq` | 交易内事件序号 | 已实现 |
| `event_index` | 事件索引补充字段 | 当前通常为空 |
| `raw_event` | 原始事件 JSON | 当前写路径已支持落库；历史数据可能仍为空 |

### 5.4 Target v2 字段方向

Schema foundation status:
- `checkpoint_ts`, `event_ts`, `ingested_at`, `package_id`, `module`, and `event_name` are introduced by migration `004_add_event_contract_v2_columns.sql`
- Historical rows are backfilled from legacy `ts` semantics
- Newly ingested rows can already populate `checkpoint_ts`, `event_ts`, `package_id`, `module`, `event_name`, and `raw_event` from the current conversion layer, while `COALESCE(event_ts, checkpoint_ts, ts)` remains the compatibility read path during rollout

后续建议将 Trade Fact 语义升级为：
- `checkpoint`
- `checkpoint_ts`
- `event_ts`
- `ingested_at`
- `pool_id`
- `side`
- `price_raw` / `price_norm`
- `base_sz_raw` / `base_sz_norm`
- `quote_sz_raw` / `quote_sz_norm`
- `maker_bm`
- `taker_bm`
- `tx_digest`
- `event_seq`
- `event_index`
- `package_id`
- `module`
- `event_name`
- `raw_event`

### 5.5 side 语义

**Current**
- `side = buy` 表示 taker 为买方
- `side = sell` 表示 taker 为卖方

这不是 maker 方向，也不是订单簿静态挂单方向；它表示本次成交里的 taker 侧方向。

---

## 6. Order Lifecycle Fact：`db_order_events`

### 6.1 作用

`db_order_events` 用于表示非成交类订单生命周期事件。

当前主要服务于：
- `GET /v1/deepbook/pools/:pool_id/execution/lifecycle`

### 6.2 主键与幂等性

**Current**
- 主键：`(tx_digest, event_seq)`

### 6.3 当前支持的 `event_type`

**Current**
- `order_placed`
- `order_canceled`
- `order_modified`

**Target v2**
- `order_expired`

### 6.4 当前字段语义

| 字段 | Current 语义 | 备注 |
|---|---|---|
| `checkpoint` | Sui checkpoint 序号 | 已实现 |
| `ts` | checkpoint 时间代理值 | 当前不是事件 payload 的 `timestamp` |
| `pool_id` | pool object id | 已实现 |
| `event_type` | 生命周期事件类型 | 见上表 |
| `order_id` | 订单 id | 字符串返回 |
| `trader` | 下单地址 | 可为空 |
| `is_bid` | 是否买单方向 | 可为空 |
| `price` | 订单价格原始值 | 可为空 |
| `original_quantity` | 原始数量 | 事件相关字段 |
| `new_quantity` | 修改后数量 | 仅部分事件有值 |
| `canceled_quantity` | 取消数量 | 仅 canceled 类事件有值 |
| `tx_digest` | 交易摘要 | 已实现 |
| `event_seq` | 交易内事件序号 | 已实现 |
| `event_index` | 补充索引字段 | 当前通常为空 |
| `raw_event` | 原始事件 JSON | 当前写路径已支持落库；历史数据可能仍为空 |

### 6.5 Target v2 字段方向

Schema foundation status:
- `checkpoint_ts`, `event_ts`, `ingested_at`, `package_id`, `module`, and `event_name` are introduced by migration `004_add_event_contract_v2_columns.sql`
- Existing rows currently backfill `checkpoint_ts` / `event_ts` from legacy `ts`
- Newly ingested lifecycle rows can already populate `checkpoint_ts`, `event_ts`, `package_id`, `module`, `event_name`, and `raw_event`, while consumers should continue to treat `COALESCE(event_ts, checkpoint_ts, ts)` as the safe read path

后续建议补齐：
- `checkpoint_ts`
- `event_ts`
- `ingested_at`
- `raw_*` / `norm_*` 数量字段
- `package_id` / `module` / `event_name`
- `raw_event` 非空落库

---

## 7. Pool Rollup：`pool_metrics_1m`

### 7.1 分桶规则

**Current**
- `bucket_start = date_trunc('minute', ts)`（UTC）
- 当前 `ts` 来自 checkpoint 时间代理值

因此当前 K 线、分钟聚合的时间语义仍是“按 checkpoint 时间近似分桶”。

### 7.2 当前字段语义

| 字段 | Current 语义 |
|---|---|
| `pool_id` | pool id |
| `bucket_start` | 1 分钟桶起点 |
| `trades` | 该分钟成交笔数 |
| `volume_base` | 该分钟 base 成交量 |
| `volume_quote` | 该分钟 quote 成交量 |
| `maker_volume` | 当前实现中的 maker 成交额聚合 |
| `taker_volume` | 当前实现中的 taker 成交额聚合 |
| `fees_quote` | 预留字段，当前通常为空 |
| `avg_price` | 当前实现中与 vwap 一致 |
| `vwap` | `SUM(price * base_sz) / SUM(base_sz)` |
| `open_price` | 桶内第一笔价格 |
| `high_price` | 桶内最高价 |
| `low_price` | 桶内最低价 |
| `last_price` | 桶内最后一笔价格 |

### 7.3 排序与开收盘定义

**Current**
桶内事件按以下顺序近似定义开收盘：
- `ORDER BY ts ASC, checkpoint ASC, tx_digest ASC, event_seq ASC`

因此 `open_price` / `last_price` 的严格精度依赖于当前 `ts` 语义；在 `event_ts` 未落地前，它更适合作为轻量行情参考，而不是严格交易级时间序列基准。

### 7.4 Target v2

后续建议：
- 允许按 `event_ts` 分桶
- 明确 `price_raw` / `price_norm` 口径
- 明确 fees 与 execution metrics 的计算来源

---

## 8. BalanceManager Rollup：`bm_metrics_1m`

### 8.1 作用

`bm_metrics_1m` 描述某个 BalanceManager 在某个 pool、某一分钟内的成交贡献。

### 8.2 当前字段语义

| 字段 | Current 语义 |
|---|---|
| `bm_id` | BalanceManager id |
| `pool_id` | pool id |
| `bucket_start` | 1 分钟桶起点 |
| `trades` | 该 BM 在该桶内参与的成交笔数 |
| `volume_quote` | quote 成交额 |
| `maker_volume` | 该 BM 作为 maker 的 quote 成交额 |
| `taker_volume` | 该 BM 作为 taker 的 quote 成交额 |

说明：
- 当前数值仍为原始单位
- 一个成交可能同时影响 maker_bm 与 taker_bm 两侧聚合

---

## 9. Target v2 Metadata Tables

以下两类表现在已经以最小脚手架形式落地，但当前仍属于 **static seed / partial coverage** 阶段：

### 9.1 `asset_metadata`（Target v2）

Current foundation:
- migration `005_add_metadata_tables.sql` creates `asset_metadata`
- indexer startup seeds repo-local rows for `SUI` / `USDC`
- current source is static and intended for normalization scaffolding, not full on-chain discovery

字段：
- `asset_id`
- `coin_type`
- `symbol`
- `name`
- `decimals`
- `status`
- `source`
- `updated_at`

用途：
- 做 normalized values
- 为前端与分析接口提供直接可消费的资产信息

### 9.2 `pool_metadata`（Target v2）

Current foundation:
- migration `005_add_metadata_tables.sql` creates `pool_metadata`
- current seed only includes one known mainnet `SUI/USDC` mapping from existing repo-local bootstrap data
- testnet pool rows are intentionally not guessed until verified identifiers are curated

字段：
- `pool_id`
- `base_asset_id`
- `quote_asset_id`
- `package_id`
- `status`
- `updated_at`

用途：
- 明确 pool 与 base / quote 的映射
- 为 candles、ranking、execution summary 提供元数据支撑

---

## 10. API 契约

## 10.1 `GET /v1/deepbook/pools/:pool_id/metrics`

### Current 输出语义

返回窗口级聚合指标：
- `pool_id`
- `window`
- `start_ts`
- `end_ts`
- `trades`
- `volume_base`
- `volume_quote`
- `maker_volume`
- `taker_volume`
- `fees_quote`
- `avg_price`
- `vwap`
- `last_price`

说明：
- 当前窗口支持：`1h`、`24h`
- `start_ts` / `end_ts` 为 API 计算窗口边界，不是数据库字段原样透传
- `vwap` 使用分钟聚合结果按 `volume_base` 加权再聚合

## 10.2 `GET /v1/deepbook/pools/:pool_id/candles`

### Current 输出语义

返回：
- `pool_id`
- `window`
- `interval`
- `start_ts`
- `end_ts`
- `candles[]`

其中 `candles[]` 包含：
- `bucket_start`
- `trades`
- `volume_base`
- `volume_quote`
- `open`
- `high`
- `low`
- `close`
- `vwap`

说明：
- 当前窗口支持：`1h`、`24h`、`7d`
- 当前 interval 支持：`1m`、`5m`、`15m`、`1h`
- 当 interval > `1m` 时，API 基于 `pool_metrics_1m` 二次聚合

## 10.3 `GET /v1/deepbook/pools/:pool_id/execution/summary`

### Current 输出语义

返回：
- `trades`
- `volume_base`
- `volume_quote`
- `buy_trades`
- `sell_trades`
- `avg_trade_notional`
- `vwap`
- `price_change_bps`
- `order_imbalance_bps`
- `execution_score`

当前公式：
- `avg_trade_notional = SUM(quote_sz) / COUNT(*)`
- `vwap = SUM(price * base_sz) / SUM(base_sz)`
- `price_change_bps = (last_price - first_price) / first_price * 10_000`
- `order_imbalance_bps = (buy_trades - sell_trades) / trades * 10_000`
- `execution_score` 为当前服务内部启发式评分，不应视为协议官方指标

时间口径：
- 当前 summary 的窗口过滤与首尾价格计算均基于 `COALESCE(event_ts, checkpoint_ts, ts)`

## 10.4 `GET /v1/deepbook/pools/:pool_id/execution/lifecycle`

### Current 输出语义

返回：
- `pool_id`
- `window`
- `event_type`
- `count`
- `next_cursor`
- `events[]`

`events[]` 包含：
- `checkpoint`
- `ts_ms`
- `pool_id`
- `event_type`
- `order_id`
- `trader`
- `is_bid`
- `price`
- `original_quantity`
- `new_quantity`
- `canceled_quantity`
- `tx_digest`
- `event_seq`

说明：
- `ts_ms` 当前来自 `COALESCE(event_ts, checkpoint_ts, ts)`
- `next_cursor` 仅当返回条数等于 limit 且非空时生成
- 当前支持按 `event_type` 过滤

## 10.5 `GET /v1/deepbook/pools/:pool_id/execution/fills`

### Current 输出语义

返回：
- `pool_id`
- `window`
- `count`
- `next_cursor`
- `fills[]`

`fills[]` 包含：
- `checkpoint`
- `ts_ms`
- `pool_id`
- `side`
- `price`
- `base_size`
- `quote_size`
- `maker_bm`
- `taker_bm`
- `tx_digest`
- `event_seq`

说明：
- 字段 `base_size` / `quote_size` 是 API 命名，数据库对应 `base_sz` / `quote_sz`
- `ts_ms` 当前来自 `COALESCE(event_ts, checkpoint_ts, ts)`

## 10.6 `GET /v1/deepbook/bm/:bm_id/volume`

### Current 输出语义

返回：
- `bm_id`
- `window`
- `start_ts`
- `end_ts`
- `total_volume_quote`
- `breakdown[]`

`breakdown[]` 包含：
- `pool_id`
- `volume_quote`
- `trades`

说明：
- 当前窗口支持：`24h`、`7d`
- 支持可选 `pool` 过滤

## 10.7 `WS /v1/deepbook/trades`

### Current 输出语义

单条消息格式：
- `type` = `trade`
- `ts_ms`
- `pool_id`
- `side`
- `price`
- `base_sz`
- `quote_sz`
- `maker_bm`
- `taker_bm`
- `tx_digest`
- `event_seq`
- `checkpoint`

当前行为：
- 建连后先发送最近 100 条成交（按时间从旧到新）
- 之后轮询数据库并持续推送新成交
- 服务端会周期性发送 `ping`
- 当前支持 `pool` 过滤
- `ts_ms` 当前来自 `COALESCE(event_ts, checkpoint_ts, ts)`；实时增量游标仍按 checkpoint 顺序推进

---

## 11. 当前已知差距

以下是当前实现与 `v2` 目标契约之间最重要的差距：

1. 历史数据中的 `raw_event` 可能仍为空，但当前写路径已经可以为新写入事件持久化该字段
2. `ts` 当前是 checkpoint 时间代理值，不是正式的 `event_ts`
3. `asset_metadata` / `pool_metadata` 已有最小静态脚手架，但覆盖面仍有限，尚未实现链上自动发现
4. 缺少 normalized 数值字段
5. `OrderExpired` 尚未进入主索引流程
6. 当前 `execution_score` 是内部启发式指标，需要在对外文档中避免被误解为协议原生指标

---

## 12. 兼容与演进策略

Migration rollout note:
- Migration `004_add_event_contract_v2_columns.sql` is optimized for the current self-hosted rollout path, not for zero-downtime large-table production migration
- For large live datasets, prefer a staged rollout: add nullable columns, backfill in batches, create concurrent indexes, then switch writers/readers


建议后续按以下方式演进，尽量减少破坏性升级：

### 12.1 字段演进策略
- 保留当前字段一段兼容期
- 新增更精确字段时优先追加，而不是直接替换
- 例如：
  - 保留 `ts`
  - 新增 `checkpoint_ts` / `event_ts`
  - 最终再逐步废弃歧义字段

### 12.2 数值演进策略
- 保留当前 raw 字段
- 新增 `*_norm` 字段
- 不建议直接把当前 `price` / `base_sz` / `quote_sz` 语义偷偷改成 normalized

### 12.3 API 演进策略
- 当前公开接口保持可用
- 当新增 v2 字段时优先做向后兼容扩展
- 若必须 breaking change，应在 README / docs 中显式标注版本边界

---

## 13. 推荐的下一步落地顺序

与 `docs/FEATURE_BACKLOG.md` 对齐，建议优先完成：

1. `F-002 数据契约 v2`
2. `F-003 Raw Event 持久化`
3. `F-004 Asset / Pool Metadata`
4. `F-005 双时间字段支持`
5. `F-006 Canonical Trade Fact 升级`
6. `F-007 Order Lifecycle 全量建模`

当这几项完成后，本文档就可以从“目标契约 + 当前兼容说明”逐步转成“正式上线契约”。
