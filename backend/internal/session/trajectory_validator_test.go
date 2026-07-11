package session

import "testing"

func TestValidateTrajectoryCrossReference(t *testing.T) {
	report := ValidateTrajectoryCrossReference([]TrajectoryPoint{
		{ProfileID: "p1", ProxyID: "a", FingerprintSeed: "1"},
		{ProfileID: "p1", ProxyID: "a", FingerprintSeed: "1", HasRecording: true},
	})
	if report.Score < 85 || report.Level != "strong" {
		t.Fatalf("%+v", report)
	}
	report = ValidateTrajectoryCrossReference([]TrajectoryPoint{
		{ProfileID: "p1", ProxyID: "a", FingerprintSeed: "1"},
		{ProfileID: "p1", ProxyID: "b", FingerprintSeed: "2"},
	})
	if len(report.Issues) == 0 {
		t.Fatal("expected issues")
	}
}
