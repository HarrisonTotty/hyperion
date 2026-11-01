//! Plan 11's binary classes inside features (P11.T8.f): the hosts a feature holds of each carved
//! class, listed on its feature-level list after plan 09's own classes, from generator version 27
//! (`FEATURE_BINARIES_VERSION`).
//!
//! # Where a host is
//!
//! A candidate of a binary class is drawn present first through its class's own sampler and scan
//! marks wherever it sits ([`BinaryHost`]): at a catalogue cell's site, whose density component's
//! laws give its age, its composition and its velocity, or at an entry of a feature's list
//! ([`ListedHost`]), which takes its feature's. The draws after the site are the class's, on the
//! class's own tags keyed by the host's system, so a listed host's marks come through the same
//! code as a catalogue entry's and never run the binary engine. A listed host's age is its
//! feature's, a member's drawn in the members' ages: its class's history is drawn given it, and
//! a candidate whose marks cannot be that old (a white dwarf whose progenitor has not yet died, a
//! donor past its turn-off) is thinned, as any candidate its class's test refuses.
//!
//! # The hold
//!
//! The feature-level lists of catalogue features and the galactic centre register the five
//! classes, the catalogue's classes give up the share of their hosts that sits in features, and a
//! feature's binary members draw their companions inside its hard–soft boundary, all from version
//! 27 ([`FeatureBinariesRules`]).

use super::{
    AwdEntry, AwdProcess, AwdSide, MergerEntry, NeutronStarMergers, NsmEntry, StellarMergers,
    XrayBinaries, XrbEntry, XrbSide,
};
use crate::GeneratorVersion;
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::catalogue_classes::core_collapse::class_site_velocity;
use crate::galaxy::catalogue_classes::registry::ClassId;
#[cfg(doc)]
use crate::galaxy::catalogue_classes::registry::FEATURE_LEVEL_LIST_ORDER;
use crate::galaxy::features::catalogue::FeatureRecord;
use crate::galaxy::features::cluster::ClusterModel;
use crate::galaxy::features::ids::PackedFeature;
use crate::galaxy::features::interior::{
    ClassKind, ClassProfile, Generation, MemberClass, Multiplicity,
};
use crate::galaxy::features::members::ListClass;
use crate::galaxy::features::members::recent::FeatureStars;
use crate::galaxy::fields::{Component, ComponentId};
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{
    CatalogueSystemId, FeatureMemberId, FeatureRef, MemberSlot, SystemId, SystemIdKind,
};
use crate::rng::{DomainTag, ObjectKey, Stream};
use crate::stellar::Composition;
#[cfg(doc)]
use crate::stellar::binary::carved_class;
use crate::stellar::binary::light_curves::XrayAccretor;
use crate::stellar::binary::scan::{awd_marks_at, merger_marks_at, nsm_marks_at, xrb_marks_at};
use crate::stellar::binary::{BinaryTimeline, CarvedClass};
use crate::stellar::system::SystemStars;
use crate::tables::binary::{ClassShareRow, NsmPairKind};
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

/// The generator version from which features list plan 11's binary classes, the catalogue's
/// classes give up the features' share of their hosts, and a feature's binary members are drawn
/// inside its hard–soft boundary (P11.T8.f; the hold of [`FeatureBinariesRules`]).
///
/// Below it every galaxy built by `Galaxy::from_params` takes the held arm,
/// [`FeatureBinariesRules::Unlisted`], the code of version 26 verbatim: no binary class is
/// registered on a list, the catalogue holds every host and a binary member's orbits are
/// unbounded. The tests reach the new arm through a galaxy built under `GalaxyRules::every_27_hold()`.
/// The bump to 27 deletes this hold, [`FeatureBinariesRules`] with its `Unlisted` arm,
/// `GalaxyRules::feature_binaries`, the `match`es that read it and the tripwire
/// (`the_feature_binaries_are_held_only_until_the_27_bump`).
pub(crate) const FEATURE_BINARIES_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether features hold plan 11's binary classes (the hold of [`FEATURE_BINARIES_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum FeatureBinariesRules {
    /// Version 26's and before: no feature lists a binary class, the catalogue holds every host,
    /// and a feature's binary members take no widest separation.
    Unlisted,
    /// Version 27's: the module documentation's.
    Listed,
}

