package backend

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/logger"
)

const (
	subscriptionURL             = "https://ly513.nb666666.com/6/4f5de92a86fee60497d895db0e970127"
	subscriptionFetchTimeout    = 30 * time.Second
	subscriptionGroupName       = "订阅代理"
	subscriptionSourceID        = "subscription-main"
	subscriptionSourcePrefix    = "SUB"
)

// ImportSubscriptionProxies 从订阅 URL 拉取代理列表并导入代理池。
func (a *App) ImportSubscriptionProxies() {
	log := logger.New("ProxySubscription")

	ctx, cancel := context.WithTimeout(context.Background(), subscriptionFetchTimeout)
	defer cancel()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, subscriptionURL, nil)
	if err != nil {
		log.Warn("创建订阅请求失败", logger.F("error", err.Error()))
		return
	}
	req.Header.Set("User-Agent", "personal-pilot/1.0")
	req.Header.Set("Accept", "text/plain,*/*")

	client := &http.Client{Timeout: subscriptionFetchTimeout}
	resp, err := client.Do(req)
	if err != nil {
		log.Warn("订阅 URL 拉取失败", logger.F("error", err.Error()))
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		log.Warn("订阅 URL 返回非成功状态码", logger.F("status", resp.StatusCode))
		return
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, 8*1024*1024))
	if err != nil {
		log.Warn("读取订阅响应失败", logger.F("error", err.Error()))
		return
	}

	rawText := decodeSubscriptionBody(body)
	proxyURLs := parseSubscriptionProxyURLs(rawText)
	if len(proxyURLs) == 0 {
		log.Warn("订阅内容未解析到代理")
		return
	}

	log.Info("订阅代理拉取成功", logger.F("total", len(proxyURLs)))

	imported := a.mergeSubscriptionProxies(proxyURLs)
	log.Info("订阅代理导入完成", logger.F("imported", imported))
}

// RefreshSubscriptionProxies 清除旧订阅代理并重新拉取。
func (a *App) RefreshSubscriptionProxies() error {
	log := logger.New("ProxySubscription")
	log.Info("手动刷新订阅代理...")

	existing := a.getLatestProxies()
	kept := make([]browser.Proxy, 0, len(existing))
	for _, item := range existing {
		if item.SourceID == subscriptionSourceID {
			continue
		}
		kept = append(kept, item)
	}
	if err := a.SaveBrowserProxies(kept); err != nil {
		return fmt.Errorf("清理旧订阅代理失败: %w", err)
	}

	a.ImportSubscriptionProxies()
	return nil
}

func decodeSubscriptionBody(body []byte) string {
	raw := strings.TrimSpace(strings.ReplaceAll(string(body), "\r\n", "\n"))
	if raw == "" {
		return ""
	}

	// 尝试 base64 解码
	if decoded, ok := tryBase64Decode(raw); ok {
		return decoded
	}

	// 尝试 URL decode
	if unescaped, err := url.QueryUnescape(raw); err == nil && unescaped != raw {
		return strings.TrimSpace(strings.ReplaceAll(unescaped, "\r\n", "\n"))
	}

	return raw
}

func tryBase64Decode(raw string) (string, bool) {
	candidate := raw
	if mod := len(candidate) % 4; mod != 0 {
		candidate += strings.Repeat("=", 4-mod)
	}

	encoders := []*base64.Encoding{
		base64.StdEncoding,
		base64.RawStdEncoding,
		base64.URLEncoding,
		base64.RawURLEncoding,
	}
	for _, enc := range encoders {
		if data, err := enc.DecodeString(raw); err == nil {
			decoded := strings.TrimSpace(strings.ReplaceAll(string(data), "\r\n", "\n"))
			if decoded != "" {
				return decoded, true
			}
		}
		if data, err := enc.DecodeString(candidate); err == nil {
			decoded := strings.TrimSpace(strings.ReplaceAll(string(data), "\r\n", "\n"))
			if decoded != "" {
				return decoded, true
			}
		}
	}
	return "", false
}

