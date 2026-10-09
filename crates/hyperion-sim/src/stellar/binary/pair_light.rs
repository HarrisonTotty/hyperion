//! The pair-light tables: what a pair of two stars can hold at each age, measured on the binary
//! engine itself, for the census's pair verdicts `Unchanged`, `Remnants` and `Bright` (plan 11,
//! P11.T17.b; `decision-p11-t16-hierarchy-bound.md` §§5, 6 and 10).
//!
//! P11.T17.a's [`PairLight::Detached`](super::PairLight::Detached) is a closed form. Past it the
//! engine can make a living star long after both stars' own deaths (plan 11's P11.T17 scope check:
//! main-sequence donors eroded onto remnants, white-dwarf mergers, stripped stars' stretched
//! lives), so the other verdicts are read from these tables, sampled through [`evolve`] and
//! certified statistically, as the single-star envelopes are. T17.c reads them; this module holds
//! their grid, their sampling, their assembly and their reader.
//!
//! - **Cells** ([`PairGrid`]), one table a layer of [`LAYERS`]: the heavier star's initial mass in
//!   [`MASS_CELLS`] intervals even in ln m over the layer's band (plan 03's `MassBand`); the mass
//!   ratio q = m₂ ÷ m₁ in the intervals of [`Q_EDGES`], the first from the lightest companion,
//!   [`LOWEST_COMPANION_MSUN`] ÷ the band's top, even in ln q, the others even in q; the
//!   metallicity coordinate log₁₀(Z<sub>fit</sub> ÷ 0.02) of the reach table in the intervals of
//!   [`FE_H_EDGES`]; and the drawn periastron in bins of [`PERIASTRON_STEP_DEX`] from
//!   [`PERIASTRON_FIRST_RSUN`], then one open bin, sampled to [`OPEN_PERIASTRON_REACH_RSUN`]. Each
//!   cell holds [`AGE_BINS`] bins of age: one to [`FIRST_AGE_EDGE_YEARS`], then [`AGE_STEP_DEX`] in
//!   log age to 10^10.1 years, then one to [`LAST_AGE_YEARS`].
//! - **Samples** ([`sample_input`]): a cell's [`CORNER_SAMPLES`] corners, then points of Roberts'
//!   R₄ low-discrepancy sequence inside it. Each pair's eccentricity follows Moe and Di Stefano's
//!   (2017) law at its period ([`MultiplicityModel::eccentricity_distribution`]), with circular and
//!   e = [`HIGH_ECCENTRICITY`] extremes; each star's draws are the generator's for a body
//!   (`StarDraws::for_star`, under the fit's own seed), with Reimers η at ±3.5σ for some and the
//!   companion-stripped mark and low kick mode set for others; orientations are isotropic and
//!   phases uniform.
//! - **What a pair holds** ([`pair_rows`]): its timeline from [`evolve`] to [`LAST_AGE_YEARS`]
//!   walked as rendering plan R06's brightness envelope walks a track. Each star's own single-star
//!   model, and each member of each segment that is not its own model, is cut at the segment's and
//!   its phases' boundaries, [`SAMPLES_PER_PHASE`] parts a phase, its knots, its paths' steps,
//!   [`SEGMENT_SPLITS`] log-even parts of a segment and the age bins' edges, and read at five
//!   points a part. Per age bin the pair holds:
//!   - **living**: the brightest V of any living star, pair-evolved or its own single-star model;
//!   - **changed**: the brightest V of any living star whose state is not its own model's at that
//!     age (a departing star: an eroded donor, a gainer, a common-envelope survivor) or that has no
//!     own model alive then (a product: a merger, a stripped star, a white-dwarf merger's star).
//!
//!   A star on its own track bit for bit (the first segment's track, at no offset) is its own
//!   model and is not read again. The brightness is the caller's photometry: the fit passes the
//!   sky's ([`sky::photometry::absolute_v_of_state`](crate::sky::photometry::absolute_v_of_state)).
//! - **Values** ([`assemble`]): each cell's brightest over its samples; cumulative over every wider
//!   periastron bin, so that a cell bounds every orbit of a periastron at least its own; dilated by
//!   one cell along each of the four axes and one age bin; brightened by [`MARGIN_MAG`]; runs of
//!   bins within a tolerance of their brightest merged at it ([`merge_tolerance_mag`] for the changed
//!   stars, [`PairGrid::living_merge_tolerance_mag`] for the living ones, which only tell where
//!   nothing lives),
//!   a closer orbit again made no fainter than a wider one and the living value no fainter than
//!   the changed one; stored in integer hundredths of a magnitude rounded brighter ([`to_cmag`]), [`DARK_CMAG`]
//!   where nothing lives and [`UNSEEN_CMAG`] where something lives that has no V (a protostar, or
//!   a star cooler than the photometry's tables). Each cell also keeps, per age bin, the class of
//!   how many of its own samples hold a changed star, before the cumulation and the dilation
//!   ([`count_class`]: 0 for none, then one class a power of two), and the table its samples per
//!   cell, so that T17.c can tell an undersampled or borderline cell (§10's safeguards).
//! - **E's reading** ([`PairGrid::read_margin_cmag`], [`PairGrid::read_smear_bins`]): E's reader
//!   brightens every finite value by a further [`E_READ_MARGIN_CMAG`], 0.7 mag in all, since E's
//!   held-out samples and the slow test's pairs reached 0.69 and 0.53 mag past their widened
//!   cells; and takes each bin's brightest over [`E_READ_SMEAR_BINS`] more bins either way, since
//!   a product made when the companion leaves its main sequence comes at an age set by the
//!   companion's lifetime, which E's low-q cells span by a dex.
//! - **The cooling floor** ([`cooling_floor_cmag`], [`with_cooling_floor`]). A star below
//!   0.1 M☉ follows P06.T13's cooling fits, which are brightest just below 0.1 M☉, where they meet
//!   the tracks, and the samples, whose companions are held at no less than
//!   [`LOWEST_COMPANION_MSUN`], seldom fall there: the slow test found such stars up to 0.2 mag
//!   past their cells' values. The engine evaluates such a member at its own age and at no more
//!   than 0.1 M☉, however much wind it accretes, so no such star is brighter than the cooling fit
//!   at 0.1 M☉ at that age. For a pair whose lighter star lies below 0.1 M☉ the reader takes each
//!   value that is not DARK as the brighter of it and that floor, margin included.
//! - **The held floor** (`held_floor`, crate-private; P11.T17.c's follow-up). A timeline capped
//!   at the engine's [`MAX_SEGMENTS`](super::MAX_SEGMENTS) holds a star the binary carries on its
//!   main sequence for ever, which the tables' samples seldom catch. The floor is the brightest hydrogen or
//!   helium main-sequence star of at most a pair's total mass, from the engine's own models, from
//!   the first age bin at which any star of that mass can have ended its main sequence. P11.T17.c's
//!   bound reads every living and changed value through it, DARK included, wherever the pair may
//!   have interacted by its window's end.
//! - **Storage**, format [`FORMAT`]: the distinct rows of a cell's bins (living, changed or count
//!   classes) packed as segments of a run of bins (6 bits) and a value (12 bits), three base64
//!   characters a segment, and per cell its three rows' indices, three characters each
//!   ([`PairLightCells::pack`]), so that each table stays under the repository's 500 kB, as
//!   R06.T8.m's phase envelope packs its rows (as plain decimals a table would be some 2 MB).
//!   [`PairLightTable::generator`] decodes a layer's table once.
//!
//! The decoded tables and the two floors are built lazily, once a process (`OnceLock` statics),
//! each a pure function of the crate's constants, the same whichever is asked first (the
//! sim-determinism skill's order independence; a test holds each to a fresh build).
//!
//! The tables depend on no galaxy. `hyperion-fit`'s tasks `binary_pair_light_c`, `_d` and `_e`
//! build them from [`sample_input`], [`pair_rows`] and [`assemble`], validate them on held-out
//! samples of every cell, and check them in as `tables::binary_pair_light_c`, `_d` and `_e`. The
//! slow test `the_pair_light_tables_bound_evolved_pairs` holds them to pairs on a seed of its own.
//! They are statistical: a rare channel left unsampled in a cell's neighbourhood could hide a
//! brighter star (the ruling's §10), which the margins, the dilation, T17.c's `None` for thin cells
//! and the slow tests guard against.

use std::sync::{Arc, OnceLock};

use crate::Seed;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::CellKey;
use crate::id::{BodyId, Layer};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::rng::Mark;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::multiplicity::MultiplicityModel;
use crate::stellar::photometry::absolute_magnitude_v;
use crate::stellar::sse::{self, MIN_INITIAL_MASS, ZCoeffs};
use crate::stellar::{Composition, StarState, substellar};
use crate::tables::{binary_pair_light_c, binary_pair_light_d, binary_pair_light_e};
use crate::units::consts::{GM_SUN, SECONDS_PER_DAY, SOLAR_RADIUS_M};
use crate::units::{
    Days, Dex, GravitationalParameter, HeliumExcess, Magnitudes, Metres, Radians, SolarMasses,
    Years,
};

use super::evolve::{evolve, own_members};
use super::reach::FE_H_NODES;
use super::star::{Member, Path};
use super::timeline::{BinaryInput, BinaryTimeline, Context};

/// The layers with a table: those whose pairs the engine changes to first order (rendering plan
/// R06's T5.c), the same three as R06.T5.d's pair-light correction.
pub const LAYERS: [Layer; 3] = [Layer::C, Layer::D, Layer::E];

/// The intervals of the heavier star's initial mass in a layer's band, even in ln m: about 10%
/// of mass apart in C and D, so that the main-sequence lifetimes across a cell and its
/// neighbours, the cell's dilation, span some 0.25 dex, and 28% in E, whose records are old.
pub const MASS_CELLS: usize = 12;

/// The intervals of the mass ratio q = m₂ ÷ m₁.
pub const Q_CELLS: usize = 8;

/// The edges of the mass-ratio intervals.
///
/// The first edge stands for the layer's lightest companion, [`LOWEST_COMPANION_MSUN`] ÷ the band's top ([`PairGrid::q_edges`]); its interval,
/// up to plan 11's direct construction's least ratio, 0.1, holds 0.8–2.5% of the probe's realised
/// pairs and is even in ln q, the others even in q. The twins of q ≥ 0.95 lie in the last.
pub const Q_EDGES: [f64; Q_CELLS + 1] = [0.0, 0.1, 0.2, 0.3, 0.45, 0.6, 0.75, 0.9, 1.0];

/// The lightest star of a star–star pair, M☉: plan 11's stellar floor.
///
/// A lighter companion is a brown dwarf, whose pair `run_pairs` does not run.
pub const LOWEST_COMPANION_MSUN: f64 = 0.08;

/// The metallicity intervals.
pub const FE_H_CELLS: usize = 4;

/// The edges of the metallicity intervals, log₁₀(Z<sub>fit</sub> ÷ 0.02) in dex, the reach
/// table's coordinate: the tracks' clamps at the ends (Z = 10⁻⁴ and 0.03), and −1, −0.5 and
/// −0.15 between, so that the disc's metallicities, 97% of the probe's pairs from −0.5 up, take
/// two cells.
pub const FE_H_EDGES: [f64; FE_H_CELLS + 1] = [FE_H_NODES[0], -1.0, -0.5, -0.15, FE_H_NODES[7]];

/// The lower edge of the first periastron bin, R☉.
///
/// The generator's 0.1-day period floor puts C's lightest pairs (0.75 + 0.08 M☉) at 0.85 R☉ and
/// two 0.08 M☉ dwarfs at 0.49 R☉; such pairs, in contact at birth, fall below it and read `None`.
pub const PERIASTRON_FIRST_RSUN: f64 = 1.0;

/// The width of the closed periastron bins, dex.
pub const PERIASTRON_STEP_DEX: f64 = 0.25;

/// The widest periastron at which the open bin is sampled, R☉: about 0.2 pc, beyond the probe's
/// widest realised periastra. The open bin is read for any wider orbit too.
pub const OPEN_PERIASTRON_REACH_RSUN: f64 = 1.0e7;

/// The upper age of the first age bin, years: younger stars are protostars, dark in V.
pub const FIRST_AGE_EDGE_YEARS: f64 = 1.0e5;

/// The width of the log-age bins after the first, dex.
pub const AGE_STEP_DEX: f64 = 0.1;

