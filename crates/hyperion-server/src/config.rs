//! Server configuration: where to listen, where saves live, how many workers, how much cache.
//!
//! [`ServerConfig::from_env`] reads the environment and fills in defaults; tests and embedders
//! build one with [`ServerConfig::builder`].
//!
//! | Variable                 | Default                                         |
//! | ------------------------ | ----------------------------------------------- |
//! | `HYPERION_ADDR`          | [`DEFAULT_ADDR`]                                |
//! | `HYPERION_DATA_DIR`      | `./hyperion-data`                               |
//! | `HYPERION_WORKERS`       | available parallelism less one, at least one    |
//! | `HYPERION_CELL_CACHE_MB` | 256 (MiB)                                       |
//! | `HYPERION_MAP_CACHE_MB`  | 64 (MiB)                                        |

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::DEFAULT_ADDR;
use crate::universe::{Entropy, OsEntropy};

/// The variable naming the address to listen on.
pub const ENV_ADDR: &str = "HYPERION_ADDR";
/// The variable naming the data directory, which holds the saves.
pub const ENV_DATA_DIR: &str = "HYPERION_DATA_DIR";
/// The variable giving the number of CPU pool workers.
pub const ENV_WORKERS: &str = "HYPERION_WORKERS";
/// The variable giving the cell cache's budget in MiB.
pub const ENV_CELL_CACHE_MB: &str = "HYPERION_CELL_CACHE_MB";
/// The variable giving the density map cache's budget in MiB.
pub const ENV_MAP_CACHE_MB: &str = "HYPERION_MAP_CACHE_MB";

/// The data directory when `HYPERION_DATA_DIR` is not set, relative to the working directory.
pub const DEFAULT_DATA_DIR: &str = "./hyperion-data";
/// The cell cache's budget when `HYPERION_CELL_CACHE_MB` is not set, in MiB.
pub const DEFAULT_CELL_CACHE_MIB: usize = 256;
/// The density map cache's budget when `HYPERION_MAP_CACHE_MB` is not set, in MiB.
pub const DEFAULT_MAP_CACHE_MIB: usize = 64;

/// Bytes in a MiB, the unit of the cache variables.
const BYTES_PER_MIB: usize = 1 << 20;

/// Everything the server is started with.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    addr: String,
    data_dir: PathBuf,
    workers: NonZeroUsize,
    cell_cache_bytes: usize,
    map_cache_bytes: usize,
    entropy: Arc<dyn Entropy>,
}

impl ServerConfig {
    /// A builder holding every default.
    #[must_use]
    pub fn builder() -> ServerConfigBuilder {
        ServerConfigBuilder::default()
    }

    /// Reads the configuration from the environment, taking the default for each variable that is
    /// not set.
    ///
    /// # Errors
    ///
    /// [`ParseConfigError`] naming the first variable that is set but not usable.
    pub fn from_env() -> Result<Self, ParseConfigError> {
        Self::from_vars(|name| std::env::var_os(name))
    }

