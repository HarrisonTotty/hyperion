//! The stellar mergers' scan marks (P11.T8.c; design note 10): what a galaxy-wide scan needs of a
//! merger, drawn first and on draws of its own, without running the binary engine (from 26 an
//! evolved donor's onset is asked of an engine built at T and not stepped).
//!
//! A stellar merger is a one-shot entry: one merger at its time T, uniform over the source horizon
//! ([`horizon_time`]), which P11.T9's luminous red nova and P12.T5.b's walk read with the pair's
//! total mass ([`MergerScanMarks::total_mass`], the red nova's merger mass). The marks are drawn in
//! this order, each group on its own stream:
//!
//! 1. the site and its component, thinned on the class's intensity (`class.merger.site`);
//! 2. T, and the system's age at the epoch from its component's own age law among its born
//!    systems (`class.merger.time`);
//! 3. its \[Fe/H\] from its component's law at the site and that age
//!    (`class.merger.metallicity`);
//! 4. its pair (`class.merger.mass`): the primary's layer by [`MERGER_SAMPLER`]'s weights,
//!    restricted to the layers that hold main-sequence stars at the merger, its initial mass from
//!    the galaxy's mass function in that band below the turn-off then
//!    ([`turn_off_mass`](crate::stellar::sse::turn_off_mass)), and the mass ratio, held so that the
//!    secondary is at least plan 06's least track mass, 0.1 M☉;
//! 5. both stars' states at the merger, each on its own track at its initial mass;
//! 6. its velocity at the site, plan 08's draw of its component's law (`class.merger.velocity`).
//!
//! From generator version 27 (P11.T8.c's fix) the pair's words begin with the channel
//! (`MergerChannel`): a contact pair, as above but its mass ratio at the tidal limit and its
//! primary heavy enough to hold a secondary there, or an evolved donor, past its main sequence
//! and living at T, its mass between the turn-off and the heaviest mass whose death comes after
//! T, beside a main-sequence companion inside the ratio the engine's stability test finds
//! unstable, on the circular orbit on which it fills its lobe. For an evolved donor the scan
//! builds the engine at T, without stepping it, and runs its onset of transfer (`class_merger`):
//! the stability test, then the common envelope; it keeps the candidate only if that merges the
//! pair into one star.
//!
//! From version 27 the channel is one of three (`MergerPick::EngineChannel`): the two above, or a
//! **dynamical main-sequence** merger, a low-mass (BSE's k = 0) main-sequence donor, the primary,
//! at its lobe beside a lighter main-sequence companion, past the critical ratio 0.695 at which the
//! engine finds its transfer dynamical, built at its lobe as an evolved donor is and kept only if
//! the engine's onset (its dynamical merger, `Engine::merge_dynamically`) merges the pair. An
//! evolved donor's companion is then drawn by its donor's kind's mark.
//!
//! Under the version's rules a candidate is kept exactly when both stars are on their main
//! sequences at the merger, which refuses only a star still contracting towards it in a young
//! population (or, at a turn-off, a track's draws ending it a little earlier than the median's).
//! The class's own test cannot refuse a candidate the scan keeps: a merger in the horizon of a
//! pair with a living star is a stellar merger.

use std::ops::Range;

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::Population;
use crate::galaxy::catalogue_classes::binary::{BinaryHost, StellarMergers};
use crate::galaxy::catalogue_classes::{CatalogueCellKey, ClassProcess};
use crate::galaxy::fields::ComponentId;
use crate::galaxy::imf::MassBand;
use crate::id::{BodyId, CatalogueSystemId, SystemId};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation, roche_lobe_radius};
use crate::rng::{Stream, tags};
use crate::stellar::binary::BinaryInput;
use crate::stellar::binary::class_merger::{
    donor_onset_merger, donor_onset_merges, unstable_companion_ratio,
};
use crate::stellar::draws::StarDraws;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{
    MAX_INITIAL_MASS, MIN_INITIAL_MASS, Track, TrackOptions, fate_of, turn_off_mass,
};
use crate::stellar::system::track_mass_of;
use crate::stellar::{Composition, Phase, StarState};
use crate::tables::binary::{MERGER_SAMPLER, MergerChannel, MergerMark, MergerPick};
use crate::time::{CLOCK_WINDOW_H, SourceHorizon, Span, UniverseTime};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{
    GravitationalParameter, Metres, PerCubicLightYear, Radians, SolarMasses, Years,
};

