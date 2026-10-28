//! The gesture -> action table (12.6) and what each action means on this desktop.
//!
//! Every gesture is remappable in `[gestures.gesture_action_map]`; the shipped defaults are the
//! reference platform's. `plan` is pure: it says which key to press or which program to run,
//! and the daemon does it.

use crate::config::lenient::{Setting, Warning, field};
use crate::gestures::FingerDirection;
use crate::keys::{Key, KeyCombo};
use palmrest_protocol::{HostAction, TapKind, Workspace};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use toml::Value;

/// A gesture that can fire an action, in the table's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gesture {
    OneFingerSwipeRight,
    OneFingerSwipeLeft,
    TwoFingerSwipeRight,
    TwoFingerSwipeLeft,
    TwoFingerDoubleTap,
    OneFingerDoubleTap,
}

impl Gesture {
    /// Every gesture, in table order.
    pub const ALL: [Gesture; 6] = [
        Self::OneFingerSwipeRight,
        Self::OneFingerSwipeLeft,
        Self::TwoFingerSwipeRight,
        Self::TwoFingerSwipeLeft,
        Self::TwoFingerDoubleTap,
        Self::OneFingerDoubleTap,
    ];

    /// The one-finger swipe the fingers made.
    pub fn one_finger_swipe(dir: FingerDirection) -> Self {
        match dir {
            FingerDirection::Right => Self::OneFingerSwipeRight,
            FingerDirection::Left => Self::OneFingerSwipeLeft,
        }
    }

    /// The two-finger swipe the fingers made.
    pub fn two_finger_swipe(dir: FingerDirection) -> Self {
        match dir {
            FingerDirection::Right => Self::TwoFingerSwipeRight,
            FingerDirection::Left => Self::TwoFingerSwipeLeft,
        }
    }

    /// The double-tap that fired.
    pub fn tap(kind: TapKind) -> Self {
        match kind {
            TapKind::One => Self::OneFingerDoubleTap,
            TapKind::Two => Self::TwoFingerDoubleTap,
        }
    }

    /// The key of this gesture in `[gestures.gesture_action_map]` and its shipped action (12.6).
    fn row(self) -> (&'static str, Action) {
        match self {
            Self::OneFingerSwipeRight => ("one_finger_swipe_right", Action::Back),
            Self::OneFingerSwipeLeft => ("one_finger_swipe_left", Action::Forward),
            Self::TwoFingerSwipeRight => {
                ("two_finger_swipe_right", Action::Workspace(Workspace::Prev))
            }
            Self::TwoFingerSwipeLeft => {
                ("two_finger_swipe_left", Action::Workspace(Workspace::Next))
            }
            Self::TwoFingerDoubleTap => (
                "two_finger_double_tap",
                Action::Workspace(Workspace::Overview),
            ),
            Self::OneFingerDoubleTap => ("one_finger_double_tap", Action::SmartZoom),
        }
    }
}

/// What a gesture does.
///
/// Read from the file as `{ kind = "keys", v = "alt+left" }` or, for an action without a value,
/// as the bare word (`"back"`). A workspace step is the word `workspace_prev`, `workspace_next`
/// or `workspace_overview`; [`Setting::read`] maps those words to `Workspace` and `Serialize`
/// writes them back, so the file is the same as when each step was its own variant.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Action {
    None,
    /// Alt+Left on the virtual keyboard.
    Back,
    /// Alt+Right on the virtual keyboard.
    Forward,
    Workspace(Workspace),
    /// Our surfaces only (a `Tap{One}` to the host under the pointer); nothing elsewhere.
    SmartZoom,
    Keys(KeyCombo),
    Command(Vec<String>),
    ShellIpc(String),
}

/// `{ kind = .. }`, the form of an action without a value.
fn bare<S: Serializer>(s: S, kind: &str) -> Result<S::Ok, S::Error> {
    let mut st = s.serialize_struct("Action", 1)?;
    st.serialize_field("kind", kind)?;
    st.end()
}

/// `{ kind = .., v = .. }`, the form of an action with a value.
fn valued<S: Serializer, T: Serialize + ?Sized>(
    s: S,
    kind: &str,
    v: &T,
) -> Result<S::Ok, S::Error> {
    let mut st = s.serialize_struct("Action", 2)?;
    st.serialize_field("kind", kind)?;
    st.serialize_field("v", v)?;
    st.end()
}

