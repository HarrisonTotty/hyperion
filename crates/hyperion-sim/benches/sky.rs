//! Benchmarks of the sky (rendering plan R06): the luminosity tables' build, the census near the
//! Sun with a cold and a warm cell cache, and the census in the nuclear disc (R06.T8), the band
//! near the Sun, marched and summed (R06.T9.b, T9.f) and under a camera's cut (R06.T9.j), and the
//! limit map (R06.T9.i).
//!
//! They run on `GalaxyParams::milky_way_like()` with a fixed seed. A miss is a finding to record,
//! not a CI failure: CI compiles these and never runs them. Every figure below is provisional
//! until taken on a quiet machine (the roadmap's "Measurements on a quiet machine").
//!
//! The censuses run much as the server will run them (R06.T11): the plan on one thread (its caps,
//! `layer_caps`, 768 realised profiles, which the server will split into pool jobs, so the printed
//! wall time overstates its), then the plan's slabs (`CensusPlan::slabs`, an x slab of one layer's
//! walk each, its cells streamed, R06.T8.f) as jobs on a pool of the machine's threads less one,
//! each job with its own noise cache, then `merge_census`. Their time is the **CPU time** a
//! census costs, the brainstorm's measure: the plan's time plus every job's, summed over the
//! threads (wall time per job, which on a quiet machine is its CPU time). Each prints its tallies
//! once, on its first census, with each layer's records past the floor, the systems generated and
//! their share. The cut is the eye's near the Sun, V 7.95 with the eye asked (Crumey
//! 2014's 7.4 at the Sun's darkest band texel, plus 0.45 for the colour offset and 0.1 of pad:
//! R06.T7 and `decision-r06-t7-caps.md`), until R06.T9's `eye_cut` computes it. The nuclear disc
//! takes it as a stand-in: its own eye cut will be shallower, so its figure is an upper bound.
//!
//! **Sampled** (R06.T8.f): with `HYPERION_SKY_BENCH_SAMPLE=k`, a census takes only the cells whose
//! key's hash ([`sample_hash`], a fixed mixer of the layer and the cell's coordinates) is 0 modulo
//! k, and prints its tallies and CPU time scaled by k, labelled an estimate: the plan's time and
//! the walk of every slab's cells unscaled, the sampled cells' census times k. The time each
//! iteration returns to criterion is that estimate. Unset, or 1, censuses every cell.
//!
//! The cell cache is the benches' own, built on `serve_from_entry` as the server's will be
//! (R06.T11.b): least recently used out, bounded by `HYPERION_SKY_CACHE_MB` (default 64 MiB), each
//! entry weighing its records plus its own size. The cold census starts each iteration
//! with an empty cache; the warm one with the cache one census of the same query left.
//!
//! | Bench | Target | Figure |
//! | ----- | ------ | ------ |
//! | `sky/luminosity_tables` | ≤ 30 CPU-s (T17) | pending a quiet machine |
//! | `sky/census_near_sun/cold` | ≤ 4,000 CPU-s (T17) | 1.8 × 10⁶ CPU-s, sampled |
//! | `sky/census_near_sun/warm` | ≤ 25% of cold (T17) | not yet run |
//! | `sky/census_nuclear_disc` | none like for like (below) | not yet run |
//! | `sky/band_near_sun/march` | within the first sky's (T17) | 22.7 CPU-s, provisional |
//! | `sky/band_near_sun/sum` | within each reply's (T17) | 0.002 CPU-s, provisional |
//! | `sky/band_near_sun/march_camera` | within the first sky's (T17) | 29.5 CPU-s, provisional |
//! | `sky/band_near_sun/march_camera_no_eye` | none (the camera's march's reference) | 22.5 CPU-s, provisional |
//! | `sky/limit_map/near_sun` | within the first sky's (T17) | 0.32 CPU-s, provisional |
//! | `sky/limit_map/synthetic_300k` | ≤ 3 CPU-s (R06.T9.i) | 1.03 CPU-s, provisional |
//!
//! The cold near-Sun figure is R06.T8.f's sampled run (2026-10-05, `HYPERION_SKY_BENCH_SAMPLE`
//! 1,000, criterion's `--test`, 15 workers, load about 15, so provisional): 1.83 × 10⁶ CPU-s
//! estimated, 125 s wall for the sample, over 1.08 × 10⁸ cells and some 4.0 × 10⁸ records past the
//! floor, of which C generates 98.3%, D 99.9% and E 99.99%. Nearly all of it is generating
//! systems. The census-cost ruling (`decision-r06-census-cost.md`) retires the brainstorm's 5–10
//! CPU-s and orders the levers, of which R06.T8.g's bound star by star is the one that moves it.
//! The warm and nuclear-disc benches are left for R06.T17 or the owner, on a quiet machine.
//!
//! The brainstorm's 400–800 CPU-s and 5 × 10⁹ candidates are the inner bulge's under the near-Sun
//! caps held fixed; the nuclear disc's bench takes its own caps, which are far smaller.
//!
//! The band benches (R06.T9.f) split the band of a near-Sun request at the eye's cut as the server
//! runs it (R06.T11.c, T11.d), one face row a job over all six faces of `BandSpec::STANDARD` (64²
//! texels a face, 24,576 rays). `band_near_sun/march` marches the rays once, keeping every reply of
//! R06.T8.i's shell plan (C, D and E complete to 500 ly, then 1,000 × 2^k ly, held at their caps,
//! then the caps; A, B and the brown dwarfs to their caps), and prints its heap.
//! `band_near_sun/sum` sums the final reply's texels from that march. Their census is empty, since
//! the overflow's points cost nothing beside the rays. Their time is the CPU time, summed over the
//! jobs, as the censuses'. R06.T9.f's one run of each (2026-10-07, criterion's `--test`, three
//! workers at `CPUQuota=400%`, without the heavy-test lock, load 13–17, so provisional): seven
//! replies and 22 slots a ray, a heap of 21.8 MB, 22.7 CPU-s for the march (7.6 s wall) against
//! 18.5 CPU-s for one of the caps alone, the band before the split, and 0.002 CPU-s for the sum.
//! R06.T9.b's one run of the band before the split (2026-10-05, under the heavy-test lock, load
//! 3–5): 20.2 CPU-s on 15 workers, 1.37 s wall.
//!
//! `band_near_sun/march_camera` marches a camera's request at V 10.06 with the eye at 7.95 beside
//! it, so the march keeps the light fainter than the eye's cut too (R06.T9.j), and
//! `band_near_sun/march_camera_no_eye` the same request with no eye; the second sums cost the
//! difference. R06.T9.j's one run of each (2026-10-07, criterion's `--test`, three workers at
//! `CPUQuota=400%`, without the heavy-test lock, which another lane held, load about 10, so
//! provisional): at 10.06 the caps are A 46, B 260, C 19,416, D 19,416, E 41,804 and the brown
//! dwarfs 1 ly, with eight replies; with the eye's light the march holds 49.3 MB and took 29.5
//! CPU-s (9.9 s wall), and without it 24.7 MB and 22.5 CPU-s (7.5 s wall). The second sums double
//! the heap and add 31% to the time. In the same run `band_near_sun/march` took 20.0 CPU-s. A 16² band took 1.7 s on one thread in the test
//! profile, about 1.1 ms a ray, four fifths of it the luminosity functions' reads and one fifth the
//! ray's profile.
//!
//! The limit map's benches (R06.T9.i) set the eye's limits of all six faces of `BandSpec::STANDARD`
//! against a glare, one face row a job, as the server will after each reply's band (R06.T11.c).
//! Their band is the near-Sun band at V 8.15 with no census, complete everywhere. `limit_map/
//! near_sun` takes the glare of the stars a census lists there within 200 ly brighter than V 8.15,
//! R06.T9.i's fixture. `limit_map/synthetic_300k` takes the glare ruling's synthetic sky of 300,000
//! stars to V 10.06 (`MAX_N_MAX`, a camera's cut with the eye open), the same stars as the sim's
//! test of the pyramid. Their time is the glare's build, on one thread, plus the jobs' CPU time.
//! The gate is provisional (`decision-r06-t9c-glare.md`, item 2): at most 3 CPU-s at 300,000 stars
//! on the dev machine. The exact sum, every star over every texel, would take about 140 CPU-s at
//! R06.T9.c's 19 ns a pair; R06.T9.i measured 161 CPU-s on one thread (21.9 ns a pair). R06.T9.i's
//! one run (2026-10-07, without the heavy-test lock, which another lane's long run held, so at
//! `CPUQuota=400%`, 3 workers, load 7.7–9.5: provisional): `near_sun` 0.32 CPU-s, 0.11 s wall
//! (criterion 289 ms an iteration); `synthetic_300k` 1.03 CPU-s, 0.38 s wall (criterion 1.03 s).
//! By the process's CPU clock on one thread, in the slow-test profile, 300,000 stars took 1.45
//! CPU-s: 1.38 for the map and 0.07 for the glare's build. A locked re-timing is pending.

