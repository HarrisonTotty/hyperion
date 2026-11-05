//! The wind's reach (plan 11, P11.T12's fix): from fitted tables alone, whether a star of a wide
//! pair can take 10⁻⁴ M☉ of its partner's wind by an age, for the census's pair bound
//! ([`pair_light_bound`](super::pair_light_bound)), whose `Detached` it bounds.
//!
//! From version 27 a pair the pre-test's lobe and decay tests pass over is still sent to the engine
//! where its main-sequence star takes `LEAST_ACCRETED_MSUN` of its partner's wind
//! (`wind_accretion`), and the star then departs from its own model. The
//! bound's `Detached`, which reads only the lobe and decay tests, answered `Detached` for such
//! pairs, and the census took each of their stars for its own model: train 27 found a bulge
//! companion of 0.70 M☉ grown to 0.95 M☉, at M<sub>V</sub> 4.26 against its bound's 6.02. So
//! from version 29 the bound calls a pair `Detached` only where this module's screen
//! (`may_take_wind`) clears the threshold by the window's end, for each of its stars as the
//! accretor and the other as the donor.
//!
//! # The screen
//!
//! The walk adds, step by step over the donor's track (`wind_accretion::walk_steps`), what the
//! accretor on its own track takes: the step's loss ΔM times the larger of BSE equation 6's
//! Bondi–Hoyle share and, beside an AGB donor, wind Roche-lobe overflow's (Abate et al. 2013, A&A
//! 552, A26). It counts the steps that start before the age it is run to, the last of which may
//! end past it. Each share is bounded, every factor at its worst:
//!
//! - **Bondi–Hoyle** (Hurley, Tout and Pols 2002, MNRAS 329, 897, equations 6 and 9): the share is
//!   at most `α_W` (G `m_a` ÷ `v_W²`)² ÷ (2 a² √(1 − e²)), with `v_W²` = 2 `β_W` G M ÷ R, which is
//!   `α_W` `m_a`² (R ÷ (`β_W` M))² ÷ (8 a² √(1 − e²)), the orbital speed's factor (≥ 1) and
//!   equation 10's cap dropped. The accretor is at most its initial mass, the top of its interval:
//!   the walk reads it on its own track, whose mass only falls. The orbit is at least the drawn
//!   one (the Jeans mode only widens it), and a² √(1 − e²) = `r_p`² √(1 + e) ÷ (1 − e)^{3/2} is at
//!   least `r_p`², the least periastron's square. So the star takes at most
//!   `α_W` `m_a`² S ÷ (8 `r_p`²), where S, the donor's Bondi–Hoyle sum, is Σ ΔM (R ÷ (`β_W` M))²
//!   over the walk's steps (R☉² M☉⁻¹), each step's R, M and `β_W` (the donor's type's, BSE
//!   equation 9 as `WindSpeedFactor` reads it) at its middle, as the walk reads them.
//! - **Overflow** (Abate et al. 2013, equation 9): its efficiency never falls as q = `m_a` ÷ M or
//!   x = `R_d` ÷ `R_L` grows, and Eggleton's lobe fraction (1983, ApJ 268, 368, equation 2) grows
//!   with M ÷ `m_a`. So over the
//!   donor's AGB steps it is at most the AGB's loss times the efficiency at the accretor's initial
//!   mass, the donor's least AGB mass and its largest dust radius (`wind_accretion::dust_radius`), the lobe at
//!   the least periastron. It applies once the donor may have reached its AGB.
//!
//! Both sums are the donor's alone: the table holds them. The table's steps are the walk's between
//! the donor's mass knots; its first and last spans are cut from age 0 and at the donor's death,
//! where the walk's are cut from the accretor's arrival and at the end of the donor's build, and
//! there [`SUM_MARGIN_DEX`] and [`AGE_SPREAD`] cover the difference (`the_screen_holds_every_wind_fed_pair`
//! checks it). Equations 6 and 9 of BSE and Abate et al.'s equation 9 are as `wind_accretion` takes
//! them (recalled, not read in the pod).
//!
//! # The table
//!
//! On the reach table's grid ([`reach`](super::reach)): its cells, its samples (each mass and
//! metallicity node at every Reimers η draw of [`ETA_DRAWS`], with the companion-stripped mark
//! both set and not inside its window) and its age bins. Each sample is walked from age 0 over its
//! whole life as the walk walks a donor, at the generator's [`BinaryParams`]. Each cell holds:
//!
//! - per age bin, the largest Bondi–Hoyle sum of its samples over the steps that start before the
//!   bin's upper age times [`AGE_SPREAD`], made non-decreasing in age, plus [`SUM_MARGIN_DEX`], in
//!   hundredths of a dex rounded up ([`NO_WIND_CENTIDEX`] for none);
//! - the earliest start of an AGB step among its samples, ÷ [`AGE_SPREAD`], and the most AGB loss,
//!   the least AGB mass and the largest dust radius, each widened by [`AGB_MARGIN`].
//!
//! A star below 0.1 M☉ is P06.T13's cooling fits, not a track, and the walk feeds no pair of one,
//! so its cells hold no wind. The table depends on no galaxy, only on the generator's tracks and
//! binary parameters. `hyperion-fit`'s task `binary_wind_reach` builds it from [`wind_reach_node`]
//! and [`assemble`] and checks it in as [`tables::binary_wind_reach`](crate::tables::binary_wind_reach);
//! a change to the tracks or the walk that would leave it stale fails the task's sim fingerprint
//! in `just fit-check`.
//!
//! Every value only grows with the age and with the mass intervals, and the screen's terms only
//! shrink as the periastron widens, so the bound's `Detached` keeps its monotonicity (the light
//! module's "inclusion monotonicity").

