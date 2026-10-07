//! Server configuration: where to listen, where saves live, how many workers, how much cache.
//!
//! [`ServerArgs`] is the command line, which `main` turns into a [`ServerConfig`]. Every option
//! can instead be set by an environment variable; one given on the command line wins. Tests and
//! embedders build a [`ServerConfig`] with [`ServerConfig::builder`].
//!
//! | Option                  | Variable                       | Default                                      |
//! | ----------------------- | ------------------------------ | -------------------------------------------- |
//! | `--address`             | `HYPERION_ADDR`                | `127.0.0.1`                                  |
//! | `--port`                | `HYPERION_PORT`                | `7878`                                       |
//! | `--data-dir`            | `HYPERION_DATA_DIR`            | `./hyperion-data`                            |
//! | `--num-workers`         | `HYPERION_WORKERS`             | available parallelism less one, at least one |
//! | `--cell-cache`          | `HYPERION_CELL_CACHE_MB`       | 256 (MiB)                                    |
//! | `--map-cache`           | `HYPERION_MAP_CACHE_MB`        | 64 (MiB)                                     |
//! | `--system-cache`        | `HYPERION_SYSTEM_CACHE_MB`     | 128 (MiB)                                    |
//! | `--body-cache`          | `HYPERION_BODY_CACHE_MB`       | 128 (MiB)                                    |
//! | `--brief-cache`         | `HYPERION_BRIEF_CACHE_MB`      | 64 (MiB)                                     |
//! | `--sky-cache`           | `HYPERION_SKY_CACHE_MB`        | 64 (MiB)                                     |
//! | `--sky-tables`          | `HYPERION_SKY_TABLES_MB`       | 160 (MiB)                                    |
//! | `--serve-sky`           | `HYPERION_SERVE_SKY`           | off                                          |
//! | `--stop-on-stdin-close` | `HYPERION_STOP_ON_STDIN_CLOSE` | off                                          |
//!
//! `--serve-sky` and `--stop-on-stdin-close` are switches. Their variables take clap's boolish
//! values, in any case: `y`, `yes`, `t`, `true`, `on` or `1` for on, and `n`, `no`, `f`, `false`,
//! `off` or `0` for off. They refuse anything else, the empty string included.
//!
//! `--serve-sky` is the sky's landing switch (rendering plan R06, R06.T11.c; decided 2026-10-07 by
//! the orchestrator). Off, the server answers `sky` as it did before R06.T11.a, `unsupported`, and
//! no job of it reaches the pool, so a client that asks for its sky is untouched until R06.T8.g,
//! whose landing turns it on by default.

use crate::scene::{CraftSource, GrantAsked, NoCraft, SceneKnowledge};
use std::error::Error;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use clap::builder::{BoolishValueParser, TypedValueParser};
use clap::{ArgAction, Parser};

use crate::DEFAULT_ADDR;
use crate::compute::SkyCaps;
use crate::stop::StdinStop;
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
/// The variable giving the system cache's budget in MiB, for `--system-cache`.
pub const ENV_SYSTEM_CACHE_MB: &str = "HYPERION_SYSTEM_CACHE_MB";
/// The variable giving the body cache's budget in MiB, for `--body-cache`.
pub const ENV_BODY_CACHE_MB: &str = "HYPERION_BODY_CACHE_MB";
/// The variable giving the brief cache's budget in MiB, for `--brief-cache`.
pub const ENV_BRIEF_CACHE_MB: &str = "HYPERION_BRIEF_CACHE_MB";
/// The variable giving the sky's cell cache's budget in MiB, for `--sky-cache`.
pub const ENV_SKY_CACHE_MB: &str = "HYPERION_SKY_CACHE_MB";
/// The variable giving the sky tables' cache's budget in MiB, for `--sky-tables`.
pub const ENV_SKY_TABLES_MB: &str = "HYPERION_SKY_TABLES_MB";
/// The variable that turns the sky on, for `--serve-sky`.
pub const ENV_SERVE_SKY: &str = "HYPERION_SERVE_SKY";
/// The variable that makes the end of standard input stop the server, for `--stop-on-stdin-close`.
pub const ENV_STOP_ON_STDIN_CLOSE: &str = "HYPERION_STOP_ON_STDIN_CLOSE";

