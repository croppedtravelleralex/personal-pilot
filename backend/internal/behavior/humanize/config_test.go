package humanize

import "testing"

func TestConfig_FromLevel_None(t *testing.T) {
	var c HumanizationConfig
	c.FromLevel(LevelNone)

	if c.Level != LevelNone {
		t.Fatalf("Level = %v, want LevelNone", c.Level)
	}
	if c.Timing.PreActionDelayMs != 0 {
		t.Fatalf("PreActionDelayMs = %d, want 0", c.Timing.PreActionDelayMs)
	}
	if c.Click.RadiusPx != 0 {
		t.Fatalf("RadiusPx = %d, want 0", c.Click.RadiusPx)
	}
	if c.Typing.BaseWPM != 9999 {
		t.Fatalf("BaseWPM = %d, want 9999", c.Typing.BaseWPM)
	}
	if c.Scroll.OvershootRatio != nil {
		t.Fatal("OvershootRatio should be nil for None")
	}
	if c.Failure.Type != FailureInstant {
		t.Fatalf("Failure.Type = %v, want FailureInstant", c.Failure.Type)
	}
}

func TestConfig_FromLevel_Medium(t *testing.T) {
	var c HumanizationConfig
	c.FromLevel(LevelMedium)

	if c.Level != LevelMedium {
		t.Fatalf("Level = %v, want LevelMedium", c.Level)
	}
	if c.Timing.PreActionDelayMs != 80 {
		t.Fatalf("PreActionDelayMs = %d, want 80", c.Timing.PreActionDelayMs)
	}
	if c.Click.RadiusPx != 8 {
		t.Fatalf("RadiusPx = %d, want 8", c.Click.RadiusPx)
	}
	if c.Typing.BaseWPM != 40 {
		t.Fatalf("BaseWPM = %d, want 40", c.Typing.BaseWPM)
	}
	if c.Scroll.OvershootRatio == nil {
		t.Fatal("OvershootRatio should not be nil for Medium")
	}
	if *c.Scroll.OvershootRatio != 0.35 {
		t.Fatalf("OvershootRatio = %f, want 0.35", *c.Scroll.OvershootRatio)
	}
	if c.Failure.Type != FailureHuman {
		t.Fatalf("Failure.Type = %v, want FailureHuman", c.Failure.Type)
	}
}

func TestConfig_FromLevel_High(t *testing.T) {
	var c HumanizationConfig
	c.FromLevel(LevelHigh)

	if c.Timing.PreActionDelayMs != 150 {
		t.Fatalf("PreActionDelayMs = %d, want 150", c.Timing.PreActionDelayMs)
	}
	if c.Failure.MaxRetries != 2 {
		t.Fatalf("MaxRetries = %d, want 2 (high level gives up faster)", c.Failure.MaxRetries)
	}
	if c.Failure.GiveUpChance != 0.25 {
		t.Fatalf("GiveUpChance = %f, want 0.25", c.Failure.GiveUpChance)
	}
}

func TestDefaultConfig(t *testing.T) {
	c := DefaultConfig()
	if c.Level != LevelMedium {
		t.Fatalf("DefaultConfig Level = %v, want LevelMedium", c.Level)
	}
}

func TestLevel_IsActive(t *testing.T) {
	if LevelNone.IsActive() {
		t.Fatal("LevelNone.IsActive() should be false")
	}
	if !LevelMedium.IsActive() {
		t.Fatal("LevelMedium.IsActive() should be true")
	}
}