impl Serialize for Action {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::None => bare(s, "none"),
            Self::Back => bare(s, "back"),
            Self::Forward => bare(s, "forward"),
            Self::Workspace(Workspace::Prev) => bare(s, "workspace_prev"),
            Self::Workspace(Workspace::Next) => bare(s, "workspace_next"),
            Self::Workspace(Workspace::Overview) => bare(s, "workspace_overview"),
            Self::SmartZoom => bare(s, "smart_zoom"),
            Self::Keys(k) => valued(s, "keys", k),
            Self::Command(argv) => valued(s, "command", argv),
            Self::ShellIpc(line) => valued(s, "shell_ipc", line),
        }
    }
}

/// The tagged table a bare word or a `{ kind = word }` table stands for; a `workspace_*` word
/// becomes `{ kind = "workspace", v = step }`.
fn tagged(v: &Value) -> Value {
    let kind = match v {
        Value::String(word) => word,
        Value::Table(t) => match (t.get("kind"), t.get("v")) {
            (Some(Value::String(word)), None) => word,
            _ => return v.clone(),
        },
        _ => return v.clone(),
    };
    let mut t = toml::Table::new();
    match kind.strip_prefix("workspace_") {
        Some(step) => {
            t.insert("kind".to_owned(), Value::String("workspace".to_owned()));
            t.insert("v".to_owned(), Value::String(step.to_owned()));
        }
        None => {
            t.insert("kind".to_owned(), Value::String(kind.clone()));
        }
    }
    Value::Table(t)
}

impl Setting for Action {
    /// Accepts the tagged form `{ kind = "keys", v = "alt+left" }` and, for actions without a
    /// value, the bare string `"back"`.
    fn read(v: &Value, key: &str, warn: &mut Vec<Warning>) -> Option<Self> {
        let action: Option<Self> = crate::config::lenient::read_serde(&tagged(v), key, warn);
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

/// One action per gesture; the defaults are the reference mapping (12.6).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionMap {
    /// By [`Gesture::ALL`] order.
    actions: [Action; 6],
}

impl Default for ActionMap {
    fn default() -> Self {
        Self {
            actions: Gesture::ALL.map(|g| g.row().1),
        }
    }
}

impl Serialize for ActionMap {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(
            Gesture::ALL
                .iter()
                .map(|&g| (g.row().0, self.action_for(g))),
        )
    }
}

impl Setting for ActionMap {
    fn read(v: &Value, key: &str, warn: &mut Vec<Warning>) -> Option<Self> {
        if let Value::Table(t) = v {
            Some(Self::from_table(t, key, warn))
        } else {
            warn.push(Warning::unusable(key, "is not a table"));
            None
        }
    }
}

impl ActionMap {
    /// Reads the table key by key, defaulting what is missing or bad.
    pub fn from_table(t: &toml::Table, path: &str, warn: &mut Vec<Warning>) -> Self {
        Self {
            actions: Gesture::ALL.map(|g| {
                let (name, default) = g.row();
                field(t, path, name, default, warn)
            }),
        }
    }

    /// The action configured for `g`.
    pub fn action_for(&self, g: Gesture) -> &Action {
        &self.actions[g as usize]
    }

    /// This map with `action` for `g`.
    pub fn with(mut self, g: Gesture, action: Action) -> Self {
        self.actions[g as usize] = action;
        self
    }

    /// Every key combination the table can press: what the virtual keyboard registers.
    pub fn combos(&self) -> Vec<KeyCombo> {
        self.actions.iter().filter_map(key_of).collect()
    }
}

/// The key an action presses, if it is a key action.
pub fn key_of(a: &Action) -> Option<KeyCombo> {
    match a {
        Action::Back => Some(KeyCombo::alt(Key::LEFT)),
        Action::Forward => Some(KeyCombo::alt(Key::RIGHT)),
        Action::Keys(k) => Some(k.clone()),
        _ => None,
    }
}

