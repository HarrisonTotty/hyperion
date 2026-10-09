//! The luminosity tables against the realised sky (rendering plan R06, R06.T5.f; decided
//! 2026-10-06, `decision-r06-t9b-band.md`, item 8). It records, and gates nothing but its own
//! sample's size: a layer that meets one of the findings below is handed to the tables lane
//! (T5.d's fit, its young age bins at solar metallicity) with these figures, and is not a failure.
//!
//! On the Milky Way fixture at the epoch, with the tables as built (R06.T5.d's pair-evolved
//! correction, refitted at generator version 21):
//!
//! 1. **The paired deficit**, T5.c's paired comparison on more cells. Blocks of 6³ cells of
//!    layers C and D, and of E, recorded only ([`BLOCKS`]): T5.c's block at the solar circle
//!    (26,000 ly from the centre, in the plane) and its turns, evenly about the centre. Every
//!    realised system's V light is read pair-evolved (`state_at(t).stars()`) and as single stars
//!    (each `StarModel` alone), and the cells' deficit, Σ(S − P) ÷ ΣS, is held beside the fit's
//!    correction that the tables apply over the same blocks (`pair_light`, with its 1σ). The
//!    deficit's standard error is a ratio of sums over systems (the delta method), beside T5.c's
//!    √Σ(S − P)² ÷ ΣS and a jackknife over the blocks; at T5.c's z of 3.29 the largest of the
//!    three is asserted under 3 percentage points in C and D, the plan's sample size (a
//!    half-width). A layer whose deficit differs from the fit's by more than 3σ of the two
//!    together (the ratio's σ and the fit's) is a finding. To place the eight observers' count
//!    findings (the science check, 2026-10-07), the same systems are also held against the
//!    tables in light, pair-evolved and single, each with its compound-Poisson σ and a jackknife,
//!    and by component; and in counts brighter than M<sub>V</sub> 4, 2 and 0, single against the
//!    single-star tables (`TablesPlan::without_pair_correction`, a test-only read) and
//!    pair-evolved against the full pair counts, with the pairs' change of the count against the
//!    fit's.
//! 2. **Eight observers** on the solar circle at z☉ (68 ly), 45° apart in azimuth from the Sun's
//!    place in the sim's tests, each a census to V 8 within 300 ly with the caps forced and no
//!    eye. Per layer, against the tables' expectation: the listed V light (Σ 10<sup>−0.4 V</sup>,
//!    dimmed) and count, with and without each layer's nearest 50 ly; the counts once as tabulated
//!    (the pair-evolved excess only) and once with the whole pair correction, deficit included
//!    (`TablesPlan::with_full_pair_counts`, the test-only read). The expectation is the density
//!    field times the tables along [`RAYS`] rays, each through its own realised dust, the
//!    selection at the cut less the distance modulus and the Sun's V extinction behind the ray's
//!    A<sub>V</sub>, as the band subtracts it (R06.T8.k). A layer whose ensemble count ratio under
//!    the full correction lies outside 1 ± 3σ is a finding, σ being the listed stars' scatter
//!    counted by system alone, since the fit tabulates no error of its count difference.
//!
//!    The light of stars near an observer is a sum the rare bright and the nearest stars carry,
//!    so a typical realisation holds less than the mean (the band ruling's item 8). A Monte Carlo
//!    of the tables' own types gives the skew's expected median: per observer and 1-ly shell,
//!    each 0.05-mag bin of the tables at the observer is a type at the bin's middle magnitude,
//!    as many stars as carry the bin's light, at the shell's sky-averaged density of each
//!    component and its sky-averaged extinction, drawn as a Poisson process uniform in volume
//!    within the shell, each star alone; each trial's light is held against the model's own exact
//!    mean. Over [`MONTE_CARLO_TRIALS`] trials of all eight observers it gives the distribution of
//!    the eight observers' median light ratio, and each observer's own. A layer whose median ratio
//!    lies more than 3σ below that distribution's median, by the share of trials at or below it, is
//!    a finding.
//!
//! Each figure is printed. On WebAssembly, which has no threads, the test is left out; it would
//! take hours.

#![cfg(not(target_family = "wasm"))]

#[expect(
    dead_code,
    reason = "these checks use the placement and census helpers of tests/common alone"
)]
mod common;

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::reference_box_component_integrals;
use common::sky::{SUN_LY, census_parts, every_layer};
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, UnitVector};
use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use hyperion_sim::galaxy::fields::MAX_COMPONENTS;
use hyperion_sim::galaxy::gas::extinction::{NoiseMode, Quality, profile};
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::caps::CAPPED_LAYERS;
use hyperion_sim::sky::census::{SkyQuery, SkyStar};
use hyperion_sim::sky::colour::solar_colour;
use hyperion_sim::sky::luminosity::{
    BRIGHTEST_MAGNITUDE, BinSums, LuminosityTables, MAGNITUDE_BINS, MAGNITUDE_STEP,
};
use hyperion_sim::sky::photometry::absolute_v_of_state;
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::consts::{SECONDS_PER_JULIAN_YEAR, SOLAR_ABSOLUTE_MAGNITUDE_V};
use hyperion_sim::units::{LightYears, Magnitudes};
use hyperion_testkit::lcg::Lcg;

/// The fixture's seed, the sim's sky tests' own.
const SEED: u64 = 0x0926_0000;

/// The most threads the test runs on: eight, a share of a machine other work uses too.
const THREADS: usize = 8;

/// A finding's threshold, in standard errors (decided 2026-10-06).
const FINDING_SIGMAS: f64 = 3.0;

/// The solar circle's radius at the sites and observers, ly: the sim's tests' Sun-like place.
const SOLAR_CIRCLE_LY: f64 = SUN_LY[1];

/// Cells along each edge of a block: 6³ = 216 cells, T5.c's block.
const BLOCK_CELLS: i32 = 6;

/// T5.c's two-sided normal quantile, α = 10⁻³.
const Z: f64 = 3.29;

/// The largest interval of the paired deficit at [`Z`], a share of the single-star light: three
/// percentage points (decided 2026-10-06).
const DEFICIT_INTERVAL: f64 = 0.03;

/// The observers about the solar circle, 45° apart.
const OBSERVERS: usize = 8;

/// The census's cut, apparent V.
const CUT_V: f64 = 8.0;

/// How far each census lists, ly.
const RADIUS_LY: usize = 300;

/// The nearest shell each layer is also measured without, ly.
const NEAREST_LY: usize = 50;

/// The radius of the band ruling's estimate of the skew's median (about 0.90 within 200 ly), ly.
const RULING_LY: usize = 200;

/// The expectation's rays, a Fibonacci lattice: the caps' count.
const RAYS: usize = 768;

/// The rays of one job of the expectation.
const RAYS_PER_JOB: usize = 32;

/// Each star's sightline quality in the census, which the expectation's rays take too.
const SIGHTLINE: Quality = Quality::Budget(NonZeroU32::new(64).expect("64 is not zero"));

/// Trials of the Monte Carlo, each of all eight observers.
const MONTE_CARLO_TRIALS: usize = 20_000;

/// The trials of one job of the Monte Carlo.
const TRIALS_PER_JOB: usize = 250;

/// The Monte Carlo's seed, its own: no generated output reads it.
const MONTE_CARLO_SEED: u64 = 0x7a5f_5eed_0000_0001;

