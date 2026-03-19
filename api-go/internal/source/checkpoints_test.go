package source

import (
	"context"
	"testing"
)

func TestRemoteStoreURLFromEnv(t *testing.T) {
	if got := remoteStoreURL("mainnet"); got != "https://checkpoints.mainnet.sui.io" {
		t.Fatalf("expected mainnet remote store URL, got %q", got)
	}
	if got := remoteStoreURL("testnet"); got != "https://checkpoints.testnet.sui.io" {
		t.Fatalf("expected testnet remote store URL, got %q", got)
	}
	if got := remoteStoreURL("unknown"); got != "https://checkpoints.testnet.sui.io" {
		t.Fatalf("expected fallback testnet URL, got %q", got)
	}
}

func TestResolveLatestCheckpointFindsHighestExistingSequence(t *testing.T) {
	existing := make(map[int64]bool)
	for i := int64(0); i <= 10; i++ {
		existing[i] = true
	}

	got, err := resolveLatestCheckpoint(context.Background(), func(_ context.Context, seq int64) (bool, error) {
		return existing[seq], nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got != 10 {
		t.Fatalf("expected latest checkpoint 10, got %d", got)
	}
}
