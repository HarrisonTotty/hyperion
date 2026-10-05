//! The pair-evolved light: what binary evolution changes in the luminosity tables' light and
//! counts (rendering plan R06, R06.T5.d; decided 2026-10-03, `decision-r06-tables.md`).
//!
//! The tables ([`super::luminosity`]) evolve every star alone, while the census reads each
//! system's pair-evolved stars (plan 11, P11.T11). R06.T5.c found the pair-evolved light 9% and
//! 21% below the single-star light in the solar circle's layers C and D: interaction strips and
//! merges giants. This module holds the correction for it, in two halves:
//!
//! - **The fit's sampling** ([`fit_galaxy`], [`system_difference`], [`cell_sums`]). For each
//!   cell, a layer of [`LAYERS`], an \[Fe/H\] node of [`FE_H_NODES`] and a log-age bin
//!   ([`age_edge`]), systems are drawn by the generator's own laws: the primary from the default
//!   mass function within the layer's band and the age log-uniform within the bin, on a
//!   low-discrepancy sequence ([`sample_point`]), and the companions and orbits from plan 11's
//!   laws through [`SystemStars::generate_with`] in the fit's own galaxy ([`GALAXY_SEED`]). Each
//!   system is read pair-evolved, `state_at(t).stars()`, and single, each
//!   [`StarModel`](crate::stellar::system::StarModel) alone; the difference, binned in 1-mag
//!   M<sub>V</sub> bins ([`MAGNITUDE_BINS`]), is the paired comparison of T5.c, so it has little
//!   noise. `hyperion-fit`'s tasks `sky_binary_light_c`, `_d` and `_e` average it per born
//!   system and write `tables::sky_binary_light_c`, `_d` and `_e`, one file a layer to keep each
//!   under the repository's 500 kB.
//! - **The build's correction** (`correction` and `apply`, crate-private). For a component bin
//!   and snapshot, the born-weighted sum over the age bins of the fitted differences, linear in
//!   \[Fe/H\] between nodes and over the component's three Gauss–Hermite nodes. Each 1-mag bin's
//!   difference in light and colour is spread over its 20 sub-bins in proportion to the
//!   single-star light there (evenly where there is none) and the light of a sub-bin is held at
//!   no less than zero; the counts take only the increase, each edge holding the larger of the
//!   single and corrected counts, made non-decreasing. A galaxy whose mass function is not the
//!   default takes no correction (a deviation recorded in the plan).
//!
//! Ages below 10⁵ years (protostars, dark) and above 1.5 × 10¹⁰ years take no correction. The
//! envelope and the caps' rule bound are not touched: they bound the brightest star, which binary
//! evolution can only brighten (R06.T16.b).
//!
//! The build draws nothing. The fit's sampling generates systems as the census does, under its own
//! galaxy's seed, and changes no generated output.

use std::ops::Range;

use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::imf::{MassBand, MassFunctionKind};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord};
use crate::id::{Layer, SystemId};
use crate::math;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::SystemStars;
use crate::stellar::{Composition, StarState};
use crate::tables::{sky_binary_light_c, sky_binary_light_d, sky_binary_light_e};
use crate::time::UniverseTime;
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

use super::luminosity::{MAGNITUDE_BINS as SUB_BINS, Seen};

/// The layers whose light pair evolution changes to first order (T5.c): A, B and the brown dwarfs
/// moved by 0–1%.
pub const LAYERS: [Layer; 3] = [Layer::C, Layer::D, Layer::E];

/// The \[Fe/H\] nodes of the fit, dex. A metallicity between two is read linearly between them;
/// one below −2 is read at −2, and none lies above +0.18, the tracks' clamp (Z 0.03).
pub const FE_H_NODES: [f64; 5] = [-2.0, -1.0, -0.5, 0.0, 0.18];

/// log₁₀ of the youngest age of the fit's bins, years: younger stars are Class 0 protostars, dark
/// in V ([`super::photometry`]; the protostar phase runs to 5 × 10⁵ years,
/// [`PROTOSTAR_DURATION`](crate::stellar::premain::PROTOSTAR_DURATION)), and too few to matter.
pub const FIRST_AGE_LOG10: f64 = 5.0;

/// The width of an age bin, dex.
pub const AGE_STEP_DEX: f64 = 0.2;

/// The oldest age of the fit's bins, years: the last bin runs from 10¹⁰ years to it.
pub const LAST_AGE_YEARS: f64 = 1.5e10;

