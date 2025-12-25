# DeepBook 数据服务（中文文档）

自托管的 DeepBook 协议数据后端，提供交易事件索引、分钟级指标计算，以及 REST / WebSocket 数据服务。

## 功能概览
- 交易事实表（`db_events`）存储 DeepBook 成交事件
- 1 分钟滚动指标（池维度 / BalanceManager 维度）
- 基于检查点的增量抓取，支持回放纠错与幂等
- RPC 主备容错、退避重试
- Go API 提供 REST 与 WebSocket 推送
- Docker Compose 一键部署

## 快速开始（Docker）

```powershell
# 在仓库根目录执行
docker compose -f docker/docker-compose.yml up -d --build

# API: http://localhost:8080
# Postgres: localhost:5432（user: sui, pass: sui, db: deepbook_indexer）
```

### 镜像加速（可选，解决拉取慢/失败）

Docker Desktop → Settings → Docker Engine，将下方 JSON 片段加入并 Apply & Restart：

```json
{
  "registry-mirrors": [
    "https://docker.m.daocloud.io"
  ]
}
```

重启后可用以下命令快速自检：

```powershell
Test-NetConnection auth.docker.io -Port 443
docker login
docker pull alpine:latest
```


启动后会：
1. 拉起 PostgreSQL
2. 自动执行迁移（migrations/*.sql）
3. 索引 DeepBook（默认 Mainnet）成交事件
4. 暴露 API 服务在 http://localhost:8080

## 配置说明（必需环境变量）

```dotenv
# 数据库连接串（本地）
DATABASE_URL=postgresql://sui:sui@localhost:5432/deepbook_indexer

# 选择一个网络（与包 ID 匹配）
# RPC_API_URL=https://fullnode.mainnet.sui.io:443
RPC_API_URL=https://fullnode.testnet.sui.io:443

# DeepBookV3 包 ID（与所选网络匹配）
# Mainnet: 0x00c1a56ec8c4c623a848b2ed2f03d23a25d17570b670c22106f336eb933785cc
# Testnet: 0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc
DEEPBOOK_PACKAGE_ID=...

# 可选：运行参数
INDEXER_POLL_INTERVAL_MS=1000
INDEXER_RPC_TIMEOUT_MS=10000
RUST_LOG=info
API_LISTEN_ADDR=0.0.0.0:8080
LOG_LEVEL=info
```

## 本地运行（非 Docker）

```powershell
# 手动初始化数据库迁移
psql "postgresql://sui:sui@localhost:5432/deepbook_indexer" -f migrations/init.sql

# 运行 API（Go）
$env:DATABASE_URL = "postgresql://sui:sui@localhost:5432/deepbook_indexer"
cd api-go
go run cmd/api/main.go

# 运行索引器（Rust）
cd ../indexer
$env:DATABASE_URL = "postgresql://sui:sui@localhost:5432/deepbook_indexer"
$env:RPC_API_URL = "https://fullnode.testnet.sui.io:443"
$env:DEEPBOOK_PACKAGE_ID = "0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc"
cargo run --package deepbook-indexer-indexer --bin deepbook-indexer-indexer -- run
```

## API 端点
- GET /health
- GET /v1/deepbook/pools/:pool_id/metrics?window=1h
- GET /v1/deepbook/bm/:bm_id/volume?window=24h
- WS  /v1/deepbook/trades?pool={pool_id}

## API 使用

```bash
# 池子指标（1 小时窗口）
curl "http://localhost:8080/v1/deepbook/pools/{pool_id}/metrics?window=1h"

# BalanceManager 成交量（24 小时窗口）
curl "http://localhost:8080/v1/deepbook/bm/{bm_id}/volume?window=24h"

# WebSocket 成交流
wscat -c "ws://localhost:8080/v1/deepbook/trades?pool={pool_id}"
```

含鉴权的 WebSocket 示例（如启用）：

```bash
wscat -H "Authorization: Bearer <API_SINGLE_KEY>" -c "ws://localhost:8080/v1/deepbook/trades?pool={pool_id}"
```

### 参数说明

- **window（池子指标）**：允许 `1h`、`24h`；默认 `1h`。
- **window（BM 成交量）**：允许 `24h`、`7d`；默认 `24h`。
- **pool 筛选**：可选，逗号分隔的池子 ID。BM 成交量（`?pool=POOL1,POOL2`）与 WebSocket 成交流（`?pool=POOL1,POOL2`）均支持。
- **鉴权（可选）**：如启用，需携带 `Authorization: Bearer <API_SINGLE_KEY>`。错误返回 `{ "error": "unauthorized" }`（HTTP 401）。

### 行为说明

- 非法 `window` 取值将回退到默认值（不返回错误）。

## WebSocket 示例

```bash
# 订阅某池子的成交事件
wscat -c "ws://localhost:8080/v1/deepbook/trades?pool={pool_id}"

# 可选：如启用鉴权，携带 Header（示例）
# Authorization: Bearer <API_SINGLE_KEY>
```

## 常见问题
- 启动报错缺少 `DATABASE_URL`：请设置环境变量或通过 Docker Compose 注入。
- 没有数据的情况：索引器正常运行但近期无成交事件，等待新的检查点即可。

## API 返回示例

池子指标（1h）：

```json
{
  "pool_id": "0xPOOL...",
  "window": "1h",
  "start_ts": "2025-12-25T09:00:00Z",
  "end_ts": "2025-12-25T10:00:00Z",
  "trades": 1234,
  "volume_base": "456.789",
  "volume_quote": "98765.4321",
  "maker_volume": "200.000",
  "taker_volume": "256.789",
  "vwap": "215.4321",
  "last_price": "217.00"
}
```

BalanceManager 成交量（24h）：

```json
{
  "bm_id": "0xBM...",
  "window": "24h",
  "start_ts": "2025-12-24T10:00:00Z",
  "end_ts": "2025-12-25T10:00:00Z",
  "total_volume_quote": "123456.789",
  "breakdown": [
    { "pool_id": "0xPOOL1", "volume_quote": "50000.000", "trades": 321 },
    { "pool_id": "0xPOOL2", "volume_quote": "73456.789", "trades": 456 }
  ]
}
```

WebSocket 成交事件：

```json
{
  "type": "trade",
  "ts_ms": 1766640000000,
  "pool_id": "0xPOOL...",
  "side": "buy",
  "price": "215.43",
  "base_sz": "1.2345",
  "quote_sz": "265.89",
  "maker_bm": "0xMAKERBM...",
  "taker_bm": "0xTAKERBM...",
  "tx_digest": "5Vjk...",
  "event_seq": 42,
  "checkpoint": 1234567
}
```

## 文档
- **[docs/USAGE.md](docs/USAGE.md)** - 最简使用指南
- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** - 系统架构

## 架构

```
Sui 区块链（Mainnet/Testnet）
	   ↓
   索引器（Rust）
   - 基于检查点的增量摄取
   - DeepBook 事件筛选
   - 1 分钟滚动指标计算
	   ↓
   PostgreSQL
   - db_events（交易事实）
   - pool_metrics_1m
   - bm_metrics_1m
	   ↓
   API 服务（Go）
   - REST 接口
   - WebSocket 推送
```

## 项目结构

```
├── api-go/          # Go API 服务
├── indexer/         # Rust 索引器
├── storage/         # 数据库模型与查询
├── common/          # 共享配置与类型
├── migrations/      # PostgreSQL 迁移脚本
├── docker/          # Docker 配置
└── docs/            # 文档
```

## 环境要求

- Docker 与 Docker Compose
- （可选）Go 1.21+ 用于本地 API 开发
- （可选）Rust 1.75+ 用于本地索引器开发

## 配置（docker-compose）

默认连接到 Testnet。如需自定义，请编辑 `docker/docker-compose.yml`：

```yaml
environment:
  RPC_API_URL: https://fullnode.testnet.sui.io:443
  DEEPBOOK_PACKAGE_ID: "0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc"  # Testnet DeepBookV3
```

切换到 Mainnet：

```yaml
environment:
  RPC_API_URL: https://fullnode.mainnet.sui.io:443
  DEEPBOOK_PACKAGE_ID: "0x00c1a56ec8c4c623a848b2ed2f03d23a25d17570b670c22106f336eb933785cc"  # Mainnet DeepBookV3
```

## 重放与数据修正

参见 [docs/USAGE.md](docs/USAGE.md) 获取最简命令。可按需补充高级回放指南。

## 许可与支持
- 许可：MIT（见仓库 LICENSE）
- 文档：见 docs/ 目录
- 问题反馈：GitHub Issues
