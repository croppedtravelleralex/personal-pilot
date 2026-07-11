package launchcode

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/config"
)

type memoryProxyDAO struct {
	items map[string]browser.Proxy
}

func newMemoryProxyDAO() *memoryProxyDAO {
	return &memoryProxyDAO{items: map[string]browser.Proxy{}}
}

func (d *memoryProxyDAO) List() ([]browser.Proxy, error) {
	out := make([]browser.Proxy, 0, len(d.items))
	for _, item := range d.items {
		out = append(out, item)
	}
	return out, nil
}

func (d *memoryProxyDAO) ListByGroup(groupName string) ([]browser.Proxy, error) {
	out := []browser.Proxy{}
	for _, item := range d.items {
		if item.GroupName == groupName {
			out = append(out, item)
		}
	}
	return out, nil
}

func (d *memoryProxyDAO) ListGroups() ([]string, error) { return nil, nil }

func (d *memoryProxyDAO) Upsert(item browser.Proxy) error {
	d.items[item.ProxyId] = item
	return nil
}

func (d *memoryProxyDAO) Delete(proxyId string) error {
	delete(d.items, proxyId)
	return nil
}

func (d *memoryProxyDAO) DeleteAll() error {
	d.items = map[string]browser.Proxy{}
	return nil
}

func (d *memoryProxyDAO) UpdateSpeedResult(proxyId string, ok bool, latencyMs int64, testedAt string) error {
	item := d.items[proxyId]
	item.LastTestOk = ok
	item.LastLatencyMs = latencyMs
	item.LastTestedAt = testedAt
	d.items[proxyId] = item
	return nil
}

func (d *memoryProxyDAO) UpdateIPHealthResult(proxyId string, healthJSON string) error {
	item := d.items[proxyId]
	item.LastIPHealthJSON = healthJSON
	d.items[proxyId] = item
	return nil
}

func newManualProxyTestServer() (*LaunchServer, *memoryProxyDAO) {
	dao := newMemoryProxyDAO()
	mgr := browser.NewManager(config.DefaultConfig(), "")
	mgr.ProxyDAO = dao
	return NewLaunchServer(NewLaunchCodeService(NewMemoryLaunchCodeDAO()), nil, nil, mgr, 0), dao
}

func decodeManualProxyResponse(t *testing.T, rr *httptest.ResponseRecorder) map[string]interface{} {
	t.Helper()
	var payload map[string]interface{}
	if err := json.Unmarshal(rr.Body.Bytes(), &payload); err != nil {
		t.Fatalf("invalid JSON response: %v", err)
	}
	return payload
}

