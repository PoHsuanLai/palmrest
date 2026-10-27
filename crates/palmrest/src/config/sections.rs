//! The sections of `palmrest/gestures.toml`, one struct per table. Only what a person chooses is
//! a setting: how fast scrolling goes and what each gesture does. Every other number is a
//! constant next to the code that uses it.

use super::units::Count;
use crate::action::ActionMap;
use crate::settings_struct;

settings_struct! {
    /// `[scroll]`: how fast a finger's motion scrolls.
    pub struct ScrollSettings {
        speed: Count = Count(22), in Count(0) => Count(63);
    }
}

settings_struct! {
    /// `[gestures]`: what each gesture does.
    pub struct GestureSettings {
        gesture_action_map: ActionMap = ActionMap::default();
    }
}
