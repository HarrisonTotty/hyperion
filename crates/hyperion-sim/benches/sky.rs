//! Benchmarks of the sky (rendering plan R06): the luminosity tables' build, the census near the
//! Sun with a cold and a warm cell cache, and the census in the nuclear disc (R06.T8), and later
//! the band.
//!
//! They run on `GalaxyParams::milky_way_like()` with a fixed seed. A miss is a finding to record,
//! not a CI failure: CI compiles these and never runs them. Every figure below is provisional
//! until taken on a quiet machine (the roadmap's "Measurements on a quiet machine").
//!
//! The censuses run much as the server will run them (R06.T11): the plan on one thread (its caps,
//! `layer_caps`, 768 realised profiles, which the server will split into pool jobs, so the printed
//! wall time overstates its), then the cells in chunks on a pool of the machine's threads less
//! one, each job with its own noise cache, then `merge_census`. Their time is the **CPU time** a
//! census costs, the brainstorm's measure: the plan's time plus every job's, summed over the
//! threads (wall time per job, which on a quiet machine is its CPU time). Each prints its tallies
//! once, on its first census. The cut is the eye's near the Sun, V 7.95 with the eye asked (Crumey
//! 2014's 7.4 at the Sun's darkest band texel, plus 0.45 for the colour offset and 0.1 of pad:
//! R06.T7 and `decision-r06-t7-caps.md`), until R06.T9's `eye_cut` computes it. The nuclear disc
//! takes it as a stand-in: its own eye cut will be shallower, so its figure is an upper bound.
//!
//! The cell cache is the benches' own, built on `serve_from_entry` as the server's will be
//! (R06.T11.b): least recently used out, bounded by `HYPERION_SKY_CACHE_MB` (default 64 MiB), each
//! entry weighing its records plus its own size. The cold census starts each iteration
//! with an empty cache; the warm one with the cache one census of the same query left.
//!
//! | Bench | Target | Figure |
//! | ----- | ------ | ------ |
//! | `sky/luminosity_tables` | ≤ 30 CPU-s (T17) | pending a quiet machine |
//! | `sky/census_near_sun/cold` | 5–10 CPU-s, 6 × 10⁷ candidates (brainstorm) | ≥ 14,000 CPU-s |
//! | `sky/census_near_sun/warm` | none | not yet run |
//! | `sky/census_nuclear_disc` | none like for like (below) | not yet run |
//!
//! The cold near-Sun figure is one run stopped unfinished after 20 minutes (2026-10-05, 15
//! workers, RSS 1.6 GB, on a machine shared with other lanes): 14,750 CPU-s of the process, of
//! which the tables' build is some 300. It misses the target by over a thousand times; R06's
//! Risks give the likely causes. The warm and nuclear-disc benches are left for R06.T17 or the
//! owner, on a quiet machine.
//!
//! The brainstorm's 400–800 CPU-s and 5 × 10⁹ candidates are the inner bulge's under the near-Sun
//! caps held fixed; the nuclear disc's bench takes its own caps, which are far smaller.

use std::cell::OnceCell;
use std::collections::{BTreeMap, HashMap};
use std::env::VarError;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, cell_heap_bytes};
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::caps::{CAPPED_LAYERS, LayerCap};
use hyperion_sim::sky::census::{
    CensusTallies, NoSkyCellCache, Served, SkyCellCache, SkyCensus, SkyContext, SkyQuery,
    census_cell, census_plan, merge_census, serve_from_entry,
};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::luminosity::{BinSums, LuminosityTables};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, Magnitudes, SolarMasses};

/// The fixture's seed, the sim's sky tests' own.
const SEED: u64 = 0x0926_0000;

/// The eye's cut near the Sun, V: 7.4 + 0.45 + 0.1 (R06.T7, `decision-r06-t7-caps.md`), the cut
/// the caps' decision and the census's slow tests use, until R06.T9's `eye_cut` computes it.
const EYE_CUT_V: f64 = 7.95;

/// Near the Sun, ly: the plan's Sun-like test place, 26,000 ly from the centre on +y (the
/// fixture's solar circle is 3.8 disc lengths, about 26,600 ly; R₀ = 8.178 kpc, GRAVITY
/// Collaboration 2019, A&A 625, L10) and 68 ly above the plane (z☉ = 20.8 ± 0.3 pc, Bennett and
/// Bovy 2019, MNRAS 482, 1417).
const SUN_LY: [f64; 3] = [0.0, 26_000.0, 68.0];

/// In the nuclear disc, ly: 150 ly from Sgr A* in the plane.
const NUCLEAR_DISC_LY: [f64; 3] = [0.0, 150.0, 0.0];

/// Cells a pool job takes at a time.
const CHUNK_CELLS: usize = 256;

