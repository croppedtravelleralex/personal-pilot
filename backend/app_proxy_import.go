package backend

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

const (
	maxClashSubscriptionBytes = 8 * 1024 * 1024
	clashSubscriptionTimeout  = 25 * time.Second
)

// resolveSubURL 解析 sub:// 自定义方案，返回真实 URL 和建议分组名。
// 格式: sub://base64(real_url)#group_name
func resolveSubURL(rawURL string) (realURL string, groupName string, err error) {
	rawURL = strings.TrimSpace(rawURL)
	if !strings.HasPrefix(strings.ToLower(rawURL), "sub://") {
		return rawURL, "", nil
	}
	rest := rawURL[len("sub://"):]
	// 提取 fragment (#之后) 作为分组名
	if idx := strings.LastIndex(rest, "#"); idx >= 0 {
		groupName = decodeProxyFragmentName(strings.TrimSpace(rest[idx+1:]))
		rest = rest[:idx]
	}
	// rest 是 base64 编码的真实 URL
	decoded, ok := decodeBase64Text(strings.TrimSpace(rest))
	if !ok || decoded == "" {
		return "", "", fmt.Errorf("sub:// URL base64 解码失败")
	}
	realURL = strings.TrimSpace(decoded)
	if realURL == "" {
		return "", "", fmt.Errorf("sub:// 解码后 URL 为空")
	}
	return realURL, groupName, nil
}

// BrowserProxyFetchClashByURL 拉取 Clash 订阅 URL，并返回可直接导入的 YAML 文本与建议配置。
func (a *App) BrowserProxyFetchClashByURL(rawURL string) (map[string]interface{}, error) {
	rawURL = strings.TrimSpace(rawURL)
	if rawURL == "" {
		return nil, fmt.Errorf("订阅 URL 不能为空")
	}

	// 处理 sub:// 自定义方案
	resolvedURL, _, err := resolveSubURL(rawURL)
	if err != nil {
		return nil, err
	}
	rawURL = resolvedURL

	parsedURL, err := url.Parse(rawURL)
	if err != nil || parsedURL.Host == "" {
		return nil, fmt.Errorf("URL 格式无效")
	}
	scheme := strings.ToLower(strings.TrimSpace(parsedURL.Scheme))
	if scheme != "http" && scheme != "https" {
		return nil, fmt.Errorf("仅支持 http/https URL")
	}

	req, err := http.NewRequest(http.MethodGet, parsedURL.String(), nil)
	if err != nil {
		return nil, fmt.Errorf("创建请求失败: %w", err)
	}
	req.Header.Set("User-Agent", "clash-verge/2.0 personal-pilot/1.0")
	req.Header.Set("Accept", "application/yaml,text/yaml,text/plain,*/*")
	req.Header.Set("Cache-Control", "no-cache")

	client := &http.Client{
		Timeout: clashSubscriptionTimeout,
	}
	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("拉取订阅失败: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("拉取订阅失败: HTTP %d", resp.StatusCode)
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, maxClashSubscriptionBytes+1))
	if err != nil {
		return nil, fmt.Errorf("读取订阅内容失败: %w", err)
	}
	if len(body) > maxClashSubscriptionBytes {
		return nil, fmt.Errorf("订阅内容过大（超过 8MB）")
	}

	content, payload, err := normalizeClashSubscriptionContent(body)
	if err != nil {
		// 不是 Clash YAML，尝试作为 base64 代理列表导入
		return a.tryImportSubscriptionFallback(body, parsedURL)
	}

	proxyCount := clashProxyCount(payload)
	if proxyCount <= 0 {
		return a.tryImportSubscriptionFallback(body, parsedURL)
	}

	dnsYAML := extractClashDNSYAML(payload)
	suggestedGroup := suggestClashGroupName(payload, parsedURL.Hostname())

	return map[string]interface{}{
		"url":            parsedURL.String(),
		"content":        content,
		"proxyCount":     proxyCount,
		"dnsServers":     dnsYAML,
		"suggestedGroup": suggestedGroup,
	}, nil
}

