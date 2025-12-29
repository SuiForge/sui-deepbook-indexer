# DeepBook v3 事件清单（按 Move 包）

本仓库当前的索引器默认只抓取 **DeepBook Core** 的成交事件（`OrderFilled`），用于落库到 `db_events` 并生成 `pool_metrics_1m` / `bm_metrics_1m`。

如果你要扩展索引范围（例如把下单/撤单/保证金相关事件也入库），下面是 `../deepbookv3/packages/*/sources` 里 **会被 `event::emit(...)` 发出来的主要事件结构体**（按包/模块归类，便于选型）。

> 说明：事件名以 Move `struct` 名为准；泛型事件会带类型参数（例如 `PoolCreated<Base, Quote>`）。

## DeepBook Core（`packages/deepbook`）

### 交易 / 订单簿
- `OrderFilled`：撮合成交（核心 trade/volume 来源）
- `OrderPlaced`：挂单进入订单簿
- `OrderExpired`：挂单过期
- `OrderFullyFilled`：订单完全成交
- `OrderCanceled`：撤单
- `OrderModified`：改单
- `OrderInfo`：订单信息快照（`event::emit(*self)`）

### 池子 / 参数 / Referral
- `PoolCreated<Base, Quote>`：创建交易池
- `BookParamsUpdated<Base, Quote>`：tick/lot/min_size 等参数更新
- `DeepBurned<Base, Quote>`：DEEP 销毁（与费率/激励相关）
- `ReferralClaimed`：推荐奖励领取
- `ReferralFeeEvent`：推荐费分配/累计

### BalanceManager 资金与 Referral 绑定
- `BalanceManagerEvent`：创建 BalanceManager
- `BalanceEvent`：充值/提现（deposit/withdraw）
- `DeepBookReferralCreatedEvent`：创建 referral
- `DeepBookReferralSetEvent`：referral 绑定到 balance_manager

### 治理 / 质押 / 返利 / 惩罚
- `StakeEvent`：质押/解除质押
- `ProposalEvent`：提交参数提案
- `VoteEvent`：投票
- `RebateEventV2`（以及历史的 `RebateEvent`）：返利领取
- `TradeParamsUpdateEvent`：交易参数更新（taker/maker fee、stake_required）
- `TakerFeePenaltyApplied`：EWMA/惩罚相关的 taker fee 调整
- `EpochData`：epoch 切换时的统计快照
- `Volumes`：epoch volumes（在 `History::reset_volumes` 时 emit）
- `EWMAUpdate`：EWMA 状态更新

### Vault / 价格（DEEP）
- `FlashLoanBorrowed`：闪电贷借出
- `PriceAdded`：DEEP 价格点更新

## DeepBook Margin（`packages/deepbook_margin`）

### TPSL / 条件单
- `ConditionalOrderAdded`
- `ConditionalOrderCancelled`
- `ConditionalOrderExecuted`
- `ConditionalOrderInsufficientFunds`

### Registry / 配置
- `MaintainerCapUpdated`
- `DeepbookPoolRegistered`
- `DeepbookPoolUpdated`
- `DeepbookPoolConfigUpdated`
- `PauseCapUpdated`

### MarginPool（供给、费率、协议费）
- `MarginPoolCreated`
- `DeepbookPoolUpdated`（同名事件在不同模块里也会出现）
- `InterestParamsUpdated`
- `MarginPoolConfigUpdated`
- `SupplierCapMinted`
- `AssetSupplied`
- `AssetWithdrawn`
- `SupplyReferralMinted`
- `MaintainerFeesWithdrawn`
- `ProtocolFeesWithdrawn`
- `ProtocolFeesIncreasedEvent`
- `ReferralFeesClaimedEvent`

### MarginManager（抵押/借贷/清算）
- `MarginManagerCreatedEvent`
- `DepositCollateralEvent`
- `WithdrawCollateralEvent`
- `LoanBorrowedEvent`
- `LoanRepaidEvent`
- `LiquidationEvent`

## Margin Liquidation（`packages/margin_liquidation`）

- `LiquidationByVault`：由 liquidation vault 触发的清算事件

## 与本仓库配置的关系

- `DEEPBOOK_PACKAGE_ID`：用于过滤 `event.package_id` 的 **包 ID**（可以用逗号/空格分隔多个 ID）。想同时抓 Core + Margin，就需要把两个包的 package id 都写进去。
- `DEEPBOOK_EVENT_TYPE`：当前索引器只解析“成交事件”，默认匹配 `OrderFilled`（`contains` 子串匹配）。

