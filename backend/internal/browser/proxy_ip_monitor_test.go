package browser_test

import (
	"testing"
	"time"

	"personal-pilot/backend/internal/browser"
)

func TestProxyIPMonitorNoteExitIPAndDrift(t *testing.T) {
	var driftCalls int
	m := browser.NewProxyIPMonitor(60*time.Second, func(string, string) {}, func(string, string, string, string) {
		driftCalls++
	})
	old, changed := m.NoteExitIP("proxy-1", "1.2.3.4")
	if changed || old != "" {
		t.Fatalf("first note should not drift: old=%q changed=%v", old, changed)
	}
	old, changed = m.NoteExitIP("proxy-1", "5.6.7.8")
	if !changed || old != "1.2.3.4" {
		t.Fatalf("expected drift old=1.2.3.4 changed=true got old=%q changed=%v", old, changed)
	}
	if driftCalls != 1 {
		t.Fatalf("expected 1 drift callback, got %d", driftCalls)
	}
}
