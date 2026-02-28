package handlers

import (
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