/// The age bins: 25 of 0.2 dex from 10⁵ to 10¹⁰ years, then one to 1.5 × 10¹⁰.
pub const AGE_BINS: usize = 26;

/// The cells of one layer's table: per \[Fe/H\] node, its age bins.
pub const CELLS: usize = FE_H_NODES.len() * AGE_BINS;

/// The 1-mag M<sub>V</sub> bins over the tables' range, −12 to +20; a star brighter is counted in
/// the first and one fainter in the last.
pub const MAGNITUDE_BINS: usize = 32;

/// The tables' 0.05-mag bins in one of the fit's 1-mag bins.
const SUB_BINS_PER_BIN: usize = SUB_BINS / MAGNITUDE_BINS;

const _: () = assert!(
    SUB_BINS == MAGNITUDE_BINS * SUB_BINS_PER_BIN,
    "the 1-mag bins hold whole sub-bins"
);

/// The values of a cell's bin, each a mean per born system of pair-evolved less single-evolved:
/// the V light (L☉,V), the four colour sums of `luminosity`'s bins (the V light times
/// `lux_per_v0`, times that and each chroma channel, and times that and ρ), and the star count.
pub const VALUES: usize = 6;

/// The index of the star count among a bin's [`VALUES`].
const COUNT: usize = 5;

/// The seed of the fit's galaxy, whose streams draw the companions and their orbits: the fit's
/// own, not any galaxy's the game generates.
pub const GALAXY_SEED: u64 = 0x5b1a_0005_0000_5eed;

/// The most systems a cell can average, 2²⁴: the candidate indices set aside for each cell. A
/// cell's systems are its first indices, so the fit's sample of a cell is the same systems
/// whatever its size, and no two cells share a system.
pub const MAX_CELL_SYSTEMS: u32 = 1 << 24;

/// [`MAX_CELL_SYSTEMS`] as the stride between cells' candidate indices.
const CELL_STRIDE: u64 = MAX_CELL_SYSTEMS as u64;

/// The y coordinate of the fit's systems and of the cells their IDs come from, ly: the solar
/// circle's, where only the potential's tidal limit on wide orbits reads the position.
const SOLAR_Y_LY: f64 = 26_000.0;

/// The R2 sequence's increments, 2⁶⁴ ÷ φ₂ and 2⁶⁴ ÷ φ₂², with φ₂ the plastic number
/// 1.324 717 957 244 746, the real root of x³ = x + 1 (Roberts, M. 2018, "The Unreasonable
/// Effectiveness of Quasirandom Sequences", Extreme Learning, 25 April 2018,
/// extremelearning.com.au/unreasonable-effectiveness-of-quasirandom-sequences/): rounded to the
/// nearest integer, so the sequence is exact integer arithmetic.
const R2_INCREMENTS: [u64; 2] = [0xc13f_a9a9_02a6_328f, 0x91e1_0da5_c79e_7b1d];

/// 2⁻⁵², exact.
const TWO_TO_MINUS_52: f64 = 1.0 / 4_503_599_627_370_496.0;

/// The fit's galaxy: the Milky Way-like parameters under [`GALAXY_SEED`].
///
/// # Panics
///
/// Never: the Milky Way-like gas is mostly neutral.
#[must_use]
pub fn fit_galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(GALAXY_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like gas is mostly neutral")
}

/// The `k`-th edge of the age bins, years, for `k` in 0–[`AGE_BINS`]: 10^(5 + 0.2 k), and
/// [`LAST_AGE_YEARS`] for the last.
///
/// # Panics
///
/// If `k` is beyond [`AGE_BINS`].
#[must_use]
pub fn age_edge(k: usize) -> f64 {
    assert!(k <= AGE_BINS, "age edge {k} of {AGE_BINS}");
    if k == AGE_BINS {
        return LAST_AGE_YEARS;
    }
    let k = u32::try_from(k).expect("at most 26");
    math::exp10(FIRST_AGE_LOG10 + AGE_STEP_DEX * f64::from(k))
}

/// The point in (0, 1)² of the `n`-th system of the sequence: the R2 sequence's from ½ (Roberts'
/// seed point, his term 0), in exact integer arithmetic, each coordinate at the middle of its 2⁻⁵²
/// step, exactly.
#[must_use]
pub fn sample_point(n: u64) -> [f64; 2] {
    R2_INCREMENTS.map(|step| {
        let word = (1_u64 << 63).wrapping_add(n.wrapping_mul(step));
        #[expect(
            clippy::cast_precision_loss,
            reason = "the top 52 bits of a word are exact in an f64, and so is that plus a half"
        )]
        let top = (word >> 12) as f64;
        (top + 0.5) * TWO_TO_MINUS_52
    })
}

