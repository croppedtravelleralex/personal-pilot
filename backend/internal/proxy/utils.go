package proxy

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
	"personal-pilot/backend/internal/config"
)

// TestResult 代理测试结果
type TestResult struct {
	ProxyId   string
	Ok        bool
	LatencyMs int64
	Error     string
}

// proxyEndpoint 从代理配置中提取 server:port，用于 TCP ping
func proxyEndpoint(src string) (string, error) {
	src = NormalizeStandardProxyScheme(src)
	l := strings.ToLower(src)

	// 标准 URL 格式: socks5://host:port, http://host:port
	if strings.HasPrefix(l, "socks5://") || strings.HasPrefix(l, "http://") || strings.HasPrefix(l, "https://") {
		u, err := url.Parse(src)
		if err != nil {
			return "", err
		}
		if strings.TrimSpace(u.Hostname()) == "" || strings.TrimSpace(u.Port()) == "" {
			return "", fmt.Errorf("标准代理地址缺少 host 或 port")
		}
		return net.JoinHostPort(u.Hostname(), u.Port()), nil
	}

	// vmess:// URL (base64 encoded JSON)
	if strings.HasPrefix(l, "vmess://") {
		raw := strings.TrimPrefix(src, "vmess://")
		decoded, err := decodeBase64String(strings.TrimSpace(raw))
		if err == nil {
			var v struct {
				Add  string      `json:"add"`
				Port interface{} `json:"port"`
			}
			if jsonErr := json.Unmarshal(decoded, &v); jsonErr == nil && v.Add != "" {
				return fmt.Sprintf("%s:%v", v.Add, v.Port), nil
			}
		}
	}

	// vless:// URL: vless://uuid@host:port?...
	if strings.HasPrefix(l, "vless://") {
		rest := src[len("vless://"):]
		if at := strings.LastIndex(rest, "@"); at >= 0 {
			hostport := strings.SplitN(rest[at+1:], "?", 2)[0]
			hostport = strings.SplitN(hostport, "#", 2)[0]
			return hostport, nil
		}
	}

	// Clash YAML 格式
	var payload interface{}
	if err := yaml.Unmarshal([]byte(src), &payload); err == nil {
		node := pickClashNode(payload)
		if node != nil {
			server := getMapString(node, "server")
			port := getMapInt(node, "port")
			if server != "" && port > 0 {
				return fmt.Sprintf("%s:%d", server, port), nil
			}
		}
	}

	return "", fmt.Errorf("无法解析代理地址")
}

// TestConnectivity 通过 TCP 握手测试代理服务器的可达性和延迟
// 直接对 server:port 建立 TCP 连接测量 RTT，无需启动外部进程
func TestConnectivity(proxyId string, proxyConfig string, proxies []config.BrowserProxy, _ interface{}) TestResult {
	src := strings.TrimSpace(proxyConfig)
	if proxyId != "" {
		for _, item := range proxies {
			if strings.EqualFold(item.ProxyId, proxyId) {
				src = strings.TrimSpace(item.ProxyConfig)
				break
			}
		}
	}
	if src == "" {
		return TestResult{ProxyId: proxyId, Ok: false, Error: "代理配置为空"}
	}

	endpoint, err := proxyEndpoint(src)
	if err != nil {
		return TestResult{ProxyId: proxyId, Ok: false, Error: fmt.Sprintf("地址解析失败: %v", err)}
	}

	start := time.Now()
	conn, err := net.DialTimeout("tcp", endpoint, 10*time.Second)
	latency := time.Since(start).Milliseconds()

	if err != nil {
		return TestResult{ProxyId: proxyId, Ok: false, LatencyMs: latency, Error: err.Error()}
	}
	conn.Close()
	return TestResult{ProxyId: proxyId, Ok: true, LatencyMs: latency}
}

func toStringMap(input interface{}) map[string]interface{} {
	switch v := input.(type) {
	case map[string]interface{}:
		return v
	case map[interface{}]interface{}:
		out := map[string]interface{}{}
		for k, val := range v {
			out[fmt.Sprint(k)] = val
		}
		return out
	}
	return nil
}

