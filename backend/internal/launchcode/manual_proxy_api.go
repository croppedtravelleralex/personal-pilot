package launchcode

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/google/uuid"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/proxy"
)

// ManualProxyRequest 手动添加代理的请求体
type ManualProxyRequest struct {
	Name       string `json:"name"`
	Protocol   string `json:"protocol"`
	Domain     string `json:"domain"`
	Port       int    `json:"port"`
	Username   string `json:"username,omitempty"`
	Password   string `json:"password,omitempty"`
	GroupName  string `json:"groupName,omitempty"`
	DNSServers string `json:"dnsServers,omitempty"`
	ViaSSH     string `json:"viaSsh,omitempty"`
	SSHPort    int    `json:"sshPort,omitempty"`
}

// ManualProxyBatchItem 批量添加代理的单条记录
type ManualProxyBatchItem struct {
	Name     string `json:"name"`
	Protocol string `json:"protocol"`
	Domain   string `json:"domain"`
	Port     int    `json:"port"`
	Username string `json:"username,omitempty"`
	Password string `json:"password,omitempty"`
}

// ManualProxyBatchRequest 批量添加代理的请求体
type ManualProxyBatchRequest struct {
	Proxies []ManualProxyBatchItem `json:"proxies"`
}

// validProxyProtocols 支持的代理协议集合
var validProxyProtocols = map[string]bool{
	"http":    true,
	"https":   true,
	"socks":   true,
	"socks5":  true,
	"socks5h": true,
	"socket":  true,
}

func normalizeManualProxyProtocol(raw string) string {
	switch strings.ToLower(strings.TrimSpace(raw)) {
	case "socks", "socks5h", "socket":
		return "socks5"
	default:
		return strings.ToLower(strings.TrimSpace(raw))
	}
}

func validateManualProxyRequest(req ManualProxyRequest) (int, string) {
	if strings.TrimSpace(req.Name) == "" {
		return http.StatusBadRequest, "name is required"
	}
	if !validProxyProtocols[strings.ToLower(strings.TrimSpace(req.Protocol))] {
		return http.StatusBadRequest, "protocol must be one of http, https, socks5"
	}
	if strings.TrimSpace(req.Domain) == "" {
		return http.StatusBadRequest, "domain is required"
	}
	if req.Port < 1 || req.Port > 65535 {
		return http.StatusBadRequest, "port must be between 1 and 65535"
	}
	if strings.TrimSpace(req.Password) != "" && strings.TrimSpace(req.Username) == "" {
		return http.StatusBadRequest, "username is required when password is set"
	}
	return http.StatusOK, ""
}

func validateManualProxyBatchItem(item ManualProxyBatchItem) (int, string) {
	if strings.TrimSpace(item.Name) == "" {
		return http.StatusBadRequest, "name is required"
	}
	if !validProxyProtocols[strings.ToLower(strings.TrimSpace(item.Protocol))] {
		return http.StatusBadRequest, "protocol must be one of http, https, socks5"
	}
	if strings.TrimSpace(item.Domain) == "" {
		return http.StatusBadRequest, "domain is required"
	}
	if item.Port < 1 || item.Port > 65535 {
		return http.StatusBadRequest, "port must be between 1 and 65535"
	}
	if strings.TrimSpace(item.Password) != "" && strings.TrimSpace(item.Username) == "" {
		return http.StatusBadRequest, "username is required when password is set"
	}
	return http.StatusOK, ""
}

func appendProxyTunnelQuery(raw string, viaSSH string, sshPort int) string {
	viaSSH = strings.TrimSpace(viaSSH)
	if viaSSH == "" {
		return raw
	}
	u, err := url.Parse(raw)
	if err != nil {
		return raw
	}
	q := u.Query()
	q.Set("pp_via_ssh", viaSSH)
	if sshPort > 0 {
		q.Set("pp_ssh_port", strconv.Itoa(sshPort))
	}
	u.RawQuery = q.Encode()
	return u.String()
}

