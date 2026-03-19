//! DeepBook event structures for BCS deserialization
//!
//! These structures match the Move events emitted by DeepBook contracts.
//! Used for type-safe BCS deserialization.

use serde::{Deserialize, Serialize};
use sui_types::base_types::ObjectID;

use crate::config::DeepbookEnv;

fn timestamp_from_ms(ms: i64) -> chrono::DateTime<chrono::Utc> {
    use chrono::{TimeZone, Utc};

    Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
}

fn timestamp_from_u64_or(ms: u64, fallback: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    i64::try_from(ms)
        .ok()
        .map(timestamp_from_ms)
        .unwrap_or(fallback)
}

/// Trait for Move struct event types
pub trait MoveStruct {
    const MODULE: &'static str;
    const NAME: &'static str;

    /// Check if an event type matches this struct
    fn matches_event_type(event_type: &sui_types::event::Event, env: DeepbookEnv) -> bool {
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
        use chrono::Utc;
        use rust_decimal::Decimal;

        let side = if self.taker_is_bid { "buy" } else { "sell" };
        let checkpoint_ts = timestamp_from_ms(checkpoint_ts_ms);
        let event_ts = timestamp_from_u64_or(self.timestamp, checkpoint_ts);

        crate::DbEventRow {
            checkpoint,
            ts: checkpoint_ts,
            checkpoint_ts: Some(checkpoint_ts),
            event_ts: Some(event_ts),
            ingested_at: Utc::now(),
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
            package_id: None,
            module: Some(<Self as MoveStruct>::MODULE.to_string()),
            event_name: Some(<Self as MoveStruct>::NAME.to_string()),
            raw_event: serde_json::to_value(self).ok(),
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

impl OrderPlaced {
    pub fn to_order_event_row(
        &self,
        checkpoint: i64,
        checkpoint_ts_ms: i64,
        tx_digest: &str,
        event_seq: i32,
    ) -> crate::DbOrderEventRow {
        use chrono::Utc;
        use rust_decimal::Decimal;

        let checkpoint_ts = timestamp_from_ms(checkpoint_ts_ms);
        let event_ts = timestamp_from_u64_or(self.timestamp, checkpoint_ts);

        crate::DbOrderEventRow {
            checkpoint,
            ts: checkpoint_ts,
            checkpoint_ts: Some(checkpoint_ts),
            event_ts: Some(event_ts),
            ingested_at: Utc::now(),
            pool_id: self.pool_id.to_string(),
            event_type: "order_placed".to_string(),
            order_id: Some(self.order_id.to_string()),
            trader: Some(self.trader.to_string()),
            is_bid: Some(self.is_bid),
            price: Some(Decimal::from(self.price)),
            original_quantity: Some(Decimal::from(self.placed_quantity)),
            new_quantity: Some(Decimal::from(self.placed_quantity)),
            canceled_quantity: None,
            tx_digest: tx_digest.to_string(),
            event_seq,
            event_index: None,
            package_id: None,
            module: Some(<Self as MoveStruct>::MODULE.to_string()),
            event_name: Some(<Self as MoveStruct>::NAME.to_string()),
            raw_event: serde_json::to_value(self).ok(),
        }
    }
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

impl OrderCanceled {
    pub fn to_order_event_row(
        &self,
        checkpoint: i64,
        checkpoint_ts_ms: i64,
        tx_digest: &str,
        event_seq: i32,
    ) -> crate::DbOrderEventRow {
        use chrono::Utc;
        use rust_decimal::Decimal;

        let checkpoint_ts = timestamp_from_ms(checkpoint_ts_ms);
        let event_ts = timestamp_from_u64_or(self.timestamp, checkpoint_ts);

        crate::DbOrderEventRow {
            checkpoint,
            ts: checkpoint_ts,
            checkpoint_ts: Some(checkpoint_ts),
            event_ts: Some(event_ts),
            ingested_at: Utc::now(),
            pool_id: self.pool_id.to_string(),
            event_type: "order_canceled".to_string(),
            order_id: Some(self.order_id.to_string()),
            trader: Some(self.trader.to_string()),
            is_bid: Some(self.is_bid),
            price: Some(Decimal::from(self.price)),
            original_quantity: Some(Decimal::from(self.original_quantity)),
            new_quantity: None,
            canceled_quantity: Some(Decimal::from(self.base_asset_quantity_canceled)),
            tx_digest: tx_digest.to_string(),
            event_seq,
            event_index: None,
            package_id: None,
            module: Some(<Self as MoveStruct>::MODULE.to_string()),
            event_name: Some(<Self as MoveStruct>::NAME.to_string()),
            raw_event: serde_json::to_value(self).ok(),
        }
    }
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

impl OrderModified {
    pub fn to_order_event_row(
        &self,
        checkpoint: i64,
        checkpoint_ts_ms: i64,
        tx_digest: &str,
        event_seq: i32,
    ) -> crate::DbOrderEventRow {
        use chrono::Utc;
        use rust_decimal::Decimal;

        let checkpoint_ts = timestamp_from_ms(checkpoint_ts_ms);
        let event_ts = timestamp_from_u64_or(self.timestamp, checkpoint_ts);

        crate::DbOrderEventRow {
            checkpoint,
            ts: checkpoint_ts,
            checkpoint_ts: Some(checkpoint_ts),
            event_ts: Some(event_ts),
            ingested_at: Utc::now(),
            pool_id: self.pool_id.to_string(),
            event_type: "order_modified".to_string(),
            order_id: Some(self.order_id.to_string()),
            trader: Some(self.trader.to_string()),
            is_bid: Some(self.is_bid),
            price: Some(Decimal::from(self.price)),
            original_quantity: Some(Decimal::from(self.previous_quantity)),
            new_quantity: Some(Decimal::from(self.new_quantity)),
            canceled_quantity: None,
            tx_digest: tx_digest.to_string(),
            event_seq,
            event_index: None,
            package_id: None,
            module: Some(<Self as MoveStruct>::MODULE.to_string()),
            event_name: Some(<Self as MoveStruct>::NAME.to_string()),
            raw_event: serde_json::to_value(self).ok(),
        }
    }
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_filled_row_sets_v2_compatibility_fields() {
        let event = OrderFilled {
            pool_id: ObjectID::from_hex_literal("0x2").unwrap(),
            maker_order_id: 1,
            taker_order_id: 2,
            maker_client_order_id: 3,
            taker_client_order_id: 4,
            price: 100,
            taker_is_bid: true,
            taker_fee: 0,
            taker_fee_is_deep: false,
            maker_fee: 0,
            maker_fee_is_deep: false,
            base_quantity: 5,
            quote_quantity: 500,
            maker_balance_manager_id: ObjectID::from_hex_literal("0x3").unwrap(),
            taker_balance_manager_id: ObjectID::from_hex_literal("0x4").unwrap(),
            timestamp: 1_700_000_000_500,
        };

        let row = event.to_db_row(10, 1_700_000_000_000, "0xtx", 7);
        assert!(row.checkpoint_ts.is_some());
        assert!(row.event_ts.is_some());
        assert_eq!(row.module.as_deref(), Some("order_info"));
        assert_eq!(row.event_name.as_deref(), Some("OrderFilled"));
        assert!(row.raw_event.is_some());
    }

    #[test]
    fn order_canceled_row_sets_v2_compatibility_fields() {
        let event = OrderCanceled {
            balance_manager_id: ObjectID::from_hex_literal("0x5").unwrap(),
            pool_id: ObjectID::from_hex_literal("0x6").unwrap(),
            order_id: 7,
            client_order_id: 8,
            trader: sui_sdk_types::Address::from([9u8; 32]),
            price: 111,
            is_bid: false,
            original_quantity: 10,
            base_asset_quantity_canceled: 4,
            timestamp: 1_700_000_100_000,
        };

        let row = event.to_order_event_row(11, 1_700_000_000_000, "0xtx2", 8);
        assert!(row.checkpoint_ts.is_some());
        assert!(row.event_ts.is_some());
        assert_eq!(row.module.as_deref(), Some("order"));
        assert_eq!(row.event_name.as_deref(), Some("OrderCanceled"));
        assert!(row.raw_event.is_some());
    }
}
