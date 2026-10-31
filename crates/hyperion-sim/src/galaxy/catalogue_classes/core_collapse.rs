//! The core-collapse class: the field's recent deaths of massive stars, placed on 512 ly cells of
//! their own (plan 09, P09.T33.a).
//!
//! A layer-E system whose primary dies by core collapse within the supernova test's interval is a
//! catalogue entry, not a grid star (brainstorm, "Events in time": "Rare events are found by
//! carving out their hosts, not the events"). By the marking theorem those systems are a Poisson
//! process of their own, which this module places by thinning, as plan 03 places the grid, on the
//! class grid of P09.T32 ([`ClassProcess`]). The grid side's carve-out, which drops the same
//! systems from the cells, is P09.T35's.
//!
//! # What the class follows
//!
//! The grid's layer E places a system of component c at x with initial mass m in band E and age a
//! at the epoch at the intensity `s_c n_c(x) p(m) f_c(a)`: the component's density, its layer-E
//! share (plan 02's [`ShareMatrix`](crate::galaxy::shares::ShareMatrix)), the band's mass function
//! and the component's age distribution. Its primary dies at T = lifetime − a, so in T the same
//! intensity is `s_c n_c(x) p(m) f_c(lifetime − T)`: the death rate per system at a mass is the age
//! density at its lifetime. Only the components with stars young enough to die in the interval
//! contribute, the young field: the young thin disc and the nuclear disc's last gigayear
//! ([`RECENT_DEATH_AGES`]), each with its own age distribution, which carries the field's
//! (1 − φ(a)) factor once plan 09's P09.T2.c gives the field up to the features.
//!
//! # The envelope and a candidate
//!
//! A cell's bound is the envelope of that intensity, summed over the young field's components:
//! each one's layer-E density bound over the cell's closed box
//! ([`Component::bound`](crate::galaxy::fields::Component::bound)) × the field's thinning cap,
//! W + L + 2H ([`SupernovaInterval::thinning_cap`]) × its largest death rate per system, the
//! greatest age density it has below [`RECENT_DEATH_AGES`]. Each component carries its own rate
//! because the nuclear disc's, a twentieth of its stars spread over its last gigayear, is some two
//! hundred times below the young thin disc's: thinned against the young disc's, the cells about
//! the centre drew millions of candidates for a few dozen entries. A candidate, on its own
//! streams (Design note 23):
//!
//! 1. draws its site uniformly in the cell and is thinned on the young field's envelope there,
//!    each component's density × its largest death rate, ÷ the bound, the same mark picking its
//!    component by the odds of those terms, as plan 03's candidates pick theirs
//!    (`class.core_collapse.site`);
//! 2. draws its primary's initial mass from the galaxy's mass function in band E
//!    (`class.core_collapse.mass`) and its \[Fe/H\] from its component's law at the site
//!    (`class.core_collapse.metallicity`), read at age zero, since both young components' laws
//!    are flat in age (plan 02's `fields::metallicity`);
//! 3. takes its primary's draws, plan 06's [`StarDraws::for_star`] of its body 0, and their
//!    lifetime;
//! 4. draws T uniformly over the field's interval (−(cap + L + H), +H] and sets its age at the
//!    epoch to lifetime − T (`class.core_collapse.death`, word 0);
//! 5. is thinned on its component's age density at that age ÷ the component's largest (word 1);
//! 6. draws its velocity at the site from its component's law, plan 08's draw
//!    (`class.core_collapse.velocity`), the one velocity its test and its entry read (rule 6).
//!
//! The candidates kept are a Poisson process of intensity `s_c n_c(x) p(m) f_c(lifetime − T)` and
//! the composition, draws and velocity of the grid's systems: the grid's, exactly.
//!
//! # The test
//!
//! A candidate is an entry if and only if plan 08's
//! [`death_in_interval`] on its primary's marks is
//! `Some` and [`claims`](super::supernova::claims) holds on [`DeathMarks::of`] its stars in the
//! field's surroundings, which is the one shared test (P08.T13's interval predicate is part of it;
//! main's ruling, 2026-10-15). A death that makes no shell, a direct collapse to a black hole or a
//! star that ends as a white dwarf, has no marks and is thinned like any other: the class keeps the
//! deaths the grid side would carve, and no other. The entry's window is the one the test cut
//! ([`claimed_window`]).
//!
//! # The entry
//!
//! An entry has one member, its system: the progenitor, then the supernova, then the bare remnant,
//! by evaluation time ([`SupernovaEntry::state_at`]). It sits at its site at T, moves at its drawn
//! velocity before T and at that plus its primary's natal kick from T ([`CatalogueMotion`]); its
//! record's epoch position is where that puts it at the epoch. Its record is a catalogue system's
//! ([`SystemOrigin::CatalogueSystem`]), its stars plan 06's at its composition with the model's own
//! multiplicity.

