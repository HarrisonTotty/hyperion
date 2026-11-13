//! "What went off?" (plan 12, P12.T5.d): the census per class, each class's walk, the galactic
//! centre on plan 10's global list and a range result's local events, merged.
//!
//! [`alerts_in`] answers one [`AlertQuery`] in four parts, each a walk as built, and merges their
//! detections by arrival and then by event:
//!
//! 1. **The catalogue classes**, each asked about in class order, through a lookup of their walks
//!    ([`AlertClasses`]; [`ProductionAlertClasses`] in production). A class is complete or yields
//!    nothing, decided before walking (Design note 7, [`class_alerts`]):
//!    - its reach is [`class_reach`] at the query's sensor, widened by
//!      [`UNBOUND_PAD_SPEED`] ÷ c, 1%, since a host was where its light left it (Design note 5);
//!    - its walk spans the class's cells that can hold an entry within that sphere
//!      ([`cells_in_sphere`]). Past [`BACKGROUND_THRESHOLD_CELLS`] of them the class is
//!      [`NeedsBackgroundScan`], uncounted: a cell costs its bound and its draw even when empty,
//!      and up to milliseconds when not, so a walk over the whole cube's 16.8 million cells of 512
//!      ly is no interactive walk whatever its hosts (plan 12's Risks, "P12.T5.b as built" and
//!      "P12.T5.c as built": "the census must count the cells it would walk");
//!    - its expected hosts are the candidates those cells draw, Σ bound × volume over them
//!      ([`ClassProcess::bound`]): an upper bound on its hosts in reach, which the class's
//!      thinning and test only lower, and the walk's own cost. Over the query's
//!      [`host_limit`](AlertQuery::host_limit) the class is [`OverLimit`]; over
//!      [`BACKGROUND_THRESHOLD_HOSTS`], [`NeedsBackgroundScan`]; otherwise it is walked and
//!      [`Complete`];
//!    - a class whose walk or source is not built is [`Unwalked`], with the task that builds it.
//! 2. **The galactic centre**, the first entry of the global list's entries with class members
//!    ([`GlobalList::entries_with_class_members`]), through [`centre_detections`]: its black hole's
//!    flares, which have no class, and the disruptions of the victims on its feature-level list,
//!    [`ClassId::TIDAL_DISRUPTION_VICTIM`]'s hosts, whose census is the list's count. Without a
//!    global list in the context ([`AlertContext::with_global_list`]) neither is walked.
//! 3. **The other sources** of class members ([`SourceCensus`]): the dwarf cores' feature-level
//!    lists on the global list, which hold none yet and answer none (P10.T7.b), and the catalogue
//!    features'. From generator version 29 every class's walk reads the catalogue features' and
//!    the centre's lists of its hosts beside its cells, under its own census (part 1,
//!    [`listed`](super::listed)): their recently dead (P09.T36), their luminous blue variables
//!    (P09.T43) and plan 11's binary classes (P11.T8.f), so the lists' source is complete; below
//!    it, it names the task that walks them (P09.T36).
//! 4. **The local events** of an observed range result's systems ([`local_events`], P12.T5.e),
//!    with the same query.
//!
//! The parts share no host: a range query finds no catalogue class's host and no centre event,
//! and each event is its own source's. So a window split in two gives each onset of the whole
//! once, in the half its light arrives in, and the first half the whole's events in progress, as
//! each walk does (Design note 6), and the result does not depend on the order of the classes,
//! the caches lent or what was asked before.
//!
//! [`NeedsBackgroundScan`]: ClassStatus::NeedsBackgroundScan
//! [`OverLimit`]: ClassStatus::OverLimit
//! [`Complete`]: ClassStatus::Complete
//! [`Unwalked`]: ClassStatus::Unwalked
//! [`ClassProcess::bound`]: crate::galaxy::catalogue_classes::ClassProcess::bound

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::binary::{
    AccretingWdFast, AccretingWdSlow, NeutronStarMergers, StellarMergers, XrayBinaries,
};
use crate::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use crate::galaxy::catalogue_classes::lbv::LbvProcess;
use crate::galaxy::catalogue_classes::type_ia::TypeIaProcess;
use crate::galaxy::catalogue_classes::{ClassId, ClassProcess, cells_in_sphere};
use crate::galaxy::features::centre::events::CentreVictims;
use crate::galaxy::global_list::{GlobalEntryKind, GlobalList};
use crate::galaxy::query::{QuerySphere, UNBOUND_PAD_SPEED};
use crate::observe::{ObservedSystem, StarsCache};
use crate::units::consts::SPEED_OF_LIGHT;
use crate::units::{LightYears, MetresPerSecond};

