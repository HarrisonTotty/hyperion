//! The recently dead and the luminous blue variables among a catalogue feature's members, on its
//! feature-level list (plan 09, P09.T36.a; the globulars' Type Ia of P09.T36.b; the luminous blue
//! variables' feature half, P09.T43), from generator version 27.
//!
//! # The classes
//!
//! A nursery (a star-forming region, a young cluster or an association) and an old open cluster
//! list, in Design note 16's order:
//!
//! - **`CORE_COLLAPSE`**: band-E members whose primary dies in the supernova test's interval and
//!   makes a shell that the test keeps. The candidates' expected count is a closed form in the
//!   feature's band-E systems, its members' ages and the interval at the longest cap its deaths can
//!   take, the bubble's or, for a site outside its wall, the field's: the band-E systems whose
//!   initial mass and age lie in the envelope (`Candidates`) of those a lifetime bracket
//!   ([`LifetimeBracket::shared`]) can put a death of in the interval. A candidate draws its mass
//!   in the envelope by the galaxy's mass function and its age uniformly, and the one shared test
//!   decides it: plan 08's [`death_in_interval`] on its own marks under its galaxy's stellar rules,
//!   as its stars are (`death_in_interval_under`), then [`DeathMarks::of`] and
//!   [`claims`] in the feature's surroundings, the nursery's superbubble at the death
//!   ([`ShellEnvironment::Bubble`], the window ended at the wall) where the site lies in one, the
//!   field's otherwise. From generator version 27 (P09.T33.b) a
//!   candidate whose ejection marks put a runaway's ejection before its death is no entry: a
//!   runaway dies some 1,900 ly from its birth cluster, in the field, where the class's runaway
//!   terms place it at the whole budget, while a walkaway dies inside its feature's bubble and
//!   stays listed (lean, 2026-11-01; research-leans-21). The drop is a thinning of the
//!   candidates, whose expected count is unchanged.
//! - **`TYPE_IA`**: the Type Ia of the feature's members, exploding in the interval, drawn delay
//!   first as the field's class draws them ([`IaProgenitor::draw_in`]) on a delay-time distribution
//!   of the feature's own ages, their window the shared test's in the feature's surroundings, in
//!   the Type Ia's smooth gas outside a bubble.
//! - **`LUMINOUS_BLUE_VARIABLE`**, after `TYPE_IA`: band-E members whose window near the
//!   Humphreys–Davidson limit overlaps the source horizon and that the core collapses do not
//!   claim, the class's test ([`may_host`], [`horizon_window`]) on their own marks. It is listed
//!   where some of the sampler's nodes may host at the members' ages: on the young features.
//!
//! A globular lists only its Type Ia (P09.T36.b): hosts drawn from the delay-time distribution on
//! its mass, listed with no window, since a globular's gas makes no shell, so exploding within
//! the source horizon, (−(L + H), +H], where a host with no window can be seen. It lists no core
//! collapse: its stars are ten gigayears old.
//!
//! # The cells' half
//!
//! A feature whose members are placed in cells (P09.T21) gives the list the members it claims:
//! each band-E member is asked `claimed_by`, the same test on its own marks, and a member it
//! claims resolves to "no such system", so that a recent death or a variable is the list's or the
//! cells' and never both (P09.T38's complementarity; plan 03's carve-out, P09.T35's for the grid).
//! From 27 a death the claim finds a runaway's (`Claim::Runaway`) is neither's: the
//! core-collapse class's runaway terms hold it, in the field, so the cells give it up and the list
//! drops it (P09.T37's fix of the claim, which named it the list's).
//!
//! # The draws
//!
//! Each entry's draws are keyed by its own feature-level member ID and read on tags the feature's
//! cell members read keyed by theirs (`member.mass` for the mass, the age and the node,
//! `class.type_ia.progenitor` and `class.type_ia.leftover` for a Type Ia), so the registry gains
//! no tag. The velocity is the feature's bulk motion, as [`MemberRecord`]'s is less its internal
//! part, and a list is defined, as the cells are, only in a galaxy with its kinematic tables.
//!
//! A core collapse's remnant leaves that line at its death, at the bulk motion plus the natal
//! kick of its own stars ([`SystemStars::natal_kick`], `ForcedSingle`), on the class's broken line
//! ([`CatalogueMotion::broken`]), and its shell's entry takes the kick as the class's does
//! ([`SupernovaEntry::with_remnant`]): its member's place and velocity at the epoch are the
//! remnant's once it has died (fix-p09-recent-kicks; research-leans-31, `RecentEntry::motion`).

