# DeepBook Data Infrastructure Feature Backlog

- Status: Proposed
- Last Updated: 2026-03-19
- Related Docs:
  - `docs/PRODUCT_PRD.md`
  - `docs/ARCHITECTURE_V2.md`

## 1. How to Use This Document

本清单用于：
- 指导接下来的开发顺序
- 作为 grant 申请时的 roadmap / deliverables
- 让功能实现和产品定位保持一致

建议优先顺序：
- **P0**：先把基础能力和数据语义补齐
- **P1**：把服务做成 builder-facing 的可消费产品
- **P2**：再做高级分析与扩展入口

估算口径（单人 Rust / Go 开发者）：
- **S**：1~3 天
- **M**：4~7 天
- **L**：1~2 周

---

## 2. Milestones Overview

| Milestone | Scope | Goal |
|---|---|---|
| M1 | F-001 ~ F-005 | 统一配置、契约、时间语义、metadata |
| M2 | F-006 ~ F-008 | 建立 canonical facts 与 rollup v2 |
| M3 | F-009 ~ F-010 | 建立 replay / consistency / 测试闭环 |
| M4 | F-101 ~ F-106 | Builder-facing API、WS、监控、文档 |
| M5 | F-201 ~ F-204 | 高级分析与多源扩展 |

---

## 3. P0 - Foundation

### F-001 配置收口与文档统一
- Priority: P0
- Effort: S
- Why:
  - 当前 README、docs、docker、代码中的配置模型存在漂移
- Scope:
  - 统一环境变量命名
  - 明确 mainnet / testnet / source 配置方式
  - 更新 README、docs/USAGE、docker compose
- Acceptance Criteria:
  - 文档中的配置项与代码实际读取配置一致
  - Docker 默认配置能直接启动当前推荐模式
  - 不再出现旧版 `RPC_API_URL` / `DEEPBOOK_PACKAGE_ID` 与新版模型混用的歧义
- Dependencies: 无

### F-002 数据契约 v2
- Priority: P0
- Effort: M
- Why:
  - 当前 `DATA_CONTRACT.md` 仍偏向旧版单一成交事件视角
- Scope:
  - 定义 trade fact / lifecycle fact / metadata / rollup 的字段语义
  - 区分 raw vs normalized
  - 区分 `checkpoint_ts` vs `event_ts`
- Acceptance Criteria:
  - `docs/DATA_CONTRACT.md` 升级到 v2
  - 所有公开 API 字段能在文档中找到对应语义
  - 明确向后兼容策略或 breaking change 策略
- Dependencies: F-001

### F-003 Raw Event 持久化
- Priority: P0
- Effort: M
- Why:
  - 当前文档声称可追溯，但实现里 `raw_event` 未真正完整持久化
- Scope:
  - 支持将原始 event payload 以 JSON/BCS 可回放形式入库
  - 为 trade / lifecycle 事件统一保留 raw_event
- Acceptance Criteria:
  - 支持事件的 `raw_event` 字段非空
  - replay / debug 可直接使用 raw_event 辅助定位问题
  - 不影响现有幂等写入
- Dependencies: F-002

### F-004 Asset / Pool Metadata 模块
- Priority: P0
- Effort: L
- Why:
  - 没有 metadata 就很难输出真正可消费的 normalized data
- Scope:
  - 新增 `asset_metadata` / `pool_metadata`
  - 维护 decimals、symbol、base/quote 映射
  - 定义 metadata 更新策略
- Acceptance Criteria:
  - 关键 pool 均能关联到 base / quote 资产
  - decimals 可用于 normalized values 计算
  - API 可查询 pools / assets 基本信息
- Dependencies: F-001

### F-005 双时间字段支持
- Priority: P0
- Effort: M
- Why:
  - 当前大量逻辑默认用 checkpoint timestamp，时间语义不够准确
- Scope:
  - 增加 `checkpoint_ts`、`event_ts`、`ingested_at`
  - 约定 fallback 规则
  - candles / queries 明确使用哪个时间字段
- Acceptance Criteria:
  - 数据表和 API 中明确体现时间字段语义
  - `DATA_CONTRACT.md` 中说明 fallback 逻辑
  - 相关聚合计算不再默认混淆两类时间
- Dependencies: F-002, F-004