/// The golden gamma, which spreads the trials' seeds (Vigna 2015, `splitmix64.c`).
const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// Ten parsecs in light-years, the distance modulus's zero.
const TEN_PARSECS_LY: f64 = 10.0 * LIGHT_YEARS_PER_PARSEC;

/// The fixture: the Milky Way-like galaxy the sim's sky tests take.
fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid")
}

/// A count as a float.
fn count_f64(k: u64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "a count far below 2^53")]
    let k = k as f64;
    k
}

/// An index as a float.
fn index_f64(k: usize) -> f64 {
    count_f64(u64::try_from(k).expect("an index fits u64"))
}

/// The V light of an absolute magnitude, L☉,V.
fn light_of(m_v: f64) -> f64 {
    math::exp10(-0.4 * (m_v - SOLAR_ABSOLUTE_MAGNITUDE_V))
}

/// The distance modulus at `d` ly.
fn distance_modulus(d: f64) -> f64 {
    5.0 * math::log10(d / TEN_PARSECS_LY)
}

/// The V extinction of the Sun's light behind a sightline's `a_v`, mag: the selection the band
/// subtracts at (R06.T8.k).
fn solar_v_extinction(a_v: f64) -> f64 {
    solar_colour()
        .reddening()
        .through(Magnitudes::new(a_v))
        .v_extinction()
        .value()
}

/// `run` over `jobs` on up to [`THREADS`] threads, each taking the next job; the results in
/// `jobs`' order, so no reduction of them depends on the threads.
fn on_threads<J: Sync, R: Send>(jobs: &[J], run: impl Fn(&J) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut mine = Vec::new();
        loop {
            let k = next.fetch_add(1, Ordering::Relaxed);
            let Some(job) = jobs.get(k) else {
                return mine;
            };
            mine.push((k, run(job)));
        }
    };
    let threads = THREADS.min(jobs.len()).max(1);
    let mut all: Vec<(usize, R)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads).map(|_| scope.spawn(worker)).collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("no job panics"))
            .collect()
    });
    all.sort_unstable_by_key(|(k, _)| *k);
    all.into_iter().map(|(_, r)| r).collect()
}

/// The fixture's tables as shipped, with the full pair counts and as single stars (the test-only
/// reads), from one run of the plan's jobs: one stage's samples held at a time, as the bench builds
/// them, and the sums assembled three ways.
fn tables(galaxy: &Galaxy) -> [LuminosityTables; 3] {
    let plan = LuminosityTables::plan(galaxy);
    let sums: Vec<Mutex<BinSums>> = plan.bin_sums().into_iter().map(Mutex::new).collect();
    for stage in plan.stages() {
        let jobs: Vec<_> = plan.sample_jobs(stage).collect();
        let samples = plan.track_samples(on_threads(&jobs, |job| plan.run_samples(job.clone())));
        let jobs: Vec<_> = plan.accumulate_jobs(stage).collect();
        on_threads(&jobs, |&job| {
            let mut bin = sums[job.bin()].lock().expect("no job panics");
            plan.run_accumulate(&samples, job, &mut bin);
        });
    }
    let sums: Vec<BinSums> = sums
        .into_iter()
        .map(|bin| bin.into_inner().expect("no job panics"))
        .collect();
    let shipped = plan.assemble(sums.clone());
    let full = plan.clone().with_full_pair_counts().assemble(sums.clone());
    let single = plan.without_pair_correction().assemble(sums);
    [shipped, full, single]
}

// ---------------------------------------------------------------------------------------------
// 1. The paired deficit at the solar circle.
// ---------------------------------------------------------------------------------------------

/// Whether a layer's paired deficit asserts its interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Interval {
    /// Under [`DEFICIT_INTERVAL`] at [`Z`]: C and D, as the task sets them.
    Asserted,
    /// Printed only: E, which the task does not ask for.
    Recorded,
}

/// The blocks of each layer about the solar circle, the first T5.c's own. C and D take enough for
/// the paired deficit's interval: a pilot of eight blocks a layer (2026-10-07) gave a 1σ of 1.3
/// points in C and 3.2–4.9 in D, whose few bright systems carry its scatter. D takes no more than
/// fit about the circle without two blocks sharing a cell (638 ly apart, a block 384 ly wide). E
/// is recorded for the eight observers' E counts, which the paired comparison can place (the
/// science check, 2026-10-07).
const BLOCKS: [(Layer, usize, Interval); 3] = [
    (Layer::C, 64, Interval::Asserted),
    (Layer::D, 256, Interval::Asserted),
    (Layer::E, 64, Interval::Recorded),
];

/// The absolute magnitudes each system's stars are counted brighter than, M<sub>V</sub>: the
/// census's limit at 300 ly to V 8 (about +3.2) and brighter.
const COUNT_EDGES: [f64; 3] = [4.0, 2.0, 0.0];

/// One edge's counts over systems: each system's stars brighter than it as single stars, k<sub>S</sub>, and
/// pair-evolved, k<sub>P</sub>, with the sums their errors take.
#[derive(Debug, Default, Clone, Copy)]
struct Counted {
    single: f64,
    pair: f64,
    /// Σ k<sub>S</sub>².
    single_squared: f64,
    /// Σ k<sub>P</sub>².
    pair_squared: f64,
    /// Σ (k<sub>P</sub> − k<sub>S</sub>)².
    change_squared: f64,
    /// Σ (k<sub>P</sub> − k<sub>S</sub>) k<sub>S</sub>.
    change_single: f64,
}

impl Counted {
    fn add(&mut self, other: &Self) {
        self.single += other.single;
        self.pair += other.pair;
        self.single_squared += other.single_squared;
        self.pair_squared += other.pair_squared;
        self.change_squared += other.change_squared;
        self.change_single += other.change_single;
    }

    fn add_system(&mut self, single: u64, pair: u64) {
        let (s, p) = (count_f64(single), count_f64(pair));
        self.add(&Self {
            single: s,
            pair: p,
            single_squared: s * s,
            pair_squared: p * p,
            change_squared: (p - s) * (p - s),
            change_single: (p - s) * s,
        });
    }

    /// The pairs' change of the count, a share of the single-star count.
    fn change(&self) -> f64 {
        (self.pair - self.single) / self.single
    }

    /// Its standard error as a ratio of sums over systems (the delta method).
    fn change_sigma(&self) -> f64 {
        let f = self.change();
        let s = self.change_squared - 2.0 * f * self.change_single + f * f * self.single_squared;
        s.max(0.0).sqrt() / self.single
    }
}

/// The V light of realised systems, L☉,V: as single stars, S (each star's own model, which the
/// tables assume), and pair-evolved, P, with the sums the deficit's errors take; per component;
/// and their stars brighter than each of [`COUNT_EDGES`].
#[derive(Debug, Default, Clone, Copy)]
struct Paired {
    systems: u64,
    /// Systems not placed by the grid, which the tables do not hold.
    not_grid: u64,
    single: f64,
    pair: f64,
    /// Σ δ², with δ = S − P each system's deficit.
    deficit_squared: f64,
    /// Σ S².
    single_squared: f64,
    /// Σ P².
    pair_squared: f64,
    /// Σ δ S.
    deficit_single: f64,
    /// Per component, by index: S, P and Σ S².
    by_component: [[f64; 3]; MAX_COMPONENTS],
    counts: [Counted; COUNT_EDGES.len()],
}