use super::super::sampler::ClassSampler;

/// What a galaxy-wide scan reads of a stellar merger (P11.T8.c): its place and motion, its merger
/// time T, its system's age and composition, and its pair before T.
///
/// # Examples
///
/// The mergers of one cell about the Sun-like point whose red novae went off in the last thousand
/// years, and the heaviest of them, as an alert scan reads them:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::StellarMergers;
/// use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
/// use hyperion_sim::stellar::binary::scan::scan_merger_cell;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = StellarMergers::new(&galaxy);
/// let mut mergers = Vec::new();
/// let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [0, 6, 0])?;
/// scan_merger_cell(&galaxy, &process, key, &mut mergers);
/// assert!(!mergers.is_empty());
/// let since = UniverseTime::from_julian_years(-1_000).ok_or("on the clock")?;
/// let recent = mergers.iter().filter(|m| since <= m.merger_time() && m.merger_time() < UniverseTime::EPOCH);
/// let heaviest = recent.map(|m| m.total_mass().value()).fold(0.0, f64::max);
/// assert!(heaviest < 300.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MergerScanMarks {
    host: BinaryHost,
    population: Population,
    velocity: GalacticVelocity,
    merger_time: UniverseTime,
    age_at_epoch: Years,
    composition: Composition,
    drawn: Drawn,
    masses: [SolarMasses; 2],
    stars: [StarState; 2],
}

impl MergerScanMarks {
    /// The merger's ID: member 0 of its catalogue entry, or its feature's list entry (P11.T8.f).
    #[must_use]
    pub const fn id(&self) -> SystemId {
        self.host.system()
    }

    /// Where the system sits and what it shares with its surroundings.
    #[must_use]
    pub const fn host(&self) -> &BinaryHost {
        &self.host
    }

    /// The density component the merger was drawn from, for a catalogue entry.
    #[must_use]
    pub const fn component(&self) -> Option<ComponentId> {
        self.host.component()
    }

    /// The population the system is counted in: its component's, or its feature's members'.
    #[must_use]
    pub const fn population(&self) -> Population {
        self.population
    }

    /// Where the system is at the epoch: its site in its cell, or its list entry's position.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        self.host.position()
    }

    /// The system's velocity, galactic, the one its entry moves at (plan 09's rule 6).
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The system's position at `t`, on its straight line through its site at the epoch: at
    /// [`merger_time`](Self::merger_time), where its red nova goes off.
    ///
    /// # Panics
    ///
    /// As plan 09's `CatalogueMotion::position_at`, beyond any time the clock reaches.
    #[must_use]
    pub fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        crate::galaxy::catalogue_classes::CatalogueMotion::straight(*self.position(), self.velocity)
            .position_at(t)
    }

    /// The merger time T, inside the source horizon, `[−(H + L), +H)`: the onset of its
    /// luminous red nova.
    #[must_use]
    pub const fn merger_time(&self) -> UniverseTime {
        self.merger_time
    }

    /// The system's age at the epoch, Julian years.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// The system's age at T, Julian years: the pair's `merger_age`, as plan 03's record counts
    /// its age at a time.
    #[must_use]
    pub fn merger_age(&self) -> Years {
        Years::new(self.age_at_epoch.value() + self.merger_time.since_epoch().as_julian_years_f64())
    }

    /// The composition both stars share.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The primary's initial mass, M☉: the record's.
    #[must_use]
    pub const fn primary_mass(&self) -> SolarMasses {
        self.masses[0]
    }

    /// The secondary's initial mass, M☉, at most the primary's.
    #[must_use]
    pub const fn secondary_mass(&self) -> SolarMasses {
        self.masses[1]
    }

    /// The merger's mass, M☉: the pair's initial masses' sum, which a luminous red nova's plateau
    /// and peak scale with (P11.T9.a's `LuminousRedNovaCurve::of`). On the main sequence a pair
    /// keeps nearly all of it: winds take a few per cent off only above some 20 M☉.
    #[must_use]
    pub fn total_mass(&self) -> SolarMasses {
        self.masses[0] + self.masses[1]
    }

    /// Each star's state at T on its own track, primary first: both on their main sequences in a
    /// contact pair and (from 27) a dynamical main-sequence merger, the primary past its main
    /// sequence for an evolved donor.
    #[must_use]
    pub const fn stars(&self) -> &[StarState; 2] {
        &self.stars
    }

    /// The merger's channel: one of the engine's three, a contact pair, an evolved donor or the
    /// dynamical main sequence (P11.T8.c's fix and its missing channel).
    #[must_use]
    pub const fn channel(&self) -> MergerChannel {
        match self.drawn {
            Drawn::Contact => MergerChannel::Contact,
            Drawn::EvolvedDonor { .. } => MergerChannel::EvolvedDonor,
            Drawn::DynamicalMainSequence { .. } => MergerChannel::DynamicalMainSequence,
        }
    }

    /// For a donor at its lobe, evolved or (from 27) a dynamical main-sequence donor, the
    /// separation on which it fills its Roche lobe at T, on which the engine's onset merges the
    /// pair; `None` for a contact pair.
    #[must_use]
    pub const fn donor_separation(&self) -> Option<Metres> {
        match self.drawn {
            Drawn::Contact => None,
            Drawn::EvolvedDonor { separation } | Drawn::DynamicalMainSequence { separation } => {
                Some(separation)
            }
        }
    }
}

