# Architecture Guide

本文档深入解释 DeepBook Data Service 的架构设计、数据流和关键决策。

## 📐 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Sui Blockchain (Mainnet)                 │
│                   (检查点驱动的事件源)                       │
└────────────────────────┬────────────────────────────────────┘
                         │
                         │ RPC (sui_rpc v0.1.1)
                         │ ├─ 主 RPC: fullnode.mainnet.sui.io
                         │ └─ 备用 RPC: 可选故障转移
                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Indexer (Rust / deepbook-indexer-indexer)        │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ run_indexer()                                        │   │
│  │ ├─ Checkpoint 轮询（可配置间隔）                     │   │
│  │ ├─ RPC 故障转移（主 → 备用）                        │   │
│  │ ├─ 指数退避重试（200ms ~ 30s，+抖动）              │   │
│  │ └─ Graceful shutdown (Ctrl+C)                      │   │
│  └──────────────────────────────────────────────────────┘   │
│                         │                                     │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ ingest_checkpoint()                                  │   │
│  │ ├─ 解析 DeepBook 事件（OrderFilled）                │   │
│  │ ├─ 提取字段（可配置，支持版本适配）                 │   │
│  │ ├─ 计算 1 分钟 Rollup 指标                          │   │
│  │ │  ├─ Pool 维度聚合                                 │   │
│  │ │  └─ BalanceManager 维度聚合                       │   │
│  │ └─ 原子数据库事务提交                               │   │
│  └──────────────────────────────────────────────────────┘   │
│                         │                                     │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ replay_range()（可选）                               │   │
│  │ ├─ 重新查询指定 checkpoint 范围的事件                │   │
│  │ ├─ 删除受影响的 rollup bucket                       │   │
│  │ └─ 重新计算并保存指标                               │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                               │
└────────────────┬────────────────────────────────────────────┘
                 │
                 │ 批量 INSERT/UPSERT
                 │ (单 checkpoint 事务)
                 ▼
┌─────────────────────────────────────────────────────────────┐
│              PostgreSQL (sqlx tokio)                          │
│                                                               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ indexer_state (状态追踪)                            │   │
│  │ ├─ processed_checkpoint: i64 (恢复点)              │   │
│  │ └─ updated_at: timestamp                            │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ db_events (原始交易事实)                            │   │
│  │ ├─ checkpoint: i64                                  │   │
│  │ ├─ ts: timestamp                                    │   │
│  │ ├─ pool_id: string                                 │   │
│  │ ├─ side: 'buy'|'sell'                              │   │
│  │ ├─ price: decimal                                  │   │
│  │ ├─ base_sz, quote_sz: decimal                      │   │
│  │ ├─ maker_bm, taker_bm: string (optional)          │   │
│  │ ├─ tx_digest: string                               │   │
│  │ └─ raw_event: json (完整事件数据)                  │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ pool_metrics_1m (1 分钟聚合)                        │   │
│  │ ├─ pool_id, bucket_start: 复合主键                 │   │
│  │ ├─ trades: 交易笔数                                 │   │
│  │ ├─ volume_base, volume_quote: 成交额                │   │
│  │ ├─ maker_volume, taker_volume: 分离统计            │   │
│  │ ├─ avg_price, vwap, last_price: 价格指标           │   │
│  │ └─ fees_quote: 手续费 (预留字段)                   │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                               │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ bm_metrics_1m (BalanceManager 维度)                │   │
│  │ ├─ (bm_id, pool_id, bucket_start): 复合主键        │   │
│  │ ├─ trades: 交易笔数                                 │   │
│  │ ├─ volume_quote: 成交额                             │   │
│  │ ├─ maker_volume, taker_volume: 分离统计            │   │
│  │ └─ 用于追踪 BalanceManager 的交易行为               │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                               │
└────────────────┬────────────────────────────────────────────┘
                 │
                 │ SELECT 查询 (可选缓存)
                 │
                 ▼
