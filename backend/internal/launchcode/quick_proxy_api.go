package launchcode

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"

	"personal-pilot/backend/internal/proxy"
)

// QuickAddRequest POST /api/proxy/quick-add 的请求体
type QuickAddRequest struct {
	Raw       string `json:"raw"`
	GroupName string `json:"groupName"`
	Name      string `json:"name"`
}

// ParseRequest POST /api/proxy/parse 的请求体
type ParseRequest struct {
	Raw string `json:"raw"`
}

// handleQuickAddProxy POST /api/proxy/quick-add
func (s *LaunchServer) handleQuickAddProxy(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}

	var req QuickAddRequest
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid request body",
		})
		return
	}

	raw := strings.TrimSpace(req.Raw)
	if raw == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "raw is required",
		})
		return
	}

	name := strings.TrimSpace(req.Name)

	// Determine format
	format := detectProxyFormat(raw)
	proxyConfig := raw

	if format == "url" {
		// For URL format, just use the raw as-is
		proxyConfig = raw
	} else if format == "vmess" || format == "vless" || format == "trojan" || format == "ss" {
		// Pass through as-is
		proxyConfig = raw
	} else if format == "clash" {
		// Pass through as Clash format
		proxyConfig = raw
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"proxyConfig": proxy.RedactProxyURL(proxyConfig),
		"format":      format,
		"name":        name,
	})
}

// handleParseProxy POST /api/proxy/parse
func (s *LaunchServer) handleParseProxy(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{
			"ok":    false,
			"error": "method not allowed",
		})
		return
	}

	var req ParseRequest
	dec := json.NewDecoder(io.LimitReader(r.Body, 1<<20))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&req); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "invalid request body",
		})
		return
	}

	raw := strings.TrimSpace(req.Raw)
	if raw == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{
			"ok":    false,
			"error": "raw is required",
		})
		return
	}

	format := detectProxyFormat(raw)

	if format == "url" {
		// Parse the URL
		parsed, err := url.Parse(raw)
		if err != nil {
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{
				"ok":    false,
				"error": fmt.Sprintf("invalid proxy URL: %v", err),
			})
			return
		}
		hasAuth := parsed.User != nil
		port := 0
		if parsed.Port() != "" {
			fmt.Sscanf(parsed.Port(), "%d", &port)
		}

		writeJSON(w, http.StatusOK, map[string]interface{}{
			"ok":          true,
			"format":      format,
			"protocol":    parsed.Scheme,
			"host":        parsed.Hostname(),
			"port":        port,
			"hasAuth":     hasAuth,
			"proxyConfig": proxy.RedactProxyURL(raw),
		})
		return
	}

	// For other formats, return format and raw config
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":          true,
		"format":      format,
		"proxyConfig": proxy.RedactProxyURL(raw),
	})
}

// detectProxyFormat 检测代理配置格式
func detectProxyFormat(raw string) string {
	raw = strings.TrimSpace(raw)

	if strings.HasPrefix(raw, "socks5://") || strings.HasPrefix(raw, "http://") || strings.HasPrefix(raw, "https://") {
		return "url"
	}
	if strings.HasPrefix(raw, "vmess://") {
		return "vmess"
	}
	if strings.HasPrefix(raw, "vless://") {
		return "vless"
	}
	if strings.HasPrefix(raw, "trojan://") {
		return "trojan"
	}
	if strings.HasPrefix(raw, "ss://") {
		return "ss"
	}
	if strings.Contains(raw, "type:") || strings.Contains(raw, "proxies:") {
		return "clash"
	}
	return "unknown"
}