/// What an entry of a feature's list shares with its feature (P11.T8.f): its members' population,
/// the range of their ages at the epoch, their composition, the feature's motion, and the origin
/// its record takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListedHost {
    population: Population,
    ages: (Years, Years),
    composition: Composition,
    velocity: GalacticVelocity,
    origin: SystemOrigin,
}

impl ListedHost {
    /// A host among members of `population`, `ages` old at the epoch (the youngest first) and of
    /// `composition`, moving at `velocity`, whose record is of `origin`.
    ///
    /// # Panics
    ///
    /// If an age is negative or not finite, or the youngest is older than the oldest.
    #[must_use]
    pub(crate) fn new(
        population: Population,
        ages: (Years, Years),
        composition: Composition,
        velocity: GalacticVelocity,
        origin: SystemOrigin,
    ) -> Self {
        let (young, old) = (ages.0.value(), ages.1.value());
        assert!(
            young >= 0.0 && young.is_finite() && old.is_finite() && young <= old,
            "a listed host's members are {young} to {old} years old"
        );
        Self {
            population,
            ages,
            composition,
            velocity,
            origin,
        }
    }

    /// The members' population.
    #[must_use]
    pub const fn population(&self) -> Population {
        self.population
    }

    /// The members' ages at the epoch, the youngest first.
    #[must_use]
    pub const fn ages(&self) -> (Years, Years) {
        self.ages
    }

    /// The members' composition.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The feature's velocity, galactic, at which the host moves.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The origin of the host's record: a feature's member, or the galactic centre's.
    #[must_use]
    pub const fn origin(&self) -> SystemOrigin {
        self.origin
    }

    /// The age at `share` of the members' ages, `share` in [0, 1]: the youngest at 0.
    #[must_use]
    pub fn age_at(&self, share: f64) -> Years {
        let (young, old) = (self.ages.0.value(), self.ages.1.value());
        Years::new(young + (old - young) * share.clamp(0.0, 1.0))
    }
}

/// Where a binary class's candidate sits (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HostPlace {
    /// A catalogue cell's site, in the field of its density component.
    Field(ComponentId),
    /// An entry of a feature's list (P11.T8.f).
    Listed(ListedHost),
}

/// A binary class's candidate's system, its place at the epoch and what it shares with its
/// surroundings (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinaryHost {
    system: SystemId,
    position: GalacticPosition,
    place: HostPlace,
}

impl BinaryHost {
    /// Catalogue candidate `id` at its thinned `site` in `component`'s field.
    #[must_use]
    pub(crate) fn field(
        id: CatalogueSystemId,
        site: GalacticPosition,
        component: ComponentId,
    ) -> Self {
        Self {
            system: SystemId::from(id),
            position: site,
            place: HostPlace::Field(component),
        }
    }

    /// The list entry `system` at `position`, of `host`.
    #[must_use]
    pub(crate) const fn listed(
        system: SystemId,
        position: GalacticPosition,
        host: ListedHost,
    ) -> Self {
        Self {
            system,
            position,
            place: HostPlace::Listed(host),
        }
    }

    /// Its system: a catalogue entry's member 0, or a list entry's member.
    #[must_use]
    pub const fn system(&self) -> SystemId {
        self.system
    }

    /// Its place at the epoch: a catalogue candidate's site, a list entry's position.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// Its catalogue ID, for a catalogue candidate: member 0 of its entry.
    #[must_use]
    pub fn catalogue_id(&self) -> Option<CatalogueSystemId> {
        match self.system.kind() {
            SystemIdKind::Catalogue(id) => Some(id),
            SystemIdKind::Grid(_)
            | SystemIdKind::FeatureMember(_)
            | SystemIdKind::Centre(_)
            | SystemIdKind::Stream(_)
            | SystemIdKind::DwarfCore(_)
            | SystemIdKind::Pinned(_) => None,
        }
    }

    /// Where it sits.
    #[must_use]
    pub const fn place(&self) -> &HostPlace {
        &self.place
    }

    /// Its density component, for a catalogue candidate.
    #[must_use]
    pub const fn component(&self) -> Option<ComponentId> {
        match self.place {
            HostPlace::Field(component) => Some(component),
            HostPlace::Listed(_) => None,
        }
    }

