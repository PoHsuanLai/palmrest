//! Where the file lives and how it is read: `$XDG_CONFIG_HOME/palmrest/gestures.toml`
//! (22-SETTINGS §2).

use super::file::{Config, parse};
use super::lenient::Warning;
use std::path::{Path, PathBuf};

/// The file name the Settings app writes.
pub const FILE_NAME: &str = "gestures.toml";

/// The directory holding the file: `$XDG_CONFIG_HOME/palmrest`, else `~/.config/palmrest`.
pub fn config_dir(lookup: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let base = lookup("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            lookup("HOME")
                .filter(|s| !s.is_empty())
                .map(|h| Path::new(&h).join(".config"))
        })?;
    Some(base.join("palmrest"))
}

/// What a load produced: the config, which file it came from, and every complaint.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub config: Config,
    pub source: Option<PathBuf>,
    pub warnings: Vec<Warning>,
}

/// Reads the file from `dir` (if any). A missing file is the defaults; an unreadable or
/// non-TOML file is the defaults plus a warning, because the mouse must keep scrolling whatever
/// the file says.
pub fn load(dir: Option<&Path>) -> Loaded {
    let found = dir.map(|d| d.join(FILE_NAME)).filter(|p| p.exists());
    let (config, warnings) = match &found {
        None => (Config::default(), Vec::new()),
        Some(p) => match std::fs::read_to_string(p) {
            Ok(text) => parse(&text)
                .unwrap_or_else(|e| (Config::default(), vec![whole_file(p, e.to_string())])),
            Err(e) => (Config::default(), vec![whole_file(p, e.to_string())]),
        },
    };
    Loaded {
        config,
        source: found,
        warnings,
    }
}

fn whole_file(p: &Path, problem: String) -> Warning {
    Warning::unusable(p.display().to_string(), problem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_prefers_xdg_then_home() {
        let both = |k: &str| match k {
            "XDG_CONFIG_HOME" => Some("/x".to_owned()),
            "HOME" => Some("/h".to_owned()),
            _ => None,
        };
        assert_eq!(config_dir(both), Some(PathBuf::from("/x/palmrest")));
        let home = |k: &str| (k == "HOME").then(|| "/h".to_owned());
        assert_eq!(config_dir(home), Some(PathBuf::from("/h/.config/palmrest")));
        assert_eq!(config_dir(|_| None), None);
    }

    #[test]
    fn missing_dir_is_defaults() {
        let l = load(None);
        assert_eq!(l.config, Config::default());
        assert!(l.warnings.is_empty());
    }
}
