//! The words a message is made of: gesture ids, phases, axes, taps, touch and pointer state.

use serde::{Deserialize, Serialize};

/// One gesture from MayBegin to Ended or Cancelled; every message of it carries the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GestureId(pub u32);

/// Where a gesture is in its life (12.3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    MayBegin,
    Began,
    Changed,
    Ended,
    Cancelled,
}

/// The axis a scroll is locked to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
}

/// Which double-tap fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapKind {
    One,
    Two,
}

/// Fingers on the mouse or not, for the host's double-scroll window (11.3.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveTouch {
    Down,
    Up,
}

/// Whether the pointer is over one of the subscriber's surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Over {
    Ours,
    #[default]
    NotOurs,
}

/// Whether the person's session is locked. The shell that draws the lock screen says so, because
/// it is the one that knows; a gesture does nothing on a locked session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockState {
    Locked,
    #[default]
    Unlocked,
}

/// The most words a program or shell line may have.
pub const ACTION_WORDS_MAX: usize = 16;
/// The longest word of such a line, in bytes.
pub const ACTION_WORD_MAX_BYTES: usize = 256;

/// What the daemon asks the shell to do in the person's session: the daemon is a system service
/// and has no session, so a gesture that needs one is carried out by the shell, as the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum HostAction {
    /// The workspace before the active one.
    WorkspacePrev,
    /// The workspace after the active one.
    WorkspaceNext,
    /// The workspace overview.
    WorkspaceOverview,
    /// A line of the shell's own command language (what follows `sill` on its command line).
    Shell { words: Vec<String> },
    /// A program of the person's, with its arguments (the program first); run without a shell.
    Command { argv: Vec<String> },
}

impl HostAction {
    /// Whether the words fit the bounds (at most [`ACTION_WORDS_MAX`] words of at most
    /// [`ACTION_WORD_MAX_BYTES`] bytes, and a program or a line is not empty). A side that gets an
    /// action that does not fit drops it.
    pub fn fits(&self) -> bool {
        match self {
            Self::WorkspacePrev | Self::WorkspaceNext | Self::WorkspaceOverview => true,
            Self::Shell { words } | Self::Command { argv: words } => {
                !words.is_empty()
                    && words.len() <= ACTION_WORDS_MAX
                    && words.iter().all(|w| w.len() <= ACTION_WORD_MAX_BYTES)
            }
        }
    }
}
