//! The catalogue features' and the galactic centre's lists as the alert walks read them, kept
//! between walks (the [module](super) documentation; fix-p09-alert-walk-features-2).
//!
//! A whole-cube walk of a class reads the list of every feature in the cube, 2.5 × 10⁵ at the
//! Milky Way fixture, 6.6 × 10⁴ of them with an entry, and each list costs its profile's table and
//! its entries' tests: built afresh for each walk, as the first lane built them, the lists of a
//! whole-cube walk of the core collapses took 134 thread-seconds (pod, 16 vCPU, dev; the lane's
//! outbox). What a walk reads of a list that no window changes is kept instead ([`ListedHosts`]):
//!
//! - each class's expected candidates and count, found without the profile's table
//!   (`FeatureLevelList::class_counts_of`), as the census reads them;
//! - each class's hosts, found when a walk of the class first reads the list: the core
//!   collapses' deaths' times and kicks, read first from their lifetimes (`FeatureStars::
//!   core_collapse_death`), whose entries a walk builds only once its cut takes one of them, every
//!   one at once (`FeatureLevelList::core_collapse_told`, [`ToldEntries`]), and a cache keeps
//!   apart, charged as it is built (perf-alert-walks-cpu); the Type Ia's deaths, built whole; the
//!   luminous blue variables' hosts; plan 11's binaries' scan marks, and the mergers' events.
//!
//! The galactic centre's list is kept the same way ([`ListedHosts::of_centre`]), its core
//! collapses' deaths built whole, as the walks read its few hundred. A feature cell's lists are kept
//! together ([`ListedCell`]), one entry of a caller's cache for the cell's hundreds of features,
//! and a caller lends them through [`ListedHostsCache`]. Each is a pure function of the galaxy and
//! its cell, and each class's hosts the same whenever they are found, so what a cache holds changes
//! no walk's answer, each detection to the bit.

use std::sync::{Arc, OnceLock};

use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::feature::ListedBinary;
use crate::galaxy::catalogue_classes::{CatalogueMotion, ClassId};
use crate::galaxy::features::FeatureId;
use crate::galaxy::features::FeatureProcess;
use crate::galaxy::features::catalogue::{FeatureCellCache, FeatureCellContents, FeatureRecord};
use crate::galaxy::features::centre::events::victims::victim_count;
use crate::galaxy::features::centre::recent::{CentreRecent, TracerProfiles};
use crate::galaxy::features::members::FeatureLevelList;
use crate::galaxy::features::members::recent::{FeatureStars, RecentClass, RecentEntry};
use crate::galaxy::placement::SystemRecord;
use crate::id::FeatureCell;
use crate::stellar::Composition;
use crate::stellar::binary::scan::{AwdScanMarks, XrbScanMarks};
use crate::stellar::remnant::NatalKick;
use crate::time::UniverseTime;
use crate::units::consts::SPEED_OF_LIGHT;

use super::super::oneshot::{OneShotEvent, listed_merger_event};
use super::super::recurrent::{EruptionHost, StarRecipe};
use super::Death;