// tryImportSubscriptionFallback 当 Clash YAML 解析失败时，尝试作为 base64 代理列表导入。
func (a *App) tryImportSubscriptionFallback(body []byte, parsedURL *url.URL) (map[string]interface{}, error) {
	raw := string(body)
	lines, err := decodeSubscriptionProxyList(raw)
	if err != nil || len(lines) == 0 {
		return nil, fmt.Errorf("URL 内容不是有效 Clash YAML，也不是 base64 代理列表")
	}
	// 自动导入到数据库
	result, importErr := a.importProxyLines(lines, parsedURL.String(), suggestGroupFromURL(parsedURL))
	if importErr != nil {
		return nil, importErr
	}
	result["url"] = parsedURL.String()
	result["autoFallback"] = true
	return result, nil
}

// importProxyLines 将解析出的代理 URL 列表导入数据库。
func (a *App) importProxyLines(lines []string, sourceURL string, groupName string) (map[string]interface{}, error) {
	proxyList := a.getLatestProxies()
	existingByConfig := make(map[string]struct{})
	for _, item := range proxyList {
		key := strings.ToLower(strings.TrimSpace(item.ProxyConfig))
		if key != "" {
			existingByConfig[key] = struct{}{}
		}
	}

	imported := make([]BrowserProxy, 0, len(lines))
	skipped := 0
	for _, proxyURL := range lines {
		key := strings.ToLower(strings.TrimSpace(proxyURL))
		if _, exists := existingByConfig[key]; exists {
			skipped++
			continue
		}
		existingByConfig[key] = struct{}{}

		proxyID := generateUUID()
		proxyName := extractProxyNameFromURL(proxyURL)
		imported = append(imported, BrowserProxy{
			ProxyId:     proxyID,
			ProxyName:   proxyName,
			ProxyConfig: proxyURL,
			GroupName:   groupName,
			SourceID:    sourceURL,
			SourceURL:   sourceURL,
		})
	}

	if len(imported) == 0 {
		return map[string]interface{}{
			"importedCount": 0,
			"skippedCount":  skipped,
			"totalCount":    len(lines),
			"allProxies":    a.BrowserProxyList(),
		}, nil
	}

	allProxies := append(append([]BrowserProxy{}, proxyList...), imported...)
	if err := a.SaveBrowserProxies(allProxies); err != nil {
		return nil, err
	}

	return map[string]interface{}{
		"importedCount": len(imported),
		"skippedCount":  skipped,
		"totalCount":    len(lines),
		"groupName":     groupName,
		"allProxies":    a.BrowserProxyList(),
	}, nil
}

