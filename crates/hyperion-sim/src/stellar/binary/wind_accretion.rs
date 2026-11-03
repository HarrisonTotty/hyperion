//! A wide pair's wind accretion (plan 11, P11.T12's fix).
//!
//! As first built, a pair whose stars never reach their Roche lobes, on the drawn orbit or after
//! the engine's sinks have shrunk it, was passed over by the pre-test ([`can_interact`]) as two
//! single stars on their orbit, plan 06's own. So the Bondi–Hoyle accretion of Hurley, Tout and
//! Pols (2002, MNRAS 329, 897, "BSE", equations 6–10), which the engine takes in `detached.rs`,
//! never fed the wide pairs whose wind transfer makes the field's long-period blue stragglers (P of
//! 10²–10⁴ d, low e, white-dwarf companions: Preston and Sneden 2000, AJ 120, 1014; Carney et al.
//! 2005, AJ 129, 466; recalled, not read in the pod). research-leans-29 found, under every 27 hold,
//! 0.215 blue stragglers per horizontal-branch star in the halo against the halo field's about 4,
//! and 0.145 in the bulge against 0.29–0.37 (Clarkson et al. 2011, ApJ 735, 37; recalled). Of the
//! halo's companions within 0.25 M☉ under the turn-off beside an evolved primary, about 12.5 per
//! horizontal-branch star stayed detached at P ≥ 10³ d, every one with no mass accreted.
//!
//! So ([`WindAccretionRules::WindRocheLobe`]) a pair the lobe and decay tests pass over is walked
//! over its partner's own track: a hydrogen main-sequence star of at least 0.1 M☉ on its own
//! track takes BSE's equation-6 share of its partner's wind ([`bondi_hoyle_share`], the engine's own
//! expression), with equation 9's wind speed and at most equation 10's 0.8, or, beside a partner on
//! its asymptotic giant branch, the larger share of wind Roche-lobe overflow
//! ([`wind_roche_lobe_share`], Abate et al. 2013), step by step between
//! the knots at which the partner's track integrates its mass (`Track::mass_breaks`, cut into
//! [`STEPS_PER_INTERVAL`]), at the orbit the Jeans mode widens (a ∝ 1 ÷ M, the drawn eccentricity
//! kept). Where what it takes reaches [`LEAST_ACCRETED_MSUN`] the pair is no longer two single
//! stars: [`can_interact`] says so, and [`evolve`](super::evolve) builds the pair's timeline here
//! ([`wind_fed_timeline`]) rather than on the engine, in three detached segments:
//!
//! 1. both stars their own, to the start of the step in which what the star takes reaches the
//!    threshold (what it took before that is left out, under 10⁻⁴ M☉);
//! 2. the accretor a main sequence whose mass and τ the binary carries, its mass grown by its share
//!    of each step and its τ rejuvenated as the engine rejuvenates an accretor
//!    ([`rejuvenated_tau_unheld`]), to the end of its main sequence;
//! 3. from there, the accretor on a track of its mass, placed at its Hertzsprung gap's start as the
//!    engine places a carried main sequence that ends (`Engine::after_boundary`).
//!
//! Wind Roche-lobe overflow (Mohamed and Podsiadlowski 2007, ASP Conf. Ser. 372, 397; Abate et al.
//! 2013, A&A 552, A26): an AGB star's slow wind is accelerated by the dust that condenses in it, and
//! where the dust forms near the star's Roche lobe the wind fills the lobe and flows through its
//! inner point, so the companion takes far more of it than the Bondi–Hoyle share. research-leans-30
//! (§2) chose it over engine-side wind as the first lever on the halo's blue stragglers.
//!
//! The donor keeps its own track throughout, and the orbit shown is the drawn one, as for every
//! pair passed over: the widening enters the share alone. Leans of 2026-11-01, recorded in plan 11:
//! a star accretes only on its main sequence, so neither a remnant (the symbiotic stars are
//! classified from the state, as before) nor a giant nor a star below 0.1 M☉ takes a wind here; and
//! the accretor's later radius on its new track is not tested against its lobe.
//!
//! Its one reading is [`onset`], which [`can_interact`] and `evolve`'s pass-over read. The pair's
//! past does not depend on the age it is run to: the steps
//! are the partner's track's own, the onset is read at the start of a step, and the last step taken
//! is the one that reaches or passes the age, as the engine's is (`detached::StepLimit`).
//!
//! [`can_interact`]: super::can_interact
//! [`rejuvenated_tau_unheld`]: super::rlof::rejuvenated_tau_unheld

