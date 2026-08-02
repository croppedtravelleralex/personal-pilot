package transport

type TLSProfile struct {
	JA3          string
	ALPN         []string
	CipherSuites []string
}
type HTTP2Profile struct {
	FrameOrder        []string
	PseudoHeaderOrder []string
}
type HeaderOrder struct{ Names []string }

type OutboundConfig struct {
	TLS      TLSProfile
	HTTP2    HTTP2Profile
	Headers  HeaderOrder
	RouteTag string
}

type RuntimeFamily string

const (
	RuntimeFamilyChrome   RuntimeFamily = "chrome"
	RuntimeFamilyEdge     RuntimeFamily = "edge"
	RuntimeFamilyFirefox  RuntimeFamily = "firefox"
	RuntimeFamilyCamoufox RuntimeFamily = "camoufox"
)

func RuntimeProfile(family RuntimeFamily) OutboundConfig {
	switch family {
	case RuntimeFamilyFirefox, RuntimeFamilyCamoufox:
		return OutboundConfig{
			TLS:      TLSProfile{JA3: "firefox-stable", ALPN: []string{"h2", "http/1.1"}, CipherSuites: []string{"TLS_AES_128_GCM_SHA256", "TLS_CHACHA20_POLY1305_SHA256", "TLS_AES_256_GCM_SHA384"}},
			HTTP2:    HTTP2Profile{FrameOrder: []string{"SETTINGS", "WINDOW_UPDATE", "HEADERS", "DATA"}, PseudoHeaderOrder: []string{":method", ":scheme", ":authority", ":path"}},
			Headers:  HeaderOrderTemplate([]string{"host", "user-agent", "accept", "accept-language", "accept-encoding", "connection"}),
			RouteTag: "proxy-out",
		}
	case RuntimeFamilyEdge:
		return OutboundConfig{
			TLS:      TLSProfile{JA3: "edge-stable", ALPN: []string{"h2", "http/1.1"}, CipherSuites: []string{"TLS_AES_128_GCM_SHA256", "TLS_AES_256_GCM_SHA384", "TLS_CHACHA20_POLY1305_SHA256"}},
			HTTP2:    HTTP2Profile{FrameOrder: []string{"SETTINGS", "HEADERS", "WINDOW_UPDATE", "DATA"}, PseudoHeaderOrder: []string{":method", ":authority", ":scheme", ":path"}},
			Headers:  HeaderOrderTemplate(nil),
			RouteTag: "proxy-out",
		}
	default:
		return OutboundConfig{
			TLS:      TLSProfile{JA3: "chrome-stable", ALPN: []string{"h2", "http/1.1"}, CipherSuites: []string{"TLS_AES_128_GCM_SHA256", "TLS_AES_256_GCM_SHA384", "TLS_CHACHA20_POLY1305_SHA256"}},
			HTTP2:    HTTP2Profile{FrameOrder: []string{"SETTINGS", "WINDOW_UPDATE", "HEADERS", "DATA"}, PseudoHeaderOrder: []string{":method", ":authority", ":scheme", ":path"}},
			Headers:  HeaderOrderTemplate(nil),
			RouteTag: "proxy-out",
		}
	}
}

func ChromiumLaunchArgs(config OutboundConfig) []string {
	args := []string{}
	if config.RouteTag != "" {
		args = append(args, "--proxy-server="+config.RouteTag)
	}
	if len(config.TLS.ALPN) > 0 {
		args = append(args, "--enable-features=UseDnsHttpsSvcbAlpn")
	}
	return args
}
