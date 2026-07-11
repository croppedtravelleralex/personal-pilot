package backend

import "testing"

func TestBuildHealthDailyRow(t *testing.T) {
	row := buildHealthDailyRow("2026-07-11", "xhs", 2, 8, 0, 1, 1)
	if row.ChallengeRate <= 0 || row.Status != "ok" {
		t.Fatalf("%+v", row)
	}
	thin := buildHealthDailyRow("2026-07-11", "", 0, 0, 0, 0, 0)
	if thin.Status != "insufficient_data" {
		t.Fatalf("%+v", thin)
	}
}
