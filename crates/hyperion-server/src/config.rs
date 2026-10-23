//! Server configuration: where to listen, where saves live, how many workers, how much cache.
//!
//! [`ServerArgs`] is the command line, which `main` turns into a [`ServerConfig`]. Every option
//! can instead be set by an environment variable; one given on the command line wins. Tests and
//! embedders build a [`ServerConfig`] with [`ServerConfig::builder`].
//!
//! | Option                   | Variable                           | Default                                                |
//! | ------------------------ | ---------------------------------- | ------------------------------------------------------ |
//! | `--address`              | `HYPERION_ADDR`                    | `127.0.0.1`                                            |
//! | `--port`                 | `HYPERION_PORT`                    | `7878`                                                 |
//! | `--data-dir`             | `HYPERION_DATA_DIR`                | `./hyperion-data`                                      |
//! | `--num-workers`          | `HYPERION_WORKERS`                 | available parallelism less one, at least one           |
//! | `--cell-cache`           | `HYPERION_CELL_CACHE_MB`           | 256 (MiB)                                              |
//! | `--map-cache`            | `HYPERION_MAP_CACHE_MB`            | 64 (MiB)                                               |
//! | `--system-cache`         | `HYPERION_SYSTEM_CACHE_MB`         | 128 (MiB)                                              |
//! | `--body-cache`           | `HYPERION_BODY_CACHE_MB`           | 128 (MiB)                                              |
//! | `--brief-cache`          | `HYPERION_BRIEF_CACHE_MB`          | 64 (MiB)                                               |
//! | `--extinction-map-cache` | `HYPERION_EXTINCTION_MAP_CACHE_MB` | 16 (MiB)                                               |
//! | `--sightline-cache`      | `HYPERION_SIGHTLINE_CACHE_MB`      | 4 (MiB)                                                |
//! | `--sky-cache`            | `HYPERION_SKY_CACHE_MB`            | 0 (MiB), off                                           |
//! | `--sky-tables`           | `HYPERION_SKY_TABLES_MB`           | 160 (MiB)                                              |
//! | `--surface-cache`        | `HYPERION_SURFACE_CACHE_MB`        | 128 (MiB)                                              |
//! | `--surface-passes`       | `HYPERION_SURFACE_PASSES`          | the lesser of 4 and the workers less one, at least one |
//! | `--serve-sky`            | `HYPERION_SERVE_SKY`               | on                                                     |
//! | `--stop-on-stdin-close`  | `HYPERION_STOP_ON_STDIN_CLOSE`     | off                                                    |
//!
//! `--serve-sky` and `--stop-on-stdin-close` are switches. Their variables take clap's boolish
//! values, in any case: `y`, `yes`, `t`, `true`, `on` or `1` for on, and `n`, `no`, `f`, `false`,
//! `off` or `0` for off. They refuse anything else, the empty string included.
//! `--stop-on-stdin-close`, off by default, takes no value: given, it is on. `--serve-sky`, on by
//! default, also takes one of those values after an equals sign, so that the command line can turn
//! it off: `--serve-sky=false`. Given alone, it is on.
//!
//! `--serve-sky` is the sky's switch (rendering plan R06, R06.T11.c), added off by the
//! orchestrator's ruling of 2026-10-07. It is on by default since RM3's interim sky, rendering plan
//! R13's R13.T2, landed beside R06.T11.d and T11.g, as the owner decided on 2026-10-08 (R06.T11.d,
//! "The default switch and the interim"). Off, the server answers `sky` as it did before
//! R06.T11.a, `unsupported`, no job of it reaches the pool, and an open starts no sky tables.
//!
//! `--sky-cache` is 0 by default, which turns the sky's cell cache off: each census reads no cache
//! and keeps no cell, R06.T8.g's path (rendering plan R06, R06.T8.o; decided 2026-10-10,
//! `decision-r06-t8n.md`). At version 21 the cache saves nothing a player feels, since a warm
//! census still generates every system a cold one does, and it would hold up to 1,280 MiB to do so.
//! 1,280 MiB is R06.T8.n's sized budget, the value to set for a warm sky: one near-Sun sky and a
//! 1,000 ly jump's new cells. The default returns to it once a repeated sky costs at most half of a
//! cold one.

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
use crate::surface::{InputsSource, RecordInputs};
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
/// The variable giving the extinction map cache's budget in MiB, for `--extinction-map-cache`.
pub const ENV_EXTINCTION_MAP_CACHE_MB: &str = "HYPERION_EXTINCTION_MAP_CACHE_MB";
/// The variable giving the sightline cache's budget in MiB, for `--sightline-cache`.
pub const ENV_SIGHTLINE_CACHE_MB: &str = "HYPERION_SIGHTLINE_CACHE_MB";
/// The variable giving the sky's cell cache's budget in MiB, for `--sky-cache`.
pub const ENV_SKY_CACHE_MB: &str = "HYPERION_SKY_CACHE_MB";
/// The variable giving the sky tables' cache's budget in MiB, for `--sky-tables`.
pub const ENV_SKY_TABLES_MB: &str = "HYPERION_SKY_TABLES_MB";
/// The variable giving the surface service's cache budget in MiB, for `--surface-cache`.
pub const ENV_SURFACE_CACHE_MB: &str = "HYPERION_SURFACE_CACHE_MB";
/// The variable giving the number of coarse surface passes run at once, for `--surface-passes`.
pub const ENV_SURFACE_PASSES: &str = "HYPERION_SURFACE_PASSES";
/// The variable that turns the sky off, or on, for `--serve-sky`.
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
/// full track, at 25–35 kB (P06.T38.e's measurement), or some 700 paired ones, whose model holds
/// its system's stars and timelines, at 87–110 kB on average (P11.T11.a's measurement; 5% of the
/// rows near the Sun, 18–63% in the bulge).
pub const DEFAULT_BRIEF_CACHE_MIB: usize = 64;
/// The extinction map cache's budget when `--extinction-map-cache` is not given, in MiB (plan 07,
/// P07.T10.a): every view at every resolution of one galaxy, 128 to 1,024 pixels across and the
/// edge-on views half as tall, is 2,088,960 `f32` pixels, 8.0 MiB, so this holds two galaxies'
/// worth, or one with room to spare.
pub const DEFAULT_EXTINCTION_MAP_CACHE_MIB: usize = 16;
/// The sightline cache's budget when `--sightline-cache` is not given, in MiB (plan 07,
/// P07.T10.c): a line is charged its 32 bytes and the cache's 96 an entry, 128 bytes, so this holds
/// 32,768, a few hundred charts' selections.
pub const DEFAULT_SIGHTLINE_CACHE_MIB: usize = 4;
/// The sky's cell cache's budget when `--sky-cache` is not given, in MiB: 0, which turns the cache
/// off (rendering plan R06, R06.T8.o and Design note 12; decided 2026-10-10,
/// `decision-r06-t8n.md`).
///
/// At 0 each census job reads no cache, R06.T8.g's path bit for bit: no entry is built and no
/// verdict is taken twice, and the reply is the cached census's. At version 21 a warm census still
/// generates every system its cold census generates, which keeps its reply the cold one, so it
/// costs 96–108% of cold (R06.T8.n's benches, 2026-10-10, provisional): less saved than the runs'
/// spread, for up to 1,280 MiB held on a laptop whose 16 GB the CPU and the GPU share.
///
/// R06.T8.n's sizing stands, as the value to set for a warm sky and the budget at which R06.T17's
/// warm gate is judged: 1,280 MiB, by the rule of `decision-r06-t8h-warm.md` §2.2, the larger of 64
/// and 1.25 times the held bytes of one near-Sun sky's entries and a 1,000 ly jump's, rounded up to
/// a multiple of 256 MiB, at most 2,048. The sky is the one served since RM3's interim (rendering
/// plan R13, R13.T2.b): the eye at its own cut, C, D and E within their real boundary at V 5.0 and
/// 2,000 ly. Its entries, sampled 1 block in 20, are 705 MB after the Sun's sky on the server's
/// galaxy and 949 MB after a jump toward the galactic centre (379 MB and 525 MB on the sim's
/// fixture), so 1.25 × 949 MB is 1,131 MiB, rounded up to 1,280 MiB.
///
/// The default returns to the sized budget, measured again, once the repeat's census
/// (`census_near_sun_served/served_warm_5.0`) costs at most 50% of its cold by the threads' CPU
/// time (R06's Risks, "The sky cache at version 21").
pub const DEFAULT_SKY_CACHE_MIB: usize = 0;
/// The sky tables' cache's budget when `--sky-tables` is not given, in MiB: two galaxies' luminosity
/// tables, about 55 MiB each with their snapshots (rendering plan R06, R06.T11.c; decided
/// 2026-10-03, `decision-r06-tables.md`, item B.2).
pub const DEFAULT_SKY_TABLES_MIB: usize = 160;
/// The surface service's cache budget when `--surface-cache` is not given, in MiB: the bodies'
/// coarse fields (rendering plan R09: its Provides, "Server", gives 128 MiB, and Design note 19,
/// for context, the 32 GB of RAM of the development machine that hosts the server in testing).
///
/// A field is charged its heap bytes by its vectors' capacities. On the reference worlds at
/// version 21, before the coarse craters (R09.T12.d) add theirs, that is 24.4 MB at level 8 (an
/// Earth's), 6.1 MB at level 7 (a Mars's), 1.5 MB at level 6 (the Moon's) and 0.38 MB at level 5
/// (Ceres's), measured 2026-10-13 (R09.T17): the default keeps five Earths, or twenty bodies the
/// size of Mars. An Earth's whole payload on the wire is 14.7 MB.
pub const DEFAULT_SURFACE_CACHE_MIB: usize = 128;

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
const DEFAULT_EXTINCTION_MAP_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_EXTINCTION_MAP_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_SIGHTLINE_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_SIGHTLINE_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_SKY_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_SKY_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_SKY_TABLES: CacheBudget = CacheBudget {
    bytes: DEFAULT_SKY_TABLES_MIB * BYTES_PER_MIB,
};
const DEFAULT_SURFACE_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_SURFACE_CACHE_MIB * BYTES_PER_MIB,
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

    /// Budget of the cache of galaxy extinction maps, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_EXTINCTION_MAP_CACHE_MB, default_value_t = DEFAULT_EXTINCTION_MAP_CACHE)]
    extinction_map_cache: CacheBudget,

    /// Budget of the cache of lines of sight through the gas, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_SIGHTLINE_CACHE_MB, default_value_t = DEFAULT_SIGHTLINE_CACHE)]
    sightline_cache: CacheBudget,

    /// Budget of the cache of the sky census's cells, in MiB; 0, the default, keeps no cells, and
    /// 1280 holds a warm sky
    #[arg(long, value_name = "MIB", env = ENV_SKY_CACHE_MB, default_value_t = DEFAULT_SKY_CACHE)]
    sky_cache: CacheBudget,

    /// Budget of the cache of each galaxy's sky tables, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_SKY_TABLES_MB, default_value_t = DEFAULT_SKY_TABLES)]
    sky_tables: CacheBudget,

    /// Budget of the cache of bodies' coarse surface fields, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_SURFACE_CACHE_MB, default_value_t = DEFAULT_SURFACE_CACHE)]
    surface_cache: CacheBudget,

    /// Coarse surface passes run at once; defaults to the lesser of 4 and the workers less one, at
    /// least one
    #[arg(long, value_name = "COUNT", env = ENV_SURFACE_PASSES)]
    surface_passes: Option<NonZeroUsize>,

    /// Serve `sky` requests; `--serve-sky=false` answers them `unsupported`
    #[arg(
        long,
        value_name = "BOOL",
        env = ENV_SERVE_SKY,
        num_args = 0..=1,
        require_equals = true,
        default_value = "true",
        default_missing_value = "true",
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

