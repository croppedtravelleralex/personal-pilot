//! Humanization configuration types

use serde::{Deserialize, Serialize};

/// How strongly to mutate actions to simulate human behavior.
/// None = pure machine precision (for efficiency-focused tasks)
/// High = maximum humanization (for anti-detection)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HumanizationLevel {
    /// No mutation at all — machine speed, exact coordinates, instant timing
    None,
    /// Barely noticeable — small timing variance, slight offset
    Minimal,
    /// Noticeable but not slow — clear human-like variation
    Medium,
    /// Highly realistic — slower, more variable, occasional mistakes
    High,
}

impl HumanizationLevel {
    pub fn or(self, other: HumanizationLevel) -> HumanizationLevel {
        match self {
            HumanizationLevel::None => other,
            other => other,
        }
    }

    /// Returns true if this level applies any humanization
    pub fn is_active(&self) -> bool {
        !matches!(self, HumanizationLevel::None)
    }
}

impl Default for HumanizationLevel {
    fn default() -> Self {
        HumanizationLevel::Medium
    }
}

/// Timing distribution model for action gaps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TimeDistribution {
    /// Uniform random between min and max
    Uniform {
        min_ms: u32,
        max_ms: u32,
    },
    /// Normal/Gaussian distribution with mean and stddev
    Normal {
        mean_ms: u32,
        stddev_ms: u32,
    },
    /// Right-skewed: mostly short waits, occasionally long pauses (like real humans)
    RightSkewed {
        min_ms: u32,
        mode_ms: u32,
        max_ms: u32,
    },
}

impl Default for TimeDistribution {
    fn default() -> Self {
        TimeDistribution::RightSkewed {
            min_ms: 100,
            mode_ms: 300,
            max_ms: 1500,
        }
    }
}

/// Scroll speed categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollSpeed {
    Fast,
    Normal,
    Slow,
}

/// Failure recovery style — how human-like the retry behavior is
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "style")]
pub enum FailureStyle {
    /// Instant machine retry — no hesitation
    Instant,
    /// Human-like: wait a bit, sometimes give up
    Human {
        /// Min wait before retry (ms)
        min_wait_ms: u32,
        /// Max wait before retry (ms)
        max_wait_ms: u32,
        /// Max number of retries before giving up
        max_retries: u32,
        /// Probability of just giving up without retry (0.0 = always retry, 0.3 = 30% give up chance)
        give_up_chance: f32,
    },
}

impl Default for FailureStyle {
    fn default() -> Self {
        FailureStyle::Human {
            min_wait_ms: 1000,
            max_wait_ms: 5000,
            max_retries: 3,
            give_up_chance: 0.1,
        }
    }
}

/// Click offset configuration — where on an element we actually click
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickOffset {
    /// Maximum pixel radius for click offset from element center
    pub radius: u32,
    /// If true, always click slightly off-center in a consistent direction
    pub bias_direction: Option<BiasDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BiasDirection {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl Default for ClickOffset {
    fn default() -> Self {
        ClickOffset {
            radius: 5,
            bias_direction: None,
        }
    }
}

/// Typing rhythm configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingPattern {
    /// Base typing speed in words per minute
    pub base_wpm: u32,
    /// Variance percentage (0-100) — how much speed fluctuates
    pub speed_variance_percent: u32,
    /// Probability of making a typo and backspacing (0.0 - 1.0)
    pub error_retry_chance: f32,
    /// Probability of pausing mid-text (e.g., thinking)
    pub pause_chance: f32,
    /// Duration of a thinking pause (ms)
    pub pause_duration_ms: u32,
}

impl Default for TypingPattern {
    fn default() -> Self {
        TypingPattern {
            base_wpm: 45,
            speed_variance_percent: 40,
            error_retry_chance: 0.02,
            pause_chance: 0.05,
            pause_duration_ms: 800,
        }
    }
}

/// Scroll behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollBehavior {
    /// Whether to overshoot and then scroll back (human behavior)
    pub overshoot_ratio: Option<f32>,
    /// Target scroll speed
    pub speed: ScrollSpeed,
    /// Whether to hover before scrolling
    pub hover_before_scroll_ms: Option<u32>,
}