use std::sync::Arc;

use crate::planetary::context::UNIVERSE_AGE;
use crate::stellar::Phase;
use crate::stellar::sse::{self, Track};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{SolarMasses, Years};

use super::BinaryParams;
use super::detached::last_living;
use super::evolve::{
    drawn_orbit_path, offset_for, reach_margin_years, roche_lobe, runs_or_collapses_by, track_mass,
    track_reaching,
};
use super::rlof::rejuvenated_tau_unheld;
use super::star::{G, Kind, Member, Path};
use super::timeline::{BinaryInput, BinaryTimeline, Context, Segment, SegmentKind};

/// Which share of its partner's wind a passed-over pair's main-sequence star takes: the input's
/// arm (`BinaryInput::wind`), 27's in every generated pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum WindAccretionRules {
    /// Its main-sequence star takes its Bondi–Hoyle share of its partner's wind alone, and from
    /// [`LEAST_ACCRETED_MSUN`] the pair is wind-fed: the arm as first built, which no pair takes
    /// but in the tests that hold the closed form against the engine's own Bondi–Hoyle wind.
    #[cfg(test)]
    BondiHoyle,
    /// Version 27's: as [`WindAccretionRules::BondiHoyle`], with the larger share of wind Roche-lobe
    /// overflow beside a partner on its asymptotic giant branch ([`wind_roche_lobe_share`]).
    WindRocheLobe,
}

impl WindAccretionRules {
    /// Whether an AGB partner's wind overflows to its companion through its Roche lobe under this
    /// arm ([`wind_roche_lobe_share`]).
    #[must_use]
    const fn overflows(self) -> bool {
        match self {
            #[cfg(test)]
            Self::BondiHoyle => false,
            Self::WindRocheLobe => true,
        }
    }
}

/// The least mass, M☉, a star of a pair the lobe and decay tests pass over must take of its
/// partner's wind for the pair to be no longer two single stars on their orbit: 10⁻⁴ (the brief's
/// lean of 2026-11-01), under which the pair stays plan 06's own, and the cheap path cheap.
pub(crate) const LEAST_ACCRETED_MSUN: f64 = 1.0e-4;

/// The latest age, years, to which a wind-fed pair is held to be passed over by the lobe and decay
/// tests with no star of it dying suddenly, so that the engine runs it at no age and its past is
/// the same in a run to any age up to it: the universe's, [`UNIVERSE_AGE`],
/// 13.787 Gyr, past every system's (the halo's 13 Gyr at most, `galaxy::ages::HALO_AGES`).
const WIND_FED_HORIZON_YEARS: f64 = UNIVERSE_AGE.value();

/// The `helium` flag of a hydrogen main sequence, the only one that takes a wind here: an accretor
/// on its own track is a hydrogen main-sequence star ([`Walk::onset`]).
const HYDROGEN: bool = false;

/// The midpoint steps the walk takes between two of the partner's mass knots: the share is read
/// at each step's middle, and the mass lost over it exactly. Two hold the accreted mass to 0.3%
/// of eight's on the halo's 0.85 + 0.80 M☉ pair at 3,000 d (`the_walk_has_converged`).
pub(crate) const STEPS_PER_INTERVAL: u32 = 2;

/// The condensation temperature of the dust that drives an AGB star's wind, K: 1,500 K for
/// carbon-rich dust (Höfner 2007, ASP Conf. Ser. 378, 145), as Abate et al. (2013, A&A 552, A26,
/// section 2) take it (both recalled, not read in the pod). They take it for every AGB donor, as
/// here, though a low-mass metal-poor donor's dredge-up may not lift its C/O above 1.
pub(crate) const DUST_CONDENSATION_K: f64 = 1_500.0;

/// The coefficients c₁, c₂ and c₃ of Abate et al. (2013, A&A 552, A26, equation 9)'s quadratic
/// in `R_d` ÷ `R_L`, their fit to the accretion efficiencies of Mohamed and Podsiadlowski's (2007,
/// 2012) hydrodynamical runs (recalled, not read in the pod).
pub(crate) const WIND_ROCHE_LOBE_COEFFICIENTS: [f64; 3] = [-0.284, 0.918, -0.234];