use std::error::Error;
use std::fmt;

use crate::galaxy::displaced::binarity::NEVER_STRIPPED;
use crate::math;
use crate::rng::Mark;
use crate::stellar::Composition;
use crate::stellar::fates::STRIPPED_WINDOW;
use crate::stellar::sse::{MIN_INITIAL_MASS, Track};
use crate::tables::binary_wind_reach;
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

use super::BinaryParams;
use super::detached::{last_living, lobe_fraction};
use super::evolve::track_mass;
use super::reach::{
    AGE_BINS, AGE_SPREAD, CELLS, ETA_DRAWS, FE_H_CELLS, FE_H_SUBDIVISIONS, HIGHEST_MASS_MSUN,
    LOWEST_MASS_MSUN, MASS_CELLS, MASS_SUBDIVISIONS, age_bin, age_edge_years, draws_at, fe_h_cell,
    mass_cell,
};
use super::star::Kind;
use super::wind_accretion::{
    LEAST_ACCRETED_MSUN, STEPS_PER_INTERVAL, dust_radius, on_the_agb, walk_steps,
    wind_roche_lobe_efficiency,
};

/// The margin added to every cell's Bondi–Hoyle sum, dex: 0.1, a factor of 1.26, for the stars
/// between the samples (lean, 2026-11-05: the reach table's 0.02 dex covers a radius, and the sum
/// grows as a radius squared over a mass squared, with the loss; the generated pairs' tallies
/// check it).
pub const SUM_MARGIN_DEX: f64 = 0.1;

/// The relative margin on each cell's AGB figures: its loss and dust radius are raised and its
/// least mass lowered by 10% (lean, 2026-11-05, for the stars between the samples).
pub const AGB_MARGIN: f64 = 0.1;

/// The stored value of a cell's Bondi–Hoyle sum where its samples lose no wind by the bin's age.
pub const NO_WIND_CENTIDEX: i16 = i16::MIN;

/// The latest age, years, a sample is walked to if it lives longer: 10¹², far past the last bin's
/// upper age times [`AGE_SPREAD`] (1.7 × 10¹⁰), so that the step that straddles a bin's age is
/// its own.
const NEVER_DYING_HORIZON_YEARS: f64 = 1.0e12;

