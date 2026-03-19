# DeepBook Data Infrastructure v1 PRD

- Status: Proposed
- Last Updated: 2026-03-19
- Target Repo: `sui-deepbook-indexer`

## 1. Overview

当前的 `sui-deepbook-indexer` 已经不是一个玩具项目，而是一个具备雏形的 **DeepBook 数据服务 MVP**：
- 有 checkpoint 驱动的 Rust indexer
- 有 PostgreSQL 事实表与分钟级聚合
- 有 Go REST / WebSocket 服务
- 有 replay、幂等写入和多事件支持的基础

但从 grant / builder adoption 的视角看，它目前更像：

> 一个“可用的 DeepBook 数据索引器 MVP”

而我们希望把它升级成：

> 一个“面向 Sui Builder 的 DeepBook 数据基础设施服务”

这个 PRD 的目标不是把项目做成另一个交易前端或撮合系统，而是把它定义成一个 **可自托管、可重放、可扩展、数据语义稳定** 的 DeepBook 数据底座。

---

## 2. Product Positioning

### 2.1 One-line Positioning

**DeepBook Data Infrastructure for Sui Builders**

### 2.2 Product Value

为 Sui 生态里的开发者、量化团队、分析平台和基础设施团队提供：
- Canonical DeepBook trade / order lifecycle facts
- Normalized market data
- Replay / backfill / consistency tooling
- Stable REST / WebSocket data contract
- Self-hosted deployment capability

### 2.3 Why This Matters

随着 Sui 主线能力迭代，DeepBook 的核心价值不只是协议本身，而是围绕它的：
- 交易分析
- 行情展示
- 路由与做市策略
- 研究回测
- Builder 侧数据消费能力

当前生态里更稀缺的不是“又一个前端”，而是 **稳定、可复用、可解释的数据基础设施**。

---

## 3. Goals and Non-Goals

### 3.1 Goals

1. 建立 **DeepBook canonical fact layer**
2. 提供 **可直接被构建者消费的 normalized 数据接口**
3. 支持 **deterministic replay / backfill / consistency check**
4. 提供 **实时订阅能力**（REST + WebSocket）
5. 形成一套 **可自托管、可运维、可观测** 的服务形态
6. 将现有 MVP 升级成对 grant 更有说服力的 builder infra 项目

### 3.2 Non-Goals

1. 不做撮合引擎
2. 不做钱包、账户体系、交易执行前端
3. 不做泛化到所有 Sui 协议的通用 indexer（V1 不做）
4. 不做 oracle-grade 结算源
5. 不在 V1 追求完整跨协议聚合路由

---

## 4. Target Users

### 4.1 Sui Builder
需要快速接入 DeepBook 数据做：
- DEX frontend
- 交易监控
- 市场页 / 排行页
- 量化策略信号

### 4.2 Data / Research Developer
需要：
- 稳定事实表
- 可回放的数据源
- K 线 / VWAP / volume / maker/taker 维度数据

### 4.3 Infra / Analytics Team
需要：
- 自托管部署
- 清晰的数据契约
- 可观测的 lag / health / replay 能力

### 4.4 Grant Evaluator / Ecosystem Reviewer
关注：
- 是否贴近 Sui 主线路线
- 是否为生态提供通用能力
- 是否能被其他项目复用
- 是否具备工程完成度和迭代路线

---

## 5. Current State and Gaps

### 5.1 Current Strengths

当前项目已具备：
- Rust + Postgres + Go 的清晰服务拆分
- checkpoint 驱动 ingestion
- 幂等写入和 replay 基础
- 分钟级 rollup
- REST / WebSocket 对外服务
- DeepBook 多事件扩展的代码基础

### 5.2 Main Gaps

当前距离“builder-grade data infra”仍有明显差距：

1. **文档与实现漂移**
   - README、docker 配置、docs 之间存在不一致
2. **数据语义不完整**
   - decimals 未标准化
   - `checkpoint_ts` 与真实 `event_ts` 未分离
3. **追溯性不足**
   - `raw_event` 设计存在，但并未真正完整持久化
4. **元数据层缺失**
   - 缺少 asset / pool metadata 的统一建模
5. **控制面不足**
   - replay / backfill / consistency check 还未形成完整产品能力
6. **对外契约不够稳定**
   - 当前 API 更偏 MVP，而不是 builder-facing stable contract

---

## 6. Service Boundary

### 6.1 In Scope

- DeepBook 事件 ingestion
- 事实数据落库（trade / lifecycle）
- 元数据补齐（asset / pool）
- 数值标准化（raw + normalized）
- 分钟级 / 多窗口衍生指标
- REST / WebSocket 查询服务
- replay / backfill / consistency tooling
- 健康检查、lag、监控指标

### 6.2 Out of Scope

- 交易执行
- 用户账户 / 权限体系
- 深度撮合计算引擎
- 泛化到全链协议索引
- 完整跨协议路由器
- 面向终端用户的交易 UI

---

## 7. Product Capability Model

### 7.1 Canonical Fact Layer

必须定义并稳定输出以下事实层：
- Trade Fact
- Order Lifecycle Fact
- Pool Metadata
- Asset Metadata

### 7.2 Normalization Layer

必须同时保留：
- raw on-chain values
- normalized human-readable values

