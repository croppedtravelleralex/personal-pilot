package launchcode

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"personal-pilot/backend/internal/transport"

	"gopkg.in/yaml.v3"
)

const (
	maxSubscriptionPayloadBytes = 8 * 1024 * 1024
	subscriptionRequestTimeout  = 30 * time.Second
)

// SubscriptionFetcher resolves a URL subscription into normalized nodes.
type SubscriptionFetcher interface {
	Fetch(ctx context.Context, sourceURL string) ([]SubscriptionNode, error)
}

// HTTPSubscriptionFetcher fetches bounded HTTP(S) subscription payloads.
type HTTPSubscriptionFetcher struct {
	client *http.Client
}

func NewHTTPSubscriptionFetcher(client *http.Client) *HTTPSubscriptionFetcher {
	if client == nil {
		client = &http.Client{
			Timeout: subscriptionRequestTimeout,
			CheckRedirect: func(req *http.Request, via []*http.Request) error {
				if len(via) >= 10 {
					return fmt.Errorf("subscription redirect limit exceeded")
				}
				_, err := normalizeSubscriptionSourceURL(req.URL.String())
				return err
			},
		}
	}
	return &HTTPSubscriptionFetcher{client: client}
}

func (f *HTTPSubscriptionFetcher) Fetch(ctx context.Context, sourceURL string) ([]SubscriptionNode, error) {
	if f == nil || f.client == nil {
		return nil, fmt.Errorf("subscription HTTP client is unavailable")
	}
	normalizedURL, err := normalizeSubscriptionSourceURL(sourceURL)
	if err != nil {
		return nil, err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, normalizedURL, nil)
	if err != nil {
		return nil, fmt.Errorf("create subscription request: %w", err)
	}
	req.Header.Set("User-Agent", transport.ProductUserAgent)
	req.Header.Set("Accept", "application/yaml,text/yaml,text/plain,*/*")
	req.Header.Set("Cache-Control", "no-cache")

	resp, err := f.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("fetch subscription: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode < http.StatusOK || resp.StatusCode >= http.StatusMultipleChoices {
		return nil, fmt.Errorf("subscription returned HTTP %d", resp.StatusCode)
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, maxSubscriptionPayloadBytes+1))
	if err != nil {
		return nil, fmt.Errorf("read subscription response: %w", err)
	}
	if len(body) > maxSubscriptionPayloadBytes {
		return nil, fmt.Errorf("subscription payload exceeds %d bytes", maxSubscriptionPayloadBytes)
	}
	return parseSubscriptionPayload(body)
}

func normalizeSubscriptionSourceURL(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return "", fmt.Errorf("subscription URL is required")
	}
	if len(raw) > 4096 {
		return "", fmt.Errorf("subscription URL is too long")
	}
	parsed, err := url.Parse(raw)
	if err != nil {
		return "", fmt.Errorf("invalid subscription URL: %w", err)
	}
	parsed.Scheme = strings.ToLower(strings.TrimSpace(parsed.Scheme))
	if parsed.Scheme != "http" && parsed.Scheme != "https" {
		return "", fmt.Errorf("subscription URL must use http or https")
	}
	if strings.TrimSpace(parsed.Hostname()) == "" {
		return "", fmt.Errorf("subscription URL host is empty")
	}
	if parsed.User != nil {
		return "", fmt.Errorf("subscription URL must not embed credentials")
	}
	parsed.Host = strings.ToLower(parsed.Host)
	parsed.Fragment = ""
	return parsed.String(), nil
}

func redactSubscriptionSourceURL(raw string) string {
	if strings.TrimSpace(raw) == "" {
		return ""
	}
	parsed, err := url.Parse(raw)
	if err != nil {
		return "[redacted-subscription-url]"
	}
	parsed.User = nil
	parsed.RawQuery = ""
	parsed.Fragment = ""
	segments := strings.Split(strings.Trim(parsed.Path, "/"), "/")
	if len(segments) > 0 && segments[0] != "" {
		segments[len(segments)-1] = "***"
		parsed.Path = "/" + strings.Join(segments, "/")
	}
	return parsed.String()
}

func parseSubscriptionPayload(body []byte) ([]SubscriptionNode, error) {
	raw := strings.TrimSpace(strings.ReplaceAll(string(body), "\r\n", "\n"))
	if raw == "" {
		return nil, fmt.Errorf("subscription payload is empty")
	}
	candidates := []string{raw}
	if decoded, ok := decodeSubscriptionBase64(raw); ok && decoded != raw {
		candidates = append(candidates, decoded)
	}
	for _, candidate := range candidates {
		if nodes := parseURLSubscriptionNodes(candidate); len(nodes) > 0 {
			return normalizeSubscriptionNodes(nodes)
		}
	}
	for _, candidate := range candidates {
		if nodes, err := parseClashSubscriptionNodes(candidate); err == nil && len(nodes) > 0 {
			return normalizeSubscriptionNodes(nodes)
		}
	}
	return nil, fmt.Errorf("subscription payload contains no supported proxy nodes")
}

func decodeSubscriptionBase64(raw string) (string, bool) {
	candidate := strings.TrimSpace(raw)
	if candidate == "" || strings.Contains(candidate, "\n") {
		return "", false
	}
	padded := candidate
	if mod := len(padded) % 4; mod != 0 {
		padded += strings.Repeat("=", 4-mod)
	}
	for _, encoding := range []*base64.Encoding{
		base64.StdEncoding,
		base64.RawStdEncoding,
		base64.URLEncoding,
		base64.RawURLEncoding,
	} {
		for _, value := range []string{candidate, padded} {
			decoded, err := encoding.DecodeString(value)
			if err != nil {
				continue
			}
			text := strings.TrimSpace(strings.ReplaceAll(string(decoded), "\r\n", "\n"))
			if text != "" {
				return text, true
			}
		}
	}
	return "", false
}

