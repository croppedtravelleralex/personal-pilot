package workflow

import "time"

type RetryPolicy struct {
	MaxAttempts uint32
	Backoff     time.Duration
}

func NextRetryDelay(policy RetryPolicy, attempt uint32) (time.Duration, bool) {
	if policy.MaxAttempts == 0 || attempt >= policy.MaxAttempts {
		return 0, false
	}
	if policy.Backoff <= 0 {
		return 250 * time.Millisecond, true
	}
	return policy.Backoff * time.Duration(attempt+1), true
}