/// The age every pair is run to, years, and the last bin's upper edge.
pub const LAST_AGE_YEARS: f64 = 1.5e10;

/// The age bins: the first, to [`FIRST_AGE_EDGE_YEARS`], then 51 of [`AGE_STEP_DEX`] to 10^10.1
/// years, then one to [`LAST_AGE_YEARS`].
pub const AGE_BINS: usize = 53;

/// The margin every finite magnitude is brightened by, mag: the brightness envelope's (R06).
pub const MARGIN_MAG: f64 = 0.3;

/// The further margin layer E's reader takes, hundredths of a magnitude: 0.4 mag, for 0.7 mag in
/// all.
///
/// E's massive stars' light changes fastest between samples: its fit's held-out samples
/// reached 0.69 mag past their widened cells (C's 0.10 and D's 0.03), and the slow test's own
/// pairs 0.53 mag. Read, not stored, so that E's fitted table stands; the version-22 refit stores
/// it.
pub const E_READ_MARGIN_CMAG: i16 = 40;

/// The further age bins either way over which layer E's reader takes each bin's brightest: 3,
/// ±0.3 dex.
///
/// A product made when the companion leaves its main sequence (a giant merging with the
/// primary's white dwarf, briefly at M<sub>V</sub> −7.6, in the slow test's realised pair of 8.4
/// and 1.3 M☉) comes at an age set by the companion's lifetime, which spans about a dex across
/// E's low-q cells, so the cells' samples hit its brief bright phases at scattered bins. Read, not
/// stored; the version-22 refit takes it into the fit.
pub const E_READ_SMEAR_BINS: usize = 3;

/// The merge tolerance's base for the changed stars, mag: a run of bins whose magnitudes lie
/// within it of the run's brightest is stored as one value, the brightest.
pub const MERGE_TOLERANCE_MAG: f64 = 0.2;

/// The merge tolerance's base for the living stars, mag: wider, since T17.c reads the living
/// value only where nothing lives (`Remnants`), and the living rows, each star's own brightness
/// over its life, would otherwise be the tables' largest part.
pub const LIVING_MERGE_TOLERANCE_MAG: f64 = 1.0;

/// Layer E's living merge tolerance's base, mag: wider still, so that E's table, whose massive
/// stars' light changes fastest, stays well under the 500 kB rule (D's, at
/// [`LIVING_MERGE_TOLERANCE_MAG`], packs to 420 kB).
pub const LIVING_MERGE_TOLERANCE_E_MAG: f64 = 2.0;

/// The magnitude past which the merge tolerance grows, mag: only near stars this faint can be
/// listed.
pub const FAINT_FROM_MAG: f64 = 10.0;

/// How fast, mag a mag, the merge tolerance grows past [`FAINT_FROM_MAG`].
pub const FAINT_TOLERANCE_SLOPE: f64 = 0.2;

/// The stored value of a bin where nothing lives: the largest of a stored value's 12 bits.
pub const DARK_CMAG: i16 = 2_047;

/// The stored value of a bin where something lives that has no V: one below [`DARK_CMAG`], so
/// that the brightest of two values is still the lesser.
pub const UNSEEN_CMAG: i16 = 2_046;

/// The faintest stored magnitude, hundredths of a magnitude: +20.45.
///
/// A fainter star is stored at it, brightened, which only near stars of layer A's companions could reach.
pub const FAINTEST_CMAG: i16 = 2_045;

/// The brightest stored magnitude, hundredths of a magnitude: −20.48, the least of 12 bits, far
/// past any star's M<sub>V</sub>.
pub const BRIGHTEST_CMAG: i16 = -2_048;

/// A living star that has no V, as a magnitude: fainter than any V, brighter than nothing
/// (`+∞`), so that the brightest of two is still the lesser.
pub const UNSEEN_MAG: f64 = f64::MAX;

/// The seed of the fit's own draws: the stars' `StarDraws` and the orbits' words.
///
/// No galaxy the game generates uses it.
pub const FIT_SEED: u64 = 0x5eed_0017_b0b0_f17e;

/// The samples of a cell taken at its corners: 2⁴, every combination of each axis's two edges.
pub const CORNER_SAMPLES: u32 = 16;

/// The parts each phase of a track is cut into, besides its knots, as the brightness envelope
/// cuts it.
pub const SAMPLES_PER_PHASE: u32 = 32;

/// The log-even parts each segment of a timeline is cut into, so that a long segment whose member
/// has no track (a main-sequence star the binary carries) is read along it.
pub const SEGMENT_SPLITS: u32 = 8;

/// The high-eccentricity extreme of the samples.
pub const HIGH_ECCENTRICITY: f64 = 0.9;

/// The largest eccentricity a sample takes from the law.
pub const MAX_SAMPLED_ECCENTRICITY: f64 = 0.95;

/// The η extremes some samples take, as standard-normal draws: ±3.5σ.
pub const ETA_EXTREME: f64 = 3.5;

/// The system age at the epoch every sample takes, years.
///
/// The engine reads it only to place the stars on their orbit at a supernova, with the mean
/// anomaly at the epoch, which the samples draw uniformly, so any value samples the same phases.
const SAMPLE_AGE_AT_EPOCH_YEARS: f64 = 1.0e10;

/// The version of the fitted tables' format, which [`PairLightTable::generator`] checks: 1, the
/// packed rows and cell rows of [`PairLightCells::pack`].
pub const FORMAT: u32 = 1;

/// The R₄ sequence's increments, 2⁶⁴ ÷ φ₄ⁱ for i = 1–4, rounded to the nearest integer, so that
/// the sequence is exact integer arithmetic.
///
/// φ₄ = 1.167 303 978 261 418 7 is the positive root of x⁵ = x + 1 (Roberts, M. 2018, "The
/// Unreasonable Effectiveness of Quasirandom Sequences", Extreme Learning; the page is now read at
/// its Wayback Machine copy, web.archive.org/web/20241231143848/https://extremelearning.com.au/
/// unreasonable-effectiveness-of-quasirandom-sequences/).
const R4_INCREMENTS: [u64; 4] = [
    0xdb4f_0b91_75ae_2165,
    0xbbe0_5633_03a4_615f,
    0xa0f2_ec75_a1fe_1576,
    0x89e1_8285_7d9e_d689,
];

/// 2⁻⁵², exact.
const TWO_TO_MINUS_52: f64 = 1.0 / 4_503_599_627_370_496.0;

// --- The grid --------------------------------------------------------------------------------

/// The cells of one layer's table (see the [module](self) documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairGrid {
    layer: Layer,
    mass_lo_msun: f64,
    mass_hi_msun: f64,
    periastron_cells: usize,
}

impl PairGrid {
    /// The grid of `layer`'s table, or `None` for a layer without one (not in [`LAYERS`]).
    #[must_use]
    pub fn of(layer: Layer) -> Option<Self> {
        // The closed periastron bins reach past each layer's widest departing pair in P11.T17.b's
        // probe (2,068, 3,079 and 13,656 R☉ in C, D and E) by at least 0.35 dex.
        let closed = match layer {
            Layer::C => 15,
            Layer::D => 16,
            Layer::E => 18,
            Layer::A | Layer::B | Layer::BrownDwarf | Layer::RoguePlanet => return None,
        };
        let band = MassBand::of_layer(layer);
        Some(Self {
            layer,
            mass_lo_msun: band.lo(),
            mass_hi_msun: band.hi(),
            periastron_cells: closed + 1,
        })
    }

    /// The layer.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// The periastron bins, the open one included.
    #[must_use]
    pub const fn periastron_cells(&self) -> usize {
        self.periastron_cells
    }

    /// The further margin the layer's reader takes, hundredths of a magnitude:
    /// [`E_READ_MARGIN_CMAG`] in E, none in C and D.
    #[must_use]
    pub const fn read_margin_cmag(&self) -> i16 {
        match self.layer {
            Layer::E => E_READ_MARGIN_CMAG,
            Layer::A | Layer::B | Layer::C | Layer::D | Layer::BrownDwarf | Layer::RoguePlanet => 0,
        }
    }

    /// The further age bins either way over which the layer's reader takes each bin's brightest:
    /// [`E_READ_SMEAR_BINS`] in E, none in C and D.
    #[must_use]
    pub const fn read_smear_bins(&self) -> usize {
        match self.layer {
            Layer::E => E_READ_SMEAR_BINS,
            Layer::A | Layer::B | Layer::C | Layer::D | Layer::BrownDwarf | Layer::RoguePlanet => 0,
        }
    }

    /// The base of the living stars' merge tolerance, mag: [`LIVING_MERGE_TOLERANCE_MAG`], and
    /// [`LIVING_MERGE_TOLERANCE_E_MAG`] in E.
    #[must_use]
    pub const fn living_merge_base_mag(&self) -> f64 {
        match self.layer {
            Layer::E => LIVING_MERGE_TOLERANCE_E_MAG,
            Layer::A | Layer::B | Layer::C | Layer::D | Layer::BrownDwarf | Layer::RoguePlanet => {
                LIVING_MERGE_TOLERANCE_MAG
            }
        }
    }

    /// The living stars' merge tolerance, mag, for a run whose brightest bin is `v`.
    #[must_use]
    pub fn living_merge_tolerance_mag(&self, v: f64) -> f64 {
        self.living_merge_base_mag() + FAINT_TOLERANCE_SLOPE * (v - FAINT_FROM_MAG).max(0.0)
    }

    /// The cells: mass × mass ratio × metallicity × periastron intervals.
    #[must_use]
    pub const fn cells(&self) -> usize {
        MASS_CELLS * Q_CELLS * FE_H_CELLS * self.periastron_cells
    }

    /// The cell of the intervals `parts` = [mass, mass ratio, metallicity, periastron], mass-major.
    ///
    /// # Panics
    ///
    /// If a part is past its axis.
    #[must_use]
    pub fn cell_index(&self, parts: [usize; 4]) -> usize {
        let [m, q, f, p] = parts;
        assert!(
            m < MASS_CELLS && q < Q_CELLS && f < FE_H_CELLS && p < self.periastron_cells,
            "cell parts {parts:?} are inside the grid"
        );
        ((m * Q_CELLS + q) * FE_H_CELLS + f) * self.periastron_cells + p
    }

    /// The intervals [mass, mass ratio, metallicity, periastron] of `cell`.
    ///
    /// # Panics
    ///
    /// If `cell` is past the grid.
    #[must_use]
    pub fn cell_parts(&self, cell: usize) -> [usize; 4] {
        assert!(cell < self.cells(), "cell {cell} of {}", self.cells());
        let p = cell % self.periastron_cells;
        let rest = cell / self.periastron_cells;
        let f = rest % FE_H_CELLS;
        let rest = rest / FE_H_CELLS;
        [rest / Q_CELLS, rest % Q_CELLS, f, p]
    }

    /// The edges of mass interval `k`, M☉: even in ln m over the layer's band, the last exactly
    /// the band's top.
    ///
    /// # Panics
    ///
    /// If `k` is past the axis.
    #[must_use]
    pub fn mass_edges_msun(&self, k: usize) -> (f64, f64) {
        assert!(k < MASS_CELLS, "mass interval {k} of {MASS_CELLS}");
        let edge = |n: usize| {
            if n == MASS_CELLS {
                return self.mass_hi_msun;
            }
            let n = u32::try_from(n).expect("a dozen intervals");
            let cells = u32::try_from(MASS_CELLS).expect("a dozen intervals");
            self.mass_lo_msun
                * math::exp(
                    math::ln(self.mass_hi_msun / self.mass_lo_msun) * f64::from(n)
                        / f64::from(cells),
                )
        };
        (edge(k), edge(k + 1))
    }

    /// The edges of mass-ratio interval `k`: [`Q_EDGES`], the first from the layer's lightest
    /// companion, [`LOWEST_COMPANION_MSUN`] ÷ the band's top.
    ///
    /// # Panics
    ///
    /// If `k` is past the axis.
    #[must_use]
    pub fn q_edges(&self, k: usize) -> (f64, f64) {
        assert!(k < Q_CELLS, "mass-ratio interval {k} of {Q_CELLS}");
        let lo = if k == 0 {
            LOWEST_COMPANION_MSUN / self.mass_hi_msun
        } else {
            Q_EDGES[k]
        };
        (lo, Q_EDGES[k + 1])
    }

