//! The whole of `palmrest/gestures.toml`, parsed leniently.

use super::enums::{ForeignOutput, Rejection};
use super::lenient::{Warning, field, section};
use super::sections::{
    GestureSettings, PalmRejectionSettings, ScrollSettings, SwipeSettings, TapSettings,
    TouchpadSettings,
};
use serde::Serialize;
use thiserror::Error;

/// The schema version this build writes and understands.
pub const CONFIG_VERSION: u16 = 1;

/// Every setting the daemon reads.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Config {
    pub version: u16,
    pub rejection: Rejection,
    pub foreign_output: ForeignOutput,
    pub scroll: ScrollSettings,
    pub swipe: SwipeSettings,
    pub tap: TapSettings,
    pub gestures: GestureSettings,
    pub palm_rejection: PalmRejectionSettings,
    pub touchpad: TouchpadSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            rejection: Rejection::On,
            foreign_output: ForeignOutput::Touchpad,
            scroll: ScrollSettings::default(),
            swipe: SwipeSettings::default(),
            tap: TapSettings::default(),
            gestures: GestureSettings::default(),
            palm_rejection: PalmRejectionSettings::default(),
            touchpad: TouchpadSettings::default(),
        }
    }
}

/// The file is not TOML at all; nothing in it can be trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("gestures.toml is not valid TOML: {0}")]
pub struct ParseError(pub String);

crate::serde_settings!(
    u16,
    super::units::Px,
    super::units::Ms,
    super::units::Percent,
    super::units::Fraction,
    super::units::Count,
    super::units::Units,
    Rejection,
    ForeignOutput,
    super::enums::WorkspaceMode,
    super::enums::SwipeRepeat,
    super::enums::G4Mode,
    super::enums::ContactNibblePolicy,
);

/// Parses the file's text. Unknown keys are ignored (the Settings app owns preserving them);
/// bad values fall back per key and are listed in the warnings.
pub fn parse(text: &str) -> Result<(Config, Vec<Warning>), ParseError> {
    let root: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| ParseError(e.to_string()))?;
    let mut warn = Vec::new();
    let d = Config::default();
    let w = &mut warn;
    let config = Config {
        version: field(&root, "", "version", d.version, w),
        rejection: field(&root, "", "rejection", d.rejection, w),
        foreign_output: field(&root, "", "foreign_output", d.foreign_output, w),
        scroll: ScrollSettings::from_table(&section(&root, "", "scroll", w), "scroll", w),
        swipe: SwipeSettings::from_table(&section(&root, "", "swipe", w), "swipe", w),
        tap: TapSettings::from_table(&section(&root, "", "tap", w), "tap", w),
        gestures: GestureSettings::from_table(&section(&root, "", "gestures", w), "gestures", w),
        palm_rejection: PalmRejectionSettings::from_table(
            &section(&root, "", "palm_rejection", w),
            "palm_rejection",
            w,
        ),
        touchpad: TouchpadSettings::from_table(&section(&root, "", "touchpad", w), "touchpad", w),
    };
    Ok((config, warn))
}

/// The defaults as TOML, for `palmrest --print-config` and the install script.
pub fn to_toml(c: &Config) -> String {
    toml::to_string(c).unwrap_or_default() // every field is a plain value or table; cannot fail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::units::{Fraction, Ms, Units};

    #[test]
    fn empty_file_is_all_defaults() {
        assert_eq!(parse(""), Ok((Config::default(), vec![])));
    }

    #[test]
    fn defaults_round_trip_through_toml() {
        let text = to_toml(&Config::default());
        assert_eq!(parse(&text), Ok((Config::default(), vec![])), "{text}");
    }

    #[test]
    fn a_bad_value_costs_only_its_key() {
        let text = "rejection = \"sometimes\"\n[scroll]\nspeed = 30\nlock_threshold = 5000\nlock_ratio = \"x\"\n[swipe]\ncooldown_ms = 300\n";
        let (c, warn) = parse(text).unwrap();
        assert_eq!(c.scroll.speed.0, 30);
        assert_eq!(c.scroll.lock_threshold, Units(40));
        assert_eq!(c.scroll.lock_ratio, Fraction(1500));
        assert_eq!(c.swipe.cooldown_ms, Ms(300));
        assert_eq!(c.rejection, Rejection::On);
        let keys: Vec<&str> = warn.iter().map(|w| w.key.as_str()).collect();
        assert_eq!(
            keys,
            ["rejection", "scroll.lock_threshold", "scroll.lock_ratio"]
        );
    }

    #[test]
    fn capital_d_keys_keep_their_documented_spelling() {
        let (c, warn) = parse("[gestures]\nhost_swipe_D_units = 800\n").unwrap();
        assert!(warn.is_empty());
        assert_eq!(c.gestures.host_swipe_d_units, Units(800));
        assert!(to_toml(&c).contains("host_swipe_D_units = 800"));
    }

    #[test]
    fn a_section_that_is_not_a_table_is_reported() {
        let (c, warn) = parse("scroll = 3\n").unwrap();
        assert_eq!(c.scroll, ScrollSettings::default());
        assert_eq!(warn[0].key, "scroll");
    }

    #[test]
    fn not_toml_is_an_error() {
        assert!(parse("[scroll").is_err());
    }
}
