//! Humanized failure recovery — how a "human" would handle action failures

use rand::Rng;

use super::config::{FailureStyle, HumanizationConfig};

/// Error codes that can occur during browser action execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ActionErrorCode {
    /// Target element not found on the page
    ElementNotFound,
    /// Click action failed
    ClickFailed,
    /// Navigation timed out
    NavigationTimeout,
    /// Proxy is unresponsive
    ProxyDead,
    /// Fingerprint was detected / rejected
    FingerprintRejected,
    /// Browser session crashed
    SessionCrashed,
    /// Rate limited by target site
    RateLimited,
    /// Content visibility blocked
    ContentBlinded,
    /// Unknown / unexpected error
    Unknown,
}

impl ActionErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionErrorCode::ElementNotFound => "ELEMENT_NOT_FOUND",
            ActionErrorCode::ClickFailed => "CLICK_FAILED",
            ActionErrorCode::NavigationTimeout => "NAVIGATION_TIMEOUT",
            ActionErrorCode::ProxyDead => "PROXY_DEAD",
            ActionErrorCode::FingerprintRejected => "FINGERPRINT_REJECTED",
            ActionErrorCode::SessionCrashed => "SESSION_CRASHED",
            ActionErrorCode::RateLimited => "RATE_LIMITED",
            ActionErrorCode::ContentBlinded => "CONTENT_BLINDED",
            ActionErrorCode::Unknown => "UNKNOWN",
        }
    }
}

/// Recovery action the system should take
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    /// Retry immediately
    Retry,
    /// Wait then retry
    RetryAfter { wait_ms: u32 },
    /// Give up on this action
    GiveUp,
    /// Switch to a different approach (e.g., different selector)
    SwitchApproach,
    /// Skip this action and continue
    Skip,
}

/// Decision on whether and how to retry after a failure
#[derive(Debug, Clone)]
pub struct RetryDecision {
    pub action: RecoveryAction,
    /// Probability that this decision was correct (for logging/analytics)
    pub estimated_success_probability: f32,
}

impl RetryDecision {
    pub fn retry() -> Self {
        Self {
            action: RecoveryAction::Retry,
            estimated_success_probability: 0.6,
        }
    }

    pub fn retry_after(wait_ms: u32) -> Self {
        Self {
            action: RecoveryAction::RetryAfter { wait_ms },
            estimated_success_probability: 0.75,
        }
    }

    pub fn give_up() -> Self {
        Self {
            action: RecoveryAction::GiveUp,
            estimated_success_probability: 0.0,
        }
    }

    pub fn switch_approach() -> Self {
        Self {
            action: RecoveryAction::SwitchApproach,
            estimated_success_probability: 0.5,
        }
    }

    pub fn skip() -> Self {
        Self {
            action: RecoveryAction::Skip,
            estimated_success_probability: 0.0,
        }
    }
}

/// Humanized retry decision based on error type, attempt number, and config
pub fn humanized_retry_decision(
    error: ActionErrorCode,
    attempt: u32,
    config: &HumanizationConfig,
) -> RetryDecision {
    let style = &config.failure;

    match style {
        FailureStyle::Instant => {
            // Machine-style: always retry up to limit
            if attempt < 3 {
                RetryDecision::retry()
            } else {
                RetryDecision::give_up()
            }
        }
        FailureStyle::Human {
            min_wait_ms,
            max_wait_ms,
            max_retries,
            give_up_chance,
        } => {
            let mut rng = rand::thread_rng();

            // Give up immediately on certain fatal errors
            if matches!(
                error,
                ActionErrorCode::SessionCrashed
                    | ActionErrorCode::FingerprintRejected
                    | ActionErrorCode::ProxyDead
            ) {
                if attempt >= 1 {
                    // Fatal errors: give up after 1 attempt (or occasionally retry proxy once)
                    if error == ActionErrorCode::ProxyDead && attempt == 1 {
                        let wait = rng.gen_range(*min_wait_ms..=*max_wait_ms);
                        return RetryDecision::retry_after(wait);
                    }
                    return RetryDecision::give_up();
                }
            }

            // Already exhausted retries
            if attempt >= *max_retries {
                return RetryDecision::give_up();
            }

            // Give up with some probability even before max retries
            if rng.gen::<f32>() < *give_up_chance {
                return RetryDecision::give_up();
            }

            // Human hesitation: wait before retry
            let wait_ms = rng.gen_range(*min_wait_ms..=*max_wait_ms);

            // More attempts = longer waits (less motivated to keep trying)
            let attempts_multiplier = 1.0 + (attempt as f32 * 0.3);
            let adjusted_wait = (wait_ms as f32 * attempts_multiplier) as u32;

            RetryDecision::retry_after(adjusted_wait)
        }
    }
}

/// Get suggested alternative approach for a given error code
pub fn suggested_approach(error: ActionErrorCode) -> &'static str {
    match error {
        ActionErrorCode::ElementNotFound => "retry_with_wait",
        ActionErrorCode::ClickFailed => "use_offset_click",
        ActionErrorCode::NavigationTimeout => "retry_with_different_proxy",
        ActionErrorCode::ProxyDead => "switch_proxy",
        ActionErrorCode::FingerprintRejected => "switch_fingerprint",
        ActionErrorCode::SessionCrashed => "restart_session",
        ActionErrorCode::RateLimited => "wait_longer",
        ActionErrorCode::ContentBlinded => "scroll_into_view",
        ActionErrorCode::Unknown => "skip",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn medium_config() -> HumanizationConfig {
        HumanizationConfig::from_level(super::super::config::HumanizationLevel::Medium)
    }

    #[test]
    fn test_fatal_error_gives_up_quickly() {
        let config = medium_config();
        let decision = humanized_retry_decision(ActionErrorCode::SessionCrashed, 1, &config);
        assert!(matches!(decision.action, RecoveryAction::GiveUp));
    }

    #[test]
    fn test_non_fatal_error_retries() {
        let config = medium_config();
        let decision = humanized_retry_decision(ActionErrorCode::ElementNotFound, 0, &config);
        assert!(matches!(
            decision.action,
            RecoveryAction::RetryAfter { .. } | RecoveryAction::Retry
        ));
    }

    #[test]
    fn test_give_up_chance_respected() {
        let config = HumanizationConfig::from_level(super::super::config::HumanizationLevel::High);
        let give_up_chance = match &config.failure {
            FailureStyle::Human { give_up_chance, .. } => *give_up_chance,
            _ => panic!("expected Human style"),
        };
        assert!(
            give_up_chance > 0.1,
            "High level should have meaningful give_up_chance"
        );
    }
}