    /// The edges of metallicity interval `k`, dex: [`FE_H_EDGES`].
    ///
    /// # Panics
    ///
    /// If `k` is past the axis.
    #[must_use]
    pub fn fe_h_edges(&self, k: usize) -> (f64, f64) {
        assert!(k < FE_H_CELLS, "metallicity interval {k} of {FE_H_CELLS}");
        (FE_H_EDGES[k], FE_H_EDGES[k + 1])
    }

    /// The edges of periastron bin `k`, R☉: 10^(0.25 k) R☉ onwards, the open bin's from its lower
    /// edge to [`OPEN_PERIASTRON_REACH_RSUN`], the reach of its samples.
    ///
    /// # Panics
    ///
    /// If `k` is past the axis.
    #[must_use]
    pub fn periastron_edges_rsun(&self, k: usize) -> (f64, f64) {
        assert!(
            k < self.periastron_cells,
            "periastron bin {k} of {}",
            self.periastron_cells
        );
        let edge = |n: usize| {
            let n = u32::try_from(n).expect("a few dozen bins");
            PERIASTRON_FIRST_RSUN * math::exp10(PERIASTRON_STEP_DEX * f64::from(n))
        };
        if k + 1 == self.periastron_cells {
            (edge(k), OPEN_PERIASTRON_REACH_RSUN)
        } else {
            (edge(k), edge(k + 1))
        }
    }

    /// The cell holding a pair, or `None` outside the table.
    ///
    /// The pair has initial masses `heavier_msun` and `lighter_msun` (M☉), metallicity coordinate
    /// `fe_h` (dex, log₁₀(Z<sub>fit</sub> ÷ 0.02), held inside the tracks' clamps) and drawn
    /// periastron `periastron_rsun` (R☉). Outside the table means: a heavier
    /// star outside the layer's band, a lighter one below [`LOWEST_COMPANION_MSUN`] or above the
    /// heavier, a periastron below [`PERIASTRON_FIRST_RSUN`], or a value that is not a number. A
    /// value on an edge is read in the interval above it; a periastron past the open bin's samples
    /// is read in the open bin.
    #[must_use]
    pub fn cell_of(
        &self,
        heavier_msun: f64,
        lighter_msun: f64,
        fe_h: f64,
        periastron_rsun: f64,
    ) -> Option<usize> {
        self.cell_with(
            |k| self.mass_edges_msun(k).0,
            |k| self.periastron_edges_rsun(k).0,
            [heavier_msun, lighter_msun, fe_h, periastron_rsun],
        )
    }

    /// [`Self::cell_of`] of `[heavier, lighter, fe_h, periastron]`, with the lower edges of mass
    /// interval and periastron bin k ≥ 1 given by `mass_edge` and `periastron_edge`: computed for
    /// the grid, or read from [`PairLightTable`]'s copy of the same values.
    #[must_use]
    fn cell_with(
        &self,
        mass_edge: impl Fn(usize) -> f64,
        periastron_edge: impl Fn(usize) -> f64,
        [heavier_msun, lighter_msun, fe_h, periastron_rsun]: [f64; 4],
    ) -> Option<usize> {
        let in_band = heavier_msun >= self.mass_lo_msun && heavier_msun <= self.mass_hi_msun;
        let paired = lighter_msun >= LOWEST_COMPANION_MSUN && lighter_msun <= heavier_msun;
        if !in_band
            || !paired
            || fe_h.is_nan()
            || periastron_rsun.is_nan()
            || periastron_rsun < PERIASTRON_FIRST_RSUN
        {
            return None;
        }
        let m = (1..MASS_CELLS)
            .take_while(|&k| heavier_msun >= mass_edge(k))
            .last()
            .unwrap_or(0);
        let q_value = lighter_msun / heavier_msun;
        let q = (1..Q_CELLS)
            .take_while(|&k| q_value >= Q_EDGES[k])
            .last()
            .unwrap_or(0);
        let fe = fe_h.clamp(FE_H_EDGES[0], FE_H_EDGES[FE_H_CELLS]);
        let f = (1..FE_H_CELLS)
            .take_while(|&k| fe >= FE_H_EDGES[k])
            .last()
            .unwrap_or(0);
        let p = (1..self.periastron_cells)
            .take_while(|&k| periastron_rsun >= periastron_edge(k))
            .last()
            .unwrap_or(0);
        Some(self.cell_index([m, q, f, p]))
    }
}

/// The bin holding `age_years`, or `None` past [`LAST_AGE_YEARS`] or for an age that is not a
/// number; a negative age is the first bin's.
#[must_use]
pub fn age_bin(age_years: f64) -> Option<usize> {
    if age_years.is_nan() || age_years > LAST_AGE_YEARS {
        return None;
    }
    if age_years < FIRST_AGE_EDGE_YEARS {
        return Some(0);
    }
    let steps = (math::log10(age_years) - math::log10(FIRST_AGE_EDGE_YEARS)) / AGE_STEP_DEX;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative number of steps below a hundred, held below the bins"
    )]
    let k = steps.floor().clamp(0.0, 1.0e3) as usize;
    Some((k + 1).min(AGE_BINS - 1))
}

/// The lower edge of age bin `k`, years: 0 for the first, then 10^(5 + 0.1 (k − 1)); and for
/// `k` = [`AGE_BINS`], [`LAST_AGE_YEARS`], the last bin's upper edge.
///
/// # Panics
///
/// If `k` is past [`AGE_BINS`].
#[must_use]
pub fn age_edge_years(k: usize) -> f64 {
    assert!(k <= AGE_BINS, "age edge {k} of {AGE_BINS}");
    match k {
        0 => 0.0,
        AGE_BINS => LAST_AGE_YEARS,
        _ => {
            let k = u32::try_from(k - 1).expect("a few dozen bins");
            math::exp10(math::log10(FIRST_AGE_EDGE_YEARS) + AGE_STEP_DEX * f64::from(k))
        }
    }
}

// --- The samples -----------------------------------------------------------------------------

/// A splitmix64 stream: the fit's own words for a sample's orbit, never the generator's streams,
/// so that the sampling adds no tag.
struct Words(u64);

impl Words {
    /// The stream of sample `index` of `cell` of `layer`'s table under `seed`.
    #[must_use]
    fn of(seed: u64, layer: Layer, cell: usize, index: u32) -> Self {
        let cell = u64::try_from(cell).expect("a cell index fits 64 bits");
        Self(seed ^ (u64::from(layer.value()) << 56) ^ (cell << 24) ^ u64::from(index))
    }

    fn word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in (0, 1): the top 52 bits of a word and a half step, exact.
    fn unit(&mut self) -> f64 {
        top_unit(self.word())
    }
}

/// `word`'s top 52 bits plus a half step, times 2⁻⁵²: in (0, 1), exact.
#[must_use]
fn top_unit(word: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the top 52 bits of a word are exact in an f64, and so is that plus a half"
    )]
    let top = (word >> 12) as f64;
    (top + 0.5) * TWO_TO_MINUS_52
}

/// The point in [0, 1]⁴ of sample `index` of a cell: for the first [`CORNER_SAMPLES`] the corner
/// whose axis d is at its upper edge where bit d of the index is set, then the R₄ sequence's from
/// ½ (Roberts' seed point), in exact integer arithmetic.
#[must_use]
pub(crate) fn unit_point(index: u32) -> [f64; 4] {
    if index < CORNER_SAMPLES {
        return core::array::from_fn(|d| f64::from((index >> d) & 1));
    }
    let n = u64::from(index - CORNER_SAMPLES);
    R4_INCREMENTS.map(|step| top_unit((1_u64 << 63).wrapping_add(n.wrapping_mul(step))))
}

/// `lo` + (`hi` − `lo`) `u`, exactly an edge at `u` = 0 or 1.
#[must_use]
fn lerp(lo: f64, hi: f64, u: f64) -> f64 {
    if u <= 0.0 {
        lo
    } else if u >= 1.0 {
        hi
    } else {
        (lo + (hi - lo) * u).clamp(lo, hi)
    }
}

/// `lo` (`hi` ÷ `lo`)^`u`, exactly an edge at `u` = 0 or 1.
#[must_use]
fn log_lerp(lo: f64, hi: f64, u: f64) -> f64 {
    if u <= 0.0 {
        lo
    } else if u >= 1.0 {
        hi
    } else {
        (lo * math::exp(u * math::ln(hi / lo))).clamp(lo, hi)
    }
}

/// The period, days, of an orbit of semi-major axis `a_rsun` (R☉) about a total mass of
/// `total_msun` (M☉): Kepler's third law.
#[must_use]
fn period_days(a_rsun: f64, total_msun: f64) -> f64 {
    let a = a_rsun * SOLAR_RADIUS_M;
    std::f64::consts::TAU * (a * a * a / (GM_SUN * total_msun)).sqrt() / SECONDS_PER_DAY
}

/// A sample's eccentricity: circular for every fourth sample from the second, and
/// [`HIGH_ECCENTRICITY`] for every eighth from the fourth, the extremes; otherwise Moe and Di
/// Stefano's law at quantile `u`, at the period of the orbit of periastron `periastron_rsun` about
/// the heavier star of `m1` (M☉) and the pair's `total` (M☉), the period and the eccentricity
/// solved together, at most [`MAX_SAMPLED_ECCENTRICITY`].
#[must_use]
fn sample_eccentricity(index: u32, u: f64, m1: f64, total: f64, periastron_rsun: f64) -> f64 {
    if index % 4 == 1 {
        return 0.0;
    }
    if index % 8 == 3 {
        return HIGH_ECCENTRICITY;
    }
    let model = MultiplicityModel::default_v1();
    let mut e: f64 = 0.0;
    for _ in 0..4 {
        let period = period_days(periastron_rsun / (1.0 - e), total);
        e = model
            .eccentricity_distribution(SolarMasses::new(m1), Days::new(period))
            .quantile(u)
            .clamp(0.0, MAX_SAMPLED_ECCENTRICITY);
    }
    e
}

/// The draws of star `star` (0 the heavier) of sample `index` of `cell` of `layer`'s table: the
/// generator's draws of a body (`StarDraws::for_star` under `seed`, of candidate `index` of a cell
/// near the origin numbered as the table's cell), with the extremes: η at +3.5σ for every eighth
/// sample from the third, −3.5σ from the seventh, and the companion-stripped mark and the low
/// kick mode set from the eighth.
#[must_use]
fn sample_draws(seed: u64, layer: Layer, cell: usize, index: u32, star: u16) -> StarDraws {
    let c = i32::try_from(cell).expect("a table's cells number a few thousand");
    let key = CellKey::new(layer, [c % 16, (c / 16) % 16, c / 256])
        .expect("a cell within some 20 kly of the origin is inside the root cube");
    let id = key
        .candidate_id(index)
        .expect("a cell's samples number far below a layer's index field");
    let draws = StarDraws::for_star(Seed::new(seed), BodyId::new(id, star));
    let parts = draws.parts();
    let eta = |z: f64| StandardNormal::new(z).expect("a finite extreme");
    match index % 8 {
        2 => StarDraws::from_parts(StarDrawsParts {
            eta: eta(ETA_EXTREME),
            ..parts.clone()
        }),
        6 => StarDraws::from_parts(StarDrawsParts {
            eta: eta(-ETA_EXTREME),
            ..parts.clone()
        }),
        7 => StarDraws::from_parts(StarDrawsParts {
            stripped: Mark::from_word(0),
            kick_mode: Mark::from_word(0),
            ..parts.clone()
        }),
        _ => draws,
    }
}

