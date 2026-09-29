//! O2-O4: the virtual devices foreign clients see, behind one seam.
//!
//! `Emitter` is a real seam: the daemon writes through the uinput emitter, and every test
//! writes through `RecordingEmitter`, so no test ever creates an input device.

pub mod events;
pub mod pad;

use crate::feel::WheelDelta;
use crate::keys::KeyCombo;
use pad::PadFrame;
use thiserror::Error;

/// A virtual device write that failed.
#[derive(Debug, Error)]
pub enum OutputError {
    #[error("uinput is not available: {0}")]
    Unavailable(String),
    #[error("writing to the virtual device failed: {0}")]
    Write(#[from] std::io::Error),
}

/// Where foreign-client frames go.
pub trait Emitter {
    fn pad(&mut self, f: &PadFrame) -> Result<(), OutputError>;
    fn wheel(&mut self, d: &WheelDelta) -> Result<(), OutputError>;
    fn key(&mut self, k: &KeyCombo) -> Result<(), OutputError>;
}

/// One frame an emitter was asked to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Emitted {
    Pad(PadFrame),
    Wheel(WheelDelta),
    Key(KeyCombo),
}

/// Remembers every frame instead of writing it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordingEmitter {
    pub emitted: Vec<Emitted>,
}

impl Emitter for RecordingEmitter {
    fn pad(&mut self, f: &PadFrame) -> Result<(), OutputError> {
        self.emitted.push(Emitted::Pad(*f));
        Ok(())
    }

    fn wheel(&mut self, d: &WheelDelta) -> Result<(), OutputError> {
        self.emitted.push(Emitted::Wheel(*d));
        Ok(())
    }

    fn key(&mut self, k: &KeyCombo) -> Result<(), OutputError> {
        self.emitted.push(Emitted::Key(k.clone()));
        Ok(())
    }
}