/// The largest share of its partner's wind a star takes by wind Roche-lobe overflow: Abate et al.
/// (2013, A&A 552, A26)'s `β_acc,max`, 0.5 (recalled, not read in the pod).
pub(crate) const WIND_ROCHE_LOBE_MAX_SHARE: f64 = 0.5;

/// The ratio x = `R_d` ÷ `R_L` at which [`wind_roche_lobe_efficiency`]'s quadratic peaks, −c₂ ÷ 2c₁
/// (about 1.62), past which the efficiency is held at its peak.
const WIND_ROCHE_LOBE_PEAK_X: f64 =
    -WIND_ROCHE_LOBE_COEFFICIENTS[1] / (2.0 * WIND_ROCHE_LOBE_COEFFICIENTS[0]);

/// The radius at which the dust of the wind of a star of radius `r` (any unit) and effective
/// temperature `t_eff_k` (K) condenses, in `r`'s unit: Höfner (2007, ASP Conf. Ser. 378, 145)'s
/// `R_d` = ½ R (`T_eff` ÷ `T_cond`)^((4 + p) ÷ 2), with p = 1 for carbon-rich dust, as Abate et al.
/// (2013, A&A 552, A26, section 2) take it (recalled, not read in the pod), at
/// [`DUST_CONDENSATION_K`].
#[must_use]
pub(crate) fn dust_radius(r: f64, t_eff_k: f64) -> f64 {
    let y = (t_eff_k / DUST_CONDENSATION_K).max(0.0);
    // (T ÷ T_cond)^2.5 as y² √y: products and a square root alone, so the bits are every target's.
    0.5 * r * (y * y * y.sqrt())
}

/// The share of an AGB star's wind its companion takes by wind Roche-lobe overflow, for a mass
/// ratio `q` = `M_accretor` ÷ `M_donor` and a ratio `x` of the donor's dust radius to its Roche lobe:
/// Abate et al. (2013, A&A 552, A26, equation 9)'s (25 ÷ 9) q² (c₁ x² + c₂ x + c₃), normalised to
/// Mohamed and Podsiadlowski's q of 0.6, at most [`WIND_ROCHE_LOBE_MAX_SHARE`] and at least 0
/// (the quadratic falls below 0 for x under about 0.28, where Bondi–Hoyle's share is the one).
///
/// Lean (2026-11-02): past the quadratic's peak, x ≈ 1.62 ([`WIND_ROCHE_LOBE_PEAK_X`]), the
/// efficiency is held at the peak's. The fit falls again beyond it, to 0 at x ≈ 2.95, past the runs
/// it fits, which would give a closer pair less of the wind than a wider one. Beside a donor
/// heavier than about 0.6 of its partner's mass the cap, not the fit, sets the share near the lobe:
/// the q² law is the fit's, carried past the one q its runs took.
#[must_use]
pub(crate) fn wind_roche_lobe_efficiency(q: f64, x: f64) -> f64 {
    let [c1, c2, c3] = WIND_ROCHE_LOBE_COEFFICIENTS;
    let x = x.min(WIND_ROCHE_LOBE_PEAK_X);
    let fit = 25.0 / 9.0 * q * q * ((c1 * x + c2) * x + c3);
    fit.clamp(0.0, WIND_ROCHE_LOBE_MAX_SHARE)
}

/// The share of its partner's wind a star of `m_accretor` M☉ takes by wind Roche-lobe overflow
/// from a partner on its asymptotic giant branch of `m_donor` M☉, radius `r_donor` (R☉) and
/// effective temperature `t_eff_k` (K), on an orbit of semi-major axis `a` (R☉): its
/// [`wind_roche_lobe_efficiency`] at the donor's [`dust_radius`] over its Roche lobe at `a`
/// (Eggleton 1983). Lean (2026-11-02): the lobe at the semi-major axis, as for the circular orbits
/// Abate et al. (2013) ran, whatever the eccentricity.
#[must_use]
fn wind_roche_lobe_share(m_donor: f64, r_donor: f64, t_eff_k: f64, m_accretor: f64, a: f64) -> f64 {
    let x = dust_radius(r_donor, t_eff_k) / roche_lobe(m_donor, m_accretor, a);
    wind_roche_lobe_efficiency(m_accretor / m_donor, x)
}

/// Whether a star in `phase` loses an AGB star's dust-driven wind: the early and the thermally
/// pulsing asymptotic giant branch (HPT types 5 and 6).
#[must_use]
const fn on_the_agb(phase: Phase) -> bool {
    matches!(phase, Phase::EarlyAgb | Phase::ThermallyPulsingAgb)
}

