//! Configuration loading for `christen`.
//!
//! Config lives at `~/.config/christen/christen.toml` (optional).
//! When the file is absent, documented defaults are used.
//! Loading is pure — no filesystem scan, no unit parse, no `/proc` read.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The christen configuration, loaded from `christen.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChristenConfig {
    /// Budget string injected into every `Wire` action.
    /// Default: `"wall=7200s,fork=2000"`.
    #[serde(default = "default_budget")]
    pub default_budget: String,

    /// Directory to scan for systemd user units.
    /// Default: `~/.config/systemd/user`.
    #[serde(default = "default_systemd_dir")]
    pub systemd_dir: PathBuf,

    /// Override the derived intent tag for a given site id.
    /// Keys are site ids (e.g. `"claude-build.service"`), values are intent tags (e.g. `"/build"`).
    #[serde(default)]
    pub intent_overrides: HashMap<String, String>,
}

fn default_budget() -> String {
    "wall=7200s,fork=2000".to_owned()
}

fn default_systemd_dir() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| PathBuf::from("/home/jsy"))
        .join(".config/systemd/user")
}

impl Default for ChristenConfig {
    fn default() -> Self {
        Self {
            default_budget: default_budget(),
            systemd_dir: default_systemd_dir(),
            intent_overrides: HashMap::new(),
        }
    }
}

/// Error type for config loading failures.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The file could not be read.
    #[error("failed to read config file at {path}: {source}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The TOML could not be parsed.
    #[error("failed to parse config file at {path}: {source}")]
    Parse {
        /// Path that failed.
        path: PathBuf,
        /// Underlying TOML error.
        #[source]
        source: toml::de::Error,
    },
}

impl ChristenConfig {
    /// Loads configuration from `path`.
    ///
    /// If the file does not exist, returns [`ChristenConfig::default()`] silently.
    /// Any other I/O error or parse failure is returned as [`ConfigError`].
    ///
    /// # Errors
    /// Returns [`ConfigError`] if the file exists but cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).map_err(|source| ConfigError::Parse {
                path: path.to_owned(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Returns the configured or default path for the christen config file.
    ///
    /// Checks `$XDG_CONFIG_HOME/christen/christen.toml` first,
    /// then `~/.config/christen/christen.toml`.
    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs_next::config_dir()
            .unwrap_or_else(|| {
                dirs_next::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/home/jsy"))
                    .join(".config")
            })
            .join("christen/christen.toml")
    }
}
