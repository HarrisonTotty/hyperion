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
//! | `--feature-cache`        | `HYPERION_FEATURE_CACHE_MB`        | 16 (MiB)                                               |
//! | `--cluster-model-cache`  | `HYPERION_CLUSTER_MODEL_CACHE_MB`  | 64 (MiB)                                               |
//! | `--class-cell-cache`     | `HYPERION_CLASS_CELL_CACHE_MB`     | 32 (MiB)                                               |
//! | `--alert-cache`          | `HYPERION_ALERT_CACHE_MB`          | 32 (MiB)                                               |
//! | `--sky-cache`            | `HYPERION_SKY_CACHE_MB`            | 0 (MiB), off                                           |
//! | `--sky-tables`           | `HYPERION_SKY_TABLES_MB`           | 160 (MiB)                                              |
//! | `--surface-cache`        | `HYPERION_SURFACE_CACHE_MB`        | 128 (MiB)                                              |
//! | `--surface-passes`       | `HYPERION_SURFACE_PASSES`          | the lesser of 4 and the workers less one, at least one |
//! | `--serve-sky`            | `HYPERION_SERVE_SKY`               | on                                                     |
//! | `--keep-skies`           | `HYPERION_KEEP_SKIES`              | on                                                     |
//! | `--kept-skies-disk`      | `HYPERION_KEPT_SKIES_DISK_MB`      | 256 (MiB)                                              |
//! | `--hold-kept-skies`      | `HYPERION_HOLD_KEPT_SKIES`         | on                                                     |
//! | `--sky-qos`              | `HYPERION_SKY_QOS`                 | `background` on macOS, `utility` elsewhere             |
//! | `--stop-on-stdin-close`  | `HYPERION_STOP_ON_STDIN_CLOSE`     | off                                                    |
//!
//! On Apple silicon `--num-workers` defaults to no more than its performance threads with a third
//! of its efficiency threads, less two, so that the pool and a client's height workers together
//! stay within about twice the performance cores (four on an M5's 4 performance and 6 efficiency
//! cores).
//!
//! `--serve-sky`, `--keep-skies`, `--hold-kept-skies` and `--stop-on-stdin-close` are switches.
//! Their variables take clap's boolish values, in any case: `y`, `yes`, `t`, `true`, `on` or `1` for
//! on, and `n`, `no`, `f`, `false`, `off` or `0` for off. They refuse anything else, the empty
//! string included. `--stop-on-stdin-close`, off by default, takes no value: given, it is on.
//! `--serve-sky`, `--keep-skies` and `--hold-kept-skies`, on by default, also take one of those
//! values after an equals sign, so that the command line can turn them off: `--serve-sky=false`.
//! Given alone, each is on.
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
//!
//! `--keep-skies` keeps the final replies of the last skies finished, in
//! [`SKY_REPLY_CACHE_BYTES`](crate::limits::SKY_REPLY_CACHE_BYTES), and answers a sky asked again
//! exactly as one of them from memory, with no sky job on the pool (rendering plan R06, R06.T11.l;
//! the owner's priority, 2026-11-01). It is a different cache from `--sky-cache`'s: whole replies,
//! not census cells, so it saves nothing for a sky asked from anywhere else, but a relaunched
//! client's or a reopened view's repeated sky costs next to nothing. `--keep-skies=false` computes
//! every sky afresh.
//!
//! `--kept-skies-disk` bounds the same final replies kept on disk, in `skies/` under the data
//! directory, so that a sky asked again after a restart is answered from its file (rendering plan
//! R06, R06.T11.n; the owner's priority, 2026-11-05). 0 keeps none there, and `--keep-skies=false`
//! none anywhere.
//!
//! `--hold-kept-skies` answers a sky asked at a later time, or from a little way off, with a kept
//! reply of the same request otherwise that still holds there ([`HeldSkies`], R06.T11.p): before
//! its `valid_until`, and from within a tenth of a pixel of its nearest star's parallax.
//! `--hold-kept-skies=false` answers only a sky asked exactly again.
//!
//! `--sky-qos` is the class a sky's census runs at after its first reply ([`SkyQos`], R06.T11.o):
//! `utility` or `background`. Only macOS's pool follows it; elsewhere either value leaves every
//! bulk job at the pool's lowered level.

use crate::scene::{CraftSource, GrantAsked, NoCraft, SceneKnowledge};
use std::error::Error;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use clap::builder::{BoolishValueParser, TypedValueParser};
use clap::{ArgAction, Parser, ValueEnum};