/// The time at `share`, in [0, 1), of the source horizon `[−(H + L), +H)`: a one-shot entry's T
/// for a uniform `share` (P11.T8.c–d), floored to the nanosecond.
///
/// # Panics
///
/// Never: the horizon is some 8 × 10¹² s long, on the clock.
#[must_use]
pub fn horizon_time(share: f64) -> UniverseTime {
    let horizon = SourceHorizon::END
        .checked_since(SourceHorizon::START)
        .expect("the source horizon runs forward");
    let into = Span::from_seconds_f64(share.clamp(0.0, 1.0) * horizon.as_seconds_f64())
        .expect("a share of the horizon is a span");
    let t = SourceHorizon::START
        .checked_add(into)
        .expect("the source horizon is on the clock");
    // A share that rounds up to the horizon's end stays inside it.
    if t < SourceHorizon::END {
        t
    } else {
        SourceHorizon::END
            .checked_sub(Span::new(0, 1).expect("a nanosecond is a span"))
            .expect("the horizon's end is on the clock")
    }
}

/// The state at `age` of a star of initial mass `m0`, `composition` and `draws` under `rules`, on
/// its own track, if it is on its main sequence then.
#[must_use]
fn main_sequence_at(
    m0: SolarMasses,
    composition: &Composition,
    draws: &StarDraws,
    rules: StellarRules,
    age: Years,
) -> Option<StarState> {
    if !(age.value() > 0.0 && age.value().is_finite()) {
        return None;
    }
    let options = TrackOptions::default().with_rules(rules);
    let track = Track::to_age_with(track_mass_of(m0), composition, draws, options, age);
    let state = track.state_at(age);
    (state.phase() == Phase::MainSequence).then_some(state)
}

/// The scan marks of candidate `id` of the stellar merger class cell `cell` under its `bound`, or
/// `None` if the class keeps no entry there (P11.T8.c; module documentation). It never runs the
/// binary engine.
///
/// The marks are the entry's own, bit for bit: the class's candidate draws them through this
/// function first.
///
/// # Panics
///
/// If `cell` is not a stellar merger cell; in debug builds, on a density above the bound.
#[must_use]
pub fn merger_scan_marks(
    galaxy: &Galaxy,
    process: &StellarMergers,
    cell: &CatalogueCellKey,
    id: CatalogueSystemId,
    bound: PerCubicLightYear,
) -> Option<MergerScanMarks> {
    let (position, component) =
        process
            .hosts()
            .site(galaxy, cell, id, bound, tags::CLASS_MERGER_SITE)?;
    merger_marks_at(galaxy, BinaryHost::field(id, position, component))
}