func parseURLSubscriptionNodes(raw string) []SubscriptionNode {
	supported := []string{
		"vmess://", "vless://", "trojan://", "ss://", "ssr://",
		"hysteria2://", "tuic://", "anytls://", "http://", "https://", "socks5://", "socks5h://",
	}
	lines := strings.Split(raw, "\n")
	nodes := make([]SubscriptionNode, 0, len(lines))
	seen := make(map[string]struct{}, len(lines))
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		proxyConfig, nodeName := splitSubscriptionNodeURL(line)
		lower := strings.ToLower(proxyConfig)
		supportedNode := false
		for _, prefix := range supported {
			if strings.HasPrefix(lower, prefix) {
				supportedNode = true
				break
			}
		}
		if !supportedNode {
			continue
		}
		key := strings.TrimSpace(proxyConfig)
		if _, exists := seen[key]; exists {
			continue
		}
		seen[key] = struct{}{}
		if nodeName == "" {
			nodeName = subscriptionNodeNameFromURL(proxyConfig)
		}
		nodes = append(nodes, SubscriptionNode{Name: nodeName, ProxyConfig: proxyConfig})
	}
	return nodes
}

func splitSubscriptionNodeURL(raw string) (string, string) {
	raw = strings.TrimSpace(raw)
	if index := strings.LastIndex(raw, "#"); index >= 0 {
		name := strings.TrimSpace(raw[index+1:])
		if decoded, err := url.QueryUnescape(name); err == nil {
			name = decoded
		}
		return strings.TrimSpace(raw[:index]), name
	}
	return raw, ""
}

func subscriptionNodeNameFromURL(raw string) string {
	lower := strings.ToLower(strings.TrimSpace(raw))
	if strings.HasPrefix(lower, "vmess://") {
		encoded := strings.TrimSpace(raw[len("vmess://"):])
		if decoded, ok := decodeSubscriptionBase64(encoded); ok {
			var payload struct {
				Name string `json:"ps"`
			}
			if json.Unmarshal([]byte(decoded), &payload) == nil && strings.TrimSpace(payload.Name) != "" {
				return strings.TrimSpace(payload.Name)
			}
		}
	}
	parsed, err := url.Parse(raw)
	if err == nil && parsed.Hostname() != "" {
		return strings.ToUpper(parsed.Scheme) + "-" + parsed.Hostname()
	}
	for _, prefix := range []string{"vless://", "trojan://", "ss://", "ssr://", "hysteria2://", "tuic://", "anytls://"} {
		if strings.HasPrefix(lower, prefix) {
			return strings.ToUpper(strings.TrimSuffix(prefix, "://")) + "-node"
		}
	}
	return "subscription-node"
}

func parseClashSubscriptionNodes(raw string) ([]SubscriptionNode, error) {
	var payload struct {
		Proxies []map[string]interface{} `yaml:"proxies"`
	}
	if err := yaml.Unmarshal([]byte(raw), &payload); err != nil {
		return nil, fmt.Errorf("parse Clash subscription: %w", err)
	}
	if len(payload.Proxies) == 0 {
		return nil, fmt.Errorf("Clash subscription contains no proxies")
	}
	nodes := make([]SubscriptionNode, 0, len(payload.Proxies))
	for index, item := range payload.Proxies {
		name := strings.TrimSpace(clashString(item, "name"))
		if name == "" {
			name = fmt.Sprintf("clash-node-%d", index+1)
		}
		config, err := clashNodeProxyConfig(item)
		if err != nil {
			return nil, fmt.Errorf("parse Clash node %q: %w", name, err)
		}
		nodes = append(nodes, SubscriptionNode{Name: name, ProxyConfig: config})
	}
	return nodes, nil
}

func clashNodeProxyConfig(item map[string]interface{}) (string, error) {
	nodeType := strings.ToLower(strings.TrimSpace(clashString(item, "type")))
	if nodeType == "http" || nodeType == "https" || nodeType == "socks5" {
		host := strings.TrimSpace(clashString(item, "server"))
		port := clashInt(item, "port")
		if host == "" || port < 1 || port > 65535 {
			return "", fmt.Errorf("standard proxy host or port is invalid")
		}
		proxyURL := url.URL{Scheme: nodeType, Host: fmt.Sprintf("%s:%d", host, port)}
		username := clashString(item, "username")
		password := clashString(item, "password")
		if username != "" || password != "" {
			proxyURL.User = url.UserPassword(username, password)
		}
		return proxyURL.String(), nil
	}
	if nodeType == "" {
		return "", fmt.Errorf("node type is required")
	}
	encoded, err := yaml.Marshal(item)
	if err != nil {
		return "", fmt.Errorf("encode Clash node: %w", err)
	}
	return strings.TrimSpace(string(encoded)), nil
}

func clashString(item map[string]interface{}, key string) string {
	value, exists := item[key]
	if !exists || value == nil {
		return ""
	}
	return strings.TrimSpace(fmt.Sprint(value))
}

func clashInt(item map[string]interface{}, key string) int {
	value := clashString(item, key)
	parsed, _ := strconv.Atoi(value)
	return parsed
}
