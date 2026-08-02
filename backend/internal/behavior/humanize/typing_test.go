package humanize

import "testing"

func TestBuildTypingPlan_Empty(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	plan := BuildTypingPlan("", &cfg)
	if len(plan.Events) != 0 {
		t.Fatalf("Events = %d, want 0 for empty string", len(plan.Events))
	}
	if plan.TotalMs != 0 {
		t.Fatalf("TotalMs = %d, want 0", plan.TotalMs)
	}
}

func TestBuildTypingPlan_Short(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	plan := BuildTypingPlan("hi", &cfg)
	if len(plan.Events) == 0 {
		t.Fatal("Events should not be empty for 'hi'")
	}
	if plan.TotalMs == 0 {
		t.Fatal("TotalMs should be > 0 for 'hi'")
	}
}

func TestBuildTypingPlan_LongText(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	text := "Hello world this is a test"
	plan := BuildTypingPlan(text, &cfg)

	// Count key events
	keyCount := 0
	for _, ev := range plan.Events {
		if ev.Type == TypingEventKey {
			keyCount++
		}
	}
	// Should have at least len(text) keys (more if typos)
	if keyCount < len(text) {
		t.Fatalf("Key count = %d, want >= %d", keyCount, len(text))
	}
	// TotalMs should be reasonable for 30 chars at ~40 WPM
	// 30/40 * 60000 = 45000ms minimum, but can be more due to pauses/typos
	if plan.TotalMs < 20000 {
		t.Fatalf("TotalMs = %d, want >= 20000", plan.TotalMs)
	}
}

func TestCharInterval(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	for i := 0; i < 20; i++ {
		interval := CharInterval(&cfg, 'a')
		if interval < 500 || interval > 3500 {
			t.Fatalf("CharInterval = %d, want [500, 3500]", interval)
		}
	}
}

func TestCharInterval_UppercasePenalty(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	cfg.Typing.SpeedVariancePercent = 0

	lower := CharInterval(&cfg, 'a')
	upper := CharInterval(&cfg, 'A')
	if upper != lower+25 {
		t.Fatalf("Uppercase interval = %d, want lowercase+25 (%d)", upper, lower+25)
	}
}

func TestTypingP1Plans(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	password := BuildContextTypingPlan("secret", TypingContext{FieldKind: "password", MinutesElapsed: 2}, &cfg)
	normal := BuildContextTypingPlan("secret", TypingContext{}, &cfg)
	if password.TotalMs <= normal.TotalMs {
		t.Fatalf("password TotalMs = %d, want > normal %d", password.TotalMs, normal.TotalMs)
	}
	captcha := BuildContextTypingPlan("1234", TypingContext{FieldKind: "captcha", CaptchaCellCount: 4}, &cfg)
	pauses := 0
	for _, event := range captcha.Events {
		if event.Type == TypingEventPause {
			pauses++
		}
	}
	if pauses == 0 {
		t.Fatal("captcha plan should include cell pauses")
	}
	ime := BuildIMEPlan("nihao", 2, &cfg)
	if ime.Events[len(ime.Events)-1].Action != "candidate_enter" {
		t.Fatalf("last IME action = %q", ime.Events[len(ime.Events)-1].Action)
	}
	clipboard := BuildClipboardPlan(&cfg)
	if len(clipboard.Events) != 9 {
		t.Fatalf("clipboard events = %d, want 9", len(clipboard.Events))
	}
	tabs := BuildTabSwitchPlan(3)
	if len(tabs.Events) != 3 || tabs.TotalMs == 0 {
		t.Fatalf("tab plan = %+v", tabs)
	}
	if FingerSpeedRatio('a') >= FingerSpeedRatio('f') {
		t.Fatal("pinky keys should be slower than index keys")
	}
}
