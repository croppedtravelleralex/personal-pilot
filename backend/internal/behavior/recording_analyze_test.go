package behavior

import "testing"

func TestAnalyzeRecordingCadence(t *testing.T) {
	score := AnalyzeRecordingCadence([]RecordedEvent{
		{T: 0, Type: "move"},
		{T: 120, Type: "move"},
		{T: 340, Type: "scroll"},
		{T: 900, Type: "down"},
	})
	if score.Score < 50 {
		t.Fatalf("score too low: %+v", score)
	}
	if score.EventCount != 4 {
		t.Fatalf("event count = %d", score.EventCount)
	}
}
