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
	// With variance, uppercase might not always be slower, but on average it should
	var lowerSum, upperSum uint32
	const n = 500
	for i := 0; i < n; i++ {
		lowerSum += CharInterval(&cfg, 'a')
		upperSum += CharInterval(&cfg, 'A')
	}
	lowerAvg := float64(lowerSum) / float64(n)
	upperAvg := float64(upperSum) / float64(n)
	// Uppercase should be ~25ms slower on average
	if upperAvg < lowerAvg-10 {
		t.Fatalf("Uppercase avg (%.1f) should be >= lowercase avg (%.1f)", upperAvg, lowerAvg)
	}
}
