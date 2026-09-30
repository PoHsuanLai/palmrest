//! The rows: one per setting of `Config`, in the order the Settings app lists them. A row's
//! default is not written here; it is read from `Config::default()` by path, so it cannot
//! drift. The ranges are the reader's own where `sections.rs` gives one (`in lo => hi`) and
//! otherwise a generous bound the reader accepts (the tests read every bound back).

use super::{Exposure, KeySpec, Kind, Page};
use Exposure::{Advanced, Basic};
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

const MS: Option<&str> = Some("ms");
const UM: Option<&str> = Some("\u{b5}m");
const PER_MILLE: Option<&str> = None;

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
        "rejection",
        "Palm rejection",
        "Ignore a resting palm and the light touches around a scroll.",
        "Palm rejection",
        Basic,
        Words(&["on", "off"]),
    ),
    row(
        "foreign_output",
        "Scrolling in other apps",
        "How apps that do not know the mouse receive its scrolling: a virtual touchpad, a wheel, or nothing.",
        "Other apps",
        Basic,
        Words(&["touchpad", "wheel", "off"]),
    ),
    row(
        "foreign_momentum",
        "Momentum in other apps",
        "Who runs the glide after a flick: the app's own kinetic scroll, or palmrest.",
        "Other apps",
        Advanced,
        Words(&["toolkit", "ours"]),
    ),
    row(
        "gestures.gesture_action_map.one_finger_swipe_right",
        "One-finger swipe right",
        "What sliding one finger to the right does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.one_finger_swipe_left",
        "One-finger swipe left",
        "What sliding one finger to the left does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.two_finger_swipe_right",
        "Two-finger swipe right",
        "What sliding two fingers to the right does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.two_finger_swipe_left",
        "Two-finger swipe left",
        "What sliding two fingers to the left does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.two_finger_double_tap",
        "Two-finger double-tap",
        "What tapping twice with two fingers does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "gestures.gesture_action_map.one_finger_double_tap",
        "One-finger double-tap",
        "What tapping twice with one finger does.",
        "Gestures",
        Basic,
        Words(ACTIONS),
    ),
    row(
        "swipe.repeat",
        "Long swipe",
        "Whether a long two-finger swipe switches again after the pause, or once.",
        "Gestures",
        Basic,
        Words(&["after_cooldown", "once_per_gesture"]),
    ),
    row(
        "scroll.speed",
        "Scroll speed",
        "How far a movement of the finger scrolls.",
        "Scrolling",
        Advanced,
        Range(0, 63, None),
    ),
    row(
        "scroll.lock_threshold_x_um",
        "Horizontal lock distance",
        "How far a finger moves sideways before scrolling locks to that axis.",
        "Scrolling",
        Advanced,
        Range(385, 7692, UM),
    ),
    row(
        "scroll.lock_threshold_y_um",
        "Vertical lock distance",
        "How far a finger moves up or down before scrolling locks to that axis.",
        "Scrolling",
        Advanced,
        Range(143, 2857, UM),
    ),
    row(
        "scroll.lock_ratio",
        "Axis lock ratio",
        "How much further one axis must move than the other to lock (stored per-mille: 1500 is 1.5).",
        "Scrolling",
        Advanced,
        Range(1000, 4000, PER_MILLE),
    ),
    row(
        "scroll.lock_timeout_ms",
        "Axis lock time",
        "How long the finger has to settle on an axis.",
        "Scrolling",
        Advanced,
        Range(0, 2000, MS),
    ),
    row(
        "scroll.velocity_gain_max",
        "Fast-stroke gain",
        "The most a fast stroke speeds scrolling up (stored per-mille: 2000 is 2.0).",
        "Scrolling",
        Advanced,
        Range(1000, 4000, PER_MILLE),
    ),
    row(
        "scroll.repeat_gain_max",
        "Repeated-stroke gain",
        "The most quick successive strokes speed scrolling up (stored per-mille: 2300 is 2.3).",
        "Scrolling",
        Advanced,
        Range(1000, 4000, PER_MILLE),
    ),
    row(
        "scroll.detent_px",
        "Wheel step",
        "How many pixels one wheel click scrolls.",
        "Scrolling",
        Advanced,
        Range(20, 200, Some("px")),
    ),
    row(
        "feel.curve",
        "Acceleration curve",
        "Classic multiplies two gains; Mac-like is precise when slow, then rises smoothly.",
        "Scrolling",
        Advanced,
        Words(&["classic", "mac_like"]),
    ),
    row(
        "feel.mac_like_threshold_units_s",
        "Mac-like: precise below",
        "Finger speed below which the Mac-like curve adds no gain.",
        "Scrolling",
        Advanced,
        Range(0, 20000, None),
    ),
    row(
        "feel.mac_like_scale_units_s",
        "Mac-like: ramp length",
        "How much faster than the threshold the gain takes to grow.",
        "Scrolling",
        Advanced,
        Range(100, 20000, None),
    ),
    row(
        "feel.mac_like_exponent",
        "Mac-like: exponent",
        "How steeply the gain grows (stored per-mille: 1600 is 1.6).",
        "Scrolling",
        Advanced,
        Range(1000, 4000, PER_MILLE),
    ),
    row(
        "feel.mac_like_gain_max",
        "Mac-like: gain limit",
        "The most the Mac-like curve speeds scrolling up (stored per-mille: 6000 is 6.0).",
        "Scrolling",
        Advanced,
        Range(1000, 20000, PER_MILLE),
    ),
    row(
        "momentum.a",
        "Glide drag",
        "The glide's drag constant (stored per-mille: 30000 is 30).",
        "Glide",
        Advanced,
        Range(1000, 65000, PER_MILLE),
    ),
    row(
        "momentum.b",
        "Glide exponent",
        "How the drag grows with speed; below 1 so the glide ends (stored per-mille: 700 is 0.7).",
        "Glide",
        Advanced,
        Range(100, 950, PER_MILLE),
    ),
    row(
        "momentum.stop_px_s",
        "Glide stops below",
        "The speed, in pixels a second, at which the glide ends.",
        "Glide",
        Advanced,
        Range(1, 1000, None),
    ),
    row(
        "momentum.window_ms",
        "Release window",
        "How much of the last movement decides the glide's speed.",
        "Glide",
        Advanced,
        Range(20, 200, MS),
    ),
    row(
        "momentum.held_still_ms",
        "Held still",
        "A finger that stops this long before lifting starts no glide.",
        "Glide",
        Advanced,
        Range(10, 200, MS),
    ),
    row(
        "momentum.min_samples",
        "Samples needed",
        "The fewest movements a flick needs to start a glide.",
        "Glide",
        Advanced,
        Range(2, 20, None),
    ),
    row(
        "momentum.start_px_s",
        "Glide starts above",
        "The slowest flick, in pixels a second, that glides.",
        "Glide",
        Advanced,
        Range(0, 5000, None),
    ),
    row(
        "momentum.cap_px_s",
        "Glide speed limit",
        "The fastest glide, in pixels a second.",
        "Glide",
        Advanced,
        Range(100, 60000, None),
    ),
    row(
        "momentum.frame_ms",
        "Glide frame interval",
        "The time between the glide's steps.",
        "Glide",
        Advanced,
        Range(1, 50, MS),
    ),
    row(
        "swipe.threshold_um",
        "Swipe distance",
        "How far two fingers slide sideways to fire a swipe.",
        "Swipes and taps",
        Advanced,
        Range(1923, 38462, UM),
    ),
    row(
        "swipe.vertical_max_um",
        "Swipe vertical limit",
        "Past this much vertical movement, two fingers are scrolling, not swiping.",
        "Swipes and taps",
        Advanced,
        Range(286, 14286, UM),
    ),
    row(
        "swipe.cooldown_ms",
        "Swipe pause",
        "The pause between one swipe and the next.",
        "Swipes and taps",
        Advanced,
        Range(0, 2000, MS),
    ),
    row(
        "tap.max_ms",
        "Tap length",
        "The longest a touch can last and still be a tap.",
        "Swipes and taps",
        Advanced,
        Range(0, 2000, MS),
    ),
    row(
        "tap.gap_ms",
        "Double-tap gap",
        "The longest pause between the two taps of a double-tap.",
        "Swipes and taps",
        Advanced,
        Range(0, 2000, MS),
    ),
    row(
        "tap.max_move_x_um",
        "Tap sideways slack",
        "How far a finger may slide sideways during a tap.",
        "Swipes and taps",
        Advanced,
        Range(0, 20000, UM),
    ),
    row(
        "tap.max_move_y_um",
        "Tap lengthways slack",
        "How far a finger may slide up or down during a tap.",
        "Swipes and taps",
        Advanced,
        Range(0, 20000, UM),
    ),
    row(
        "tap.centroid_max_x_um",
        "Double-tap spread, sideways",
        "How far apart, sideways, the two taps of a double-tap may land.",
        "Swipes and taps",
        Advanced,
        Range(0, 40000, UM),
    ),
    row(
        "tap.centroid_max_y_um",
        "Double-tap spread, lengthways",
        "How far apart, up and down, the two taps of a double-tap may land.",
        "Swipes and taps",
        Advanced,
        Range(0, 20000, UM),
    ),
    row(
        "gestures.g4_foreign_mode",
        "One-finger swipe in other apps",
        "Press the back and forward keys and scroll, or scroll only and leave navigation to the app.",
        "One-finger swipe",
        Advanced,
        Words(&["inject_and_scroll", "native"]),
    ),
    row(
        "gestures.g4_travel_um",
        "One-finger swipe distance",
        "How far one finger slides to swipe.",
        "One-finger swipe",
        Advanced,
        Range(0, 60000, UM),
    ),
    row(
        "gestures.g4_max_duration_ms",
        "One-finger swipe time",
        "The longest a one-finger swipe may take.",
        "One-finger swipe",
        Advanced,
        Range(0, 2000, MS),
    ),
    row(
        "gestures.g4_axis_ratio",
        "One-finger swipe straightness",
        "How much further sideways than up or down it must move (stored per-mille: 2000 is 2.0).",
        "One-finger swipe",
        Advanced,
        Range(1000, 8000, PER_MILLE),
    ),
    row(
        "gestures.g4_settle_ms",
        "One-finger swipe settle",
        "How soon before it lifts the finger's last movement must have been.",
        "One-finger swipe",
        Advanced,
        Range(0, 500, MS),
    ),
    row(
        "gestures.host_swipe_D_um",
        "Swipe distance on shell surfaces",
        "The distance the desktop shell counts as one page of a swipe.",
        "One-finger swipe",
        Advanced,
        Range(1000, 100000, UM),
    ),
    row(
        "gestures.contact_nibble_policy",
        "Contact states",
        "Which touch states of the mouse's reports count as a contact.",
        "One-finger swipe",
        Advanced,
        Words(&["one_to_four", "kernel_only"]),
    ),
    row(
        "palm_rejection.light_touch_size_max",
        "Light-touch size",
        "Touches no larger than this are ignored (the mouse's contact size, 0 to 63).",
        "Palm rejection",
        Advanced,
        Range(0, 63, None),
    ),
    row(
        "palm_rejection.side_band_left_um",
        "Left edge band",
        "A resting touch this close to the left edge is ignored until it moves.",
        "Palm rejection",
        Advanced,
        Range(0, 20000, UM),
    ),
    row(
        "palm_rejection.side_band_right_um",
        "Right edge band",
        "A resting touch this close to the right edge is ignored until it moves.",
        "Palm rejection",
        Advanced,
        Range(0, 20000, UM),
    ),
    row(
        "palm_rejection.rear_band_percent",
        "Rear band",
        "The share of the surface, from the back, where a resting touch is ignored.",
        "Palm rejection",
        Advanced,
        Range(0, 100, Some("%")),
    ),
    row(
        "palm_rejection.resting_move_floor_x_um",
        "Resting movement, sideways",
        "A touch in a band that moves this far sideways is a finger.",
        "Palm rejection",
        Advanced,
        Range(0, 10000, UM),
    ),
    row(
        "palm_rejection.resting_move_floor_y_um",
        "Resting movement, lengthways",
        "A touch in a band that moves this far up or down is a finger.",
        "Palm rejection",
        Advanced,
        Range(0, 10000, UM),
    ),
    row(
        "palm_rejection.resting_window_ms",
        "Resting window",
        "How long a band touch is watched before it counts as resting.",
        "Palm rejection",
        Advanced,
        Range(0, 2000, MS),
    ),
    row(
        "palm_rejection.large_contact_major_units",
        "Palm size",
        "Touches at least this long on their major axis are a palm.",
        "Palm rejection",
        Advanced,
        Range(0, 1000, None),
    ),
    row(
        "palm_rejection.click_guard_ms",
        "Click guard",
        "Touches this soon after a click are ignored.",
        "Palm rejection",
        Advanced,
        Range(0, 2000, MS),
    ),
    row(
        "palm_rejection.mouse_moving_threshold_counts_s",
        "Mouse moving speed",
        "The mouse's counts a second above which it is being moved.",
        "Palm rejection",
        Advanced,
        Range(0, 5000, None),
    ),
    row(
        "palm_rejection.mouse_moving_window_ms",
        "Mouse moving window",
        "How long the mouse is measured for.",
        "Palm rejection",
        Advanced,
        Range(0, 1000, MS),
    ),
    row(
        "palm_rejection.mouse_moving_lock_multiplier",
        "Lock while moving",
        "How much harder the axis lock is while the mouse moves (stored per-mille: 2000 is 2.0).",
        "Palm rejection",
        Advanced,
        Range(1000, 8000, PER_MILLE),
    ),
    row(
        "touchpad.mm_per_px",
        "Touchpad scale",
        "Millimetres of virtual touchpad per pixel scrolled (stored per-mille: 250 is 0.25).",
        "Virtual touchpad",
        Advanced,
        Range(50, 2000, PER_MILLE),
    ),
    row(
        "touchpad.contact_spacing_mm",
        "Contact spacing",
        "The gap between the two virtual fingers.",
        "Virtual touchpad",
        Advanced,
        Range(1, 100, Some("mm")),
    ),
    row(
        "touchpad.min_down_ms",
        "Minimum touch time",
        "How long the virtual fingers stay down, so a toolkit sees a scroll.",
        "Virtual touchpad",
        Advanced,
        Range(0, 2000, MS),
    ),
    row(
        "device.source",
        "Sensor",
        "Where touches come from: the mouse's hidraw node, or its evdev node. Applies at the next reconnect.",
        "Device",
        Advanced,
        Words(&["hidraw", "evdev"]),
    ),
];

/// What a person reads for each word a choice can hold.
const WORD_LABELS: &[(&str, &str)] = &[
    ("on", "On"),
    ("off", "Off"),
    ("touchpad", "Touchpad"),
    ("wheel", "Wheel"),
    ("toolkit", "The app"),
    ("ours", "palmrest"),
    ("none", "Nothing"),
    ("back", "Back"),
    ("forward", "Forward"),
    ("workspace_prev", "Previous workspace"),
    ("workspace_next", "Next workspace"),
    ("workspace_overview", "Workspace overview"),
    ("smart_zoom", "Smart zoom"),
    ("after_cooldown", "After the pause"),
    ("once_per_gesture", "Once"),
    ("classic", "Classic"),
    ("mac_like", "Mac-like"),
    ("inject_and_scroll", "Keys and scroll"),
    ("native", "Scroll only"),
    ("one_to_four", "Touching or nearly"),
    ("kernel_only", "Touching only"),
    ("hidraw", "Hidraw"),
    ("evdev", "Evdev"),
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
