//! The reach table: an upper bound on the largest radius any star of a mass and metallicity
//! interval has had by an age, as the binary engine's pre-test reads it (plan 11, P11.T17.a).
//!
//! [`can_interact`](super::can_interact) reads each star's [`Track::max_radius_until`] at its
//! engine age, which is held to no earlier than the star's arrival on the main sequence
//! (P11.T4.i). Building a track costs 0.1–1 ms, so a caller that must decide before generation
//! whether a pair can interact, as rendering plan R06's census does, reads this table instead.
//!
//! - **Cells.** [`MASS_CELLS`] intervals even in ln m over
//!   [`LOWEST_MASS_MSUN`]–[`HIGHEST_MASS_MSUN`] M☉, by the intervals of log₁₀(Z<sub>fit</sub> ÷
//!   0.02) between [`FE_H_NODES`] (the tracks read Z clamped to 10⁻⁴–0.03, so the first and last
//!   node are those clamps), by age bins: one from 0 to [`FIRST_AGE_EDGE_YEARS`], then
//!   [`AGE_BINS`] − 1 even in log age of [`AGE_STEP_DEX`].
//! - **Values.** Each cell holds the largest log₁₀ of `max_radius_until` (R☉) of its sampled
//!   stars at the bin's upper age times [`AGE_SPREAD`], held to no earlier than each star's
//!   arrival, plus [`RADIUS_MARGIN_DEX`], in integer hundredths of a dex rounded up. The stars are
//!   sampled at the [`MASS_SUBDIVISIONS`] + 1 masses and [`FE_H_SUBDIVISIONS`] + 1 metallicities
//!   that span the cell, edges included, at the Reimers η draws of [`ETA_DRAWS`], and, inside plan
//!   06's companion-stripped window ([`STRIPPED_WINDOW`]), with the mark both set and not, since
//!   the mark moves the track's electron-capture window there and so its last phases. Below
//!   [`MIN_INITIAL_MASS`] a star is P06.T13's cooling fits at age 0, as the engine reads it.
//!   - The spread covers the stars between samples, which reach a sample's radii a few per cent
//!     later or earlier in age.
//!   - Where two neighbouring samples in mass fall, the cell also allows the rise into the larger
//!     from its own neighbour, at most [`DROP_ALLOWANCE_MAX_DEX`], since the largest radius rises
//!     steeply with mass up to a discontinuity and drops there.
//!   - Each row is then made non-decreasing in age, as `max_radius_until` is.
//! - **Cores.** Each cell also holds the earliest end of a main sequence among its samples, over
//!   the spread: before it no star of the cell holds a core, so its spin's moment of inertia is
//!   the envelope's (BSE equation 35's k′₂ term alone).
//! - **Collapses.** Each cell also holds the earliest sudden death among its samples (a core's
//!   collapse, electron capture, pair instability), over the spread, +∞ where none dies so: before
//!   it no star of the cell can have exploded, whose kick could bring a pair's periastron in
//!   (`decision-p11-t16-hierarchy-bound.md` §5).
//!
//! The table depends on no galaxy, only on the generator's tracks. `hyperion-fit`'s task
//! `binary_reach` builds it from [`reach_node`] and [`assemble`] and checks it in as
//! [`tables::binary_reach`](crate::tables::binary_reach). A change to the tracks that would
//! leave it stale fails the task's sim fingerprint in `just fit-check`, and the slow test
//! `the_reach_table_bounds_dense_tracks` compares it with dense tracks.

use std::error::Error;
use std::fmt;

use crate::galaxy::displaced::binarity::NEVER_STRIPPED;
use crate::math;
use crate::rng::Mark;
use crate::stellar::Composition;
use crate::stellar::composition::Z_SOLAR;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::fates::STRIPPED_WINDOW;
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::sse::{MIN_INITIAL_MASS, Track};
use crate::stellar::substellar;
use crate::tables::binary_reach;
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

use super::star::engine_track_age_years;

/// The mass intervals of the table, even in ln m.
pub const MASS_CELLS: usize = 96;

/// The lightest star the table holds, M☉: plan 11's stellar floor, the lightest companion.
pub const LOWEST_MASS_MSUN: f64 = 0.08;

/// The heaviest star the table holds, M☉: the tracks' top. A heavier star reads it, as its track
/// does ([`MAX_INITIAL_MASS`](crate::stellar::sse::MAX_INITIAL_MASS)).
pub const HIGHEST_MASS_MSUN: f64 = 150.0;

/// The sub-intervals each mass interval is sampled at, besides its edges: about 1% in mass apart.
pub const MASS_SUBDIVISIONS: usize = 8;

/// The nodes of log₁₀(Z<sub>fit</sub> ÷ 0.02), dex, between which the table's metallicity
/// intervals lie: the tracks' clamps at Z = 10⁻⁴ and 0.03 (−2.301 and +0.176) at the ends, then
/// −2, −1.5, −1, −0.5, −0.25 and 0.
pub const FE_H_NODES: [f64; 8] = [
    -2.301_029_995_663_981,
    -2.0,
    -1.5,
    -1.0,
    -0.5,
    -0.25,
    0.0,
    0.176_091_259_055_681_24,
];

/// The sub-intervals each metallicity interval is sampled at, besides its edges: at most
/// 0.083 dex apart.
pub const FE_H_SUBDIVISIONS: usize = 6;

/// The metallicity intervals.
pub const FE_H_CELLS: usize = FE_H_NODES.len() - 1;

/// The cells of mass and metallicity, mass-major: cell `k × FE_H_CELLS + j` is mass interval `k`
/// and metallicity interval `j`.
pub const CELLS: usize = MASS_CELLS * FE_H_CELLS;

/// The standard-normal draws of Reimers η (η = 0.5 + 0.07 z, plan 06's design note 7) at which
/// every sampled star is built: η = 0, no Reimers wind, where a giant keeps the most envelope,
/// then from −3.5σ to +3.5σ, densest where the draws are, and +7σ.
pub const ETA_DRAWS: [f64; 11] = [
    -0.5 / 0.07,
    -3.5,
    -2.5,
    -1.5,
    -0.75,
    0.0,
    0.75,
    1.5,
    2.5,
    3.5,
    7.0,
];

/// The upper age of the first bin, years: younger stars are protostars or contracting, and the
/// engine reads every star there at its arrival.
pub const FIRST_AGE_EDGE_YEARS: f64 = 1.0e5;

/// The width of the log-age bins after the first, dex.
pub const AGE_STEP_DEX: f64 = 0.05;

