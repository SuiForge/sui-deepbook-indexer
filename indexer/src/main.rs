use anyhow::{anyhow, Context as _};
use chrono::{DateTime, TimeZone, Utc, Duration as ChronoDuration};
use clap::{Parser, Subcommand};
use prost_types::{ListValue, Struct, Value as ProtoValue};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::{collections::{HashMap, HashSet}, env, future::Future, time::Duration};
use deepbook_indexer_storage::{
    db,
    models::{BmMetric1mRow, DbEventRow, PoolMetric1mRow},
    queries,
};
use sui_rpc::{
    field::{FieldMask, FieldMaskUtil},
    proto::sui::rpc::v2::{GetCheckpointRequest, GetCheckpointResponse},
    Client as SuiRpcClient,
};
use tonic::{Code, Response, Status};
use tracing::{info, warn};

#[derive(Debug, Clone)]
struct IndexerConfig {
    database_url: String,
    rpc_api_url: String,
    rpc_api_fallback: Option<String>,
    poll_interval: Duration,
    rpc_timeout: Duration,
    backoff_base_ms: u64,
    backoff_max_ms: u64,
    start_checkpoint: Option<i64>,
    stop_checkpoint: Option<i64>,
    deepbook_package_ids: Vec<String>,
    deepbook_event_type: String,
    field_pool: String,
    field_side: String,
    field_price: String,
    field_base_sz: String,
    field_quote_sz: String,
    field_maker_bm: String,
    field_taker_bm: String,
}

impl IndexerConfig {
    fn from_env() -> anyhow::Result<Self> {
        // Load .env files if present
        let _ = dotenvy::from_filename(".env.local");
        let _ = dotenvy::from_filename(".env");

        let database_url = env::var("DATABASE_URL").context("missing env var DATABASE_URL")?;
        let rpc_api_url = env::var("RPC_API_URL").context("missing env var RPC_API_URL")?;
        let rpc_api_fallback = env::var("RPC_API_FALLBACK").ok();

        let poll_interval_ms = env_opt_u64("INDEXER_POLL_INTERVAL_MS")?.unwrap_or(1_000);
        let rpc_timeout_ms = env_opt_u64("INDEXER_RPC_TIMEOUT_MS")?.unwrap_or(10_000);

        let backoff_base_ms = env_opt_u64("INDEXER_BACKOFF_BASE_MS")?.unwrap_or(200);
        let backoff_max_ms = env_opt_u64("INDEXER_BACKOFF_MAX_MS")?.unwrap_or(30_000);

        let start_checkpoint = env_opt_i64("INDEXER_START_CHECKPOINT")?;
        let stop_checkpoint = env_opt_i64("INDEXER_STOP_CHECKPOINT")?;

        let deepbook_package_id_raw =
            env::var("DEEPBOOK_PACKAGE_ID").context("missing env var DEEPBOOK_PACKAGE_ID")?;
        let deepbook_package_ids: Vec<String> = deepbook_package_id_raw
            .split(|c| c == ',' || c == ' ' || c == '\n' || c == '\t')
            .filter_map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_ascii_lowercase())
                }
            })
            .collect();
        if deepbook_package_ids.is_empty() {
            anyhow::bail!("DEEPBOOK_PACKAGE_ID is empty");
        }
        let deepbook_event_type = env::var("DEEPBOOK_EVENT_TYPE")
            .unwrap_or_else(|_| "OrderFilled".to_string());

        let field_pool = env::var("DEEPBOOK_FIELD_POOL_ID").unwrap_or_else(|_| "pool_id".into());
        let field_side = env::var("DEEPBOOK_FIELD_SIDE").unwrap_or_else(|_| "side".into());
        let field_price = env::var("DEEPBOOK_FIELD_PRICE").unwrap_or_else(|_| "price".into());
        let field_base_sz = env::var("DEEPBOOK_FIELD_BASE_SZ").unwrap_or_else(|_| "base_sz".into());
        let field_quote_sz = env::var("DEEPBOOK_FIELD_QUOTE_SZ").unwrap_or_else(|_| "quote_sz".into());
        let field_maker_bm = env::var("DEEPBOOK_FIELD_MAKER_BM").unwrap_or_else(|_| "maker_bm".into());
        let field_taker_bm = env::var("DEEPBOOK_FIELD_TAKER_BM").unwrap_or_else(|_| "taker_bm".into());

        Ok(Self {
            database_url,
            rpc_api_url,
            rpc_api_fallback,
            poll_interval: Duration::from_millis(poll_interval_ms),
            rpc_timeout: Duration::from_millis(rpc_timeout_ms),
            backoff_base_ms,
            backoff_max_ms,
            start_checkpoint,
            stop_checkpoint,
            deepbook_package_ids,
            deepbook_event_type,
            field_pool,
            field_side,
            field_price,
            field_base_sz,
            field_quote_sz,
            field_maker_bm,
            field_taker_bm,
        })
    }
}