/// The scan marks of a stellar merger at `host`, after its site, or `None` if its stars are not
/// both on their main sequences at T: T and its age, its composition, its pair and its velocity. A
/// catalogue candidate's ([`merger_scan_marks`]) and a feature's list entry's (P11.T8.f), on the
/// same draws keyed by each one's system; a list entry's age is drawn in its members' ages by the
/// same rank, and its composition is theirs.
#[must_use]
pub(crate) fn merger_marks_at(galaxy: &Galaxy, host: BinaryHost) -> Option<MergerScanMarks> {
    let seed = galaxy.seed();
    let system = host.system();
    let key = host.key();
    let sampler = ClassSampler::new(&MERGER_SAMPLER);
    // 2. T and the age.
    let mut stream = Stream::open(seed, tags::CLASS_MERGER_TIME, key);
    let merger_time = horizon_time(stream.uniform());
    let rank = sampler.draw(MergerMark::AgeRank, 0.0, &mut stream);
    let age_at_epoch = host.age_at_rank(galaxy, rank);
    let merger_age =
        Years::new(age_at_epoch.value() + merger_time.since_epoch().as_julian_years_f64());
    // 3. The composition, at the site and the age.
    let composition = host.composition_at(galaxy, age_at_epoch, tags::CLASS_MERGER_METALLICITY);
    // 4–5. The pair and both stars at T: the channel first, one of three.
    let mut stream = Stream::open(seed, tags::CLASS_MERGER_MASS, key);
    let draws = [0_u16, 1].map(|body| StarDraws::for_star(seed, BodyId::new(system, body)));
    let at = (age_at_epoch, merger_age);
    let channel = MergerChannel::ALL[sampler.pick(MergerPick::EngineChannel, &mut stream)];
    let pair = match channel {
        MergerChannel::Contact => contact_pair(
            galaxy,
            &composition,
            &draws,
            (merger_age, MergerMark::ContactMassRatio),
            &mut stream,
        )?,
        MergerChannel::EvolvedDonor => evolved_pair(galaxy, &composition, &draws, at, &mut stream)?,
        MergerChannel::DynamicalMainSequence => {
            dynamical_pair(galaxy, &composition, &draws, at, &mut stream)?
        }
    };
    // 6. The velocity.
    let velocity = host.velocity(galaxy, tags::CLASS_MERGER_VELOCITY);
    Some(MergerScanMarks {
        host,
        population: host.population(galaxy),
        velocity,
        merger_time,
        age_at_epoch,
        composition,
        drawn: pair.drawn,
        masses: pair.masses,
        stars: pair.stars,
    })
}

/// A merger's channel as the scan drew it (P11.T8.c's channels), with what the channel carries:
/// for a donor at its lobe the separation on which it fills its Roche lobe at T.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drawn {
    /// A contact pair of two main-sequence stars.
    Contact,
    /// An evolved donor, which the engine's onset of transfer merges with its companion.
    EvolvedDonor {
        /// The pair's separation at T, m.
        separation: Metres,
    },
    /// A low-mass main-sequence donor, which the engine's onset of transfer merges dynamically
    /// with its companion (from 27).
    DynamicalMainSequence {
        /// The pair's separation at T, m.
        separation: Metres,
    },
}

/// A merger's pair as the scan draws it: its channel, initial masses and states at T.
struct DrawnPair {
    drawn: Drawn,
    masses: [SolarMasses; 2],
    stars: [StarState; 2],
}

/// A contact pair of two main-sequence stars at `merger_age` (`at`), drawn on `stream` (the
/// pair's words): the primary's layer among those with main-sequence stars at T whose primaries
/// can hold a secondary of plan 06's least track mass at the mark's largest ratio, its mass in the
/// galaxy's mass function in that range below the turn-off then, and the mass ratio from `at`'s
/// mark, held so that the secondary is at least that least mass; `None` unless both stars are on
/// their main sequences at T. Version 25's class verbatim under its mark, whose largest ratio, 1,
/// cuts no primary.
fn contact_pair(
    galaxy: &Galaxy,
    composition: &Composition,
    draws: &[StarDraws; 2],
    (merger_age, mass_ratio): (Years, MergerMark),
    stream: &mut Stream,
) -> Option<DrawnPair> {
    let sampler = ClassSampler::new(&MERGER_SAMPLER);
    let turn_off = turn_off_mass(Years::new(merger_age.value().max(0.0)), composition).value();
    let floor = MIN_INITIAL_MASS.value();
    // The lightest primary whose secondary at the mark's largest ratio is a track's.
    let lightest = floor / MERGER_SAMPLER.mark(mass_ratio).limits().1;
    let range = |band: MassBand| (band.lo().max(floor).max(lightest), band.hi().min(turn_off));
    let shares = MERGER_SAMPLER.pick(MergerPick::Layer).weights();
    let mut weights = [0.0; 5];
    for ((weight, &share), band) in weights.iter_mut().zip(shares).zip(MassBand::ALL) {
        let (lo, hi) = range(band);
        if lo < hi {
            *weight = share;
        }
    }
    let total = weights.iter().fold(0.0, |sum, w| sum + w);
    let mark = stream.mark();
    let fallback = weights.iter().rposition(|&w| w > 0.0)?;
    let layer = mark.pick_weighted(&weights, total).unwrap_or(fallback);
    let (lo, hi) = range(MassBand::ALL[layer]);
    let rank = sampler.draw(MergerMark::PrimaryRank, 0.0, stream);
    let m1 = galaxy.mass_function().quantile_in(lo, hi, rank);
    let q_floor = sampler.rank_of(mass_ratio, 0.0, floor / m1);
    let q = sampler.draw_in_ranks(mass_ratio, 0.0, (q_floor, 1.0), stream);
    let m2 = (q * m1).clamp(floor, m1);
    let masses = [SolarMasses::new(m1), SolarMasses::new(m2)];
    let rules = galaxy.rules().stellar;
    let star = |k: usize| main_sequence_at(masses[k], composition, &draws[k], rules, merger_age);
    Some(DrawnPair {
        drawn: Drawn::Contact,
        masses,
        stars: [star(0)?, star(1)?],
    })
}

