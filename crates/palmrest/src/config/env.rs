//! The Python daemon's environment variables, still honoured so a parity run can configure both
//! daemons the same way (12.3.12). Each variable overrides its file key, with the clamping the
//! Python daemon applies and no other range check. The three raw-unit variables are converted
//! to micrometres on the grid (`sensor::lattice`), which the core rounds back to the same units.

use super::file::Config;
use super::lenient::Warning;
use super::units::{Count, Fraction, Ms};
use crate::sensor::lattice::{Along, micrometres};

/// Applies every variable `lookup` knows. Unparseable values are ignored with a warning, as
/// the Python daemon ignores them.
pub fn apply_env(mut c: Config, lookup: impl Fn(&str) -> Option<String>) -> (Config, Vec<Warning>) {
    let mut warn = Vec::new();
    let mut int = |name: &str| read(&lookup, name, &mut warn, |s| s.parse::<i64>().ok());
    if let Some(v) = int("SCROLL_SPEED") {
        c.scroll.speed = Count(v.clamp(0, 63) as u16);
    }
    if let Some(v) = int("SCROLL_LOCK_THRESHOLD") {
        c.scroll.lock_threshold_x_um = micrometres(raw_units(v), Along::X);
        c.scroll.lock_threshold_y_um = micrometres(raw_units(v), Along::Y);
    }
    if let Some(v) = int("SWIPE_THRESHOLD") {
        c.swipe.threshold_um = micrometres(raw_units(v), Along::X);
    }
    if let Some(v) = int("SWIPE_VERTICAL_MAX") {
        c.swipe.vertical_max_um = micrometres(raw_units(v), Along::Y);
    }
    let mut float = |name: &str| {
        read(&lookup, name, &mut warn, |s| {
            s.parse::<f64>().ok().filter(|f| f.is_finite())
        })
    };
    if let Some(v) = float("SCROLL_LOCK_RATIO") {
        c.scroll.lock_ratio = Fraction::from_ratio(v.max(1.0));
    }
    if let Some(v) = float("SCROLL_LOCK_TIMEOUT") {
        c.scroll.lock_timeout_ms = seconds_to_ms(v);
    }
    if let Some(v) = float("SCROLL_VELOCITY_GAIN_MAX") {
        c.scroll.velocity_gain_max = Fraction::from_ratio(v.max(1.0));
    }
    if let Some(v) = float("SCROLL_ACCEL_MAX") {
        c.scroll.repeat_gain_max = Fraction::from_ratio(v.max(1.0));
    }
    if let Some(v) = float("SWIPE_COOLDOWN") {
        c.swipe.cooldown_ms = seconds_to_ms(v);
    }
    (c, warn)
}

fn read<T>(
    lookup: &impl Fn(&str) -> Option<String>,
    name: &str,
    warn: &mut Vec<Warning>,
    parse: impl Fn(&str) -> Option<T>,
) -> Option<T> {
    let raw = lookup(name)?;
    let parsed = parse(raw.trim());
    if parsed.is_none() {
        warn.push(Warning::unusable(name, format!("{raw:?} is not a number")));
    }
    parsed
}

/// A raw-unit variable, clamped as Python clamps it (at least 1) and to what a micrometre
/// key can hold.
fn raw_units(v: i64) -> i32 {
    v.clamp(1, 1_000_000) as i32
}

fn seconds_to_ms(s: f64) -> Ms {
    Ms((s.max(0.0) * 1000.0).round().min(f64::from(u16::MAX)) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(vars: &[(&str, &str)]) -> (Config, Vec<Warning>) {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        apply_env(Config::default(), move |k| {
            vars.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
        })
    }

    #[test]
    fn python_variables_override_and_clamp_as_python_does() {
        let (c, warn) = with(&[
            ("SCROLL_SPEED", "99"),
            ("SCROLL_LOCK_RATIO", "0.5"),
            ("SCROLL_LOCK_TIMEOUT", "0.3"),
            ("SCROLL_LOCK_THRESHOLD", "60"),
            ("SCROLL_VELOCITY_GAIN_MAX", "3"),
            ("SCROLL_ACCEL_MAX", "1.0"),
            ("SWIPE_THRESHOLD", "250"),
            ("SWIPE_VERTICAL_MAX", "100"),
            ("SWIPE_COOLDOWN", "0.75"),
        ]);
        assert!(warn.is_empty());
        assert_eq!(c.scroll.speed, Count(63));
        assert_eq!(c.scroll.lock_ratio, Fraction(1000));
        assert_eq!(c.scroll.lock_timeout_ms, Ms(300));
        assert_eq!(c.scroll.lock_threshold_x_um, micrometres(60, Along::X));
        assert_eq!(c.scroll.lock_threshold_y_um, micrometres(60, Along::Y));
        assert_eq!(c.scroll.velocity_gain_max, Fraction(3000));
        assert_eq!(c.scroll.repeat_gain_max, Fraction(1000));
        assert_eq!(c.swipe.threshold_um, micrometres(250, Along::X));
        assert_eq!(c.swipe.vertical_max_um, micrometres(100, Along::Y));
        assert_eq!(c.swipe.cooldown_ms, Ms(750));
    }

    #[test]
    fn garbage_is_ignored_with_a_warning() {
        let (c, warn) = with(&[("SCROLL_SPEED", "fast"), ("SWIPE_COOLDOWN", "nan")]);
        assert_eq!(c, Config::default());
        assert_eq!(warn.len(), 2);
    }
}
