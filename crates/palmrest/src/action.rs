//! The gesture -> action table (12.6) and what each action means on this desktop.
//!
//! Every gesture is remappable in `[gestures.gesture_action_map]`; the shipped defaults are the
//! reference platform's. `plan` is pure: it says which key to press or which program to run,
//! and the daemon does it.

use crate::config::lenient::{Setting, Warning};
use crate::keys::{Key, KeyCombo};
use serde::{Deserialize, Serialize};
use toml::Value;

/// A gesture that can fire an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gesture {
    OneFingerSwipeLeft,
    OneFingerSwipeRight,
    TwoFingerSwipeLeft,
    TwoFingerSwipeRight,
    TwoFingerDoubleTap,
    OneFingerDoubleTap,
}

/// What a gesture does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Action {
    None,
    /// Alt+Left on the virtual keyboard.
    Back,
    /// Alt+Right on the virtual keyboard.
    Forward,
    WorkspacePrev,
    WorkspaceNext,
    WorkspaceOverview,
    /// Our surfaces only (a `Tap{One}` to the host under the pointer); nothing elsewhere.
    SmartZoom,
    Keys(KeyCombo),
    Command(Vec<String>),
    ShellIpc(String),
}

impl Setting for Action {
    /// Accepts the tagged form `{ kind = "keys", v = "alt+left" }` and, for actions without a
    /// value, the bare string `"back"`.
    fn read(v: &Value, key: &str, warn: &mut Vec<Warning>) -> Option<Self> {
        let tagged = match v {
            Value::String(s) => {
                let mut t = toml::Table::new();
                t.insert("kind".to_owned(), Value::String(s.clone()));
                Value::Table(t)
            }
            other => other.clone(),
        };
        let action: Option<Self> = crate::config::lenient::read_serde(&tagged, key, warn);
        if let Some(Self::Keys(combo)) = &action
            && !combo.is_sendable()
        {
            warn.push(Warning::partly_unusable(
                key,
                format!(
                    "\"{combo}\" is not sent on its own, only together with ctrl, alt or super, \
                     so this gesture will do nothing"
                ),
            ));
        }
        action
    }
}

crate::settings_struct! {
    /// One action per gesture; the defaults are the reference mapping (12.6).
    pub struct ActionMap {
        one_finger_swipe_right: Action = Action::Back;
        one_finger_swipe_left: Action = Action::Forward;
        two_finger_swipe_right: Action = Action::WorkspacePrev;
        two_finger_swipe_left: Action = Action::WorkspaceNext;
        two_finger_double_tap: Action = Action::WorkspaceOverview;
        one_finger_double_tap: Action = Action::SmartZoom;
    }
}

impl Setting for ActionMap {
    fn read(v: &Value, key: &str, warn: &mut Vec<Warning>) -> Option<Self> {
        match v {
            Value::Table(t) => Some(Self::from_table(t, key, warn)),
            _ => {
                warn.push(Warning::unusable(key, "is not a table"));
                None
            }
        }
    }
}

impl ActionMap {
    /// The action configured for `g`.
    pub fn action_for(&self, g: Gesture) -> &Action {
        match g {
            Gesture::OneFingerSwipeLeft => &self.one_finger_swipe_left,
            Gesture::OneFingerSwipeRight => &self.one_finger_swipe_right,
            Gesture::TwoFingerSwipeLeft => &self.two_finger_swipe_left,
            Gesture::TwoFingerSwipeRight => &self.two_finger_swipe_right,
            Gesture::TwoFingerDoubleTap => &self.two_finger_double_tap,
            Gesture::OneFingerDoubleTap => &self.one_finger_double_tap,
        }
    }

    /// Every key combination the table can press: what the virtual keyboard registers.
    pub fn combos(&self) -> Vec<KeyCombo> {
        ALL_GESTURES
            .iter()
            .filter_map(|&g| key_of(self.action_for(g)))
            .collect()
    }
}

