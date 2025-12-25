use crate::models::{
    BmMetric1mRow,
    DbEventRow,
    EventRow,
    IndexerStateRow,
    ObjectRow,
    PoolMetric1mRow,
    TransactionRow,
};
use sqlx::{PgPool, Postgres, QueryBuilder};
use deepbook_indexer_common::types::{EventCursor, TxCursor};

pub async fn get_indexer_state(pool: &PgPool) -> Result<IndexerStateRow, sqlx::Error> {
    sqlx::query_as::<_, IndexerStateRow>(
        r#"
        SELECT processed_checkpoint, updated_at
        FROM indexer_state
        WHERE id = 1
        "#,
    )
    .fetch_one(pool)
    .await
}

pub async fn get_transaction(
    pool: &PgPool,
    digest: &str,
) -> Result<Option<TransactionRow>, sqlx::Error> {
    sqlx::query_as::<_, TransactionRow>(
        r#"
        SELECT digest, sender, checkpoint, timestamp_ms, raw
        FROM transactions
        WHERE digest = $1
        "#,
    )
    .bind(digest)
    .fetch_optional(pool)
    .await
}

pub async fn list_transactions_by_address(
    pool: &PgPool,
    address: &str,
    limit: i64,
    cursor: Option<&TxCursor>,
) -> Result<Vec<TransactionRow>, sqlx::Error> {
    if let Some(cursor) = cursor {
        sqlx::query_as::<_, TransactionRow>(
            r#"
            SELECT t.digest, t.sender, t.checkpoint, t.timestamp_ms, t.raw
            FROM address_transactions addr_tx
            JOIN transactions t ON t.digest = addr_tx.digest
            WHERE addr_tx.address = $1
              AND (addr_tx.checkpoint, addr_tx.digest) < ($2, $3)
            ORDER BY addr_tx.checkpoint DESC, addr_tx.digest DESC
            LIMIT $4
            "#,
        )
        .bind(address)
        .bind(cursor.checkpoint)
        .bind(&cursor.digest)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, TransactionRow>(
            r#"
            SELECT t.digest, t.sender, t.checkpoint, t.timestamp_ms, t.raw
            FROM address_transactions addr_tx
            JOIN transactions t ON t.digest = addr_tx.digest
            WHERE addr_tx.address = $1
            ORDER BY addr_tx.checkpoint DESC, addr_tx.digest DESC
            LIMIT $2
            "#,
        )
        .bind(address)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}

pub async fn list_events(
    pool: &PgPool,
    address: Option<&str>,
    event_type: Option<&str>,
    limit: i64,
    cursor: Option<&EventCursor>,
) -> Result<Vec<EventRow>, sqlx::Error> {
    let mut qb = QueryBuilder::<Postgres>::new(
        "SELECT id, digest, checkpoint, timestamp_ms, sender, event_type, raw FROM events",
    );

    let mut has_where = false;
    if let Some(address) = address {
        qb.push(if has_where { " AND " } else { " WHERE " });
        has_where = true;
        qb.push("sender = ");
        qb.push_bind(address);
    }

    if let Some(event_type) = event_type {
        qb.push(if has_where { " AND " } else { " WHERE " });
        has_where = true;
        qb.push("event_type = ");
        qb.push_bind(event_type);
    }

    if let Some(cursor) = cursor {
        qb.push(if has_where { " AND " } else { " WHERE " });
        qb.push("(checkpoint, id) < (");
        qb.push_bind(cursor.checkpoint);
        qb.push(", ");
        qb.push_bind(cursor.id);
        qb.push(")");
    }

    qb.push(" ORDER BY checkpoint DESC, id DESC LIMIT ");
    qb.push_bind(limit);

    qb.build_query_as::<EventRow>().fetch_all(pool).await
}

pub async fn get_object(pool: &PgPool, object_id: &str) -> Result<Option<ObjectRow>, sqlx::Error> {
    sqlx::query_as::<_, ObjectRow>(
        r#"
        SELECT object_id, owner, object_type, version, raw, updated_checkpoint
        FROM objects
        WHERE object_id = $1
        "#,
    )
    .bind(object_id)
    .fetch_optional(pool)
    .await
}

// --- DeepBook-specific helpers ---

pub async fn insert_db_events(pool: &PgPool, events: &[DbEventRow]) -> Result<(), sqlx::Error> {
    if events.is_empty() {
        return Ok(());
    }

    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO db_events (checkpoint, ts, pool_id, side, price, base_sz, quote_sz, maker_bm, taker_bm, tx_digest, event_seq, event_index, raw_event) ",
    );

    qb.push_values(events, |mut b, ev| {
        b.push_bind(ev.checkpoint)
            .push_bind(ev.ts)
            .push_bind(&ev.pool_id)
            .push_bind(&ev.side)
            .push_bind(&ev.price)
            .push_bind(&ev.base_sz)
            .push_bind(&ev.quote_sz)
            .push_bind(&ev.maker_bm)
            .push_bind(&ev.taker_bm)
            .push_bind(&ev.tx_digest)
            .push_bind(ev.event_seq)
            .push_bind(ev.event_index)
            .push_bind(&ev.raw_event);
    });

    qb.push(" ON CONFLICT (tx_digest, event_seq) DO NOTHING");
    qb.build().execute(pool).await.map(|_| ())
}