use super::centre::centre_detections;
use super::listed::ListedWalk;
use super::local::{LocalEventsCache, local_events};
use super::oneshot::one_shot_detections;
use super::reach::class_reach;
use super::recurrent::{magnetar_detections, recurrent_detections};
use super::{
    AlertContext, AlertQuery, AlertResult, BACKGROUND_THRESHOLD_HOSTS, ClassCensus, ClassStatus,
    Detection, Unwalked,
};

/// Above this many class cells in its walk a class is not walked interactively: `alerts_in`
/// reports it [`ClassStatus::NeedsBackgroundScan`] without counting its hosts (module
/// documentation): 10⁵, the cells of a 512 ly class within some 10,500 ly, whose bounds alone
/// take 47–82 ms to sum for the supernova classes (pod, 16 vCPU, P12.T5.d; provisional).
pub const BACKGROUND_THRESHOLD_CELLS: usize = 100_000;

/// A source of class members beyond the class grids that [`alerts_in`] reads (module
/// documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AlertSource {
    /// The galactic centre, the global list's first entry: its black hole's flares and the
    /// disruptions of its feature-level list's victims.
    Centre,
    /// The dwarf cores of the global list, through their feature-level lists.
    DwarfCores,
    /// Plan 09's catalogue features within reach, through their feature-level lists. From
    /// generator version 29 every class they hold is walked with its class (part 1 of the module
    /// documentation), and this source is complete; below it, unwalked.
    FeatureLists,
}

impl AlertSource {
    /// Every source, in the result's order.
    pub const ALL: [Self; 3] = [Self::Centre, Self::DwarfCores, Self::FeatureLists];
}

/// How a source's walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceStatus {
    /// Every class member it holds was walked: its detections are complete.
    Complete,
    /// It was not walked, for the reason given: no detections.
    Unwalked(Unwalked),
}

/// One source's census in an [`AlertResult`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceCensus {
    source: AlertSource,
    status: SourceStatus,
}

impl SourceCensus {
    /// The source.
    #[must_use]
    pub const fn source(&self) -> AlertSource {
        self.source
    }

    /// How its walk ended.
    #[must_use]
    pub const fn status(&self) -> SourceStatus {
        self.status
    }
}

/// The catalogue classes' walks that [`alerts_in_with`] asks, one class at a time: the production
/// classes ([`ProductionAlertClasses`]), or a test's.
///
/// An implementation answers with [`class_alerts`] over the class's process, which decides the
/// census and calls the walk only for a class it finds complete, or with [`unwalked_class`] for a
/// class whose walk it does not have. [`ClassId::TIDAL_DISRUPTION_VICTIM`] is never asked: its
/// hosts are the centre's, which [`alerts_in_with`] walks through the global list.
///
/// # Examples
///
/// A lookup that walks the core collapses' supernovae alone, without their magnetars:
///
/// ```
/// use hyperion_sim::alerts::{
///     AlertClasses, AlertContext, AlertQuery, ClassCensus, Detection, Unwalked, class_alerts,
///     one_shot_detections, unwalked_class,
/// };
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
/// use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
///
/// struct SupernovaeOnly;
///
/// impl AlertClasses for SupernovaeOnly {
///     fn class_alerts(
///         &self,
///         galaxy: &Galaxy,
///         ctx: &mut AlertContext<'_>,
///         query: &AlertQuery,
///         class: ClassId,
///         out: &mut Vec<Detection>,
///     ) -> ClassCensus {
///         if class != ClassId::CORE_COLLAPSE {
///             return unwalked_class(class, query, Unwalked::NotBuilt("a test's"));
///         }
///         let process = || CoreCollapseProcess::new(galaxy);
///         class_alerts(galaxy, query, class, process, |process| {
///             one_shot_detections(galaxy, ctx, query, process, out);
///         })
///     }
/// }
/// # let _ = SupernovaeOnly;
/// ```
pub trait AlertClasses {
    /// The census of `class` for `query` (Design note 7), its detections appended to `out` if it
    /// is complete, in any order.
    fn class_alerts(
        &self,
        galaxy: &Galaxy,
        ctx: &mut AlertContext<'_>,
        query: &AlertQuery,
        class: ClassId,
        out: &mut Vec<Detection>,
    ) -> ClassCensus;
}

