package lifecycle

func AssessRisk(state State, coherenceScore float64) float64 {
	risk := state.RiskScore * 0.6
	if coherenceScore < 0.7 {
		risk += (0.7 - coherenceScore)
	}
	if state.SessionCount == 0 {
		risk += 0.1
	}
	if risk > 1 {
		return 1
	}
	if risk < 0 {
		return 0
	}
	return risk
}