/// The phases of an evolved donor at its merger: past its main sequence and living, from the
/// Hertzsprung gap to the thermally pulsing asymptotic giant branch.
const EVOLVED_PHASES: [Phase; 5] = [
    Phase::HertzsprungGap,
    Phase::FirstGiantBranch,
    Phase::CoreHeliumBurning,
    Phase::EarlyAgb,
    Phase::ThermallyPulsingAgb,
];

/// The step, dex, by which the bracket of the heaviest star still living at a merger widens from
/// the turn-off: 0.15. Past the main sequence a star lives some 10–20% of its life, a few
/// hundredths of a dex in mass, so one step nearly always brackets it.
const LIVING_BRACKET_DEX: f64 = 0.15;

/// Bisections of that bracket: 10, to 1.5 × 10⁻⁴ dex. The edge needs no more: the range runs to
/// the bracket's dead side, so a donor drawn a little past the edge is dead at T and refused by its
/// own state, and each bisection costs a fate, some 0.4 ms (plan 06's `fate_of`, measured in a
/// release build).
const LIVING_BISECTIONS: u32 = 10;

/// An evolved donor and its companion at the merger, drawn on `stream` (the pair's words, after the
/// channel's), with `at` the system's age at the epoch and at T: the donor's layer among those with
/// stars past their main sequence and living at T, its mass in the galaxy's mass function between
/// the turn-off then and the heaviest mass whose death, plan 06's fate of its own draws, comes
/// after T ([`heaviest_living`]); then its state at T, and the companion's mass by its donor's
/// kind's mark ([`companion_ratio_mark`]) inside the ratio the engine's stability test finds
/// unstable for that donor (`class_merger::unstable_companion_ratio`), on its main sequence at T;
/// the circular orbit on which the donor fills its Roche lobe (Eggleton 1983). `None` unless the
/// donor is in one of
/// [`EVOLVED_PHASES`] at T, the companion on its main sequence, and the engine's onset of
/// transfer, its stability test and common envelope (`class_merger`), merges them into one star.
fn evolved_pair(
    galaxy: &Galaxy,
    composition: &Composition,
    draws: &[StarDraws; 2],
    (age_at_epoch, merger_age): (Years, Years),
    stream: &mut Stream,
) -> Option<DrawnPair> {
    let sampler = ClassSampler::new(&MERGER_SAMPLER);
    let rules = galaxy.rules().stellar;
    let options = TrackOptions::default().with_rules(rules);
    if merger_age.value().is_nan() || merger_age.value() <= 0.0 {
        return None;
    }
    let turn_off = turn_off_mass(merger_age, composition);
    let floor = MIN_INITIAL_MASS.value();
    let living_end = heaviest_living(composition, &draws[0], options, merger_age, turn_off).value();
    let turn_off = turn_off.value();
    let shares = MERGER_SAMPLER.pick(MergerPick::Layer).weights();
    let mut weights = [0.0; 5];
    for ((weight, &share), band) in weights.iter_mut().zip(shares).zip(MassBand::ALL) {
        if band.lo() < living_end && turn_off < band.hi() {
            *weight = share;
        }
    }
    let total = weights.iter().fold(0.0, |sum, w| sum + w);
    let mark = stream.mark();
    let fallback = weights.iter().rposition(|&w| w > 0.0)?;
    let layer = mark.pick_weighted(&weights, total).unwrap_or(fallback);
    let band = MassBand::ALL[layer];
    let (m_lo, m_hi) = (band.lo().max(turn_off), band.hi().min(living_end));
    let rank = sampler.draw(MergerMark::EvolvedPrimaryRank, 0.0, stream);
    let m1 = galaxy.mass_function().quantile_in(m_lo, m_hi, rank);
    let donor = Track::to_age_with(
        track_mass_of(SolarMasses::new(m1)),
        composition,
        &draws[0],
        options,
        merger_age,
    )
    .state_at(merger_age);
    if !EVOLVED_PHASES.contains(&donor.phase()) {
        return None;
    }
    // The companion on its main sequence at T, below the turn-off and inside the unstable ratio
    // of the donor's mass now (its own mass now is at most its initial mass, so the ratio holds),
    // at least the least track mass.
    let unstable = unstable_companion_ratio(&donor, composition)? * donor.mass().value();
    let ceiling = turn_off.min(unstable).min(m1);
    if ceiling <= floor {
        return None;
    }
    let ratio = companion_ratio_mark(donor.phase());
    let ranks = (
        sampler.rank_of(ratio, 0.0, floor / m1),
        sampler.rank_of(ratio, 0.0, ceiling / m1),
    );
    let q = sampler.draw_in_ranks(ratio, 0.0, ranks, stream);
    let m2 = (q * m1).clamp(floor, ceiling);
    let masses = [SolarMasses::new(m1), SolarMasses::new(m2)];
    let companion = main_sequence_at(masses[1], composition, &draws[1], rules, merger_age)?;
    let stars = [donor, companion];
    let (md, mc) = (donor.mass().value(), companion.mass().value());
    let share = roche_lobe_radius(md / mc, Metres::new(1.0)).value();
    let separation = Metres::new(donor.radius().value() * SOLAR_RADIUS_M / share);
    let input = lobe_input(
        (masses, &stars),
        separation,
        composition,
        draws,
        (age_at_epoch, rules),
    );
    let until = lobe_until(age_at_epoch, merger_age);
    donor_onset_merges(&input, merger_age, until).then_some(DrawnPair {
        drawn: Drawn::EvolvedDonor { separation },
        masses,
        stars,
    })
}