    /// The key its draws are read at.
    #[must_use]
    pub(crate) fn key(&self) -> ObjectKey {
        ObjectKey::from(self.system)
    }

    /// The population its system is counted in: its component's, or its feature's members'.
    #[must_use]
    pub fn population(&self, galaxy: &Galaxy) -> Population {
        match &self.place {
            HostPlace::Field(component) => galaxy.fields().component(*component).population(),
            HostPlace::Listed(host) => host.population,
        }
    }

    /// The origin of its record: a catalogue system of `class`, or its list entry's.
    #[must_use]
    pub const fn origin(&self, class: ClassId) -> SystemOrigin {
        match &self.place {
            HostPlace::Field(_) => SystemOrigin::CatalogueSystem(class),
            HostPlace::Listed(host) => host.origin,
        }
    }

    /// Its velocity at the epoch, galactic: a catalogue candidate's, plan 08's draw of its
    /// component's law at its site on `tag` (zero in a galaxy without its kinematic tables); a list
    /// entry's, its feature's, with no draw.
    #[must_use]
    pub(crate) fn velocity(&self, galaxy: &Galaxy, tag: DomainTag) -> GalacticVelocity {
        match &self.place {
            HostPlace::Field(component) => {
                class_site_velocity(galaxy, *component, &self.position, tag, self.key())
            }
            HostPlace::Listed(host) => host.velocity,
        }
    }

    /// Its system's age at the epoch at `rank`, in [0, 1], of the ages it may have: a catalogue
    /// candidate's in its component's age law among its born systems, a list entry's in its
    /// members' ages.
    #[must_use]
    pub(crate) fn age_at_rank(&self, galaxy: &Galaxy, rank: f64) -> Years {
        match &self.place {
            HostPlace::Field(component) => {
                let ages = galaxy.fields().component(*component).ages();
                let unborn = ages.cdf(Years::ZERO);
                ages.quantile(unborn + (1.0 - unborn) * rank)
            }
            HostPlace::Listed(host) => host.age_at(rank),
        }
    }

    /// Its composition at the age `age`: a catalogue candidate's drawn on `tag` from its
    /// component's law at its site; a list entry's, its feature's members', with no draw.
    #[must_use]
    pub(crate) fn composition_at(
        &self,
        galaxy: &Galaxy,
        age: Years,
        tag: DomainTag,
    ) -> Composition {
        match &self.place {
            HostPlace::Field(component) => {
                let laws = galaxy.fields().component(*component);
                let feh = laws.metallicity(&PointLy::from(&self.position), age);
                let mut stream = Stream::open(galaxy.seed(), tag, self.key());
                Composition::from_fe_h(
                    Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
                    HeliumExcess::ZERO,
                )
            }
            HostPlace::Listed(host) => host.composition,
        }
    }

    /// Its list entry's host, if it is one.
    #[must_use]
    pub const fn listed_host(&self) -> Option<&ListedHost> {
        match &self.place {
            HostPlace::Field(_) => None,
            HostPlace::Listed(host) => Some(host),
        }
    }
}

/// The share of a carved class's hosts in `component` that the catalogue keeps under `galaxy`'s
/// rules: all of them under the held arm, and from version 27 all but the features' share, which
/// the features' lists hold ([`features_share`]).
#[must_use]
pub(crate) fn catalogue_share(galaxy: &Galaxy, row: &ClassShareRow, component: &Component) -> f64 {
    match galaxy.rules().feature_binaries {
        FeatureBinariesRules::Unlisted => 1.0,
        FeatureBinariesRules::Listed => 1.0 - features_share(galaxy, row, component),
    }
}

/// The share of a carved class's hosts in `component` that sit in features: plan 09's share φ of
/// the component's systems in each band inside features (for a sub-disc of the old thin disc its
/// own, as `FeatureShares::component_field_factor` reads it), weighted by the share of the class's
/// hosts whose primary lies in the band ([`ClassShareRow::layer_share`]).
#[must_use]
pub(crate) fn features_share(galaxy: &Galaxy, row: &ClassShareRow, component: &Component) -> f64 {
    let shares = galaxy.feature_shares();
    let population = component.population();
    MassBand::ALL.iter().fold(0.0, |sum, &band| {
        let phi = match component.sub_disc() {
            Some(sub_disc) => shares.phi_sub_disc(sub_disc) + shares.phi_globular(population, band),
            None => shares.phi(population, band),
        };
        sum + row.layer_share(band) * phi
    })
}

