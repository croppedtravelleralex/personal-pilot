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