/// The mark an evolved donor's companion's mass ratio is drawn from: from 27 its donor's kind's, by
/// the donor's phase at T (the giant branch's [`MergerMark::EvolvedMassRatio`] itself, version 26's
/// for every donor).
#[must_use]
const fn companion_ratio_mark(donor: Phase) -> MergerMark {
    match donor {
        Phase::HertzsprungGap => MergerMark::GapMassRatio,
        Phase::CoreHeliumBurning => MergerMark::HeliumBurningMassRatio,
        Phase::EarlyAgb | Phase::ThermallyPulsingAgb => MergerMark::AgbMassRatio,
        // A giant-branch donor keeps the evolved mark; an evolved donor is in `EVOLVED_PHASES`, so
        // the other phases are refused before they reach here.
        Phase::FirstGiantBranch
        | Phase::Protostar
        | Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::HeliumWhiteDwarf
        | Phase::CarbonOxygenWhiteDwarf
        | Phase::OxygenNeonWhiteDwarf
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant
        | Phase::Substellar => MergerMark::EvolvedMassRatio,
    }
}

/// A dynamical main-sequence merger's pair at the merger (`at`: the system's age at the epoch and
/// at T), drawn on `stream` (the pair's words, after the channel's; version 27's channel): the
/// donor, the primary, of initial mass by [`MergerMark::DynamicalPrimaryMass`] (a low-mass, BSE's
/// k = 0, main-sequence star, below the turn-off then), and its companion's mass by
/// [`MergerMark::DynamicalMassRatio`], at least plan 06's least track mass; both on their main
/// sequences at T, on the circular orbit on which the donor fills its Roche lobe (Eggleton 1983)
/// and the companion lies inside its own. `None` unless they are, and the engine's onset of
/// transfer from the donor (`class_merger`: its stability test, the donor past its critical ratio
/// 0.695 as a lighter companion always leaves it, then `Engine::merge_dynamically`) merges them
/// into one star.
fn dynamical_pair(
    galaxy: &Galaxy,
    composition: &Composition,
    draws: &[StarDraws; 2],
    (age_at_epoch, merger_age): (Years, Years),
    stream: &mut Stream,
) -> Option<DrawnPair> {
    let sampler = ClassSampler::new(&MERGER_SAMPLER);
    let rules = galaxy.rules().stellar;
    if merger_age.value().is_nan() || merger_age.value() <= 0.0 {
        return None;
    }
    let floor = MIN_INITIAL_MASS.value();
    let turn_off = turn_off_mass(merger_age, composition).value();
    let top = sampler.rank_of(MergerMark::DynamicalPrimaryMass, 0.0, turn_off);
    let m1 = sampler.draw_in_ranks(MergerMark::DynamicalPrimaryMass, 0.0, (0.0, top), stream);
    let q_floor = sampler.rank_of(MergerMark::DynamicalMassRatio, 0.0, floor / m1);
    let q = sampler.draw_in_ranks(MergerMark::DynamicalMassRatio, 0.0, (q_floor, 1.0), stream);
    let m2 = (q * m1).clamp(floor, m1);
    let masses = [SolarMasses::new(m1), SolarMasses::new(m2)];
    let star = |k: usize| main_sequence_at(masses[k], composition, &draws[k], rules, merger_age);
    let stars = [star(0)?, star(1)?];
    let [donor, companion] = stars;
    let (md, mc) = (donor.mass().value(), companion.mass().value());
    let unit = Metres::new(1.0);
    let share = roche_lobe_radius(md / mc, unit).value();
    let separation = Metres::new(donor.radius().value() * SOLAR_RADIUS_M / share);
    let companion_lobe = roche_lobe_radius(mc / md, separation).value();
    if companion.radius().value() * SOLAR_RADIUS_M >= companion_lobe {
        return None;
    }
    let input = lobe_input(
        (masses, &stars),
        separation,
        composition,
        draws,
        (age_at_epoch, rules),
    );
    let until = lobe_until(age_at_epoch, merger_age);
    donor_onset_merges(&input, merger_age, until).then_some(DrawnPair {
        drawn: Drawn::DynamicalMainSequence { separation },
        masses,
        stars,
    })
}

