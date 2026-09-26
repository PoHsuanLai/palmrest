//! The closed-set settings values (22-SETTINGS §3.9, §3.10). Two-state choices are named
//! enums, never `bool`.

use serde::{Deserialize, Serialize};

/// Palm and rest rejection P1-P3 and P5; `Off` exists for bit-exact Python comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    #[default]
    On,
    Off,
}

/// What foreign (non-shell) clients receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForeignOutput {
    /// The "palmrest touchpad" uinput device (O2).
    #[default]
    Touchpad,
    /// The Python daemon's wheel device, identity and quantisation kept (O4).
    Wheel,
    Off,
}

/// Two-finger horizontal swipe: fire once per threshold, or track live (later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceMode {
    #[default]
    Trigger,
    Live,
}

/// Whether a long two-finger drag switches again after the cooldown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwipeRepeat {
    #[default]
    AfterCooldown,
    OncePerGesture,
}

/// One-finger swipe on foreign clients: keys plus scroll, or scroll only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum G4Mode {
    #[default]
    InjectAndScroll,
    Native,
}

/// Which MTTouch state nibbles count as a contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactNibblePolicy {
    /// 1..=4, as the Python daemon.
    #[default]
    PythonParity,
    /// 3..=4 only, the kernel's "down".
    KernelOnly,
}

/// The scroll acceleration model (`feel.curve`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    /// The Python daemon's velocity and repeat gains, bit for bit.
    #[default]
    Current,
    /// Precise below a finger speed, then a smooth power-law gain up to a cap (FINDINGS F22).
    MacLike,
}

/// Who runs the glide after a flick on foreign clients (`foreign_momentum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForeignMomentum {
    /// The toolkit: the virtual touchpad lifts at Ended and the app runs its own kinetic scroll.
    #[default]
    Toolkit,
    /// palmrest: the contacts keep moving along the design/11 momentum curve, then lift.
    Ours,
}
