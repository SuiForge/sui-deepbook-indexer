# 数据契约（v1）

本仓库的开源版目前聚焦 **DeepBook Core 的成交事件**（`OrderFilled`），并提供：
- 成交事实表：`db_events`
- 1 分钟聚合：`pool_metrics_1m`、`bm_metrics_1m`
- API：指标查询、K 线（OHLCV）、WebSocket 成交流

> 重要：本版本大部分数值字段来自链上事件的原始值，通常是“最小单位”（未按 decimals 归一化）。如果你需要可直接交易/展示的“人类单位”，需要配合 asset 元数据（decimals）做转换（后续会补齐）。

## 1) 事件过滤范围

- `DEEPBOOK_PACKAGE_ID`：用于匹配 RPC 返回的 `event.package_id`（支持逗号/空格分隔多个 package id）。
- `DEEPBOOK_EVENT_TYPE`：用于匹配 `event.event_type`（默认 `OrderFilled`，使用 `contains` 子串匹配）。

## 2) 表：`db_events`（成交事实）

主键：`(tx_digest, event_seq)`（幂等、可回放）

字段语义：
- `checkpoint`: Sui checkpoint 序号
- `ts`: **checkpoint 的时间戳**（`CheckpointSummary.timestamp`），不是 `OrderFilled.timestamp`
- `pool_id`: DeepBook pool object id
- `side`: `buy`/`sell`，当前由 `OrderFilled.taker_is_bid` 推导（表示 taker 方向：`true=buy`，`false=sell`）
- `price`: 事件里的价格字段（原始值）
- `base_sz`: `base_quantity`（原始值）
- `quote_sz`: `quote_quantity`（原始值）
- `maker_bm` / `taker_bm`: maker / taker 的 BalanceManager object id（可选）
- `raw_event`: 事件完整 JSON（便于追溯/纠错/未来扩展）

## 3) 表：`pool_metrics_1m`（池维度 1 分钟聚合）

分桶规则：
- `bucket_start = date_trunc('minute', ts)`（UTC）

聚合字段：
- `trades`: 成交笔数
- `volume_base`, `volume_quote`: 分桶内 base/quote 成交量（原始单位）
- `vwap`: `SUM(price * base_sz) / SUM(base_sz)`（当 `SUM(base_sz)=0` 时为 null）
- `last_price`: 分桶内最后一笔成交价（按写入时的事件顺序）
- `open_price`, `high_price`, `low_price`: 分桶内开/高/低

> 注意：因为 `ts` 来自 checkpoint，而不是每笔成交的链上 `timestamp` 字段，严格意义上的开/收盘是“按本服务时间序”近似，适合作为轻量行情参考。需要更精确时间线建议扩展入库 `OrderFilled.timestamp` 并以此分桶。

## 4) 表：`bm_metrics_1m`（BalanceManager 维度 1 分钟聚合）

- `volume_quote`: 该 BM 在该池子的成交额（原始单位）
- `maker_volume`: 该 BM 作为 maker 的成交额（原始单位）
- `taker_volume`: 该 BM 作为 taker 的成交额（原始单位）

## 5) API 输出约定

- REST/WS 输出的数值（`price/base_sz/quote_sz/volume_*`）均为字符串（decimal），避免精度丢失。
- 本版本未提供 decimals 归一化字段；请按业务自行转换。

