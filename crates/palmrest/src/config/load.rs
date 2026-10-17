//! Where the file lives and how it is read: `/etc/palmrest/gestures.toml` for the system service
//! (its `ConfigurationDirectory=`), else `$XDG_CONFIG_HOME/palmrest/gestures.toml` for a tool run
//! by a person (22-SETTINGS §2).

use super::file::{Config, parse};
use super::lenient::Warning;
use rustix::fs::OFlags;
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The file name the Settings app writes.
pub const FILE_NAME: &str = "gestures.toml";

/// The most the file may hold; the real one is a few kilobytes, so more is not a settings file.
pub const FILE_MAX_BYTES: u64 = 256 * 1024;

/// The directory holding the file: the service's `$CONFIGURATION_DIRECTORY`, else
/// `$XDG_CONFIG_HOME/palmrest`, else `~/.config/palmrest`.
pub fn config_dir(lookup: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(dir) = first_directory(lookup("CONFIGURATION_DIRECTORY")) {
        return Some(dir);
    }
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

/// The first directory of one of systemd's directory variables (`$CONFIGURATION_DIRECTORY`,
/// `$RUNTIME_DIRECTORY`: colon separated, absolute).
pub fn first_directory(value: Option<String>) -> Option<PathBuf> {
    value?
        .split(':')
        .next()
        .filter(|s| s.starts_with('/'))
        .map(PathBuf::from)
}

/// What a load produced: the config, which file it came from, and every complaint.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub config: Config,
    /// The file's text when it read and parsed, else empty: what the person's layer is laid over.
    pub text: String,
    pub source: Option<PathBuf>,
    pub warnings: Vec<Warning>,
}

/// Reads the file from `dir` (if any). A missing file is the defaults; an unreadable or
/// non-TOML file is the defaults plus a warning, because the mouse must keep scrolling whatever
/// the file says.
pub fn load(dir: Option<&Path>) -> Loaded {
    let found = dir.map(|d| d.join(FILE_NAME)).filter(|p| p.exists());
    let (config, text, warnings) = match &found {
        None => (Config::default(), String::new(), Vec::new()),
        Some(p) => match read_regular(p) {
            Ok(text) => match parse(&text) {
                Ok((config, warnings)) => (config, text, warnings),
                Err(e) => (
                    Config::default(),
                    String::new(),
                    vec![whole_file(p, e.to_string())],
                ),
            },
            Err(e) => (
                Config::default(),
                String::new(),
                vec![whole_file(p, e.to_string())],
            ),
        },
    };
    Loaded {
        config,
        text,
        source: found,
        warnings,
    }
}

/// The text of a regular file of at most `FILE_MAX_BYTES`. Opened without blocking, so a named
/// pipe put in its place is refused instead of waiting for a writer for ever; a symbolic link to
/// a regular file (a dotfile manager's) is followed.
fn read_regular(path: &Path) -> io::Result<String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(OFlags::NONBLOCK.bits() as i32)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let mut text = String::new();
    file.take(FILE_MAX_BYTES + 1).read_to_string(&mut text)?;
    if text.len() as u64 > FILE_MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("larger than {FILE_MAX_BYTES} bytes"),
        ));
    }
    Ok(text)
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
    fn the_units_directory_wins_and_only_an_absolute_first_entry_counts() {
        let unit = |k: &str| match k {
            "CONFIGURATION_DIRECTORY" => Some("/etc/palmrest:/etc/other".to_owned()),
            "HOME" => Some("/h".to_owned()),
            _ => None,
        };
        assert_eq!(config_dir(unit), Some(PathBuf::from("/etc/palmrest")));
        assert_eq!(first_directory(Some("relative".to_owned())), None);
        assert_eq!(first_directory(Some(String::new())), None);
        assert_eq!(first_directory(None), None);
    }

    #[test]
    fn missing_dir_is_defaults() {
        let l = load(None);
        assert_eq!(l.config, Config::default());
        assert!(l.warnings.is_empty());
    }

    #[test]
    fn a_pipe_or_a_huge_file_is_the_defaults_with_a_warning() {
        let dir = std::env::temp_dir().join(format!("palmrest-load-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(FILE_NAME);
        rustix::fs::mknodat(
            rustix::fs::CWD,
            file.as_path(),
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        let pipe = load(Some(&dir));
        assert_eq!(pipe.config, Config::default());
        assert_eq!(pipe.warnings.len(), 1, "{:?}", pipe.warnings);

        std::fs::remove_file(&file).unwrap();
        std::fs::write(&file, vec![b'#'; FILE_MAX_BYTES as usize + 1]).unwrap();
        let huge = load(Some(&dir));
        assert_eq!(huge.config, Config::default());
        assert_eq!(huge.warnings.len(), 1, "{:?}", huge.warnings);

        std::fs::write(&file, "# nothing to say\n").unwrap();
        assert!(load(Some(&dir)).warnings.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
