//! Timing jitter — computes human-like delays between actions

use rand::Rng;

use super::config::{HumanizationConfig, TimeDistribution};
use crate::humanize::config::HumanizationLevel;

/// Compute the delay (in ms) that should be inserted before the next action.
/// This is the core timing mutation function.
pub fn compute_action_gap(config: &HumanizationConfig) -> u32 {
    let total = pre_action_delay(config) + sample_gap(&config.timing.distribution);
    saturating_int(total, config.level)
}

/// Compute just the pre-action "decision" delay
pub fn pre_action_delay(config: &HumanizationConfig) -> u32 {
    if config.timing.pre_action_delay_ms == 0 {
        return 0;
    }
    let mut rng = rand::thread_rng();
    // Pre-action delay itself can vary ±30%
    let variance = (config.timing.pre_action_delay_ms / 3) as i64;
    let base = config.timing.pre_action_delay_ms as i64;
    let jitter = rng.gen_range(-variance..=variance);
    let adjusted = (base + jitter).max(0) as u32;
    saturating_int(adjusted, config.level)
}

/// Sample a gap from the configured distribution
fn sample_gap(distribution: &TimeDistribution) -> u32 {
    let mut rng = rand::thread_rng();
    match distribution {
        TimeDistribution::Uniform { min_ms, max_ms } => rng.gen_range(*min_ms..=*max_ms),
        TimeDistribution::Normal { mean_ms, stddev_ms } => {
            // Box-Muller transform for normal distribution
            let u1: f64 = rng.gen_range(0.0..1.0);
            let u2: f64 = rng.gen_range(0.0..1.0);
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            let sample = *mean_ms as f64 + z * (*stddev_ms as f64);
            sample.clamp(0.0, (*mean_ms as f64) * 3.0) as u32
        }
        TimeDistribution::RightSkewed {
            min_ms,
            mode_ms,
            max_ms,
        } => {
            // Gamma distribution approximation using transformed exponential
            // Right-skewed: most waits are short, some are very long
            let lambda = 1.0 / (*mode_ms as f64 - *min_ms as f64 + 1.0).max(1.0);
            let u: f64 = rng.gen_range(0.0..1.0);
            // Transform: short values more likely than long
            let exp_sample = -((1.0 - u).ln()) / lambda;
            let raw = *min_ms as f64 + exp_sample;
            raw.clamp(*min_ms as f64, *max_ms as f64) as u32
        }
    }
}

/// Apply per-level saturation — higher humanization = longer delays
fn saturating_int(base: u32, level: HumanizationLevel) -> u32 {
    match level {
        HumanizationLevel::None => base,
        HumanizationLevel::Minimal => base,
        HumanizationLevel::Medium => base,
        HumanizationLevel::High => base,
    }
}

/// Compute typing interval between keystrokes (in ms per character)
pub fn compute_typing_interval(config: &HumanizationConfig, char: char) -> u32 {
    let pattern = &config.typing;
    let base_interval = 60000 / pattern.base_wpm.max(1);

    let mut rng = rand::thread_rng();

    // Speed variance
    let variance_factor =
        1.0 + (rng.gen::<f32>() * 2.0 - 1.0) * (pattern.speed_variance_percent as f32 / 100.0);
    let interval = (base_interval as f32 * variance_factor) as u32;

    // Special keys (shift, backspace) take longer
    if char.is_uppercase() || char.is_ascii_punctuation() {
        return interval + 30;
    }

    saturating_int(interval, config.level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_right_skewed_produces_short_gaps() {
        let dist = TimeDistribution::RightSkewed {
            min_ms: 100,
            mode_ms: 300,
            max_ms: 2000,
        };

        let samples: Vec<u32> = (0..1000).map(|_| sample_gap(&dist)).collect();

        let mean = samples.iter().sum::<u32>() as f64 / samples.len() as f64;
        // Should be biased toward shorter values
        assert!(mean < 800.0, "mean {} should be less than 800", mean);

        // Most samples should be small
        let short_count = samples.iter().filter(|&&v| v < 500).count();
        assert!(
            short_count > 600,
            "most samples should be short, got {}",
            short_count
        );
    }

    #[test]
    fn test_normal_distribution() {
        let dist = TimeDistribution::Normal {
            mean_ms: 500,
            stddev_ms: 150,
        };

        let samples: Vec<u32> = (0..1000).map(|_| sample_gap(&dist)).collect();

        let mean = samples.iter().sum::<u32>() as f64 / samples.len() as f64;
        // Should cluster around mean
        assert!(
            (mean - 500.0).abs() < 50.0,
            "mean {} should be close to 500",
            mean
        );
    }

    #[test]
    fn test_uniform_distribution() {
        let dist = TimeDistribution::Uniform {
            min_ms: 100,
            max_ms: 200,
        };

        let samples: Vec<u32> = (0..1000).map(|_| sample_gap(&dist)).collect();

        let min = *samples.iter().min().unwrap();
        let max = *samples.iter().max().unwrap();
        assert!(min >= 100, "min {} should be >= 100", min);
        assert!(max <= 200, "max {} should be <= 200", max);
    }
}