/// The age bins: the first, to [`FIRST_AGE_EDGE_YEARS`], then 104 of [`AGE_STEP_DEX`] to
/// 10^10.2 years, past any star of any galaxy.
pub const AGE_BINS: usize = 105;

/// The factor by which each bin's upper age is stretched before the samples are read there.
///
/// A star between two samples reaches their radii somewhat later or earlier in age. Hurley, Pols
/// and Tout's (2000) base-of-the-giant-branch time (their equation 4) changes by up to 4% across a
/// mass sub-interval of [`MASS_SUBDIVISIONS`] and up to 10.5% across 0.167 dex of metallicity
/// (P11.T17.a's science check, at 0.8–0.9 M☉): so about 5% across a sub-interval of
/// [`FE_H_SUBDIVISIONS`], and at most about 9.5% corner to corner, which the factor covers. The
/// dense slow test checks it, with a stratum at the old turnoff.
pub const AGE_SPREAD: f64 = 1.1;

/// The margin added to every value, dex: for the stars between the samples, which the dense slow
/// test measures.
pub const RADIUS_MARGIN_DEX: f64 = 0.02;

/// The most the drop allowance adds to a sample, dex: about twice the steepest rise of the largest
/// radius between neighbouring mass samples short of a discontinuity in P11.T17.a's probe. At
/// \[Fe/H\] −2.3 the largest radius climbs from 1,308 R☉ at 41.60 M☉ through 1,440 R☉ at the
/// sample at 41.89 M☉ to 1,531 R☉ at 42.19 M☉, about 0.06 dex a sample, and falls 25% by
/// 42.20 M☉; the sample short of the drop lies below the peak. A larger step between samples is
/// itself a discontinuity, a phase reached by the heavier sample and not the lighter, not a slope
/// to carry on.
pub const DROP_ALLOWANCE_MAX_DEX: f64 = 0.1;

/// The upper age of bin `k`, years, for `k` below [`AGE_BINS`]: [`FIRST_AGE_EDGE_YEARS`] for the
/// first, then 10^(5 + 0.05 k).
///
/// # Panics
///
/// If `k` is [`AGE_BINS`] or more.
#[must_use]
pub(crate) fn age_edge_years(k: usize) -> f64 {
    assert!(k < AGE_BINS, "age bin {k} of {AGE_BINS}");
    let k = u32::try_from(k).expect("at most 105");
    math::exp10(math::log10(FIRST_AGE_EDGE_YEARS) + AGE_STEP_DEX * f64::from(k))
}

/// The bin that holds `age_years`, or `None` past the last edge or for an age that is not a
/// number. A negative age is the first bin's.
#[must_use]
pub(crate) fn age_bin(age_years: f64) -> Option<usize> {
    if age_years.is_nan() {
        return None;
    }
    if age_years <= FIRST_AGE_EDGE_YEARS {
        return Some(0);
    }
    let steps = (math::log10(age_years) - math::log10(FIRST_AGE_EDGE_YEARS)) / AGE_STEP_DEX;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive number of steps below a few hundred, checked against the bins"
    )]
    let k = steps.ceil().clamp(1.0, 1.0e3) as usize;
    (k < AGE_BINS).then_some(k)
}

/// The width of a mass interval in ln m.
#[must_use]
fn mass_step() -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "96 cells")]
    let cells = MASS_CELLS as f64;
    math::ln(HIGHEST_MASS_MSUN / LOWEST_MASS_MSUN) / cells
}

/// The `n`-th sampled mass, M☉, for `n` from 0 to [`MASS_CELLS`] × [`MASS_SUBDIVISIONS`]: even
/// in ln m, every [`MASS_SUBDIVISIONS`]-th an interval's edge, the last exactly
/// [`HIGHEST_MASS_MSUN`].
///
/// # Panics
///
/// If `n` is beyond the last node.
#[must_use]
pub fn mass_node_msun(n: usize) -> f64 {
    let last = MASS_CELLS * MASS_SUBDIVISIONS;
    assert!(n <= last, "mass node {n} of {last}");
    if n == last {
        return HIGHEST_MASS_MSUN;
    }
    let n = u32::try_from(n).expect("a few hundred nodes");
    let per = u32::try_from(MASS_SUBDIVISIONS).expect("a handful");
    LOWEST_MASS_MSUN * math::exp(mass_step() * f64::from(n) / f64::from(per))
}

/// The `n`-th sampled metallicity, log₁₀(Z<sub>fit</sub> ÷ 0.02) in dex, for `n` from 0 to
/// [`FE_H_CELLS`] × [`FE_H_SUBDIVISIONS`]: every [`FE_H_SUBDIVISIONS`]-th a node of
/// [`FE_H_NODES`], linear between them.
///
/// # Panics
///
/// If `n` is beyond the last node.
#[must_use]
pub fn fe_h_node(n: usize) -> f64 {
    let last = FE_H_CELLS * FE_H_SUBDIVISIONS;
    assert!(n <= last, "metallicity node {n} of {last}");
    let (cell, part) = (n / FE_H_SUBDIVISIONS, n % FE_H_SUBDIVISIONS);
    if part == 0 {
        return FE_H_NODES[cell];
    }
    let part = u32::try_from(part).expect("a handful");
    let per = u32::try_from(FE_H_SUBDIVISIONS).expect("a handful");
    let (lo, hi) = (FE_H_NODES[cell], FE_H_NODES[cell + 1]);
    lo + (hi - lo) * f64::from(part) / f64::from(per)
}

/// The metallicity coordinate the table is read at for `comp`: log₁₀(Z<sub>fit</sub> ÷ Z☉), dex,
/// held inside [`FE_H_NODES`], which the tracks' clamp makes exact.
#[must_use]
pub(crate) fn fe_h_of(comp: &Composition) -> f64 {
    let x = math::log10(comp.z_fit().value() / Z_SOLAR.value());
    x.clamp(FE_H_NODES[0], FE_H_NODES[FE_H_CELLS])
}

/// Whether `comp` has a helium excess, or one that is not a number: the table's tracks hold
/// none, so such a composition is outside it.
#[must_use]
pub(crate) fn has_helium_excess(comp: &Composition) -> bool {
    let excess = comp.helium_excess().value();
    excess.is_nan() || excess > 0.0
}