┌─────────────────────────────────────────────────────────────┐
│              API Server (Go / Gin)                            │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ REST Endpoints                                       │   │
│  │ ├─ GET /health (健康检查)                            │   │
│  │ ├─ GET /v1/deepbook/pools/:pool_id/metrics         │   │
│  │ │  └─ 查询池指标（可选时间窗口）                    │   │
│  │ ├─ GET /v1/deepbook/bm/:bm_id/volume              │   │
│  │ │  └─ 查询 BalanceManager 成交额                    │   │
│  │ └─ GET /v1/deepbook/trades (WebSocket 升级)      │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ WebSocket Streaming                                  │   │
│  │ ├─ 实时交易流推送                                   │   │
│  │ ├─ 支持 pool_id 过滤                               │   │
│  │ └─ 自动重连机制                                     │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Auth Middleware                                      │   │
│  │ ├─ API 密钥验证（可选）                             │   │
│  │ └─ 请求速率限制（可选）                             │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                               │
└─────────────────────────────────────────────────────────────┘
                         │
                         │ HTTP/WebSocket
                         ▼
                    ┌────────────┐
                    │  Clients   │
                    │ (Browser,  │
                    │  Desktop,  │
                    │  Mobile)   │
                    └────────────┘
```

---

## 🔄 数据流

### 1. 链上数据捕获流程

```
Sui Checkpoint N
    ↓
  ┌─────────────────────────────────────────┐
  │ RPC get_checkpoint(sequence=N)           │
  │ ├─ sequence_number                      │
  │ ├─ summary (timestamp, tx_count, etc)   │
  │ ├─ transactions[] {                     │
  │ │  ├─ digest: tx hash                   │
  │ │  └─ events[] {                        │
  │ │     ├─ package_id (过滤 DeepBook)    │
  │ │     ├─ event_type (过滤 OrderFilled)│
  │ │     └─ json (事件数据)                │
  │ │  }                                    │
  │ └─ }                                    │
  └─────────────────────────────────────────┘
    ↓
  ┌─────────────────────────────────────────┐
  │ parse_deepbook_trade()                  │
  │ ├─ 快速过滤 (package_id, event_type)  │
  │ ├─ 提取字段 {                          │
  │ │  ├─ pool_id (必需)                   │
  │ │  ├─ price (必需)                     │
  │ │  ├─ base_sz, quote_sz (必需)        │
  │ │  ├─ side (必需)                      │
  │ │  ├─ maker_bm, taker_bm (可选)       │
  │ │  └─ ...                              │
  │ └─ }                                    │
  └─────────────────────────────────────────┘
    ↓
  ┌─────────────────────────────────────────┐
  │ compute_rollups() (内存中)              │
  │ ├─ 按 (pool_id, bucket_start) 分组    │
  │ ├─ 计算聚合指标 {                      │
  │ │  ├─ 交易笔数                         │
  │ │  ├─ 成交量                           │
  │ │  ├─ maker/taker 分离统计             │
  │ │  ├─ VWAP (交易量加权平均价)          │
  │ │  └─ last_price                       │
  │ └─ }                                    │
  └─────────────────────────────────────────┘
    ↓
  ┌─────────────────────────────────────────┐
  │ 原子数据库事务:                         │
  │ ├─ INSERT INTO db_events (原始事件)    │
  │ ├─ UPSERT pool_metrics_1m              │
  │ ├─ UPSERT bm_metrics_1m                │
  │ └─ UPDATE indexer_state (checkpoint)   │
  └─────────────────────────────────────────┘
    ↓
  ✅ Checkpoint N 完成
```

### 2. API 查询流程

```
GET /v1/deepbook/pools/{pool_id}/metrics?window=1h
    ↓
┌─────────────────────────────┐
│ API Handler                 │
│ ├─ 验证参数 (pool_id, window│)
│ ├─ 计算时间范围            │
│ └─ 查询数据库               │
└─────────────────────────────┘
    ↓
