//! Which connection is the desktop's shell. Every local account can reach the socket, and every
//! program of the person at the seat can use it, so a connection is believed about the lock
//! screen and the pointer only when the kernel says it runs in the shell's own unit under the
//! person's own manager. That is a speed bump and not a wall: a program of the same user can put
//! itself in a control group of that name. What it buys is that no ordinary program of theirs can
//! say the session is unlocked.
//!
//! palmrest is a dynamic user without privileges, so it cannot read another user's image path;
//! the unit name in the control group is all there is to go on, and the process is held by its
//! pidfd so the name cannot belong to a process that has since been replaced. The check itself is
//! `latchkey::guard`; this file is palmrest's table and its choice of who is believed.

use latchkey::guard::{SeatUser, ShellCheck, Vouch};
use peephole_core::Table;
use peephole_core::roles::SHELL;

/// The one program allowed to speak for the session, as a row of a caller table. The paths only
/// matter when the image can be read at all, which a dynamic user cannot do for another user's
/// process.
pub(super) const SHELL_TABLE: &str = r#"
[[peer]]
app  = "org.quire.Shell"
unit = "sill.service"
exe  = [
  "/opt/quire/releases/*/bin/sill",
  "/opt/quire/current/bin/sill",
  "/usr/local/bin/sill",
  "/usr/bin/sill",
]
role = "shell"
"#;

/// Who is believed about the lock screen, chosen by the unit's environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Steering {
    /// Only the shell's unit.
    #[default]
    ShellUnit,
    /// Every program of the seat's user, for a daemon started by hand in development, whose
    /// clients are not in a unit. Only the environment sets it, never a file.
    SeatUser,
}

impl Steering {
    /// The choice `PALMREST_STEERING` names: only `seat-user` changes the default.
    pub fn from_env_value(v: Option<&str>) -> Self {
        match v {
            Some("seat-user") => Steering::SeatUser,
            _ => Steering::ShellUnit,
        }
    }

    pub(super) fn vouch(self) -> Box<dyn Vouch> {
        match self {
            Steering::ShellUnit => Box::new(shell_check()),
            Steering::SeatUser => Box::new(SeatUser),
        }
    }
}

/// The check of this machine against the shell table. A table that does not parse leaves nobody
/// believed, which is the safe way to fail.
fn shell_check() -> ShellCheck {
    let table = Table::from_toml(SHELL_TABLE).unwrap_or_else(|e| {
        eprintln!("palmrest: nobody is believed about the lock: the shell table is unusable: {e}");
        Table::empty()
    });
    ShellCheck::system(table, vec![SHELL])
}

#[cfg(test)]
mod tests {
    use super::*;
    use latchkey::guard::Claim;
    use peephole::{FixtureImage, FixtureProc, FixtureProcess, Pinned, Prober};
    use peephole_core::{FileTrust, Pid, Pin, Uid, Writer};

    const PID: u32 = 4242;
    const MANAGER_1000: &str = "/user.slice/user-1000.slice/user@1000.service";

    fn check(cgroup: &str, image: FixtureImage<'_>) -> ShellCheck<FixtureProc> {
        let proc = FixtureProc::new().process(FixtureProcess {
            pid: Pid::new(PID).unwrap(),
            cgroup: &format!("0::{cgroup}\n"),
            image,
            flatpak_info: None,
        });
        ShellCheck::new(
            Prober::new(proc),
            Table::from_toml(SHELL_TABLE).unwrap(),
            vec![SHELL],
        )
    }

    fn held(uid: u32) -> Pinned {
        Pinned::introduced(Pid::new(PID).unwrap(), Uid::new(uid), Pin::Reopened)
    }

    fn claim_of(cgroup: &str, image: FixtureImage<'_>) -> Claim {
        check(cgroup, image).origin_of(held(1000)).claim()
    }

    #[test]
    fn only_the_shells_unit_under_the_users_manager_claims_the_shell() {
        let shell = Claim::Shell { user: 1000 };
        let cases: &[(&str, String, Claim)] = &[
            (
                "the shell",
                format!("{MANAGER_1000}/session.slice/sill.service"),
                shell,
            ),
            (
                "another unit",
                format!("{MANAGER_1000}/app.slice/mailo.service"),
                Claim::Other,
            ),
            (
                "a scope",
                format!("{MANAGER_1000}/app.slice/app-sill-1.scope"),
                Claim::Other,
            ),
        ];
        for (name, cgroup, want) in cases {
            assert_eq!(claim_of(cgroup, FixtureImage::Hidden), *want, "{name}");
        }
    }

    #[test]
    fn a_readable_image_must_be_the_shells_and_root_owned() {
        let cgroup = format!("{MANAGER_1000}/session.slice/sill.service");
        let shell = Claim::Shell { user: 1000 };
        let cases: &[(&str, FixtureImage<'_>, Claim)] = &[
            (
                "the installed shell",
                FixtureImage::Present("/usr/bin/sill", FileTrust::RootOnly),
                shell,
            ),
            (
                "another program in its unit",
                FixtureImage::Present("/usr/bin/sh", FileTrust::RootOnly),
                Claim::Other,
            ),
            (
                "the shell's path, but writable",
                FixtureImage::Present("/usr/bin/sill", FileTrust::Writable(Writer::Group)),
                Claim::Other,
            ),
        ];
        for (name, image, want) in cases {
            assert_eq!(claim_of(&cgroup, *image), *want, "{name}");
        }
    }

    #[test]
    fn only_the_exact_word_asks_for_the_seat_user() {
        const CASES: &[(Option<&str>, Steering)] = &[
            (None, Steering::ShellUnit),
            (Some(""), Steering::ShellUnit),
            (Some("1"), Steering::ShellUnit),
            (Some("seat-user"), Steering::SeatUser),
        ];
        for &(value, want) in CASES {
            assert_eq!(Steering::from_env_value(value), want, "{value:?}");
        }
    }
}