/// Sample `index` of `cell` of `grid`'s table under `seed` (see the [module](self)
/// documentation): the heavier star first.
///
/// # Panics
///
/// If `cell` is past the grid, which a caller's loop over [`PairGrid::cells`] rules out.
#[must_use]
pub fn sample_input(grid: &PairGrid, cell: usize, index: u32, seed: u64) -> BinaryInput {
    let [mi, qi, fi, pi] = grid.cell_parts(cell);
    let u = unit_point(index);
    let (m_lo, m_hi) = grid.mass_edges_msun(mi);
    let m1 = log_lerp(m_lo, m_hi, u[0]);
    let (q_lo, q_hi) = grid.q_edges(qi);
    let q = if qi == 0 {
        log_lerp(q_lo, q_hi, u[1])
    } else {
        lerp(q_lo, q_hi, u[1])
    };
    let m2 = (q * m1).clamp(LOWEST_COMPANION_MSUN, m1);
    let (f_lo, f_hi) = grid.fe_h_edges(fi);
    let fe_h = lerp(f_lo, f_hi, u[2]);
    let (p_lo, p_hi) = grid.periastron_edges_rsun(pi);
    let periastron = log_lerp(p_lo, p_hi, u[3]);
    let mut words = Words::of(seed, grid.layer(), cell, index);
    let e = sample_eccentricity(index, words.unit(), m1, m1 + m2, periastron);
    let tau = std::f64::consts::TAU;
    let orientation = Orientation::new(
        Radians::new(math::acos(1.0 - 2.0 * words.unit())),
        Radians::new(tau * words.unit()),
        Radians::new(tau * words.unit()),
    )
    .expect("an inclination inside [0, π] and angles inside a turn");
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::new(periastron / (1.0 - e) * SOLAR_RADIUS_M),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::new(e).expect("an eccentricity inside [0, 0.95]"),
        orientation,
        Radians::new(tau * words.unit()),
    )
    .expect("a positive axis about a positive mass");
    let draws = [0, 1].map(|star| sample_draws(seed, grid.layer(), cell, index, star));
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
        orbit,
        draws,
        Years::new(SAMPLE_AGE_AT_EPOCH_YEARS),
    )
    .expect("a table's stars are of 0.08-150 M_sun")
}

// --- What a pair holds -----------------------------------------------------------------------

/// What one pair holds at each age bin, as magnitudes.
///
/// A bin holds +∞ where nothing lives, and [`UNSEEN_MAG`] where something lives that has no V (see
/// the [module](self) documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct PairRows {
    /// Per age bin, the brightest V of any living star, pair-evolved or its own single-star model,
    /// mag.
    living: [f64; AGE_BINS],
    /// Per age bin, the brightest V of any living star that departs from its own single-star
    /// model, or product, mag.
    changed: [f64; AGE_BINS],
}

impl PairRows {
    /// Nothing at any age.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            living: [f64::INFINITY; AGE_BINS],
            changed: [f64::INFINITY; AGE_BINS],
        }
    }

    /// Per age bin, the brightest V of any living star, pair-evolved or its own single-star model,
    /// mag.
    #[must_use]
    pub const fn living_mag(&self) -> &[f64; AGE_BINS] {
        &self.living
    }

    /// Per age bin, the brightest V of any living star that departs from its own single-star
    /// model, or product, mag.
    #[must_use]
    pub const fn changed_mag(&self) -> &[f64; AGE_BINS] {
        &self.changed
    }

    /// Whether some bin holds a changed star.
    #[must_use]
    pub fn changes(&self) -> bool {
        self.changed.iter().any(|v| v.is_finite())
    }
}

/// What the pair of `input` holds at each age bin, from its timeline run to [`LAST_AGE_YEARS`].
///
/// The timeline comes from [`evolve`] and is walked as the [module](self) documentation says.
/// `magnitude` is the absolute V of a state, `None` where it has none; the fit passes the sky's.
///
/// # Panics
///
/// - If `magnitude` gives a value that is not finite, so that the fit fails rather than write an
///   unsound cell.
/// - In debug builds only, where the binary engine's own debug assertion `rlof.rs`'s "a transfer
///   step from a donor with nothing living" fires: about 1 in 3.9 × 10⁴ of E's table samples, a
///   deferred engine finding (`deferred-corrections.md`). Release builds run on.
///
/// # Examples
///
/// A Sun-like star 5,000 au from a red dwarf: the pre-test passes the pair over, so it holds only
/// its stars' own models: nothing changed at any age, and a living star whose V is the Sun's
/// near 4.6 Gyr.
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::sky::photometry::absolute_v_of_state;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::binary::BinaryInput;
/// use hyperion_sim::stellar::binary::pair_light::{age_bin, pair_rows};
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{GravitationalParameter, Metres, Radians, SolarMasses, Years};
///
/// let orbit = KeplerElements::from_semi_major_axis(
///     Metres::new(5_000.0 * METRES_PER_AU),
///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.5)),
///     Eccentricity::CIRCULAR,
///     Orientation::new(Radians::new(0.5), Radians::new(0.0), Radians::new(0.0))?,
///     Radians::new(0.0),
/// )?;
/// let input = BinaryInput::new(
///     SolarMasses::new(1.0),
///     SolarMasses::new(0.5),
///     Composition::SOLAR,
///     orbit,
///     [StarDraws::median(), StarDraws::median()],
///     Years::new(1.0e10),
/// )?;
/// let rows = pair_rows(&input, &absolute_v_of_state);
/// assert!(!rows.changes());
/// let now = age_bin(4.6e9).ok_or("a bin")?;
/// assert!((3.5..5.5).contains(&rows.living_mag()[now]));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn pair_rows(
    input: &BinaryInput,
    magnitude: &dyn Fn(&StarState) -> Option<Magnitudes>,
) -> PairRows {
    let timeline = evolve(input, Years::new(LAST_AGE_YEARS));
    rows_of(input, &timeline, magnitude)
}

/// [`pair_rows`] of `input`'s timeline `timeline`, run to [`LAST_AGE_YEARS`].
#[must_use]
fn rows_of(
    input: &BinaryInput,
    timeline: &BinaryTimeline,
    magnitude: &dyn Fn(&StarState) -> Option<Magnitudes>,
) -> PairRows {
    let seen = |state: &StarState| {
        magnitude(state).map_or(UNSEEN_MAG, |v| {
            assert!(v.value().is_finite(), "a magnitude is finite: {state:?}");
            v.value()
        })
    };
    let mut rows = PairRows::empty();
    let own = own_members(input, LAST_AGE_YEARS, None);
    let own_ctx = Context::of(input);
    let mut cuts = Vec::new();
    // The own models go into the living column, so that a pair the generator passes over at its
    // +H, where it holds only its own models, is bounded even where the long run departs earlier.
    for (slot, member) in own.iter().enumerate() {
        member_cuts(member, 0.0, LAST_AGE_YEARS, &mut cuts);
        for part in cuts.windows(2) {
            let mut v = f64::INFINITY;
            for t in part_points(part[0], part[1]) {
                let state = member.state_at(&own_ctx, slot, t);
                if state.phase().is_living() {
                    v = v.min(seen(&state));
                }
            }
            enter(&mut rows.living, part[0], part[1], v);
        }
    }
    // A member on the first segment's own track at no offset is its own model, read above; every
    // other form is read again and compared with its own model at each point.
    let ctx = timeline.context();
    let Some(first) = timeline.segments().first() else {
        return rows;
    };
    for segment in timeline.segments() {
        let (start, end) = (segment.start().value(), segment.end().value());
        if end <= start {
            continue;
        }
        let members = segment.members().iter().zip(first.members()).zip(&own);
        for (slot, ((member, first_member), own_member)) in members.enumerate() {
            if matches!(member, Member::Gone | Member::Remnant { .. })
                || is_own_track(member, first_member)
            {
                continue;
            }
            member_cuts(member, start, end, &mut cuts);
            for part in cuts.windows(2) {
                let mut v = f64::INFINITY;
                for t in part_points(part[0], part[1]) {
                    let state = member.state_at(ctx, slot, t);
                    // `StarState`'s derived equality compares every field by value (−0 and +0 alike;
                    // its debug assertions keep NaN out): a state equal to its own model's is no
                    // departure, and a near miss counted as one only brightens the bound.
                    if !state.phase().is_living() || state == own_member.state_at(&own_ctx, slot, t)
                    {
                        continue;
                    }
                    v = v.min(seen(&state));
                }
                enter(&mut rows.changed, part[0], part[1], v);
                enter(&mut rows.living, part[0], part[1], v);
            }
        }
    }
    rows
}

/// Whether `member` is the first segment's `first` member on its own track at no offset: the
/// star's own single-star model bit for bit.
#[must_use]
pub(super) fn is_own_track(member: &Member, first: &Member) -> bool {
    match (member, first) {
        (
            Member::Track { track, offset },
            Member::Track {
                track: own,
                offset: own_offset,
            },
        ) => {
            offset.total_cmp(&0.0).is_eq()
                && own_offset.total_cmp(&0.0).is_eq()
                && Arc::ptr_eq(track, own)
        }
        _ => false,
    }
}

/// The paths a member's form carries, whose steps cut it.
#[must_use]
fn member_paths(member: &Member) -> [Option<&Path>; 2] {
    match member {
        Member::Shaped { mass, .. }
        | Member::Cooling { mass, .. }
        | Member::Remnant { mass, .. } => [Some(mass), None],
        Member::MainSequence { mass, tau, .. } => [Some(mass), Some(tau)],
        Member::Track { .. } | Member::Frozen { .. } | Member::Gone => [None, None],
    }
}

/// The ages, in rising order, that cut `member` between `start` and `end` (years): the ends, the
/// age bins' edges, [`SEGMENT_SPLITS`] log-even parts, its track's phases' boundaries each cut into
/// [`SAMPLES_PER_PHASE`] parts and its knots, and its paths' steps.
pub(super) fn member_cuts(member: &Member, start: f64, end: f64, cuts: &mut Vec<f64>) {
    cuts.clear();
    cuts.push(start);
    cuts.push(end);
    cuts.extend(
        (1..AGE_BINS)
            .map(age_edge_years)
            .filter(|&t| t > start && t < end),
    );
    let (log_start, log_end) = (math::ln(start.max(1.0)), math::ln(end.max(1.0)));
    cuts.extend((1..SEGMENT_SPLITS).map(|k| {
        math::exp(log_start + (log_end - log_start) * f64::from(k) / f64::from(SEGMENT_SPLITS))
    }));
    if let Some((track, offset)) = member.track() {
        for (phase_start, phase_end, knots) in track.segment_ages() {
            let (a, b) = (
                (phase_start + offset).max(start),
                (phase_end + offset).min(end),
            );
            if b <= a {
                continue;
            }
            cuts.extend(
                (0..=SAMPLES_PER_PHASE)
                    .map(|k| a + (b - a) * f64::from(k) / f64::from(SAMPLES_PER_PHASE)),
            );
            cuts.extend(knots.map(|t| t + offset));
        }
    }
    for path in member_paths(member).into_iter().flatten() {
        cuts.extend(path.knots().iter().map(|knot| knot[0]));
    }
    cuts.retain(|&t| t >= start && t <= end);
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
}

/// The five points at which a part `a`–`b` is read: its ends and three between, each end just
/// inside the part.
pub(super) fn part_points(a: f64, b: f64) -> impl Iterator<Item = f64> {
    [0.0, 0.25, 0.5, 0.75, 1.0]
        .into_iter()
        .map(move |f| (a + (b - a) * f).clamp(a + (b - a) * 1e-9, b - (b - a) * 1e-9))
}

/// Enters `v` in the bin of the part `a`–`b` of `row` (each part lies inside one bin, its edges
/// being cuts): the brighter of the two.
fn enter(row: &mut [f64; AGE_BINS], a: f64, b: f64, v: f64) {
    if v.is_infinite() {
        return;
    }
    if let Some(bin) = age_bin(f64::midpoint(a, b)) {
        row[bin] = row[bin].min(v);
    }
}

// --- A cell's samples ------------------------------------------------------------------------

/// The samples of one cell summed: each bin's brightest living and changed star, and how many
/// samples hold a changed star there.
#[derive(Debug, Clone, PartialEq)]
pub struct CellRows {
    samples: u32,
    living: [f64; AGE_BINS],
    changed: [f64; AGE_BINS],
    departing: [u32; AGE_BINS],
}

impl Default for CellRows {
    fn default() -> Self {
        Self {
            samples: 0,
            living: [f64::INFINITY; AGE_BINS],
            changed: [f64::INFINITY; AGE_BINS],
            departing: [0; AGE_BINS],
        }
    }
}

impl CellRows {
    /// Adds one sample.
    pub fn add(&mut self, rows: &PairRows) {
        self.samples += 1;
        for k in 0..AGE_BINS {
            self.living[k] = brighter(self.living[k], rows.living[k]);
            self.changed[k] = brighter(self.changed[k], rows.changed[k]);
            self.departing[k] += u32::from(rows.changed[k] < f64::INFINITY);
        }
    }

