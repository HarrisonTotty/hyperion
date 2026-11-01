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
//! ([`RECENT_DEATH_AGES`]), each with its own age distribution. From generator version 26 the
//! field gives the features their share (plan 09, P09.T2.c): the share is then the field's, the
//! share matrix's times the component's field factor, and the young disc's ages the young field's,
//! the budget's times `1 − φ(a)` renormalised, so the class follows the field's deaths alone.
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
//! `Some` and [`claims`] holds on [`DeathMarks::of`] its stars in the
//! field's surroundings, which is the one shared test (P08.T13's interval predicate is part of it;
//! main's ruling, 2026-10-15). A death that makes no shell, a direct collapse to a black hole or a
//! star that ends as a white dwarf, has no marks and is thinned like any other: the class keeps the
//! deaths the grid side would carve, and no other. The entry's window is the one the test cut
//! ([`claimed_window`]).
//!
//! # Runaways and walkaways, from 27
//!
//! A massive star of the young field may die away from where it was born: plan 08's runaways and
//! walkaways (`displaced::runaway`), ejected by an encounter in its young cluster or by its
//! companion's supernova, at the shares, speeds and ejection ages of `RunawayModel`. From
//! `RUNAWAY_DEATHS_VERSION` (27, P09.T33.b) the class follows them too, in the same cells and index
//! space:
//!
//! - **The field term** takes its component's band budget taken to the field, not the share the
//!   field keeps of its systems now (`ShareMatrix::component_share`'s stay share), and keeps a
//!   death only if its ejection marks say it was not ejected before it died
//!   (`class.core_collapse.ejection`): its kind by its mass's runaway and walkaway shares, then
//!   its ejection age among its kind's (the channel with it), judged at its source's reference
//!   lifetime, at which the class table counts its classes.
//! - **One term per band-E class of plan 08's runaways and walkaways** of each young component of
//!   the thin disc and the nuclear disc (`EjectedTerm`): the class of the ejection's speed bin and
//!   of the age bin of the time from the ejection to the death. Its density is the class's form
//!   (P08.T12.a's displaced fields) × the component's band-E systems taken to the field × the
//!   largest share of a star's deaths the class can take × the field's largest death rate; the
//!   site's mark picks a term as it picks a component. Its candidate draws its mass from the band,
//!   thinned on its class's share of that mass's deaths against the largest, then its kind and
//!   ejection age among the class's, its speed inside the class's bin and an azimuth; its
//!   \[Fe/H\] at its source's reference site, where plan 08's displaced records read theirs; its
//!   velocity by its class's law at the site, plan 08's (Design note 22), as the grid's members of
//!   the class move; then the field's death-rate thinning and the shared test.
//!
//! So the field's part of each star's deaths splits between the field term and the classes at its
//! mass exactly as the model's shares do: the field drops what the classes take, mass by mass, and
//! the class's ejected share is `RunawayModel`'s closed form. The features keep the rest of the
//! young disc's stars and list their deaths themselves (P09.T36.a), ejected or not, so nothing is
//! counted twice; where a feature's ejected members should die is the owner's (plan 09's Risks,
//! "P09.T33.b, as built"). The grid side already asks every layer-E record its claim (P09.T35), a
//! runaway's and a walkaway's with the rest.
//!
//! # The entry
//!
//! An entry has one member, its system: the progenitor, then the supernova, then the bare remnant,
//! by evaluation time ([`SupernovaEntry::state_at`]). It sits at its site at T, moves at its drawn
//! velocity before T and at that plus its primary's natal kick from T ([`CatalogueMotion`]); its
//! record's epoch position is where that puts it at the epoch. Its record is a catalogue system's
//! ([`SystemOrigin::CatalogueSystem`]), its stars plan 06's at its composition with the model's own
//! multiplicity.
//!
//! [`death_in_interval`]: crate::galaxy::displaced::death_in_interval

use std::cell::LazyCell;

use super::grid::{CatalogueMember, ClassProcess};
use super::ids::CatalogueCellKey;
use super::registry::ClassId;
use super::supernova::{
    DeathMarks, SupernovaEntry, SupernovaInterval, Surroundings, claimed_window, claims,
    lifetime_cap,
};
use super::system::{CatalogueMotion, CatalogueSystem};
use crate::GeneratorVersion;
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::ages::YOUNG_AGE_LIMIT;
use crate::galaxy::bounds::CellBox;
use crate::galaxy::displaced::class_table::{ClassKey, ClassTable, ejects};
use crate::galaxy::displaced::forms::{FormPoint, young_disc};
use crate::galaxy::displaced::marks::{MARK_MASS_NODES, age_bin_years};
use crate::galaxy::displaced::runaway::{
    Ejected, RUNAWAY_CUT_OF_V_C, RUNAWAY_MIN_SPEED, RUNAWAY_SHARE_HIGH, RunawayModel, WALKAWAY_CUT,
};
use crate::galaxy::displaced::{
    AgeBin, BirthSource, DisplacedClassId, DisplacedFields, DisplacedKind, ExplosionSite,
    SPEED_BINS, SPEED_EDGES, SpeedBin, death_in_interval_knowing,
};
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::imf::MassBand;
use crate::galaxy::kinematics::{BallisticKick, class_velocity, draw_on};
use crate::galaxy::placement::{SystemOrigin, SystemRecord, is_ballistic};
use crate::galaxy::query::epoch_velocity;
use crate::galaxy::snr::{ShellEnvironment, ShellWindow};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{BodyId, CatalogueSystemId, Layer, SystemId};
use crate::rng::{DomainTag, Mark, ObjectKey, Stream, Threshold, tags};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::sse::lifetime_under;
use crate::stellar::system::{SystemStars, track_mass_of};
use crate::time::{CLOCK_WINDOW_H, Span, UniverseTime};
use crate::units::{
    Dex, HeliumExcess, KilometresPerSecond, LightYears, PerCubicLightYear, SolarMasses, Years,
};

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

