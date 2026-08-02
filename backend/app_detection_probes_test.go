package backend

import (
	"testing"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/proxy"
)

func TestCollectLiveDetectionSignalsDirectProxy(t *testing.T) {
	app := &App{}
	signals := app.collectLiveDetectionSignals("p1", &browser.FingerprintSnapshot{Webdriver: false, CanvasHash: "abc"})
	if signals.DNSConsistent {
		t.Fatal("direct profile should not claim dns consistent")
	}
	// Direct mode must not invent a WebRTC leak when live probe is clean.
	if !signals.WebrtcClean {
		t.Fatal("direct profile without proxy should keep webrtcClean=true by default")
	}
}

func TestProfileLaunchArgsContainMaterializedWebRTC(t *testing.T) {
	profile := &BrowserProfile{ProfileId: "p1", ProxyId: "px1"}
	if !profileLaunchArgsContain(profile, "--webrtc-ip-handling-policy=disable_non_proxied_udp") {
		t.Fatal("expected materialized webrtc hardening flag")
	}
}

func TestApplyDNSProbeResultKeepsSystemResolverObservationInconclusive(t *testing.T) {
	signals := LiveDetectionSignals{DNSConsistent: true}
	applyDNSProbeResult(&signals, proxy.LeakProbeResult{
		DNSStatus:      proxy.DNSProbeStatusInconclusive,
		DNSConsistent:  false,
		DNSLeakSuspect: false,
	})
	if signals.DNSObserved || signals.DNSConsistent || signals.DNSLeakSuspect {
		t.Fatalf("inconclusive DNS observation must stay neutral: %+v", signals)
	}
	if signals.DNSStatus != proxy.DNSProbeStatusInconclusive {
		t.Fatalf("dnsStatus=%q", signals.DNSStatus)
	}
}
