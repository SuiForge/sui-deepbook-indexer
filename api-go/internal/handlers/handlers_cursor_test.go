package handlers

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/gin-gonic/gin"

	"github.com/Lab-JY/deepbook-indexer/api-go/internal/source"
	"github.com/Lab-JY/deepbook-indexer/api-go/internal/store"
)

type fakeStore struct {
	assets               []store.AssetMetadata
	pools                []store.PoolMetadata
	poolByID             map[string]*store.PoolMetadata
	topMarkets           []store.TopMarket
	serviceStatus        *store.ServiceStatus
	lastTopMarketsWindow string
	lastTopMarketsSort   string
	lastTopMarketsLimit  int
}

type fakeSourceProbe struct {
	status *source.CheckpointStatus
	err    error
}

type assetsResponse struct {
	Count  int                   `json:"count"`
	Assets []store.AssetMetadata `json:"assets"`
}

type poolsResponse struct {
	Count int                  `json:"count"`
	Pools []store.PoolMetadata `json:"pools"`
}

type topMarketsResponse struct {
	Window  string            `json:"window"`
	Sort    string            `json:"sort"`
	Limit   int               `json:"limit"`
	Count   int               `json:"count"`
	Markets []store.TopMarket `json:"markets"`
}

func decodeJSON[T any](t *testing.T, body *httptest.ResponseRecorder) T {
	t.Helper()

	var out T
	if err := json.NewDecoder(body.Body).Decode(&out); err != nil {
		t.Fatalf("decode json: %v", err)
	}
	return out
}

func newTestContext(method string, target string, params gin.Params) (*gin.Context, *httptest.ResponseRecorder) {
	gin.SetMode(gin.TestMode)

	w := httptest.NewRecorder()
	c, _ := gin.CreateTestContext(w)
	c.Request = httptest.NewRequest(method, target, nil)
	c.Params = params

	return c, w
}

func stringPtr(v string) *string {
	return &v
}

func int32Ptr(v int32) *int32 {
	return &v
}

func samplePoolMetadata(updatedAt time.Time) store.PoolMetadata {
	return store.PoolMetadata{
		PoolID:       "0xpool",
		BaseAssetID:  stringPtr("sui"),
		QuoteAssetID: stringPtr("usdc"),
		PackageID:    stringPtr("0xpackage"),
		Status:       stringPtr("active"),
		UpdatedAt:    updatedAt,
		Pair:         stringPtr("SUI/USDC"),
		BaseAsset: &store.AssetMetadata{
			AssetID:   "sui",
			CoinType:  stringPtr("0x2::sui::SUI"),
			Symbol:    stringPtr("SUI"),
			Name:      stringPtr("Sui"),
			Decimals:  int32Ptr(9),
			Status:    stringPtr("active"),
			Source:    stringPtr("seed"),
			UpdatedAt: updatedAt,
		},
		QuoteAsset: &store.AssetMetadata{
			AssetID:   "usdc",
			CoinType:  stringPtr("0xdba34672::usdc::USDC"),
			Symbol:    stringPtr("USDC"),
			Name:      stringPtr("USD Coin"),
			Decimals:  int32Ptr(6),
			Status:    stringPtr("active"),
			Source:    stringPtr("seed"),
			UpdatedAt: updatedAt,
		},
	}
}

func (f *fakeStore) ListAssets(context.Context) ([]store.AssetMetadata, error) {
	return f.assets, nil
}

func (f *fakeStore) ListPools(context.Context) ([]store.PoolMetadata, error) {
	return f.pools, nil
}

func (f *fakeStore) GetPoolMetadata(_ context.Context, poolID string) (*store.PoolMetadata, error) {
	if f.poolByID == nil {
		return nil, nil
	}
	return f.poolByID[poolID], nil
}

func (f *fakeStore) ListTopMarkets(_ context.Context, window string, sort string, limit int) ([]store.TopMarket, error) {
	f.lastTopMarketsWindow = window
	f.lastTopMarketsSort = sort
	f.lastTopMarketsLimit = limit
	return f.topMarkets, nil
}

func (f *fakeStore) GetServiceStatus(context.Context) (*store.ServiceStatus, error) {
	return f.serviceStatus, nil
}