func buildManualProxyConfig(req ManualProxyRequest) string {
	u := url.URL{
		Scheme: normalizeManualProxyProtocol(req.Protocol),
		Host:   net.JoinHostPort(strings.Trim(strings.TrimSpace(req.Domain), "[]"), strconv.Itoa(req.Port)),
	}
	if strings.TrimSpace(req.Username) != "" || strings.TrimSpace(req.Password) != "" {
		u.User = url.UserPassword(strings.TrimSpace(req.Username), req.Password)
	}
	return appendProxyTunnelQuery(u.String(), req.ViaSSH, req.SSHPort)
}

func buildManualProxyConfigFromItem(item ManualProxyBatchItem) string {
	u := url.URL{
		Scheme: normalizeManualProxyProtocol(item.Protocol),
		Host:   net.JoinHostPort(strings.Trim(strings.TrimSpace(item.Domain), "[]"), strconv.Itoa(item.Port)),
	}
	if strings.TrimSpace(item.Username) != "" || strings.TrimSpace(item.Password) != "" {
		u.User = url.UserPassword(strings.TrimSpace(item.Username), item.Password)
	}
	return u.String()
}

func (s *LaunchServer) manualProxyServiceAvailable() bool {
	return s != nil && s.browserMgr != nil && s.browserMgr.ProxyDAO != nil
}

func isManualProxyItem(item browser.Proxy) bool {
	return strings.EqualFold(strings.TrimSpace(item.SourceID), "manual") ||
		strings.HasPrefix(strings.ToLower(strings.TrimSpace(item.ProxyId)), "manual-")
}

func manualProxyResponseItem(item browser.Proxy) map[string]interface{} {
	return map[string]interface{}{
		"proxyId":          item.ProxyId,
		"proxyName":        item.ProxyName,
		"proxyConfig":      proxy.RedactProxyURL(item.ProxyConfig),
		"groupName":        item.GroupName,
		"dnsServers":       item.DnsServers,
		"sortOrder":        item.SortOrder,
		"lastLatencyMs":    item.LastLatencyMs,
		"lastTestOk":       item.LastTestOk,
		"lastTestedAt":     item.LastTestedAt,
		"lastIPHealthJson": item.LastIPHealthJSON,
	}
}

func (s *LaunchServer) findManualProxy(proxyID string) (browser.Proxy, bool, error) {
	if !s.manualProxyServiceAvailable() {
		return browser.Proxy{}, false, fmt.Errorf("proxy service not available")
	}
	list, err := s.browserMgr.ProxyDAO.List()
	if err != nil {
		return browser.Proxy{}, false, err
	}
	for _, item := range list {
		if strings.EqualFold(item.ProxyId, proxyID) && isManualProxyItem(item) {
			return item, true, nil
		}
	}
	return browser.Proxy{}, false, nil
}

// handleManualProxyCreate POST /api/proxy/manual
func (s *LaunchServer) handleManualProxyCreate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}

	var req ManualProxyRequest
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid JSON",
		})
		return
	}

	if status, errMsg := validateManualProxyRequest(req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{
			"ok":    false,
			"error": errMsg,
		})
		return
	}
	if !s.manualProxyServiceAvailable() {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "proxy service not available",
		})
		return
	}

	proxyID := fmt.Sprintf("manual-%s", uuid.New().String()[:8])
	proxyConfig := buildManualProxyConfig(req)
	existing, _ := s.browserMgr.ProxyDAO.List()
	item := browser.Proxy{
		ProxyId:     proxyID,
		ProxyName:   strings.TrimSpace(req.Name),
		ProxyConfig: proxyConfig,
		DnsServers:  strings.TrimSpace(req.DNSServers),
		GroupName:   strings.TrimSpace(req.GroupName),
		SourceID:    "manual",
		SortOrder:   len(existing),
	}
	if err := s.browserMgr.ProxyDAO.Upsert(item); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"proxyId":     proxyID,
		"item":        manualProxyResponseItem(item),
		"proxyConfig": proxy.RedactProxyURL(proxyConfig),
	})
}