    /// Adds another sum of samples of the cell: the result does not depend on the order.
    pub fn merge(&mut self, other: &Self) {
        self.samples += other.samples;
        for k in 0..AGE_BINS {
            self.living[k] = brighter(self.living[k], other.living[k]);
            self.changed[k] = brighter(self.changed[k], other.changed[k]);
            self.departing[k] += other.departing[k];
        }
    }

    /// The samples summed.
    #[must_use]
    pub const fn samples(&self) -> u32 {
        self.samples
    }

    /// Per age bin, the brightest living star of the samples, mag.
    #[must_use]
    pub const fn living_mag(&self) -> &[f64; AGE_BINS] {
        &self.living
    }

    /// Per age bin, the brightest changed star of the samples, mag.
    #[must_use]
    pub const fn changed_mag(&self) -> &[f64; AGE_BINS] {
        &self.changed
    }

    /// Per age bin, the samples that hold a changed star.
    #[must_use]
    pub const fn departing(&self) -> &[u32; AGE_BINS] {
        &self.departing
    }
}

/// The brighter of two magnitudes, NaN if either is, so that an unsound sample reaches
/// [`assemble`]'s check ([`f64::min`] would drop it).
#[must_use]
fn brighter(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

// --- The assembly ----------------------------------------------------------------------------

/// The changed stars' merge tolerance, mag, for a run whose brightest bin is `v`.
#[must_use]
pub fn merge_tolerance_mag(v: f64) -> f64 {
    MERGE_TOLERANCE_MAG + FAINT_TOLERANCE_SLOPE * (v - FAINT_FROM_MAG).max(0.0)
}

/// The stored class of `count` samples: 0 for none, and for one or more ⌊log₂ count⌋ + 1, so
/// that class c ≥ 1 holds 2^(c − 1) to 2^c − 1 samples.
///
/// # Panics
///
/// Never: a class is at most 32.
#[must_use]
pub fn count_class(count: u32) -> i16 {
    i16::try_from(u32::BITS - count.leading_zeros()).expect("at most 32")
}

/// The stored value of magnitude `v`: +∞ is [`DARK_CMAG`], [`UNSEEN_MAG`] [`UNSEEN_CMAG`], and a
/// finite `v` integer hundredths of a magnitude rounded toward −∞ (lowered further should
/// [`from_cmag`] of it lie above `v`), held at most [`FAINTEST_CMAG`]; `None` for a value the
/// table cannot store (NaN, −∞, or brighter than [`BRIGHTEST_CMAG`]).
#[must_use]
pub fn to_cmag(v: f64) -> Option<i16> {
    if v.is_infinite() && v.is_sign_positive() {
        return Some(DARK_CMAG);
    }
    if v.total_cmp(&UNSEEN_MAG).is_eq() {
        return Some(UNSEEN_CMAG);
    }
    if !v.is_finite() || v < f64::from(BRIGHTEST_CMAG) / 100.0 {
        return None;
    }
    let floor = (v * 100.0).floor().min(f64::from(FAINTEST_CMAG));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number inside the stored range, checked above"
    )]
    let mut k = floor as i16;
    while from_cmag(k) > v {
        if k == BRIGHTEST_CMAG {
            return None;
        }
        k -= 1;
    }
    Some(k)
}

/// The magnitude of a stored value `k`: +∞ for [`DARK_CMAG`], [`UNSEEN_MAG`] for [`UNSEEN_CMAG`],
/// `k` ÷ 100 otherwise.
#[must_use]
pub fn from_cmag(k: i16) -> f64 {
    match k {
        DARK_CMAG => f64::INFINITY,
        UNSEEN_CMAG => UNSEEN_MAG,
        _ => f64::from(k) / 100.0,
    }
}

/// One layer's table, as `hyperion-fit` writes it.
#[derive(Debug, Clone, PartialEq)]
pub struct PairLightCells {
    grid: PairGrid,
    samples_per_cell: u32,
    /// Per cell, per age bin, the stored living value, hundredths of a magnitude.
    living: Vec<[i16; AGE_BINS]>,
    /// Per cell, per age bin, the stored changed value, hundredths of a magnitude.
    changed: Vec<[i16; AGE_BINS]>,
    /// Per cell, per age bin, the class of the samples that hold a changed star
    /// ([`count_class`]).
    departing: Vec<[i16; AGE_BINS]>,
    /// Per cell, per age bin, the changed value before the margin and the merge, magnitudes, for
    /// the fit's validation.
    changed_unmargined: Vec<[f64; AGE_BINS]>,
    /// Per cell, per age bin, the living value before the margin and the merge, magnitudes.
    living_unmargined: Vec<[f64; AGE_BINS]>,
}

/// An error in [`assemble`]'s input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AssemblePairLightError {
    /// The cells are not one per cell of the grid.
    WrongShape {
        /// The cells the grid holds.
        expected: usize,
        /// The cells given.
        found: usize,
    },
    /// A cell holds no sample.
    EmptyCell {
        /// The cell's index.
        cell: usize,
    },
    /// A cell holds a magnitude that is not a number, −∞, or too bright to store.
    UnsoundCell {
        /// The cell's index.
        cell: usize,
    },
    /// A cell holds more samples than a stored count holds.
    TooManySamples {
        /// The cell's index.
        cell: usize,
    },
}

impl std::fmt::Display for AssemblePairLightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongShape { expected, found } => {
                write!(f, "expected {expected} pair-light cells, found {found}")
            }
            Self::EmptyCell { cell } => write!(f, "pair-light cell {cell} holds no sample"),
            Self::UnsoundCell { cell } => {
                write!(
                    f,
                    "pair-light cell {cell} holds a magnitude the table cannot store"
                )
            }
            Self::TooManySamples { cell } => {
                write!(
                    f,
                    "pair-light cell {cell} holds more samples than a count stores"
                )
            }
        }
    }
}

impl std::error::Error for AssemblePairLightError {}

/// Takes the brightest of each value and its neighbours one step either way along an axis of
/// `values` (a value's neighbours are `stride` apart, `len` to a line), in place.
fn dilate_axis<T: Copy>(values: &mut [T], stride: usize, len: usize, min: impl Fn(T, T) -> T) {
    let line = stride * len;
    let mut column: Vec<T> = Vec::with_capacity(len);
    for base in (0..values.len()).step_by(line) {
        for offset in 0..stride {
            column.clear();
            column.extend((0..len).map(|i| values[base + offset + i * stride]));
            for i in 0..len {
                let mut v = column[i];
                if i > 0 {
                    v = min(v, column[i - 1]);
                }
                if i + 1 < len {
                    v = min(v, column[i + 1]);
                }
                values[base + offset + i * stride] = v;
            }
        }
    }
}

/// The rows of one value of every cell, cumulated over wider periastra and dilated by one cell
/// along each axis and one age bin (see the [module](self) documentation), in place.
#[must_use]
fn widened(grid: &PairGrid, rows: Vec<[f64; AGE_BINS]>) -> Vec<[f64; AGE_BINS]> {
    let p_cells = grid.periastron_cells();
    let mut out = rows;
    // Cumulative over every wider periastron: the periastron is the fastest axis.
    for column in out.chunks_mut(p_cells) {
        for p in (0..p_cells - 1).rev() {
            let wider = column[p + 1];
            for (v, w) in column[p].iter_mut().zip(wider) {
                *v = v.min(w);
            }
        }
    }
    let row_min = |a: [f64; AGE_BINS], b: [f64; AGE_BINS]| -> [f64; AGE_BINS] {
        core::array::from_fn(|k| a[k].min(b[k]))
    };
    // A box's brightest is the brightest along each of its axes in turn, so the four cell axes
    // and then age are dilated one at a time.
    let strides = [
        (1, p_cells),
        (p_cells, FE_H_CELLS),
        (p_cells * FE_H_CELLS, Q_CELLS),
        (p_cells * FE_H_CELLS * Q_CELLS, MASS_CELLS),
    ];
    for (stride, len) in strides {
        dilate_axis(&mut out, stride, len, row_min);
    }
    for row in &mut out {
        dilate_axis(row, 1, AGE_BINS, f64::min);
    }
    out
}

/// Brightens a widened row's finite values by [`MARGIN_MAG`], merges runs within `tolerance` of
/// their brightest at it, and stores it ([`to_cmag`]); `None` for a value the table cannot store.
#[must_use]
fn stored_row(row: &[f64; AGE_BINS], tolerance: &dyn Fn(f64) -> f64) -> Option<[i16; AGE_BINS]> {
    let finite = |v: f64| v.is_finite() && v < UNSEEN_MAG;
    let margined: [f64; AGE_BINS] = core::array::from_fn(|k| {
        let v = row[k];
        if finite(v) { v - MARGIN_MAG } else { v }
    });
    let mut merged = margined;
    let mut start = 0;
    while start < AGE_BINS {
        let mut end = start + 1;
        let (mut brightest, mut faintest) = (margined[start], margined[start]);
        if finite(margined[start]) {
            while end < AGE_BINS && finite(margined[end]) {
                let (b, f) = (brightest.min(margined[end]), faintest.max(margined[end]));
                if f - b > tolerance(b) {
                    break;
                }
                (brightest, faintest) = (b, f);
                end += 1;
            }
        }
        merged[start..end].fill(brightest);
        start = end;
    }
    let mut out = [DARK_CMAG; AGE_BINS];
    for (slot, &v) in out.iter_mut().zip(&merged) {
        *slot = to_cmag(v)?;
    }
    Some(out)
}

/// One layer's table from each cell's summed samples, `cells[cell]` in cell order (see the
/// [module](self) documentation).
///
/// # Errors
///
/// - [`AssemblePairLightError::WrongShape`] if `cells` is not one per cell of `grid`;
/// - [`AssemblePairLightError::EmptyCell`] if a cell holds no sample;
/// - [`AssemblePairLightError::TooManySamples`] if a cell holds more samples than an `i16`;
/// - [`AssemblePairLightError::UnsoundCell`] if a cell holds a magnitude that is not a number,
///   −∞, or too bright to store.
///
/// # Panics
///
/// Never: a grid holds a cell.
///
/// # Examples
///
/// The fit sums each cell's samples into a [`CellRows`] and assembles every cell at once. Here
/// every cell of C holds one sample of nothing, so the table holds nothing anywhere:
///
/// ```
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::binary::pair_light::{
///     AGE_BINS, CellRows, DARK_CMAG, PairGrid, PairLightTable, PairRows, assemble,
/// };
///
/// let grid = PairGrid::of(Layer::C).ok_or("C has a table")?;
/// let mut cells = vec![CellRows::default(); grid.cells()];
/// for cell in &mut cells {
///     cell.add(&PairRows::empty());
/// }
/// let table = PairLightTable::from_cells(&assemble(&grid, &cells).map_err(|e| e.to_string())?);
/// assert!((0..AGE_BINS).all(|bin| table.changed_cmag(0, bin) == DARK_CMAG));
/// assert_eq!(table.samples_per_cell(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn assemble(
    grid: &PairGrid,
    cells: &[CellRows],
) -> Result<PairLightCells, AssemblePairLightError> {
    if cells.len() != grid.cells() {
        return Err(AssemblePairLightError::WrongShape {
            expected: grid.cells(),
            found: cells.len(),
        });
    }
    for (cell, rows) in cells.iter().enumerate() {
        if rows.samples == 0 {
            return Err(AssemblePairLightError::EmptyCell { cell });
        }
        if i16::try_from(rows.samples).is_err() {
            return Err(AssemblePairLightError::TooManySamples { cell });
        }
        if rows
            .living
            .iter()
            .chain(&rows.changed)
            .any(|v| v.is_nan() || (v.is_infinite() && v.is_sign_negative()))
        {
            return Err(AssemblePairLightError::UnsoundCell { cell });
        }
    }
    let living_unmargined = widened(grid, cells.iter().map(|c| c.living).collect());
    let changed_unmargined = widened(grid, cells.iter().map(|c| c.changed).collect());
    let store = |rows: &[[f64; AGE_BINS]],
                 tolerance: &dyn Fn(f64) -> f64|
     -> Result<Vec<[i16; AGE_BINS]>, AssemblePairLightError> {
        rows.iter()
            .enumerate()
            .map(|(cell, row)| {
                stored_row(row, tolerance).ok_or(AssemblePairLightError::UnsoundCell { cell })
            })
            .collect()
    };
    let mut living = store(&living_unmargined, &|v| grid.living_merge_tolerance_mag(v))?;
    let mut changed = store(&changed_unmargined, &merge_tolerance_mag)?;
    // Merging runs of age bins can reorder neighbouring cells; restore what the reader may rely
    // on, which only brightens: a closer orbit is no fainter than a wider one, and the living
    // value no fainter than the changed one.
    let p_cells = grid.periastron_cells();
    for column in living
        .chunks_mut(p_cells)
        .chain(changed.chunks_mut(p_cells))
    {
        for p in (0..p_cells - 1).rev() {
            let wider = column[p + 1];
            for (v, w) in column[p].iter_mut().zip(wider) {
                *v = (*v).min(w);
            }
        }
    }
    for (l, c) in living.iter_mut().zip(&changed) {
        for (v, &w) in l.iter_mut().zip(c) {
            *v = (*v).min(w);
        }
    }
    let departing = cells.iter().map(|c| c.departing.map(count_class)).collect();
    let samples_per_cell = cells
        .iter()
        .map(CellRows::samples)
        .min()
        .expect("a grid holds a cell");
    Ok(PairLightCells {
        grid: *grid,
        samples_per_cell,
        living,
        changed,
        departing,
        changed_unmargined,
        living_unmargined,
    })
}

