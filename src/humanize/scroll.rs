//! Scroll mutation — human-like scroll paths (overshoot + return, not instant jumps)

use rand::Rng;

use super::config::{HumanizationConfig, ScrollSpeed};

/// A single scroll step in the mutated plan
#[derive(Debug, Clone)]
pub enum ScrollStep {
    /// Move scroll by a delta (positive = down, negative = up)
    ScrollBy { delta_px: i32, speed: ScrollSpeed },
    /// Pause at current position (like human reading)
    Pause { duration_ms: u32 },
    /// Move to absolute scroll position
    ScrollTo { y: i32, speed: ScrollSpeed },
}

/// Full scroll plan for a target scroll operation
#[derive(Debug, Clone)]
pub struct ScrollPlan {
    pub steps: Vec<ScrollStep>,
    /// Total scroll distance (px)
    pub total_distance_px: u32,
    /// Total time for full scroll (ms)
    pub total_ms: u32,
}

/// Build a humanized scroll plan toward a target distance/position
pub fn build_scroll_plan(target_distance_px: u32, config: &HumanizationConfig) -> ScrollPlan {
    let scroll = &config.scroll;

    // No humanization — instant scroll
    if config.level == super::config::HumanizationLevel::None || scroll.overshoot_ratio.is_none() {
        return ScrollPlan {
            steps: vec![ScrollStep::ScrollBy {
                delta_px: target_distance_px as i32,
                speed: ScrollSpeed::Fast,
            }],
            total_distance_px: target_distance_px,
            total_ms: 0,
        };
    }

    let ratio = scroll.overshoot_ratio.unwrap_or(0.0);
    let mut rng = rand::thread_rng();

    // Human scroll: go past the target, pause, then scroll back
    let overshoot_px = ((target_distance_px as f32) * ratio) as u32;
    let total_distance = target_distance_px + overshoot_px * 2;
    let speed = scroll.speed;

    let mut steps = Vec::new();
    let mut total_ms: u32 = 0;

    // Hover before scrolling (like human positioning mouse first)
    if let Some(hover_ms) = scroll.hover_before_scroll_ms {
        let jitter = rng.gen_range(0..=30);
        steps.push(ScrollStep::Pause {
            duration_ms: hover_ms + jitter,
        });
        total_ms += hover_ms + jitter;
    }

    // Phase 1: scroll past target (overshoot)
    let overshoot_step_ms = scroll_step_duration(overshoot_px as i32, speed);
    steps.push(ScrollStep::ScrollBy {
        delta_px: overshoot_px as i32,
        speed,
    });
    total_ms += overshoot_step_ms;

    // Phase 2: pause at overshoot (reading / orienting)
    let pause1 = rng.gen_range(200..=450);
    steps.push(ScrollStep::Pause {
        duration_ms: pause1,
    });
    total_ms += pause1;

    // Phase 3: scroll back to real target
    let back_step_ms = scroll_step_duration(overshoot_px as i32, ScrollSpeed::Slow);
    steps.push(ScrollStep::ScrollBy {
        delta_px: -(overshoot_px as i32),
        speed: ScrollSpeed::Slow,
    });
    total_ms += back_step_ms;

    // Phase 4: micro-adjustments (humans rarely scroll exactly right first time)
    let micro_adjust: i32 = rng.gen_range(-30..=30);
    if micro_adjust.abs() > 5 {
        let adjust_ms = scroll_step_duration(micro_adjust.abs(), ScrollSpeed::Slow);
        steps.push(ScrollStep::ScrollBy {
            delta_px: micro_adjust,
            speed: ScrollSpeed::Slow,
        });
        total_ms += adjust_ms;
    }

    ScrollPlan {
        steps,
        total_distance_px: total_distance,
        total_ms,
    }
}

/// Build a scroll plan that scrolls to a specific element (via selector)
pub fn build_element_scroll_plan(
    element_y_position: i32,
    viewport_height: i32,
    config: &HumanizationConfig,
) -> ScrollPlan {
    let target_px = (element_y_position - viewport_height / 3).max(0) as u32;
    build_scroll_plan(target_px, config)
}

/// Duration estimate for a scroll step based on distance and speed
fn scroll_step_duration(distance_px: i32, speed: ScrollSpeed) -> u32 {
    let abs = distance_px.unsigned_abs();
    match speed {
        ScrollSpeed::Fast => abs / 3,
        ScrollSpeed::Normal => abs / 2,
        ScrollSpeed::Slow => abs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn medium_config() -> HumanizationConfig {
        HumanizationConfig::from_level(super::super::config::HumanizationLevel::Medium)
    }

    #[test]
    fn test_overshoot_scroll_has_return() {
        let config = medium_config();
        let plan = build_scroll_plan(500, &config);

        // Should have overshoot + return, not just single scroll
        assert!(
            plan.steps.len() >= 3,
            "should have multiple steps, got {}",
            plan.steps.len()
        );
        assert!(
            plan.total_distance_px > 500,
            "total distance should exceed target due to overshoot"
        );
    }

    #[test]
    fn test_none_level_single_step() {
        let config = HumanizationConfig::from_level(super::super::config::HumanizationLevel::None);
        let plan = build_scroll_plan(300, &config);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.total_ms, 0);
    }
}
