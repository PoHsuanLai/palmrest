//! `palmrest/gestures.toml`: every tunable value, its default, and live reload.

pub mod enums;
pub mod env;
pub mod file;
pub mod lenient;
pub mod load;
pub mod sections;
pub mod units;
pub mod watch;

pub use enums::{
    ContactNibblePolicy, Curve, ForeignMomentum, ForeignOutput, G4Mode, Rejection, SwipeRepeat,
    WorkspaceMode,
};
pub use file::{CONFIG_VERSION, Config, ParseError, parse, to_toml};
pub use lenient::Warning;
pub use load::{Loaded, config_dir, load};
pub use sections::{
    FeelSettings, GestureSettings, MomentumSettings, PalmRejectionSettings, ScrollSettings,
    SwipeSettings, TapSettings, TouchpadSettings,
};
pub use units::{Count, Fraction, Ms, Percent, Px, Units};