/// The engine's input for a donor's pair at its lobe, evolved or (from 27) a dynamical
/// main-sequence donor, of initial `masses` and states at T `stars`, on the circular orbit of
/// `separation` about their masses then, with zero angles, at `composition` with the stars'
/// `draws`, the system `age_at_epoch` old at the epoch, under the stellar `rules`: the one input
/// both the scan's test and the candidate's star read.
///
/// # Panics
///
/// Never: zero angles are an orientation, a donor's positive radius over its lobe's share of the
/// separation is a positive separation about a positive mass, and the masses are tracks' at a
/// finite age.
#[must_use]
fn lobe_input(
    (masses, stars): ([SolarMasses; 2], &[StarState; 2]),
    separation: Metres,
    composition: &Composition,
    draws: &[StarDraws; 2],
    (age_at_epoch, rules): (Years, StellarRules),
) -> BinaryInput {
    let orbit = KeplerElements::from_semi_major_axis(
        separation,
        GravitationalParameter::from_solar_masses(stars[0].mass() + stars[1].mass()),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)
            .expect("zero angles are an orientation"),
        Radians::ZERO,
    )
    .expect("a donor's positive radius over its lobe's share is a positive separation");
    BinaryInput::new(
        masses[0],
        masses[1],
        *composition,
        orbit,
        draws.clone(),
        age_at_epoch,
    )
    .expect("masses drawn in 0.1–150 M☉ and a finite age are an input")
    .with_rules(rules)
}

