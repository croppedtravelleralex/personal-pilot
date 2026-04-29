package behavior

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"
	"testing"
)

// TestAutoRecordNurturing runs a full auto-record session against a real browser
// and saves the result to the recording store directory.
func TestAutoRecordNurturing(t *testing.T) {
	debugPort := 59222
	url := fmt.Sprintf("http://127.0.0.1:%d/json/version", debugPort)
	resp, err := behaviorHTTPClient.Get(url)
	if err != nil {
		t.Skipf("Chrome not reachable on port %d: %v", debugPort, err)
		return
	}
	resp.Body.Close()

	name := fmt.Sprintf("养号录制 %s", "Xiaohongshu")
	recording, err := AutoRecord(debugPort, name, NurturingActions())
	if err != nil {
		t.Fatalf("AutoRecord failed: %v", err)
	}

	t.Logf("Recording: id=%s name=%s duration=%dms events=%d viewport=%dx%d",
		recording.ID, recording.Name, recording.DurationMs,
		len(recording.Events), recording.ViewportW, recording.ViewportH)

	// Count event types
	counts := map[string]int{}
	for _, evt := range recording.Events {
		counts[evt.Type]++
	}
	t.Logf("Event counts: %v", counts)

	// Save to file
	saveDir := "../../../data/recordings"
	if err := os.MkdirAll(saveDir, 0755); err != nil {
		t.Fatalf("create save dir: %v", err)
	}

	data, err := json.MarshalIndent(recording, "", "  ")
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	savePath := saveDir + "/" + recording.ID + ".json"
	if err := os.WriteFile(savePath, data, 0644); err != nil {
		t.Fatalf("save: %v", err)
	}

	t.Logf("Saved to %s", savePath)

	if len(recording.Events) < 5 {
		t.Errorf("Expected at least 5 events, got %d — mouseWheel events may not be captured", len(recording.Events))
	}
}

type failingAutoRecordWriter struct{}

func (f failingAutoRecordWriter) WriteJSON(v interface{}) error {
	return errors.New("write failed")
}

func TestAutoRecordWriteJSONReturnsError(t *testing.T) {
	err := writeAutoRecordJSON(failingAutoRecordWriter{}, "dispatch scroll wheel", map[string]interface{}{
		"id":     1,
		"method": "Input.dispatchMouseEvent",
	})
	if err == nil {
		t.Fatal("expected write error")
	}
	if !strings.Contains(err.Error(), "dispatch scroll wheel") || !strings.Contains(err.Error(), "write failed") {
		t.Fatalf("unexpected error: %v", err)
	}
}
