package launchcode

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"

	"github.com/google/uuid"
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
}

// validProxyProtocols 支持的代理协议集合
var validProxyProtocols = map[string]bool{
	"http":   true,
	"https":  true,
	"socks5": true,
}

func validateManualProxyRequest(req ManualProxyRequest) (int, string) {
	if strings.TrimSpace(req.Name) == "" {
		return http.StatusBadRequest, "name is required"
	}
	if !validProxyProtocols[req.Protocol] {
		return http.StatusBadRequest, "protocol must be one of http, https, socks5"
	}
	if strings.TrimSpace(req.Domain) == "" {
		return http.StatusBadRequest, "domain is required"
	}
	if req.Port < 1 || req.Port > 65535 {
		return http.StatusBadRequest, "port must be between 1 and 65535"
	}
	return http.StatusOK, ""
}

func buildManualProxyConfig(req ManualProxyRequest) string {
	scheme := req.Protocol
	userinfo := ""
	if strings.TrimSpace(req.Username) != "" || strings.TrimSpace(req.Password) != "" {
		userinfo = fmt.Sprintf("%s:%s@", req.Username, req.Password)
	}
	return fmt.Sprintf("%s://%s%s:%d", scheme, userinfo, req.Domain, req.Port)
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

	proxyID := fmt.Sprintf("manual-%s", uuid.New().String()[:8])
	proxyConfig := buildManualProxyConfig(req)

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"proxyId":     proxyID,
		"proxyConfig": proxyConfig,
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

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":    true,
		"count": 0,
		"items": []interface{}{},
	})
}

// handleManualProxyByID PUT/DELETE /api/proxy/manual/{id}
func (s *LaunchServer) handleManualProxyByID(w http.ResponseWriter, r *http.Request) {
	proxyID := strings.TrimPrefix(r.URL.Path, "/api/proxy/manual/")
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

	proxyConfig := buildManualProxyConfig(req)

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"proxyId":     proxyID,
		"proxyConfig": proxyConfig,
	})
}

// handleManualProxyDelete DELETE /api/proxy/manual/{id}
func (s *LaunchServer) handleManualProxyDelete(w http.ResponseWriter, _ *http.Request, proxyID string) {
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":      true,
		"deleted": true,
		"id":      proxyID,
	})
}
