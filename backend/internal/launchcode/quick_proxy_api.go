package launchcode

import (
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strconv"
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
	Raw      string `json:"raw"`
	Protocol string `json:"protocol,omitempty"`
}

type directProxyLineParts struct {
	Server   string
	Port     string
	Username string
	Password string
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

	if payload, ok, err := parseStandardProxyURLPreview(raw); ok {
		if err != nil {
			writeJSON(w, http.StatusBadRequest, map[string]interface{}{
				"ok":    false,
				"error": fmt.Sprintf("invalid proxy URL: %v", err),
			})
			return
		}
		writeJSON(w, http.StatusOK, payload)
		return
	}

	if line, ok := parseDirectProxyLine(raw); ok {
		protocol := normalizeParseProxyProtocol(req.Protocol)
		port, _ := strconv.Atoi(line.Port)
		proxyConfig := buildDirectProxyConfig(protocol, line)
		writeJSON(w, http.StatusOK, map[string]interface{}{
			"ok":          true,
			"format":      "direct",
			"protocol":    protocol,
			"host":        line.Server,
			"port":        port,
			"hasAuth":     strings.TrimSpace(line.Username) != "" || strings.TrimSpace(line.Password) != "",
			"proxyConfig": proxy.RedactProxyURL(proxyConfig),
		})
		return
	}

	format := detectProxyFormat(raw)

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
	normalized := proxy.NormalizeStandardProxyScheme(raw)

	if strings.HasPrefix(strings.ToLower(normalized), "socks5://") ||
		strings.HasPrefix(strings.ToLower(normalized), "http://") ||
		strings.HasPrefix(strings.ToLower(normalized), "https://") {
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

func parseStandardProxyURLPreview(raw string) (map[string]interface{}, bool, error) {
	src := proxy.NormalizeStandardProxyScheme(raw)
	parsed, err := url.Parse(src)
	if err != nil {
		if strings.Contains(raw, "://") {
			return nil, true, err
		}
		return nil, false, nil
	}
	protocol := strings.ToLower(strings.TrimSpace(parsed.Scheme))
	if protocol != "http" && protocol != "https" && protocol != "socks5" {
		return nil, false, nil
	}
	port := 0
	if parsed.Port() != "" {
		port, _ = strconv.Atoi(parsed.Port())
	}
	hasAuth := false
	if parsed.User != nil {
		password, hasPassword := parsed.User.Password()
		hasAuth = strings.TrimSpace(parsed.User.Username()) != "" || (hasPassword && password != "")
	}
	return map[string]interface{}{
		"ok":          true,
		"format":      "url",
		"protocol":    protocol,
		"host":        parsed.Hostname(),
		"port":        port,
		"hasAuth":     hasAuth,
		"proxyConfig": proxy.RedactProxyURL(src),
	}, true, nil
}

func normalizeParseProxyProtocol(raw string) string {
	switch strings.ToLower(strings.TrimSpace(raw)) {
	case "https":
		return "https"
	case "socks", "socks5", "socks5h", "socket":
		return "socks5"
	default:
		return "http"
	}
}

func parseDirectProxyLine(raw string) (*directProxyLineParts, bool) {
	input := strings.TrimSpace(raw)
	if input == "" || strings.Contains(input, "://") {
		return nil, false
	}

	if atIndex := strings.Index(input, "@"); atIndex > 0 && atIndex < len(input)-1 {
		left := strings.TrimSpace(input[:atIndex])
		right := strings.TrimSpace(input[atIndex+1:])
		leftParts := splitDirectProxyLineFields(left)
		rightParts := splitDirectProxyLineFields(right)
		if len(leftParts) != 2 || len(rightParts) != 2 {
			return nil, false
		}
		return chooseDirectProxyLineParts(
			buildDirectProxyLineParts(leftParts[0], leftParts[1], rightParts[0], rightParts[1]),
			buildDirectProxyLineParts(rightParts[0], rightParts[1], leftParts[0], leftParts[1]),
		)
	}

	parts := splitDirectProxyLineFields(input)
	if len(parts) == 2 && isValidDirectProxyPort(parts[1]) {
		part := buildDirectProxyLineParts(parts[0], parts[1], "", "")
		return part, part != nil
	}
	if len(parts) != 4 {
		return nil, false
	}
	return chooseDirectProxyLineParts(
		buildDirectProxyLineParts(parts[0], parts[1], parts[2], parts[3]),
		buildDirectProxyLineParts(parts[2], parts[3], parts[0], parts[1]),
	)
}

func splitDirectProxyLineFields(input string) []string {
	rawParts := strings.Split(input, ":")
	parts := make([]string, 0, len(rawParts))
	for _, part := range rawParts {
		parts = append(parts, strings.TrimSpace(part))
	}
	return parts
}

func buildDirectProxyLineParts(server string, port string, username string, password string) *directProxyLineParts {
	server = normalizeDirectProxyServer(server)
	if server == "" || strings.ContainsAny(server, "/?#@") || !isValidDirectProxyPort(port) {
		return nil
	}
	return &directProxyLineParts{
		Server:   server,
		Port:     strings.TrimSpace(port),
		Username: strings.TrimSpace(username),
		Password: password,
	}
}

func chooseDirectProxyLineParts(preferred *directProxyLineParts, alternative *directProxyLineParts) (*directProxyLineParts, bool) {
	if preferred == nil && alternative == nil {
		return nil, false
	}
	if preferred == nil {
		return alternative, true
	}
	if alternative == nil {
		return preferred, true
	}
	if scoreDirectProxyServer(alternative.Server) > scoreDirectProxyServer(preferred.Server) {
		return alternative, true
	}
	return preferred, true
}

func normalizeDirectProxyServer(raw string) string {
	return strings.Trim(strings.TrimSpace(raw), "[]")
}

func scoreDirectProxyServer(raw string) int {
	server := normalizeDirectProxyServer(raw)
	if server == "" {
		return -1
	}
	if net.ParseIP(server) != nil {
		return 4
	}
	if strings.Contains(server, ".") {
		return 3
	}
	if strings.Contains(server, ":") {
		return 3
	}
	if strings.EqualFold(server, "localhost") {
		return 2
	}
	if isSimpleHostname(server) {
		return 1
	}
	return 0
}

func isSimpleHostname(server string) bool {
	if server == "" {
		return false
	}
	for _, r := range server {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '-' {
			continue
		}
		return false
	}
	_, numericErr := strconv.Atoi(server)
	return numericErr != nil
}

func isValidDirectProxyPort(raw string) bool {
	if raw == "" {
		return false
	}
	port, err := strconv.Atoi(strings.TrimSpace(raw))
	if err != nil {
		return false
	}
	return port >= 1 && port <= 65535
}

func buildDirectProxyConfig(protocol string, line *directProxyLineParts) string {
	u := url.URL{
		Scheme: protocol,
		Host:   net.JoinHostPort(line.Server, line.Port),
	}
	if strings.TrimSpace(line.Username) != "" || strings.TrimSpace(line.Password) != "" {
		u.User = url.UserPassword(strings.TrimSpace(line.Username), line.Password)
	}
	return u.String()
}
