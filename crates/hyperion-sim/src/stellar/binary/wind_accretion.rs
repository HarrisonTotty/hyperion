//! The hold on a wide pair's wind accretion (plan 11, P11.T12's fix, at
//! [`WIND_ACCRETION_VERSION`]).
//!
//! As built, a pair whose stars never reach their Roche lobes, on the drawn orbit or after the
//! engine's sinks have shrunk it, is passed over by the pre-test ([`can_interact`]) as two single
//! stars on their orbit, plan 06's own. So the Bondi–Hoyle accretion of Hurley, Tout and Pols
//! (2002, MNRAS 329, 897, "BSE", equations 6–10), which the engine takes in `detached.rs`, never
//! feeds the wide pairs whose wind transfer makes the field's long-period blue stragglers (P of
//! 10²–10⁴ d, low e, white-dwarf companions: Preston and Sneden 2000, AJ 120, 1014; Carney et al.
//! 2005, AJ 129, 466; recalled, not read in the pod). research-leans-29 found, under every 27 hold,
//! 0.215 blue stragglers per horizontal-branch star in the halo against the halo field's about 4,
//! and 0.145 in the bulge against 0.29–0.37 (Clarkson et al. 2011, ApJ 735, 37; recalled). Of the
//! halo's companions within 0.25 M☉ under the turn-off beside an evolved primary, about 12.5 per
//! horizontal-branch star stayed detached at P ≥ 10³ d, every one with no mass accreted.
//!
//! From 27 ([`WindAccretionRules::BondiHoyle`]) a pair the lobe and decay tests pass over is walked
//! over its partner's own track: a hydrogen main-sequence star of at least 0.1 M☉ on its own track
//! takes BSE's equation-6 share of its partner's wind ([`bondi_hoyle_share`], the engine's own
//! expression), with equation 9's wind speed and at most equation 10's 0.8, step by step between
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
//! The donor keeps its own track throughout, and the orbit shown is the drawn one, as for every
//! pair passed over: the widening enters the share alone. Leans of 2026-11-01, recorded in plan 11:
//! a star accretes only on its main sequence, so neither a remnant (the symbiotic stars are
//! classified from the state, as before) nor a giant nor a star below 0.1 M☉ takes a wind here; and
//! the accretor's later radius on its new track is not tested against its lobe.
//!
//! The rules' one reading is [`onset`], which [`can_interact`] and `evolve`'s pass-over read
//! through the input's rules. The pair's past does not depend on the age it is run to: the steps
//! are the partner's track's own, the onset is read at the start of a step, and the last step taken
//! is the one that reaches or passes the age, as the engine's is (`detached::StepLimit`).
//!
//! [`can_interact`]: super::can_interact
//! [`rejuvenated_tau_unheld`]: super::rlof::rejuvenated_tau_unheld

use std::sync::Arc;

use crate::GeneratorVersion;
use crate::stellar::Phase;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{self, Track};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{SolarMasses, Years};

use super::BinaryParams;
use super::detached::last_living;
use super::evolve::{drawn_orbit_path, offset_for, reach_margin_years, track_mass, track_reaching};
use super::rlof::rejuvenated_tau_unheld;
use super::star::{G, Kind, Member, Path};
use super::timeline::{BinaryInput, BinaryTimeline, Context, Segment, SegmentKind};

/// The generator version whose stellar rules give a passed-over pair's main-sequence star its share
/// of its partner's wind (P11.T12's fix): [`WindAccretionRules::BondiHoyle`].
///
/// Below it every pair takes the held arm, [`WindAccretionRules::OwnTracks`], the code of version
/// 24 verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_27_hold()`. The bump to 27 deletes this hold: [`WindAccretionRules`], the
/// stellar rules' `wind_accretion` field, [`onset`]'s read of it, the brief's read of
/// [`WindAccretionRules::fitted_tables_hold`], and the tripwire
/// (`the_wind_accretion_is_held_only_until_the_27_bump`). The bump refits the pair-light and reach
/// tables, which the engine's runs fit, and gives `pair_light_bound`'s `Detached` the wind's reach,
/// which `detached_generated_pairs_are_passed_over` then asks of it.
pub(crate) const WIND_ACCRETION_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether a passed-over pair's stars take each other's wind (the stellar rules' `wind_accretion`
/// field, the hold of [`WIND_ACCRETION_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum WindAccretionRules {
    /// Version 26's and before: a pair the lobe and decay tests pass over is two single stars on
    /// their orbit, plan 06's own.
    OwnTracks,
    /// Version 27's: its main-sequence star takes its Bondi–Hoyle share of its partner's wind, and
    /// from [`LEAST_ACCRETED_MSUN`] the pair is wind-fed.
    BondiHoyle,
}