use std::sync::OnceLock;

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::Population;
use crate::galaxy::ages::AgeDistribution;
use crate::galaxy::catalogue_classes::core_collapse::feature_runaway;
use crate::galaxy::catalogue_classes::lbv::{NODES, horizon_window, host_ages, may_host};
use crate::galaxy::catalogue_classes::supernova::{
    BubbleInterior, DeathMarks, SupernovaEntry, SupernovaInterval, Surroundings, claimed_window,
    claims,
};
use crate::galaxy::catalogue_classes::type_ia::{DelayTimeDistribution, IaLeftover, IaProgenitor};
use crate::galaxy::catalogue_classes::{CatalogueMotion, ClassId};
#[cfg(doc)]
use crate::galaxy::displaced::death_in_interval;
use crate::galaxy::displaced::marks::LifetimeBracket;
use crate::galaxy::displaced::{ExplosionSite, death_in_interval_under};
use crate::galaxy::features::catalogue::{FeatureMarks, FeatureRecord, bulk_velocity};
use crate::galaxy::features::cluster::PLUMMER_HALF_MASS_RATIO;
use crate::galaxy::features::ids::PackedFeature;
use crate::galaxy::features::interior::{
    ClassKind, ClassProfile, Generation, MemberClass, Multiplicity, ProfileShape,
};
use crate::galaxy::features::kinds::nursery::{NurseryMarks, NurseryStage, Superbubble};
use crate::galaxy::features::kinds::open_cluster::disruption_survival;
use crate::galaxy::imf::{MassBand, MassFunction};
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::snr::{ShellEnvironment, ShellWindow, SiteGas};
use crate::id::{FeatureMemberId, Layer, MemberSlot, SystemId};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::{SystemStars, primary_draws, primary_eta};
use crate::tables::lbv::LBV_SAMPLER;
use crate::time::UniverseTime;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{
    Dex, HeliumExcess, KilometresPerSecond, LightYears, Seconds, SolarMasses, Years,
};

use super::MemberRecord;
use super::level_list::ListClass;

/// Whether catalogue features list their recently dead and their luminous blue variables, and
/// their band-E cells give them up (plan 09, P09.T36, on the record from generator version 27):
/// every version's features do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum FeatureClassesRules {
    /// No feature lists a class, and the cells claim no member: which no version's galaxy takes.
    /// The tests' reference for what the lists hold (P09.T38's complementarity, the lists'
    /// budgets), and the features' source's lookup of their interiors alone.
    Unlisted,
    /// Every version's: the classes of the module documentation, and the cells' claim.
    Listed,
}

/// Masses of the core collapses' envelope search over band E: 512 nodes, log-spaced.
const ENVELOPE_NODES: u32 = 512;

/// The width given to a single age for a Type Ia's delay draw, which needs a range: a thousand
/// years, under the interval's resolution (lean, 2026-10-31).
const SINGLE_AGE_WIDTH: Years = Years::new(1.0e3);

/// Which recent class claims a member, or holds an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecentClass {
    /// A recent core collapse: `CORE_COLLAPSE`.
    CoreCollapse,
    /// A Type Ia exploding in the interval: `TYPE_IA`.
    TypeIa,
    /// A luminous blue variable whose window overlaps the source horizon:
    /// `LUMINOUS_BLUE_VARIABLE`.
    LuminousBlueVariable,
}

impl RecentClass {
    /// Its catalogue class.
    #[must_use]
    pub const fn class(self) -> ClassId {
        match self {
            Self::CoreCollapse => ClassId::CORE_COLLAPSE,
            Self::TypeIa => ClassId::TYPE_IA,
            Self::LuminousBlueVariable => ClassId::LUMINOUS_BLUE_VARIABLE,
        }
    }

    /// The layer its entries' primaries are of (P09.T37): band E's for a core collapse and a
    /// variable, layer D's for a Type Ia, whose primary is of 2.5–8 M☉
    /// ([`type_ia::PRIMARY_MIN`](crate::galaxy::catalogue_classes::type_ia::PRIMARY_MIN) and
    /// [`PRIMARY_MAX`](crate::galaxy::catalogue_classes::type_ia::PRIMARY_MAX)).
    #[must_use]
    pub const fn layer(self) -> Layer {
        match self {
            Self::CoreCollapse | Self::LuminousBlueVariable => Layer::E,
            Self::TypeIa => Layer::D,
        }
    }

    /// The class of `class`, if it is one of the three.
    #[must_use]
    pub fn of(class: ClassId) -> Option<Self> {
        [Self::CoreCollapse, Self::TypeIa, Self::LuminousBlueVariable]
            .into_iter()
            .find(|c| c.class() == class)
    }
}

/// A uniform distribution of ages at the epoch from `youngest` to `oldest`, a single age widened
/// to [`SINGLE_AGE_WIDTH`] below it: what a Type Ia's delay is drawn on, for a feature's members
/// and for a component of the centre's.
///
/// # Panics
///
/// If either age is not finite.
#[must_use]
pub(crate) fn uniform_ages(youngest: Years, oldest: Years) -> AgeDistribution {
    let lo = youngest
        .value()
        .min(oldest.value() - SINGLE_AGE_WIDTH.value());
    AgeDistribution::uniform(Years::new(lo), oldest)
        .expect("finite ages, the range widened to a thousand years at least")
}

/// What a feature's members are, for its recent classes (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FeatureStars {
    feature: FeatureRecord,
    population: Population,
    /// The stellar mass the members formed with, less what disruption has taken.
    formed: SolarMasses,
    /// The members' band-E systems, born at the feature's birth and since.
    band_e: f64,
    /// The members' ages at the epoch, from the youngest to the oldest.
    ages: (Years, Years),
    composition: Composition,
    velocity: GalacticVelocity,
    nursery: Option<NurseryMarks>,
    profile: ClassProfile,
    /// The core collapses' candidates, where a member's age lets it be one.
    core_collapses: Option<Candidates>,
    /// The luminous blue variables' candidates, where a member's age lets it be one.
    variables: Option<Candidates>,
}