impl Paired {
    fn add(&mut self, other: &Self) {
        self.systems += other.systems;
        self.not_grid += other.not_grid;
        self.single += other.single;
        self.pair += other.pair;
        self.deficit_squared += other.deficit_squared;
        self.single_squared += other.single_squared;
        self.pair_squared += other.pair_squared;
        self.deficit_single += other.deficit_single;
        for (a, b) in self.by_component.iter_mut().zip(&other.by_component) {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
        }
        for (a, b) in self.counts.iter_mut().zip(&other.counts) {
            a.add(b);
        }
    }

    fn deficit(&self) -> f64 {
        self.single - self.pair
    }

    /// The deficit's share of the single-star light.
    fn share(&self) -> f64 {
        self.deficit() / self.single
    }

    /// The share's standard error as a ratio of sums over systems (the delta method):
    /// √Σ(δ − f S)² ÷ ΣS.
    fn share_sigma(&self) -> f64 {
        let f = self.share();
        let s = self.deficit_squared - 2.0 * f * self.deficit_single + f * f * self.single_squared;
        s.max(0.0).sqrt() / self.single
    }

    /// T5.c's form of it: √Σδ² ÷ ΣS, the deficit's compound-Poisson scatter over the single-star
    /// light.
    fn t5c_sigma(&self) -> f64 {
        self.deficit_squared.sqrt() / self.single
    }
}

/// The absolute V of each star `states` gives that is not dark.
fn magnitudes<'a>(
    states: impl IntoIterator<Item = &'a hyperion_sim::stellar::StarState>,
) -> Vec<f64> {
    states
        .into_iter()
        .filter_map(absolute_v_of_state)
        .map(Magnitudes::value)
        .collect()
}

/// Every system of `key`, realised at the epoch.
fn paired_cell(galaxy: &Galaxy, key: CellKey) -> Paired {
    let mut out = Paired::default();
    let mut records: Vec<SystemRecord> = Vec::new();
    generate_cell(galaxy, key, &mut records);
    for record in &records {
        out.systems += 1;
        let stars = SystemStars::generate(galaxy, record);
        let Some(state) = stars.state_at(UniverseTime::EPOCH) else {
            continue;
        };
        let pair = magnitudes(state.stars());
        let alone: Vec<_> = stars
            .stars()
            .iter()
            .filter_map(|model| model.state_at(UniverseTime::EPOCH))
            .collect();
        let single = magnitudes(&alone);
        let light = |m: &[f64]| m.iter().fold(0.0, |sum, &v| sum + light_of(v));
        let (s, p) = (light(&single), light(&pair));
        let deficit = s - p;
        out.single += s;
        out.pair += p;
        out.deficit_squared += deficit * deficit;
        out.single_squared += s * s;
        out.pair_squared += p * p;
        out.deficit_single += deficit * s;
        // A non-grid system (a feature's or the centre's member), which no table holds.
        if let SystemOrigin::Grid(id) = record.origin() {
            let c = &mut out.by_component[id.index()];
            *c = [c[0] + s, c[1] + p, c[2] + s * s];
        } else {
            out.not_grid += 1;
        }
        let brighter = |m: &[f64], edge: f64| m.iter().map(|&v| u64::from(v < edge)).sum::<u64>();
        for (counted, &edge) in out.counts.iter_mut().zip(&COUNT_EDGES) {
            counted.add_system(brighter(&single, edge), brighter(&pair, edge));
        }
    }
    out
}

/// The site of block `b` of `blocks`, ly: T5.c's (0, 26,000, 0) turned by `b` of `blocks` turns
/// about the centre, to whole light-years.
fn block_site(b: usize, blocks: usize) -> [i32; 3] {
    let turn = core::f64::consts::TAU * index_f64(b) / index_f64(blocks);
    let (sin, cos) = math::sin_cos(turn);
    let whole = |x: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a site on the solar circle, a whole number of ly inside the root cube"
        )]
        let x = x.round() as i32;
        x
    };
    [
        whole(SOLAR_CIRCLE_LY * sin),
        whole(SOLAR_CIRCLE_LY * cos),
        0,
    ]
}

/// The block of `layer`'s cells about `site`, as T5.c takes it: its keys, its low corner, ly, and
/// its edge, ly.
fn block(layer: Layer, site: [i32; 3]) -> (Vec<CellKey>, [i32; 3], u32) {
    let cell_ly = i32::try_from(layer.cell_size_ly()).expect("cells are small");
    let corner = site.map(|x| x.div_euclid(cell_ly) - BLOCK_CELLS / 2);
    let mut keys = Vec::new();
    for i in 0..BLOCK_CELLS {
        for j in 0..BLOCK_CELLS {
            for k in 0..BLOCK_CELLS {
                keys.push(
                    CellKey::new(layer, [corner[0] + i, corner[1] + j, corner[2] + k])
                        .expect("the block lies in the root cube"),
                );
            }
        }
    }
    let edge = u32::try_from(BLOCK_CELLS * cell_ly).expect("positive");
    (keys, corner.map(|c| c * cell_ly), edge)
}

/// What the tables expect of a block, summed over its components: its systems, their V light
/// (pair-corrected), the correction's light (`pair_light`, negative where pairs remove light) and
/// its 1σ, which the components share and so add linearly (an upper bound, since components read
/// the same fitted cells); per component, the single-star light; and per edge of
/// [`COUNT_EDGES`], the stars brighter as the single-star, the full and the shipped tables count
/// them.
#[derive(Debug, Default, Clone, Copy)]
struct Tabulated {
    systems: f64,
    light: f64,
    pair: f64,
    pair_sigma: f64,
    by_component: [f64; MAX_COMPONENTS],
    counts: [[f64; 3]; COUNT_EDGES.len()],
}

impl Tabulated {
    fn add(&mut self, other: &Self) {
        self.systems += other.systems;
        self.light += other.light;
        self.pair += other.pair;
        self.pair_sigma += other.pair_sigma;
        for (a, b) in self.by_component.iter_mut().zip(&other.by_component) {
            *a += b;
        }
        for (a, b) in self.counts.iter_mut().zip(&other.counts) {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
        }
    }

    /// The single-star light.
    fn single(&self) -> f64 {
        self.light - self.pair
    }

    /// The fit's deficit, a share of the single-star light.
    fn share(&self) -> f64 {
        -self.pair / self.single()
    }

    fn share_sigma(&self) -> f64 {
        self.pair_sigma / self.single()
    }
}

fn tabulated(read: &Read<'_>, layer: Layer, site: [i32; 3]) -> Tabulated {
    let galaxy = read.galaxy;
    let (_, min_ly, edge) = block(layer, site);
    let integrals = reference_box_component_integrals(galaxy, min_ly, edge, 4 * 6);
    let now = read.tables.age_for(UniverseTime::EPOCH, Span::ZERO);
    let band = MassBand::from(layer);
    let at = site.map(f64::from);
    let at = PointLy::new(at[0], at[1], at[2]);
    let mut out = Tabulated::default();
    for id in galaxy.fields().component_ids() {
        let component = galaxy.fields().component(id);
        let n = integrals[id.index()] * galaxy.shares().component_share(band, component);
        let [shipped, full, single] =
            [read.tables, read.full, read.single].map(|t| t.get_at(id, layer, &at));
        let mut part = Tabulated {
            systems: n,
            light: n * shipped.total_light(now).value(),
            pair: n * shipped.pair_light(now).value(),
            pair_sigma: n * shipped.pair_light_sigma(now).value(),
            ..Tabulated::default()
        };
        part.by_component[id.index()] = n * single.total_light(now).value();
        for (counts, &m) in part.counts.iter_mut().zip(&COUNT_EDGES) {
            let m = Magnitudes::new(m);
            *counts = [single, full, shipped].map(|t| n * t.count_brighter_than(m, now));
        }
        out.add(&part);
    }
    out
}

