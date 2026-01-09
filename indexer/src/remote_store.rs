//! Remote Store client for fetching checkpoint data
//!
//! Downloads checkpoint files from Sui's CDN instead of using RPC.
//! This provides better reliability, no rate limiting, and global CDN acceleration.

use anyhow::{Context, Result};
use bytes::Bytes;
use reqwest::Client;
use std::time::Duration;
use sui_types::full_checkpoint_content::CheckpointData;
use tracing::{debug, warn};
use url::Url;

/// Remote Store client for downloading checkpoint files
#[derive(Clone)]
pub struct RemoteStoreClient {
    client: Client,
    base_url: Url,
}

impl RemoteStoreClient {
    /// Create a new Remote Store client
    pub fn new(base_url: Url, timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .gzip(true)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, base_url })
    }

    /// Build the URL for a checkpoint file
    fn checkpoint_url(&self, seq: u64) -> String {
        format!("{}/{}.chk", self.base_url.as_str().trim_end_matches('/'), seq)
    }

    /// Fetch raw checkpoint bytes
    pub async fn fetch_checkpoint_bytes(&self, seq: u64) -> Result<Option<Bytes>> {
        let url = self.checkpoint_url(seq);
        debug!(checkpoint = seq, url = %url, "Fetching checkpoint");

        let response = self.client.get(&url).send().await;

        match response {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::NOT_FOUND {
                    debug!(checkpoint = seq, "Checkpoint not yet available");
                    return Ok(None);
                }

                if !resp.status().is_success() {
                    anyhow::bail!(
                        "Failed to fetch checkpoint {}: HTTP {}",
                        seq,
                        resp.status()
                    );
                }

                let bytes = resp.bytes().await.context("Failed to read response body")?;
                debug!(checkpoint = seq, bytes = bytes.len(), "Checkpoint fetched");
                Ok(Some(bytes))
            }
            Err(err) => {
                if err.is_timeout() {
                    warn!(checkpoint = seq, "Checkpoint fetch timed out");
                    anyhow::bail!("Checkpoint {} fetch timed out", seq);
                }
                Err(err).context(format!("Failed to fetch checkpoint {}", seq))
            }
        }
    }

    /// Fetch and parse checkpoint data using official sui-types
    pub async fn fetch_checkpoint(&self, seq: u64) -> Result<Option<CheckpointData>> {
        let bytes = match self.fetch_checkpoint_bytes(seq).await? {
            Some(b) => b,
            None => return Ok(None),
        };

        let checkpoint: CheckpointData =
            bcs::from_bytes(&bytes).context("Failed to deserialize checkpoint")?;

        // Validate sequence number
        let got_seq = checkpoint.checkpoint_summary.sequence_number;
        if got_seq != seq {
            anyhow::bail!(
                "Checkpoint sequence mismatch: requested {}, got {}",
                seq,
                got_seq
            );
        }

        Ok(Some(checkpoint))
    }

    /// Get the latest available checkpoint sequence number by binary search
    pub async fn get_latest_checkpoint(&self) -> Result<u64> {
        let mut low: u64 = 0;
        let mut high: u64 = u64::MAX / 2;

        // First, find an upper bound by doubling
        let mut probe = 1_000_000u64;
        loop {
            if self.fetch_checkpoint_bytes(probe).await?.is_none() {
                high = probe;
                break;
            }
            low = probe;
            probe = probe.saturating_mul(2);
            if probe > high {
                break;
            }
        }

        // Binary search for the exact latest
        while low < high {
            let mid = low + (high - low) / 2;
            if self.fetch_checkpoint_bytes(mid).await?.is_some() {
                low = mid + 1;
            } else {
                high = mid;
            }
        }

        if low > 0 {
            Ok(low - 1)
        } else {
            anyhow::bail!("No checkpoints found")
        }
    }
}

/// Exponential backoff helper
pub struct Backoff {
    base_ms: u64,
    max_ms: u64,
    attempt: u32,
}

impl Backoff {
    pub fn new(base_ms: u64, max_ms: u64) -> Self {
        Self {
            base_ms: base_ms.max(1),
            max_ms: max_ms.max(1),
            attempt: 0,
        }
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn next_delay(&mut self) -> Duration {
        let shift = self.attempt.min(20);
        let exp = self.base_ms.saturating_mul(1u64 << shift).min(self.max_ms);
        self.attempt = self.attempt.saturating_add(1);

        // Add 10% jitter
        let jitter = (exp / 10).min(500);
        let jitter_val = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64)
            % (jitter + 1);

        Duration::from_millis(exp + jitter_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_url() {
        let client = RemoteStoreClient::new(
            Url::parse("https://checkpoints.testnet.sui.io").unwrap(),
            Duration::from_secs(30),
        )
        .unwrap();

        assert_eq!(
            client.checkpoint_url(12345),
            "https://checkpoints.testnet.sui.io/12345.chk"
        );
    }

    #[test]
    fn test_backoff() {
        let mut backoff = Backoff::new(100, 30000);
        let d1 = backoff.next_delay();
        let d2 = backoff.next_delay();
        let d3 = backoff.next_delay();

        assert!(d2 > d1);
        assert!(d3 > d2);

        backoff.reset();
        let d4 = backoff.next_delay();
        assert!(d4.as_millis() < 200);
    }
}