impl FeatureStars {
    /// The stars of `feature` whose list holds its recent classes: a feature [`of`](Self::of)
    /// gives, where the galaxy's features list them (every version's; [`FeatureClassesRules`]).
    #[must_use]
    pub(crate) fn listing(galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Self> {
        match galaxy.rules().feature_classes {
            FeatureClassesRules::Unlisted => None,
            FeatureClassesRules::Listed => Self::of(galaxy, feature),
        }
    }

    /// The stars of `feature` whose band-E cells give up the members [`claimed_by`] claims: where
    /// the galaxy's features list the classes, a feature that lists them, which a globular's
    /// members never are.
    #[must_use]
    pub(crate) fn claiming(galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Self> {
        match galaxy.rules().feature_classes {
            FeatureClassesRules::Unlisted => None,
            FeatureClassesRules::Listed => {
                Self::of(galaxy, feature).filter(|stars| !stars.is_globular())
            }
        }
    }

    /// The stars of `feature`, or `None` for a cloud, a dissolved feature, a feature with no
    /// population or a galaxy without its kinematic tables.
    #[must_use]
    pub(crate) fn of(galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Self> {
        feature.kind_at(Years::ZERO)?;
        let velocity = bulk_velocity(galaxy, feature)?;
        let component = feature.component()?;
        let population = galaxy.fields().component(component).population();
        let (born, survival, ages, fe_h, nursery, half_mass) = match feature.marks() {
            FeatureMarks::Nursery(marks) => {
                let age = marks.age_at_epoch();
                let bound = matches!(
                    marks.stage_at(Years::ZERO),
                    NurseryStage::BoundCluster | NurseryStage::Embedded
                ) && marks.is_bound();
                let survival = if bound {
                    disruption_survival(marks.mass(), age)
                } else {
                    1.0
                };
                let youngest = Years::new((age.value() - marks.age_spread().value()).max(0.0));
                let half_mass = 0.5 * marks.size_at(age).value();
                (
                    marks.mass(),
                    survival,
                    (youngest, age),
                    marks.fe_h(),
                    Some(*marks),
                    half_mass,
                )
            }
            FeatureMarks::OpenCluster(marks) => {
                let age = marks.age_at_epoch();
                (
                    marks.initial_mass(),
                    disruption_survival(marks.initial_mass(), age),
                    (age, age),
                    marks.fe_h(),
                    None,
                    marks.half_mass_radius().value(),
                )
            }
            FeatureMarks::Globular(marks) => (
                marks.mass(),
                1.0,
                (marks.age(), marks.age()),
                marks.fe_h(),
                None,
                marks.half_mass_radius().value(),
            ),
            FeatureMarks::Cloud(_) => return None,
        };
        let formed = SolarMasses::new(born.value() * survival);
        let band_e = formed.value() / galaxy.mean_formed_mass().value()
            * galaxy.shares().share(MassBand::E, population);
        // Tabulated now, so that its list's classes, plan 11's among them, share one table
        // (`ClassProfile::tabulated`; P09.T37: a range query reads some thousand lists near the
        // inner disc).
        let profile = ClassProfile::new(
            ProfileShape::Plummer {
                scale: half_mass / PLUMMER_HALF_MASS_RATIO,
            },
            feature.reach().value(),
            1.0,
        )
        .tabulated();
        let mut stars = Self {
            feature: *feature,
            population,
            formed,
            band_e,
            ages,
            composition: Composition::from_fe_h(Dex::new(fe_h.value()), HeliumExcess::ZERO),
            velocity,
            nursery,
            profile,
            core_collapses: None,
            variables: None,
        };
        if !stars.is_globular() {
            let imf = galaxy.mass_function();
            stars.core_collapses =
                Candidates::core_collapse(imf, ages, &stars.core_collapse_interval(galaxy));
            stars.variables = Candidates::luminous_blue_variable(imf, ages);
        }
        Some(stars)
    }

    /// The feature.
    #[must_use]
    pub(crate) const fn feature(&self) -> &FeatureRecord {
        &self.feature
    }

    /// The members' population: the budget they are drawn from.
    #[must_use]
    pub(crate) const fn population(&self) -> Population {
        self.population
    }

    /// The stellar mass the members formed with, less what disruption has taken.
    #[must_use]
    pub(crate) const fn formed(&self) -> SolarMasses {
        self.formed
    }

    /// The members' ages at the epoch, from the youngest to the oldest.
    #[must_use]
    pub(crate) const fn ages(&self) -> (Years, Years) {
        self.ages
    }

    /// The members' composition.
    #[must_use]
    pub(crate) const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The feature's bulk velocity, galactic.
    #[must_use]
    pub(crate) const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The members' profile about the feature's centre: a Plummer sphere of its half-mass radius,
    /// cut at its reach.
    #[must_use]
    pub(crate) const fn profile(&self) -> &ClassProfile {
        &self.profile
    }

    /// The bytes it owns on the heap: its profile's ([`ClassProfile::heap_bytes`]) and its
    /// candidates' buffers. What a caller's byte-bounded cache charges a list's stars (P09.T37).
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.profile.heap_bytes()
            + [&self.core_collapses, &self.variables]
                .into_iter()
                .flatten()
                .map(Candidates::heap_bytes)
                .sum::<usize>()
    }

    /// Whether it is a globular, which lists Type Ia hosts alone, with no window.
    #[must_use]
    pub(crate) fn is_globular(&self) -> bool {
        matches!(self.feature.marks(), FeatureMarks::Globular(_))
    }

    /// The interval its core collapses' candidates are drawn over: the supernova test's at the
    /// longest cap a death of theirs can take, a nursery's bubble's or, for a site outside its
    /// wall or a nursery with no bubble, the field's.
    #[must_use]
    fn core_collapse_interval(&self, galaxy: &Galaxy) -> SupernovaInterval {
        let caps = galaxy.window_caps();
        let field = SupernovaInterval::of_cap(caps, ShellEnvironment::Field);
        if self.nursery.is_none() {
            return field;
        }
        earlier(
            SupernovaInterval::of_cap(caps, ShellEnvironment::Bubble),
            field,
        )
    }

    /// The interval its Type Ia are drawn over: a globular's hosts, listed with no window, within
    /// the source horizon, `SupernovaInterval::of_window` of none; the others' at the Type Ia's
    /// cap, the longer of it and the bubble's for a nursery's.
    #[must_use]
    fn type_ia_interval(&self, galaxy: &Galaxy) -> SupernovaInterval {
        if self.is_globular() {
            return SupernovaInterval::of_window(Years::ZERO);
        }
        let caps = galaxy.window_caps();
        let ia = SupernovaInterval::of_cap(caps, ShellEnvironment::TypeIa);
        if self.nursery.is_none() {
            return ia;
        }
        earlier(
            SupernovaInterval::of_cap(caps, ShellEnvironment::Bubble),
            ia,
        )
    }

    /// The surroundings of a death at `site`: the nursery's superbubble at the feature's age then,
    /// where the site lies inside it, with the distance to its wall; `field` otherwise.
    #[must_use]
    fn surroundings(
        &self,
        galaxy: &Galaxy,
        site: &ExplosionSite,
        field: Surroundings,
    ) -> Surroundings {
        let Some(marks) = &self.nursery else {
            return field;
        };
        let since = site.death().since_epoch().as_julian_years_f64();
        let centre = self.feature.position();
        let gas = galaxy.gas();
        let Some(bubble) = Superbubble::of(
            marks,
            Years::new(marks.age_at_epoch().value() + since),
            gas.mean_density(centre),
            gas.params().neutral_height(),
            galaxy.feature_shares().supernova_clock(),
        ) else {
            return field;
        };
        let distance = site.position().distance_to(centre).value() / METRES_PER_LIGHT_YEAR;
        let wall = bubble.radius().value() - distance;
        if wall <= 0.0 {
            return field;
        }
        let interior = SiteGas::hot_interior(bubble.interior_density(), bubble.temperature())
            .expect("a bubble's interior is a positive density at a finite temperature");
        Surroundings::Bubble(BubbleInterior::new(interior, LightYears::new(wall)))
    }

    /// The candidates of its `class`, a core collapse's or a luminous blue variable's, or `None`
    /// if no member of its ages can be one (a Type Ia's are drawn delay first).
    #[must_use]
    fn candidates(&self, class: RecentClass) -> Option<&Candidates> {
        match class {
            RecentClass::CoreCollapse => self.core_collapses.as_ref(),
            RecentClass::LuminousBlueVariable => self.variables.as_ref(),
            RecentClass::TypeIa => None,
        }
    }

    /// Its members' ages as a distribution, for a Type Ia's delay ([`uniform_ages`]).
    #[must_use]
    fn age_distribution(&self) -> AgeDistribution {
        uniform_ages(self.ages.0, self.ages.1)
    }

    /// The expected candidates of each of its classes, in list order, those expecting none left
    /// out.
    #[must_use]
    fn classes(&self, galaxy: &Galaxy) -> Vec<(RecentClass, f64)> {
        let mut classes = Vec::new();
        if let Some(candidates) = &self.core_collapses {
            classes.push((RecentClass::CoreCollapse, self.band_e * candidates.share()));
        }
        let years = self.type_ia_interval(galaxy).length().as_julian_years_f64();
        let rate = DelayTimeDistribution::mean_rate_over(&self.age_distribution());
        classes.push((RecentClass::TypeIa, self.formed.value() * rate * years));
        if let Some(candidates) = &self.variables {
            classes.push((
                RecentClass::LuminousBlueVariable,
                self.band_e * candidates.share(),
            ));
        }
        classes.retain(|&(_, expected)| expected > 0.0);
        classes
    }

    /// Its classes as its list registers them, in list order, with their expected candidates and
    /// its profile.
    #[must_use]
    pub(crate) fn list_classes(&self, galaxy: &Galaxy) -> Vec<ListClass> {
        self.classes(galaxy)
            .into_iter()
            .map(|(class, expected)| ListClass::new(class.class(), expected, self.profile.clone()))
            .collect()
    }

    /// The record of `system` at `position`, with initial mass `mass` and age `age`.
    #[must_use]
    fn record(
        &self,
        system: SystemId,
        position: &GalacticPosition,
        mass: SolarMasses,
        age: Years,
    ) -> SystemRecord {
        SystemRecord::from_parts(
            system,
            *position,
            SystemOrigin::FeatureMember {
                feature: PackedFeature::from(self.feature.id()),
                attempt: 0,
            },
            self.population,
            mass,
            age,
        )
    }

    /// The system ID of the list's entry `index`, or `None` for an index past its 13 bits, which
    /// names no entry.
    #[must_use]
    pub(crate) fn member_system(&self, index: u16) -> Option<SystemId> {
        // `IndexTooLarge` is the one error, and an index past any list's is no entry.
        FeatureMemberId::new(
            self.feature.id().feature_ref(),
            MemberSlot::FeatureLevel { index },
        )
        .ok()
        .map(SystemId::from)
    }

    /// Its member record of `record`, of a single star of band E drawn from `range`.
    #[must_use]
    fn member(&self, record: SystemRecord, range: (SolarMasses, SolarMasses)) -> MemberRecord {
        MemberRecord::of_single(
            record,
            MemberClass {
                kind: ClassKind::Living,
                band: MassBand::E,
                generation: Generation::First,
                multiplicity: Multiplicity::Single,
                initial_mass_range: range,
                mean_mass: record.primary_initial_mass(),
            },
            self.composition,
            self.velocity,
        )
    }

    /// The candidate at entry `index` of the list, of class `class` (a core collapse or a luminous
    /// blue variable), at `position`, before its class's test: its record and its part's masses.
    #[must_use]
    fn candidate(
        &self,
        galaxy: &Galaxy,
        class: RecentClass,
        index: u16,
        position: &GalacticPosition,
    ) -> Option<(SystemRecord, (SolarMasses, SolarMasses))> {
        let candidates = self.candidates(class)?;
        let system = self.member_system(index)?;
        let mut stream = Stream::open(galaxy.seed(), tags::MEMBER_MASS, ObjectKey::from(system));
        let (mass, age, range) = candidates.draw(galaxy.mass_function(), &mut stream)?;
        Some((self.record(system, position, mass, age), range))
    }

    /// Entry `index` of the list, of class `class`, at `position`: the candidate's draws and the
    /// class's test (module documentation), or `None` if the test rejects it or the index names no
    /// entry.
    #[must_use]
    pub(crate) fn entry(
        &self,
        galaxy: &Galaxy,
        class: RecentClass,
        index: u16,
        position: &GalacticPosition,
    ) -> Option<RecentEntry> {
        match class {
            RecentClass::CoreCollapse | RecentClass::LuminousBlueVariable => {
                let (record, range) = self.candidate(galaxy, class, index, position)?;
                if class == RecentClass::LuminousBlueVariable {
                    return (claimed_by(galaxy, self, &record, &self.composition, self.velocity)
                        == Some(Claim::Listed(RecentClass::LuminousBlueVariable)))
                    .then(|| RecentEntry {
                        class,
                        member: self.member(record, range),
                        site: None,
                        window: None,
                        motion: CatalogueMotion::straight(*record.epoch_position(), self.velocity),
                        supernova: None,
                    });
                }
                // From 27 a runaway's death is the class's, in the field (P09.T33.b).
                if self.runaway(galaxy, &record) {
                    return None;
                }
                let death =
                    self.core_collapse(galaxy, &record, &self.composition, self.velocity)?;
                // The remnant where its kick takes it (fix-p09-recent-kicks): the member's epoch
                // place and velocity are its broken line's.
                let at = death.motion.position_at(UniverseTime::EPOCH);
                let kicked = self.record(
                    record.id(),
                    &at,
                    record.primary_initial_mass(),
                    record.age_at_epoch(),
                );
                Some(RecentEntry {
                    class,
                    member: self
                        .member(kicked, range)
                        .moving_at(death.motion.velocity_at(UniverseTime::EPOCH)),
                    site: Some(death.site),
                    window: Some(*death.supernova.window()),
                    motion: death.motion,
                    supernova: Some(death.supernova),
                })
            }
            RecentClass::TypeIa => self.type_ia(galaxy, index, position),
        }
    }

    /// Entry `index`, a Type Ia: its progenitor delay first on the members' ages, exploding in the
    /// interval, what it leaves, and its window in the feature's surroundings; a globular's with
    /// none.
    #[must_use]
    fn type_ia(
        &self,
        galaxy: &Galaxy,
        index: u16,
        position: &GalacticPosition,
    ) -> Option<RecentEntry> {
        let system = self.member_system(index)?;
        let key = ObjectKey::from(system);
        let delays =
            DelayTimeDistribution::of_feature(galaxy, self.population, self.age_distribution());
        let first = galaxy.fields().component_id(0)?;
        let interval = self.type_ia_interval(galaxy);
        let mut stream = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_PROGENITOR, key);
        let progenitor = IaProgenitor::draw_in(
            &delays,
            &mut stream,
            first,
            interval.start(),
            interval.end(),
        )?;
        let mut stream = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_LEFTOVER, key);
        let leftover = IaLeftover::draw(progenitor.channel(), &mut stream);
        let site = ExplosionSite::new(
            moved(position, self.velocity, progenitor.explosion()),
            progenitor.explosion(),
        );
        let mass = progenitor.primary_mass();
        let record = self.record(system, position, mass, self.ages.1);
        let window = if self.is_globular() {
            None
        } else {
            let marks = DeathMarks::of_type_ia(
                galaxy,
                system,
                &progenitor,
                &leftover,
                &site,
                self.velocity,
            );
            let surroundings = self.surroundings(galaxy, &site, Surroundings::TypeIa);
            Some(claimed_window(galaxy, &site, &marks, surroundings)?)
        };
        Some(RecentEntry {
            class: RecentClass::TypeIa,
            member: self.member(record, (mass, mass)),
            site: Some(site),
            window,
            motion: CatalogueMotion::straight(*record.epoch_position(), self.velocity),
            supernova: None,
        })
    }

