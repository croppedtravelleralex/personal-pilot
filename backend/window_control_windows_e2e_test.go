//go:build windows
// +build windows

package backend

import (
	"os"
	"os/exec"
	"testing"
	"time"
)

func TestWindowControlE2EWithNotepad(t *testing.T) {
	if os.Getenv("PERSONAL_PILOT_WINDOW_CONTROL_E2E") != "1" {
		t.Skip("set PERSONAL_PILOT_WINDOW_CONTROL_E2E=1 to run visible Windows window-control E2E")
	}

	cmd := exec.Command("notepad.exe")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start notepad: %v", err)
	}
	t.Cleanup(func() {
		if cmd.Process != nil {
			_ = cmd.Process.Kill()
		}
	})

	deadline := time.Now().Add(5 * time.Second)
	for {
		if _, err := findExternalWindowByPID(cmd.Process.Pid); err == nil {
			break
		}
		if time.Now().After(deadline) {
			t.Skip("notepad did not expose a visible top-level window for its process")
		}
		time.Sleep(100 * time.Millisecond)
	}

	area, err := externalWindowWorkArea()
	if err != nil {
		t.Fatalf("work area: %v", err)
	}
	target := workbenchWindowRect{
		x:      area.x,
		y:      area.y,
		width:  maxInt(320, area.width/3),
		height: maxInt(240, area.height/3),
	}
	found, err := moveExternalWindowByPID(cmd.Process.Pid, target)
	if err != nil {
		t.Fatalf("move window: found=%v err=%v", found, err)
	}
	if !found {
		t.Fatal("expected to find notepad window")
	}
	if err := activateExternalWindowByPID(cmd.Process.Pid); err != nil {
		t.Fatalf("activate window: %v", err)
	}
}

func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}