/// The heaviest initial mass whose death, plan 06's fate at the donor's `draws` under `options`,
/// comes after the merger at `merger_age`, from the turn-off `turn_off` up, bracketed in
/// [`LIVING_BRACKET_DEX`] steps and bisected [`LIVING_BISECTIONS`] times: the bracket's dead side,
/// so that the range it ends misses no living mass.
#[must_use]
fn heaviest_living(
    composition: &Composition,
    draws: &StarDraws,
    options: TrackOptions,
    merger_age: Years,
    turn_off: SolarMasses,
) -> SolarMasses {
    let t = merger_age.value();
    // The round trip through log₁₀ can land a part in 10¹⁶ above the track's 150 M☉.
    let dead_by = |log_m: f64| {
        fate_of(
            SolarMasses::new(math::exp10(log_m).min(MAX_INITIAL_MASS.value())),
            composition,
            draws,
            options,
        )
        .death
        .age()
        .value()
            <= t
    };
    let ceiling = math::log10(MAX_INITIAL_MASS.value());
    let mut lo = math::log10(turn_off.value());
    let mut hi = (lo + LIVING_BRACKET_DEX).min(ceiling);
    while !dead_by(hi) && hi < ceiling {
        lo = hi;
        hi = (hi + LIVING_BRACKET_DEX).min(ceiling);
    }
    if !dead_by(hi) {
        return MAX_INITIAL_MASS;
    }
    for _ in 0..LIVING_BISECTIONS {
        let mid = f64::midpoint(lo, hi);
        if dead_by(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    SolarMasses::new(math::exp10(hi).min(MAX_INITIAL_MASS.value()))
}

/// The age the engine of a donor's pair at its lobe is run to: H past the later of T and the
/// epoch, as `SystemStars` runs its pairs.
#[must_use]
fn lobe_until(age_at_epoch: Years, merger_age: Years) -> Years {
    Years::new(merger_age.value().max(age_at_epoch.value()) + CLOCK_WINDOW_H.as_julian_years_f64())
}

/// The star a donor's merger at its lobe leaves at T (P11.T8.c's channels: an evolved donor, or
/// from 27 a dynamical main-sequence donor), the engine's own for the pair of `scan`, the scan's
/// own input rebuilt from its marks ([`lobe_input`]) with the stars' `draws` under the stellar
/// `rules`; `None` for a contact pair, and for a donor whose onset the engine does not merge,
/// which the scan never keeps.
#[must_use]
pub(crate) fn donor_product(
    scan: &MergerScanMarks,
    draws: &[StarDraws; 2],
    rules: StellarRules,
) -> Option<StarState> {
    let separation = scan.donor_separation()?;
    let input = lobe_input(
        (scan.masses, &scan.stars),
        separation,
        &scan.composition,
        draws,
        (scan.age_at_epoch, rules),
    );
    let until = lobe_until(scan.age_at_epoch, scan.merger_age());
    donor_onset_merger(&input, scan.merger_age(), until)
}

/// Appends to `out` the scan marks of every entry of the stellar merger class cell `key`, in index
/// order: one bound, one count and [`merger_scan_marks`] of each candidate, the cell's entries
/// exactly.
///
/// # Panics
///
/// As [`merger_scan_marks`], and as plan 09's count on a bound past the Poisson sampler's limit.
pub fn scan_merger_cell(
    galaxy: &Galaxy,
    process: &StellarMergers,
    key: CatalogueCellKey,
    out: &mut Vec<MergerScanMarks>,
) {
    scan_merger_cell_in(galaxy, process, key, 0..u32::MAX, out);
}

/// Appends to `out` the scan marks of the entries of the stellar merger class cell `key` whose
/// candidates' indices lie in `indices`, held to the cell's count, in index order.
///
/// It is a part of [`scan_merger_cell`]'s marks, which a background walk splits a dense cell
/// into (plan 12). Over the parts of any partition of `0..u32::MAX`, or of `0..` the count, in
/// order, they are `scan_merger_cell`'s marks exactly: every part evaluates the cell's one bound
/// ([`ClassProcess::bound`]) and count, and each candidate's marks are a pure function of its ID.
///
/// # Panics
///
/// As [`scan_merger_cell`].
pub(crate) fn scan_merger_cell_in(
    galaxy: &Galaxy,
    process: &StellarMergers,
    key: CatalogueCellKey,
    indices: Range<u32>,
    out: &mut Vec<MergerScanMarks>,
) {
    let bound = ClassProcess::bound(process, galaxy, &key);
    let candidates =
        crate::galaxy::catalogue_classes::grid::candidate_count(galaxy.seed(), &key, bound);
    out.extend(
        (indices.start.min(candidates)..indices.end.min(candidates)).filter_map(|index| {
            let id = key
                .candidate_id(index, 0)
                .expect("a count held under 2^24 numbers every candidate");
            merger_scan_marks(galaxy, process, &key, id, bound)
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// From 27 a gap donor's companion takes the gap's mark, a helium-burning donor's its own, an
    /// asymptotic-giant-branch donor's the AGB's and a giant branch's the evolved mark.
    #[test]
    fn an_evolved_companion_s_mark_follows_its_donor_s_kind_from_27() {
        let by_kind = [
            MergerMark::GapMassRatio,
            MergerMark::EvolvedMassRatio,
            MergerMark::HeliumBurningMassRatio,
            MergerMark::AgbMassRatio,
            MergerMark::AgbMassRatio,
        ];
        for (phase, mark) in EVOLVED_PHASES.into_iter().zip(by_kind) {
            assert_eq!(companion_ratio_mark(phase), mark, "{phase:?}");
        }
    }
}