// BrowserProxyImportSubscriptionByURL 从订阅 URL 拉取 base64 编码的代理列表并导入。
// 订阅返回的格式为 base64 编码的纯文本，每行一个代理 URL（支持 anytls/vmess/vless/trojan/ss/hysteria2/tuic 等）。
func (a *App) BrowserProxyImportSubscriptionByURL(rawURL string, groupName string) (map[string]interface{}, error) {
	rawURL = strings.TrimSpace(rawURL)
	if rawURL == "" {
		return nil, fmt.Errorf("订阅 URL 不能为空")
	}

	// 处理 sub:// 自定义方案
	resolvedURL, resolvedGroup, err := resolveSubURL(rawURL)
	if err != nil {
		return nil, err
	}
	rawURL = resolvedURL
	if resolvedGroup != "" && groupName == "" {
		groupName = resolvedGroup
	}

	parsedURL, err := url.Parse(rawURL)
	if err != nil || parsedURL.Host == "" {
		return nil, fmt.Errorf("URL 格式无效")
	}
	scheme := strings.ToLower(strings.TrimSpace(parsedURL.Scheme))
	if scheme != "http" && scheme != "https" {
		return nil, fmt.Errorf("仅支持 http/https URL")
	}

	req, err := http.NewRequest(http.MethodGet, parsedURL.String(), nil)
	if err != nil {
		return nil, fmt.Errorf("创建请求失败: %w", err)
	}
	req.Header.Set("User-Agent", "PersonalPilot/1.0")
	req.Header.Set("Accept", "text/plain,*/*")
	req.Header.Set("Cache-Control", "no-cache")

	client := &http.Client{Timeout: clashSubscriptionTimeout}
	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("拉取订阅失败: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("拉取订阅失败: HTTP %d", resp.StatusCode)
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, maxClashSubscriptionBytes+1))
	if err != nil {
		return nil, fmt.Errorf("读取订阅内容失败: %w", err)
	}
	if len(body) > maxClashSubscriptionBytes {
		return nil, fmt.Errorf("订阅内容过大（超过 8MB）")
	}

	lines, err := decodeSubscriptionProxyList(string(body))
	if err != nil {
		return nil, err
	}
	if len(lines) == 0 {
		return nil, fmt.Errorf("未检测到可导入的代理节点")
	}

	if groupName == "" {
		groupName = suggestGroupFromURL(parsedURL)
	}

	sourceURL := parsedURL.String()
	proxyList := a.getLatestProxies()
	existingByConfig := make(map[string]struct{})
	for _, item := range proxyList {
		key := strings.ToLower(strings.TrimSpace(item.ProxyConfig))
		if key != "" {
			existingByConfig[key] = struct{}{}
		}
	}

	imported := make([]BrowserProxy, 0, len(lines))
	skipped := 0
	for _, proxyURL := range lines {
		key := strings.ToLower(strings.TrimSpace(proxyURL))
		if _, exists := existingByConfig[key]; exists {
			skipped++
			continue
		}
		existingByConfig[key] = struct{}{}

		proxyID := generateUUID()
		proxyName := extractProxyNameFromURL(proxyURL)
		imported = append(imported, BrowserProxy{
			ProxyId:     proxyID,
			ProxyName:   proxyName,
			ProxyConfig: proxyURL,
			GroupName:   groupName,
			SourceID:    sourceURL,
			SourceURL:   sourceURL,
		})
	}

	if len(imported) == 0 {
		return map[string]interface{}{
			"url":           sourceURL,
			"importedCount": 0,
			"skippedCount":  skipped,
			"totalCount":    len(lines),
			"allProxies":    a.BrowserProxyList(),
		}, nil
	}

	allProxies := append(append([]BrowserProxy{}, proxyList...), imported...)
	if err := a.SaveBrowserProxies(allProxies); err != nil {
		return nil, err
	}

	return map[string]interface{}{
		"url":           sourceURL,
		"importedCount": len(imported),
		"skippedCount":  skipped,
		"totalCount":    len(lines),
		"groupName":     groupName,
		"allProxies":    a.BrowserProxyList(),
	}, nil
}

func decodeSubscriptionProxyList(raw string) ([]string, error) {
	text := strings.TrimSpace(strings.ReplaceAll(raw, "\r\n", "\n"))
	if text == "" {
		return nil, fmt.Errorf("订阅内容为空")
	}

	if decoded, ok := decodeBase64Text(text); ok {
		text = decoded
	}

	lines := strings.Split(text, "\n")
	var result []string
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		if !isProxyProtocol(line) {
			continue
		}
		result = append(result, line)
	}
	return result, nil
}

func isProxyProtocol(line string) bool {
	lower := strings.ToLower(line)
	for _, prefix := range []string{
		"vmess://", "vless://", "trojan://", "ss://",
		"hysteria2://", "tuic://", "anytls://",
		"http://", "https://", "socks5://", "socks://",
	} {
		if strings.HasPrefix(lower, prefix) {
			return true
		}
	}
	return false
}

