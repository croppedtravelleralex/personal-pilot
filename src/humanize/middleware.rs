//! BehavioralMutationMiddleware — assembles all mutation dimensions into a single trait
//!
//! The middleware sits between the action planner (template engine / LLM) and the
//! browser executor (CDP calls via lightpanda). It receives a planned action, mutates
//! it according to the HumanizationConfig, and passes the mutated version downstream.

use rand::Rng;
use serde::{Deserialize, Serialize};

use super::config::HumanizationConfig;
use super::failure::{ActionErrorCode, RetryDecision, humanized_retry_decision, suggested_approach};
use super::scroll::{ScrollPlan, build_scroll_plan};
use super::timing::compute_action_gap;
use super::trajectory::{ClickTarget, compute_click_target};
use super::typing::{TypingPlan, build_typing_plan};

/// All possible action types that can be planned by the template engine / LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum LlmAction {
    /// Start a new browser session
    OpenBrowser { url: Option<String> },
    /// Navigate to a URL
    Goto { url: String },
    /// Click an element
    Click {
        selector: String,
        offset: Option<OffsetOverride>,
    },
    /// Type text into an element
    Type {
        selector: String,
        text: String,
    },
    /// Wait for a duration
    Wait { duration_ms: u32 },
    /// Take a screenshot
    Screenshot { full_page: bool },
    /// Get page HTML
    GetHtml { selector: Option<String> },
    /// Get element text
    GetText { selector: String },
    /// Scroll the page
    Scroll {
        direction: ScrollDirection,
        distance_px: Option<u32>,
        target_selector: Option<String>,
    },
    /// Execute arbitrary JavaScript
    ExecuteJs { script: String },
    /// Close the browser session
    CloseBrowser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    ToElement,
}

/// Override for click offset (from action params or LLM)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetOverride {
    pub dx: i32,
    pub dy: i32,
}

/// The mutated action ready for execution
#[derive(Debug, Clone)]
pub enum MutatedAction {
    OpenBrowser {
        url: Option<String>,
        pre_gap_ms: u32,
    },
    Goto {
        url: String,
        pre_gap_ms: u32,
    },
    Click {
        selector: String,
        target: ClickTarget,
        pre_gap_ms: u32,
    },
    Type {
        selector: String,
        plan: TypingPlan,
        pre_gap_ms: u32,
    },
    Wait {
        duration_ms: u32,
        jitter_ms: u32,
    },
    Screenshot {
        full_page: bool,
        pre_gap_ms: u32,
    },
    GetHtml {
        selector: Option<String>,
        pre_gap_ms: u32,
    },
    GetText {
        selector: String,
        pre_gap_ms: u32,
    },
    Scroll {
        plan: ScrollPlan,
        pre_gap_ms: u32,
    },
    ExecuteJs {
        script: String,
        pre_gap_ms: u32,
    },
    CloseBrowser {
        pre_gap_ms: u32,
    },
}

/// Action execution result from the browser executor
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub success: bool,
    pub error_code: Option<ActionErrorCode>,
    pub error_message: Option<String>,
    /// Actual selector that succeeded (may differ from planned selector)
    pub resolved_selector: Option<String>,
    /// Elements affected by this action
    pub affected_selectors: Vec<String>,
}

impl ActionResult {
    pub fn success() -> Self {
        Self {
            success: true,
            error_code: None,
            error_message: None,
            resolved_selector: None,
            affected_selectors: vec![],
        }
    }

    pub fn failure(code: ActionErrorCode, message: impl Into<String>) -> Self {
        Self {
            success: false,
            error_code: Some(code),
            error_message: Some(message.into()),
            resolved_selector: None,
            affected_selectors: vec![],
        }
    }
}

/// The main behavioral mutation middleware.
///
/// Usage:
/// ```ignore
/// let middleware = BehavioralMutationMiddleware::new(profile.humanization_level);
/// let mutated = middleware.mutate(llm_action, &element_info);
/// let result = browser_executor.execute(mutated).await;
/// let retry = middleware.decide_retry(&result, attempt_number);
/// ```
pub struct BehavioralMutationMiddleware {
    config: HumanizationConfig,
}

impl BehavioralMutationMiddleware {
    pub fn new(config: HumanizationConfig) -> Self {
        Self { config }
    }