use super::grid::{CatalogueMember, ClassProcess};
use super::ids::CatalogueCellKey;
use super::registry::ClassId;
use super::supernova::{
    DeathMarks, SupernovaEntry, SupernovaInterval, Surroundings, claimed_window,
};
use super::system::{CatalogueMotion, CatalogueSystem};
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::ages::YOUNG_AGE_LIMIT;
use crate::galaxy::bounds::CellBox;
use crate::galaxy::displaced::{ExplosionSite, death_in_interval};
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::imf::MassBand;
use crate::galaxy::kinematics::draw_on;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::snr::{ShellEnvironment, ShellWindow};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{BodyId, CatalogueSystemId, SystemId};
use crate::rng::{DomainTag, ObjectKey, Stream, Threshold, tags};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::{SystemStars, track_mass_of};
use crate::stellar::{Composition, lifetime};
use crate::time::{CLOCK_WINDOW_H, Span, UniverseTime};
use crate::units::{Dex, HeliumExcess, KilometresPerSecond, PerCubicLightYear, SolarMasses, Years};

/// The ages at the epoch below which a component's stars can die within the supernova test's
/// interval: 100 Myr, the young thin disc's oldest (plan 09, P09.T33.a).
///
/// A recent death of band E has an age at the epoch of its lifetime less T, at most the longest
/// lifetime in the band, an 8 M☉ star's, plus the interval, at most 4.26 Myr: some 50 Myr, inside
/// this (a test holds the mass bracket's longest lifetime at 8 M☉ under it). So the class follows
/// every component with ages below it, the young field, and the largest death rate it thins
/// against is the greatest age density any of them has below it. The old thin disc's youngest
/// sub-disc starts at it, so none of its stars dies recently.
pub const RECENT_DEATH_AGES: Years = YOUNG_AGE_LIMIT;

/// The relative margin a component's largest death rate carries above the age densities it
/// bounds: 2⁻⁴⁰, as plan 02's bounds carry ([`bounds`](crate::galaxy::bounds), "Floating point"),
/// so that a candidate's age density as computed never exceeds it.
const RATE_MARGIN: f64 = 1.0 / 1_099_511_627_776.0;

/// One of the young field's components, as the class thins on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YoungComponent {
    id: ComponentId,
    share: f64,
    deaths_per_system: f64,
}

impl YoungComponent {
    /// The component.
    #[must_use]
    pub const fn id(&self) -> ComponentId {
        self.id
    }

    /// Its layer-E share: the fraction of its systems whose primaries are of band E.
    #[must_use]
    pub const fn share(&self) -> f64 {
        self.share
    }

    /// The bound on its deaths per system over the interval: its largest age density below
    /// [`RECENT_DEATH_AGES`], per year, raised by 2⁻⁴⁰, × the interval's length in Julian years.
    #[must_use]
    pub const fn deaths_per_system(&self) -> f64 {
        self.deaths_per_system
    }
}

/// The core-collapse class's process (plan 09, P09.T33.a): the young field's recent deaths of
/// massive stars (module documentation).
///
/// Built from the galaxy alone, cheaply, each time the production lookup visits the class: it holds
/// the young field's components, with their layer-E shares and largest death rates, and the
/// field's thinning interval.
///
/// # Examples
///
/// The class's cells about the Sun-like point hold a few recent deaths, each resolving by its ID to
/// itself, its window under the field's cap:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
/// use hyperion_sim::galaxy::catalogue_classes::{
///     CatalogueCellKey, CatalogueClassCell, ClassId, resolve_entry,
/// };
/// use hyperion_sim::galaxy::snr::ShellEnvironment;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = CoreCollapseProcess::new(&galaxy);
/// let cap = galaxy.window_caps().cap(ShellEnvironment::Field);
/// for y in 48..52 {
///     let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, y, 0])?;
///     for entry in CatalogueClassCell::generate(&galaxy, &process, key).entries() {
///         assert_eq!(resolve_entry(&galaxy, &process, entry.id())?.entry(), entry);
///         assert!(entry.value().supernova().window().duration() <= cap);
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CoreCollapseProcess {
    /// The young field's components, in component order.
    components: Vec<YoungComponent>,
    /// The field's thinning interval, at the field's cap.
    interval: SupernovaInterval,
}

