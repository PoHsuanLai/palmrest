//! What the daemon and its verbs take from the process environment, read once in
//! `command::run` so nothing below it reads `std::env`.

use super::socket::{SEAT_FILE, SeatFile, Steering};
use crate::action::Desktop;
use crate::config;
use std::path::PathBuf;

/// Where the socket directory is when systemd does not provide one.
const DEFAULT_RUNTIME_DIR: &str = "/run/palmrest";

/// The process environment as the daemon uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
    /// `$CONFIGURATION_DIRECTORY` (systemd's, `/etc/palmrest` for the unit), else
    /// `$XDG_CONFIG_HOME/palmrest`, else `~/.config/palmrest` for a tool run by a person; none
    /// when none is set.
    pub config_dir: Option<PathBuf>,
    /// `$RUNTIME_DIRECTORY` (systemd's), else `/run/palmrest`: where the socket lives.
    pub runtime_dir: PathBuf,
    /// `$PALMREST_SEAT_FILE`, else logind's `seat0` state file: where the person at the seat is
    /// read from; injectable so tests need no logind.
    pub seat_file: SeatFile,
    /// `$PALMREST_STEERING`: which connection is believed about the lock screen (only
    /// `seat-user` changes the default, for a development run outside the shell's unit).
    pub steering: Steering,
    /// The desktop an action is planned for (`$XDG_CURRENT_DESKTOP`).
    pub desktop: Desktop,
}

impl Env {
    /// Reads every variable through `lookup`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            config_dir: config::config_dir(&lookup),
            runtime_dir: config::first_directory(lookup("RUNTIME_DIRECTORY"))
                .unwrap_or_else(|| PathBuf::from(DEFAULT_RUNTIME_DIR)),
            seat_file: SeatFile(
                lookup("PALMREST_SEAT_FILE")
                    .filter(|s| !s.is_empty())
                    .map_or_else(|| PathBuf::from(SEAT_FILE), PathBuf::from),
            ),
            steering: Steering::from_env_value(lookup("PALMREST_STEERING").as_deref()),
            desktop: Desktop::from_env_value(&lookup("XDG_CURRENT_DESKTOP").unwrap_or_default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn lookup_table() {
        // (name, variables set, config dir, runtime dir, desktop)
        type Case = (
            &'static str,
            &'static [(&'static str, &'static str)],
            Option<&'static str>,
            &'static str,
            Desktop,
        );
        const CASES: &[Case] = &[
            (
                "every variable read from the lookup",
                &[
                    ("HOME", "/home/u"),
                    ("RUNTIME_DIRECTORY", "/run/palmrest"),
                    ("XDG_CURRENT_DESKTOP", "KDE"),
                ],
                Some("/home/u/.config/palmrest"),
                "/run/palmrest",
                Desktop::Kde,
            ),
            (
                "the unit's directories win over the home folder",
                &[
                    ("HOME", "/home/u"),
                    ("CONFIGURATION_DIRECTORY", "/etc/palmrest"),
                    ("RUNTIME_DIRECTORY", "/run/x:/run/y"),
                ],
                Some("/etc/palmrest"),
                "/run/x",
                Desktop::Other,
            ),
            (
                "an empty environment has the system runtime directory and no config",
                &[("RUNTIME_DIRECTORY", "")],
                None,
                "/run/palmrest",
                Desktop::Other,
            ),
        ];
        for &(name, vars, config_dir, runtime_dir, desktop) in CASES {
            let env = Env::from_lookup(|k| {
                vars.iter()
                    .find(|(var, _)| *var == k)
                    .map(|(_, v)| (*v).to_owned())
            });
            assert_eq!(
                env.config_dir.as_deref(),
                config_dir.map(Path::new),
                "{name}"
            );
            assert_eq!(env.runtime_dir, Path::new(runtime_dir), "{name}");
            assert_eq!(env.desktop, desktop, "{name}");
        }
    }
}