    /// Whether the member `record` is a runaway whose death is the core-collapse class's, by its
    /// own ejection marks ([`feature_runaway`] on its member ID's key): never below 27 or in a
    /// feature with no component.
    #[must_use]
    fn runaway(&self, galaxy: &Galaxy, record: &SystemRecord) -> bool {
        self.feature.component().is_some_and(|component| {
            feature_runaway(
                galaxy,
                component,
                record.primary_initial_mass(),
                ObjectKey::from(record.id()),
            )
        })
    }

    /// The core collapses' test on `record`, of composition `composition` moving at `velocity`: its
    /// death ([`ListedDeath`]), if the one shared test keeps it.
    #[must_use]
    fn core_collapse(
        &self,
        galaxy: &Galaxy,
        record: &SystemRecord,
        composition: &Composition,
        velocity: GalacticVelocity,
    ) -> Option<ListedDeath> {
        let death = death_in_interval_under(
            record.primary_initial_mass(),
            composition,
            primary_eta(galaxy, record),
            record.age_at_epoch(),
            || primary_draws(galaxy, record),
            galaxy.rules().stellar,
        )?;
        let site = ExplosionSite::new(moved(record.epoch_position(), velocity, death), death);
        let stars = SystemStars::generate_with(
            galaxy,
            record,
            composition,
            MultiplicityContext::ForcedSingle,
        );
        let marks = DeathMarks::of(galaxy, &stars, &site, velocity)?;
        let surroundings = self.surroundings(galaxy, &site, Surroundings::Field);
        let window = claimed_window(galaxy, &site, &marks, surroundings)?;
        debug_assert!(claims(galaxy, &site, &marks, surroundings));
        Some(ListedDeath::of(&stars, site, (marks, window), velocity))
    }
}

