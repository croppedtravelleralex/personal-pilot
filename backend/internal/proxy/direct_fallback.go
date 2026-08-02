package proxy

import (
	"fmt"
	"strings"

	"personal-pilot/backend/internal/config"
)

const DirectProxyTagAllowFallback = "allow-direct-fallback"

// IsDirectProxy reports whether the proxy config represents an explicit direct connection.
func IsDirectProxy(config string) bool {
	normalized := strings.ToLower(strings.TrimSpace(NormalizeStandardProxyScheme(config)))
	switch normalized {
	case "", "direct://", "__direct__":
		return true
	default:
		return false
	}
}

// ResolveProxyConfig returns the effective proxy config for a profile + proxy catalog entry.
func ResolveProxyConfig(profileConfig string, proxies []config.BrowserProxy, proxyID string) string {
	resolved := strings.TrimSpace(profileConfig)
	if strings.TrimSpace(proxyID) == "" {
		return resolved
	}
	for _, item := range proxies {
		if strings.EqualFold(item.ProxyId, proxyID) {
			if cfg := strings.TrimSpace(item.ProxyConfig); cfg != "" {
				return cfg
			}
			break
		}
	}
	return resolved
}

// ProxyRequiresBridge reports whether the proxy config needs a local sing-box/xray bridge.
func ProxyRequiresBridge(config string, proxies []config.BrowserProxy, proxyID string) bool {
	resolved := ResolveProxyConfig(config, proxies, proxyID)
	if IsDirectProxy(resolved) {
		return false
	}
	return IsSingBoxProtocol(resolved) ||
		IsStandardAuthProxy(resolved) ||
		HasSSHTunnelDirective(resolved) ||
		RequiresBridge(resolved, proxies, proxyID)
}

// ValidateDirectFallbackSwitch rejects silent direct fallback from a bridged proxy.
func ValidateDirectFallbackSwitch(
	currentConfig string,
	proxies []config.BrowserProxy,
	currentProxyID string,
	newProxyID string,
	newProxyConfig string,
	allowFallback bool,
) error {
	if allowFallback {
		return nil
	}
	currentResolved := ResolveProxyConfig(currentConfig, proxies, currentProxyID)
	if !ProxyRequiresBridge(currentResolved, proxies, currentProxyID) {
		return nil
	}

	newResolved := strings.TrimSpace(newProxyConfig)
	if strings.TrimSpace(newProxyID) != "" {
		for _, item := range proxies {
			if strings.EqualFold(item.ProxyId, newProxyID) {
				if cfg := strings.TrimSpace(item.ProxyConfig); cfg != "" {
					newResolved = cfg
				}
				break
			}
		}
	}
	if strings.EqualFold(strings.TrimSpace(newProxyID), "__direct__") || IsDirectProxy(newResolved) {
		return fmt.Errorf("禁止静默回退直连：当前代理需要桥接，切换至 direct:// 需实例标签 %s", DirectProxyTagAllowFallback)
	}
	return nil
}