use std::cell::OnceCell;
use std::collections::{BTreeMap, HashMap};
use std::env::VarError;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, criterion_group, criterion_main};
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, UnitVector};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, cell_heap_bytes};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::band::{
    BandMarch, BandSpec, BandTexel, CompleteTo, CubeFace, band_rows, march_rows, sum_rows,
};
use hyperion_sim::sky::caps::{CAPPED_LAYERS, LayerCap, layer_caps};
use hyperion_sim::sky::census::{
    CellOffsets, CellSlab, CensusTallies, MAX_N_MAX, NoSkyCellCache, Served, SkyCellCache,
    SkyCensus, SkyContext, SkyQuery, SkyStar, census_cell, census_plan, merge_census,
    serve_from_entry,
};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::eye::{SpRatio, illuminance_of_magnitude};
use hyperion_sim::sky::limits::{Glare, limit_rows};
use hyperion_sim::sky::luminosity::{BinSums, LuminosityTables};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::consts::RADIANS_PER_DEGREE;
use hyperion_sim::units::{LightYears, Lux, Magnitudes, SolarMasses};
use hyperion_testkit::golden::f64_digest;

/// The fixture's seed, the sim's sky tests' own.
const SEED: u64 = 0x0926_0000;

/// The eye's cut near the Sun, V: 7.4 + 0.45 + 0.1 (R06.T7, `decision-r06-t7-caps.md`), the cut
/// the caps' decision and the census's slow tests use, until R06.T9's `eye_cut` computes it.
const EYE_CUT_V: f64 = 7.95;

