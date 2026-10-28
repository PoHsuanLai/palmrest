//! palmrest's virtual touchpad and keyboard: the devices every client but shell-host sees.
//!
//! They are made by the input service (turnstile), which closes the uinput node to everyone
//! else and checks every event palmrest sends; `route` picks the service or, for development,
//! the node itself (`direct`). The emitter is one piece of code over either, through
//! `turnstile_client::Session`.

mod caps;
mod direct;
mod emitter;
mod probe;
mod route;

pub use emitter::UinputEmitter;
pub use probe::probe;
pub use route::{LiveSource, Origin};