┌─────────────────────────────┐
│ SELECT * FROM              │
│ pool_metrics_1m            │
│ WHERE pool_id = $1         │
│   AND bucket_start > now() │
│   - interval '1 hour'      │
│ ORDER BY bucket_start DESC │
└─────────────────────────────┘
    ↓
┌─────────────────────────────┐
│ JSON 响应                   │
│ {                           │
│   "metrics": [              │
│     {                       │
│       "bucket_start": "",   │
│       "trades": 123,        │
│       "volume_quote": "...", │
│       "vwap": "..."        │
│     },                      │
│     ...                     │
│   ]                         │
│ }                           │
└─────────────────────────────┘
```

---

## 🛡️ 容错机制

### 1. RPC 故障转移

```
fetch_checkpoint_with_fallback()
    ↓
┌──────────────────────────────────┐
│ 尝试 Primary RPC (主)            │
│ timeout: RPC_TIMEOUT_MS          │
└──────────────────────────────────┘
    │
    ├─ ✅ 成功 → 返回 checkpoint
    │
    ├─ ❌ 超时
    │  └─ 尝试 Fallback RPC (备)
    │      ├─ ✅ 成功 → 返回 checkpoint
    │      └─ ❌ 失败 → 返回错误
    │
    └─ ❌ 其他错误
       ├─ Code::NotFound (checkpoint 不存在) → 返回 None
       └─ 其他 → 尝试 Fallback
```

### 2. 指数退避重试

```
Backoff { base_ms, max_ms, attempt }

next_delay() 计算:
    shift = attempt.min(20)
    exp = base_ms * 2^shift
    capped = exp.min(max_ms)
    jitter = random(0, capped/10.min(250))
    
    delay = capped + jitter

示例（base=200ms, max=30s）:
    attempt=0 → 200ms    + jitter
    attempt=1 → 400ms    + jitter
    attempt=2 → 800ms    + jitter
    ...
    attempt=7 → 25600ms  + jitter (已接近 max)
    attempt=8+ → 30000ms + jitter (已达 max)
```

### 3. 优雅关闭

```
run_indexer()
    │
    ├─ tokio::signal::ctrl_c() 监听 Ctrl+C
    │
    └─ 每个 tokio::select! 都可以被打断:
       ├─ checkpoint 获取被打断 → break
       ├─ 等待中被打断 → break
       └─ 处理完当前 checkpoint 后，安全退出
