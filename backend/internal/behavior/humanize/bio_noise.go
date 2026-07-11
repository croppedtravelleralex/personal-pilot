package humanize

import (
	"hash/fnv"
	"math/rand"
	"strings"
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

// BioNoiseConfigFromSeed derives stable bio-noise parameters from a humanize seed.
func BioNoiseConfigFromSeed(seed string) BioNoiseConfig {
	cfg := DefaultBioNoiseConfig()
	if strings.TrimSpace(seed) == "" {
		return cfg
	}
	h := fnv.New32a()
	_, _ = h.Write([]byte(seed))
	v := h.Sum32()
	cfg.BaseDelayMs = 120 + int(v%180)
	cfg.LongPauseChance = 0.02 + float64(v%20)/1000.0
	cfg.LongPauseMinMs = 800 + int(v%400)
	cfg.LongPauseMaxMs = cfg.LongPauseMinMs + 800 + int((v>>8)%900)
	cfg.InterActionJitter = 0.2 + float64((v>>16)%15)/100.0
	return cfg
}

// DwellFromSeed returns a short dwell duration for typing/click gaps (docs/53 L5).
func DwellFromSeed(seed, context string) time.Duration {
	cfg := BioNoiseConfigFromSeed(seed + "|" + context)
	base := cfg.BaseDelayMs / 4
	if base < 20 {
		base = 20
	}
	h := fnv.New32a()
	_, _ = h.Write([]byte(seed + "|" + context + "|dwell"))
	jitter := int(h.Sum32() % 40)
	return time.Duration(base+jitter) * time.Millisecond
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