// handleManualProxyList GET /api/proxy/manual/list
func (s *LaunchServer) handleManualProxyList(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}
	if !s.manualProxyServiceAvailable() {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "proxy service not available",
		})
		return
	}

	list, err := s.browserMgr.ProxyDAO.List()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	items := make([]map[string]interface{}, 0, len(list))
	for _, item := range list {
		if isManualProxyItem(item) {
			items = append(items, manualProxyResponseItem(item))
		}
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"count": len(items),
		"items": items,
	})
}

// handleManualProxyByID PUT/DELETE /api/proxy/manual/{id}
func (s *LaunchServer) handleManualProxyByID(w http.ResponseWriter, r *http.Request) {
	path := r.URL.Path
	// Check for test sub-resource
	if strings.HasSuffix(path, "/test") {
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		id := strings.TrimSuffix(strings.TrimPrefix(path, "/api/proxy/manual/"), "/test")
		id = strings.TrimSpace(id)
		if id == "" || strings.Contains(id, "/") {
			writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "proxy not found"})
			return
		}
		s.handleManualProxyTest(w, r, id)
		return
	}

	proxyID := strings.TrimPrefix(path, "/api/proxy/manual/")
	proxyID = strings.TrimSpace(proxyID)
	if proxyID == "" || strings.Contains(proxyID, "/") {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{
			"ok":    false,
			"error": "proxy not found",
		})
		return
	}

	switch r.Method {
	case http.MethodPut:
		s.handleManualProxyUpdate(w, r, proxyID)
	case http.MethodDelete:
		s.handleManualProxyDelete(w, r, proxyID)
	default:
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
	}
}

// handleManualProxyBatch POST /api/proxy/manual/batch
func (s *LaunchServer) handleManualProxyBatch(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}

	var req ManualProxyBatchRequest
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid JSON",
		})
		return
	}

	if len(req.Proxies) == 0 {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "proxies array is required",
		})
		return
	}

	results := make([]map[string]interface{}, 0, len(req.Proxies))
	if !s.manualProxyServiceAvailable() {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "proxy service not available"})
		return
	}
	existing, _ := s.browserMgr.ProxyDAO.List()
	sortOrder := len(existing)
	created := 0
	for _, item := range req.Proxies {
		if _, errMsg := validateManualProxyBatchItem(item); errMsg != "" {
			results = append(results, map[string]interface{}{
				"error": errMsg,
				"name":  item.Name,
			})
			continue
		}
		proxyID := fmt.Sprintf("manual-%s", uuid.New().String()[:8])
		proxyConfig := buildManualProxyConfigFromItem(item)
		proxyItem := browser.Proxy{
			ProxyId:     proxyID,
			ProxyName:   strings.TrimSpace(item.Name),
			ProxyConfig: proxyConfig,
			SourceID:    "manual",
			SortOrder:   sortOrder,
		}
		sortOrder++
		if err := s.browserMgr.ProxyDAO.Upsert(proxyItem); err != nil {
			results = append(results, map[string]interface{}{
				"error": err.Error(),
				"name":  item.Name,
			})
			continue
		}
		created++
		results = append(results, manualProxyResponseItem(proxyItem))
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":      true,
		"count":   created,
		"results": results,
	})
}

