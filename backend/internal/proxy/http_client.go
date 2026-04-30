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

	xproxy "golang.org/x/net/proxy"
)

const defaultProxyHTTPSCanaryURL = "https://www.cloudflare.com/cdn-cgi/trace"

// buildProxyHTTPClient 根据代理配置构建 HTTP 客户端，统一用于测速/健康检测场景。
func buildProxyHTTPClient(
	src string,
	proxyId string,
	proxies []config.BrowserProxy,
	xrayMgr *XrayManager,
	singboxMgr *SingBoxManager,
	timeout time.Duration,
) (*http.Client, error) {
	l := strings.ToLower(strings.TrimSpace(src))
	if l == "" || l == "direct://" {
		return &http.Client{Timeout: timeout}, nil
	}

	if IsSingBoxProtocol(src) {
		if singboxMgr == nil {
			return nil, fmt.Errorf("sing-box 管理器未初始化")
		}
		socks5Addr, err := singboxMgr.EnsureBridge(src, proxies, proxyId)
		if err != nil {
			return nil, fmt.Errorf("sing-box 桥接启动失败: %w", err)
		}
		return buildSocks5HTTPClient(strings.TrimPrefix(socks5Addr, "socks5://"), timeout)
	}

	if RequiresBridge(src, proxies, proxyId) {
		if xrayMgr == nil {
			return nil, fmt.Errorf("xray 管理器未初始化")
		}
		socks5Addr, err := xrayMgr.EnsureBridge(src, proxies, proxyId)
		if err != nil {
			return nil, fmt.Errorf("xray 桥接启动失败: %w", err)
		}
		return buildSocks5HTTPClient(strings.TrimPrefix(socks5Addr, "socks5://"), timeout)
	}

	if strings.HasPrefix(l, "socks5://") {
		u, err := url.Parse(src)
		if err != nil {
			return nil, fmt.Errorf("SOCKS5 地址解析失败: %w", err)
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
		return nil, fmt.Errorf("代理地址解析失败: %w", err)
	}
	transport := &http.Transport{Proxy: http.ProxyURL(proxyURL)}
	return &http.Client{Transport: transport, Timeout: timeout}, nil
}

func CheckProxyHTTPSConnectivity(
	proxyId string,
	proxies []config.BrowserProxy,
	xrayMgr *XrayManager,
	singboxMgr *SingBoxManager,
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

	client, err := buildProxyHTTPClient(src, proxyId, proxies, xrayMgr, singboxMgr, timeout)
	if err != nil {
		return TestResult{ProxyId: proxyId, Ok: false, Error: err.Error()}
	}
	latency, statusCode, err := checkHTTPClientGET(context.Background(), client, defaultProxyHTTPSCanaryURL, "PersonalPilot/1.0")
	if err != nil {
		return TestResult{ProxyId: proxyId, Ok: false, LatencyMs: latency, Error: err.Error()}
	}
	if !isUsableHTTPStatus(statusCode) {
		return TestResult{ProxyId: proxyId, Ok: false, LatencyMs: latency, Error: fmt.Sprintf("HTTPS HTTP %d", statusCode)}
	}
	return TestResult{ProxyId: proxyId, Ok: true, LatencyMs: latency}
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