/// The \[Fe/H\] node and the age bin of `cell` (node-major, as the tables lay them out).
#[must_use]
fn cell_parts(cell: usize) -> (usize, usize) {
    (cell / AGE_BINS, cell % AGE_BINS)
}

/// The composition at `fe_h`, with no helium excess: the tables' reference compositions.
#[must_use]
fn composition_at(fe_h: f64) -> Composition {
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO)
}

/// The ID of the `n`-th candidate of `layer`, walking cells along +x from the solar circle's.
fn candidate(layer: Layer, n: u64) -> SystemId {
    let size = f64::from(layer.cell_size_ly());
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the solar circle's cell row is a few hundred cells out"
    )]
    let row = (SOLAR_Y_LY / size).floor() as i32;
    let first = CellKey::new(layer, [0, row, 0]).expect("a cell on the solar circle");
    let capacity = u64::from(first.index_capacity());
    let along = i32::try_from(n / capacity).expect("a few hundred cells along +x at most");
    let key = CellKey::new(layer, [along, row, 0]).expect("a cell inside the root cube");
    key.candidate_id(u32::try_from(n % capacity).expect("below the index capacity"))
        .expect("inside the index field")
}

/// The record of the fit's `index`-th system of `cell` of `layer` in `galaxy`, and its
/// composition.
///
/// # Panics
///
/// If `layer` is not a stellar layer, `cell` is not below [`CELLS`], `index` is not below
/// [`MAX_CELL_SYSTEMS`] or the galaxy has no component.
#[must_use]
pub fn fit_record(
    galaxy: &Galaxy,
    layer: Layer,
    cell: usize,
    index: u32,
) -> (SystemRecord, Composition) {
    assert!(cell < CELLS, "cell {cell} of {CELLS}");
    assert!(
        index < MAX_CELL_SYSTEMS,
        "system {index} of a cell's {MAX_CELL_SYSTEMS}"
    );
    let band = MassBand::from(layer);
    assert!(band.is_stellar(), "{layer:?} is not a stellar layer");
    let (fe_h, age_bin) = cell_parts(cell);
    let n = u64::try_from(cell).expect("a few hundred cells") * CELL_STRIDE + u64::from(index);
    let [u, v] = sample_point(n);
    let mass = galaxy.mass_function().quantile_in(band.lo(), band.hi(), u);
    let (lo, hi) = (
        math::log10(age_edge(age_bin)),
        math::log10(age_edge(age_bin + 1)),
    );
    let age = math::exp10(lo + (hi - lo) * v);
    let component = galaxy
        .fields()
        .component_id(0)
        .expect("a galaxy has components");
    let at = GalacticPosition::from_light_years([0.0, SOLAR_Y_LY, 0.0])
        .expect("the solar circle is inside the root cube");
    let record = SystemRecord::from_parts(
        candidate(layer, n),
        at,
        SystemOrigin::Grid(component),
        galaxy.fields().component(component).population(),
        SolarMasses::new(mass),
        Years::new(age),
    );
    (record, composition_at(FE_H_NODES[fe_h]))
}

/// One system's pair-evolved less single-evolved light, colour sums and count in each 1-mag bin
/// ([`VALUES`]), and its total V light so.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemDifference {
    /// Per 1-mag bin, the differences.
    pub bins: [[f64; VALUES]; MAGNITUDE_BINS],
    /// The total V light, pair-evolved less single-evolved, L☉,V.
    pub light: f64,
}

/// Adds `state`'s light, colour sums and one star to its 1-mag bin of `bins`, if it shines.
fn add_star(bins: &mut [[f64; VALUES]; MAGNITUDE_BINS], state: &StarState) {
    let (bin, light, colour) = match Seen::of(state) {
        Seen::Bin { bin, light, colour } => (usize::from(bin) / SUB_BINS_PER_BIN, light, colour),
        Seen::Beyond { light, colour } => (MAGNITUDE_BINS - 1, light, colour),
        Seen::Dark | Seen::Remnant => return,
    };
    let b = &mut bins[bin];
    b[0] += light;
    for (sum, c) in b[1..COUNT].iter_mut().zip(colour) {
        *sum += light * c;
    }
    b[COUNT] += 1.0;
}

