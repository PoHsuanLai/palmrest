//! Raw units to px: the base ratio and the hi-res step (12.3.8, 11.5.5).
//!
//! The operation order is fixed: a reordered float product can move a truncation by one.

/// Raw units per detent are `(64 - speed) * ACCEL_BASE`.
pub const ACCEL_BASE: u32 = 7;
/// Hi-res wheel units per detent (Linux `REL_WHEEL_HI_RES`): the scale of the exact stream.
pub const HI_RES_PER_DETENT: i32 = 120;
/// The acceleration applies only to samples at most this far apart.
pub const VELOCITY_DT_MAX: f64 = 0.05;
/// The smallest raw-per-hi-res step, so an extreme speed cannot divide by nearly zero.
pub const RAW_PER_HI_RES_MIN: f64 = 0.05;

/// Raw units per hi-res unit at this speed and gain.
pub fn raw_per_hi_res(speed: u16, gain: f64) -> f64 {
    let num = f64::from((64 - u32::from(speed.min(63))) * ACCEL_BASE);
    let den = f64::from(HI_RES_PER_DETENT) * gain;
    (num / den).max(RAW_PER_HI_RES_MIN)
}

/// Px for `raw` units, given the raw-per-hi-res step: a hi-res unit is 1/120 detent.
pub fn px(raw: f64, raw_per_hi_res: f64, detent_px: f64) -> f64 {
    raw / raw_per_hi_res * (detent_px / f64::from(HI_RES_PER_DETENT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_ratio_is_sixty_px_per_294_units() {
        let rpu = raw_per_hi_res(22, 1.0);
        assert!((rpu - 294.0 / 120.0).abs() < 1e-12);
        assert!((px(294.0, rpu, 60.0) - 60.0).abs() < 1e-9);
        assert!((px(1.0, rpu, 60.0) - 0.2041).abs() < 1e-4);
        assert_eq!(raw_per_hi_res(63, 4.6), RAW_PER_HI_RES_MIN);
    }
}