/// One sampled star of the table, before the cells take their largest.
#[derive(Debug, Clone, PartialEq)]
pub struct WindReachNode {
    /// Per age bin, log₁₀ of the largest Bondi–Hoyle sum over the draws (R☉² M☉⁻¹), over the
    /// walk's steps that start before the bin's upper age times [`AGE_SPREAD`]; −∞ where none
    /// loses any wind by then.
    pub log_sum_rsun2_per_msun: [f64; AGE_BINS],
    /// The earliest age, years, at which a step of any of its draws on its asymptotic giant branch
    /// starts, divided by [`AGE_SPREAD`]; +∞ if none reaches it.
    pub agb_start_years: f64,
    /// The most mass, M☉, any of its draws loses on its asymptotic giant branch; 0 if none does.
    pub agb_lost_msun: f64,
    /// The least mass, M☉, of any of its draws at the middle of a step on its asymptotic giant
    /// branch; +∞ if none reaches it.
    pub agb_least_msun: f64,
    /// The largest dust radius, R☉ (`wind_accretion::dust_radius`), of any of its draws at the middle of a step
    /// on its asymptotic giant branch; 0 if none reaches it.
    pub agb_dust_rsun: f64,
}

impl WindReachNode {
    /// A sample that loses no wind: a star on the cooling fits.
    const NONE: Self = Self {
        log_sum_rsun2_per_msun: [f64::NEG_INFINITY; AGE_BINS],
        agb_start_years: f64::INFINITY,
        agb_lost_msun: 0.0,
        agb_least_msun: f64::INFINITY,
        agb_dust_rsun: 0.0,
    };
}

/// The table's samples at initial mass `m_msun` (M☉) and metallicity coordinate `fe_h` (dex,
/// log₁₀(Z<sub>fit</sub> ÷ 0.02)): the reach table's draws ([`ETA_DRAWS`], and the
/// companion-stripped mark inside [`STRIPPED_WINDOW`] both set and not), each a full track walked
/// from age 0 to its last living age as the walk walks a donor, at [`BinaryParams::GENERATOR`].
///
/// A star below [`MIN_INITIAL_MASS`] is P06.T13's cooling fits, which the walk never feeds from:
/// no wind.
///
/// # Panics
///
/// - If `m_msun` is outside [`LOWEST_MASS_MSUN`]–[`HIGHEST_MASS_MSUN`] or `fe_h` is not finite.
/// - If a step's sum is not a number, so that the fit fails rather than write an unsound cell.
#[must_use]
pub fn wind_reach_node(m_msun: f64, fe_h: f64) -> WindReachNode {
    assert!(
        (LOWEST_MASS_MSUN..=HIGHEST_MASS_MSUN).contains(&m_msun),
        "a wind sample's mass, {m_msun} M_sun, is inside the table"
    );
    assert!(fe_h.is_finite(), "a wind sample's metallicity is finite");
    let mass = SolarMasses::new(m_msun);
    if mass < MIN_INITIAL_MASS {
        return WindReachNode::NONE;
    }
    let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
    let marks: &[Mark] = if (STRIPPED_WINDOW.0..=STRIPPED_WINDOW.1).contains(&m_msun) {
        &[NEVER_STRIPPED, Mark::from_word(0)]
    } else {
        &[NEVER_STRIPPED]
    };
    let edges: [f64; AGE_BINS] = std::array::from_fn(|k| age_edge_years(k) * AGE_SPREAD);
    let mut node = WindReachNode::NONE;
    for &mark in marks {
        for &z in &ETA_DRAWS {
            let track = Track::full(track_mass(mass), &comp, &draws_at(z, mark));
            walk_sample(&track, &edges, &mut node);
        }
    }
    node.agb_start_years /= AGE_SPREAD;
    node
}

