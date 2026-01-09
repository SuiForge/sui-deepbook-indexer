//! DeepBook event structures for BCS deserialization
//!
//! These structures match the Move events emitted by DeepBook contracts.
//! Used for type-safe BCS deserialization.

use serde::{Deserialize, Serialize};
use sui_types::base_types::ObjectID;

use crate::config::DeepbookEnv;

/// Trait for Move struct event types
pub trait MoveStruct {
    const MODULE: &'static str;
    const NAME: &'static str;

    /// Check if an event type matches this struct
    fn matches_event_type(
        event_type: &sui_types::event::Event,
        env: DeepbookEnv,
    ) -> bool {
        let packages = env.parse_package_bytes();

        packages.iter().any(|pkg| {
            *pkg == ObjectID::from(event_type.type_.address)
                && event_type.type_.module.as_str() == Self::MODULE
                && event_type.type_.name.as_str() == Self::NAME
        })
    }
}

/// OrderFilled event - emitted when an order is filled
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderFilled {
    pub pool_id: ObjectID,
    pub maker_order_id: u128,
    pub taker_order_id: u128,
    pub maker_client_order_id: u64,
    pub taker_client_order_id: u64,
    pub price: u64,
    pub taker_is_bid: bool,
    pub taker_fee: u64,
    pub taker_fee_is_deep: bool,
    pub maker_fee: u64,
    pub maker_fee_is_deep: bool,
    pub base_quantity: u64,
    pub quote_quantity: u64,
    pub maker_balance_manager_id: ObjectID,
    pub taker_balance_manager_id: ObjectID,
    pub timestamp: u64,
}

impl MoveStruct for OrderFilled {
    const MODULE: &'static str = "order_info";
    const NAME: &'static str = "OrderFilled";
}

impl OrderFilled {
    /// Convert to the database event row
    pub fn to_db_row(
        &self,
        checkpoint: i64,
        checkpoint_ts_ms: i64,
        tx_digest: &str,
        event_seq: i32,
    ) -> crate::DbEventRow {
        use chrono::{TimeZone, Utc};
        use rust_decimal::Decimal;

        let side = if self.taker_is_bid { "buy" } else { "sell" };
        let ts = Utc.timestamp_millis_opt(checkpoint_ts_ms).single().unwrap_or_else(Utc::now);

        crate::DbEventRow {
            checkpoint,
            ts,
            pool_id: self.pool_id.to_string(),
            side: side.to_string(),
            price: Decimal::from(self.price),
            base_sz: Decimal::from(self.base_quantity),
            quote_sz: Decimal::from(self.quote_quantity),
            maker_bm: Some(self.maker_balance_manager_id.to_string()),
            taker_bm: Some(self.taker_balance_manager_id.to_string()),
            tx_digest: tx_digest.to_string(),
            event_seq,
            event_index: None,
            raw_event: None,
        }
    }
}

/// OrderPlaced event - emitted when a new order is placed
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderPlaced {
    pub balance_manager_id: ObjectID,
    pub pool_id: ObjectID,
    pub order_id: u128,
    pub client_order_id: u64,
    pub trader: sui_sdk_types::Address,
    pub price: u64,
    pub is_bid: bool,
    pub placed_quantity: u64,
    pub expire_timestamp: u64,
    pub timestamp: u64,
}

impl MoveStruct for OrderPlaced {
    const MODULE: &'static str = "order_info";
    const NAME: &'static str = "OrderPlaced";
}

/// OrderCanceled event - emitted when an order is canceled
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderCanceled {
    pub balance_manager_id: ObjectID,
    pub pool_id: ObjectID,
    pub order_id: u128,
    pub client_order_id: u64,
    pub trader: sui_sdk_types::Address,
    pub price: u64,
    pub is_bid: bool,
    pub original_quantity: u64,
    pub base_asset_quantity_canceled: u64,
    pub timestamp: u64,
}

impl MoveStruct for OrderCanceled {
    const MODULE: &'static str = "order";
    const NAME: &'static str = "OrderCanceled";
}

/// OrderModified event - emitted when an order is modified
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderModified {
    pub balance_manager_id: ObjectID,
    pub pool_id: ObjectID,
    pub order_id: u128,
    pub client_order_id: u64,
    pub trader: sui_sdk_types::Address,
    pub price: u64,
    pub is_bid: bool,
    pub previous_quantity: u64,
    pub filled_quantity: u64,
    pub new_quantity: u64,
    pub timestamp: u64,
}

impl MoveStruct for OrderModified {
    const MODULE: &'static str = "order";
    const NAME: &'static str = "OrderModified";
}

/// OrderExpired event - emitted when an order expires
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderExpired {
    pub balance_manager_id: ObjectID,
    pub pool_id: ObjectID,
    pub order_id: u128,
    pub client_order_id: u64,
    pub trader: sui_sdk_types::Address,
    pub price: u64,
    pub is_bid: bool,
    pub original_quantity: u64,
    pub base_asset_quantity_canceled: u64,
    pub timestamp: u64,
}

impl MoveStruct for OrderExpired {
    const MODULE: &'static str = "order_info";
    const NAME: &'static str = "OrderExpired";
}

/// BalanceEvent - emitted on balance changes
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BalanceEvent {
    pub balance_manager_id: ObjectID,
    pub asset: String,
    pub amount: u64,
    pub deposit: bool,
}

impl MoveStruct for BalanceEvent {
    const MODULE: &'static str = "balance_manager";
    const NAME: &'static str = "BalanceEvent";
}
