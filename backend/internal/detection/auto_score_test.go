package detection

import "testing"

func TestEvaluateAutoScorePass(t *testing.T) {
	score := EvaluateAutoScore(AutoScoreInput{
		SiteID: "creepjs", WebdriverHidden: true, WebrtcClean: true, DNSConsistent: true, CanvasConsistent: true, TrustScore: 90,
	})
	if !score.Passed || score.Score < 85 {
		t.Fatalf("%+v", score)
	}
}

func TestRemediationHints(t *testing.T) {
	hints := RemediationHints(SiteScore{Passed: false, Summary: "webrtc leak suspect"})
	if len(hints) == 0 {
		t.Fatal("expected hints")
	}
}

func TestEvaluateAutoScoreTreatsInconclusiveDNSAsNeutral(t *testing.T) {
	score := EvaluateAutoScore(AutoScoreInput{
		SiteID: "dns-inconclusive", WebdriverHidden: true, WebrtcClean: true,
		DNSObserved: false, DNSConsistent: false, CanvasConsistent: true, TrustScore: 90,
	})
	if !score.Passed || score.Score != 100 {
		t.Fatalf("inconclusive DNS should be neutral: %+v", score)
	}
}

func TestEvaluateAutoScorePenalizesObservedDNSMismatch(t *testing.T) {
	score := EvaluateAutoScore(AutoScoreInput{
		SiteID: "dns-mismatch", WebdriverHidden: true, WebrtcClean: true,
		DNSObserved: true, DNSConsistent: false, CanvasConsistent: true, TrustScore: 90,
	})
	if score.Score != 80 || score.Level != "normal" {
		t.Fatalf("observed DNS mismatch should be penalized: %+v", score)
	}
}