/// The share of a wind-losing star's wind that its companion accretes: BSE equation 6, with
/// equation 9's wind speed `v_W²` = 2 `β_W` G `M_d` ÷ `R_d`, at most equation 10's
/// [`max_wind_accretion`](BinaryParams::max_wind_accretion), the engine's own expression
/// (`Engine::rates`). The donor is of type `kind`, mass `m_donor` (M☉) and radius `r_donor` (R☉),
/// the accretor of `m_accretor` M☉, on an orbit of semi-major axis `a` (R☉) and eccentricity `e`.
#[must_use]
fn bondi_hoyle_share(
    params: &BinaryParams,
    kind: Kind,
    m_donor: f64,
    r_donor: f64,
    m_accretor: f64,
    a: f64,
    e: f64,
) -> f64 {
    let beta = params.wind_speed_factor.of(kind, m_donor);
    let v_wind2 = 2.0 * beta * G * m_donor / r_donor.max(1e-6);
    let v_orb2 = G * (m_donor + m_accretor) / a;
    let ratio = {
        let x = 1.0 + v_orb2 / v_wind2;
        x * x.sqrt()
    };
    let focus = G * m_accretor / v_wind2;
    let share = params.bondi_hoyle * focus * focus / (2.0 * a * a) / ratio / (1.0 - e * e).sqrt();
    share.min(params.max_wind_accretion)
}

/// Where a wide pair becomes wind-fed ([`onset`]): its accretor, the age at which the accretor
/// becomes a carried main sequence, years, and the walk over its partner's track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Onset {
    accretor: usize,
    age: f64,
    /// The walk's first and last ages, years: the accretor's arrival on its main sequence, and the
    /// partner's last living age or the end of its build.
    start: f64,
    end: f64,
}

/// The walk of one star of a pair over its partner's own track.
struct Walk<'a> {
    params: &'a BinaryParams,
    donor: &'a Track,
    accretor: &'a Track,
    /// The drawn orbit's semi-major axis, R☉, eccentricity and total mass, M☉.
    a0: f64,
    e: f64,
    total0: f64,
    /// The steps between two of the partner's mass knots, [`STEPS_PER_INTERVAL`] but in tests.
    steps_per_interval: u32,
    /// Whether an AGB partner's wind overflows its Roche lobe ([`WindAccretionRules::overflows`]).
    overflows: bool,
}

impl<'a> Walk<'a> {
    /// The walk of `members[accretor]` over its partner's track, if both are on their own tracks
    /// (the members a pair passed over holds, at offset 0).
    #[must_use]
    fn of(input: &'a BinaryInput, members: &'a [Member; 2], accretor: usize) -> Option<Self> {
        let (Member::Track { track: own, .. }, Member::Track { track: donor, .. }) =
            (&members[accretor], &members[1 - accretor])
        else {
            return None;
        };
        let orbit = input.orbit();
        let [m1, m2] = input.masses().map(SolarMasses::value);
        Some(Self {
            params: input.params(),
            donor,
            accretor: own,
            a0: orbit.semi_major_axis().value() / SOLAR_RADIUS_M,
            e: orbit.eccentricity().value(),
            total0: m1 + m2,
            steps_per_interval: STEPS_PER_INTERVAL,
            overflows: input.wind().overflows(),
        })
    }

    /// The walk's first and last ages, years, if the accretor has arrived on its main sequence: from
    /// that arrival to the partner's last living age, or to the end of its build if it lives past
    /// it. A death's own loss (a supernova's ejecta, a white dwarf's hand-over) is no wind.
    #[must_use]
    fn span(&self) -> Option<(f64, f64)> {
        let start = self.accretor.main_sequence_arrival()?.value();
        let built = self.donor.built_until().value();
        let end = self
            .donor
            .lifetime()
            .map_or(built, |death| last_living(death.value()).min(built));
        (end > start).then_some((start, end))
    }

    /// Whether the accretor surely takes under [`LEAST_ACCRETED_MSUN`] of its partner's wind from
    /// `start` to `end`, the screen before the walk: its [`Walk::bondi_hoyle_bound`] is under it,
    /// and so is that and its [`Walk::overflow_bound`] where the partner's wind overflows (read
    /// only then, as it reads the partner's AGB step by step).
    #[must_use]
    fn takes_too_little(&self, start: f64, end: f64) -> bool {
        let bondi_hoyle = self.bondi_hoyle_bound(end);
        bondi_hoyle < LEAST_ACCRETED_MSUN
            && (!self.overflows
                || bondi_hoyle + self.overflow_bound(start, end) < LEAST_ACCRETED_MSUN)
    }