/// The binary classes of [`FEATURE_LEVEL_LIST_ORDER`], plan 11's five, in its order.
const LISTED_CLASSES: [ClassId; 5] = [
    ClassId::ACCRETING_WHITE_DWARF,
    ClassId::ACCRETING_WHITE_DWARF_SLOW,
    ClassId::XRAY_BINARY,
    ClassId::STELLAR_MERGER,
    ClassId::NEUTRON_STAR_MERGER,
];

/// The carved class of plan 11's binary class `class`, if it is one.
#[must_use]
fn carved_of(class: ClassId) -> Option<CarvedClass> {
    match class {
        ClassId::ACCRETING_WHITE_DWARF => Some(CarvedClass::AccretingWdFast),
        ClassId::ACCRETING_WHITE_DWARF_SLOW => Some(CarvedClass::AccretingWdSlow),
        ClassId::XRAY_BINARY => Some(CarvedClass::XrayBinary),
        ClassId::STELLAR_MERGER => Some(CarvedClass::StellarMerger),
        ClassId::NEUTRON_STAR_MERGER => Some(CarvedClass::NeutronStarMerger),
        _ => None,
    }
}

/// Whether `class` is one of plan 11's binary classes, which a list holds through
/// [`listed_entry`].
#[must_use]
pub fn is_listed_binary(class: ClassId) -> bool {
    carved_of(class).is_some()
}

/// The hosts per solar mass formed of plan 11's binary class `class` among members of
/// `population`: its row of the galaxy's share table (`GalaxyRules::binary_tables`:
/// [`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES),
/// or from version 27 plan 15's fitted rows), the X-ray binaries' on the side the population's
/// members are on ([`XrayBinaries::hosts_per_formed_mass`]).
///
/// # Panics
///
/// If `class` is not one of plan 11's binary classes.
#[must_use]
pub(crate) fn hosts_per_formed_mass(
    galaxy: &Galaxy,
    class: ClassId,
    population: Population,
) -> f64 {
    let carved = carved_of(class).expect("a listed class is one of plan 11's binary classes");
    match carved {
        CarvedClass::XrayBinary => XrayBinaries::new(galaxy).hosts_per_formed_mass(population),
        CarvedClass::AccretingWdFast
        | CarvedClass::AccretingWdSlow
        | CarvedClass::StellarMerger
        | CarvedClass::NeutronStarMerger => galaxy
            .rules()
            .binary_tables
            .class_shares()
            .row(carved)
            .hosts_per_formed_mass(population),
    }
}

/// A catalogue feature's hosts of plan 11's binary classes on its list (P11.T8.f; module
/// documentation): each class's expected hosts and what its entries share with the feature.
///
/// A class's hosts are the feature's budget, the stellar mass its members formed with less what
/// disruption has taken, times its hosts per solar mass formed in the members' population
/// ([`hosts_per_formed_mass`]); a globular's X-ray binaries are plan 09's encounter-rate law
/// (P09.T9.e, [`ClusterModel::xray_binaries`]) in their place.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FeatureBinaries {
    feature: FeatureRef,
    host: ListedHost,
    profile: ClassProfile,
    /// Each class's expected hosts, in list order, those expecting none left out.
    classes: Vec<(ClassId, f64)>,
}

impl FeatureBinaries {
    /// The binary classes of `feature`, whose members are `stars` where its list holds plan 09's
    /// recent classes, or `None` under the hold's held arm and for a feature without members
    /// ([`FeatureStars::of`]).
    #[must_use]
    pub(crate) fn of_feature(
        galaxy: &Galaxy,
        feature: &FeatureRecord,
        stars: Option<&FeatureStars>,
    ) -> Option<Self> {
        if galaxy.rules().feature_binaries == FeatureBinariesRules::Unlisted {
            return None;
        }
        match stars {
            Some(stars) => Some(Self::of(galaxy, stars)),
            None => FeatureStars::of(galaxy, feature).map(|stars| Self::of(galaxy, &stars)),
        }
    }

