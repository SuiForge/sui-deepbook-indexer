// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::info;

use crate::config::DeepbookEnv;
use deepbook_indexer_storage::{
    models::{AssetMetadataRow, PoolMetadataRow},
    queries,
};

// Keep the first seed pinned to repo-local operational knowledge so we do not
// guess pool identifiers. This pair is already used by scripts/seed_recent_sui_usdc.py.
const MAINNET_SUI_USDC_PACKAGE_ID: &str =
    "0x2c8d603bc51326b8c13cef9dd07031a408a48dddb541963357661df5d3204809";
const MAINNET_SUI_USDC_POOL_ID: &str =
    "0xe05dafb5133bcffb8d59f4e12465dc0e9faeaa05e3e342a08fe135800e3e4407";
const STATIC_SEED_SOURCE: &str = "static_seed:repo-local";
const SEEDED_STATUS: &str = "seeded";

pub async fn seed_known_metadata(pool: &PgPool, env: DeepbookEnv) -> Result<()> {
    let now = Utc::now();
    let asset_rows = build_asset_seed_rows(now);
    let pool_rows = build_pool_seed_rows(env, now);

    queries::upsert_asset_metadata(pool, &asset_rows).await?;
    queries::upsert_pool_metadata(pool, &pool_rows).await?;

    info!(
        env = %env,
        asset_rows = asset_rows.len(),
        pool_rows = pool_rows.len(),
        "Seeded metadata scaffolding"
    );

    Ok(())
}

fn build_asset_seed_rows(now: DateTime<Utc>) -> Vec<AssetMetadataRow> {
    vec![
        AssetMetadataRow {
            asset_id: "sui".to_string(),
            coin_type: Some("0x2::sui::SUI".to_string()),
            symbol: Some("SUI".to_string()),
            name: Some("Sui".to_string()),
            decimals: Some(9),
            status: Some(SEEDED_STATUS.to_string()),
            source: Some(STATIC_SEED_SOURCE.to_string()),
            updated_at: now,
        },
        AssetMetadataRow {
            asset_id: "usdc".to_string(),
            coin_type: None,
            symbol: Some("USDC".to_string()),
            name: Some("USD Coin".to_string()),
            decimals: Some(6),
            status: Some(SEEDED_STATUS.to_string()),
            source: Some(STATIC_SEED_SOURCE.to_string()),
            updated_at: now,
        },
    ]
}

fn build_pool_seed_rows(env: DeepbookEnv, now: DateTime<Utc>) -> Vec<PoolMetadataRow> {
    match env {
        DeepbookEnv::Mainnet => vec![PoolMetadataRow {
            pool_id: MAINNET_SUI_USDC_POOL_ID.to_string(),
            base_asset_id: Some("sui".to_string()),
            quote_asset_id: Some("usdc".to_string()),
            package_id: Some(MAINNET_SUI_USDC_PACKAGE_ID.to_string()),
            status: Some(SEEDED_STATUS.to_string()),
            updated_at: now,
        }],
        DeepbookEnv::Testnet => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn mainnet_seed_contains_known_sui_usdc_pool() {
        let now = Utc
            .timestamp_millis_opt(1_700_000_000_000)
            .single()
            .unwrap();
        let rows = build_pool_seed_rows(DeepbookEnv::Mainnet, now);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pool_id, MAINNET_SUI_USDC_POOL_ID);
        assert_eq!(rows[0].base_asset_id.as_deref(), Some("sui"));
        assert_eq!(rows[0].quote_asset_id.as_deref(), Some("usdc"));
    }

    #[test]
    fn testnet_seed_only_adds_assets() {
        let now = Utc
            .timestamp_millis_opt(1_700_000_000_000)
            .single()
            .unwrap();
        let assets = build_asset_seed_rows(now);
        let pools = build_pool_seed_rows(DeepbookEnv::Testnet, now);

        assert!(assets.iter().any(|row| row.asset_id == "sui"));
        assert!(assets.iter().any(|row| row.asset_id == "usdc"));
        assert!(pools.is_empty());
    }
}
