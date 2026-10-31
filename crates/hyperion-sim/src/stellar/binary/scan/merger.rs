//! The stellar mergers' scan marks (P11.T8.c; design note 10): what a galaxy-wide scan needs of a
//! merger, drawn first and on draws of its own, without the binary engine.
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
//! A candidate is kept exactly when both stars are on their main sequences at the merger, which
//! refuses only a star still contracting towards it in a young population (or, at a turn-off, a
//! track's draws ending it a little earlier than the median's). The class's own test cannot refuse
//! a candidate the scan keeps: a merger in the horizon of a pair with a living star is a stellar
//! merger.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::CatalogueCellKey;
use crate::galaxy::catalogue_classes::binary::{ClassHosts, StellarMergers};
use crate::galaxy::fields::ComponentId;
use crate::galaxy::imf::MassBand;
use crate::galaxy::{PointLy, Population};
use crate::id::{BodyId, CatalogueSystemId, SystemId};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::StarDraws;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{MIN_INITIAL_MASS, Track, TrackOptions, turn_off_mass};
use crate::stellar::system::track_mass_of;
use crate::stellar::{Composition, Phase, StarState};
use crate::tables::binary::{MERGER_SAMPLER, MergerMark, MergerPick};
use crate::time::{SourceHorizon, Span, UniverseTime};
use crate::units::{Dex, HeliumExcess, PerCubicLightYear, SolarMasses, Years};

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
    id: CatalogueSystemId,
    component: ComponentId,
    population: Population,
    position: GalacticPosition,
    velocity: GalacticVelocity,
    merger_time: UniverseTime,
    age_at_epoch: Years,
    composition: Composition,
    masses: [SolarMasses; 2],
    stars: [StarState; 2],
}

impl MergerScanMarks {
    /// The merger's ID: member 0 of its catalogue entry.
    #[must_use]
    pub const fn id(&self) -> CatalogueSystemId {
        self.id
    }

    /// The density component the merger was drawn from.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// The component's population.
    #[must_use]
    pub const fn population(&self) -> Population {
        self.population
    }

    /// Where the system is at the epoch: its site in its cell.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
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
        crate::galaxy::catalogue_classes::CatalogueMotion::straight(self.position, self.velocity)
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

    /// Each star's state at T on its own track, primary first, both on their main sequences.
    #[must_use]
    pub const fn stars(&self) -> &[StarState; 2] {
        &self.stars
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
    let seed = galaxy.seed();
    let system = SystemId::from(id);
    let key = ObjectKey::from(system);
    let sampler = ClassSampler::new(&MERGER_SAMPLER);
    let laws = galaxy.fields().component(component);
    // 2. T and the age.
    let mut stream = Stream::open(seed, tags::CLASS_MERGER_TIME, key);
    let merger_time = horizon_time(stream.uniform());
    let ages = laws.ages();
    let unborn = ages.cdf(Years::ZERO);
    let rank = sampler.draw(MergerMark::AgeRank, 0.0, &mut stream);
    let age_at_epoch = ages.quantile(unborn + (1.0 - unborn) * rank);
    let merger_age =
        Years::new(age_at_epoch.value() + merger_time.since_epoch().as_julian_years_f64());
    // 3. The composition, at the site and the age.
    let feh = laws.metallicity(&PointLy::from(&position), age_at_epoch);
    let mut stream = Stream::open(seed, tags::CLASS_MERGER_METALLICITY, key);
    let composition = Composition::from_fe_h(
        Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
        HeliumExcess::ZERO,
    );
    // 4. The pair: a layer with main-sequence stars at T, the primary below the turn-off then.
    let mut stream = Stream::open(seed, tags::CLASS_MERGER_MASS, key);
    let turn_off = turn_off_mass(Years::new(merger_age.value().max(0.0)), &composition).value();
    let floor = MIN_INITIAL_MASS.value();
    let shares = MERGER_SAMPLER.pick(MergerPick::Layer).weights();
    let mut weights = [0.0; 5];
    for ((weight, &share), band) in weights.iter_mut().zip(shares).zip(MassBand::ALL) {
        if band.lo().max(floor) < turn_off {
            *weight = share;
        }
    }
    let total = weights.iter().fold(0.0, |sum, w| sum + w);
    let mark = stream.mark();
    let fallback = weights.iter().rposition(|&w| w > 0.0)?;
    let layer = mark.pick_weighted(&weights, total).unwrap_or(fallback);
    let band = MassBand::ALL[layer];
    let (lo, hi) = (band.lo().max(floor), band.hi().min(turn_off));
    let rank = sampler.draw(MergerMark::PrimaryRank, 0.0, &mut stream);
    let m1 = galaxy.mass_function().quantile_in(lo, hi, rank);
    let q_floor = sampler.rank_of(MergerMark::MassRatio, 0.0, floor / m1);
    let q = sampler.draw_in_ranks(MergerMark::MassRatio, 0.0, (q_floor, 1.0), &mut stream);
    let m2 = (q * m1).clamp(floor, m1);
    let masses = [SolarMasses::new(m1), SolarMasses::new(m2)];
    // 5. Both stars on their main sequences at T.
    let rules = galaxy.rules().stellar;
    let star = |body: u16, mass: SolarMasses| {
        let draws = StarDraws::for_star(seed, BodyId::new(system, body));
        main_sequence_at(mass, &composition, &draws, rules, merger_age)
    };
    let stars = [star(0, masses[0])?, star(1, masses[1])?];
    // 6. The velocity.
    let velocity = ClassHosts::velocity(
        galaxy,
        component,
        &position,
        tags::CLASS_MERGER_VELOCITY,
        key,
    );
    Some(MergerScanMarks {
        id,
        component,
        population: laws.population(),
        position,
        velocity,
        merger_time,
        age_at_epoch,
        composition,
        masses,
        stars,
    })
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
    let bound = process.hosts().bound(galaxy, &key);
    let candidates =
        crate::galaxy::catalogue_classes::grid::candidate_count(galaxy.seed(), &key, bound);
    out.extend((0..candidates).filter_map(|index| {
        let id = key
            .candidate_id(index, 0)
            .expect("a count held under 2^24 numbers every candidate");
        merger_scan_marks(galaxy, process, &key, id, bound)
    }));
}