    /// The binary classes of the feature whose members are `stars`.
    #[must_use]
    fn of(galaxy: &Galaxy, stars: &FeatureStars) -> Self {
        let population = stars.population();
        let formed = stars.formed().value();
        let globular = stars
            .is_globular()
            .then(|| ClusterModel::from_record(galaxy, stars.feature()))
            .flatten();
        let classes = LISTED_CLASSES
            .iter()
            .map(|&class| {
                let expected = match &globular {
                    Some(model) if class == ClassId::XRAY_BINARY => model.xray_binaries(),
                    _ => formed * hosts_per_formed_mass(galaxy, class, population),
                };
                (class, expected)
            })
            .filter(|&(_, expected)| expected > 0.0)
            .collect();
        let origin = SystemOrigin::FeatureMember {
            feature: PackedFeature::from(stars.feature().id()),
            attempt: 0,
        };
        Self {
            feature: stars.feature().id().feature_ref(),
            host: ListedHost::new(
                population,
                stars.ages(),
                *stars.composition(),
                stars.velocity(),
                origin,
            ),
            profile: stars.profile().clone(),
            classes,
        }
    }

    /// Whether it lists no class.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    /// The entry of class `class` at list index `index` and `position`, if its class's draws and
    /// test keep it ([`listed_entry`]); `None` for an index past the 13 bits of a feature's list.
    #[must_use]
    pub(crate) fn entry(
        &self,
        galaxy: &Galaxy,
        class: ClassId,
        index: u16,
        position: &GalacticPosition,
    ) -> Option<ListedBinary> {
        // `IndexTooLarge` is the one error, and an index past any list's is no entry.
        let id = FeatureMemberId::new(self.feature, MemberSlot::FeatureLevel { index }).ok()?;
        listed_entry(
            galaxy,
            class,
            BinaryHost::listed(SystemId::from(id), *position, self.host),
        )
    }

    /// Each class's expected hosts, in list order.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn expected(&self) -> &[(ClassId, f64)] {
        &self.classes
    }

    /// Its classes as its list registers them, in list order, with their expected hosts and the
    /// members' profile.
    #[must_use]
    pub(crate) fn list_classes(&self) -> Vec<ListClass> {
        self.classes
            .iter()
            .map(|&(class, expected)| ListClass::new(class, expected, self.profile.clone()))
            .collect()
    }

    /// What its entries share with the feature.
    #[must_use]
    pub(crate) const fn host(&self) -> &ListedHost {
        &self.host
    }
}

/// One entry of plan 11's binary classes on a feature's list (P11.T8.f): its class's entry, drawn
/// at its list host through its class's own sampler and scan marks.
#[derive(Debug, Clone, PartialEq)]
pub enum ListedBinary {
    /// An accreting white dwarf, fast or slow.
    AccretingWd(AwdEntry),
    /// An X-ray binary.
    XrayBinary(XrbEntry),
    /// A stellar merger in the source horizon.
    StellarMerger(MergerEntry),
    /// A neutron-star merger in the source horizon.
    NeutronStarMerger(NsmEntry),
}

impl ListedBinary {
    /// Its class.
    #[must_use]
    pub fn class(&self) -> ClassId {
        match self {
            Self::AccretingWd(entry) => entry.scan().side().class(),
            Self::XrayBinary(_) => ClassId::XRAY_BINARY,
            Self::StellarMerger(_) => ClassId::STELLAR_MERGER,
            Self::NeutronStarMerger(_) => ClassId::NEUTRON_STAR_MERGER,
        }
    }

    /// Its carved class, which its timeline's [`carved_class`] is.
    #[must_use]
    pub fn carved(&self) -> CarvedClass {
        carved_of(self.class()).expect("a listed binary is of plan 11's classes")
    }

