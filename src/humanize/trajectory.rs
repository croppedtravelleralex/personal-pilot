//! Trajectory mutation — click offsets and mouse movement paths

use rand::Rng;

use super::config::{BiasDirection, ClickOffset, HumanizationConfig};

/// The final mutated click target coordinates (relative to element bounding box)
#[derive(Debug, Clone)]
pub struct ClickTarget {
    /// Absolute X coordinate on the page
    pub x: i32,
    /// Absolute Y coordinate on the page
    pub y: i32,
    /// Whether a hover step was inserted before click
    pub hover_before_ms: Option<u32>,
    /// Mouse movement trajectory waypoints
    pub trajectory: Vec<(i32, i32)>,
}

/// Compute the mutated click target from an element center + config
pub fn compute_click_target(
    element_center_x: i32,
    element_center_y: i32,
    _element_width: u32,
    _element_height: u32,
    config: &HumanizationConfig,
) -> ClickTarget {
    let offset = &config.click;

    if offset.radius == 0 {
        // No mutation — click dead center
        return ClickTarget {
            x: element_center_x,
            y: element_center_y,
            hover_before_ms: None,
            trajectory: vec![],
        };
    }

    let mut rng = rand::thread_rng();

    // Compute biased offset
    let (dx, dy) = compute_offset_vector(offset, &mut rng);

    // Final click position
    let target_x = element_center_x + dx;
    let target_y = element_center_y + dy;

    // Hover point — slightly different from click point (arc movement)
    let hover_x = target_x + rng.gen_range(-10..=10);
    let hover_y = target_y + rng.gen_range(-10..=10);

    // Pre-click hover (human always hovers briefly before clicking)
    let hover_before_ms = if offset.radius > 0 {
        Some(config.scroll.hover_before_scroll_ms.unwrap_or(200))
    } else {
        None
    };

    ClickTarget {
        x: target_x,
        y: target_y,
        hover_before_ms,
        trajectory: vec![(hover_x, hover_y), (target_x, target_y)],
    }
}

/// Compute a random offset vector based on bias direction
fn compute_offset_vector(offset: &ClickOffset, rng: &mut impl Rng) -> (i32, i32) {
    let radius = rng.gen_range(0..=offset.radius as i32);

    match offset.bias_direction {
        Some(BiasDirection::TopLeft) => {
            let angle = rng.gen_range(135.0..=225.0_f32).to_radians();
            (
                radius as i32 * angle.cos() as i32,
                radius as i32 * angle.sin() as i32,
            )
        }
        Some(BiasDirection::TopRight) => {
            let angle = rng.gen_range(225.0..=315.0_f32).to_radians();
            (
                radius as i32 * angle.cos() as i32,
                radius as i32 * angle.sin() as i32,
            )
        }
        Some(BiasDirection::BottomLeft) => {
            let angle = rng.gen_range(45.0..=135.0_f32).to_radians();
            (
                radius as i32 * angle.cos() as i32,
                radius as i32 * angle.sin() as i32,
            )
        }
        Some(BiasDirection::BottomRight) => {
            let angle = rng
                .gen_range(315.0..=360.0_f32)
                .max(rng.gen_range(0.0..=45.0_f32))
                .to_radians();
            (
                radius as i32 * angle.cos() as i32,
                radius as i32 * angle.sin() as i32,
            )
        }
        Some(BiasDirection::Center) | None => {
            // Random angle, uniform distribution within circle
            let angle = rng.gen_range(0.0..360.0_f32).to_radians();
            (
                radius as i32 * angle.cos() as i32,
                radius as i32 * angle.sin() as i32,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_click_offset_within_radius() {
        let config =
            HumanizationConfig::from_level(super::super::config::HumanizationLevel::Medium);
        let center_x = 500;
        let center_y = 300;

        for _ in 0..100 {
            let target = compute_click_target(center_x, center_y, 200, 100, &config);
            let dx = (target.x - center_x).abs() as u32;
            let dy = (target.y - center_y).abs() as u32;
            assert!(
                dx <= config.click.radius + 1,
                "dx {} should be within radius {}",
                dx,
                config.click.radius
            );
            assert!(
                dy <= config.click.radius + 1,
                "dy {} should be within radius {}",
                dy,
                config.click.radius
            );
        }
    }

    #[test]
    fn test_none_level_no_offset() {
        let config = HumanizationConfig::from_level(super::super::config::HumanizationLevel::None);
        let target = compute_click_target(100, 200, 50, 50, &config);
        assert_eq!(target.x, 100);
        assert_eq!(target.y, 200);
    }
}