### F-006 Canonical Trade Fact 升级
- Priority: P0
- Effort: L
- Why:
  - 当前 trade facts 可用，但还不是稳定的 canonical contract
- Scope:
  - 升级 `db_events` 字段体系
  - 同时保留 raw / normalized 值
  - 明确 side、maker/taker、pool、package version 等字段
- Acceptance Criteria:
  - trade fact 可单独支撑 fills API 与 rollup 重建
  - 字段语义清晰，适合作为 builder-facing contract
  - 支持主要 DeepBook 成交查询场景
- Dependencies: F-003, F-004, F-005

### F-007 Order Lifecycle 全量建模
- Priority: P0
- Effort: L
- Why:
  - 当前代码和迁移已出现生命周期方向，但还没有形成完整能力层
- Scope:
  - 支持 placed / modified / canceled / expired 等事件
  - 建立 `order_lifecycle_events` 或等价事实层
  - 统一 cursor / event kind 语义
- Acceptance Criteria:
  - API 可按 pool / window / event_type 查询生命周期事件
  - 生命周期事件具备可追溯 raw_event
  - 文档明确每类事件的字段语义
- Dependencies: F-003, F-005

### F-008 Rollup Engine v2
- Priority: P0
- Effort: L
- Why:
  - 当前 rollup 有基础，但需要更稳定地服务 normalized analytics
- Scope:
  - 明确 rollup 输入只依赖 canonical facts
  - 支持局部 bucket 重算
  - 为 candles、summary、ranking 提供统一基础
- Acceptance Criteria:
  - trade facts 变化后仅重算受影响 bucket
  - replay 结果与在线重算一致
  - rollup 代码路径与事实层清晰解耦
- Dependencies: F-006, F-007

### F-009 Replay / Backfill / Consistency Check
- Priority: P0
- Effort: L
- Why:
  - 这是 infra 项目非常关键的可信度能力
- Scope:
  - replay checkpoint range
  - backfill missing range
  - consistency check 聚合与事实层的一致性
- Acceptance Criteria:
  - 支持指定 checkpoint 范围回放
  - 能产出 consistency check 结果摘要
  - 管理文档说明常见修复流程
- Dependencies: F-006, F-008

### F-010 Integration / E2E 测试
- Priority: P0
- Effort: L
- Why:
  - 当前测试深度还不足以支撑 builder-grade 叙事
- Scope:
  - parser tests
  - DB integration tests
  - replay / rollup correctness tests
  - Go API integration tests
- Acceptance Criteria:
  - 核心 ingestion -> DB -> API 路径具备集成测试
  - 至少覆盖 trade / lifecycle / replay 主路径
  - CI 可执行关键测试集
- Dependencies: F-006, F-007, F-008, F-009

---

## 4. P1 - Builder-facing Productization

### F-101 Normalized REST API
- Priority: P1
- Effort: M
- Why:
  - 当前 API 更偏内部结果暴露，尚未充分 builder-first
- Scope:
  - 统一 REST contract
  - 补齐 normalized fields
  - 标准化 pagination / cursor / error response
- Acceptance Criteria:
  - pool metrics / candles / fills / lifecycle / summary 返回结构一致
  - 文档中给出请求示例与字段语义
  - 支持主要查询窗口与过滤项
- Dependencies: F-006, F-007, F-008

### F-102 WebSocket 实时交易流 v2
- Priority: P1
- Effort: M
- Why:
  - 实时流是 builder 集成的重要入口
- Scope:
  - 统一 snapshot + live stream 模型
  - 支持 pool filter / cursor resume
  - 收敛 auth 与错误事件格式
- Acceptance Criteria:
  - 新连接可以拿到明确的初始快照策略
  - 断线重连后可按 cursor 恢复
  - 文档说明 WS contract 与错误处理
- Dependencies: F-006, F-010

### F-103 Pool Ranking / Top Markets
- Priority: P1
- Effort: M
- Why:
  - grant 演示和生态接入都需要更直观的市场维度能力
- Scope:
  - 按 volume / trades / activity 输出 top pools
  - 支持 1h / 24h / 7d 窗口
- Acceptance Criteria:
  - 提供 ranking API
  - 排名逻辑有明确窗口与排序规则
  - 可直接用于 market overview 页
- Dependencies: F-004, F-008

### F-104 Execution Quality Metrics
- Priority: P1
- Effort: M
- Why:
  - 这是区别于“普通 indexer”的高价值分析层能力
