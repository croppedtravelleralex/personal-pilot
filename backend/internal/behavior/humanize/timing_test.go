package humanize

import "testing"

func TestComputeActionGap_None(t *testing.T) {
	cfg := ConfigForLevel(LevelNone)
	gap := ComputeActionGap(&cfg)
	// Level None: pre-action delay is 0, uniform [0,0] returns 0
	if gap != 0 {
		t.Fatalf("ActionGap = %d, want 0 for LevelNone", gap)
	}
}

func TestComputeActionGap_Active(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	// Run 100 times to make sure we don't crash and get reasonable values
	for i := 0; i < 100; i++ {
		gap := ComputeActionGap(&cfg)
		// Pre-action delay (~80ms with ±33% jitter) + right-skewed sample (150-2000ms)
		if gap < 50 {
			t.Fatalf("ActionGap = %d, expected at least 50ms for LevelMedium", gap)
		}
		if gap > 3000 {
			t.Fatalf("ActionGap = %d, expected at most ~3000ms for LevelMedium", gap)
		}
	}
}

func TestPreActionDelay_Zero(t *testing.T) {
	cfg := ConfigForLevel(LevelNone)
	d := PreActionDelay(&cfg)
	if d != 0 {
		t.Fatalf("PreActionDelay = %d, want 0", d)
	}
}

func TestPreActionDelay_Jitter(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	// With base=80ms and ±33% jitter, values should be in [53, 106] approx
	base := cfg.Timing.PreActionDelayMs
	for i := 0; i < 100; i++ {
		d := PreActionDelay(&cfg)
		// Allow wider range due to integer truncation
		if d > base+base/2 {
			t.Fatalf("PreActionDelay = %d, jitter too high (base=%d)", d, base)
		}
	}
}

func TestComputeTypingInterval(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	// 40 WPM base
	for i := 0; i < 50; i++ {
		interval := ComputeTypingInterval(&cfg, 'a')
		if interval < 500 || interval > 3500 {
			t.Fatalf("TypingInterval for 'a' = %d, want [500, 3500]", interval)
		}
	}
}

func TestSampleNormal_Approximation(t *testing.T) {
	// Test that normal samples are centered around the mean
	var sum uint32
	const n = 1000
	for i := 0; i < n; i++ {
		sum += sampleNormal(500, 100, 500)
	}
	avg := float64(sum) / float64(n)
	// Should be roughly near 500 (allow wide tolerance for randomness)
	if avg < 350 || avg > 650 {
		t.Fatalf("Normal sample average = %.1f, want ~500", avg)
	}
}

func TestSampleRightSkewed(t *testing.T) {
	var sum uint32
	const n = 1000
	for i := 0; i < n; i++ {
		sum += sampleRightSkewed(100, 300, 1500)
	}
	avg := float64(sum) / float64(n)
	// Should cluster near mode (300), but mean of right-skewed is > mode
	if avg < 200 || avg > 600 {
		t.Fatalf("Right-skewed sample average = %.1f, want [200, 600]", avg)
	}
}
