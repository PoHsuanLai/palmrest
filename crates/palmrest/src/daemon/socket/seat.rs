//! Who has the seat, and what that makes a connection. palmrest is a system service, so any local
//! account can reach its socket; only the person at the seat is served, and only their shell is
//! believed about the lock screen and the pointer. The decisions are pure; reading the seat file
//! is the one effect.

use super::shell::Claim;
use std::path::PathBuf;

/// Where logind records the seat's state.
pub const SEAT_FILE: &str = "/run/systemd/seats/seat0";

/// What a connection is to the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Peer {
    /// The shell of the person at the seat: its word about the pointer and the lock counts.
    Shell,
    /// Another program of the person at the seat: it hears the stream and may lock the session,
    /// never unlock it.
    Plain,
    /// Anyone else: not served.
    Observer,
}

/// The `ACTIVE_UID=` line of a `seat0` state file; `None` when there is no active session.
pub(super) fn active_uid(text: &str) -> Option<u32> {
    text.lines()
        .find_map(|line| line.strip_prefix("ACTIVE_UID="))
        .and_then(|value| value.trim().parse().ok())
}

/// A connection's standing, from the uid the kernel vouches for, the seat's active uid and what
/// the connection has shown itself to be. The shell counts only for the user whose manager runs it.
pub(super) fn peer_of(uid: u32, active_uid: Option<u32>, claim: Claim) -> Peer {
    match (active_uid, claim) {
        (Some(active), Claim::Shell { user }) if active == uid && user == uid => Peer::Shell,
        (Some(active), _) if active == uid => Peer::Plain,
        _ => Peer::Observer,
    }
}

/// The file the active uid is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatFile(pub PathBuf);

impl SeatFile {
    /// The active uid now; a missing or unreadable file means nobody has the seat.
    pub(super) fn active_uid(&self) -> Option<u32> {
        std::fs::read_to_string(&self.0)
            .ok()
            .and_then(|text| active_uid(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_uid_table() {
        const CASES: &[(&str, &str, Option<u32>)] = &[
            (
                "typical file",
                "# This is private data.\nIS_SEAT0=1\nACTIVE=c2\nACTIVE_UID=1000\nSESSIONS=c2\n",
                Some(1000),
            ),
            ("no active session", "IS_SEAT0=1\nSESSIONS=\n", None),
            ("empty file", "", None),
            ("not a number", "ACTIVE_UID=bob\n", None),
            ("negative", "ACTIVE_UID=-1\n", None),
            ("a key that ends the same", "OLD_ACTIVE_UID=5\n", None),
            ("indented is not the key", " ACTIVE_UID=7\n", None),
            ("first wins", "ACTIVE_UID=1\nACTIVE_UID=2\n", Some(1)),
            ("no final newline", "ACTIVE_UID=9", Some(9)),
        ];
        for &(name, text, want) in CASES {
            assert_eq!(active_uid(text), want, "{name}");
        }
    }

    #[test]
    fn peer_table() {
        const SHELL_1000: Claim = Claim::Shell { user: 1000 };
        const CASES: &[(&str, u32, Option<u32>, Claim, Peer)] = &[
            (
                "the user's shell",
                1000,
                Some(1000),
                SHELL_1000,
                Peer::Shell,
            ),
            (
                "another program of the user",
                1000,
                Some(1000),
                Claim::Other,
                Peer::Plain,
            ),
            (
                "a shell under another user's manager",
                1000,
                Some(1000),
                Claim::Shell { user: 1001 },
                Peer::Plain,
            ),
            (
                "another user's shell",
                1001,
                Some(1000),
                Claim::Shell { user: 1001 },
                Peer::Observer,
            ),
            (
                "the shell while another user is active",
                1000,
                Some(1001),
                SHELL_1000,
                Peer::Observer,
            ),
            ("no active user", 1000, None, SHELL_1000, Peer::Observer),
            (
                "root is not special",
                0,
                Some(1000),
                Claim::Other,
                Peer::Observer,
            ),
        ];
        for &(name, uid, active, claim, want) in CASES {
            assert_eq!(peer_of(uid, active, claim), want, "{name}");
        }
    }
}
