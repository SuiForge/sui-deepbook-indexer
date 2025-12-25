use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{de::DeserializeOwned, Serialize};

pub fn clamp_limit(limit: Option<i64>, default: i64, max: i64) -> i64 {
    limit.unwrap_or(default).clamp(1, max)
}

pub fn encode_cursor<T: Serialize>(cursor: &T) -> Result<String, CursorError> {
    let payload = serde_json::to_vec(cursor)?;
    Ok(URL_SAFE_NO_PAD.encode(payload))
}

pub fn decode_cursor<T: DeserializeOwned>(cursor: &str) -> Result<T, CursorError> {
    let payload = URL_SAFE_NO_PAD.decode(cursor.as_bytes())?;
    Ok(serde_json::from_slice(&payload)?)
}

#[derive(Debug, thiserror::Error)]
pub enum CursorError {
    #[error("invalid cursor encoding")]
    Base64(#[from] base64::DecodeError),
    #[error("invalid cursor payload")]
    Json(#[from] serde_json::Error),
}
