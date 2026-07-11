package launchcode

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"personal-pilot/backend/internal/database"
)

type failingSubscriptionStore struct {
	err error
}

func (s failingSubscriptionStore) Create(context.Context, ProxySubscription, []SubscriptionNode) (ProxySubscription, error) {
	return ProxySubscription{}, s.err
}
func (s failingSubscriptionStore) List(context.Context) ([]ProxySubscription, error) {
	return nil, s.err
}
func (s failingSubscriptionStore) Get(context.Context, string) (ProxySubscription, error) {
	return ProxySubscription{}, s.err
}
func (s failingSubscriptionStore) ReplaceNodes(context.Context, string, []SubscriptionNode) (ProxySubscription, error) {
	return ProxySubscription{}, s.err
}
func (s failingSubscriptionStore) ListNodes(context.Context, string) ([]SubscriptionNode, error) {
	return nil, s.err
}
func (s failingSubscriptionStore) Delete(context.Context, string) (int, error) {
	return 0, s.err
}
func (s failingSubscriptionStore) RecordRefreshError(context.Context, string, string) error {
	return s.err
}

type fakeSubscriptionFetcher struct {
	mu        sync.Mutex
	responses map[string][]SubscriptionNode
	errors    map[string]error
	calls     []string
}

func (f *fakeSubscriptionFetcher) Fetch(_ context.Context, sourceURL string) ([]SubscriptionNode, error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.calls = append(f.calls, sourceURL)
	if err := f.errors[sourceURL]; err != nil {
		return nil, err
	}
	nodes := f.responses[sourceURL]
	return append([]SubscriptionNode(nil), nodes...), nil
}

func (f *fakeSubscriptionFetcher) setResponse(sourceURL string, nodes []SubscriptionNode) {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.responses[sourceURL] = append([]SubscriptionNode(nil), nodes...)
	delete(f.errors, sourceURL)
}

func (f *fakeSubscriptionFetcher) setError(sourceURL string, err error) {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.errors[sourceURL] = err
}

