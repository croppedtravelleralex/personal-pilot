package humanize

import (
	"math/rand"
	"time"
)

// BioNoiseConfig controls biological delay injection (entropy against ML rhythm detectors).
type BioNoiseConfig struct {
	BaseDelayMs       int
	LongPauseChance   float64 // ~0.03 mimics distraction
	LongPauseMinMs    int
	LongPauseMaxMs    int
	InterActionJitter float64 // 0.7-1.3 multiplier band
}

func DefaultBioNoiseConfig() BioNoiseConfig {
	return BioNoiseConfig{
		BaseDelayMs:       200,
		LongPauseChance:   0.03,
		LongPauseMinMs:    1000,
		LongPauseMaxMs:    2500,
		InterActionJitter: 0.3,
	}
}

// NaturalDelay sleeps with human-like jitter and occasional long pauses.
func NaturalDelay(cfg BioNoiseConfig) {
	if cfg.BaseDelayMs <= 0 {
		cfg = DefaultBioNoiseConfig()
	}
	mult := 1.0
	if cfg.InterActionJitter > 0 {
		mult = 1.0 - cfg.InterActionJitter + rand.Float64()*(2*cfg.InterActionJitter)
	}
	delay := float64(cfg.BaseDelayMs) * mult
	if rand.Float64() < cfg.LongPauseChance {
		delay += float64(cfg.LongPauseMinMs) + rand.Float64()*float64(cfg.LongPauseMaxMs-cfg.LongPauseMinMs)
	}
	time.Sleep(time.Duration(delay) * time.Millisecond)
}

// OperationGap returns a randomized gap between scheduled operations (30-180s band).
func OperationGap(minSec, maxSec int) time.Duration {
	if minSec <= 0 {
		minSec = 30
	}
	if maxSec <= minSec {
		maxSec = minSec + 150
	}
	sec := minSec + rand.Intn(maxSec-minSec+1)
	if rand.Float64() < 0.1 {
		sec += 60 + rand.Intn(240)
	}
	return time.Duration(sec) * time.Second
}