    /// [`ServerConfig::from_env`] over any source of variables, so that tests need not touch the
    /// process environment.
    pub(crate) fn from_vars(
        var: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, ParseConfigError> {
        let mut builder = Self::builder();
        if let Some(addr) = var(ENV_ADDR) {
            builder = builder.addr(unicode(ENV_ADDR, addr)?);
        }
        if let Some(dir) = var(ENV_DATA_DIR) {
            // An empty path would silently mean the working directory.
            if dir.is_empty() {
                return Err(ParseConfigError::InvalidValue {
                    variable: ENV_DATA_DIR,
                    value: String::new(),
                    expected: "a directory path",
                });
            }
            builder = builder.data_dir(dir);
        }
        if let Some(workers) = var(ENV_WORKERS) {
            let text = unicode(ENV_WORKERS, workers)?;
            let workers =
                text.parse::<NonZeroUsize>()
                    .map_err(|_| ParseConfigError::InvalidValue {
                        variable: ENV_WORKERS,
                        value: text,
                        expected: "a whole number of at least 1",
                    })?;
            builder = builder.workers(workers);
        }
        if let Some(mib) = var(ENV_CELL_CACHE_MB) {
            builder = builder.cell_cache_bytes(mebibytes(ENV_CELL_CACHE_MB, mib)?);
        }
        if let Some(mib) = var(ENV_MAP_CACHE_MB) {
            builder = builder.map_cache_bytes(mebibytes(ENV_MAP_CACHE_MB, mib)?);
        }
        Ok(builder.build())
    }

    /// The address to listen on, as `host:port`.
    #[must_use]
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// The data directory. It is created with the first universe, not before.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Worker threads in the CPU pool.
    #[must_use]
    pub fn workers(&self) -> NonZeroUsize {
        self.workers
    }

    /// The cell cache's budget, in bytes.
    #[must_use]
    pub fn cell_cache_bytes(&self) -> usize {
        self.cell_cache_bytes
    }

    /// The density map cache's budget, in bytes.
    #[must_use]
    pub fn map_cache_bytes(&self) -> usize {
        self.map_cache_bytes
    }

    /// Where seeds and universe IDs are drawn from.
    #[must_use]
    pub fn entropy(&self) -> &Arc<dyn Entropy> {
        &self.entropy
    }
}

/// Builds a [`ServerConfig`], starting from the defaults.
#[derive(Debug, Clone)]
pub struct ServerConfigBuilder {
    config: ServerConfig,
}

impl Default for ServerConfigBuilder {
    fn default() -> Self {
        Self {
            config: ServerConfig {
                addr: DEFAULT_ADDR.to_owned(),
                data_dir: PathBuf::from(DEFAULT_DATA_DIR),
                workers: default_workers(),
                cell_cache_bytes: DEFAULT_CELL_CACHE_MIB * BYTES_PER_MIB,
                map_cache_bytes: DEFAULT_MAP_CACHE_MIB * BYTES_PER_MIB,
                entropy: Arc::new(OsEntropy),
            },
        }
    }
}

impl ServerConfigBuilder {
    /// The address to listen on, as `host:port`.
    #[must_use]
    pub fn addr(mut self, addr: impl Into<String>) -> Self {
        self.config.addr = addr.into();
        self
    }

    /// The data directory.
    #[must_use]
    pub fn data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.data_dir = dir.into();
        self
    }

    /// Worker threads in the CPU pool.
    #[must_use]
    pub fn workers(mut self, workers: NonZeroUsize) -> Self {
        self.config.workers = workers;
        self
    }

    /// The cell cache's budget, in bytes. Zero caches nothing.
    #[must_use]
    pub fn cell_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.cell_cache_bytes = bytes;
        self
    }

    /// The density map cache's budget, in bytes. Zero caches nothing.
    #[must_use]
    pub fn map_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.map_cache_bytes = bytes;
        self
    }

    /// Where seeds and universe IDs are drawn from; [`OsEntropy`] by default. Tests inject a
    /// [`SequenceEntropy`](crate::universe::SequenceEntropy).
    #[must_use]
    pub fn entropy(mut self, entropy: impl Entropy + 'static) -> Self {
        self.config.entropy = Arc::new(entropy);
        self
    }

    /// The configuration.
    #[must_use]
    pub fn build(self) -> ServerConfig {
        self.config
    }
}

/// A configuration variable is set but cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParseConfigError {
    /// The value is not valid Unicode.
    NotUnicode {
        /// The variable.
        variable: &'static str,
    },
    /// The value does not have the form the variable needs.
    InvalidValue {
        /// The variable.
        variable: &'static str,
        /// The value it holds.
        value: String,
        /// What it should hold.
        expected: &'static str,
    },
}

impl fmt::Display for ParseConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUnicode { variable } => write!(f, "{variable} is not valid unicode"),
            Self::InvalidValue {
                variable,
                value,
                expected,
            } => write!(f, "{variable}={value:?} is not {expected}"),
        }
    }
}

impl Error for ParseConfigError {}

/// Available parallelism less one, for the runtime and the sockets, and at least one.
fn default_workers() -> NonZeroUsize {
    std::thread::available_parallelism()
        .ok()
        .and_then(|cores| NonZeroUsize::new(cores.get() - 1))
        .unwrap_or(NonZeroUsize::MIN)
}

fn unicode(variable: &'static str, value: OsString) -> Result<String, ParseConfigError> {
    value
        .into_string()
        .map_err(|_| ParseConfigError::NotUnicode { variable })
}