/// The difference pair evolution makes to the fit's `index`-th system of `cell` of `layer`
/// ([`fit_record`]) at its age: its stars as `SystemStars::state_at` gives them less each
/// [`StarModel`](crate::stellar::system::StarModel) alone, both at the epoch.
///
/// # Panics
///
/// As [`fit_record`] does.
#[must_use]
pub fn system_difference(
    galaxy: &Galaxy,
    layer: Layer,
    cell: usize,
    index: u32,
) -> SystemDifference {
    let (record, composition) = fit_record(galaxy, layer, cell, index);
    let stars =
        SystemStars::generate_with(galaxy, &record, &composition, MultiplicityContext::Free);
    let mut pair = [[0.0; VALUES]; MAGNITUDE_BINS];
    let mut single = [[0.0; VALUES]; MAGNITUDE_BINS];
    if let Some(state) = stars.state_at(UniverseTime::EPOCH) {
        for star in state.stars() {
            add_star(&mut pair, star);
        }
    }
    for model in stars.stars() {
        if let Some(state) = model.state_at(UniverseTime::EPOCH) {
            add_star(&mut single, &state);
        }
    }
    let mut bins = [[0.0; VALUES]; MAGNITUDE_BINS];
    let (mut pair_light, mut single_light) = (0.0, 0.0);
    for ((out, p), s) in bins.iter_mut().zip(&pair).zip(&single) {
        for ((o, a), b) in out.iter_mut().zip(p).zip(s) {
            *o = a - b;
        }
        pair_light += p[0];
        single_light += s[0];
    }
    SystemDifference {
        bins,
        light: pair_light - single_light,
    }
}

/// The sums over some systems of one cell: of each bin's differences, and of the total light's
/// difference and its square.
#[derive(Debug, Clone, PartialEq)]
pub struct CellSums {
    systems: u64,
    changed: u64,
    bins: [[f64; VALUES]; MAGNITUDE_BINS],
    light: f64,
    light_squared: f64,
}

impl Default for CellSums {
    fn default() -> Self {
        Self {
            systems: 0,
            changed: 0,
            bins: [[0.0; VALUES]; MAGNITUDE_BINS],
            light: 0.0,
            light_squared: 0.0,
        }
    }
}

impl CellSums {
    /// Adds one system.
    pub fn add_system(&mut self, d: &SystemDifference) {
        self.systems += 1;
        self.changed += u64::from(d.bins.iter().flatten().any(|v| v.abs() > 0.0));
        for (sum, add) in self.bins.iter_mut().zip(&d.bins) {
            for (s, a) in sum.iter_mut().zip(add) {
                *s += a;
            }
        }
        self.light += d.light;
        self.light_squared += d.light * d.light;
    }

    /// Adds the sums of other systems of the cell, after these.
    pub fn merge(&mut self, other: &Self) {
        self.systems += other.systems;
        self.changed += other.changed;
        for (sum, add) in self.bins.iter_mut().zip(&other.bins) {
            for (s, a) in sum.iter_mut().zip(add) {
                *s += a;
            }
        }
        self.light += other.light;
        self.light_squared += other.light_squared;
    }

    /// The systems summed.
    #[must_use]
    pub const fn systems(&self) -> u64 {
        self.systems
    }

    /// The systems summed whose stars pair evolution changed: any bin's difference not zero.
    #[must_use]
    pub const fn changed(&self) -> u64 {
        self.changed
    }

    /// Each bin's mean difference per system; zeros for no system.
    #[must_use]
    pub fn mean(&self) -> [[f64; VALUES]; MAGNITUDE_BINS] {
        if self.systems == 0 {
            return [[0.0; VALUES]; MAGNITUDE_BINS];
        }
        let n = self.count();
        self.bins.map(|b| b.map(|v| v / n))
    }

    /// The standard error of the mean difference in total V light, L☉,V: the sample's standard
    /// deviation over √n; zero for fewer than two systems.
    #[must_use]
    pub fn light_standard_error(&self) -> f64 {
        if self.systems < 2 {
            return 0.0;
        }
        let n = self.count();
        let variance = ((self.light_squared - self.light * self.light / n) / (n - 1.0)).max(0.0);
        (variance / n).sqrt()
    }

    fn count(&self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a cell's systems, some 10⁴, are exact in f64"
        )]
        let n = self.systems as f64;
        n
    }
}

/// The sums of the fit's systems `systems` of `cell` of `layer`, in index order.
///
/// # Panics
///
/// As [`fit_record`] does.
#[must_use]
pub fn cell_sums(galaxy: &Galaxy, layer: Layer, cell: usize, systems: Range<u32>) -> CellSums {
    let mut sums = CellSums::default();
    for index in systems {
        sums.add_system(&system_difference(galaxy, layer, cell, index));
    }
    sums
}