impl PairLightCells {
    /// The grid.
    #[must_use]
    pub const fn grid(&self) -> &PairGrid {
        &self.grid
    }

    /// The least samples of a cell.
    #[must_use]
    pub const fn samples_per_cell(&self) -> u32 {
        self.samples_per_cell
    }

    /// `cell`'s stored living value at age bin `bin`, hundredths of a magnitude, as the table
    /// stores it: without any layer's reading ([`PairGrid::read_margin_cmag`],
    /// [`PairGrid::read_smear_bins`]), for the fit's own figures.
    ///
    /// # Panics
    ///
    /// If `cell` or `bin` is past the table.
    #[must_use]
    pub fn stored_living_cmag(&self, cell: usize, bin: usize) -> i16 {
        self.living[cell][bin]
    }

    /// `cell`'s stored changed value at age bin `bin`, hundredths of a magnitude, as the table
    /// stores it ([`Self::stored_living_cmag`]).
    ///
    /// # Panics
    ///
    /// If `cell` or `bin` is past the table.
    #[must_use]
    pub fn stored_changed_cmag(&self, cell: usize, bin: usize) -> i16 {
        self.changed[cell][bin]
    }

    /// Per age bin, `cell`'s changed value before the margin and the merge, magnitudes: the
    /// brightest changed star of its widened neighbourhood.
    ///
    /// # Panics
    ///
    /// If `cell` is past the grid.
    #[must_use]
    pub fn changed_unmargined_mag(&self, cell: usize) -> &[f64; AGE_BINS] {
        &self.changed_unmargined[cell]
    }

    /// Per age bin, `cell`'s living value before the margin and the merge, magnitudes.
    ///
    /// # Panics
    ///
    /// If `cell` is past the grid.
    #[must_use]
    pub fn living_unmargined_mag(&self, cell: usize) -> &[f64; AGE_BINS] {
        &self.living_unmargined[cell]
    }

    /// The table packed in format [`FORMAT`]: the distinct rows, in the order cells first use
    /// them (living, changed, count classes); per cell its three rows' indices, three base64
    /// characters each; and the rows as segments of a run of bins (6 bits) and a value (12 bits,
    /// two's complement), three characters each.
    ///
    /// # Panics
    ///
    /// If the table holds more than 2¹⁶ distinct rows, or a value outside 12 bits, which
    /// [`assemble`] rules out.
    #[must_use]
    pub fn pack(&self) -> PackedPairLight {
        let mut index: std::collections::BTreeMap<[i16; AGE_BINS], u16> =
            std::collections::BTreeMap::new();
        let mut order: Vec<[i16; AGE_BINS]> = Vec::new();
        let mut cell_rows = String::with_capacity(self.living.len() * 9);
        for cell in 0..self.living.len() {
            for row in [
                &self.living[cell],
                &self.changed[cell],
                &self.departing[cell],
            ] {
                let id = *index.entry(*row).or_insert_with(|| {
                    order.push(*row);
                    u16::try_from(order.len() - 1).expect("at most 2^16 distinct rows")
                });
                push_sextets(&mut cell_rows, u32::from(id), 3);
            }
        }
        let mut rows = String::new();
        let mut segments = 0;
        for row in &order {
            let mut start = 0;
            while start < AGE_BINS {
                let end = (start + 1..AGE_BINS)
                    .find(|&k| row[k] != row[start])
                    .unwrap_or(AGE_BINS);
                let run = u32::try_from(end - start).expect("a run within a row of 53 bins");
                let value = row[start];
                assert!(
                    (BRIGHTEST_CMAG..=DARK_CMAG).contains(&value),
                    "a stored value of 12 bits: {value}"
                );
                let code = u32::from(value.cast_unsigned()) & 0xfff;
                push_sextets(&mut rows, (run << 12) | code, 3);
                segments += 1;
                start = end;
            }
        }
        PackedPairLight {
            cell_rows,
            rows,
            row_count: order.len(),
            segments,
        }
    }
}

/// A table packed in format [`FORMAT`] ([`PairLightCells::pack`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackedPairLight {
    cell_rows: String,
    rows: String,
    row_count: usize,
    segments: usize,
}

impl PackedPairLight {
    /// Per cell, its living, changed and count rows' indices, three base64 characters each.
    #[must_use]
    pub fn cell_rows(&self) -> &str {
        &self.cell_rows
    }

    /// The distinct rows, each as segments of a run of bins (6 bits) and a value (12 bits, two's
    /// complement), three base64 characters a segment.
    #[must_use]
    pub fn rows(&self) -> &str {
        &self.rows
    }

    /// The distinct rows' count.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// The rows' segments.
    #[must_use]
    pub const fn segments(&self) -> usize {
        self.segments
    }
}

/// The alphabet of the packed tables: RFC 4648's base64 (its §4), without padding.
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Appends the low `count` sextets of `word`, the highest first, as base64 characters.
fn push_sextets(out: &mut String, word: u32, count: u32) {
    for k in (0..count).rev() {
        let sextet = usize::try_from((word >> (6 * k)) & 0x3f).expect("six bits");
        out.push(char::from(BASE64[sextet]));
    }
}

/// The value of a base64 character, or `None` outside the alphabet.
#[must_use]
fn sextet(c: u8) -> Option<u32> {
    let v = match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(u32::from(v))
}

/// The word of base64 characters `chars`, the highest sextet first.
#[must_use]
fn word_of(chars: &[u8]) -> Option<u32> {
    chars
        .iter()
        .try_fold(0_u32, |word, &c| Some((word << 6) | sextet(c)?))
}

// --- The cooling floor -----------------------------------------------------------------------

/// The masses, M☉, at which the cooling floor reads the cooling fits: 0.08–0.1 M☉ in 64 even
/// steps, the fits' top (P06.T13; [`substellar::MAX_MASS`]) last.
const COOLING_FLOOR_MASSES: u32 = 64;

/// The metallicities a cooling floor's interval is read at, edges included.
const COOLING_FLOOR_FE_H: u32 = 8;

/// The ages a cooling floor's bin is read at, edges included.
const COOLING_FLOOR_AGES: u32 = 8;

/// The brightest V, mag, of a star on P06.T13's cooling fits of 0.08–0.1 M☉ and metallicity in
/// interval `fe_cell` of [`FE_H_EDGES`] at an age in bin `bin`: read on a grid of masses,
/// metallicities and ages, edges included, through plan 06's photometry (the sky's for these
/// states, which it never darkens).
#[must_use]
fn cooling_brightest(fe_cell: usize, bin: usize) -> f64 {
    let (f_lo, f_hi) = (FE_H_EDGES[fe_cell], FE_H_EDGES[fe_cell + 1]);
    let (t_lo, t_hi) = (age_edge_years(bin), age_edge_years(bin + 1));
    let mut brightest = f64::INFINITY;
    for j in 0..=COOLING_FLOOR_FE_H {
        let fe_h = lerp(f_lo, f_hi, f64::from(j) / f64::from(COOLING_FLOOR_FE_H));
        let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        for k in 0..=COOLING_FLOOR_AGES {
            let t = lerp(t_lo, t_hi, f64::from(k) / f64::from(COOLING_FLOOR_AGES));
            for n in 0..=COOLING_FLOOR_MASSES {
                let m = lerp(
                    LOWEST_COMPANION_MSUN,
                    substellar::MAX_MASS.value(),
                    f64::from(n) / f64::from(COOLING_FLOOR_MASSES),
                );
                let state = substellar::cooling(SolarMasses::new(m), Years::new(t), &comp)
                    .expect("0.08-0.1 M_sun at a finite age is inside the cooling fits");
                if let Some(v) = absolute_magnitude_v(&state) {
                    brightest = brightest.min(v.value());
                }
            }
        }
    }
    brightest
}

/// The cooling floor of metallicity interval `fe_cell` at age bin `bin`, hundredths of a magnitude.
///
/// It is the brightest V ([`from_cmag`]) of a star on the cooling fits of the interval at an age in
/// the bin, brightened by [`MARGIN_MAG`] and rounded brighter, or [`UNSEEN_CMAG`] where none has a
/// V. Computed once, at the first call.
///
/// # Panics
///
/// If `fe_cell` or `bin` is past its axis.
#[must_use]
pub fn cooling_floor_cmag(fe_cell: usize, bin: usize) -> i16 {
    static FLOOR: OnceLock<Vec<[i16; AGE_BINS]>> = OnceLock::new();
    let floor = FLOOR.get_or_init(|| {
        (0..FE_H_CELLS)
            .map(|f| {
                core::array::from_fn(|k| {
                    let v = cooling_brightest(f, k);
                    let v = if v.is_finite() {
                        v - MARGIN_MAG
                    } else {
                        UNSEEN_MAG
                    };
                    to_cmag(v).expect("a cooling star's V is storable")
                })
            })
            .collect()
    });
    floor[fe_cell][bin]
}

/// A cell's value as a pair whose lighter star lies below 0.1 M☉ reads it.
///
/// `stored` is a living or changed value of a cell of metallicity interval `fe_cell` at age bin
/// `bin`. The pair's lighter star is on P06.T13's cooling fits, so the value read is the brighter
/// of `stored` and [`cooling_floor_cmag`], DARK staying DARK (see the [module](self)
/// documentation).
///
/// # Panics
///
/// If `fe_cell` or `bin` is past its axis.
///
/// # Examples
///
/// A pair of a 20 M☉ star and a 0.09 M☉ companion reads its cell's changed value at 1 Myr through
/// the floor, since the companion follows the cooling fits.
///
/// ```
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::binary::pair_light::{
///     DARK_CMAG, PairLightTable, age_bin, reads_cooling_floor, with_cooling_floor,
/// };
///
/// let table = PairLightTable::generator(Layer::E).ok_or("E's table is fitted")?;
/// let (heavier, lighter) = (20.0, 0.09);
/// let cell = table.grid().cell_of(heavier, lighter, 0.0, 300.0).ok_or("inside the table")?;
/// let fe_cell = table.grid().cell_parts(cell)[2];
/// let bin = age_bin(1.0e6).ok_or("inside the table")?;
/// let stored = table.changed_cmag(cell, bin);
/// let read = if reads_cooling_floor(lighter) {
///     with_cooling_floor(stored, fe_cell, bin)
/// } else {
///     stored
/// };
/// assert!(read <= stored);
/// assert_eq!(with_cooling_floor(DARK_CMAG, fe_cell, bin), DARK_CMAG);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn with_cooling_floor(stored: i16, fe_cell: usize, bin: usize) -> i16 {
    if stored == DARK_CMAG {
        stored
    } else {
        stored.min(cooling_floor_cmag(fe_cell, bin))
    }
}

/// Whether a pair whose lighter star has initial mass `lighter_msun` (M☉) reads the cooling floor.
///
/// It does for a star below 0.1 M☉, which follows the cooling fits.
#[must_use]
pub fn reads_cooling_floor(lighter_msun: f64) -> bool {
    lighter_msun < MIN_INITIAL_MASS.value()
}

// --- The held floor --------------------------------------------------------------------------

/// The pair masses, M☉, at which the held floor is tabulated: [`HELD_FLOOR_NODES`] nodes even in
/// ln m from [`MIN_INITIAL_MASS`] (a lighter star the binary carries is on the cooling fits) to
/// [`HELD_FLOOR_TOP_MSUN`].
const HELD_FLOOR_NODES: usize = 57;