func extractProxyNameFromURL(rawURL string) string {
	// URL fragment: vmess://...#name
	if idx := strings.LastIndex(rawURL, "#"); idx >= 0 {
		fragment := rawURL[idx+1:]
		if decoded, err := url.QueryUnescape(fragment); err == nil {
			fragment = decoded
		}
		fragment = strings.TrimSpace(fragment)
		if fragment != "" {
			return fragment
		}
	}

	lower := strings.ToLower(rawURL)

	// vmess://base64json — 解码 JSON 提取 ps（服务器名称）
	if strings.HasPrefix(lower, "vmess://") {
		raw := strings.TrimPrefix(rawURL, "vmess://")
		raw = strings.TrimSpace(raw)
		if decodedStr, ok := decodeBase64Text(raw); ok {
			var v struct {
				Ps string `json:"ps"`
			}
			if jsonErr := json.Unmarshal([]byte(decodedStr), &v); jsonErr == nil && v.Ps != "" {
				return v.Ps
			}
		}
		short := raw
		if len(short) > 60 {
			short = short[:60] + "..."
		}
		return "vmess:" + short
	}

	// vless://uuid@host:port?... / trojan://pass@host:port?... / tuic://...
	if strings.HasPrefix(lower, "vless://") || strings.HasPrefix(lower, "trojan://") || strings.HasPrefix(lower, "tuic://") {
		rest := rawURL[strings.Index(rawURL, "://")+3:]
		if at := strings.LastIndex(rest, "@"); at >= 0 {
			hostport := rest[at+1:]
			hostport = strings.SplitN(hostport, "?", 2)[0]
			hostport = strings.SplitN(hostport, "#", 2)[0]
			if host := strings.TrimSpace(strings.SplitN(hostport, ":", 2)[0]); host != "" {
				return host
			}
		}
	}

	// 标准 URL (http/https/socks5/anytls/hysteria2 等): 取 host
	parsed, err := url.Parse(rawURL)
	if err == nil && parsed.Host != "" {
		return parsed.Host
	}

	// 最终回退
	return strings.TrimSpace(rawURL[:min(len(rawURL), 80)])
}

func suggestGroupFromURL(parsedURL *url.URL) string {
	host := parsedURL.Hostname()
	host = strings.TrimPrefix(host, "www.")
	if idx := strings.LastIndex(host, "."); idx > 0 {
		if idx2 := strings.LastIndex(host[:idx], "."); idx2 > 0 {
			host = host[idx2+1:]
		}
	}
	if host != "" {
		return host
	}
	return "订阅代理"
}

func normalizeClashSubscriptionContent(body []byte) (string, interface{}, error) {
	baseText := strings.TrimSpace(strings.ReplaceAll(string(body), "\r\n", "\n"))
	if baseText == "" {
		return "", nil, fmt.Errorf("订阅内容为空")
	}

	tryTexts := make([]string, 0, 4)
	tryTexts = append(tryTexts, baseText)

	if unescaped, err := url.QueryUnescape(baseText); err == nil {
		unescaped = strings.TrimSpace(strings.ReplaceAll(unescaped, "\r\n", "\n"))
		if unescaped != "" && unescaped != baseText {
			tryTexts = append(tryTexts, unescaped)
		}
	}

	if decoded, ok := decodeBase64Text(baseText); ok {
		tryTexts = append(tryTexts, decoded)
	}

	for _, text := range tryTexts {
		payload, ok := parseClashPayload(text)
		if !ok {
			continue
		}
		if clashProxyCount(payload) > 0 {
			return text, payload, nil
		}
	}

	return "", nil, fmt.Errorf("URL 内容不是有效 Clash YAML（需包含 proxies）")
}

func decodeBase64Text(raw string) (string, bool) {
	candidate := strings.TrimSpace(raw)
	if candidate == "" {
		return "", false
	}
	padded := candidate
	if mod := len(padded) % 4; mod != 0 {
		padded += strings.Repeat("=", 4-mod)
	}

	encoders := []*base64.Encoding{
		base64.StdEncoding,
		base64.RawStdEncoding,
		base64.URLEncoding,
		base64.RawURLEncoding,
	}
	for _, enc := range encoders {
		if data, err := enc.DecodeString(candidate); err == nil {
			decoded := strings.TrimSpace(strings.ReplaceAll(string(data), "\r\n", "\n"))
			if decoded != "" {
				return decoded, true
			}
		}
		if data, err := enc.DecodeString(padded); err == nil {
			decoded := strings.TrimSpace(strings.ReplaceAll(string(data), "\r\n", "\n"))
			if decoded != "" {
				return decoded, true
			}
		}
	}
	return "", false
}

func parseClashPayload(text string) (interface{}, bool) {
	var payload interface{}
	if err := yaml.Unmarshal([]byte(text), &payload); err != nil {
		return nil, false
	}
	return payload, true
}