/// Where a caller keeps the catalogue features' and the galactic centre's lists as the alert walks
/// read them, between walks (module documentation).
///
/// Like [`FeatureCellCache`] it takes `&self`, so an implementation that keeps lists uses interior
/// mutability, and it must lend each exactly as [`ListedCell::of`] and [`ListedHosts::of_centre`]
/// build it. **An implementation that keeps them between calls keys them by galaxy as well as by
/// cell**: the trait does not check the galaxy, and a bare galaxy and its
/// [`with_full_potential`](Galaxy::with_full_potential), which share a seed, list differently.
///
/// # Examples
///
/// A cache for one galaxy that keeps every list it builds, and every feature's told deaths'
/// entries, so that a second walk of the same sphere, or another class's, builds none:
///
/// ```
/// use std::collections::BTreeMap;
/// use std::sync::{Arc, Mutex, OnceLock};
///
/// use hyperion_sim::alerts::{ListedCell, ListedHosts, ListedHostsCache, ToldEntries};
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::FeatureId;
/// use hyperion_sim::galaxy::features::catalogue::FeatureCellCache;
/// use hyperion_sim::id::FeatureCell;
///
/// #[derive(Default)]
/// struct KeepLists {
///     cells: Mutex<BTreeMap<[i32; 3], Arc<ListedCell>>>,
///     centre: OnceLock<Arc<ListedHosts>>,
///     told: Mutex<BTreeMap<FeatureId, Arc<ToldEntries>>>,
/// }
///
/// impl ListedHostsCache for KeepLists {
///     fn cell(
///         &self,
///         galaxy: &Galaxy,
///         cell: FeatureCell,
///         features: &dyn FeatureCellCache,
///     ) -> Arc<ListedCell> {
///         let mut kept = self.cells.lock().expect("no keeper panics");
///         let lists = kept.entry(cell.to_array()).or_insert_with(|| {
///             Arc::new(ListedCell::of(galaxy, &features.contents(galaxy, cell)))
///         });
///         Arc::clone(lists)
///     }
///
///     fn centre(&self, galaxy: &Galaxy) -> Arc<ListedHosts> {
///         Arc::clone(self.centre.get_or_init(|| Arc::new(ListedHosts::of_centre(galaxy))))
///     }
///
///     fn told_entries(
///         &self,
///         _galaxy: &Galaxy,
///         feature: FeatureId,
///         build: &mut dyn FnMut() -> ToldEntries,
///     ) -> Arc<ToldEntries> {
///         let mut kept = self.told.lock().expect("no keeper panics");
///         Arc::clone(kept.entry(feature).or_insert_with(|| Arc::new(build())))
///     }
/// }
/// # let _ = KeepLists::default();
/// ```
pub trait ListedHostsCache {
    /// The lists of the features of `cell` in `galaxy` as the walks read them, the cell's whole
    /// contents read through `features`: exactly what [`ListedCell::of`] builds of them.
    fn cell(
        &self,
        galaxy: &Galaxy,
        cell: FeatureCell,
        features: &dyn FeatureCellCache,
    ) -> Arc<ListedCell>;

    /// The galactic centre's list in `galaxy` as the walks read it: exactly what
    /// [`ListedHosts::of_centre`] builds.
    ///
    /// # Panics
    ///
    /// As [`ListedHosts::of_centre`].
    fn centre(&self, galaxy: &Galaxy) -> Arc<ListedHosts>;

    /// The entries of the core collapses told on the list of `feature` in `galaxy`, every one
    /// built: exactly what `build` makes ([`ToldEntries`]). By default built afresh each time,
    /// every told death's entry of the feature where a walk without a cache built only those its
    /// cut took; a cache that keeps them keys them by galaxy and feature, calls `build` once for
    /// each, and charges them as they are built (perf-alert-walks-cpu).
    fn told_entries(
        &self,
        _galaxy: &Galaxy,
        _feature: FeatureId,
        build: &mut dyn FnMut() -> ToldEntries,
    ) -> Arc<ToldEntries> {
        Arc::new(build())
    }
}

/// The cache that keeps nothing: each lookup builds its cell's lists, or the centre's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoListedHostsCache;

impl ListedHostsCache for NoListedHostsCache {
    fn cell(
        &self,
        galaxy: &Galaxy,
        cell: FeatureCell,
        features: &dyn FeatureCellCache,
    ) -> Arc<ListedCell> {
        Arc::new(ListedCell::of(galaxy, &features.contents(galaxy, cell)))
    }

    fn centre(&self, galaxy: &Galaxy) -> Arc<ListedHosts> {
        Arc::new(ListedHosts::of_centre(galaxy))
    }
}

/// The lists of one feature cell's features as the alert walks read them (module documentation):
/// each feature's that registers a class.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedCell {
    cell: FeatureCell,
    /// Each feature of the cell that registers a class, in index order, with its index.
    lists: Box<[(u16, ListedHosts)]>,
}

