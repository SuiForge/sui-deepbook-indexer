use std::{env, net::SocketAddr, str::FromStr};

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub listen_addr: SocketAddr,
    pub database_url: String,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url =
            env::var("DATABASE_URL").map_err(|_| ConfigError::Missing("DATABASE_URL"))?;
        let listen_addr =
            env::var("HTTP_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        Ok(Self {
            listen_addr: SocketAddr::from_str(&listen_addr)
                .map_err(|_| ConfigError::InvalidSocketAddr("HTTP_LISTEN_ADDR"))?,
            database_url,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing env var: {0}")]
    Missing(&'static str),
    #[error("invalid socket addr in env var: {0}")]
    InvalidSocketAddr(&'static str),
}
