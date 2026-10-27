//! What the tracker asks the daemon to do, and how a fired gesture becomes outputs.

use crate::action::{Action, Deferred, Gesture, key_of};
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
    /// A key combination on the virtual keyboard (O3).
    Key(KeyCombo),
    /// What a gesture asks of the person's session, which the daemon plans for the desktop it
    /// runs under (`action::plan`).
    Deferred(Deferred),
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
/// produces nothing while a host holds the pointer (the host runs its own page swipe).
pub fn action_outs(action: &Action, source: Source, claimed: Over) -> Vec<Out> {
    if source == Source::OneFingerSwipe && claimed == Over::Ours {
        return vec![];
    }
    match key_of(action) {
        Some(k) => vec![Out::Key(k)],
        None => action.deferred().map(Out::Deferred).into_iter().collect(),
    }
}

/// The outputs of a completed double-tap.
pub fn tap_outs(kind: TapKind, action: &Action, claimed: Over, t_ns: u64) -> Vec<Out> {
    let host = (claimed == Over::Ours).then_some(Out::Host(ToHost::Tap { kind, t_ns }));
    host.into_iter()
        .chain(action_outs(action, Source::Other, claimed))
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
            .filter(|o| !matches!(o, Out::Key(_) | Out::Deferred(_)))
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
    use palmrest_protocol::Workspace;

    #[test]
    fn g4_is_silent_under_a_claim() {
        let back = Action::Back;
        let key = vec![Out::Key(KeyCombo::alt(Key::LEFT))];
        assert_eq!(
            action_outs(&back, Source::OneFingerSwipe, Over::NotOurs),
            key
        );
        assert_eq!(
            action_outs(&back, Source::OneFingerSwipe, Over::Ours),
            vec![]
        );
        assert_eq!(action_outs(&back, Source::Other, Over::Ours), key);
    }

    #[test]
    fn a_locked_session_keeps_only_what_is_not_a_gesture_effect() {
        let outs = vec![
            Out::Host(ToHost::DeviceGone),
            Out::Key(KeyCombo::alt(Key::LEFT)),
            Out::Deferred(Deferred::Workspace(Workspace::Overview)),
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
            vec![Out::Deferred(Deferred::Workspace(Workspace::Overview))]
        );
    }
}