/// Walks the sample `track` as the walk walks a donor, raising `node`'s values to its own: its
/// Bondi–Hoyle sum over the steps that start before each of `edges` (years), and its AGB figures.
fn walk_sample(track: &Track, edges: &[f64; AGE_BINS], node: &mut WindReachNode) {
    let params = BinaryParams::GENERATOR;
    let built = track.built_until().value();
    let end = track
        .lifetime()
        .map_or(built, |death| last_living(death.value()).min(built))
        .min(NEVER_DYING_HORIZON_YEARS);
    let (mut sum, mut k) = (0.0, 0);
    let mut sums = [0.0; AGE_BINS];
    let mut agb_lost = 0.0;
    for (t0, t1) in walk_steps(track, 0.0, end, STEPS_PER_INTERVAL) {
        while k < AGE_BINS && t0 >= edges[k] {
            sums[k] = sum;
            k += 1;
        }
        let lost = (track.mass_at(t0) - track.mass_at(t1)).max(0.0);
        if lost <= 0.0 {
            continue;
        }
        let state = track.state_at(Years::new(f64::midpoint(t0, t1)));
        let (m, r) = (state.mass().value(), state.radius().value());
        if !(m > 0.0 && r > 0.0) {
            continue;
        }
        let beta = params.wind_speed_factor.of(Kind::of(state.phase(), m), m);
        let focus = r / (beta * m);
        sum += lost * focus * focus;
        if on_the_agb(state.phase()) {
            node.agb_start_years = node.agb_start_years.min(t0);
            node.agb_least_msun = node.agb_least_msun.min(m);
            let dust = dust_radius(r, state.effective_temperature().value());
            node.agb_dust_rsun = node.agb_dust_rsun.max(dust);
            agb_lost += lost;
        }
    }
    assert!(!sum.is_nan(), "a wind sample's sum is a number");
    sums[k..].fill(sum);
    node.agb_lost_msun = node.agb_lost_msun.max(agb_lost);
    for (slot, &s) in node.log_sum_rsun2_per_msun.iter_mut().zip(&sums) {
        if s > 0.0 {
            *slot = slot.max(math::log10(s));
        }
    }
}

/// `log_sum_rsun2_per_msun` (dex of R☉² M☉⁻¹) plus [`SUM_MARGIN_DEX`], in hundredths of a dex rounded up, so
/// that the stored value bounds it; [`NO_WIND_CENTIDEX`] for −∞; `None` if it is +∞, not a number
/// or beyond an `i16`.
#[must_use]
pub(crate) fn to_centidex(log_sum_rsun2_per_msun: f64) -> Option<i16> {
    if log_sum_rsun2_per_msun.is_infinite() && log_sum_rsun2_per_msun.is_sign_negative() {
        return Some(NO_WIND_CENTIDEX);
    }
    let v = ((log_sum_rsun2_per_msun + SUM_MARGIN_DEX) * 100.0).ceil();
    if !v.is_finite() || v <= f64::from(NO_WIND_CENTIDEX) || v > f64::from(i16::MAX) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number inside the i16 range, checked above"
    )]
    let k = v as i16;
    Some(k)
}

/// The Bondi–Hoyle sum, R☉² M☉⁻¹, of a stored value `k`: 10^(k ÷ 100), or 0 for
/// [`NO_WIND_CENTIDEX`].
#[must_use]
pub fn centidex_to_rsun2_per_msun(k: i16) -> f64 {
    if k == NO_WIND_CENTIDEX {
        0.0
    } else {
        math::exp10(f64::from(k) / 100.0)
    }
}

/// The table's cells, as `hyperion-fit` writes them.
#[derive(Debug, Clone, PartialEq)]
pub struct WindReachCells {
    /// Per cell and age bin, the stored Bondi–Hoyle sum: its log₁₀ (R☉² M☉⁻¹) plus
    /// [`SUM_MARGIN_DEX`], in hundredths of a dex rounded up, or [`NO_WIND_CENTIDEX`].
    pub log_sum_centidex: Vec<[i16; AGE_BINS]>,
    /// Per cell, the earliest age, years, at which a star of it may be on its asymptotic giant
    /// branch; +∞ for none.
    pub agb_start_years: Vec<f64>,
    /// Per cell, the most mass, M☉, a star of it may lose on its asymptotic giant branch.
    pub agb_lost_msun: Vec<f64>,
    /// Per cell, the least mass, M☉, of a star of it on its asymptotic giant branch; +∞ for none.
    pub agb_least_msun: Vec<f64>,
    /// Per cell, the largest dust radius, R☉, of a star of it on its asymptotic giant branch.
    pub agb_dust_rsun: Vec<f64>,
}