    /// What the partner loses by `end`, M☉, and the most the accretor's mass can be by then: its
    /// initial mass and the largest share of every wind, BSE's 0.8, of that loss.
    #[must_use]
    fn lost_and_most(&self, end: f64) -> (f64, f64) {
        let lost = (self.donor.initial_mass().value() - self.donor.mass_at(end)).max(0.0);
        let most = self.accretor.initial_mass().value() + self.params.max_wind_accretion * lost;
        (lost, most)
    }

    /// At most what the accretor takes of its partner's wind by `end` of the Bondi–Hoyle share,
    /// M☉: the share at its largest, every factor at its bound (the partner's largest radius and
    /// least mass, the accretor at its most, the drawn orbit, the slowest wind, no orbital speed),
    /// times all the partner loses.
    #[must_use]
    fn bondi_hoyle_bound(&self, end: f64) -> f64 {
        let (lost, most) = self.lost_and_most(end);
        if lost <= 0.0 {
            return 0.0;
        }
        let least = self.donor.mass_at(end);
        let radius = self.donor.max_radius_until(Years::new(end)).value();
        // G M_a ÷ v_W² = M_a R ÷ (2 β_W M_d).
        let focus = most * radius / (2.0 * self.params.wind_speed_factor.least() * least);
        let share = self.params.bondi_hoyle * focus * focus
            / (2.0 * self.a0 * self.a0)
            / (1.0 - self.e * self.e).sqrt();
        share.min(self.params.max_wind_accretion) * lost
    }

    /// At most what the accretor takes of its partner's wind from `start` to `end` by wind
    /// Roche-lobe overflow, M☉: over the walk's own steps on the partner's asymptotic giant branch,
    /// each read where [`Walk::taken`] reads it, the overflow's share with the accretor at its most
    /// ([`Walk::lost_and_most`]) and the orbit the drawn one (the Jeans mode only widens it), times
    /// what the partner loses over the step: the efficiency never falls as q or x grows. With
    /// [`Walk::bondi_hoyle_bound`] it bounds the walk, which takes the larger of the two shares.
    #[must_use]
    fn overflow_bound(&self, start: f64, end: f64) -> f64 {
        let agb = [Phase::EarlyAgb, Phase::ThermallyPulsingAgb]
            .into_iter()
            .filter_map(|phase| self.donor.age_in_phase(phase, 0.0))
            .reduce(f64::min);
        let Some(agb) = agb.filter(|&age| age < end) else {
            return 0.0;
        };
        let (_, most) = self.lost_and_most(end);
        self.steps(start, end)
            .skip_while(|&(_, t1)| t1 <= agb)
            .map(|(t0, t1)| {
                let lost = (self.donor.mass_at(t0) - self.donor.mass_at(t1)).max(0.0);
                if lost <= 0.0 {
                    return 0.0;
                }
                let state = self.donor.state_at(Years::new(f64::midpoint(t0, t1)));
                let (m_donor, r_donor) = (state.mass().value(), state.radius().value());
                if !(on_the_agb(state.phase()) && m_donor > 0.0 && r_donor > 0.0) {
                    return 0.0;
                }
                let dust = dust_radius(r_donor, state.effective_temperature().value());
                let x = dust / roche_lobe(m_donor, most, self.a0);
                lost * wind_roche_lobe_efficiency(most / m_donor, x)
            })
            .sum()
    }

    /// The walk's steps from `start` to `end`, years, in age order: the spans between the
    /// partner's mass knots and phase boundaries, each cut into `steps_per_interval` equal steps.
    fn steps(&self, start: f64, end: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
        let n = self.steps_per_interval;
        let mut nodes = std::iter::once(start)
            .chain(self.donor.mass_breaks(start, end))
            .chain(std::iter::once(end));
        let first = nodes.next();
        nodes
            .scan(first, |previous, node| {
                let from = previous.replace(node)?;
                Some((from, node))
            })
            .filter(|&(from, to)| to > from)
            .flat_map(move |(from, to)| {
                let at = move |k: u32| {
                    if k == n {
                        to
                    } else {
                        from + (to - from) * f64::from(k) / f64::from(n)
                    }
                };
                (0..n).map(move |k| (at(k), at(k + 1)))
            })
    }