/// The production walks of the catalogue classes, as built:
///
/// - [`ClassId::CORE_COLLAPSE`]: its supernovae ([`one_shot_detections`], P12.T5.b) and its
///   magnetars ([`magnetar_detections`], P12.T5.c), on plan 09's process;
/// - [`ClassId::TYPE_IA`]: its supernovae, on plan 09's process (P09.T34.a);
/// - [`ClassId::STELLAR_MERGER`] and [`ClassId::NEUTRON_STAR_MERGER`]: their luminous red novae
///   and kilonovae ([`one_shot_detections`], P12.T5.b), on plan 11's processes, read from their
///   scan marks;
/// - [`ClassId::ACCRETING_WHITE_DWARF`] and [`ClassId::ACCRETING_WHITE_DWARF_SLOW`]: their novae,
///   [`ClassId::XRAY_BINARY`]: its transients, Type I bursts and Be/X outbursts, and
///   [`ClassId::LUMINOUS_BLUE_VARIABLE`]: its giant eruptions ([`recurrent_detections`],
///   P12.T5.c), each counted on its own process; the dwarf novae are local (P12.T5.e).
///
/// From generator version 29 each class's walk reads the hosts of the features' and the centre's
/// lists beside its cells ([`listed`](super::listed)). Every class but the centre's victims is
/// walked here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProductionAlertClasses;

impl AlertClasses for ProductionAlertClasses {
    fn class_alerts(
        &self,
        galaxy: &Galaxy,
        ctx: &mut AlertContext<'_>,
        query: &AlertQuery,
        class: ClassId,
        out: &mut Vec<Detection>,
    ) -> ClassCensus {
        match class {
            ClassId::CORE_COLLAPSE => {
                let process = || CoreCollapseProcess::new(galaxy);
                let walk = |ctx: &mut AlertContext<'_>, process: &_, out: &mut _| {
                    one_shot_detections(galaxy, ctx, query, process, out);
                    magnetar_detections(galaxy, ctx, query, process, out);
                };
                listed_class_alerts(galaxy, (ctx, query), class, process, walk, out)
            }
            ClassId::TYPE_IA => {
                // Its process costs some 37 ms (its delay-time distribution; pod, 16 vCPU), so it
                // is built only for a class whose cells are counted.
                let process = || TypeIaProcess::new(galaxy);
                let walk = |ctx: &mut AlertContext<'_>, process: &_, out: &mut _| {
                    one_shot_detections(galaxy, ctx, query, process, out);
                };
                listed_class_alerts(galaxy, (ctx, query), class, process, walk, out)
            }
            ClassId::STELLAR_MERGER => {
                let process = || StellarMergers::new(galaxy);
                let walk = |ctx: &mut AlertContext<'_>, process: &_, out: &mut _| {
                    one_shot_detections(galaxy, ctx, query, process, out);
                };
                listed_class_alerts(galaxy, (ctx, query), class, process, walk, out)
            }
            ClassId::NEUTRON_STAR_MERGER => {
                let process = || NeutronStarMergers::new(galaxy);
                let walk = |ctx: &mut AlertContext<'_>, process: &_, out: &mut _| {
                    one_shot_detections(galaxy, ctx, query, process, out);
                };
                listed_class_alerts(galaxy, (ctx, query), class, process, walk, out)
            }
            // The recurrent walk builds the class's process for each cell it scans (`scan_cell`);
            // the one here counts the hosts.
            ClassId::ACCRETING_WHITE_DWARF => {
                let process = || AccretingWdFast::new(galaxy);
                let walk = |ctx: &mut AlertContext<'_>, _: &_, out: &mut _| {
                    recurrent_detections(galaxy, ctx, class, query, out);
                };
                listed_class_alerts(galaxy, (ctx, query), class, process, walk, out)
            }
            ClassId::ACCRETING_WHITE_DWARF_SLOW => {
                let process = || AccretingWdSlow::new(galaxy);
                let walk = |ctx: &mut AlertContext<'_>, _: &_, out: &mut _| {
                    recurrent_detections(galaxy, ctx, class, query, out);
                };
                listed_class_alerts(galaxy, (ctx, query), class, process, walk, out)
            }
            ClassId::XRAY_BINARY => {
                let process = || XrayBinaries::new(galaxy);
                let walk = |ctx: &mut AlertContext<'_>, _: &_, out: &mut _| {
                    recurrent_detections(galaxy, ctx, class, query, out);
                };
                listed_class_alerts(galaxy, (ctx, query), class, process, walk, out)
            }
            ClassId::LUMINOUS_BLUE_VARIABLE => {
                let process = || LbvProcess::new(galaxy);
                let walk = |ctx: &mut AlertContext<'_>, _: &_, out: &mut _| {
                    recurrent_detections(galaxy, ctx, class, query, out);
                };
                listed_class_alerts(galaxy, (ctx, query), class, process, walk, out)
            }
            // The victims are the centre's, walked through the global list (trait
            // documentation); a class registered later is walked by the task that adds it.
            ClassId::TIDAL_DISRUPTION_VICTIM => {
                unwalked_class(class, query, Unwalked::NoGlobalList)
            }
            _ => unwalked_class(class, query, Unwalked::NotBuilt("P12.T5")),
        }
    }
}

