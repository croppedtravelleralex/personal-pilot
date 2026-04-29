package humanize

import "math/rand/v2"

// ActionErrorCode enumerates known failure modes for browser actions.
type ActionErrorCode int

const (
	ErrElementNotFound     ActionErrorCode = 0
	ErrClickFailed         ActionErrorCode = 1
	ErrNavigationTimeout   ActionErrorCode = 2
	ErrProxyDead           ActionErrorCode = 3
	ErrFingerprintRejected ActionErrorCode = 4
	ErrSessionCrashed      ActionErrorCode = 5
	ErrRateLimited         ActionErrorCode = 6
	ErrContentBlinded      ActionErrorCode = 7
	ErrUnknown             ActionErrorCode = 8
)

func (e ActionErrorCode) String() string {
	switch e {
	case ErrElementNotFound:
		return "ELEMENT_NOT_FOUND"
	case ErrClickFailed:
		return "CLICK_FAILED"
	case ErrNavigationTimeout:
		return "NAVIGATION_TIMEOUT"
	case ErrProxyDead:
		return "PROXY_DEAD"
	case ErrFingerprintRejected:
		return "FINGERPRINT_REJECTED"
	case ErrSessionCrashed:
		return "SESSION_CRASHED"
	case ErrRateLimited:
		return "RATE_LIMITED"
	case ErrContentBlinded:
		return "CONTENT_BLINDED"
	default:
		return "UNKNOWN"
	}
}

// RecoveryActionType enumerates what to do after a failure.
type RecoveryActionType int

const (
	RecoveryRetry          RecoveryActionType = 0
	RecoveryRetryAfter     RecoveryActionType = 1 // wait, then retry
	RecoveryGiveUp         RecoveryActionType = 2
	RecoverySwitchApproach RecoveryActionType = 3
	RecoverySkip           RecoveryActionType = 4
)

// RecoveryAction describes the recovery strategy after a failure.
type RecoveryAction struct {
	Type   RecoveryActionType
	WaitMs uint32 // only for RetryAfter
}

// RetryDecision combines a recovery action with an estimated success probability.
type RetryDecision struct {
	Action                      RecoveryAction
	EstimatedSuccessProbability float64
}

// Convenience constructors
func retryNow() RetryDecision {
	return RetryDecision{Action: RecoveryAction{Type: RecoveryRetry}, EstimatedSuccessProbability: 0.6}
}
func retryAfter(ms uint32) RetryDecision {
	return RetryDecision{Action: RecoveryAction{Type: RecoveryRetryAfter, WaitMs: ms}, EstimatedSuccessProbability: 0.75}
}
func giveUp() RetryDecision {
	return RetryDecision{Action: RecoveryAction{Type: RecoveryGiveUp}, EstimatedSuccessProbability: 0.0}
}
func switchApproach() RetryDecision {
	return RetryDecision{Action: RecoveryAction{Type: RecoverySwitchApproach}, EstimatedSuccessProbability: 0.5}
}
func skipRetry() RetryDecision {
	return RetryDecision{Action: RecoveryAction{Type: RecoverySkip}, EstimatedSuccessProbability: 0.0}
}

// HumanizedRetryDecision decides what to do after an action fails.
// Incorporates human-like hesitation, fatal error recognition, and early give-up.
func HumanizedRetryDecision(errCode ActionErrorCode, attempt uint32, config *HumanizationConfig) RetryDecision {
	switch config.Failure.Type {
	case FailureInstant:
		// Machine-like: always retry up to max retries
		if attempt < config.Failure.MaxRetries {
			return retryNow()
		}
		return giveUp()

	case FailureHuman:
		// 1. Fatal errors: give up immediately (with one exception for ProxyDead)
		if isFatal(errCode) {
			if errCode == ErrProxyDead && attempt <= 1 {
				wait := randRangeUint32(config.Failure.MinWaitMs, config.Failure.MaxWaitMs)
				return retryAfter(wait)
			}
			return giveUp()
		}

		// 2. Exhausted retries
		if attempt >= config.Failure.MaxRetries {
			return giveUp()
		}

		// 3. Random early give-up (humans sometimes just quit)
		if rand.Float64() < config.Failure.GiveUpChance {
			return giveUp()
		}

		// 4. Human hesitation: wait, then retry
		waitMs := randRangeUint32(config.Failure.MinWaitMs, config.Failure.MaxWaitMs)
		multiplier := 1.0 + float64(attempt)*0.3
		adjustedWait := uint32(float64(waitMs) * multiplier)
		return retryAfter(adjustedWait)

	default:
		return giveUp()
	}
}

// SuggestedApproach returns a human-readable suggestion for how to recover from an error.
func SuggestedApproach(errCode ActionErrorCode) string {
	switch errCode {
	case ErrElementNotFound:
		return "retry_with_wait"
	case ErrClickFailed:
		return "use_offset_click"
	case ErrNavigationTimeout:
		return "retry_with_different_proxy"
	case ErrProxyDead:
		return "switch_proxy"
	case ErrFingerprintRejected:
		return "switch_fingerprint"
	case ErrSessionCrashed:
		return "restart_session"
	case ErrRateLimited:
		return "wait_longer"
	case ErrContentBlinded:
		return "scroll_into_view"
	default:
		return "skip"
	}
}

func isFatal(errCode ActionErrorCode) bool {
	switch errCode {
	case ErrSessionCrashed, ErrFingerprintRejected, ErrProxyDead:
		return true
	default:
		return false
	}
}
