//! palmrest: the Magic Mouse gesture daemon (12-BEHAVIOUR-gestures).
//!
//! The pure core (`sensor`, `touch`, `stamp`, `feel`, `gestures`, `tracker`, `action`,
//! `protocol`, `config`) takes bytes and instants and returns values; `daemon` owns every
//! thread, socket, device and clock, and `sys` holds the only `unsafe`. The Magic Mouse's own
//! codec (`decode`, `encode`, `report`) lives in `sensor::magic_mouse`.

pub mod action;
pub mod compare;
pub mod config;
pub mod daemon;
pub mod feel;
pub mod gestures;
pub mod keys;
pub mod outputs;
pub mod protocol;
pub mod recording;
pub mod replay;
pub mod sensor;
pub mod stamp;
pub mod sys;
pub mod time;
pub mod touch;
pub mod tracker;

pub use config::Config;
pub use protocol::{ToDaemon, ToHost};
pub use sensor::{Frame, Sensor, Surface};
pub use time::Mono;
pub use tracker::{Out, Tracker};