/// The data directory when `--data-dir` is not given, relative to the working directory.
pub const DEFAULT_DATA_DIR: &str = "./hyperion-data";
/// The cell cache's budget when `--cell-cache` is not given, in MiB.
pub const DEFAULT_CELL_CACHE_MIB: usize = 256;
/// The density map cache's budget when `--map-cache` is not given, in MiB.
pub const DEFAULT_MAP_CACHE_MIB: usize = 64;
/// The system cache's budget when `--system-cache` is not given, in MiB (plan 06, P06.T34).
pub const DEFAULT_SYSTEM_CACHE_MIB: usize = 128;
/// The body cache's budget when `--body-cache` is not given, in MiB (plan 14, P14.T36.a).
pub const DEFAULT_BODY_CACHE_MIB: usize = 128;
/// The brief cache's budget when `--brief-cache` is not given, in MiB (plan 06, P06.T34): some
/// 25,000 main-sequence rows at about 2.5 kB each, or 2,000–2,500 dead ones, whose model holds a
/// full track, at 25–35 kB (P06.T38.e's measurement).
pub const DEFAULT_BRIEF_CACHE_MIB: usize = 64;
/// The sky's cell cache's budget when `--sky-cache` is not given, in MiB (rendering plan R06,
/// Design note 12).
///
/// Provisional: R06.T8.h sets it from one near-Sun sky's entry bytes. A sky forced to 200 ly near
/// the Sun at V 11 kept 25.5 MB of entries (R06's Risks, "Deviations in T11.b, as built").
pub const DEFAULT_SKY_CACHE_MIB: usize = 64;
/// The sky tables' cache's budget when `--sky-tables` is not given, in MiB: two galaxies' luminosity
/// tables, about 55 MiB each with their snapshots (rendering plan R06, R06.T11.c; decided
/// 2026-10-03, `decision-r06-tables.md`, item B.2).
pub const DEFAULT_SKY_TABLES_MIB: usize = 160;

/// Bytes in a MiB, the unit of the cache options.
const BYTES_PER_MIB: usize = 1 << 20;

const DEFAULT_CELL_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_CELL_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_MAP_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_MAP_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_SYSTEM_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_SYSTEM_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_BODY_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_BODY_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_BRIEF_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_BRIEF_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_SKY_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_SKY_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_SKY_TABLES: CacheBudget = CacheBudget {
    bytes: DEFAULT_SKY_TABLES_MIB * BYTES_PER_MIB,
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

    /// Budget of the cache of generated systems' stars, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_SYSTEM_CACHE_MB, default_value_t = DEFAULT_SYSTEM_CACHE)]
    system_cache: CacheBudget,

    /// Budget of the cache of generated planetary systems, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_BODY_CACHE_MB, default_value_t = DEFAULT_BODY_CACHE)]
    body_cache: CacheBudget,

    /// Budget of the cache of range briefs' star models, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_BRIEF_CACHE_MB, default_value_t = DEFAULT_BRIEF_CACHE)]
    brief_cache: CacheBudget,

    /// Budget of the cache of the sky census's cells, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_SKY_CACHE_MB, default_value_t = DEFAULT_SKY_CACHE)]
    sky_cache: CacheBudget,

    /// Budget of the cache of each galaxy's sky tables, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_SKY_TABLES_MB, default_value_t = DEFAULT_SKY_TABLES)]
    sky_tables: CacheBudget,

    /// Serve `sky` requests, which are answered `unsupported` otherwise (until rendering plan R06's
    /// census is fast enough to serve)
    #[arg(
        long,
        env = ENV_SERVE_SKY,
        action = ArgAction::SetTrue,
        value_parser = BoolishValueParser::new().map(sky_service),
    )]
    serve_sky: SkyService,

    /// Stop gracefully when standard input closes, for a server run as another program's child
    #[arg(
        long,
        env = ENV_STOP_ON_STDIN_CLOSE,
        action = ArgAction::SetTrue,
        value_parser = BoolishValueParser::new().map(stdin_stop),
    )]
    stop_on_stdin_close: StdinStop,
}

