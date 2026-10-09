//! The phase envelope: the brightest absolute V a single star can have, read at its own initial
//! mass, \[Fe/H\], Reimers η and age relative to its lifetime (rendering plan R06, R06.T8.m;
//! decided 2026-10-05, `decision-r06-census-cost.md`, and split from R06.T8.g on 2026-10-07,
//! `decision-p11-t16-hierarchy-bound.md`). R06.T8.g bounds each star of a record with it before the
//! record is generated (Design note 10).
//!
//! The brightness envelope ([`super::envelope`]) is a running maximum over mass and age: any system
//! past its turnoff holds a companion at the turnoff in it, so it reaches M<sub>V</sub> −5.7 at
//! 10 Gyr for nearly every old system, through the η = 0 tail draw. This table is not maximised
//! over mass, metallicity or η, and it is indexed by the age relative to the star's own lifetime,
//! so a star far from its bright phases takes a faint bound.
//!
//! - **Cells.** A mass interval between two of the brightness envelope's nodes
//!   ([`mass_nodes`](super::envelope::mass_nodes), 197 intervals over 0.0124–150 M☉), an interval
//!   of its \[Fe/H\] nodes ([`FE_H_NODES`], 11 over −2.5 to +0.18, read at the metal fraction the
//!   tracks see), an η interval of [`ETA_NODES`] (3, from η = 0 to +7σ), and a bin of relative age
//!   ([`BINS`]): one below [`FIRST_RELATIVE_AGE`], [`COARSE_BINS`] even in log to [`FINE_START`],
//!   [`FINE_BINS`] of [`FINE_BIN_WIDTH`] over 0.8–1.05, where nearly every star's giant phases and
//!   death lie, and one beyond, where only remnants are expected (dark until plan 06's ask A4),
//!   which the spread lights in 64 cells. Stars of 1.9–3.3 M☉ at \[Fe/H\] −0.5 to +0.18 leave
//!   their main sequence at 0.77–0.80, in the last coarse bin (0.64–0.80), so a third of their main
//!   sequence is bounded 1.5–2 mag too bright (R06's Risks, a deferred correction).
//! - **Values.** Each cell holds the brightest V of the single stars sampled within it:
//!   [`MASS_SAMPLES`] masses even in ln m, [`FE_H_SAMPLES`] \[Fe/H\] and [`ETA_SAMPLES`] η, each
//!   interval's ends among them. Each sample's track is cut into parts as the brightness envelope
//!   cuts it ([`SAMPLES_PER_PHASE`] parts a phase, and its knots, each part the brightest of five
//!   points), and each part enters every bin its relative ages overlap. Within the
//!   electron-capture window ([`STRIPPED_WINDOW`]) a sample is the brighter of the star with and
//!   without plan 06's companion-stripped mark, which moves its death by up to 1.05%.
//! - **Spread.** Each cell is then widened in relative age by the least of [`SPREADS`] under which
//!   each of its samples with neighbours on both sides along an axis is bounded by those two
//!   alone, margin included: a test at twice the samples' spacing of how far their phases drift.
//!   The slow test found a massive star's excursions moving by up to 3% of its lifetime over
//!   0.18 dex of \[Fe/H\], and lifetimes kinking by up to 1.3% between their nodes at a change of
//!   route (a strong-winded solar-mass star becoming a helium star). Most cells keep the least
//!   spread, half a fine bin.
//! - **Margin and merging.** Then [`MARGIN_MAG`] is taken off, and runs of bins within
//!   `merge_tolerance` of their brightest (0.2 mag, growing fainter than M<sub>V</sub> +10,
//!   which only layer A and the brown dwarfs could list) are stored as one segment at their
//!   brightest, in integer millimagnitudes rounded brighter (`sky::envelope::to_millimag`), as
//!   `sky_envelope` is. Widening, merging and rounding only brighten, so the table bounds its samples.
//! - **Storage, format [`FORMAT`].** Cells with the same segments share one row. The rows are
//!   packed four base64 characters a segment: its run of bins (a byte), then its value (an `i16`,
//!   little-endian), since the giant branches climb several magnitudes within the fine bins and a
//!   cell needs some twenty segments, so the table as decimal would be about a megabyte.
//!   [`PhaseEnvelope::fitted`] checks the format and the dimensions and decodes the rows once.
//! - **Relative age.** A star's relative age is its age over its lifetime L as the table holds it:
//!   the death age of the generator's own track at each node of mass, \[Fe/H\] and η (the median
//!   draws but η), capped at [`LIFETIME_CAP_YEARS`], interpolated linearly in ln L over ln m,
//!   \[Fe/H\] and the η draw within the star's cell ([`PhaseFrame`]). The fit takes every
//!   sample's relative age by the same interpolation, and the reader reads one bin an age, the
//!   lifetime's own errors being the spread's to cover.
//!
//! The ruling read the relative age through `FittedFates::lifetime_bracket` (P06.T38.d), over the
//! bracket's span. That bracket starts at 0.741 M☉; answers only within its η nodes, −3σ to +3σ
//! (±2.4σ above 8 M☉), so not at η = 0 or ±3.5σ; refuses changes of route; allocates; and reaches
//! ±1.4% at its 99th percentile below 2.5 M☉ and ±2% in a usable cell (14–20 fine bins). So the
//! table
//! folds the lifetime into its own index instead (the ruling's first option): a reading costs a
//! few multiplications and no allocation, and reaches every mass. A star below 0.741 M☉ is bounded
//! as any other, against its track's lifetime; below 0.1 M☉, and where a track outlives the cap,
//! against the cap, 2 × 10¹⁰ years, so its relative age never reaches the fine bins.
//!
//! The table depends on no galaxy: `hyperion-fit`'s task `sky_phase_envelope` builds it from
//! [`node_lifetime`], [`sample_parts`] and [`PhaseEnvelope::assemble`], and checks it in as
//! [`tables::sky_phase_envelope`](crate::tables::sky_phase_envelope). A change to the tracks that
//! would leave it stale fails the fit's sim fingerprint in `just fit-check`, and the slow
//! `phase_envelope_bounds_dense_tracks` holds random tracks to it.
//!
//! It bounds this generator's single stars. A star of η above +7σ (one in 10¹²) is read at +7σ,
//! outside what the slow test checks, as the brightness envelope reads it. The table is built at no
//! helium excess, as the brightness envelope is, which the provisional `tables::helium` (P06.T17)
//! leaves without effect: a star with one reads the table's brightest value, which skips nothing,
//! and when plan 15's P15.T7 fits that table, this one must be refitted with helium-excess nodes.
//! Pair evolution is plan 11's `pair_light_bound`'s (P11.T17), which bounds only the stars that
//! depart from their own models.

use std::collections::BTreeMap;

use crate::galaxy::displaced::binarity::NEVER_STRIPPED;
use crate::math;
use crate::rng::Mark;
use crate::stellar::Composition;
use crate::stellar::composition::Z_SOLAR;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::fates::STRIPPED_WINDOW;
use crate::stellar::sse::{self, MIN_INITIAL_MASS, Track};
use crate::stellar::substellar;
use crate::units::{Dex, HeliumExcess, Magnitudes, SolarMasses, Years};

use crate::tables::{sky_envelope, sky_phase_envelope};

use super::envelope::{
    COOLING_TOP_MSUN, ETA_DRAWS, FE_H_NODES, MARGIN_MAG, MAX_AGE_YEARS, SAMPLES_PER_PHASE,
    to_millimag, track_parts,
};
use super::photometry::absolute_v_of_state;

/// The nodes of the η axis, as standard-normal draws (η = 0.5 + 0.07 z, plan 06's design note 7).
///
/// They are the brightness envelope's [`ETA_DRAWS`] but +3.5σ, so three intervals: the tail from η = 0, no
/// Reimers wind (z = −0.5 ÷ 0.07), whose late giants are the brightest, to −3.5σ, one star in
/// 4,000; then −3.5σ to the median, and the median to +7σ. A draw below the first reads as the
/// first, which the tracks treat alike (η clamps at 0), and one above the last as the last. The
/// envelope's +3.5σ node is left out to keep the table small: it would split only the 2 × 10⁻⁴ of
/// stars above it, whose winds are stronger and giants fainter.
pub const ETA_NODES: [f64; 4] = [ETA_DRAWS[0], ETA_DRAWS[1], ETA_DRAWS[2], ETA_DRAWS[4]];

/// The masses sampled in each mass interval, even in ln m, both ends among them.
pub const MASS_SAMPLES: usize = 8;

/// The \[Fe/H\] sampled in each interval: its ends and its middle.
pub const FE_H_SAMPLES: usize = 3;

/// The η draws sampled in each interval: its ends and its middle.
pub const ETA_SAMPLES: usize = 3;

/// The relative age below which every star shares the first bin: about 1,300 years at the Sun's
/// lifetime in the table (1.29 × 10¹⁰ years) and 2,000 years at [`LIFETIME_CAP_YEARS`], within
/// every star's dark protostar phase of 0.5 Myr.
pub const FIRST_RELATIVE_AGE: f64 = 1e-7;

/// The bins even in log relative age from [`FIRST_RELATIVE_AGE`] to [`FINE_START`]: about 0.1 dex
/// each, over the pre-main sequence, the main sequence and a brown dwarf's cooling.
pub const COARSE_BINS: usize = 70;

/// The relative age at which the fine bins start, as ruled.
///
/// Before it nearly every star is on its main sequence or younger; stars of 1.9–3.3 M☉ at
/// \[Fe/H\] −0.5 to +0.18 leave theirs at 0.77–0.80 (see the [module](self) documentation).
pub const FINE_START: f64 = 0.8;

/// The width of a fine bin, in relative age.
pub const FINE_BIN_WIDTH: f64 = 1e-3;

/// The fine bins, over 0.8–1.05: past every star's death at the table's lifetime, with the
/// interpolation's slack.
pub const FINE_BINS: usize = 250;

/// The bins of relative age: one below [`FIRST_RELATIVE_AGE`], [`COARSE_BINS`], [`FINE_BINS`] and
/// one from 1.05 on.
pub const BINS: usize = COARSE_BINS + FINE_BINS + 2;

