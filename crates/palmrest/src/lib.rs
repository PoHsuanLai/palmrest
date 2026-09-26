//! palmrest: the Magic Mouse gesture daemon (12-BEHAVIOUR-gestures).
//!
//! The pure core (`decode`, `touch`, `stamp`, `feel`, `gestures`, `tracker`, `action`,
//! `protocol`, `config`) takes bytes and instants and returns values; `daemon` owns every
//! thread, socket, device and clock, and `sys` holds the only `unsafe`.

pub mod action;
pub mod compare;
pub mod config;
pub mod daemon;
pub mod decode;
pub mod encode;
pub mod feel;
pub mod gestures;
pub mod keys;
pub mod outputs;
pub mod protocol;
pub mod recording;
pub mod replay;
pub mod report;
pub mod stamp;
pub mod sys;
pub mod time;
pub mod touch;
pub mod tracker;

pub use config::Config;
pub use decode::{DecodeError, decode};
pub use protocol::{ToDaemon, ToHost};
pub use report::Report;
pub use time::Mono;
pub use tracker::{Out, Tracker};
