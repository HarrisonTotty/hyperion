//! Server configuration: where to listen, where saves live, how many workers, how much cache.
//!
//! [`ServerArgs`] is the command line, which `main` turns into a [`ServerConfig`]. Every option
//! can instead be set by an environment variable; one given on the command line wins. Tests and
//! embedders build a [`ServerConfig`] with [`ServerConfig::builder`].
//!
//! | Option          | Variable                 | Default                                      |
//! | --------------- | ------------------------ | -------------------------------------------- |
//! | `--address`     | `HYPERION_ADDR`          | `127.0.0.1`                                  |
//! | `--port`        | `HYPERION_PORT`          | `7878`                                       |
//! | `--data-dir`    | `HYPERION_DATA_DIR`      | `./hyperion-data`                            |
//! | `--num-workers` | `HYPERION_WORKERS`       | available parallelism less one, at least one |
//! | `--cell-cache`  | `HYPERION_CELL_CACHE_MB` | 256 (MiB)                                    |
//! | `--map-cache`   | `HYPERION_MAP_CACHE_MB`  | 64 (MiB)                                     |

use std::error::Error;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;

use crate::DEFAULT_ADDR;
use crate::universe::{Entropy, OsEntropy};

/// The variable giving the IP address to listen on, for `--address`.
pub const ENV_ADDR: &str = "HYPERION_ADDR";
/// The variable giving the port to listen on, for `--port`.
pub const ENV_PORT: &str = "HYPERION_PORT";
/// The variable naming the data directory, which holds the saves, for `--data-dir`.
pub const ENV_DATA_DIR: &str = "HYPERION_DATA_DIR";
/// The variable giving the number of CPU pool workers, for `--num-workers`.
pub const ENV_WORKERS: &str = "HYPERION_WORKERS";
/// The variable giving the cell cache's budget in MiB, for `--cell-cache`.
pub const ENV_CELL_CACHE_MB: &str = "HYPERION_CELL_CACHE_MB";
/// The variable giving the density map cache's budget in MiB, for `--map-cache`.
pub const ENV_MAP_CACHE_MB: &str = "HYPERION_MAP_CACHE_MB";

/// The data directory when `--data-dir` is not given, relative to the working directory.
pub const DEFAULT_DATA_DIR: &str = "./hyperion-data";
/// The cell cache's budget when `--cell-cache` is not given, in MiB.
pub const DEFAULT_CELL_CACHE_MIB: usize = 256;
/// The density map cache's budget when `--map-cache` is not given, in MiB.
pub const DEFAULT_MAP_CACHE_MIB: usize = 64;

/// Bytes in a MiB, the unit of the cache options.
const BYTES_PER_MIB: usize = 1 << 20;

const DEFAULT_CELL_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_CELL_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_MAP_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_MAP_CACHE_MIB * BYTES_PER_MIB,
};

/// The server's command line.
///
/// Each option falls back to its environment variable (see the [module docs](self)) and then to
/// its default. The field docs are the `--help` text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Parser)]
#[command(version, about, long_about = None)]
pub struct ServerArgs {
    /// IP address to listen on
    #[arg(long, value_name = "IP", env = ENV_ADDR, default_value_t = DEFAULT_ADDR.ip())]
    address: IpAddr,

    /// Port to listen on; 0 lets the OS choose
    #[arg(long, env = ENV_PORT, default_value_t = DEFAULT_ADDR.port())]
    port: u16,

    /// Directory the universes are saved in, created with the first universe
    #[arg(long, value_name = "DIR", env = ENV_DATA_DIR, default_value = DEFAULT_DATA_DIR)]
    data_dir: PathBuf,

    /// Generation worker threads; defaults to the available parallelism less one, at least one
    #[arg(long, value_name = "COUNT", env = ENV_WORKERS, default_value_t = default_workers())]
    num_workers: NonZeroUsize,

    /// Budget of the cache of generated cells, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_CELL_CACHE_MB, default_value_t = DEFAULT_CELL_CACHE)]
    cell_cache: CacheBudget,

    /// Budget of the cache of galaxy density maps, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_MAP_CACHE_MB, default_value_t = DEFAULT_MAP_CACHE)]
    map_cache: CacheBudget,
}

impl From<ServerArgs> for ServerConfig {
    fn from(args: ServerArgs) -> Self {
        let ServerArgs {
            address,
            port,
            data_dir,
            num_workers,
            cell_cache,
            map_cache,
        } = args;
        Self::builder()
            .addr(SocketAddr::new(address, port))
            .data_dir(data_dir)
            .workers(num_workers)
            .cell_cache_bytes(cell_cache.bytes)
            .map_cache_bytes(map_cache.bytes)
            .build()
    }
}