并明确：
- `checkpoint_ts`
- `event_ts`
- `ingested_at`

### 7.3 Derived Analytics Layer

必须支持：
- 1m candles / OHLCV
- pool metrics
- maker / taker volume
- execution summary
- top pools / rankings

### 7.4 Serving Layer

必须提供：
- REST 查询 API
- WebSocket 实时流
- 管理 / 状态 / lag 接口
- 稳定的数据契约版本

### 7.5 Ops / Control Plane

必须提供：
- replay
- backfill
- consistency check
- 监控指标
- structured logs
- self-hosted deployment

---

## 8. Functional Requirements

### FR-01 事实层完整性
系统必须能稳定产出 DeepBook 的 canonical trade facts，并支持后续扩展到完整 order lifecycle facts。

### FR-02 Raw + Normalized 双轨输出
所有关键数值字段必须保留原始链上值，并可生成标准化值供前端 / 分析直接消费。

### FR-03 元数据驱动
池与资产的 decimals、symbol、base / quote 关系必须由 metadata 模块统一管理，而不是散落在业务代码中。

### FR-04 双时间语义
系统必须能区分 checkpoint 时间与事件时间，避免把“索引到达时间”误当作“事件发生时间”。

### FR-05 可重放性
给定 checkpoint 范围，系统必须可 deterministic replay，并得到与全量重建一致的结果。

### FR-06 Builder-facing API
API 必须面向开发者使用场景设计，返回字段稳定、语义明确、易于集成。

### FR-07 Real-time Streaming
系统必须支持基于 pool / market 的实时交易订阅，并支持断线后的游标恢复策略。

### FR-08 可运维性
必须暴露 health、lag、processed checkpoint、source status、DB connectivity 等状态信息。

### FR-09 可测试性
核心 parsing、normalization、rollup、replay 必须具备 integration / e2e 覆盖。

### FR-10 自托管友好
本项目必须能通过 Docker Compose 或最小化配置完成自托管部署。

---

## 9. Non-Functional Requirements

### 9.1 Correctness
- 写入幂等
- replay 结果可重复
- 聚合结果可重建

### 9.2 Performance
- 正常链上吞吐下可持续追平
- 聚合回算具备受影响 bucket 局部重算能力
- API 在常见查询窗口下响应稳定

### 9.3 Reliability
- source 拉取失败有退避重试
- 组件失败可恢复
- checkpoint 进度可持久化

### 9.4 Observability
- 必须输出 metrics、logs、lag、source health
- 能定位数据断层、重复处理、回放异常

### 9.5 Compatibility
- 支持 testnet / mainnet
- 支持多 package id / 版本迁移窗口

### 9.6 Maintainability
- 配置统一
- 文档与实现一致
- 数据契约可版本化

---

## 10. Success Metrics

### 10.1 Product Metrics
- Builder 能在 1 小时内完成本服务的本地部署与首次查询
- 对外 API 字段语义明确，无需阅读源码才能理解
- 至少形成 2~3 个可直接复用的典型接入场景（行情页、研究分析、监控面板）

### 10.2 Data Metrics
- Indexer lag 可观测
- replay 后受影响区间的聚合结果与重建一致
- 关键事实字段完整率达到目标范围

### 10.3 Engineering Metrics
- 核心模块具备 integration / e2e tests
- 文档、docker、env 配置一致
- 发布时有明确的 data contract version

### 10.4 Ecosystem / Grant Metrics
- 明确定位为 Sui builder infra，而不是单点 demo
- 提供对生态项目复用价值的能力说明
- 能展示阶段性 roadmap 与交付计划

---

## 11. Release Plan

### Phase 0 - Foundation Alignment
目标：把当前 MVP 的配置、文档、契约统一起来。

重点：
- 配置收口
- 数据契约 v2
- raw_event 与双时间字段
- metadata 模块落地

### Phase 1 - Builder-grade MVP
目标：形成可对外宣传的 DeepBook 数据基础设施服务。

重点：
- canonical trade / lifecycle facts
- rollup engine v2
- replay / consistency tooling
- normalized REST / WS API
- observability

### Phase 2 - Advanced Analytics
目标：增强高级分析能力，提升 grant / adoption 价值。

重点：
- ranking / top markets
- execution quality
- slippage / liquidity analytics
- multi-source ingestion
- GraphQL / gRPC layer

---

## 12. Why This Project Is Grant-worthy

这个项目更适合以如下叙事去申请 grant：

> 为 Sui 生态提供可复用的 DeepBook 数据基础设施，而不是单一面向终端用户的交易产品。

它的 grant 价值来自：
- **贴合 Sui 主线路线**：服务 DeepBook 与 builder ecosystem
- **具备可复用性**：其他项目可以直接集成
- **具备 infra 属性**：数据契约、回放、可观测、自托管
- **具备演进空间**：可从 MVP 平滑升级到 builder-grade service

---

## 13. Appendix: Recommended Deliverables

建议把对外可交付物收敛为：
- `docs/PRODUCT_PRD.md`
- `docs/ARCHITECTURE_V2.md`
- `docs/FEATURE_BACKLOG.md`
- `docs/DATA_CONTRACT.md`（后续升级为 v2）
- `docker/docker-compose.yml`（与文档配置一致）