/// A core-collapse candidate the thinning kept, with every mark its test reads.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreCollapseCandidate {
    id: CatalogueSystemId,
    site: GalacticPosition,
    component: ComponentId,
    population: Population,
    mass: SolarMasses,
    composition: Composition,
    draws: StarDraws,
    age_at_epoch: Years,
    velocity: GalacticVelocity,
}

/// A core-collapse entry: a recent death of the young field (plan 09, P09.T33.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreCollapseEntry {
    record: SystemRecord,
    component: ComponentId,
    composition: Composition,
    site: ExplosionSite,
    motion: CatalogueMotion,
    marks: DeathMarks,
    supernova: SupernovaEntry,
}

impl CoreCollapseEntry {
    /// The record of the entry's system, of origin [`SystemOrigin::CatalogueSystem`], at its
    /// epoch position.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The young field's component the system was drawn from.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// The composition every star of the system shares.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Where and when the primary explodes.
    #[must_use]
    pub const fn site(&self) -> &ExplosionSite {
        &self.site
    }

    /// How the system moves: at its velocity until T, at that plus the kick from T.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// What the supernova test read of the death.
    #[must_use]
    pub const fn marks(&self) -> &DeathMarks {
        &self.marks
    }

    /// The supernova: its time, kind, window and remnant.
    #[must_use]
    pub const fn supernova(&self) -> &SupernovaEntry {
        &self.supernova
    }

    /// The system's stars: plan 06's at its composition, with the model's own multiplicity.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        SystemStars::generate_with(
            galaxy,
            &self.record,
            &self.composition,
            MultiplicityContext::Free,
        )
    }
}

impl CoreCollapseProcess {
    /// The process in `galaxy`: its young field's components, the largest death rate and the
    /// field's interval.
    ///
    /// # Panics
    ///
    /// Never: the field's window cap is at most `SHELL_WINDOW_CAP`, 4 Myr, whose interval is a
    /// few 10¹⁴ s.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        let fields = galaxy.fields();
        let shares = galaxy.shares();
        let interval = SupernovaInterval::of_cap(galaxy.window_caps(), ShellEnvironment::Field);
        let interval_years = interval.length().as_julian_years_f64();
        let floor = Years::new(-CLOCK_WINDOW_H.as_julian_years_f64());
        let components = fields
            .component_ids()
            .filter_map(|id| {
                let component = fields.component(id);
                let share = shares.component_share(MassBand::E, component);
                let ages = component.ages();
                let rate = ages.max_pdf_between(floor, RECENT_DEATH_AGES);
                // The old thin disc's youngest sub-disc starts at the limit, which the range's
                // closed top would reach: the limit itself is no young age.
                (share > 0.0 && ages.min() < RECENT_DEATH_AGES && rate > 0.0).then_some(
                    YoungComponent {
                        id,
                        share,
                        deaths_per_system: rate * (1.0 + RATE_MARGIN) * interval_years,
                    },
                )
            })
            .collect();
        Self {
            components,
            interval,
        }
    }

    /// The young field's components, in component order: those with layer-E systems and ages
    /// below [`RECENT_DEATH_AGES`].
    #[must_use]
    pub fn components(&self) -> &[YoungComponent] {
        &self.components
    }

    /// The interval over which candidates draw T: the field's, at its cap.
    #[must_use]
    pub const fn interval(&self) -> &SupernovaInterval {
        &self.interval
    }

    /// The envelope's terms at `point` into `out`, in the components' order, recent deaths per
    /// cubic light-year: each component's layer-E share × its density × its deaths per system.
    fn weights(&self, galaxy: &Galaxy, point: &PointLy, out: &mut [f64; MAX_COMPONENTS]) -> usize {
        let fields = galaxy.fields();
        for (slot, young) in out.iter_mut().zip(&self.components) {
            *slot =
                young.share * fields.component(young.id).density(point) * young.deaths_per_system;
        }
        self.components.len()
    }
}

impl ClassProcess for CoreCollapseProcess {
    type Candidate = CoreCollapseCandidate;
    type Entry = CoreCollapseEntry;

    fn class(&self) -> ClassId {
        ClassId::CORE_COLLAPSE
    }