#[derive(Debug)]
struct Backoff {
    base_ms: u64,
    max_ms: u64,
    attempt: u32,
}

impl Backoff {
    fn new(base_ms: u64, max_ms: u64) -> Self {
        Self {
            base_ms: base_ms.max(1),
            max_ms: max_ms.max(1),
            attempt: 0,
        }
    }

    fn reset(&mut self) {
        self.attempt = 0;
    }

    fn next_delay(&mut self) -> Duration {
        // Exponential backoff: base * 2^attempt, capped at max_ms
        let shift = self.attempt.min(20);
        let exp = self.base_ms.saturating_mul(1u64 << shift).min(self.max_ms);
        self.attempt = self.attempt.saturating_add(1);
        
        // Add 10% jitter to prevent thundering herd
        let jitter = (exp / 10).min(250);
        let delay_ms = exp + ((std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64) % (jitter + 1));
        
        Duration::from_millis(delay_ms)
    }
}

fn env_opt_u64(name: &'static str) -> anyhow::Result<Option<u64>> {
    match env::var(name) {
        Ok(v) => Ok(Some(
            v.parse::<u64>()
                .with_context(|| format!("invalid {name}: expected u64"))?,
        )),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn env_opt_i64(name: &'static str) -> anyhow::Result<Option<i64>> {
    match env::var(name) {
        Ok(v) => Ok(Some(
            v.parse::<i64>()
                .with_context(|| format!("invalid {name}: expected i64"))?,
        )),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Helper to wait with graceful shutdown support
async fn should_wait_with_shutdown(
    shutdown: &mut std::pin::Pin<&mut impl Future<Output = Result<(), std::io::Error>>>,
    duration: Duration,
) -> bool {
    tokio::select! {
        _ = shutdown => true,
        _ = tokio::time::sleep(duration) => false,
    }
}

async fn fetch_checkpoint_with_fallback(
    primary: &mut SuiRpcClient,
    fallback: Option<&mut SuiRpcClient>,
    seq: u64,
    timeout: Duration,
) -> anyhow::Result<Option<sui_rpc::proto::sui::rpc::v2::Checkpoint>>
{
    // Try primary RPC
    let mut ledger_primary = primary.ledger_client();
    match fetch_checkpoint(|req| ledger_primary.get_checkpoint(req), seq, timeout).await {
        Ok(result) => return Ok(result),
        Err(err) => {
            // If primary fails and we have fallback, try it
            if let Some(fb) = fallback {
                warn!(error=?err, checkpoint=seq, "primary RPC failed; trying fallback");
                let mut ledger_fallback = fb.ledger_client();
                match fetch_checkpoint(|req| ledger_fallback.get_checkpoint(req), seq, timeout).await {
                    Ok(result) => return Ok(result),
                    Err(err2) => {
                        warn!(error=?err2, checkpoint=seq, "fallback RPC also failed");
                        return Err(err2);
                    }
                }
            }
            // No fallback, return primary error
            Err(err)
        }
    }
}

async fn fetch_checkpoint<F, Fut>(
    fetch: F,
    seq: u64,
    timeout: Duration,
) -> anyhow::Result<Option<sui_rpc::proto::sui::rpc::v2::Checkpoint>>
where
    F: FnOnce(GetCheckpointRequest) -> Fut,
    Fut: Future<Output = Result<Response<GetCheckpointResponse>, Status>>,
{
    let mut req = GetCheckpointRequest::by_sequence_number(seq);
    req.read_mask = Some(FieldMask::from_paths([
        "sequence_number",
        "summary",
        "transactions",
    ]));

    let resp = match tokio::time::timeout(timeout, fetch(req)).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(status)) if status.code() == Code::NotFound => return Ok(None),
        Ok(Err(status)) => return Err(anyhow!(status))
            .with_context(|| format!("get_checkpoint({seq}) failed")),
        Err(_) => return Err(anyhow!("get_checkpoint({seq}) timed out after {timeout:?}")),
    };

    let checkpoint = resp.into_inner().checkpoint.ok_or_else(|| 
        anyhow!("checkpoint response empty for sequence {}", seq)
    )?;
    
    let got = checkpoint.sequence_number.context("checkpoint missing sequence_number")?;
    if got != seq {
        anyhow::bail!("checkpoint sequence mismatch: requested {}, got {}", seq, got);
    }

    Ok(Some(checkpoint))
}

#[derive(Parser)]
#[command(name = "indexer", about = "DeepBook indexer & replay tool")]
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let cli = Cli::parse();
    let cfg = IndexerConfig::from_env()?;
    let pool = db::connect(&cfg.database_url).await?;
    sqlx::migrate!("../migrations").run(&pool).await?;

    match cli.command {
        Some(Commands::Replay {
            from_checkpoint,
            to_checkpoint,
        }) => replay_range(&pool, &cfg, from_checkpoint, to_checkpoint).await,
        Some(Commands::Run) | None => run_indexer(pool, cfg).await,
    }
}

async fn run_indexer(pool: PgPool, cfg: IndexerConfig) -> anyhow::Result<()> {

    let mut processed_checkpoint: i64 = sqlx::query_scalar(
        r#"
        SELECT processed_checkpoint
        FROM indexer_state
        WHERE id = 1
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let start_checkpoint = cfg
        .start_checkpoint
        .unwrap_or(processed_checkpoint.saturating_add(1));

    if start_checkpoint > processed_checkpoint.saturating_add(1) {
        anyhow::bail!(
            "INDEXER_START_CHECKPOINT ({}) is ahead of processed_checkpoint ({}); refusing to create a gap",
            start_checkpoint,
            processed_checkpoint
        );
    }

    if let Some(stop) = cfg.stop_checkpoint {
        if stop < start_checkpoint {
            anyhow::bail!(
                "INDEXER_STOP_CHECKPOINT ({}) must be >= start_checkpoint ({})",
                stop,
                start_checkpoint
            );
        }
    }

    info!(
        processed_checkpoint,
        start_checkpoint,
        stop_checkpoint = cfg.stop_checkpoint,
        poll_interval_ms = cfg.poll_interval.as_millis() as u64,
        rpc_timeout_ms = cfg.rpc_timeout.as_millis() as u64,
        rpc_api_url = %cfg.rpc_api_url,
        rpc_api_fallback = cfg.rpc_api_fallback.as_deref(),
        "checkpoint indexer started"
    );

    let mut rpc_client_primary = SuiRpcClient::new(&cfg.rpc_api_url)?;
    let mut rpc_client_fallback = match cfg.rpc_api_fallback.as_deref() {
        Some(url) => Some(SuiRpcClient::new(url)?),
        None => None,
    };

    let mut next_checkpoint = start_checkpoint;
    let mut backoff = Backoff::new(cfg.backoff_base_ms, cfg.backoff_max_ms);
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        // Check for stop condition
        if let Some(stop) = cfg.stop_checkpoint {
            if next_checkpoint > stop {
                info!(stop_checkpoint = stop, "reached stop checkpoint; exiting");
                break;
            }
        }

        // Fetch checkpoint with primary RPC, fallback to secondary if needed
        let checkpoint = tokio::select! {
            _ = &mut shutdown => break,
            res = fetch_checkpoint_with_fallback(
                &mut rpc_client_primary,
                rpc_client_fallback.as_mut(),
                next_checkpoint as u64,
                cfg.rpc_timeout
            ) => res,
        };

        // Handle fetch result
        match checkpoint {
            Ok(Some(cp)) => {
                // Successfully fetched checkpoint, ingest it
                match ingest_checkpoint(&pool, &cfg, cp).await {
                    Ok(seq) => {
                        processed_checkpoint = seq.max(processed_checkpoint);
                        next_checkpoint = seq.saturating_add(1);
                        backoff.reset();
                        info!(checkpoint = seq, "checkpoint committed");
                    }
                    Err(err) => {
                        warn!(error=?err, checkpoint=next_checkpoint, "failed to ingest; will retry");
                        let delay = backoff.next_delay();
                        if should_wait_with_shutdown(&mut shutdown, delay).await {
                            break;
                        }
                    }
                }
            }
            Ok(None) => {
                // Checkpoint not yet available, wait and retry
                backoff.reset();
                if should_wait_with_shutdown(&mut shutdown, cfg.poll_interval).await {
                    break;
                }
            }
            Err(err) => {
                warn!(error=?err, checkpoint=next_checkpoint, "failed to fetch; will retry");
                let delay = backoff.next_delay();
                if should_wait_with_shutdown(&mut shutdown, delay).await {
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn ingest_checkpoint(
    pool: &PgPool,
    cfg: &IndexerConfig,
    checkpoint: sui_rpc::proto::sui::rpc::v2::Checkpoint,
) -> anyhow::Result<i64> {
    let sui_rpc::proto::sui::rpc::v2::Checkpoint {
        sequence_number,
        summary,
        transactions,
        ..
    } = checkpoint;

    let seq: i64 = sequence_number
        .context("checkpoint missing sequence_number")?
        .try_into()
        .context("checkpoint sequence_number does not fit in i64")?;

    let summary = summary
        .ok_or_else(|| anyhow::anyhow!("checkpoint missing summary"))?;
    let timestamp_ms: i64 = summary
        .timestamp
        .as_ref()
        .map(timestamp_to_ms)
        .unwrap_or(0);

    let mut trade_events: Vec<DbEventRow> = Vec::new();

    for executed in transactions {
        let Some(digest) = executed.digest.clone().filter(|d| !d.trim().is_empty()) else {
            continue;
        };

        if let Some(events) = executed.events {
            for (event_seq, event) in events.events.into_iter().enumerate() {
                if let Some(row) = parse_deepbook_trade(
                    cfg,
                    &event,
                    seq,
                    timestamp_ms,
                    event_seq as i32,
                    &digest,
                ) {
                    trade_events.push(row);
                }
            }
        }
    }

    let mut tx = pool.begin().await?;

    queries::insert_db_events(&mut *tx, &trade_events).await?;

    // Recompute rollups for all affected 1-minute buckets from the canonical db_events table.
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

    sqlx::query(
        r#"
        UPDATE indexer_state
        SET processed_checkpoint = GREATEST(processed_checkpoint, $1), updated_at = now()
        WHERE id = 1
        "#,
    )
    .bind(seq)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(seq)
}

fn timestamp_to_ms(ts: &prost_types::Timestamp) -> i64 {
    (ts.seconds as i64)
        .saturating_mul(1_000)
        .saturating_add((ts.nanos as i64) / 1_000_000)
}

fn proto_value_to_json(v: &ProtoValue) -> serde_json::Value {
    use prost_types::value::Kind;

    match v.kind.as_ref() {
        Some(Kind::NullValue(_)) | None => serde_json::Value::Null,
        Some(Kind::NumberValue(n)) => serde_json::json!(n),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s.clone()),
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(*b),
        Some(Kind::StructValue(s)) => struct_to_json(s),
        Some(Kind::ListValue(l)) => list_to_json(l),
    }
}

fn struct_to_json(s: &Struct) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in &s.fields {
        map.insert(k.clone(), proto_value_to_json(v));
    }
    serde_json::Value::Object(map)
}

fn list_to_json(l: &ListValue) -> serde_json::Value {
    serde_json::Value::Array(l.values.iter().map(proto_value_to_json).collect())
}

fn parse_deepbook_trade(
    cfg: &IndexerConfig,
    event: &sui_rpc::proto::sui::rpc::v2::Event,
    checkpoint: i64,
    checkpoint_ts_ms: i64,
    event_seq: i32,
    digest: &str,
) -> Option<DbEventRow> {
    // Fast path: check package ID and event type early
    let package_ok = event.package_id.as_deref().map(|p| {
        let p = p.trim().to_ascii_lowercase();
        cfg.deepbook_package_ids.iter().any(|id| id == &p)
    }).unwrap_or(false);

    if !package_ok
        || !event.event_type.as_deref().unwrap_or("").contains(&cfg.deepbook_event_type)
    {
        return None;
    }

    let body = event
        .json
        .as_deref()
        .map(proto_value_to_json)
        .unwrap_or(serde_json::Value::Null);

    // Extract required fields with fail-fast
    let pool_id = get_string_field(&body, &cfg.field_pool)?;
    let price = get_decimal_field(&body, &cfg.field_price)?;
    let base_sz = get_decimal_field(&body, &cfg.field_base_sz)
        .or_else(|| get_decimal_field(&body, "base_quantity"))?;
    let quote_sz = get_decimal_field(&body, &cfg.field_quote_sz)
        .or_else(|| get_decimal_field(&body, "quote_quantity"))?;

    // Prefer explicit "side" string, fallback to OrderFilled's taker_is_bid boolean.
    let side = if let Some(side_raw) = get_string_field(&body, &cfg.field_side) {
        normalize_side(&side_raw)?
    } else if let Some(is_bid) = get_bool_field(&body, &cfg.field_side) {
        if is_bid { "buy".to_string() } else { "sell".to_string() }
    } else if let Some(is_bid) = get_bool_field(&body, "taker_is_bid") {
        if is_bid { "buy".to_string() } else { "sell".to_string() }
    } else {
        return None;
    };
    let ts = Utc.timestamp_millis_opt(checkpoint_ts_ms).single()?;

    Some(DbEventRow {
        checkpoint,
        ts,
        pool_id,
        side,
        price,
        base_sz,
        quote_sz,
        maker_bm: get_string_field(&body, &cfg.field_maker_bm)
            .or_else(|| get_string_field(&body, "maker_balance_manager_id")),
        taker_bm: get_string_field(&body, &cfg.field_taker_bm)
            .or_else(|| get_string_field(&body, "taker_balance_manager_id")),
        tx_digest: digest.to_string(),
        event_seq,
        event_index: None,
        raw_event: Some(body),
    })
}

fn get_string_field(v: &serde_json::Value, key: &str) -> Option<String> {
    match v.get(key) {
        Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn get_decimal_field(v: &serde_json::Value, key: &str) -> Option<Decimal> {
    match v.get(key) {
        Some(serde_json::Value::Number(n)) => n.to_string().parse().ok(),
        Some(serde_json::Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn get_bool_field(v: &serde_json::Value, key: &str) -> Option<bool> {
    match v.get(key) {
        Some(serde_json::Value::Bool(b)) => Some(*b),
        Some(serde_json::Value::Number(n)) => {
            if n == &serde_json::Number::from(0) {
                Some(false)
            } else if n == &serde_json::Number::from(1) {
                Some(true)
            } else {
                None
            }
        }
        Some(serde_json::Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn normalize_side(s: &str) -> Option<String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "buy" | "bid" => Some("buy".to_string()),
        "sell" | "ask" => Some("sell".to_string()),
        _ => None,
    }
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

        // Update BM metrics (unified logic for maker and taker)
        let mut update_bm = |bm_opt: Option<&String>, is_maker: bool| -> () {
            if let Some(bm) = bm_opt {
                let key = (bm.clone(), ev.pool_id.clone(), bucket);
                let entry = bm_map
                    .entry(key)
                    .or_insert(BmAgg {
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

    let mut pool_rows = Vec::with_capacity(pool_map.len());
    for ((pool_id, bucket_start), agg) in pool_map {
        let vwap = if agg.sum_base.is_zero() {
            None
        } else {
            Some(agg.sum_px_base / agg.sum_base)
        };

        pool_rows.push(PoolMetric1mRow {
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
        });
    }

    let mut bm_rows = Vec::with_capacity(bm_map.len());
    for ((bm_id, pool_id, bucket_start), agg) in bm_map {
        bm_rows.push(BmMetric1mRow {
            bm_id,
            pool_id,
            bucket_start,
            trades: agg.trades,
            volume_quote: agg.volume_quote,
            maker_volume: agg.maker_volume,
            taker_volume: agg.taker_volume,
        });
    }

    (pool_rows, bm_rows)
}

async fn replay_range(
    pool: &PgPool,
    _cfg: &IndexerConfig,
    from: i64,
    to: i64,
) -> anyhow::Result<()> {
    info!(
        from_checkpoint = from,
        to_checkpoint = to,
        "starting replay"
    );

    if from > to {
        anyhow::bail!("from_checkpoint must be <= to_checkpoint");
    }

    let mut tx = pool.begin().await?;

    let events = queries::list_events_in_checkpoint_range(&mut *tx, from, to).await?;
    info!(event_count = events.len(), "fetched events for replay");

    let affected_buckets: Vec<DateTime<Utc>> = events
        .iter()
        .map(|e| truncate_to_minute(e.ts))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if !affected_buckets.is_empty() {
        info!(bucket_count = affected_buckets.len(), "recomputing affected rollup buckets");
    }

    for bucket_start in affected_buckets {
        let bucket_end = bucket_start + ChronoDuration::minutes(1);
        let bucket_events =
            queries::list_events_in_time_range(&mut *tx, bucket_start, bucket_end).await?;
        let (pool_rows, bm_rows) = compute_rollups(&bucket_events);
        info!(bucket_start = %bucket_start, pool_rows = pool_rows.len(), bm_rows = bm_rows.len(), "upserting rollups");
        queries::upsert_pool_metrics(&mut *tx, &pool_rows).await?;
        queries::upsert_bm_metrics(&mut *tx, &bm_rows).await?;
    }

    tx.commit().await?;

    info!("replay complete");
    Ok(())
}
