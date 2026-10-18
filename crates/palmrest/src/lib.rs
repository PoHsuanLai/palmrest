//! palmrest: the Magic Mouse gesture daemon (12-BEHAVIOUR-gestures).
//!
//! The pure core (`sensor`, `touch`, `stamp`, `feel`, `gestures`, `tracker`, `action`,
//! `config`, `outputs`) takes bytes and instants and returns values; `recording` and `replay`
//! feed recorded reads through it. The daemon owns every thread, socket, device and clock, and
//! its `hidioc` holds the only `unsafe` (one hidraw ioctl); `command::run` is the one way in to it. The socket
//! messages are the `palmrest-protocol` crate.

pub mod action;
pub mod command;
pub mod config;
pub(crate) mod daemon;
pub mod error;
pub mod feel;
pub mod gestures;
pub mod keys;
pub mod outputs;
pub mod recording;
pub mod replay;
pub mod sensor;
pub mod stamp;
pub mod time;
pub mod touch;
pub mod tracker;

pub use config::Config;
pub use sensor::{Frame, Sensor, Surface};
pub use time::Mono;
pub use tracker::{Out, Tracker};
