//! Reading one settings key leniently (22-SETTINGS §2).
//!
//! A bad value costs that key only: it is reported as a `Warning` and the key's shipped default
//! is used, so one typo in the Settings app never takes the daemon's whole configuration down.
//! The warnings are returned, not printed, so loading stays a pure function of the text.

use serde::de::DeserializeOwned;
use std::fmt;
use toml::{Table, Value};

/// A key that was not read as written, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub key: String,
    pub problem: String,
    pub outcome: Outcome,
}

/// What was done about a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The value was unusable; the key's default is in force.
    DefaultUsed,
    /// A deprecated key was read and converted to its replacement.
    Converted,
    /// A deprecated key was ignored because its replacement is set.
    Ignored,
}

impl Warning {
    /// An unusable value: the default stands.
    pub fn unusable(key: impl Into<String>, problem: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            problem: problem.into(),
            outcome: Outcome::DefaultUsed,
        }
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.outcome {
            Outcome::DefaultUsed => {
                write!(f, "{}: {} (using the default)", self.key, self.problem)
            }
            Outcome::Converted | Outcome::Ignored => write!(f, "{}: {}", self.key, self.problem),
        }
    }
}

/// A value that can be read from one TOML value.
pub trait Setting: Sized {
    /// The value, or `None` with a warning pushed.
    fn read(v: &Value, key: &str, warn: &mut Vec<Warning>) -> Option<Self>;
}

/// Reads any serde type; the default `Setting` for leaf values.
pub fn read_serde<T: DeserializeOwned>(v: &Value, key: &str, warn: &mut Vec<Warning>) -> Option<T> {
    match v.clone().try_into::<T>() {
        Ok(t) => Some(t),
        Err(e) => {
            warn.push(Warning::unusable(key, e.to_string().trim()));
            None
        }
    }
}

/// Implements `Setting` through serde for leaf types.
#[macro_export]
macro_rules! serde_settings {
    ($($t:ty),* $(,)?) => {
        $(impl $crate::config::lenient::Setting for $t {
            fn read(v: &toml::Value, key: &str, warn: &mut Vec<$crate::config::lenient::Warning>) -> Option<Self> {
                $crate::config::lenient::read_serde(v, key, warn)
            }
        })*
    };
}

/// The dotted name of `key` inside the table at `path`.
pub fn dotted(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

/// Reads `key` of `table`, falling back to `default` when absent or unusable.
pub fn field<T: Setting>(
    table: &Table,
    path: &str,
    key: &str,
    default: T,
    warn: &mut Vec<Warning>,
) -> T {
    match table.get(key) {
        None => default,
        Some(v) => T::read(v, &dotted(path, key), warn).unwrap_or(default),
    }
}

/// Like `field`, then rejects a value outside `lo..=hi` in favour of the default.
pub fn ranged<T: Setting + PartialOrd + fmt::Debug>(
    table: &Table,
    path: &str,
    key: &str,
    default: T,
    (lo, hi): (T, T),
    warn: &mut Vec<Warning>,
) -> T {
    let Some(v) = table.get(key) else {
        return default;
    };
    let name = dotted(path, key);
    match T::read(v, &name, warn) {
        Some(t) if t >= lo && t <= hi => t,
        Some(t) => {
            warn.push(Warning::unusable(
                name,
                format!("{t:?} is outside {lo:?}..={hi:?}"),
            ));
            default
        }
        None => default,
    }
}

/// The sub-table `key` of `table`, or an empty one (with a warning if `key` is not a table).
pub fn section(table: &Table, path: &str, key: &str, warn: &mut Vec<Warning>) -> Table {
    match table.get(key) {
        None => Table::new(),
        Some(Value::Table(t)) => t.clone(),
        Some(_) => {
            warn.push(Warning::unusable(dotted(path, key), "is not a table"));
            Table::new()
        }
    }
}

/// Declares a settings struct: one field per key, its shipped default and optional range,
/// a `Default` impl built from those defaults, and a lenient `from_table`.
#[macro_export]
macro_rules! settings_struct {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(
                $(#[$fmeta:meta])*
                $field:ident $(as $key:literal)? : $ty:ty = $default:expr $(, in $lo:expr => $hi:expr)?;
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, serde::Serialize)]
        pub struct $name {
            $(
                $(#[$fmeta])*
                $(#[serde(rename = $key)])?
                pub $field: $ty,
            )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: $default,)* }
            }
        }

        impl $name {
            /// Reads this section from its table, key by key, defaulting what is missing or bad.
            pub fn from_table(
                t: &toml::Table,
                path: &str,
                warn: &mut Vec<$crate::config::lenient::Warning>,
            ) -> Self {
                let d = Self::default();
                Self {
                    $($field: $crate::settings_struct!(@read t, path, warn, d.$field,
                        $crate::settings_struct!(@key $field $($key)?) $(, ($lo, $hi))?),)*
                }
            }
        }
    };
    (@key $field:ident) => { stringify!($field) };
    (@key $field:ident $key:literal) => { $key };
    (@read $t:ident, $path:ident, $warn:ident, $d:expr, $key:expr) => {
        $crate::config::lenient::field($t, $path, $key, $d, $warn)
    };
    (@read $t:ident, $path:ident, $warn:ident, $d:expr, $key:expr, $range:expr) => {
        $crate::config::lenient::ranged($t, $path, $key, $d, $range, $warn)
    };
}