/// The top of the held floor's masses, M☉: twice the tracks' 150 M☉, the heaviest pair.
const HELD_FLOOR_TOP_MSUN: f64 = 300.0;

/// The masses a held floor's interval of masses is read at, edges included.
const HELD_FLOOR_MASS_STEPS: u32 = 4;

/// The fractional ages a held floor's main sequences are read at, edges included.
const HELD_FLOOR_TAU_STEPS: u32 = 32;

/// The metallicities a held floor's interval is read at, edges included.
const HELD_FLOOR_FE_H: u32 = 4;

/// Node `k` of the held floor's masses, M☉.
#[must_use]
fn held_floor_node_msun(k: usize) -> f64 {
    let last = u32::try_from(HELD_FLOOR_NODES - 1).expect("a few dozen nodes");
    let k = u32::try_from(k).expect("a few dozen nodes");
    log_lerp(
        MIN_INITIAL_MASS.value(),
        HELD_FLOOR_TOP_MSUN,
        f64::from(k) / f64::from(last),
    )
}

/// What the held floor holds for a metallicity interval: per mass node, the first age bin and the
/// value.
#[derive(Debug, Clone, PartialEq)]
struct HeldFloorTable {
    /// The nodes' masses, M☉ ([`held_floor_node_msun`]), kept so that a query reads no logarithm.
    node_msun: [f64; HELD_FLOOR_NODES],
    /// The first age bin in which a held star of a pair of at most the node's mass can live,
    /// [`AGE_BINS`] for none.
    first_bin: [usize; HELD_FLOOR_NODES],
    /// Its brightest V, hundredths of a magnitude, margin included.
    cmag: [i16; HELD_FLOOR_NODES],
}

/// The held floor of metallicity interval `fe_cell` (see [`held_floor`]): read on a grid of
/// masses, fractional ages and metallicities, edges included, through plan 06's photometry (the
/// sky's for these states), each node's values taken over every lighter mass.
#[must_use]
fn held_floor_of(fe_cell: usize) -> HeldFloorTable {
    let (f_lo, f_hi) = (FE_H_EDGES[fe_cell], FE_H_EDGES[fe_cell + 1]);
    let draws = StarDraws::median();
    let coeffs: Vec<(Composition, ZCoeffs)> = (0..=HELD_FLOOR_FE_H)
        .map(|j| {
            let fe_h = lerp(f_lo, f_hi, f64::from(j) / f64::from(HELD_FLOOR_FE_H));
            let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            (comp, ZCoeffs::new(comp.z_fit()))
        })
        .collect();
    let mut floor = HeldFloorTable {
        node_msun: core::array::from_fn(held_floor_node_msun),
        first_bin: [AGE_BINS; HELD_FLOOR_NODES],
        cmag: [DARK_CMAG; HELD_FLOOR_NODES],
    };
    let (mut brightest, mut earliest) = (f64::INFINITY, f64::INFINITY);
    for k in 0..HELD_FLOOR_NODES {
        let lo = held_floor_node_msun(k.saturating_sub(1));
        let hi = held_floor_node_msun(k);
        for n in 0..=HELD_FLOOR_MASS_STEPS {
            let m = log_lerp(lo, hi, f64::from(n) / f64::from(HELD_FLOOR_MASS_STEPS));
            for (comp, c) in &coeffs {
                earliest = earliest.min(sse::main_sequence_lifetime(c, false, m));
                for helium in [false, true] {
                    for t in 0..=HELD_FLOOR_TAU_STEPS {
                        let tau = f64::from(t) / f64::from(HELD_FLOOR_TAU_STEPS);
                        let (s, _) =
                            sse::main_sequence_structure(c, comp, &draws, helium, m, tau, 0.0);
                        if let Some(v) = absolute_magnitude_v(&s.state) {
                            brightest = brightest.min(v.value());
                        }
                    }
                }
            }
        }
        floor.cmag[k] =
            to_cmag(brightest - MARGIN_MAG).expect("a main-sequence star's V is storable");
        // A lifetime past the table's ages holds no star of the pair at its end: never.
        floor.first_bin[k] = age_bin(earliest).unwrap_or(AGE_BINS);
    }
    floor
}

/// The held floor of a pair of total initial mass `total_msun` (M☉) and metallicity interval
/// `fe_cell` of [`FE_H_EDGES`]: the first age bin from which it applies, and its value,
/// hundredths of a magnitude ([`HeldFloor`]); `None` for a total that is not a number or lies
/// past [`HELD_FLOOR_TOP_MSUN`].
///
/// **Why.** A timeline that reaches the engine's cap on segments ([`MAX_SEGMENTS`]) is left as it
/// stands, its last segment stretched to the age asked (a broken invariant the engine's tests
/// count). A star the binary carries on its main sequence there ([`Member::MainSequence`],
/// hydrogen or helium) is held at its last mass and fractional age, alive at every later age,
/// long past its own lifetime. The engine reaches that cap where an accretor so carried is fed
/// as it nears its main sequence's end, each step aimed at that end landing short of it once
/// the accretion has lowered its fractional age (plan 11's Risks: finding F15 for a helium
/// accretor, and the same for a hydrogen one above 1.25 M☉), and in P11.T4.g's swell and strip
/// cycle of a fed helium star (finding C2). Such a pair is rare (about 10⁻⁴ of close pairs of
/// 5–8 M☉, P11.T17.c's follow-up probe) and the tables' 72 samples a cell seldom hold one, so the
/// tables can read DARK where it lives. The floor bounds it from the engine's own models:
///
/// - its value is the brightest V of a hydrogen or helium main-sequence star of mass at most
///   `total_msun` (the binary never adds mass), at any fractional age and any metallicity of the
///   interval, Z held at the tracks' 0.03 in the top one ([`sse::main_sequence_structure`], the
///   state the engine gives such a member; plan 06's photometry), brightened by [`MARGIN_MAG`]
///   and rounded brighter. Hurley, Pols and Tout's (2000) models are fitted over 0.5–50 M☉ and
///   extrapolated past them, as the engine extrapolates them: above about 50 M☉ the floor is
///   brighter than any star seen (a 150 M☉ pair's −11.6 against R136a1's M<sub>V</sub> −7.4,
///   Crowther et al. 2010, MNRAS 408, 731), safe but loose;
/// - its first bin is that of the least main-sequence lifetime of such a star: no star of the
///   pair can reach its main sequence's end, or be stripped to a helium star, sooner, since a
///   star's τ grows at no more than 1 ÷ `t_MS` of its mass, and accretion never raises it.
///
/// Computed once, at the first call.
///
/// # Panics
///
/// If `fe_cell` is past [`FE_H_CELLS`].
///
/// [`MAX_SEGMENTS`]: super::MAX_SEGMENTS
#[must_use]
pub(crate) fn held_floor(fe_cell: usize, total_msun: f64) -> Option<HeldFloor> {
    static FLOOR: OnceLock<Vec<HeldFloorTable>> = OnceLock::new();
    let floor = &FLOOR.get_or_init(|| (0..FE_H_CELLS).map(held_floor_of).collect())[fe_cell];
    if total_msun.is_nan() || total_msun > HELD_FLOOR_TOP_MSUN {
        return None;
    }
    // The first node at or above the total; each node's values hold for every lighter pair.
    let k = floor
        .node_msun
        .partition_point(|&m| m < total_msun)
        .min(HELD_FLOOR_NODES - 1);
    Some(HeldFloor {
        first_bin: floor.first_bin[k],
        cmag: floor.cmag[k],
    })
}

/// A pair's held floor ([`held_floor`]): the first age bin from which it applies, and its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HeldFloor {
    first_bin: usize,
    cmag: i16,
}

impl HeldFloor {
    /// The first age bin from which the floor applies: [`AGE_BINS`] for none.
    #[must_use]
    pub(crate) const fn first_bin(self) -> usize {
        self.first_bin
    }

    /// The floor's value, hundredths of a magnitude ([`from_cmag`]), margin included.
    #[must_use]
    pub(crate) const fn cmag(self) -> i16 {
        self.cmag
    }
}

// --- The reader ------------------------------------------------------------------------------

/// Why a packed table cannot be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReadPairLightError {
    /// The table's format is not [`FORMAT`].
    WrongFormat,
    /// The table's dimensions are not the grid's.
    WrongDimensions,
    /// The packed text is not whole cells and rows of the format, or a cell names a row the table
    /// does not hold.
    Malformed,
}

impl std::fmt::Display for ReadPairLightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongFormat => write!(f, "a pair-light table of another format"),
            Self::WrongDimensions => write!(f, "a pair-light table of another grid"),
            Self::Malformed => write!(f, "a malformed pair-light table"),
        }
    }
}

impl std::error::Error for ReadPairLightError {}

/// The dimensions a layer's fitted table states: mass, mass-ratio, metallicity and periastron
/// intervals, age bins, distinct rows and segments.
pub type Dimensions = [usize; 7];

/// One layer's pair-light table, decoded (see the [module](self) documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct PairLightTable {
    grid: PairGrid,
    samples_per_cell: u32,
    /// Per cell, its living, changed and count rows.
    cell_rows: Vec<[u16; 3]>,
    /// The distinct rows.
    rows: Vec<[i16; AGE_BINS]>,
    /// Per cell, its living values as the reader gives them (the layer's reading applied).
    living: Vec<[i16; AGE_BINS]>,
    /// Per cell, its changed values as the reader gives them.
    changed: Vec<[i16; AGE_BINS]>,
    /// The grid's lower edges of mass interval k ≥ 1, M☉, as [`PairGrid::mass_edges_msun`] gives
    /// them, so that [`Self::cell_of`] computes none: computing them took some 60 `exp` calls a
    /// query, most of P11.T17.c's 1 µs budget, and the census's bound now costs 0.55 µs a call
    /// (plan 11's Risks, "P11.T17.c as built").
    mass_edges_msun: [f64; MASS_CELLS],
    /// The grid's lower edges of periastron bin k ≥ 1, R☉, as
    /// [`PairGrid::periastron_edges_rsun`] gives them (index 0 unused).
    periastron_edges_rsun: Vec<f64>,
}

/// A stored magnitude brightened by `margin_cmag`, DARK and UNSEEN as they are.
#[must_use]
fn read_value(stored: i16, margin_cmag: i16) -> i16 {
    if stored >= UNSEEN_CMAG {
        stored
    } else {
        stored.saturating_sub(margin_cmag).max(BRIGHTEST_CMAG)
    }
}

/// A stored row as `grid`'s reader gives it: each value brightened by its read margin, then each
/// bin the brightest of itself and its read smear's bins either way.
#[must_use]
fn read_row(grid: &PairGrid, row: &[i16; AGE_BINS]) -> [i16; AGE_BINS] {
    let margined = row.map(|k| read_value(k, grid.read_margin_cmag()));
    let w = grid.read_smear_bins();
    core::array::from_fn(|k| {
        margined[k.saturating_sub(w)..=(k + w).min(AGE_BINS - 1)]
            .iter()
            .copied()
            .min()
            .unwrap_or(DARK_CMAG)
    })
}

