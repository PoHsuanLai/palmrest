//! What the tracker asks the daemon to do, and how a fired gesture becomes outputs.

use crate::action::{Action, Gesture, key_of};
use crate::config::G4Mode;
use crate::feel::WheelDelta;
use crate::keys::KeyCombo;
use crate::outputs::pad::PadFrame;
use palmrest_protocol::{LockState, Over, TapKind, ToHost};
use serde::{Deserialize, Serialize};

/// One effect of a report, in the order it must happen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Out {
    /// A socket message for every subscriber (O1).
    Host(ToHost),
    /// A virtual touchpad frame (O2).
    Pad(PadFrame),
    /// A wheel frame (O4).
    Wheel(WheelDelta),
    /// A key combination on the virtual keyboard (O3).
    Key(KeyCombo),
    /// A desktop action the daemon performs (`action::plan`).
    Action(Action),
}

/// Who asked for an action: G4 is suppressed differently from the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    OneFingerSwipe,
    Other,
}

/// The outputs of `action` fired by a gesture, given who holds the pointer.
///
/// SmartZoom is realised as the `Tap{One}` the host receives, so it produces nothing here. G4
/// produces nothing while a host holds the pointer (the host runs its own page swipe) or in
/// `Native` mode.
pub fn action_outs(action: &Action, source: Source, claimed: Over, g4: G4Mode) -> Vec<Out> {
    let silenced =
        source == Source::OneFingerSwipe && (claimed == Over::Ours || g4 == G4Mode::Native);
    if silenced {
        return vec![];
    }
    match (action, key_of(action)) {
        (_, Some(k)) => vec![Out::Key(k)],
        (Action::None | Action::SmartZoom, None) => vec![],
        (other, None) => vec![Out::Action(other.clone())],
    }
}

/// The outputs of a completed double-tap.
pub fn tap_outs(kind: TapKind, action: &Action, claimed: Over, t_ns: u64) -> Vec<Out> {
    let host = (claimed == Over::Ours).then_some(Out::Host(ToHost::Tap { kind, t_ns }));
    host.into_iter()
        .chain(action_outs(
            action,
            Source::Other,
            claimed,
            G4Mode::InjectAndScroll,
        ))
        .collect()
}

/// `outs` without the keys and desktop actions a gesture fired, while the session is locked:
/// they would reach the lock screen or run the person's commands on a locked machine. Messages
/// to the shell and the scroll devices stay.
pub fn while_unlocked(session: LockState, outs: Vec<Out>) -> Vec<Out> {
    match session {
        LockState::Unlocked => outs,
        LockState::Locked => outs
            .into_iter()
            .filter(|o| !matches!(o, Out::Key(_) | Out::Action(_)))
            .collect(),
    }
}

/// The gesture for a double-tap kind.
pub fn tap_gesture(kind: TapKind) -> Gesture {
    match kind {
        TapKind::One => Gesture::OneFingerDoubleTap,
        TapKind::Two => Gesture::TwoFingerDoubleTap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Key;

    #[test]
    fn g4_is_silent_under_a_claim_and_in_native_mode() {
        let back = Action::Back;
        let key = vec![Out::Key(KeyCombo::alt(Key::LEFT))];
        assert_eq!(
            action_outs(
                &back,
                Source::OneFingerSwipe,
                Over::NotOurs,
                G4Mode::InjectAndScroll
            ),
            key
        );
        assert_eq!(
            action_outs(
                &back,
                Source::OneFingerSwipe,
                Over::Ours,
                G4Mode::InjectAndScroll
            ),
            vec![]
        );
        assert_eq!(
            action_outs(&back, Source::OneFingerSwipe, Over::NotOurs, G4Mode::Native),
            vec![]
        );
        assert_eq!(
            action_outs(&back, Source::Other, Over::Ours, G4Mode::Native),
            key
        );
    }

    #[test]
    fn a_locked_session_keeps_only_what_is_not_a_gesture_effect() {
        let outs = vec![
            Out::Host(ToHost::DeviceGone),
            Out::Key(KeyCombo::alt(Key::LEFT)),
            Out::Action(Action::WorkspaceOverview),
        ];
        assert_eq!(while_unlocked(LockState::Unlocked, outs.clone()), outs);
        assert_eq!(
            while_unlocked(LockState::Locked, outs),
            vec![Out::Host(ToHost::DeviceGone)]
        );
    }

    #[test]
    fn smart_zoom_is_only_the_host_tap() {
        let z = Action::SmartZoom;
        assert_eq!(tap_outs(TapKind::One, &z, Over::NotOurs, 5), vec![]);
        assert_eq!(
            tap_outs(TapKind::One, &z, Over::Ours, 5),
            vec![Out::Host(ToHost::Tap {
                kind: TapKind::One,
                t_ns: 5
            })]
        );
        let o = Action::WorkspaceOverview;
        assert_eq!(
            tap_outs(TapKind::Two, &o, Over::NotOurs, 5),
            vec![Out::Action(o.clone())]
        );
    }
}
