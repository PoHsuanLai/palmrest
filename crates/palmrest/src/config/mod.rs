//! `palmrest/gestures.toml`: every tunable value, its default, and live reload.

pub mod enums;
pub mod file;
pub mod lenient;
pub mod load;
pub mod schema;
pub mod sections;
pub mod units;
pub mod watch;

pub use enums::{
    ContactNibblePolicy, Curve, DeviceSource, ForeignMomentum, ForeignOutput, G4Mode, Rejection,
    SwipeRepeat,
};
pub use file::{Config, ParseError, parse, to_toml};
pub use lenient::Warning;
pub use load::{Loaded, config_dir, first_directory, load};
pub use schema::{FILE_NAME as SCHEMA_FILE_NAME, Schema};
pub use sections::{
    DeviceSettings, FeelSettings, GestureSettings, MomentumSettings, PalmRejectionSettings,
    ScrollSettings, SwipeSettings, TapSettings, TouchpadSettings,
};
pub use units::{Count, Fraction, Ms, Percent, Px, Um, Units};