impl ListedCell {
    /// The lists of the features of `contents`, a whole feature cell of `galaxy`
    /// ([`FeatureCatalogue::cell`](crate::galaxy::features::catalogue::FeatureCatalogue::cell)):
    /// each class's expected candidates and count, and its hosts when a walk first reads them.
    ///
    /// # Panics
    ///
    /// If `contents` holds only some of the cell's processes, which would leave a feature a later
    /// walk can find without its list.
    #[must_use]
    pub fn of(galaxy: &Galaxy, contents: &FeatureCellContents) -> Self {
        let last = FeatureProcess::ALL[FeatureProcess::ALL.len() - 1];
        assert!(
            contents.holds(last),
            "a cell's lists are of its whole contents, every process's"
        );
        let lists = contents
            .features()
            .iter()
            .filter_map(|feature| {
                ListedHosts::of_feature(galaxy, feature).map(|hosts| (feature.id().index(), hosts))
            })
            .collect();
        Self {
            cell: contents.cell(),
            lists,
        }
    }

    /// The cell.
    #[must_use]
    pub const fn cell(&self) -> FeatureCell {
        self.cell
    }

    /// The list of `feature`, one of the cell's, or `None` for a feature that registers no class.
    #[must_use]
    pub(super) fn hosts(&self, feature: &FeatureRecord) -> Option<&ListedHosts> {
        debug_assert_eq!(feature.id().cell(), self.cell, "a feature of the cell's");
        let index = feature.id().index();
        self.lists
            .binary_search_by_key(&index, |(i, _)| *i)
            .ok()
            .map(|at| &self.lists[at].1)
    }

    /// The bytes it owns on the heap: its lists' buffer, by its length, and what each list owns
    /// ([`ListedHosts::heap_bytes`]), whether or not a walk has yet found its hosts: what a
    /// caller's byte-bounded cache charges it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.lists.len() * size_of::<(u16, ListedHosts)>()
            + self
                .lists
                .iter()
                .map(|(_, hosts)| hosts.heap_bytes())
                .sum::<usize>()
    }
}

/// Which list a [`ListedHosts`] is.
#[derive(Debug, Clone, PartialEq)]
enum ListOf {
    /// A catalogue feature's.
    Feature,
    /// The galactic centre's, with the fastest speed over light's of the candidates of each class
    /// with a candidate, in list order, found when a walk first needs it.
    Centre(Box<[Found<f64>]>),
}

/// What a walk finds of a list, found when first asked for: a function of the list and its galaxy,
/// so no part of its value, and two lists are equal whatever either has found (as a
/// `FeatureLevelList`'s kept deaths are).
#[derive(Debug, Clone)]
struct Found<T>(OnceLock<T>);

impl<T> Default for Found<T> {
    fn default() -> Self {
        Self(OnceLock::new())
    }
}

impl<T> PartialEq for Found<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// A core collapse on a feature's list whose first steps keep its death: its index, its death's
/// time and its remnant's kick, read before its place (`FeatureStars::core_collapse_death`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ToldDeath {
    pub(super) index: u16,
    pub(super) death: UniverseTime,
    pub(super) kick: Option<NatalKick>,
}

/// The entries of a feature's core collapses whose deaths its list tells, read first from their
/// lifetimes, every one built, in index order, each with the time its death was told
/// (perf-alert-walks-cpu).
///
/// A walk builds them the first time its cut takes one of the feature's told deaths, and a caller's
/// [`ListedHostsCache`] may keep them, so that each later walk filters them by its own window and
/// builds none: they are a pure function of the galaxy and the feature, and what a cache holds
/// changes no walk's answer. A cache charges them as it keeps them ([`heap_bytes`](Self::heap_bytes)).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToldEntries {
    entries: Box<[(UniverseTime, Death)]>,
}

impl ToldEntries {
    /// The entries of `told`, `feature`'s told deaths in `galaxy`, from its own list: each that is
    /// one, in index order. Its entries' places read its profile's table
    /// (`FeatureLevelList::core_collapse_told`).
    #[must_use]
    pub(super) fn of(galaxy: &Galaxy, feature: &FeatureRecord, told: &[ToldDeath]) -> Self {
        let list = FeatureLevelList::of(galaxy, feature);
        let entries = told
            .iter()
            .filter_map(|told| {
                list.core_collapse_told(galaxy, told.index, (told.death, told.kick))
                    .and_then(|entry| Death::of_feature(galaxy, &entry))
                    .map(|death| (told.death, death))
            })
            .collect();
        Self { entries }
    }

