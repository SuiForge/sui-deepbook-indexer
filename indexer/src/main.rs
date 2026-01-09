//! DeepBook Indexer - Remote Store Edition
//!
//! Indexes DeepBook events from Sui checkpoints using the Remote Store CDN.
//! Features:
//! - Remote Store data source (more stable than RPC)
//! - Multi-package version support (backward compatibility)
//! - BCS event deserialization (type-safe, using official sui-types)
//! - Pre-aggregated OHLCV metrics

mod config;
mod events;
mod remote_store;

use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, TimeZone, Utc};
use clap::{Parser, Subcommand};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::time::Duration;
use sui_types::base_types::ObjectID;
use sui_types::full_checkpoint_content::CheckpointData;
use tracing::{debug, info, warn};

use config::{DeepbookEnv, IndexerConfig};
use deepbook_indexer_storage::{db, models::*, queries};
use events::{MoveStruct, OrderFilled};
use remote_store::{Backoff, RemoteStoreClient};

// Re-export for events module
pub use deepbook_indexer_storage::models::DbEventRow;

#[derive(Parser)]
#[command(name = "deepbook-indexer", about = "DeepBook indexer (Remote Store edition)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run live indexer
    Run,
    /// Replay checkpoint range and recompute rollups
    Replay {
        #[arg(long)]
        from_checkpoint: i64,
        #[arg(long)]
        to_checkpoint: i64,
    },
    /// Show current sync status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let cli = Cli::parse();
    let cfg = IndexerConfig::from_env()?;

    info!(
        env = %cfg.env,
        remote_store = %cfg.remote_store_url(),
        packages = cfg.env.package_addresses().len(),
        "DeepBook Indexer starting"
    );

    let pool = db::connect(&cfg.database_url).await?;
    sqlx::migrate!("../migrations").run(&pool).await?;

    match cli.command {
        Some(Commands::Replay { from_checkpoint, to_checkpoint }) => {
            replay_range(&pool, from_checkpoint, to_checkpoint).await
        }
        Some(Commands::Status) => show_status(&pool, &cfg).await,
        Some(Commands::Run) | None => run_indexer(pool, cfg).await,
    }
}

async fn run_indexer(pool: PgPool, cfg: IndexerConfig) -> Result<()> {
    // Get last processed checkpoint from DB
    let processed_checkpoint: i64 = sqlx::query_scalar(
        "SELECT processed_checkpoint FROM indexer_state WHERE id = 1",
    )
    .fetch_one(&pool)
    .await?;

    let start_checkpoint = cfg
        .start_checkpoint
        .map(|c| c as i64)
        .unwrap_or(processed_checkpoint.saturating_add(1));

    if start_checkpoint > processed_checkpoint.saturating_add(1) {
        anyhow::bail!(
            "INDEXER_START_CHECKPOINT ({}) is ahead of processed_checkpoint ({}); refusing to create a gap",
            start_checkpoint,
            processed_checkpoint
        );
    }

    info!(
        processed_checkpoint,
        start_checkpoint,
        stop_checkpoint = cfg.stop_checkpoint,
        "Checkpoint indexer starting"
    );

    // Create Remote Store client
    let client = RemoteStoreClient::new(cfg.remote_store_url(), cfg.request_timeout)?;

    // Get package IDs for filtering
    let package_ids = cfg.env.parse_package_bytes();

    let mut next_checkpoint = start_checkpoint as u64;
    let mut backoff = Backoff::new(cfg.backoff_base_ms, cfg.backoff_max_ms);

    // Graceful shutdown
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        // Check stop condition
        if let Some(stop) = cfg.stop_checkpoint {
            if next_checkpoint > stop {
                info!(stop_checkpoint = stop, "Reached stop checkpoint; exiting");
                break;
            }
        }

        // Fetch checkpoint from Remote Store
        let checkpoint_result = tokio::select! {
            _ = &mut shutdown => break,
            res = client.fetch_checkpoint(next_checkpoint) => res,
        };

        match checkpoint_result {
            Ok(Some(checkpoint)) => {
                // Process checkpoint
                match ingest_checkpoint(&pool, &cfg.env, &package_ids, checkpoint).await {
                    Ok(seq) => {
                        next_checkpoint = seq + 1;
                        backoff.reset();
                        info!(checkpoint = seq, "Checkpoint committed");
                    }
                    Err(err) => {
                        warn!(error = ?err, checkpoint = next_checkpoint, "Failed to ingest; will retry");
                        let delay = backoff.next_delay();
                        if should_wait_with_shutdown(&mut shutdown, delay).await {
                            break;
                        }
                    }
                }
            }
            Ok(None) => {
                // Checkpoint not yet available
                debug!(checkpoint = next_checkpoint, "Checkpoint not yet available");
                backoff.reset();
                if should_wait_with_shutdown(&mut shutdown, cfg.poll_interval).await {
                    break;
                }
            }
            Err(err) => {
                warn!(error = ?err, checkpoint = next_checkpoint, "Failed to fetch; will retry");
                let delay = backoff.next_delay();
                if should_wait_with_shutdown(&mut shutdown, delay).await {
                    break;
                }
            }
        }
    }

    info!("Indexer stopped");
    Ok(())
}

