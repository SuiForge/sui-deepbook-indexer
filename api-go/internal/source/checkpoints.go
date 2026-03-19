// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

package source

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"strings"
	"sync"
	"time"
)

type CheckpointStatus struct {
	LatestCheckpoint int64
	SourceURL        string
}

type Probe interface {
	LatestCheckpoint(ctx context.Context) (*CheckpointStatus, error)
}

type RemoteStoreProbe struct {
	sourceURL string
	client    *http.Client
	cacheTTL  time.Duration

	mu        sync.Mutex
	cachedAt  time.Time
	cachedVal *CheckpointStatus
}

func NewRemoteStoreProbe(env string, timeout time.Duration, cacheTTL time.Duration) *RemoteStoreProbe {
	if timeout <= 0 {
		timeout = 5 * time.Second
	}
	if cacheTTL <= 0 {
		cacheTTL = 15 * time.Second
	}
	return &RemoteStoreProbe{
		sourceURL: remoteStoreURL(env),
		client: &http.Client{
			Timeout: timeout,
		},
		cacheTTL: cacheTTL,
	}
}

func (p *RemoteStoreProbe) LatestCheckpoint(ctx context.Context) (*CheckpointStatus, error) {
	p.mu.Lock()
	if p.cachedVal != nil && time.Since(p.cachedAt) < p.cacheTTL {
		cached := *p.cachedVal
		p.mu.Unlock()
		return &cached, nil
	}
	p.mu.Unlock()

	latest, err := resolveLatestCheckpoint(ctx, p.checkpointExists)
	if err != nil {
		return nil, err
	}

	status := &CheckpointStatus{
		LatestCheckpoint: latest,
		SourceURL:        p.sourceURL,
	}

	p.mu.Lock()
	p.cachedVal = status
	p.cachedAt = time.Now()
	p.mu.Unlock()

	return status, nil
}

func (p *RemoteStoreProbe) checkpointExists(ctx context.Context, seq int64) (bool, error) {
	url := fmt.Sprintf("%s/%d.chk", strings.TrimRight(p.sourceURL, "/"), seq)
	req, err := http.NewRequestWithContext(ctx, http.MethodHead, url, nil)
	if err != nil {
		return false, err
	}

	resp, err := p.client.Do(req)
	if err != nil {
		return false, err
	}
	defer resp.Body.Close()

	switch resp.StatusCode {
	case http.StatusOK:
		io.Copy(io.Discard, resp.Body)
		return true, nil
	case http.StatusNotFound:
		return false, nil
	case http.StatusMethodNotAllowed:
		return p.checkpointExistsWithGet(ctx, url)
	default:
		return false, fmt.Errorf("unexpected source status code %d", resp.StatusCode)
	}
}

func (p *RemoteStoreProbe) checkpointExistsWithGet(ctx context.Context, url string) (bool, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return false, err
	}

	resp, err := p.client.Do(req)
	if err != nil {
		return false, err
	}
	defer resp.Body.Close()
	io.Copy(io.Discard, resp.Body)

	switch resp.StatusCode {
	case http.StatusOK:
		return true, nil
	case http.StatusNotFound:
		return false, nil
	default:
		return false, fmt.Errorf("unexpected source status code %d", resp.StatusCode)
	}
}

func remoteStoreURL(env string) string {
	switch strings.ToLower(strings.TrimSpace(env)) {
	case "mainnet", "main":
		return "https://checkpoints.mainnet.sui.io"
	case "testnet", "test", "":
		return "https://checkpoints.testnet.sui.io"
	default:
		return "https://checkpoints.testnet.sui.io"
	}
}

func resolveLatestCheckpoint(ctx context.Context, exists func(context.Context, int64) (bool, error)) (int64, error) {
	ok, err := exists(ctx, 0)
	if err != nil {
		return 0, err
	}
	if !ok {
		return 0, fmt.Errorf("checkpoint 0 not available")
	}

	low := int64(0)
	high := int64(1)
	for {
		ok, err := exists(ctx, high)
		if err != nil {
			return 0, err
		}
		if !ok {
			break
		}
		low = high
		if high > (1 << 62) {
			return low, nil
		}
		high *= 2
	}

	for low+1 < high {
		mid := low + (high-low)/2
		ok, err := exists(ctx, mid)
		if err != nil {
			return 0, err
		}
		if ok {
			low = mid
		} else {
			high = mid
		}
	}

	return low, nil
}
