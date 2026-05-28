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
