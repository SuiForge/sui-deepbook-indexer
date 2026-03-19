package handlers

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
)

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