/// One layer's fitted table, as `tables::sky_binary_light_<layer>` holds it.
#[derive(Debug, Clone, Copy)]
struct Fitted {
    /// The first 1-mag bin the table holds.
    first_bin: usize,
    /// The bins it holds from there.
    bins: usize,
    /// Per cell, per bin held, the mean differences ([`VALUES`]), cell-major.
    delta: &'static [[f64; VALUES]],
    /// Per cell, the standard error of its mean difference in total V light, L☉,V.
    light_se: &'static [f64],
}

impl Fitted {
    /// The table of `layer`, if pair evolution is corrected there.
    fn of(layer: Layer) -> Option<Self> {
        let (first_bin, delta, light_se): (usize, &'static [[f64; VALUES]], &'static [f64]) =
            match layer {
                Layer::C => (
                    sky_binary_light_c::FIRST_BIN,
                    sky_binary_light_c::DELTA.as_flattened(),
                    &sky_binary_light_c::LIGHT_SE,
                ),
                Layer::D => (
                    sky_binary_light_d::FIRST_BIN,
                    sky_binary_light_d::DELTA.as_flattened(),
                    &sky_binary_light_d::LIGHT_SE,
                ),
                Layer::E => (
                    sky_binary_light_e::FIRST_BIN,
                    sky_binary_light_e::DELTA.as_flattened(),
                    &sky_binary_light_e::LIGHT_SE,
                ),
                Layer::A | Layer::B | Layer::BrownDwarf | Layer::RoguePlanet => return None,
            };
        Some(Self {
            first_bin,
            bins: delta.len() / CELLS,
            delta,
            light_se,
        })
    }
}

/// Whether `galaxy`'s tables take the correction: the fit drew its primaries from the default
/// mass function, so a galaxy of another takes none.
#[must_use]
pub(crate) fn applies_to(galaxy: &Galaxy) -> bool {
    galaxy.params().mass_function() == MassFunctionKind::default()
}

/// The correction of one layer's function at one snapshot: per 1-mag bin, the born-weighted sum
/// of the fitted differences ([`VALUES`]), and the standard error of their total light.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Correction {
    delta: [[f64; VALUES]; MAGNITUDE_BINS],
    sigma: f64,
}

/// The node below `fe_h` among [`FE_H_NODES`] and the weight of the one above: held at the ends.
#[must_use]
fn fe_h_bracket(fe_h: f64) -> (usize, f64) {
    let last = FE_H_NODES.len() - 1;
    if fe_h.is_nan() || fe_h <= FE_H_NODES[0] {
        return (0, 0.0);
    }
    if fe_h >= FE_H_NODES[last] {
        return (last, 0.0);
    }
    let k = FE_H_NODES.partition_point(|&node| node <= fe_h) - 1;
    (
        k,
        (fe_h - FE_H_NODES[k]) / (FE_H_NODES[k + 1] - FE_H_NODES[k]),
    )
}

/// The correction of `layer` for a component bin whose metallicity nodes are `metallicities`
/// (\[Fe/H\] and weight each) and whose born systems with ages between `lo` and `hi` at the
/// snapshot are the share `weight_of(lo, hi)`: `None` for a layer without a table.
#[must_use]
pub(crate) fn correction(
    layer: Layer,
    weight_of: &impl Fn(f64, f64) -> f64,
    metallicities: &[(f64, f64)],
) -> Option<Correction> {
    Fitted::of(layer).map(|table| correction_from(&table, weight_of, metallicities))
}

#[must_use]
fn correction_from(
    table: &Fitted,
    weight_of: &impl Fn(f64, f64) -> f64,
    metallicities: &[(f64, f64)],
) -> Correction {
    let edges: [f64; AGE_BINS + 1] = std::array::from_fn(age_edge);
    let weights: [f64; AGE_BINS] = std::array::from_fn(|b| weight_of(edges[b], edges[b + 1]));
    // Each cell's coefficient first, so that two nodes reading one cell count its error once.
    let mut coefficients = [0.0; CELLS];
    for &(fe_h, share) in metallicities {
        let (k, t) = fe_h_bracket(fe_h);
        for (b, &w) in weights.iter().enumerate() {
            if w <= 0.0 {
                continue;
            }
            coefficients[k * AGE_BINS + b] += share * w * (1.0 - t);
            if t > 0.0 {
                coefficients[(k + 1) * AGE_BINS + b] += share * w * t;
            }
        }
    }
    let mut delta = [[0.0; VALUES]; MAGNITUDE_BINS];
    let mut variance = 0.0;
    for (cell, &c) in coefficients.iter().enumerate() {
        if c <= 0.0 {
            continue;
        }
        let rows = &table.delta[cell * table.bins..(cell + 1) * table.bins];
        for (out, row) in delta[table.first_bin..].iter_mut().zip(rows) {
            for (o, v) in out.iter_mut().zip(row) {
                *o += c * v;
            }
        }
        let s = c * table.light_se[cell];
        variance += s * s;
    }
    Correction {
        delta,
        sigma: variance.sqrt(),
    }
}