func newSubscriptionAPIServer(t *testing.T) (*LaunchServer, *fakeSubscriptionFetcher, *database.DB) {
	t.Helper()
	db, err := database.NewDB(filepath.Join(t.TempDir(), "subscription-api.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	if err := db.Migrate(); err != nil {
		t.Fatal(err)
	}
	fetcher := &fakeSubscriptionFetcher{
		responses: make(map[string][]SubscriptionNode),
		errors:    make(map[string]error),
	}
	srv := newTestServer(t)
	srv.SetProxySubscriptionStore(NewSQLiteProxySubscriptionStore(db.GetConn()))
	srv.SetSubscriptionFetcher(fetcher)
	return srv, fetcher, db
}

func TestSubscriptionAPICreateListValidateRefreshNodesAndDelete(t *testing.T) {
	srv, fetcher, db := newSubscriptionAPIServer(t)
	sourceURL := "https://example.com/subscription?token=top-secret"
	fetcher.setResponse(sourceURL, []SubscriptionNode{
		{Name: "node-a", ProxyConfig: "http://user:secret@proxy.example:8080"},
		{Name: "node-b", ProxyConfig: "vless://00000000-0000-0000-0000-000000000001@vless.example:443"},
	})

	create := serve(srv, http.MethodPost, "/api/proxy/subscribe", map[string]interface{}{
		"name":             "primary",
		"url":              sourceURL,
		"groupName":        "subscription-group",
		"autoRefresh":      true,
		"refreshIntervalM": 5,
	})
	if create.Code != http.StatusCreated {
		t.Fatalf("create status=%d body=%s", create.Code, create.Body.String())
	}
	var created struct {
		OK   bool `json:"ok"`
		Item struct {
			SubscriptionID string `json:"subscriptionId"`
			SourceURL      string `json:"sourceUrl"`
			NodeCount      int    `json:"nodeCount"`
		} `json:"item"`
	}
	decodeJSON(t, create, &created)
	if !created.OK || created.Item.SubscriptionID == "" || created.Item.NodeCount != 2 {
		t.Fatalf("created=%+v", created)
	}
	if strings.Contains(created.Item.SourceURL, "top-secret") || strings.Contains(create.Body.String(), "top-secret") {
		t.Fatalf("create response leaked subscription token: %s", create.Body.String())
	}
	subscriptionID := created.Item.SubscriptionID

	list := serve(srv, http.MethodGet, "/api/proxy/subscribe/list", nil)
	if list.Code != http.StatusOK || !strings.Contains(list.Body.String(), subscriptionID) {
		t.Fatalf("list status=%d body=%s", list.Code, list.Body.String())
	}

	nodes := serve(srv, http.MethodGet, "/api/proxy/subscribe/"+subscriptionID+"/nodes", nil)
	if nodes.Code != http.StatusOK {
		t.Fatalf("nodes status=%d body=%s", nodes.Code, nodes.Body.String())
	}
	if strings.Contains(nodes.Body.String(), "secret") || strings.Contains(nodes.Body.String(), "00000000-0000-0000-0000-000000000001") {
		t.Fatalf("nodes response leaked proxy credentials: %s", nodes.Body.String())
	}
	var storedSourceURL string
	if err := db.GetConn().QueryRow(`SELECT source_url FROM browser_proxies WHERE source_id = ? LIMIT 1`, subscriptionID).Scan(&storedSourceURL); err != nil {
		t.Fatal(err)
	}
	if strings.Contains(storedSourceURL, "top-secret") {
		t.Fatalf("browser proxy source URL leaked subscription token: %q", storedSourceURL)
	}

	fetcher.setResponse(sourceURL, []SubscriptionNode{
		{Name: "node-a", ProxyConfig: "http://user:secret@proxy.example:8080"},
		{Name: "node-b", ProxyConfig: "vless://00000000-0000-0000-0000-000000000001@vless.example:443"},
		{Name: "node-c", ProxyConfig: "https://proxy.example:8443"},
	})
	validate := serve(srv, http.MethodPost, "/api/proxy/subscribe/"+subscriptionID+"/validate", nil)
	if validate.Code != http.StatusOK || !strings.Contains(validate.Body.String(), `"nodeCount":3`) {
		t.Fatalf("validate status=%d body=%s", validate.Code, validate.Body.String())
	}
	beforeRefresh := serve(srv, http.MethodGet, "/api/proxy/subscribe/"+subscriptionID+"/nodes", nil)
	if !strings.Contains(beforeRefresh.Body.String(), `"count":2`) {
		t.Fatalf("validate mutated stored nodes: %s", beforeRefresh.Body.String())
	}

	refresh := serve(srv, http.MethodPost, "/api/proxy/subscribe/"+subscriptionID+"/refresh", nil)
	if refresh.Code != http.StatusOK || !strings.Contains(refresh.Body.String(), `"nodeCount":3`) {
		t.Fatalf("refresh status=%d body=%s", refresh.Code, refresh.Body.String())
	}

	deleted := serve(srv, http.MethodDelete, "/api/proxy/subscribe/"+subscriptionID, nil)
	if deleted.Code != http.StatusOK || !strings.Contains(deleted.Body.String(), `"deletedNodeCount":3`) {
		t.Fatalf("delete status=%d body=%s", deleted.Code, deleted.Body.String())
	}
	listAfterDelete := serve(srv, http.MethodGet, "/api/proxy/subscribe/list", nil)
	if !strings.Contains(listAfterDelete.Body.String(), `"count":0`) {
		t.Fatalf("list after delete: %s", listAfterDelete.Body.String())
	}
}

func TestSubscriptionAPIValidatesRequestsAndRejectsDuplicateSource(t *testing.T) {
	srv, fetcher, _ := newSubscriptionAPIServer(t)
	for _, body := range []map[string]interface{}{
		{"url": "file:///tmp/sub"},
		{"url": "https://user:pass@example.com/sub"},
		{"url": "https://example.com/sub", "autoRefresh": true, "refreshIntervalM": 1},
	} {
		resp := serve(srv, http.MethodPost, "/api/proxy/subscribe", body)
		if resp.Code != http.StatusBadRequest || !strings.Contains(resp.Body.String(), invalidSubscriptionRequestCode) {
			t.Fatalf("invalid request status=%d body=%s", resp.Code, resp.Body.String())
		}
	}

	sourceURL := "https://example.com/duplicate"
	fetcher.setResponse(sourceURL, []SubscriptionNode{{Name: "node", ProxyConfig: "socks5://127.0.0.1:1080"}})
	first := serve(srv, http.MethodPost, "/api/proxy/subscribe", map[string]interface{}{"url": sourceURL})
	if first.Code != http.StatusCreated {
		t.Fatalf("first create status=%d body=%s", first.Code, first.Body.String())
	}
	duplicate := serve(srv, http.MethodPost, "/api/proxy/subscribe", map[string]interface{}{"url": sourceURL})
	if duplicate.Code != http.StatusConflict || !strings.Contains(duplicate.Body.String(), subscriptionExistsCode) {
		t.Fatalf("duplicate status=%d body=%s", duplicate.Code, duplicate.Body.String())
	}

	badClash := serve(srv, http.MethodPost, "/api/proxy/subscribe/import-clash", map[string]interface{}{"raw": "proxies: []"})
	if badClash.Code != http.StatusBadRequest || !strings.Contains(badClash.Body.String(), invalidSubscriptionRequestCode) {
		t.Fatalf("bad Clash status=%d body=%s", badClash.Code, badClash.Body.String())
	}
}

func TestSubscriptionAPIRefreshFailurePreservesOldNodes(t *testing.T) {
	srv, fetcher, _ := newSubscriptionAPIServer(t)
	sourceURL := "https://example.com/failure-safe"
	fetcher.setResponse(sourceURL, []SubscriptionNode{{Name: "old", ProxyConfig: "socks5://127.0.0.1:1080"}})
	create := serve(srv, http.MethodPost, "/api/proxy/subscribe", map[string]interface{}{"url": sourceURL})
	if create.Code != http.StatusCreated {
		t.Fatalf("create status=%d body=%s", create.Code, create.Body.String())
	}
	var payload struct {
		Item struct {
			SubscriptionID string `json:"subscriptionId"`
		} `json:"item"`
	}
	decodeJSON(t, create, &payload)

	fetcher.setError(sourceURL, errors.New("upstream unavailable at https://user:pass@redirect.example/private-token?token=redirect-secret"))
	refresh := serve(srv, http.MethodPost, "/api/proxy/subscribe/"+payload.Item.SubscriptionID+"/refresh", nil)
	if refresh.Code != http.StatusBadGateway || !strings.Contains(refresh.Body.String(), "subscription_fetch_failed") {
		t.Fatalf("refresh status=%d body=%s", refresh.Code, refresh.Body.String())
	}
	nodes := serve(srv, http.MethodGet, "/api/proxy/subscribe/"+payload.Item.SubscriptionID+"/nodes", nil)
	if nodes.Code != http.StatusOK || !strings.Contains(nodes.Body.String(), `"count":1`) || !strings.Contains(nodes.Body.String(), "old") {
		t.Fatalf("nodes after failed refresh: status=%d body=%s", nodes.Code, nodes.Body.String())
	}
	list := serve(srv, http.MethodGet, "/api/proxy/subscribe/list", nil)
	if !strings.Contains(list.Body.String(), "upstream unavailable") {
		t.Fatalf("refresh error not recorded: %s", list.Body.String())
	}
	if strings.Contains(list.Body.String(), "redirect-secret") || strings.Contains(list.Body.String(), "private-token") || strings.Contains(list.Body.String(), "user:pass") {
		t.Fatalf("redirect error URL leaked credentials: %s", list.Body.String())
	}
}

func TestSubscriptionAPIImportsStaticClashWithoutCredentialLeak(t *testing.T) {
	srv, _, _ := newSubscriptionAPIServer(t)
	raw := `proxies:
  - name: clash-http
    type: http
    server: proxy.example
    port: 8080
    username: user
    password: super-secret
  - name: clash-socks
    type: socks5
    server: socks.example
    port: 1080
`
	resp := serve(srv, http.MethodPost, "/api/proxy/subscribe/import-clash", map[string]interface{}{
		"name":      "static-clash",
		"groupName": "clash-group",
		"raw":       raw,
	})
	if resp.Code != http.StatusCreated || !strings.Contains(resp.Body.String(), `"nodeCount":2`) {
		t.Fatalf("import status=%d body=%s", resp.Code, resp.Body.String())
	}
	if strings.Contains(resp.Body.String(), "super-secret") {
		t.Fatalf("import response leaked credentials: %s", resp.Body.String())
	}
	var created struct {
		Item struct {
			SubscriptionID string `json:"subscriptionId"`
		} `json:"item"`
	}
	decodeJSON(t, resp, &created)
	nodes := serve(srv, http.MethodGet, "/api/proxy/subscribe/"+created.Item.SubscriptionID+"/nodes", nil)
	if strings.Contains(nodes.Body.String(), "super-secret") {
		t.Fatalf("nodes leaked clash credentials: %s", nodes.Body.String())
	}
	refresh := serve(srv, http.MethodPost, "/api/proxy/subscribe/"+created.Item.SubscriptionID+"/refresh", nil)
	if refresh.Code != http.StatusConflict || !strings.Contains(refresh.Body.String(), "subscription_not_refreshable") {
		t.Fatalf("static refresh status=%d body=%s", refresh.Code, refresh.Body.String())
	}
}

func TestSubscriptionAPIAutoRefreshesDueURLSources(t *testing.T) {
	srv, fetcher, db := newSubscriptionAPIServer(t)
	sourceURL := "https://example.com/auto"
	fetcher.setResponse(sourceURL, []SubscriptionNode{{Name: "old", ProxyConfig: "socks5://127.0.0.1:1080"}})
	create := serve(srv, http.MethodPost, "/api/proxy/subscribe", map[string]interface{}{
		"url": sourceURL, "autoRefresh": true, "refreshIntervalM": 5,
	})
	if create.Code != http.StatusCreated {
		t.Fatalf("create status=%d body=%s", create.Code, create.Body.String())
	}
	var created struct {
		Item struct {
			SubscriptionID string `json:"subscriptionId"`
		} `json:"item"`
	}
	decodeJSON(t, create, &created)
	if _, err := db.GetConn().Exec(`UPDATE proxy_subscriptions SET last_refresh_at = ? WHERE subscription_id = ?`, time.Now().Add(-10*time.Minute).UTC().Format(time.RFC3339), created.Item.SubscriptionID); err != nil {
		t.Fatal(err)
	}
	fetcher.setResponse(sourceURL, []SubscriptionNode{{Name: "new", ProxyConfig: "socks5://127.0.0.1:2080"}})
	if err := srv.refreshDueSubscriptions(context.Background(), time.Now().UTC()); err != nil {
		t.Fatalf("refreshDueSubscriptions: %v", err)
	}
	nodes := serve(srv, http.MethodGet, "/api/proxy/subscribe/"+created.Item.SubscriptionID+"/nodes", nil)
	if !strings.Contains(nodes.Body.String(), "new") || strings.Contains(nodes.Body.String(), "old") {
		t.Fatalf("auto-refreshed nodes: %s", nodes.Body.String())
	}
}

func TestSubscriptionAPIDoesNotExposeInternalStoreErrors(t *testing.T) {
	srv := newTestServer(t)
	srv.SetProxySubscriptionStore(failingSubscriptionStore{err: errors.New(`sqlite failure at D:\private\pilot.db: SELECT secret`)})
	resp := serve(srv, http.MethodGet, "/api/proxy/subscribe/list", nil)
	if resp.Code != http.StatusInternalServerError || !strings.Contains(resp.Body.String(), "subscription_store_error") {
		t.Fatalf("status=%d body=%s", resp.Code, resp.Body.String())
	}
	if strings.Contains(resp.Body.String(), "private") || strings.Contains(resp.Body.String(), "SELECT secret") {
		t.Fatalf("internal store error leaked: %s", resp.Body.String())
	}
}

func TestSubscriptionAPIReportsUnavailableStoreWithoutFakeSuccess(t *testing.T) {
	srv := newTestServer(t)
	recorder := serve(srv, http.MethodGet, "/api/proxy/subscribe/list", nil)
	if recorder.Code != http.StatusServiceUnavailable {
		t.Fatalf("status=%d body=%s", recorder.Code, recorder.Body.String())
	}
	if !strings.Contains(recorder.Body.String(), "subscription_store_unavailable") {
		t.Fatalf("body=%s", recorder.Body.String())
	}
}

func TestRecordingStopPlayRouteIsStableWhenRecorderUnavailable(t *testing.T) {
	srv := newTestServer(t)
	srv.recording = nil
	recorder := httptest.NewRecorder()
	NewTestHandler(srv).ServeHTTP(recorder, httptest.NewRequest(http.MethodPost, "/api/recording/play/stop", bytes.NewBufferString(`{"profileId":"p1"}`)))
	if recorder.Code != http.StatusServiceUnavailable {
		t.Fatalf("status=%d body=%s", recorder.Code, recorder.Body.String())
	}
}

func decodeSubscriptionResponse(t *testing.T, recorder *httptest.ResponseRecorder) map[string]interface{} {
	t.Helper()
	var payload map[string]interface{}
	if err := json.NewDecoder(recorder.Body).Decode(&payload); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	return payload
}
