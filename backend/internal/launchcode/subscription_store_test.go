package launchcode

import (
	"context"
	"errors"
	"path/filepath"
	"testing"

	"personal-pilot/backend/internal/database"
)

func newSQLiteSubscriptionStoreForTest(t *testing.T) (*SQLiteProxySubscriptionStore, *database.DB) {
	t.Helper()
	db, err := database.NewDB(filepath.Join(t.TempDir(), "subscriptions.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	if err := db.Migrate(); err != nil {
		t.Fatal(err)
	}
	return NewSQLiteProxySubscriptionStore(db.GetConn()), db
}

func TestSQLiteProxySubscriptionStoreCRUDAndStableNodeIDs(t *testing.T) {
	store, db := newSQLiteSubscriptionStoreForTest(t)
	ctx := context.Background()
	subscription := ProxySubscription{
		SubscriptionID:   "sub-stable",
		Name:             "主订阅",
		SourceType:       SubscriptionSourceURL,
		SourceURL:        "https://example.com/sub/token",
		GroupName:        "订阅组",
		AutoRefresh:      true,
		RefreshIntervalM: 30,
	}
	created, err := store.Create(ctx, subscription, []SubscriptionNode{
		{Name: "node-a", ProxyConfig: "socks5://127.0.0.1:1080"},
		{Name: "node-b", ProxyConfig: "http://user:secret@proxy.example:8080"},
	})
	if err != nil {
		t.Fatalf("Create: %v", err)
	}
	if created.NodeCount != 2 || created.CreatedAt == "" || created.UpdatedAt == "" || created.LastRefreshAt == "" {
		t.Fatalf("created = %+v", created)
	}

	initialNodes, err := store.ListNodes(ctx, subscription.SubscriptionID)
	if err != nil {
		t.Fatalf("ListNodes: %v", err)
	}
	if len(initialNodes) != 2 {
		t.Fatalf("initial nodes = %d, want 2", len(initialNodes))
	}
	stableID := initialNodes[0].ProxyID

	updated, err := store.ReplaceNodes(ctx, subscription.SubscriptionID, []SubscriptionNode{
		{Name: "node-a-renamed", ProxyConfig: "socks5://127.0.0.1:1080"},
		{Name: "node-c", ProxyConfig: "https://proxy.example:8443"},
	})
	if err != nil {
		t.Fatalf("ReplaceNodes: %v", err)
	}
	if updated.NodeCount != 2 || updated.LastError != "" {
		t.Fatalf("updated = %+v", updated)
	}
	refreshedNodes, err := store.ListNodes(ctx, subscription.SubscriptionID)
	if err != nil {
		t.Fatalf("ListNodes after refresh: %v", err)
	}
	if len(refreshedNodes) != 2 || refreshedNodes[0].ProxyID != stableID || refreshedNodes[0].Name != "node-a-renamed" {
		t.Fatalf("refreshed nodes = %+v", refreshedNodes)
	}

	if _, err := store.ReplaceNodes(ctx, subscription.SubscriptionID, []SubscriptionNode{{Name: "invalid", ProxyConfig: ""}}); err == nil {
		t.Fatal("expected invalid replacement to fail")
	}
	preservedNodes, err := store.ListNodes(ctx, subscription.SubscriptionID)
	if err != nil {
		t.Fatalf("ListNodes after failed refresh: %v", err)
	}
	if len(preservedNodes) != 2 || preservedNodes[0].ProxyID != stableID {
		t.Fatalf("failed replacement changed nodes: %+v", preservedNodes)
	}

	if _, err := db.GetConn().Exec(`
		CREATE TRIGGER fail_subscription_node_insert
		BEFORE INSERT ON browser_proxies
		WHEN NEW.proxy_name = 'force-sql-failure'
		BEGIN
			SELECT RAISE(ABORT, 'forced subscription insert failure');
		END`); err != nil {
		t.Fatal(err)
	}
	if _, err := store.ReplaceNodes(ctx, subscription.SubscriptionID, []SubscriptionNode{{Name: "force-sql-failure", ProxyConfig: "socks5://127.0.0.1:3080"}}); err == nil {
		t.Fatal("expected SQL replacement failure")
	}
	afterSQLFailure, err := store.ListNodes(ctx, subscription.SubscriptionID)
	if err != nil {
		t.Fatal(err)
	}
	if len(afterSQLFailure) != 2 || afterSQLFailure[0].ProxyID != stableID {
		t.Fatalf("SQL failure did not roll back old nodes: %+v", afterSQLFailure)
	}

	deleted, err := store.Delete(ctx, subscription.SubscriptionID)
	if err != nil {
		t.Fatalf("Delete: %v", err)
	}
	if deleted != 2 {
		t.Fatalf("deleted nodes = %d, want 2", deleted)
	}
	if _, err := store.Get(ctx, subscription.SubscriptionID); !errors.Is(err, ErrSubscriptionNotFound) {
		t.Fatalf("Get after delete error = %v", err)
	}
}

func TestSQLiteProxySubscriptionStoreKeepsCaseSensitiveConfigsDistinct(t *testing.T) {
	store, _ := newSQLiteSubscriptionStoreForTest(t)
	created, err := store.Create(context.Background(), ProxySubscription{
		SubscriptionID: "sub-case-sensitive",
		Name:           "case-sensitive",
		SourceType:     SubscriptionSourceURL,
		SourceURL:      "https://example.com/case-sensitive",
		GroupName:      "group",
	}, []SubscriptionNode{
		{Name: "upper", ProxyConfig: "vless://ABCDEF@example.com:443"},
		{Name: "lower", ProxyConfig: "vless://abcdef@example.com:443"},
	})
	if err != nil {
		t.Fatalf("Create: %v", err)
	}
	if created.NodeCount != 2 {
		t.Fatalf("NodeCount = %d, want 2", created.NodeCount)
	}
	nodes, err := store.ListNodes(context.Background(), created.SubscriptionID)
	if err != nil {
		t.Fatal(err)
	}
	if len(nodes) != 2 || nodes[0].ProxyID == nodes[1].ProxyID {
		t.Fatalf("case-sensitive nodes = %+v", nodes)
	}
}

func TestSQLiteProxySubscriptionStoreRejectsDuplicateURLAndRecordsRefreshError(t *testing.T) {
	store, _ := newSQLiteSubscriptionStoreForTest(t)
	ctx := context.Background()
	first := ProxySubscription{
		SubscriptionID: "sub-first",
		Name:           "first",
		SourceType:     SubscriptionSourceURL,
		SourceURL:      "https://example.com/sub",
		GroupName:      "group",
	}
	if _, err := store.Create(ctx, first, []SubscriptionNode{{Name: "node", ProxyConfig: "socks5://127.0.0.1:1080"}}); err != nil {
		t.Fatal(err)
	}
	duplicate := first
	duplicate.SubscriptionID = "sub-duplicate"
	if _, err := store.Create(ctx, duplicate, []SubscriptionNode{{Name: "node", ProxyConfig: "socks5://127.0.0.1:2080"}}); !errors.Is(err, ErrSubscriptionExists) {
		t.Fatalf("duplicate create error = %v", err)
	}

	if err := store.RecordRefreshError(ctx, first.SubscriptionID, "upstream timeout"); err != nil {
		t.Fatal(err)
	}
	got, err := store.Get(ctx, first.SubscriptionID)
	if err != nil {
		t.Fatal(err)
	}
	if got.LastError != "upstream timeout" || got.NodeCount != 1 {
		t.Fatalf("subscription after refresh error = %+v", got)
	}
}
