//! What the daemon and its verbs take from the process environment, read once in
//! `command::run` so nothing below it reads `std::env`.

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
            Option<&'static str>,
            Desktop,
        );
        const CASES: &[Case] = &[
            (
                "every variable read from the lookup",
                &[
                    ("HOME", "/home/u"),
                    ("XDG_RUNTIME_DIR", "/run/user/1"),
                    ("XDG_CURRENT_DESKTOP", "KDE"),
                ],
                Some("/home/u/.config/palmrest"),
                Some("/run/user/1"),
                Desktop::Kde,
            ),
            (
                "an empty environment has no directories",
                &[("XDG_RUNTIME_DIR", "")],
                None,
                None,
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
            assert_eq!(
                env.runtime_dir.as_deref(),
                runtime_dir.map(Path::new),
                "{name}"
            );
            assert_eq!(env.desktop, desktop, "{name}");
        }
    }
}