impl PairLightTable {
    /// The generator's table of `layer`, decoded once.
    ///
    /// `None` for a layer without one, or while its fitted table is a placeholder whose dimensions
    /// are not the grid's.
    ///
    /// # Panics
    ///
    /// If a fitted table's format or packed text is not its grid's: a committed table the reader
    /// cannot read is a broken invariant, which `hyperion-fit check` and the tests rule out.
    ///
    /// # Examples
    ///
    /// What a 10 M☉ star and its 1.5 M☉ companion, 20 au apart at periastron at solar
    /// metallicity, can hold at 2 Gyr: the brightest V of any star of theirs that departs from
    /// its own single-star model, or of a product, if one may live then.
    ///
    /// ```
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::stellar::binary::pair_light::{
    ///     DARK_CMAG, PairLightTable, age_bin, from_cmag,
    /// };
    /// use hyperion_sim::units::consts::{METRES_PER_AU, SOLAR_RADIUS_M};
    ///
    /// let table = PairLightTable::generator(Layer::E).ok_or("E's table is fitted")?;
    /// let periastron_rsun = 20.0 * METRES_PER_AU / SOLAR_RADIUS_M;
    /// let cell = table
    ///     .grid()
    ///     .cell_of(10.0, 1.5, 0.0, periastron_rsun)
    ///     .ok_or("inside the table")?;
    /// let bin = age_bin(2.0e9).ok_or("inside the table")?;
    /// let changed = table.changed_cmag(cell, bin);
    /// if changed != DARK_CMAG {
    ///     assert!(from_cmag(changed) > -20.0);
    /// }
    /// // The living stars include the pair's own models: the companion is alive at 2 Gyr.
    /// assert_ne!(table.living_cmag(cell, bin), DARK_CMAG);
    /// # Ok::<(), &str>(())
    /// ```
    #[must_use]
    pub fn generator(layer: Layer) -> Option<&'static Self> {
        static TABLES: [OnceLock<Option<PairLightTable>>; 3] =
            [OnceLock::new(), OnceLock::new(), OnceLock::new()];
        let k = LAYERS.iter().position(|&l| l == layer)?;
        TABLES[k]
            .get_or_init(|| {
                let grid = PairGrid::of(layer)?;
                let (format, dimensions, samples, cell_rows, rows) = match layer {
                    Layer::C => (
                        binary_pair_light_c::FORMAT,
                        binary_pair_light_c::DIMENSIONS,
                        binary_pair_light_c::SAMPLES_PER_CELL,
                        binary_pair_light_c::CELL_ROWS,
                        binary_pair_light_c::ROWS,
                    ),
                    Layer::D => (
                        binary_pair_light_d::FORMAT,
                        binary_pair_light_d::DIMENSIONS,
                        binary_pair_light_d::SAMPLES_PER_CELL,
                        binary_pair_light_d::CELL_ROWS,
                        binary_pair_light_d::ROWS,
                    ),
                    Layer::E => (
                        binary_pair_light_e::FORMAT,
                        binary_pair_light_e::DIMENSIONS,
                        binary_pair_light_e::SAMPLES_PER_CELL,
                        binary_pair_light_e::CELL_ROWS,
                        binary_pair_light_e::ROWS,
                    ),
                    Layer::A | Layer::B | Layer::BrownDwarf | Layer::RoguePlanet => return None,
                };
                match Self::unpack(&grid, format, dimensions, samples, cell_rows, rows) {
                    Ok(table) => Some(table),
                    // A placeholder (no dimensions), as a table stands before its first fit and
                    // as the version-22 refit writes one first: no table, so no verdict.
                    Err(ReadPairLightError::WrongDimensions) => None,
                    Err(error) => panic!("the fitted pair-light table of {layer:?}: {error}"),
                }
            })
            .as_ref()
    }

    /// The dimensions `grid`'s table states, with `rows` distinct rows of `segments` segments.
    #[must_use]
    pub const fn dimensions_of(grid: &PairGrid, rows: usize, segments: usize) -> Dimensions {
        [
            MASS_CELLS,
            Q_CELLS,
            FE_H_CELLS,
            grid.periastron_cells(),
            AGE_BINS,
            rows,
            segments,
        ]
    }

    /// The table of `grid` packed as `cell_rows` and `rows` ([`PairLightCells::pack`]), in
    /// `format` with `dimensions` and `samples_per_cell` (ASCII whitespace in the text ignored).
    ///
    /// # Errors
    ///
    /// [`ReadPairLightError`] if the format, the dimensions or the text are not the grid's.
    pub fn unpack(
        grid: &PairGrid,
        format: u32,
        dimensions: Dimensions,
        samples_per_cell: u32,
        cell_rows: &str,
        rows: &str,
    ) -> Result<Self, ReadPairLightError> {
        if format != FORMAT {
            return Err(ReadPairLightError::WrongFormat);
        }
        let [_, _, _, _, _, row_count, segment_count] = dimensions;
        if dimensions != Self::dimensions_of(grid, row_count, segment_count) || row_count == 0 {
            return Err(ReadPairLightError::WrongDimensions);
        }
        let text: Vec<u8> = rows.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
        let (groups, rest) = text.as_chunks::<3>();
        if !rest.is_empty() || groups.len() != segment_count {
            return Err(ReadPairLightError::Malformed);
        }
        let mut decoded = Vec::with_capacity(row_count);
        let mut row = [DARK_CMAG; AGE_BINS];
        let mut next = 0;
        for group in groups {
            let word = word_of(group).ok_or(ReadPairLightError::Malformed)?;
            let run = usize::try_from(word >> 12).map_err(|_| ReadPairLightError::Malformed)?;
            let end = next + run;
            if run == 0 || end > AGE_BINS {
                return Err(ReadPairLightError::Malformed);
            }
            // The low 12 bits, sign-extended.
            let code = u16::try_from(word & 0xfff).map_err(|_| ReadPairLightError::Malformed)?;
            let value = (code << 4).cast_signed() >> 4;
            row[next..end].fill(value);
            next = end;
            if next == AGE_BINS {
                decoded.push(row);
                next = 0;
            }
        }
        if next != 0 || decoded.len() != row_count {
            return Err(ReadPairLightError::Malformed);
        }
        let text: Vec<u8> = cell_rows
            .bytes()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        let (cells, rest) = text.as_chunks::<9>();
        if !rest.is_empty() || cells.len() != grid.cells() {
            return Err(ReadPairLightError::Malformed);
        }
        let mut indices = Vec::with_capacity(cells.len());
        for cell in cells {
            let mut ids = [0_u16; 3];
            for (id, chars) in ids.iter_mut().zip(cell.chunks(3)) {
                let word = word_of(chars).ok_or(ReadPairLightError::Malformed)?;
                *id = u16::try_from(word).map_err(|_| ReadPairLightError::Malformed)?;
                if usize::from(*id) >= row_count {
                    return Err(ReadPairLightError::Malformed);
                }
            }
            // A count row holds classes, 0 for none to 32 at most (`count_class`).
            if decoded[usize::from(ids[2])]
                .iter()
                .any(|&class| !(0..=32).contains(&class))
            {
                return Err(ReadPairLightError::Malformed);
            }
            indices.push(ids);
        }
        let read = |kind: usize| -> Vec<[i16; AGE_BINS]> {
            indices
                .iter()
                .map(|ids| read_row(grid, &decoded[usize::from(ids[kind])]))
                .collect()
        };
        let (living, changed) = (read(0), read(1));
        Ok(Self {
            grid: *grid,
            samples_per_cell,
            cell_rows: indices,
            rows: decoded,
            living,
            changed,
            mass_edges_msun: core::array::from_fn(|k| grid.mass_edges_msun(k).0),
            periastron_edges_rsun: (0..grid.periastron_cells())
                .map(|k| grid.periastron_edges_rsun(k).0)
                .collect(),
        })
    }

    /// The table of `cells` as the reader holds it, without packing: what [`Self::unpack`] of
    /// its [`pack`](PairLightCells::pack) gives.
    ///
    /// # Panics
    ///
    /// As [`PairLightCells::pack`] does.
    #[must_use]
    pub fn from_cells(cells: &PairLightCells) -> Self {
        let packed = cells.pack();
        let dimensions = Self::dimensions_of(&cells.grid, packed.row_count, packed.segments);
        Self::unpack(
            &cells.grid,
            FORMAT,
            dimensions,
            cells.samples_per_cell,
            &packed.cell_rows,
            &packed.rows,
        )
        .expect("a packed table reads back")
    }

    /// The grid.
    #[must_use]
    pub const fn grid(&self) -> &PairGrid {
        &self.grid
    }

    /// The cell holding a pair, or `None` outside the table: [`PairGrid::cell_of`] bit for bit,
    /// from the grid's edges as the table holds them, so that it evaluates no logarithm or power.
    #[must_use]
    pub fn cell_of(
        &self,
        heavier_msun: f64,
        lighter_msun: f64,
        fe_h: f64,
        periastron_rsun: f64,
    ) -> Option<usize> {
        self.grid.cell_with(
            |k| self.mass_edges_msun[k],
            |k| self.periastron_edges_rsun[k],
            [heavier_msun, lighter_msun, fe_h, periastron_rsun],
        )
    }

    /// The samples of each cell (the least of any cell).
    #[must_use]
    pub const fn samples_per_cell(&self) -> u32 {
        self.samples_per_cell
    }

    /// The distinct rows.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    /// `cell`'s living value at age bin `bin`, hundredths of a magnitude ([`from_cmag`]).
    ///
    /// It is the brightest V of any living star of its widened neighbourhood's pairs, pair-evolved
    /// or its own model, margin included (and E's reading, [`PairGrid::read_margin_cmag`] and
    /// [`PairGrid::read_smear_bins`]), merged within [`PairGrid::living_merge_tolerance_mag`]:
    /// [`DARK_CMAG`] where none lives, [`UNSEEN_CMAG`] where one lives that has no V. A pair whose
    /// lighter star lies below 0.1 M☉ reads [`Self::living_cmag_for`] instead.
    ///
    /// # Panics
    ///
    /// If `cell` or `bin` is past the table.
    #[must_use]
    pub fn living_cmag(&self, cell: usize, bin: usize) -> i16 {
        self.living[cell][bin]
    }

    /// `cell`'s changed value at age bin `bin`, hundredths of a magnitude ([`from_cmag`]).
    ///
    /// It is the brightest V of any living departing star or product of its widened
    /// neighbourhood's pairs, margin included (and E's reading), merged within
    /// [`merge_tolerance_mag`]: [`DARK_CMAG`] where none lives, [`UNSEEN_CMAG`] where one lives
    /// that has no V. A pair whose lighter star lies below 0.1 M☉ reads
    /// [`Self::changed_cmag_for`] instead.
    ///
    /// # Panics
    ///
    /// If `cell` or `bin` is past the table.
    #[must_use]
    pub fn changed_cmag(&self, cell: usize, bin: usize) -> i16 {
        self.changed[cell][bin]
    }

    /// `cell`'s living value at age bin `bin` as a pair whose lighter star has initial mass
    /// `lighter_msun` (M☉) reads it.
    ///
    /// It is [`Self::living_cmag`], through the cooling floor ([`with_cooling_floor`]) where the
    /// lighter star lies below 0.1 M☉.
    ///
    /// # Panics
    ///
    /// If `cell` or `bin` is past the table.
    #[must_use]
    pub fn living_cmag_for(&self, cell: usize, bin: usize, lighter_msun: f64) -> i16 {
        self.floored(self.living_cmag(cell, bin), cell, bin, lighter_msun)
    }

    /// `cell`'s changed value at age bin `bin` as a pair whose lighter star has initial mass
    /// `lighter_msun` (M☉) reads it: [`Self::changed_cmag`], through the cooling floor where the
    /// lighter star lies below 0.1 M☉ ([`Self::living_cmag_for`]).
    ///
    /// # Panics
    ///
    /// If `cell` or `bin` is past the table.
    #[must_use]
    pub fn changed_cmag_for(&self, cell: usize, bin: usize, lighter_msun: f64) -> i16 {
        self.floored(self.changed_cmag(cell, bin), cell, bin, lighter_msun)
    }

    /// `value` of `cell` at `bin` through the cooling floor, where `lighter_msun` reads it.
    #[must_use]
    fn floored(&self, value: i16, cell: usize, bin: usize, lighter_msun: f64) -> i16 {
        if reads_cooling_floor(lighter_msun) {
            with_cooling_floor(value, self.grid.cell_parts(cell)[2], bin)
        } else {
            value
        }
    }

    /// The class ([`count_class`]) of how many of `cell`'s own samples, of
    /// [`samples_per_cell`](Self::samples_per_cell), hold a living departing star or product at
    /// age bin `bin`.
    ///
    /// # Panics
    ///
    /// If `cell` or `bin` is past the table.
    #[must_use]
    pub fn departing_class(&self, cell: usize, bin: usize) -> u8 {
        u8::try_from(self.rows[usize::from(self.cell_rows[cell][2])][bin])
            .expect("`unpack` holds count classes to 0-32")
    }

    /// The least count of `cell`'s own samples that hold a living departing star or product at
    /// age bin `bin`: 0 for class 0, 2^(c − 1) for class c.
    ///
    /// # Panics
    ///
    /// If `cell` or `bin` is past the table.
    #[must_use]
    pub fn departing_at_least(&self, cell: usize, bin: usize) -> u32 {
        match self.departing_class(cell, bin) {
            0 => 0,
            c => 1_u32 << (c - 1).min(31),
        }
    }
}

#[cfg(test)]
mod tests;