/// Why [`assemble`] refused its samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AssembleWindReachError {
    /// The samples are not the grid's: (`MASS_CELLS` × `MASS_SUBDIVISIONS` + 1) ×
    /// (`FE_H_CELLS` × `FE_H_SUBDIVISIONS` + 1) of them, in mass-major order.
    WrongShape {
        /// The samples the grid holds.
        expected: usize,
        /// The samples given.
        found: usize,
    },
    /// A sample holds a sum of +∞ or not a number, an AGB start that is not a number, or AGB
    /// figures that are negative or not numbers.
    UnsoundSample {
        /// The sample's index.
        sample: usize,
    },
    /// A cell's sum is beyond what an `i16` stores.
    UnstorableCell {
        /// The cell's index.
        cell: usize,
    },
}

impl fmt::Display for AssembleWindReachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongShape { expected, found } => write!(
                f,
                "the wind table's grid holds {expected} samples, not {found}"
            ),
            Self::UnsoundSample { sample } => {
                write!(f, "the wind table's sample {sample} is not sound")
            }
            Self::UnstorableCell { cell } => {
                write!(f, "the wind table's cell {cell} cannot be stored")
            }
        }
    }
}

impl Error for AssembleWindReachError {}

/// The table's cells from its `nodes`, mass-major ([`MASS_CELLS`] × [`MASS_SUBDIVISIONS`] + 1
/// masses, each at [`FE_H_CELLS`] × [`FE_H_SUBDIVISIONS`] + 1 metallicities): each cell's
/// largest sum over its samples, edges included, made non-decreasing in age and stored with its
/// margin, and its widened AGB figures.
///
/// # Errors
///
/// - [`AssembleWindReachError::WrongShape`] if `nodes` is not the grid's;
/// - [`AssembleWindReachError::UnsoundSample`] if a sample is not sound;
/// - [`AssembleWindReachError::UnstorableCell`] if a cell's sum is beyond an `i16`.
pub fn assemble(nodes: &[WindReachNode]) -> Result<WindReachCells, AssembleWindReachError> {
    let per_mass = FE_H_CELLS * FE_H_SUBDIVISIONS + 1;
    let expected = (MASS_CELLS * MASS_SUBDIVISIONS + 1) * per_mass;
    if nodes.len() != expected {
        return Err(AssembleWindReachError::WrongShape {
            expected,
            found: nodes.len(),
        });
    }
    let sound = |v: f64| v >= 0.0;
    if let Some(sample) = nodes.iter().position(|node| {
        node.log_sum_rsun2_per_msun
            .iter()
            .any(|&v| v.is_nan() || (v.is_infinite() && v.is_sign_positive()))
            || node.agb_start_years.is_nan()
            || !sound(node.agb_lost_msun)
            || !sound(node.agb_least_msun)
            || !sound(node.agb_dust_rsun)
    }) {
        return Err(AssembleWindReachError::UnsoundSample { sample });
    }
    let mut cells = WindReachCells {
        log_sum_centidex: Vec::with_capacity(CELLS),
        agb_start_years: Vec::with_capacity(CELLS),
        agb_lost_msun: Vec::with_capacity(CELLS),
        agb_least_msun: Vec::with_capacity(CELLS),
        agb_dust_rsun: Vec::with_capacity(CELLS),
    };
    for k in 0..MASS_CELLS {
        for j in 0..FE_H_CELLS {
            let mut cell = WindReachNode::NONE;
            for mass in k * MASS_SUBDIVISIONS..=(k + 1) * MASS_SUBDIVISIONS {
                for fe_h in j * FE_H_SUBDIVISIONS..=(j + 1) * FE_H_SUBDIVISIONS {
                    let node = &nodes[mass * per_mass + fe_h];
                    for (slot, &v) in cell
                        .log_sum_rsun2_per_msun
                        .iter_mut()
                        .zip(&node.log_sum_rsun2_per_msun)
                    {
                        *slot = slot.max(v);
                    }
                    cell.agb_start_years = cell.agb_start_years.min(node.agb_start_years);
                    cell.agb_lost_msun = cell.agb_lost_msun.max(node.agb_lost_msun);
                    cell.agb_least_msun = cell.agb_least_msun.min(node.agb_least_msun);
                    cell.agb_dust_rsun = cell.agb_dust_rsun.max(node.agb_dust_rsun);
                }
            }
            // Each sample's sum never falls with age, but the largest of them is made so anyway.
            let mut so_far = f64::NEG_INFINITY;
            let mut row = [NO_WIND_CENTIDEX; AGE_BINS];
            for (stored, &v) in row.iter_mut().zip(&cell.log_sum_rsun2_per_msun) {
                so_far = so_far.max(v);
                *stored = to_centidex(so_far).ok_or(AssembleWindReachError::UnstorableCell {
                    cell: k * FE_H_CELLS + j,
                })?;
            }
            cells.log_sum_centidex.push(row);
            cells.agb_start_years.push(cell.agb_start_years);
            cells
                .agb_lost_msun
                .push(cell.agb_lost_msun * (1.0 + AGB_MARGIN));
            cells
                .agb_least_msun
                .push(cell.agb_least_msun / (1.0 + AGB_MARGIN));
            cells
                .agb_dust_rsun
                .push(cell.agb_dust_rsun * (1.0 + AGB_MARGIN));
        }
    }
    Ok(cells)
}

