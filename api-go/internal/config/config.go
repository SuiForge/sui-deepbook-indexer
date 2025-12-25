package config

import (
	"log"
	"os"
	"strconv"
	"time"

	"github.com/joho/godotenv"
)

type Config struct {
	DatabaseURL    string
	ListenAddr     string
	WSPingInterval time.Duration
	APISingleKey   string // optional single-key auth
	LogLevel       string
}

func Load() *Config {
	loadEnvFiles()
	return &Config{
		DatabaseURL:    getEnv("DATABASE_URL", ""),
		ListenAddr:     getEnv("API_LISTEN_ADDR", "0.0.0.0:8080"),
		WSPingInterval: getDuration("WS_PING_INTERVAL_SEC", 15) * time.Second,
		APISingleKey:   getEnv("API_SINGLE_KEY", ""),
		LogLevel:       getEnv("LOG_LEVEL", "info"),
	}
}

// loadEnvFiles loads local .env files if present; harmless if missing.
func loadEnvFiles() {
	files := []string{".env.local", ".env"}
	for _, f := range files {
		if err := godotenv.Load(f); err != nil {
			if !os.IsNotExist(err) {
				log.Printf("warning: failed to load %s: %v", f, err)
			}
		}
	}
}

func getEnv(key, defaultVal string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return defaultVal
}

func getDuration(key string, defaultVal int) time.Duration {
	if v := os.Getenv(key); v != "" {
		if i, err := strconv.Atoi(v); err == nil {
			return time.Duration(i)
		}
	}
	return time.Duration(defaultVal)
}
