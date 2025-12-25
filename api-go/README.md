# DeepBook Data API (Go)

Go 服务提供 REST 与 WebSocket 接口，查询 PostgreSQL 中的 DeepBook 池/BM 指标与交易流。

## 启动

```bash
cd api-go
export DATABASE_URL="postgres://user:pass@localhost:5432/dbname"
export API_LISTEN_ADDR="0.0.0.0:8080"
go run cmd/api/main.go
```

## 环境变量

- `DATABASE_URL`：PostgreSQL 连接字符串（必填）
- `API_LISTEN_ADDR`：监听地址，默认 `0.0.0.0:8080`
- `WS_PING_INTERVAL_SEC`：WebSocket 心跳间隔（秒），默认 15
- `API_SINGLE_KEY`：可选单 key 鉴权，设置后需 `Authorization: Bearer <key>`

## API 端点

### REST
- `GET /health` - 健康检查
- `GET /v1/deepbook/pools/{pool_id}/metrics?window=1h|24h` - 池指标
- `GET /v1/deepbook/bm/{bm_id}/volume?window=24h|7d&pool=...` - BM 成交量

### WebSocket
- `WS /v1/deepbook/trades?pool=poolA,poolB` - 交易流（按池过滤）
