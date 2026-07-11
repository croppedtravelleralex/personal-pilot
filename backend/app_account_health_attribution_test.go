package backend

import "testing"

func TestBuildAttributionBucket(t *testing.T) {
	b := buildAttributionBucket("site", "xhs", 3, 7, 0)
	if b.ChallengeRate < 0.29 || b.ChallengeRate > 0.31 {
		t.Fatalf("rate=%v", b.ChallengeRate)
	}
	if b.Status != "ok" {
		t.Fatalf("status=%s", b.Status)
	}
	thin := buildAttributionBucket("hour", "03", 1, 0, 0)
	if thin.Status != "insufficient_data" {
		t.Fatalf("thin status=%s", thin.Status)
	}
}

func TestExtractJSONStringField(t *testing.T) {
	payload := `{"humanizeSeed":"abc","proxyId":"px-1"}`
	if got := extractJSONStringField(payload, "proxyId"); got != "px-1" {
		t.Fatalf("got %q", got)
	}
	if got := extractProxyIDFromChallengePayload(payload); got != "px-1" {
		t.Fatalf("proxy %q", got)
	}
}

func TestTopRiskBuckets(t *testing.T) {
	in := []ChallengeAttributionBucket{
		{Key: "a", Challenges: 1, ChallengeRate: 0.1},
		{Key: "b", Challenges: 5, ChallengeRate: 0.8},
		{Key: "c", Challenges: 0, ChallengeRate: 1},
	}
	top := topRiskBuckets(in, 1)
	if len(top) != 1 || top[0].Key != "b" {
		t.Fatalf("%+v", top)
	}
}