/// Noise-cache slots per job: 1 MiB of the sightlines' lattice words.
const NOISE_SLOTS: usize = 1 << 16;

/// The server's default cell-cache budget, MiB (R06.T11.b), when `HYPERION_SKY_CACHE_MB` is unset.
const DEFAULT_CACHE_MB: usize = 64;

/// Whether each census bench has printed its first census.
static PRINTED_COLD: AtomicBool = AtomicBool::new(false);
static PRINTED_FILL: AtomicBool = AtomicBool::new(false);
static PRINTED_WARM: AtomicBool = AtomicBool::new(false);
static PRINTED_NUCLEAR: AtomicBool = AtomicBool::new(false);

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid")
}

/// The pool's width: the machine's threads less one, as the server leaves one to its runtime.
fn workers() -> usize {
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .saturating_sub(1)
        .max(1)
}

/// Runs `run` over `jobs` on `workers` threads, each taking the next job, and returns the
/// results with the summed time of the jobs.
fn on_pool<J: Sync, R: Send>(
    jobs: &[J],
    workers: usize,
    run: &(impl Fn(&J) -> R + Sync),
) -> (Vec<(usize, R)>, Duration) {
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut mine = Vec::new();
        let mut busy = Duration::ZERO;
        loop {
            let k = next.fetch_add(1, Ordering::Relaxed);
            let Some(job) = jobs.get(k) else {
                return (mine, busy);
            };
            let start = Instant::now();
            mine.push((k, run(job)));
            busy += start.elapsed();
        }
    };
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers).map(|_| scope.spawn(worker)).collect();
        let mut all = Vec::with_capacity(jobs.len());
        let mut busy = Duration::ZERO;
        for h in handles {
            let (mine, b) = h.join().expect("a pool job does not panic");
            all.extend(mine);
            busy += b;
        }
        all.sort_unstable_by_key(|(k, _)| *k);
        (all, busy)
    })
}

/// The fixture's galaxy, its full luminosity tables (built once, on the pool) and the envelope.
struct Sky {
    galaxy: Galaxy,
    tables: LuminosityTables,
    envelope: BrightnessEnvelope,
}

fn sky() -> &'static Sky {
    static SKY: OnceLock<Sky> = OnceLock::new();
    SKY.get_or_init(|| {
        let galaxy = galaxy();
        let plan = LuminosityTables::plan(&galaxy);
        let workers = workers();
        // Stage by stage, one metallicity's samples held at a time; each bin's sums behind a lock
        // of its own, since a stage's accumulation jobs are of different bins.
        let sums: Vec<Mutex<BinSums>> = plan.bin_sums().into_iter().map(Mutex::new).collect();
        for stage in plan.stages() {
            let sample_jobs: Vec<_> = plan.sample_jobs(stage).collect();
            let (chunks, _) = on_pool(&sample_jobs, workers, &|job| plan.run_samples(job.clone()));
            let samples = plan.track_samples(chunks.into_iter().map(|(_, c)| c));
            let accumulate_jobs: Vec<_> = plan.accumulate_jobs(stage).collect();
            on_pool(&accumulate_jobs, workers, &|job| {
                let mut bin = sums[job.bin()]
                    .lock()
                    .expect("an accumulation job does not panic");
                plan.run_accumulate(&samples, *job, &mut bin);
            });
        }
        let tables = plan.assemble(sums.into_iter().map(|bin| {
            bin.into_inner()
                .expect("an accumulation job does not panic")
        }));
        let envelope = BrightnessEnvelope::build(&galaxy);
        Sky {
            galaxy,
            tables,
            envelope,
        }
    })
}

fn eye_query(ly: [f64; 3]) -> SkyQuery {
    let at = GalacticPosition::from_light_years(ly).expect("in the root cube");
    let observer = Observer::new(at, UniverseTime::EPOCH).expect("the epoch is on the clock");
    SkyQuery::builder(observer, Magnitudes::new(EYE_CUT_V))
        .eye(EyeObserver::default())
        .build()
        .expect("a valid query")
}

/// One census's result and costs.
struct Run {
    census: SkyCensus,
    /// Each capped layer's cap, ly.
    caps: Vec<LightYears>,
    cells: usize,
    plan: Duration,
    jobs: Duration,
    wall: Duration,
}

impl Run {
    /// The census's CPU time: the plan's and every job's.
    fn cpu(&self) -> Duration {
        self.plan + self.jobs
    }
}