/// The generator version from which the core-collapse class follows the young field's runaways and
/// walkaways (plan 09, P09.T33.b): the field term at its band budget keeps the deaths its ejection
/// marks leave it, and one term per band-E runaway and walkaway class of the thin disc and the
/// nuclear disc places the rest (module documentation).
///
/// Below it the class takes the held arm, [`RunawayDeathsRules::FieldOnly`], the code of version 26
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `GalaxyRules::every_27_hold()`. The bump to 27 deletes this hold: [`RunawayDeathsRules`] with
/// its `FieldOnly` arm, the galaxy rules' `runaway_deaths` field, the match on it in
/// [`CoreCollapseProcess::new`] (whose terms `candidate` and `bound` read), and the tripwire
/// (`the_runaway_deaths_are_held_only_until_the_27_bump`).
pub(crate) const RUNAWAY_DEATHS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether the core-collapse class follows the young field's runaways and walkaways (the galaxy
/// rules' `runaway_deaths` field, the hold of [`RUNAWAY_DEATHS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum RunawayDeathsRules {
    /// Version 26's and before: the field's deaths alone, at the share its field keeps of each
    /// component's systems (P09.T33.a).
    FieldOnly,
    /// Version 27's: the field's deaths not ejected before they die, at its band budget, and the
    /// runaways' and walkaways' deaths, by class (P09.T33.b). A galaxy without plan 08's displaced
    /// classes has no runaways to follow and takes the held arm.
    Ejected,
}

/// One of the runaway and walkaway terms the class follows from 27 (plan 09, P09.T33.b): plan 08's
/// band-E displaced class `class`, of a young component's source, whose density is its form, and
/// the deaths of that component's field stars ejected into it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EjectedTerm {
    /// The displaced class whose form the term's deaths follow.
    class: DisplacedClassId,
    /// The young component the term's stars were born in.
    component: ComponentId,
    /// The component's source, whose classes, reference lifetimes and speed scale it reads.
    source: BirthSource,
    /// The class's speed bin: the ejection's.
    speed: SpeedBin,
    /// The class's age bin, the time from the ejection to the death, for a thin-disc class;
    /// `None` for a nuclear-disc class, which has speed bins only.
    age: Option<AgeBin>,
    /// The component's band-E systems taken to the field ([`Galaxy::field_part`]): the features
    /// keep the rest of its young stars and list their deaths themselves (P09.T36.a).
    systems: f64,
    /// An upper bound, over every mass, on the share of a band-E star's deaths that the class
    /// takes ([`largest_ejected_share`]), raised by 2⁻⁴⁰. The candidate thins its mass against it.
    largest_share: f64,
    /// The bound on the component's field's deaths per system over the interval, the field term's
    /// own ([`YoungComponent::deaths_per_system`]).
    deaths_per_system: f64,
}

impl EjectedTerm {
    /// The term's weight at the point `at`, recent deaths per cubic light-year: its systems × its
    /// class's form there × its largest share × its deaths per system.
    #[must_use]
    fn weight_at(&self, displaced: &DisplacedFields, at: &FormPoint<'_>) -> f64 {
        self.systems
            * displaced.density_at(self.class, at)
            * self.largest_share
            * self.deaths_per_system
    }
}

/// How a runaway's or a walkaway's candidate was ejected (plan 09, P09.T33.b): its class, its kind,
/// its ejection age at its source's reference lifetime and its ejection speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ejection {
    class: DisplacedClassId,
    kind: DisplacedKind,
    age: Years,
    speed: KilometresPerSecond,
}

impl Ejection {
    /// The displaced class of the ejection's speed bin and the time from it to the death.
    #[must_use]
    pub const fn class(&self) -> DisplacedClassId {
        self.class
    }

    /// A runaway or a walkaway.
    #[must_use]
    pub const fn kind(&self) -> DisplacedKind {
        self.kind
    }

