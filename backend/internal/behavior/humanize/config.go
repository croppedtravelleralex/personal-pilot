// Package humanize provides behavioral humanization for browser automation.
// It injects "real human" characteristics into machine-executed actions:
// typing rhythm, mouse trajectory, scroll patterns, and failure recovery.
//
// Ported from persona-pilot/src/humanize/ (Rust) to Go.
package humanize

// HumanizationLevel controls the intensity of behavioral injection.
type HumanizationLevel int

const (
	LevelNone    HumanizationLevel = 0
	LevelMinimal HumanizationLevel = 1
	LevelMedium  HumanizationLevel = 2
	LevelHigh    HumanizationLevel = 3
)

func (l HumanizationLevel) IsActive() bool { return l != LevelNone }

// TimeDistribution models how delays between actions are sampled.
type TimeDistributionType int

const (
	DistUniform      TimeDistributionType = 0
	DistNormal       TimeDistributionType = 1
	DistRightSkewed  TimeDistributionType = 2
)

// TimeDistribution is a tagged union for delay sampling.
// Only the fields matching Type are meaningful.
type TimeDistribution struct {
	Type    TimeDistributionType
	MinMs   uint32
	MaxMs   uint32
	MeanMs  uint32
	StddevMs uint32
	ModeMs  uint32
}

// ScrollSpeed controls the speed of scroll operations.
type ScrollSpeed int

const (
	ScrollFast   ScrollSpeed = 0
	ScrollNormal ScrollSpeed = 1
	ScrollSlow   ScrollSpeed = 2
)

// FailureStyle models how the system responds to action failures.
type FailureStyleType int

const (
	FailureInstant FailureStyleType = 0
	FailureHuman   FailureStyleType = 1
)

type FailureStyle struct {
	Type         FailureStyleType
	MinWaitMs    uint32
	MaxWaitMs    uint32
	MaxRetries   uint32
	GiveUpChance float64
}

// BiasDirection biases click offset toward a screen quadrant.
type BiasDirection int

const (
	BiasTopLeft     BiasDirection = 0
	BiasTopRight    BiasDirection = 1
	BiasBottomLeft  BiasDirection = 2
	BiasBottomRight BiasDirection = 3
	BiasCenter      BiasDirection = 4
)

// ClickOffset configures click position randomization within an element.
type ClickOffset struct {
	RadiusPx      uint32
	BiasDirection *BiasDirection // nil = uniform random within radius
}

// TypingPattern configures keyboard typing behavior.
type TypingPattern struct {
	BaseWPM             uint32  // base words per minute
	SpeedVariancePercent uint32 // 0-100, per-keystroke speed fluctuation
	ErrorRetryChance    float64 // 0.0-1.0, probability of typo+backspace per character
	PauseChance         float64 // 0.0-1.0, probability of mid-text "thinking" pause
	PauseDurationMs     uint32  // duration of a thinking pause in ms
}

// ScrollBehavior configures scroll humanization.
type ScrollBehavior struct {
	OvershootRatio      *float64   // nil = no overshoot; e.g. 0.3 = 30% past target
	Speed               ScrollSpeed
	HoverBeforeScrollMs *uint32    // nil = no hover; value = hover duration before scroll
}

// TimingConfig configures delay/jitter injection.
type TimingConfig struct {
	Distribution     TimeDistribution
	PreActionDelayMs uint32 // fixed "decision time" added before every action
}

// HumanizationConfig is the master configuration for behavioral humanization.
type HumanizationConfig struct {
	Level   HumanizationLevel
	Timing  TimingConfig
	Click   ClickOffset
	Typing  TypingPattern
	Scroll  ScrollBehavior
	Failure FailureStyle
}