/// One sampled star of the table at each age bin, before the cells take their largest.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachNode {
    /// Per age bin, log₁₀ of the largest radius, in R☉, over the draws, as [`reach_node`] reads
    /// it.
    pub log_radius_rsun: [f64; AGE_BINS],
    /// The earliest age, years, at which any of its draws has left its main sequence, divided by
    /// [`AGE_SPREAD`]; +∞ for a star on the cooling fits.
    pub main_sequence_end_years: f64,
    /// The earliest age, years, at which any of its draws dies suddenly, divided by
    /// [`AGE_SPREAD`]; +∞ if none does.
    pub earliest_collapse_years: f64,
}

/// The table's samples at initial mass `m_msun` (M☉) and metallicity coordinate `fe_h` (dex,
/// log₁₀(Z<sub>fit</sub> ÷ 0.02)): for each age bin the largest radius over the [`ETA_DRAWS`],
/// each a full track at the median draws but η and the companion-stripped mark (never set, and
/// inside [`STRIPPED_WINDOW`] also always set), read at the engine's age
/// ([`engine_track_age_years`]) of the bin's upper age times [`AGE_SPREAD`]; the earliest end of a
/// main sequence and the earliest sudden death among them.
///
/// A star below [`MIN_INITIAL_MASS`] is P06.T13's cooling fits at age 0, the largest radius the
/// engine reads for it, in every bin.
///
/// # Panics
///
/// - If `m_msun` is outside [`LOWEST_MASS_MSUN`]–[`HIGHEST_MASS_MSUN`] or `fe_h` is not finite.
/// - If a track gives a radius or an age that is not a number, or a radius that is not positive
///   and finite, so that the fit fails rather than write an unsound cell.
#[must_use]
pub fn reach_node(m_msun: f64, fe_h: f64) -> ReachNode {
    assert!(
        (LOWEST_MASS_MSUN..=HIGHEST_MASS_MSUN).contains(&m_msun),
        "a reach sample's mass, {m_msun} M_sun, is inside the table"
    );
    assert!(fe_h.is_finite(), "a reach sample's metallicity is finite");
    let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    let mass = SolarMasses::new(m_msun);
    let log_of = |radius: f64| {
        assert!(
            radius.is_finite() && radius > 0.0,
            "{m_msun} M_sun at {fe_h} dex has a radius of {radius} R_sun"
        );
        math::log10(radius)
    };
    if mass < MIN_INITIAL_MASS {
        let radius = substellar::cooling(mass, Years::ZERO, &comp)
            .expect("the cooling fits hold 0.08-0.1 M_sun")
            .radius()
            .value();
        return ReachNode {
            log_radius_rsun: [log_of(radius); AGE_BINS],
            main_sequence_end_years: f64::INFINITY,
            earliest_collapse_years: f64::INFINITY,
        };
    }
    let marks: &[Mark] = if (STRIPPED_WINDOW.0..=STRIPPED_WINDOW.1).contains(&m_msun) {
        &[NEVER_STRIPPED, Mark::from_word(0)]
    } else {
        &[NEVER_STRIPPED]
    };
    let mut log_radius_rsun = [f64::NEG_INFINITY; AGE_BINS];
    let mut main_sequence_end_years = f64::INFINITY;
    let mut earliest_collapse_years = f64::INFINITY;
    for &mark in marks {
        for &z in &ETA_DRAWS {
            let draws = draws_at(z, mark);
            let track = Track::full(super::evolve::track_mass(mass), &comp, &draws);
            for (k, slot) in log_radius_rsun.iter_mut().enumerate() {
                let at = engine_track_age_years(&track, 0.0, age_edge_years(k) * AGE_SPREAD);
                *slot = slot.max(log_of(track.max_radius_until(Years::new(at)).value()));
            }
            // A track with no main sequence holds a core from the start, as the engine reads it.
            let end = track
                .main_sequence_end()
                .map_or(0.0, |end| end.value() / AGE_SPREAD);
            assert!(!end.is_nan(), "{m_msun} M_sun has a main sequence's end");
            main_sequence_end_years = main_sequence_end_years.min(end);
            if let Some(fate) = track.fate_with(RemnantDraws::of(&draws))
                && fate.death.kind().is_sudden()
            {
                let death = fate.death.age().value() / AGE_SPREAD;
                assert!(!death.is_nan(), "{m_msun} M_sun has a death age");
                earliest_collapse_years = earliest_collapse_years.min(death);
            }
        }
    }
    ReachNode {
        log_radius_rsun,
        main_sequence_end_years,
        earliest_collapse_years,
    }
}

/// The draws of a sample: η at the standard-normal `z`, the companion-stripped mark `stripped`,
/// and every other draw at its median.
#[must_use]
fn draws_at(z: f64, stripped: Mark) -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        eta: StandardNormal::new(z).expect("the draws of the table are finite"),
        stripped,
        ..StarDrawsParts::MEDIAN
    })
}

/// `log_radius_rsun` (dex of R☉) plus [`RADIUS_MARGIN_DEX`], in hundredths of a dex rounded up,
/// so that the stored value bounds it; `None` if it is not finite or beyond an `i16`.
#[must_use]
pub(crate) fn to_centidex(log_radius_rsun: f64) -> Option<i16> {
    let v = ((log_radius_rsun + RADIUS_MARGIN_DEX) * 100.0).ceil();
    if !v.is_finite() || v < f64::from(i16::MIN) || v > f64::from(i16::MAX) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number inside the i16 range, checked above"
    )]
    let k = v as i16;
    Some(k)
}

/// The radius, R☉, of a stored value `k`: 10^(k ÷ 100).
#[must_use]
pub fn centidex_to_rsun(k: i16) -> f64 {
    math::exp10(f64::from(k) / 100.0)
}

/// The table's cells, as `hyperion-fit` writes them.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachCells {
    /// Per cell (mass-major, see [`CELLS`]), per age bin, the stored radius in hundredths of a
    /// dex of R☉, rounded up, non-decreasing in age.
    pub log_radius_centidex: Vec<[i16; AGE_BINS]>,
    /// Per cell, the earliest end of a main sequence among its samples, years
    /// ([`ReachNode::main_sequence_end_years`]).
    pub main_sequence_end_years: Vec<f64>,
    /// Per cell, the earliest sudden death among its samples, years
    /// ([`ReachNode::earliest_collapse_years`]).
    pub earliest_collapse_years: Vec<f64>,
}

/// An error in [`assemble`]'s input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AssembleReachError {
    /// The samples are not one per mass node and metallicity node.
    WrongShape {
        /// The samples the grid holds.
        expected: usize,
        /// The samples given.
        found: usize,
    },
    /// A sample holds a log radius that is not finite, or an age that is not a number.
    UnsoundSample {
        /// The sample's index, mass node major: mass node × metallicity nodes + metallicity node.
        sample: usize,
    },
    /// A cell's largest radius is beyond what the table can store.
    UnstorableCell {
        /// The cell's index (see [`CELLS`]).
        cell: usize,
    },
}

