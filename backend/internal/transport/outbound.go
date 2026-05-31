package transport

import "strings"

type ProxyBackend string

const (
	ProxyBackendXray    ProxyBackend = "xray"
	ProxyBackendSingBox ProxyBackend = "sing-box"
)

type OutboundRoute struct {
	Backend  ProxyBackend
	Tag      string
	Server   string
	Port     int
	Protocol string
}

func BuildOutboundConfig(route OutboundRoute) map[string]any {
	config := map[string]any{"tag": route.Tag, "server": route.Server, "port": route.Port, "protocol": route.Protocol}
	switch route.Backend {
	case ProxyBackendXray:
		config["dialer"] = "xray-outbound"
	case ProxyBackendSingBox:
		config["dialer"] = "sing-box-outbound"
	default:
		config["dialer"] = "unknown"
	}
	return config
}

func HeaderOrderTemplate(names []string) HeaderOrder {
	if len(names) == 0 {
		names = []string{"host", "connection", "sec-ch-ua", "user-agent", "accept", "accept-language", "accept-encoding"}
	}
	for i := range names {
		names[i] = strings.ToLower(names[i])
	}
	return HeaderOrder{Names: names}
}

func Metadata(config OutboundConfig, family RuntimeFamily) map[string]any {
	// Metadata is for reports and explainability contracts only. Do not write it
	// into Xray or sing-box runtime configs because both tools may reject unknown
	// outbound fields.
	return map[string]any{
		"runtimeFamily":      string(family),
		"tlsProfile":         config.TLS.JA3,
		"alpn":               append([]string{}, config.TLS.ALPN...),
		"cipherSuites":       append([]string{}, config.TLS.CipherSuites...),
		"http2FrameOrder":    append([]string{}, config.HTTP2.FrameOrder...),
		"pseudoHeaderOrder":  append([]string{}, config.HTTP2.PseudoHeaderOrder...),
		"requestHeaderOrder": append([]string{}, config.Headers.Names...),
	}
}