    /// Each entry, with the time its death was told, in index order.
    #[must_use]
    pub(super) fn entries(&self) -> &[(UniverseTime, Death)] {
        &self.entries
    }

    /// The bytes it owns on the heap, its entries' buffer: what a caller's byte-bounded cache
    /// charges it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.entries.len() * size_of::<(UniverseTime, Death)>()
    }
}

/// One class's hosts on a list as the walks read them (module documentation), in index order.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ClassHosts {
    /// A feature's core collapses, their deaths told.
    Told(Box<[ToldDeath]>),
    /// Deaths built whole: a feature's Type Ia, and the centre's core collapses and Type Ia.
    Deaths(Box<[Death]>),
    /// The luminous blue variables.
    Eruptions(Box<[EruptionHost]>),
    /// The accreting white dwarfs, fast or slow, by their scan marks.
    Novae(Box<[AwdScanMarks]>),
    /// The X-ray binaries, by their scan marks.
    XrayBinaries(Box<[XrbScanMarks]>),
    /// The mergers' events: luminous red novae or kilonovae.
    Mergers(Box<[OneShotEvent]>),
}

/// A catalogue feature's or the galactic centre's list as the alert walks read it (module
/// documentation): each class it registers, with its expected candidates and its count, and the
/// hosts of each class with a candidate, found when a walk of the class first reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedHosts {
    of: ListOf,
    /// Each class it registers, in list order, with its expected candidates and its count.
    classes: Box<[(ClassId, f64, u32)]>,
    /// The hosts of each class with a candidate, in list order.
    hosts: Box<[Found<ClassHosts>]>,
}