    /// What an accretor of `m_accretor` M☉ takes of the partner's wind over the step from `t0` to
    /// `t1`, M☉: the share at the step's middle times the mass the partner loses over it, the
    /// Bondi–Hoyle share or, from an AGB partner whose wind overflows, the overflow's if larger
    /// (Abate et al. 2013, section 2.2: the larger of the two, recalled).
    #[must_use]
    fn taken(&self, t0: f64, t1: f64, m_accretor: f64) -> f64 {
        let lost = (self.donor.mass_at(t0) - self.donor.mass_at(t1)).max(0.0);
        if lost <= 0.0 {
            return 0.0;
        }
        let state = self.donor.state_at(Years::new(f64::midpoint(t0, t1)));
        let (m_donor, r_donor) = (state.mass().value(), state.radius().value());
        if !(m_donor > 0.0 && r_donor > 0.0) {
            return 0.0;
        }
        let a = self.a0 * self.total0 / (m_donor + m_accretor);
        let kind = Kind::of(state.phase(), m_donor);
        let share = bondi_hoyle_share(self.params, kind, m_donor, r_donor, m_accretor, a, self.e);
        if self.overflows && on_the_agb(state.phase()) {
            let t_eff = state.effective_temperature().value();
            lost * share.max(wind_roche_lobe_share(
                m_donor, r_donor, t_eff, m_accretor, a,
            ))
        } else {
            lost * share
        }
    }

    /// The first step's start at which what the accretor, on its own track, has taken by that
    /// step's end reaches [`LEAST_ACCRETED_MSUN`], if one starts before `until` years while the
    /// accretor is still on its main sequence.
    #[must_use]
    fn onset(&self, start: f64, end: f64, until: f64) -> Option<f64> {
        let mut taken = 0.0;
        for (t0, t1) in self.steps(start, end) {
            if t0 >= until {
                return None;
            }
            self.accretor.main_sequence_fraction(t0)?;
            taken += self.taken(t0, t1, self.accretor.mass_at(t0));
            if taken >= LEAST_ACCRETED_MSUN {
                return Some(t0);
            }
        }
        None
    }
}

/// Where the pair of `input`, whose own `members` the lobe and decay tests pass over by `until`
/// years, becomes wind-fed under the input's arm ([`WindAccretionRules`]), if it does by then:
/// the earlier of its two stars' onsets, the primary's first.
///
/// None for a pair that its lobe and decay tests send to the engine at some later age up to
/// [`WIND_FED_HORIZON_YEARS`], or a star of which dies suddenly by then ([`runs_or_collapses_by`]),
/// which stays its own models here. Lean (2026-11-02): the engine gives a star on its own track no
/// wind (`detached.rs`), so such a pair fed here would show a past that depends on the age it is
/// run to; and the closed form records no supernova, whether P11.T10's fix runs the pair or not.
#[must_use]
pub(super) fn onset(input: &BinaryInput, members: &[Member; 2], until: f64) -> Option<Onset> {
    (0..2)
        .filter_map(|accretor| {
            let walk = Walk::of(input, members, accretor)?;
            let (start, end) = walk.span()?;
            if walk.takes_too_little(start, end) {
                return None;
            }
            let age = walk.onset(start, end, until)?;
            Some(Onset {
                accretor,
                age,
                start,
                end,
            })
        })
        .min_by(|a, b| a.age.total_cmp(&b.age))
        .filter(|_| !runs_or_collapses_by(input, Years::new(until.max(WIND_FED_HORIZON_YEARS))))
}