/// The jackknife's standard error of `ratio` over blocks: `ratio` of every block's sum but one's,
/// for each in turn.
fn jackknife(count: usize, ratio: impl Fn(usize) -> f64) -> f64 {
    let ratios: Vec<f64> = (0..count).map(ratio).collect();
    let k = index_f64(count);
    let mean = ratios.iter().sum::<f64>() / k;
    let spread = ratios.iter().map(|s| (s - mean) * (s - mean)).sum::<f64>();
    (spread * (k - 1.0) / k).sqrt()
}

/// Percentage points.
fn pp(x: f64) -> f64 {
    100.0 * x
}

/// T5.c's paired comparison over `count` blocks of `layer` at the solar circle: prints it, its
/// light, components and counts, asserts its interval as `interval` says and returns its finding,
/// if any.
fn paired_deficit(
    read: &Read<'_>,
    (layer, count, interval): (Layer, usize, Interval),
) -> Option<String> {
    let keys: Vec<CellKey> = (0..count)
        .flat_map(|b| block(layer, block_site(b, count)).0)
        .collect();
    let cells = on_threads(&keys, |&key| paired_cell(read.galaxy, key));
    let per_block = cells.len() / count;
    let blocks: Vec<Paired> = cells
        .chunks_exact(per_block)
        .map(|chunk| {
            let mut sum = Paired::default();
            for cell in chunk {
                sum.add(cell);
            }
            sum
        })
        .collect();
    let expected: Vec<Tabulated> = (0..count)
        .map(|b| tabulated(read, layer, block_site(b, count)))
        .collect();
    let (mut realised, mut fit) = (Paired::default(), Tabulated::default());
    for (part, tables) in blocks.iter().zip(&expected) {
        realised.add(part);
        fit.add(tables);
    }
    let (f, sigma, t5c) = (
        realised.share(),
        realised.share_sigma(),
        realised.t5c_sigma(),
    );
    let jackknifed = jackknife(count, |b| {
        (realised.deficit() - blocks[b].deficit()) / (realised.single - blocks[b].single)
    });
    let (fit_f, fit_sigma) = (fit.share(), fit.share_sigma());
    let z = (f - fit_f) / math::hypot(sigma, fit_sigma);
    let widest = sigma.max(t5c).max(jackknifed);
    eprintln!(
        "paired deficit, solar circle {layer:?}: {count} blocks of {per_block} cells, {} systems \
         ({} not placed by the grid; the tables expect {:.1}); realised {:.2}% ± {:.2} (1σ, ratio \
         of sums), ± {:.2} (T5.c's form), ± {:.2} (the blocks' jackknife); the fit's {:.2}% ± \
         {:.2}; difference {:+.2} points, {z:+.2}σ ({:+.2}σ by the jackknife); interval at z {Z}: \
         {:.2} points. T5.c's block alone: realised {:.2}% of {} systems, the fit's {:.2}%",
        realised.systems,
        realised.not_grid,
        fit.systems,
        pp(f),
        pp(sigma),
        pp(t5c),
        pp(jackknifed),
        pp(fit_f),
        pp(fit_sigma),
        pp(f - fit_f),
        (f - fit_f) / math::hypot(jackknifed, fit_sigma),
        pp(Z * widest),
        pp(blocks[0].share()),
        blocks[0].systems,
        pp(expected[0].share()),
    );
    paired_light(read, layer, &blocks, &expected, &realised, &fit);
    paired_counts(layer, &realised, &fit);
    if interval == Interval::Asserted {
        assert!(
            Z * widest < DEFICIT_INTERVAL,
            "{layer:?}: the paired deficit's interval is {:.2} points, not under {:.0}: take \
             more blocks",
            pp(Z * widest),
            pp(DEFICIT_INTERVAL)
        );
    }
    (z.abs() > FINDING_SIGMAS).then(|| {
        format!(
            "{layer:?}: the paired deficit {:.2}% differs from the fit's {:.2}% by {z:+.2}σ",
            pp(f),
            pp(fit_f)
        )
    })
}

/// Prints the blocks' realised light against the tables', pair-evolved and single, each with its
/// compound-Poisson σ and the blocks' jackknife, and the single-star light by component.
fn paired_light(
    read: &Read<'_>,
    layer: Layer,
    blocks: &[Paired],
    expected: &[Tabulated],
    realised: &Paired,
    fit: &Tabulated,
) {
    let count = blocks.len();
    let pair_jackknife = jackknife(count, |b| {
        (realised.pair - blocks[b].pair) / (fit.light - expected[b].light)
    });
    let single_jackknife = jackknife(count, |b| {
        (realised.single - blocks[b].single) / (fit.single() - expected[b].single())
    });
    eprintln!(
        "realised light, solar circle {layer:?}: {:.4e} pair-evolved against the tables' {:.4e}, \
         ratio {:.4} ± {:.4} (1σ, compound Poisson) ± {:.4} (jackknife); {:.4e} single against \
         the single-star tables' {:.4e}, ratio {:.4} ± {:.4} ± {:.4}",
        realised.pair,
        fit.light,
        realised.pair / fit.light,
        realised.pair_squared.sqrt() / fit.light,
        pair_jackknife,
        realised.single,
        fit.single(),
        realised.single / fit.single(),
        realised.single_squared.sqrt() / fit.single(),
        single_jackknife,
    );
    let fields = read.galaxy.fields();
    for id in fields.component_ids() {
        let i = id.index();
        let (tabulated, [single, pair, squared]) = (fit.by_component[i], realised.by_component[i]);
        // The components that carry at least a hundredth of the layer's light.
        if tabulated < 0.01 * fit.single() {
            continue;
        }
        let component = fields.component(id);
        eprintln!(
            "  component {i} ({:?}, mean age {:.2} Gyr): single light {single:.4e} against \
             {tabulated:.4e}, ratio {:.4} ± {:.4}; pair-evolved {pair:.4e}, deficit {:.2}%",
            component.population(),
            component.ages().mean().value() / 1e9,
            single / tabulated,
            squared.sqrt() / tabulated,
            pp((single - pair) / single),
        );
    }
}