async fn ingest_checkpoint(
    pool: &PgPool,
    env: &DeepbookEnv,
    package_ids: &[ObjectID],
    checkpoint: CheckpointData,
) -> Result<u64> {
    let seq = checkpoint.checkpoint_summary.sequence_number;
    let timestamp_ms = checkpoint.checkpoint_summary.timestamp_ms as i64;

    let mut trade_events: Vec<DbEventRow> = Vec::new();

    // Process each transaction
    for tx in &checkpoint.transactions {
        let Some(events) = &tx.events else { continue };

        // Get transaction digest
        let tx_digest = tx.transaction.digest().to_string();

        // Process each event
        for (event_idx, event) in events.data.iter().enumerate() {
            // Check if this is a DeepBook event
            let is_deepbook = package_ids.iter().any(|pkg| event.package_id == *pkg);
            if !is_deepbook {
                continue;
            }

            // Try to parse as OrderFilled
            if OrderFilled::matches_event_type(event, *env) {
                match bcs::from_bytes::<OrderFilled>(&event.contents) {
                    Ok(order_filled) => {
                        let row = order_filled.to_db_row(
                            seq as i64,
                            timestamp_ms,
                            &tx_digest,
                            event_idx as i32,
                        );
                        trade_events.push(row);
                        debug!(
                            pool_id = %order_filled.pool_id,
                            price = order_filled.price,
                            "OrderFilled event"
                        );
                    }
                    Err(err) => {
                        warn!(
                            error = ?err,
                            event_type = %format!("{}::{}", event.type_.module, event.type_.name),
                            "Failed to deserialize OrderFilled"
                        );
                    }
                }
            }

            // TODO: Add more event types here (OrderPlaced, OrderCanceled, etc.)
        }
    }

    // Commit to database
    let mut tx = pool.begin().await?;

    // Insert events
    queries::insert_db_events(&mut *tx, &trade_events).await?;

    // Recompute rollups for affected buckets
    let affected_buckets: HashSet<DateTime<Utc>> = trade_events
        .iter()
        .map(|e| truncate_to_minute(e.ts))
        .collect();

    for bucket_start in affected_buckets {
        let bucket_end = bucket_start + ChronoDuration::minutes(1);
        let events = queries::list_events_in_time_range(&mut *tx, bucket_start, bucket_end).await?;
        let (pool_rows, bm_rows) = compute_rollups(&events);
        queries::upsert_pool_metrics(&mut *tx, &pool_rows).await?;
        queries::upsert_bm_metrics(&mut *tx, &bm_rows).await?;
    }

    // Update progress
    sqlx::query(
        "UPDATE indexer_state SET processed_checkpoint = GREATEST(processed_checkpoint, $1), updated_at = now() WHERE id = 1",
    )
    .bind(seq as i64)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(seq)
}