/// The first fine bin's index.
const FIRST_FINE_BIN: usize = COARSE_BINS + 1;

/// The longest lifetime the table holds, years: a star that lives longer is read against it, so
/// its relative age never passes 0.75 within [`MAX_AGE_YEARS`] and stays in the coarse bins, as
/// does every brown dwarf's.
pub const LIFETIME_CAP_YEARS: f64 = 2e10;

/// The spreads in relative age a cell may be widened by, ascending.
///
/// Each cell takes the least under which every sample with neighbours on both sides along an axis
/// is bounded by those two neighbours alone, margin included ([`rows_of_interval`]). That tests the cell at twice its
/// samples' spacing, so a star between two samples, whose phases fall between theirs, is bounded
/// with room to spare. The least, half a fine bin at a relative age of 1, covers the drift
/// within a bin; the largest, 6.4%, covers a lifetime's kink between the nodes at a change of
/// route, such as a strong-winded star's becoming a helium star.
pub const SPREADS: [f64; 8] = [5e-4, 1e-3, 2e-3, 4e-3, 8e-3, 1.6e-2, 3.2e-2, 6.4e-2];

/// The most, mag, by which the bins of one stored segment may differ where its brightest is at
/// or brighter than [`FAINT_FROM_MAG`]: a segment holds its brightest, so the table is looser by
/// at most this where it merges.
pub const MERGE_TOLERANCE_MAG: f64 = 0.2;

/// The magnitude past which the merge tolerance grows: M<sub>V</sub> +10, which only layer A's
/// systems (cap about 11 ly, where the cut 8 reaches M<sub>V</sub> +10.4) and the brown dwarfs'
/// (2 ly) could list.
pub const FAINT_FROM_MAG: f64 = 10.0;

/// How fast, mag a mag, the merge tolerance grows past [`FAINT_FROM_MAG`].
pub const FAINT_TOLERANCE_SLOPE: f64 = 0.2;

/// The merge tolerance, mag, for a segment whose brightest bin is `v`, mag.
#[must_use]
pub(crate) fn merge_tolerance(v: f64) -> f64 {
    MERGE_TOLERANCE_MAG + FAINT_TOLERANCE_SLOPE * (v - FAINT_FROM_MAG).max(0.0)
}

/// The lifetimes' unit in the fitted table: 10⁻⁴ dex of log₁₀ years below the cap.
pub const LIFETIME_UNITS_PER_DEX: f64 = 1e4;

/// The bin of relative age `r` (see [`BINS`]); 0 for a NaN.
#[must_use]
pub(crate) fn relative_age_bin(r: f64) -> usize {
    if r.is_nan() || r < FIRST_RELATIVE_AGE {
        return 0;
    }
    if r < FINE_START {
        let x = math::ln(r / FIRST_RELATIVE_AGE) / math::ln(FINE_START / FIRST_RELATIVE_AGE);
        #[expect(clippy::cast_precision_loss, reason = "70 bins")]
        let bins = COARSE_BINS as f64;
        let k = (x * bins).floor().clamp(0.0, bins - 1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "k is clamped into [0, 70)"
        )]
        let k = k as usize;
        return 1 + k;
    }
    let k = ((r - FINE_START) / FINE_BIN_WIDTH).floor();
    #[expect(clippy::cast_precision_loss, reason = "250 bins")]
    if k >= FINE_BINS as f64 {
        return BINS - 1;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "k lies in [0, 250)"
    )]
    let k = k.max(0.0) as usize;
    FIRST_FINE_BIN + k
}

/// The least relative age of bin `k` (see [`BINS`]).
///
/// # Panics
///
/// If `k` is not below [`BINS`].
#[must_use]
pub(crate) fn bin_start(k: usize) -> f64 {
    assert!(k < BINS, "bin {k} of {BINS}");
    match k {
        0 => 0.0,
        k if k < FIRST_FINE_BIN => {
            #[expect(clippy::cast_precision_loss, reason = "under 70")]
            let x = (k - 1) as f64 / COARSE_BINS as f64;
            FIRST_RELATIVE_AGE * math::exp(x * math::ln(FINE_START / FIRST_RELATIVE_AGE))
        }
        k => {
            #[expect(clippy::cast_precision_loss, reason = "under 251")]
            let x = (k - FIRST_FINE_BIN) as f64;
            FINE_START + x * FINE_BIN_WIDTH
        }
    }
}

/// The relative age of `age`, years, against the lifetime whose natural logarithm is `ln_lifetime`:
/// what the fit and the reader both take, so they agree bit for bit.
#[must_use]
pub(crate) fn relative_age(age: f64, ln_lifetime: f64) -> f64 {
    age / math::exp(ln_lifetime)
}

/// The \[Fe/H\] coordinate of `composition` on the table's axis, dex.
///
/// It is log₁₀ of the metal fraction the tracks read ([`Composition::z_fit`]) over the solar 0.02,
/// so every composition with one track has one coordinate, −2.30 to +0.18.
#[must_use]
pub(crate) fn fe_h_coordinate(composition: &Composition) -> Dex {
    Dex::new(math::log10(composition.z_fit().value() / Z_SOLAR.value()))
}

/// One cell of the table: its mass, \[Fe/H\] and η intervals.
///
/// A cell is made only by the frame ([`PhaseFrame::cell_of`]'s crate-wide reader), by
/// [`rows_of_interval`] and by [`Cell::from_index`], so its intervals lie within the table's axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    /// The mass interval, from the lighter node.
    mass: usize,
    /// The \[Fe/H\] interval, from the poorer node.
    fe_h: usize,
    /// The η interval, from the lower node.
    eta: usize,
}

/// The \[Fe/H\] intervals of a mass interval's cells.
const FE_H_CELLS: usize = FE_H_NODES.len() - 1;

/// The η intervals of an \[Fe/H\] interval's cells.
const ETA_CELLS: usize = ETA_NODES.len() - 1;

impl Cell {
    /// The cell of index `index` ([`Cell::index`]) among `cells` cells, or `None` past them.
    #[must_use]
    pub const fn from_index(index: usize, cells: usize) -> Option<Self> {
        if index >= cells {
            return None;
        }
        Some(Self {
            mass: index / (FE_H_CELLS * ETA_CELLS),
            fe_h: index / ETA_CELLS % FE_H_CELLS,
            eta: index % ETA_CELLS,
        })
    }

    /// The cell's index among the table's cells: mass, then \[Fe/H\], then η.
    #[must_use]
    pub const fn index(self) -> usize {
        (self.mass * FE_H_CELLS + self.fe_h) * ETA_CELLS + self.eta
    }

    /// The mass interval, from the lighter node of the brightness envelope's.
    #[must_use]
    pub const fn mass(self) -> usize {
        self.mass
    }

    /// The \[Fe/H\] interval, from the poorer of [`FE_H_NODES`].
    #[must_use]
    pub const fn fe_h(self) -> usize {
        self.fe_h
    }

    /// The η interval, from the lower of [`ETA_NODES`].
    #[must_use]
    pub const fn eta(self) -> usize {
        self.eta
    }
}

/// Where a star lies in the table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Coordinates {
    /// Initial mass.
    pub(crate) mass: SolarMasses,
    /// [`fe_h_coordinate`].
    pub(crate) fe_h: Dex,
    /// The η draw.
    pub(crate) eta: StandardNormal,
}

/// The axes of the table and its lifetimes: which cell holds a star and its lifetime there.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseFrame {
    /// The mass nodes, M☉, ascending.
    masses: Vec<f64>,
    /// Their natural logarithms.
    ln_masses: Vec<f64>,
    /// ln L at each node of mass, \[Fe/H\] and η, in that order of nesting, years.
    ln_lifetimes: Vec<f64>,
}

impl PhaseFrame {
    /// The frame of `masses` (the brightness envelope's nodes) with the lifetimes `lifetimes`, the
    /// table's integers ([`quantize_lifetime`]), one per node of mass, \[Fe/H\] and η in that
    /// order of nesting.
    ///
    /// # Panics
    ///
    /// If `masses` has fewer than two nodes or does not increase, or `lifetimes` is not one per
    /// node.
    #[doc(hidden)]
    #[must_use]
    pub fn new(masses: Vec<f64>, lifetimes: &[u16]) -> Self {
        assert!(
            masses.len() >= 2 && masses.windows(2).all(|w| w[0] < w[1]),
            "the mass nodes increase"
        );
        assert_eq!(
            lifetimes.len(),
            masses.len() * FE_H_NODES.len() * ETA_NODES.len(),
            "one lifetime a node"
        );
        let ln_masses = masses.iter().map(|&m| math::ln(m)).collect();
        let ln_lifetimes = lifetimes.iter().map(|&k| dequantize_lifetime(k)).collect();
        Self {
            masses,
            ln_masses,
            ln_lifetimes,
        }
    }

    /// The number of cells.
    #[must_use]
    pub fn cells(&self) -> usize {
        (self.masses.len() - 1) * (FE_H_NODES.len() - 1) * (ETA_NODES.len() - 1)
    }

    /// The mass nodes, M☉.
    #[must_use]
    pub fn masses(&self) -> &[f64] {
        &self.masses
    }

    /// The cell holding `at`.
    ///
    /// Each coordinate is clamped into its axis, and one on a node lies in the interval above it
    /// (the last node in the last interval). A NaN reads the first interval; the reader handles a
    /// NaN or non-positive mass before it asks.
    #[must_use]
    pub(crate) fn cell_of(&self, at: Coordinates) -> Cell {
        Cell {
            mass: interval(&self.masses, at.mass.value()),
            fe_h: interval(&FE_H_NODES, at.fe_h.value()),
            eta: interval(&ETA_NODES, at.eta.value()),
        }
    }