    /// The star's age at its ejection, years, as the class table weighs it.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// The ejection's speed, inside the class's speed bin: plan 08's law of its kind (Design note
    /// 24), which the class's velocity reads in its ballistic bins.
    #[must_use]
    pub const fn speed(&self) -> KilometresPerSecond {
        self.speed
    }
}

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
/// Built from the galaxy alone each time the production lookup visits the class: it holds the
/// young field's components, with their layer-E shares and largest death rates, and the field's
/// thinning interval; from 27 (P09.T33.b) also the runaway and walkaway terms, whose bounds read
/// the class table's ejection ages at its 33 band-E mass nodes, some two hundred lookups.
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
    /// From 27 (P09.T33.b), the runaway and walkaway terms, in the components' order and then the
    /// classes'; empty under the held arm, and wherever the new arm finds no term to place them.
    /// The candidates read whether it is empty, so that the field drops a death only where a term
    /// takes it.
    ejected: Vec<EjectedTerm>,
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
    /// The primary's lifetime under the galaxy's stellar rules, which the shared test's exact tier
    /// reads rather than computing it again.
    lifetime: Years,
    age_at_epoch: Years,
    velocity: GalacticVelocity,
    ejection: Option<Ejection>,
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
    ejection: Option<Ejection>,
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

    /// How the progenitor was ejected, for a runaway's or a walkaway's death (from 27, P09.T33.b);
    /// `None` for a death in the field.
    #[must_use]
    pub const fn ejection(&self) -> Option<&Ejection> {
        self.ejection.as_ref()
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
    /// field's interval; from 27 the runaway and walkaway terms, and the field's components at
    /// their band budgets (P09.T33.b).
    ///
    /// # Panics
    ///
    /// Never: the field's window cap is at most `SHELL_WINDOW_CAP`, 4 Myr, whose interval is a
    /// few 10¹⁴ s.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        let fields = galaxy.fields();
        let interval = SupernovaInterval::of_cap(galaxy.window_caps(), ShellEnvironment::Field);
        let interval_years = interval.length().as_julian_years_f64();
        let floor = Years::new(-CLOCK_WINDOW_H.as_julian_years_f64());
        let held: Vec<YoungComponent> = fields
            .component_ids()
            .filter_map(|id| {
                // The field's share and ages: the budget's until P09.T2.c's version 26.
                let share = galaxy.field_component_share(MassBand::E, id);
                let ages = galaxy.field_ages(id);
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
        let displaced = match galaxy.rules().runaway_deaths {
            RunawayDeathsRules::FieldOnly => None,
            RunawayDeathsRules::Ejected => fields.displaced(),
        };
        let ejected = displaced.map_or_else(Vec::new, |displaced| {
            ejected_terms(galaxy, displaced, &held)
        });
        let ejecting = !ejected.is_empty();
        // From 27 the field's share is its band budget's taken to the field, not what the field
        // keeps of the component's systems now: the ejection marks thin its deaths instead.
        let components = if ejecting {
            held.into_iter()
                .map(|young| YoungComponent {
                    share: galaxy.field_part(
                        young.id,
                        MassBand::E,
                        galaxy
                            .shares()
                            .share(MassBand::E, fields.component(young.id).population()),
                    ),
                    ..young
                })
                .collect()
        } else {
            held
        };
        Self {
            components,
            interval,
            ejected,
        }
    }

    /// The bytes the process owns on the heap: its components and terms.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.components.capacity() * size_of::<YoungComponent>()
            + self.ejected.capacity() * size_of::<EjectedTerm>()
    }

    /// The runaway and walkaway terms the class follows: none under the held arm (from 27,
    /// P09.T33.b; module documentation).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn ejected_terms(&self) -> &[EjectedTerm] {
        &self.ejected
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

    /// The galaxy's displaced classes where the process follows their runaways and walkaways: where
    /// it holds a term.
    #[must_use]
    fn ejecting<'g>(&self, galaxy: &'g Galaxy) -> Option<&'g DisplacedFields> {
        if self.ejected.is_empty() {
            None
        } else {
            galaxy.fields().displaced()
        }
    }
}

/// The index `mark` picks among `weights` against `bound`, as [`Mark::pick_weighted`] picks over a
/// slice: the first whose running total's ratio to the bound lies above the mark, or `None` past
/// them all. Here each weight is computed only as the pick reaches it, so a term's form is read
/// only when no earlier weight took the mark.
#[must_use]
fn pick_lazily(mark: Mark, weights: impl IntoIterator<Item = f64>, bound: f64) -> Option<usize> {
    let mut sum = 0.0;
    for (index, weight) in weights.into_iter().enumerate() {
        sum += weight;
        if mark.is_below(Threshold::from_ratio(sum, bound)) {
            return Some(index);
        }
    }
    None
}

/// A source's band-E mass nodes as the terms' bound reads them (P09.T33.b): the class table's
/// masses, their reference lifetimes, and each kind's ejection ages at each node.
struct SourceNodes {
    masses: [f64; MARK_MASS_NODES],
    lifetimes: [f64; MARK_MASS_NODES],
    ages: Vec<[Vec<(Years, f64)>; 2]>,
}

impl SourceNodes {
    /// `source`'s nodes in `table`.
    #[must_use]
    fn of(table: &ClassTable, source: BirthSource) -> Self {
        let masses = *table.nodes(MassBand::E).masses();
        let lifetimes = masses.map(|m| {
            table
                .reference_lifetime(source, MassBand::E, SolarMasses::new(m))
                .value()
        });
        let ages = masses
            .iter()
            .map(|&m| KINDS.map(|kind| table.ejection_ages(source, kind, SolarMasses::new(m))))
            .collect();
        Self {
            masses,
            lifetimes,
            ages,
        }
    }
}