impl fmt::Display for AssembleReachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongShape { expected, found } => {
                write!(f, "expected {expected} reach samples, found {found}")
            }
            Self::UnsoundSample { sample } => {
                write!(
                    f,
                    "reach sample {sample} holds a value that is not a number"
                )
            }
            Self::UnstorableCell { cell } => {
                write!(f, "reach cell {cell} holds a radius the table cannot store")
            }
        }
    }
}

impl Error for AssembleReachError {}

/// The table's cells from its samples: `nodes` holds [`reach_node`] at every mass node of
/// [`mass_node_msun`] and metallicity node of [`fe_h_node`], mass node major.
///
/// Each cell takes the largest of the samples on and inside its edges, in a fixed order, with the
/// drop allowance (the module's documentation) between neighbouring mass samples; its row is
/// then made non-decreasing in age and stored rounded up.
///
/// # Errors
///
/// - [`AssembleReachError::WrongShape`] if `nodes` is not the grid's shape;
/// - [`AssembleReachError::UnsoundSample`] if a sample's log radius is not finite or one of its
///   ages is not a number;
/// - [`AssembleReachError::UnstorableCell`] if a cell's value is beyond an `i16`.
pub fn assemble(nodes: &[ReachNode]) -> Result<ReachCells, AssembleReachError> {
    let per_mass = FE_H_CELLS * FE_H_SUBDIVISIONS + 1;
    let expected = (MASS_CELLS * MASS_SUBDIVISIONS + 1) * per_mass;
    if nodes.len() != expected {
        return Err(AssembleReachError::WrongShape {
            expected,
            found: nodes.len(),
        });
    }
    if let Some(sample) = nodes.iter().position(|node| {
        node.log_radius_rsun.iter().any(|v| !v.is_finite())
            || node.main_sequence_end_years.is_nan()
            || node.earliest_collapse_years.is_nan()
    }) {
        return Err(AssembleReachError::UnsoundSample { sample });
    }
    let mut log_radius_centidex = Vec::with_capacity(CELLS);
    let mut main_sequence_end_years = Vec::with_capacity(CELLS);
    let mut earliest_collapse_years = Vec::with_capacity(CELLS);
    for k in 0..MASS_CELLS {
        for j in 0..FE_H_CELLS {
            let mut largest = [f64::NEG_INFINITY; AGE_BINS];
            let mut earliest = f64::INFINITY;
            let mut collapse = f64::INFINITY;
            for mass in k * MASS_SUBDIVISIONS..=(k + 1) * MASS_SUBDIVISIONS {
                for fe_h in j * FE_H_SUBDIVISIONS..=(j + 1) * FE_H_SUBDIVISIONS {
                    let node = &nodes[mass * per_mass + fe_h];
                    for (slot, &v) in largest.iter_mut().zip(&node.log_radius_rsun) {
                        *slot = slot.max(v);
                    }
                    earliest = earliest.min(node.main_sequence_end_years);
                    collapse = collapse.min(node.earliest_collapse_years);
                }
            }
            // A drop between two samples in mass may hide a peak just short of it.
            for mass in k * MASS_SUBDIVISIONS..(k + 1) * MASS_SUBDIVISIONS {
                for fe_h in j * FE_H_SUBDIVISIONS..=(j + 1) * FE_H_SUBDIVISIONS {
                    let at = |n: usize| {
                        nodes
                            .get(n * per_mass + fe_h)
                            .map(|node| &node.log_radius_rsun)
                    };
                    let (Some(lo), Some(hi)) = (at(mass), at(mass + 1)) else {
                        continue;
                    };
                    let outer = [mass.checked_sub(1).and_then(at), at(mass + 2)];
                    for (b, slot) in largest.iter_mut().enumerate() {
                        *slot = slot.max(drop_allowance(
                            [outer[0].map(|o| o[b]), outer[1].map(|o| o[b])],
                            [lo[b], hi[b]],
                        ));
                    }
                }
            }
            // The largest radius by an age never falls with it; the allowance can switch sides
            // between bins, so the row is made a running maximum.
            let mut so_far = f64::NEG_INFINITY;
            for slot in &mut largest {
                so_far = so_far.max(*slot);
                *slot = so_far;
            }
            let cell = k * FE_H_CELLS + j;
            let mut row = [0_i16; AGE_BINS];
            for (stored, &v) in row.iter_mut().zip(&largest) {
                *stored = to_centidex(v).ok_or(AssembleReachError::UnstorableCell { cell })?;
            }
            log_radius_centidex.push(row);
            main_sequence_end_years.push(earliest);
            earliest_collapse_years.push(collapse);
        }
    }
    Ok(ReachCells {
        log_radius_centidex,
        main_sequence_end_years,
        earliest_collapse_years,
    })
}

/// The bound, dex, on the largest log radius between two neighbouring samples in mass of log
/// radii `inner` (the lighter first) whose outer neighbours are `outer`, beside the samples'
/// own.
///
/// Where the lighter sample is the larger, a peak short of a drop is bounded by it plus the rise
/// into it from its own lighter neighbour, at most [`DROP_ALLOWANCE_MAX_DEX`], and likewise from
/// the heavier side; elsewhere the samples bound it. The dense slow test measures what remains.
#[must_use]
fn drop_allowance(outer: [Option<f64>; 2], inner: [f64; 2]) -> f64 {
    let [lo, hi] = inner;
    let rise = |from: f64, to: f64| (to - from).clamp(0.0, DROP_ALLOWANCE_MAX_DEX);
    let mut bound = f64::NEG_INFINITY;
    if hi < lo
        && let Some(before) = outer[0]
    {
        bound = bound.max(lo + rise(before, lo));
    }
    if lo < hi
        && let Some(after) = outer[1]
    {
        bound = bound.max(hi + rise(after, hi));
    }
    bound
}

/// What the table bounds of the stars of a mass interval at an age: their largest radius,
/// whether any may hold a core, and whether any may have died suddenly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReachBound {
    radius_rsun: f64,
    may_hold_core: bool,
    may_have_collapsed: bool,
}

impl ReachBound {
    /// The largest radius any of the stars can have had by the age, R☉: never below the
    /// `max_radius_until` the binary engine reads at its age.
    #[must_use]
    pub const fn radius_rsun(&self) -> f64 {
        self.radius_rsun
    }

