//! The whole of `palmrest/gestures.toml`, parsed leniently.

use super::enums::{ForeignMomentum, ForeignOutput, Rejection};
use super::lenient::{Warning, field, section};
use super::sections::{
    DeviceSettings, FeelSettings, GestureSettings, MomentumSettings, PalmRejectionSettings,
    ScrollSettings, SwipeSettings, TapSettings, TouchpadSettings,
};
use serde::Serialize;
use thiserror::Error;

/// Every setting the daemon reads.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Config {
    pub rejection: Rejection,
    pub foreign_output: ForeignOutput,
    pub foreign_momentum: ForeignMomentum,
    pub scroll: ScrollSettings,
    pub feel: FeelSettings,
    pub momentum: MomentumSettings,
    pub swipe: SwipeSettings,
    pub tap: TapSettings,
    pub gestures: GestureSettings,
    pub palm_rejection: PalmRejectionSettings,
    pub touchpad: TouchpadSettings,
    pub device: DeviceSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rejection: Rejection::On,
            foreign_output: ForeignOutput::Touchpad,
            foreign_momentum: ForeignMomentum::Toolkit,
            scroll: ScrollSettings::default(),
            feel: FeelSettings::default(),
            momentum: MomentumSettings::default(),
            swipe: SwipeSettings::default(),
            tap: TapSettings::default(),
            gestures: GestureSettings::default(),
            palm_rejection: PalmRejectionSettings::default(),
            touchpad: TouchpadSettings::default(),
            device: DeviceSettings::default(),
        }
    }
}

/// The file is not TOML at all; nothing in it can be trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("gestures.toml is not valid TOML: {0}")]
pub struct ParseError(pub String);

crate::serde_settings!(
    super::units::Px,
    super::units::Ms,
    super::units::Percent,
    super::units::Fraction,
    super::units::Count,
    super::units::Units,
    super::units::Um,
    super::enums::DeviceSource,
    Rejection,
    ForeignOutput,
    ForeignMomentum,
    super::enums::Curve,
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
    Ok(parse_table(&root))
}

/// Reads a parsed table, leniently like [`parse`]; the layers of the machine's and the person's
/// files are merged as tables first (`layer`).
pub fn parse_table(root: &toml::Table) -> (Config, Vec<Warning>) {
    let mut warn = Vec::new();
    let d = Config::default();
    let w = &mut warn;
    let config = Config {
        rejection: field(root, "", "rejection", d.rejection, w),
        foreign_output: field(root, "", "foreign_output", d.foreign_output, w),
        foreign_momentum: field(root, "", "foreign_momentum", d.foreign_momentum, w),
        scroll: ScrollSettings::from_table(&section(root, "", "scroll", w), "scroll", w),
        feel: FeelSettings::from_table(&section(root, "", "feel", w), "feel", w),
        momentum: MomentumSettings::from_table(&section(root, "", "momentum", w), "momentum", w),
        swipe: SwipeSettings::from_table(&section(root, "", "swipe", w), "swipe", w),
        tap: TapSettings::from_table(&section(root, "", "tap", w), "tap", w),
        gestures: GestureSettings::from_table(&section(root, "", "gestures", w), "gestures", w),
        palm_rejection: PalmRejectionSettings::from_table(
            &section(root, "", "palm_rejection", w),
            "palm_rejection",
            w,
        ),
        touchpad: TouchpadSettings::from_table(&section(root, "", "touchpad", w), "touchpad", w),
        device: DeviceSettings::from_table(&section(root, "", "device", w), "device", w),
    };
    (config, warn)
}

/// The defaults as TOML, for `palmrest --print-config` and the install script.
pub fn to_toml(c: &Config) -> String {
    toml::to_string(c).unwrap_or_default() // every field is a plain value or table; cannot fail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::units::{Fraction, Ms, Um};

    #[test]
    fn an_empty_file_and_the_printed_defaults_are_all_defaults() {
        assert_eq!(parse(""), Ok((Config::default(), vec![])), "empty file");
        let text = to_toml(&Config::default());
        assert_eq!(
            parse(&text),
            Ok((Config::default(), vec![])),
            "defaults round trip through toml: {text}"
        );
    }

    #[test]
    fn a_bad_value_costs_only_its_key() {
        let text = "rejection = \"sometimes\"\n[scroll]\nspeed = 30\nlock_threshold_x_um = 99999\nlock_ratio = \"x\"\n[swipe]\ncooldown_ms = 300\n";
        let (c, warn) = parse(text).unwrap();
        assert_eq!(c.scroll.speed.0, 30);
        assert_eq!(c.scroll.lock_threshold_x_um, Um(1538));
        assert_eq!(c.scroll.lock_threshold_y_um, Um(571));
        assert_eq!(c.scroll.lock_ratio, Fraction(1500));
        assert_eq!(c.swipe.cooldown_ms, Ms(300));
        assert_eq!(c.rejection, Rejection::On);
        let keys: Vec<&str> = warn.iter().map(|w| w.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "rejection",
                "scroll.lock_threshold_x_um",
                "scroll.lock_ratio"
            ]
        );
    }

    #[test]
    fn capital_d_keys_keep_their_documented_spelling() {
        let (c, warn) = parse("[gestures]\nhost_swipe_D_um = 30000\n").unwrap();
        assert!(warn.is_empty());
        assert_eq!(c.gestures.host_swipe_d_um, Um(30000));
        assert!(to_toml(&c).contains("host_swipe_D_um = 30000"));
    }

    #[test]
    fn a_section_that_is_not_a_table_is_reported() {
        let (c, warn) = parse("scroll = 3\n").unwrap();
        assert_eq!(c.scroll, ScrollSettings::default());
        assert_eq!(warn[0].key, "scroll");
    }

    #[test]
    fn the_presets_are_opt_in_keys() {
        use crate::config::{Curve, ForeignMomentum};
        let d = Config::default();
        assert_eq!(d.feel.curve, Curve::Classic);
        assert_eq!(d.foreign_momentum, ForeignMomentum::Toolkit);
        let text = "foreign_momentum = \"ours\"\n[feel]\ncurve = \"mac_like\"\nmac_like_gain_max = 4000\n[momentum]\nb = 1000\n";
        let (c, warn) = parse(text).unwrap();
        assert_eq!(c.feel.curve, Curve::MacLike);
        assert_eq!(c.feel.mac_like_gain_max, Fraction(4000));
        assert_eq!(c.foreign_momentum, ForeignMomentum::Ours);
        assert_eq!(c.momentum.b, Fraction(700), "b = 1 has no finite glide");
        let keys: Vec<&str> = warn.iter().map(|w| w.key.as_str()).collect();
        assert_eq!(keys, ["momentum.b"]);
    }

    #[test]
    fn not_toml_is_an_error() {
        assert!(parse("[scroll").is_err());
    }
}