func clashProxyCount(payload interface{}) int {
	if m := toStringMap(payload); m != nil {
		if arr, ok := m["proxies"].([]interface{}); ok {
			return len(arr)
		}
		if arr, ok := m["proxy"].([]interface{}); ok {
			return len(arr)
		}
		if arr, ok := m["Proxy"].([]interface{}); ok {
			return len(arr)
		}
	}
	if arr, ok := payload.([]interface{}); ok {
		return len(arr)
	}
	return 0
}

func extractClashDNSYAML(payload interface{}) string {
	m := toStringMap(payload)
	if m == nil {
		return ""
	}
	dnsRaw, exists := m["dns"]
	if !exists || dnsRaw == nil {
		return ""
	}
	data, err := yaml.Marshal(map[string]interface{}{
		"dns": dnsRaw,
	})
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}

func suggestClashGroupName(payload interface{}, fallbackHost string) string {
	fallbackHost = strings.TrimSpace(fallbackHost)
	m := toStringMap(payload)
	if m != nil {
		if groups, ok := m["proxy-groups"].([]interface{}); ok {
			for _, item := range groups {
				if groupMap := toStringMap(item); groupMap != nil {
					if name := strings.TrimSpace(getMapString(groupMap, "name")); name != "" {
						return name
					}
				}
			}
		}
	}
	if strings.HasPrefix(strings.ToLower(fallbackHost), "www.") {
		fallbackHost = fallbackHost[4:]
	}
	return fallbackHost
}

func toStringMap(value interface{}) map[string]interface{} {
	switch m := value.(type) {
	case map[string]interface{}:
		return m
	case map[interface{}]interface{}:
		out := make(map[string]interface{}, len(m))
		for k, v := range m {
			key := fmt.Sprint(k)
			out[key] = v
		}
		return out
	default:
		return nil
	}
}

func getMapString(m map[string]interface{}, key string) string {
	if m == nil {
		return ""
	}
	value, ok := m[key]
	if !ok || value == nil {
		return ""
	}
	switch v := value.(type) {
	case string:
		return v
	default:
		return fmt.Sprint(v)
	}
}

// BrowserProxyFixNames 重新从配置 URL 提取代理名称，修复存量的 base64 blob 名称。
func (a *App) BrowserProxyFixNames() map[string]interface{} {
	if a.browserMgr.ProxyDAO == nil {
		return map[string]interface{}{
			"ok":    false,
			"error": "ProxyDAO 未初始化",
		}
	}

	proxies, err := a.browserMgr.ProxyDAO.List()
	if err != nil {
		return map[string]interface{}{
			"ok":    false,
			"error": fmt.Sprintf("读取代理列表失败: %v", err),
		}
	}

	fixed := 0
	for i := range proxies {
		oldName := proxies[i].ProxyName
		if !looksLikeGeneratedProxyName(oldName) {
			continue
		}
		newName := extractProxyNameFromURL(proxies[i].ProxyConfig)
		if newName != oldName {
			proxies[i].ProxyName = newName
			fixed++
		}
	}

	if fixed == 0 {
		return map[string]interface{}{
			"ok":      true,
			"fixed":   0,
			"total":   len(proxies),
			"message": "所有代理名称均正常，无需修复",
		}
	}

	if err := a.SaveBrowserProxies(proxies); err != nil {
		return map[string]interface{}{
			"ok":    false,
			"error": fmt.Sprintf("保存修复后的代理失败: %v", err),
			"fixed": fixed,
		}
	}

	return map[string]interface{}{
		"ok":      true,
		"fixed":   fixed,
		"total":   len(proxies),
		"message": fmt.Sprintf("已修复 %d 个代理名称", fixed),
	}
}

func looksLikeGeneratedProxyName(name string) bool {
	trimmed := strings.TrimSpace(name)
	if trimmed == "" {
		return true
	}
	lower := strings.ToLower(trimmed)
	if strings.HasPrefix(trimmed, "SUB-") || strings.HasPrefix(lower, "vmess:") {
		return true
	}
	if strings.Contains(trimmed, "eyj") && len(trimmed) > 48 {
		return true
	}
	return len(trimmed) > 96
}