/// What a correction did to one snapshot of a function, L☉,V per system.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Applied {
    /// The change in the total light: the pair-evolved light less the single-star light.
    pub(crate) light: f64,
    /// The standard error of the fitted change in total light.
    pub(crate) sigma: f64,
    /// The light the clamp at zero kept: how far the sub-bins would have gone below it.
    pub(crate) clamped: f64,
}

/// Applies `c` to one snapshot's bins (`luminosity`'s 0.05-mag bins from the brightest edge: the
/// light, the counts and the colour sums), returning what it did.
pub(crate) fn apply(
    c: &Correction,
    light: &mut [f64],
    count: &mut [f64],
    colour: &mut [[f64; 4]],
) -> Applied {
    let mut applied = Applied {
        sigma: c.sigma,
        ..Applied::default()
    };
    // Light and colour: each 1-mag bin's difference over its sub-bins by their light.
    for (j, d) in c.delta.iter().enumerate() {
        let sub = j * SUB_BINS_PER_BIN..(j + 1) * SUB_BINS_PER_BIN;
        let total = light[sub.clone()].iter().fold(0.0, |a, &l| a + l);
        for k in sub {
            #[expect(clippy::cast_precision_loss, reason = "20 sub-bins, exact in f64")]
            let share = if total > 0.0 {
                light[k] / total
            } else {
                1.0 / SUB_BINS_PER_BIN as f64
            };
            let was = light[k];
            let now = was + share * d[0];
            if now < 0.0 {
                applied.clamped -= now;
                light[k] = 0.0;
                colour[k] = [0.0; 4];
            } else {
                light[k] = now;
                for (sum, dc) in colour[k].iter_mut().zip(&d[1..COUNT]) {
                    *sum = (*sum + share * dc).max(0.0);
                }
            }
            applied.light += light[k] - was;
        }
    }
    // Counts: the increase only, edge by edge from the brightest, then non-decreasing.
    let (mut single, mut corrected, mut held, mut previous) = (0.0, 0.0, 0.0_f64, 0.0);
    for (j, d) in c.delta.iter().enumerate() {
        let sub = j * SUB_BINS_PER_BIN..(j + 1) * SUB_BINS_PER_BIN;
        let total = count[sub.clone()].iter().fold(0.0, |a, &n| a + n);
        for k in sub {
            #[expect(clippy::cast_precision_loss, reason = "20 sub-bins, exact in f64")]
            let share = if total > 0.0 {
                count[k] / total
            } else {
                1.0 / SUB_BINS_PER_BIN as f64
            };
            single += count[k];
            corrected += count[k] + share * d[COUNT];
            held = held.max(single.max(corrected));
            count[k] = held - previous;
            previous = held;
        }
    }
    applied
}