/// The one of two intervals that starts the earlier: the longer, since both end at +H.
#[must_use]
fn earlier(a: SupernovaInterval, b: SupernovaInterval) -> SupernovaInterval {
    if b.start() < a.start() { b } else { a }
}

/// Where a system at `position` at the epoch moving at `velocity` is at `t`, in a straight line.
///
/// # Panics
///
/// Never for a system below light speed and a time within the supernova test's interval: a few
/// million light-years at most from a position in the galaxy, far inside the ±2³¹ ly the galactic
/// frame addresses.
#[must_use]
pub(crate) fn moved(
    position: &GalacticPosition,
    velocity: GalacticVelocity,
    t: crate::time::UniverseTime,
) -> GalacticPosition {
    let since = Seconds::new(t.since_epoch().as_seconds_f64());
    position
        .translated(velocity.displacement_over(since))
        .expect("below light speed for a few million years stays inside the frame's ±2³¹ ly")
}

/// A band-E class's candidates over stars whose ages at the epoch are uniform between two ages
/// (a single age where both are one): parts, each an interval of initial masses and the window of
/// ages within those at which a star of those masses can be the class's, and its weight, the share
/// of band E's systems in its masses times the share of the ages in its window.
///
/// A core collapse's window at a mass is the ages from which the mass's lifetime bracket
/// ([`LifetimeBracket::shared`]) can reach the interval, on 512 log-spaced nodes over band E; a
/// luminous blue variable's is a sampler node's host ages ([`host_ages`]). Every star the class's
/// test keeps lies in some part, so a candidate drawn by the parts' weights, its mass by the mass
/// function and its age uniform in its window, and kept by the test, has the stars' own density:
/// the candidates are an envelope, never a cut (P09.T36).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidates {
    parts: Vec<CandidatePart>,
    /// The parts' weights, in their order, and their sum.
    weights: Vec<f64>,
    share: f64,
}