/// A camera's cut near the Sun beside the eye's, V (R06.T9.j): the glare ruling's 10.06, a camera
/// with the eye open, at which a census near the Sun lists `MAX_N_MAX` stars.
const CAMERA_CUT_V: f64 = 10.06;

/// Near the Sun, ly: the plan's Sun-like test place, 26,000 ly from the centre on +y (the
/// fixture's solar circle is 3.8 disc lengths, about 26,600 ly; R₀ = 8.178 kpc, GRAVITY
/// Collaboration 2019, A&A 625, L10) and 68 ly above the plane (z☉ = 20.8 ± 0.3 pc, Bennett and
/// Bovy 2019, MNRAS 482, 1417).
const SUN_LY: [f64; 3] = [0.0, 26_000.0, 68.0];

/// In the nuclear disc, ly: 150 ly from Sgr A* in the plane.
const NUCLEAR_DISC_LY: [f64; 3] = [0.0, 150.0, 0.0];

/// Noise-cache slots per job: 1 MiB of the sightlines' lattice words.
const NOISE_SLOTS: usize = 1 << 16;

/// The server's default cell-cache budget, MiB (R06.T11.b), when `HYPERION_SKY_CACHE_MB` is unset.
const DEFAULT_CACHE_MB: usize = 64;

/// Whether each census bench has printed its first census.
static PRINTED_COLD: AtomicBool = AtomicBool::new(false);
static PRINTED_FILL: AtomicBool = AtomicBool::new(false);
static PRINTED_WARM: AtomicBool = AtomicBool::new(false);
static PRINTED_NUCLEAR: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND_SUM: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND_CAMERA: AtomicBool = AtomicBool::new(false);
static PRINTED_BAND_CAMERA_NO_EYE: AtomicBool = AtomicBool::new(false);
static PRINTED_LIMIT_MAP_NEAR_SUN: AtomicBool = AtomicBool::new(false);
static PRINTED_LIMIT_MAP_SYNTHETIC: AtomicBool = AtomicBool::new(false);

/// The limit map's near-Sun fixture (R06.T9.i): the eye's cut there, V 8.15 (R06.T9.d's figure
/// for the real sky, at which T9.c's to T9.i's tests are ruled), and the census within 200 ly.
const LIMIT_MAP_CUT_V: f64 = 8.15;
const LIMIT_MAP_RADIUS_LY: f64 = 200.0;

/// The synthetic sky's depth, V: a camera's cut with the eye open, at which the census lists its
/// largest number of stars, `MAX_N_MAX` (the glare ruling's sky at `N_max`).
const SYNTHETIC_CUT_V: f64 = 10.06;

/// The synthetic sky's fingerprint at `MAX_N_MAX` stars to `SYNTHETIC_CUT_V`: `f64_digest` of each
/// star's direction, illuminance and ratio, in order. The sim's test of the pyramid asserts the
/// same (`sky::limits`' `SYNTHETIC_SKY_DIGEST`), so this copy of its sky cannot part from it.
const SYNTHETIC_SKY_DIGEST: u64 = 0x8b8b_938a_6811_c91f;

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

/// The fixture's galaxy, its full luminosity tables (built once, on the pool), the envelope and
/// the cells' offset bounds.
struct Sky {
    galaxy: Galaxy,
    tables: LuminosityTables,
    envelope: BrightnessEnvelope,
    offsets: CellOffsets,
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
        let offsets = CellOffsets::build(&galaxy);
        Sky {
            galaxy,
            tables,
            envelope,
            offsets,
        }
    })
}