/// Whether the server answers `sky` (rendering plan R06): its landing switch, `--serve-sky`
/// (R06.T11.c; decided 2026-10-07 by the orchestrator).
///
/// R06.T11.a–c land behind it, off, so that the live client, which asks for its sky whenever a view
/// opens, is untouched until R06.T8.g makes a sky near the Sun cheap enough to serve; T8.g's landing
/// turns it on by default. Tests turn it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum SkyService {
    /// `sky` is answered `unsupported`, as before R06.T11.a, and none of its work reaches the pool:
    /// the default.
    #[default]
    Unsupported,
    /// `sky` is served: its census, band, limit map and host discs (R06.T11.a–c).
    Served,
}

/// The sky's service that a switch asks for.
#[must_use]
fn sky_service(serve: bool) -> SkyService {
    if serve {
        SkyService::Served
    } else {
        SkyService::Unsupported
    }
}

/// The stop on standard input that a switch asks for.
#[must_use]
fn stdin_stop(watch: bool) -> StdinStop {
    if watch {
        StdinStop::Watch
    } else {
        StdinStop::Ignore
    }
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
            system_cache,
            body_cache,
            brief_cache,
            sky_cache,
            sky_tables,
            serve_sky,
            stop_on_stdin_close,
        } = args;
        Self::builder()
            .addr(SocketAddr::new(address, port))
            .data_dir(data_dir)
            .workers(num_workers)
            .cell_cache_bytes(cell_cache.bytes)
            .map_cache_bytes(map_cache.bytes)
            .system_cache_bytes(system_cache.bytes)
            .body_cache_bytes(body_cache.bytes)
            .brief_cache_bytes(brief_cache.bytes)
            .sky_cache_bytes(sky_cache.bytes)
            .sky_tables_bytes(sky_tables.bytes)
            .sky_service(serve_sky)
            .stdin_stop(stop_on_stdin_close)
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
    system_cache_bytes: usize,
    body_cache_bytes: usize,
    brief_cache_bytes: usize,
    sky_cache_bytes: usize,
    sky_tables_bytes: usize,
    sky_service: SkyService,
    stdin_stop: StdinStop,
    entropy: Arc<dyn Entropy>,
    scene_knowledge: Arc<dyn SceneKnowledge>,
    craft_source: Arc<dyn CraftSource>,
    sky_caps: SkyCaps,
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

    /// The system cache's budget, in bytes.
    #[must_use]
    pub fn system_cache_bytes(&self) -> usize {
        self.system_cache_bytes
    }

    /// The body cache's budget, in bytes.
    #[must_use]
    pub fn body_cache_bytes(&self) -> usize {
        self.body_cache_bytes
    }

    /// The brief cache's budget, in bytes.
    #[must_use]
    pub fn brief_cache_bytes(&self) -> usize {
        self.brief_cache_bytes
    }

    /// The sky's cell cache's budget, in bytes (rendering plan R06, Design note 12).
    #[must_use]
    pub fn sky_cache_bytes(&self) -> usize {
        self.sky_cache_bytes
    }

    /// The sky tables' cache's budget, in bytes (rendering plan R06, R06.T11.c).
    #[must_use]
    pub fn sky_tables_bytes(&self) -> usize {
        self.sky_tables_bytes
    }

    /// Whether the server answers `sky` (rendering plan R06, R06.T11.c's landing switch).
    #[must_use]
    pub fn sky_service(&self) -> SkyService {
        self.sky_service
    }

    /// Whether the end of standard input stops the server.
    ///
    /// The binary's `main` reads this, for [`StopRequests::listen`](crate::stop::StopRequests::listen);
    /// [`Server`](crate::Server) does not, since a program that embeds one stops it itself.
    #[must_use]
    pub fn stdin_stop(&self) -> StdinStop {
        self.stdin_stop
    }

    /// Where seeds and universe IDs are drawn from.
    #[must_use]
    pub fn entropy(&self) -> &Arc<dyn Entropy> {
        &self.entropy
    }

    /// What the ship knows, as the scene asks it (rendering plan R03, Design note 13).
    #[must_use]
    pub fn scene_knowledge(&self) -> &Arc<dyn SceneKnowledge> {
        &self.scene_knowledge
    }

    /// The craft each universe's scene is told of (rendering plan R03, Design note 4).
    #[must_use]
    pub fn craft_source(&self) -> &Arc<dyn CraftSource> {
        &self.craft_source
    }

    /// How far a sky's census looks (rendering plan R06, Design note 9).
    #[must_use]
    pub fn sky_caps(&self) -> SkyCaps {
        self.sky_caps
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
                system_cache_bytes: DEFAULT_SYSTEM_CACHE.bytes,
                body_cache_bytes: DEFAULT_BODY_CACHE.bytes,
                brief_cache_bytes: DEFAULT_BRIEF_CACHE.bytes,
                sky_cache_bytes: DEFAULT_SKY_CACHE.bytes,
                sky_tables_bytes: DEFAULT_SKY_TABLES.bytes,
                sky_service: SkyService::default(),
                stdin_stop: StdinStop::default(),
                entropy: Arc::new(OsEntropy),
                scene_knowledge: Arc::new(GrantAsked),
                craft_source: Arc::new(NoCraft),
                sky_caps: SkyCaps::DERIVED,
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

    /// The system cache's budget, in bytes. Zero caches nothing.
    #[must_use]
    pub fn system_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.system_cache_bytes = bytes;
        self
    }

    /// The body cache's budget, in bytes. Zero caches nothing, though concurrent requests for one
    /// system still share its generation.
    #[must_use]
    pub fn body_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.body_cache_bytes = bytes;
        self
    }

    /// The brief cache's budget, in bytes. Zero caches nothing: every brief is built, answered
    /// from and dropped.
    #[must_use]
    pub fn brief_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.brief_cache_bytes = bytes;
        self
    }

    /// The sky's cell cache's budget, in bytes.
    ///
    /// Zero caches nothing: every census cell is generated, served and dropped.
    #[must_use]
    pub fn sky_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.sky_cache_bytes = bytes;
        self
    }

    /// The sky tables' cache's budget, in bytes.
    ///
    /// Zero caches nothing: every sky builds its galaxy's tables, though skies asked at once still
    /// share one build.
    #[must_use]
    pub fn sky_tables_bytes(mut self, bytes: usize) -> Self {
        self.config.sky_tables_bytes = bytes;
        self
    }

    /// Whether the server answers `sky`; [`SkyService::Unsupported`] by default, until R06.T8.g.
    /// Tests serve it.
    #[must_use]
    pub fn sky_service(mut self, service: SkyService) -> Self {
        self.config.sky_service = service;
        self
    }

    /// Whether the end of standard input stops the server; [`StdinStop::Ignore`] by default.
    #[must_use]
    pub fn stdin_stop(mut self, stdin: StdinStop) -> Self {
        self.config.stdin_stop = stdin;
        self
    }

    /// Where seeds and universe IDs are drawn from; [`OsEntropy`] by default. Tests inject a
    /// [`SequenceEntropy`](crate::universe::SequenceEntropy).
    #[must_use]
    pub fn entropy(mut self, entropy: impl Entropy + 'static) -> Self {
        self.config.entropy = Arc::new(entropy);
        self
    }

    /// What the ship knows, as the scene asks it; [`GrantAsked`] by default, until the sensors plan.
    /// Tests restrict the scene with their own.
    #[must_use]
    pub fn scene_knowledge(mut self, knowledge: impl SceneKnowledge + 'static) -> Self {
        self.config.scene_knowledge = Arc::new(knowledge);
        self
    }

    /// The craft each universe's scene is told of; [`NoCraft`] by default, until sessions. Tests
    /// supply their own.
    #[must_use]
    pub fn craft_source(mut self, craft: impl CraftSource + 'static) -> Self {
        self.config.craft_source = Arc::new(craft);
        self
    }

    /// How far a sky's census looks; [`SkyCaps::DERIVED`] by default, each layer's own cap. Tests
    /// force one small radius on every layer, so that a census takes seconds; no option or
    /// variable sets it.
    #[must_use]
    pub fn sky_caps(mut self, caps: SkyCaps) -> Self {
        self.config.sky_caps = caps;
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

    type Fields<'a> = (
        SocketAddr,
        &'a Path,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
        SkyService,
        StdinStop,
    );

    fn fields(config: &ServerConfig) -> Fields<'_> {
        (
            config.addr(),
            config.data_dir(),
            config.workers().get(),
            config.cell_cache_bytes(),
            config.map_cache_bytes(),
            config.system_cache_bytes(),
            config.body_cache_bytes(),
            config.brief_cache_bytes(),
            config.sky_cache_bytes(),
            config.sky_tables_bytes(),
            config.sky_service(),
            config.stdin_stop(),
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
                128 * 1024 * 1024,
                128 * 1024 * 1024,
                64 * 1024 * 1024,
                64 * 1024 * 1024,
                160 * 1024 * 1024,
                SkyService::Unsupported,
                StdinStop::Ignore,
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
            "--system-cache",
            "2",
            "--body-cache",
            "5",
            "--brief-cache",
            "7",
            "--sky-cache",
            "11",
            "--sky-tables",
            "13",
            "--serve-sky",
            "--stop-on-stdin-close",
        ]);
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::UNSPECIFIED, 9000)),
                Path::new("/srv/hyperion"),
                3,
                1 << 20,
                0,
                2 << 20,
                5 << 20,
                7 << 20,
                11 << 20,
                13 << 20,
                SkyService::Served,
                StdinStop::Watch,
            )
        );
    }

    #[test]
    fn the_stop_on_stdin_close_is_a_switch() {
        assert_eq!(
            refusal(&["--stop-on-stdin-close=yes"]),
            ErrorKind::TooManyValues
        );
    }

    /// The sky's landing switch is off unless it is given (R06.T11.c).
    #[test]
    fn the_sky_is_served_only_when_its_switch_is_on() {
        assert_eq!(config(&[]).sky_service(), SkyService::Unsupported);
        assert_eq!(config(&["--serve-sky"]).sky_service(), SkyService::Served);
        assert_eq!(refusal(&["--serve-sky=yes"]), ErrorKind::TooManyValues);
        assert_eq!(
            ServerConfig::builder().build().sky_service(),
            SkyService::Unsupported
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
                (Some("system-cache"), Some("HYPERION_SYSTEM_CACHE_MB")),
                (Some("body-cache"), Some("HYPERION_BODY_CACHE_MB")),
                (Some("brief-cache"), Some("HYPERION_BRIEF_CACHE_MB")),
                (Some("sky-cache"), Some("HYPERION_SKY_CACHE_MB")),
                (Some("sky-tables"), Some("HYPERION_SKY_TABLES_MB")),
                (Some("serve-sky"), Some("HYPERION_SERVE_SKY")),
                (
                    Some("stop-on-stdin-close"),
                    Some("HYPERION_STOP_ON_STDIN_CLOSE")
                ),
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
        assert_eq!(DEFAULT_SYSTEM_CACHE.to_string(), "128");
        assert_eq!(DEFAULT_BODY_CACHE.to_string(), "128");
        assert_eq!(DEFAULT_BRIEF_CACHE.to_string(), "64");
        assert_eq!(DEFAULT_SKY_CACHE.to_string(), "64");
        assert_eq!(DEFAULT_SKY_TABLES.to_string(), "160");
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
            .system_cache_bytes(30)
            .body_cache_bytes(40)
            .brief_cache_bytes(50)
            .sky_cache_bytes(60)
            .sky_tables_bytes(70)
            .sky_service(SkyService::Served)
            .stdin_stop(StdinStop::Watch)
            .build();
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                Path::new("/tmp/x"),
                2,
                10,
                20,
                30,
                40,
                50,
                60,
                70,
                SkyService::Served,
                StdinStop::Watch,
            )
        );
    }
}