impl Action {
    /// Whether this action asks something of the person's session: everything that is not a key
    /// and not nothing, which the daemon cannot do alone.
    pub fn asks_session(&self) -> bool {
        matches!(
            self,
            Self::Workspace(_) | Self::ShellIpc(_) | Self::Command(_)
        )
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

/// What `action` asks of the shell on `desktop`, if anything: the daemon is a system service, with
/// no session of its own to run a program or a shell command in.
pub fn plan(action: &Action, desktop: Desktop) -> Option<HostAction> {
    match (action, desktop) {
        (Action::Workspace(Workspace::Prev), Desktop::Kde) => {
            Some(kwin("Switch One Desktop to the Left"))
        }
        (Action::Workspace(Workspace::Next), Desktop::Kde) => {
            Some(kwin("Switch One Desktop to the Right"))
        }
        (Action::Workspace(Workspace::Overview), Desktop::Kde) => Some(kwin("Overview")),
        (Action::Workspace(step), _) => Some(HostAction::Workspace(*step)),
        (Action::ShellIpc(s), _) => bounded(HostAction::Shell {
            words: s.split_whitespace().map(str::to_owned).collect(),
        }),
        (Action::Command(argv), _) => bounded(HostAction::Command { argv: argv.clone() }),
        _ => None,
    }
}

/// `action` when its words fit what the shell is asked for.
fn bounded(action: HostAction) -> Option<HostAction> {
    action.fits().then_some(action)
}

fn kwin(shortcut: &str) -> HostAction {
    HostAction::Command {
        argv: [
            "qdbus-qt6",
            "org.kde.kglobalaccel",
            "/component/kwin",
            "org.kde.kglobalaccel.Component.invokeShortcut",
            shortcut,
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect(),
    }
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
            (
                Gesture::TwoFingerSwipeRight,
                Action::Workspace(Workspace::Prev),
            ),
            (
                Gesture::TwoFingerSwipeLeft,
                Action::Workspace(Workspace::Next),
            ),
            (
                Gesture::TwoFingerDoubleTap,
                Action::Workspace(Workspace::Overview),
            ),
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
        assert_eq!(m.action_for(Gesture::OneFingerSwipeRight), &Action::Forward);
        assert_eq!(
            m.action_for(Gesture::OneFingerSwipeLeft),
            &Action::Keys(KeyCombo::try_from("ctrl+pagedown".to_owned()).unwrap())
        );
        assert_eq!(
            m.action_for(Gesture::TwoFingerDoubleTap),
            &Action::Command(vec!["notify-send".into(), "hi".into()])
        );
        assert_eq!(
            m.action_for(Gesture::TwoFingerSwipeLeft),
            &Action::Workspace(Workspace::Next),
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
            m.action_for(Gesture::OneFingerSwipeRight),
            &Action::Keys(KeyCombo::try_from("enter".to_owned()).unwrap())
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
        let workspace = Action::Workspace;
        let kde_left = plan(&workspace(Workspace::Prev), Desktop::Kde);
        assert!(matches!(
            &kde_left,
            Some(HostAction::Command { argv })
                if argv.last().map(String::as_str) == Some("Switch One Desktop to the Left")
        ));
        for desktop in [Desktop::Quire, Desktop::Other] {
            for step in [Workspace::Prev, Workspace::Next, Workspace::Overview] {
                assert_eq!(
                    plan(&workspace(step), desktop),
                    Some(HostAction::Workspace(step)),
                    "{step:?} on {desktop:?}"
                );
            }
        }
        assert_eq!(plan(&Action::Command(vec![]), Desktop::Kde), None);
        assert_eq!(plan(&Action::Back, Desktop::Kde), None);
        assert_eq!(
            plan(&Action::ShellIpc("dock toggle".into()), Desktop::Quire),
            Some(HostAction::Shell {
                words: vec!["dock".into(), "toggle".into()]
            })
        );
        assert_eq!(
            plan(
                &Action::Command(vec!["notify-send".into(), "hi".into()]),
                Desktop::Other
            ),
            Some(HostAction::Command {
                argv: vec!["notify-send".into(), "hi".into()]
            })
        );
        assert_eq!(
            plan(&Action::ShellIpc("w ".repeat(17)), Desktop::Other),
            None,
            "a line over the bounds asks for nothing"
        );
    }

    #[test]
    fn the_shipped_words_are_written_and_read_back() {
        for step in [Workspace::Prev, Workspace::Next, Workspace::Overview] {
            let action = Action::Workspace(step);
            let written = toml::Value::try_from(&action).unwrap();
            let word = written["kind"].as_str().unwrap().to_owned();
            assert!(word.starts_with("workspace_"), "{word}");
            assert!(written.get("v").is_none(), "{written}");
            let mut warn = Vec::new();
            assert_eq!(Action::read(&written, "k", &mut warn), Some(action.clone()));
            assert_eq!(Action::read(&word.into(), "k", &mut warn), Some(action));
            assert!(warn.is_empty());
        }
    }
}