/// The runaway and walkaway terms of `galaxy`'s young field `components` (P09.T33.b): for each
/// young component of a source that ejects (the thin disc and the nuclear disc), in the
/// components' order, each of its source's classes with a form that can take a share of some
/// band-E star's deaths, in the classes' order; at the component's band-E systems taken to the
/// field, the field's largest death rate, and the class's largest share of a star's deaths.
#[must_use]
fn ejected_terms(
    galaxy: &Galaxy,
    displaced: &DisplacedFields,
    components: &[YoungComponent],
) -> Vec<EjectedTerm> {
    let table = displaced.class_table();
    let model = table.runaway_model();
    let fields = galaxy.fields();
    let mut terms = Vec::new();
    for young in components {
        let source = table.component_source(young.id);
        if !ejects(source) {
            continue;
        }
        let component = fields.component(young.id);
        let systems = galaxy.field_part(
            young.id,
            MassBand::E,
            component.count() * galaxy.shares().share(MassBand::E, component.population()),
        );
        if systems <= 0.0 {
            continue;
        }
        let nodes = SourceNodes::of(table, source);
        let speeds = KINDS.map(|kind| model.speed_bins(kind, table.speed_scale(source)));
        let tau_unit = table.scales().tau_unit();
        for (index, class) in displaced.classes().iter().enumerate() {
            let (speed, age) = match class.key() {
                ClassKey::Thin { speed, age } if source == BirthSource::ThinDisc => {
                    (speed, Some(age))
                }
                ClassKey::Old { source: old, speed } if old.source() == source => (speed, None),
                ClassKey::Thin { .. } | ClassKey::Old { .. } | ClassKey::Hypervelocity => continue,
            };
            if class.form().is_none() {
                continue;
            }
            let largest_share = largest_ejected_share(
                &nodes,
                model,
                (speed, age.map(|age| age_bin_years(age, tau_unit))),
                &speeds,
            ) * (1.0 + RATE_MARGIN);
            if largest_share <= 0.0 {
                continue;
            }
            terms.push(EjectedTerm {
                class: DisplacedClassId::new(u16::try_from(index).expect("under 97 classes")),
                component: young.id,
                source,
                speed,
                age,
                systems,
                largest_share,
                deaths_per_system: young.deaths_per_system,
            });
        }
    }
    terms
}

/// The two kinds of an ejected star, in the order the terms sum them.
const KINDS: [Ejected; 2] = [Ejected::Runaway, Ejected::Walkaway];

/// The most ways a star of a given mass can die in one term ([`ejected_parts`]): every ejection
/// age of both kinds, a runaway's two 16-node rules and a walkaway's one.
const MAX_PARTS: usize = 48;

/// An upper bound, over band E's masses, on the share of a star's deaths that the class of speed
/// bin `speed` and, for a thin-disc class, the age bin spanning `bin` (years) takes,
/// [`ejected_parts`]' sum, its kinds' shares in the speed bins `speeds`, at its source's `nodes`
/// (P09.T33.b).
///
/// Between two of the class table's mass nodes a star's reference lifetime is linear in log mass,
/// so its range there is its ends'. Where the nodes' lifetimes do not rise with mass, an ejection
/// age (an encounter's, fixed, or the lifetime of a companion heavier in a fixed ratio) does not
/// rise either, so its range is its ends' too, and the time from it to the death lies between
/// those ends' least and greatest. On each interval the bound is then each kind's lifetime share
/// at the heavier end (neither share falls with mass), times its share in the speed bin, times the
/// weights of the ejection ages that can fall before the death with that time in the age bin; and
/// the largest over the intervals bounds every mass. Should the lifetimes rise anywhere, every
/// ejection age counts, each kind's largest share at any mass.
#[must_use]
fn largest_ejected_share(
    nodes: &SourceNodes,
    model: RunawayModel,
    (speed, bin): (SpeedBin, Option<(Years, Option<Years>)>),
    speeds: &[[f64; SPEED_BINS]; 2],
) -> f64 {
    if nodes.lifetimes.windows(2).any(|w| w[1] > w[0]) {
        return KINDS.iter().zip(speeds).fold(0.0, |sum, (&kind, bins)| {
            sum + largest_lifetime_share(model, kind) * bins[speed.index()]
        });
    }
    let mut largest = 0.0_f64;
    for i in 0..MARK_MASS_NODES - 1 {
        let heavy = SolarMasses::new(nodes.masses[i + 1]);
        let (shortest, longest) = (nodes.lifetimes[i + 1], nodes.lifetimes[i]);
        let mut interval = 0.0;
        for (k, (&kind, bins)) in KINDS.iter().zip(speeds).enumerate() {
            let share = lifetime_share(model, kind, heavy) * bins[speed.index()];
            if share <= 0.0 {
                continue;
            }
            let weight = nodes.ages[i][k]
                .iter()
                .zip(&nodes.ages[i + 1][k])
                .filter(|((early, _), (late, _))| {
                    let earliest = early.value().min(late.value());
                    let latest = early.value().max(late.value());
                    let (soonest, slowest) = (shortest - latest, longest - earliest);
                    longest > earliest
                        && bin.is_none_or(|(lo, hi)| {
                            slowest >= lo.value() && hi.is_none_or(|hi| soonest < hi.value())
                        })
                })
                .fold(0.0, |sum, ((_, w), _)| sum + w);
            interval += share * weight;
        }
        largest = largest.max(interval);
    }
    largest
}

/// The largest lifetime share of `kind` at any mass: the runaways' [`RUNAWAY_SHARE_HIGH`], from
/// 20 M☉, and the walkaways' scale W, its ramp being at most 1.
#[must_use]
fn largest_lifetime_share(model: RunawayModel, kind: Ejected) -> f64 {
    match kind {
        Ejected::Runaway => RUNAWAY_SHARE_HIGH,
        Ejected::Walkaway => model.walkaway_scale(),
    }
}