func getMapString(m map[string]interface{}, key string) string {
	v, ok := m[key]
	if !ok {
		return ""
	}
	switch s := v.(type) {
	case string:
		return strings.TrimSpace(s)
	case int:
		return strconv.Itoa(s)
	case int64:
		return strconv.FormatInt(s, 10)
	case float64:
		return strconv.Itoa(int(s))
	case bool:
		if s {
			return "true"
		}
		return "false"
	}
	return strings.TrimSpace(fmt.Sprint(v))
}

func getMapInt(m map[string]interface{}, key string) int {
	v, ok := m[key]
	if !ok {
		return 0
	}
	switch s := v.(type) {
	case int:
		return s
	case int64:
		return int(s)
	case float64:
		return int(s)
	case string:
		value, _ := strconv.Atoi(s)
		return value
	}
	return 0
}

func getMapBool(m map[string]interface{}, key string) bool {
	v, ok := m[key]
	if !ok {
		return false
	}
	switch s := v.(type) {
	case bool:
		return s
	case string:
		return strings.ToLower(s) == "true"
	case int:
		return s != 0
	case float64:
		return int(s) != 0
	}
	return false
}

func decodeBase64String(raw string) ([]byte, error) {
	if raw == "" {
		return nil, fmt.Errorf("base64 内容为空")
	}
	if data, err := base64.StdEncoding.DecodeString(raw); err == nil {
		return data, nil
	}
	if data, err := base64.RawStdEncoding.DecodeString(raw); err == nil {
		return data, nil
	}
	if data, err := base64.URLEncoding.DecodeString(raw); err == nil {
		return data, nil
	}
	if data, err := base64.RawURLEncoding.DecodeString(raw); err == nil {
		return data, nil
	}
	return nil, fmt.Errorf("base64 解析失败")
}

func mapStringValue(m map[string]interface{}, key string) string {
	return getMapString(m, key)
}

func mapBoolValue(m map[string]interface{}, key string) bool {
	return getMapBool(m, key)
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if strings.TrimSpace(v) != "" {
			return strings.TrimSpace(v)
		}
	}
	return ""
}

// isUnsupportedProtocol 判断是否为不支持的协议（hysteria/hysteria2）
func isUnsupportedProtocol(src string) bool {
	l := strings.ToLower(strings.TrimSpace(src))
	return strings.HasPrefix(l, "hysteria://") || strings.HasPrefix(l, "hysteria2://")
}

// TestRealConnectivity 通过代理链路发起真实 HTTP 请求测量端到端延迟。
func TestRealConnectivity(
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
) TestResult {
	return testRealConnectivity(proxyId, proxies, managers)
}

func testRealConnectivity(
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
) TestResult {
	src := ""
	for _, item := range proxies {
		if strings.EqualFold(item.ProxyId, proxyId) {
			src = strings.TrimSpace(item.ProxyConfig)
			break
		}
	}
	if src == "" {
		return TestResult{ProxyId: proxyId, Ok: false, Error: "代理配置为空"}
	}

	const targetURL = "http://www.gstatic.com/generate_204"
	const timeout = 15 * time.Second

	client, err := buildProxyHTTPClient(src, proxyId, proxies, managers, timeout)
	if err != nil {
		return TestResult{ProxyId: proxyId, Ok: false, Error: err.Error()}
	}

	start := time.Now()
	resp, err := client.Get(targetURL)
	latency := time.Since(start).Milliseconds()
	if err != nil {
		return TestResult{ProxyId: proxyId, Ok: false, LatencyMs: latency, Error: err.Error()}
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		return TestResult{ProxyId: proxyId, Ok: false, LatencyMs: latency, Error: fmt.Sprintf("HTTP %d", resp.StatusCode)}
	}
	return TestResult{ProxyId: proxyId, Ok: true, LatencyMs: latency}
}
