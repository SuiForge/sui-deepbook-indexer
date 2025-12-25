use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageResponse<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCursor {
    pub checkpoint: i64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCursor {
    pub checkpoint: i64,
    pub id: i64,
}