/// What the table bounds of the wind the stars of a mass interval lose by an age.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WindReach {
    /// Their largest Bondi–Hoyle sum by the age, R☉² M☉⁻¹, over the steps that start before it.
    sum_rsun2_per_msun: f64,
    /// Their AGB figures, if any of them may be on its asymptotic giant branch by the age.
    agb: Option<AgbWind>,
}

/// The AGB figures of a [`WindReach`]: the most AGB loss (M☉), the least AGB mass (M☉) and the
/// largest dust radius (R☉).
#[derive(Debug, Clone, Copy, PartialEq)]
struct AgbWind {
    lost_msun: f64,
    least_msun: f64,
    dust_rsun: f64,
}

impl WindReach {
    /// At most what a star of at most `accretor_msun` M☉, on its own track at periastra of at
    /// least `periastron_rsun` R☉, takes of the wind these stars lose by the age, M☉: the
    /// Bondi–Hoyle share's bound and, beside a donor that may be on its AGB, the overflow's (the
    /// module's "The screen"), with `α_W` `bondi_hoyle`.
    #[must_use]
    fn most_taken_msun(&self, bondi_hoyle: f64, accretor_msun: f64, periastron_rsun: f64) -> f64 {
        let (m_a, r_p) = (accretor_msun, periastron_rsun);
        let bondi_hoyle = bondi_hoyle * m_a * m_a * self.sum_rsun2_per_msun / (8.0 * r_p * r_p);
        let overflow = self.agb.map_or(0.0, |agb| {
            let lobe = r_p * lobe_fraction(agb.least_msun / m_a);
            agb.lost_msun * wind_roche_lobe_efficiency(m_a / agb.least_msun, agb.dust_rsun / lobe)
        });
        bondi_hoyle + overflow
    }
}

/// The table's reader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WindReachTable<'t> {
    log_sum_centidex: &'t [[i16; AGE_BINS]],
    agb_start_years: &'t [f64],
    agb_lost_msun: &'t [f64],
    agb_least_msun: &'t [f64],
    agb_dust_rsun: &'t [f64],
}

impl WindReachTable<'static> {
    /// The generator's table, [`tables::binary_wind_reach`](crate::tables::binary_wind_reach).
    #[must_use]
    pub(crate) const fn generator() -> Self {
        Self {
            log_sum_centidex: &binary_wind_reach::LOG_SUM_CENTIDEX,
            agb_start_years: &binary_wind_reach::AGB_START_YEARS,
            agb_lost_msun: &binary_wind_reach::AGB_LOST_MSUN,
            agb_least_msun: &binary_wind_reach::AGB_LEAST_MSUN,
            agb_dust_rsun: &binary_wind_reach::AGB_DUST_RSUN,
        }
    }
}