// handleManualProxyTest POST /api/proxy/manual/{id}/test
func (s *LaunchServer) handleManualProxyTest(w http.ResponseWriter, _ *http.Request, id string) {
	item, found, err := s.findManualProxy(id)
	if err != nil {
		status := http.StatusInternalServerError
		if strings.Contains(err.Error(), "not available") {
			status = http.StatusServiceUnavailable
		}
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": err.Error()})
		return
	}
	if !found {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "proxy not found"})
		return
	}
	proxies := []browser.Proxy{item}
	var managers []proxy.BridgeManager
	if s.starter != nil {
		if bp, ok := s.starter.(ProxyBridgeProvider); ok {
			managers = bp.ProxyBridgeManagers()
		}
	}
	speedResult := proxy.SpeedTest(context.Background(), item.ProxyId, proxies, managers, &proxy.SpeedTestConfig{
		Timeout:    12 * time.Second,
		TCPTimeout: 6 * time.Second,
	})
	testedAt := time.Now().Format(time.RFC3339)
	_ = s.browserMgr.ProxyDAO.UpdateSpeedResult(item.ProxyId, speedResult.Ok, speedResult.LatencyMs, testedAt)

	ipHealth, ipErr := proxy.FetchProxyIPInfo(context.Background(), item.ProxyId, proxies, managers)
	if ipHealth != nil {
		if payload, marshalErr := json.Marshal(ipHealth); marshalErr == nil {
			_ = s.browserMgr.ProxyDAO.UpdateIPHealthResult(item.ProxyId, string(payload))
		}
	}
	ipHealthOK := ipErr == nil
	ipHealthError := ""
	if ipErr != nil {
		ipHealthError = ipErr.Error()
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":            speedResult.Ok && ipHealthOK,
		"tested":        true,
		"id":            id,
		"reachable":     speedResult.Ok,
		"latencyMs":     speedResult.LatencyMs,
		"error":         speedResult.Error,
		"ipHealthOk":    ipHealthOK,
		"ipHealthError": ipHealthError,
		"ipHealth":      ipHealth,
	})
}

// handleManualProxyUpdate PUT /api/proxy/manual/{id}
func (s *LaunchServer) handleManualProxyUpdate(w http.ResponseWriter, r *http.Request, proxyID string) {
	var req ManualProxyRequest
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid JSON",
		})
		return
	}

	if status, errMsg := validateManualProxyRequest(req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{
			"ok":    false,
			"error": errMsg,
		})
		return
	}
	if !s.manualProxyServiceAvailable() {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "proxy service not available",
		})
		return
	}
	existing, found, err := s.findManualProxy(proxyID)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	if !found {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{
			"ok":    false,
			"error": "proxy not found",
		})
		return
	}

	proxyConfig := buildManualProxyConfig(req)
	item := browser.Proxy{
		ProxyId:                proxyID,
		ProxyName:              strings.TrimSpace(req.Name),
		ProxyConfig:            proxyConfig,
		DnsServers:             strings.TrimSpace(req.DNSServers),
		GroupName:              strings.TrimSpace(req.GroupName),
		SourceID:               "manual",
		SortOrder:              existing.SortOrder,
		LastLatencyMs:          existing.LastLatencyMs,
		LastTestOk:             existing.LastTestOk,
		LastTestedAt:           existing.LastTestedAt,
		LastIPHealthJSON:       existing.LastIPHealthJSON,
		SourceURL:              existing.SourceURL,
		SourceNamePrefix:       existing.SourceNamePrefix,
		SourceAutoRefresh:      existing.SourceAutoRefresh,
		SourceRefreshIntervalM: existing.SourceRefreshIntervalM,
		SourceLastRefreshAt:    existing.SourceLastRefreshAt,
	}
	if err := s.browserMgr.ProxyDAO.Upsert(item); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"proxyId":     proxyID,
		"item":        manualProxyResponseItem(item),
		"proxyConfig": proxy.RedactProxyURL(proxyConfig),
	})
}

// handleManualProxyDelete DELETE /api/proxy/manual/{id}
func (s *LaunchServer) handleManualProxyDelete(w http.ResponseWriter, _ *http.Request, proxyID string) {
	if !s.manualProxyServiceAvailable() {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "proxy service not available",
		})
		return
	}
	if _, found, err := s.findManualProxy(proxyID); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	} else if !found {
		writeJSON(w, http.StatusNotFound, map[string]interface{}{
			"ok":    false,
			"error": "proxy not found",
		})
		return
	}
	if err := s.browserMgr.ProxyDAO.Delete(proxyID); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":      true,
		"deleted": true,
		"id":      proxyID,
	})
}
