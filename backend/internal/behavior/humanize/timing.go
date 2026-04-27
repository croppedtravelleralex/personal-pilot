package humanize

import (
	"math"
	"math/rand/v2"
)

// ComputeActionGap computes the total delay before any action.
// Returns pre_action_delay + sampled gap from the timing distribution.
func ComputeActionGap(config *HumanizationConfig) uint32 {
	return PreActionDelay(config) + sampleGap(&config.Timing.Distribution)
}

// PreActionDelay returns the pre-action decision delay with ±33% jitter.
func PreActionDelay(config *HumanizationConfig) uint32 {
	base := config.Timing.PreActionDelayMs
	if base == 0 {
		return 0
	}
	variance := int64(base) / 3
	jitter := randRangeInt(-variance, variance)
	adjusted := int64(base) + jitter
	if adjusted < 0 {
		adjusted = 0
	}
	return uint32(adjusted)
}

// ComputeTypingInterval returns the typing interval for one character in milliseconds.
func ComputeTypingInterval(config *HumanizationConfig, ch rune) uint32 {
	baseInterval := uint32(60000 / max(1, config.Typing.BaseWPM))
	varianceFactor := 1.0 + (rand.Float64()*2-1)*(float64(config.Typing.SpeedVariancePercent)/100.0)
	interval := uint32(float64(baseInterval) * varianceFactor)

	// Add penalty for uppercase or ASCII punctuation
	if (ch >= 'A' && ch <= 'Z') || isASCIISymbol(ch) {
		interval += 25
	}
	return interval
}

// sampleGap draws a delay from the configured time distribution.
func sampleGap(d *TimeDistribution) uint32 {
	switch d.Type {
	case DistUniform:
		return randRangeUint32(d.MinMs, d.MaxMs)
	case DistNormal:
		return sampleNormal(d.MeanMs, d.StddevMs, d.MeanMs)
	case DistRightSkewed:
		return sampleRightSkewed(d.MinMs, d.ModeMs, d.MaxMs)
	default:
		return 0
	}
}

// sampleNormal uses the Box-Muller transform to generate a normally-distributed value.
// Clamps to [0, 3*mean] to avoid extreme outliers.
func sampleNormal(mean, stddev, clampMax uint32) uint32 {
	// Box-Muller: generate two uniform (0,1] values
	u1 := rand.Float64()
	for u1 == 0 { // ln(0) = -Inf, avoid
		u1 = rand.Float64()
	}
	u2 := rand.Float64()

	z := math.Sqrt(-2*math.Log(u1)) * math.Cos(2*math.Pi*u2)
	sample := float64(mean) + z*float64(stddev)

	// Clamp to [0, 3*mean]
	if sample < 0 {
		sample = 0
	}
	maxVal := float64(clampMax) * 3.0
	if sample > maxVal {
		sample = maxVal
	}
	return uint32(sample)
}

// sampleRightSkewed uses an inverse-CDF exponential transform to produce
// values that cluster near the mode but allow long tails toward the max.
func sampleRightSkewed(minMs, modeMs, maxMs uint32) uint32 {
	lambda := 1.0 / float64(max(1, modeMs-minMs+1))
	u := rand.Float64()
	expSample := -math.Log(1.0-u) / lambda
	raw := float64(minMs) + expSample
	return clampUint32(uint32(raw), minMs, maxMs)
}

// sampleUniform is a convenience wrapper around randRangeUint32.
func sampleUniform(minMs, maxMs uint32) uint32 {
	return randRangeUint32(minMs, maxMs)
}

// ---- helpers ----

// randRangeInt returns a random int64 in [min, max].
func randRangeInt(min, max int64) int64 {
	if min >= max {
		return min
	}
	return min + rand.Int64N(max-min+1)
}

// randRangeUint32 returns a random uint32 in [min, max].
func randRangeUint32(min, max uint32) uint32 {
	if min >= max {
		return min
	}
	return min + rand.Uint32N(max-min+1)
}

// randRangeFloat64 returns a random float64 in [min, max).
func randRangeFloat64(min, max float64) float64 {
	return min + rand.Float64()*(max-min)
}

func clampUint32(v, lo, hi uint32) uint32 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

func isASCIISymbol(ch rune) bool {
	return (ch >= '!' && ch <= '/') || (ch >= ':' && ch <= '@') ||
		(ch >= '[' && ch <= '`') || (ch >= '{' && ch <= '~')
}