#[cfg(test)]
impl<'t> WindReachTable<'t> {
    /// A reader over cells written by [`assemble`].
    #[must_use]
    pub(crate) fn of(cells: &'t WindReachCells) -> Self {
        Self {
            log_sum_centidex: &cells.log_sum_centidex,
            agb_start_years: &cells.agb_start_years,
            agb_lost_msun: &cells.agb_lost_msun,
            agb_least_msun: &cells.agb_least_msun,
            agb_dust_rsun: &cells.agb_dust_rsun,
        }
    }
}

impl WindReachTable<'_> {
    /// The wind's reach of the stars of initial mass `lo_msun`–`hi_msun` (M☉) and metallicity
    /// coordinate `fe_h` (`reach::fe_h_of`) by the age `age_years`, or `None` where the table does
    /// not reach: a mass below [`LOWEST_MASS_MSUN`] or not finite, an age past the last bin, or a
    /// table of the wrong shape. A mass above [`HIGHEST_MASS_MSUN`] reads it, as its track does.
    #[must_use]
    pub(crate) fn reach(
        &self,
        lo_msun: f64,
        hi_msun: f64,
        fe_h: f64,
        age_years: f64,
    ) -> Option<WindReach> {
        if [
            self.log_sum_centidex.len(),
            self.agb_start_years.len(),
            self.agb_lost_msun.len(),
            self.agb_least_msun.len(),
            self.agb_dust_rsun.len(),
        ]
        .iter()
        .any(|&n| n != CELLS)
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
        let mut largest = NO_WIND_CENTIDEX;
        let mut agb: Option<AgbWind> = None;
        for k in first..=last {
            let cell = k * FE_H_CELLS + j;
            largest = largest.max(self.log_sum_centidex[cell][bin]);
            if age_years >= self.agb_start_years[cell] {
                let this = AgbWind {
                    lost_msun: self.agb_lost_msun[cell],
                    least_msun: self.agb_least_msun[cell],
                    dust_rsun: self.agb_dust_rsun[cell],
                };
                agb = Some(agb.map_or(this, |a| AgbWind {
                    lost_msun: a.lost_msun.max(this.lost_msun),
                    least_msun: a.least_msun.min(this.least_msun),
                    dust_rsun: a.dust_rsun.max(this.dust_rsun),
                }));
            }
        }
        Some(WindReach {
            sum_rsun2_per_msun: centidex_to_rsun2_per_msun(largest),
            agb,
        })
    }
}

/// Whether a star of the pairs of initial masses `ends` (M☉, each `[lo, hi]`, the lighter's `lo`
/// first) at metallicity coordinate `fe_h`, on any orbit of periastron at least `periastron_rsun`
/// (R☉), may take [`LEAST_ACCRETED_MSUN`] of its partner's wind by `until_years`, each star the
/// accretor in turn (the module's "The screen"): `true` too where the table does not reach.
///
/// `false` where the lighter star is below 0.1 M☉ at every mass of its interval: it is P06.T13's
/// cooling fits, and the walk feeds no pair that holds one.
#[must_use]
pub(crate) fn may_take_wind(
    ends: [[f64; 2]; 2],
    periastron_rsun: f64,
    fe_h: f64,
    until_years: f64,
) -> bool {
    if ends[0][1] < MIN_INITIAL_MASS.value() {
        return false;
    }
    let table = WindReachTable::generator();
    let (Some(lighter), Some(heavier)) = (
        table.reach(ends[0][0], ends[0][1], fe_h, until_years),
        table.reach(ends[1][0], ends[1][1], fe_h, until_years),
    ) else {
        return true;
    };
    let bondi_hoyle = BinaryParams::GENERATOR.bondi_hoyle;
    let takes = |donor: &WindReach, accretor_msun: f64| {
        donor.most_taken_msun(bondi_hoyle, accretor_msun, periastron_rsun) >= LEAST_ACCRETED_MSUN
    };
    takes(&lighter, ends[1][1]) || takes(&heavier, ends[0][1])
}

#[cfg(test)]
mod tests;
