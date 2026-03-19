package handlers

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/gorilla/websocket"

	"github.com/Lab-JY/deepbook-indexer/api-go/internal/store"
)

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool {
		return true
	},
}

type Handler struct {
	store          storeBackend
	singleKey      string
	wsPingInterval time.Duration
}

type storeBackend interface {
	ListAssets(ctx context.Context) ([]store.AssetMetadata, error)
	ListPools(ctx context.Context) ([]store.PoolMetadata, error)
	GetPoolMetadata(ctx context.Context, poolID string) (*store.PoolMetadata, error)
	GetPoolMetrics(ctx context.Context, poolID string, window string) (*store.PoolMetrics, error)
	GetPoolCandles(ctx context.Context, poolID string, window string, interval string) (*store.CandleSeries, error)
	GetExecutionSummary(ctx context.Context, poolID string, window string) (*store.ExecutionSummary, error)
	GetOrderLifecycleEvents(ctx context.Context, poolID string, window string, eventType string, limit int, cursor *store.OrderLifecycleCursor) ([]store.OrderLifecycleEvent, error)
	GetExecutionFills(ctx context.Context, poolID string, window string, limit int, cursor *store.OrderLifecycleCursor) ([]store.ExecutionFill, error)
	GetBMVolume(ctx context.Context, bmID string, window string, poolFilter []string) (*store.BMVolume, error)
	StreamTrades(ctx context.Context, poolFilter []string, out chan<- *store.TradeEvent) error
}

var _ storeBackend = (*store.Store)(nil)

func New(store storeBackend, singleKey string, wsPingInterval time.Duration) *Handler {
	if wsPingInterval <= 0 {
		wsPingInterval = 15 * time.Second
	}
	return &Handler{store: store, singleKey: singleKey, wsPingInterval: wsPingInterval}
}

func (h *Handler) GetAssets(c *gin.Context) {
	assets, err := h.store.ListAssets(c.Request.Context())
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"count":  len(assets),
		"assets": assets,
	})
}

func (h *Handler) GetPools(c *gin.Context) {
	pools, err := h.store.ListPools(c.Request.Context())
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"count": len(pools),
		"pools": pools,
	})
}

func (h *Handler) GetPoolMetadata(c *gin.Context) {
	poolID := c.Param("pool_id")
	if strings.TrimSpace(poolID) == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "pool_id is required"})
		return
	}

	pool, err := h.store.GetPoolMetadata(c.Request.Context(), poolID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}
	if pool == nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "pool metadata not found"})
		return
	}

	c.JSON(http.StatusOK, pool)
}

func (h *Handler) AuthMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		if h.singleKey == "" {
			c.Next()
			return
		}
		auth := c.GetHeader("Authorization")
		token := strings.TrimPrefix(auth, "Bearer ")
		if token != h.singleKey {
			c.JSON(http.StatusUnauthorized, gin.H{"error": "unauthorized"})
			c.Abort()
			return
		}
		c.Next()
	}
}

func (h *Handler) GetPoolMetrics(c *gin.Context) {
	poolID := c.Param("pool_id")
	window := c.DefaultQuery("window", "1h")

	metrics, err := h.store.GetPoolMetrics(c.Request.Context(), poolID, window)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, metrics)
}

func (h *Handler) GetPoolCandles(c *gin.Context) {
	poolID := c.Param("pool_id")
	window := c.DefaultQuery("window", "1h")
	interval := c.DefaultQuery("interval", "1m")

	series, err := h.store.GetPoolCandles(c.Request.Context(), poolID, window, interval)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, series)
}

func (h *Handler) GetExecutionSummary(c *gin.Context) {
	poolID := c.Param("pool_id")
	window := c.DefaultQuery("window", "1h")

	summary, err := h.store.GetExecutionSummary(c.Request.Context(), poolID, window)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, summary)
}

