package proxy

import (
	"testing"

	"personal-pilot/backend/internal/transport"
)

func TestXrayTransportProfileKeepsRuntimeConfigStrict(t *testing.T) {
	profile := transport.ChromeMajorTLSBaseline(139)
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
	utls, ok := settings["utls"].(map[string]any)
	if !ok || utls["fingerprint_tag"] != "Chrome_133" {
		t.Fatalf("expected Chrome_133 utls tag, got %+v", settings["utls"])
	}
	if _, ok := settings["_personaPilotTransport"]; ok {
		t.Fatalf("runtime config must not contain internal metadata: %+v", settings)
	}
	if _, ok := settings["httpSettings"]; ok {
		t.Fatalf("runtime config must not invent unsupported httpSettings fields: %+v", settings)
	}
}

func TestSingBoxTransportProfileKeepsRuntimeConfigStrict(t *testing.T) {
	profile := transport.ChromeMajorTLSBaseline(139)
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
	utls, ok := tls["utls"].(map[string]any)
	if !ok || utls["fingerprint_tag"] != "Chrome_133" {
		t.Fatalf("expected Chrome_133 utls tag, got %+v", tls["utls"])
	}
	if _, ok := outbound["_personaPilotTransport"]; ok {
		t.Fatalf("runtime config must not contain internal metadata: %+v", outbound)
	}
}

func TestChromeMajorTLSBaselineChangesWithMajor(t *testing.T) {
	a := transport.ChromeMajorTLSBaseline(139)
	b := transport.ChromeMajorTLSBaseline(120)
	if a.TLS.JA3 == b.TLS.JA3 {
		t.Fatalf("major change should change TLS template: %q == %q", a.TLS.JA3, b.TLS.JA3)
	}
	if a.TLS.JA3 != "Chrome_133" || b.TLS.JA3 != "Chrome_120" {
		t.Fatalf("unexpected templates a=%q b=%q", a.TLS.JA3, b.TLS.JA3)
	}
}