use crate::DEFAULT_ADDR;
use crate::compute::{BulkGate, SkyCaps, TwinPriority};
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
/// The variable giving the feature cell cache's budget in MiB, for `--feature-cache`.
pub const ENV_FEATURE_CACHE_MB: &str = "HYPERION_FEATURE_CACHE_MB";
/// The variable giving the cluster model cache's budget in MiB, for `--cluster-model-cache`.
pub const ENV_CLUSTER_MODEL_CACHE_MB: &str = "HYPERION_CLUSTER_MODEL_CACHE_MB";
/// The variable giving the catalogue class cell cache's budget in MiB, for `--class-cell-cache`.
pub const ENV_CLASS_CELL_CACHE_MB: &str = "HYPERION_CLASS_CELL_CACHE_MB";
/// The variable giving the alert scans' cache's budget in MiB, for `--alert-cache`.
pub const ENV_ALERT_CACHE_MB: &str = "HYPERION_ALERT_CACHE_MB";
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
/// The variable that turns the kept skies off, or on, for `--keep-skies`.
pub const ENV_KEEP_SKIES: &str = "HYPERION_KEEP_SKIES";
/// The variable giving the kept skies' files' budget in MiB, for `--kept-skies-disk`.
pub const ENV_KEPT_SKIES_DISK_MB: &str = "HYPERION_KEPT_SKIES_DISK_MB";
/// The variable that turns the held kept skies off, or on, for `--hold-kept-skies`.
pub const ENV_HOLD_KEPT_SKIES: &str = "HYPERION_HOLD_KEPT_SKIES";
/// The variable naming the class a sky's census runs at after its first reply, for `--sky-qos`.
pub const ENV_SKY_QOS: &str = "HYPERION_SKY_QOS";
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
/// The alert scans' cache's budget when `--alert-cache` is not given, in MiB (plan 12, P12.T8.b):
/// a track is charged some 600 bytes, its detection's 456 and two or three crossings' 32 each
/// (measured 2026-11-01), so a year's scan of a few thousand local events is about 2 MB, and this
/// holds some fifteen such scans, a look-ahead each for as many subscriptions.
pub const DEFAULT_ALERT_CACHE_MIB: usize = 32;
/// The sightline cache's budget when `--sightline-cache` is not given, in MiB (plan 07,
/// P07.T10.c): a line is charged its 32 bytes and the cache's 96 an entry, 128 bytes, so this holds
/// 32,768, a few hundred charts' selections.
pub const DEFAULT_SIGHTLINE_CACHE_MIB: usize = 4;
/// The feature cell cache's budget when `--feature-cache` is not given, in MiB (plan 09,
/// P09.T40.a): a cell's contents are charged their records, 168 bytes each, so a 4,096 ly cell
/// near the Sun, some 220 features, is about 38 kB (measured 2026-10-30 on the server's tests'
/// galaxy: the 64 cells a lookup within 3,000 ly of the Sun-like point reads, 2.4 MB in all), and
/// this holds some 440 of them, seven such lookups' cells.
pub const DEFAULT_FEATURE_CACHE_MIB: usize = 16;
/// The cluster model cache's budget when `--cluster-model-cache` is not given, in MiB (plan 09,
/// P09.T40.a): each feature's interior, its cluster model, class table and nested grid, which
/// costs 14–17 ms to build (plan 09's Risks, "Ruling 139 as built", 139.5) and is charged its class
/// table's rows and profiles,
/// 27 kB on average (measured 2026-10-30: the 477 interiors of the features within 3,000 ly of the
/// Sun-like point, 13 MB), so this holds some 2,400.
pub const DEFAULT_CLUSTER_MODEL_CACHE_MIB: usize = 64;
/// The catalogue class cell cache's budget when `--class-cell-cache` is not given, in MiB (plan
/// 09, P09.T40): each class cell's places
/// read first and other classes' members ([`ClassCellSystems`]), charged their buffers. A cold
/// Sun-like 50 ly query under 27's rules reads 8,720 class cells, 1.45 MB (measured 2026-11-01 on
/// the server's tests' galaxy, pod), so this holds some twenty such neighbourhoods' cells, and a
/// query within the classes' 4,096 ly reach of another shares most of them. A placed class's cell
/// keeps at most the places a quarter of the budget holds, 174,762 here: a denser one keeps none,
/// and a query reads its places again as it walks it, so that a query at the galactic centre makes
/// no cell of 557 MiB (the mergers' balance, part 3). A larger budget keeps denser cells.
///
/// [`ClassCellSystems`]: hyperion_sim::galaxy::catalogue_classes::ClassCellSystems
pub const DEFAULT_CLASS_CELL_CACHE_MIB: usize = 32;
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
/// 2026-10-03, `decision-r06-tables.md`, item B.2). With each galaxy's features' gas index beside
/// its tables, some 52 MB more (P09.T40.a), it holds one galaxy's.
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
/// The kept skies' files' budget when `--kept-skies-disk` is not given, in MiB (rendering plan R06,
/// R06.T11.n; lean, 2026-11-05): a sky at `N_max`'s 3 × 10⁵ stars is a file of some 7.6 MB, its
/// 7.49 MB payload and its JSON, so this holds some 33 of them, and more of a camera's sky near the
/// Sun, which lists some 7 × 10⁴ stars.
pub const DEFAULT_KEPT_SKIES_DISK_MIB: usize = 256;
/// The streams' envelopes' files' budget, in MiB (plan 10, P10.T10.b; perf-range-envelopes-on-disk,
/// lean, 2026-11-05): an envelope's file is 1,586 bytes, so this holds some 10,000, the envelopes of
/// 32 galaxies of the server tests' 321 streams, or of 7 of the largest galaxies' 1,500. No option
/// or variable sets it; [`ServerConfigBuilder::kept_envelopes_disk_bytes`] does.
pub const DEFAULT_KEPT_ENVELOPES_DISK_MIB: usize = 16;
/// The sky tables' files' budget, in MiB (rendering plan R06; perf-sky-tables-on-disk, lean,
/// 2026-11-05): the tables' budget in memory, [`DEFAULT_SKY_TABLES_MIB`] (`decision-r06-tables.md`,
/// item B.2), two galaxies' tables of some 68 MiB, or one galaxy's with its features' gas index.
/// No option or variable sets it; [`ServerConfigBuilder::kept_sky_tables_disk_bytes`] does.
pub const DEFAULT_KEPT_SKY_TABLES_DISK_MIB: usize = DEFAULT_SKY_TABLES_MIB;

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
const DEFAULT_FEATURE_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_FEATURE_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_CLUSTER_MODEL_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_CLUSTER_MODEL_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_CLASS_CELL_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_CLASS_CELL_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_ALERT_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_ALERT_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_SKY_CACHE: CacheBudget = CacheBudget {
    bytes: DEFAULT_SKY_CACHE_MIB * BYTES_PER_MIB,
};
const DEFAULT_SKY_TABLES: CacheBudget = CacheBudget {
    bytes: DEFAULT_SKY_TABLES_MIB * BYTES_PER_MIB,
};
const DEFAULT_KEPT_SKIES_DISK: CacheBudget = CacheBudget {
    bytes: DEFAULT_KEPT_SKIES_DISK_MIB * BYTES_PER_MIB,
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

    /// Generation worker threads; defaults to the available parallelism less one, at least one,
    /// and on Apple silicon to its performance threads with a third of its efficiency threads,
    /// less two
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

    /// Budget of the cache of generated planetary systems, and as much again of their event
    /// models, in MiB; 0 caches nothing
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

    /// Budget of the cache of the large features' catalogue cells, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_FEATURE_CACHE_MB, default_value_t = DEFAULT_FEATURE_CACHE)]
    feature_cache: CacheBudget,

    /// Budget of the cache of the large features' interiors, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_CLUSTER_MODEL_CACHE_MB, default_value_t = DEFAULT_CLUSTER_MODEL_CACHE)]
    cluster_model_cache: CacheBudget,

    /// Budget of the cache of the catalogue classes' cells the range query reads, in MiB; 0 caches
    /// nothing
    #[arg(long, value_name = "MIB", env = ENV_CLASS_CELL_CACHE_MB, default_value_t = DEFAULT_CLASS_CELL_CACHE)]
    class_cell_cache: CacheBudget,

    /// Budget of the cache of the alert scans' walks, in MiB; 0 caches nothing
    #[arg(long, value_name = "MIB", env = ENV_ALERT_CACHE_MB, default_value_t = DEFAULT_ALERT_CACHE)]
    alert_cache: CacheBudget,

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

    /// Answer a sky asked again exactly from the last skies' final replies, kept;
    /// `--keep-skies=false` computes every sky afresh
    #[arg(
        long,
        value_name = "BOOL",
        env = ENV_KEEP_SKIES,
        num_args = 0..=1,
        require_equals = true,
        default_value = "true",
        default_missing_value = "true",
        value_parser = BoolishValueParser::new().map(kept_skies),
    )]
    keep_skies: KeptSkies,

    /// Budget of the kept skies' final replies on disk, under the data directory, in MiB; 0 keeps
    /// none there
    #[arg(long, value_name = "MIB", env = ENV_KEPT_SKIES_DISK_MB, default_value_t = DEFAULT_KEPT_SKIES_DISK)]
    kept_skies_disk: CacheBudget,

    /// Answer a sky asked at a later time, or from a little way off, from a kept sky that still
    /// holds there; `--hold-kept-skies=false` answers only a sky asked exactly again
    #[arg(
        long,
        value_name = "BOOL",
        env = ENV_HOLD_KEPT_SKIES,
        num_args = 0..=1,
        require_equals = true,
        default_value = "true",
        default_missing_value = "true",
        value_parser = BoolishValueParser::new().map(held_skies),
    )]
    hold_kept_skies: HeldSkies,

    /// The class a sky's census runs at after its first reply, on macOS: `background`, the default
    /// there, keeps it on the efficiency cores, and `utility` leaves it at the pool's own class.
    /// Elsewhere either leaves it at the pool's lowered level
    #[arg(long, value_name = "CLASS", env = ENV_SKY_QOS, value_enum, default_value_t = SkyQos::default())]
    sky_qos: SkyQos,

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