func parseSubscriptionProxyURLs(rawText string) []string {
	lines := strings.Split(rawText, "\n")
	urls := make([]string, 0, len(lines))
	seen := make(map[string]struct{}, len(lines))

	supportedProtocols := []string{
		"vmess://", "vless://", "trojan://", "ss://", "ssr://",
		"hysteria2://", "tuic://", "anytls://",
		"http://", "https://", "socks5://",
	}

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		proxyURL, _ := splitProxyURLAndName(line)
		key := strings.ToLower(strings.TrimSpace(proxyURL))
		if key == "" {
			continue
		}
		if _, exists := seen[key]; exists {
			continue
		}
		seen[key] = struct{}{}

		lower := strings.ToLower(proxyURL)
		for _, proto := range supportedProtocols {
			if strings.HasPrefix(lower, proto) {
				urls = append(urls, proxyURL)
				break
			}
		}
	}

	return urls
}

func splitProxyURLAndName(raw string) (urlPart, namePart string) {
	if idx := strings.LastIndex(raw, "#"); idx >= 0 {
		return strings.TrimSpace(raw[:idx]), decodeProxyFragmentName(strings.TrimSpace(raw[idx+1:]))
	}
	return strings.TrimSpace(raw), ""
}

func decodeProxyFragmentName(encoded string) string {
	if unescaped, err := url.QueryUnescape(encoded); err == nil && unescaped != encoded {
		return unescaped
	}
	return encoded
}

func proxyNameFromURL(rawURL string) string {
	_, name := splitProxyURLAndName(rawURL)
	if name != "" {
		return name
	}

	lower := strings.ToLower(rawURL)

	// vmess://base64json — 解码 JSON 提取 ps
	if strings.HasPrefix(lower, "vmess://") {
		raw := strings.TrimPrefix(rawURL, "vmess://")
		if decodedStr, ok := decodeBase64Text(raw); ok {
			var v struct {
				Ps string `json:"ps"`
			}
			if jsonErr := json.Unmarshal([]byte(decodedStr), &v); jsonErr == nil && v.Ps != "" {
				return v.Ps
			}
		}
		short := strings.TrimSpace(raw)
		if len(short) > 60 {
			short = short[:60] + "..."
		}
		return "vmess:" + short
	}

	scheme := "UNKNOWN"
	for _, proto := range []string{"vless://", "trojan://", "ss://", "hysteria2://", "tuic://", "anytls://", "http://", "https://", "socks5://"} {
		if strings.HasPrefix(lower, proto) {
			scheme = strings.TrimSuffix(strings.ToUpper(proto), "://")
			break
		}
	}

	rest := rawURL
	if idx := strings.Index(rest, "://"); idx >= 0 {
		rest = rest[idx+3:]
	}
	if idx := strings.Index(rest, "@"); idx >= 0 {
		rest = rest[idx+1:]
	}
	if idx := strings.Index(rest, ":"); idx >= 0 {
		rest = rest[:idx]
	}
	for _, sep := range []string{"/", "?", "#"} {
		if idx := strings.Index(rest, sep); idx >= 0 {
			rest = rest[:idx]
		}
	}
	if rest != "" {
		return subscriptionSourcePrefix + "-" + scheme + "-" + rest
	}
	return subscriptionSourcePrefix + "-" + scheme
}

func (a *App) mergeSubscriptionProxies(proxyURLs []string) int {
	existing := a.getLatestProxies()
	existingByConfig := make(map[string]struct{}, len(existing))
	for _, item := range existing {
		key := strings.ToLower(strings.TrimSpace(item.ProxyConfig))
		if key != "" {
			existingByConfig[key] = struct{}{}
		}
	}

	imported := 0
	for _, proxyURL := range proxyURLs {
		key := strings.ToLower(strings.TrimSpace(proxyURL))
		if _, exists := existingByConfig[key]; exists {
			continue
		}

		proxyID := generateUUID()
		name := proxyNameFromURL(proxyURL)

		proxy := browser.Proxy{
			ProxyId:          proxyID,
			ProxyName:        name,
			ProxyConfig:      proxyURL,
			GroupName:        subscriptionGroupName,
			SourceID:         subscriptionSourceID,
			SourceURL:        subscriptionURL,
			SourceNamePrefix: subscriptionSourcePrefix,
		}

		if a.browserMgr != nil && a.browserMgr.ProxyDAO != nil {
			if err := a.browserMgr.ProxyDAO.Upsert(proxy); err != nil {
				continue
			}
		}

		existingByConfig[key] = struct{}{}
		imported++
	}

	return imported
}
