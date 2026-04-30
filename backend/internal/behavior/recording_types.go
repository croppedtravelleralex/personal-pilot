package behavior

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

const RecordingExportVersion = 1

const defaultRecordingEventPageLimit = 100

// RecordingExportBundle is the JSON-safe payload used by import/export flows.
type RecordingExportBundle struct {
	Version    int        `json:"version"`
	ExportedAt string     `json:"exportedAt"`
	Recording  *Recording `json:"recording"`
}

// RecordedEvent is a single captured user interaction during recording.
type RecordedEvent struct {
	T          int64   `json:"t"`             // timestamp in ms from recording start
	Type       string  `json:"type"`          // new recordings use "move"|"down"|"up"|"key"|"scroll"|"input"|"change"|"paste"|"composition"
	X          float64 `json:"x,omitempty"`   // clientX
	Y          float64 `json:"y,omitempty"`   // clientY
	Button     int     `json:"btn,omitempty"` // 0=left, 1=middle, 2=right
	Key        string  `json:"key,omitempty"`
	Text       string  `json:"text,omitempty"`
	DeltaX     float64 `json:"dx,omitempty"`
	DeltaY     float64 `json:"dy,omitempty"`
	InputType  string  `json:"inputType,omitempty"`
	TargetPath string  `json:"targetPath,omitempty"`
	Sensitive  bool    `json:"sensitive,omitempty"`
}

// Recording is a complete recorded behavior session.
type Recording struct {
	ID               string          `json:"id"`
	Name             string          `json:"name"`
	Description      string          `json:"description"`
	Events           []RecordedEvent `json:"events"`
	EventCount       int             `json:"eventCount,omitempty"`
	DurationMs       int64           `json:"durationMs"`
	ViewportW        int             `json:"viewportW"`
	ViewportH        int             `json:"viewportH"`
	StartURL         string          `json:"startUrl,omitempty"`
	CurrentURL       string          `json:"currentUrl,omitempty"`
	Title            string          `json:"title,omitempty"`
	DevicePixelRatio float64         `json:"devicePixelRatio,omitempty"`
	Scale            float64         `json:"scale,omitempty"`
	CreatedAt        string          `json:"createdAt"`
}

// RecordingSummary is the list/API shape for recordings without full events.
type RecordingSummary struct {
	ID               string  `json:"id"`
	Name             string  `json:"name"`
	Description      string  `json:"description"`
	EventCount       int     `json:"eventCount"`
	DurationMs       int64   `json:"durationMs"`
	ViewportW        int     `json:"viewportW"`
	ViewportH        int     `json:"viewportH"`
	StartURL         string  `json:"startUrl,omitempty"`
	CurrentURL       string  `json:"currentUrl,omitempty"`
	Title            string  `json:"title,omitempty"`
	DevicePixelRatio float64 `json:"devicePixelRatio,omitempty"`
	Scale            float64 `json:"scale,omitempty"`
	CreatedAt        string  `json:"createdAt"`
}

// RecordingEventStats summarizes event counts across a full recording.
type RecordingEventStats struct {
	Total  int `json:"total"`
	Move   int `json:"move"`
	Click  int `json:"click"`
	Key    int `json:"key"`
	Scroll int `json:"scroll"`
}

// RecordingDetailPage is the detail/API shape for loading event slices on demand.
type RecordingDetailPage struct {
	Recording   *RecordingSummary   `json:"recording"`
	Events      []RecordedEvent     `json:"events"`
	EventOffset int                 `json:"eventOffset"`
	EventLimit  int                 `json:"eventLimit"`
	EventTotal  int                 `json:"eventTotal"`
	Stats       RecordingEventStats `json:"stats"`
}

// Summary returns metadata suitable for list views.
func (r *Recording) Summary() *RecordingSummary {
	if r == nil {
		return nil
	}
	eventCount := r.EventCount
	if eventCount == 0 && len(r.Events) > 0 {
		eventCount = len(r.Events)
	}
	return &RecordingSummary{
		ID:               r.ID,
		Name:             r.Name,
		Description:      r.Description,
		EventCount:       eventCount,
		DurationMs:       r.DurationMs,
		ViewportW:        r.ViewportW,
		ViewportH:        r.ViewportH,
		StartURL:         r.StartURL,
		CurrentURL:       r.CurrentURL,
		Title:            r.Title,
		DevicePixelRatio: r.DevicePixelRatio,
		Scale:            r.Scale,
		CreatedAt:        r.CreatedAt,
	}
}

// DetailPage returns a bounded event page plus metadata and full-recording stats.
func (r *Recording) DetailPage(offset int, limit int) *RecordingDetailPage {
	if r == nil {
		return nil
	}

	total := len(r.Events)
	offset, limit, end := normalizeRecordingEventPage(offset, limit, total)

	events := make([]RecordedEvent, 0)
	if offset < end {
		events = append(events, r.Events[offset:end]...)
	}

	return &RecordingDetailPage{
		Recording:   r.Summary(),
		Events:      events,
		EventOffset: offset,
		EventLimit:  limit,
		EventTotal:  total,
		Stats:       BuildRecordingEventStats(r.Events),
	}
}

func normalizeRecordingEventPage(offset int, limit int, total int) (int, int, int) {
	if offset < 0 {
		offset = 0
	}
	if limit <= 0 {
		limit = defaultRecordingEventPageLimit
	}
	if total < 0 {
		total = 0
	}
	if offset > total {
		offset = total
	}
	end := offset + limit
	if end > total {
		end = total
	}
	return offset, limit, end
}

