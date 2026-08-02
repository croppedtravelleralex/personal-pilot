package transport

import "testing"

func TestTransportArgs(t *testing.T) {
	args := ChromiumLaunchArgs(OutboundConfig{RouteTag: "socks5://127.0.0.1:1080", TLS: TLSProfile{ALPN: []string{"h2"}}})
	if len(args) != 2 {
		t.Fatalf("args = %+v", args)
	}
}

func TestOutboundConfig(t *testing.T) {
	config := BuildOutboundConfig(OutboundRoute{Backend: ProxyBackendXray, Tag: "proxy-a", Server: "127.0.0.1", Port: 1080, Protocol: "socks"})
	if config["dialer"] != "xray-outbound" {
		t.Fatalf("config = %+v", config)
	}
	if len(HeaderOrderTemplate(nil).Names) == 0 {
		t.Fatal("default header order missing")
	}
}

func TestRuntimeProfileMetadata(t *testing.T) {
	profile := RuntimeProfile(RuntimeFamilyCamoufox)
	metadata := Metadata(profile, RuntimeFamilyCamoufox)
	if metadata["runtimeFamily"] != "camoufox" {
		t.Fatalf("metadata = %+v", metadata)
	}
	if len(profile.TLS.ALPN) == 0 || len(profile.HTTP2.PseudoHeaderOrder) == 0 || len(profile.Headers.Names) == 0 {
		t.Fatalf("profile incomplete: %+v", profile)
	}
}