/// Whether the server keeps the last skies' final replies and answers a sky asked again exactly
/// from them: its switch, `--keep-skies` (rendering plan R06, R06.T11.l; the owner's priority,
/// 2026-11-01).
///
/// VIEW asks its camera's sky as each inspection opens, and a relaunch or a reopened inspection asks
/// the same sky again: near the Sun its census is hours of CPU, all paid again without them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum KeptSkies {
    /// Every sky is computed afresh, as before R06.T11.l.
    Off,
    /// The final replies of the last skies finished are kept, in
    /// [`SKY_REPLY_CACHE_BYTES`](crate::limits::SKY_REPLY_CACHE_BYTES), and a sky asked again
    /// exactly as one of them is answered from it: the default.
    #[default]
    Kept,
}

/// Whether a sky asked at a later time, or from a little way off, is answered from a kept sky that
/// still holds there: its switch, `--hold-kept-skies` (rendering plan R06, R06.T11.p).
///
/// VIEW asks at the scene's time and from the camera, so with the clock running a relaunched client
/// asks a sky that differs from every kept one in its time: near the Sun the same camera's sky an
/// hour of universe time later cost a cold census again, some 4,000 s on four workers. A kept sky
/// holds from its own time until its `valid_until`, and for an observer within a tenth of a pixel
/// of its nearest star's parallax (Design note 13), as the client holds the sky it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum HeldSkies {
    /// Only a sky asked exactly as one kept is answered from it, as before R06.T11.p.
    Exact,
    /// A sky is also answered from a kept sky of the same request but for its time and observer
    /// that holds for it: the default.
    #[default]
    Held,
}

/// The class a sky's census runs at after its first reply: its switch, `--sky-qos` (rendering plan
/// R06, R06.T11.o; the owner's priority, 2026-11-05).
///
/// On the owner's Mac a cold camera sky near the Sun keeps the pool's twins busy for about an hour
/// at the quality-of-service class utility, which leans to the efficiency cores but may take the
/// performance cores the client draws with. Background holds the census's jobs after its first
/// reply, the one the player waits for, to the efficiency cores. Only macOS's twins follow a job's
/// class, job by job, and only lowered ones ([`TwinPriority::Lowered`]); on Linux and Windows
/// either value runs every bulk job at the twin's own level, as before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, ValueEnum)]
pub enum SkyQos {
    /// Every job at the twins' own class, utility on macOS: the default elsewhere.
    Utility,
    /// The jobs after the sky's first reply at the class background on macOS: the default there
    /// (lean, 2026-11-05).
    Background,
}

impl Default for SkyQos {
    /// [`SkyQos::Background`] on macOS, and [`SkyQos::Utility`] elsewhere, where it changes
    /// nothing.
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            Self::Background
        } else {
            Self::Utility
        }
    }
}

