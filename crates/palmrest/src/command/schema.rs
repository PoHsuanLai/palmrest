//! `palmrest --write-schema DIR`: writes the schema of `gestures.toml` for the Settings app.

use crate::config::{SCHEMA_FILE_NAME, Schema};
use crate::error::Error;
use std::path::Path;

/// Writes `palmrest.settings.toml` into `dir`, creating it, by a temporary file and a rename
/// (the way every settings file is written), and prints where it went.
pub(super) fn write_schema(dir: &Path) -> Result<(), Error> {
    let target = write_to(dir).map_err(|source| Error::Schema {
        dir: dir.to_path_buf(),
        source,
    })?;
    println!("{}", target.display());
    Ok(())
}

fn write_to(dir: &Path) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let target = dir.join(SCHEMA_FILE_NAME);
    let temp = dir.join(format!(".{SCHEMA_FILE_NAME}.part"));
    std::fs::write(&temp, Schema::palmrest().to_toml())?;
    std::fs::rename(&temp, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::write_to;

    #[test]
    fn the_schema_lands_in_a_directory_it_creates() {
        let dir = std::env::temp_dir().join(format!("palmrest-schema-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let target = write_to(&dir.join("nested")).unwrap();
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(text.contains("app = \"palmrest\""));
        assert!(!dir.join("nested/.palmrest.settings.toml.part").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
