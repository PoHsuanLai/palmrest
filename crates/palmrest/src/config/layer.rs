//! The person's gesture settings over the machine's. palmrest is a system service and reads only
//! `/etc/palmrest/gestures.toml` itself; the shell sends the text of the person's own
//! `~/.config/palmrest/gestures.toml` (`ToDaemon::UserConfig`), and the two are merged as tables, key
//! by key, the person's value winning. What a person may not set is dropped with a warning, and
//! what is too big is not read at all. Pure.

use super::file::{Config, parse_table};
use super::lenient::Warning;
use palmrest_protocol::{ACTION_WORD_MAX_BYTES, ACTION_WORDS_MAX, USER_CONFIG_MAX_BYTES};
use toml::{Table, Value};

/// The machine's settings text with the person's laid over it. `user` is the person's file as the
/// shell sent it (`None` or blank: no layer). A person's text that is too large, or not TOML, is
/// left out whole and the machine's settings stand.
pub fn layered(system: &str, user: Option<&str>) -> (Config, Vec<Warning>) {
    let mut table: Table = system.parse().unwrap_or_default();
    let mut warnings = Vec::new();
    if let Some(text) = user.filter(|t| !t.trim().is_empty()) {
        match theirs(text) {
            Ok(mut theirs) => {
                confine(&mut theirs, &mut warnings);
                merge(&mut table, theirs);
            }
            Err(why) => warnings.push(Warning::partly_unusable("your gestures.toml", why)),
        }
    }
    let (config, more) = parse_table(&table);
    warnings.extend(more);
    (config, warnings)
}

fn theirs(text: &str) -> Result<Table, String> {
    if text.len() > USER_CONFIG_MAX_BYTES {
        return Err(format!(
            "is larger than {USER_CONFIG_MAX_BYTES} bytes; the machine's settings stand"
        ));
    }
    text.parse().map_err(|e: toml::de::Error| {
        format!("is not valid TOML ({e}); the machine's settings stand")
    })
}

/// Takes out what a person may not set: a gesture's program or shell line that is longer than
/// the bounds of a request to the shell.
fn confine(theirs: &mut Table, warnings: &mut Vec<Warning>) {
    let Some(Value::Table(gestures)) = theirs.get_mut("gestures") else {
        return;
    };
    let Some(Value::Table(map)) = gestures.get_mut("gesture_action_map") else {
        return;
    };
    let too_long: Vec<String> = map
        .iter()
        .filter(|(_, action)| !words_in_bounds(action))
        .map(|(gesture, _)| gesture.clone())
        .collect();
    for gesture in too_long {
        map.remove(&gesture);
        warnings.push(Warning::partly_unusable(
            format!("gestures.gesture_action_map.{gesture}"),
            format!(
                "runs more than {ACTION_WORDS_MAX} words or a word of more than {ACTION_WORD_MAX_BYTES} bytes; your value is ignored"
            ),
        ));
    }
}

/// Whether an action's words (a program line, or a shell line) fit the bounds. Other actions
/// have no words.
fn words_in_bounds(action: &Value) -> bool {
    let Value::Table(action) = action else {
        return true;
    };
    let words: Vec<&str> = match (action.get("kind"), action.get("v")) {
        (Some(Value::String(kind)), Some(Value::Array(argv))) if kind == "command" => {
            argv.iter().filter_map(Value::as_str).collect()
        }
        (Some(Value::String(kind)), Some(Value::String(line))) if kind == "shell_ipc" => {
            line.split_whitespace().collect()
        }
        _ => return true,
    };
    words.len() <= ACTION_WORDS_MAX && words.iter().all(|w| w.len() <= ACTION_WORD_MAX_BYTES)
}

/// `theirs` laid over `ours`: a table meeting a table is merged, anything else is replaced.
fn merge(ours: &mut Table, theirs: Table) {
    for (key, value) in theirs {
        match (ours.get_mut(&key), value) {
            (Some(Value::Table(mine)), Value::Table(more)) => merge(mine, more),
            (_, value) => {
                ours.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, Gesture};
    use crate::config::units::Count;

    const MACHINE: &str =
        "[scroll]\nspeed = 30\n[gestures.gesture_action_map]\none_finger_swipe_left = \"none\"\n";

    #[test]
    fn the_persons_keys_win_and_the_machines_stand_where_they_say_nothing() {
        let (c, warn) = layered(MACHINE, Some("[scroll]\nspeed = 40\n"));
        assert!(warn.is_empty(), "{warn:?}");
        assert_eq!(c.scroll.speed, Count(40));
        assert_eq!(
            c.gestures
                .gesture_action_map
                .action_for(Gesture::OneFingerSwipeLeft),
            &Action::None,
            "untouched section"
        );
        let (c, _) = layered(MACHINE, None);
        assert_eq!(c.scroll.speed, Count(30));
        let (c, _) = layered(MACHINE, Some("  \n"));
        assert_eq!(c.scroll.speed, Count(30), "a blank layer is no layer");
        let (c, _) = layered("", Some("[scroll]\nspeed = 12\n"));
        assert_eq!(c.scroll.speed, Count(12), "no machine file");
    }

    #[test]
    fn what_a_person_may_not_set_is_dropped_with_a_warning() {
        let words = "w".repeat(ACTION_WORD_MAX_BYTES + 1);
        let user = format!(
            "[gestures.gesture_action_map]\n\
             one_finger_double_tap = {{ kind = \"command\", v = [\"{words}\"] }}\n\
             two_finger_swipe_left = {{ kind = \"command\", v = [\"notify-send\", \"hi\"] }}\n"
        );
        let (c, warn) = layered(MACHINE, Some(&user));
        assert_eq!(
            c.gestures
                .gesture_action_map
                .action_for(Gesture::OneFingerDoubleTap),
            &Action::SmartZoom,
            "the long line is left out and the default stands"
        );
        assert_eq!(
            c.gestures
                .gesture_action_map
                .action_for(Gesture::TwoFingerSwipeLeft),
            &Action::Command(vec!["notify-send".into(), "hi".into()])
        );
        let keys: Vec<&str> = warn.iter().map(|w| w.key.as_str()).collect();
        assert_eq!(keys, ["gestures.gesture_action_map.one_finger_double_tap"]);
    }

    #[test]
    fn a_layer_that_is_too_big_or_not_toml_leaves_the_machines_settings() {
        let big = format!("# {}\n", "x".repeat(USER_CONFIG_MAX_BYTES));
        for (name, text) in [("too big", big.as_str()), ("not toml", "[scroll")] {
            let (c, warn) = layered(MACHINE, Some(text));
            assert_eq!(c.scroll.speed, Count(30), "{name}");
            assert_eq!(warn.len(), 1, "{name}: {warn:?}");
            assert_eq!(warn[0].key, "your gestures.toml", "{name}");
        }
    }
}