/// Whether opening a universe starts its galaxy's global list and its streams' tube tables on the
/// pool (plan 10, P10.T10.b).
///
/// A Milky Way galaxy's list costs some 0.5 CPU-s and its tables some minutes, at bulk priority,
/// so a test of something else that opens universes turns it off, as it turns the sky's tables
/// off ([`SkyService`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum GlobalListWarmUp {
    /// An open starts neither: a galaxy's list is built for its first request that reads it, and
    /// a stream's table for the first that reads the stream's tube.
    Off,
    /// An open builds the galaxy's list, then each of its streams' tube tables in turn, both at
    /// bulk priority, and answers without waiting for them: the default.
    #[default]
    AtOpen,
}

/// Whether the server serves plan 09's feature members (P09.T40.a), which it does from generator
/// version 27, with P09.T2.c's field share (main's ruling of 2026-10-30; the serve gate opened at
/// 27's bump).
///
/// Served, the range query registers the catalogue features' members as a source beside the grid
/// (`FeatureMemberSource`, over the server's feature caches), every row carries its `origin`, and
/// every ray and star sightline of the sky takes the features' gas (`FeatureGas`), looked up in a
/// `FeatureGasIndex` of the galaxy's nurseries and clouds that its sky tables are built with (some
/// 52 MB for the Milky Way-like galaxy, and some 7 CPU-s on the pool). Held is the tests' and
/// benches' door to the grid's answer alone, which no option or variable sets: with the field share
/// on the record, a server that holds them back misses each feature's share of its population. The
/// catalogue classes' hosts join the members from generator version 27
/// (`CatalogueClassSource::registered_in`, P09.T37.a); the galactic centre's members wait on
/// P09.T37.b.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FeatureMembers {
    /// No feature member is served, and no row carries an origin: the grid's answer alone, the
    /// tests' and benches' door.
    Held,
    /// The features' members, the rows' origins and the features' gas are served.
    Served,
}

