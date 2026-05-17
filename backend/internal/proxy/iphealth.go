package proxy

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"sort"
	"strings"
	"time"

	"personal-pilot/backend/internal/config"
)

const (
	defaultIPPureInfoURL      = "https://my.ippure.com/v1/info"
	defaultIPAPIInfoURL       = "http://ip-api.com/json/?fields=status,message,query,country,countryCode,regionName,city,as,isp,org,hosting,proxy"
	defaultProxyIPInfoTimeout = 10 * time.Second
	legacyIPPureInfoTimeout   = 20 * time.Second
)

type proxyIPInfoEndpoint struct {
	source string
	url    string
}

var proxyIPInfoEndpoints = []proxyIPInfoEndpoint{
	{source: "ippure", url: defaultIPPureInfoURL},
	{source: "ip-api", url: defaultIPAPIInfoURL},
}

func FetchIPPureInfo(
	ctx context.Context,
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
) (map[string]interface{}, error) {
	src, err := findProxyConfig(proxyId, proxies)
	if err != nil {
		return nil, err
	}
	client, err := buildIPPureHTTPClient(src, proxyId, proxies, managers, legacyIPPureInfoTimeout)
	if err != nil {
		return nil, err
	}
	return fetchProxyIPInfoEndpoint(client, defaultIPPureInfoURL, "ippure")
}

func FetchProxyIPInfo(
	ctx context.Context,
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
) (map[string]interface{}, error) {
	src, err := findProxyConfig(proxyId, proxies)
	if err != nil {
		return nil, err
	}
	client, err := buildIPPureHTTPClient(src, proxyId, proxies, managers, defaultProxyIPInfoTimeout)
	if err != nil {
		return nil, err
	}

	metadataErrors := map[string]string{}
	for _, endpoint := range proxyIPInfoEndpoints {
		result, err := fetchProxyIPInfoEndpoint(client, endpoint.url, endpoint.source)
		if err == nil {
			return result, nil
		}
		metadataErrors[endpoint.source] = err.Error()
	}

	latency, statusCode, canaryErr := checkHTTPClientGET(ctx, client, defaultProxyHTTPSCanaryURL, "PersonalPilot/1.0")
	canaryData := map[string]interface{}{
		"source":              "https-canary",
		"metadataErrors":      metadataErrors,
		"metadataError":       joinMetadataErrors(metadataErrors),
		"realCheckUrl":        defaultProxyHTTPSCanaryURL,
		"realCheckLatencyMs":  latency,
		"realCheckStatusCode": statusCode,
	}
	if canaryErr != nil {
		canaryData["realCheckOk"] = false
		canaryData["realCheckError"] = canaryErr.Error()
		return canaryData, fmt.Errorf("IP metadata failed (%s); HTTPS canary failed: %w", joinMetadataErrors(metadataErrors), canaryErr)
	}
	if !isUsableHTTPStatus(statusCode) {
		canaryData["realCheckOk"] = false
		err := fmt.Errorf("HTTPS canary HTTP %d", statusCode)
		canaryData["realCheckError"] = err.Error()
		return canaryData, fmt.Errorf("IP metadata failed (%s); %w", joinMetadataErrors(metadataErrors), err)
	}
	canaryData["realCheckOk"] = true
	return canaryData, nil
}

func buildIPPureHTTPClient(
	src string,
	proxyId string,
	proxies []config.BrowserProxy,
	managers []BridgeManager,
	timeout time.Duration,
) (*http.Client, error) {
	return buildProxyHTTPClient(src, proxyId, proxies, managers, timeout)
}

func findProxyConfig(proxyId string, proxies []config.BrowserProxy) (string, error) {
	for _, item := range proxies {
		if strings.EqualFold(item.ProxyId, proxyId) {
			if src := strings.TrimSpace(item.ProxyConfig); src != "" {
				return src, nil
			}
			break
		}
	}
	return "", fmt.Errorf("代理配置为空")
}

func fetchProxyIPInfoEndpoint(client *http.Client, endpointURL string, source string) (map[string]interface{}, error) {
	req, _ := http.NewRequest(http.MethodGet, endpointURL, nil)
	req.Header.Set("Accept", "application/json")
	req.Header.Set("User-Agent", "PersonalPilot/1.0")

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("%s request failed: %w", source, err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(io.LimitReader(resp.Body, 256*1024))
	if err != nil {
		return nil, fmt.Errorf("%s response read failed: %w", source, err)
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("%s HTTP %d: %s", source, resp.StatusCode, bodySnippet(body, 180))
	}

	var result map[string]interface{}
	if err := json.Unmarshal(body, &result); err != nil {
		return nil, fmt.Errorf("%s JSON parse failed: %w", source, err)
	}
	if source == "ip-api" {
		if status := mapStringValue(result, "status"); status != "" && !strings.EqualFold(status, "success") {
			message := firstNonEmpty(mapStringValue(result, "message"), status)
			return result, fmt.Errorf("ip-api status %s: %s", status, message)
		}
	}
	result["source"] = source
	return result, nil
}

func joinMetadataErrors(errorsBySource map[string]string) string {
	if len(errorsBySource) == 0 {
		return ""
	}
	sources := make([]string, 0, len(errorsBySource))
	for source := range errorsBySource {
		sources = append(sources, source)
	}
	sort.Strings(sources)
	parts := make([]string, 0, len(sources))
	for _, source := range sources {
		parts = append(parts, source+": "+errorsBySource[source])
	}
	return strings.Join(parts, "; ")
}

func bodySnippet(body []byte, max int) string {
	s := strings.TrimSpace(string(body))
	if len(s) <= max {
		return s
	}
	return s[:max] + "..."
}