fn mebibytes(variable: &'static str, value: OsString) -> Result<usize, ParseConfigError> {
    let text = unicode(variable, value)?;
    text.parse::<usize>()
        .ok()
        .and_then(|mib| mib.checked_mul(BYTES_PER_MIB))
        .ok_or(ParseConfigError::InvalidValue {
            variable,
            value: text,
            expected: "a whole number of MiB that fits in memory",
        })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn from(vars: &[(&str, &str)]) -> Result<ServerConfig, ParseConfigError> {
        let vars: HashMap<String, OsString> = vars
            .iter()
            .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
            .collect();
        ServerConfig::from_vars(|name| vars.get(name).cloned())
    }

    #[test]
    fn from_env_defaults() {
        let config = from(&[]).unwrap();
        assert_eq!(config.addr(), DEFAULT_ADDR);
        assert_eq!(config.data_dir(), Path::new("./hyperion-data"));
        assert_eq!(config.workers(), default_workers());
        assert_eq!(config.cell_cache_bytes(), 256 * 1024 * 1024);
        assert_eq!(config.map_cache_bytes(), 64 * 1024 * 1024);
        let cores = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
        assert_eq!(config.workers().get(), cores.saturating_sub(1).max(1));
    }

    #[test]
    fn from_env_reads_every_variable() {
        let config = from(&[
            ("HYPERION_ADDR", "0.0.0.0:9000"),
            ("HYPERION_DATA_DIR", "/srv/hyperion"),
            ("HYPERION_WORKERS", "3"),
            ("HYPERION_CELL_CACHE_MB", "1"),
            ("HYPERION_MAP_CACHE_MB", "0"),
        ])
        .unwrap();
        assert_eq!(config.addr(), "0.0.0.0:9000");
        assert_eq!(config.data_dir(), Path::new("/srv/hyperion"));
        assert_eq!(config.workers().get(), 3);
        assert_eq!(config.cell_cache_bytes(), 1 << 20);
        assert_eq!(config.map_cache_bytes(), 0);
    }

    #[test]
    fn a_bad_worker_count_is_refused() {
        assert_eq!(
            from(&[("HYPERION_WORKERS", "four")]).unwrap_err(),
            ParseConfigError::InvalidValue {
                variable: "HYPERION_WORKERS",
                value: "four".to_owned(),
                expected: "a whole number of at least 1",
            }
        );
        assert_eq!(
            from(&[("HYPERION_WORKERS", "-2")]).unwrap_err().to_string(),
            r#"HYPERION_WORKERS="-2" is not a whole number of at least 1"#
        );
    }

    #[test]
    fn zero_workers_are_refused() {
        assert_eq!(
            from(&[("HYPERION_WORKERS", "0")]).unwrap_err(),
            ParseConfigError::InvalidValue {
                variable: "HYPERION_WORKERS",
                value: "0".to_owned(),
                expected: "a whole number of at least 1",
            }
        );
    }

    #[test]
    fn an_empty_data_directory_is_refused() {
        assert_eq!(
            from(&[("HYPERION_DATA_DIR", "")]).unwrap_err(),
            ParseConfigError::InvalidValue {
                variable: "HYPERION_DATA_DIR",
                value: String::new(),
                expected: "a directory path",
            }
        );
    }

    #[test]
    fn a_cache_size_that_is_not_a_number_or_too_large_is_refused() {
        for value in ["lots", "1.5", &usize::MAX.to_string()] {
            assert_eq!(
                from(&[("HYPERION_MAP_CACHE_MB", value)]).unwrap_err(),
                ParseConfigError::InvalidValue {
                    variable: "HYPERION_MAP_CACHE_MB",
                    value: value.to_owned(),
                    expected: "a whole number of MiB that fits in memory",
                }
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_value_that_is_not_unicode_is_refused() {
        use std::os::unix::ffi::OsStringExt;

        let vars =
            |name: &str| (name == ENV_WORKERS).then(|| OsString::from_vec(vec![0x66, 0x6f, 0x80]));
        assert_eq!(
            ServerConfig::from_vars(vars).unwrap_err(),
            ParseConfigError::NotUnicode {
                variable: "HYPERION_WORKERS"
            }
        );
    }

    #[test]
    fn the_builder_overrides_defaults() {
        let config = ServerConfig::builder()
            .addr("127.0.0.1:0")
            .data_dir("/tmp/x")
            .workers(NonZeroUsize::new(2).unwrap())
            .cell_cache_bytes(10)
            .map_cache_bytes(20)
            .build();
        assert_eq!(
            (
                config.addr(),
                config.data_dir(),
                config.workers().get(),
                config.cell_cache_bytes(),
                config.map_cache_bytes()
            ),
            ("127.0.0.1:0", Path::new("/tmp/x"), 2, 10, 20)
        );
    }
}