- Scope:
  - maker/taker 占比
  - 平均成交价 / VWAP
  - 基础执行质量指标
- Acceptance Criteria:
  - summary API 输出清晰的 execution metrics
  - 文档定义每个指标公式
  - 支持 pool 维度窗口查询
- Dependencies: F-006, F-008

### F-105 OpenAPI / Developer SDK
- Priority: P1
- Effort: M
- Why:
  - builder adoption 需要更低集成门槛
- Scope:
  - 产出 OpenAPI 文档
  - 提供最小可用 SDK 或示例 client
- Acceptance Criteria:
  - REST API 有 OpenAPI 描述
  - 至少有一个 JS/TS 或 Go 示例 client
  - README 有接入示例
- Dependencies: F-101

### F-106 Observability / Admin API
- Priority: P1
- Effort: M
- Why:
  - 自托管 infra 若没有状态面板能力，运营成本较高
- Scope:
  - health / status / lag
  - Prometheus metrics
  - admin replay / consistency trigger（可先只读）
- Acceptance Criteria:
  - 可以查询当前 processed checkpoint 与 lag
  - 暴露核心 metrics
  - 文档包含运维排障路径
- Dependencies: F-009, F-010

---

## 5. P2 - Advanced Extensions

### F-201 Orderbook Snapshot / Depth
- Priority: P2
- Effort: L
- Why:
  - 若后续对接交易前端或高级量化，需要深度数据能力
- Scope:
  - orderbook snapshot
  - depth aggregation
  - 指定级别输出
- Acceptance Criteria:
  - 能输出可消费的 depth snapshot
  - 更新策略与一致性语义明确
- Dependencies: F-007

### F-202 Slippage / Liquidity Analytics
- Priority: P2
- Effort: M
- Why:
  - 进一步增强分析价值与 grant story
- Scope:
  - 基础滑点估算
  - liquidity profile
  - market quality signals
- Acceptance Criteria:
  - 至少提供一种可解释的 slippage / liquidity 指标
  - 指标公式在文档中清晰定义
- Dependencies: F-201, F-104

### F-203 Multi-source Ingestion
- Priority: P2
- Effort: L
- Why:
  - 提升可用性与未来扩展性
- Scope:
  - 在 Remote Store 之外保留 gRPC / streaming 适配能力
  - 定义 source abstraction
- Acceptance Criteria:
  - source adapter 接口清晰
  - 可以切换至少两类 source 模式中的一种扩展实现
- Dependencies: F-001, F-009

### F-204 GraphQL / gRPC Query Layer
- Priority: P2
- Effort: L
- Why:
  - 面向不同 builder 类型提供更灵活的查询方式
- Scope:
  - GraphQL 或 gRPC 查询层
  - 面向高频消费者的契约定义
- Acceptance Criteria:
  - 至少实现一种新增查询层
  - 文档说明与 REST 的职责边界
- Dependencies: F-101, F-105

---

## 6. Recommended Build Order

### Week 1
- F-001 配置收口
- F-002 数据契约 v2

### Week 2
- F-003 Raw Event 持久化
- F-004 Metadata 模块
- F-005 双时间字段

### Week 3
- F-006 Canonical Trade Fact
- F-007 Lifecycle 建模

### Week 4
- F-008 Rollup Engine v2
- F-009 Replay / Consistency

### Week 5
- F-010 Integration / E2E 测试
- F-101 Normalized REST API

### Week 6
- F-102 WebSocket v2
- F-106 Observability / Admin API

### Week 7
- F-103 Pool Ranking
- F-104 Execution Quality Metrics

### Week 8
- F-105 OpenAPI / SDK
- 整理 grant 申请材料、演示视频、部署文档

---

## 7. Definition of Done

每个功能项完成时，至少应满足：
- 代码实现完成
- 文档更新完成
- 数据契约与字段语义明确
- 最少一层自动化验证存在
- README / docs 中能指导外部使用者理解与接入

---

## 8. Recommended Narrative for External Use

对外建议不要表述成：
- “我做了一个 DeepBook 索引器”

更建议表述成：
- “我在做一个面向 Sui Builder 的 DeepBook 数据基础设施服务，提供 canonical facts、normalized APIs、replay/backfill 和实时流能力。”

这会显著更贴近 grant / infra / ecosystem narrative。

