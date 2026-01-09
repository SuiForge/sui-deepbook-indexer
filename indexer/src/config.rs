//! DeepBook indexer configuration
//!
//! Supports multiple package versions for backward compatibility with historical data.

use anyhow::{Context, Result};
use std::env;
use std::time::Duration;
use sui_types::base_types::ObjectID;
use url::Url;

/// Remote Store URLs for checkpoint data
pub const MAINNET_REMOTE_STORE_URL: &str = "https://checkpoints.mainnet.sui.io";
pub const TESTNET_REMOTE_STORE_URL: &str = "https://checkpoints.testnet.sui.io";

/// DeepBook package addresses - multiple versions for backward compatibility
/// These are all the package versions that have been deployed, allowing us to index historical transactions
const MAINNET_PACKAGES: &[&str] = &[
    "0xb29d83c26cdd2a64959263abbcfc4a6937f0c9fccaf98580ca56faded65be244",
    "0x2c8d603bc51326b8c13cef9dd07031a408a48dddb541963357661df5d3204809",
    "0xcaf6ba059d539a97646d47f0b9ddf843e138d215e2a12ca1f4585d386f7aec3a",
    "0x00c1a56ec8c4c623a848b2ed2f03d23a25d17570b670c22106f336eb933785cc",
    "0x2d93777cc8b67c064b495e8606f2f8f5fd578450347bbe7b36e0bc03963c1c40", // Latest
];

const TESTNET_PACKAGES: &[&str] = &[
    "0x467e34e75debeea8b89d03aea15755373afc39a7c96c9959549c7f5f689843cf",
    "0x5d520a3e3059b68530b2ef4080126dbb5d234e0afd66561d0d9bd48127a06044",
    "0xcd40faffa91c00ce019bfe4a4b46f8d623e20bf331eb28990ee0305e9b9f3e3c",
    "0x16c4e050b9b19b25ce1365b96861bc50eb7e58383348a39ea8a8e1d063cfef73",
    "0xc483dba510597205749f2e8410c23f19be31a710aef251f353bc1b97755efd4d",
    "0x5da5bbf6fb097d108eaf2c2306f88beae4014c90a44b95c7e76a6bfccec5f5ee",
    "0xa3886aaa8aa831572dd39549242ca004a438c3a55967af9f0387ad2b01595068",
    "0x9592ac923593f37f4fed15ee15f760ebd4c39729f53ee3e8c214de7a17157769",
    "0x984757fc7c0e6dd5f15c2c66e881dd6e5aca98b725f3dbd83c445e057ebb790a",
    "0xfb28c4cbc6865bd1c897d26aecbe1f8792d1509a20ffec692c800660cbec6982",
    "0x926c446869fa175ec3b0dbf6c4f14604d86a415c1fccd8c8f823cfc46a29baed",
    "0xa0936c6ea82fbfc0356eedc2e740e260dedaaa9f909a0715b1cc31e9a8283719",
    "0x9ae1cbfb7475f6a4c2d4d3273335459f8f9d265874c4d161c1966cdcbd4e9ebc",
    "0xb48d47cb5f56d0f489f48f186d06672df59d64bd2f514b2f0ba40cbb8c8fd487",
    "0xbc331f09e5c737d45f074ad2d17c3038421b3b9018699e370d88d94938c53d28",
    "0x23018638bb4f11ef9ffb0de922519bea52f960e7a5891025ca9aaeeaff7d5034",
    "0x22be4cade64bf2d02412c7e8d0e8beea2f78828b948118d46735315409371a3c", // Latest
];

/// DeepBook environment
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DeepbookEnv {
    Mainnet,
    Testnet,
}

impl DeepbookEnv {
    /// Get the Remote Store URL for this environment
    pub fn remote_store_url(&self) -> Url {
        let url = match self {
            DeepbookEnv::Mainnet => MAINNET_REMOTE_STORE_URL,
            DeepbookEnv::Testnet => TESTNET_REMOTE_STORE_URL,
        };
        Url::parse(url).expect("Invalid remote store URL")
    }

    /// Get all package addresses for this environment (for multi-version support)
    pub fn package_addresses(&self) -> &'static [&'static str] {
        match self {
            DeepbookEnv::Mainnet => MAINNET_PACKAGES,
            DeepbookEnv::Testnet => TESTNET_PACKAGES,
        }
    }

    /// Parse package addresses to ObjectID
    pub fn parse_package_bytes(&self) -> Vec<ObjectID> {
        self.package_addresses()
            .iter()
            .filter_map(|addr| ObjectID::from_hex_literal(addr).ok())
            .collect()
    }
}

impl std::fmt::Display for DeepbookEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeepbookEnv::Mainnet => write!(f, "mainnet"),
            DeepbookEnv::Testnet => write!(f, "testnet"),
        }
    }
}

/// Indexer configuration
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub database_url: String,
    pub env: DeepbookEnv,
    pub poll_interval: Duration,
    pub request_timeout: Duration,
    pub backoff_base_ms: u64,
    pub backoff_max_ms: u64,
    pub start_checkpoint: Option<u64>,
    pub stop_checkpoint: Option<u64>,
}

impl IndexerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        // Load .env files if present
        let _ = dotenvy::from_filename(".env.local");
        let _ = dotenvy::from_filename(".env");

        let database_url = env::var("DATABASE_URL").context("missing env var DATABASE_URL")?;

        let env = match env::var("DEEPBOOK_ENV")
            .unwrap_or_else(|_| "testnet".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "mainnet" | "main" => DeepbookEnv::Mainnet,
            "testnet" | "test" => DeepbookEnv::Testnet,
            other => anyhow::bail!("Invalid DEEPBOOK_ENV: {}. Use 'mainnet' or 'testnet'", other),
        };

        let poll_interval_ms = env_opt_u64("INDEXER_POLL_INTERVAL_MS")?.unwrap_or(500);
        let request_timeout_ms = env_opt_u64("INDEXER_REQUEST_TIMEOUT_MS")?.unwrap_or(30_000);
        let backoff_base_ms = env_opt_u64("INDEXER_BACKOFF_BASE_MS")?.unwrap_or(100);
        let backoff_max_ms = env_opt_u64("INDEXER_BACKOFF_MAX_MS")?.unwrap_or(30_000);
        let start_checkpoint = env_opt_u64("INDEXER_START_CHECKPOINT")?;
        let stop_checkpoint = env_opt_u64("INDEXER_STOP_CHECKPOINT")?;

        Ok(Self {
            database_url,
            env,
            poll_interval: Duration::from_millis(poll_interval_ms),
            request_timeout: Duration::from_millis(request_timeout_ms),
            backoff_base_ms,
            backoff_max_ms,
            start_checkpoint,
            stop_checkpoint,
        })
    }

    /// Get the Remote Store URL
    pub fn remote_store_url(&self) -> Url {
        self.env.remote_store_url()
    }
}

fn env_opt_u64(name: &str) -> Result<Option<u64>> {
    match env::var(name) {
        Ok(v) => Ok(Some(
            v.parse::<u64>()
                .with_context(|| format!("invalid {name}: expected u64"))?,
        )),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mainnet_packages() {
        let env = DeepbookEnv::Mainnet;
        assert_eq!(env.package_addresses().len(), 5);
    }

    #[test]
    fn test_testnet_packages() {
        let env = DeepbookEnv::Testnet;
        assert_eq!(env.package_addresses().len(), 17);
    }

    #[test]
    fn test_parse_package_bytes() {
        let env = DeepbookEnv::Mainnet;
        let packages = env.parse_package_bytes();
        assert_eq!(packages.len(), 5);
    }
}
