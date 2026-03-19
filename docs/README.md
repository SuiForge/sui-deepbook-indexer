# Documentation

## Current Docs

- `docs/USAGE.md`: how to run (Docker / local)
- `docs/ARCHITECTURE.md`: current architecture notes (legacy / v1-oriented)
- `docs/DATA_CONTRACT.md`: v2 目标契约 + 当前兼容说明
- `docs/DEEPBOOK_EVENTS.md`: DeepBook v3 event inventory (from Move sources)
- `docs/openapi/deepbook-v1.yaml`: current public REST OpenAPI contract

## Current Rollout State

- execution read-path now prefers `COALESCE(event_ts, checkpoint_ts, ts)` for fills/lifecycle `ts_ms`
- indexer write-path now persists dual timestamps, metadata columns, and `raw_event`
- metadata scaffolding (`asset_metadata`, `pool_metadata`) is now created by migration `005` and seeded on startup
- builder-facing metadata APIs are now available at `/v1/deepbook/assets`, `/v1/deepbook/pools`, `/v1/deepbook/pools/:pool_id/metadata`
- market discovery and DB-observed service status APIs are now available at `/v1/deepbook/markets/top` and `/v1/deepbook/status`
- Prometheus-compatible metrics are now available at `/metrics`
- OpenAPI contract and JS example client are now available for builder onboarding

## Planning / V2 Docs

- `docs/PRODUCT_PRD.md`: 产品定位、能力边界、目标用户、阶段目标
- `docs/ARCHITECTURE_V2.md`: 面向 builder-grade data infra 的 V2 架构设计
- `docs/FEATURE_BACKLOG.md`: 功能清单、优先级、验收标准与建议开发顺序

> 建议阅读顺序：`PRODUCT_PRD.md` → `ARCHITECTURE_V2.md` → `FEATURE_BACKLOG.md`


## Plans

- `docs/plans/2026-03-19-v2-foundation-implementation.md`: v2 数据契约基础阶段实施计划
- `docs/plans/2026-03-19-builder-mvp-phase1-implementation.md`: builder-grade MVP phase 1 实施计划
