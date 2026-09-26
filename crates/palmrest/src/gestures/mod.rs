//! The gesture layer: contact tracking and rejection, then G1-G7 over the contact set.

pub mod click;
pub mod contacts;
pub mod live;
pub mod motion;
pub mod one_swipe;
pub mod scroll;
pub mod swipe;
pub mod tap;

pub use click::{ClickGuard, Press};
pub use contacts::{Contact, Contacts, PalmTurn, Seen};
pub use motion::{MotionWindow, Moving};
pub use one_swipe::OneSwipe;
pub use scroll::{ScrollEvent, ScrollG, Stage};
pub use swipe::{FingerDirection, SwipeEvent, SwipeG};
pub use tap::TapG;