/// The lifetime share of a star of initial mass `m` that becomes `kind`.
#[must_use]
fn lifetime_share(model: RunawayModel, kind: Ejected, m: SolarMasses) -> f64 {
    match kind {
        Ejected::Runaway => model.runaway_share(m),
        Ejected::Walkaway => model.walkaway_share(m),
    }
}

/// One way a star of a given mass dies in a runaway's or walkaway's term: its kind, its ejection
/// age and the share of the star's deaths it stands for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EjectedPart {
    kind: Ejected,
    age: Years,
    share: f64,
}

/// No part: a buffer's filler.
const NO_PART: EjectedPart = EjectedPart {
    kind: Ejected::Runaway,
    age: Years::ZERO,
    share: 0.0,
};

/// The share of a band-E star of initial mass `m`'s deaths that `term`'s class takes, by kind and
/// ejection age into the first parts of `parts`, with their number and sum (P09.T33.b): for each
/// kind and each of its ejection ages (the class table's `ejection_ages`, both channels), the
/// kind's lifetime share × the ejection's weight × its speed law's share in the class's bin,
/// where the ejection comes before the star's death at its source's reference lifetime and, for a
/// thin-disc class, the time from it to the death lies in the class's age bin. Summed over a
/// source's classes it is the share of the star's deaths after an ejection, which the field
/// term's marks drop.
fn ejected_parts(
    table: &ClassTable,
    term: &EjectedTerm,
    m: SolarMasses,
    parts: &mut [EjectedPart; MAX_PARTS],
) -> (usize, f64) {
    let model = table.runaway_model();
    let life = table
        .reference_lifetime(term.source, MassBand::E, m)
        .value();
    let v_ref = table.speed_scale(term.source);
    let bin = term
        .age
        .map(|age| age_bin_years(age, table.scales().tau_unit()));
    let (mut count, mut total) = (0, 0.0);
    for kind in KINDS {
        let share = lifetime_share(model, kind, m);
        let speed = model.speed_bins(kind, v_ref)[term.speed.index()];
        if share <= 0.0 || speed <= 0.0 {
            continue;
        }
        for (age, weight) in table.ejection_ages(term.source, kind, m) {
            let since = life - age.value();
            let in_bin = bin.is_none_or(|(lo, hi)| {
                lo.value() <= since && hi.is_none_or(|hi| since < hi.value())
            });
            if age.value() >= life || !in_bin {
                continue;
            }
            // Both kinds' ages number 48 at most, so every part has its slot.
            let Some(slot) = parts.get_mut(count) else {
                debug_assert!(false, "more than {MAX_PARTS} ejection ages at {m:?}");
                continue;
            };
            let part = share * weight * speed;
            total += part;
            *slot = EjectedPart {
                kind,
                age,
                share: part,
            };
            count += 1;
        }
    }
    (count, total)
}

/// Whether a field candidate of `source`, of initial mass `m`, was ejected before it died, by its
/// ejection marks on `stream` (`class.core_collapse.ejection`; P09.T33.b): word 0 picks a runaway,
/// a walkaway or neither by the mass's lifetime shares; for a runaway or a walkaway, word 1 picks
/// its ejection age among its kind's by weight (an encounter's or a supernova's, the channel with
/// it); and it was ejected before its death if that age is below its source's reference lifetime
/// at the mass, as the class table weighs its classes. A star of a source that ejects none draws
/// nothing.
#[must_use]
fn ejected_before_death(
    table: &ClassTable,
    source: BirthSource,
    m: SolarMasses,
    stream: &mut Stream,
) -> bool {
    if !ejects(source) {
        return false;
    }
    let model = table.runaway_model();
    let shares = KINDS.map(|kind| lifetime_share(model, kind, m));
    let odds = [shares[0], shares[1], (1.0 - shares[0] - shares[1]).max(0.0)];
    let total = odds[0] + odds[1] + odds[2];
    let kind = match stream.mark().pick_weighted(&odds, total) {
        Some(0) => Ejected::Runaway,
        Some(1) => Ejected::Walkaway,
        Some(_) | None => return false,
    };
    let ages = table.ejection_ages(source, kind, m);
    let total = ages.iter().fold(0.0, |sum, &(_, w)| sum + w);
    let Some(last) = ages.iter().rposition(|&(_, w)| w > 0.0) else {
        return false;
    };
    let i = pick_lazily(stream.mark(), ages.iter().map(|&(_, w)| w), total).unwrap_or(last);
    ages[i].0 < table.reference_lifetime(source, MassBand::E, m)
}

