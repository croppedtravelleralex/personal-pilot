package behavior

import (
	"math"
	"sort"
	"fmt"
)

// CadenceScore summarizes access rhythm naturalness from recorded events.
type CadenceScore struct {
	Score           int     `json:"score"`
	IKIMeanMs       float64 `json:"ikiMeanMs"`
	IKIStdMs        float64 `json:"ikiStdMs"`
	DwellMeanMs     float64 `json:"dwellMeanMs"`
	ScrollBurstRate float64 `json:"scrollBurstRate"`
	EventCount      int     `json:"eventCount"`
	Message         string  `json:"message"`
}

// AnalyzeRecordingCadence computes rhythm metrics from a recording's events.
func AnalyzeRecordingCadence(events []RecordedEvent) CadenceScore {
	if len(events) < 2 {
		return CadenceScore{Score: 40, Message: "insufficient events for cadence analysis"}
	}
	intervals := make([]float64, 0, len(events)-1)
	var scrolls int
	for i := 1; i < len(events); i++ {
		delta := float64(events[i].T - events[i-1].T)
		if delta >= 0 {
			intervals = append(intervals, delta)
		}
		if events[i].Type == "scroll" {
			scrolls++
		}
	}
	if len(intervals) == 0 {
		return CadenceScore{Score: 45, Message: "no timing intervals"}
	}
	sort.Float64s(intervals)
	mean := meanFloat64(intervals)
	std := stdFloat64(intervals, mean)
	dwell := intervals[len(intervals)/2]
	scrollRate := float64(scrolls) / float64(len(events))

	score := 55
	if mean >= 80 && mean <= 2500 {
		score += 15
	}
	if std >= 40 && std <= 1800 {
		score += 10
	}
	if scrollRate >= 0.05 && scrollRate <= 0.35 {
		score += 10
	}
	if score > 100 {
		score = 100
	}
	return CadenceScore{
		Score:           score,
		IKIMeanMs:       mean,
		IKIStdMs:        std,
		DwellMeanMs:     dwell,
		ScrollBurstRate: scrollRate,
		EventCount:      len(events),
		Message:         "cadence analyzed from recording events",
	}
}

// RecordingAnalyzeReport aggregates heatmap/dwell/cadence stats for a recording.
type RecordingAnalyzeReport struct {
	RecordingID string              `json:"recordingId"`
	Stats       RecordingEventStats `json:"stats"`
	Cadence     CadenceScore          `json:"cadence"`
	Hotspots    []ClickHotspot        `json:"hotspots"`
	DurationMs  int64               `json:"durationMs"`
}

type ClickHotspot struct {
	X     float64 `json:"x"`
	Y     float64 `json:"y"`
	Count int     `json:"count"`
}

// AnalyzeRecording builds a full analyze report for API/UI consumption.
func AnalyzeRecording(rec *Recording) *RecordingAnalyzeReport {
	if rec == nil {
		return nil
	}
	return &RecordingAnalyzeReport{
		RecordingID: rec.ID,
		Stats:       BuildRecordingEventStats(rec.Events),
		Cadence:     AnalyzeRecordingCadence(rec.Events),
		Hotspots:    buildClickHotspots(rec.Events),
		DurationMs:  rec.DurationMs,
	}
}

func buildClickHotspots(events []RecordedEvent) []ClickHotspot {
	buckets := map[[2]int]int{}
	for _, ev := range events {
		if ev.Type != "down" && ev.Type != "click" {
			continue
		}
		key := [2]int{int(ev.X) / 50, int(ev.Y) / 50}
		buckets[key]++
	}
	out := make([]ClickHotspot, 0, len(buckets))
	for key, count := range buckets {
		out = append(out, ClickHotspot{
			X:     float64(key[0]*50 + 25),
			Y:     float64(key[1]*50 + 25),
			Count: count,
		})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Count > out[j].Count })
	if len(out) > 12 {
		out = out[:12]
	}
	return out
}

// RecordingDiffReport compares two recordings for analyze/merge workflows.
type RecordingDiffReport struct {
	LeftID         string              `json:"leftId"`
	RightID        string              `json:"rightId"`
	EventDelta     int                 `json:"eventDelta"`
	DurationDeltaMs int64              `json:"durationDeltaMs"`
	LeftStats      RecordingEventStats `json:"leftStats"`
	RightStats     RecordingEventStats `json:"rightStats"`
	Summary        []string            `json:"summary"`
}

// DiffRecordings compares event counts and duration between two recordings.
func DiffRecordings(left, right *Recording) *RecordingDiffReport {
	if left == nil || right == nil {
		return nil
	}
	leftStats := BuildRecordingEventStats(left.Events)
	rightStats := BuildRecordingEventStats(right.Events)
	report := &RecordingDiffReport{
		LeftID:          left.ID,
		RightID:         right.ID,
		EventDelta:      len(right.Events) - len(left.Events),
		DurationDeltaMs: right.DurationMs - left.DurationMs,
		LeftStats:       leftStats,
		RightStats:      rightStats,
	}
	report.Summary = append(report.Summary,
		"event delta: "+itoa(report.EventDelta),
		"duration delta ms: "+itoa64(report.DurationDeltaMs),
	)
	return report
}

// MergeRecordings concatenates two recordings with rebased timestamps.
func MergeRecordings(left, right *Recording, name string) *Recording {
	if left == nil {
		return NewRecordingCopy(right, name)
	}
	if right == nil {
		return NewRecordingCopy(left, name)
	}
	mergedEvents := copyRecordedEvents(left.Events)
	offset := int64(0)
	if len(mergedEvents) > 0 {
		offset = mergedEvents[len(mergedEvents)-1].T + 50
	}
	for _, ev := range right.Events {
		copyEv := ev
		copyEv.T += offset
		mergedEvents = append(mergedEvents, copyEv)
	}
	merged := newRecordingClone(left, name, mergedEvents, true)
	if len(right.NetworkEvents) > 0 {
		merged.NetworkEvents = append(append([]NetworkRecordedEvent{}, left.NetworkEvents...), right.NetworkEvents...)
	}
	return merged
}

func itoa(v int) string {
	return fmt.Sprintf("%d", v)
}

func itoa64(v int64) string {
	return fmt.Sprintf("%d", v)
}

func meanFloat64(values []float64) float64 {
	if len(values) == 0 {
		return 0
	}
	sum := 0.0
	for _, v := range values {
		sum += v
	}
	return sum / float64(len(values))
}

func stdFloat64(values []float64, mean float64) float64 {
	if len(values) == 0 {
		return 0
	}
	sum := 0.0
	for _, v := range values {
		d := v - mean
		sum += d * d
	}
	return math.Sqrt(sum / float64(len(values)))
}
