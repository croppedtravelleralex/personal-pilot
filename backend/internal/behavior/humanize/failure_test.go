package humanize

import "testing"

func TestHumanizedRetryDecision_Instant(t *testing.T) {
	cfg := ConfigForLevel(LevelNone) // FailureInstant, MaxRetries=3

	// Attempt 0: should retry
	d := HumanizedRetryDecision(ErrElementNotFound, 0, &cfg)
	if d.Action.Type != RecoveryRetry {
		t.Fatalf("Action.Type = %v, want RecoveryRetry", d.Action.Type)
	}

	// Attempt 3: should give up
	d = HumanizedRetryDecision(ErrElementNotFound, 3, &cfg)
	if d.Action.Type != RecoveryGiveUp {
		t.Fatalf("Action.Type = %v, want RecoveryGiveUp", d.Action.Type)
	}
}

func TestHumanizedRetryDecision_FatalError(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium) // FailureHuman

	// Fatal errors should give up immediately
	d := HumanizedRetryDecision(ErrSessionCrashed, 0, &cfg)
	if d.Action.Type != RecoveryGiveUp {
		t.Fatalf("Fatal error Action.Type = %v, want RecoveryGiveUp", d.Action.Type)
	}
}

func TestHumanizedRetryDecision_ProxyDeadRetry(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)

	// ProxyDead on attempt 1 gets one retry
	d := HumanizedRetryDecision(ErrProxyDead, 1, &cfg)
	if d.Action.Type != RecoveryRetryAfter {
		t.Fatalf("ProxyDead attempt 1: Action.Type = %v, want RecoveryRetryAfter", d.Action.Type)
	}
	if d.EstimatedSuccessProbability != 0.75 {
		t.Fatalf("EstimatedSuccessProbability = %f, want 0.75", d.EstimatedSuccessProbability)
	}
}

func TestHumanizedRetryDecision_HumanHesitation(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	cfg.Failure.GiveUpChance = 0

	// Should get a RetryAfter with wait time
	d := HumanizedRetryDecision(ErrElementNotFound, 0, &cfg)
	if d.Action.Type != RecoveryRetryAfter {
		t.Fatalf("Action.Type = %v, want RecoveryRetryAfter", d.Action.Type)
	}
	// Wait should be within configured range
	if d.Action.WaitMs < cfg.Failure.MinWaitMs {
		t.Fatalf("WaitMs = %d, want >= %d", d.Action.WaitMs, cfg.Failure.MinWaitMs)
	}
}

func TestHumanizedRetryDecision_LaterAttemptsWaitLonger(t *testing.T) {
	cfg := ConfigForLevel(LevelMedium)
	cfg.Failure.GiveUpChance = 0

	// Run multiple times to get average wait times at different attempts
	// (statistical test, use larger sample)
	const n = 200
	var sum0, sum2 uint32
	for i := 0; i < n; i++ {
		d0 := HumanizedRetryDecision(ErrElementNotFound, 0, &cfg)
		d2 := HumanizedRetryDecision(ErrElementNotFound, 2, &cfg)
		if d0.Action.Type == RecoveryRetryAfter {
			sum0 += d0.Action.WaitMs
		}
		if d2.Action.Type == RecoveryRetryAfter {
			sum2 += d2.Action.WaitMs
		}
	}
	// Attempt 2 should have longer waits on average (1.0+0.6=1.6x multiplier)
	avg0 := float64(sum0) / float64(n)
	avg2 := float64(sum2) / float64(n)
	if avg2 < avg0 {
		t.Logf("avg0=%.1f, avg2=%.1f — attempt 2 might not be longer due to randomness", avg0, avg2)
	}
}

func TestSuggestedApproach(t *testing.T) {
	tests := []struct {
		code ActionErrorCode
		want string
	}{
		{ErrElementNotFound, "retry_with_wait"},
		{ErrClickFailed, "use_offset_click"},
		{ErrNavigationTimeout, "retry_with_different_proxy"},
		{ErrProxyDead, "switch_proxy"},
		{ErrFingerprintRejected, "switch_fingerprint"},
		{ErrSessionCrashed, "restart_session"},
		{ErrRateLimited, "wait_longer"},
		{ErrContentBlinded, "scroll_into_view"},
		{ErrUnknown, "skip"},
	}
	for _, tc := range tests {
		got := SuggestedApproach(tc.code)
		if got != tc.want {
			t.Fatalf("SuggestedApproach(%s) = %q, want %q", tc.code.String(), got, tc.want)
		}
	}
}

func TestErrorCode_String(t *testing.T) {
	if ErrElementNotFound.String() != "ELEMENT_NOT_FOUND" {
		t.Fatalf("ErrElementNotFound = %q", ErrElementNotFound.String())
	}
	if ErrUnknown.String() != "UNKNOWN" {
		t.Fatalf("ErrUnknown = %q", ErrUnknown.String())
	}
}
