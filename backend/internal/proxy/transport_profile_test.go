package proxy

import (
	"testing"

	"personal-pilot/backend/internal/transport"
)

func TestXrayTransportProfileKeepsRuntimeConfigStrict(t *testing.T) {
	profile := transport.RuntimeProfile(transport.RuntimeFamilyChrome)
	settings := mergeXrayStreamSettings(map[string]interface{}{
		"security":    "tls",
		"tlsSettings": map[string]interface{}{"serverName": "example.com"},
	}, profile)

	tlsSettings, ok := settings["tlsSettings"].(map[string]interface{})
	if !ok {
		t.Fatalf("tlsSettings missing: %+v", settings)
	}
	if got := tlsSettings["alpn"]; got == nil {
		t.Fatalf("expected ALPN to be applied, got %+v", tlsSettings)
	}
	if _, ok := settings["_personaPilotTransport"]; ok {
		t.Fatalf("runtime config must not contain internal metadata: %+v", settings)
	}
	if _, ok := settings["httpSettings"]; ok {
		t.Fatalf("runtime config must not invent unsupported httpSettings fields: %+v", settings)
	}
}

func TestSingBoxTransportProfileKeepsRuntimeConfigStrict(t *testing.T) {
	profile := transport.RuntimeProfile(transport.RuntimeFamilyChrome)
	outbound := applySingBoxTransportProfile(map[string]interface{}{
		"type": "hysteria2",
		"tls":  map[string]interface{}{"enabled": true},
	}, profile)

	tls, ok := outbound["tls"].(map[string]interface{})
	if !ok {
		t.Fatalf("tls missing: %+v", outbound)
	}
	if got := tls["alpn"]; got == nil {
		t.Fatalf("expected ALPN to be applied, got %+v", tls)
	}
	if _, ok := outbound["_personaPilotTransport"]; ok {
		t.Fatalf("runtime config must not contain internal metadata: %+v", outbound)
	}
}