/// A cache budget: written in whole MiB, held in bytes that fit in a `usize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CacheBudget {
    bytes: usize,
}

impl FromStr for CacheBudget {
    type Err = ParseCacheBudgetError;

    fn from_str(mib: &str) -> Result<Self, Self::Err> {
        let mib = mib
            .parse::<usize>()
            .map_err(|_| ParseCacheBudgetError::NotWholeMib)?;
        mib.checked_mul(BYTES_PER_MIB)
            .map(|bytes| Self { bytes })
            .ok_or(ParseCacheBudgetError::TooLarge)
    }
}

impl fmt::Display for CacheBudget {
    /// Writes the budget in MiB, the unit it is given in, so that it parses back.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.bytes / BYTES_PER_MIB)
    }
}

/// A cache budget on the command line cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ParseCacheBudgetError {
    /// It is not a whole number.
    NotWholeMib,
    /// It is more bytes than a `usize` holds.
    TooLarge,
}

impl fmt::Display for ParseCacheBudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotWholeMib => f.write_str("expected a whole number of MiB"),
            Self::TooLarge => f.write_str("more bytes than this machine can address"),
        }
    }
}

impl Error for ParseCacheBudgetError {}

/// Everything the server is started with.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    addr: SocketAddr,
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

    /// The address to listen on.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
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
                addr: DEFAULT_ADDR,
                data_dir: PathBuf::from(DEFAULT_DATA_DIR),
                workers: default_workers(),
                cell_cache_bytes: DEFAULT_CELL_CACHE.bytes,
                map_cache_bytes: DEFAULT_MAP_CACHE.bytes,
                entropy: Arc::new(OsEntropy),
            },
        }
    }
}