func (h *Handler) GetOrderLifecycle(c *gin.Context) {
	poolID := c.Param("pool_id")
	if strings.TrimSpace(poolID) == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "pool_id is required"})
		return
	}
	window := c.DefaultQuery("window", "1h")
	eventType := c.Query("event_type")
	limitStr := c.DefaultQuery("limit", "100")
	limit, err := strconv.Atoi(limitStr)
	if err != nil {
		limit = 100
	}
	cursor, err := parseLifecycleCursor(c.Query("cursor"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid cursor format"})
		return
	}

	events, err := h.store.GetOrderLifecycleEvents(c.Request.Context(), poolID, window, eventType, limit, cursor)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	nextCursor := ""
	if len(events) == limit && len(events) > 0 {
		last := events[len(events)-1]
		nextCursor = fmt.Sprintf("%d|%d|%d", last.TsMs, last.Checkpoint, last.EventSeq)
	}

	c.JSON(http.StatusOK, gin.H{
		"pool_id":     poolID,
		"window":      window,
		"event_type":  eventType,
		"count":       len(events),
		"next_cursor": nextCursor,
		"events":      events,
	})
}

func (h *Handler) GetExecutionFills(c *gin.Context) {
	poolID := c.Param("pool_id")
	if strings.TrimSpace(poolID) == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "pool_id is required"})
		return
	}
	window := c.DefaultQuery("window", "1h")
	limitStr := c.DefaultQuery("limit", "100")
	limit, err := strconv.Atoi(limitStr)
	if err != nil {
		limit = 100
	}
	cursor, err := parseLifecycleCursor(c.Query("cursor"))
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid cursor format"})
		return
	}

	fills, err := h.store.GetExecutionFills(c.Request.Context(), poolID, window, limit, cursor)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	nextCursor := ""
	if len(fills) == limit && len(fills) > 0 {
		last := fills[len(fills)-1]
		nextCursor = fmt.Sprintf("%d|%d|%d", last.TsMs, last.Checkpoint, last.EventSeq)
	}

	c.JSON(http.StatusOK, gin.H{
		"pool_id":     poolID,
		"window":      window,
		"count":       len(fills),
		"next_cursor": nextCursor,
		"fills":       fills,
	})
}

func parseLifecycleCursor(raw string) (*store.OrderLifecycleCursor, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return nil, nil
	}

	parts := strings.Split(raw, "|")
	if len(parts) != 3 {
		return nil, fmt.Errorf("invalid cursor parts")
	}

	tsMs, err := strconv.ParseInt(parts[0], 10, 64)
	if err != nil {
		return nil, err
	}
	checkpoint, err := strconv.ParseInt(parts[1], 10, 64)
	if err != nil {
		return nil, err
	}
	eventSeq, err := strconv.ParseInt(parts[2], 10, 32)
	if err != nil {
		return nil, err
	}

	return &store.OrderLifecycleCursor{TsMs: tsMs, Checkpoint: checkpoint, EventSeq: int32(eventSeq)}, nil
}

func (h *Handler) GetBMVolume(c *gin.Context) {
	bmID := c.Param("bm_id")
	window := c.DefaultQuery("window", "24h")

	poolParam := c.Query("pool")
	var poolFilter []string
	if poolParam != "" {
		poolFilter = strings.Split(poolParam, ",")
	}

	vol, err := h.store.GetBMVolume(c.Request.Context(), bmID, window, poolFilter)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, vol)
}

func (h *Handler) TradesWS(c *gin.Context) {
	poolParam := c.Query("pool")
	var poolFilter []string
	if poolParam != "" {
		poolFilter = strings.Split(poolParam, ",")
	}

	conn, err := upgrader.Upgrade(c.Writer, c.Request, nil)
	if err != nil {
		log.Println("ws upgrade error:", err)
		return
	}
	defer conn.Close()

	out := make(chan *store.TradeEvent, 10)
	done := make(chan struct{})

	ctx, cancel := context.WithCancel(c.Request.Context())
	defer cancel()

	go func() {
		defer close(done)
		if err := h.store.StreamTrades(ctx, poolFilter, out); err != nil {
			log.Println("stream trades error:", err)
		}
		close(out)
	}()

	ticker := time.NewTicker(h.wsPingInterval)
	defer ticker.Stop()

	for {
		select {
		case ev, ok := <-out:
			if !ok {
				return
			}
			if err := conn.WriteJSON(ev); err != nil {
				return
			}
		case <-ticker.C:
			ping := map[string]interface{}{"type": "ping", "ts_ms": time.Now().UnixMilli()}
			if err := conn.WriteJSON(ping); err != nil {
				return
			}
		case <-done:
			return
		}
	}
}

func (h *Handler) Health(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "ok"})
}