/// Each cell's part of `layer`'s correction error for a component bin, as [`correction`] weighs
/// the cells: its coefficient times its standard error. The tests name the cells that carry a
/// failing bin's error, where the fit's systems are raised (decided 2026-10-04, "T5.d gate
/// reading").
#[cfg(test)]
pub(crate) fn cell_errors(
    layer: Layer,
    weight_of: &impl Fn(f64, f64) -> f64,
    metallicities: &[(f64, f64)],
) -> Vec<(usize, f64)> {
    let table = Fitted::of(layer).expect("a table");
    let edges: [f64; AGE_BINS + 1] = std::array::from_fn(age_edge);
    let weights: [f64; AGE_BINS] = std::array::from_fn(|b| weight_of(edges[b], edges[b + 1]));
    let mut coefficients = [0.0; CELLS];
    for &(fe_h, share) in metallicities {
        let (k, t) = fe_h_bracket(fe_h);
        for (b, &w) in weights.iter().enumerate() {
            if w <= 0.0 {
                continue;
            }
            coefficients[k * AGE_BINS + b] += share * w * (1.0 - t);
            if t > 0.0 {
                coefficients[(k + 1) * AGE_BINS + b] += share * w * t;
            }
        }
    }
    (0..CELLS)
        .map(|c| (c, coefficients[c] * table.light_se[c]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_age_bins_run_from_a_tenth_of_a_megayear_to_the_oldest_star() {
        assert!((age_edge(0) - 1e5).abs() < 1e-9);
        assert!((age_edge(25) / 1e10 - 1.0).abs() < 1e-14);
        assert!((age_edge(AGE_BINS) - LAST_AGE_YEARS).abs() < 1e-3);
        assert!((1..=AGE_BINS).all(|k| age_edge(k) > age_edge(k - 1)));
    }

    #[test]
    fn the_sample_points_fill_the_unit_square_evenly() {
        // The R2 sequence: every one of the 10 × 10 squares holds 100 ± 3 of 10⁴ points.
        let mut counts = [[0_u32; 10]; 10];
        for n in 0..10_000 {
            let [u, v] = sample_point(n);
            assert!(u > 0.0 && u < 1.0 && v > 0.0 && v < 1.0);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "u and v lie in [0, 1)"
            )]
            let (i, j) = ((u * 10.0) as usize, (v * 10.0) as usize);
            counts[i][j] += 1;
        }
        assert!(
            counts.iter().flatten().all(|&c| c.abs_diff(100) <= 3),
            "{counts:?}"
        );
        assert!(sample_point(0).iter().all(|&u| (u - 0.5).abs() < 1e-15));
    }

    #[test]
    fn metallicities_read_linearly_between_nodes_and_are_held_at_the_ends() {
        let near = |(k, t): (usize, f64), (k0, t0): (usize, f64)| k == k0 && (t - t0).abs() < 1e-15;
        assert!(near(fe_h_bracket(-2.3), (0, 0.0)));
        assert!(near(fe_h_bracket(0.176), (3, 0.176 / 0.18)));
        assert!(near(fe_h_bracket(0.18), (4, 0.0)));
        assert!(near(fe_h_bracket(-0.75), (1, 0.5)));
    }

    /// A table of one bin at bin 10 whose every cell holds `cell + 1` in light and colour, one star,
    /// and the standard error 0.1.
    fn synthetic() -> Fitted {
        static DELTA: [[f64; VALUES]; CELLS] = {
            let mut rows = [[0.0; VALUES]; CELLS];
            let mut cell = 0;
            while cell < CELLS {
                #[expect(clippy::cast_precision_loss, reason = "a few hundred cells")]
                let v = (cell + 1) as f64;
                rows[cell] = [v, v, v, v, v, 1.0];
                cell += 1;
            }
            rows
        };
        static SE: [f64; CELLS] = [0.1; CELLS];
        Fitted {
            first_bin: 10,
            bins: 1,
            delta: &DELTA,
            light_se: &SE,
        }
    }

    #[test]
    fn a_correction_weighs_the_cells_by_their_born_share_and_metallicity() {
        let table = synthetic();
        // Every system born in age bin 3; [Fe/H] −0.75, half-way between nodes 1 and 2.
        let weight_of = |lo: f64, hi: f64| {
            if lo >= age_edge(3) && hi <= age_edge(4) {
                1.0
            } else {
                0.0
            }
        };
        let c = correction_from(&table, &weight_of, &[(-0.75, 1.0)]);
        #[expect(clippy::cast_precision_loss, reason = "a few hundred cells")]
        let expected = 0.5 * (AGE_BINS + 3 + 1) as f64 + 0.5 * (2 * AGE_BINS + 3 + 1) as f64;
        assert!(
            (c.delta[10][0] - expected).abs() < 1e-12,
            "{:?}",
            c.delta[10]
        );
        assert!((c.delta[10][COUNT] - 1.0).abs() < 1e-15);
        assert!(
            c.delta[9]
                .iter()
                .chain(&c.delta[11])
                .all(|v| v.abs() < 1e-300),
            "only bin 10 is held"
        );
        assert!(
            (c.sigma - 0.1 * math::hypot(0.5, 0.5)).abs() < 1e-15,
            "{}",
            c.sigma
        );
        // Two nodes on one cell count its error once: the sum of their weights times it.
        let both = correction_from(&table, &weight_of, &[(0.0, 0.5), (0.0, 0.5)]);
        assert!((both.sigma - 0.1).abs() < 1e-15, "{}", both.sigma);
    }

    #[test]
    fn applying_spreads_by_light_clamps_at_zero_and_takes_only_the_count_increase() {
        let mut delta = [[0.0; VALUES]; MAGNITUDE_BINS];
        // Bin 1, dark, gains light and half a star; bin 2 loses more light than it has, and a star.
        delta[1] = [2.0, 2.0, 2.0, 2.0, 2.0, 0.5];
        delta[2] = [-3.0, -3.0, -3.0, -3.0, -3.0, -1.0];
        let c = Correction { delta, sigma: 0.25 };
        let mut light = vec![0.0; SUB_BINS];
        let mut count = vec![0.0; SUB_BINS];
        let mut colour = vec![[0.0; 4]; SUB_BINS];
        for k in 40..50 {
            light[k] = 0.2;
            count[k] = 1.0;
            colour[k] = [0.2; 4];
        }
        light[70] = 1.0;
        count[70] = 2.0;
        let applied = apply(&c, &mut light, &mut count, &mut colour);
        // Bin 1 had no light: its gain is spread evenly.
        assert!(light[20..40].iter().all(|&l| (l - 0.1).abs() < 1e-15));
        assert!(
            colour[20..40]
                .iter()
                .flatten()
                .all(|&s| (s - 0.1).abs() < 1e-15)
        );
        // Bin 2's 2.0 of light, all in its first ten sub-bins, would go to −1: clamped.
        assert!(light[40..60].iter().all(|&l| l.abs() < 1e-300));
        assert!(colour[40..60].iter().flatten().all(|&s| s.abs() < 1e-300));
        assert!((applied.clamped - 1.0).abs() < 1e-12, "{applied:?}");
        assert!(applied.light.abs() < 1e-12, "{applied:?}");
        assert!((applied.sigma - 0.25).abs() < 1e-15);
        assert!((light[70] - 1.0).abs() < 1e-15);
        // Counts: bin 1's half star is taken; bin 2's loss is not, and no edge falls back.
        let mut cumulative = 0.0;
        let mut edges = Vec::new();
        for &n in &count {
            assert!(n >= 0.0, "{count:?}");
            cumulative += n;
            edges.push(cumulative);
        }
        assert!((edges[39] - 0.5).abs() < 1e-12, "{}", edges[39]);
        assert!((edges[42] - 3.2).abs() < 1e-12, "{}", edges[42]);
        assert!((edges[49] - 10.0).abs() < 1e-12, "{}", edges[49]);
        assert!(
            (edges[SUB_BINS - 1] - 12.0).abs() < 1e-12,
            "{}",
            edges[SUB_BINS - 1]
        );
    }

    #[test]
    fn layers_without_a_table_take_no_correction() {
        for layer in [Layer::A, Layer::B, Layer::BrownDwarf, Layer::RoguePlanet] {
            assert!(correction(layer, &|_, _| 1.0, &[(0.0, 1.0)]).is_none());
        }
        for layer in LAYERS {
            let table = Fitted::of(layer).expect("a table");
            assert_eq!(table.delta.len(), CELLS * table.bins, "{layer:?}");
            assert_eq!(table.light_se.len(), CELLS, "{layer:?}");
            assert!(table.first_bin + table.bins <= MAGNITUDE_BINS, "{layer:?}");
        }
    }

    #[test]
    fn the_fit_galaxy_draws_from_the_default_mass_function() {
        assert!(applies_to(&fit_galaxy()));
    }

    #[test]
    fn a_system_difference_is_the_same_twice_and_zero_for_a_lone_star() {
        let galaxy = fit_galaxy();
        let cell = 3 * AGE_BINS + 22;
        let (mut lone, mut multiple) = (0, 0);
        for index in 0..40 {
            let a = system_difference(&galaxy, Layer::C, cell, index);
            assert_eq!(a, system_difference(&galaxy, Layer::C, cell, index));
            let summed = a.bins.iter().fold(0.0, |s, b| s + b[0]);
            assert!(
                (summed - a.light).abs() <= 1e-9 * (1.0 + a.light.abs()),
                "{a:?}"
            );
            let (record, composition) = fit_record(&galaxy, Layer::C, cell, index);
            let stars = SystemStars::generate_with(
                &galaxy,
                &record,
                &composition,
                MultiplicityContext::Free,
            );
            if stars.stars().len() == 1 {
                lone += 1;
                assert!(a.bins.iter().flatten().all(|v| v.abs() < 1e-300), "{a:?}");
            } else {
                multiple += 1;
            }
        }
        assert!(lone > 0 && multiple > 0, "{lone} lone, {multiple} multiple");
    }
}
