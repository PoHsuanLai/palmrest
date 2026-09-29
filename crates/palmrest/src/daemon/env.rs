//! What the daemon and its verbs take from the process environment, read once in
//! `command::run` so nothing below it reads `std::env`.

use super::uinput::{PROBE_PERMIT, ProbePermit};
use crate::action::Desktop;
use crate::config;
use std::path::PathBuf;

/// The process environment as the daemon uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
    /// `$XDG_CONFIG_HOME/palmrest`, else `~/.config/palmrest`; none when neither is set.
    pub config_dir: Option<PathBuf>,
    /// `$XDG_RUNTIME_DIR`, where the socket lives; none when unset or empty.
    pub runtime_dir: Option<PathBuf>,
    /// The desktop an action is planned for (`$XDG_CURRENT_DESKTOP`).
    pub desktop: Desktop,
    /// Whether `--check-uinput` may open the uinput node.
    pub probe: ProbePermit,
}

impl Env {
    /// Reads every variable through `lookup`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            config_dir: config::config_dir(&lookup),
            runtime_dir: lookup("XDG_RUNTIME_DIR")
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
            desktop: Desktop::from_env_value(&lookup("XDG_CURRENT_DESKTOP").unwrap_or_default()),
            probe: ProbePermit::from_env_value(lookup(PROBE_PERMIT).as_deref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn reads_each_variable_from_the_lookup() {
        let env = Env::from_lookup(|k| {
            match k {
                "HOME" => Some("/home/u"),
                "XDG_RUNTIME_DIR" => Some("/run/user/1"),
                "XDG_CURRENT_DESKTOP" => Some("KDE"),
                PROBE_PERMIT => Some("1"),
                _ => None,
            }
            .map(str::to_owned)
        });
        assert_eq!(
            env.config_dir.as_deref(),
            Some(Path::new("/home/u/.config/palmrest"))
        );
        assert_eq!(env.runtime_dir.as_deref(), Some(Path::new("/run/user/1")));
        assert_eq!(env.desktop, Desktop::Kde);
        assert_eq!(env.probe, ProbePermit::Granted);
    }

    #[test]
    fn an_empty_environment_has_no_directories() {
        let env = Env::from_lookup(|k| (k == "XDG_RUNTIME_DIR").then(String::new));
        assert_eq!(env.config_dir, None);
        assert_eq!(env.runtime_dir, None);
        assert_eq!(env.desktop, Desktop::Other);
        assert_eq!(env.probe, ProbePermit::Refused);
    }
}