    /// Σ, over the young field's components in order, its layer-E share × its density bound over
    /// the cell's closed box ([`Component::bound`](crate::galaxy::fields::Component::bound)) × its
    /// deaths per system: each term bounds the matching weight of the site's thinning, in the same
    /// arithmetic, so their sum bounds the weights' running total.
    ///
    /// # Panics
    ///
    /// Never: a class cell is a cell box of the root cube that straddles no plane.
    fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let cube = CellBox::new(cell.origin_ly(), cell.edge_ly())
            .expect("a class cell is a cell box of the root cube");
        let fields = galaxy.fields();
        PerCubicLightYear::new(self.components.iter().fold(0.0, |sum, young| {
            sum + young.share * fields.component_bound(young.id, &cube) * young.deaths_per_system
        }))
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<CoreCollapseCandidate> {
        let seed = galaxy.seed();
        let system = SystemId::from(id);
        let key = ObjectKey::from(system);
        // 1. The site, thinned on the envelope there, the mark picking the component.
        if bound.value().is_nan() || bound.value() <= 0.0 {
            return None;
        }
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_SITE, key);
        let site =
            cell.position_from_words([stream.next_u64(), stream.next_u64(), stream.next_u64()]);
        let point = PointLy::from(&site);
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, &point, &mut weights);
        let young = self.components[stream.mark().pick_weighted(&weights[..n], bound.value())?];
        let component = young.id;
        let laws = galaxy.fields().component(component);
        // 2. The primary's initial mass and its [Fe/H].
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_MASS, key);
        let mass = SolarMasses::new(
            galaxy
                .mass_function()
                .sample_in_band(MassBand::E, &mut stream),
        );
        let feh = laws.metallicity(&point, Years::ZERO);
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_METALLICITY, key);
        let composition = Composition::from_fe_h(
            Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
            HeliumExcess::ZERO,
        );
        // 3. The primary's draws, at attempt 0 as its record reads them, and its lifetime, as plan
        //    08's predicate's exact tier computes it.
        let draws = StarDraws::for_star(seed, BodyId::new(system, 0));
        let life = lifetime(track_mass_of(mass), &composition, &draws).value();
        // 4. T across the field's interval, and the age at the epoch it implies.
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_DEATH, key);
        let death = self.death_at(stream.uniform());
        let age_at_epoch = Years::new(life - death.since_epoch().as_julian_years_f64());
        // 5. Thinned on the component's death rate at that age: its age density, as deaths per
        //    system over the interval, against the component's largest.
        let deaths = laws.ages().pdf(age_at_epoch) * self.interval.length().as_julian_years_f64();
        if !stream.decide(Threshold::from_ratio(deaths, young.deaths_per_system)) {
            return None;
        }
        // 6. The one velocity.
        let velocity = site_velocity(galaxy, component, &site, key);
        Some(CoreCollapseCandidate {
            id,
            site,
            component,
            population: laws.population(),
            mass,
            composition,
            draws,
            age_at_epoch,
            velocity,
        })
    }

    fn test(&self, galaxy: &Galaxy, candidate: CoreCollapseCandidate) -> Option<CoreCollapseEntry> {
        let death = RecentDeath::of(galaxy, &candidate)?;
        let window = claimed_window(galaxy, &death.site, &death.marks, Surroundings::Field)?;
        Some(death.into_entry(&candidate, window))
    }

    fn members(&self, _: &CoreCollapseEntry) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<CoreCollapseEntry>) -> CatalogueSystem {
        let entry = member.entry().value();
        CatalogueSystem::new(entry.record, entry.motion)
            .with_stars(entry.composition, MultiplicityContext::Free)
    }
}

/// A candidate's death with a shell in the interval: the shared test's first half, plan 08's
/// interval predicate, and the marks its second half reads.
#[derive(Debug, Clone, PartialEq)]
struct RecentDeath {
    stars: SystemStars,
    site: ExplosionSite,
    marks: DeathMarks,
}

