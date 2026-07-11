package behavior

import "testing"

func TestBridgeWorkflowSteps(t *testing.T) {
	steps := BridgeWorkflowSteps(&Recording{
		StartURL: "https://example.com",
		Events: []RecordedEvent{
			{T: 0, Type: "down", TargetPath: "button"},
			{T: 50, Type: "key", Text: "hello"},
		},
	})
	if len(steps) < 2 {
		t.Fatalf("steps=%d", len(steps))
	}
}

func TestDiffAndMergeRecordings(t *testing.T) {
	left := &Recording{ID: "l", Events: []RecordedEvent{{T: 0, Type: "move"}}, DurationMs: 100}
	right := &Recording{ID: "r", Events: []RecordedEvent{{T: 0, Type: "scroll"}}, DurationMs: 200}
	diff := DiffRecordings(left, right)
	if diff == nil || diff.EventDelta != 0 {
		t.Fatalf("diff=%+v", diff)
	}
	merged := MergeRecordings(left, right, "merged")
	if merged == nil || len(merged.Events) != 2 {
		t.Fatalf("merged events=%d", len(merged.Events))
	}
}
