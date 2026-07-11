package proxy

import (
	"testing"
	"time"

	"personal-pilot/backend/internal/config"
)

func TestReconcileProfileRoutingHealthFail(t *testing.T) {
	result := ReconcileProfileRouting(ReconcileInput{
		Binding:  RoutingBinding{ProfileID: "p1", ProxyID: "a"},
		HealthOK: false,
	}, []config.BrowserProxy{{ProxyId: "b", LastTestOk: true, LastLatencyMs: 100}})
	if !result.SwitchRequired || result.NewProxyID != "b" {
		t.Fatalf("expected switch to b, got %+v", result)
	}
}

func TestStickySessionTrackerDrift(t *testing.T) {
	tracker := NewStickySessionTracker()
	tracker.Bind("p1", "proxy-a", "1.1.1.1", time.Hour)
	ok, reason := tracker.Validate("p1", "proxy-a", "2.2.2.2")
	if ok || reason != "sticky_exit_ip_drift" {
		t.Fatalf("got ok=%v reason=%q", ok, reason)
	}
}

func TestVerifyV2StreakPass(t *testing.T) {
	streak := VerifyV2Streak{RequiredPasses: 3}
	for i := 0; i < 3; i++ {
		streak.Record(VerifyV2Sample{OK: true, ExitIP: "8.8.8.8"})
	}
	passed, count, ip := streak.Evaluate()
	if !passed || count < 3 || ip != "8.8.8.8" {
		t.Fatalf("unexpected evaluate: passed=%v count=%d ip=%q", passed, count, ip)
	}
}