/// Prints the blocks' realised stars brighter than each of [`COUNT_EDGES`], single against the
/// single-star tables and pair-evolved against the full and the shipped counts, and the pairs'
/// change of the count against the fit's.
fn paired_counts(layer: Layer, realised: &Paired, fit: &Tabulated) {
    for ((counted, &[single, full, shipped]), edge) in
        realised.counts.iter().zip(&fit.counts).zip(COUNT_EDGES)
    {
        eprintln!(
            "realised counts, solar circle {layer:?}, brighter than M_V {edge}: {} single \
             against the single-star tables' {single:.1}, ratio {:.4} ± {:.4}; {} pair-evolved \
             against the full pair counts' {full:.1}, ratio {:.4} ± {:.4}, and the shipped \
             {shipped:.1}; the pairs' change {:+.2}% ± {:.2} against the fit's {:+.2}%",
            counted.single,
            counted.single / single,
            counted.single_squared.sqrt() / single,
            counted.pair,
            counted.pair / full,
            counted.pair_squared.sqrt() / full,
            pp(counted.change()),
            pp(counted.change_sigma()),
            pp(full / single - 1.0),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Eight observers on the solar circle.
// ---------------------------------------------------------------------------------------------

/// The layers measured, in [`CAPPED_LAYERS`]' order.
const LAYERS: usize = CAPPED_LAYERS.len();

/// The fewest stars the tables expect of a layer in one census for it to be measured: the brown
/// dwarfs, some 10⁻⁸ a census, are left out (their ratios read 1).
const LEAST_EXPECTED: f64 = 0.01;

/// Observer `j`: on the solar circle at z☉, `j` eighths of a turn from [`SUN_LY`].
fn observer(j: usize) -> Observer {
    let turn = core::f64::consts::TAU * index_f64(j) / index_f64(OBSERVERS);
    let (sin, cos) = math::sin_cos(turn);
    let ly = [SOLAR_CIRCLE_LY * sin, SOLAR_CIRCLE_LY * cos, SUN_LY[2]];
    Observer::new(
        GalacticPosition::from_light_years(ly).expect("in the root cube"),
        UniverseTime::EPOCH,
    )
    .expect("the epoch is within the clock window")
}

/// One layer's stars as a census lists them within [`RADIUS_LY`]: their count and V light (in
/// units of a V 0 star), within [`NEAREST_LY`] and beyond, and within [`RULING_LY`]; and each
/// system's listed stars, for the count's compound-Poisson variance.
#[derive(Debug, Default, Clone)]
struct Listed {
    count: u64,
    near_count: u64,
    light: f64,
    near_light: f64,
    ruling_light: f64,
    by_system: BTreeMap<SystemId, u64>,
}

impl Listed {
    fn add(&mut self, star: &SkyStar) {
        let d = star.distance().value();
        let light = math::exp10(-0.4 * star.v().value());
        self.count += 1;
        self.light += light;
        if d <= index_f64(NEAREST_LY) {
            self.near_count += 1;
            self.near_light += light;
        }
        if d <= index_f64(RULING_LY) {
            self.ruling_light += light;
        }
        *self.by_system.entry(star.system()).or_default() += 1;
    }

    /// Σ over systems of the square of each one's listed stars.
    fn count_variance(&self) -> f64 {
        self.by_system.values().map(|&k| count_f64(k * k)).sum()
    }
}

/// Each layer's listed stars for `observer`: the census to [`CUT_V`] with every cap forced to
/// [`RADIUS_LY`], no eye, its stars within the radius.
fn listed(galaxy: &Galaxy, observer: Observer) -> [Listed; LAYERS] {
    let query = SkyQuery::builder(observer, Magnitudes::new(CUT_V))
        .build()
        .expect("a valid query");
    let radius = LightYears::new(index_f64(RADIUS_LY));
    let mut out: [Listed; LAYERS] = Default::default();
    for (stars, _) in census_parts(galaxy, query, &every_layer(radius)) {
        for star in stars.iter().filter(|s| s.distance() <= radius) {
            let l = CAPPED_LAYERS
                .iter()
                .position(|&layer| layer == star.layer())
                .expect("a capped layer");
            out[l].add(star);
        }
    }
    out
}

/// One quantity's integral within [`NEAREST_LY`], [`RULING_LY`] and [`RADIUS_LY`].
#[derive(Debug, Default, Clone, Copy)]
struct Within {
    near: f64,
    ruling: f64,
    all: f64,
}

/// The tables' expectation of one layer's listed stars: the counts as tabulated and with the full
/// pair counts, and the light in units of a V 0 star.
#[derive(Debug, Default, Clone, Copy)]
struct Expected {
    count: Within,
    full_count: Within,
    light: Within,
}

/// Along the rays at each whole light-year from 0 to [`RADIUS_LY`]: per layer, the counts and the
/// light per unit radius summed over the rays, each ray weighted by its solid angle; and for the
/// Monte Carlo, each component's density so summed and the Sun's V extinction so summed.
#[derive(Debug, Clone)]
struct Rays {
    count: Vec<[f64; LAYERS]>,
    full_count: Vec<[f64; LAYERS]>,
    light: Vec<[f64; LAYERS]>,
    density: Vec<[f64; MAX_COMPONENTS]>,
    extinction: Vec<f64>,
}

impl Rays {
    fn zero() -> Self {
        let nodes = RADIUS_LY + 1;
        Self {
            count: vec![[0.0; LAYERS]; nodes],
            full_count: vec![[0.0; LAYERS]; nodes],
            light: vec![[0.0; LAYERS]; nodes],
            density: vec![[0.0; MAX_COMPONENTS]; nodes],
            extinction: vec![0.0; nodes],
        }
    }

    fn add(&mut self, other: &Self) {
        let add = |a: &mut [f64], b: &[f64]| a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
        for n in 0..=RADIUS_LY {
            add(&mut self.count[n], &other.count[n]);
            add(&mut self.full_count[n], &other.full_count[n]);
            add(&mut self.light[n], &other.light[n]);
            add(&mut self.density[n], &other.density[n]);
        }
        add(&mut self.extinction, &other.extinction);
    }

    /// Layer `l`'s expectation, by the trapezoid rule over the nodes.
    fn expected(&self, l: usize) -> Expected {
        let within = |values: &[[f64; LAYERS]]| {
            let to = |end: usize| {
                (0..end).fold(0.0, |sum, n| {
                    sum + f64::midpoint(values[n][l], values[n + 1][l])
                })
            };
            Within {
                near: to(NEAREST_LY),
                ruling: to(RULING_LY),
                all: to(RADIUS_LY),
            }
        };
        Expected {
            count: within(&self.count),
            full_count: within(&self.full_count),
            light: within(&self.light),
        }
    }
}

/// The fixture and its tables: as shipped, with the full pair counts and as single stars (the
/// test-only reads).
struct Read<'a> {
    galaxy: &'a Galaxy,
    tables: &'a LuminosityTables,
    full: &'a LuminosityTables,
    single: &'a LuminosityTables,
}

/// `n` directions spread evenly over the sphere (a Fibonacci lattice), as the caps take them.
fn fibonacci_sphere(n: usize) -> Vec<UnitVector> {
    let golden = core::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    (0..n)
        .map(|k| {
            let k = index_f64(k);
            let z = 1.0 - (2.0 * k + 1.0) / index_f64(n);
            let rho = (1.0 - z * z).sqrt();
            let (sin, cos) = math::sin_cos(golden * k);
            UnitVector::from_components([rho * cos, rho * sin, z]).expect("a unit vector")
        })
        .collect()
}

/// The rays `directions` from `observer`, summed in their order.
fn march(read: &Read<'_>, observer: &Observer, directions: &[UnitVector]) -> Rays {
    let Read {
        galaxy,
        tables,
        full,
        ..
    } = *read;
    let fields = galaxy.fields();
    let components: Vec<_> = fields.component_ids().collect();
    let origin = observer.position();
    let p0 = origin.to_light_years_f64();
    let nodes: Vec<LightYears> = (0..=RADIUS_LY)
        .map(|n| LightYears::new(index_f64(n)))
        .collect();
    let weight = 4.0 * core::f64::consts::PI / index_f64(RAYS);
    let mut cache = NoiseCache::with_capacity(1 << 16);
    let (mut a_v, mut densities, mut out) = (Vec::new(), [0.0; MAX_COMPONENTS], Rays::zero());
    for direction in directions {
        let mut ray = Rays::zero();
        profile(
            galaxy.gas(),
            origin,
            *direction,
            &nodes,
            NoiseMode::Realised,
            SIGHTLINE,
            &[],
            &mut cache,
            &mut a_v,
        );
        let u = direction.components();
        for (n, node) in nodes.iter().enumerate() {
            let r = node.value();
            let p = PointLy::new(p0[0] + r * u[0], p0[1] + r * u[1], p0[2] + r * u[2]);
            let ago = tables.age_for(
                observer.time(),
                Span::from_seconds_f64(r * SECONDS_PER_JULIAN_YEAR)
                    .expect("a light time within 300 ly is a span"),
            );
            let dimming = solar_v_extinction(a_v[n].value());
            // At the observer every star is bright enough: the faintest edge reads them all.
            let limit = if n == 0 {
                Magnitudes::new(f64::MAX)
            } else {
                Magnitudes::new(CUT_V - distance_modulus(r) - dimming)
            };
            // The light of a star of L at r through `dimming`, times r², in V 0 stars: L
            // 10^(−0.4 (M☉ + A)) (10 pc)².
            let flux = math::exp10(-0.4 * (SOLAR_ABSOLUTE_MAGNITUDE_V + dimming))
                * TEN_PARSECS_LY
                * TEN_PARSECS_LY;
            fields.densities(&p, &mut densities);
            ray.extinction[n] = weight * dimming;
            for &id in &components {
                let rho = densities[id.index()];
                ray.density[n][id.index()] = weight * rho;
                if rho <= 0.0 {
                    continue;
                }
                for (l, &layer) in CAPPED_LAYERS.iter().enumerate() {
                    let share = galaxy
                        .shares()
                        .component_share(MassBand::from(layer), fields.component(id));
                    let systems = weight * rho * share;
                    let f = tables.get_at(id, layer, &p);
                    let brighter =
                        f.total_light(ago).value() - f.light_fainter_than(limit, ago).value();
                    ray.count[n][l] += systems * r * r * f.count_brighter_than(limit, ago);
                    ray.full_count[n][l] += systems
                        * r
                        * r
                        * full.get_at(id, layer, &p).count_brighter_than(limit, ago);
                    ray.light[n][l] += systems * brighter * flux;
                }
            }
        }
        out.add(&ray);
    }
    out
}

/// The expectation's rays from `observer`, in jobs of [`RAYS_PER_JOB`] summed in their order.
fn rays(read: &Read<'_>, observer: &Observer) -> Rays {
    let directions = fibonacci_sphere(RAYS);
    let jobs: Vec<&[UnitVector]> = directions.chunks(RAYS_PER_JOB).collect();
    let parts = on_threads(&jobs, |&chunk| march(read, observer, chunk));
    let mut out = Rays::zero();
    for part in &parts {
        out.add(part);
    }
    out
}

/// One layer's types in one shell of the Monte Carlo: per 0.05-mag bin of the tables, its stars
/// expected in the shell (running sums from the brightest), and the shell's dimming.
#[derive(Debug, Clone)]
struct Shell {
    /// Stars expected in the shell of every bin brighter than each bin's end.
    running: Vec<f64>,
    /// The bins that some star of the shell can be listed from: a star of bin j at the middle
    /// magnitude m is listed out to the distance where m, its distance modulus and A sum to the
    /// cut.
    listable: usize,
    /// The shell's sky-averaged V extinction of the Sun's light, mag.
    dimming: f64,
}

/// One layer's Monte Carlo model for one observer: its shells, and its exact mean light within
/// [`NEAREST_LY`], [`RULING_LY`] and [`RADIUS_LY`].
#[derive(Debug, Clone)]
struct Model {
    shells: Vec<Shell>,
    mean: Within,
}

/// The middle magnitude of the tables' bin `j`.
fn bin_middle(j: usize) -> f64 {
    BRIGHTEST_MAGNITUDE + MAGNITUDE_STEP * (index_f64(j) + 0.5)
}

/// How far a star of absolute magnitude `m` is listed behind `dimming`, ly.
fn listed_out_to(m: f64, dimming: f64) -> f64 {
    TEN_PARSECS_LY * math::exp10((CUT_V - m - dimming) / 5.0)
}

/// The light of a star of absolute magnitude `m` behind `dimming` at 1 ly, in V 0 stars.
fn light_at_one_ly(m: f64, dimming: f64) -> f64 {
    math::exp10(-0.4 * (m + dimming)) * TEN_PARSECS_LY * TEN_PARSECS_LY
}

/// Layer `l`'s model for `observer`: its tables' light per bin at the observer, at the shells'
/// sky-averaged density of each component. A layer of which the tables expect fewer than
/// [`LEAST_EXPECTED`] stars has none.
fn model(read: &Read<'_>, observer: &Observer, rays: &Rays, l: usize) -> Model {
    if rays.expected(l).count.all < LEAST_EXPECTED {
        return Model {
            shells: Vec::new(),
            mean: Within::default(),
        };
    }
    let galaxy = read.galaxy;
    let fields = galaxy.fields();
    let layer = CAPPED_LAYERS[l];
    let band = MassBand::from(layer);
    let at = observer.position().to_light_years_f64();
    let at = PointLy::new(at[0], at[1], at[2]);
    // The light ages within the radius differ by under 300 years, over which a population's light
    // changes by about 10⁻⁵ (the tables' module docs): one read at the middle radius serves.
    let ago = read.tables.age_for(
        observer.time(),
        Span::from_seconds_f64(index_f64(RADIUS_LY / 2) * SECONDS_PER_JULIAN_YEAR).expect("a span"),
    );
    let edge = |j: usize| Magnitudes::new(BRIGHTEST_MAGNITUDE + MAGNITUDE_STEP * index_f64(j));
    // Per component, the light per system in each bin, L☉,V, times its layer share.
    let lights: Vec<(usize, Vec<f64>)> = fields
        .component_ids()
        .map(|id| {
            let f = read.tables.get_at(id, layer, &at);
            let share = galaxy.shares().component_share(band, fields.component(id));
            let bins = (0..MAGNITUDE_BINS)
                .map(|j| {
                    share
                        * (f.light_fainter_than(edge(j), ago).value()
                            - f.light_fainter_than(edge(j + 1), ago).value())
                })
                .collect();
            (id.index(), bins)
        })
        .collect();
    let mut shells = Vec::with_capacity(RADIUS_LY);
    let mut mean = Within::default();
    for k in 0..RADIUS_LY {
        let (inner, outer) = (index_f64(k), index_f64(k + 1));
        let dimming = f64::midpoint(rays.extinction[k], rays.extinction[k + 1])
            / (4.0 * core::f64::consts::PI);
        let volume = (outer * outer * outer - inner * inner * inner) / 3.0;
        let (mut running, mut sum, mut listable, mut light) = (Vec::new(), 0.0, 0, 0.0);
        for j in 0..MAGNITUDE_BINS {
            let m = bin_middle(j);
            // The bin's light in the shell, L☉,V: the sky-summed density times the volume.
            let bin_light = lights.iter().fold(0.0, |s, (c, bins)| {
                s + f64::midpoint(rays.density[k][*c], rays.density[k + 1][*c]) * bins[j]
            }) * volume;
            sum += bin_light / light_of(m);
            running.push(sum);
            let reach = listed_out_to(m, dimming);
            if reach > inner {
                listable = j + 1;
                // ∫ over the shell of r² × the light at r, 1 ÷ r², where it is listed: the
                // stretch of the shell within its reach.
                light += bin_light / light_of(m) * light_at_one_ly(m, dimming) * 3.0
                    / (outer * outer * outer - inner * inner * inner)
                    * (reach.min(outer) - inner);
            }
        }
        if k < NEAREST_LY {
            mean.near += light;
        }
        if k < RULING_LY {
            mean.ruling += light;
        }
        mean.all += light;
        shells.push(Shell {
            running,
            listable,
            dimming,
        });
    }
    Model { shells, mean }
}

/// A Poisson deviate of mean `mean`: Knuth's product of uniforms, in parts of a mean at most 30.
fn poisson(rng: &mut Lcg, mean: f64) -> u64 {
    let mut left = mean;
    let mut k = 0;
    while left > 0.0 {
        let part = left.min(30.0);
        left -= part;
        let floor = math::exp(-part);
        let mut product = rng.next_f64();
        while product > floor {
            k += 1;
            product *= rng.next_f64();
        }
    }
    k
}

/// One trial of `model`: its light within [`NEAREST_LY`], [`RULING_LY`] and [`RADIUS_LY`], in V 0
/// stars.
fn trial(model: &Model, rng: &mut Lcg) -> Within {
    let mut out = Within::default();
    for (k, shell) in model.shells.iter().enumerate() {
        if shell.listable == 0 {
            continue;
        }
        let total = shell.running[shell.listable - 1];
        let (inner, outer) = (index_f64(k), index_f64(k + 1));
        let (lo, hi) = (inner * inner * inner, outer * outer * outer);
        for _ in 0..poisson(rng, total) {
            let u = rng.next_f64() * total;
            let j = shell.running[..shell.listable].partition_point(|&s| s <= u);
            let j = j.min(shell.listable - 1);
            let m = bin_middle(j);
            let r = math::cbrt(lo + rng.next_f64() * (hi - lo));
            if r >= listed_out_to(m, shell.dimming) || r <= 0.0 {
                continue;
            }
            let light = light_at_one_ly(m, shell.dimming) / (r * r);
            if k < NEAREST_LY {
                out.near += light;
            }
            if k < RULING_LY {
                out.ruling += light;
            }
            out.all += light;
        }
    }
    out
}

/// What the medians are taken of: each layer's light ratio within [`RADIUS_LY`], and beyond
/// [`NEAREST_LY`]; then every layer's together within [`RADIUS_LY`], beyond [`NEAREST_LY`] and
/// within [`RULING_LY`].
const RATIOS: usize = 2 * LAYERS + 3;

/// The ratios of `light` against `mean`, per layer, laid out as [`RATIOS`] says; a layer with no
/// light expected reads 1.
fn ratios(light: &[Within; LAYERS], mean: &[Within; LAYERS]) -> [f64; RATIOS] {
    let ratio = |a: f64, b: f64| if b > 0.0 { a / b } else { 1.0 };
    let mut out = [0.0; RATIOS];
    let (mut all, mut far, mut ruling) = ([0.0; 2], [0.0; 2], [0.0; 2]);
    for l in 0..LAYERS {
        let (got, want) = (light[l], mean[l]);
        out[l] = ratio(got.all, want.all);
        out[LAYERS + l] = ratio(got.all - got.near, want.all - want.near);
        all = [all[0] + got.all, all[1] + want.all];
        far = [far[0] + got.all - got.near, far[1] + want.all - want.near];
        ruling = [ruling[0] + got.ruling, ruling[1] + want.ruling];
    }
    out[2 * LAYERS] = ratio(all[0], all[1]);
    out[2 * LAYERS + 1] = ratio(far[0], far[1]);
    out[2 * LAYERS + 2] = ratio(ruling[0], ruling[1]);
    out
}

/// The Monte Carlo's [`RATIOS`] for each of `models`' observers in each trial, trial-major.
fn monte_carlo(models: &[[Model; LAYERS]]) -> Vec<Vec<[f64; RATIOS]>> {
    let starts: Vec<usize> = (0..MONTE_CARLO_TRIALS).step_by(TRIALS_PER_JOB).collect();
    let jobs = on_threads(&starts, |&start| {
        (start..(start + TRIALS_PER_JOB).min(MONTE_CARLO_TRIALS))
            .map(|t| {
                models
                    .iter()
                    .enumerate()
                    .map(|(o, layers)| {
                        let draw = u64::try_from(t * OBSERVERS + o).expect("a trial fits u64");
                        let mut rng = Lcg::new(
                            MONTE_CARLO_SEED.wrapping_add(draw.wrapping_mul(GOLDEN_GAMMA)),
                        );
                        let light: [Within; LAYERS] =
                            std::array::from_fn(|l| trial(&layers[l], &mut rng));
                        let mean: [Within; LAYERS] = std::array::from_fn(|l| layers[l].mean);
                        ratios(&light, &mean)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });
    jobs.into_iter().flatten().collect()
}

/// The median of `values`, sorted in place.
fn median(values: &mut [f64]) -> f64 {
    values.sort_unstable_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        f64::midpoint(values[n / 2 - 1], values[n / 2])
    }
}

/// The value at quantile `q` of sorted `values`, the nearest rank.
fn quantile(sorted: &[f64], q: f64) -> f64 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "q lies in [0, 1], so the rounded rank is a whole number from 0 to the last index"
    )]
    let k = (q * index_f64(sorted.len() - 1)).round() as usize;
    sorted[k]
}

/// The name of ratio `i` of [`RATIOS`].
fn ratio_name(i: usize) -> String {
    if i < LAYERS {
        format!("{:?} within {RADIUS_LY} ly", CAPPED_LAYERS[i])
    } else if i < 2 * LAYERS {
        format!(
            "{:?} from {NEAREST_LY} to {RADIUS_LY} ly",
            CAPPED_LAYERS[i - LAYERS]
        )
    } else {
        match i - 2 * LAYERS {
            0 => format!("every layer within {RADIUS_LY} ly"),
            1 => format!("every layer from {NEAREST_LY} to {RADIUS_LY} ly"),
            _ => format!("every layer within {RULING_LY} ly"),
        }
    }
}

/// The eight observers: prints each one's listed stars against the tables, the ensemble's
/// counts and the medians against the Monte Carlo's, and returns the findings.
fn eight_observers(read: &Read<'_>) -> Vec<String> {
    let mut realised = Vec::with_capacity(OBSERVERS);
    let mut models = Vec::with_capacity(OBSERVERS);
    let mut sums = [(0_u64, 0.0, 0.0, 0.0); LAYERS];
    for j in 0..OBSERVERS {
        let observer = observer(j);
        let stars = listed(read.galaxy, observer);
        let rays = rays(read, &observer);
        let layers: [Model; LAYERS] = std::array::from_fn(|l| model(read, &observer, &rays, l));
        let mut light = [Within::default(); LAYERS];
        let mut mean = [Within::default(); LAYERS];
        for (l, listed) in stars.iter().enumerate() {
            let expected = rays.expected(l);
            if expected.count.all < LEAST_EXPECTED {
                continue;
            }
            light[l] = Within {
                near: listed.near_light,
                ruling: listed.ruling_light,
                all: listed.light,
            };
            mean[l] = expected.light;
            let s = &mut sums[l];
            *s = (
                s.0 + listed.count,
                s.1 + expected.count.all,
                s.2 + expected.full_count.all,
                s.3 + listed.count_variance(),
            );
            eprintln!(
                "observer {j} {:?}: {} listed ({} within {NEAREST_LY} ly) against {:.2} \
                 tabulated ({:.2}) and {:.2} with the full pair counts ({:.2}); light {:.4e} \
                 against {:.4e}, ratio {:.3}; from {NEAREST_LY} ly, {:.4e} against {:.4e}, \
                 ratio {:.3}; the Monte Carlo's mean {:.4e} ({:+.2}% of the rays')",
                CAPPED_LAYERS[l],
                listed.count,
                listed.near_count,
                expected.count.all,
                expected.count.near,
                expected.full_count.all,
                expected.full_count.near,
                listed.light,
                expected.light.all,
                listed.light / expected.light.all,
                listed.light - listed.near_light,
                expected.light.all - expected.light.near,
                (listed.light - listed.near_light) / (expected.light.all - expected.light.near),
                layers[l].mean.all,
                100.0 * (layers[l].mean.all / expected.light.all - 1.0),
            );
        }
        realised.push(ratios(&light, &mean));
        models.push(layers);
    }
    let mut findings = Vec::new();
    for (l, &(count, tabulated, full, variance)) in sums.iter().enumerate() {
        let layer = CAPPED_LAYERS[l];
        if tabulated < LEAST_EXPECTED {
            continue;
        }
        let n = count_f64(count);
        let (ratio, sigma) = (n / full, variance.sqrt() / full);
        eprintln!(
            "ensemble {layer:?}: {count} listed against {tabulated:.1} tabulated (ratio {:.3}) \
             and {full:.1} with the full pair counts (ratio {ratio:.3} ± {sigma:.3}, {:+.2}σ; σ \
             the listed stars' scatter alone, since the fit tabulates no count error)",
            n / tabulated,
            (ratio - 1.0) / sigma,
        );
        if (ratio - 1.0).abs() > FINDING_SIGMAS * sigma {
            findings.push(format!(
                "{layer:?}: the ensemble count ratio under the full pair counts is {ratio:.3} \
                 ± {sigma:.3}"
            ));
        }
    }
    let trials = monte_carlo(&models);
    observer_medians(&realised, &trials);
    findings.extend(medians(&realised, &trials));
    findings
}

/// The eight observers' median of each ratio against the Monte Carlo's distribution of it:
/// prints them and returns the findings among the layers' ratios within [`RADIUS_LY`].
fn medians(realised: &[[f64; RATIOS]], trials: &[Vec<[f64; RATIOS]>]) -> Vec<String> {
    let mut findings = Vec::new();
    for i in 0..RATIOS {
        let mut ours: Vec<f64> = realised.iter().map(|r| r[i]).collect();
        let observed = median(&mut ours);
        let mut one: Vec<f64> = trials.iter().flatten().map(|r| r[i]).collect();
        let single = median(&mut one);
        let mut of_eight: Vec<f64> = trials
            .iter()
            .map(|t| median(&mut t.iter().map(|r| r[i]).collect::<Vec<_>>()))
            .collect();
        let expected = median(&mut of_eight);
        let (low, high) = (
            quantile(&of_eight, 0.158_655),
            quantile(&of_eight, 0.841_345),
        );
        if high <= low {
            // A layer with no light expected (the brown dwarfs) reads 1 in every trial.
            continue;
        }
        // The tail's share of the trials at or below the observed median, held off 0 and 1 by half
        // a trial, as a normal deviate: the test's statistic (the science check, 2026-10-07).
        let n = index_f64(of_eight.len());
        let below = of_eight.iter().filter(|&&m| m <= observed).count();
        let tail = ((index_f64(below)).clamp(0.5, n - 0.5)) / n;
        let z = math::normal_quantile(tail);
        let half = if observed < expected {
            expected - low
        } else {
            high - expected
        };
        eprintln!(
            "median light ratio, {}: the eight observers' {observed:.3} (each {}); the Monte \
             Carlo's one observer {single:.3} (interquartile {:.3}–{:.3}), eight observers' \
             median {expected:.3} (16–84% {low:.3}–{high:.3}); {below} of {} trials at or below, \
             {z:+.2}σ by the tail ({:+.2}σ by the half-width)",
            ratio_name(i),
            realised
                .iter()
                .map(|r| format!("{:.3}", r[i]))
                .collect::<Vec<_>>()
                .join(", "),
            quantile(&one, 0.25),
            quantile(&one, 0.75),
            of_eight.len(),
            (observed - expected) / half,
        );
        if i < LAYERS && z < -FINDING_SIGMAS {
            findings.push(format!(
                "{}: the median light ratio {observed:.3} lies {z:.2}σ below the skew's \
                 {expected:.3}",
                ratio_name(i)
            ));
        }
    }
    findings
}

/// Prints each observer's light ratios beside its own Monte Carlo's median and interquartile
/// range: each layer's within [`RADIUS_LY`], then every layer's within it and within
/// [`RULING_LY`], so that the Sun's place reads against its own skew.
fn observer_medians(realised: &[[f64; RATIOS]], trials: &[Vec<[f64; RATIOS]>]) {
    for (o, ours) in realised.iter().enumerate() {
        let columns: Vec<String> = (0..LAYERS)
            .chain([2 * LAYERS, 2 * LAYERS + 2])
            .filter_map(|i| {
                let mut v: Vec<f64> = trials.iter().map(|t| t[o][i]).collect();
                let m = median(&mut v);
                let (q1, q3) = (quantile(&v, 0.25), quantile(&v, 0.75));
                (q3 > q1).then(|| {
                    format!(
                        "{} {:.3} (median {m:.3}, interquartile {q1:.3}–{q3:.3})",
                        ratio_name(i),
                        ours[i]
                    )
                })
            })
            .collect();
        eprintln!(
            "observer {o} against its own Monte Carlo: {}",
            columns.join("; ")
        );
    }
}

#[test]
#[ignore = "slow: realises some 1.7 × 10⁶ systems at the solar circle and eight censuses within 300 ly"]
fn tables_against_the_realised_sky() {
    let galaxy = galaxy();
    let [tables, full, single] = tables(&galaxy);
    let read = Read {
        galaxy: &galaxy,
        tables: &tables,
        full: &full,
        single: &single,
    };
    let mut findings: Vec<String> = BLOCKS
        .into_iter()
        .filter_map(|blocks| paired_deficit(&read, blocks))
        .collect();
    findings.extend(eight_observers(&read));
    // Recorded, not asserted (decided 2026-10-06): each is a finding for the tables lane.
    eprintln!(
        "findings for T5.d's fit ({}): {findings:#?}",
        findings.len()
    );
}