/// `reach` widened by [`UNBOUND_PAD_SPEED`] ÷ c (Design note 5).
#[must_use]
fn widened(reach: LightYears) -> LightYears {
    let share = MetresPerSecond::from(UNBOUND_PAD_SPEED).value() / SPEED_OF_LIGHT;
    LightYears::new(reach.value() * (1.0 + share))
}

/// The census of `class` for `query` (module documentation), calling `walk` with its process only
/// if the class is complete: its reach, the candidates of its walk's cells, and its status.
///
/// `process` builds the class's process, only for a class whose cells are counted, since a
/// process can cost more than the census. A class the sensor cannot see at all, of no reach, is
/// complete with no host and is not walked.
///
/// # Panics
///
/// As the process's [`bound`](ClassProcess::bound), which a production class never does; and in
/// debug builds if the process is not of `class`.
///
/// # Examples
///
/// The census of the core collapses for a bright visual limit near the Sun: some 3,000 candidates
/// in the 2,800 cells within 4,096 ly of a reach of 440 ly, complete.
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::{AlertBand, AlertQuery, BandLimits, ClassStatus, SensorHorizon, class_alerts};
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
/// use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let w = WattsPerSquareMetre::new;
/// let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), BandLimits::new(w(1e-4), w(1e-3))?)])?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in the cube")?;
/// let year = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(1).ok_or("on the clock")?)?;
/// let query = AlertQuery::builder(sun, year, sensor).build()?;
/// let mut walked = false;
/// let class = ClassId::CORE_COLLAPSE;
/// let census = class_alerts(&galaxy, &query, class, || CoreCollapseProcess::new(&galaxy), |_| walked = true);
/// assert_eq!(census.status(), ClassStatus::Complete);
/// assert!(walked && census.expected_hosts().is_some_and(|n| n > 1_000.0));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn class_alerts<P: ClassProcess>(
    galaxy: &Galaxy,
    query: &AlertQuery,
    class: ClassId,
    process: impl FnOnce() -> P,
    walk: impl FnOnce(&P),
) -> ClassCensus {
    match counted_cells(galaxy, query, class, process) {
        Counted::Done(census) => census,
        Counted::Cells(cells) => {
            let status = status_of(cells.expected, query);
            if status == ClassStatus::Complete {
                walk(&cells.process);
            }
            cells.census(class, cells.expected, status)
        }
    }
}