fn truncate_to_minute(ts: DateTime<Utc>) -> DateTime<Utc> {
    let secs = ts.timestamp();
    let truncated = secs - (secs % 60);
    Utc.timestamp_opt(truncated, 0).single().unwrap_or(ts)
}

fn compute_rollups(events: &[DbEventRow]) -> (Vec<PoolMetric1mRow>, Vec<BmMetric1mRow>) {
    struct PoolAgg {
        trades: i64,
        volume_base: Decimal,
        volume_quote: Decimal,
        maker_volume: Decimal,
        taker_volume: Decimal,
        sum_px_base: Decimal,
        sum_base: Decimal,
        open_price: Option<Decimal>,
        high_price: Option<Decimal>,
        low_price: Option<Decimal>,
        last_price: Option<Decimal>,
    }

    struct BmAgg {
        trades: i64,
        volume_quote: Decimal,
        maker_volume: Decimal,
        taker_volume: Decimal,
    }

    let mut pool_map: HashMap<(String, DateTime<Utc>), PoolAgg> = HashMap::new();
    let mut bm_map: HashMap<(String, String, DateTime<Utc>), BmAgg> = HashMap::new();

    for ev in events {
        let bucket = truncate_to_minute(ev.ts);

        // Update pool metrics
        let pool_entry = pool_map
            .entry((ev.pool_id.clone(), bucket))
            .or_insert(PoolAgg {
                trades: 0,
                volume_base: Decimal::ZERO,
                volume_quote: Decimal::ZERO,
                maker_volume: Decimal::ZERO,
                taker_volume: Decimal::ZERO,
                sum_px_base: Decimal::ZERO,
                sum_base: Decimal::ZERO,
                open_price: None,
                high_price: None,
                low_price: None,
                last_price: None,
            });

        pool_entry.trades += 1;
        pool_entry.volume_base += ev.base_sz;
        pool_entry.volume_quote += ev.quote_sz;
        pool_entry.sum_px_base += ev.price * ev.base_sz;
        pool_entry.sum_base += ev.base_sz;

        if pool_entry.open_price.is_none() {
            pool_entry.open_price = Some(ev.price);
        }
        pool_entry.high_price = Some(match pool_entry.high_price {
            Some(h) if h >= ev.price => h,
            _ => ev.price,
        });
        pool_entry.low_price = Some(match pool_entry.low_price {
            Some(l) if l <= ev.price => l,
            _ => ev.price,
        });
        pool_entry.last_price = Some(ev.price);

        if ev.side == "sell" {
            pool_entry.maker_volume += ev.quote_sz;
        } else {
            pool_entry.taker_volume += ev.quote_sz;
        }

        // Update BM metrics
        let mut update_bm = |bm_opt: Option<&String>, is_maker: bool| {
            if let Some(bm) = bm_opt {
                let key = (bm.clone(), ev.pool_id.clone(), bucket);
                let entry = bm_map.entry(key).or_insert(BmAgg {
                    trades: 0,
                    volume_quote: Decimal::ZERO,
                    maker_volume: Decimal::ZERO,
                    taker_volume: Decimal::ZERO,
                });
                entry.trades += 1;
                entry.volume_quote += ev.quote_sz;
                if is_maker {
                    entry.maker_volume += ev.quote_sz;
                } else {
                    entry.taker_volume += ev.quote_sz;
                }
            }
        };

        update_bm(ev.maker_bm.as_ref(), true);
        update_bm(ev.taker_bm.as_ref(), false);
    }

    // Convert to output rows
    let pool_rows: Vec<PoolMetric1mRow> = pool_map
        .into_iter()
        .map(|((pool_id, bucket_start), agg)| {
            let vwap = if agg.sum_base.is_zero() {
                None
            } else {
                Some(agg.sum_px_base / agg.sum_base)
            };

            PoolMetric1mRow {
                pool_id,
                bucket_start,
                trades: agg.trades,
                volume_base: agg.volume_base,
                volume_quote: agg.volume_quote,
                maker_volume: agg.maker_volume,
                taker_volume: agg.taker_volume,
                fees_quote: None,
                avg_price: vwap,
                vwap,
                open_price: agg.open_price,
                high_price: agg.high_price,
                low_price: agg.low_price,
                last_price: agg.last_price,
            }
        })
        .collect();

    let bm_rows: Vec<BmMetric1mRow> = bm_map
        .into_iter()
        .map(|((bm_id, pool_id, bucket_start), agg)| BmMetric1mRow {
            bm_id,
            pool_id,
            bucket_start,
            trades: agg.trades,
            volume_quote: agg.volume_quote,
            maker_volume: agg.maker_volume,
            taker_volume: agg.taker_volume,
        })
        .collect();

    (pool_rows, bm_rows)
}

