package store

import (
	"context"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/shopspring/decimal"
)

type Store struct {
	pool *pgxpool.Pool
}

func New(databaseURL string) (*Store, error) {
	pool, err := pgxpool.New(context.Background(), databaseURL)
	if err != nil {
		return nil, err
	}
	return &Store{pool: pool}, nil
}

func (s *Store) Close() {
	s.pool.Close()
}

type PoolMetrics struct {
	PoolID      string           `json:"pool_id"`
	Window      string           `json:"window"`
	StartTs     time.Time        `json:"start_ts"`
	EndTs       time.Time        `json:"end_ts"`
	Trades      int64            `json:"trades"`
	VolumeBase  decimal.Decimal  `json:"volume_base"`
	VolumeQuote decimal.Decimal  `json:"volume_quote"`
	MakerVolume decimal.Decimal  `json:"maker_volume"`
	TakerVolume decimal.Decimal  `json:"taker_volume"`
	FeesQuote   *decimal.Decimal `json:"fees_quote,omitempty"`
	AvgPrice    *decimal.Decimal `json:"avg_price,omitempty"`
	VWAP        *decimal.Decimal `json:"vwap,omitempty"`
	LastPrice   *decimal.Decimal `json:"last_price,omitempty"`
}

type BMVolume struct {
	BMID             string            `json:"bm_id"`
	Window           string            `json:"window"`
	StartTs          time.Time         `json:"start_ts"`
	EndTs            time.Time         `json:"end_ts"`
	TotalVolumeQuote decimal.Decimal   `json:"total_volume_quote"`
	Breakdown        []BMPoolBreakdown `json:"breakdown"`
}

type BMPoolBreakdown struct {
	PoolID      string          `json:"pool_id"`
	VolumeQuote decimal.Decimal `json:"volume_quote"`
	Trades      int64           `json:"trades"`
}

type TradeEvent struct {
	Type       string          `json:"type"`
	TsMs       int64           `json:"ts_ms"`
	PoolID     string          `json:"pool_id"`
	Side       string          `json:"side"`
	Price      decimal.Decimal `json:"price"`
	BaseSz     decimal.Decimal `json:"base_sz"`
	QuoteSz    decimal.Decimal `json:"quote_sz"`
	MakerBM    *string         `json:"maker_bm,omitempty"`
	TakerBM    *string         `json:"taker_bm,omitempty"`
	TxDigest   string          `json:"tx_digest"`
	EventSeq   int32           `json:"event_seq"`
	Checkpoint int64           `json:"checkpoint"`
}

func (s *Store) GetPoolMetrics(ctx context.Context, poolID string, window string) (*PoolMetrics, error) {
	var interval string
	switch window {
	case "1h":
		interval = "1 hour"
	case "24h":
		interval = "24 hours"
	default:
		interval = "1 hour"
	}

	query := `
		SELECT 
			$1 AS pool_id,
			$2 AS window,
			MIN(bucket_start) AS start_ts,
			MAX(bucket_start) + INTERVAL '1 minute' AS end_ts,
			COALESCE(SUM(trades), 0) AS trades,
			COALESCE(SUM(volume_base), 0) AS volume_base,
			COALESCE(SUM(volume_quote), 0) AS volume_quote,
			COALESCE(SUM(maker_volume), 0) AS maker_volume,
			COALESCE(SUM(taker_volume), 0) AS taker_volume,
			CASE WHEN SUM(volume_base) > 0 THEN SUM(vwap * volume_base) / SUM(volume_base) END AS vwap,
			(ARRAY_AGG(last_price ORDER BY bucket_start DESC))[1] AS last_price
		FROM pool_metrics_1m
		WHERE pool_id = $1
		  AND bucket_start >= NOW() - $3::INTERVAL
	`

	var m PoolMetrics
	err := s.pool.QueryRow(ctx, query, poolID, window, interval).Scan(
		&m.PoolID,
		&m.Window,
		&m.StartTs,
		&m.EndTs,
		&m.Trades,
		&m.VolumeBase,
		&m.VolumeQuote,
		&m.MakerVolume,
		&m.TakerVolume,
		&m.VWAP,
		&m.LastPrice,
	)
	if err != nil {
		return nil, err
	}

	return &m, nil
}

func (s *Store) GetBMVolume(ctx context.Context, bmID string, window string, poolFilter []string) (*BMVolume, error) {
	var interval string
	switch window {
	case "24h":
		interval = "24 hours"
	case "7d":
		interval = "7 days"
	default:
		interval = "24 hours"
	}

	query := `
		SELECT 
			bm_id,
			pool_id,
			SUM(volume_quote) AS volume_quote,
			SUM(trades) AS trades
		FROM bm_metrics_1m
		WHERE bm_id = $1
		  AND bucket_start >= NOW() - $2::INTERVAL
	`
	args := []interface{}{bmID, interval}

	if len(poolFilter) > 0 {
		query += ` AND pool_id = ANY($3)`
		args = append(args, poolFilter)
	}

	query += ` GROUP BY bm_id, pool_id`

	rows, err := s.pool.Query(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var breakdown []BMPoolBreakdown
	var totalVolume decimal.Decimal
	for rows.Next() {
		var bmIDRes, poolID string
		var volQuote decimal.Decimal
		var trades int64
		if err := rows.Scan(&bmIDRes, &poolID, &volQuote, &trades); err != nil {
			return nil, err
		}
		breakdown = append(breakdown, BMPoolBreakdown{
			PoolID:      poolID,
			VolumeQuote: volQuote,
			Trades:      trades,
		})
		totalVolume = totalVolume.Add(volQuote)
	}

	return &BMVolume{
		BMID:             bmID,
		Window:           window,
		StartTs:          time.Now().Add(-parseDuration(interval)),
		EndTs:            time.Now(),
		TotalVolumeQuote: totalVolume,
		Breakdown:        breakdown,
	}, nil
}

func (s *Store) StreamTrades(ctx context.Context, poolFilter []string, out chan<- *TradeEvent) error {
	query := `
		SELECT checkpoint, ts, pool_id, side, price, base_sz, quote_sz, maker_bm, taker_bm, tx_digest, event_seq
		FROM db_events
		WHERE 1=1
	`
	args := []interface{}{}
	if len(poolFilter) > 0 {
		query += ` AND pool_id = ANY($1)`
		args = append(args, poolFilter)
	}
	query += ` ORDER BY checkpoint DESC, event_seq DESC LIMIT 100`

	rows, err := s.pool.Query(ctx, query, args...)
	if err != nil {
		return err
	}
	defer rows.Close()

	for rows.Next() {
		var ev TradeEvent
		var ts time.Time
		if err := rows.Scan(&ev.Checkpoint, &ts, &ev.PoolID, &ev.Side, &ev.Price, &ev.BaseSz, &ev.QuoteSz, &ev.MakerBM, &ev.TakerBM, &ev.TxDigest, &ev.EventSeq); err != nil {
			return err
		}
		ev.Type = "trade"
		ev.TsMs = ts.UnixMilli()
		out <- &ev
	}

	return nil
}

func parseDuration(s string) time.Duration {
	d, _ := time.ParseDuration(s)
	return d
}