impl RecentDeath {
    /// The death of `candidate`'s primary, if plan 08's
    /// [`death_in_interval`] puts it in the interval
    /// and it makes a shell ([`DeathMarks::of`]).
    #[must_use]
    fn of(galaxy: &Galaxy, candidate: &CoreCollapseCandidate) -> Option<Self> {
        let draws = &candidate.draws;
        let death = death_in_interval(
            candidate.mass,
            &candidate.composition,
            draws.eta(),
            candidate.age_at_epoch,
            || draws.clone(),
        )?;
        // The primary, on a record at the site: it reads no position, and the record the entry
        // keeps is moved to the epoch once the kick is known. Its companions are no members of the
        // entry and are not read (`DeathMarks::of`), so none is drawn here.
        let at_site = candidate.record(candidate.site);
        let stars = SystemStars::generate_with(
            galaxy,
            &at_site,
            &candidate.composition,
            MultiplicityContext::ForcedSingle,
        );
        let site = ExplosionSite::new(candidate.site, death);
        let marks = DeathMarks::of(galaxy, &stars, &site, candidate.velocity)?;
        Some(Self { stars, site, marks })
    }

    /// The entry of `candidate`, whose death this is, with the shell `window` the test cut.
    #[must_use]
    fn into_entry(
        self,
        candidate: &CoreCollapseCandidate,
        window: ShellWindow,
    ) -> CoreCollapseEntry {
        let Self { stars, site, marks } = self;
        let death = site.death();
        let kick = stars.natal_kick();
        let remnant = kick.map_or(candidate.velocity, |kick| {
            candidate.velocity + GalacticVelocity::new(kick.velocity())
        });
        let motion = CatalogueMotion::broken(*site.position(), death, candidate.velocity, remnant);
        let record = candidate.record(motion.position_at(UniverseTime::EPOCH));
        let kick_speed = kick.map_or(KilometresPerSecond::ZERO, |kick| {
            KilometresPerSecond::from(kick.speed())
        });
        let supernova = SupernovaEntry::new(record.id(), death, marks.kind(), window)
            .with_remnant(kick_speed, stars.primary().neutron_star());
        CoreCollapseEntry {
            record,
            component: candidate.component,
            composition: candidate.composition,
            site,
            motion,
            marks,
            supernova,
        }
    }
}

impl CoreCollapseCandidate {
    /// The candidate's record with its system at `position` at the epoch.
    #[must_use]
    fn record(&self, position: GalacticPosition) -> SystemRecord {
        SystemRecord::from_parts(
            SystemId::from(self.id),
            position,
            SystemOrigin::CatalogueSystem(ClassId::CORE_COLLAPSE),
            self.population,
            self.mass,
            self.age_at_epoch,
        )
    }
}

impl CoreCollapseProcess {
    /// The death time a uniform `u` in [0, 1) places in the interval: +H less `u` × its length,
    /// floored to the nanosecond, so in (−(cap + L + H), +H].
    ///
    /// # Panics
    ///
    /// Never: the interval is a few 10¹⁴ s.
    #[must_use]
    fn death_at(&self, u: f64) -> UniverseTime {
        let back = Span::from_seconds_f64(u * self.interval.length().as_seconds_f64())
            .expect("a share of a 4.3 Myr interval is a span");
        self.interval
            .end()
            .checked_sub(back)
            .expect("the interval lies inside the clock's range")
    }
}

/// A candidate's velocity at its site, galactic: plan 08's draw of its component's law at the site
/// on `class.core_collapse.velocity` (rule 6), or zero in a galaxy built without its kinematic
/// tables, which has no velocities, as plan 08's `epoch_velocity` has it.
#[must_use]
fn site_velocity(
    galaxy: &Galaxy,
    component: ComponentId,
    site: &GalacticPosition,
    key: ObjectKey,
) -> GalacticVelocity {
    class_site_velocity(
        galaxy,
        component,
        site,
        tags::CLASS_CORE_COLLAPSE_VELOCITY,
        key,
    )
}

/// A class candidate's velocity at its site, galactic: the core collapses', the Type Ia's and the
/// luminous blue variables' one velocity (rule 6).
///
/// It is plan 08's draw of its `component`'s law at `site` on its class's velocity `tag` keyed by
/// `key`, or zero in a galaxy built without its kinematic tables, as plan 08's `epoch_velocity`
/// has it.
#[must_use]
pub(super) fn class_site_velocity(
    galaxy: &Galaxy,
    component: ComponentId,
    site: &GalacticPosition,
    tag: DomainTag,
    key: ObjectKey,
) -> GalacticVelocity {
    if galaxy.kinematics().is_none() {
        return GalacticVelocity::default();
    }
    let mut stream = Stream::open(galaxy.seed(), tag, key);
    draw_on(galaxy, component, site, &mut stream).velocity()
}

#[cfg(test)]
mod tests;
