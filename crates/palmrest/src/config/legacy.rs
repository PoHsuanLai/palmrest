//! The settings keys from before millimetres, still read so an existing `gestures.toml` keeps
//! working (FINDINGS F25).
//!
//! Each was a raw Magic Mouse 2 unit. It is read with its old range, converted onto the
//! micrometre key(s) that replaced it at the grid's 26 units/mm across and 70 units/mm along,
//! and reported as deprecated. The conversion is exact: the core rounds the micrometres back
//! onto the same integer. A replacement key set in the same table wins, and the old key is
//! reported as ignored.

use super::lenient::{Outcome, Setting, Warning, dotted};
use super::sections::{
    GestureSettings, PalmRejectionSettings, ScrollSettings, SwipeSettings, TapSettings,
};
use super::units::{Um, Units};
use crate::sensor::lattice::{Along, micrometres};
use crate::sensor::magic_mouse::X_RANGE;
use toml::Table;

/// One deprecated key of the section `S`.
pub struct Legacy<S> {
    pub key: &'static str,
    /// The range the key accepted.
    pub range: Option<(i32, i32)>,
    /// The keys that replaced it, in the order `convert` returns their values.
    pub replaced_by: &'static [&'static str],
    pub convert: fn(i32) -> Vec<Um>,
    pub apply: fn(S, &[Um]) -> S,
}

/// Reads the deprecated keys of the table at `path`, then the section itself (so warnings keep
/// the file's key order within the section), then applies what the old keys said.
pub fn read_section<S>(
    t: &Table,
    path: &str,
    keys: &[Legacy<S>],
    section: impl FnOnce(&mut Vec<Warning>) -> S,
    warn: &mut Vec<Warning>,
) -> S {
    let found: Vec<(&Legacy<S>, Vec<Um>)> = keys
        .iter()
        .filter_map(|k| read(k, t, path, warn).map(|v| (k, v)))
        .collect();
    let s = section(warn);
    found.into_iter().fold(s, |s, (k, v)| (k.apply)(s, &v))
}

fn read<S>(k: &Legacy<S>, t: &Table, path: &str, warn: &mut Vec<Warning>) -> Option<Vec<Um>> {
    let v = t.get(k.key)?;
    let name = dotted(path, k.key);
    if let Some(set) = k.replaced_by.iter().find(|r| t.contains_key(**r)) {
        warn.push(Warning {
            key: name,
            problem: format!("is deprecated and ignored: {} is set", dotted(path, set)),
            outcome: Outcome::Ignored,
        });
        return None;
    }
    let units = Units::read(v, &name, warn)?.0;
    if let Some((lo, hi)) = k.range.filter(|&(lo, hi)| units < lo || units > hi) {
        warn.push(Warning::unusable(
            name,
            format!(
                "{:?} is outside {:?}..={:?}",
                Units(units),
                Units(lo),
                Units(hi)
            ),
        ));
        return None;
    }
    let ums = (k.convert)(units);
    let read_as: Vec<String> = k
        .replaced_by
        .iter()
        .zip(&ums)
        .map(|(r, um)| format!("{} = {}", dotted(path, r), um.0))
        .collect();
    warn.push(Warning {
        key: name,
        problem: format!(
            "is in raw Magic Mouse units, which are deprecated; read as {}",
            read_as.join(", ")
        ),
        outcome: Outcome::Converted,
    });
    Some(ums)
}

fn both(u: i32) -> Vec<Um> {
    vec![micrometres(u, Along::X), micrometres(u, Along::Y)]
}

fn across(u: i32) -> Vec<Um> {
    vec![micrometres(u, Along::X)]
}

fn along(u: i32) -> Vec<Um> {
    vec![micrometres(u, Along::Y)]
}

pub const SCROLL: &[Legacy<ScrollSettings>] = &[Legacy {
    key: "lock_threshold",
    range: Some((10, 200)),
    replaced_by: &["lock_threshold_x_um", "lock_threshold_y_um"],
    convert: both,
    apply: |s, v| ScrollSettings {
        lock_threshold_x_um: v[0],
        lock_threshold_y_um: v[1],
        ..s
    },
}];

pub const SWIPE: &[Legacy<SwipeSettings>] = &[
    Legacy {
        key: "threshold",
        range: Some((50, 1000)),
        replaced_by: &["threshold_um"],
        convert: across,
        apply: |s, v| SwipeSettings {
            threshold_um: v[0],
            ..s
        },
    },
    Legacy {
        key: "vertical_max",
        range: Some((20, 1000)),
        replaced_by: &["vertical_max_um"],
        convert: along,
        apply: |s, v| SwipeSettings {
            vertical_max_um: v[0],
            ..s
        },
    },
];

pub const TAP: &[Legacy<TapSettings>] = &[
    Legacy {
        key: "max_move_units",
        range: None,
        replaced_by: &["max_move_x_um", "max_move_y_um"],
        convert: both,
        apply: |s, v| TapSettings {
            max_move_x_um: v[0],
            max_move_y_um: v[1],
            ..s
        },
    },
    Legacy {
        key: "centroid_max_units",
        range: None,
        replaced_by: &["centroid_max_x_um", "centroid_max_y_um"],
        convert: both,
        apply: |s, v| TapSettings {
            centroid_max_x_um: v[0],
            centroid_max_y_um: v[1],
            ..s
        },
    },
];

