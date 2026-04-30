package proxy

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"time"

	xproxy "golang.org/x/net/proxy"
)

const (
	defaultFreeProxyLimit       = 200
	maxFreeProxySourceBytes     = 4 * 1024 * 1024
	defaultFreeProxyCheckURL    = "http://ip-api.com/json/?fields=status,message,query,country,countryCode,regionName,city,as,isp,org,hosting,proxy"
	defaultFreeProxyUserAgent   = "PersonalPilot-FreeProxyCheck/1.0"
	defaultFreeProxyHTTPTimeout = 8 * time.Second
)

var defaultFreeProxySourceURLs = []string{
	"https://raw.githubusercontent.com/TheSpeedX/PROXY-List/master/http.txt",
	"https://raw.githubusercontent.com/TheSpeedX/PROXY-List/master/socks5.txt",
	"https://raw.githubusercontent.com/monosans/proxy-list/main/proxies/http.txt",
	"https://raw.githubusercontent.com/monosans/proxy-list/main/proxies/socks5.txt",
}

type FreeProxyCandidate struct {
	Raw         string `json:"raw"`
	SourceURL   string `json:"sourceUrl"`
	ProxyConfig string `json:"proxyConfig"`
	Protocol    string `json:"protocol"`
	Host        string `json:"host"`
	Port        int    `json:"port"`
	Endpoint    string `json:"endpoint"`
}

type FreeProxyCheckResult struct {
	Candidate      FreeProxyCandidate     `json:"candidate"`
	Ok             bool                   `json:"ok"`
	Error          string                 `json:"error"`
	LatencyMs      int64                  `json:"latencyMs"`
	IP             string                 `json:"ip"`
	Country        string                 `json:"country"`
	Region         string                 `json:"region"`
	City           string                 `json:"city"`
	AsOrganization string                 `json:"asOrganization"`
	Hosting        bool                   `json:"hosting"`
	ProxyFlag      bool                   `json:"proxyFlag"`
	RawData        map[string]interface{} `json:"rawData"`
	CheckedAt      string                 `json:"checkedAt"`
}

func DefaultFreeProxySourceURLs() []string {
	return append([]string{}, defaultFreeProxySourceURLs...)
}

func NormalizeFreeProxyLimit(limit int) int {
	if limit <= 0 {
		return defaultFreeProxyLimit
	}
	if limit > 5000 {
		return 5000
	}
	return limit
}

func NormalizeFreeProxyConcurrency(concurrency int) int {
	if concurrency <= 0 {
		return 8
	}
	if concurrency > 50 {
		return 50
	}
	return concurrency
}

func NormalizeFreeProxyTimeout(timeout time.Duration) time.Duration {
	if timeout <= 0 {
		return defaultFreeProxyHTTPTimeout
	}
	if timeout < time.Second {
		return time.Second
	}
	if timeout > 30*time.Second {
		return 30 * time.Second
	}
	return timeout
}

func FetchFreeProxyCandidates(ctx context.Context, sourceURLs []string, limit int, timeout time.Duration) ([]FreeProxyCandidate, []string) {
	sourceURLs = normalizeFreeProxySourceURLs(sourceURLs)
	limit = NormalizeFreeProxyLimit(limit)
	timeout = NormalizeFreeProxyTimeout(timeout)
	client := &http.Client{Timeout: timeout}

	var candidates []FreeProxyCandidate
	var sourceErrors []string
	for _, sourceURL := range sourceURLs {
		body, err := fetchFreeProxySource(ctx, client, sourceURL)
		if err != nil {
			sourceErrors = append(sourceErrors, fmt.Sprintf("%s: %v", sourceURL, err))
			continue
		}
		parsed := ParseFreeProxyCandidates(string(body), sourceURL)
		candidates = append(candidates, parsed...)
	}

	candidates = DeduplicateFreeProxyCandidates(candidates)
	if len(candidates) > limit {
		candidates = candidates[:limit]
	}
	return candidates, sourceErrors
}

func ParseFreeProxyCandidates(raw string, sourceURL string) []FreeProxyCandidate {
	defaultScheme := defaultSchemeForFreeProxySource(sourceURL)
	lines := strings.Split(raw, "\n")
	candidates := make([]FreeProxyCandidate, 0, len(lines))
	for _, line := range lines {
		for _, token := range freeProxyLineTokens(line) {
			if candidate, ok := parseFreeProxyToken(token, defaultScheme, sourceURL); ok {
				candidates = append(candidates, candidate)
			}
		}
	}
	return candidates
}

