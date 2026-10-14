//! Fader math: percent ↔ gain, relative changes and linked faders.

/// Converts a fader position (0–100 %) into a linear gain with a cubic curve,
/// the same perceptual curve PulseAudio and KDE use.
pub fn to_gain(percent: u8) -> f32 {
    let p = f32::from(percent.min(100)) / 100.0;
    p * p * p
}

/// Applies a relative change, clamped to 0–100.
pub fn apply_delta(current: u8, delta: i32) -> u8 {
    i32::from(current).saturating_add(delta).clamp(0, 100) as u8
}

/// New position of the partner fader when a linked fader moves from `old` to `new`.
/// The distance in percentage points is preserved and the result clamped.
pub fn linked_partner(old: u8, new: u8, partner: u8) -> u8 {
    apply_delta(partner, i32::from(new) - i32::from(old))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_curve_is_cubic() {
        assert_eq!(to_gain(0), 0.0);
        assert_eq!(to_gain(100), 1.0);
        assert!((to_gain(50) - 0.125).abs() < 1e-6);
        assert_eq!(to_gain(200), 1.0);
    }

    #[test]
    fn delta_is_clamped() {
        assert_eq!(apply_delta(50, 10), 60);
        assert_eq!(apply_delta(95, 10), 100);
        assert_eq!(apply_delta(5, -10), 0);
    }

    #[test]
    fn delta_saturates_on_extreme_input() {
        assert_eq!(apply_delta(50, i32::MAX), 100);
        assert_eq!(apply_delta(50, i32::MIN), 0);
    }

    #[test]
    fn linked_partner_keeps_distance_and_clamps() {
        assert_eq!(linked_partner(50, 40, 80), 70);
        assert_eq!(linked_partner(50, 90, 80), 100);
        assert_eq!(linked_partner(50, 0, 20), 0);
    }
}
