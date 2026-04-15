//! Typing mutation — human-like keystroke rhythm and error handling

use rand::Rng;

use super::config::HumanizationConfig;

/// A single typing event
#[derive(Debug, Clone)]
pub enum TypingEvent {
    /// Type a single character
    Key { ch: char, interval_ms: u32 },
    /// Backspace (correction)
    Backspace { interval_ms: u32 },
    /// Pause mid-text (thinking)
    Pause { duration_ms: u32 },
}

/// Full typing plan for a string
#[derive(Debug, Clone)]
pub struct TypingPlan {
    pub events: Vec<TypingEvent>,
    /// Total time to type the full string (ms)
    pub total_ms: u32,
}

/// Build a humanized typing plan from raw text + config
pub fn build_typing_plan(text: &str, config: &HumanizationConfig) -> TypingPlan {
    let mut events = Vec::new();
    let mut total_ms: u32 = 0;
    let mut rng = rand::thread_rng();
    let pattern = &config.typing;

    // Edge case
    if text.is_empty() {
        return TypingPlan { events, total_ms: 0 };
    }

    for (i, ch) in text.chars().enumerate() {
        // Insert pre-character delay (thinking/reaction time)
        let pre_delay = pattern.speed_variance_percent.min(30) as u32;
        let pre_ms = rng.gen_range(0..=pre_delay);
        if pre_ms > 0 {
            total_ms += pre_ms;
            events.push(TypingEvent::Pause { duration_ms: pre_ms });
        }

        // Occasional mid-text "thinking" pause
        if pattern.pause_chance > 0.0 && rng.gen::<f32>() < pattern.pause_chance {
            let pause = pattern.pause_duration_ms;
            total_ms += pause;
            events.push(TypingEvent::Pause { duration_ms: pause });
        }

        // Determine base interval for this character
        let base_interval = 60000 / pattern.base_wpm.max(1);
        let variance = (base_interval as f32 * pattern.speed_variance_percent as f32 / 100.0) as u32;
        let interval = if variance > 0 {
            rng.gen_range(base_interval.saturating_sub(variance)..=base_interval + variance)
        } else {
            base_interval
        };

        // Special keys: shift for uppercase takes longer, punctuation slightly longer
        let adjusted_interval = if ch.is_uppercase() || ch.is_ascii_punctuation() {
            interval.saturating_add(25)
        } else {
            interval
        };

        // Simulate typo + correction
        if pattern.error_retry_chance > 0.0 && rng.gen::<f32>() < pattern.error_retry_chance {
            // Type wrong character first
            events.push(TypingEvent::Key { ch, interval_ms: adjusted_interval / 2 });
            total_ms += adjusted_interval / 2;

            // Brief pause to "notice" the mistake
            let notice_pause: u32 = rng.gen_range(150..=350);
            total_ms += notice_pause;
            events.push(TypingEvent::Pause { duration_ms: notice_pause });

            // Backspace
            events.push(TypingEvent::Backspace { interval_ms: 80 });
            total_ms += 80;

            // Retype correctly
            events.push(TypingEvent::Key { ch, interval_ms: adjusted_interval });
            total_ms += adjusted_interval;
        } else {
            events.push(TypingEvent::Key { ch, interval_ms: adjusted_interval });
            total_ms += adjusted_interval;
        }

        // End of word: small natural pause
        if ch.is_whitespace() && i < text.len() - 1 {
            let word_pause: u32 = rng.gen_range(30..=80);
            total_ms += word_pause;
            events.push(TypingEvent::Pause { duration_ms: word_pause });
        }
    }

    TypingPlan { events, total_ms }
}

/// Compute single-character interval without building full plan (for inline use)
pub fn char_interval(config: &HumanizationConfig, ch: char) -> u32 {
    let pattern = &config.typing;
    let base = 60000 / pattern.base_wpm.max(1);
    let variance = (base as f32 * pattern.speed_variance_percent as f32 / 100.0) as u32;

    let mut rng = rand::thread_rng();
    let interval = if variance > 0 {
        rng.gen_range(base.saturating_sub(variance)..=base + variance)
    } else {
        base
    };

    if ch.is_uppercase() || ch.is_ascii_punctuation() {
        interval.saturating_add(25)
    } else {
        interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn medium_config() -> HumanizationConfig {
        HumanizationConfig::from_level(super::super::config::HumanizationLevel::Medium)
    }

    #[test]
    fn test_empty_text() {
        let plan = build_typing_plan("", &medium_config());
        assert!(plan.events.is_empty());
        assert_eq!(plan.total_ms, 0);
    }

    #[test]
    fn test_hello_plan() {
        let config = medium_config();
        let plan = build_typing_plan("hello", &config);
        assert!(!plan.events.is_empty());
        assert!(plan.total_ms > 0);
        // Should contain at least 5 key events (one per char)
        let key_count = plan.events.iter().filter(|e| matches!(e, TypingEvent::Key { .. })).count();
        assert_eq!(key_count, 5, "should have 5 key events for 'hello'");
    }

    #[test]
    fn test_no_humanization() {
        let config = HumanizationConfig::from_level(super::super::config::HumanizationLevel::None);
        let plan = build_typing_plan("test", &config);
        // With None level, there should be no pauses or corrections
        let pauses: usize = plan.events.iter().filter(|e| matches!(e, TypingEvent::Pause { .. })).count();
        assert_eq!(pauses, 0, "no pauses when humanization is None");
    }
}
