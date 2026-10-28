//! The gesture layer: contact tracking and rejection, then G1-G7 over the contact set.

pub mod bands;
pub mod click;
pub mod contacts;
pub mod motion;
pub mod one_swipe;
pub mod scroll;
pub mod swipe;
pub mod tap;

pub use bands::Bands;
pub use click::ClickGuard;
pub use contacts::{Contact, Contacts, Seen};
pub use motion::MotionWindow;
pub use one_swipe::OneSwipe;
pub use scroll::{ScrollEvent, ScrollG, Stage};
pub use swipe::{FingerDirection, SwipeEvent, SwipeG};
pub use tap::TapG;