/// Every gesture, in table order.
pub const ALL_GESTURES: [Gesture; 6] = [
    Gesture::OneFingerSwipeRight,
    Gesture::OneFingerSwipeLeft,
    Gesture::TwoFingerSwipeRight,
    Gesture::TwoFingerSwipeLeft,
    Gesture::TwoFingerDoubleTap,
    Gesture::OneFingerDoubleTap,
];

/// The key an action presses, if it is a key action.
pub fn key_of(a: &Action) -> Option<KeyCombo> {
    match a {
        Action::Back => Some(KeyCombo::alt(Key::LEFT)),
        Action::Forward => Some(KeyCombo::alt(Key::RIGHT)),
        Action::Keys(k) => Some(k.clone()),
        _ => None,
    }
}

/// The desktop the daemon runs under, from `XDG_CURRENT_DESKTOP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Desktop {
    Kde,
    /// Our desktop (`XDG_CURRENT_DESKTOP=quire`).
    Quire,
    Other,
}

impl Desktop {
    /// Reads the colon-separated desktop list, matching whole names.
    pub fn from_env_value(v: &str) -> Self {
        let names: Vec<&str> = v.split(':').map(str::trim).collect();
        if names.iter().any(|n| n.eq_ignore_ascii_case("KDE")) {
            Self::Kde
        } else if names.iter().any(|n| n.eq_ignore_ascii_case("quire")) {
            Self::Quire
        } else {
            Self::Other
        }
    }
}

/// What the daemon does for a non-key action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Key(KeyCombo),
    Spawn(Vec<String>),
    Nothing,
}

/// The effect of `a` on `desktop`.
pub fn plan(a: &Action, desktop: Desktop) -> Effect {
    if let Some(k) = key_of(a) {
        return Effect::Key(k);
    }
    match (a, desktop) {
        (Action::WorkspacePrev, Desktop::Kde) => kwin("Switch One Desktop to the Left"),
        (Action::WorkspaceNext, Desktop::Kde) => kwin("Switch One Desktop to the Right"),
        (Action::WorkspaceOverview, Desktop::Kde) => kwin("Overview"),
        (Action::WorkspacePrev, _) => spawn(&["sill", "workspace", "prev"]),
        (Action::WorkspaceNext, _) => spawn(&["sill", "workspace", "next"]),
        (Action::ShellIpc(s), _) => Effect::Spawn(
            std::iter::once("sill")
                .chain(s.split_whitespace())
                .map(str::to_owned)
                .collect(),
        ),
        (Action::Command(argv), _) if !argv.is_empty() => Effect::Spawn(argv.clone()),
        _ => Effect::Nothing,
    }
}

fn kwin(shortcut: &str) -> Effect {
    spawn(&[
        "qdbus-qt6",
        "org.kde.kglobalaccel",
        "/component/kwin",
        "org.kde.kglobalaccel.Component.invokeShortcut",
        shortcut,
    ])
}