    /// ln L, years, of a star at `at` in `cell`.
    ///
    /// The nodes' ln L is interpolated linearly in ln m, the \[Fe/H\] coordinate and the η draw,
    /// each fraction clamped into the cell, so a sample on a boundary has the boundary's lifetime
    /// in either cell.
    ///
    /// # Panics
    ///
    /// If `cell` lies outside the frame's mass intervals.
    #[must_use]
    pub(crate) fn ln_lifetime_in(&self, cell: Cell, at: Coordinates) -> f64 {
        let fraction = |lo: f64, hi: f64, v: f64| ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
        let tm = fraction(
            self.ln_masses[cell.mass],
            self.ln_masses[cell.mass + 1],
            math::ln(at.mass.value()),
        );
        let tz = fraction(
            FE_H_NODES[cell.fe_h],
            FE_H_NODES[cell.fe_h + 1],
            at.fe_h.value(),
        );
        let te = fraction(ETA_NODES[cell.eta], ETA_NODES[cell.eta + 1], at.eta.value());
        let node = |j: usize, i: usize, k: usize| {
            self.ln_lifetimes[(j * FE_H_NODES.len() + i) * ETA_NODES.len() + k]
        };
        let lerp = |a: f64, b: f64, t: f64| (1.0 - t) * a + t * b;
        let (j, i, k) = (cell.mass, cell.fe_h, cell.eta);
        let along_eta = |j: usize, i: usize| lerp(node(j, i, k), node(j, i, k + 1), te);
        let along_fe_h = |j: usize| lerp(along_eta(j, i), along_eta(j, i + 1), tz);
        lerp(along_fe_h(j), along_fe_h(j + 1), tm)
    }
}

/// The interval of `nodes` holding `v`: the last whose lower node is at or below it, within
/// `0..nodes.len() − 1`.
#[must_use]
fn interval(nodes: &[f64], v: f64) -> usize {
    nodes
        .partition_point(|&node| node <= v)
        .saturating_sub(1)
        .min(nodes.len() - 2)
}

/// The table's integer for a lifetime: how far below [`LIFETIME_CAP_YEARS`] it lies.
///
/// The integer is in [`LIFETIME_UNITS_PER_DEX`] of log₁₀ years, rounded to the nearest, and 0 at or
/// above the cap. The table's relative ages are taken against this integer, by the fit and the
/// reader alike, so its rounding (at most 1.2 × 10⁻⁴ of the lifetime) moves no bound.
///
/// # Panics
///
/// If `years` is NaN, not positive, or shorter than about 5,600 years, 6.55 dex below the cap,
/// beyond a `u16` (the shortest track lives some 3 × 10⁶ years).
#[doc(hidden)]
#[must_use]
pub fn quantize_lifetime(years: Years) -> u16 {
    let years = years.value();
    assert!(years > 0.0, "a lifetime is positive: {years}");
    let below = math::log10(LIFETIME_CAP_YEARS) - math::log10(years.min(LIFETIME_CAP_YEARS));
    let units = (below * LIFETIME_UNITS_PER_DEX).round();
    assert!(
        units <= f64::from(u16::MAX),
        "a lifetime of {years} years is within 6.55 dex of the cap"
    );
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "units is an integer from 0 to u16::MAX, which the assertion bounds"
    )]
    let k = units as u16;
    k
}

/// ln L, years, of the table's integer `k` ([`quantize_lifetime`]).
#[must_use]
pub(crate) fn dequantize_lifetime(k: u16) -> f64 {
    math::ln(LIFETIME_CAP_YEARS) - f64::from(k) / LIFETIME_UNITS_PER_DEX * core::f64::consts::LN_10
}

/// The draws of a sample at the η draw `eta`: the median draws but η, as the brightness envelope's
/// and the fate table's nodes take them, with the companion-stripped mark `stripped`.
#[must_use]
fn sample_draws(eta: StandardNormal, stripped: Mark) -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        eta,
        stripped,
        ..StarDrawsParts::MEDIAN
    })
}

/// A companion-stripped mark that is set wherever the mark can matter: below every share.
const STRIPPED: Mark = Mark::from_word(0);

/// The lifetime the table holds at a node of mass `m`, \[Fe/H\] `fe_h` and η draw `eta`.
///
/// It is the death age of the generator's track at the median draws but η ([`sse::lifetime`]),
/// capped at [`LIFETIME_CAP_YEARS`]; the cap below 0.1 M☉, where the cooling fits never die.
#[doc(hidden)]
#[must_use]
pub fn node_lifetime(m: SolarMasses, fe_h: Dex, eta: StandardNormal) -> Years {
    if m < MIN_INITIAL_MASS {
        return Years::new(LIFETIME_CAP_YEARS);
    }
    let composition = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
    Years::new(
        sse::lifetime(m, &composition, &sample_draws(eta, NEVER_STRIPPED))
            .value()
            .min(LIFETIME_CAP_YEARS),
    )
}

/// The table's lifetimes at every node of `masses` (M☉), [`FE_H_NODES`] and [`ETA_NODES`], in the
/// frame's order: [`node_lifetime`], [`quantize_lifetime`]d.
///
/// The table's 198 × 12 × 4 = 9,504 take a few seconds on one thread.
///
/// # Panics
///
/// As [`quantize_lifetime`], for a lifetime the tracks do not give.
#[doc(hidden)]
#[must_use]
pub fn node_lifetimes(masses: &[f64]) -> Vec<u16> {
    masses
        .iter()
        .flat_map(|&m| {
            FE_H_NODES.iter().flat_map(move |&z| {
                ETA_NODES.iter().map(move |&e| {
                    let eta = StandardNormal::new(e).expect("a node is finite");
                    quantize_lifetime(node_lifetime(SolarMasses::new(m), Dex::new(z), eta))
                })
            })
        })
        .collect()
}

/// The `k`-th of [`MASS_SAMPLES`] masses of the interval from node `lo` to node `hi`: even in
/// ln m, the ends exact.
#[doc(hidden)]
#[must_use]
pub fn mass_sample(lo: SolarMasses, hi: SolarMasses, k: usize) -> SolarMasses {
    match k {
        0 => lo,
        k if k + 1 >= MASS_SAMPLES => hi,
        k => {
            #[expect(clippy::cast_precision_loss, reason = "k < 8")]
            let t = k as f64 / (MASS_SAMPLES - 1) as f64;
            let (lo, hi) = (math::ln(lo.value()), math::ln(hi.value()));
            SolarMasses::new(math::exp(lo + (hi - lo) * t))
        }
    }
}

/// The `k`-th of three samples of the interval from `lo` to `hi`: its ends and its middle.
#[must_use]
fn three_samples(lo: f64, hi: f64, k: usize) -> f64 {
    match k {
        0 => lo,
        1 => f64::midpoint(lo, hi),
        _ => hi,
    }
}

/// One part of a sample's life: an age range and the brightest absolute V over it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplePart {
    /// The part's first age.
    from: Years,
    /// Its last age.
    to: Years,
    /// The brightest absolute V the fit read over it.
    brightest: Magnitudes,
}

impl SamplePart {
    /// The part of ages `from` to `to`, years, with brightest V `v`.
    #[must_use]
    const fn of((from, to, v): (f64, f64, f64)) -> Self {
        Self {
            from: Years::new(from),
            to: Years::new(to),
            brightest: Magnitudes::new(v),
        }
    }

    /// The part's first age.
    #[must_use]
    pub const fn from(&self) -> Years {
        self.from
    }

    /// Its last age.
    #[must_use]
    pub const fn to(&self) -> Years {
        self.to
    }

    /// The brightest absolute V the fit read over it.
    #[must_use]
    pub const fn brightest(&self) -> Magnitudes {
        self.brightest
    }
}

/// A sample's life cut into parts, each an age range and the brightest absolute V over it.
///
/// A star of at least 0.1 M☉ is its track to [`MAX_AGE_YEARS`] cut as the brightness envelope cuts
/// it ([`SAMPLES_PER_PHASE`]), and within [`STRIPPED_WINDOW`] the track with the companion-stripped
/// mark set too, whose death may come later. One at or below 0.1 M☉ is the cooling fits (read at
/// 0.099 999 M☉ at 0.1 itself, where both answer), which fade with age, cut at the relative-age
/// bins' edges against [`LIFETIME_CAP_YEARS`], each part the brightest of its young edge and two
/// points within. A pure function of its arguments.
#[doc(hidden)]
#[must_use]
pub fn sample_parts(m: SolarMasses, fe_h: Dex, eta: StandardNormal) -> Vec<SamplePart> {
    let composition = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
    let mut parts = Vec::new();
    if m <= MIN_INITIAL_MASS {
        cooling_parts(m.value().min(COOLING_TOP_MSUN), &composition, &mut parts);
        if m < MIN_INITIAL_MASS {
            return parts;
        }
    }
    let marks: &[Mark] = if (STRIPPED_WINDOW.0..=STRIPPED_WINDOW.1).contains(&m.value()) {
        &[NEVER_STRIPPED, STRIPPED]
    } else {
        &[NEVER_STRIPPED]
    };
    for &mark in marks {
        let track = Track::to_age(
            m,
            &composition,
            &sample_draws(eta, mark),
            Years::new(MAX_AGE_YEARS),
        );
        parts.extend(
            track_parts(&track, SAMPLES_PER_PHASE)
                .into_iter()
                .map(SamplePart::of),
        );
    }
    parts
}

/// Appends a cooling object's parts: each coarse bin's ages at the cap, up to [`MAX_AGE_YEARS`].
fn cooling_parts(mass: f64, composition: &Composition, parts: &mut Vec<SamplePart>) {
    let v_at = |age: f64| {
        substellar::cooling(SolarMasses::new(mass), Years::new(age), composition)
            .ok()
            .and_then(|state| absolute_v_of_state(&state))
            .map_or(f64::INFINITY, Magnitudes::value)
    };
    for k in 0..FIRST_FINE_BIN {
        let from = bin_start(k) * LIFETIME_CAP_YEARS;
        if from > MAX_AGE_YEARS {
            break;
        }
        let to = (bin_start(k + 1) * LIFETIME_CAP_YEARS).min(MAX_AGE_YEARS);
        let young = from.max(1.0);
        let v = [
            young,
            young + 0.25 * (to - young),
            young + 0.5 * (to - young),
        ]
        .into_iter()
        .map(v_at)
        .fold(f64::INFINITY, f64::min);
        if v.is_finite() {
            parts.push(SamplePart::of((from, to, v)));
        }
    }
}

