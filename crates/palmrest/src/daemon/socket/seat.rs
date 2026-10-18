//! Who has the seat, and what that makes a connection. palmrest is a system service, so any local
//! account can reach its socket; only the person at the seat is served, and only their shell is
//! believed about the lock screen and the pointer. The decisions are pure; the seat file is
//! `latchkey::guard::SeatFile`.

use latchkey::guard::Claim;

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

/// A connection's standing, from the uid the kernel vouches for, the seat's active uid and what
/// the connection has shown itself to be. The shell counts only for the user whose manager runs it.
pub(super) fn peer_of(uid: u32, active_uid: Option<u32>, claim: Claim) -> Peer {
    match (active_uid, claim) {
        (Some(active), Claim::Shell { user }) if active == uid && user == uid => Peer::Shell,
        (Some(active), _) if active == uid => Peer::Plain,
        _ => Peer::Observer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