/// The census of `query` through `cells`, as the server will run it.
fn census(sky: &Sky, query: &SkyQuery, cells: &dyn SkyCellCache, workers: usize) -> Run {
    let began = Instant::now();
    let mut caps_noise = NoiseCache::with_capacity(NOISE_SLOTS);
    let plan = census_plan(
        &sky.galaxy,
        &sky.tables,
        &sky.envelope,
        query,
        &mut caps_noise,
    );
    let planned = began.elapsed();
    let chunks: Vec<&[CellKey]> = plan.cells().chunks(CHUNK_CELLS).collect();
    let (parts, jobs) = on_pool(&chunks, workers, &|keys: &&[CellKey]| {
        let mut ctx = SkyContext {
            tables: &sky.tables,
            envelope: &sky.envelope,
            noise: NoiseCache::with_capacity(NOISE_SLOTS),
            cells,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let mut stars = Vec::new();
        let mut tallies = CensusTallies::default();
        for &key in *keys {
            tallies.add(&census_cell(&sky.galaxy, &mut ctx, key, query, &mut stars));
        }
        (stars, tallies)
    });
    let census = merge_census(parts.into_iter().map(|(_, p)| p), query.n_max());
    Run {
        census,
        caps: plan.caps().iter().map(LayerCap::radius).collect(),
        cells: plan.cells().len(),
        plan: planned,
        jobs,
        wall: began.elapsed(),
    }
}

/// Prints `run`'s tallies and costs the first time `printed` is unset.
fn report_once(printed: &AtomicBool, name: &str, run: &Run, cache: Option<&BenchCellCache<'_>>) {
    if printed.swap(true, Ordering::Relaxed) {
        return;
    }
    let t = run.census.tallies();
    eprintln!(
        "{name}: {} cells, plan {:.1} s, jobs {:.1} s, CPU {:.1} s, wall {:.1} s on {} workers; \
         {} listed, {} overflow",
        run.cells,
        run.plan.as_secs_f64(),
        run.jobs.as_secs_f64(),
        run.cpu().as_secs_f64(),
        run.wall.as_secs_f64(),
        workers(),
        run.census.listed().len(),
        run.census.overflow().len(),
    );
    for (cap, &layer) in run.caps.iter().zip(&CAPPED_LAYERS) {
        let l = t.layer(layer);
        eprintln!(
            "  {layer:?}: cap {:.0} ly; {} cells, {} candidates, {} generated, {} accepted, \
             {} listed",
            cap.value(),
            l.cells(),
            l.candidates(),
            l.generated(),
            l.accepted(),
            l.listed(),
        );
    }
    if let Some(cache) = cache {
        let kept = cache.lock();
        eprintln!(
            "  cache: {} served, {} missed, {} rebuilt, {} evicted; {} entries, {} of {} bytes",
            kept.served,
            kept.missed,
            kept.rebuilt,
            kept.evicted,
            kept.entries.len(),
            kept.bytes,
            cache.max_bytes,
        );
    }
}

/// One cell's entry in [`BenchCellCache`]: its records shared, so that a lookup filters them
/// outside the lock.
#[derive(Debug)]
struct Entry {
    held: SolarMasses,
    records: Arc<[SystemRecord]>,
    bytes: usize,
    used: u64,
}

/// What an entry weighs beyond its records: its key twice (in both maps), itself, its place in the
/// recency order and its records' reference counts.
const ENTRY_OVERHEAD_BYTES: usize =
    2 * size_of::<CellKey>() + size_of::<Entry>() + 3 * size_of::<u64>();

#[derive(Debug, Default)]
struct Kept {
    entries: HashMap<CellKey, Entry>,
    by_use: BTreeMap<u64, CellKey>,
    bytes: usize,
    clock: u64,
    served: u64,
    missed: u64,
    rebuilt: u64,
    evicted: u64,
}

/// The benches' cell cache, built on `serve_from_entry` as the server's will be: one galaxy's
/// entries behind a lock, the least recently used out first, at most `max_bytes` bytes held, an
/// entry weighing its records plus [`ENTRY_OVERHEAD_BYTES`] (the maps' nodes are not counted). The
/// lock is held only to find, touch or store an entry: an entry is filtered, and a cell built,
/// outside it, and a built cell is kept unless its entry already serves its floor.
#[derive(Debug)]
struct BenchCellCache<'g> {
    galaxy: &'g Galaxy,
    max_bytes: usize,
    kept: Mutex<Kept>,
}