impl Default for FeatureMembers {
    /// [`FeatureMembers::Served`], every version's.
    fn default() -> Self {
        Self::Served
    }
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

/// The kept skies that a switch asks for.
#[must_use]
fn kept_skies(keep: bool) -> KeptSkies {
    if keep {
        KeptSkies::Kept
    } else {
        KeptSkies::Off
    }
}

/// The held kept skies that a switch asks for.
#[must_use]
fn held_skies(hold: bool) -> HeldSkies {
    if hold {
        HeldSkies::Held
    } else {
        HeldSkies::Exact
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
            feature_cache,
            cluster_model_cache,
            class_cell_cache,
            alert_cache,
            sky_cache,
            sky_tables,
            surface_cache,
            surface_passes,
            serve_sky,
            keep_skies,
            kept_skies_disk,
            hold_kept_skies,
            sky_qos,
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
            .feature_cache_bytes(feature_cache.bytes)
            .cluster_model_cache_bytes(cluster_model_cache.bytes)
            .class_cell_cache_bytes(class_cell_cache.bytes)
            .alert_cache_bytes(alert_cache.bytes)
            .sky_cache_bytes(sky_cache.bytes)
            .sky_tables_bytes(sky_tables.bytes)
            .surface_cache_bytes(surface_cache.bytes)
            .sky_service(serve_sky)
            .kept_skies(keep_skies)
            .kept_skies_disk_bytes(kept_skies_disk.bytes)
            .held_skies(hold_kept_skies)
            .sky_qos(sky_qos)
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
    feature_cache_bytes: usize,
    cluster_model_cache_bytes: usize,
    class_cell_cache_bytes: usize,
    alert_cache_bytes: usize,
    sky_cache_bytes: usize,
    sky_tables_bytes: usize,
    surface_cache_bytes: usize,
    /// The coarse passes run at once, if given; if not, they follow `workers`
    /// (`default_surface_passes`).
    surface_passes: Option<NonZeroUsize>,
    sky_service: SkyService,
    kept_skies: KeptSkies,
    kept_skies_disk_bytes: usize,
    held_skies: HeldSkies,
    sky_qos: SkyQos,
    global_list_warm_up: GlobalListWarmUp,
    kept_envelopes_disk_bytes: usize,
    kept_sky_tables_disk_bytes: usize,
    feature_members: FeatureMembers,
    stdin_stop: StdinStop,
    entropy: Arc<dyn Entropy>,
    scene_knowledge: Arc<dyn SceneKnowledge>,
    craft_source: Arc<dyn CraftSource>,
    sky_caps: SkyCaps,
    surface_inputs: Arc<dyn InputsSource>,
    /// A test's hold on the pool's bulk jobs; none by default.
    bulk_gate: Option<BulkGate>,
    /// The priority the pool's twins run its bulk and background jobs at; lowered by default.
    twin_priority: TwinPriority,
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

    /// The feature cell cache's budget, in bytes (plan 09, P09.T40.a).
    #[must_use]
    pub fn feature_cache_bytes(&self) -> usize {
        self.feature_cache_bytes
    }

    /// The cluster model cache's budget, in bytes (plan 09, P09.T40.a).
    #[must_use]
    pub fn cluster_model_cache_bytes(&self) -> usize {
        self.cluster_model_cache_bytes
    }

    /// The catalogue class cell cache's budget, in bytes (plan 09, P09.T40).
    #[must_use]
    pub fn class_cell_cache_bytes(&self) -> usize {
        self.class_cell_cache_bytes
    }

    /// The alert scans' cache's budget, in bytes (plan 12, P12.T8.b).
    #[must_use]
    pub fn alert_cache_bytes(&self) -> usize {
        self.alert_cache_bytes
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

    /// Whether the server keeps the last skies' final replies (rendering plan R06, R06.T11.l's
    /// switch).
    #[must_use]
    pub fn kept_skies(&self) -> KeptSkies {
        self.kept_skies
    }

    /// The kept skies' files' budget, bytes, under the data directory (rendering plan R06,
    /// R06.T11.n): 0, keeping none there, where the kept skies are off.
    #[must_use]
    pub fn kept_skies_disk_bytes(&self) -> usize {
        match self.kept_skies {
            KeptSkies::Kept => self.kept_skies_disk_bytes,
            KeptSkies::Off => 0,
        }
    }

    /// Whether a sky is answered from a kept sky that holds for it (rendering plan R06,
    /// R06.T11.p): [`HeldSkies::Exact`] where the kept skies are off, there being none.
    #[must_use]
    pub fn held_skies(&self) -> HeldSkies {
        match self.kept_skies {
            KeptSkies::Kept => self.held_skies,
            KeptSkies::Off => HeldSkies::Exact,
        }
    }

    /// The class a sky's census runs at after its first reply (rendering plan R06, R06.T11.o).
    #[must_use]
    pub fn sky_qos(&self) -> SkyQos {
        self.sky_qos
    }

    /// Whether an open starts its galaxy's global list and tube tables (plan 10, P10.T10.b).
    #[must_use]
    pub fn global_list_warm_up(&self) -> GlobalListWarmUp {
        self.global_list_warm_up
    }

    /// The streams' envelopes' files' budget, bytes, under the data directory (plan 10,
    /// P10.T10.b; perf-range-envelopes-on-disk): 0 keeps none there.
    #[must_use]
    pub fn kept_envelopes_disk_bytes(&self) -> usize {
        self.kept_envelopes_disk_bytes
    }

    /// The sky tables' files' budget, bytes, under the data directory (rendering plan R06;
    /// perf-sky-tables-on-disk): 0 keeps none there.
    #[must_use]
    pub fn kept_sky_tables_disk_bytes(&self) -> usize {
        self.kept_sky_tables_disk_bytes
    }

    /// Whether the server serves plan 09's feature members (P09.T40.a).
    #[must_use]
    pub fn feature_members(&self) -> FeatureMembers {
        self.feature_members
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

    /// The gate the pool's bulk jobs wait at before they run, if a test gave one.
    #[must_use]
    pub fn bulk_gate(&self) -> Option<&BulkGate> {
        self.bulk_gate.as_ref()
    }

    /// The priority the pool's twins run its bulk and background jobs at:
    /// [`TwinPriority::Lowered`] unless a test keeps its workers'.
    #[must_use]
    pub fn twin_priority(&self) -> TwinPriority {
        self.twin_priority
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
                feature_cache_bytes: DEFAULT_FEATURE_CACHE.bytes,
                cluster_model_cache_bytes: DEFAULT_CLUSTER_MODEL_CACHE.bytes,
                class_cell_cache_bytes: DEFAULT_CLASS_CELL_CACHE.bytes,
                alert_cache_bytes: DEFAULT_ALERT_CACHE.bytes,
                sky_cache_bytes: DEFAULT_SKY_CACHE.bytes,
                sky_tables_bytes: DEFAULT_SKY_TABLES.bytes,
                surface_cache_bytes: DEFAULT_SURFACE_CACHE.bytes,
                surface_passes: None,
                sky_service: SkyService::default(),
                kept_skies: KeptSkies::default(),
                kept_skies_disk_bytes: DEFAULT_KEPT_SKIES_DISK.bytes,
                held_skies: HeldSkies::default(),
                sky_qos: SkyQos::default(),
                global_list_warm_up: GlobalListWarmUp::default(),
                kept_envelopes_disk_bytes: DEFAULT_KEPT_ENVELOPES_DISK_MIB * BYTES_PER_MIB,
                kept_sky_tables_disk_bytes: DEFAULT_KEPT_SKY_TABLES_DISK_MIB * BYTES_PER_MIB,
                feature_members: FeatureMembers::default(),
                stdin_stop: StdinStop::default(),
                entropy: Arc::new(OsEntropy),
                scene_knowledge: Arc::new(GrantAsked),
                craft_source: Arc::new(NoCraft),
                sky_caps: SkyCaps::DERIVED,
                surface_inputs: Arc::new(RecordInputs),
                bulk_gate: None,
                twin_priority: TwinPriority::default(),
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

    /// The feature cell cache's budget, in bytes. Zero caches nothing: every cell is generated.
    #[must_use]
    pub fn feature_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.feature_cache_bytes = bytes;
        self
    }

    /// The cluster model cache's budget, in bytes. Zero caches nothing: every interior is built.
    #[must_use]
    pub fn cluster_model_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.cluster_model_cache_bytes = bytes;
        self
    }

    /// The catalogue class cell cache's budget, in bytes. Zero caches nothing: every class cell a
    /// query reads is made for it.
    #[must_use]
    pub fn class_cell_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.class_cell_cache_bytes = bytes;
        self
    }

    /// The alert scans' cache's budget, in bytes. Zero caches nothing: every scan walks afresh,
    /// and only the subscriptions that ask for one walk at once share it.
    #[must_use]
    pub fn alert_cache_bytes(mut self, bytes: usize) -> Self {
        self.config.alert_cache_bytes = bytes;
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

    /// Whether the server keeps the last skies' final replies; [`KeptSkies::Kept`] by default.
    #[must_use]
    pub fn kept_skies(mut self, kept: KeptSkies) -> Self {
        self.config.kept_skies = kept;
        self
    }

    /// Budget of the kept skies' files under the data directory, bytes;
    /// [`DEFAULT_KEPT_SKIES_DISK_MIB`] by default, and 0 keeps none there.
    #[must_use]
    pub fn kept_skies_disk_bytes(mut self, bytes: usize) -> Self {
        self.config.kept_skies_disk_bytes = bytes;
        self
    }

    /// Whether a sky is answered from a kept sky that holds for it; [`HeldSkies::Held`] by default.
    #[must_use]
    pub fn held_skies(mut self, held: HeldSkies) -> Self {
        self.config.held_skies = held;
        self
    }

    /// The class a sky's census runs at after its first reply; [`SkyQos::default`] by default.
    #[must_use]
    pub fn sky_qos(mut self, qos: SkyQos) -> Self {
        self.config.sky_qos = qos;
        self
    }

    /// Whether an open starts its galaxy's global list and tube tables;
    /// [`GlobalListWarmUp::AtOpen`] by default.
    #[must_use]
    pub fn global_list_warm_up(mut self, warm_up: GlobalListWarmUp) -> Self {
        self.config.global_list_warm_up = warm_up;
        self
    }

    /// Budget of the streams' envelopes' files under the data directory, bytes;
    /// [`DEFAULT_KEPT_ENVELOPES_DISK_MIB`] by default, and 0 keeps none there.
    #[must_use]
    pub fn kept_envelopes_disk_bytes(mut self, bytes: usize) -> Self {
        self.config.kept_envelopes_disk_bytes = bytes;
        self
    }

    /// Budget of the sky tables' files under the data directory, bytes;
    /// [`DEFAULT_KEPT_SKY_TABLES_DISK_MIB`] by default, and 0 keeps none there.
    #[must_use]
    pub fn kept_sky_tables_disk_bytes(mut self, bytes: usize) -> Self {
        self.config.kept_sky_tables_disk_bytes = bytes;
        self
    }

    /// Whether the server serves plan 09's feature members; [`FeatureMembers::Served`] by default.
    ///
    /// Tests and benches hold them back, to compare an answer with the grid's alone; no option or
    /// variable sets it (P09.T40.a).
    #[must_use]
    pub fn feature_members(mut self, members: FeatureMembers) -> Self {
        self.config.feature_members = members;
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
    /// [`RecordInputs`], the body's record, by default, which serves every generated body's field
    /// the sim reads from generator version 25, when the serve gate opened (P14.T54.a). Tests give
    /// the coarse pass's reference worlds, or a source of their own; no option or variable sets it.
    #[must_use]
    pub fn surface_inputs(mut self, inputs: impl InputsSource + 'static) -> Self {
        self.config.surface_inputs = Arc::new(inputs);
        self
    }

    /// A gate every bulk job waits at, once a worker has taken it, before it runs; none by default.
    /// A test holds the bulk work with it while it queues other work, so that it asserts the order
    /// the workers take jobs in rather than a margin of time ([`BulkGate`]); no option or variable
    /// sets it.
    #[must_use]
    pub fn bulk_gate(mut self, gate: BulkGate) -> Self {
        self.config.bulk_gate = Some(gate);
        self
    }

    /// The priority the pool's twins run its bulk and background jobs at:
    /// [`TwinPriority::Lowered`], the server's, by default.
    ///
    /// A test that runs beside other tests keeps its workers' ([`TwinPriority::Workers`]), so that
    /// its bulk work does not wait on theirs under load; no option or variable sets it.
    #[must_use]
    pub fn twin_priority(mut self, twins: TwinPriority) -> Self {
        self.config.twin_priority = twins;
        self
    }

    /// The configuration.
    #[must_use]
    pub fn build(self) -> ServerConfig {
        self.config
    }
}

/// The workers by default on this machine ([`workers_for`] of its parallelism and its core
/// levels).
fn default_workers() -> NonZeroUsize {
    workers_for(
        std::thread::available_parallelism().ok(),
        CoreLevels::of_this_machine(),
    )
}

/// The logical processors of a machine whose OS tells its performance cores from its efficiency
/// cores: Apple silicon's, as `hw.perflevel0` and `hw.perflevel1` count them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CoreLevels {
    performance: usize,
    efficiency: usize,
}

impl CoreLevels {
    /// This machine's levels, read through gdt-cpus (`CpuInfo::detect`, which reads the
    /// `hw.perflevelN` nodes on macOS): `None` off macOS, as the client's own count reads them on
    /// macOS alone (`performanceThreads.ts`), on a Mac with one level (an Intel one), and where the
    /// reading fails.
    fn of_this_machine() -> Option<Self> {
        if !cfg!(target_os = "macos") {
            return None;
        }
        // A failed reading only leaves the default as it is off Apple silicon.
        let info = gdt_cpus::CpuInfo::detect().ok()?;
        let mut levels = Self {
            performance: 0,
            efficiency: 0,
        };
        for lp in &info.lps {
            match lp.kind {
                gdt_cpus::CoreKind::Performance => levels.performance += 1,
                gdt_cpus::CoreKind::Efficiency | gdt_cpus::CoreKind::LpEfficiency => {
                    levels.efficiency += 1;
                }
                gdt_cpus::CoreKind::Unknown => {}
            }
        }
        (levels.performance > 0 && levels.efficiency > 0).then_some(levels)
    }
}

/// The efficiency cores' threads counted as one performance core's thread: three, as the
/// client's height workers count them (`EFFICIENCY_THREADS_A_PERFORMANCE_THREAD` in
/// `performanceThreads.ts`, a lean of 2026-11-01).
const EFFICIENCY_THREADS_A_PERFORMANCE_THREAD: usize = 3;

/// The performance threads the pool leaves by default on Apple silicon: two, as the client's height
/// workers leave for the render thread and the GPU process
/// (`PERFORMANCE_THREADS_LEFT_BESIDE_HEIGHT_WORKERS` in `workers/pool.ts`), so that the pool is as
/// many as they are.
const PERFORMANCE_THREADS_LEFT_BESIDE_WORKERS: usize = 2;

/// The default worker count: `available` less one, for the runtime and the sockets, and at least
/// one; on a machine of [`CoreLevels`], no more than its P performance and E efficiency threads'
/// P + ⌊E ÷ 3⌋ − 2, at least one (a lean, 2026-11-03).
///
/// A new terrain session on the owner's Mac (an M5's 4 performance and 6 efficiency cores) took the
/// load from about 4 to 20–45. The client's height workers there are 4, counted by performance
/// cores (`defaultHeightWorkers`), where this pool took 9, the processors less one. Bounded the same
/// way, the pool is 4 there too, so that the two together stay under about twice the performance
/// cores. A worker's count moves no value, only when jobs end, and `--num-workers` still sets any
/// count.
#[must_use]
fn workers_for(available: Option<NonZeroUsize>, levels: Option<CoreLevels>) -> NonZeroUsize {
    let less_one = available
        .and_then(|cores| NonZeroUsize::new(cores.get() - 1))
        .unwrap_or(NonZeroUsize::MIN);
    let Some(levels) = levels else {
        return less_one;
    };
    let threads = levels.performance + levels.efficiency / EFFICIENCY_THREADS_A_PERFORMANCE_THREAD;
    NonZeroUsize::new(threads.saturating_sub(PERFORMANCE_THREADS_LEFT_BESIDE_WORKERS))
        .unwrap_or(NonZeroUsize::MIN)
        .min(less_one)
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
    /// budgets (cells, maps, systems, bodies, briefs, extinction maps, lines of sight, feature
    /// cells, feature interiors, class cells, alert scans, the sky's cells, the sky tables, the
    /// surface and the kept skies' files),
    /// the sky's switch, the kept skies' switch, the feature members' gate, the stop on standard
    /// input, the class of a sky's later steps and the held kept skies' switch.
    type Fields<'a> = (
        SocketAddr,
        &'a Path,
        usize,
        usize,
        [usize; 15],
        SkyService,
        KeptSkies,
        FeatureMembers,
        StdinStop,
        SkyQos,
        HeldSkies,
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
                config.feature_cache_bytes(),
                config.cluster_model_cache_bytes(),
                config.class_cell_cache_bytes(),
                config.alert_cache_bytes(),
                config.sky_cache_bytes(),
                config.sky_tables_bytes(),
                config.surface_cache_bytes(),
                config.kept_skies_disk_bytes(),
            ],
            config.sky_service(),
            config.kept_skies(),
            config.feature_members(),
            config.stdin_stop(),
            config.sky_qos(),
            config.held_skies(),
        )
    }