// FromLevel populates the config with a built-in preset for the given level.
func (c *HumanizationConfig) FromLevel(level HumanizationLevel) {
	c.Level = level

	switch level {
	case LevelNone:
		c.Timing = TimingConfig{
			Distribution:     TimeDistribution{Type: DistUniform, MinMs: 0, MaxMs: 0},
			PreActionDelayMs: 0,
		}
		c.Click = ClickOffset{RadiusPx: 0, BiasDirection: nil}
		c.Typing = TypingPattern{
			BaseWPM: 9999, SpeedVariancePercent: 0, ErrorRetryChance: 0,
			PauseChance: 0, PauseDurationMs: 0,
		}
		c.Scroll = ScrollBehavior{OvershootRatio: nil, Speed: ScrollFast, HoverBeforeScrollMs: nil}
		c.Failure = FailureStyle{Type: FailureInstant, MaxRetries: 3}

	case LevelMinimal:
		c.Timing = TimingConfig{
			Distribution:     TimeDistribution{Type: DistRightSkewed, MinMs: 100, ModeMs: 300, MaxMs: 1500},
			PreActionDelayMs: 50,
		}
		c.Click = ClickOffset{RadiusPx: 3, BiasDirection: nil}
		c.Typing = TypingPattern{
			BaseWPM: 60, SpeedVariancePercent: 20, ErrorRetryChance: 0.01,
			PauseChance: 0.02, PauseDurationMs: 500,
		}
		hms := uint32(100)
		c.Scroll = ScrollBehavior{OvershootRatio: ptrF64(0.15), Speed: ScrollFast, HoverBeforeScrollMs: &hms}
		c.Failure = FailureStyle{Type: FailureHuman, MinWaitMs: 500, MaxWaitMs: 3000, MaxRetries: 3, GiveUpChance: 0.05}

	case LevelMedium:
		c.Timing = TimingConfig{
			Distribution:     TimeDistribution{Type: DistRightSkewed, MinMs: 150, ModeMs: 400, MaxMs: 2000},
			PreActionDelayMs: 80,
		}
		c.Click = ClickOffset{RadiusPx: 8, BiasDirection: nil}
		c.Typing = TypingPattern{
			BaseWPM: 40, SpeedVariancePercent: 50, ErrorRetryChance: 0.03,
			PauseChance: 0.08, PauseDurationMs: 1000,
		}
		hms := uint32(250)
		c.Scroll = ScrollBehavior{OvershootRatio: ptrF64(0.35), Speed: ScrollNormal, HoverBeforeScrollMs: &hms}
		c.Failure = FailureStyle{Type: FailureHuman, MinWaitMs: 1500, MaxWaitMs: 6000, MaxRetries: 3, GiveUpChance: 0.10}

	case LevelHigh:
		c.Timing = TimingConfig{
			Distribution:     TimeDistribution{Type: DistRightSkewed, MinMs: 300, ModeMs: 800, MaxMs: 4000},
			PreActionDelayMs: 150,
		}
		c.Click = ClickOffset{RadiusPx: 12, BiasDirection: nil}
		c.Typing = TypingPattern{
			BaseWPM: 35, SpeedVariancePercent: 60, ErrorRetryChance: 0.05,
			PauseChance: 0.12, PauseDurationMs: 1500,
		}
		hms := uint32(400)
		c.Scroll = ScrollBehavior{OvershootRatio: ptrF64(0.50), Speed: ScrollSlow, HoverBeforeScrollMs: &hms}
		c.Failure = FailureStyle{Type: FailureHuman, MinWaitMs: 3000, MaxWaitMs: 12000, MaxRetries: 2, GiveUpChance: 0.25}
	}
}

// DefaultConfig returns a Medium-level humanization config.
func DefaultConfig() HumanizationConfig {
	var c HumanizationConfig
	c.FromLevel(LevelMedium)
	return c
}

// ConfigForLevel returns a config pre-set to the given level.
func ConfigForLevel(level HumanizationLevel) HumanizationConfig {
	var c HumanizationConfig
	c.FromLevel(level)
	return c
}

func ptrF64(v float64) *float64 { return &v }