impl<'g> BenchCellCache<'g> {
    /// An empty cache for `galaxy` under `HYPERION_SKY_CACHE_MB`.
    fn new(galaxy: &'g Galaxy) -> Self {
        let mb = match std::env::var("HYPERION_SKY_CACHE_MB") {
            Err(VarError::NotPresent) => DEFAULT_CACHE_MB,
            Err(VarError::NotUnicode(_)) => panic!("HYPERION_SKY_CACHE_MB is not Unicode"),
            Ok(v) => v.parse::<usize>().unwrap_or_else(|e| {
                panic!("HYPERION_SKY_CACHE_MB={v} is not a whole number of MiB: {e}")
            }),
        };
        Self {
            galaxy,
            max_bytes: mb.saturating_mul(1 << 20),
            kept: Mutex::new(Kept::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().expect("a cache user does not panic")
    }

    fn keep(&self, key: CellKey, held: SolarMasses, records: &[SystemRecord]) {
        if held.value().is_nan() {
            return;
        }
        let bytes = cell_heap_bytes(records).saturating_add(ENTRY_OVERHEAD_BYTES);
        if bytes > self.max_bytes {
            return;
        }
        let mut guard = self.lock();
        let kept = &mut *guard;
        if let Some(old) = kept.entries.get(&key)
            && old.held.value() <= held.value()
        {
            return;
        }
        kept.clock += 1;
        let used = kept.clock;
        if let Some(old) = kept.entries.insert(
            key,
            Entry {
                held,
                records: Arc::from(records),
                bytes,
                used,
            },
        ) {
            kept.bytes -= old.bytes;
            kept.by_use.remove(&old.used);
        }
        kept.by_use.insert(used, key);
        kept.bytes += bytes;
        while kept.bytes > self.max_bytes {
            let (_, oldest) = kept
                .by_use
                .pop_first()
                .expect("over the bound, so not empty");
            let gone = kept.entries.remove(&oldest).expect("in both maps");
            kept.bytes -= gone.bytes;
            kept.evicted += 1;
        }
    }
}

impl SkyCellCache for BenchCellCache<'_> {
    fn bright_subset(
        &self,
        galaxy: &Galaxy,
        key: CellKey,
        floor: SolarMasses,
        out: &mut Vec<SystemRecord>,
    ) {
        assert!(
            std::ptr::eq(galaxy, self.galaxy),
            "a cell cache is for the galaxy it was made for"
        );
        let found = {
            let mut guard = self.lock();
            let kept = &mut *guard;
            kept.clock += 1;
            let clock = kept.clock;
            match kept.entries.get_mut(&key) {
                None => {
                    kept.missed += 1;
                    None
                }
                Some(entry) => {
                    kept.by_use.remove(&entry.used);
                    entry.used = clock;
                    kept.by_use.insert(clock, key);
                    Some((entry.held, Arc::clone(&entry.records)))
                }
            }
        };
        if let Some((held, records)) = found {
            match serve_from_entry(held, &records, floor, out) {
                Served::Served => {
                    self.lock().served += 1;
                    return;
                }
                Served::Rebuild => self.lock().rebuilt += 1,
            }
        }
        NoSkyCellCache.bright_subset(galaxy, key, floor, out);
        self.keep(key, floor, out);
    }
}

fn census_near_sun(c: &mut Criterion) {
    // The tables and the warm cache are made only when a bench that reads them is run, so that a
    // filtered run builds nothing it does not time.
    let query = eye_query(SUN_LY);
    let workers = workers();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("census_near_sun/cold", |b| {
        let sky = sky();
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let cache = BenchCellCache::new(&sky.galaxy);
                let run = census(sky, black_box(&query), &cache, workers);
                report_once(&PRINTED_COLD, "census_near_sun/cold", &run, Some(&cache));
                cpu += run.cpu();
            }
            cpu
        });
    });
    let warm: OnceCell<BenchCellCache<'static>> = OnceCell::new();
    group.bench_function("census_near_sun/warm", |b| {
        let sky = sky();
        // The warm cache is what one cold census of the same query leaves.
        let warm = warm.get_or_init(|| {
            let cache = BenchCellCache::new(&sky.galaxy);
            let fill = census(sky, &query, &cache, workers);
            report_once(&PRINTED_FILL, "census_near_sun/fill", &fill, Some(&cache));
            cache
        });
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let run = census(sky, black_box(&query), warm, workers);
                report_once(&PRINTED_WARM, "census_near_sun/warm", &run, Some(warm));
                cpu += run.cpu();
            }
            cpu
        });
    });
    group.finish();
}

fn census_nuclear_disc(c: &mut Criterion) {
    let query = eye_query(NUCLEAR_DISC_LY);
    let workers = workers();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("census_nuclear_disc", |b| {
        let sky = sky();
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let run = census(sky, black_box(&query), &NoSkyCellCache, workers);
                report_once(&PRINTED_NUCLEAR, "census_nuclear_disc", &run, None);
                cpu += run.cpu();
            }
            cpu
        });
    });
    group.finish();
}

fn luminosity_tables(c: &mut Criterion) {
    let galaxy = galaxy();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("luminosity_tables", |b| {
        b.iter(|| LuminosityTables::build(black_box(&galaxy)));
    });
    group.finish();
}

criterion_group!(
    sky_benches,
    luminosity_tables,
    census_near_sun,
    census_nuclear_disc
);
criterion_main!(sky_benches);
