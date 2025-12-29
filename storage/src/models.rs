use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
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