func TestManualProxyCreateListUpdateDeleteUsesProxyDAO(t *testing.T) {
	srv, dao := newManualProxyTestServer()
	handler := NewTestHandler(srv)

	createBody := `{"name":"Webshare HTTP","protocol":"http","domain":"p.webshare.io","port":80,"username":"user:name","password":"pa:ss","groupName":"manual"}`
	createReq := httptest.NewRequest(http.MethodPost, "/api/proxy/manual", strings.NewReader(createBody))
	createRR := httptest.NewRecorder()
	handler.ServeHTTP(createRR, createReq)
	if createRR.Code != http.StatusOK {
		t.Fatalf("create status = %d, body=%s", createRR.Code, createRR.Body.String())
	}
	createPayload := decodeManualProxyResponse(t, createRR)
	proxyID, _ := createPayload["proxyId"].(string)
	if proxyID == "" {
		t.Fatalf("create response missing proxyId: %#v", createPayload)
	}
	created := dao.items[proxyID]
	if created.SourceID != "manual" {
		t.Fatalf("SourceID = %q, want manual", created.SourceID)
	}
	if !strings.Contains(created.ProxyConfig, "user%3Aname") || !strings.Contains(created.ProxyConfig, "pa%3Ass") {
		t.Fatalf("proxy config was not URL-encoded correctly: %s", created.ProxyConfig)
	}
	if strings.Contains(createRR.Body.String(), "pa:ss") {
		t.Fatalf("create response leaked password: %s", createRR.Body.String())
	}

	listReq := httptest.NewRequest(http.MethodGet, "/api/proxy/manual/list", nil)
	listRR := httptest.NewRecorder()
	handler.ServeHTTP(listRR, listReq)
	if listRR.Code != http.StatusOK {
		t.Fatalf("list status = %d, body=%s", listRR.Code, listRR.Body.String())
	}
	listPayload := decodeManualProxyResponse(t, listRR)
	if got := int(listPayload["count"].(float64)); got != 1 {
		t.Fatalf("list count = %d, want 1", got)
	}
	if strings.Contains(listRR.Body.String(), "pa:ss") {
		t.Fatalf("list response leaked password: %s", listRR.Body.String())
	}

	updateBody := `{"name":"Webshare SOCKS","protocol":"socks5","domain":"p.webshare.io","port":1080,"username":"user","password":"pass","groupName":"manual"}`
	updateReq := httptest.NewRequest(http.MethodPut, "/api/proxy/manual/"+proxyID, strings.NewReader(updateBody))
	updateRR := httptest.NewRecorder()
	handler.ServeHTTP(updateRR, updateReq)
	if updateRR.Code != http.StatusOK {
		t.Fatalf("update status = %d, body=%s", updateRR.Code, updateRR.Body.String())
	}
	updated := dao.items[proxyID]
	if updated.ProxyName != "Webshare SOCKS" {
		t.Fatalf("ProxyName = %q, want updated name", updated.ProxyName)
	}
	if !strings.HasPrefix(updated.ProxyConfig, "socks5://") {
		t.Fatalf("ProxyConfig = %q, want socks5 URL", updated.ProxyConfig)
	}

	deleteReq := httptest.NewRequest(http.MethodDelete, "/api/proxy/manual/"+proxyID, nil)
	deleteRR := httptest.NewRecorder()
	handler.ServeHTTP(deleteRR, deleteReq)
	if deleteRR.Code != http.StatusOK {
		t.Fatalf("delete status = %d, body=%s", deleteRR.Code, deleteRR.Body.String())
	}
	if _, ok := dao.items[proxyID]; ok {
		t.Fatalf("proxy was not deleted")
	}
}

func TestManualProxyCreateNormalizesSOCKSAlias(t *testing.T) {
	srv, dao := newManualProxyTestServer()
	handler := NewTestHandler(srv)

	body := `{"name":"UDEAL SOCKS","protocol":"socks","domain":"70.39.164.200","port":30000,"username":"user","password":"pass"}`
	req := httptest.NewRequest(http.MethodPost, "/api/proxy/manual", strings.NewReader(body))
	rr := httptest.NewRecorder()
	handler.ServeHTTP(rr, req)
	if rr.Code != http.StatusOK {
		t.Fatalf("create status = %d, body=%s", rr.Code, rr.Body.String())
	}
	payload := decodeManualProxyResponse(t, rr)
	proxyID, _ := payload["proxyId"].(string)
	created := dao.items[proxyID]
	if !strings.HasPrefix(created.ProxyConfig, "socks5://") {
		t.Fatalf("ProxyConfig = %q, want socks5 URL", created.ProxyConfig)
	}
}

func TestManualProxyRejectsDeletingNonManualProxy(t *testing.T) {
	srv, dao := newManualProxyTestServer()
	dao.items["subscription-1"] = browser.Proxy{
		ProxyId:     "subscription-1",
		ProxyName:   "Subscription Proxy",
		ProxyConfig: "http://example.com:80",
		SourceID:    "subscription",
	}

	req := httptest.NewRequest(http.MethodDelete, "/api/proxy/manual/subscription-1", nil)
	rr := httptest.NewRecorder()
	NewTestHandler(srv).ServeHTTP(rr, req)
	if rr.Code != http.StatusNotFound {
		t.Fatalf("delete status = %d, want 404, body=%s", rr.Code, rr.Body.String())
	}
	if _, ok := dao.items["subscription-1"]; !ok {
		t.Fatalf("non-manual proxy should not be deleted")
	}
}