impl ListedHosts {
    /// `feature`'s list in `galaxy` as the walks read it, or `None` for a feature that registers no
    /// class.
    #[must_use]
    fn of_feature(galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Self> {
        let classes = FeatureLevelList::class_counts_of(galaxy, feature);
        (!classes.is_empty()).then(|| Self::of_classes(ListOf::Feature, classes))
    }

    /// The galactic centre's list in `galaxy` as the walks read it: its recent classes and plan
    /// 11's binary classes after its victims.
    ///
    /// # Panics
    ///
    /// If the galaxy's centre has no model or no disruptions' model, which no galaxy the builder
    /// allows reaches.
    #[must_use]
    pub fn of_centre(galaxy: &Galaxy) -> Self {
        let list = centre_recent(galaxy).list(galaxy);
        let classes: Vec<_> = list.class_counts().collect();
        let fastest = (0..slots(&classes)).map(|_| Found::default()).collect();
        Self::of_classes(ListOf::Centre(fastest), classes)
    }

    /// The list `of` of `classes`, its hosts not yet found.
    #[must_use]
    fn of_classes(of: ListOf, classes: Vec<(ClassId, f64, u32)>) -> Self {
        let slots = slots(&classes);
        Self {
            of,
            classes: classes.into_boxed_slice(),
            hosts: (0..slots).map(|_| Found::default()).collect(),
        }
    }

    /// The expected candidates and the count of `class` on the list: none for a class it does not
    /// register.
    #[must_use]
    pub(super) fn class(&self, class: ClassId) -> (f64, u32) {
        self.classes
            .iter()
            .find(|&&(c, _, _)| c == class)
            .map_or((0.0, 0), |&(_, expected, count)| (expected, count))
    }

    /// The slot of `class`'s hosts among the classes with a candidate, or `None` for a class with
    /// no candidate on the list.
    #[must_use]
    fn slot(&self, class: ClassId) -> Option<usize> {
        self.classes
            .iter()
            .filter(|&&(_, _, count)| count > 0)
            .position(|&(c, _, _)| c == class)
    }

    /// The hosts of `class` on the list of `feature` in `galaxy`, its own list, found when first
    /// asked and kept; `None` for a class with no candidate on it.
    #[must_use]
    pub(super) fn feature_hosts(
        &self,
        galaxy: &Galaxy,
        feature: &FeatureRecord,
        class: ClassId,
    ) -> Option<&ClassHosts> {
        debug_assert_eq!(self.of, ListOf::Feature, "a feature's list");
        let (_, count) = self.class(class);
        self.slot(class).map(|slot| {
            self.hosts[slot]
                .0
                .get_or_init(|| feature_hosts(galaxy, feature, (class, count)))
        })
    }

    /// The hosts of `class` on the galactic centre's list in `galaxy`, the list's own, found when
    /// first asked and kept; `None` for a class with no candidate on it.
    ///
    /// # Panics
    ///
    /// As [`of_centre`](Self::of_centre).
    #[must_use]
    pub(super) fn centre_hosts(&self, galaxy: &Galaxy, class: ClassId) -> Option<&ClassHosts> {
        debug_assert!(matches!(self.of, ListOf::Centre(_)), "the centre's list");
        self.slot(class).map(|slot| {
            self.hosts[slot]
                .0
                .get_or_init(|| centre_hosts(galaxy, class))
        })
    }

    /// The fastest speed over light's of the candidates of `class` on the galactic centre's list in
    /// `galaxy`, its places' (`CentrePlace::velocity`), which bounds its hosts' travel: found when
    /// first asked and kept; 0 for a class with no candidate on it.
    ///
    /// # Panics
    ///
    /// As [`of_centre`](Self::of_centre), and if the list is a feature's.
    #[must_use]
    pub(super) fn centre_fastest(&self, galaxy: &Galaxy, class: ClassId) -> f64 {
        let ListOf::Centre(fastest) = &self.of else {
            panic!("the fastest of a list's candidates is asked of the centre's");
        };
        self.slot(class).map_or(0.0, |slot| {
            *fastest[slot]
                .0
                .get_or_init(|| centre_fastest(galaxy, class))
        })
    }

    /// The bytes it owns on the heap: its classes' and slots' buffers, and each class's hosts at a
    /// host for each candidate, whether or not a walk has yet found them, so that a cache charges a
    /// list the same whatever has read it (a class's hosts are its candidates that its test keeps,
    /// and own nothing on the heap themselves).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let of = match &self.of {
            ListOf::Feature => 0,
            ListOf::Centre(fastest) => fastest.len() * size_of::<Found<f64>>(),
        };
        self.classes.len() * size_of::<(ClassId, f64, u32)>()
            + self.hosts.len() * size_of::<Found<ClassHosts>>()
            + of
            + self
                .classes
                .iter()
                .map(|&(class, _, count)| {
                    usize::try_from(count)
                        .unwrap_or(usize::MAX)
                        .saturating_mul(host_bytes(&self.of, class))
                })
                .fold(0_usize, usize::saturating_add)
    }
}

/// How many of `classes` have a candidate: a list's slots.
#[must_use]
fn slots(classes: &[(ClassId, f64, u32)]) -> usize {
    classes.iter().filter(|&&(_, _, count)| count > 0).count()
}

/// The bytes one host of `class` takes on the list `of` ([`ClassHosts`]).
#[must_use]
fn host_bytes(of: &ListOf, class: ClassId) -> usize {
    match class {
        ClassId::CORE_COLLAPSE if *of == ListOf::Feature => size_of::<ToldDeath>(),
        ClassId::CORE_COLLAPSE | ClassId::TYPE_IA => size_of::<Death>(),
        ClassId::LUMINOUS_BLUE_VARIABLE => size_of::<EruptionHost>(),
        ClassId::ACCRETING_WHITE_DWARF | ClassId::ACCRETING_WHITE_DWARF_SLOW => {
            size_of::<AwdScanMarks>()
        }
        ClassId::XRAY_BINARY => size_of::<XrbScanMarks>(),
        _ => size_of::<OneShotEvent>(),
    }
}