    /// Its system's record, of its list entry's origin.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        match self {
            Self::AccretingWd(entry) => entry.record(),
            Self::XrayBinary(entry) => entry.record(),
            Self::StellarMerger(entry) => entry.record(),
            Self::NeutronStarMerger(entry) => entry.record(),
        }
    }

    /// The composition both its stars share.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        match self {
            Self::AccretingWd(entry) => entry.composition(),
            Self::XrayBinary(entry) => entry.composition(),
            Self::StellarMerger(entry) => entry.scan().composition(),
            Self::NeutronStarMerger(entry) => entry.scan().composition(),
        }
    }

    /// Its pair's timeline, from its marks.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        match self {
            Self::AccretingWd(entry) => entry.timeline(),
            Self::XrayBinary(entry) => entry.timeline(),
            Self::StellarMerger(entry) => entry.timeline(),
            Self::NeutronStarMerger(entry) => entry.timeline(),
        }
    }

    /// Its stars: its class's marked pair.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        match self {
            Self::AccretingWd(entry) => entry.stars(galaxy),
            Self::XrayBinary(entry) => entry.stars(galaxy),
            Self::StellarMerger(entry) => entry.stars(galaxy),
            Self::NeutronStarMerger(entry) => entry.stars(galaxy),
        }
    }

    /// The member class it is counted in: its primary's kind now (a white dwarf, a neutron star or
    /// a black hole, or a living star for a stellar merger) in its primary's band, binary, its
    /// mean mass its pair's at the epoch.
    #[must_use]
    pub fn member_class(&self) -> MemberClass {
        let record = self.record();
        let kind = match self {
            Self::AccretingWd(_) => ClassKind::WhiteDwarf,
            Self::XrayBinary(entry) => match entry.scan().kind().accretor() {
                XrayAccretor::NeutronStar => ClassKind::NeutronStar,
                XrayAccretor::BlackHole => ClassKind::BlackHole,
            },
            Self::StellarMerger(_) => ClassKind::Living,
            Self::NeutronStarMerger(entry) => match entry.scan().kind() {
                NsmPairKind::DoubleNeutronStar => ClassKind::NeutronStar,
                NsmPairKind::NeutronStarBlackHole => ClassKind::BlackHole,
            },
        };
        let m0 = record.primary_initial_mass();
        MemberClass {
            kind,
            band: band_of(m0),
            generation: Generation::First,
            multiplicity: Multiplicity::Binary,
            initial_mass_range: (m0, m0),
            mean_mass: self.timeline().state_at(record.age_at_epoch()).total_mass(),
        }
    }
}

/// The band of a primary of initial mass `mass`: the first whose upper edge lies above it, band E
/// for the heaviest.
#[must_use]
fn band_of(mass: SolarMasses) -> MassBand {
    MassBand::ALL
        .into_iter()
        .find(|band| mass.value() < band.hi())
        .unwrap_or(MassBand::E)
}

/// The entry of plan 11's binary class `class` at the list host `host`, drawn present first
/// through its class's own sampler and scan marks, or `None` if they or its class's test refuse it
/// (module documentation), or `class` is not one of plan 11's.
#[must_use]
pub(crate) fn listed_entry(
    galaxy: &Galaxy,
    class: ClassId,
    host: BinaryHost,
) -> Option<ListedBinary> {
    match carved_of(class)? {
        CarvedClass::AccretingWdFast | CarvedClass::AccretingWdSlow => {
            let side = if class == ClassId::ACCRETING_WHITE_DWARF {
                AwdSide::Fast
            } else {
                AwdSide::Slow
            };
            let process = AwdProcess::new(galaxy, side);
            let scan = awd_marks_at(galaxy, &process, host)?;
            let candidate = process.candidate_of(galaxy, &scan)?;
            process.test(candidate).map(ListedBinary::AccretingWd)
        }
        CarvedClass::XrayBinary => {
            let process = XrayBinaries::new(galaxy);
            let side = XrbSide::of(host.population(galaxy));
            let scan = xrb_marks_at(galaxy, &process, host, side);
            let candidate = process.candidate_of(galaxy, &scan)?;
            XrayBinaries::test_candidate(candidate).map(ListedBinary::XrayBinary)
        }
        CarvedClass::StellarMerger => {
            let scan = merger_marks_at(galaxy, host)?;
            StellarMergers::test(StellarMergers::candidate_of(galaxy, &scan))
                .map(ListedBinary::StellarMerger)
        }
        CarvedClass::NeutronStarMerger => {
            let scan = nsm_marks_at(galaxy, host)?;
            NeutronStarMergers::test(NeutronStarMergers::candidate_of(galaxy, &scan))
                .map(ListedBinary::NeutronStarMerger)
        }
    }
}

#[cfg(test)]
mod tests;
