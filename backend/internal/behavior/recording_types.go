package behavior

// RecordedEvent is a single captured user interaction during recording.
type RecordedEvent struct {
	T      int64   `json:"t"`            // timestamp in ms from recording start
	Type   string  `json:"type"`         // "move"|"down"|"up"|"key"|"scroll"|"click"
	X      float64 `json:"x,omitempty"`  // clientX
	Y      float64 `json:"y,omitempty"`  // clientY
	Button int     `json:"btn,omitempty"` // 0=left, 1=middle, 2=right
	Key    string  `json:"key,omitempty"`
	Text   string  `json:"text,omitempty"`
	DeltaX float64 `json:"dx,omitempty"`
	DeltaY float64 `json:"dy,omitempty"`
}

// Recording is a complete recorded behavior session.
type Recording struct {
	ID          string          `json:"id"`
	Name        string          `json:"name"`
	Description string          `json:"description"`
	Events      []RecordedEvent `json:"events"`
	DurationMs  int64           `json:"durationMs"`
	ViewportW   int             `json:"viewportW"`
	ViewportH   int             `json:"viewportH"`
	CreatedAt   string          `json:"createdAt"`
}

// VariationConfig controls how much random variation to apply during playback.
type VariationConfig struct {
	Intensity        float64 `json:"intensity"`        // 0.0-1.0 overall variation strength
	TimingJitter     float64 `json:"timingJitter"`     // max timing offset in ms
	PositionJitter   float64 `json:"positionJitter"`   // max position offset in pixels
	SpeedVariation   float64 `json:"speedVariation"`   // 0.0-1.0 speed multiplier range
	MicroCorrections bool    `json:"microCorrections"` // add overshoot/undershoot corrections
	ExtraPauses      bool    `json:"extraPauses"`      // insert random pauses between actions
}
