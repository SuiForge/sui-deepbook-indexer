package handlers

import (
	"context"
	"log"
	"net/http"
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
	store          *store.Store
	singleKey      string
	wsPingInterval time.Duration
}

func New(store *store.Store, singleKey string, wsPingInterval time.Duration) *Handler {
	if wsPingInterval <= 0 {
		wsPingInterval = 15 * time.Second
	}
	return &Handler{store: store, singleKey: singleKey, wsPingInterval: wsPingInterval}
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