/// One node of the core collapses' envelope ([`envelope_nodes`]): its masses, M☉, and its
/// lifetimes' least and greatest, years.
type EnvelopeNode = ((f64, f64), (f64, f64));

/// The core collapses' envelope nodes over band E ([`Candidates::core_collapse`]): each node's
/// masses, and the least and greatest lifetime at its ends' brackets
/// ([`LifetimeBracket::shared`]), widened by its neighbours', for every node whose ends both have
/// one. They depend on the shared bracket alone, so they are made once, on first use (P09.T37: a
/// range query near the inner disc reads some thousand features' lists).
fn envelope_nodes() -> &'static [EnvelopeNode] {
    static NODES: OnceLock<Vec<EnvelopeNode>> = OnceLock::new();
    NODES.get_or_init(|| {
        let (lo, hi) = (MassBand::E.lo(), MassBand::E.hi());
        let step = crate::math::ln(hi / lo) / f64::from(ENVELOPE_NODES);
        let mass = |k: u32| (lo * crate::math::exp(step * f64::from(k))).clamp(lo, hi);
        let bracket = LifetimeBracket::shared();
        let bounds = |k: u32| bracket.bracket(SolarMasses::new(mass(k)));
        (0..ENVELOPE_NODES)
            .filter_map(|k| {
                let (a, b) = (bounds(k)?, bounds(k + 1)?);
                // The node's lifetimes, at its ends' brackets, widened by its neighbours'.
                let neighbours = [
                    k.checked_sub(1).and_then(bounds),
                    bounds((k + 2).min(ENVELOPE_NODES)),
                ];
                let (least, most) = neighbours
                    .into_iter()
                    .flatten()
                    .chain([a, b])
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, m), (x, y)| {
                        (l.min(x.value()), m.max(y.value()))
                    });
                Some(((mass(k), mass(k + 1)), (least, most)))
            })
            .collect()
    })
}

