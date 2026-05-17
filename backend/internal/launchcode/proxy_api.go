package launchcode

import (
	"encoding/json"
	"io"
	"net/http"
	"strings"

	"personal-pilot/backend/internal/browser"
)

// proxyUpdateRequest PUT /api/proxy/{id} 的请求体
type proxyUpdateRequest struct {
	ProxyName   string `json:"proxyName"`
	ProxyConfig string `json:"proxyConfig"`
	GroupName   string `json:"groupName"`
	DnsServers  string `json:"dnsServers"`
	SortOrder   int    `json:"sortOrder"`
}

// handleProxyList GET /api/proxy/list — 列出所有代理节点
func (s *LaunchServer) handleProxyList(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}
	if s.browserMgr == nil || s.browserMgr.ProxyDAO == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "proxy service not available",
		})
		return
	}
	proxies, err := s.browserMgr.ProxyDAO.List()
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}
	if proxies == nil {
		proxies = []browser.Proxy{}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"count": len(proxies),
		"items": proxies,
	})
}

// handleProxyByID PUT /api/proxy/{id} — 更新/替换一个代理节点
func (s *LaunchServer) handleProxyByID(w http.ResponseWriter, r *http.Request) {
	id := strings.TrimPrefix(r.URL.Path, "/api/proxy/")
	if id == "" || strings.Contains(id, "/") {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid proxy id",
		})
		return
	}
	if r.Method != http.MethodPut {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}
	if s.browserMgr == nil || s.browserMgr.ProxyDAO == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{
			"ok":    false,
			"error": "proxy service not available",
		})
		return
	}

	var req proxyUpdateRequest
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid request body",
		})
		return
	}

	proxy := browser.Proxy{
		ProxyId:     id,
		ProxyName:   req.ProxyName,
		ProxyConfig: req.ProxyConfig,
		GroupName:   req.GroupName,
		DnsServers:  req.DnsServers,
		SortOrder:   req.SortOrder,
	}

	if err := s.browserMgr.ProxyDAO.Upsert(proxy); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{
			"ok":    false,
			"error": err.Error(),
		})
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":      true,
		"proxyId": id,
		"msg":     "proxy updated",
	})
}
