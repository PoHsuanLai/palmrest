//! Why a palmrest command stopped: each variant's text is the line the command prints.

use crate::outputs::OutputError;
use crate::recording::RecordingError;
use crate::sensor::magic_mouse::Model;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// A command that could not do what it was asked.
#[derive(Debug, Error)]
pub enum Error {
    /// The arguments name no command; the text is the usage.
    #[error("{0}")]
    Usage(&'static str),
    /// No hidraw node of a known mouse.
    #[error("Magic Mouse not found")]
    NoMagicMouse,
    /// No evdev node of a multitouch mouse.
    #[error("no multitouch mouse evdev node found")]
    NoEvdevNode,
    /// The mouse sends a report the hidraw decoder does not read.
    #[error(
        "{path} is a {model:?}, whose report 0x29 palmrest does not decode; \
         set [device] source = \"evdev\" to read it through the kernel driver"
    )]
    UndecodableReport { path: String, model: Model },
    /// The hidraw node could not be opened.
    #[error("cannot read {}: {source}; no gestures until it can be read", node.display())]
    HidrawOpen { node: PathBuf, source: io::Error },
    /// The evdev node could not be opened.
    #[error("cannot read {}: {source}", node.display())]
    EvdevOpen { node: PathBuf, source: io::Error },
    /// The settings folder cannot be watched.
    #[error("config watch: {0}")]
    ConfigWatch(#[source] notify::Error),
    /// The socket could not be bound.
    #[error("{}: {source}", path.display())]
    SocketBind { path: PathBuf, source: io::Error },
    /// The socket thread did not start.
    #[error("socket thread: {0}")]
    SocketThread(#[source] io::Error),
    /// A file could not be created or read.
    #[error("{}: {source}", path.display())]
    File { path: PathBuf, source: io::Error },
    /// A recording could not be written to.
    #[error("{0}")]
    Write(#[from] io::Error),
    /// Reading the sensor failed while recording.
    #[error("{0}")]
    Read(String),
    /// A file is not a readable recording.
    #[error("{}: {source}", path.display())]
    Recording {
        path: PathBuf,
        source: RecordingError,
    },
    /// The schema could not be written.
    #[error("cannot write the schema into {}: {source}", dir.display())]
    Schema { dir: PathBuf, source: io::Error },
    /// The virtual devices could not be made.
    #[error("{0}")]
    Virtual(#[from] OutputError),
}
