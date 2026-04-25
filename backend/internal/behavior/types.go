package behavior

import "time"

// Profile defines a complete behavioral persona.
type Profile struct {
	ID          string          `json:"id"`
	Name        string          `json:"name"`
	Description string          `json:"description"`
	Mouse       MouseProfile    `json:"mouse"`
	Keyboard    KeyboardProfile `json:"keyboard"`
	Scroll      ScrollProfile   `json:"scroll"`
	Idle        IdleProfile     `json:"idle"`
}

// MouseProfile controls mouse movement simulation.
type MouseProfile struct {
	Enabled          bool    `json:"enabled"`
	IdleMoveInterval string  `json:"idleMoveInterval"` // e.g. "3s-15s" random interval
	CurveStyle       string  `json:"curveStyle"`       // "bezier2" | "bezier3" | "natural"
	SpeedMean        float64 `json:"speedMean"`         // mean pixels per move step (200-800)
	SpeedStdDev      float64 `json:"speedStdDev"`       // stddev for speed variation
	JitterPx         int     `json:"jitterPx"`          // max random offset per step (1-5)
	PauseProb        float64 `json:"pauseProb"`         // probability of micro-pause mid-path (0-0.3)
	PauseMaxMs       int     `json:"pauseMaxMs"`        // max pause duration ms (50-300)
}

// KeyboardProfile controls typing rhythm simulation.
type KeyboardProfile struct {
	Enabled       bool    `json:"enabled"`
	BaseDelayMs   int     `json:"baseDelayMs"`   // base inter-key delay (50-200)
	DelayStdDevMs int     `json:"delayStdDevMs"` // stddev for delay variation (20-80)
	BurstProb     float64 `json:"burstProb"`     // probability of fast key burst (0-0.15)
	BurstKeys     int     `json:"burstKeys"`     // keys in a burst (2-4)
	TypoProb      float64 `json:"typoProb"`      // probability of typo + backspace (0-0.03)
	BigramDelays  bool    `json:"bigramDelays"`  // add extra delay for common bigrams
}

// ScrollProfile controls scroll behavior simulation.
type ScrollProfile struct {
	Enabled           bool    `json:"enabled"`
	IdleScrollProb    float64 `json:"idleScrollProb"`    // prob of scroll while "reading" (0-0.3)
	ScrollStepPx      int     `json:"scrollStepPx"`      // typical scroll step (50-200)
	ScrollStepStdDev  int     `json:"scrollStepStdDev"`  // variation in step size
	PauseBetweenMs    int     `json:"pauseBetweenMs"`    // delay between scroll steps (200-2000)
	PauseStdDevMs     int     `json:"pauseStdDevMs"`     // variation in pause
	OverscrollProb    float64 `json:"overscrollProb"`    // prob of overscroll+correction
	ReverseProb       float64 `json:"reverseProb"`       // prob of brief reverse scroll
}

// IdleProfile controls idle-time behavior patterns.
type IdleProfile struct {
	MouseWander bool   `json:"mouseWander"` // move mouse to random position periodically
	TabSwitch   bool   `json:"tabSwitch"`   // simulate tab switches (if multiple tabs)
	FocusLoss   bool   `json:"focusLoss"`   // simulate brief focus loss (click outside)
}

// Config is the runtime behavior configuration (serializable subset).
type Config struct {
	ProfileID string `json:"profileId"`
	Enabled   bool   `json:"enabled"`
	Intensity float64 `json:"intensity"` // 0.0 (subtle) to 1.0 (aggressive)
}

// State holds the engine's runtime state.
type State struct {
	Running      bool      `json:"running"`
	StartedAt    time.Time `json:"startedAt"`
	LastMoveAt   time.Time `json:"lastMoveAt"`
	LastTypeAt   time.Time `json:"lastTypeAt"`
	LastScrollAt time.Time `json:"lastScrollAt"`
}