    /// Whether any of the stars may have left its main sequence by the age, and so hold a core.
    #[must_use]
    pub const fn may_hold_core(&self) -> bool {
        self.may_hold_core
    }

    /// Whether any of the stars may have died suddenly by the age, as a single star: a core's
    /// collapse, an electron capture or a pair instability, which a kick can follow.
    #[must_use]
    pub const fn may_have_collapsed(&self) -> bool {
        self.may_have_collapsed
    }
}

/// The reach table's reader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReachTable<'t> {
    log_radius_centidex: &'t [[i16; AGE_BINS]],
    main_sequence_end_years: &'t [f64],
    earliest_collapse_years: &'t [f64],
}

impl ReachTable<'static> {
    /// The generator's table, [`tables::binary_reach`](crate::tables::binary_reach).
    #[must_use]
    pub(crate) const fn generator() -> Self {
        Self {
            log_radius_centidex: &binary_reach::LOG_RADIUS_CENTIDEX,
            main_sequence_end_years: &binary_reach::MAIN_SEQUENCE_END_YEARS,
            earliest_collapse_years: &binary_reach::EARLIEST_COLLAPSE_YEARS,
        }
    }
}

#[cfg(test)]
impl<'t> ReachTable<'t> {
    /// A reader over cells written by [`assemble`].
    #[must_use]
    pub(crate) fn of(cells: &'t ReachCells) -> Self {
        Self {
            log_radius_centidex: &cells.log_radius_centidex,
            main_sequence_end_years: &cells.main_sequence_end_years,
            earliest_collapse_years: &cells.earliest_collapse_years,
        }
    }
}

impl ReachTable<'_> {
    /// The bound over the stars of initial mass `lo_msun`–`hi_msun` (M☉) and metallicity
    /// coordinate `fe_h` ([`fe_h_of`]) at the engine's age `age_years`, or `None` where the table
    /// does not reach: a mass below [`LOWEST_MASS_MSUN`] or not finite, an age past the last bin,
    /// or a table of the wrong shape. A mass above [`HIGHEST_MASS_MSUN`] reads it, as its track
    /// does.
    #[must_use]
    pub(crate) fn bound(
        &self,
        lo_msun: f64,
        hi_msun: f64,
        fe_h: f64,
        age_years: f64,
    ) -> Option<ReachBound> {
        if self.log_radius_centidex.len() != CELLS
            || self.main_sequence_end_years.len() != CELLS
            || self.earliest_collapse_years.len() != CELLS
        {
            return None;
        }
        if !(lo_msun >= LOWEST_MASS_MSUN && hi_msun >= lo_msun && hi_msun.is_finite())
            || fe_h.is_nan()
        {
            return None;
        }
        let bin = age_bin(age_years)?;
        let (first, last) = (mass_cell(lo_msun), mass_cell(hi_msun));
        let j = fe_h_cell(fe_h);
        let mut largest = i16::MIN;
        let mut may_hold_core = false;
        let mut may_have_collapsed = false;
        for k in first..=last {
            let cell = k * FE_H_CELLS + j;
            largest = largest.max(self.log_radius_centidex[cell][bin]);
            may_hold_core |= age_years > self.main_sequence_end_years[cell];
            may_have_collapsed |= age_years >= self.earliest_collapse_years[cell];
        }
        Some(ReachBound {
            radius_rsun: centidex_to_rsun(largest),
            may_hold_core,
            may_have_collapsed,
        })
    }
}

/// The mass interval holding `m_msun` (M☉, at least [`LOWEST_MASS_MSUN`]): a mass on an edge may
/// be read in either interval it bounds, since both hold the edge's samples.
#[must_use]
fn mass_cell(m_msun: f64) -> usize {
    let x = math::ln(m_msun.min(HIGHEST_MASS_MSUN) / LOWEST_MASS_MSUN) / mass_step();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative number of cells, held below the count"
    )]
    let k = x.floor().clamp(0.0, 1.0e3) as usize;
    k.min(MASS_CELLS - 1)
}

/// The metallicity interval holding `fe_h` (dex, inside [`FE_H_NODES`]).
#[must_use]
fn fe_h_cell(fe_h: f64) -> usize {
    (1..FE_H_CELLS)
        .take_while(|&j| fe_h >= FE_H_NODES[j])
        .last()
        .unwrap_or(0)
}