    #[test]
    fn the_command_line_is_well_formed() {
        ServerArgs::command().debug_assert();
    }

    /// The default pool is the processors less one off Apple silicon, and on it P + ⌊E ÷ 3⌋ − 2,
    /// at least one and never more than the processors less one.
    #[test]
    fn the_default_workers_leave_the_performance_cores_beside_the_height_workers() {
        let cores = |n: usize| NonZeroUsize::new(n);
        let levels = |performance, efficiency| {
            Some(CoreLevels {
                performance,
                efficiency,
            })
        };
        let counts = |available: Option<NonZeroUsize>, levels: Option<CoreLevels>| {
            workers_for(available, levels).get()
        };
        assert_eq!(
            [cores(16), cores(10), cores(2), cores(1), None].map(|each| counts(each, None)),
            [15, 9, 1, 1, 1]
        );
        // An M5's four and six, an M1's four and four, an M1 Pro's eight and two, an M4 Max's
        // twelve and four, and a chip of two and two.
        assert_eq!(
            [
                (10, levels(4, 6)),
                (8, levels(4, 4)),
                (10, levels(8, 2)),
                (16, levels(12, 4)),
                (4, levels(2, 2)),
            ]
            .map(|(available, levels)| counts(cores(available), levels)),
            [4, 3, 6, 11, 1]
        );
        // Never past the processors the OS lets the server have, less one.
        assert_eq!(counts(cores(3), levels(4, 6)), 2);
        assert_eq!(counts(None, levels(4, 6)), 1);
    }

