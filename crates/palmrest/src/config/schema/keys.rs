//! The rows: one per setting of `Config`, in the order the Settings app lists them. A row's
//! default is not written here; it is read from `Config::default()` by path, so it cannot
//! drift. The range is the reader's own (`in lo => hi` in `sections.rs`).

use super::{EXPOSURE, KeySpec, Kind, PAGE};
use std::collections::BTreeMap;

/// What a gesture can do without a value: the words of `Action` that need no payload. A
/// key chord, a command or a shell request is written in the file (`gestures.toml`).
const ACTIONS: &[&str] = &[
    "none",
    "back",
    "forward",
    "workspace_prev",
    "workspace_next",
    "workspace_overview",
    "smart_zoom",
];

/// The gesture rows: the key and what the page calls it.
const GESTURES: [(&str, &str); 6] = [
    (
        "gestures.gesture_action_map.one_finger_swipe_right",
        "One-finger swipe right",
    ),
    (
        "gestures.gesture_action_map.one_finger_swipe_left",
        "One-finger swipe left",
    ),
    (
        "gestures.gesture_action_map.two_finger_swipe_right",
        "Two-finger swipe right",
    ),
    (
        "gestures.gesture_action_map.two_finger_swipe_left",
        "Two-finger swipe left",
    ),
    (
        "gestures.gesture_action_map.two_finger_double_tap",
        "Two-finger double-tap",
    ),
    (
        "gestures.gesture_action_map.one_finger_double_tap",
        "One-finger double-tap",
    ),
];

/// What a person reads for each word a choice can hold.
const WORD_LABELS: &[(&str, &str)] = &[
    ("none", "Nothing"),
    ("back", "Back"),
    ("forward", "Forward"),
    ("workspace_prev", "Previous workspace"),
    ("workspace_next", "Next workspace"),
    ("workspace_overview", "Workspace overview"),
    ("smart_zoom", "Smart zoom"),
];

/// The labels of `words`.
fn labels_of(words: &[&str]) -> BTreeMap<String, String> {
    words
        .iter()
        .filter_map(|word| {
            WORD_LABELS
                .iter()
                .find(|(w, _)| w == word)
                .map(|(w, l)| ((*w).to_owned(), (*l).to_owned()))
        })
        .collect()
}

/// The value at dotted `path` of the defaults, as the schema writes it: a bare word for an
/// action that has no value (`{ kind = "back" }` in the config's own form).
fn default_at(defaults: &toml::Value, path: &str) -> toml::Value {
    let value = path
        .split('.')
        .try_fold(defaults, |at, part| at.get(part))
        .unwrap_or_else(|| panic!("{path} is not a setting of Config"));
    match value.get("kind") {
        Some(word) => word.clone(),
        None => value.clone(),
    }
}

/// One row as a `KeySpec`.
fn spec(
    defaults: &toml::Value,
    (path, label): (&str, &str),
    help: &str,
    section: &str,
    kind: Kind,
    labels: BTreeMap<String, String>,
) -> KeySpec {
    KeySpec {
        path: path.to_owned(),
        default: default_at(defaults, path),
        label: label.to_owned(),
        help: help.to_owned(),
        section: section.to_owned(),
        exposure: EXPOSURE,
        kind,
        page: PAGE,
        labels,
    }
}

/// Every row as a `KeySpec`.
pub(super) fn rows(defaults: &toml::Value) -> Vec<KeySpec> {
    let gestures = GESTURES.iter().map(|&row| {
        spec(
            defaults,
            row,
            "What this gesture does.",
            "Gestures",
            Kind::choice(ACTIONS),
            labels_of(ACTIONS),
        )
    });
    let speed = spec(
        defaults,
        ("scroll.speed", "Scroll speed"),
        "How far each movement scrolls.",
        "Scrolling",
        Kind::Bounded {
            min: 0,
            max: 63,
            unit: None,
        },
        BTreeMap::new(),
    );
    gestures.chain([speed]).collect()
}
