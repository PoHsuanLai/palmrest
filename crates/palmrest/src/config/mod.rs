//! `palmrest/gestures.toml`: the settings a person chooses, their defaults, and live reload.

pub mod file;
pub mod layer;
pub mod lenient;
pub mod load;
pub mod schema;
pub mod sections;
pub mod units;
pub mod watch;

pub use file::{Config, ParseError, parse, parse_table, to_toml};
pub use layer::layered;
pub use lenient::Warning;
pub use load::{Loaded, config_dir, first_directory, load};
pub use schema::{FILE_NAME as SCHEMA_FILE_NAME, Schema};
pub use sections::{GestureSettings, ScrollSettings};
pub use units::{Count, Fraction, Ms, Percent, Px, Um, Units};
