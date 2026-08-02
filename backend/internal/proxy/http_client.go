package proxy

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"personal-pilot/backend/internal/config"
	"personal-pilot/backend/internal/transport"

	xproxy "golang.org/x/net/proxy"
)

const defaultProxyHTTPSCanaryURL = "https://www.cloudflare.com/cdn-cgi/trace"

// BuildHTTPClient builds an HTTP client for the given proxy config (exported for Graph/ops egress).
func BuildHTTPClient(
	src string,
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
	timeout time.Duration,
) (*http.Client, error) {
	return buildProxyHTTPClient(src, proxyId, proxies, managers, timeout)
}

// ContextDialerForProxy resolves a ContextDialer that exits via the same bridge/proxy as the browser.
// Returns (nil, nil) for direct:// so callers can use a local dialer.
func ContextDialerForProxy(
	src string,
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
) (xproxy.ContextDialer, error) {
	src = NormalizeStandardProxyScheme(src)
	l := strings.ToLower(strings.TrimSpace(src))
	if l == "" || l == "direct://" {
		return nil, nil
	}

	if m := findBridgeManager(src, managers); m != nil {
		socks5Addr, err := m.EnsureBridge(src, proxies, proxyId)
		if err != nil {
			return nil, fmt.Errorf("桥接启动失败: %w", err)
		}
		return socks5ContextDialer(bridgeSocksHostPort(socks5Addr))
	}

	if strings.HasPrefix(l, "socks5://") {
		u, err := url.Parse(src)
		if err != nil {
			return nil, fmt.Errorf("SOCKS5 地址解析失败: %s", RedactProxyURL(src))
		}
		var auth *xproxy.Auth
		if u.User != nil {
			pass, _ := u.User.Password()
			auth = &xproxy.Auth{
				User:     u.User.Username(),
				Password: pass,
			}
		}
		dialer, err := xproxy.SOCKS5("tcp", u.Host, auth, xproxy.Direct)
		if err != nil {
			return nil, fmt.Errorf("SOCKS5 dialer 创建失败: %w", err)
		}
		contextDialer, ok := dialer.(xproxy.ContextDialer)
		if !ok {
			return nil, fmt.Errorf("SOCKS5 dialer 不支持 ContextDialer")
		}
		return contextDialer, nil
	}

	// HTTP(S) forward proxies: DialContext goes direct; ProxyURL handles CONNECT.
	return nil, fmt.Errorf("http forward proxy requires BuildHTTPClient, not ContextDialerForProxy: %s", RedactProxyURL(src))
}

func socks5ContextDialer(socks5Host string) (xproxy.ContextDialer, error) {
	dialer, err := xproxy.SOCKS5("tcp", socks5Host, nil, xproxy.Direct)
	if err != nil {
		return nil, fmt.Errorf("SOCKS5 dialer 创建失败: %w", err)
	}
	contextDialer, ok := dialer.(xproxy.ContextDialer)
	if !ok {
		return nil, fmt.Errorf("SOCKS5 dialer 不支持 ContextDialer")
	}
	return contextDialer, nil
}

