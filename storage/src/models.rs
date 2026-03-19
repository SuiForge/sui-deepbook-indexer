use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IndexerStateRow {
    pub processed_checkpoint: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TransactionRow {
    pub digest: String,
    pub sender: String,
    pub checkpoint: i64,
    pub timestamp_ms: i64,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventRow {
    pub id: i64,
    pub digest: String,
    pub checkpoint: i64,
    pub timestamp_ms: i64,
    pub sender: Option<String>,
    pub event_type: String,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ObjectRow {
    pub object_id: String,
    pub owner: Option<String>,
    pub object_type: Option<String>,
    pub version: Option<i64>,
    pub raw: serde_json::Value,
    pub updated_checkpoint: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DbEventRow {
    pub checkpoint: i64,
    pub ts: DateTime<Utc>,
    pub checkpoint_ts: Option<DateTime<Utc>>,
    pub event_ts: Option<DateTime<Utc>>,
    pub ingested_at: DateTime<Utc>,
    pub pool_id: String,
    pub side: String,
    pub price: Decimal,
    pub base_sz: Decimal,
    pub quote_sz: Decimal,
    pub maker_bm: Option<String>,
    pub taker_bm: Option<String>,
    pub tx_digest: String,
    pub event_seq: i32,
    pub event_index: Option<i32>,
    pub package_id: Option<String>,
    pub module: Option<String>,
    pub event_name: Option<String>,
    pub raw_event: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DbOrderEventRow {
    pub checkpoint: i64,
    pub ts: DateTime<Utc>,
    pub checkpoint_ts: Option<DateTime<Utc>>,
    pub event_ts: Option<DateTime<Utc>>,
    pub ingested_at: DateTime<Utc>,
    pub pool_id: String,
    pub event_type: String,
    pub order_id: Option<String>,
    pub trader: Option<String>,
    pub is_bid: Option<bool>,
    pub price: Option<Decimal>,
    pub original_quantity: Option<Decimal>,
    pub new_quantity: Option<Decimal>,
    pub canceled_quantity: Option<Decimal>,
    pub tx_digest: String,
    pub event_seq: i32,
    pub event_index: Option<i32>,
    pub package_id: Option<String>,
    pub module: Option<String>,
    pub event_name: Option<String>,
    pub raw_event: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PoolMetric1mRow {
    pub pool_id: String,
    pub bucket_start: DateTime<Utc>,
    pub trades: i64,
    pub volume_base: Decimal,
    pub volume_quote: Decimal,
    pub maker_volume: Decimal,
    pub taker_volume: Decimal,
    pub fees_quote: Option<Decimal>,
    pub avg_price: Option<Decimal>,
    pub vwap: Option<Decimal>,
    pub open_price: Option<Decimal>,
    pub high_price: Option<Decimal>,
    pub low_price: Option<Decimal>,
    pub last_price: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BmMetric1mRow {
    pub bm_id: String,
    pub pool_id: String,
    pub bucket_start: DateTime<Utc>,
    pub trades: i64,
    pub volume_quote: Decimal,
    pub maker_volume: Decimal,
    pub taker_volume: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(ms).single().unwrap()
    }

    #[test]
    fn db_event_row_serializes_v2_fields() {
        let row = DbEventRow {
            checkpoint: 1,
            ts: ts(1_700_000_000_000),
            checkpoint_ts: Some(ts(1_700_000_000_000)),
            event_ts: Some(ts(1_700_000_000_100)),
            ingested_at: ts(1_700_000_000_200),
            pool_id: "0xpool".to_string(),
            side: "buy".to_string(),
            price: Decimal::from(100u64),
            base_sz: Decimal::from(2u64),
            quote_sz: Decimal::from(200u64),
            maker_bm: Some("0xmaker".to_string()),
            taker_bm: Some("0xtaker".to_string()),
            tx_digest: "0xtx".to_string(),
            event_seq: 7,
            event_index: Some(0),
            package_id: Some("0xpackage".to_string()),
            module: Some("order_info".to_string()),
            event_name: Some("OrderFilled".to_string()),
            raw_event: Some(serde_json::json!({"kind": "fill"})),
        };

        let value = serde_json::to_value(&row).unwrap();
        assert_eq!(value["checkpoint_ts"], "2023-11-14T22:13:20Z");
        assert_eq!(value["event_name"], "OrderFilled");
        assert_eq!(value["raw_event"]["kind"], "fill");
    }

    #[test]
    fn db_order_event_row_serializes_v2_fields() {
        let row = DbOrderEventRow {
            checkpoint: 2,
            ts: ts(1_700_000_100_000),
            checkpoint_ts: Some(ts(1_700_000_100_000)),
            event_ts: None,
            ingested_at: ts(1_700_000_100_200),
            pool_id: "0xpool".to_string(),
            event_type: "order_placed".to_string(),
            order_id: Some("42".to_string()),
            trader: Some("0xtrader".to_string()),
            is_bid: Some(true),
            price: Some(Decimal::from(101u64)),
            original_quantity: Some(Decimal::from(3u64)),
            new_quantity: Some(Decimal::from(3u64)),
            canceled_quantity: None,
            tx_digest: "0xtx2".to_string(),
            event_seq: 8,
            event_index: Some(1),
            package_id: None,
            module: Some("order_info".to_string()),
            event_name: Some("OrderPlaced".to_string()),
            raw_event: Some(serde_json::json!({"kind": "lifecycle"})),
        };

        let value = serde_json::to_value(&row).unwrap();
        assert_eq!(value["event_ts"], serde_json::Value::Null);
        assert_eq!(value["module"], "order_info");
        assert_eq!(value["raw_event"]["kind"], "lifecycle");
    }
}
