//! The whole of `palmrest/gestures.toml`, parsed leniently.

use super::lenient::{Warning, section};
use super::sections::{GestureSettings, ScrollSettings};
use serde::Serialize;
use thiserror::Error;

/// Every setting the daemon reads.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Config {
    pub scroll: ScrollSettings,
    pub gestures: GestureSettings,
}

/// The file is not TOML at all; nothing in it can be trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("gestures.toml is not valid TOML: {0}")]
pub struct ParseError(pub String);

crate::serde_settings!(super::units::Count);

/// Parses the file's text. Unknown keys are ignored (the Settings app owns preserving them, and
/// a file written before a key was removed still loads); bad values fall back per key and are
/// listed in the warnings.
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
    let w = &mut warn;
    let config = Config {
        scroll: ScrollSettings::from_table(&section(root, "", "scroll", w), "scroll", w),
        gestures: GestureSettings::from_table(&section(root, "", "gestures", w), "gestures", w),
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
    use crate::action::Action;
    use crate::config::units::Count;

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
        let text = "[scroll]\nspeed = 99\n[gestures.gesture_action_map]\none_finger_double_tap = \"back\"\n";
        let (c, warn) = parse(text).unwrap();
        assert_eq!(c.scroll.speed, Count(22));
        assert_eq!(
            c.gestures.gesture_action_map.one_finger_double_tap,
            Action::Back
        );
        let keys: Vec<&str> = warn.iter().map(|w| w.key.as_str()).collect();
        assert_eq!(keys, ["scroll.speed"]);
    }

    #[test]
    fn a_file_with_removed_keys_still_loads() {
        let text = "rejection = \"off\"\nforeign_output = \"wheel\"\n[feel]\ncurve = \"classic\"\n\
                    [scroll]\nspeed = 30\nlock_ratio = 2000\n[swipe]\nrepeat = \"once_per_gesture\"\n\
                    [device]\nsource = \"evdev\"\n";
        let (c, warn) = parse(text).unwrap();
        assert!(warn.is_empty(), "{warn:?}");
        assert_eq!(c.scroll.speed, Count(30), "the kept key is read");
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
