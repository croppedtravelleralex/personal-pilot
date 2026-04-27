package proxy

// TrustScoreComponents holds the weighted dimensions of proxy trust.
type TrustScoreComponents struct {
	LatencyScore      float64 // 0-1, based on latency percentile
	IPHealthScore     float64 // 0-1, based on IP fraud score / datacenter detection
	HistoryScore      float64 // 0-1, based on historical success rate
	RegionMatchScore  float64 // 0-1, based on proxy region matching target region
}

// TrustScoreResult is the composite trust score for a proxy.
type TrustScoreResult struct {
	OverallScore float64 // 0-100
	Level        string  // "excellent", "good", "fair", "poor"
	Components   TrustScoreComponents
}

// TrustScoreCalculator computes multidimensional trust scores for proxies.
type TrustScoreCalculator struct {
	// Weights sum to 1.0
	LatencyWeight     float64
	IPHealthWeight    float64
	HistoryWeight     float64
	RegionMatchWeight float64

	// Latency thresholds (ms)
	LatencyExcellentMs int64 // below this = 1.0
	LatencyPoorMs      int64 // above this = 0.0

	// History lookback
	RecentTestCount int // number of recent tests to consider
}

// DefaultTrustScoreCalculator returns a calculator with standard weights.
func DefaultTrustScoreCalculator() *TrustScoreCalculator {
	return &TrustScoreCalculator{
		LatencyWeight:      0.30,
		IPHealthWeight:     0.25,
		HistoryWeight:      0.25,
		RegionMatchWeight:  0.20,
		LatencyExcellentMs: 200,
		LatencyPoorMs:      5000,
		RecentTestCount:    10,
	}
}

// ComputeScore calculates the trust score from raw inputs.
func (c *TrustScoreCalculator) ComputeScore(latencyMs int64, fraudScore int64, isDatacenter bool, historySuccessRate float64, regionMatch bool) TrustScoreResult {
	// Latency: linear interpolation between excellent and poor thresholds
	var latencyScore float64
	switch {
	case latencyMs <= 0:
		latencyScore = 0
	case latencyMs <= c.LatencyExcellentMs:
		latencyScore = 1.0
	case latencyMs >= c.LatencyPoorMs:
		latencyScore = 0.0
	default:
		latencyScore = 1.0 - float64(latencyMs-c.LatencyExcellentMs)/float64(c.LatencyPoorMs-c.LatencyExcellentMs)
	}

	// IP Health: based on fraud score (0-100) and datacenter detection
	var ipHealthScore float64
	if fraudScore >= 0 {
		ipHealthScore = 1.0 - float64(fraudScore)/100.0
		if ipHealthScore < 0 {
			ipHealthScore = 0
		}
	} else {
		ipHealthScore = 0.5 // unknown
	}
	if isDatacenter {
		ipHealthScore *= 0.5 // penalty for datacenter IPs
	}

	// History: direct pass-through
	historyScore := historySuccessRate
	if historyScore < 0 {
		historyScore = 0
	}
	if historyScore > 1 {
		historyScore = 1
	}

	// Region match: binary for now
	var regionScore float64
	if regionMatch {
		regionScore = 1.0
	}

	components := TrustScoreComponents{
		LatencyScore:     latencyScore,
		IPHealthScore:    ipHealthScore,
		HistoryScore:     historyScore,
		RegionMatchScore: regionScore,
	}

	overall := (latencyScore*c.LatencyWeight +
		ipHealthScore*c.IPHealthWeight +
		historyScore*c.HistoryWeight +
		regionScore*c.RegionMatchWeight) * 100.0

	var level string
	switch {
	case overall >= 80:
		level = "excellent"
	case overall >= 60:
		level = "good"
	case overall >= 40:
		level = "fair"
	default:
		level = "poor"
	}

	return TrustScoreResult{
		OverallScore: overall,
		Level:        level,
		Components:   components,
	}
}

// ComputeScoreFromHistory computes a trust score incorporating historical test results.
func (c *TrustScoreCalculator) ComputeScoreFromHistory(latencyMs int64, fraudScore int64, isDatacenter bool, recentResults []TestResult, regionMatch bool) TrustScoreResult {
	historyRate := computeHistorySuccessRate(recentResults, c.RecentTestCount)
	return c.ComputeScore(latencyMs, fraudScore, isDatacenter, historyRate, regionMatch)
}

// computeHistorySuccessRate calculates the success rate from the N most recent results.
func computeHistorySuccessRate(results []TestResult, maxCount int) float64 {
	if len(results) == 0 {
		return 0.5 // neutral unknown
	}
	count := len(results)
	if count > maxCount {
		count = maxCount
		results = results[len(results)-count:] // take most recent
	}
	successes := 0
	for _, r := range results {
		if r.Ok {
			successes++
		}
	}
	return float64(successes) / float64(count)
}