// BuildRecordingEventStats counts event families used by the detail UI.
func BuildRecordingEventStats(events []RecordedEvent) RecordingEventStats {
	stats := RecordingEventStats{Total: len(events)}
	for _, ev := range events {
		switch ev.Type {
		case "move":
			stats.Move++
		case "click", "down", "up":
			stats.Click++
		case "key":
			stats.Key++
		case "scroll":
			stats.Scroll++
		}
	}
	return stats
}

// ActiveRecordingStatus exposes the backend as the single source of truth.
type ActiveRecordingStatus struct {
	Active                bool     `json:"active"`
	Count                 int      `json:"count"`
	ProfileID             string   `json:"profileId,omitempty"`
	ProfileIDs            []string `json:"profileIds"`
	InMemoryProfileIDs    []string `json:"inMemoryProfileIds,omitempty"`
	RecoverableProfileIDs []string `json:"recoverableProfileIds,omitempty"`
}

// VariationConfig controls how much random variation to apply during playback.
type VariationConfig struct {
	Intensity         float64            `json:"intensity"`        // 0.0-1.0 overall variation strength
	TimingJitter      float64            `json:"timingJitter"`     // max timing offset in ms
	PositionJitter    float64            `json:"positionJitter"`   // max position offset in pixels
	SpeedVariation    float64            `json:"speedVariation"`   // 0.0-1.0 speed multiplier range
	MicroCorrections  bool               `json:"microCorrections"` // add overshoot/undershoot corrections
	ExtraPauses       bool               `json:"extraPauses"`      // insert random pauses between actions
	ExecutionPolicy   *ExecutionPolicy   `json:"executionPolicy,omitempty"`
	TemplateSemantics *TemplateSemantics `json:"templateSemantics,omitempty"`
}

// NewRecordingExportBundle creates a stable JSON export wrapper.
func NewRecordingExportBundle(recording *Recording) *RecordingExportBundle {
	return &RecordingExportBundle{
		Version:    RecordingExportVersion,
		ExportedAt: time.Now().Format(time.RFC3339),
		Recording:  cloneRecording(recording),
	}
}

// NewRecordingFromImportPayload parses either a bundle or a raw recording JSON payload.
func NewRecordingFromImportPayload(payload string, name string) (*Recording, error) {
	payload = strings.TrimSpace(payload)
	if payload == "" {
		return nil, fmt.Errorf("payload is required")
	}

	var bundle RecordingExportBundle
	if err := json.Unmarshal([]byte(payload), &bundle); err == nil && bundle.Recording != nil {
		return NewRecordingCopy(bundle.Recording, name), nil
	}

	var recording Recording
	if err := json.Unmarshal([]byte(payload), &recording); err != nil {
		return nil, fmt.Errorf("parse recording JSON: %w", err)
	}
	if strings.TrimSpace(recording.ID) == "" && strings.TrimSpace(recording.Name) == "" && len(recording.Events) == 0 {
		return nil, fmt.Errorf("payload does not contain a recording")
	}
	return NewRecordingCopy(&recording, name), nil
}

// NewRecordingCopy creates a new saved recording from an existing template.
func NewRecordingCopy(source *Recording, name string) *Recording {
	if source == nil {
		return nil
	}
	return newRecordingClone(source, name, source.Events, false)
}

// NewRecordingTrim creates a new recording from a 1-based inclusive event range.
func NewRecordingTrim(source *Recording, startEvent int, endEvent int, name string) (*Recording, error) {
	if source == nil {
		return nil, fmt.Errorf("recording is nil")
	}
	total := len(source.Events)
	if startEvent < 1 || endEvent < startEvent || endEvent > total {
		return nil, fmt.Errorf("event range must be 1-based inclusive and within 1..%d", total)
	}
	return newRecordingClone(source, name, source.Events[startEvent-1:endEvent], true), nil
}

func newRecordingClone(source *Recording, name string, events []RecordedEvent, rebase bool) *Recording {
	if source == nil {
		return nil
	}
	clone := cloneRecording(source)
	clone.ID = generateID()
	if trimmedName := strings.TrimSpace(name); trimmedName != "" {
		clone.Name = name
	}
	clone.CreatedAt = time.Now().Format(time.RFC3339)
	clone.Events = copyRecordedEvents(events)
	if rebase {
		rebaseRecordedEvents(clone.Events)
	}
	clone.EventCount = len(clone.Events)
	clone.DurationMs = recalculateRecordingDuration(clone.Events)
	return clone
}

func cloneRecording(source *Recording) *Recording {
	if source == nil {
		return nil
	}
	clone := *source
	clone.Events = copyRecordedEvents(source.Events)
	clone.EventCount = len(clone.Events)
	clone.DurationMs = recalculateRecordingDuration(clone.Events)
	return &clone
}

func copyRecordedEvents(events []RecordedEvent) []RecordedEvent {
	if len(events) == 0 {
		return []RecordedEvent{}
	}
	copied := make([]RecordedEvent, len(events))
	copy(copied, events)
	return copied
}

func rebaseRecordedEvents(events []RecordedEvent) {
	if len(events) == 0 {
		return
	}
	offset := events[0].T
	for i := range events {
		events[i].T -= offset
	}
}

func recalculateRecordingDuration(events []RecordedEvent) int64 {
	if len(events) == 0 {
		return 0
	}
	duration := events[len(events)-1].T - events[0].T
	if duration < 0 {
		return 0
	}
	return duration
}