/// One part of a class's [`Candidates`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct CandidatePart {
    /// Its initial masses, M☉.
    masses: (f64, f64),
    /// Its ages at the epoch, years.
    ages: (f64, f64),
    /// Its weight: the share of band E's systems in its masses times the share of the stars' ages
    /// in its ages.
    weight: f64,
}

impl Candidates {
    /// The core collapses' candidates over `ages` for `interval`, or `None` if no star of those
    /// ages can die in it.
    #[must_use]
    pub(crate) fn core_collapse(
        imf: &dyn MassFunction,
        ages: (Years, Years),
        interval: &SupernovaInterval,
    ) -> Option<Self> {
        let (start, end) = (
            interval.start().since_epoch().as_julian_years_f64(),
            interval.end().since_epoch().as_julian_years_f64(),
        );
        // Ages at which a death of a node's lifetimes falls in (start, end].
        let parts = envelope_nodes()
            .iter()
            .map(|&(masses, (least, most))| (masses, (least - end, most - start)));
        Self::of(imf, ages, parts)
    }

    /// The luminous blue variables' candidates over `ages`, or `None` if no sampler node may host
    /// at them.
    #[must_use]
    pub(crate) fn luminous_blue_variable(
        imf: &dyn MassFunction,
        ages: (Years, Years),
    ) -> Option<Self> {
        let parts = (0..NODES).map(|k| {
            let (lo, hi) = LBV_SAMPLER.masses(k);
            let (first, last) = host_ages(k);
            ((lo.value(), hi.value()), (first.value(), last.value()))
        });
        Self::of(imf, ages, parts)
    }

    /// The parts of `windows` (masses, and the ages a star of them may be the class's at) that
    /// meet `ages`, weighted (type documentation), or `None` if none does.
    #[must_use]
    fn of(
        imf: &dyn MassFunction,
        (youngest, oldest): (Years, Years),
        windows: impl Iterator<Item = ((f64, f64), (f64, f64))>,
    ) -> Option<Self> {
        let (youngest, oldest) = (youngest.value(), oldest.value());
        let band = imf.integral(MassBand::E.lo(), MassBand::E.hi());
        let span = oldest - youngest;
        let parts: Vec<CandidatePart> = windows
            .filter_map(|((m_lo, m_hi), (a_lo, a_hi))| {
                let (lo, hi) = (a_lo.max(youngest), a_hi.min(oldest));
                if hi < lo || m_hi <= m_lo {
                    return None;
                }
                let ages = if span > 0.0 { (hi - lo) / span } else { 1.0 };
                let weight = imf.integral(m_lo, m_hi) / band * ages;
                (weight > 0.0).then_some(CandidatePart {
                    masses: (m_lo, m_hi),
                    ages: (lo, hi),
                    weight,
                })
            })
            .collect();
        if parts.is_empty() {
            return None;
        }
        let weights: Vec<f64> = parts.iter().map(|part| part.weight).collect();
        let share = weights.iter().fold(0.0, |s, w| s + w);
        Some(Self {
            parts,
            weights,
            share,
        })
    }

    /// Whether a star of initial mass `mass` and age `age` lies in some part: what every star the
    /// class's test keeps must (type documentation).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn contains(&self, mass: SolarMasses, age: Years) -> bool {
        let (mass, age) = (mass.value(), age.value());
        self.parts.iter().any(|part| {
            (part.masses.0..=part.masses.1).contains(&mass)
                && (part.ages.0..=part.ages.1).contains(&age)
        })
    }

    /// The share of band E's systems of the stars' ages that are candidates: the parts' weights
    /// summed.
    #[must_use]
    pub(crate) const fn share(&self) -> f64 {
        self.share
    }

    /// The bytes it owns on the heap: its parts' and weights' buffers, by their capacities.
    #[must_use]
    fn heap_bytes(&self) -> usize {
        self.parts.capacity() * size_of::<CandidatePart>()
            + self.weights.capacity() * size_of::<f64>()
    }

    /// One candidate's initial mass and age, and its part's masses, from `stream`: a mark picks
    /// the part by weight, a uniform its mass by `imf`, a uniform its age in the part's window.
    #[must_use]
    pub(crate) fn draw(
        &self,
        imf: &dyn MassFunction,
        stream: &mut Stream,
    ) -> Option<(SolarMasses, Years, (SolarMasses, SolarMasses))> {
        let CandidatePart {
            masses,
            ages: (lo, hi),
            ..
        } = self.parts[stream.mark().pick_weighted(&self.weights, self.share)?];
        let mass = imf.quantile_in(masses.0, masses.1, stream.uniform());
        let age = hi - (hi - lo) * stream.uniform();
        Some((
            SolarMasses::new(mass),
            Years::new(age),
            (SolarMasses::new(masses.0), SolarMasses::new(masses.1)),
        ))
    }
}

/// What claims a band-E member of a catalogue feature ([`claimed_by`]), which its cells then give
/// up: exactly one side holds each mark set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Claim {
    /// One of the feature's recent classes, on its list.
    Listed(RecentClass),
    /// From generator version 27, a runaway's death: its own ejection marks put it out of its
    /// feature before it dies ([`feature_runaway`]), so the core-collapse class's runaway terms
    /// hold it, in the field at the whole budget, and neither the cells nor the list (P09.T33.b's
    /// deaths by kind; research-leans-21).
    Runaway,
}