pub const GESTURES: &[Legacy<GestureSettings>] = &[
    Legacy {
        key: "g4_travel_units",
        range: None,
        replaced_by: &["g4_travel_um"],
        convert: across,
        apply: |s, v| GestureSettings {
            g4_travel_um: v[0],
            ..s
        },
    },
    Legacy {
        key: "host_swipe_D_units",
        range: None,
        replaced_by: &["host_swipe_D_um"],
        convert: across,
        apply: |s, v| GestureSettings {
            host_swipe_d_um: v[0],
            ..s
        },
    },
    Legacy {
        key: "live_workspace_D_units",
        range: None,
        replaced_by: &["live_workspace_D_um"],
        convert: across,
        apply: |s, v| GestureSettings {
            live_workspace_d_um: v[0],
            ..s
        },
    },
];

/// The side bands were absolute x positions on the Magic Mouse 2; they become widths from its
/// edges (x -1100 and 1258), which is the same line on the mouse.
pub const PALM_REJECTION: &[Legacy<PalmRejectionSettings>] = &[
    Legacy {
        key: "side_band_x_min",
        range: None,
        replaced_by: &["side_band_left_um"],
        convert: |u| across(u - X_RANGE.0),
        apply: |s, v| PalmRejectionSettings {
            side_band_left_um: v[0],
            ..s
        },
    },
    Legacy {
        key: "side_band_x_max",
        range: None,
        replaced_by: &["side_band_right_um"],
        convert: |u| across(X_RANGE.1 - u),
        apply: |s, v| PalmRejectionSettings {
            side_band_right_um: v[0],
            ..s
        },
    },
    Legacy {
        key: "resting_move_floor_units",
        range: None,
        replaced_by: &["resting_move_floor_x_um", "resting_move_floor_y_um"],
        convert: both,
        apply: |s, v| PalmRejectionSettings {
            resting_move_floor_x_um: v[0],
            resting_move_floor_y_um: v[1],
            ..s
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, parse};
    use crate::sensor::lattice::{Ellipse, units};

    #[test]
    fn the_defaults_are_the_old_raw_defaults_on_the_grid() {
        let c = Config::default();
        let x = |um| units(um, Along::X);
        let y = |um| units(um, Along::Y);
        let s = &c.scroll;
        assert_eq!(
            (x(s.lock_threshold_x_um), y(s.lock_threshold_y_um)),
            (40, 40)
        );
        assert_eq!(x(c.swipe.threshold_um), 200);
        assert_eq!(y(c.swipe.vertical_max_um), 150);
        let t = &c.tap;
        assert_eq!(
            Ellipse::of(t.max_move_x_um, t.max_move_y_um),
            Ellipse { x: 40, y: 40 }
        );
        assert_eq!(
            Ellipse::of(t.centroid_max_x_um, t.centroid_max_y_um),
            Ellipse { x: 300, y: 300 }
        );
        let g = &c.gestures;
        assert_eq!(x(g.g4_travel_um), 300);
        assert_eq!(
            (x(g.host_swipe_d_um), x(g.live_workspace_d_um)),
            (1000, 1000)
        );
        let p = &c.palm_rejection;
        assert_eq!(X_RANGE.0 + x(p.side_band_left_um), -900);
        assert_eq!(X_RANGE.1 - x(p.side_band_right_um), 1060);
        assert_eq!(
            Ellipse::of(p.resting_move_floor_x_um, p.resting_move_floor_y_um),
            Ellipse { x: 30, y: 30 }
        );
    }

    #[test]
    fn old_raw_defaults_read_as_the_new_defaults() {
        let text = "[scroll]\nlock_threshold = 40\n[swipe]\nthreshold = 200\nvertical_max = 150\n\
            [tap]\nmax_move_units = 40\ncentroid_max_units = 300\n\
            [gestures]\ng4_travel_units = 300\nhost_swipe_D_units = 1000\nlive_workspace_D_units = 1000\n\
            [palm_rejection]\nside_band_x_min = -900\nside_band_x_max = 1060\nresting_move_floor_units = 30\n";
        let (c, warn) = parse(text).unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(warn.len(), 11);
        assert!(
            warn.iter().all(|w| w.outcome == Outcome::Converted),
            "{warn:?}"
        );
        assert_eq!(
            warn[0].to_string(),
            "scroll.lock_threshold: is in raw Magic Mouse units, which are deprecated; read as \
             scroll.lock_threshold_x_um = 1538, scroll.lock_threshold_y_um = 571"
        );
    }

    #[test]
    fn old_keys_convert_exactly_and_new_keys_win() {
        let (c, _) =
            parse("[palm_rejection]\nside_band_x_min = -950\nside_band_x_max = 1000\n").unwrap();
        let p = &c.palm_rejection;
        assert_eq!(X_RANGE.0 + units(p.side_band_left_um, Along::X), -950);
        assert_eq!(X_RANGE.1 - units(p.side_band_right_um, Along::X), 1000);

        let (c, warn) = parse("[swipe]\nthreshold = 180\nthreshold_um = 5000\n").unwrap();
        assert_eq!(c.swipe.threshold_um, Um(5000));
        assert_eq!(warn[0].outcome, Outcome::Ignored);

        let (c, warn) = parse("[scroll]\nlock_threshold = 5000\n").unwrap();
        assert_eq!(c.scroll, ScrollSettings::default());
        assert_eq!(warn[0].outcome, Outcome::DefaultUsed);
    }
}