    /// Mutate a planned LLM action into an execution-ready MutatedAction
    pub fn mutate(
        &self,
        action: LlmAction,
        element_info: Option<&ElementBounds>,
    ) -> MutatedAction {
        // Compute the pre-action gap (human "decision" delay)
        let pre_gap_ms = compute_action_gap(&self.config);

        match action {
            LlmAction::OpenBrowser { url } => {
                MutatedAction::OpenBrowser { url, pre_gap_ms }
            }
            LlmAction::Goto { url } => {
                MutatedAction::Goto { url, pre_gap_ms }
            }
            LlmAction::Click { selector, offset } => {
                let target = self.mutate_click(selector.clone(), offset, element_info);
                MutatedAction::Click {
                    selector,
                    target,
                    pre_gap_ms,
                }
            }
            LlmAction::Type { selector, text } => {
                let plan = build_typing_plan(&text, &self.config);
                MutatedAction::Type {
                    selector,
                    plan,
                    pre_gap_ms,
                }
            }
            LlmAction::Wait { duration_ms } => {
                // Wait already has built-in duration; add jitter
                let jitter = if self.config.level.is_active() {
                    let jitter_pct = (duration_ms as f32 * 0.2).round() as u32;
                    rand::thread_rng().gen_range(0..=jitter_pct)
                } else {
                    0
                };
                MutatedAction::Wait {
                    duration_ms,
                    jitter_ms: jitter,
                }
            }
            LlmAction::Screenshot { full_page } => {
                MutatedAction::Screenshot { full_page, pre_gap_ms }
            }
            LlmAction::GetHtml { selector } => {
                MutatedAction::GetHtml { selector, pre_gap_ms }
            }
            LlmAction::GetText { selector } => {
                MutatedAction::GetText { selector, pre_gap_ms }
            }
            LlmAction::Scroll { direction, distance_px, target_selector } => {
                let plan = self.mutate_scroll(direction, distance_px, target_selector);
                MutatedAction::Scroll { plan, pre_gap_ms }
            }
            LlmAction::ExecuteJs { script } => {
                MutatedAction::ExecuteJs { script, pre_gap_ms }
            }
            LlmAction::CloseBrowser => {
                MutatedAction::CloseBrowser { pre_gap_ms }
            }
        }
    }

    /// Mutate a click action
    fn mutate_click(
        &self,
        _selector: String,
        offset: Option<OffsetOverride>,
        element_info: Option<&ElementBounds>,
    ) -> ClickTarget {
        match element_info {
            Some(el) => {
                // Use element's actual bounding box
                compute_click_target(
                    (el.x1 + el.x2) as i32 / 2,
                    (el.y1 + el.y2) as i32 / 2,
                    (el.x2 - el.x1) as u32,
                    (el.y2 - el.y1) as u32,
                    &self.config,
                )
            }
            None => {
                // No element info: use explicit offset or default
                if let Some(off) = offset {
                    ClickTarget {
                        x: off.dx,
                        y: off.dy,
                        hover_before_ms: self.config.scroll.hover_before_scroll_ms,
                        trajectory: vec![],
                    }
                } else {
                    // Dead center, no offset
                    ClickTarget {
                        x: 0,
                        y: 0,
                        hover_before_ms: None,
                        trajectory: vec![],
                    }
                }
            }
        }
    }

    /// Mutate a scroll action
    fn mutate_scroll(
        &self,
        direction: ScrollDirection,
        distance_px: Option<u32>,
        _target_selector: Option<String>,
    ) -> ScrollPlan {
        match direction {
            ScrollDirection::ToElement => {
                // For scrolling to element, we'd need element Y position
                // This would be resolved at execution time
                build_scroll_plan(distance_px.unwrap_or(300), &self.config)
            }
            ScrollDirection::Up | ScrollDirection::Down => {
                let distance = distance_px.unwrap_or(300);
                build_scroll_plan(distance, &self.config)
            }
        }
    }

    /// Decide whether and how to retry after a failed action result
    pub fn decide_retry(&self, result: &ActionResult, attempt: u32) -> RetryDecision {
        let error = result.error_code.unwrap_or(ActionErrorCode::Unknown);
        humanized_retry_decision(error, attempt, &self.config)
    }

    /// Get suggested approach for a given error code
    pub fn suggest_approach(&self, error: ActionErrorCode) -> &'static str {
        suggested_approach(error)
    }
}

/// Element bounding box information (from CDP)
#[derive(Debug, Clone)]
pub struct ElementBounds {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub width: u32,
    pub height: u32,
}

impl ElementBounds {
    pub fn center_x(&self) -> i32 {
        (self.x1 + self.x2) / 2
    }

    pub fn center_y(&self) -> i32 {
        (self.y1 + self.y2) / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn medium_config() -> HumanizationConfig {
        HumanizationConfig::from_level(super::super::config::HumanizationLevel::Medium)
    }

    #[test]
    fn test_wait_action_has_jitter() {
        let mw = BehavioralMutationMiddleware::new(medium_config());
        let mutated = mw.mutate(LlmAction::Wait { duration_ms: 1000 }, None);

        if let MutatedAction::Wait { duration_ms: _, jitter_ms } = mutated {
            assert!(jitter_ms > 0, "jitter should be > 0 for Medium level");
        } else {
            panic!("expected Wait variant");
        }
    }

    #[test]
    fn test_none_level_no_mutation() {
        let config = HumanizationConfig::from_level(super::super::config::HumanizationLevel::None);
        let mw = BehavioralMutationMiddleware::new(config);
        let mutated = mw.mutate(LlmAction::Wait { duration_ms: 500 }, None);

        if let MutatedAction::Wait { duration_ms, jitter_ms } = mutated {
            assert_eq!(jitter_ms, 0, "no jitter at None level");
        } else {
            panic!("expected Wait");
        }
    }

    #[test]
    fn test_retry_decision_on_failure() {
        let mw = BehavioralMutationMiddleware::new(medium_config());
        let result = ActionResult::failure(ActionErrorCode::ElementNotFound, "element gone");
        let decision = mw.decide_retry(&result, 0);
        assert!(matches!(
            decision.action,
            RecoveryAction::RetryAfter { .. } | RecoveryAction::Retry
        ));
    }
}