/// Whether the server answers `sky` (rendering plan R06): its switch, `--serve-sky` (R06.T11.c).
///
/// R06.T11.a–c landed behind it, off (decided 2026-10-07 by the orchestrator), so that the live
/// client, which asks for its sky whenever a view opens, was untouched until a sky near the Sun
/// could be served. It is on by default since RM3's interim sky, rendering plan R13's R13.T2,
/// landed beside R06.T11.d and T11.g (decided by the owner 2026-10-08; R06.T11.d, "The default
/// switch and the interim").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum SkyService {
    /// `sky` is answered `unsupported`, as before R06.T11.a, none of its work reaches the pool, and
    /// an open starts no sky tables.
    Unsupported,
    /// `sky` is served, the default: its census, band, limit map and host discs (R06.T11.a–d), and
    /// an open starts its galaxy's sky tables (R06.T11.g).
    #[default]
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
            extinction_map_cache,
            sightline_cache,
            sky_cache,
            sky_tables,
            surface_cache,
            surface_passes,
            serve_sky,
            stop_on_stdin_close,
        } = args;
        let builder = Self::builder()
            .addr(SocketAddr::new(address, port))
            .data_dir(data_dir)
            .workers(num_workers)
            .cell_cache_bytes(cell_cache.bytes)
            .map_cache_bytes(map_cache.bytes)
            .system_cache_bytes(system_cache.bytes)
            .body_cache_bytes(body_cache.bytes)
            .brief_cache_bytes(brief_cache.bytes)
            .extinction_map_cache_bytes(extinction_map_cache.bytes)
            .sightline_cache_bytes(sightline_cache.bytes)
            .sky_cache_bytes(sky_cache.bytes)
            .sky_tables_bytes(sky_tables.bytes)
            .surface_cache_bytes(surface_cache.bytes)
            .sky_service(serve_sky)
            .stdin_stop(stop_on_stdin_close);
        // Not given, the bound follows the workers (`ServerConfig::surface_passes`).
        match surface_passes {
            Some(passes) => builder.surface_passes(passes),
            None => builder,
        }
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
    extinction_map_cache_bytes: usize,
    sightline_cache_bytes: usize,
    sky_cache_bytes: usize,
    sky_tables_bytes: usize,
    surface_cache_bytes: usize,
    /// The coarse passes run at once, if given; if not, they follow `workers`
    /// (`default_surface_passes`).
    surface_passes: Option<NonZeroUsize>,
    sky_service: SkyService,
    stdin_stop: StdinStop,
    entropy: Arc<dyn Entropy>,
    scene_knowledge: Arc<dyn SceneKnowledge>,
    craft_source: Arc<dyn CraftSource>,
    sky_caps: SkyCaps,
    surface_inputs: Arc<dyn InputsSource>,
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

    /// The extinction map cache's budget, in bytes.
    #[must_use]
    pub fn extinction_map_cache_bytes(&self) -> usize {
        self.extinction_map_cache_bytes
    }

    /// The sightline cache's budget, in bytes.
    #[must_use]
    pub fn sightline_cache_bytes(&self) -> usize {
        self.sightline_cache_bytes
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

    /// The surface service's cache budget, in bytes (rendering plan R09, Design note 19).
    #[must_use]
    pub fn surface_cache_bytes(&self) -> usize {
        self.surface_cache_bytes
    }

    /// Coarse surface passes the surface service runs at once (rendering plan R09, R09.T17.a): the
    /// count given, or else the lesser of 4 and the [workers](Self::workers) less one, at least 1.
    #[must_use]
    pub fn surface_passes(&self) -> NonZeroUsize {
        self.surface_passes
            .unwrap_or_else(|| default_surface_passes(self.workers))
    }

    /// Whether the server answers `sky` (rendering plan R06, R06.T11.c's switch).
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

    /// Where the surface service takes each body's coarse-pass inputs from (rendering plan R09,
    /// the `InputsSource` seam).
    #[must_use]
    pub fn surface_inputs(&self) -> &Arc<dyn InputsSource> {
        &self.surface_inputs
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
                extinction_map_cache_bytes: DEFAULT_EXTINCTION_MAP_CACHE.bytes,
                sightline_cache_bytes: DEFAULT_SIGHTLINE_CACHE.bytes,
                sky_cache_bytes: DEFAULT_SKY_CACHE.bytes,
                sky_tables_bytes: DEFAULT_SKY_TABLES.bytes,
                surface_cache_bytes: DEFAULT_SURFACE_CACHE.bytes,
                surface_passes: None,
                sky_service: SkyService::default(),
                stdin_stop: StdinStop::default(),
                entropy: Arc::new(OsEntropy),
                scene_knowledge: Arc::new(GrantAsked),
                craft_source: Arc::new(NoCraft),
                sky_caps: SkyCaps::DERIVED,
                surface_inputs: Arc::new(RecordInputs),
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
    ///
    /// Unless they are given, the surface passes run at once follow them
    /// ([`ServerConfig::surface_passes`]).
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

    /// The extinction map cache's budget, in bytes. Zero caches nothing.
    #[must_use]
    pub fn extinction_map_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.extinction_map_cache_bytes = bytes;
        self
    }

    /// The sightline cache's budget, in bytes. Zero caches nothing: every line is marched.
    #[must_use]
    pub fn sightline_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.sightline_cache_bytes = bytes;
        self
    }

    /// The sky's cell cache's budget, in bytes.
    ///
    /// Zero, the default, turns the cache off: each census reads no cache and keeps no cell
    /// (R06.T8.o). [`DEFAULT_SKY_CACHE_MIB`] gives the budget to set for a warm sky.
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

    /// The surface service's cache budget, in bytes.
    ///
    /// Zero caches nothing: every request computes its body's field, though requests made at once
    /// still share one pass.
    #[must_use]
    pub fn surface_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.surface_cache_bytes = bytes;
        self
    }

    /// Coarse surface passes the surface service runs at once.
    ///
    /// Not given, the bound follows the [workers](Self::workers), whenever they are set: the lesser
    /// of 4 and the workers less one, at least 1 ([`ServerConfig::surface_passes`]).
    #[must_use]
    pub fn surface_passes(mut self, passes: NonZeroUsize) -> Self {
        self.config.surface_passes = Some(passes);
        self
    }

    /// Whether the server answers `sky`; [`SkyService::Served`] by default, since rendering plan
    /// R13's R13.T2.
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

    /// Where the surface service takes each body's coarse-pass inputs from.
    ///
    /// [`RecordInputs`], the body's record, by default. Until the serve gate opens
    /// (`GENERATED_SURFACES_FROM`, 24; P14.T54.a), that answers every body it reads `NotModelled`,
    /// so tests give the coarse pass's reference worlds; no option or variable sets it.
    #[must_use]
    pub fn surface_inputs(mut self, inputs: impl InputsSource + 'static) -> Self {
        self.config.surface_inputs = Arc::new(inputs);
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

/// The most coarse surface passes run at once by default, whatever the workers.
const MOST_DEFAULT_SURFACE_PASSES: usize = 4;

/// The coarse surface passes run at once on a pool of `workers` when none is given: the lesser of
/// 4 and the workers less one, at least 1 (rendering plan R09, R09.T17.a; decision-r09-pass-cost
/// §C).
///
/// A level-8 pass, an Earth-sized wet world's, peaks at 408 MB (R09.T16's bench, its 22.3 MB floor
/// included), so four at once hold about 1.6 GB. One worker less than the pool's leaves a worker,
/// on a pool of two or more, for the interactive jobs that the displays' requests wait on, which a
/// pass on every worker would hold back by a whole pass. A one-worker pool still runs its passes,
/// one at a time.
#[must_use]
fn default_surface_passes(workers: NonZeroUsize) -> NonZeroUsize {
    NonZeroUsize::new(
        workers
            .get()
            .saturating_sub(1)
            .min(MOST_DEFAULT_SURFACE_PASSES),
    )
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

    /// The address, the data directory, the workers, the surface passes run at once, the caches'
    /// budgets (cells, maps, systems, bodies, briefs, extinction maps, lines of sight, the sky's
    /// cells, the sky tables and the surface), the sky's switch and the stop on standard input.
    type Fields<'a> = (
        SocketAddr,
        &'a Path,
        usize,
        usize,
        [usize; 10],
        SkyService,
        StdinStop,
    );

    fn fields(config: &ServerConfig) -> Fields<'_> {
        (
            config.addr(),
            config.data_dir(),
            config.workers().get(),
            config.surface_passes().get(),
            [
                config.cell_cache_bytes(),
                config.map_cache_bytes(),
                config.system_cache_bytes(),
                config.body_cache_bytes(),
                config.brief_cache_bytes(),
                config.extinction_map_cache_bytes(),
                config.sightline_cache_bytes(),
                config.sky_cache_bytes(),
                config.sky_tables_bytes(),
                config.surface_cache_bytes(),
            ],
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
        let workers = cores.saturating_sub(1).max(1);
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::LOCALHOST, 7878)),
                Path::new("./hyperion-data"),
                workers,
                workers.saturating_sub(1).clamp(1, 4),
                [
                    256 * 1024 * 1024,
                    64 * 1024 * 1024,
                    128 * 1024 * 1024,
                    128 * 1024 * 1024,
                    64 * 1024 * 1024,
                    16 * 1024 * 1024,
                    4 * 1024 * 1024,
                    0,
                    160 * 1024 * 1024,
                    128 * 1024 * 1024,
                ],
                SkyService::Served,
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
            "--extinction-map-cache",
            "11",
            "--sightline-cache",
            "13",
            "--sky-cache",
            "17",
            "--sky-tables",
            "19",
            "--surface-cache",
            "23",
            "--surface-passes",
            "29",
            "--serve-sky=false",
            "--stop-on-stdin-close",
        ]);
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::UNSPECIFIED, 9000)),
                Path::new("/srv/hyperion"),
                3,
                29,
                [
                    1 << 20,
                    0,
                    2 << 20,
                    5 << 20,
                    7 << 20,
                    11 << 20,
                    13 << 20,
                    17 << 20,
                    19 << 20,
                    23 << 20
                ],
                SkyService::Unsupported,
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

    /// The sky's switch is on unless it is turned off, by a boolish value after an equals sign
    /// (R06.T11.c; on by default since rendering plan R13's R13.T2).
    #[test]
    fn the_sky_is_served_unless_its_switch_is_off() {
        assert_eq!(config(&[]).sky_service(), SkyService::Served);
        assert_eq!(
            ServerConfig::builder().build().sky_service(),
            SkyService::Served
        );
        assert_eq!(config(&["--serve-sky"]).sky_service(), SkyService::Served);
        for on in ["true", "yes", "on", "1", "TRUE"] {
            let flag = format!("--serve-sky={on}");
            assert_eq!(config(&[&flag]).sky_service(), SkyService::Served, "{flag}");
        }
        for off in ["false", "no", "off", "0", "Off"] {
            let flag = format!("--serve-sky={off}");
            assert_eq!(
                config(&[&flag]).sky_service(),
                SkyService::Unsupported,
                "{flag}"
            );
        }
        // Given twice, it is refused, as every option is.
        assert_eq!(
            refusal(&["--serve-sky", "--serve-sky=off"]),
            ErrorKind::ArgumentConflict
        );
        for refused in ["--serve-sky=", "--serve-sky=maybe"] {
            assert_eq!(refusal(&[refused]), ErrorKind::ValueValidation, "{refused}");
        }
        // The value takes an equals sign: a word after the switch is not its value.
        assert_eq!(
            refusal(&["--serve-sky", "false"]),
            ErrorKind::UnknownArgument
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
                (
                    Some("extinction-map-cache"),
                    Some("HYPERION_EXTINCTION_MAP_CACHE_MB")
                ),
                (Some("sightline-cache"), Some("HYPERION_SIGHTLINE_CACHE_MB")),
                (Some("sky-cache"), Some("HYPERION_SKY_CACHE_MB")),
                (Some("sky-tables"), Some("HYPERION_SKY_TABLES_MB")),
                (Some("surface-cache"), Some("HYPERION_SURFACE_CACHE_MB")),
                (Some("surface-passes"), Some("HYPERION_SURFACE_PASSES")),
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

    /// A count of surface passes is a whole number above 0, as a count of workers is: 0 is refused
    /// whether written after an equals sign or as the next word (rendering plan R09, R09.T17.a).
    #[test]
    fn a_bad_surface_pass_count_is_refused() {
        for count in ["0", "four", "-2", "1.5"] {
            assert_eq!(
                refusal(&[&format!("--surface-passes={count}")]),
                ErrorKind::ValueValidation,
                "{count:?}"
            );
        }
        assert_eq!(
            refusal(&["--surface-passes", "0"]),
            ErrorKind::ValueValidation
        );
    }

    /// Not given, the surface passes run at once are the lesser of 4 and the workers less one, at
    /// least one, whether the workers come from the command line or the builder, in either order;
    /// given, the count wins, whatever the workers (rendering plan R09, R09.T17.a).
    #[test]
    fn the_surface_passes_follow_the_workers_unless_given() {
        for (workers, passes) in [(1, 1), (2, 1), (3, 2), (4, 3), (5, 4), (16, 4)] {
            let workers = NonZeroUsize::new(workers).unwrap();
            assert_eq!(default_surface_passes(workers).get(), passes, "{workers}");
            assert_eq!(
                ServerConfig::builder()
                    .workers(workers)
                    .build()
                    .surface_passes()
                    .get(),
                passes,
                "{workers}"
            );
            assert_eq!(
                config(&["--num-workers", &workers.to_string()])
                    .surface_passes()
                    .get(),
                passes,
                "{workers}"
            );
        }
        let given = NonZeroUsize::new(7).unwrap();
        for chosen in [
            ServerConfig::builder()
                .surface_passes(given)
                .workers(NonZeroUsize::MIN)
                .build(),
            ServerConfig::builder()
                .workers(NonZeroUsize::new(16).unwrap())
                .surface_passes(given)
                .build(),
            config(&["--surface-passes", "7", "--num-workers", "1"]),
        ] {
            assert_eq!(chosen.surface_passes(), given, "{chosen:?}");
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
        assert_eq!(DEFAULT_EXTINCTION_MAP_CACHE.to_string(), "16");
        assert_eq!(DEFAULT_SIGHTLINE_CACHE.to_string(), "4");
        assert_eq!(DEFAULT_SKY_CACHE.to_string(), "0");
        assert_eq!(DEFAULT_SKY_TABLES.to_string(), "160");
        assert_eq!(DEFAULT_SURFACE_CACHE.to_string(), "128");
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

    /// "fo" and an unpaired high surrogate: UTF-16 that no `str` can hold.
    #[cfg(windows)]
    #[test]
    fn a_number_that_is_not_unicode_is_refused() {
        use std::os::windows::ffi::OsStringExt;

        let error = parse_os([
            OsString::from("--num-workers"),
            OsString::from_wide(&[0x66, 0x6F, 0xD800]),
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

    /// "fo" and an unpaired high surrogate, which a path keeps.
    #[cfg(windows)]
    #[test]
    fn a_data_directory_need_not_be_unicode() {
        use std::os::windows::ffi::OsStringExt;

        let dir = OsString::from_wide(&[0x66, 0x6F, 0xD800]);
        let args = parse_os([OsString::from("--data-dir"), dir.clone()]).unwrap();
        assert_eq!(ServerConfig::from(args).data_dir(), Path::new(&dir));
    }

    /// The surface service reads each body's record by default, and a test gives its own source.
    #[test]
    fn the_surface_inputs_are_the_records_unless_given() {
        #[derive(Debug)]
        struct Given;
        impl InputsSource for Given {
            fn inputs(
                &self,
                _: &hyperion_sim::planetary::record::BodyRecord,
                _: Option<&hyperion_sim::planetary::record::BodyRecord>,
                _: &hyperion_sim::planetary::SystemContext,
            ) -> Result<
                hyperion_sim::planetary::surface::CoarseInputs,
                hyperion_sim::planetary::surface::ReadCoarseInputsError,
            > {
                Err(hyperion_sim::planetary::surface::SurfaceInputsError::NoSolidSurface.into())
            }
        }
        assert_eq!(
            format!("{:?}", ServerConfig::builder().build().surface_inputs()),
            "RecordInputs"
        );
        assert_eq!(
            format!(
                "{:?}",
                ServerConfig::builder()
                    .surface_inputs(Given)
                    .build()
                    .surface_inputs()
            ),
            "Given"
        );
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
            .extinction_map_cache_bytes(60)
            .sightline_cache_bytes(70)
            .sky_cache_bytes(80)
            .sky_tables_bytes(90)
            .surface_cache_bytes(100)
            .surface_passes(NonZeroUsize::new(3).unwrap())
            .sky_service(SkyService::Unsupported)
            .stdin_stop(StdinStop::Watch)
            .build();
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                Path::new("/tmp/x"),
                2,
                3,
                [10, 20, 30, 40, 50, 60, 70, 80, 90, 100],
                SkyService::Unsupported,
                StdinStop::Watch,
            )
        );
    }
}