/// Enters `parts` (from [`sample_parts`]) in `row`, a cell's bins, against the lifetime whose
/// natural logarithm is `ln_lifetime`: each part's relative ages take its V in every bin they
/// overlap.
///
/// # Panics
///
/// If a part ends before it starts, which [`sample_parts`] rules out.
fn enter_parts(parts: &[SamplePart], ln_lifetime: f64, row: &mut [f64; BINS]) {
    for part in parts {
        let lo = relative_age_bin(relative_age(part.from.value(), ln_lifetime));
        let hi = relative_age_bin(relative_age(part.to.value(), ln_lifetime));
        for bin in &mut row[lo..=hi] {
            *bin = bin.min(part.brightest.value());
        }
    }
}

/// The packed rows' value of a segment where no single star shines, millimagnitudes: the fitted
/// table's `DARK`, `i16::MAX`.
pub(crate) const DARK_MMAG: i16 = sky_phase_envelope::DARK;

/// The version of the fitted table's format, which [`PhaseEnvelope::fitted`] checks: 1, the packed
/// rows of four base64 characters a segment described in
/// [`tables::sky_phase_envelope`](crate::tables::sky_phase_envelope).
pub const FORMAT: u32 = 1;

/// The longest run of bins one packed segment holds; a longer run is stored as several.
const LONGEST_RUN: usize = 255;

/// The alphabet of the packed rows: RFC 4648's base64 (its §4), without padding.
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Appends one segment packed: its run of bins (1–255) and its value (millimagnitudes, as an
/// `i16`), three bytes little-endian after the run, as four base64 characters.
fn pack_segment(run: u8, mmag: i16, out: &mut String) {
    let [lo, hi] = mmag.to_le_bytes();
    let word = (u32::from(run) << 16) | (u32::from(lo) << 8) | u32::from(hi);
    for shift in [18, 12, 6, 0] {
        let k = usize::try_from((word >> shift) & 0x3f).expect("six bits");
        out.push(char::from(BASE64[k]));
    }
}

/// The run and value of one packed segment, four base64 characters.
///
/// # Panics
///
/// If a character is outside the alphabet.
#[must_use]
fn unpack_segment(chars: [u8; 4]) -> (u8, i16) {
    let mut word = 0_u32;
    for c in chars {
        let k = BASE64
            .iter()
            .position(|&b| b == c)
            .expect("a packed row holds base64 characters only");
        word = (word << 6) | u32::try_from(k).expect("under 64");
    }
    let [_, run, lo, hi] = word.to_be_bytes();
    (run, i16::from_le_bytes([lo, hi]))
}

/// The segments of packed rows (ASCII whitespace ignored): per row its first segment, then one past
/// the last; per segment its last bin and its value, millimagnitudes. A row ends where its runs
/// reach the last bin.
///
/// # Panics
///
/// If the text is not whole segments of whole rows: a character outside the alphabet, a run of
/// zero bins, or a row that passes the last bin or is cut short.
#[must_use]
fn unpack_rows(text: &str) -> (Vec<u32>, Vec<u16>, Vec<i16>) {
    let chars: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    assert!(chars.len().is_multiple_of(4), "whole packed segments");
    let (mut offsets, mut ends, mut mmag) = (vec![0_u32], Vec::new(), Vec::new());
    let mut next = 0_usize;
    let (groups, _) = chars.as_chunks::<4>();
    for &group in groups {
        let (run, value) = unpack_segment(group);
        assert!(run > 0, "a segment holds a bin at least");
        let end = next + usize::from(run) - 1;
        assert!(end < BINS, "a row ends at the last bin");
        ends.push(u16::try_from(end).expect("under 400 bins"));
        mmag.push(value);
        next = end + 1;
        if next == BINS {
            offsets.push(u32::try_from(ends.len()).expect("under 2^32 segments"));
            next = 0;
        }
    }
    assert_eq!(next, 0, "the last row is whole");
    (offsets, ends, mmag)
}

/// The brightest absolute V a single star can have, by its own initial mass, \[Fe/H\], η draw
/// and age relative to its lifetime (see the [module](self) documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseEnvelope {
    /// The axes and the lifetimes.
    frame: PhaseFrame,
    /// Per cell, its row among the distinct rows.
    cell_rows: Vec<u16>,
    /// Per distinct row, its first segment; then one past the last.
    row_offsets: Vec<u32>,
    /// Per segment, its last bin; a row's last segment ends at the last bin.
    ends: Vec<u16>,
    /// Per segment, the brightest M<sub>V</sub> of its bins, margin included, in millimagnitudes
    /// rounded brighter; `i16::MAX` where none shines.
    mmag: Vec<i16>,
    /// The brightest value of the table, millimagnitudes.
    brightest_ever: i16,
}

impl PhaseEnvelope {
    /// The envelope of the fitted table, its packed rows decoded once.
    ///
    /// The table is [`tables::sky_phase_envelope`](crate::tables::sky_phase_envelope), on the
    /// brightness envelope's mass nodes
    /// ([`tables::sky_envelope::MASSES`](crate::tables::sky_envelope::MASSES)). The census reads
    /// the process's one copy, [`shared`](Self::shared).
    ///
    /// # Panics
    ///
    /// If the table's format is not [`FORMAT`]; if its axes, distinct rows or segments disagree with
    /// its `DIMENSIONS` or the reader's axes; if its packed rows are not whole segments of whole
    /// rows; or if a cell names a row that is not there. The fit rules each out.
    #[must_use]
    pub fn fitted() -> Self {
        assert_eq!(
            sky_phase_envelope::FORMAT,
            FORMAT,
            "the fitted table's format is the reader's"
        );
        let [masses, fe_h, eta, bins, rows, segments] = sky_phase_envelope::DIMENSIONS;
        assert_eq!(
            [masses, fe_h, eta, bins],
            [
                sky_envelope::MASSES.len(),
                FE_H_NODES.len(),
                ETA_NODES.len(),
                BINS
            ],
            "the fitted table's axes are the reader's"
        );
        let frame = PhaseFrame::new(
            sky_envelope::MASSES.to_vec(),
            &sky_phase_envelope::LIFETIMES,
        );
        let (row_offsets, ends, mmag) = unpack_rows(sky_phase_envelope::ROWS);
        assert_eq!(
            [row_offsets.len() - 1, ends.len()],
            [rows, segments],
            "the fitted table's rows and segments are those its header states"
        );
        Self::from_parts(
            frame,
            sky_phase_envelope::CELL_ROWS.to_vec(),
            row_offsets,
            ends,
            mmag,
        )
    }