/// [`class_alerts`] for a class whose hosts the features' and the centre's lists hold too
/// (P09.T36, P09.T43, P11.T8.f; [`listed`](super::listed)): where its cells alone leave it
/// complete, the lists its walk reads add their expected candidates to its cells', and a class
/// still complete is walked, `walk` over its cells, then its lists, into `out`.
/// A class its cells send to the background or put over its limit is not counted further: a
/// background walk counts the candidates its lists drew ([`listed_scan_slab`](super::listed_scan_slab)).
///
/// # Panics
///
/// As [`class_alerts`], and as the lists' walk does.
#[must_use]
fn listed_class_alerts<P: ClassProcess>(
    galaxy: &Galaxy,
    (ctx, query): (&mut AlertContext<'_>, &AlertQuery),
    class: ClassId,
    process: impl FnOnce() -> P,
    walk: impl FnOnce(&mut AlertContext<'_>, &P, &mut Vec<Detection>),
    out: &mut Vec<Detection>,
) -> ClassCensus {
    match counted_cells(galaxy, query, class, process) {
        Counted::Done(census) => census,
        Counted::Cells(cells) => {
            let listed = (status_of(cells.expected, query) == ClassStatus::Complete)
                .then(|| {
                    let lists = (ctx.feature_cells(), ctx.listed_hosts());
                    ListedWalk::find(galaxy, lists, query, class)
                })
                .flatten();
            let expected = listed
                .as_ref()
                .map_or(cells.expected, |listed| cells.expected + listed.expected());
            let status = status_of(expected, query);
            if status == ClassStatus::Complete {
                walk(ctx, &cells.process, out);
                if let Some(listed) = &listed {
                    listed.detections(galaxy, ctx, query, class, out);
                }
            }
            cells.census(class, expected, status)
        }
    }
}

/// A class's census as far as its cells: done, or its cells' expected hosts with its process.
enum Counted<P> {
    /// Decided without its hosts: a class the sensor cannot see, or one whose walk spans too many
    /// cells.
    Done(ClassCensus),
    /// Its cells counted.
    Cells(CountedCells<P>),
}

/// A class's reach, its cells' expected hosts and its process, built for them.
struct CountedCells<P> {
    reach: LightYears,
    expected: f64,
    process: P,
}

impl<P> CountedCells<P> {
    /// The census of `class` of `expected` hosts and `status`.
    #[must_use]
    const fn census(&self, class: ClassId, expected: f64, status: ClassStatus) -> ClassCensus {
        ClassCensus {
            class,
            reach: self.reach,
            expected_hosts: Some(expected),
            status,
        }
    }
}

/// [`class_alerts`]' census of `class`'s cells for `query` (module documentation): its reach, the
/// candidates of its walk's cells and the process `process` builds for them, or the census of a
/// class decided without them.
///
/// # Panics
///
/// As [`class_alerts`].
#[must_use]
fn counted_cells<P: ClassProcess>(
    galaxy: &Galaxy,
    query: &AlertQuery,
    class: ClassId,
    process: impl FnOnce() -> P,
) -> Counted<P> {
    let reach = class_reach(class, query.sensor());
    let census = |expected_hosts, status| ClassCensus {
        class,
        reach,
        expected_hosts,
        status,
    };
    if reach <= LightYears::ZERO {
        return Counted::Done(census(Some(0.0), ClassStatus::Complete));
    }
    let sphere = QuerySphere::new(
        *query.observer_position(),
        widened(reach),
        query.window().start(),
        LightYears::ZERO,
    )
    .expect("a reach is a positive length within a hundredth over the cube's diagonal");
    // Counted first, for nothing but the walk's integer steps, so that a class over the limit
    // costs no bounds.
    let cells = cells_in_sphere(class, &sphere)
        .take(BACKGROUND_THRESHOLD_CELLS + 1)
        .count();
    if cells > BACKGROUND_THRESHOLD_CELLS {
        return Counted::Done(census(None, ClassStatus::NeedsBackgroundScan));
    }
    let process = process();
    debug_assert_eq!(
        process.class(),
        class,
        "the process is of the class censused"
    );
    let expected = cells_in_sphere(class, &sphere).fold(0.0, |sum, key| {
        sum + process.bound(galaxy, &key).value() * key.volume_ly3()
    });
    Counted::Cells(CountedCells {
        reach,
        expected,
        process,
    })
}

/// The status of a class expecting `expected` hosts for `query` (Design note 7).
#[must_use]
fn status_of(expected: f64, query: &AlertQuery) -> ClassStatus {
    if expected > f64::from(query.host_limit().get()) {
        ClassStatus::OverLimit
    } else if expected > BACKGROUND_THRESHOLD_HOSTS {
        ClassStatus::NeedsBackgroundScan
    } else {
        ClassStatus::Complete
    }
}

/// The census of `class`, not walked for `why`: its reach at the query's sensor, no hosts
/// counted.
#[must_use]
pub fn unwalked_class(class: ClassId, query: &AlertQuery, why: Unwalked) -> ClassCensus {
    ClassCensus {
        class,
        reach: class_reach(class, query.sensor()),
        expected_hosts: None,
        status: ClassStatus::Unwalked(why),
    }
}

/// What went off for `query` (module documentation): the production classes' walks
/// ([`ProductionAlertClasses`]), the centre on the context's global list, and the local events of
/// `observed`, an observed range result's rows, whose stars come from `stars` and whose stars'
/// rates `cache` keeps ([`local_events`]).
///
/// A caller with no range result passes no rows, `&[]`, with any caches. The result does not
/// depend on what the caches held.
///
/// # Panics
///
/// As the walks do: [`one_shot_detections`], [`magnetar_detections`], [`recurrent_detections`],
/// [`centre_detections`] and [`local_events`]; and if the galaxy's centre has no model, which no
/// galaxy the builder allows reaches ([`CentreVictims::of`]).
///
/// # Examples
///
/// What a sensor with a sensitive X-ray band and a visual one sees go off in a year from near the
/// Sun: the supernova classes reach the whole Galaxy, so they wait for a background scan; the
/// centre is not walked without the global list; and there are no local rows.
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::alerts::local::LocalEventsCache;
/// use hyperion_sim::alerts::{
///     AlertBand, AlertContext, AlertQuery, BandLimits, ClassStatus, SensorHorizon, Unwalked,
///     alerts_in,
/// };
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::ClassId;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::NoStarsCache;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::WattsPerSquareMetre;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let w = WattsPerSquareMetre::new;
/// let sensor = SensorHorizon::new([
///     (AlertBand::Photometric(Band::V), BandLimits::new(w(1e-12), w(1e-10))?),
///     (AlertBand::XRay, BandLimits::new(w(1e-15), w(1e-13))?),
/// ])?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in the cube")?;
/// let year = TimeWindow::new(UniverseTime::EPOCH, UniverseTime::from_julian_years(1).ok_or("on the clock")?)?;
/// let query = AlertQuery::builder(sun, year, sensor).build()?;
/// let mut noise = NoiseCache::with_capacity(4_096);
/// let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
/// let result = alerts_in(&galaxy, &mut ctx, &query, &[], &mut NoStarsCache, &mut LocalEventsCache::default());
/// let census = |class| result.class_census(class).map(|c| c.status());
/// assert_eq!(census(ClassId::CORE_COLLAPSE), Some(ClassStatus::NeedsBackgroundScan));
/// assert_eq!(
///     census(ClassId::TIDAL_DISRUPTION_VICTIM),
///     Some(ClassStatus::Unwalked(Unwalked::NoGlobalList))
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn alerts_in<S: StarsCache>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    observed: &[ObservedSystem],
    stars: &mut S,
    cache: &mut LocalEventsCache,
) -> AlertResult {
    alerts_in_with(
        galaxy,
        ctx,
        query,
        &ProductionAlertClasses,
        observed,
        stars,
        cache,
    )
}

/// [`alerts_in`] with the classes' walks of `classes`.
///
/// # Panics
///
/// As [`alerts_in`], and as `classes` does.
pub fn alerts_in_with<L: AlertClasses + ?Sized, S: StarsCache>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    classes: &L,
    observed: &[ObservedSystem],
    stars: &mut S,
    cache: &mut LocalEventsCache,
) -> AlertResult {
    let mut detections = Vec::new();
    let mut census = Vec::new();
    for class in query.classes().iter() {
        if class != ClassId::TIDAL_DISRUPTION_VICTIM {
            census.push(classes.class_alerts(galaxy, ctx, query, class, &mut detections));
        }
    }
    let list = ctx.global_list();
    let victim_class = query.classes().contains(ClassId::TIDAL_DISRUPTION_VICTIM);
    let sources = if let Some(list) = list {
        let (victims, victims_census) = victims_of(galaxy, query, list, victim_class);
        census.extend(victims_census);
        if centre_of(list) {
            let walked = victims
                .filter(|_| victims_census.is_none_or(|c| c.status == ClassStatus::Complete));
            centre_detections(galaxy, ctx, query, walked.as_ref(), &mut detections);
        }
        let cores = list
            .entries_with_class_members()
            .any(|entry| matches!(entry.kind(), GlobalEntryKind::DwarfCore(_)));
        [
            SourceStatus::Complete,
            if cores {
                SourceStatus::Unwalked(Unwalked::NotBuilt("P10.T7.b"))
            } else {
                SourceStatus::Complete
            },
        ]
    } else {
        if victim_class {
            census.push(unwalked_class(
                ClassId::TIDAL_DISRUPTION_VICTIM,
                query,
                Unwalked::NoGlobalList,
            ));
        }
        [SourceStatus::Unwalked(Unwalked::NoGlobalList); 2]
    };
    let [centre, cores] = sources;
    let sources = vec![
        SourceCensus {
            source: AlertSource::Centre,
            status: centre,
        },
        SourceCensus {
            source: AlertSource::DwarfCores,
            status: cores,
        },
        SourceCensus {
            source: AlertSource::FeatureLists,
            status: ListedWalk::source_status(),
        },
    ];
    local_events(galaxy, ctx, query, observed, stars, cache, &mut detections);
    detections.sort_by(|a, b| {
        a.arrival()
            .cmp(&b.arrival())
            .then_with(|| a.event().cmp(&b.event()))
    });
    debug_assert!(
        detections
            .windows(2)
            .all(|pair| pair[0].event() != pair[1].event()),
        "no event is reported by two walks"
    );
    census.sort_by_key(|c| c.class);
    AlertResult {
        detections,
        census,
        sources,
    }
}

