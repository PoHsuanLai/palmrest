//! Key combinations for the virtual keyboard (O3), written in settings as `"alt+left"`.
//!
//! A `Key` can only come from the name table below, so every key the daemon may press is one
//! it knows the evdev code of; the keyboard device registers exactly the keys in use.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// A modifier held while the key is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

impl Modifier {
    /// The evdev code of the left-hand key.
    pub fn code(self) -> u16 {
        match self {
            Self::Ctrl => 29,
            Self::Alt => 56,
            Self::Shift => 42,
            Self::Super => 125,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Ctrl => "ctrl",
            Self::Alt => "alt",
            Self::Shift => "shift",
            Self::Super => "super",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [Self::Ctrl, Self::Alt, Self::Shift, Self::Super]
            .into_iter()
            .find(|m| m.name() == s)
    }
}

/// A non-modifier key, by evdev code; constructible only from `NAMES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(u16);

/// Every key name the settings file may use, with its evdev code.
#[rustfmt::skip]
pub const NAMES: &[(&str, u16)] = &[
    ("escape", 1), ("1", 2), ("2", 3), ("3", 4), ("4", 5), ("5", 6), ("6", 7), ("7", 8),
    ("8", 9), ("9", 10), ("0", 11), ("minus", 12), ("equal", 13), ("backspace", 14),
    ("tab", 15), ("q", 16), ("w", 17), ("e", 18), ("r", 19), ("t", 20), ("y", 21), ("u", 22),
    ("i", 23), ("o", 24), ("p", 25), ("bracketleft", 26), ("bracketright", 27), ("enter", 28),
    ("a", 30), ("s", 31), ("d", 32), ("f", 33), ("g", 34), ("h", 35), ("j", 36), ("k", 37),
    ("l", 38), ("z", 44), ("x", 45), ("c", 46), ("v", 47), ("b", 48), ("n", 49), ("m", 50),
    ("comma", 51), ("period", 52), ("slash", 53), ("space", 57), ("f1", 59), ("f2", 60),
    ("f3", 61), ("f4", 62), ("f5", 63), ("f6", 64), ("f7", 65), ("f8", 66), ("f9", 67),
    ("f10", 68), ("f11", 87), ("f12", 88), ("home", 102), ("up", 103), ("pageup", 104),
    ("left", 105), ("right", 106), ("end", 107), ("down", 108), ("pagedown", 109),
    ("insert", 110), ("delete", 111), ("back", 158), ("forward", 159),
];

impl Key {
    pub const LEFT: Key = Key(105);
    pub const RIGHT: Key = Key(106);

    /// The evdev code.
    pub fn code(self) -> u16 {
        self.0
    }

    fn parse(s: &str) -> Option<Self> {
        NAMES.iter().find(|(n, _)| *n == s).map(|&(_, c)| Key(c))
    }

    fn name(self) -> &'static str {
        NAMES
            .iter()
            .find(|&&(_, c)| c == self.0)
            .map_or("?", |(n, _)| n)
    }
}

/// Modifiers plus one key, pressed in order and released in reverse.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KeyCombo {
    pub mods: Vec<Modifier>,
    pub key: Key,
}

/// A combo string that names an unknown key or modifier.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0:?} is not a key combination like \"alt+left\"")]
pub struct ComboError(pub String);

impl KeyCombo {
    /// Alt+Left: Back in every browser and file manager.
    pub fn alt(key: Key) -> Self {
        Self {
            mods: vec![Modifier::Alt],
            key,
        }
    }

    /// Every evdev code this combo presses.
    pub fn codes(&self) -> impl Iterator<Item = u16> + '_ {
        self.mods
            .iter()
            .map(|m| m.code())
            .chain(std::iter::once(self.key.code()))
    }
}

impl TryFrom<String> for KeyCombo {
    type Error = ComboError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        let lower = s.trim().to_ascii_lowercase();
        let parts: Vec<&str> = lower.split('+').map(str::trim).collect();
        let err = || ComboError(s.clone());
        let (key, mods) = parts.split_last().ok_or_else(err)?;
        let mods = mods
            .iter()
            .map(|m| Modifier::parse(m))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(err)?;
        Ok(Self {
            mods,
            key: Key::parse(key).ok_or_else(err)?,
        })
    }
}

impl From<KeyCombo> for String {
    fn from(k: KeyCombo) -> String {
        k.to_string()
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for m in &self.mods {
            write!(f, "{}+", m.name())?;
        }
        f.write_str(self.key.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_table() {
        const CASES: &[(&str, Option<&str>)] = &[
            ("alt+left", Some("alt+left")),
            ("Alt + Right", Some("alt+right")),
            ("ctrl+shift+t", Some("ctrl+shift+t")),
            ("super+pageup", Some("super+pageup")),
            ("f5", Some("f5")),
            ("alt+", None),
            ("hyper+left", None),
            ("alt+leftish", None),
            ("", None),
        ];
        for &(input, want) in CASES {
            let got = KeyCombo::try_from(input.to_owned())
                .ok()
                .map(|k| k.to_string());
            assert_eq!(got.as_deref(), want, "{input:?}");
        }
    }

    #[test]
    fn codes_press_modifiers_first() {
        let k = KeyCombo::alt(Key::LEFT);
        assert_eq!(k.codes().collect::<Vec<_>>(), [56, 105]);
    }

    #[test]
    fn names_are_unique() {
        for (i, (n, c)) in NAMES.iter().enumerate() {
            assert!(NAMES[i + 1..].iter().all(|(m, d)| m != n && d != c), "{n}");
        }
    }
}