func DeduplicateFreeProxyCandidates(candidates []FreeProxyCandidate) []FreeProxyCandidate {
	seen := make(map[string]struct{}, len(candidates))
	unique := make([]FreeProxyCandidate, 0, len(candidates))
	for _, candidate := range candidates {
		key := strings.ToLower(strings.TrimSpace(candidate.ProxyConfig))
		if key == "" {
			continue
		}
		if _, exists := seen[key]; exists {
			continue
		}
		seen[key] = struct{}{}
		unique = append(unique, candidate)
	}
	return unique
}

func CheckFreeDirectProxies(ctx context.Context, candidates []FreeProxyCandidate, concurrency int, timeout time.Duration) []FreeProxyCheckResult {
	if len(candidates) == 0 {
		return []FreeProxyCheckResult{}
	}
	concurrency = NormalizeFreeProxyConcurrency(concurrency)
	if concurrency > len(candidates) {
		concurrency = len(candidates)
	}
	timeout = NormalizeFreeProxyTimeout(timeout)

	results := make([]FreeProxyCheckResult, len(candidates))
	type job struct {
		idx       int
		candidate FreeProxyCandidate
	}
	jobs := make(chan job, len(candidates))
	var wg sync.WaitGroup

	for worker := 0; worker < concurrency; worker++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for item := range jobs {
				results[item.idx] = CheckFreeDirectProxy(ctx, item.candidate, timeout)
			}
		}()
	}

	for idx, candidate := range candidates {
		select {
		case <-ctx.Done():
			results[idx] = FreeProxyCheckResult{
				Candidate: candidate,
				Ok:        false,
				Error:     ctx.Err().Error(),
				CheckedAt: time.Now().Format(time.RFC3339),
			}
		case jobs <- job{idx: idx, candidate: candidate}:
		}
	}
	close(jobs)
	wg.Wait()
	return results
}