/// The hosts of `class`, of `count` candidates, on `feature`'s list in `galaxy` ([`ClassHosts`]):
/// its core collapses told from their stars alone, the rest found on its list, entry by entry, as
/// its tests keep them.
#[must_use]
fn feature_hosts(
    galaxy: &Galaxy,
    feature: &FeatureRecord,
    (class, count): (ClassId, u32),
) -> ClassHosts {
    if class == ClassId::CORE_COLLAPSE {
        // The first class of a catalogue feature's list, at indices 1 upward (`deaths_of`).
        let told = FeatureStars::listing(galaxy, feature).map_or_else(Vec::new, |stars| {
            (1..=count)
                .filter_map(|number| {
                    let index = u16::try_from(number).ok()?;
                    let (record, range) = stars.core_collapse_draws(galaxy, index)?;
                    let death = stars.core_collapse_death(galaxy, record, range)?;
                    Some(ToldDeath {
                        index,
                        death: death.death(),
                        kick: death.kick(),
                    })
                })
                .collect()
        });
        return ClassHosts::Told(told.into_boxed_slice());
    }
    let list = FeatureLevelList::of(galaxy, feature);
    let indices = list
        .entries()
        .filter(|entry| entry.class() == class)
        .map(|entry| entry.index());
    let recent = |index| list.recent_entry(galaxy, index);
    let binary = |index| list.binary_entry(galaxy, index);
    match class {
        ClassId::TYPE_IA => ClassHosts::Deaths(
            indices
                .filter_map(recent)
                .filter_map(|entry| Death::of_feature(galaxy, &entry))
                .collect(),
        ),
        ClassId::LUMINOUS_BLUE_VARIABLE => ClassHosts::Eruptions(
            indices
                .filter_map(recent)
                .filter_map(|entry| listed_variable(galaxy, &entry))
                .collect(),
        ),
        _ => binary_hosts(galaxy, class, indices.filter_map(binary)),
    }
}

/// The luminous blue variable `entry` of a feature's list as the walks read it: its system, its
/// primary as the stellar stage builds it and the window in which that is one (plan 06's
/// `StarModel::lbv_window`, which every eruption falls in), on its line at its feature's motion.
#[must_use]
fn listed_variable(galaxy: &Galaxy, entry: &RecentEntry) -> Option<EruptionHost> {
    let member = entry.member();
    eruption_host(
        galaxy,
        member.record(),
        *member.composition(),
        *entry.motion(),
    )
}

/// The eruption host of the luminous blue variable `record` at `composition`, moving by
/// `motion`, or `None` if its primary is none, which a variable its class's test keeps never is.
#[must_use]
fn eruption_host(
    galaxy: &Galaxy,
    record: &SystemRecord,
    composition: Composition,
    motion: CatalogueMotion,
) -> Option<EruptionHost> {
    let star = StarRecipe::of_record(galaxy, record, composition);
    let window = star.to_star().lbv_window()?;
    // A list's motions are below light's speed and a window runs forward.
    EruptionHost::new(record.id(), window, motion, star).ok()
}

/// The hosts of plan 11's binary class `class` among `entries`, a list's in index order
/// ([`ClassHosts`]).
#[must_use]
fn binary_hosts(
    galaxy: &Galaxy,
    class: ClassId,
    entries: impl Iterator<Item = ListedBinary>,
) -> ClassHosts {
    match class {
        ClassId::ACCRETING_WHITE_DWARF | ClassId::ACCRETING_WHITE_DWARF_SLOW => ClassHosts::Novae(
            entries
                .filter_map(|entry| match entry {
                    ListedBinary::AccretingWd(entry) => Some(*entry.scan()),
                    ListedBinary::XrayBinary(_)
                    | ListedBinary::StellarMerger(_)
                    | ListedBinary::NeutronStarMerger(_) => None,
                })
                .collect(),
        ),
        ClassId::XRAY_BINARY => ClassHosts::XrayBinaries(
            entries
                .filter_map(|entry| match entry {
                    ListedBinary::XrayBinary(entry) => Some(*entry.scan()),
                    ListedBinary::AccretingWd(_)
                    | ListedBinary::StellarMerger(_)
                    | ListedBinary::NeutronStarMerger(_) => None,
                })
                .collect(),
        ),
        _ => ClassHosts::Mergers(
            entries
                .filter_map(|entry| listed_merger_event(galaxy, &entry))
                .collect(),
        ),
    }
}