    /// The process's one envelope of the fitted table, decoded lazily on its first read.
    ///
    /// It is [`fitted`](Self::fitted), bit for bit, whoever reads it first: the table depends on no
    /// galaxy, its decoding draws nothing and never reads this, and it never changes afterwards
    /// (the sim-determinism skill's lazy-value exception; the test
    /// `the_shared_envelope_is_the_fitted_table` pins it). The census bounds every star of every
    /// record with it (R06.T8.g), so a process decodes the table once, about 0.5 MB, rather than
    /// once per census job, and the census's context carries nothing more for it.
    ///
    /// # Panics
    ///
    /// As [`fitted`](Self::fitted), on the first read.
    #[must_use]
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<PhaseEnvelope> = std::sync::OnceLock::new();
        SHARED.get_or_init(Self::fitted)
    }

    /// The envelope of `frame` from each cell's brightest V per bin, `rows[cell]`, before the
    /// margin (+∞ where none shines): each bin brightened by [`MARGIN_MAG`], runs within
    /// [`merge_tolerance`] of their brightest merged at their brightest, each value rounded
    /// brighter to the millimagnitude ([`to_millimag`]) and fainter values brightened to the
    /// faintest the packed rows hold, 32.766 mag. Cells with the same segments share one row, so
    /// the result is what the fitted table stores.
    ///
    /// # Panics
    ///
    /// If `rows` is not one per cell, holds more than 65,536 distinct rows, or holds a value
    /// brighter than −32.768 mag, which no star is.
    #[doc(hidden)]
    #[must_use]
    pub fn assemble(frame: PhaseFrame, rows: &[[f64; BINS]]) -> Self {
        assert_eq!(rows.len(), frame.cells(), "one row a cell");
        let mut distinct: BTreeMap<Vec<(u16, i16)>, u16> = BTreeMap::new();
        let mut order: Vec<Vec<(u16, i16)>> = Vec::new();
        let mut cell_rows = Vec::with_capacity(rows.len());
        for row in rows {
            let stored = stored_segments(row);
            let id = *distinct.entry(stored.clone()).or_insert_with(|| {
                order.push(stored);
                u16::try_from(order.len() - 1).expect("at most 65,536 distinct rows")
            });
            cell_rows.push(id);
        }
        let (mut row_offsets, mut ends, mut mmag) = (vec![0_u32], Vec::new(), Vec::new());
        for row in &order {
            for &(end, k) in row {
                ends.push(end);
                mmag.push(k);
            }
            row_offsets.push(u32::try_from(ends.len()).expect("under 2^32 segments"));
        }
        Self::from_parts(frame, cell_rows, row_offsets, ends, mmag)
    }

    /// The envelope of its parts.
    ///
    /// # Panics
    ///
    /// If `cell_rows` is not one per cell of `frame` or names a row that is not there.
    #[must_use]
    fn from_parts(
        frame: PhaseFrame,
        cell_rows: Vec<u16>,
        row_offsets: Vec<u32>,
        ends: Vec<u16>,
        mmag: Vec<i16>,
    ) -> Self {
        assert_eq!(cell_rows.len(), frame.cells(), "a row a cell");
        assert!(
            cell_rows
                .iter()
                .all(|&r| usize::from(r) + 1 < row_offsets.len()),
            "every cell's row is there"
        );
        let brightest_ever = mmag.iter().copied().min().unwrap_or(DARK_MMAG);
        Self {
            frame,
            cell_rows,
            row_offsets,
            ends,
            mmag,
            brightest_ever,
        }
    }

    /// The frame: the axes and the lifetimes.
    #[must_use]
    pub fn frame(&self) -> &PhaseFrame {
        &self.frame
    }

    /// Cell `cell`'s segments, as (last bin, value) pairs, the value in millimagnitudes, `None`
    /// where none shines.
    ///
    /// # Panics
    ///
    /// If `cell` is not below the frame's cells.
    #[doc(hidden)]
    #[must_use]
    pub fn segments_of(&self, cell: usize) -> Vec<(u16, Option<i32>)> {
        let (a, b) = self.span(cell);
        self.ends[a..b]
            .iter()
            .zip(&self.mmag[a..b])
            .map(|(&end, &k)| (end, (k != DARK_MMAG).then_some(i32::from(k))))
            .collect()
    }

    /// Each cell's row among the distinct rows: what the fitted table's `CELL_ROWS` stores.
    #[doc(hidden)]
    #[must_use]
    pub fn cell_rows(&self) -> &[u16] {
        &self.cell_rows
    }

    /// The distinct rows, each packed as the fitted table's `ROWS` stores it: four base64
    /// characters a segment of up to 255 bins.
    ///
    /// # Panics
    ///
    /// Never for an envelope of [`assemble`](Self::assemble) or [`fitted`](Self::fitted), whose
    /// runs fit a byte.
    #[doc(hidden)]
    #[must_use]
    pub fn packed_rows(&self) -> Vec<String> {
        self.row_offsets
            .windows(2)
            .map(|w| {
                let (a, b) = (to_index(w[0]), to_index(w[1]));
                let mut out = String::with_capacity(4 * (b - a));
                let mut first = 0;
                for (&end, &k) in self.ends[a..b].iter().zip(&self.mmag[a..b]) {
                    let end = usize::from(end);
                    let run = u8::try_from(end + 1 - first).expect("a run of at most 255 bins");
                    pack_segment(run, k, &mut out);
                    first = end + 1;
                }
                out
            })
            .collect()
    }

    /// The number of distinct rows.
    #[doc(hidden)]
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.row_offsets.len() - 1
    }

    /// The number of segments of the distinct rows.
    #[doc(hidden)]
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.ends.len()
    }

    /// The segments of cell `cell`, as a range of indices.
    #[must_use]
    fn span(&self, cell: usize) -> (usize, usize) {
        let row = usize::from(self.cell_rows[cell]);
        (
            to_index(self.row_offsets[row]),
            to_index(self.row_offsets[row + 1]),
        )
    }

    /// The brightest absolute V magnitude, margin included, a single star can have at an age
    /// within `ages`.
    ///
    /// The star has initial mass `mass`, composition `composition` and η draw `eta`; `ages` are
    /// years, inclusive, a negative lower end read as 0. It is `None` if the star cannot shine in V
    /// then, or the range is empty or either end NaN. The reading allocates nothing: the star's
    /// cell, its lifetime there, the bins its relative ages span, and a binary search of the
    /// cell's segments.
    ///
    /// A mass above 0.0124 M☉ but outside the table, up to 150 M☉, reads the nearest cell, as the
    /// tracks read 150 M☉ above it. A NaN or non-positive mass, and a composition with a helium
    /// excess, which the table is not built for (see the [module](self) documentation), read the
    /// table's brightest value, which skips nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::sky::phase::PhaseEnvelope;
    /// use hyperion_sim::stellar::Composition;
    /// use hyperion_sim::stellar::draws::StandardNormal;
    /// use hyperion_sim::units::{SolarMasses, Years};
    ///
    /// let table = PhaseEnvelope::fitted();
    /// let sun = |age: f64| {
    ///     let ages = (Years::new(age), Years::new(age));
    ///     table.brightest(SolarMasses::new(1.0), &Composition::SOLAR, StandardNormal::ZERO, ages)
    /// };
    /// // Today the Sun is bounded near its own M_V 4.8, less the margin; near the tip of its
    /// // giant branch, at 12.4 Gyr, it may be far brighter.
    /// let today = sun(4.57e9).ok_or("the Sun shines")?.value();
    /// let giant = sun(1.24e10).ok_or("a giant shines")?.value();
    /// assert!(today > 4.0 && giant < today - 3.0);
    /// # Ok::<(), &str>(())
    /// ```
    #[must_use]
    pub fn brightest(
        &self,
        mass: SolarMasses,
        composition: &Composition,
        eta: StandardNormal,
        ages: (Years, Years),
    ) -> Option<Magnitudes> {
        let (lo, hi) = (ages.0.value(), ages.1.value());
        // An empty range, or a NaN end, reads nothing.
        if lo.is_nan()
            || hi
                .partial_cmp(&lo.max(0.0))
                .is_none_or(core::cmp::Ordering::is_lt)
        {
            return None;
        }
        let lo = lo.max(0.0);
        let m = mass.value();
        if m.is_nan() || m <= 0.0 || composition.helium_excess().value() > 0.0 {
            return magnitude_of(self.brightest_ever);
        }
        let at = Coordinates {
            mass,
            fe_h: fe_h_coordinate(composition),
            eta,
        };
        let cell = self.frame.cell_of(at);
        // `relative_age`'s division, the lifetime computed once for both ends.
        let lifetime = math::exp(self.frame.ln_lifetime_in(cell, at));
        let first = relative_age_bin(lo / lifetime);
        let last = relative_age_bin(hi / lifetime);
        let (a, b) = self.span(cell.index());
        let ends = &self.ends[a..b];
        let start = ends.partition_point(|&end| usize::from(end) < first);
        let stop = ends
            .partition_point(|&end| usize::from(end) < last)
            .min(ends.len() - 1);
        magnitude_of(
            self.mmag[a + start..=a + stop]
                .iter()
                .copied()
                .min()
                .unwrap_or(DARK_MMAG),
        )
    }

    /// The lifetime, years, the table reads a star of initial mass `mass`, composition
    /// `composition` and η draw `eta` against.
    #[must_use]
    pub fn lifetime(
        &self,
        mass: SolarMasses,
        composition: &Composition,
        eta: StandardNormal,
    ) -> Years {
        let at = Coordinates {
            mass,
            fe_h: fe_h_coordinate(composition),
            eta,
        };
        Years::new(math::exp(
            self.frame.ln_lifetime_in(self.frame.cell_of(at), at),
        ))
    }

    /// The bytes the envelope owns on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        (self.frame.masses.capacity()
            + self.frame.ln_masses.capacity()
            + self.frame.ln_lifetimes.capacity())
            * size_of::<f64>()
            + self.row_offsets.capacity() * size_of::<u32>()
            + (self.ends.capacity() + self.cell_rows.capacity() + self.mmag.capacity())
                * size_of::<u16>()
    }
}

/// A cell's segments as the table stores them, from its brightest V per bin before the margin:
/// [`segments`], each value in millimagnitudes rounded brighter and at most 32.766 mag, a run
/// longer than a packed segment holds split into several.
///
/// # Panics
///
/// If a value is brighter than −32.768 mag, which no star is.
#[must_use]
fn stored_segments(row: &[f64; BINS]) -> Vec<(u16, i16)> {
    let mut stored = Vec::new();
    let mut first = 0;
    for (end, v) in segments(row) {
        let k = to_millimag(v).map_or(DARK_MMAG, |k| {
            let k = k.min(i32::from(DARK_MMAG) - 1);
            assert!(
                k > i32::from(i16::MIN),
                "a magnitude of {v} fits the packed rows"
            );
            i16::try_from(k).expect("within the bounds just checked")
        });
        let mut from = first;
        while end + 1 - from > LONGEST_RUN {
            from += LONGEST_RUN;
            stored.push((u16::try_from(from - 1).expect("under 400 bins"), k));
        }
        stored.push((u16::try_from(end).expect("under 400 bins"), k));
        first = end + 1;
    }
    stored
}

/// `k` as an index.
#[must_use]
fn to_index(k: u32) -> usize {
    usize::try_from(k).expect("a segment index fits in usize")
}

/// The magnitude of a table value `k`, millimagnitudes, or `None` for [`DARK_MMAG`].
#[must_use]
fn magnitude_of(k: i16) -> Option<Magnitudes> {
    (k != DARK_MMAG).then(|| Magnitudes::new(f64::from(k) / 1000.0))
}

/// `row`'s bins brightened by [`MARGIN_MAG`] and cut into runs, each `(last bin, brightest)`: a
/// run holds bins that all shine within [`merge_tolerance`] of its brightest, or bins where none
/// shines. Greedy from the first bin.
#[must_use]
fn segments(row: &[f64; BINS]) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < BINS {
        let first = row[k];
        let (mut lo, mut hi) = (first, first);
        let mut j = k + 1;
        while j < BINS {
            let v = row[j];
            let fits = if first.is_finite() {
                v.is_finite() && v.max(hi) - v.min(lo) <= merge_tolerance(v.min(lo))
            } else {
                !v.is_finite()
            };
            if !fits {
                break;
            }
            lo = lo.min(v);
            hi = hi.max(v);
            j += 1;
        }
        out.push((j - 1, lo - MARGIN_MAG));
        k = j;
    }
    out
}

/// The distinct samples along an axis of `nodes` nodes with `per` samples an interval, its ends
/// shared with its neighbours.
#[must_use]
const fn samples_along(nodes: usize, per: usize) -> usize {
    (nodes - 1) * (per - 1) + 1
}

const _: () = assert!(
    FE_H_SAMPLES == 3 && ETA_SAMPLES == 3,
    "axis_sample takes three samples an interval"
);

/// The `index`-th of the distinct samples along the axis of `nodes`, three an interval.
#[must_use]
fn axis_sample(nodes: &[f64], index: usize) -> f64 {
    let interval = (index / 2).min(nodes.len() - 2);
    three_samples(nodes[interval], nodes[interval + 1], index - 2 * interval)
}

/// For each bin, the bins whose relative ages its own, divided by 1 + `spread` and by
/// 1 − `spread`, overlap: the window [`dilate`] reads.
#[must_use]
fn spread_windows(spread: f64) -> Vec<(usize, usize)> {
    (0..BINS)
        .map(|b| {
            let lo = relative_age_bin(bin_start(b) / (1.0 + spread)).min(b);
            let hi = if b + 1 < BINS {
                // Just inside the bin's upper edge, so that no spread reads the next bin by
                // itself.
                relative_age_bin(bin_start(b + 1) * (1.0 - 1e-12) / (1.0 - spread)).max(b)
            } else {
                BINS - 1
            };
            (lo, hi)
        })
        .collect()
}