/// The speed of `kind`'s law, km/s, at rank `u` in [0, 1) inside speed bin `bin` of a source of
/// circular speed `v_ref`: the law's inverse CDF restricted to the bin's part of its support, by
/// 60 bisections; the bin's part's lower end where the law puts nothing in it.
#[must_use]
fn speed_in_bin(
    model: RunawayModel,
    kind: Ejected,
    bin: SpeedBin,
    v_ref: KilometresPerSecond,
    u: f64,
) -> KilometresPerSecond {
    let reference = v_ref.value();
    let (law_lo, law_hi) = match kind {
        Ejected::Runaway => (RUNAWAY_MIN_SPEED.value(), RUNAWAY_CUT_OF_V_C * reference),
        Ejected::Walkaway => (0.0, WALKAWAY_CUT.value()),
    };
    let index = bin.index();
    let lo = if index == 0 {
        0.0
    } else {
        SPEED_EDGES[index - 1] * reference
    };
    let hi = SPEED_EDGES
        .get(index)
        .map_or(f64::INFINITY, |e| e * reference);
    let (a, b) = (lo.max(law_lo), hi.min(law_hi));
    if b <= a {
        return KilometresPerSecond::new(a);
    }
    let cdf = |v: f64| model.speed_cdf(kind, KilometresPerSecond::new(v), v_ref);
    let (low, high) = (cdf(a), cdf(b));
    let target = low + u.clamp(0.0, 1.0) * (high - low);
    let (mut lo, mut hi) = (a, b);
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if cdf(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    KilometresPerSecond::new(f64::midpoint(lo, hi))
}

impl ClassProcess for CoreCollapseProcess {
    type Candidate = CoreCollapseCandidate;
    type Entry = CoreCollapseEntry;

    fn class(&self) -> ClassId {
        ClassId::CORE_COLLAPSE
    }

    /// Σ, over the young field's components in order, its layer-E share × its density bound over
    /// the cell's closed box ([`Component::bound`](crate::galaxy::fields::Component::bound)) × its
    /// deaths per system, and from 27 (P09.T33.b) then over the runaway and walkaway terms in
    /// order, each one's systems × its class's form bound over the box (plan 08's P08.T11) × its
    /// largest share × its deaths per system: each term bounds the matching weight of the site's
    /// thinning, in the same arithmetic, so their sum bounds the weights' running total.
    ///
    /// # Panics
    ///
    /// Never: a class cell is a cell box of the root cube that straddles no plane.
    fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let cube = CellBox::new(cell.origin_ly(), cell.edge_ly())
            .expect("a class cell is a cell box of the root cube");
        let fields = galaxy.fields();
        let field = self.components.iter().fold(0.0, |sum, young| {
            sum + young.share * fields.component_bound(young.id, &cube) * young.deaths_per_system
        });
        // From 27, the runaway and walkaway terms after the components, each its class's form
        // bound over the cell in the terms' weights' arithmetic (P09.T33.b).
        let total = self.ejecting(galaxy).map_or(field, |displaced| {
            self.ejected.iter().fold(field, |sum, term| {
                sum + term.systems
                    * displaced.bound(term.class, &cube)
                    * term.largest_share
                    * term.deaths_per_system
            })
        });
        PerCubicLightYear::new(total)
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
        let mark = stream.mark();
        let displaced = self.ejecting(galaxy);
        let picked = match displaced {
            None => mark.pick_weighted(&weights[..n], bound.value())?,
            // From 27 the same mark picks a runaway or walkaway term after the components. The
            // terms' forms read the point's radius and arm point, found once, when the pick first
            // reaches a term, the same bits as each form's own: a rejected candidate reads every
            // term, and at the centre's cell this cut a candidate's pick from 2.2 to 1.5 µs
            // (release, pod, 16 vCPU; P09.T33.b's cost).
            Some(displaced) => {
                let at = LazyCell::new(|| {
                    FormPoint::new(&point, Some(young_disc(galaxy.fields()).1.geometry()))
                });
                pick_lazily(
                    mark,
                    weights[..n].iter().copied().chain(
                        self.ejected
                            .iter()
                            .map(|term| term.weight_at(displaced, &at)),
                    ),
                    bound.value(),
                )?
            }
        };
        if let (Some(displaced), Some(term)) = (displaced, picked.checked_sub(n)) {
            return self.ejected_candidate(galaxy, displaced, (id, site), &self.ejected[term]);
        }
        let young = self.components[picked];
        let component = young.id;
        let laws = galaxy.fields().component(component);
        // 2. The primary's initial mass and its [Fe/H].
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_MASS, key);
        let mass = SolarMasses::new(
            galaxy
                .mass_function()
                .sample_in_band(MassBand::E, &mut stream),
        );
        // From 27 its ejection marks: a death after an ejection is a runaway's or a walkaway's
        // term's, not the field's (P09.T33.b).
        if let Some(displaced) = displaced {
            let table = displaced.class_table();
            let mut ejection = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_EJECTION, key);
            if ejected_before_death(
                table,
                table.component_source(component),
                mass,
                &mut ejection,
            ) {
                return None;
            }
        }
        let feh = laws.metallicity(&point, Years::ZERO);
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_METALLICITY, key);
        let composition = Composition::from_fe_h(
            Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
            HeliumExcess::ZERO,
        );
        // 3. The primary's draws, at attempt 0 as its record reads them, and its lifetime, as plan
        //    08's predicate's exact tier computes it, under the galaxy's stellar rules.
        let draws = StarDraws::for_star(seed, BodyId::new(system, 0));
        let lifetime = lifetime_under(
            track_mass_of(mass),
            &composition,
            &draws,
            galaxy.rules().stellar,
        );
        let life = lifetime.value();
        // 4. T across the field's interval, and the age at the epoch it implies.
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_DEATH, key);
        let death = self.death_at(stream.uniform());
        let age_at_epoch = Years::new(life - death.since_epoch().as_julian_years_f64());
        // 5. Thinned on the component's death rate at that age: its age density, as deaths per
        //    system over the interval, against the component's largest.
        let deaths = galaxy.field_ages(component).pdf(age_at_epoch)
            * self.interval.length().as_julian_years_f64();
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
            lifetime,
            age_at_epoch,
            velocity,
            ejection: None,
        })
    }

    /// Its death time T, read first (rule 10): word 0 of `class.core_collapse.death`, placed in
    /// the interval as the candidate's step 4 places it. Its entry's T is its primary's death at
    /// the age at the epoch that step sets, lifetime − T: the same time but for the rounding of
    /// lifetime − (lifetime − T) in years, a fraction of a second.
    fn event_time(&self, galaxy: &Galaxy, id: CatalogueSystemId) -> Option<UniverseTime> {
        let key = ObjectKey::from(SystemId::from(id));
        let mut stream = Stream::open(galaxy.seed(), tags::CLASS_CORE_COLLAPSE_DEATH, key);
        Some(self.death_at(stream.uniform()))
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
    /// The death of `candidate`'s primary, if plan 08's interval predicate
    /// ([`death_in_interval_under`](crate::galaxy::displaced::death_in_interval_under)) puts it in
    /// the interval, under the galaxy's stellar rules as its stars are, and it makes a shell
    /// ([`DeathMarks::of`]). Its exact tier reads the lifetime the candidate computed
    /// ([`death_in_interval_knowing`]), the same bits, so that a kept candidate pays for one
    /// lifetime, not two.
    #[must_use]
    fn of(galaxy: &Galaxy, candidate: &CoreCollapseCandidate) -> Option<Self> {
        // The primary, on a record at the site: it reads no position, and the record the entry
        // keeps is moved to the epoch once the kick is known. Its companions are no members of the
        // entry and are not read (`DeathMarks::of`), so none is drawn here.
        Self::of_record(galaxy, candidate, &candidate.record(candidate.site))
    }

    /// [`RecentDeath::of`] with the primary's stars drawn on `record`'s, the candidate's own at
    /// its site in production; a test passes a grid record whose marks the candidate carries.
    #[must_use]
    fn of_record(
        galaxy: &Galaxy,
        candidate: &CoreCollapseCandidate,
        record: &SystemRecord,
    ) -> Option<Self> {
        let draws = &candidate.draws;
        let death = death_in_interval_knowing(
            candidate.mass,
            &candidate.composition,
            draws.eta(),
            candidate.age_at_epoch,
            || draws.clone(),
            galaxy.rules().stellar,
            candidate.lifetime,
        )?;
        let stars = SystemStars::generate_with(
            galaxy,
            record,
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
            ejection: candidate.ejection,
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
    /// Candidate `id`'s marks at its `site` as a death of the runaway or walkaway term `term` of
    /// `displaced`'s classes, after the site's mark picked it (from 27, P09.T33.b; module
    /// documentation), on the field candidate's streams and `class.core_collapse.ejection`:
    ///
    /// 2. its primary's initial mass in band E, thinned on the class's share of that mass's deaths
    ///    against the term's largest (the mass stream's words 0 and 1); then its kind and ejection
    ///    age among the class's by their shares (ejection word 0), its speed inside the class's bin
    ///    (word 1) and an azimuth (word 2); and its \[Fe/H\] from its component's law at its
    ///    source's reference site, at age zero;
    /// 3. its primary's draws and lifetime, as a field candidate's;
    /// 4. T across the interval, and its age at the epoch;
    /// 5. thinned on its component's field's age density at that age, as a field candidate is;
    /// 6. its velocity at the site by its class's law, plan 08's (Design note 22): in a ballistic
    ///    bin the circular velocity plus the ejection, its vertical part its height over the time
    ///    since the ejection at its reference lifetime; otherwise the class's Gaussian, on
    ///    `class.core_collapse.velocity`.
    #[must_use]
    fn ejected_candidate(
        &self,
        galaxy: &Galaxy,
        displaced: &DisplacedFields,
        (id, site): (CatalogueSystemId, GalacticPosition),
        term: &EjectedTerm,
    ) -> Option<CoreCollapseCandidate> {
        let seed = galaxy.seed();
        let system = SystemId::from(id);
        let key = ObjectKey::from(system);
        let table = displaced.class_table();
        let component = term.component;
        let laws = galaxy.fields().component(component);
        // 2. The mass, thinned on the class's share of its deaths, and the ejection.
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_MASS, key);
        let mass = SolarMasses::new(
            galaxy
                .mass_function()
                .sample_in_band(MassBand::E, &mut stream),
        );
        let mut parts = [NO_PART; MAX_PARTS];
        let (count, share) = ejected_parts(table, term, mass, &mut parts);
        if !stream.decide(Threshold::from_ratio(share, term.largest_share)) {
            return None;
        }
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_EJECTION, key);
        let last = count.checked_sub(1)?;
        let picked = pick_lazily(
            stream.mark(),
            parts[..count].iter().map(|part| part.share),
            share,
        );
        let part = parts[picked.unwrap_or(last)];
        let ejection_speed = speed_in_bin(
            table.runaway_model(),
            part.kind,
            term.speed,
            table.speed_scale(term.source),
            stream.uniform(),
        );
        let azimuth = std::f64::consts::TAU * stream.uniform();
        let feh = laws.metallicity(&table.reference_site(term.source), Years::ZERO);
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_METALLICITY, key);
        let composition = Composition::from_fe_h(
            Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
            HeliumExcess::ZERO,
        );
        // 3. The primary's draws and lifetime.
        let draws = StarDraws::for_star(seed, BodyId::new(system, 0));
        let lifetime = lifetime_under(
            track_mass_of(mass),
            &composition,
            &draws,
            galaxy.rules().stellar,
        );
        let life = lifetime.value();
        // 4–5. T, the age, and the field's death rate there.
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_DEATH, key);
        let death = self.death_at(stream.uniform());
        let age_at_epoch = Years::new(life - death.since_epoch().as_julian_years_f64());
        let deaths = galaxy.field_ages(component).pdf(age_at_epoch)
            * self.interval.length().as_julian_years_f64();
        if !stream.decide(Threshold::from_ratio(deaths, term.deaths_per_system)) {
            return None;
        }
        // 6. The one velocity, by the class's law at the site.
        let since = table
            .reference_lifetime(term.source, MassBand::E, mass)
            .value()
            - part.age.value();
        let ballistic = is_ballistic(displaced.class(term.class).key()).then(|| BallisticKick {
            height: LightYears::new(PointLy::from(&site).z),
            since: Years::new(since),
            speed: ejection_speed.value(),
            azimuth,
        });
        let velocity = ejected_velocity(galaxy, displaced, term.class, &site, ballistic, key);
        let kind = match part.kind {
            Ejected::Runaway => DisplacedKind::Runaway,
            Ejected::Walkaway => DisplacedKind::Walkaway,
        };
        Some(CoreCollapseCandidate {
            id,
            site,
            component,
            population: laws.population(),
            mass,
            composition,
            draws,
            lifetime,
            age_at_epoch,
            velocity,
            ejection: Some(Ejection {
                class: term.class,
                kind,
                age: part.age,
                speed: ejection_speed,
            }),
        })
    }

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