/// The galactic centre's recent classes in `galaxy`.
///
/// # Panics
///
/// As [`ListedHosts::of_centre`].
#[must_use]
fn centre_recent(galaxy: &Galaxy) -> CentreRecent {
    let model = galaxy
        .centre()
        .expect("no galaxy the builder allows fails its centre (CentreModel::new's tests)");
    let victims =
        victim_count(galaxy).expect("no galaxy the builder allows fails its disruptions' model");
    CentreRecent::new(galaxy, model, victims)
}

/// The fastest speed over light's of the candidates of `class` on the galactic centre's list in
/// `galaxy`, their places' ([`ListedHosts::centre_fastest`]).
///
/// # Panics
///
/// As [`ListedHosts::of_centre`].
#[must_use]
fn centre_fastest(galaxy: &Galaxy, class: ClassId) -> f64 {
    let model = galaxy
        .centre()
        .expect("no galaxy the builder allows fails its centre (CentreModel::new's tests)");
    let recent = centre_recent(galaxy);
    let list = recent.list(galaxy);
    let profiles = TracerProfiles::of(model);
    list.entries()
        .filter(|entry| entry.class() == class)
        .filter_map(|entry| match RecentClass::of(class) {
            Some(recent_class) => recent
                .place(galaxy, model, &profiles, recent_class, entry.index())
                .map(|place| place.velocity()),
            None => recent
                .binaries()
                .place(galaxy, model, &profiles, class, entry.index())
                .map(|place| place.velocity()),
        })
        .map(|velocity| velocity.speed().value())
        .fold(0.0, f64::max)
        / SPEED_OF_LIGHT
}

/// The hosts of `class` on the galactic centre's list in `galaxy` ([`ClassHosts`]): each entry
/// placed and kept by its class's test, as the first lane's walk read them, its deaths built whole.
///
/// # Panics
///
/// As [`ListedHosts::of_centre`].
#[must_use]
fn centre_hosts(galaxy: &Galaxy, class: ClassId) -> ClassHosts {
    let model = galaxy
        .centre()
        .expect("no galaxy the builder allows fails its centre (CentreModel::new's tests)");
    let recent = centre_recent(galaxy);
    let list = recent.list(galaxy);
    let profiles = TracerProfiles::of(model);
    let indices = list
        .entries()
        .filter(|entry| entry.class() == class)
        .map(|entry| entry.index());
    match RecentClass::of(class) {
        Some(recent_class @ (RecentClass::CoreCollapse | RecentClass::TypeIa)) => {
            ClassHosts::Deaths(
                indices
                    .filter_map(|index| {
                        let kept = recent
                            .place(galaxy, model, &profiles, recent_class, index)?
                            .kept(galaxy)?;
                        let (site, kind) = (kept.site()?, kept.explosion_kind()?);
                        let member = kept.member();
                        Some(Death::of_parts(
                            galaxy,
                            (member.record(), *member.composition()),
                            kind,
                            site.death(),
                            (*kept.motion(), kept.supernova()),
                        ))
                    })
                    .collect(),
            )
        }
        Some(RecentClass::LuminousBlueVariable) => ClassHosts::Eruptions(
            indices
                .filter_map(|index| {
                    let kept = recent
                        .place(
                            galaxy,
                            model,
                            &profiles,
                            RecentClass::LuminousBlueVariable,
                            index,
                        )?
                        .kept(galaxy)?;
                    let member = kept.member();
                    eruption_host(
                        galaxy,
                        member.record(),
                        *member.composition(),
                        *kept.motion(),
                    )
                })
                .collect(),
        ),
        None => binary_hosts(
            galaxy,
            class,
            indices.filter_map(|index| {
                recent
                    .binaries()
                    .place(galaxy, model, &profiles, class, index)?
                    .kept(galaxy)
                    .map(|(entry, _)| entry)
            }),
        ),
    }
}