impl ServerConfigBuilder {
    /// The address to listen on.
    #[must_use]
    pub fn addr(mut self, addr: SocketAddr) -> Self {
        self.config.addr = addr;
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

/// Available parallelism less one, for the runtime and the sockets, and at least one.
fn default_workers() -> NonZeroUsize {
    std::thread::available_parallelism()
        .ok()
        .and_then(|cores| NonZeroUsize::new(cores.get() - 1))
        .unwrap_or(NonZeroUsize::MIN)
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};
    use std::net::{Ipv4Addr, Ipv6Addr};

    use clap::error::ErrorKind;
    use clap::{CommandFactory, FromArgMatches};

    use super::*;

    /// Parses `args` as the command line without the process environment, so that a variable set
    /// where the tests run cannot change their outcome.
    fn parse_os(args: impl IntoIterator<Item = OsString>) -> Result<ServerArgs, clap::Error> {
        let matches = ServerArgs::command()
            .mut_args(|arg| arg.env(None))
            .try_get_matches_from(std::iter::once(OsString::from("hyperion-server")).chain(args))?;
        ServerArgs::from_arg_matches(&matches)
    }

    fn parse(args: &[&str]) -> Result<ServerArgs, clap::Error> {
        parse_os(args.iter().map(OsString::from))
    }

    fn config(args: &[&str]) -> ServerConfig {
        ServerConfig::from(parse(args).unwrap())
    }

    fn refusal(args: &[&str]) -> ErrorKind {
        parse(args).unwrap_err().kind()
    }

    fn fields(config: &ServerConfig) -> (SocketAddr, &Path, usize, usize, usize) {
        (
            config.addr(),
            config.data_dir(),
            config.workers().get(),
            config.cell_cache_bytes(),
            config.map_cache_bytes(),
        )
    }

    #[test]
    fn the_command_line_is_well_formed() {
        ServerArgs::command().debug_assert();
    }

    #[test]
    fn defaults_match_the_builder() {
        let config = config(&[]);
        assert_eq!(fields(&config), fields(&ServerConfig::builder().build()));
        let cores = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::LOCALHOST, 7878)),
                Path::new("./hyperion-data"),
                cores.saturating_sub(1).max(1),
                256 * 1024 * 1024,
                64 * 1024 * 1024,
            )
        );
    }

    #[test]
    fn every_option_is_read() {
        let config = config(&[
            "--address",
            "0.0.0.0",
            "--port",
            "9000",
            "--data-dir",
            "/srv/hyperion",
            "--num-workers",
            "3",
            "--cell-cache",
            "1",
            "--map-cache",
            "0",
        ]);
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::UNSPECIFIED, 9000)),
                Path::new("/srv/hyperion"),
                3,
                1 << 20,
                0
            )
        );
    }

    #[test]
    fn an_ipv6_address_is_accepted() {
        assert_eq!(
            config(&["--address", "::1", "--port", "0"]).addr(),
            SocketAddr::from((Ipv6Addr::LOCALHOST, 0))
        );
    }

    #[test]
    fn every_option_falls_back_to_its_variable() {
        let command = ServerArgs::command();
        let variables: Vec<_> = command
            .get_arguments()
            .map(|arg| (arg.get_long(), arg.get_env().and_then(OsStr::to_str)))
            .collect();
        assert_eq!(
            variables,
            [
                (Some("address"), Some("HYPERION_ADDR")),
                (Some("port"), Some("HYPERION_PORT")),
                (Some("data-dir"), Some("HYPERION_DATA_DIR")),
                (Some("num-workers"), Some("HYPERION_WORKERS")),
                (Some("cell-cache"), Some("HYPERION_CELL_CACHE_MB")),
                (Some("map-cache"), Some("HYPERION_MAP_CACHE_MB")),
            ]
        );
    }

    #[test]
    fn help_and_version_are_offered() {
        for flag in ["-h", "--help"] {
            assert_eq!(refusal(&[flag]), ErrorKind::DisplayHelp, "{flag}");
        }
        let version = parse(&["--version"]).unwrap_err();
        assert_eq!(version.kind(), ErrorKind::DisplayVersion);
        assert_eq!(
            version.to_string(),
            concat!("hyperion-server ", env!("CARGO_PKG_VERSION"), "\n")
        );
    }

    #[test]
    fn an_address_must_be_an_ip_without_a_port() {
        for address in ["localhost", "127.0.0.1:7878", "256.0.0.1", ""] {
            assert_eq!(
                refusal(&[&format!("--address={address}")]),
                ErrorKind::ValueValidation,
                "{address:?}"
            );
        }
    }

    #[test]
    fn a_port_out_of_range_is_refused() {
        for port in ["65536", "-1", "http"] {
            assert_eq!(
                refusal(&[&format!("--port={port}")]),
                ErrorKind::ValueValidation,
                "{port:?}"
            );
        }
    }

    #[test]
    fn a_bad_worker_count_is_refused() {
        for count in ["0", "four", "-2", "1.5"] {
            assert_eq!(
                refusal(&[&format!("--num-workers={count}")]),
                ErrorKind::ValueValidation,
                "{count:?}"
            );
        }
    }

    #[test]
    fn an_empty_data_directory_is_refused() {
        assert_eq!(refusal(&["--data-dir="]), ErrorKind::InvalidValue);
    }

    #[test]
    fn a_cache_budget_is_whole_mib_that_fit_in_a_usize() {
        let largest = usize::MAX / BYTES_PER_MIB;
        assert_eq!("0".parse(), Ok(CacheBudget { bytes: 0 }));
        assert_eq!(
            largest.to_string().parse(),
            Ok(CacheBudget {
                bytes: largest * BYTES_PER_MIB
            })
        );
        assert_eq!(
            (largest + 1).to_string().parse::<CacheBudget>(),
            Err(ParseCacheBudgetError::TooLarge)
        );
        for text in ["lots", "1.5", "-1", ""] {
            assert_eq!(
                text.parse::<CacheBudget>(),
                Err(ParseCacheBudgetError::NotWholeMib),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_cache_budget_is_written_in_mib() {
        assert_eq!(DEFAULT_CELL_CACHE.to_string(), "256");
        assert_eq!(DEFAULT_MAP_CACHE.to_string(), "64");
    }

    #[test]
    fn a_bad_cache_budget_is_refused_with_the_reason() {
        let error = parse(&["--cell-cache", &usize::MAX.to_string()]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
        assert!(
            error
                .to_string()
                .contains("more bytes than this machine can address"),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_number_that_is_not_unicode_is_refused() {
        use std::os::unix::ffi::OsStringExt;

        let error = parse_os([
            OsString::from("--num-workers"),
            OsString::from_vec(vec![0x66, 0x6f, 0x80]),
        ])
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidUtf8);
    }

    #[cfg(unix)]
    #[test]
    fn a_data_directory_need_not_be_unicode() {
        use std::os::unix::ffi::OsStringExt;

        let dir = OsString::from_vec(vec![0x66, 0x6f, 0x80]);
        let args = parse_os([OsString::from("--data-dir"), dir.clone()]).unwrap();
        assert_eq!(ServerConfig::from(args).data_dir(), Path::new(&dir));
    }

    #[test]
    fn the_builder_overrides_defaults() {
        let config = ServerConfig::builder()
            .addr(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .data_dir("/tmp/x")
            .workers(NonZeroUsize::new(2).unwrap())
            .cell_cache_bytes(10)
            .map_cache_bytes(20)
            .build();
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                Path::new("/tmp/x"),
                2,
                10,
                20
            )
        );
    }
}
