package backend

import (
	"testing"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/detection"
	"personal-pilot/backend/internal/proxy"
	"personal-pilot/backend/internal/session"
)

func TestUserRequirementsNetworkLeakSuite(t *testing.T) {
	probe := proxy.AnalyzeICECandidates("203.0.113.1", []string{"candidate:1 1 UDP 2130706431 203.0.113.1 12345 typ srflx"})
	if probe.LeakSuspect {
		t.Fatalf("unexpected leak: %+v", probe)
	}
	dns := proxy.ProbeDNSConsistency(t.Context(), "p1", "203.0.113.1", "example.com")
	if dns.ExitIP == "" {
		t.Fatal("expected exit ip in probe result")
	}
	streak := &proxy.VerifyV2Streak{RequiredPasses: 2}
	streak.Record(proxy.VerifyV2Sample{OK: true, ExitIP: "1.1.1.1"})
	streak.Record(proxy.VerifyV2Sample{OK: true, ExitIP: "1.1.1.1"})
	if passed, _, _ := streak.Evaluate(); !passed {
		t.Fatal("expected verify v2 pass")
	}
}

func TestUserRequirementsFingerprintBehaviorSuite(t *testing.T) {
	report := browser.FullRuntimeProjectionReport(&browser.Profile{
		ProfileId: "p1", BehaviorProfileID: "bp", HumanizeSeed: "s", ProxyId: "px",
	}, nil, nil)
	if report.AppliedCount != 80 {
		t.Fatalf("runtime applied=%d missing=%v", report.AppliedCount, report.MissingControls)
	}
	if len(behavior.ShippedPrimitives) < 30 {
		t.Fatalf("primitives=%d", len(behavior.ShippedPrimitives))
	}
	score := detection.EvaluateAutoScore(detection.AutoScoreInput{SiteID: "gate", WebdriverHidden: true, WebrtcClean: true, DNSConsistent: true, CanvasConsistent: true, TrustScore: 90})
	if !score.Passed {
		t.Fatalf("%+v", score)
	}
}

func TestUserRequirementsAccountAndCDPSuite(t *testing.T) {
	cadence := behavior.AnalyzeRecordingCadence([]behavior.RecordedEvent{{T: 0, Type: "move"}, {T: 200, Type: "scroll"}, {T: 500, Type: "down"}})
	if cadence.Score < 60 {
		t.Fatalf("cadence=%+v", cadence)
	}
	traj := session.ValidateTrajectoryCrossReference(session.BuildTrajectoryFromProfileMetadata("p1", "px", "seed", "bp", "2026-01-01T00:00:00Z", "2026-06-01T00:00:00Z", true))
	if traj.Score < 65 {
		t.Fatalf("trajectory=%+v", traj)
	}
	rec := behavior.AnalyzeRecording(&behavior.Recording{ID: "r1", Events: []behavior.RecordedEvent{{T: 0, Type: "move"}, {T: 120, Type: "down"}}, DurationMs: 120})
	if rec == nil || rec.Cadence.Score == 0 {
		t.Fatal("analyze report missing")
	}
}

func TestUserRequirementsMouseDefault(t *testing.T) {
	if !profileWantsMousePointer(nil, &browser.Settings{ShowMousePointerDefault: true}) {
		t.Fatal("expected default mouse pointer")
	}
}