func (f *fakeSourceProbe) LatestCheckpoint(context.Context) (*source.CheckpointStatus, error) {
	return f.status, f.err
}

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
	h := &Handler{}

	w := httptest.NewRecorder()
	c, _ := gin.CreateTestContext(w)
	c.Request = httptest.NewRequest(http.MethodGet, "/v1/deepbook/pools//execution/fills", nil)

	h.GetExecutionFills(c)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestGetAssetsSuccess(t *testing.T) {
	updatedAt := time.Date(2026, time.March, 19, 10, 0, 0, 0, time.UTC)
	h := New(&fakeStore{
		assets: []store.AssetMetadata{
			{
				AssetID:   "sui",
				CoinType:  stringPtr("0x2::sui::SUI"),
				Symbol:    stringPtr("SUI"),
				Name:      stringPtr("Sui"),
				Decimals:  int32Ptr(9),
				Status:    stringPtr("active"),
				Source:    stringPtr("seed"),
				UpdatedAt: updatedAt,
			},
		},
	}, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/assets", nil)
	h.GetAssets(c)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	resp := decodeJSON[assetsResponse](t, w)
	if resp.Count != 1 {
		t.Fatalf("expected count 1, got %d", resp.Count)
	}
	if len(resp.Assets) != 1 {
		t.Fatalf("expected 1 asset, got %d", len(resp.Assets))
	}
	if resp.Assets[0].AssetID != "sui" {
		t.Fatalf("expected asset_id sui, got %q", resp.Assets[0].AssetID)
	}
	if resp.Assets[0].Symbol == nil || *resp.Assets[0].Symbol != "SUI" {
		t.Fatalf("expected symbol SUI, got %#v", resp.Assets[0].Symbol)
	}
	if !resp.Assets[0].UpdatedAt.Equal(updatedAt) {
		t.Fatalf("expected updated_at %s, got %s", updatedAt, resp.Assets[0].UpdatedAt)
	}
}

func TestGetPoolsSuccess(t *testing.T) {
	updatedAt := time.Date(2026, time.March, 19, 10, 30, 0, 0, time.UTC)
	pool := samplePoolMetadata(updatedAt)
	h := New(&fakeStore{
		pools: []store.PoolMetadata{pool},
	}, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/pools", nil)
	h.GetPools(c)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	resp := decodeJSON[poolsResponse](t, w)
	if resp.Count != 1 {
		t.Fatalf("expected count 1, got %d", resp.Count)
	}
	if len(resp.Pools) != 1 {
		t.Fatalf("expected 1 pool, got %d", len(resp.Pools))
	}
	if resp.Pools[0].PoolID != "0xpool" {
		t.Fatalf("expected pool_id 0xpool, got %q", resp.Pools[0].PoolID)
	}
	if resp.Pools[0].Pair == nil || *resp.Pools[0].Pair != "SUI/USDC" {
		t.Fatalf("expected pair SUI/USDC, got %#v", resp.Pools[0].Pair)
	}
	if resp.Pools[0].BaseAsset == nil || resp.Pools[0].BaseAsset.Symbol == nil || *resp.Pools[0].BaseAsset.Symbol != "SUI" {
		t.Fatalf("expected embedded base asset SUI, got %#v", resp.Pools[0].BaseAsset)
	}
	if resp.Pools[0].QuoteAsset == nil || resp.Pools[0].QuoteAsset.Symbol == nil || *resp.Pools[0].QuoteAsset.Symbol != "USDC" {
		t.Fatalf("expected embedded quote asset USDC, got %#v", resp.Pools[0].QuoteAsset)
	}
}

func TestGetPoolMetadataMissingPoolID(t *testing.T) {
	h := New(&fakeStore{}, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/pools//metadata", nil)
	h.GetPoolMetadata(c)

	if w.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", w.Code)
	}
}

func TestGetPoolMetadataSuccess(t *testing.T) {
	updatedAt := time.Date(2026, time.March, 19, 11, 0, 0, 0, time.UTC)
	pool := samplePoolMetadata(updatedAt)
	h := New(&fakeStore{
		poolByID: map[string]*store.PoolMetadata{
			"0xpool": &pool,
		},
	}, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/pools/0xpool/metadata", gin.Params{
		{Key: "pool_id", Value: "0xpool"},
	})
	h.GetPoolMetadata(c)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	resp := decodeJSON[store.PoolMetadata](t, w)
	if resp.PoolID != "0xpool" {
		t.Fatalf("expected pool_id 0xpool, got %q", resp.PoolID)
	}
	if resp.Pair == nil || *resp.Pair != "SUI/USDC" {
		t.Fatalf("expected pair SUI/USDC, got %#v", resp.Pair)
	}
	if resp.BaseAsset == nil || resp.BaseAsset.AssetID != "sui" {
		t.Fatalf("expected base asset sui, got %#v", resp.BaseAsset)
	}
	if resp.QuoteAsset == nil || resp.QuoteAsset.AssetID != "usdc" {
		t.Fatalf("expected quote asset usdc, got %#v", resp.QuoteAsset)
	}
}

func TestGetTopMarketsFallsBackToDefaultLimit(t *testing.T) {
	fake := &fakeStore{
		topMarkets: []store.TopMarket{
			{
				PoolID:       "0xpool",
				BaseAssetID:  stringPtr("sui"),
				QuoteAssetID: stringPtr("usdc"),
				Pair:         stringPtr("SUI/USDC"),
				Trades:       42,
			},
		},
	}
	h := New(fake, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/markets/top?limit=bad", nil)
	h.GetTopMarkets(c)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}
	if fake.lastTopMarketsLimit != 20 {
		t.Fatalf("expected default limit 20, got %d", fake.lastTopMarketsLimit)
	}
	if fake.lastTopMarketsWindow != "24h" {
		t.Fatalf("expected default window 24h, got %q", fake.lastTopMarketsWindow)
	}
	if fake.lastTopMarketsSort != "volume_quote" {
		t.Fatalf("expected default sort volume_quote, got %q", fake.lastTopMarketsSort)
	}

	resp := decodeJSON[topMarketsResponse](t, w)
	if resp.Limit != 20 {
		t.Fatalf("expected response limit 20, got %d", resp.Limit)
	}
	if resp.Count != 1 {
		t.Fatalf("expected count 1, got %d", resp.Count)
	}
}

func TestGetTopMarketsClampsLimit(t *testing.T) {
	fake := &fakeStore{}
	h := New(fake, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/markets/top?window=7d&sort=trades&limit=999", nil)
	h.GetTopMarkets(c)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}
	if fake.lastTopMarketsLimit != 100 {
		t.Fatalf("expected clamped limit 100, got %d", fake.lastTopMarketsLimit)
	}
	if fake.lastTopMarketsWindow != "7d" {
		t.Fatalf("expected window 7d, got %q", fake.lastTopMarketsWindow)
	}
	if fake.lastTopMarketsSort != "trades" {
		t.Fatalf("expected sort trades, got %q", fake.lastTopMarketsSort)
	}
}

func TestGetServiceStatusSuccess(t *testing.T) {
	updatedAt := time.Date(2026, time.March, 19, 12, 0, 0, 0, time.UTC)
	h := New(&fakeStore{
		serviceStatus: &store.ServiceStatus{
			Status:              "ok",
			ProcessedCheckpoint: 12345,
			IndexerUpdatedAt:    updatedAt,
			TradeEvents:         100,
			OrderEvents:         50,
			AssetMetadataCount:  2,
			PoolMetadataCount:   1,
			DistinctPools:       3,
		},
	}, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/status", nil)
	h.GetServiceStatus(c)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	resp := decodeJSON[store.ServiceStatus](t, w)
	if resp.Status != "ok" {
		t.Fatalf("expected status ok, got %q", resp.Status)
	}
	if resp.ProcessedCheckpoint != 12345 {
		t.Fatalf("expected processed checkpoint 12345, got %d", resp.ProcessedCheckpoint)
	}
	if !resp.IndexerUpdatedAt.Equal(updatedAt) {
		t.Fatalf("expected updated_at %s, got %s", updatedAt, resp.IndexerUpdatedAt)
	}
	if resp.SourceStatus != "disabled" {
		t.Fatalf("expected source_status disabled, got %q", resp.SourceStatus)
	}
}

func TestGetServiceStatusEnrichesLagFromSource(t *testing.T) {
	updatedAt := time.Date(2026, time.March, 19, 12, 0, 0, 0, time.UTC)
	h := NewWithSource(&fakeStore{
		serviceStatus: &store.ServiceStatus{
			Status:              "ok",
			ProcessedCheckpoint: 12345,
			IndexerUpdatedAt:    updatedAt,
		},
	}, &fakeSourceProbe{
		status: &source.CheckpointStatus{
			LatestCheckpoint: 12370,
			SourceURL:        "https://checkpoints.testnet.sui.io",
		},
	}, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/status", nil)
	h.GetServiceStatus(c)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	resp := decodeJSON[store.ServiceStatus](t, w)
	if resp.SourceStatus != "ok" {
		t.Fatalf("expected source_status ok, got %q", resp.SourceStatus)
	}
	if resp.LatestCheckpoint == nil || *resp.LatestCheckpoint != 12370 {
		t.Fatalf("expected latest checkpoint 12370, got %#v", resp.LatestCheckpoint)
	}
	if resp.CheckpointLag == nil || *resp.CheckpointLag != 25 {
		t.Fatalf("expected checkpoint lag 25, got %#v", resp.CheckpointLag)
	}
	if resp.SourceURL == nil || *resp.SourceURL != "https://checkpoints.testnet.sui.io" {
		t.Fatalf("expected source url, got %#v", resp.SourceURL)
	}
}

func TestGetServiceStatusMarksSourceErrorAsDegraded(t *testing.T) {
	h := NewWithSource(&fakeStore{
		serviceStatus: &store.ServiceStatus{
			Status:              "ok",
			ProcessedCheckpoint: 12345,
			IndexerUpdatedAt:    time.Date(2026, time.March, 19, 12, 0, 0, 0, time.UTC),
		},
	}, &fakeSourceProbe{
		err: errors.New("remote store unavailable"),
	}, "", 0)

	c, w := newTestContext(http.MethodGet, "/v1/deepbook/status", nil)
	h.GetServiceStatus(c)

	if w.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", w.Code)
	}

	resp := decodeJSON[store.ServiceStatus](t, w)
	if resp.Status != "degraded" {
		t.Fatalf("expected status degraded, got %q", resp.Status)
	}
	if resp.SourceStatus != "error" {
		t.Fatalf("expected source_status error, got %q", resp.SourceStatus)
	}
	if resp.SourceError == nil || *resp.SourceError != "remote store unavailable" {
		t.Fatalf("expected source_error, got %#v", resp.SourceError)
	}
}
