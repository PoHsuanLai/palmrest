//! The rows: one per setting of `Config`, in the order the Settings app lists them. A row's
//! default is not written here; it is read from `Config::default()` by path, so it cannot
//! drift. The range is the reader's own (`in lo => hi` in `sections.rs`).

use super::{Exposure, KeySpec, Kind, Page};
use Exposure::Basic;
use std::collections::BTreeMap;

/// What values a row admits.
enum Shape {
    /// A number from `lo` to `hi`, shown with `unit`.
    Range(i64, i64, Option<&'static str>),
    /// One of these words.
    Words(&'static [&'static str]),
}
use Shape::{Range, Words};

/// One row of the table.
struct Row {
    path: &'static str,
    label: &'static str,
    help: &'static str,
    section: &'static str,
    exposure: Exposure,
    shape: Shape,
}

const fn row(
    path: &'static str,
    label: &'static str,
    help: &'static str,
    section: &'static str,
    exposure: Exposure,
    shape: Shape,
) -> Row {
    Row {
        path,
        label,
        help,
        section,
        exposure,
        shape,
    }
}

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

const ROWS: &[Row] = &[
    row(
        "gestures.gesture_action_map.one_finger_swipe_right",
        "One-finger swipe right",
        "What this gesture does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.one_finger_swipe_left",
        "One-finger swipe left",
        "What this gesture does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.two_finger_swipe_right",
        "Two-finger swipe right",
        "What this gesture does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.two_finger_swipe_left",
        "Two-finger swipe left",
        "What this gesture does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.two_finger_double_tap",
        "Two-finger double-tap",
        "What this gesture does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.one_finger_double_tap",
        "One-finger double-tap",
        "What this gesture does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "scroll.speed",
        "Scroll speed",
        "How far each movement scrolls.",
        "Scrolling",
        Basic,
        Range(0, 63, None),
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

/// The labels of `shape`'s words.
fn labels_of(shape: &Shape) -> BTreeMap<String, String> {
    match shape {
        Range(..) => BTreeMap::new(),
        Words(words) => words
            .iter()
            .filter_map(|word| {
                WORD_LABELS
                    .iter()
                    .find(|(w, _)| w == word)
                    .map(|(w, l)| ((*w).to_owned(), (*l).to_owned()))
            })
            .collect(),
    }
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

/// Every row as a `KeySpec`.
pub(super) fn rows(defaults: &toml::Value) -> Vec<KeySpec> {
    ROWS.iter()
        .map(|row| KeySpec {
            path: row.path.to_owned(),
            default: default_at(defaults, row.path),
            label: row.label.to_owned(),
            help: row.help.to_owned(),
            section: row.section.to_owned(),
            exposure: row.exposure,
            kind: match row.shape {
                Range(min, max, unit) => Kind::Bounded {
                    min,
                    max,
                    unit: unit.map(str::to_owned),
                },
                Words(words) => Kind::choice(words),
            },
            page: Page::MouseAndGestures,
            labels: labels_of(&row.shape),
        })
        .collect()
}
