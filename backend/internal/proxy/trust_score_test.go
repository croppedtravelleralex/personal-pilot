package proxy

import "testing"

func TestDefaultTrustScoreCalculator(t *testing.T) {
	c := DefaultTrustScoreCalculator()
	if c.LatencyWeight+c.IPHealthWeight+c.HistoryWeight+c.RegionMatchWeight < 0.99 {
		t.Fatal("Weights should sum to ~1.0")
	}
}

func TestComputeScore_Excellent(t *testing.T) {
	c := DefaultTrustScoreCalculator()
	// Low latency, clean IP, good history, region match
	result := c.ComputeScore(100, 0, false, 1.0, true)
	if result.Level != "excellent" {
		t.Fatalf("Level = %q, want 'excellent'. Score=%.1f", result.Level, result.OverallScore)
	}
	if result.OverallScore < 80 {
		t.Fatalf("OverallScore = %.1f, want >= 80", result.OverallScore)
	}
}

func TestComputeScore_Poor(t *testing.T) {
	c := DefaultTrustScoreCalculator()
	// High latency, high fraud, datacenter, bad history, no region match
	result := c.ComputeScore(6000, 80, true, 0.1, false)
	if result.Level != "poor" {
		t.Fatalf("Level = %q, want 'poor'. Score=%.1f", result.Level, result.OverallScore)
	}
	if result.OverallScore >= 40 {
		t.Fatalf("OverallScore = %.1f, want < 40", result.OverallScore)
	}
}

func TestComputeScore_Good(t *testing.T) {
	c := DefaultTrustScoreCalculator()
	result := c.ComputeScore(1500, 30, false, 0.7, false)
	if result.Level != "fair" && result.Level != "good" {
		t.Fatalf("Level = %q, want 'fair' or 'good'. Score=%.1f", result.Level, result.OverallScore)
	}
}

func TestComputeScore_LatencyBands(t *testing.T) {
	c := DefaultTrustScoreCalculator()
	// Excellent latency
	r1 := c.ComputeScore(100, 0, false, 1.0, true)
	// Poor latency
	r2 := c.ComputeScore(6000, 0, false, 1.0, true)
	if r1.OverallScore <= r2.OverallScore {
		t.Fatalf("Good latency score (%.1f) should be > poor latency score (%.1f)", r1.OverallScore, r2.OverallScore)
	}
}

func TestComputeScore_DatacenterPenalty(t *testing.T) {
	c := DefaultTrustScoreCalculator()
	residential := c.ComputeScore(200, 0, false, 1.0, true)
	datacenter := c.ComputeScore(200, 0, true, 1.0, true)
	if residential.Components.IPHealthScore <= datacenter.Components.IPHealthScore {
		t.Fatalf("Residential IP health (%.2f) should be > datacenter (%.2f)", residential.Components.IPHealthScore, datacenter.Components.IPHealthScore)
	}
}

func TestComputeHistorySuccessRate(t *testing.T) {
	results := []TestResult{
		{Ok: true}, {Ok: true}, {Ok: false}, {Ok: true}, {Ok: true},
	}
	rate := computeHistorySuccessRate(results, 5)
	if rate != 0.8 {
		t.Fatalf("SuccessRate = %.2f, want 0.8", rate)
	}
}

func TestComputeHistorySuccessRate_Empty(t *testing.T) {
	rate := computeHistorySuccessRate(nil, 10)
	if rate != 0.5 {
		t.Fatalf("Empty history rate = %.2f, want 0.5 (neutral)", rate)
	}
}

func TestComputeHistorySuccessRate_Limited(t *testing.T) {
	results := make([]TestResult, 20)
	for i := range results {
		results[i] = TestResult{Ok: i < 15} // first 15 good, last 5 bad
	}
	// Take last 10: 5 good + 5 bad = 0.5
	rate := computeHistorySuccessRate(results, 10)
	if rate != 0.5 {
		t.Fatalf("Limited history rate = %.2f, want 0.5 (5/10)", rate)
	}
}

func TestComputeScoreFromHistory(t *testing.T) {
	c := DefaultTrustScoreCalculator()
	results := []TestResult{
		{Ok: true}, {Ok: true}, {Ok: true}, {Ok: true}, {Ok: true},
	}
	result := c.ComputeScoreFromHistory(200, 0, false, results, true)
	if result.Components.HistoryScore != 1.0 {
		t.Fatalf("HistoryScore = %.2f, want 1.0", result.Components.HistoryScore)
	}
}