/// `row` read through `windows`: each bin the brightest of its window.
#[must_use]
fn dilate_in(row: &[f64; BINS], windows: &[(usize, usize)]) -> [f64; BINS] {
    let mut out = [f64::INFINITY; BINS];
    for (v, &(lo, hi)) in out.iter_mut().zip(windows) {
        *v = row[lo..=hi].iter().fold(f64::INFINITY, |a, &x| a.min(x));
    }
    out
}

/// `row` widened in relative age by `spread`.
///
/// Each bin takes the brightest of the bins its relative ages, divided by 1 + `spread` and by
/// 1 − `spread`, overlap. A star whose phases run that much earlier or later than a sample's is
/// then bounded where the sample is.
#[must_use]
fn dilate(row: &[f64; BINS], spread: f64) -> [f64; BINS] {
    dilate_in(row, &spread_windows(spread))
}

/// Whether `row` is bounded by `bound`, margin included, wherever it shines.
#[must_use]
fn bounded_by(row: &[f64; BINS], bound: &[f64; BINS]) -> bool {
    row.iter()
        .zip(bound)
        .all(|(&v, &b)| !v.is_finite() || v >= b - MARGIN_MAG)
}

/// The least of [`SPREADS`] under which each sample of `rows` with neighbours on both sides along an
/// axis is bounded by the brighter of those two alone, widened by it, margin included.
///
/// `rows` are a cell's samples by mass, \[Fe/H\] and η place, row-major. It is `None` if no spread
/// passes; the cell then takes the largest, and only the slow test holds it.
#[must_use]
fn cell_spread(rows: &[[f64; BINS]]) -> Option<f64> {
    let index = |a: usize, b: usize, c: usize| (a * FE_H_SAMPLES + b) * ETA_SAMPLES + c;
    let mut pairs: Vec<(usize, [f64; BINS])> = Vec::new();
    let mut neighbours = |s: usize, n1: usize, n2: usize| {
        let mut both = rows[n1];
        for (x, &y) in both.iter_mut().zip(&rows[n2]) {
            *x = x.min(y);
        }
        pairs.push((s, both));
    };
    for a in 0..MASS_SAMPLES {
        for b in 0..FE_H_SAMPLES {
            for c in 0..ETA_SAMPLES {
                let s = index(a, b, c);
                if a > 0 && a + 1 < MASS_SAMPLES {
                    neighbours(s, index(a - 1, b, c), index(a + 1, b, c));
                }
                if b > 0 && b + 1 < FE_H_SAMPLES {
                    neighbours(s, index(a, b - 1, c), index(a, b + 1, c));
                }
                if c > 0 && c + 1 < ETA_SAMPLES {
                    neighbours(s, index(a, b, c - 1), index(a, b, c + 1));
                }
            }
        }
    }
    SPREADS.iter().copied().find(|&spread| {
        let windows = spread_windows(spread);
        pairs
            .iter()
            .all(|(s, both)| bounded_by(&rows[*s], &dilate_in(both, &windows)))
    })
}

/// One cell of a mass interval as the fit builds it ([`rows_of_interval`]).
#[derive(Debug, Clone, PartialEq)]
pub struct IntervalCell {
    /// The cell.
    cell: Cell,
    /// The brightest V a bin of its samples reaches, before the margin, widened by its spread;
    /// +∞ where none shines.
    row: [f64; BINS],
    /// Its spread, one of [`SPREADS`].
    spread: f64,
    /// Whether its samples passed the neighbour test at that spread, rather than falling back to
    /// the largest.
    passed: bool,
}

impl IntervalCell {
    /// The cell.
    #[must_use]
    pub const fn cell(&self) -> Cell {
        self.cell
    }

    /// The brightest V a bin of its samples reaches, mag, before the margin, widened by its
    /// spread; +∞ where none shines.
    #[must_use]
    pub const fn row(&self) -> &[f64; BINS] {
        &self.row
    }

    /// Its spread in relative age, one of [`SPREADS`].
    #[must_use]
    pub const fn spread(&self) -> f64 {
        self.spread
    }

    /// Whether its samples passed the neighbour test at its spread; if not, it took the largest.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.passed
    }
}