```

---

## 💾 数据库设计

### Indexer State 追踪

```sql
CREATE TABLE indexer_state (
    id SMALLINT PRIMARY KEY DEFAULT 1,
    processed_checkpoint BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMP NOT NULL DEFAULT now()
);
```

**作用**：
- 记录已处理的最后一个 checkpoint
- 支持从中断点恢复
- 防止数据重复或遗漏

### 原始事件表（db_events）

```sql
CREATE TABLE db_events (
    id BIGSERIAL PRIMARY KEY,
    checkpoint BIGINT NOT NULL,
    ts TIMESTAMP NOT NULL,
    pool_id VARCHAR NOT NULL,
    side VARCHAR NOT NULL,  -- 'buy' or 'sell'
    price NUMERIC NOT NULL,
    base_sz NUMERIC NOT NULL,
    quote_sz NUMERIC NOT NULL,
    maker_bm VARCHAR,
    taker_bm VARCHAR,
    tx_digest VARCHAR NOT NULL,
    event_seq INTEGER NOT NULL,
    event_index INTEGER,
    raw_event JSONB,
    
    UNIQUE(tx_digest, event_seq),
    INDEX idx_pool_ts (pool_id, ts),
    INDEX idx_checkpoint (checkpoint)
);
```

**特点**：
- 幂等性：`UNIQUE(tx_digest, event_seq)` 防止重复
- 审计能力：保存原始 JSON 用于调试
- 查询性能：索引优化

### Rollup 指标表（pool_metrics_1m）

```sql
CREATE TABLE pool_metrics_1m (
    pool_id VARCHAR NOT NULL,
    bucket_start TIMESTAMP NOT NULL,
    trades BIGINT NOT NULL,
    volume_base NUMERIC NOT NULL,
    volume_quote NUMERIC NOT NULL,
    maker_volume NUMERIC NOT NULL,
    taker_volume NUMERIC NOT NULL,
    fees_quote NUMERIC,
    avg_price NUMERIC,
    vwap NUMERIC,  -- 交易量加权平均价
    last_price NUMERIC,
    
    PRIMARY KEY (pool_id, bucket_start),
    INDEX idx_bucket (bucket_start)
);
```

**特点**：
- 时间序列数据优化
- Maker/Taker 分离统计
- VWAP 用于流动性分析

---

## 🎯 关键设计决策

### 1. Checkpoint 驱动的索引

✅ **优势**：
- 原子性：按 checkpoint 粒度提交，保证一致性
- 恢复能力：知道已处理到哪个 checkpoint，支持快速恢复
- 顺序性：保证事件处理顺序

❌ **权衡**：
- 需要轮询，而不是实时推送
- 受 RPC 节点可用性影响

### 2. 1 分钟聚合粒度

✅ **优势**：
- 平衡存储和精度
- 足够细粒度用于市场分析
- 易于扩展到多个时间窗口

❌ **权衡**：
- 不适合超短期交易
- 需要预先计算

### 3. Pool + BalanceManager 双维度

✅ **优势**：
- 完整的市场透视
- 支持交易者风险追踪
- 流动性供应商监控

❌ **权衡**：
- 存储成本翻倍
- 计算复杂度增加

### 4. 内存 Rollup + 批量插入

✅ **优势**：
- 高效的计算（避免多次 DB 查询）
- 原子事务（一次提交多张表）
- 低延迟

❌ **权衡**：
- 单 checkpoint 失败需要重做
- 内存占用（通常 < 10MB）

---

## 📊 性能特性

### Indexer 吞吐量

```
假设条件:
- Sui blocktime: 350ms
- 平均交易数: 50 txs/checkpoint
- 平均 DeepBook 事件比例: 10%

吞吐量:
- 检查点: ~3/秒
- 交易: ~150/秒
- DeepBook 事件: ~15/秒
- 处理延迟: < 1 秒
```

### 内存占用

```
Indexer 内存:
- 基础: ~50 MB
- 单 checkpoint 缓存: ~5-10 MB
- 总计: < 100 MB

API 内存:
- 基础: ~30 MB
- 连接缓存: ~1 MB per concurrent connection
- 总计: < 500 MB
```

### 数据库大小估算

```
按月增长:
- db_events: 
  - 平均每秒 15 events
  - 每月: ~39M 行
  - 大小: ~10 GB (含索引)

- pool_metrics_1m:
  - 每个 pool: 60 * 24 * 30 = 43,200 行/月
  - 假设 500 个活跃 pool
  - 大小: ~100 MB

总计: ~10 GB/月 (可通过数据分片或归档优化)
```

---

## 🔐 安全考虑

### 1. 数据完整性
- ✅ 幂等 INSERT (UNIQUE 约束)
- ✅ 原子事务 (单 checkpoint 提交)
- ✅ 检查点追踪 (恢复和重放)

### 2. API 安全
- ✅ 身份验证 (API 密钥)
- ✅ 速率限制 (可选)
- ✅ 输入验证

### 3. RPC 安全
- ✅ 超时控制
- ✅ 故障转移
- ✅ 错误隔离

---

## 🚀 可扩展性

### 垂直扩展
- 增加数据库资源
- 增加 Indexer CPU
- 优化查询和索引

### 水平扩展
- 多个 Indexer 实例（从不同 checkpoint 开始）
- 分区数据库（按 pool_id）
- 多个 API 实例（无状态）
- 缓存层（Redis）

---

## 📖 相关文档

- [README.md](README.md) - 项目概述
- [DEPLOY.md](DEPLOY.md) - 部署指南
- [docs/pg_schema.md](docs/pg_schema.md) - 数据库详细设计
- [docs/openapi.yaml](docs/openapi.yaml) - API 规范
