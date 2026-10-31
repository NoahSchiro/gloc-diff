//! The `.gloc.toml` config file.
//!
//! Same options as the CLI, read from the repo root.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::filter::Options;

/// Options as written in `.gloc.toml`. Exact same as CLI flags.
#[derive(Deserialize, Debug, Default)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct FileOptions {
    include: Vec<String>,
    exclude: Vec<String>,
    include_re: Vec<String>,
    exclude_re: Vec<String>,
    no_gitignore: bool,
}

impl FileOptions {
    fn into_options(self) -> Options {
        Options {
            include: self.include,
            exclude: self.exclude,
            include_re: self.include_re,
            exclude_re: self.exclude_re,
            no_gitignore: self.no_gitignore,
        }
    }
}

/// Why `.gloc.toml` could not be turned into options.
#[derive(Debug)]
pub enum ConfigError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, source: toml::de::Error },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Read { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            ConfigError::Parse { path, source } => {
                write!(f, "invalid TOML in {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Read { source, .. } => Some(source),
            ConfigError::Parse { source, .. } => Some(source),
        }
    }
}

/// Read `.gloc.toml` from the repo root.
pub fn load(repo_root: &Path) -> Result<Options, ConfigError> {
    let path = repo_root.join(".gloc.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(Options::default())
        }
        Err(source) => return Err(ConfigError::Read { path, source }),
    };
    toml::from_str::<FileOptions>(&text)
        .map(FileOptions::into_options)
        .map_err(|source| ConfigError::Parse { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A throwaway directory under the OS temp dir, removed on drop.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("gloc-config-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create temp dir");
            TempRoot(dir)
        }

        fn write(&self, rel: &str, contents: &str) {
            fs::write(self.0.join(rel), contents).expect("write file");
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_file_means_default_options() {
        let root = TempRoot::new();
        assert_eq!(load(root.0.as_path()).unwrap(), Options::default());
    }

    #[test]
    fn keys_mirror_the_cli_flags() {
        let root = TempRoot::new();
        root.write(
            ".gloc.toml",
            concat!(
                "include = [\"src/**\"]\n",
                "exclude = [\"**/*.lock\"]\n",
                "include-re = [\"^lib\"]\n",
                "exclude-re = [\"vend\"]\n",
                "no-gitignore = true\n",
            ),
        );
        let options = load(root.0.as_path()).unwrap();
        assert_eq!(options.include, ["src/**"]);
        assert_eq!(options.exclude, ["**/*.lock"]);
        assert_eq!(options.include_re, ["^lib"]);
        assert_eq!(options.exclude_re, ["vend"]);
        assert!(options.no_gitignore);
    }

    #[test]
    fn unknown_keys_are_an_error() {
        let root = TempRoot::new();
        root.write(".gloc.toml", "exlude = [\"x\"]\n");
        assert!(matches!(
            load(root.0.as_path()),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn malformed_toml_is_an_error() {
        let root = TempRoot::new();
        root.write(".gloc.toml", "exclude = [");
        assert!(matches!(
            load(root.0.as_path()),
            Err(ConfigError::Parse { .. })
        ));
    }
}