func CheckFreeDirectProxy(ctx context.Context, candidate FreeProxyCandidate, timeout time.Duration) FreeProxyCheckResult {
	timeout = NormalizeFreeProxyTimeout(timeout)
	checkedAt := time.Now().Format(time.RFC3339)
	client, err := buildFreeDirectHTTPClient(candidate, timeout)
	if err != nil {
		return FreeProxyCheckResult{Candidate: candidate, Ok: false, Error: err.Error(), CheckedAt: checkedAt}
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, defaultFreeProxyCheckURL, nil)
	if err != nil {
		return FreeProxyCheckResult{Candidate: candidate, Ok: false, Error: err.Error(), CheckedAt: checkedAt}
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("User-Agent", defaultFreeProxyUserAgent)

	start := time.Now()
	resp, err := client.Do(req)
	latency := time.Since(start).Milliseconds()
	if err != nil {
		return FreeProxyCheckResult{Candidate: candidate, Ok: false, Error: err.Error(), LatencyMs: latency, CheckedAt: checkedAt}
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(io.LimitReader(resp.Body, 256*1024))
	if err != nil {
		return FreeProxyCheckResult{Candidate: candidate, Ok: false, Error: err.Error(), LatencyMs: latency, CheckedAt: checkedAt}
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return FreeProxyCheckResult{
			Candidate: candidate,
			Ok:        false,
			Error:     fmt.Sprintf("ip-api HTTP %d: %s", resp.StatusCode, bodySnippet(body, 160)),
			LatencyMs: latency,
			CheckedAt: checkedAt,
		}
	}

	var payload map[string]interface{}
	if err := json.Unmarshal(body, &payload); err != nil {
		return FreeProxyCheckResult{Candidate: candidate, Ok: false, Error: fmt.Sprintf("ip-api JSON parse failed: %v", err), LatencyMs: latency, CheckedAt: checkedAt}
	}
	if status := mapStringValue(payload, "status"); status != "" && !strings.EqualFold(status, "success") {
		message := mapStringValue(payload, "message")
		if message == "" {
			message = status
		}
		return FreeProxyCheckResult{Candidate: candidate, Ok: false, Error: message, LatencyMs: latency, RawData: enrichFreeProxyRawData(candidate, payload), CheckedAt: checkedAt}
	}

	rawData := enrichFreeProxyRawData(candidate, payload)
	realLatency, realStatusCode, realErr := checkHTTPClientGET(ctx, client, defaultProxyHTTPSCanaryURL, defaultFreeProxyUserAgent)
	rawData["realCheckUrl"] = defaultProxyHTTPSCanaryURL
	rawData["realCheckLatencyMs"] = realLatency
	rawData["realCheckStatusCode"] = realStatusCode
	if realErr != nil {
		rawData["realCheckOk"] = false
		rawData["realCheckError"] = realErr.Error()
		return FreeProxyCheckResult{
			Candidate: candidate,
			Ok:        false,
			Error:     fmt.Sprintf("真实 HTTPS 可用性检测失败: %v", realErr),
			LatencyMs: latency,
			RawData:   rawData,
			CheckedAt: checkedAt,
		}
	}
	if !isUsableHTTPStatus(realStatusCode) {
		rawData["realCheckOk"] = false
		return FreeProxyCheckResult{
			Candidate: candidate,
			Ok:        false,
			Error:     fmt.Sprintf("真实 HTTPS 可用性检测失败: HTTP %d", realStatusCode),
			LatencyMs: latency,
			RawData:   rawData,
			CheckedAt: checkedAt,
		}
	}
	rawData["realCheckOk"] = true
	return FreeProxyCheckResult{
		Candidate:      candidate,
		Ok:             true,
		Error:          "",
		LatencyMs:      latency,
		IP:             mapStringValue(payload, "query"),
		Country:        firstNonEmpty(mapStringValue(payload, "countryCode"), mapStringValue(payload, "country")),
		Region:         mapStringValue(payload, "regionName"),
		City:           mapStringValue(payload, "city"),
		AsOrganization: firstNonEmpty(mapStringValue(payload, "as"), mapStringValue(payload, "isp"), mapStringValue(payload, "org")),
		Hosting:        mapBoolValue(payload, "hosting"),
		ProxyFlag:      mapBoolValue(payload, "proxy"),
		RawData:        rawData,
		CheckedAt:      checkedAt,
	}
}

func normalizeFreeProxySourceURLs(sourceURLs []string) []string {
	cleaned := make([]string, 0, len(sourceURLs))
	for _, sourceURL := range sourceURLs {
		sourceURL = strings.TrimSpace(sourceURL)
		if sourceURL == "" {
			continue
		}
		cleaned = append(cleaned, sourceURL)
	}
	if len(cleaned) == 0 {
		return DefaultFreeProxySourceURLs()
	}
	return cleaned
}

func fetchFreeProxySource(ctx context.Context, client *http.Client, sourceURL string) ([]byte, error) {
	parsed, err := url.Parse(sourceURL)
	if err != nil {
		return nil, fmt.Errorf("invalid source URL: %w", err)
	}
	if parsed.Scheme != "http" && parsed.Scheme != "https" {
		return nil, fmt.Errorf("unsupported source URL scheme: %s", parsed.Scheme)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, sourceURL, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Accept", "text/plain,application/json,*/*")
	req.Header.Set("User-Agent", defaultFreeProxyUserAgent)

	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, maxFreeProxySourceBytes+1))
	if err != nil {
		return nil, err
	}
	if len(body) > maxFreeProxySourceBytes {
		return nil, fmt.Errorf("source response exceeds %d bytes", maxFreeProxySourceBytes)
	}
	return body, nil
}

func freeProxyLineTokens(line string) []string {
	if idx := strings.Index(line, "#"); idx >= 0 {
		line = line[:idx]
	}
	line = strings.TrimSpace(line)
	if line == "" {
		return nil
	}
	return strings.FieldsFunc(line, func(r rune) bool {
		return r == ' ' || r == '\t' || r == '\r' || r == '\n' ||
			r == ',' || r == ';' || r == '"' || r == '\'' || r == '`' ||
			r == '<' || r == '>' || r == '(' || r == ')'
	})
}

func parseFreeProxyToken(token string, defaultScheme string, sourceURL string) (FreeProxyCandidate, bool) {
	token = strings.TrimSpace(strings.Trim(token, "{}"))
	token = strings.TrimRight(token, ".,;")
	if token == "" {
		return FreeProxyCandidate{}, false
	}

	scheme := defaultScheme
	rawForURL := token
	if strings.Contains(token, "://") {
		parsed, err := url.Parse(token)
		if err != nil || parsed.Host == "" {
			return FreeProxyCandidate{}, false
		}
		scheme = normalizeFreeProxyScheme(parsed.Scheme)
		if scheme == "" {
			return FreeProxyCandidate{}, false
		}
		if parsed.Port() == "" {
			return FreeProxyCandidate{}, false
		}
		rawForURL = parsed.String()
	} else {
		scheme = normalizeFreeProxyScheme(scheme)
		if scheme == "" {
			scheme = "http"
		}
		rawForURL = fmt.Sprintf("%s://%s", scheme, token)
	}

	parsed, err := url.Parse(rawForURL)
	if err != nil || parsed.Host == "" || parsed.Port() == "" {
		return FreeProxyCandidate{}, false
	}
	port, err := strconv.Atoi(parsed.Port())
	if err != nil || port < 1 || port > 65535 {
		return FreeProxyCandidate{}, false
	}
	host := strings.Trim(parsed.Hostname(), "[]")
	if host == "" {
		return FreeProxyCandidate{}, false
	}
	endpoint := net.JoinHostPort(host, strconv.Itoa(port))
	proxyConfig := fmt.Sprintf("%s://%s", scheme, endpointForProxyURL(host, port))
	if parsed.User != nil {
		proxyConfig = fmt.Sprintf("%s://%s@%s", scheme, parsed.User.String(), endpointForProxyURL(host, port))
	}
	return FreeProxyCandidate{
		Raw:         token,
		SourceURL:   sourceURL,
		ProxyConfig: proxyConfig,
		Protocol:    scheme,
		Host:        host,
		Port:        port,
		Endpoint:    endpoint,
	}, true
}

func buildFreeDirectHTTPClient(candidate FreeProxyCandidate, timeout time.Duration) (*http.Client, error) {
	switch candidate.Protocol {
	case "http", "https":
		proxyURL, err := url.Parse(candidate.ProxyConfig)
		if err != nil {
			return nil, fmt.Errorf("proxy URL parse failed: %w", err)
		}
		return &http.Client{
			Transport: &http.Transport{Proxy: http.ProxyURL(proxyURL)},
			Timeout:   timeout,
		}, nil
	case "socks5":
		proxyURL, err := url.Parse(candidate.ProxyConfig)
		if err != nil {
			return nil, fmt.Errorf("SOCKS5 URL parse failed: %w", err)
		}
		var auth *xproxy.Auth
		if proxyURL.User != nil {
			password, _ := proxyURL.User.Password()
			auth = &xproxy.Auth{User: proxyURL.User.Username(), Password: password}
		}
		dialer, err := xproxy.SOCKS5("tcp", proxyURL.Host, auth, xproxy.Direct)
		if err != nil {
			return nil, fmt.Errorf("SOCKS5 dialer failed: %w", err)
		}
		contextDialer, ok := dialer.(xproxy.ContextDialer)
		if !ok {
			return nil, fmt.Errorf("SOCKS5 dialer does not support context")
		}
		return &http.Client{
			Transport: &http.Transport{DialContext: contextDialer.DialContext},
			Timeout:   timeout,
		}, nil
	default:
		return nil, fmt.Errorf("unsupported proxy protocol: %s", candidate.Protocol)
	}
}

func defaultSchemeForFreeProxySource(sourceURL string) string {
	lower := strings.ToLower(sourceURL)
	if strings.Contains(lower, "socks5") || strings.Contains(lower, "socks-5") {
		return "socks5"
	}
	if strings.Contains(lower, "https") {
		return "http"
	}
	return "http"
}

func normalizeFreeProxyScheme(scheme string) string {
	switch strings.ToLower(strings.TrimSpace(scheme)) {
	case "http", "https", "socks5":
		return strings.ToLower(strings.TrimSpace(scheme))
	case "socks":
		return "socks5"
	default:
		return ""
	}
}

func endpointForProxyURL(host string, port int) string {
	if strings.Contains(host, ":") && !strings.HasPrefix(host, "[") {
		host = "[" + host + "]"
	}
	return fmt.Sprintf("%s:%d", host, port)
}

func enrichFreeProxyRawData(candidate FreeProxyCandidate, payload map[string]interface{}) map[string]interface{} {
	rawData := make(map[string]interface{}, len(payload)+8)
	for key, value := range payload {
		rawData[key] = value
	}
	rawData["endpoint"] = candidate.Endpoint
	rawData["sourceUrl"] = candidate.SourceURL
	rawData["protocol"] = candidate.Protocol
	rawData["host"] = candidate.Host
	rawData["port"] = candidate.Port
	rawData["proxyConfig"] = candidate.ProxyConfig
	return rawData
}

func mapStringValue(m map[string]interface{}, key string) string {
	value, ok := m[key]
	if !ok || value == nil {
		return ""
	}
	switch typed := value.(type) {
	case string:
		return strings.TrimSpace(typed)
	default:
		return strings.TrimSpace(fmt.Sprint(typed))
	}
}

func mapBoolValue(m map[string]interface{}, key string) bool {
	value, ok := m[key]
	if !ok || value == nil {
		return false
	}
	switch typed := value.(type) {
	case bool:
		return typed
	case string:
		return strings.EqualFold(typed, "true") || typed == "1"
	case int:
		return typed != 0
	case int64:
		return typed != 0
	case float64:
		return typed != 0
	default:
		return false
	}
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}