    #[test]
    fn defaults_match_the_builder() {
        let config = config(&[]);
        assert_eq!(fields(&config), fields(&ServerConfig::builder().build()));
        let workers = workers_for(
            std::thread::available_parallelism().ok(),
            CoreLevels::of_this_machine(),
        )
        .get();
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
                    16 * 1024 * 1024,
                    64 * 1024 * 1024,
                    32 * 1024 * 1024,
                    32 * 1024 * 1024,
                    0,
                    160 * 1024 * 1024,
                    128 * 1024 * 1024,
                    256 * 1024 * 1024,
                ],
                SkyService::Served,
                KeptSkies::Kept,
                FeatureMembers::Served,
                StdinStop::Ignore,
                if cfg!(target_os = "macos") {
                    SkyQos::Background
                } else {
                    SkyQos::Utility
                },
                HeldSkies::Held,
            )
        );
        // No option sets the twins' priority: the server's are lowered, whoever starts it.
        assert_eq!(config.twin_priority(), TwinPriority::Lowered);
        assert_eq!(
            ServerConfig::builder().build().twin_priority(),
            TwinPriority::Lowered
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
            "--feature-cache",
            "31",
            "--cluster-model-cache",
            "37",
            "--class-cell-cache",
            "43",
            "--alert-cache",
            "41",
            "--sky-cache",
            "17",
            "--sky-tables",
            "19",
            "--surface-cache",
            "23",
            "--surface-passes",
            "29",
            "--serve-sky=false",
            "--keep-skies",
            "--kept-skies-disk",
            "47",
            "--hold-kept-skies=off",
            "--sky-qos",
            "background",
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
                    31 << 20,
                    37 << 20,
                    43 << 20,
                    41 << 20,
                    17 << 20,
                    19 << 20,
                    23 << 20,
                    47 << 20,
                ],
                SkyService::Unsupported,
                KeptSkies::Kept,
                FeatureMembers::Served,
                StdinStop::Watch,
                SkyQos::Background,
                HeldSkies::Exact,
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

    /// The kept skies' switch is on unless it is turned off, as the sky's own is (R06.T11.l), and
    /// off it keeps no file either, whatever their budget (R06.T11.n).
    #[test]
    fn the_skies_are_kept_unless_their_switch_is_off() {
        assert_eq!(
            config(&["--keep-skies=no", "--kept-skies-disk", "47"]).kept_skies_disk_bytes(),
            0
        );
        assert_eq!(
            config(&["--kept-skies-disk", "0"]).kept_skies(),
            KeptSkies::Kept
        );
        assert_eq!(config(&[]).kept_skies(), KeptSkies::Kept);
        assert_eq!(config(&["--keep-skies"]).kept_skies(), KeptSkies::Kept);
        assert_eq!(config(&["--keep-skies=on"]).kept_skies(), KeptSkies::Kept);
        for off in ["false", "no", "off", "0"] {
            let flag = format!("--keep-skies={off}");
            assert_eq!(config(&[&flag]).kept_skies(), KeptSkies::Off, "{flag}");
        }
        assert_eq!(refusal(&["--keep-skies=maybe"]), ErrorKind::ValueValidation);
        assert_eq!(
            refusal(&["--keep-skies", "false"]),
            ErrorKind::UnknownArgument
        );
    }

    /// The held kept skies' switch is on unless it is turned off, as the kept skies' own is, and
    /// off where they are (R06.T11.p).
    #[test]
    fn kept_skies_hold_unless_their_switch_is_off() {
        assert_eq!(config(&[]).held_skies(), HeldSkies::Held);
        assert_eq!(
            ServerConfig::builder().build().held_skies(),
            HeldSkies::Held
        );
        assert_eq!(config(&["--hold-kept-skies"]).held_skies(), HeldSkies::Held);
        for off in ["false", "no", "off", "0"] {
            let flag = format!("--hold-kept-skies={off}");
            assert_eq!(config(&[&flag]).held_skies(), HeldSkies::Exact, "{flag}");
        }
        assert_eq!(
            config(&["--keep-skies=false", "--hold-kept-skies"]).held_skies(),
            HeldSkies::Exact
        );
        assert_eq!(
            refusal(&["--hold-kept-skies=maybe"]),
            ErrorKind::ValueValidation
        );
        assert_eq!(
            refusal(&["--hold-kept-skies", "false"]),
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
                (Some("feature-cache"), Some("HYPERION_FEATURE_CACHE_MB")),
                (
                    Some("cluster-model-cache"),
                    Some("HYPERION_CLUSTER_MODEL_CACHE_MB")
                ),
                (
                    Some("class-cell-cache"),
                    Some("HYPERION_CLASS_CELL_CACHE_MB")
                ),
                (Some("alert-cache"), Some("HYPERION_ALERT_CACHE_MB")),
                (Some("sky-cache"), Some("HYPERION_SKY_CACHE_MB")),
                (Some("sky-tables"), Some("HYPERION_SKY_TABLES_MB")),
                (Some("surface-cache"), Some("HYPERION_SURFACE_CACHE_MB")),
                (Some("surface-passes"), Some("HYPERION_SURFACE_PASSES")),
                (Some("serve-sky"), Some("HYPERION_SERVE_SKY")),
                (Some("keep-skies"), Some("HYPERION_KEEP_SKIES")),
                (Some("kept-skies-disk"), Some("HYPERION_KEPT_SKIES_DISK_MB")),
                (Some("hold-kept-skies"), Some("HYPERION_HOLD_KEPT_SKIES")),
                (Some("sky-qos"), Some("HYPERION_SKY_QOS")),
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
        assert_eq!(DEFAULT_FEATURE_CACHE.to_string(), "16");
        assert_eq!(DEFAULT_CLUSTER_MODEL_CACHE.to_string(), "64");
        assert_eq!(DEFAULT_CLASS_CELL_CACHE.to_string(), "32");
        assert_eq!(DEFAULT_ALERT_CACHE.to_string(), "32");
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
            .feature_cache_bytes(110)
            .cluster_model_cache_bytes(120)
            .class_cell_cache_bytes(140)
            .alert_cache_bytes(130)
            .sky_cache_bytes(80)
            .sky_tables_bytes(90)
            .surface_cache_bytes(100)
            .surface_passes(NonZeroUsize::new(3).unwrap())
            .sky_service(SkyService::Unsupported)
            .kept_skies(KeptSkies::Off)
            .kept_skies_disk_bytes(150)
            .held_skies(HeldSkies::Held)
            .sky_qos(SkyQos::Utility)
            .feature_members(FeatureMembers::Held)
            .stdin_stop(StdinStop::Watch)
            .build();
        assert_eq!(
            fields(&config),
            (
                SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                Path::new("/tmp/x"),
                2,
                3,
                // The kept skies' files' 150 bytes are none, the kept skies being off.
                [
                    10, 20, 30, 40, 50, 60, 70, 110, 120, 140, 130, 80, 90, 100, 0
                ],
                SkyService::Unsupported,
                KeptSkies::Off,
                FeatureMembers::Held,
                StdinStop::Watch,
                SkyQos::Utility,
                // Held asked, but none, the kept skies being off.
                HeldSkies::Exact,
            )
        );
    }

    /// The class of a sky's later steps is one of its two names, background on macOS and utility
    /// elsewhere unless given (R06.T11.o).
    #[test]
    fn the_sky_qos_is_one_of_two_classes_background_by_default_on_macos() {
        assert_eq!(config(&["--sky-qos", "utility"]).sky_qos(), SkyQos::Utility);
        assert_eq!(
            config(&["--sky-qos=background"]).sky_qos(),
            SkyQos::Background
        );
        let expected = if cfg!(target_os = "macos") {
            SkyQos::Background
        } else {
            SkyQos::Utility
        };
        assert_eq!(config(&[]).sky_qos(), expected);
        for refused in ["--sky-qos=default", "--sky-qos=", "--sky-qos=Background"] {
            assert_eq!(refusal(&[refused]), ErrorKind::InvalidValue, "{refused}");
        }
    }
}