/// A runaway's or a walkaway's candidate's velocity at its death site `site`, galactic (from 27,
/// P09.T33.b): plan 08's law of its displaced class `class` (Design note 22,
/// `kinematics::class_velocity`), as the grid's members of the class move, with its `ballistic`
/// motion in the class's ballistic bins, on `class.core_collapse.velocity`; or zero in a galaxy
/// built without its kinematic tables, as a field candidate's is.
#[must_use]
fn ejected_velocity(
    galaxy: &Galaxy,
    displaced: &DisplacedFields,
    class: DisplacedClassId,
    site: &GalacticPosition,
    ballistic: Option<BallisticKick>,
    key: ObjectKey,
) -> GalacticVelocity {
    if galaxy.kinematics().is_none() {
        return GalacticVelocity::default();
    }
    let mut stream = Stream::open(galaxy.seed(), tags::CLASS_CORE_COLLAPSE_VELOCITY, key);
    class_velocity(
        galaxy,
        displaced,
        class,
        (site, Layer::E),
        ballistic,
        &mut stream,
    )
    .velocity()
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

/// Whether the core-collapse class claims the grid record `record`, whose primary dies at `site`
/// inside the shared test's interval (plan 09, P09.T35): the record's primary, alone as the
/// class's candidates are (`MultiplicityContext::ForcedSingle`), at its own metallicity draw, makes
/// a shell there ([`DeathMarks::of`], at the record's epoch velocity, plan 08's `epoch_velocity`),
/// and [`claims`] keeps it in the field's surroundings.
///
/// It is the class's test on the grid's side, the second half of the one shared test whose first
/// is the site's [`death_in_interval`]: a recent death is the catalogue's or the grid's and never
/// both (P09.T38's complementarity). The carve-out asks it of each layer-E record with a site under
/// its claimed arm (`displaced::site::recent_death_claim`), and P09.T43's
/// `the_class_s_core_collapse_claim_is_the_grid_s` holds the class's own claim equal to it.
///
/// # Panics
///
/// As [`SystemStars::generate_in`] and plan 08's `epoch_velocity`, for a record that is not a grid
/// record of `galaxy`; and in debug builds, as [`DeathMarks::of`], if `site`'s death is not the
/// primary's, and if the record moves fast enough to outrun the reach before its death
/// ([`lifetime_cap`]'s edge, about 4,649 km/s).
///
/// [`death_in_interval`]: crate::galaxy::displaced::death_in_interval
#[must_use]
pub(crate) fn recent_death_claims(
    galaxy: &Galaxy,
    site: &ExplosionSite,
    record: &SystemRecord,
) -> bool {
    let velocity = epoch_velocity(galaxy, record);
    // Nothing reads an entry before the source horizon, so its progenitor's line is read for at
    // most L + 2H before T, which only one above 4,096 ly ÷ (L + 2H), about 4,649 km/s, could
    // outrun the reach (`lifetime_cap`'s edge; plan 09, "P09.T33.a's leans, kept", `v_fast`).
    debug_assert!(
        lifetime_cap(velocity.speed()).value() >= 0.0,
        "a grid record of {:?} at {} m/s outruns the reach before its death",
        record.id(),
        velocity.speed().value()
    );
    let stars = SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle);
    DeathMarks::of(galaxy, &stars, site, velocity)
        .is_some_and(|marks| claims(galaxy, site, &marks, Surroundings::Field))
}

#[cfg(test)]
mod runaway_hold;
#[cfg(test)]
mod tests;