/// The census benches' sample, `HYPERION_SKY_BENCH_SAMPLE` (1, every cell, when unset).
fn sample() -> u64 {
    match std::env::var("HYPERION_SKY_BENCH_SAMPLE") {
        Err(VarError::NotPresent) => 1,
        Err(VarError::NotUnicode(_)) => panic!("HYPERION_SKY_BENCH_SAMPLE is not Unicode"),
        Ok(v) => v
            .parse::<u64>()
            .ok()
            .filter(|&k| k > 0)
            .unwrap_or_else(|| panic!("HYPERION_SKY_BENCH_SAMPLE={v} is not a positive count")),
    }
}

/// One `SplitMix64` step from the state `word`: the golden gamma, then Stafford's Mix13 (Stafford
/// 2011, "Better bit mixing"; Steele, Lea and Flood 2014's `mix64variant13`, doi:10.1145/2660193.
/// 2660195, and JDK 8's `SplittableRandom.mix64`), as Vigna's 2015 `splitmix64.c` gives it: a fixed
/// mixer of a word.
fn mix(word: u64) -> u64 {
    let mut z = word.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The hash a sampled bench takes `key` by: [`mix`] of its layer, then of each of its origin's
/// coordinates in turn, so that a sample is spread over the sky and the same on every run.
fn sample_hash(key: CellKey) -> u64 {
    key.origin_ly()
        .iter()
        .fold(mix(u64::from(key.layer().value())), |h, &c| {
            mix(h ^ u64::from(c.cast_unsigned()))
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
    /// The plan's cells, every one, sampled or not.
    cells: u64,
    /// The sample: one cell in this many is censused.
    sample: u64,
    plan: Duration,
    /// The jobs' time walking their slabs' cells, the sampled cells' census aside.
    walk: Duration,
    /// The jobs' time censusing the sampled cells.
    cell_census: Duration,
    wall: Duration,
}

impl Run {
    /// The census's CPU time: the plan's and every job's, with the sampled cells' census scaled
    /// by the sample, so an estimate when the census is sampled.
    fn cpu(&self) -> Duration {
        let scale = u32::try_from(self.sample).expect("a sample of under 2³² cells");
        self.plan + self.walk + self.cell_census * scale
    }
}

/// The census of `query` through `cells`, as the server will run it, taking one cell in `sample`.
fn census(
    sky: &Sky,
    query: &SkyQuery,
    cells: &dyn SkyCellCache,
    workers: usize,
    sample: u64,
) -> Run {
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
    let slabs: Vec<CellSlab> = plan.slabs().collect();
    let (parts, jobs) = on_pool(&slabs, workers, &|slab: &CellSlab| {
        let mut ctx = SkyContext {
            tables: &sky.tables,
            envelope: &sky.envelope,
            offsets: &sky.offsets,
            noise: NoiseCache::with_capacity(NOISE_SLOTS),
            cells,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let mut stars = Vec::new();
        let mut tallies = CensusTallies::default();
        let mut censusing = Duration::ZERO;
        for key in slab.cells() {
            if sample > 1 && !sample_hash(key).is_multiple_of(sample) {
                continue;
            }
            let opened = Instant::now();
            tallies.add(&census_cell(&sky.galaxy, &mut ctx, key, query, &mut stars));
            censusing += opened.elapsed();
        }
        ((stars, tallies), censusing)
    });
    let cell_census: Duration = parts.iter().map(|(_, (_, c))| *c).sum();
    let census = merge_census(parts.into_iter().map(|(_, (p, _))| p), query.n_max());
    Run {
        census,
        caps: plan.caps().iter().map(LayerCap::radius).collect(),
        cells: plan.cell_count(),
        sample,
        plan: planned,
        walk: jobs.saturating_sub(cell_census),
        cell_census,
        wall: began.elapsed(),
    }
}

/// Prints `run`'s tallies and costs the first time `printed` is unset.
fn report_once(printed: &AtomicBool, name: &str, run: &Run, cache: Option<&BenchCellCache<'_>>) {
    if printed.swap(true, Ordering::Relaxed) {
        return;
    }
    let t = run.census.tallies();
    let k = run.sample;
    // A sampled census's counts and time, scaled by the sample: estimates, and labelled so.
    let (what, scale) = if k > 1 {
        (format!("ESTIMATE from 1 cell in {k}, scaled by {k}"), k)
    } else {
        ("every cell".to_owned(), 1)
    };
    eprintln!(
        "{name} ({what}): {} cells in the plan, plan {:.1} s, walk {:.1} s, cell census {:.1} s \
         sampled, CPU {:.1} s, wall {:.1} s on {} workers; {} listed, {} overflow (sampled)",
        run.cells,
        run.plan.as_secs_f64(),
        run.walk.as_secs_f64(),
        run.cell_census.as_secs_f64(),
        run.cpu().as_secs_f64(),
        run.wall.as_secs_f64(),
        workers(),
        run.census.listed().len(),
        run.census.overflow().len(),
    );
    for (cap, &layer) in run.caps.iter().zip(&CAPPED_LAYERS) {
        let l = t.layer(layer);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a share of counts under 2⁵³, for printing"
        )]
        let share = if l.candidates() == 0 {
            0.0
        } else {
            100.0 * l.generated() as f64 / l.candidates() as f64
        };
        eprintln!(
            "  {layer:?}: cap {:.0} ly; {} cells, {} records past the floor, {} generated \
             ({share:.2}% of the records), {} accepted, {} listed (sampled)",
            cap.value(),
            l.cells() * scale,
            l.candidates() * scale,
            l.generated() * scale,
            l.accepted() * scale,
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
    let sample = sample();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("census_near_sun/cold", |b| {
        let sky = sky();
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let cache = BenchCellCache::new(&sky.galaxy);
                let run = census(sky, black_box(&query), &cache, workers, sample);
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
            let fill = census(sky, &query, &cache, workers, sample);
            report_once(&PRINTED_FILL, "census_near_sun/fill", &fill, Some(&cache));
            cache
        });
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let run = census(sky, black_box(&query), warm, workers, sample);
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
    let sample = sample();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("census_nuclear_disc", |b| {
        let sky = sky();
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let run = census(sky, black_box(&query), &NoSkyCellCache, workers, sample);
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

/// The first shell's edge of R06.T8.i's shell plan, ly: the edges, per layer, are 500 ly, then
/// 1,000 × 2^k ly up to the cap.
const FIRST_SHELL_LY: f64 = 500.0;

/// The replies of a near-Sun request, nearest first, as R06.T8.i's shell plan will state them
/// (`decision-r06-census-cost.md`; not yet built, so the bench's stand-in): C, D and E complete to
/// each shell's edge, [`FIRST_SHELL_LY`] then 1,000 × 2^k ly, held at their caps, then to their
/// caps; A, B and the brown dwarfs, one shell each, to their caps in every reply.
fn shell_replies(caps: &[LayerCap]) -> Vec<CompleteTo> {
    let shelled = |layer: Layer| matches!(layer, Layer::C | Layer::D | Layer::E);
    let farthest = caps
        .iter()
        .filter(|cap| shelled(cap.layer()))
        .map(|cap| cap.radius().value())
        .fold(0.0, f64::max);
    let mut edges = vec![FIRST_SHELL_LY];
    while let Some(&last) = edges.last()
        && last < farthest
    {
        edges.push(if last < 1_000.0 { 1_000.0 } else { 2.0 * last });
    }
    let at = |edge: f64| {
        let radii: Vec<LayerCap> = caps
            .iter()
            .map(|cap| {
                let radius = if shelled(cap.layer()) {
                    cap.radius().value().min(edge)
                } else {
                    cap.radius().value()
                };
                LayerCap::forced(cap.layer(), LightYears::new(radius))
            })
            .collect();
        CompleteTo::of_caps(&radii)
    };
    let mut replies: Vec<CompleteTo> = edges
        .into_iter()
        .filter(|&edge| edge < farthest)
        .map(at)
        .collect();
    replies.push(CompleteTo::of_caps(caps));
    replies
}

/// One job's context: the sky's tables, envelope and offsets, and a noise cache of its own.
fn job_context(sky: &Sky) -> SkyContext<'_> {
    SkyContext {
        tables: &sky.tables,
        envelope: &sky.envelope,
        offsets: &sky.offsets,
        noise: NoiseCache::with_capacity(NOISE_SLOTS),
        cells: &NoSkyCellCache,
        sources: &[],
        modifiers: &NoModifiers,
    }
}

/// The near-Sun request's caps at the eye's cut, as its final reply is complete to them.
fn band_caps(sky: &Sky, query: &SkyQuery) -> Vec<LayerCap> {
    let mut noise = NoiseCache::with_capacity(NOISE_SLOTS);
    layer_caps(
        &sky.galaxy,
        &sky.tables,
        &sky.envelope,
        query.observer(),
        query.cut(),
        &mut noise,
    )
}

/// The near-Sun request of a camera at [`CAMERA_CUT_V`], with the eye asked beside it at
/// [`EYE_CUT_V`] (`eye` true) or not at all (R06.T9.j).
fn camera_query(ly: [f64; 3], eye: bool) -> SkyQuery {
    let at = GalacticPosition::from_light_years(ly).expect("in the root cube");
    let observer = Observer::new(at, UniverseTime::EPOCH).expect("the epoch is on the clock");
    let builder = SkyQuery::builder(observer, Magnitudes::new(CAMERA_CUT_V));
    let builder = if eye {
        builder
            .eye(EyeObserver::default())
            .eye_cut(Magnitudes::new(EYE_CUT_V))
    } else {
        builder
    };
    builder.build().expect("a valid query")
}

/// A request's caps and the replies of its shell plan ([`shell_replies`]), made once.
type Replies = OnceCell<(Vec<LayerCap>, Vec<CompleteTo>)>;

/// The caps and replies of `query`, made in `cell` on first use.
fn replies_of<'a>(cell: &'a Replies, query: &SkyQuery) -> &'a (Vec<LayerCap>, Vec<CompleteTo>) {
    cell.get_or_init(|| {
        let caps = band_caps(sky(), query);
        let replies = shell_replies(&caps);
        (caps, replies)
    })
}

/// Marches all six faces of `spec` for `query`, one face row of `jobs` a job on `workers` threads,
/// keeping `replies`: each job's march and the jobs' CPU time.
fn march_all(
    query: &SkyQuery,
    replies: &[CompleteTo],
    spec: BandSpec,
    jobs: &[(CubeFace, u16)],
    workers: usize,
) -> (Vec<(usize, BandMarch)>, Duration) {
    let sky = sky();
    on_pool(jobs, workers, &|&(face, row): &(CubeFace, u16)| {
        march_rows(
            &sky.galaxy,
            &mut job_context(sky),
            black_box(query),
            replies.iter().copied(),
            &spec,
            face,
            row..row + 1,
        )
    })
}

/// The march's bench `name` (`sky/band_near_sun/<name>`) for `query`: all six faces of
/// `BandSpec::STANDARD`, one face row a job, keeping the replies of its shell plan. Prints once
/// (`printed`) the rows, the replies and caps, the heap, the CPU time and the wall time.
fn bench_march(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    query: &SkyQuery,
    printed: &AtomicBool,
) {
    let workers = workers();
    let spec = BandSpec::STANDARD;
    let jobs: Vec<(CubeFace, u16)> = CubeFace::ALL
        .iter()
        .flat_map(|&face| (0..spec.face_texels()).map(move |row| (face, row)))
        .collect();
    // The caps and the replies, made outside the timing.
    let replies: Replies = OnceCell::new();
    group.bench_function(format!("band_near_sun/{name}"), |b| {
        let (caps, replies) = replies_of(&replies, query);
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let began = Instant::now();
                let (marches, busy) = march_all(query, replies, spec, &jobs, workers);
                if !printed.swap(true, Ordering::Relaxed) {
                    let heap = marches.iter().map(|(_, m)| m.heap_bytes()).sum::<usize>();
                    let eye = marches.first().and_then(|(_, m)| m.eye_cut()).map_or_else(
                        || "no eye light".to_owned(),
                        |cut| format!("the eye's light to V {}", cut.value()),
                    );
                    let radii: Vec<String> = caps
                        .iter()
                        .map(|cap| format!("{:?} {:.0}", cap.layer(), cap.radius().value()))
                        .collect();
                    println!(
                        "sky/band_near_sun/{name}: cut V {}, {eye}; {} rows, {} replies (caps \
                         {}), heap {heap} bytes, {:.1} CPU-s on {workers} workers, {:.2} s wall",
                        query.cut().value(),
                        marches.len(),
                        replies.len(),
                        radii.join(", "),
                        busy.as_secs_f64(),
                        began.elapsed().as_secs_f64()
                    );
                }
                cpu += busy;
            }
            cpu
        });
    });
}

/// The band of a near-Sun request, split as the server runs it (R06.T9.f): `band_near_sun/march`
/// marches all six faces once, one face row a job, keeping every reply of [`shell_replies`];
/// `band_near_sun/sum` sums the final reply's texels from that march, one face row a job. Each
/// prints, once, its CPU time and wall time, and the march its heap. `band_near_sun/march_camera`
/// marches a camera's request at [`CAMERA_CUT_V`] with the eye at [`EYE_CUT_V`] beside it, keeping
/// the eye's light too (R06.T9.j), and `band_near_sun/march_camera_no_eye` the same request with
/// no eye: the second sums cost their difference.
fn band_near_sun(c: &mut Criterion) {
    let query = eye_query(SUN_LY);
    let workers = workers();
    let spec = BandSpec::STANDARD;
    let jobs: Vec<(CubeFace, u16)> = CubeFace::ALL
        .iter()
        .flat_map(|&face| (0..spec.face_texels()).map(move |row| (face, row)))
        .collect();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    bench_march(&mut group, "march", &query, &PRINTED_BAND);
    let held: OnceCell<Vec<BandMarch>> = OnceCell::new();
    let replies: Replies = OnceCell::new();
    group.bench_function("band_near_sun/sum", |b| {
        let (caps, replies) = replies_of(&replies, &query);
        let marches = held.get_or_init(|| {
            march_all(&query, replies, spec, &jobs, workers)
                .0
                .into_iter()
                .map(|(_, march)| march)
                .collect()
        });
        let last = CompleteTo::of_caps(caps);
        b.iter_custom(|iters| {
            let mut cpu = Duration::ZERO;
            for _ in 0..iters {
                let began = Instant::now();
                let (rows, busy) = on_pool(marches, workers, &|march: &BandMarch| {
                    let mut out = Vec::with_capacity(usize::from(spec.face_texels()));
                    sum_rows(march, &SkyCensus::empty(), black_box(&last), &mut out);
                    out
                });
                if !PRINTED_BAND_SUM.swap(true, Ordering::Relaxed) {
                    let texels = rows.iter().map(|(_, row)| row.len()).sum::<usize>();
                    println!(
                        "sky/band_near_sun/sum: {texels} texels of the final reply, {:.3} CPU-s on \
                         {workers} workers, {:.3} s wall",
                        busy.as_secs_f64(),
                        began.elapsed().as_secs_f64()
                    );
                }
                cpu += busy;
            }
            cpu
        });
    });
    bench_march(
        &mut group,
        "march_camera",
        &camera_query(SUN_LY, true),
        &PRINTED_BAND_CAMERA,
    );
    bench_march(
        &mut group,
        "march_camera_no_eye",
        &camera_query(SUN_LY, false),
        &PRINTED_BAND_CAMERA_NO_EYE,
    );
    group.finish();
}

/// The limit map's near-Sun fixture: the 64² band of the stars fainter than the cut with no
/// census, complete everywhere, its faces in `CubeFace::ALL`'s order, and the stars the census
/// lists within 200 ly brighter than the cut, with their observer.
struct LimitMapFixture {
    observer: Observer,
    band: Vec<BandTexel>,
    listed: Vec<SkyStar>,
}

/// The near-Sun fixture of the limit map, the band and the census each on the pool.
fn limit_map_fixture(sky: &Sky, workers: usize, jobs: &[(CubeFace, u16)]) -> LimitMapFixture {
    let at = GalacticPosition::from_light_years(SUN_LY).expect("in the root cube");
    let observer = Observer::new(at, UniverseTime::EPOCH).expect("the epoch is on the clock");
    let query = SkyQuery::builder(observer, Magnitudes::new(LIMIT_MAP_CUT_V))
        .build()
        .expect("a valid query");
    let spec = BandSpec::STANDARD;
    let (rows, _) = on_pool(jobs, workers, &|&(face, row): &(CubeFace, u16)| {
        let mut ctx = SkyContext {
            tables: &sky.tables,
            envelope: &sky.envelope,
            offsets: &sky.offsets,
            noise: NoiseCache::with_capacity(NOISE_SLOTS),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let mut out = Vec::with_capacity(usize::from(spec.face_texels()));
        band_rows(
            &sky.galaxy,
            &mut ctx,
            &query,
            &SkyCensus::empty(),
            &CompleteTo::everywhere(),
            &spec,
            face,
            row..row + 1,
            &mut out,
        );
        out
    });
    let forced = query
        .with_caps_forced(LightYears::new(LIMIT_MAP_RADIUS_LY))
        .expect("a forced cap");
    let run = census(sky, &forced, &NoSkyCellCache, workers, 1);
    LimitMapFixture {
        observer,
        band: rows.into_iter().flat_map(|(_, row)| row).collect(),
        listed: run.census.listed().to_vec(),
    }
}

/// The glare ruling's synthetic sky (`decision-r06-t9c-glare.md`, item 2), as the sim's own test of
/// the pyramid takes it, star for star (`sky::limits`' `synthetic_sky`, on the same `SplitMix64`
/// stream as the sky tests' `uniforms`): `n` stars over the whole sky, concentrated towards the
/// plane as 1 + 3 exp(−|b| ÷ 10°), of V from −1.5 to `faintest` with N(< V) ∝ 10^(0.45 V), and of
/// S/P ratio uniform in 1.5–3.
fn synthetic_sky(n: usize, faintest: f64) -> Vec<(UnitVector, Lux, SpRatio)> {
    const SLOPE: f64 = 0.45;
    let (lo, hi) = (math::exp10(SLOPE * -1.5), math::exp10(SLOPE * faintest));
    let mut state = 0x0009_1a00_u64;
    let mut next = || {
        let z = mix(state);
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        #[expect(clippy::cast_precision_loss, reason = "53 bits, exact in f64")]
        let u = (z >> 11) as f64 / (1_u64 << 53) as f64;
        u
    };
    let mut sky = Vec::with_capacity(n);
    while sky.len() < n {
        let z = 2.0 * next() - 1.0;
        let (sin, cos) = math::sin_cos(std::f64::consts::TAU * next());
        let b = math::asin(z).abs() / RADIANS_PER_DEGREE;
        if 4.0 * next() > 1.0 + 3.0 * math::exp(-b / 10.0) {
            continue;
        }
        let across = (1.0 - z * z).sqrt();
        let direction =
            UnitVector::from_components([across * cos, across * sin, z]).expect("a direction");
        let v = math::log10(lo + next() * (hi - lo)) / SLOPE;
        let rho = SpRatio::new(1.5 + 1.5 * next()).expect("a ratio within 1.5–3");
        sky.push((direction, illuminance_of_magnitude(Magnitudes::new(v)), rho));
    }
    let values: Vec<f64> = sky
        .iter()
        .flat_map(|(u, e, rho)| {
            let [x, y, z] = u.components();
            [x, y, z, e.value(), rho.value()]
        })
        .collect();
    let digest = f64_digest(&values);
    assert_eq!(
        digest, SYNTHETIC_SKY_DIGEST,
        "the synthetic sky's digest {digest:#018x}: the sim's test's sky, star for star"
    );
    sky
}

/// One limit map as the server will run it (R06.T11.c): the glare `build` gives, on one thread,
/// then each of `jobs` (a face's row) on the pool, `band`'s rows' limits set against it. Its CPU
/// time is the build's and the jobs' together. Prints the first run's costs as `name`.
fn limit_map_once(
    name: &str,
    printed: &AtomicBool,
    build: &dyn Fn() -> Glare,
    band: &[BandTexel],
    jobs: &[(CubeFace, u16)],
    workers: usize,
) -> Duration {
    let spec = BandSpec::STANDARD;
    let side = usize::from(spec.face_texels());
    let began = Instant::now();
    let glare = build();
    let glare_time = began.elapsed();
    let (rows, busy) = on_pool(jobs, workers, &|&(face, row): &(CubeFace, u16)| {
        let first = (usize::from(face.layer()) * side + usize::from(row)) * side;
        let mut texels = band[first..first + side].to_vec();
        limit_rows(
            &EyeObserver::default(),
            &spec,
            black_box(&glare),
            face,
            row..row + 1,
            &mut texels,
        );
        texels
    });
    black_box(rows);
    if !printed.swap(true, Ordering::Relaxed) {
        println!(
            "sky/limit_map/{name}: {} stars over {} texels: the glare built in {:.3} s, the map \
             {:.2} CPU-s on {workers} workers ({:.2} s wall), {:.2} CPU-s in all",
            glare.len(),
            band.len(),
            glare_time.as_secs_f64(),
            busy.as_secs_f64(),
            began.elapsed().as_secs_f64(),
            (glare_time + busy).as_secs_f64()
        );
    }
    glare_time + busy
}

fn limit_map(c: &mut Criterion) {
    let workers = workers();
    let spec = BandSpec::STANDARD;
    let jobs: Vec<(CubeFace, u16)> = CubeFace::ALL
        .iter()
        .flat_map(|&face| (0..spec.face_texels()).map(move |row| (face, row)))
        .collect();
    // The fixture and the synthetic sky are made only when a bench that reads them is run.
    let fixture: OnceCell<LimitMapFixture> = OnceCell::new();
    let synthetic: OnceCell<Vec<(UnitVector, Lux, SpRatio)>> = OnceCell::new();
    let mut group = c.benchmark_group("sky");
    group.sample_size(10);
    group.bench_function("limit_map/near_sun", |b| {
        let fixture = fixture.get_or_init(|| limit_map_fixture(sky(), workers, &jobs));
        let cut = Magnitudes::new(LIMIT_MAP_CUT_V);
        let build = || Glare::of_listed(&fixture.observer, &fixture.listed, &spec, cut);
        b.iter_custom(|iters| {
            (0..iters)
                .map(|_| {
                    let name = "near_sun";
                    let printed = &PRINTED_LIMIT_MAP_NEAR_SUN;
                    limit_map_once(name, printed, &build, &fixture.band, &jobs, workers)
                })
                .sum()
        });
    });
    group.bench_function("limit_map/synthetic_300k", |b| {
        let fixture = fixture.get_or_init(|| limit_map_fixture(sky(), workers, &jobs));
        let stars = synthetic.get_or_init(|| {
            let n = usize::try_from(MAX_N_MAX).expect("300,000 fits a usize");
            synthetic_sky(n, SYNTHETIC_CUT_V)
        });
        let build = || Glare::of_points(stars.iter().copied(), &spec);
        b.iter_custom(|iters| {
            (0..iters)
                .map(|_| {
                    let name = "synthetic_300k";
                    let printed = &PRINTED_LIMIT_MAP_SYNTHETIC;
                    limit_map_once(name, printed, &build, &fixture.band, &jobs, workers)
                })
                .sum()
        });
    });
    group.finish();
}

criterion_group!(
    sky_benches,
    luminosity_tables,
    census_near_sun,
    census_nuclear_disc,
    band_near_sun,
    limit_map
);
criterion_main!(sky_benches);
