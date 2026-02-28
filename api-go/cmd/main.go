package main

import (
	"log"

	"github.com/Lab-JY/deepbook-indexer/api-go/internal/config"
	"github.com/Lab-JY/deepbook-indexer/api-go/internal/handlers"
	"github.com/Lab-JY/deepbook-indexer/api-go/internal/store"
	"github.com/gin-gonic/gin"
)

func main() {
	cfg := config.Load()

	if cfg.DatabaseURL == "" {
		log.Fatal("DATABASE_URL is required")
	}

	st, err := store.New(cfg.DatabaseURL)
	if err != nil {
		log.Fatalf("failed to connect to database: %v", err)
	}
	defer st.Close()

	if cfg.LogLevel == "debug" {
		gin.SetMode(gin.DebugMode)
	} else {
		gin.SetMode(gin.ReleaseMode)
	}

	r := gin.Default()
	h := handlers.New(st, cfg.APISingleKey, cfg.WSPingInterval)

	// Auth middleware
	authMiddleware := h.AuthMiddleware()

	r.GET("/health", h.Health)

	api := r.Group("/v1/deepbook")
	api.Use(authMiddleware)
	{
		api.GET("/pools/:pool_id/metrics", h.GetPoolMetrics)
		api.GET("/pools/:pool_id/candles", h.GetPoolCandles)
		api.GET("/pools/:pool_id/execution/summary", h.GetExecutionSummary)
		api.GET("/pools/:pool_id/execution/lifecycle", h.GetOrderLifecycle)
		api.GET("/pools/:pool_id/execution/fills", h.GetExecutionFills)
		api.GET("/bm/:bm_id/volume", h.GetBMVolume)
		api.GET("/trades", h.TradesWS)
	}

	log.Printf("API server listening on %s", cfg.ListenAddr)
	if err := r.Run(cfg.ListenAddr); err != nil {
		log.Fatalf("server error: %v", err)
	}
}