/// What claims the member `record` of `stars`'s feature, of composition `composition` moving at
/// `velocity` (module documentation): the core collapses if the one shared test keeps its death's
/// shell, or from 27 the class's runaway terms if its ejection marks make it a runaway; else the
/// luminous blue variables if its window overlaps the source horizon; a globular's members none.
///
/// The cells ask it of each band-E member and the list of each candidate, on their own marks, so
/// the cells give up what the list or the runaway terms hold, and the list holds its classes'
/// alone: each mark set is one side's.
#[must_use]
pub(crate) fn claimed_by(
    galaxy: &Galaxy,
    stars: &FeatureStars,
    record: &SystemRecord,
    composition: &Composition,
    velocity: GalacticVelocity,
) -> Option<Claim> {
    if stars.is_globular() {
        return None;
    }
    if stars
        .core_collapse(galaxy, record, composition, velocity)
        .is_some()
    {
        return Some(if stars.runaway(galaxy, record) {
            Claim::Runaway
        } else {
            Claim::Listed(RecentClass::CoreCollapse)
        });
    }
    (may_host(record.primary_initial_mass(), record.age_at_epoch())
        && horizon_window(&SystemStars::generate_with(
            galaxy,
            record,
            composition,
            MultiplicityContext::ForcedSingle,
        ))
        .is_some())
    .then_some(Claim::Listed(RecentClass::LuminousBlueVariable))
}

/// A listed core collapse's death (P09.T36; fix-p09-recent-kicks): its site, its shell's entry,
/// whose compact remnant takes the natal kick of its own stars as the class's entries' does
/// (`SupernovaEntry::with_remnant`), and how it moves: its progenitor's line through the site,
/// broken there at its death, from where the remnant moves at the progenitor's velocity plus the
/// kick, as the class's [`CatalogueMotion::broken`] has it (research-leans-31).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ListedDeath {
    pub(crate) site: ExplosionSite,
    pub(crate) supernova: SupernovaEntry,
    pub(crate) motion: CatalogueMotion,
}

impl ListedDeath {
    /// The death of `stars`'s primary at `site`, whose marks are `marks` and whose shell the test
    /// cut to `window`, for a system moving at `velocity` before it.
    #[must_use]
    pub(crate) fn of(
        stars: &SystemStars,
        site: ExplosionSite,
        (marks, window): (DeathMarks, ShellWindow),
        velocity: GalacticVelocity,
    ) -> Self {
        let death = site.death();
        let kick = stars.natal_kick();
        let remnant = kick.map_or(velocity, |kick| {
            velocity + GalacticVelocity::new(kick.velocity())
        });
        let kick_speed = kick.map_or(KilometresPerSecond::ZERO, |kick| {
            KilometresPerSecond::from(kick.speed())
        });
        Self {
            site,
            supernova: SupernovaEntry::new(stars.record().id(), death, marks.kind(), window)
                .with_remnant(kick_speed, stars.primary().neutron_star()),
            motion: CatalogueMotion::broken(*site.position(), death, velocity, remnant),
        }
    }
}

/// One recent entry of a feature's list: its class, its member, for a death its site and the
/// window of its shell, how it moves, and for a core collapse its shell's entry.
#[derive(Debug, Clone, PartialEq)]
pub struct RecentEntry {
    class: RecentClass,
    member: MemberRecord,
    site: Option<ExplosionSite>,
    window: Option<ShellWindow>,
    motion: CatalogueMotion,
    supernova: Option<SupernovaEntry>,
}

impl RecentEntry {
    /// Its class.
    #[must_use]
    pub const fn class(&self) -> RecentClass {
        self.class
    }

    /// Its member.
    #[must_use]
    pub const fn member(&self) -> &MemberRecord {
        &self.member
    }

    /// Where and when it explodes or exploded, for a death.
    #[must_use]
    pub const fn site(&self) -> Option<&ExplosionSite> {
        self.site.as_ref()
    }

    /// The window of its shell, for a death that makes one: none for a globular's Type Ia or a
    /// luminous blue variable.
    #[must_use]
    pub const fn window(&self) -> Option<&ShellWindow> {
        self.window.as_ref()
    }

    /// How it moves: a core collapse on its progenitor's line broken at its death, where its
    /// remnant takes its natal kick (fix-p09-recent-kicks); any other entry on one straight line
    /// at its feature's bulk motion.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// Its shell's entry, for a core collapse: its explosion, its kind, its window and its
    /// remnant's kick, which the remnant's offset, its bow shock and its nebula read.
    #[must_use]
    pub const fn supernova(&self) -> Option<&SupernovaEntry> {
        self.supernova.as_ref()
    }
}

/// The classes `feature` registers on its list, in list order, with their expected candidates and
/// profile: none where the features list no class, a feature without stars or a galaxy without its
/// kinematic tables (module documentation).
#[cfg(test)]
#[must_use]
pub(crate) fn recent_classes(galaxy: &Galaxy, feature: &FeatureRecord) -> Vec<ListClass> {
    FeatureStars::listing(galaxy, feature).map_or_else(Vec::new, |stars| stars.list_classes(galaxy))
}

#[cfg(test)]
mod tests;