impl Default for ScrollBehavior {
    fn default() -> Self {
        ScrollBehavior {
            overshoot_ratio: Some(0.3),
            speed: ScrollSpeed::Normal,
            hover_before_scroll_ms: Some(200),
        }
    }
}

/// Master humanization configuration — controls all mutation dimensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanizationConfig {
    /// Global humanization level
    pub level: HumanizationLevel,

    /// Timing between actions
    pub timing: TimingConfig,

    /// Click offset behavior
    pub click: ClickOffset,

    /// Typing rhythm
    pub typing: TypingPattern,

    /// Scroll behavior
    pub scroll: ScrollBehavior,

    /// Failure recovery style
    pub failure: FailureStyle,
}

impl Default for HumanizationConfig {
    fn default() -> Self {
        Self {
            level: HumanizationLevel::default(),
            timing: TimingConfig::default(),
            click: ClickOffset::default(),
            typing: TypingPattern::default(),
            scroll: ScrollBehavior::default(),
            failure: FailureStyle::default(),
        }
    }
}

impl HumanizationConfig {
    /// Build a config from a level — quick constructor
    pub fn from_level(level: HumanizationLevel) -> Self {
        match level {
            HumanizationLevel::None => Self {
                level,
                timing: TimingConfig {
                    distribution: TimeDistribution::Uniform { min_ms: 0, max_ms: 0 },
                    pre_action_delay_ms: 0,
                },
                click: ClickOffset { radius: 0, bias_direction: None },
                typing: TypingPattern {
                    base_wpm: 9999,
                    speed_variance_percent: 0,
                    error_retry_chance: 0.0,
                    pause_chance: 0.0,
                    pause_duration_ms: 0,
                },
                scroll: ScrollBehavior {
                    overshoot_ratio: None,
                    speed: ScrollSpeed::Fast,
                    hover_before_scroll_ms: None,
                },
                failure: FailureStyle::Instant,
            },
            HumanizationLevel::Minimal => Self::default(),
            HumanizationLevel::Medium => Self {
                level,
                timing: TimingConfig {
                    distribution: TimeDistribution::RightSkewed {
                        min_ms: 150,
                        mode_ms: 400,
                        max_ms: 2000,
                    },
                    pre_action_delay_ms: 80,
                },
                click: ClickOffset {
                    radius: 8,
                    bias_direction: None,
                },
                typing: TypingPattern {
                    base_wpm: 40,
                    speed_variance_percent: 50,
                    error_retry_chance: 0.03,
                    pause_chance: 0.08,
                    pause_duration_ms: 1000,
                },
                scroll: ScrollBehavior {
                    overshoot_ratio: Some(0.35),
                    speed: ScrollSpeed::Normal,
                    hover_before_scroll_ms: Some(250),
                },
                failure: FailureStyle::Human {
                    min_wait_ms: 1500,
                    max_wait_ms: 6000,
                    max_retries: 3,
                    give_up_chance: 0.1,
                },
            },
            HumanizationLevel::High => Self {
                level,
                timing: TimingConfig {
                    distribution: TimeDistribution::RightSkewed {
                        min_ms: 300,
                        mode_ms: 800,
                        max_ms: 4000,
                    },
                    pre_action_delay_ms: 150,
                },
                click: ClickOffset {
                    radius: 12,
                    bias_direction: None,
                },
                typing: TypingPattern {
                    base_wpm: 35,
                    speed_variance_percent: 60,
                    error_retry_chance: 0.05,
                    pause_chance: 0.12,
                    pause_duration_ms: 1500,
                },
                scroll: ScrollBehavior {
                    overshoot_ratio: Some(0.5),
                    speed: ScrollSpeed::Slow,
                    hover_before_scroll_ms: Some(400),
                },
                failure: FailureStyle::Human {
                    min_wait_ms: 3000,
                    max_wait_ms: 12000,
                    max_retries: 2,
                    give_up_chance: 0.25,
                },
            },
        }
    }
}

/// Timing-specific config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    /// Distribution model for gap between consecutive actions
    pub distribution: TimeDistribution,
    /// Fixed delay before each action (ms) — mimics "decision time"
    pub pre_action_delay_ms: u32,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            distribution: TimeDistribution::default(),
            pre_action_delay_ms: 100,
        }
    }
}