pub async fn upsert_pool_metrics(
    pool: &PgPool,
    rows: &[PoolMetric1mRow],
) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO pool_metrics_1m (pool_id, bucket_start, trades, volume_base, volume_quote, maker_volume, taker_volume, fees_quote, avg_price, vwap, last_price) ",
    );

    qb.push_values(rows, |mut b, r| {
        b.push_bind(&r.pool_id)
            .push_bind(r.bucket_start)
            .push_bind(r.trades)
            .push_bind(&r.volume_base)
            .push_bind(&r.volume_quote)
            .push_bind(&r.maker_volume)
            .push_bind(&r.taker_volume)
            .push_bind(&r.fees_quote)
            .push_bind(&r.avg_price)
            .push_bind(&r.vwap)
            .push_bind(&r.last_price);
    });

    qb.push(
        " ON CONFLICT (pool_id, bucket_start) DO UPDATE SET
          trades = EXCLUDED.trades,
          volume_base = EXCLUDED.volume_base,
          volume_quote = EXCLUDED.volume_quote,
          maker_volume = EXCLUDED.maker_volume,
          taker_volume = EXCLUDED.taker_volume,
          fees_quote = EXCLUDED.fees_quote,
          avg_price = EXCLUDED.avg_price,
          vwap = EXCLUDED.vwap,
          last_price = EXCLUDED.last_price",
    );

    qb.build().execute(pool).await.map(|_| ())
}

pub async fn upsert_bm_metrics(
    pool: &PgPool,
    rows: &[BmMetric1mRow],
) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut qb = QueryBuilder::<Postgres>::new(
        "INSERT INTO bm_metrics_1m (bm_id, pool_id, bucket_start, trades, volume_quote, maker_volume, taker_volume) ",
    );

    qb.push_values(rows, |mut b, r| {
        b.push_bind(&r.bm_id)
            .push_bind(&r.pool_id)
            .push_bind(r.bucket_start)
            .push_bind(r.trades)
            .push_bind(&r.volume_quote)
            .push_bind(&r.maker_volume)
            .push_bind(&r.taker_volume);
    });

    qb.push(
        " ON CONFLICT (bm_id, pool_id, bucket_start) DO UPDATE SET
          trades = EXCLUDED.trades,
          volume_quote = EXCLUDED.volume_quote,
          maker_volume = EXCLUDED.maker_volume,
          taker_volume = EXCLUDED.taker_volume",
    );

    qb.build().execute(pool).await.map(|_| ())
}

pub async fn list_events_in_checkpoint_range(
    pool: &PgPool,
    from: i64,
    to: i64,
) -> Result<Vec<DbEventRow>, sqlx::Error> {
    sqlx::query_as::<_, DbEventRow>(
        r#"
        SELECT checkpoint, ts, pool_id, side, price, base_sz, quote_sz, maker_bm, taker_bm,
               tx_digest, event_seq, event_index, raw_event
        FROM db_events
        WHERE checkpoint BETWEEN $1 AND $2
        ORDER BY checkpoint, event_seq
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
}