/// The reach table's bound for stars of initial mass `masses` and composition `comp` at the
/// engine's age `age` (plan 11, P11.T17.a), or `None` where the table does not reach: a mass
/// below 0.08 M☉ or not finite, an age past 10^10.2 years, or a composition with a helium excess,
/// which the table's tracks do not hold.
///
/// # Examples
///
/// Whether a companion could have filled the Roche lobe of a 1,000-day orbit without building its
/// track: a 1.2 M☉ star at solar metallicity is still a dwarf at 2 Gyr, and has climbed its giant
/// branch by 6 Gyr.
///
/// ```
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::binary::largest_radius_bound;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let star = SolarMasses::new(1.2);
/// let young = largest_radius_bound(star..=star, &Composition::SOLAR, Years::new(2.0e9))
///     .ok_or("inside the table")?;
/// let old = largest_radius_bound(star..=star, &Composition::SOLAR, Years::new(6.0e9))
///     .ok_or("inside the table")?;
/// assert!(young.radius_rsun() < 3.0);
/// assert!(old.radius_rsun() > 100.0 && old.may_hold_core());
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn largest_radius_bound(
    masses: std::ops::RangeInclusive<SolarMasses>,
    comp: &Composition,
    age: Years,
) -> Option<ReachBound> {
    if has_helium_excess(comp) {
        return None;
    }
    ReachTable::generator().bound(
        masses.start().value(),
        masses.end().value(),
        fe_h_of(comp),
        age.value(),
    )
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    #[test]
    fn the_age_bins_cover_their_edges() {
        assert_eq!(age_bin(0.0), Some(0));
        assert_eq!(age_bin(-5.0), Some(0));
        assert_eq!(age_bin(1.0e5), Some(0));
        assert_eq!(age_bin(f64::NAN), None);
        for k in 1..AGE_BINS {
            let edge = age_edge_years(k);
            let inside = age_bin(edge * 0.999).expect("inside");
            assert_eq!(inside, k, "just below edge {k}");
            assert!(age_edge_years(inside) >= edge * 0.999);
        }
        assert!((age_edge_years(AGE_BINS - 1) - math::exp10(10.2)).abs() < 1.0e5);
        assert_eq!(age_bin(1.6e10), None);
    }

    #[test]
    fn the_nodes_span_the_grid() {
        assert!((mass_node_msun(0) - LOWEST_MASS_MSUN).abs() < 1e-15);
        assert_same_bits(
            mass_node_msun(MASS_CELLS * MASS_SUBDIVISIONS),
            HIGHEST_MASS_MSUN,
        );
        let last_fe_h = FE_H_CELLS * FE_H_SUBDIVISIONS;
        assert_same_bits(fe_h_node(0), FE_H_NODES[0]);
        assert_same_bits(fe_h_node(last_fe_h), FE_H_NODES[FE_H_CELLS]);
        for j in 0..last_fe_h {
            assert!(fe_h_node(j) < fe_h_node(j + 1));
        }
        // The ends are the tracks' clamps of Z.
        let poor = Composition::from_fe_h(Dex::new(-3.0), HeliumExcess::ZERO);
        let rich = Composition::from_fe_h(Dex::new(0.5), HeliumExcess::ZERO);
        let z = Z_SOLAR.value();
        assert!((math::log10(poor.z_fit().value() / z) - FE_H_NODES[0]).abs() < 1e-12);
        assert!((math::log10(rich.z_fit().value() / z) - FE_H_NODES[FE_H_CELLS]).abs() < 1e-12);
        assert_same_bits(fe_h_of(&poor), FE_H_NODES[0]);
        assert_same_bits(fe_h_of(&rich), FE_H_NODES[FE_H_CELLS]);
    }

    #[test]
    fn every_mass_falls_in_a_cell_whose_edges_hold_it() {
        let step = mass_step();
        for n in 0..=MASS_CELLS * MASS_SUBDIVISIONS {
            let m = mass_node_msun(n);
            let k = mass_cell(m);
            let lo =
                LOWEST_MASS_MSUN * math::exp(step * f64::from(u32::try_from(k).expect("small")));
            let hi = mass_node_msun((k + 1) * MASS_SUBDIVISIONS);
            assert!(
                m >= lo * (1.0 - 1e-12) && m <= hi * (1.0 + 1e-12),
                "{m} in cell {k}"
            );
        }
        assert_eq!(mass_cell(200.0), MASS_CELLS - 1);
        for (j, &node) in FE_H_NODES.iter().enumerate().take(FE_H_CELLS) {
            assert_eq!(fe_h_cell(node), j);
        }
        assert_eq!(fe_h_cell(FE_H_NODES[FE_H_CELLS]), FE_H_CELLS - 1);
    }

    #[test]
    fn stored_values_round_up_and_read_back() {
        for v in [-1.05, -0.3, 0.0, 0.004, 1.234_5, 3.7] {
            let k = to_centidex(v).expect("storable");
            assert!(math::log10(centidex_to_rsun(k)) >= v + RADIUS_MARGIN_DEX - 1e-12);
            assert!(math::log10(centidex_to_rsun(k)) < v + RADIUS_MARGIN_DEX + 0.010_001);
        }
        assert_eq!(to_centidex(f64::INFINITY), None);
        assert_eq!(to_centidex(f64::NAN), None);
    }

    /// A node bounds its own tracks at the ages the engine reads, and holds the cooling fits'
    /// radius below 0.1 M☉.
    #[test]
    fn a_node_bounds_its_own_tracks() {
        let node = reach_node(1.0, 0.0);
        let draws = draws_at(0.0, NEVER_STRIPPED);
        let track = Track::full(SolarMasses::new(1.0), &Composition::SOLAR, &draws);
        for age in [1.0e5, 3.0e7, 4.6e9, 1.1e10, 1.3e10] {
            let k = age_bin(age).expect("inside");
            let at = engine_track_age_years(&track, 0.0, age);
            let r = track.max_radius_until(Years::new(at)).value();
            assert!(node.log_radius_rsun[k] >= math::log10(r), "at {age} yr");
        }
        assert!(
            node.earliest_collapse_years.is_infinite(),
            "the Sun leaves a white dwarf"
        );
        let massive = reach_node(20.0, 0.0);
        assert!(massive.earliest_collapse_years < 1.5e7, "{massive:?}");
        let dwarf = reach_node(0.09, 0.0);
        assert!(dwarf.main_sequence_end_years.is_infinite());
        assert!(dwarf.log_radius_rsun.iter().all(|&v| v.is_finite()));
    }

    #[test]
    fn a_drop_between_samples_is_allowed_the_rise_before_it() {
        // Rising into the drop from the left: the lighter sample plus its own rise.
        assert!((drop_allowance([Some(3.0), Some(2.0)], [3.1, 2.9]) - 3.2).abs() < 1e-12);
        // Rising into it from the right.
        assert!((drop_allowance([Some(2.0), Some(3.0)], [2.9, 3.1]) - 3.2).abs() < 1e-12);
        // A step beyond the cap is a phase reached, not a slope.
        let capped = drop_allowance([Some(1.0), Some(2.0)], [3.1, 2.9]);
        assert!((capped - (3.1 + DROP_ALLOWANCE_MAX_DEX)).abs() < 1e-12);
        // A smooth rise or fall needs nothing beyond the samples.
        assert!(drop_allowance([Some(2.8), Some(3.2)], [2.9, 3.1]) <= 3.1);
        assert!(drop_allowance([Some(3.2), Some(2.8)], [3.1, 2.9]) <= 3.1);
        assert_same_bits(drop_allowance([None, None], [3.1, 2.9]), f64::NEG_INFINITY);
    }

    /// A grid of synthetic samples, each with the log radius `value(mass node, metallicity node,
    /// bin)` and no core or collapse.
    fn synthetic(value: impl Fn(usize, usize, usize) -> f64) -> Vec<ReachNode> {
        let per_mass = FE_H_CELLS * FE_H_SUBDIVISIONS + 1;
        (0..=MASS_CELLS * MASS_SUBDIVISIONS)
            .flat_map(|m| (0..per_mass).map(move |z| (m, z)))
            .map(|(m, z)| ReachNode {
                log_radius_rsun: core::array::from_fn(|b| value(m, z, b)),
                main_sequence_end_years: f64::INFINITY,
                earliest_collapse_years: f64::INFINITY,
            })
            .collect()
    }

    #[test]
    fn assemble_refuses_a_grid_of_the_wrong_shape_or_an_unsound_sample() {
        let mut nodes = synthetic(|_, _, _| 0.0);
        let expected = nodes.len();
        assert_eq!(
            assemble(&nodes[1..]),
            Err(AssembleReachError::WrongShape {
                expected,
                found: expected - 1,
            })
        );
        nodes[1_234].log_radius_rsun[7] = f64::NAN;
        assert_eq!(
            assemble(&nodes),
            Err(AssembleReachError::UnsoundSample { sample: 1_234 })
        );
        nodes[1_234].log_radius_rsun[7] = 0.0;
        nodes[77].earliest_collapse_years = f64::NAN;
        assert_eq!(
            assemble(&nodes),
            Err(AssembleReachError::UnsoundSample { sample: 77 })
        );
        nodes[77].earliest_collapse_years = f64::INFINITY;
        nodes[5].log_radius_rsun[0] = 400.0;
        assert_eq!(
            assemble(&nodes),
            Err(AssembleReachError::UnstorableCell { cell: 0 })
        );
    }

    /// One raised sample raises exactly the cells whose edges hold it, from its bin on, and every
    /// row is non-decreasing in age even where a sample falls with it.
    #[test]
    fn a_raised_sample_raises_the_cells_that_hold_it_from_its_bin_on() {
        let per_mass = FE_H_CELLS * FE_H_SUBDIVISIONS + 1;
        // A sample on a corner of four cells: mass edge 40, metallicity node 2's edge.
        let (m, z) = (40 * MASS_SUBDIVISIONS, 2 * FE_H_SUBDIVISIONS);
        let nodes = synthetic(|mass, fe_h, b| {
            if (mass, fe_h) == (m, z) && b == 50 {
                1.0
            } else {
                0.0
            }
        });
        let cells = assemble(&nodes).expect("a sound grid");
        assert_eq!(nodes.len(), (MASS_CELLS * MASS_SUBDIVISIONS + 1) * per_mass);
        for k in 0..MASS_CELLS {
            for j in 0..FE_H_CELLS {
                let row = &cells.log_radius_centidex[k * FE_H_CELLS + j];
                assert!(row.windows(2).all(|w| w[0] <= w[1]), "cell {k}, {j}");
                let holds = (k == 39 || k == 40) && (j == 1 || j == 2);
                let raised = row[60] > to_centidex(0.0).expect("storable");
                assert_eq!(raised, holds, "cell {k}, {j}");
                assert_eq!(
                    row[49],
                    to_centidex(0.0).expect("storable"),
                    "cell {k}, {j}"
                );
            }
        }
    }

    /// Every row of the generator's table is non-decreasing in age, as `max_radius_until` is.
    #[test]
    fn the_generators_rows_never_fall_with_age() {
        for (cell, row) in binary_reach::LOG_RADIUS_CENTIDEX.iter().enumerate() {
            assert!(row.windows(2).all(|w| w[0] <= w[1]), "cell {cell}");
        }
    }

    #[test]
    fn a_short_table_is_not_read() {
        let cells = ReachCells {
            log_radius_centidex: vec![[0; AGE_BINS]; 3],
            main_sequence_end_years: vec![0.0; 3],
            earliest_collapse_years: vec![0.0; 3],
        };
        assert_eq!(ReachTable::of(&cells).bound(1.0, 1.0, 0.0, 1.0e9), None);
    }

    #[test]
    fn the_generator_reads_its_table() {
        let one = SolarMasses::new(1.0);
        let bound = largest_radius_bound(one..=one, &Composition::SOLAR, Years::new(4.6e9))
            .expect("inside the table");
        assert!((0.9..2.0).contains(&bound.radius_rsun()), "{bound:?}");
        assert!(!bound.may_hold_core());
        assert!(!bound.may_have_collapsed());
        let twenty = SolarMasses::new(20.0);
        let at = |age: f64| {
            largest_radius_bound(twenty..=twenty, &Composition::SOLAR, Years::new(age))
                .expect("inside the table")
                .may_have_collapsed()
        };
        assert!(!at(1.0e6));
        assert!(at(5.0e7));
        let wider = largest_radius_bound(
            SolarMasses::new(0.5)..=SolarMasses::new(2.0),
            &Composition::SOLAR,
            Years::new(4.6e9),
        )
        .expect("inside the table");
        assert!(wider.radius_rsun() >= bound.radius_rsun());
        assert!(wider.may_hold_core());
        let top = |m: f64| {
            let m = SolarMasses::new(m);
            largest_radius_bound(m..=m, &Composition::SOLAR, Years::new(1.0e9))
        };
        assert_eq!(top(180.0), top(150.0));
        let light = SolarMasses::new(0.05);
        assert_eq!(
            largest_radius_bound(light..=light, &Composition::SOLAR, Years::new(1.0e9)),
            None
        );
        let helium = Composition::from_fe_h(Dex::ZERO, HeliumExcess::new(0.01));
        assert_eq!(
            largest_radius_bound(one..=one, &helium, Years::new(1.0e9)),
            None
        );
    }

    /// The dense test's samples: `pairs` random pairs from `seed`'s splitmix64 stream, each with
    /// its age, years (see [`the_reach_table_bounds_dense_tracks`]). Every fifth pair's first star
    /// is in the old turnoff's stratum: 0.8–2.2 M☉, \[Fe/H\] −1.2 to −0.1, at an age within 10% of
    /// its own main sequence's end (and no later than 1.5 × 10¹⁰ years), where a sub-interval's lag
    /// in age matters most.
    fn dense_samples(seed: u64, pairs: u32) -> Vec<(super::super::timeline::BinaryInput, f64)> {
        use super::super::timeline::BinaryInput;
        use crate::orbit::{Eccentricity, KeplerElements, Orientation};
        use crate::rng::Mark;
        use crate::units::{GravitationalParameter, Radians, Seconds};

        let mut state = seed;
        let mut word = move || {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit word and 2⁵³ are exact in an f64"
        )]
        let unit = |w: u64| (w >> 11) as f64 / (1_u64 << 53) as f64;
        let log_uniform = |u: f64, lo: f64, hi: f64| lo * math::exp(u * math::ln(hi / lo));
        let star = |words: [u64; 4]| {
            let m = log_uniform(unit(words[0]), LOWEST_MASS_MSUN, HIGHEST_MASS_MSUN);
            let z = if words[1].is_multiple_of(5) {
                -0.5 / 0.07 + (7.5 + 0.5 / 0.07) * unit(words[2])
            } else {
                // Box–Muller from two of the words.
                let r = (-2.0 * math::ln(1.0 - unit(words[2]))).sqrt();
                r * math::cos(core::f64::consts::TAU * unit(words[3]))
            };
            let draws = StarDraws::from_parts(StarDrawsParts {
                eta: StandardNormal::new(z).expect("finite"),
                stripped: Mark::from_word(words[1]),
                ..StarDrawsParts::MEDIAN
            });
            (m, draws)
        };
        (0..pairs)
            .map(|i| {
                let (m1, d1) = star([word(), word(), word(), word()]);
                let (m2, d2) = star([word(), word(), word(), word()]);
                let turnoff = i % 5 == 4;
                let (m1, fe_h) = if turnoff {
                    (
                        log_uniform(unit(word()), 0.8, 2.2),
                        -1.2 + 1.1 * unit(word()),
                    )
                } else {
                    (m1, -2.6 + 2.9 * unit(word()))
                };
                let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
                let age = if turnoff {
                    let track = Track::full(SolarMasses::new(m1), &comp, &d1);
                    let end = track.main_sequence_end().map_or(1.0e10, Years::value);
                    (end * (0.9 + 0.2 * unit(word()))).min(1.5e10)
                } else if i % 2 == 0 {
                    log_uniform(unit(word()), 1.0e4, 1.5e10)
                } else {
                    1.5e10 * unit(word())
                };
                let orbit = KeplerElements::from_period(
                    Seconds::new(1.0e8 * 86_400.0),
                    GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
                    Eccentricity::CIRCULAR,
                    Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))
                        .expect("an orientation"),
                    Radians::new(0.0),
                )
                .expect("an orbit");
                let input = BinaryInput::new(
                    SolarMasses::new(m1),
                    SolarMasses::new(m2),
                    comp,
                    orbit,
                    [d1, d2],
                    Years::new(1.0e10),
                )
                .expect("a pair");
                (input, age)
            })
            .collect()
    }

    /// What the dense test found among some of its samples: the least log ratio of bound to
    /// radius, the stars checked, and each failure described.
    type DenseShare = (f64, u64, Vec<String>);

    /// The dense test's check of every `step`-th sample from the `skip`-th: each star as the
    /// engine builds it (`own_members`) and reads its radius (`largest_radii_rsun`), its core and
    /// its sudden death, against the table.
    fn check_dense(
        samples: &[(super::super::timeline::BinaryInput, f64)],
        skip: usize,
        step: usize,
    ) -> DenseShare {
        use super::super::evolve::{largest_radii_rsun, own_members};
        use super::super::star::Member;

        let mut least = f64::INFINITY;
        let mut stars = 0_u64;
        let mut failures = Vec::new();
        for (input, age) in samples.iter().skip(skip).step_by(step) {
            let members = own_members(input, *age, None);
            let radii = largest_radii_rsun(input, &members, *age);
            for (i, member) in members.iter().enumerate() {
                let m = input.masses()[i];
                let bound = largest_radius_bound(m..=m, input.composition(), Years::new(*age))
                    .expect("inside the table");
                let ratio = math::log10(bound.radius_rsun() / radii[i]);
                least = least.min(ratio);
                stars += 1;
                let (cored, collapsed) = match member {
                    Member::Track { track, .. } => {
                        let cored = match track.main_sequence_end() {
                            Some(end) => end.value() < *age,
                            None => track.main_sequence_arrival().is_none(),
                        };
                        let draws = RemnantDraws::of(&input.draws()[i]);
                        let collapsed = track.fate_with(draws).is_some_and(|fate| {
                            fate.death.kind().is_sudden() && fate.death.age().value() <= *age
                        });
                        (cored, collapsed)
                    }
                    Member::Shaped { .. }
                    | Member::MainSequence { .. }
                    | Member::Cooling { .. }
                    | Member::Frozen { .. }
                    | Member::Remnant { .. }
                    | Member::Gone => (false, false),
                };
                if ratio < 0.0
                    || (cored && !bound.may_hold_core())
                    || (collapsed && !bound.may_have_collapsed())
                {
                    failures.push(format!(
                        "{:.5} M_sun, [Fe/H] {:.3}, eta z {:.3}, age {age:.5e} yr: radius {:.5} \
                         R_sun against {:.5}, cored {cored} against {}, collapsed {collapsed} \
                         against {}",
                        m.value(),
                        input.composition().fe_h().value(),
                        input.draws()[i].eta().value(),
                        radii[i],
                        bound.radius_rsun(),
                        bound.may_hold_core(),
                        bound.may_have_collapsed(),
                    ));
                }
            }
        }
        (least, stars, failures)
    }

    /// P11.T17.a's slow test: over 10⁵ random pairs, 2 × 10⁵ stars, of mass log-uniform over
    /// 0.08–150 M☉, \[Fe/H\] uniform over −2.6 to +0.3 (past both of the tracks' clamps), Reimers
    /// η from the normal law for four in five and uniform over its extremes (η = 0 to +7.5σ) for
    /// the rest, a random companion-stripped mark, and an age log-uniform over 10⁴–1.5 × 10¹⁰
    /// years for half and uniform over 0–1.5 × 10¹⁰ for the rest: the table bounds each radius the
    /// engine's pre-test reads (`largest_radii_rsun` of the stars as `own_members` builds them),
    /// says a star may hold a core wherever the engine reads one, and says it may have collapsed
    /// wherever its own track has died suddenly by then. A fifth of the pairs put their first star
    /// at the old turnoff ([`dense_samples`]). The least ratio of bound to radius is the margin
    /// used. Three other seeds passed too when the table was fitted (plan 11's Risks, "P11.T17.a
    /// as built").
    #[test]
    #[ignore = "slow: 2 × 10⁵ stars built as the binary engine builds them"]
    fn the_reach_table_bounds_dense_tracks() {
        const SHARES: usize = 8;
        let samples = dense_samples(0x0b17_0017_aeac_4001, 100_000);
        let share = |k: usize| check_dense(&samples, k, SHARES);
        #[cfg(not(target_family = "wasm"))]
        let parts: Vec<DenseShare> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..SHARES)
                .map(|k| {
                    let share = &share;
                    scope.spawn(move || share(k))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("a share's thread"))
                .collect()
        });
        #[cfg(target_family = "wasm")]
        let parts: Vec<DenseShare> = (0..SHARES).map(share).collect();
        let least = parts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let stars: u64 = parts.iter().map(|p| p.1).sum();
        let failures: Vec<&String> = parts.iter().flat_map(|p| &p.2).collect();
        println!(
            "the reach table over {stars} stars: least margin {least:.5} dex ({:.2}%); {} failures",
            100.0 * (math::exp10(least) - 1.0),
            failures.len()
        );
        for line in failures.iter().take(30) {
            println!("  {line}");
        }
        assert!(
            failures.is_empty(),
            "{} stars outside the bound",
            failures.len()
        );
    }
}