/// The timeline of the wind-fed pair of `input`, its own `members` run to `until` years, from its
/// `onset` (see the module documentation): both stars their own, then the accretor a carried main
/// sequence fed its share of each step, then the accretor on a track of its mass from its main
/// sequence's end; the partner its own, and the orbit the drawn one, throughout.
///
/// # Panics
///
/// If `onset` is not [`onset`]'s for these `members`: its accretor and partner must be on their own
/// tracks, the accretor on its main sequence at the onset's age.
#[must_use]
pub(super) fn wind_fed_timeline(
    input: &BinaryInput,
    members: [Member; 2],
    until: f64,
    ctx: Arc<Context>,
    onset: Onset,
) -> BinaryTimeline {
    let Onset {
        accretor: i,
        age: t_on,
        start,
        end,
    } = onset;
    let walk = Walk::of(input, &members, i).expect("the onset's members are on their own tracks");
    let mut m = walk.accretor.mass_at(t_on);
    let mut tau = walk
        .accretor
        .main_sequence_fraction(t_on)
        .expect("the accretor is on its main sequence at its onset");
    let (mut mass, mut fractions) = (Path::starting(t_on, m), Path::starting(t_on, tau));
    // Where the accretor's main sequence ends, if the walk reaches it: in a step, or after the
    // partner's death at its mass then.
    let mut main_sequence_end = None;
    let mut last = t_on;
    for (t0, t1) in walk.steps(start, end).skip_while(|&(t0, _)| t0 < t_on) {
        if t0 >= until {
            break;
        }
        let m1 = m + walk.taken(t0, t1, m);
        let tau1 = rejuvenated_tau_unheld(ctx.coeffs(), HYDROGEN, tau, m, m1, t1 - t0);
        if tau1 >= 1.0 {
            // τ advanced over the step: no 0 ÷ 0 where it was 1 at the step's start.
            let x = if tau1 > tau {
                ((1.0 - tau) / (tau1 - tau)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (at, at_mass) = (t0 + x * (t1 - t0), m + x * (m1 - m));
            mass.push(at, at_mass);
            fractions.push(at, 1.0);
            main_sequence_end = Some((at, at_mass));
            break;
        }
        mass.push(t1, m1);
        fractions.push(t1, tau1);
        (m, tau, last) = (m1, tau1, t1);
    }
    // The partner's wind is over where the walk has reached its last living age.
    let partner_dead = walk.donor.lifetime().is_some();
    if main_sequence_end.is_none() && partner_dead && last >= end {
        let at = last + (1.0 - tau) * sse::main_sequence_lifetime(ctx.coeffs(), HYDROGEN, m);
        mass.push(at, m);
        fractions.push(at, 1.0);
        main_sequence_end = Some((at, m));
    }
    let carried = Member::MainSequence {
        helium: HYDROGEN,
        mass,
        tau: fractions,
    };
    let with = |member: Member| -> [Member; 2] {
        let mut pair = members.clone();
        pair[i] = member;
        pair
    };
    let segment = |from: f64, to: f64, pair: [Member; 2]| {
        Segment::new(
            SegmentKind::Detached,
            from,
            to,
            pair,
            Some(drawn_orbit_path(input)),
            None,
        )
    };
    let mut later = Vec::with_capacity(2);
    match main_sequence_end {
        Some((at, at_mass)) if at < until => {
            if at > t_on {
                later.push(segment(t_on, at, with(carried)));
            }
            let after = after_main_sequence(&ctx, i, at, at_mass, until);
            later.push(segment(at, until, with(after)));
        }
        Some(_) | None => later.push(segment(t_on, until, with(carried))),
    }
    let segments = std::iter::once(segment(0.0, t_on, members))
        .chain(later)
        .collect();
    BinaryTimeline::new(segments, None, Vec::new(), until, ctx, false)
}

/// Member `i` of `ctx`'s pair after its carried main sequence ends at `age` years at `mass` M☉,
/// for a pair run to `until` years: on a track of its mass, placed at its Hertzsprung gap's start
/// as `Engine::after_boundary` places one, built past `until`. It is the star's own track from
/// there, with the track's winds, since no partner feeds it any longer.
#[must_use]
fn after_main_sequence(ctx: &Context, i: usize, age: f64, mass: f64, until: f64) -> Member {
    let m = track_mass(SolarMasses::new(mass));
    let guess = sse::main_sequence_start(m, ctx.composition())
        + sse::main_sequence_lifetime(ctx.coeffs(), HYDROGEN, mass);
    let span = (until - age).max(0.0) + reach_margin_years(until);
    let (composition, draws, options) = (ctx.composition(), ctx.draws(i), ctx.track_options());
    let build = |reach: f64| Track::to_age_with(m, composition, draws, options, Years::new(reach));
    let (track, placed) = track_reaching(guess, span, build, |track| {
        track.age_in_phase(Phase::HertzsprungGap, 0.0)
    });
    let start = placed
        .or_else(|| track.lifetime().map(Years::value))
        .unwrap_or(0.0);
    Member::Track {
        track: Arc::new(track),
        offset: offset_for(age, start),
    }
}

#[cfg(test)]
mod tests;