async fn replay_range(pool: &PgPool, from: i64, to: i64) -> Result<()> {
    info!(from_checkpoint = from, to_checkpoint = to, "Starting replay");

    if from > to {
        anyhow::bail!("from_checkpoint must be <= to_checkpoint");
    }

    let mut tx = pool.begin().await?;

    let events = queries::list_events_in_checkpoint_range(&mut *tx, from, to).await?;
    info!(event_count = events.len(), "Fetched events for replay");

    let affected_buckets: Vec<DateTime<Utc>> = events
        .iter()
        .map(|e| truncate_to_minute(e.ts))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if !affected_buckets.is_empty() {
        info!(bucket_count = affected_buckets.len(), "Recomputing affected rollup buckets");
    }

    for bucket_start in affected_buckets {
        let bucket_end = bucket_start + ChronoDuration::minutes(1);
        let bucket_events = queries::list_events_in_time_range(&mut *tx, bucket_start, bucket_end).await?;
        let (pool_rows, bm_rows) = compute_rollups(&bucket_events);
        info!(
            bucket_start = %bucket_start,
            pool_rows = pool_rows.len(),
            bm_rows = bm_rows.len(),
            "Upserting rollups"
        );
        queries::upsert_pool_metrics(&mut *tx, &pool_rows).await?;
        queries::upsert_bm_metrics(&mut *tx, &bm_rows).await?;
    }

    tx.commit().await?;

    info!("Replay complete");
    Ok(())
}

async fn show_status(pool: &PgPool, cfg: &IndexerConfig) -> Result<()> {
    let processed_checkpoint: i64 = sqlx::query_scalar(
        "SELECT processed_checkpoint FROM indexer_state WHERE id = 1",
    )
    .fetch_one(pool)
    .await?;

    let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM db_events")
        .fetch_one(pool)
        .await?;

    let pool_count: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT pool_id) FROM db_events")
        .fetch_one(pool)
        .await?;

    // Try to get latest checkpoint from remote store
    let client = RemoteStoreClient::new(cfg.remote_store_url(), cfg.request_timeout)?;
    let latest = match client.get_latest_checkpoint().await {
        Ok(l) => Some(l),
        Err(_) => None,
    };

    println!("DeepBook Indexer Status");
    println!("=======================");
    println!("Environment:          {}", cfg.env);
    println!("Remote Store:         {}", cfg.remote_store_url());
    println!("Package versions:     {}", cfg.env.package_addresses().len());
    println!();
    println!("Processed checkpoint: {}", processed_checkpoint);
    if let Some(l) = latest {
        let lag = l as i64 - processed_checkpoint;
        println!("Latest checkpoint:    {}", l);
        println!("Checkpoint lag:       {}", lag);
    }
    println!();
    println!("Total events:         {}", event_count);
    println!("Unique pools:         {}", pool_count);

    Ok(())
}

async fn should_wait_with_shutdown(
    shutdown: &mut std::pin::Pin<&mut impl Future<Output = Result<(), std::io::Error>>>,
    duration: Duration,
) -> bool {
    tokio::select! {
        _ = shutdown => true,
        _ = tokio::time::sleep(duration) => false,
    }
}