/// The cells of mass interval `interval`, each widened by its own spread: the fit's unit of work.
///
/// Each cell holds the brightest V a bin of its samples reaches, before the margin (+∞ where none
/// shines), widened by the least of [`SPREADS`] under which its samples bound one another. The
/// interval's samples are its [`MASS_SAMPLES`] masses, at every \[Fe/H\] and η sample of the axes,
/// three a cell each, so a sample on a boundary enters both cells, each at its lifetime in that
/// cell. A pure function of its arguments.
///
/// # Panics
///
/// If `interval` is not below the frame's mass intervals.
#[doc(hidden)]
#[must_use]
pub fn rows_of_interval(frame: &PhaseFrame, interval: usize) -> Vec<IntervalCell> {
    assert!(
        interval + 1 < frame.masses.len(),
        "mass interval {interval}"
    );
    let (lo, hi) = (
        SolarMasses::new(frame.masses[interval]),
        SolarMasses::new(frame.masses[interval + 1]),
    );
    let (fe_h_axis, eta_axis) = (
        samples_along(FE_H_NODES.len(), FE_H_SAMPLES),
        samples_along(ETA_NODES.len(), ETA_SAMPLES),
    );
    let draw = |c: usize| StandardNormal::new(axis_sample(&ETA_NODES, c)).expect("finite");
    let parts: Vec<Vec<SamplePart>> = (0..MASS_SAMPLES)
        .flat_map(|a| {
            (0..fe_h_axis).flat_map(move |b| {
                (0..eta_axis).map(move |c| {
                    sample_parts(
                        mass_sample(lo, hi, a),
                        Dex::new(axis_sample(&FE_H_NODES, b)),
                        draw(c),
                    )
                })
            })
        })
        .collect();
    let mut out = Vec::with_capacity(FE_H_CELLS * ETA_CELLS);
    for fe_h in 0..FE_H_CELLS {
        for eta in 0..ETA_CELLS {
            let cell = Cell {
                mass: interval,
                fe_h,
                eta,
            };
            let mut rows = vec![[f64::INFINITY; BINS]; MASS_SAMPLES * FE_H_SAMPLES * ETA_SAMPLES];
            for a in 0..MASS_SAMPLES {
                for p in 0..FE_H_SAMPLES {
                    for q in 0..ETA_SAMPLES {
                        let (b, c) = (fe_h * (FE_H_SAMPLES - 1) + p, eta * (ETA_SAMPLES - 1) + q);
                        let composition = Composition::from_fe_h(
                            Dex::new(axis_sample(&FE_H_NODES, b)),
                            HeliumExcess::ZERO,
                        );
                        let at = Coordinates {
                            mass: mass_sample(lo, hi, a),
                            fe_h: fe_h_coordinate(&composition),
                            eta: draw(c),
                        };
                        enter_parts(
                            &parts[(a * fe_h_axis + b) * eta_axis + c],
                            frame.ln_lifetime_in(cell, at),
                            &mut rows[(a * FE_H_SAMPLES + p) * ETA_SAMPLES + q],
                        );
                    }
                }
            }
            let found = cell_spread(&rows);
            let spread = found.unwrap_or(SPREADS[SPREADS.len() - 1]);
            let mut brightest = [f64::INFINITY; BINS];
            for row in &rows {
                for (x, &y) in brightest.iter_mut().zip(row) {
                    *x = x.min(y);
                }
            }
            out.push(IntervalCell {
                cell,
                row: dilate(&brightest, spread),
                spread,
                passed: found.is_some(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::id::Layer;
    use crate::sky::envelope::{mass_nodes, max_star_mass};
    use crate::sky::testing::{milky_way_envelope, phase_envelope};
    use hyperion_testkit::float::bits;

    fn composition(fe_h: f64) -> Composition {
        Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO)
    }

    fn draw(z: f64) -> StandardNormal {
        StandardNormal::new(z).expect("finite")
    }

    fn at(age: f64) -> (Years, Years) {
        (Years::new(age), Years::new(age))
    }

    /// The bins partition relative age: just above each bin's start reads that bin and just
    /// below it the one before, and the starts increase to 1.05.
    #[test]
    fn the_bins_partition_relative_age() {
        let mut previous = -1.0;
        for k in 0..BINS {
            let start = bin_start(k);
            assert!(start > previous, "{k}: {start} after {previous}");
            if k > 0 {
                assert_eq!(relative_age_bin(start * (1.0 + 1e-12)), k, "{k}");
                assert_eq!(relative_age_bin(start * (1.0 - 1e-12)), k - 1, "{k}");
            }
            previous = start;
        }
        assert_eq!(relative_age_bin(0.0), 0);
        assert_eq!(relative_age_bin(f64::NAN), 0);
        assert_eq!(relative_age_bin(1e9), BINS - 1);
        assert!(
            (bin_start(FIRST_FINE_BIN) - FINE_START).abs() < 1e-15,
            "{}",
            bin_start(FIRST_FINE_BIN)
        );
        assert!(
            (bin_start(BINS - 1) - 1.05).abs() < 1e-12,
            "{}",
            bin_start(BINS - 1)
        );
    }

    /// The fitted table sits on the brightness envelope's mass nodes, and every cell's segments
    /// end in increasing bins up to the last, each value a magnitude or dark.
    #[test]
    fn every_cell_of_the_fitted_table_covers_every_bin() {
        let e = phase_envelope();
        let nodes = mass_nodes();
        let node_bits: Vec<u64> = nodes.iter().map(|&m| bits(m)).collect();
        let table_bits: Vec<u64> = e.frame().masses().iter().map(|&m| bits(m)).collect();
        assert_eq!(table_bits, node_bits, "the table's masses are mass_nodes()");
        assert_eq!(e.frame().cells(), (nodes.len() - 1) * 11 * 3);
        let mut lit = 0;
        for cell in 0..e.frame().cells() {
            let segments = e.segments_of(cell);
            assert!(
                segments.windows(2).all(|w| w[0].0 < w[1].0),
                "{cell}: {segments:?}"
            );
            assert_eq!(
                segments.last().map(|&(end, _)| usize::from(end)),
                Some(BINS - 1),
                "{cell}"
            );
            assert!(
                segments
                    .iter()
                    .all(|&(_, v)| v.is_none_or(|k| (-20_000..40_000).contains(&k))),
                "{cell}: {segments:?}"
            );
            lit += usize::from(segments.iter().any(|&(_, v)| v.is_some()));
        }
        assert_eq!(
            lit,
            e.frame().cells(),
            "every cell's stars shine at some age"
        );
    }

    /// At a node of mass, \[Fe/H\] and η the table's lifetime is the track's, to its rounding;
    /// below 0.1 M☉ it is the cap.
    #[test]
    fn the_lifetimes_at_the_nodes_are_the_tracks() {
        let e = phase_envelope();
        let masses = e.frame().masses();
        // \[Fe/H\] nodes within the tracks' clamps, where a node's coordinate is the node.
        for (j, i, k) in [
            (70, 10, 2),
            (95, 1, 0),
            (95, 9, 3),
            (130, 5, 1),
            (197, 10, 3),
        ] {
            let (m, fe_h, eta) = (masses[j], FE_H_NODES[i], ETA_NODES[k]);
            let read = e
                .lifetime(SolarMasses::new(m), &composition(fe_h), draw(eta))
                .value();
            let track = node_lifetime(SolarMasses::new(m), Dex::new(fe_h), draw(eta)).value();
            assert!(
                (read / track - 1.0).abs() < 2e-4,
                "{m} M☉ [Fe/H] {fe_h} η draw {eta}: {read} against {track}"
            );
        }
        let brown = e
            .lifetime(SolarMasses::new(0.05), &composition(0.0), draw(0.0))
            .value();
        assert!((brown / LIFETIME_CAP_YEARS - 1.0).abs() < 1e-12, "{brown}");
    }

    /// A sample's own ages are bounded with the whole margin to spare.
    ///
    /// At the middle of each part of its life, one of the five points the fit read, its V is no
    /// brighter than the table's bound plus the margin. This holds the fit's sampling, the
    /// lifetimes, the bins, the merging and the rounding to one another.
    #[test]
    fn a_samples_own_ages_are_bounded_with_its_whole_margin() {
        let e = phase_envelope();
        let masses = e.frame().masses().to_vec();
        for (j, place, fe_h, eta) in [
            (20, 3, 0.0, 0.0),
            (44, 0, -1.0, 0.0),
            (60, 5, 0.0, -3.5),
            (90, 2, -1.375, 3.5),
            (90, 7, 0.18, -0.5 / 0.07),
            (132, 4, -0.125, 0.0),
        ] {
            let m = mass_sample(
                SolarMasses::new(masses[j]),
                SolarMasses::new(masses[j + 1]),
                place,
            );
            let parts = sample_parts(m, Dex::new(fe_h), draw(eta));
            assert!(!parts.is_empty(), "{m:?}");
            for part in parts {
                let (a, b) = (part.from().value(), part.to().value());
                let age = a + (b - a) * 0.5;
                let bound = e
                    .brightest(m, &composition(fe_h), draw(eta), at(age))
                    .expect("a sample's lit part is lit in the table");
                assert!(
                    bound.value() + MARGIN_MAG <= part.brightest().value() + 1e-9,
                    "{m:?} [Fe/H] {fe_h} η draw {eta} at {age}: V {:?} against {bound:?}",
                    part.brightest()
                );
            }
        }
    }

    /// The Sun today is bounded a little brighter than its own V, and an old metal-rich dwarf
    /// still on its main sequence takes a bound fainter than M<sub>V</sub> +3, where the
    /// brightness envelope at twice its mass reaches past −4 through the giant phases of every
    /// lighter star.
    #[test]
    fn a_dwarf_far_from_its_giant_phases_takes_a_faint_bound() {
        let e = phase_envelope();
        let sun = e
            .brightest(
                SolarMasses::new(1.0),
                &Composition::SOLAR,
                draw(0.0),
                at(4.57e9),
            )
            .expect("the Sun shines")
            .value();
        assert!((3.5..4.83 - MARGIN_MAG).contains(&sun), "{sun}");
        let old = e
            .brightest(
                SolarMasses::new(0.85),
                &composition(0.1),
                draw(0.0),
                at(1e10),
            )
            .expect("shines")
            .value();
        assert!(old > 3.0, "{old}");
        let component = milky_way_galaxy()
            .fields()
            .component_ids()
            .next()
            .expect("a component");
        let widest = milky_way_envelope()
            .brightest(
                Layer::C,
                component,
                max_star_mass(SolarMasses::new(0.85)),
                (Years::ZERO, Years::new(1e10)),
            )
            .expect("shines")
            .value();
        assert!(widest < -4.0, "{widest}");
    }

    /// A range of ages reads the brightest segment its bins overlap: inside one segment, across
    /// several, and ending on a segment's last bin.
    #[test]
    fn a_range_of_ages_reads_the_brightest_segment_it_overlaps() {
        let e = phase_envelope();
        let frame = e.frame();
        for (m, fe_h, eta) in [(1.0, 0.0, 0.0), (2.2, -0.5, 1.0), (25.0, -1.0, -2.0)] {
            let comp = composition(fe_h);
            let at_star = Coordinates {
                mass: SolarMasses::new(m),
                fe_h: fe_h_coordinate(&comp),
                eta: draw(eta),
            };
            let cell = frame.cell_of(at_star);
            let lifetime = math::exp(frame.ln_lifetime_in(cell, at_star));
            let segments = e.segments_of(cell.index());
            // The ranges' ends: the middle of a segment's first bin, and the middle of a later
            // segment's last bin.
            let middle = |bin: usize| {
                let next = if bin + 1 < BINS {
                    bin_start(bin + 1)
                } else {
                    1.2
                };
                f64::midpoint(bin_start(bin), next) * lifetime
            };
            let starts: Vec<usize> = core::iter::once(0)
                .chain(segments.iter().map(|&(end, _)| usize::from(end) + 1))
                .take(segments.len())
                .collect();
            let mut checked = 0;
            for (i, &first_bin) in starts.iter().enumerate() {
                for j in [i, i + 1, i + 3] {
                    let Some(&(last_end, _)) = segments.get(j) else {
                        continue;
                    };
                    let (lo, hi) = (middle(first_bin), middle(usize::from(last_end)));
                    let expected = segments[i..=j]
                        .iter()
                        .filter_map(|&(_, v)| v)
                        .min()
                        .map(|k| f64::from(k) / 1000.0);
                    let read = e
                        .brightest(
                            SolarMasses::new(m),
                            &comp,
                            draw(eta),
                            (Years::new(lo), Years::new(hi)),
                        )
                        .map(Magnitudes::value);
                    assert_eq!(
                        read.map(bits),
                        expected.map(bits),
                        "{m} M☉ segments {i}..={j}: {read:?} against {expected:?}"
                    );
                    checked += 1;
                }
            }
            assert!(checked > 10, "{m} M☉: {checked} ranges");
        }
    }

    /// A NaN or non-positive mass, or a helium excess, reads the table's brightest value, which
    /// skips nothing; an empty range, or a NaN end, reads nothing.
    #[test]
    fn edge_cases_skip_nothing_or_read_nothing() {
        let e = phase_envelope();
        let one = |m: f64, comp: &Composition, ages| {
            e.brightest(SolarMasses::new(m), comp, draw(0.0), ages)
                .map(Magnitudes::value)
        };
        let brightest = one(f64::NAN, &Composition::SOLAR, at(1e9)).expect("shines");
        assert!(brightest < -12.0, "{brightest}");
        let helium_rich = Composition::from_fe_h(Dex::ZERO, HeliumExcess::new(0.05));
        for (m, comp) in [
            (-1.0, Composition::SOLAR),
            (0.0, Composition::SOLAR),
            (1.0, helium_rich),
        ] {
            assert_eq!(
                one(m, &comp, at(1e9)).map(bits),
                Some(bits(brightest)),
                "{m}"
            );
        }
        let sun =
            |lo: f64, hi: f64| one(1.0, &Composition::SOLAR, (Years::new(lo), Years::new(hi)));
        assert_eq!(sun(2e9, 1e9), None);
        assert_eq!(sun(1e9, f64::NAN), None);
        assert_eq!(sun(f64::NAN, 1e9), None);
        assert!(sun(-5.0, 1e9).is_some());
    }

    /// Compositions that share a track share a coordinate: the tracks clamp Z to 10⁻⁴–0.03.
    #[test]
    fn compositions_beyond_the_tracks_clamps_share_their_coordinate() {
        let poor = fe_h_coordinate(&composition(-2.5)).value();
        assert_eq!(
            bits(poor),
            bits(fe_h_coordinate(&composition(-4.0)).value())
        );
        assert!((poor + 2.301_03).abs() < 1e-5, "{poor}");
        let rich = fe_h_coordinate(&composition(0.18)).value();
        assert_eq!(bits(rich), bits(fe_h_coordinate(&composition(0.6)).value()));
        assert!((rich - 0.176_09).abs() < 1e-5, "{rich}");
        let solar = fe_h_coordinate(&Composition::SOLAR).value();
        assert!(solar.abs() < 1e-15, "{solar}");
    }

    /// An interval's rows are a pure function of the frame and the interval, whatever was built
    /// before them, so the fit may build them in any order or in parallel.
    #[test]
    fn rows_of_interval_are_order_independent() {
        let frame = phase_envelope().frame();
        hyperion_testkit::order::assert_order_independent(&[0, 20, 43], |&j| {
            rows_of_interval(frame, j)
        });
    }

    /// A row widened by a spread takes, in each bin, the brightest of the bins within that
    /// fraction of its relative age, and a spread of half a fine bin reads only its neighbours.
    #[test]
    fn dilation_widens_by_the_spread() {
        let mut row = [f64::INFINITY; BINS];
        let at = relative_age_bin(0.95);
        row[at] = -3.0;
        let wide = dilate(&row, 1e-2);
        for r in [0.9406, 0.95, 0.9594] {
            assert_eq!(bits(wide[relative_age_bin(r)]), bits(-3.0), "{r}");
        }
        for r in [0.938, 0.962] {
            assert!(wide[relative_age_bin(r)].is_infinite(), "{r}");
        }
        let narrow = dilate(&row, SPREADS[0]);
        assert_eq!(narrow.iter().filter(|v| v.is_finite()).count(), 3);
    }

    /// A cell whose samples agree takes the least spread, one whose middle mass sample shines ten
    /// fine bins after its neighbours takes the spread that reaches it, and one no spread reaches
    /// passes none.
    #[test]
    fn a_cell_takes_the_spread_its_samples_need() {
        let mut row = [f64::INFINITY; BINS];
        row[FIRST_FINE_BIN + 120] = 2.0;
        let mut rows = vec![row; MASS_SAMPLES * FE_H_SAMPLES * ETA_SAMPLES];
        assert_eq!(cell_spread(&rows).map(bits), Some(bits(SPREADS[0])));
        // Mass sample 3, its middle [Fe/H] and η sample: 10 bins later, about 1.1% at 0.93.
        let s = (3 * FE_H_SAMPLES + 1) * ETA_SAMPLES + 1;
        rows[s] = [f64::INFINITY; BINS];
        rows[s][FIRST_FINE_BIN + 130] = 2.0;
        assert_eq!(cell_spread(&rows).map(bits), Some(bits(1.6e-2)));
        // Brighter by more than the margin than its neighbours: no spread helps.
        rows[s][FIRST_FINE_BIN + 120] = 1.0;
        rows[s][FIRST_FINE_BIN + 130] = f64::INFINITY;
        assert_eq!(cell_spread(&rows), None);
    }

    /// The lifetimes' integers round-trip to within their unit.
    #[test]
    fn lifetimes_quantize_to_a_ten_thousandth_of_a_dex() {
        for years in [3.2e6, 1.0e8, 1.25e10, 1.9e10] {
            let back = math::exp(dequantize_lifetime(quantize_lifetime(Years::new(years))));
            let off = math::log10(back / years).abs();
            assert!(off <= 0.5e-4 + 1e-12, "{years}: {back}");
        }
        assert_eq!(quantize_lifetime(Years::new(LIFETIME_CAP_YEARS)), 0);
        assert_eq!(quantize_lifetime(Years::new(1e12)), 0);
    }

    /// A lifetime beyond the integers' reach is refused.
    #[test]
    #[should_panic(expected = "is within 6.55 dex of the cap")]
    fn a_lifetime_too_short_for_the_table_is_refused() {
        let _ = quantize_lifetime(Years::new(1_000.0));
    }

    /// The decoded table's cells are, segment for segment, what the fit makes of tracks sampled
    /// now.
    ///
    /// Four mass intervals are rebuilt over every \[Fe/H\] and η interval: a brown dwarf's, a red
    /// dwarf's, one in the electron-capture window, whose samples carry both companion-stripped
    /// marks, and a massive star's. So the packed rows can be trusted without being read, and a
    /// change to the tracks that the fingerprint missed would fail here.
    #[test]
    fn a_handful_of_cells_are_the_fit_of_todays_tracks() {
        let e = phase_envelope();
        for interval in [20, 60, 132, 160] {
            for built in rows_of_interval(e.frame(), interval) {
                let fresh: Vec<(u16, Option<i32>)> = stored_segments(built.row())
                    .into_iter()
                    .map(|(end, k)| (end, (k != DARK_MMAG).then_some(i32::from(k))))
                    .collect();
                assert_eq!(
                    e.segments_of(built.cell().index()),
                    fresh,
                    "cell {:?}",
                    built.cell()
                );
            }
        }
    }

    /// Packed rows decode to their segments.
    #[test]
    fn packed_rows_round_trip() {
        let e = phase_envelope();
        let text: String = e.packed_rows().concat();
        let (offsets, ends, mmag) = unpack_rows(&text);
        assert_eq!(offsets, e.row_offsets);
        assert_eq!(ends, e.ends);
        assert_eq!(mmag, e.mmag);
        for (run, value) in [(1, -12_850), (255, 32_767), (67, 0), (3, -1)] {
            let mut out = String::new();
            pack_segment(run, value, &mut out);
            assert_eq!(out.len(), 4, "{out}");
            let bytes = out.as_bytes();
            assert_eq!(
                unpack_segment([bytes[0], bytes[1], bytes[2], bytes[3]]),
                (run, value)
            );
        }
        // A dark row of 255 and 67 bins, the placeholder's.
        let (offsets, ends, _) = unpack_rows("//9/Q/9/");
        assert_eq!((offsets, ends), (vec![0, 2], vec![254, 321]));
        // Whitespace between characters is ignored.
        let (_, spaced, _) = unpack_rows("//9/\n    Q/9/");
        assert_eq!(spaced, vec![254, 321]);
    }

    #[test]
    #[should_panic(expected = "a row ends at the last bin")]
    fn a_row_past_the_last_bin_is_refused() {
        let _ = unpack_rows("//9///9/");
    }

    #[test]
    #[should_panic(expected = "a packed row holds base64 characters only")]
    fn a_character_outside_the_alphabet_is_refused() {
        let _ = unpack_rows("//9/Q/9@");
    }

    #[test]
    #[should_panic(expected = "a segment holds a bin at least")]
    fn a_run_of_no_bins_is_refused() {
        let _ = unpack_rows("AAAA");
    }

    #[test]
    #[should_panic(expected = "whole packed segments")]
    fn a_partial_segment_is_refused() {
        let _ = unpack_rows("//9/Q/9");
    }

    #[test]
    #[should_panic(expected = "the last row is whole")]
    fn a_row_cut_short_is_refused() {
        let _ = unpack_rows("//9/");
    }

    /// The census's reader is the fitted table's, bit for bit, whoever decodes it first.
    #[test]
    fn the_shared_envelope_is_the_fitted_table() {
        assert_eq!(*PhaseEnvelope::shared(), PhaseEnvelope::fitted());
        assert!(core::ptr::eq(PhaseEnvelope::shared(), phase_envelope()));
    }

    /// One reading of the golden's: the brightest V at `ages`, in millimagnitudes, or `dark`,
    /// each value held bit for bit to its integer's ÷ 1,000 first, so that the integer pins the
    /// value's bits; and the bins of the range's ends, read as the reader reads them, against
    /// the lifetime it reads.
    fn reading(e: &PhaseEnvelope, m: f64, comp: &Composition, z: f64, ages: (f64, f64)) -> String {
        let (mass, eta) = (SolarMasses::new(m), draw(z));
        let read = e.brightest(mass, comp, eta, (Years::new(ages.0), Years::new(ages.1)));
        let life = e.lifetime(mass, comp, eta).value();
        let bins = format!(
            "{}-{}",
            relative_age_bin(ages.0.max(0.0) / life),
            relative_age_bin(ages.1 / life)
        );
        read.map_or_else(
            || format!("dark@{bins}"),
            |v| {
                let k = (v.value() * 1000.0).round();
                hyperion_testkit::float::assert_same_bits(v.value(), k / 1000.0);
                format!("{k:.0}@{bins}")
            },
        )
    }

    /// The reader's arithmetic, pinned for every target (the determinism audit of R06.T8.m, before
    /// R06.T8.g serves it): the lifetime it reads each star against, as bits, and what it reads at
    /// relative ages about the fine bins' edges and over ranges, at masses on and between the mass
    /// nodes, \[Fe/H\] on and between its nodes and beyond the tracks' clamps, and η draws in each
    /// interval and beyond the axis; then 256 random stars and ranges. `just test-wasm-fast`
    /// compares it on wasm32-wasip1, whose `usize` is 32 bits.
    #[test]
    fn the_readers_arithmetic_is_pinned() {
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::GoldenWriter;

        let table = phase_envelope();
        let masses = &table.frame().masses;
        let mut out = GoldenWriter::new();
        out.header(crate::GENERATOR_VERSION.get());
        let relative = [
            1e-8, 1e-4, 0.1, 0.5, 0.79, 0.8, 0.85, 0.95, 0.999, 1.0, 1.0005, 1.03, 1.06,
        ];
        for k in (0..masses.len() - 1).step_by(18) {
            let between = math::exp(f64::midpoint(math::ln(masses[k]), math::ln(masses[k + 1])));
            for m in [masses[k], between] {
                for fe_h in [-3.0, -1.7, 0.0, 0.5] {
                    let comp = composition(fe_h);
                    for z in [-8.0, -5.0, -1.0, 2.5, 9.0] {
                        let label = format!("{m:.6} M☉ [Fe/H] {fe_h} z {z}");
                        let life = table.lifetime(SolarMasses::new(m), &comp, draw(z)).value();
                        out.f64(&format!("{label} lifetime"), life);
                        let points: Vec<String> = relative
                            .iter()
                            .map(|&r| reading(table, m, &comp, z, (r * life, r * life)))
                            .collect();
                        let ranges = [(0.0, 0.5), (0.5, 0.9), (0.9, 1.02), (0.0, 1.2)]
                            .map(|(lo, hi)| reading(table, m, &comp, z, (lo * life, hi * life)));
                        out.line(&format!(
                            "  at {}; over {}",
                            points.join(" "),
                            ranges.join(" ")
                        ));
                    }
                }
            }
        }
        let mut u = crate::sky::testing::uniforms(0x0007_8a5e);
        let mut next = || u.next().expect("endless");
        for i in 0..256_u32 {
            let m = 0.0124 * math::exp(next() * math::ln(150.0 / 0.0124));
            let fe_h = -2.8 + 3.2 * next();
            let z = -9.0 + 18.0 * next();
            let age = 1e5 * math::exp(next() * math::ln(1.4e10 / 1e5));
            let width = age * next() * 0.05;
            let comp = composition(fe_h);
            out.line(&format!(
                "query {i:03}: {m:.6} M☉ [Fe/H] {fe_h:.4} z {z:.4} age {age:.6e} yr +{width:.4e}: \
                 {}",
                reading(table, m, &comp, z, (age, age + width))
            ));
        }
        golden!("sky/phase_envelope", out.as_str());
    }
}
