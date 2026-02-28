package store

import (
	"context"
	"fmt"
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

type Candle struct {
	BucketStart time.Time        `json:"bucket_start"`
	Trades      int64            `json:"trades"`
	VolumeBase  decimal.Decimal  `json:"volume_base"`
	VolumeQuote decimal.Decimal  `json:"volume_quote"`
	Open        *decimal.Decimal `json:"open,omitempty"`
	High        *decimal.Decimal `json:"high,omitempty"`
	Low         *decimal.Decimal `json:"low,omitempty"`
	Close       *decimal.Decimal `json:"close,omitempty"`
	VWAP        *decimal.Decimal `json:"vwap,omitempty"`
}

type CandleSeries struct {
	PoolID   string    `json:"pool_id"`
	Window   string    `json:"window"`
	Interval string    `json:"interval"`
	StartTs  time.Time `json:"start_ts"`
	EndTs    time.Time `json:"end_ts"`
	Candles  []Candle  `json:"candles"`
}

type ExecutionSummary struct {
	PoolID            string           `json:"pool_id"`
	Window            string           `json:"window"`
	StartTs           time.Time        `json:"start_ts"`
	EndTs             time.Time        `json:"end_ts"`
	Trades            int64            `json:"trades"`
	VolumeBase        decimal.Decimal  `json:"volume_base"`
	VolumeQuote       decimal.Decimal  `json:"volume_quote"`
	BuyTrades         int64            `json:"buy_trades"`
	SellTrades        int64            `json:"sell_trades"`
	AvgTradeNotional  decimal.Decimal  `json:"avg_trade_notional"`
	VWAP              *decimal.Decimal `json:"vwap,omitempty"`
	PriceChangeBps    *float64         `json:"price_change_bps,omitempty"`
	OrderImbalanceBps *float64         `json:"order_imbalance_bps,omitempty"`
	ExecutionScore    *float64         `json:"execution_score,omitempty"`
}

type OrderLifecycleEvent struct {
	Checkpoint       int64            `json:"checkpoint"`
	TsMs             int64            `json:"ts_ms"`
	PoolID           string           `json:"pool_id"`
	EventType        string           `json:"event_type"`
	OrderID          *string          `json:"order_id,omitempty"`
	Trader           *string          `json:"trader,omitempty"`
	IsBid            *bool            `json:"is_bid,omitempty"`
	Price            *decimal.Decimal `json:"price,omitempty"`
	OriginalQuantity *decimal.Decimal `json:"original_quantity,omitempty"`
	NewQuantity      *decimal.Decimal `json:"new_quantity,omitempty"`
	CanceledQuantity *decimal.Decimal `json:"canceled_quantity,omitempty"`
	TxDigest         string           `json:"tx_digest"`
	EventSeq         int32            `json:"event_seq"`
}

type OrderLifecycleCursor struct {
	TsMs       int64
	Checkpoint int64
	EventSeq   int32
}

func (s *Store) GetPoolMetrics(ctx context.Context, poolID string, window string) (*PoolMetrics, error) {
	var interval string
	var dur time.Duration
	switch window {
	case "1h":
		interval = "1 hour"
		dur = time.Hour
	case "24h":
		interval = "24 hours"
		dur = 24 * time.Hour
	default:
		interval = "1 hour"
		dur = time.Hour
	}

	query := `
		SELECT 
			$1 AS pool_id,
			$2 AS window,
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

	now := time.Now().UTC()
	m.StartTs = now.Add(-dur)
	m.EndTs = now

	return &m, nil
}

func (s *Store) GetPoolCandles(ctx context.Context, poolID string, window string, interval string) (*CandleSeries, error) {
	var windowInterval string
	var windowDur time.Duration
	switch window {
	case "1h":
		windowInterval = "1 hour"
		windowDur = time.Hour
	case "24h":
		windowInterval = "24 hours"
		windowDur = 24 * time.Hour
	case "7d":
		windowInterval = "7 days"
		windowDur = 7 * 24 * time.Hour
	default:
		window = "1h"
		windowInterval = "1 hour"
		windowDur = time.Hour
	}

	var intervalSec int64
	switch interval {
	case "1m":
		intervalSec = 60
	case "5m":
		intervalSec = 5 * 60
	case "15m":
		intervalSec = 15 * 60
	case "1h":
		intervalSec = 60 * 60
	default:
		interval = "1m"
		intervalSec = 60
	}

	var rowsQuery string
	var args []interface{}
	if intervalSec == 60 {
		rowsQuery = `
			SELECT
				bucket_start,
				trades,
				volume_base,
				volume_quote,
				COALESCE(open_price, last_price) AS open,
				COALESCE(high_price, last_price) AS high,
				COALESCE(low_price, last_price) AS low,
				last_price AS close,
				vwap
			FROM pool_metrics_1m
			WHERE pool_id = $1
			  AND bucket_start >= NOW() - $2::INTERVAL
			ORDER BY bucket_start ASC
		`
		args = []interface{}{poolID, windowInterval}
	} else {
		rowsQuery = `
			SELECT
				to_timestamp(floor(extract(epoch from bucket_start) / $2) * $2) AT TIME ZONE 'UTC' AS bucket_start,
				COALESCE(SUM(trades), 0) AS trades,
				COALESCE(SUM(volume_base), 0) AS volume_base,
				COALESCE(SUM(volume_quote), 0) AS volume_quote,
				(ARRAY_AGG(COALESCE(open_price, last_price) ORDER BY bucket_start ASC))[1] AS open,
				MAX(COALESCE(high_price, last_price)) AS high,
				MIN(COALESCE(low_price, last_price)) AS low,
				(ARRAY_AGG(last_price ORDER BY bucket_start DESC))[1] AS close,
				CASE WHEN SUM(volume_base) > 0 THEN SUM(vwap * volume_base) / SUM(volume_base) END AS vwap
			FROM pool_metrics_1m
			WHERE pool_id = $1
			  AND bucket_start >= NOW() - $3::INTERVAL
			GROUP BY 1
			ORDER BY 1 ASC
		`
		args = []interface{}{poolID, intervalSec, windowInterval}
	}

	rows, err := s.pool.Query(ctx, rowsQuery, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var candles []Candle
	for rows.Next() {
		var c Candle
		if err := rows.Scan(&c.BucketStart, &c.Trades, &c.VolumeBase, &c.VolumeQuote, &c.Open, &c.High, &c.Low, &c.Close, &c.VWAP); err != nil {
			return nil, err
		}
		candles = append(candles, c)
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}

	now := time.Now().UTC()
	return &CandleSeries{
		PoolID:   poolID,
		Window:   window,
		Interval: interval,
		StartTs:  now.Add(-windowDur),
		EndTs:    now,
		Candles:  candles,
	}, nil
}

func (s *Store) GetBMVolume(ctx context.Context, bmID string, window string, poolFilter []string) (*BMVolume, error) {
	var interval string
	var dur time.Duration
	switch window {
	case "24h":
		interval = "24 hours"
		dur = 24 * time.Hour
	case "7d":
		interval = "7 days"
		dur = 7 * 24 * time.Hour
	default:
		interval = "24 hours"
		dur = 24 * time.Hour
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

	now := time.Now().UTC()
	return &BMVolume{
		BMID:             bmID,
		Window:           window,
		StartTs:          now.Add(-dur),
		EndTs:            now,
		TotalVolumeQuote: totalVolume,
		Breakdown:        breakdown,
	}, nil
}

func (s *Store) GetExecutionSummary(ctx context.Context, poolID string, window string) (*ExecutionSummary, error) {
	var interval string
	var dur time.Duration
	switch window {
	case "1h":
		interval = "1 hour"
		dur = time.Hour
	case "24h":
		interval = "24 hours"
		dur = 24 * time.Hour
	case "7d":
		interval = "7 days"
		dur = 7 * 24 * time.Hour
	default:
		window = "1h"
		interval = "1 hour"
		dur = time.Hour
	}

	query := `
		SELECT
			$1 AS pool_id,
			$2 AS window,
			COALESCE(COUNT(*), 0) AS trades,
			COALESCE(SUM(base_sz), 0) AS volume_base,
			COALESCE(SUM(quote_sz), 0) AS volume_quote,
			COALESCE(SUM(CASE WHEN side = 'buy' THEN 1 ELSE 0 END), 0) AS buy_trades,
			COALESCE(SUM(CASE WHEN side = 'sell' THEN 1 ELSE 0 END), 0) AS sell_trades,
			CASE WHEN COUNT(*) > 0 THEN COALESCE(SUM(quote_sz), 0) / COUNT(*) ELSE 0 END AS avg_trade_notional,
			CASE WHEN COALESCE(SUM(base_sz), 0) > 0 THEN COALESCE(SUM(price * base_sz), 0) / COALESCE(SUM(base_sz), 0) END AS vwap,
			(ARRAY_AGG(price ORDER BY ts ASC))[1] AS first_price,
			(ARRAY_AGG(price ORDER BY ts DESC))[1] AS last_price
		FROM db_events
		WHERE pool_id = $1
		  AND ts >= NOW() - $3::INTERVAL
	`

	var summary ExecutionSummary
	var firstPrice *decimal.Decimal
	var lastPrice *decimal.Decimal
	err := s.pool.QueryRow(ctx, query, poolID, window, interval).Scan(
		&summary.PoolID,
		&summary.Window,
		&summary.Trades,
		&summary.VolumeBase,
		&summary.VolumeQuote,
		&summary.BuyTrades,
		&summary.SellTrades,
		&summary.AvgTradeNotional,
		&summary.VWAP,
		&firstPrice,
		&lastPrice,
	)
	if err != nil {
		return nil, err
	}

	now := time.Now().UTC()
	summary.StartTs = now.Add(-dur)
	summary.EndTs = now

	if summary.Trades > 0 {
		imbalance := float64(summary.BuyTrades-summary.SellTrades) / float64(summary.Trades) * 10_000
		summary.OrderImbalanceBps = &imbalance

		if firstPrice != nil && lastPrice != nil && !firstPrice.IsZero() {
			change, _ := lastPrice.Sub(*firstPrice).Div(*firstPrice).Mul(decimal.NewFromInt(10_000)).Float64()
			summary.PriceChangeBps = &change
		}

		score := 50.0
		if summary.PriceChangeBps != nil {
			score += clamp(*summary.PriceChangeBps/20.0, -25, 25)
		}
		if summary.OrderImbalanceBps != nil {
			score += clamp(*summary.OrderImbalanceBps/200.0, -15, 15)
		}
		if score < 0 {
			score = 0
		}
		if score > 100 {
			score = 100
		}
		summary.ExecutionScore = &score
	}

	return &summary, nil
}

func clamp(v float64, min float64, max float64) float64 {
	if v < min {
		return min
	}
	if v > max {
		return max
	}
	return v
}

func (s *Store) GetOrderLifecycleEvents(ctx context.Context, poolID string, window string, eventType string, limit int, cursor *OrderLifecycleCursor) ([]OrderLifecycleEvent, error) {
	var interval string
	switch window {
	case "1h":
		interval = "1 hour"
	case "24h":
		interval = "24 hours"
	case "7d":
		interval = "7 days"
	default:
		interval = "1 hour"
	}

	if limit <= 0 {
		limit = 100
	}
	if limit > 1000 {
		limit = 1000
	}

	query := `
		SELECT checkpoint,
		       EXTRACT(EPOCH FROM ts) * 1000 AS ts_ms,
		       pool_id,
		       event_type,
		       order_id,
		       trader,
		       is_bid,
		       price,
		       original_quantity,
		       new_quantity,
		       canceled_quantity,
		       tx_digest,
		       event_seq
		FROM db_order_events
		WHERE pool_id = $1
		  AND ts >= NOW() - $2::INTERVAL
	`

	args := []interface{}{poolID, interval}
	nextArg := 3
	if eventType != "" {
		query += " AND event_type = $3"
		args = append(args, eventType)
		nextArg = 4
	}

	if cursor != nil {
		query += fmt.Sprintf(" AND ((EXTRACT(EPOCH FROM ts) * 1000) < $%d OR ((EXTRACT(EPOCH FROM ts) * 1000) = $%d AND (checkpoint, event_seq) < ($%d, $%d)))", nextArg, nextArg, nextArg+1, nextArg+2)
		args = append(args, cursor.TsMs, cursor.Checkpoint, cursor.EventSeq)
		nextArg += 3
	}

	query += fmt.Sprintf(" ORDER BY ts DESC, checkpoint DESC, event_seq DESC LIMIT $%d", nextArg)
	args = append(args, limit)

	rows, err := s.pool.Query(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	out := make([]OrderLifecycleEvent, 0)
	for rows.Next() {
		var e OrderLifecycleEvent
		if err := rows.Scan(
			&e.Checkpoint,
			&e.TsMs,
			&e.PoolID,
			&e.EventType,
			&e.OrderID,
			&e.Trader,
			&e.IsBid,
			&e.Price,
			&e.OriginalQuantity,
			&e.NewQuantity,
			&e.CanceledQuantity,
			&e.TxDigest,
			&e.EventSeq,
		); err != nil {
			return nil, err
		}
		out = append(out, e)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return out, nil
}

func (s *Store) StreamTrades(ctx context.Context, poolFilter []string, out chan<- *TradeEvent) error {
	type cursor struct {
		checkpoint int64
		txDigest   string
		eventSeq   int32
	}

	cur := cursor{checkpoint: 0, txDigest: "", eventSeq: 0}
	pollInterval := 1 * time.Second

	// Send a small backlog first (latest 100), oldest -> newest.
	{
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
		query += ` ORDER BY checkpoint DESC, tx_digest DESC, event_seq DESC LIMIT 100`

		rows, err := s.pool.Query(ctx, query, args...)
		if err != nil {
			return err
		}
		var backlog []*TradeEvent
		for rows.Next() {
			var ev TradeEvent
			var ts time.Time
			if err := rows.Scan(&ev.Checkpoint, &ts, &ev.PoolID, &ev.Side, &ev.Price, &ev.BaseSz, &ev.QuoteSz, &ev.MakerBM, &ev.TakerBM, &ev.TxDigest, &ev.EventSeq); err != nil {
				rows.Close()
				return err
			}
			ev.Type = "trade"
			ev.TsMs = ts.UnixMilli()
			backlog = append(backlog, &ev)
		}
		rows.Close()
		if err := rows.Err(); err != nil {
			return err
		}

		// Reverse so client sees oldest first.
		for i, j := 0, len(backlog)-1; i < j; i, j = i+1, j-1 {
			backlog[i], backlog[j] = backlog[j], backlog[i]
		}

		for _, ev := range backlog {
			select {
			case out <- ev:
				cur = cursor{checkpoint: ev.Checkpoint, txDigest: ev.TxDigest, eventSeq: ev.EventSeq}
			case <-ctx.Done():
				return ctx.Err()
			}
		}
	}

	for {
		if err := ctx.Err(); err != nil {
			return err
		}

		var query string
		var args []interface{}
		if len(poolFilter) > 0 {
			query = `
				SELECT checkpoint, ts, pool_id, side, price, base_sz, quote_sz, maker_bm, taker_bm, tx_digest, event_seq
				FROM db_events
				WHERE pool_id = ANY($1)
				  AND (checkpoint, tx_digest, event_seq) > ($2, $3, $4)
				ORDER BY checkpoint ASC, tx_digest ASC, event_seq ASC
				LIMIT 500
			`
			args = []interface{}{poolFilter, cur.checkpoint, cur.txDigest, cur.eventSeq}
		} else {
			query = `
				SELECT checkpoint, ts, pool_id, side, price, base_sz, quote_sz, maker_bm, taker_bm, tx_digest, event_seq
				FROM db_events
				WHERE (checkpoint, tx_digest, event_seq) > ($1, $2, $3)
				ORDER BY checkpoint ASC, tx_digest ASC, event_seq ASC
				LIMIT 500
			`
			args = []interface{}{cur.checkpoint, cur.txDigest, cur.eventSeq}
		}

		rows, err := s.pool.Query(ctx, query, args...)
		if err != nil {
			return err
		}

		sent := 0
		for rows.Next() {
			var ev TradeEvent
			var ts time.Time
			if err := rows.Scan(&ev.Checkpoint, &ts, &ev.PoolID, &ev.Side, &ev.Price, &ev.BaseSz, &ev.QuoteSz, &ev.MakerBM, &ev.TakerBM, &ev.TxDigest, &ev.EventSeq); err != nil {
				rows.Close()
				return err
			}
			ev.Type = "trade"
			ev.TsMs = ts.UnixMilli()

			select {
			case out <- &ev:
				cur = cursor{checkpoint: ev.Checkpoint, txDigest: ev.TxDigest, eventSeq: ev.EventSeq}
				sent++
			case <-ctx.Done():
				rows.Close()
				return ctx.Err()
			}
		}
		rows.Close()
		if err := rows.Err(); err != nil {
			return err
		}

		if sent == 0 {
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(pollInterval):
			}
		}
	}
}