impl WindAccretionRules {
    /// Whether a passed-over pair's stars take each other's wind under these rules.
    #[must_use]
    pub(crate) const fn accretes(self) -> bool {
        match self {
            Self::OwnTracks => false,
            Self::BondiHoyle => true,
        }
    }

    /// Whether the tables fitted offline from the binary engine's runs, and the closed-form bound
    /// beside them (the pair-light and reach tables and `pair_light_bound`'s `Detached`, which the
    /// range brief's gate reads), hold for a pair under this arm: under the version's arm only,
    /// until the bump refits them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).wind_accretion
    }
}

/// The least mass, M☉, a star of a pair the lobe and decay tests pass over must take of its
/// partner's wind for the pair to be no longer two single stars on their orbit: 10⁻⁴ (the brief's
/// lean of 2026-11-01), under which the pair stays plan 06's own, and the cheap path cheap.
pub(crate) const LEAST_ACCRETED_MSUN: f64 = 1.0e-4;

/// The midpoint steps the walk takes between two of the partner's mass knots: the share is read
/// at each step's middle, and the mass lost over it exactly. Two hold the accreted mass to 0.3%
/// of eight's on the halo's 0.85 + 0.80 M☉ pair at 3,000 d (`the_walk_has_converged`).
pub(crate) const STEPS_PER_INTERVAL: u32 = 2;

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
    steps: u32,
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
            steps: STEPS_PER_INTERVAL,
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

    /// At most what the accretor can take of its partner's wind from `start` to `end`, M☉, for a
    /// screen before the walk: the wind's share at its largest, every factor at its bound (the
    /// partner's largest radius and least mass, the accretor at its most, the drawn orbit, the
    /// slowest wind, no orbital speed), times all the partner loses.
    #[must_use]
    fn bound(&self, end: f64) -> f64 {
        let least = self.donor.mass_at(end);
        let lost = (self.donor.initial_mass().value() - least).max(0.0);
        if lost <= 0.0 {
            return 0.0;
        }
        let radius = self.donor.max_radius_until(Years::new(end)).value();
        let most = self.accretor.initial_mass().value() + self.params.max_wind_accretion * lost;
        // G M_a ÷ v_W² = M_a R ÷ (2 β_W M_d).
        let focus = most * radius / (2.0 * self.params.wind_speed_factor.least() * least);
        let share = self.params.bondi_hoyle * focus * focus
            / (2.0 * self.a0 * self.a0)
            / (1.0 - self.e * self.e).sqrt();
        share.min(self.params.max_wind_accretion) * lost
    }

    /// The walk's steps from `start` to `end`, years, in age order: the spans between the
    /// partner's mass knots and phase boundaries, each cut into [`Walk::steps`] equal steps.
    fn steps(&self, start: f64, end: f64) -> impl Iterator<Item = (f64, f64)> + '_ {
        let n = self.steps;
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
    /// `t1`, M☉: the share at the step's middle times the mass the partner loses over it.
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
        lost * bondi_hoyle_share(self.params, kind, m_donor, r_donor, m_accretor, a, self.e)
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
/// years, becomes wind-fed under the input's rules ([`WindAccretionRules`]), if it does by then:
/// the earlier of its two stars' onsets, the primary's first.
#[must_use]
pub(super) fn onset(input: &BinaryInput, members: &[Member; 2], until: f64) -> Option<Onset> {
    if !input.rules().wind_accretion.accretes() {
        return None;
    }
    (0..2)
        .filter_map(|accretor| {
            let walk = Walk::of(input, members, accretor)?;
            let (start, end) = walk.span()?;
            if walk.bound(end) < LEAST_ACCRETED_MSUN {
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
}

/// The timeline of the wind-fed pair of `input`, its own `members` run to `until` years, from its
/// `onset` (see the module documentation): both stars their own, then the accretor a carried main
/// sequence fed its share of each step, then the accretor on a track of its mass from its main
/// sequence's end; the partner its own, and the orbit the drawn one, throughout.
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
        let tau1 = rejuvenated_tau_unheld(ctx.coeffs(), false, tau, m, m1, t1 - t0);
        if tau1 >= 1.0 {
            let x = ((1.0 - tau) / (tau1 - tau)).clamp(0.0, 1.0);
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
        let at = last + (1.0 - tau) * sse::main_sequence_lifetime(ctx.coeffs(), false, m);
        mass.push(at, m);
        fractions.push(at, 1.0);
        main_sequence_end = Some((at, m));
    }
    let carried = Member::MainSequence {
        helium: false,
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
        + sse::main_sequence_lifetime(ctx.coeffs(), false, mass);
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