fn spawn(argv: &[&str]) -> Effect {
    Effect::Spawn(argv.iter().map(|s| (*s).to_owned()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_reference_mapping() {
        let m = ActionMap::default();
        const CASES: &[(Gesture, Action)] = &[
            (Gesture::OneFingerSwipeRight, Action::Back),
            (Gesture::OneFingerSwipeLeft, Action::Forward),
            (Gesture::TwoFingerSwipeRight, Action::WorkspacePrev),
            (Gesture::TwoFingerSwipeLeft, Action::WorkspaceNext),
            (Gesture::TwoFingerDoubleTap, Action::WorkspaceOverview),
            (Gesture::OneFingerDoubleTap, Action::SmartZoom),
        ];
        for (g, a) in CASES {
            assert_eq!(m.action_for(*g), a, "{g:?}");
        }
    }

    #[test]
    fn table_accepts_bare_and_tagged_forms() {
        let text = r#"
            one_finger_swipe_right = "forward"
            one_finger_swipe_left = { kind = "keys", v = "ctrl+pagedown" }
            two_finger_double_tap = { kind = "command", v = ["notify-send", "hi"] }
            two_finger_swipe_left = "teleport"
        "#;
        let t: toml::Table = text.parse().unwrap();
        let mut warn = Vec::new();
        let m = ActionMap::from_table(&t, "gestures.gesture_action_map", &mut warn);
        assert_eq!(m.one_finger_swipe_right, Action::Forward);
        assert_eq!(
            m.one_finger_swipe_left,
            Action::Keys(KeyCombo::try_from("ctrl+pagedown".to_owned()).unwrap())
        );
        assert_eq!(
            m.two_finger_double_tap,
            Action::Command(vec!["notify-send".into(), "hi".into()])
        );
        assert_eq!(
            m.two_finger_swipe_left,
            Action::WorkspaceNext,
            "a bad value keeps the default"
        );
        assert_eq!(warn.len(), 1);
        assert_eq!(
            warn[0].key,
            "gestures.gesture_action_map.two_finger_swipe_left"
        );
    }

    #[test]
    fn a_key_that_would_type_text_earns_a_warning_but_is_kept() {
        let text = r#"
            one_finger_swipe_right = { kind = "keys", v = "enter" }
            one_finger_swipe_left = { kind = "keys", v = "shift+t" }
            two_finger_swipe_right = { kind = "keys", v = "ctrl+enter" }
            two_finger_swipe_left = { kind = "keys", v = "f5" }
        "#;
        let t: toml::Table = text.parse().unwrap();
        let mut warn = Vec::new();
        let m = ActionMap::from_table(&t, "gestures.gesture_action_map", &mut warn);
        let keys: Vec<&str> = warn.iter().map(|w| w.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "gestures.gesture_action_map.one_finger_swipe_right",
                "gestures.gesture_action_map.one_finger_swipe_left",
            ]
        );
        assert!(warn[0].problem.contains("enter"), "{}", warn[0]);
        assert!(
            !warn[0].to_string().contains("default"),
            "the value stays: {}",
            warn[0]
        );
        assert_eq!(
            m.one_finger_swipe_right,
            Action::Keys(KeyCombo::try_from("enter".to_owned()).unwrap())
        );
    }

    #[test]
    fn desktop_names_match_whole_tokens() {
        const CASES: &[(&str, Desktop)] = &[
            ("KDE", Desktop::Kde),
            ("ubuntu:KDE", Desktop::Kde),
            ("quire", Desktop::Quire),
            ("quire:GNOME", Desktop::Quire),
            ("KDEX", Desktop::Other),
            ("GNOME", Desktop::Other),
            ("", Desktop::Other),
        ];
        for &(v, want) in CASES {
            assert_eq!(Desktop::from_env_value(v), want, "{v:?}");
        }
    }

    #[test]
    fn plan_table() {
        let kde_left = plan(&Action::WorkspacePrev, Desktop::Kde);
        assert!(
            matches!(&kde_left, Effect::Spawn(v) if v.last().map(String::as_str) == Some("Switch One Desktop to the Left"))
        );
        assert_eq!(
            plan(&Action::Back, Desktop::Other),
            Effect::Key(KeyCombo::alt(Key::LEFT))
        );
        assert_eq!(
            plan(&Action::WorkspaceNext, Desktop::Other),
            spawn(&["sill", "workspace", "next"])
        );
        for (a, want) in [
            (Action::WorkspacePrev, "prev"),
            (Action::WorkspaceNext, "next"),
        ] {
            assert_eq!(
                plan(&a, Desktop::Quire),
                spawn(&["sill", "workspace", want])
            );
        }
        assert_eq!(
            plan(&Action::WorkspaceOverview, Desktop::Quire),
            Effect::Nothing,
            "no overview command is defined for quire yet"
        );
        assert_eq!(
            plan(&Action::Back, Desktop::Quire),
            Effect::Key(KeyCombo::alt(Key::LEFT))
        );
        assert_eq!(plan(&Action::SmartZoom, Desktop::Kde), Effect::Nothing);
        assert_eq!(plan(&Action::None, Desktop::Kde), Effect::Nothing);
        assert_eq!(
            plan(&Action::Command(vec![]), Desktop::Kde),
            Effect::Nothing
        );
        assert_eq!(
            plan(&Action::ShellIpc("dock toggle".into()), Desktop::Quire),
            spawn(&["sill", "dock", "toggle"])
        );
    }
}
