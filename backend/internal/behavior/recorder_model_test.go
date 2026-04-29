package behavior

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestRecordingInjectJSDoesNotPersistClickEvents(t *testing.T) {
	if strings.Contains(RecordingInjectJS, "addEventListener('click'") ||
		strings.Contains(RecordingInjectJS, `addEventListener("click"`) {
		t.Fatal("recording script must not persist click events; playback replays down/up atomics")
	}
	if !strings.Contains(RecordingInjectJS, "listen('mousedown'") ||
		!strings.Contains(RecordingInjectJS, "listen('mouseup'") {
		t.Fatal("recording script must keep mousedown/mouseup atomic events")
	}
}

func TestNormalizeRecordedEventsRedactsSensitiveText(t *testing.T) {
	events := normalizeRecordedEvents([]RecordedEvent{
		{T: 1, Type: "input", Text: "plain text"},
		{T: 2, Type: "input", Text: "secret-password", Sensitive: true},
	})

	if events[0].Text != "plain text" {
		t.Fatalf("non-sensitive text changed: got %q", events[0].Text)
	}
	if events[1].Text != "" {
		t.Fatalf("sensitive text must be redacted, got %q", events[1].Text)
	}
}

func TestNormalizeRecordedEventsDropsDerivedClickOnlyWhenAtomicPairExists(t *testing.T) {
	events := normalizeRecordedEvents([]RecordedEvent{
		{T: 10, Type: "down", X: 11, Y: 22, Button: 0},
		{T: 40, Type: "up", X: 11, Y: 22, Button: 0},
		{T: 45, Type: "click", X: 11, Y: 22, Button: 0},
		{T: 100, Type: "click", X: 50, Y: 60, Button: 0},
	})

	if len(events) != 3 {
		t.Fatalf("got %d events, want 3", len(events))
	}
	if events[0].Type != "down" || events[1].Type != "up" || events[2].Type != "click" {
		t.Fatalf("unexpected event sequence after normalization: %#v", events)
	}
}

func TestRecordingNewMetadataFieldsAreJSONBackwardCompatible(t *testing.T) {
	const oldJSON = `{
		"id":"rec-old",
		"name":"old recording",
		"description":"",
		"events":[{"t":1,"type":"down","x":10,"y":20,"btn":0}],
		"durationMs":1,
		"viewportW":800,
		"viewportH":600,
		"createdAt":"2026-01-01T00:00:00Z"
	}`

	var old Recording
	if err := json.Unmarshal([]byte(oldJSON), &old); err != nil {
		t.Fatalf("unmarshal old recording JSON: %v", err)
	}
	if old.StartURL != "" || old.CurrentURL != "" || old.Title != "" {
		t.Fatalf("old JSON should load with empty metadata defaults: %#v", old)
	}

	current := Recording{
		ID:               "rec-new",
		Name:             "new recording",
		Events:           []RecordedEvent{},
		DurationMs:       10,
		ViewportW:        1024,
		ViewportH:        768,
		StartURL:         "https://start.example/",
		CurrentURL:       "https://current.example/",
		Title:            "Current page",
		DevicePixelRatio: 1.25,
		Scale:            1,
		CreatedAt:        "2026-01-01T00:00:00Z",
	}

	data, err := json.Marshal(current)
	if err != nil {
		t.Fatalf("marshal new recording: %v", err)
	}
	for _, field := range []string{"startUrl", "currentUrl", "title", "devicePixelRatio", "scale"} {
		if !strings.Contains(string(data), `"`+field+`"`) {
			t.Fatalf("new recording JSON missing %s: %s", field, data)
		}
	}
}
