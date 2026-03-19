package handlers

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/gin-gonic/gin"

	"github.com/Lab-JY/deepbook-indexer/api-go/internal/store"
)

type fakeStore struct{}

func (f *fakeStore) GetPoolMetrics(context.Context, string, string) (*store.PoolMetrics, error) {
	return nil, nil
}

func (f *fakeStore) GetPoolCandles(context.Context, string, string, string) (*store.CandleSeries, error) {
	return nil, nil
}

func (f *fakeStore) GetExecutionSummary(context.Context, string, string) (*store.ExecutionSummary, error) {
	return nil, nil
}

func (f *fakeStore) GetOrderLifecycleEvents(context.Context, string, string, string, int, *store.OrderLifecycleCursor) ([]store.OrderLifecycleEvent, error) {
	return nil, nil
}

func (f *fakeStore) GetExecutionFills(context.Context, string, string, int, *store.OrderLifecycleCursor) ([]store.ExecutionFill, error) {
	return nil, nil
}

func (f *fakeStore) GetBMVolume(context.Context, string, string, []string) (*store.BMVolume, error) {
	return nil, nil
}

func (f *fakeStore) StreamTrades(context.Context, []string, chan<- *store.TradeEvent) error {
	return nil
}

var _ storeBackend = (*fakeStore)(nil)

func TestParseLifecycleCursorValid(t *testing.T) {
	c, err := parseLifecycleCursor("1700000000000|12345|7")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if c == nil || c.TsMs != 1700000000000 || c.Checkpoint != 12345 || c.EventSeq != 7 {
		t.Fatalf("unexpected cursor: %#v", c)
	}
}

func TestParseLifecycleCursorRoundTripShape(t *testing.T) {
	raw := "1700000000123|42|9"

	c, err := parseLifecycleCursor(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if got := fmt.Sprintf("%d|%d|%d", c.TsMs, c.Checkpoint, c.EventSeq); got != raw {
		t.Fatalf("expected round-trip cursor %q, got %q", raw, got)
	}
}

func TestNewDefaultsWSPingInterval(t *testing.T) {
	h := New(&fakeStore{}, "", 0)
	if h.wsPingInterval != 15*time.Second {
		t.Fatalf("expected default ws ping interval, got %s", h.wsPingInterval)
	}
}

func TestParseLifecycleCursorInvalid(t *testing.T) {
	_, err := parseLifecycleCursor("bad-cursor")
	if err == nil {
		t.Fatal("expected error for invalid cursor")
	}
}

func TestGetOrderLifecycleMissingPoolID(t *testing.T) {
	gin.SetMode(gin.TestMode)
	h := &Handler{}

	w := httptest.NewRecorder()
	c, _ := gin.CreateTestContext(w)
	c.Request = httptest.NewRequest(http.MethodGet, "/v1/deepbook/pools//execution/lifecycle", nil)

	h.GetOrderLifecycle(c)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestGetExecutionFillsMissingPoolID(t *testing.T) {
	gin.SetMode(gin.TestMode)
	h := &Handler{}

	w := httptest.NewRecorder()
	c, _ := gin.CreateTestContext(w)
	c.Request = httptest.NewRequest(http.MethodGet, "/v1/deepbook/pools//execution/fills", nil)

	h.GetExecutionFills(c)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}