/// Whether `list` holds the galactic centre among its entries with class members.
#[must_use]
fn centre_of(list: &GlobalList) -> bool {
    list.entries_with_class_members()
        .any(|entry| entry.kind() == GlobalEntryKind::Centre)
}

/// The centre's victims, if `list` holds the centre and `asked` (the query asks about their
/// class), with their class's census: the list's victims are its hosts, all at the black hole,
/// counted when it lies within the class's widened reach.
///
/// # Panics
///
/// If the galaxy's centre has no model, which no galaxy the builder allows reaches
/// ([`CentreVictims::of`]).
#[must_use]
fn victims_of(
    galaxy: &Galaxy,
    query: &AlertQuery,
    list: &GlobalList,
    asked: bool,
) -> (Option<CentreVictims>, Option<ClassCensus>) {
    let class = ClassId::TIDAL_DISRUPTION_VICTIM;
    if !asked {
        return (None, None);
    }
    if !centre_of(list) {
        // The list lent holds no centre: none of its victims is in reach.
        let census = ClassCensus {
            class,
            reach: class_reach(class, query.sensor()),
            expected_hosts: Some(0.0),
            status: ClassStatus::Complete,
        };
        return (None, Some(census));
    }
    let reach = class_reach(class, query.sensor());
    let distance = LightYears::from(
        query
            .observer_position()
            .distance_to(&GalacticPosition::ORIGIN),
    );
    let victims = (reach > LightYears::ZERO && distance <= widened(reach)).then(|| {
        CentreVictims::of(galaxy)
            .expect("no galaxy the builder allows fails its centre (CentreModel::new's tests)")
    });
    let expected = victims
        .as_ref()
        .map_or(0.0, |victims| f64::from(victims.count()));
    let census = ClassCensus {
        class,
        reach,
        expected_hosts: Some(expected),
        status: status_of(expected, query),
    };
    (victims, Some(census))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::num::NonZeroU32;

    use super::*;
    use crate::alerts::{AlertBand, BandLimits, SensorHorizon};
    use crate::events::TimeWindow;
    use crate::galaxy::gas::ccm::Band;
    use crate::rng::Seed;
    use crate::time::UniverseTime;
    use crate::units::WattsPerSquareMetre;

    fn sensor(band: AlertBand, detect: f64) -> SensorHorizon {
        let w = WattsPerSquareMetre::new;
        let limits = BandLimits::new(w(detect), w(10.0 * detect)).unwrap();
        SensorHorizon::new([(band, limits)]).unwrap()
    }

    fn query(sensor: SensorHorizon, host_limit: u32) -> AlertQuery {
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap();
        let year = TimeWindow::new(
            UniverseTime::EPOCH,
            UniverseTime::from_julian_years(1).unwrap(),
        )
        .unwrap();
        AlertQuery::builder(sun, year, sensor)
            .host_limit(NonZeroU32::new(host_limit).unwrap())
            .build()
            .unwrap()
    }

    /// The reach is widened by the unbound pad speed over light's, 1.0007%.
    #[test]
    fn the_census_widens_the_reach_by_the_unbound_pad_speed() {
        let widened = widened(LightYears::new(10_000.0)).value();
        assert!((widened / 10_000.0 - 1.0 - 3e6 / 299_792_458.0).abs() < 1e-15);
    }

    /// Over the host limit a class is over the limit; under it and over 10⁵ it waits for the
    /// background; otherwise it is complete.
    #[test]
    fn the_census_parts_the_classes_by_their_expected_hosts() {
        let v = sensor(AlertBand::Photometric(Band::V), 1e-6);
        let small = query(v.clone(), 1_000);
        assert_eq!(status_of(999.0, &small), ClassStatus::Complete);
        assert_eq!(status_of(1_000.0, &small), ClassStatus::Complete);
        assert_eq!(status_of(1_000.5, &small), ClassStatus::OverLimit);
        assert_eq!(status_of(2e5, &small), ClassStatus::OverLimit);
        let large = query(v, 1 << 24);
        assert_eq!(status_of(1e5, &large), ClassStatus::Complete);
        assert_eq!(status_of(1.5e5, &large), ClassStatus::NeedsBackgroundScan);
        assert_eq!(status_of(2e7, &large), ClassStatus::OverLimit);
    }

    /// A class whose walk spans more cells than the limit waits for the background uncounted, and
    /// is not walked: the core collapses for a sensitive X-ray sensor, which their magnetars give
    /// the cube's diagonal.
    #[test]
    fn a_class_whose_walk_spans_too_many_cells_waits_for_the_background_uncounted() {
        let galaxy = Galaxy::new(Seed::new(9));
        let query = query(sensor(AlertBand::XRay, 1e-15), 1 << 24);
        let walked = Cell::new(false);
        let census = class_alerts(
            &galaxy,
            &query,
            ClassId::CORE_COLLAPSE,
            || CoreCollapseProcess::new(&galaxy),
            |_| walked.set(true),
        );
        assert_eq!(census.status(), ClassStatus::NeedsBackgroundScan);
        assert_eq!(census.expected_hosts(), None);
        assert_eq!(census.reach(), crate::alerts::ROOT_CUBE_DIAGONAL);
        assert!(!walked.get());
    }

    /// A sensor that cannot see a class at all leaves it complete with no host, unwalked: radio
    /// takes no supernova's light.
    #[test]
    fn a_class_out_of_every_band_is_complete_with_no_host() {
        let galaxy = Galaxy::new(Seed::new(9));
        let query = query(sensor(AlertBand::Photometric(Band::Radio), 1e-20), 1 << 24);
        let walked = Cell::new(false);
        let built = Cell::new(false);
        let census = class_alerts(
            &galaxy,
            &query,
            ClassId::TYPE_IA,
            || {
                built.set(true);
                TypeIaProcess::new(&galaxy)
            },
            |_| walked.set(true),
        );
        assert_eq!(census.status(), ClassStatus::Complete);
        assert_eq!(census.expected_hosts(), Some(0.0));
        assert_eq!(census.reach(), LightYears::ZERO);
        assert!(!walked.get() && !built.get());
    }

    /// The production lookup walks the recurrent walk's classes (P12.T5.c) once the census finds
    /// them complete, near the Sun at bright limits: the accreting white dwarfs and the luminous
    /// blue variables in V, the X-ray binaries in X-rays. Each counts its hosts and yields the
    /// recurrent walk's own detections.
    #[test]
    fn the_production_lookup_walks_the_recurrent_classes() {
        let galaxy = Galaxy::new(Seed::new(9));
        let v = query(sensor(AlertBand::Photometric(Band::V), 1e-4), 1 << 24);
        let x = query(sensor(AlertBand::XRay, 1e-4), 1 << 24);
        let mut noise = crate::galaxy::gas::noise::NoiseCache::with_capacity(4_096);
        for (class, query) in [
            (ClassId::ACCRETING_WHITE_DWARF, &v),
            (ClassId::ACCRETING_WHITE_DWARF_SLOW, &v),
            (ClassId::XRAY_BINARY, &x),
            (ClassId::LUMINOUS_BLUE_VARIABLE, &v),
        ] {
            let mut ctx =
                AlertContext::new(&mut noise, &crate::galaxy::gas::modifiers::NoModifiers);
            let mut out = Vec::new();
            let census =
                ProductionAlertClasses.class_alerts(&galaxy, &mut ctx, query, class, &mut out);
            assert_eq!(census.status(), ClassStatus::Complete, "{class}");
            assert!(census.expected_hosts().is_some_and(|n| n > 0.0), "{class}");
            assert!(census.reach() > LightYears::ZERO, "{class}");
            let mut walked = Vec::new();
            recurrent_detections(&galaxy, &mut ctx, class, query, &mut walked);
            assert_eq!(out, walked, "{class}");
        }
    }

    /// The production lookup walks every class but the centre's victims, which it leaves to the
    /// global list's walk: no detection, no host counted.
    #[test]
    fn the_production_lookup_leaves_the_centres_victims_to_the_global_list() {
        let galaxy = Galaxy::new(Seed::new(9));
        let query = query(sensor(AlertBand::Photometric(Band::V), 1e-12), 1 << 24);
        let mut noise = crate::galaxy::gas::noise::NoiseCache::with_capacity(16);
        let mut ctx = AlertContext::new(&mut noise, &crate::galaxy::gas::modifiers::NoModifiers);
        let class = ClassId::TIDAL_DISRUPTION_VICTIM;
        let mut out = Vec::new();
        let census =
            ProductionAlertClasses.class_alerts(&galaxy, &mut ctx, &query, class, &mut out);
        assert_eq!(
            census.status(),
            ClassStatus::Unwalked(Unwalked::NoGlobalList)
        );
        assert_eq!(census.expected_hosts(), None);
        assert_eq!(census.reach(), class_reach(class, query.sensor()));
        assert_eq!(out, []);
    }
}