// buildProxyHTTPClient 根据代理配置构建 HTTP 客户端，统一用于测速/健康检测场景。
func buildProxyHTTPClient(
	src string,
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
	timeout time.Duration,
) (*http.Client, error) {
	src = NormalizeStandardProxyScheme(src)
	l := strings.ToLower(strings.TrimSpace(src))
	if l == "" || l == "direct://" {
		return &http.Client{Timeout: timeout}, nil
	}

	if m := findBridgeManager(src, managers); m != nil {
		socks5Addr, err := m.EnsureBridge(src, proxies, proxyId)
		if err != nil {
			return nil, fmt.Errorf("桥接启动失败: %w", err)
		}
		return buildSocks5HTTPClient(bridgeSocksHostPort(socks5Addr), timeout)
	}

	if strings.HasPrefix(l, "socks5://") {
		u, err := url.Parse(src)
		if err != nil {
			return nil, fmt.Errorf("SOCKS5 地址解析失败: %s", RedactProxyURL(src))
		}
		var auth *xproxy.Auth
		if u.User != nil {
			pass, _ := u.User.Password()
			auth = &xproxy.Auth{
				User:     u.User.Username(),
				Password: pass,
			}
		}
		dialer, err := xproxy.SOCKS5("tcp", u.Host, auth, xproxy.Direct)
		if err != nil {
			return nil, fmt.Errorf("SOCKS5 dialer 创建失败: %w", err)
		}
		contextDialer, ok := dialer.(xproxy.ContextDialer)
		if !ok {
			return nil, fmt.Errorf("SOCKS5 dialer 不支持 ContextDialer")
		}
		transport := &http.Transport{DialContext: contextDialer.DialContext}
		return &http.Client{Transport: transport, Timeout: timeout}, nil
	}

	proxyURL, err := url.Parse(src)
	if err != nil {
		return nil, fmt.Errorf("代理地址解析失败: %s", RedactProxyURL(src))
	}
	transport := &http.Transport{Proxy: http.ProxyURL(proxyURL)}
	return &http.Client{Transport: transport, Timeout: timeout}, nil
}

func CheckProxyHTTPSConnectivity(
	ctx context.Context,
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
	timeout time.Duration,
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

	client, err := buildProxyHTTPClient(src, proxyId, proxies, managers, timeout)
	if err != nil {
		return TestResult{ProxyId: proxyId, Ok: false, Error: err.Error()}
	}
	latency, statusCode, err := checkHTTPClientGET(ctx, client, defaultProxyHTTPSCanaryURL, transport.ProductUserAgent)
	if err != nil {
		return TestResult{ProxyId: proxyId, Ok: false, LatencyMs: latency, Error: err.Error()}
	}
	if !isUsableHTTPStatus(statusCode) {
		return TestResult{ProxyId: proxyId, Ok: false, LatencyMs: latency, Error: fmt.Sprintf("HTTPS HTTP %d", statusCode)}
	}
	return TestResult{ProxyId: proxyId, Ok: true, LatencyMs: latency}
}

// bridgeSocksHostPort normalizes bridge URLs like socks5h://127.0.0.1:1080 to host:port.
func bridgeSocksHostPort(raw string) string {
	raw = strings.TrimSpace(raw)
	lower := strings.ToLower(raw)
	for _, prefix := range []string{"socks5h://", "socks5://", "socks://"} {
		if strings.HasPrefix(lower, prefix) {
			return raw[len(prefix):]
		}
	}
	return raw
}

func buildSocks5HTTPClient(socks5Host string, timeout time.Duration) (*http.Client, error) {
	dialer, err := xproxy.SOCKS5("tcp", socks5Host, nil, xproxy.Direct)
	if err != nil {
		return nil, fmt.Errorf("SOCKS5 dialer 创建失败: %w", err)
	}
	contextDialer, ok := dialer.(xproxy.ContextDialer)
	if !ok {
		return nil, fmt.Errorf("SOCKS5 dialer 不支持 ContextDialer")
	}
	transport := &http.Transport{DialContext: contextDialer.DialContext}
	return &http.Client{Transport: transport, Timeout: timeout}, nil
}

func checkHTTPClientGET(ctx context.Context, client *http.Client, targetURL string, userAgent string) (int64, int, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, targetURL, nil)
	if err != nil {
		return 0, 0, err
	}
	req.Header.Set("Accept", "*/*")
	req.Header.Set("User-Agent", userAgent)

	start := time.Now()
	resp, err := client.Do(req)
	latency := time.Since(start).Milliseconds()
	if err != nil {
		return latency, 0, err
	}
	defer resp.Body.Close()
	_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 64*1024))
	return latency, resp.StatusCode, nil
}

func isUsableHTTPStatus(statusCode int) bool {
	return statusCode >= 200 && statusCode < 400
}
