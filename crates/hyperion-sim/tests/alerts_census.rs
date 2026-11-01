//! "What went off?" (plan 12, P12.T5.d): `alerts_in`'s census per class, its walk of the galactic
//! centre through the global list, and the shared re-observation test.
//!
//! The census rules run on toy one-shot classes, not physics, whose candidates spread evenly over
//! a sphere about the observer, so that the expected hosts grow with the reach and the walks stay
//! fast: a class over the host limit yields nothing and says so, and halving its reach brings it
//! back; a window split in two gives the whole's onsets once each and its first half the whole's
//! events in progress; any order of the classes gives the same result. The centre runs on the
//! Milky Way fixture with its global list, its flares and its disruptions against their own
//! listings (slow to build, so shared). The production classes are checked at a bright limit
//! (slow).

#[expect(
    dead_code,
    reason = "the census uses only the alerts' comparison and cells"
)]
mod common;

use std::cell::RefCell;
use std::sync::OnceLock;

use common::observe::{KeepCells, Seen, observer_at};
use hyperion_sim::Seed;
use hyperion_sim::alerts::local::LocalEventsCache;
use hyperion_sim::alerts::{
    AlertBand, AlertClasses, AlertContext, AlertQuery, AlertResult, AlertSource, ArrivingLight,
    BandLimits, ClassCensus, ClassSet, ClassStatus, Detection, DetectionLight, DetectionPhase,
    OneShotClass, OneShotEvent, SensorHorizon, SourceStatus, TYPE_IA_BAND_PEAKS, TransientKind,
    Unwalked, alerts_in, alerts_in_with, centre_detections, class_alerts, flux,
    inverse_square_flux, magnetar_detections, one_shot_detections, one_shot_scan_cell,
    one_shot_scan_plan, transmission, unwalked_class,
};
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::supernova::{SupernovaEntry, SupernovaKind};
use hyperion_sim::galaxy::catalogue_classes::type_ia::{IaChannel, TypeIaProcess};
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueMember, CatalogueMotion, CatalogueSystem, ClassId, ClassProcess,
};
use hyperion_sim::galaxy::features::centre::events::{
    CentreFlareModel, CentreVictims, flare, flares,
};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::extinction::Sightline;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::global_list::GlobalList;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
use hyperion_sim::galaxy::snr::{ExplosionEnergy, ShellWindow, SiteGas, shell_window};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{CatalogueSystemId, CentreMemberId, EventId, SystemId};
use hyperion_sim::observe::{NoStarsCache, light_time};
use hyperion_sim::rng::{ObjectKey, Stream, tags};
use hyperion_sim::stellar::remnant::SupernovaType;
use hyperion_sim::time::{ClockWindow, Span, UniverseTime};
use hyperion_sim::units::{
    Dex, HydrogenPerCm3, KelvinPerCm3, LightYears, Metres, PerCubicLightYear, SolarMasses, Watts,
    WattsPerSquareMetre, Years,
};
use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::order::assert_order_independent;

/// The observer of the toy classes: near the Sun.
const HERE: [f64; 3] = [0.0, 26_000.0, 20.0];

/// The toys' candidates lie in the cells whose centres are within this distance of [`HERE`], ly:
/// beyond every sphere the census walks here.
const TOY_REGION_LY: f64 = 12_000.0;

/// The toys' candidates per cell inside their region.
const TOY_PER_CELL: f64 = 0.2;

/// The reach the census tests start from, ly: some 33,000 cells within it and the class grid's
/// 4,096 ly, under the cell limit, and about 6,500 candidates.
const REACH_LY: f64 = 6_000.0;

/// The host limit between the toy's expected hosts at [`REACH_LY`] and at half of it.
const TOY_HOST_LIMIT: u32 = 4_000;

/// The time `years` Julian years from the epoch.
fn at_years(years: f64) -> UniverseTime {
    let span = Span::from_seconds_f64(years.abs() * 31_557_600.0).unwrap();
    if years < 0.0 {
        UniverseTime::EPOCH.checked_sub(span).unwrap()
    } else {
        UniverseTime::EPOCH.checked_add(span).unwrap()
    }
}

/// The distance between two points given in light-years.
fn distance_ly(from: [f64; 3], to: [f64; 3]) -> f64 {
    let gap = [0, 1, 2].map(|axis| to[axis] - from[axis]);
    gap.iter().map(|g| g * g).sum::<f64>().sqrt()
}

/// A toy one-shot class, not physics (module documentation): on `class`'s grid, `TOY_PER_CELL`
/// candidates in each cell of its region, all kept, each a supernova of `supernova` whose light
/// reaches [`HERE`] in the four years about the epoch for half of them, and over the eighty years
/// before that for the rest.
#[derive(Clone)]
struct Toy {
    class: ClassId,
    supernova: SupernovaKind,
    window: ShellWindow,
}

impl Toy {
    fn new(class: ClassId, supernova: SupernovaKind) -> Self {
        let gas = SiteGas::uniform(HydrogenPerCm3::new(1.0), KelvinPerCm3::new(3_800.0)).unwrap();
        Self {
            class,
            supernova,
            window: shell_window(&gas, ExplosionEnergy::MEDIAN, Dex::new(0.0)),
        }
    }

    fn type_ia() -> Self {
        Self::new(ClassId::TYPE_IA, SupernovaKind::TypeIa(IaChannel::Merger))
    }

    fn core_collapse() -> Self {
        Self::new(
            ClassId::CORE_COLLAPSE,
            SupernovaKind::CoreCollapse(SupernovaType::IIP),
        )
    }
}

impl ClassProcess for Toy {
    type Candidate = (CatalogueSystemId, GalacticPosition, UniverseTime);
    type Entry = (SupernovaEntry, CatalogueMotion);

    fn class(&self) -> ClassId {
        self.class
    }

    fn bound(&self, _: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let edge = f64::from(cell.edge_ly());
        let middle = cell.origin_ly().map(|c| f64::from(c) + edge / 2.0);
        PerCubicLightYear::new(if distance_ly(middle, HERE) < TOY_REGION_LY {
            TOY_PER_CELL / cell.volume_ly3()
        } else {
            0.0
        })
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        _: PerCubicLightYear,
    ) -> Option<Self::Candidate> {
        let key = ObjectKey::from(SystemId::from(id));
        let mut stream = Stream::open(galaxy.seed(), tags::SELFTEST_STREAM, key);
        let site =
            cell.position_from_words([stream.next_u64(), stream.next_u64(), stream.next_u64()]);
        let arrival = if stream.uniform() < 0.5 {
            4.0 * stream.uniform() - 2.0
        } else {
            -2.0 - 80.0 * stream.uniform()
        };
        let explosion = at_years(arrival - distance_ly(site.to_light_years_f64(), HERE));
        Some((id, site, explosion))
    }

    fn test(&self, _: &Galaxy, candidate: Self::Candidate) -> Option<Self::Entry> {
        let (id, site, explosion) = candidate;
        let entry = SupernovaEntry::new(SystemId::from(id), explosion, self.supernova, self.window);
        Some((
            entry,
            CatalogueMotion::straight(site, GalacticVelocity::default()),
        ))
    }

    fn members(&self, _: &Self::Entry) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<Self::Entry>) -> CatalogueSystem {
        let (_, motion) = member.entry().value();
        let record = SystemRecord::from_parts(
            SystemId::from(member.id()),
            motion.position_at(UniverseTime::EPOCH),
            SystemOrigin::CatalogueSystem(self.class),
            Population::OldThinDisc,
            SolarMasses::new(1.0),
            Years::new(5e9),
        );
        CatalogueSystem::new(record, *motion)
    }
}

impl OneShotClass for Toy {
    fn kind(&self) -> TransientKind {
        match self.supernova {
            SupernovaKind::TypeIa(_) => TransientKind::TypeIa,
            SupernovaKind::CoreCollapse(_) => TransientKind::CoreCollapse,
        }
    }

    fn event(&self, entry: &Self::Entry) -> OneShotEvent {
        OneShotEvent::of_supernova(&entry.0, entry.1)
    }
}

/// The toys as `alerts_in_with`'s classes, each walked by the one-shot walk with the context's
/// cells; any other class is not built.
struct Toys(Vec<Toy>);

impl AlertClasses for Toys {
    fn class_alerts(
        &self,
        galaxy: &Galaxy,
        ctx: &mut AlertContext<'_>,
        query: &AlertQuery,
        class: ClassId,
        out: &mut Vec<Detection>,
    ) -> ClassCensus {
        let Some(toy) = self.0.iter().find(|toy| toy.class == class) else {
            return unwalked_class(class, query, Unwalked::NotBuilt("a toy's"));
        };
        class_alerts(
            galaxy,
            query,
            class,
            || toy.clone(),
            |toy| {
                one_shot_detections(galaxy, ctx, query, toy, out);
            },
        )
    }
}

/// The galaxy the toys live in.
fn toy_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(9)))
}

/// A visual sensor whose limit puts a Type Ia's peak at `reach_ly`, resolving at ten times it.
fn visual_reaching(reach_ly: f64) -> SensorHorizon {
    let peak = TYPE_IA_BAND_PEAKS[2];
    let detect = inverse_square_flux(peak, Metres::from(LightYears::new(reach_ly))).unwrap();
    let limits = BandLimits::new(detect, detect * 10.0).unwrap();
    SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap()
}

/// A year about the epoch.
fn a_year() -> TimeWindow {
    TimeWindow::new(at_years(-0.5), at_years(0.5)).unwrap()
}

/// The caches the toys' queries share: the noise and the toys' cells, both classes' in one.
struct Caches {
    noise: NoiseCache,
    cells: KeepCells,
}

impl Caches {
    fn new() -> RefCell<Self> {
        RefCell::new(Self {
            noise: NoiseCache::with_capacity(4_096),
            cells: KeepCells::default(),
        })
    }
}

/// `alerts_in_with` over `toys` for `query` with the shared `caches`, with no local rows.
fn toy_alerts(toys: Vec<Toy>, caches: &RefCell<Caches>, query: &AlertQuery) -> AlertResult {
    let caches = &mut *caches.borrow_mut();
    let mut ctx =
        AlertContext::new(&mut caches.noise, &NoModifiers).with_one_shot_cells(&mut caches.cells);
    alerts_in_with(
        toy_galaxy(),
        &mut ctx,
        query,
        &Toys(toys),
        &[],
        &mut NoStarsCache,
        &mut LocalEventsCache::default(),
    )
}

/// The detections as the comparison sees them, sorted.
fn seen(detections: &[Detection]) -> Vec<Seen> {
    let mut seen: Vec<_> = detections.iter().map(Seen::of).collect();
    seen.sort();
    seen
}

/// The one-shot walk of `toy` alone for `query`, as the comparison sees it.
fn walked(toy: &Toy, query: &AlertQuery) -> Vec<Seen> {
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut out = Vec::new();
    one_shot_detections(toy_galaxy(), &mut ctx, query, toy, &mut out);
    seen(&out)
}

/// A class over the host limit returns no detections and says so; with the default limit the
/// same query walks it and sees its events.
#[test]
fn alerts_of_a_class_over_the_limit_are_none_and_say_so() {
    let cells = Caches::new();
    let builder = || AlertQuery::builder(observer_at(HERE), a_year(), visual_reaching(REACH_LY));
    let limited = builder()
        .classes(ClassSet::of(ClassId::TYPE_IA))
        .host_limit(TOY_HOST_LIMIT.try_into().unwrap())
        .build()
        .unwrap();
    let result = toy_alerts(vec![Toy::type_ia()], &cells, &limited);
    let census = result.class_census(ClassId::TYPE_IA).unwrap();
    assert_eq!(census.status(), ClassStatus::OverLimit);
    let expected = census.expected_hosts().unwrap();
    assert!(expected > f64::from(TOY_HOST_LIMIT), "{expected}");
    assert!((census.reach().value() - REACH_LY).abs() < 1e-6);
    assert_eq!(result.detections(), []);

    let open = builder()
        .classes(ClassSet::of(ClassId::TYPE_IA))
        .build()
        .unwrap();
    let result = toy_alerts(vec![Toy::type_ia()], &cells, &open);
    let census = result.class_census(ClassId::TYPE_IA).unwrap();
    assert_eq!(census.status(), ClassStatus::Complete);
    assert_eq!(census.expected_hosts(), Some(expected));
    assert_ne!(result.detections(), []);
    assert_eq!(seen(result.detections()), walked(&Toy::type_ia(), &open));
}

/// Halving the reach, at four times the limit, brings the class back under the host limit: it is
/// walked, and its detections are the walk's own.
#[test]
fn alerts_of_a_class_over_the_limit_come_back_at_half_the_reach() {
    let cells = Caches::new();
    let query = AlertQuery::builder(observer_at(HERE), a_year(), visual_reaching(REACH_LY / 2.0))
        .classes(ClassSet::of(ClassId::TYPE_IA))
        .host_limit(TOY_HOST_LIMIT.try_into().unwrap())
        .build()
        .unwrap();
    let result = toy_alerts(vec![Toy::type_ia()], &cells, &query);
    let census = result.class_census(ClassId::TYPE_IA).unwrap();
    assert_eq!(census.status(), ClassStatus::Complete);
    let expected = census.expected_hosts().unwrap();
    assert!(
        (1_000.0..f64::from(TOY_HOST_LIMIT)).contains(&expected),
        "{expected}"
    );
    assert!((census.reach().value() - REACH_LY / 2.0).abs() < 1e-6);
    let detections = seen(result.detections());
    assert!(
        detections.len() > 10,
        "{} detections at half the reach",
        detections.len()
    );
    assert_eq!(detections, walked(&Toy::type_ia(), &query));
}

/// A window split in two gives each onset of the whole once, in the half its light arrives in,
/// and its first half the whole's events in progress, over two classes and the census.
#[test]
fn alerts_in_a_split_window_are_the_wholes_onsets_once_and_its_first_halfs_events_in_progress() {
    let cells = Caches::new();
    let classes = ClassSet::of(ClassId::TYPE_IA).with(ClassId::CORE_COLLAPSE);
    let [whole, first, second] = [(-1.5, 1.5), (-1.5, 0.0), (0.0, 1.5)]
        .map(|(a, b)| TimeWindow::new(at_years(a), at_years(b)).unwrap());
    let results = [whole, first, second].map(|window| {
        let query = AlertQuery::builder(observer_at(HERE), window, visual_reaching(2_500.0))
            .classes(classes)
            .build()
            .unwrap();
        toy_alerts(vec![Toy::type_ia(), Toy::core_collapse()], &cells, &query)
    });
    for result in &results {
        for census in result.census() {
            assert_eq!(census.status(), ClassStatus::Complete, "{census:?}");
        }
    }
    let phase = |result: &AlertResult, phase| {
        let mut seen: Vec<Seen> = result
            .detections()
            .iter()
            .filter(|d| d.phase() == phase)
            .map(Seen::of)
            .collect();
        seen.sort();
        seen
    };
    let [whole, first, second] = &results;
    let onsets = phase(whole, DetectionPhase::Onset);
    let mut halves = phase(first, DetectionPhase::Onset);
    halves.extend(phase(second, DetectionPhase::Onset));
    halves.sort();
    assert!(onsets.len() > 5, "{} onsets", onsets.len());
    assert_eq!(halves, onsets, "each onset once, in its half");
    let in_progress = phase(whole, DetectionPhase::InProgress);
    assert_eq!(phase(first, DetectionPhase::InProgress), in_progress);
    for detection in second.detections() {
        if detection.phase() == DetectionPhase::InProgress {
            assert!(detection.arrival() < second_start());
        }
    }
}

/// The start of the split's second half, the epoch.
fn second_start() -> UniverseTime {
    at_years(0.0)
}

/// Any order of the classes gives the same result: two classes asked together are each asked
/// alone, merged by arrival and event and their census in class order, whichever order the lookup
/// holds them in; and queries asked in any order with the same caches give the same results.
#[test]
fn alerts_are_the_same_in_any_order_of_classes() {
    let cells = Caches::new();
    let query = |classes, reach| {
        AlertQuery::builder(observer_at(HERE), a_year(), visual_reaching(reach))
            .classes(classes)
            .build()
            .unwrap()
    };
    let both = ClassSet::of(ClassId::TYPE_IA).with(ClassId::CORE_COLLAPSE);
    let forwards = || vec![Toy::type_ia(), Toy::core_collapse()];
    let backwards = || vec![Toy::core_collapse(), Toy::type_ia()];
    let together = toy_alerts(forwards(), &cells, &query(both, 2_500.0));
    assert_eq!(
        toy_alerts(backwards(), &cells, &query(both, 2_500.0)),
        together
    );
    let mut merged = Vec::new();
    let mut census = Vec::new();
    for class in [ClassId::TYPE_IA, ClassId::CORE_COLLAPSE] {
        let alone = toy_alerts(forwards(), &cells, &query(ClassSet::of(class), 2_500.0));
        merged.extend(alone.detections().iter().cloned());
        census.extend(alone.census().iter().copied());
    }
    merged.sort_by_key(|d| (d.arrival(), d.event()));
    census.sort_by_key(ClassCensus::class);
    assert_ne!(merged.len(), 0);
    assert_eq!(together.detections(), merged);
    assert_eq!(together.census(), census);

    let keys = [
        (both, 2_500.0),
        (ClassSet::of(ClassId::TYPE_IA), 2_000.0),
        (both, 1_200.0),
        (ClassSet::of(ClassId::CORE_COLLAPSE), 2_500.0),
        (ClassSet::ALL, 2_500.0),
    ];
    assert_order_independent(&keys, |&(classes, reach)| {
        toy_alerts(forwards(), &cells, &query(classes, reach))
    });
}

/// A class the census sends to the background is walked cell by cell: the one-shot walk's plan,
/// each cell on its own with a fresh context, in reverse, merges to the walk's own detections; a
/// cell of another class adds nothing.
#[test]
fn alerts_of_a_one_shot_class_walked_cell_by_cell_are_its_walks() {
    let toy = Toy::type_ia();
    let query = AlertQuery::builder(observer_at(HERE), a_year(), visual_reaching(2_500.0))
        .classes(ClassSet::of(ClassId::TYPE_IA))
        .build()
        .unwrap();
    let plan = one_shot_scan_plan(&toy, &query);
    assert!(
        plan.windows(2).all(|pair| pair[0] < pair[1]),
        "in key order"
    );
    let mut by_cell = Vec::new();
    for cell in plan.iter().rev() {
        let mut noise = NoiseCache::with_capacity(64);
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
        one_shot_scan_cell(toy_galaxy(), &mut ctx, &query, &toy, *cell, &mut by_cell);
    }
    let walk = walked(&toy, &query);
    assert_ne!(walk.len(), 0);
    assert_eq!(seen(&by_cell), walk);
    let other = CatalogueCellKey::containing(ClassId::CORE_COLLAPSE, &observer_at(HERE)).unwrap();
    let mut out = Vec::new();
    let mut noise = NoiseCache::with_capacity(64);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    one_shot_scan_cell(toy_galaxy(), &mut ctx, &query, &toy, other, &mut out);
    assert_eq!(out, []);
    let others = AlertQuery::builder(observer_at(HERE), a_year(), visual_reaching(2_500.0))
        .classes(ClassSet::of(ClassId::CORE_COLLAPSE))
        .build()
        .unwrap();
    assert_eq!(one_shot_scan_plan(&toy, &others), []);
}

/// The Milky Way fixture with its kinematic tables, and its global list.
fn milky_way() -> &'static (Galaxy, GlobalList) {
    static FIXTURE: OnceLock<(Galaxy, GlobalList)> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())
            .unwrap()
            .with_full_potential();
        let list = GlobalList::build(&galaxy);
        (galaxy, list)
    })
}

/// An X-ray sensor of `detect` W m⁻², resolving at a hundred times it.
fn xray(detect: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(100.0 * detect)).unwrap();
    SensorHorizon::new([(AlertBand::XRay, limits)]).unwrap()
}

/// `alerts_in` in the fixture for `query`, with or without its global list, with no local rows.
fn centre_alerts(query: &AlertQuery, with_list: bool) -> AlertResult {
    let (galaxy, list) = milky_way();
    let mut noise = NoiseCache::with_capacity(4_096);
    let ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut ctx = if with_list {
        ctx.with_global_list(list)
    } else {
        ctx
    };
    alerts_in(
        galaxy,
        &mut ctx,
        query,
        &[],
        &mut NoStarsCache,
        &mut LocalEventsCache::default(),
    )
}

/// The status of `source` in `result`.
fn source(result: &AlertResult, source: AlertSource) -> SourceStatus {
    result
        .sources()
        .iter()
        .find(|s| s.source() == source)
        .unwrap()
        .status()
}

/// The centre is walked through the global list: without one, neither its flares nor its
/// disruptions are, and the result says so; with it, its detections are `centre_detections`' own
/// over the galaxy's victims, the victims' class census counts them, and the sources not built
/// say which task builds them.
#[test]
fn alerts_of_the_centre_come_through_the_global_list() {
    let (galaxy, list) = milky_way();
    let week = TimeWindow::new(at_years(0.0), at_years(7.0 / 365.25)).unwrap();
    let query = AlertQuery::builder(observer_at([0.0, 8_000.0, 50.0]), week, xray(1e-15))
        .classes(ClassSet::of(ClassId::TIDAL_DISRUPTION_VICTIM))
        .build()
        .unwrap();
    let without = centre_alerts(&query, false);
    assert_eq!(without.detections(), []);
    assert_eq!(
        without
            .class_census(ClassId::TIDAL_DISRUPTION_VICTIM)
            .unwrap()
            .status(),
        ClassStatus::Unwalked(Unwalked::NoGlobalList)
    );
    for kind in [AlertSource::Centre, AlertSource::DwarfCores] {
        assert_eq!(
            source(&without, kind),
            SourceStatus::Unwalked(Unwalked::NoGlobalList)
        );
    }
    assert_eq!(
        source(&without, AlertSource::FeatureLists),
        SourceStatus::Unwalked(Unwalked::NotBuilt("P09.T36"))
    );

    let with = centre_alerts(&query, true);
    assert_eq!(source(&with, AlertSource::Centre), SourceStatus::Complete);
    let cores = list.debris().cores().len();
    assert_eq!(
        source(&with, AlertSource::DwarfCores),
        if cores > 0 {
            SourceStatus::Unwalked(Unwalked::NotBuilt("P10.T7.b"))
        } else {
            SourceStatus::Complete
        }
    );
    let victims = CentreVictims::of(galaxy).unwrap();
    let census = with.class_census(ClassId::TIDAL_DISRUPTION_VICTIM).unwrap();
    assert_eq!(census.status(), ClassStatus::Complete);
    assert_eq!(census.expected_hosts(), Some(f64::from(victims.count())));
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut direct = Vec::new();
    centre_detections(galaxy, &mut ctx, &query, Some(&victims), &mut direct);
    assert_eq!(with.detections(), direct);
    let kinds = |kind| {
        with.detections()
            .iter()
            .filter(|d| d.kind() == kind)
            .count()
    };
    assert!(kinds(TransientKind::CentralFlare) > 0, "a week of flares");
    assert!(
        kinds(TransientKind::TidalDisruption) > 0,
        "tails still above the limit"
    );
    let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
    for detection in with.detections() {
        match detection.kind() {
            TransientKind::CentralFlare => {
                assert_eq!((detection.host(), detection.class()), (black_hole, None));
            }
            TransientKind::TidalDisruption => {
                assert_eq!(detection.class(), Some(ClassId::TIDAL_DISRUPTION_VICTIM));
                assert!(victims.of_disruption(detection.event()).is_some());
            }
            other => panic!("the centre makes no {other:?}"),
        }
    }

    // A query that does not ask about the victims still sees the flares, which have no class.
    let flares_only = AlertQuery::builder(observer_at([0.0, 8_000.0, 50.0]), week, xray(1e-15))
        .classes(ClassSet::EMPTY)
        .build()
        .unwrap();
    let result = centre_alerts(&flares_only, true);
    assert_eq!(result.census(), []);
    assert!(
        result
            .detections()
            .iter()
            .all(|d| d.kind() == TransientKind::CentralFlare)
    );
    assert_eq!(
        result.detections().len(),
        kinds(TransientKind::CentralFlare)
    );
}

/// A detection as the centre's oracle compares it: arrival, event, phase and flux's bits.
type Row = (UniverseTime, EventId, DetectionPhase, u64);

/// The centre's events evaluated directly from their listings, for one observer and an X-ray
/// sensor: every flare with an onset from a day before the emission interval, and every victim
/// with a tail, each from its arrival, its phase and its light.
struct CentreOracle<'a> {
    model: CentreFlareModel,
    victims: &'a CentreVictims,
    distance: Metres,
    light: Span,
    line: Sightline,
    detect: WattsPerSquareMetre,
}

impl CentreOracle<'_> {
    /// The rows `window` holds, sorted.
    fn rows(&self, window: TimeWindow) -> Vec<Row> {
        let t1 = window.start().checked_sub(self.light).unwrap();
        let t2 = window.end().checked_sub(self.light).unwrap();
        let wide =
            TimeWindow::new(t1.checked_sub(Span::from_seconds(86_400)).unwrap(), t2).unwrap();
        let mut listed = Vec::new();
        flares(&self.model, wide, &mut listed);
        let flares = listed.iter().map(|flare| {
            let lights = (flare.peak_luminosity_watts(), flare.luminosity_watts_at(t1));
            (flare.onset(), flare.id(), lights)
        });
        let tails = self.victims.victims().iter().filter_map(|victim| {
            let tail = victim.tail()?;
            let lights = (tail.peak_luminosity_watts(), tail.luminosity_watts_at(t1));
            Some((victim.time(), victim.disruption().id(), lights))
        });
        let mut rows: Vec<Row> = flares
            .chain(tails)
            .filter_map(|(onset, event, (peak, at_start))| {
                let arrival = onset.checked_add(self.light).unwrap();
                let (phase, watts) = if window.contains(arrival) {
                    (DetectionPhase::Onset, peak)
                } else if arrival < window.start() {
                    (DetectionPhase::InProgress, at_start)
                } else {
                    return None;
                };
                let seen = flux(
                    Watts::new(watts),
                    self.distance,
                    &self.line,
                    AlertBand::XRay,
                )
                .filter(|f| *f >= self.detect)?;
                Some((arrival, event, phase, bits(seen.value())))
            })
            .collect();
        rows.sort_unstable();
        rows
    }
}

/// The rows of `rows` in `phase`.
fn in_phase(rows: &[Row], phase: DetectionPhase) -> Vec<Row> {
    rows.iter().filter(|row| row.2 == phase).copied().collect()
}

/// The centre's walk against its own listings evaluated directly ([`CentreOracle`]), over a
/// window and its halves: each onset once, in its half, and the first half the whole's events in
/// progress.
#[test]
fn alerts_of_the_centre_are_its_listings_evaluated_directly() {
    let (galaxy, _) = milky_way();
    let here = observer_at([3_000.0, 4_000.0, 900.0]);
    let sensor = xray(1e-15);
    let victims = CentreVictims::of(galaxy).unwrap();
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let origin = GalacticPosition::ORIGIN;
    let distance = here.distance_to(&origin);
    let oracle = CentreOracle {
        model: CentreFlareModel::of(galaxy),
        victims: &victims,
        distance,
        light: light_time(distance),
        line: ctx.sightline(galaxy, &origin, &here),
        detect: sensor.limits(AlertBand::XRay).unwrap().detect(),
    };
    let windows = [(0.0, 6.0), (0.0, 3.0), (3.0, 6.0)]
        .map(|(a, b)| TimeWindow::new(at_years(a / 365.25), at_years(b / 365.25)).unwrap());
    let [whole, first, second] = windows.map(|window| {
        let query = AlertQuery::builder(here, window, sensor.clone())
            .build()
            .unwrap();
        let mut walked = Vec::new();
        centre_detections(galaxy, &mut ctx, &query, Some(&victims), &mut walked);
        let rows: Vec<Row> = walked
            .iter()
            .map(|d| (d.arrival(), d.event(), d.phase(), bits(d.flux().value())))
            .collect();
        assert!(
            rows.windows(2)
                .all(|pair| (pair[0].0, pair[0].1) < (pair[1].0, pair[1].1)),
            "by arrival, then by event"
        );
        assert_eq!(rows, oracle.rows(window), "{window:?}");
        rows
    });
    let mut halves = in_phase(&first, DetectionPhase::Onset);
    halves.extend(in_phase(&second, DetectionPhase::Onset));
    halves.sort_unstable();
    assert_ne!(halves, []);
    assert_eq!(halves, in_phase(&whole, DetectionPhase::Onset));
    let in_progress = in_phase(&whole, DetectionPhase::InProgress);
    assert_ne!(in_progress, []);
    assert_eq!(in_phase(&first, DetectionPhase::InProgress), in_progress);
}

/// A sensor of `band` alone at `detect` W m⁻², resolving at a hundred times it.
fn sensor_in(band: AlertBand, detect: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(100.0 * detect)).unwrap();
    SensorHorizon::new([(band, limits)]).unwrap()
}

/// The centre's detections of `query`, with the galaxy's victims.
fn centre_walk(galaxy: &Galaxy, victims: &CentreVictims, query: &AlertQuery) -> Vec<Detection> {
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let mut out = Vec::new();
    centre_detections(galaxy, &mut ctx, query, Some(victims), &mut out);
    out
}

/// `detection`'s light, which the centre's events always give.
fn lit(detection: &Detection) -> ArrivingLight {
    match detection.light() {
        DetectionLight::Lit(light) => *light,
        DetectionLight::Unlit => panic!("the centre's events give light: {detection:?}"),
    }
}

/// `detection`'s light (P12.T8.b.1) over `window`, the walk's: in the detection's band, zero
/// before its onset arrives, and the walk's flux at the detection's own time, an onset's peak, its
/// second knot, and the window's start for an event in progress. A flare's is dark again from its
/// end, its third knot; a tail's is dark until about its peak and falls to the clock window's end.
fn the_light_is_the_walks(detection: &Detection, window: TimeWindow) {
    let light = lit(detection);
    let nanosecond = Span::new(0, 1).unwrap();
    let at = |t: UniverseTime| light.flux_at(t).value();
    assert_eq!(light.band(), detection.band());
    let knots = light.knots();
    assert_eq!(knots.len(), 3, "{detection:?}");
    assert!(knots.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(knots[0], detection.arrival());
    assert_same_bits(at(knots[0].checked_sub(nanosecond).unwrap()), 0.0);
    match detection.kind() {
        TransientKind::CentralFlare => {
            assert_eq!(light.band(), AlertBand::XRay);
            assert_same_bits(at(knots[2]), 0.0);
        }
        TransientKind::TidalDisruption => {
            // Dark until its fallback time, which an `f64` of seconds resolves to about a
            // nanosecond at three months: dark half-way there and a microsecond before.
            let microsecond = Span::new(0, 1_000).unwrap();
            let half = knots[1].checked_since(knots[0]).unwrap().as_seconds_f64() / 2.0;
            let midway = knots[0]
                .checked_add(Span::from_seconds_f64(half).unwrap())
                .unwrap();
            assert_same_bits(at(midway), 0.0);
            assert_same_bits(at(knots[1].checked_sub(microsecond).unwrap()), 0.0);
            assert_eq!(knots[2], ClockWindow::END);
            assert!(
                at(knots[2]) > 0.0 && at(knots[2]) < at(knots[1]),
                "it falls"
            );
        }
        other => panic!("the centre makes no {other:?}"),
    }
    match detection.phase() {
        DetectionPhase::Onset => {
            let peak = at(knots[1]);
            let off = (peak / detection.flux().value() - 1.0).abs();
            assert!(off < 1e-9, "{detection:?}: its peak {peak}, {off} off");
        }
        DetectionPhase::InProgress => {
            assert_same_bits(at(window.start()), detection.flux().value());
        }
    }
}

/// Each of the centre's detections carries its light, the walk's flux at the detection's own time:
/// a fortnight of flares in X-rays about the onset of the latest disruption with a tail, seen from
/// north of the black hole, that disruption's onset in X-rays and in V, and each again in progress
/// from a later window with the same light.
#[test]
fn alerts_of_the_centre_carry_lights_that_are_the_walks_flux_at_their_own_times() {
    let (galaxy, _) = milky_way();
    let victims = CentreVictims::of(galaxy).unwrap();
    let latest = victims
        .victims()
        .iter()
        .filter(|v| v.tail().is_some() && v.time() < UniverseTime::EPOCH)
        .max_by_key(|v| v.time())
        .expect("a disruption with a tail before the epoch");
    // Placed so that the disruption's onset arrives 500 years after the epoch.
    let ago = UniverseTime::EPOCH
        .checked_since(latest.time())
        .unwrap()
        .as_julian_years_f64();
    let here = observer_at([0.0, 0.0, ago + 500.0]);
    let arrival = latest
        .time()
        .checked_add(light_time(here.distance_to(&GalacticPosition::ORIGIN)))
        .unwrap();
    let day = Span::from_seconds(86_400);
    let week = Span::from_seconds(7 * 86_400);
    let around = TimeWindow::new(
        arrival.checked_sub(week).unwrap(),
        arrival.checked_add(week).unwrap(),
    )
    .unwrap();
    let decade = arrival
        .checked_add(Span::from_julian_years(10).unwrap())
        .unwrap();
    let later = TimeWindow::new(decade, decade.checked_add(day).unwrap()).unwrap();
    let query = |window, sensor| AlertQuery::builder(here, window, sensor).build().unwrap();
    let photometric = AlertBand::Photometric(Band::V);
    for band in [AlertBand::XRay, photometric] {
        let sensor = sensor_in(band, 1e-16);
        let seen = centre_walk(galaxy, &victims, &query(around, sensor.clone()));
        for detection in &seen {
            the_light_is_the_walks(detection, around);
        }
        let onset = seen
            .iter()
            .find(|d| d.event() == latest.disruption().id())
            .expect("the latest disruption's onset");
        assert_eq!(onset.phase(), DetectionPhase::Onset);
        assert_eq!(onset.band(), band);
        let again = centre_walk(galaxy, &victims, &query(later, sensor.clone()));
        let in_progress = again
            .iter()
            .find(|d| d.event() == onset.event())
            .expect("the tail is still above the limit a decade on");
        assert_eq!(in_progress.phase(), DetectionPhase::InProgress);
        assert_eq!(in_progress.light(), onset.light(), "the same light");
        the_light_is_the_walks(in_progress, later);
        if band != AlertBand::XRay {
            assert!(
                seen.iter()
                    .all(|d| d.kind() == TransientKind::TidalDisruption)
            );
            continue;
        }
        // The longest of the flares, from a window that opens half-way through its light.
        let longest = seen
            .iter()
            .filter(|d| {
                d.kind() == TransientKind::CentralFlare && d.phase() == DetectionPhase::Onset
            })
            .max_by_key(|d| lit(d).knots()[2].checked_since(d.arrival()).unwrap())
            .expect("a fortnight's flares about the onset");
        let knots = lit(longest).knots();
        let opening = knots[1];
        let midway = TimeWindow::new(opening, opening.checked_add(day).unwrap()).unwrap();
        let flare = centre_walk(galaxy, &victims, &query(midway, sensor))
            .into_iter()
            .find(|d| d.event() == longest.event())
            .expect("the flare is in progress at its peak");
        assert_eq!(flare.phase(), DetectionPhase::InProgress);
        assert_eq!(flare.light(), longest.light(), "the same light");
        the_light_is_the_walks(&flare, midway);
    }
}

/// The shared re-observation test (plan 12, P12.T5), on the black hole's flares while plan 11's
/// novae are not built: a flare seen from 1,000 ly at its arrival `t_a` is seen from 1,500 ly on
/// the same line through its place of emission at `t_a` + 500 years, to a millisecond, with the
/// same event, emitted time and peak luminosity; and from 10⁴ ly further still, at the same time,
/// the host's flares of 10⁴ years before.
#[test]
fn alerts_re_observe_a_flare_from_further_on_its_line_and_its_hosts_earlier_ones_further_still() {
    let (galaxy, list) = milky_way();
    let sensor = xray(1e-16);
    let [near, far, further] = [1_000.0, 1_500.0, 11_500.0].map(|z| observer_at([0.0, 0.0, z]));
    let in_window = |here: GalacticPosition, window: TimeWindow| {
        let query = AlertQuery::builder(here, window, sensor.clone())
            .classes(ClassSet::EMPTY)
            .build()
            .unwrap();
        let mut noise = NoiseCache::with_capacity(4_096);
        let mut ctx = AlertContext::new(&mut noise, &NoModifiers).with_global_list(list);
        let result = alerts_in(
            galaxy,
            &mut ctx,
            &query,
            &[],
            &mut NoStarsCache,
            &mut LocalEventsCache::default(),
        );
        let line = ctx.sightline(galaxy, &GalacticPosition::ORIGIN, &here);
        (result, line)
    };
    let luminosity = |detection: &Detection, here: &GalacticPosition, line| {
        let d = here.distance_to(&GalacticPosition::ORIGIN).value();
        detection.flux().value() * 4.0 * std::f64::consts::PI * d * d
            / transmission(line, detection.band())
    };
    let week = TimeWindow::new(at_years(0.0), at_years(7.0 / 365.25)).unwrap();
    let (seen, near_line) = in_window(near, week);
    let first = seen
        .detections()
        .iter()
        .find(|d| d.phase() == DetectionPhase::Onset)
        .expect("a week of flares");
    let later = Span::from_julian_years(500).unwrap();
    let t_far = first.arrival().checked_add(later).unwrap();
    let day = Span::from_seconds(86_400);
    let about = TimeWindow::new(
        t_far.checked_sub(day).unwrap(),
        t_far.checked_add(day).unwrap(),
    )
    .unwrap();
    let (seen_far, far_line) = in_window(far, about);
    let again = seen_far
        .detections()
        .iter()
        .find(|d| d.event() == first.event())
        .expect("the same flare from 1,500 ly");
    let off = again
        .arrival()
        .checked_since(t_far)
        .or_else(|| t_far.checked_since(again.arrival()))
        .unwrap();
    assert!(off <= Span::from_seconds_f64(1e-3).unwrap(), "{off:?}");
    assert_eq!(again.phase(), DetectionPhase::Onset);
    assert_eq!(again.emitted(), first.emitted());
    let (l_near, l_far) = (
        luminosity(first, &near, &near_line),
        luminosity(again, &far, &far_line),
    );
    assert!((l_far / l_near - 1.0).abs() < 1e-12, "{l_near} and {l_far}");
    let model = CentreFlareModel::of(galaxy);
    let own = flare(&model, first.event()).unwrap();
    assert!((l_near / own.peak_luminosity_watts() - 1.0).abs() < 1e-12);

    let (seen_further, _) = in_window(further, about);
    let earlier: Vec<_> = seen_further
        .detections()
        .iter()
        .filter(|d| d.phase() == DetectionPhase::Onset)
        .collect();
    assert!(!earlier.is_empty(), "the host's earlier flares");
    let ten_thousand = Span::from_julian_years(10_000).unwrap();
    for detection in earlier {
        assert_eq!(
            detection.host(),
            SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE)
        );
        let before = first.emitted().checked_since(detection.emitted()).unwrap();
        let gap = before
            .checked_sub(ten_thousand)
            .or_else(|| ten_thousand.checked_sub(before))
            .unwrap();
        assert!(gap < Span::from_seconds(3 * 86_400), "{before:?}");
        assert_eq!(
            flare(&model, detection.event()).map(|f| f.onset()),
            Some(detection.emitted())
        );
    }
}

/// The production classes at a bright visual limit near the Sun: the supernova classes complete,
/// their detections the walks' own; the others not built, each naming its task; the centre not
/// walked without the global list (slow).
#[test]
#[ignore = "slow: walks the core collapses' cells about the Sun"]
fn alerts_of_the_production_classes_at_a_bright_limit_are_their_walks() {
    let galaxy = toy_galaxy();
    let w = WattsPerSquareMetre::new;
    let sensor = SensorHorizon::new([(
        AlertBand::Photometric(Band::V),
        BandLimits::new(w(1e-4), w(1e-3)).unwrap(),
    )])
    .unwrap();
    let query = AlertQuery::builder(observer_at(HERE), a_year(), sensor)
        .build()
        .unwrap();
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let result = alerts_in(
        galaxy,
        &mut ctx,
        &query,
        &[],
        &mut NoStarsCache,
        &mut LocalEventsCache::default(),
    );
    let status = |class| result.class_census(class).unwrap().status();
    assert_eq!(status(ClassId::CORE_COLLAPSE), ClassStatus::Complete);
    assert_eq!(status(ClassId::TYPE_IA), ClassStatus::Complete);
    for (class, task) in [
        (ClassId::LUMINOUS_BLUE_VARIABLE, "P12.T5.c"),
        (ClassId::XRAY_BINARY, "P11.T8.b"),
        (ClassId::ACCRETING_WHITE_DWARF, "P11.T9"),
        (ClassId::ACCRETING_WHITE_DWARF_SLOW, "P11.T9"),
        (ClassId::STELLAR_MERGER, "P11.T9"),
        (ClassId::NEUTRON_STAR_MERGER, "P11.T9"),
    ] {
        assert_eq!(
            status(class),
            ClassStatus::Unwalked(Unwalked::NotBuilt(task)),
            "{class}"
        );
    }
    assert_eq!(
        status(ClassId::TIDAL_DISRUPTION_VICTIM),
        ClassStatus::Unwalked(Unwalked::NoGlobalList)
    );
    assert_eq!(result.census().len(), ClassSet::ALL.iter().count());
    let mut walks = Vec::new();
    let mut ctx = AlertContext::new(&mut noise, &NoModifiers);
    let core_collapses = CoreCollapseProcess::new(galaxy);
    one_shot_detections(galaxy, &mut ctx, &query, &core_collapses, &mut walks);
    magnetar_detections(galaxy, &mut ctx, &query, &core_collapses, &mut walks);
    one_shot_detections(
        galaxy,
        &mut ctx,
        &query,
        &TypeIaProcess::new(galaxy),
        &mut walks,
    );
    assert_eq!(seen(result.detections()), seen(&walks));
    eprintln!(
        "{} detections; census {:?}",
        result.detections().len(),
        result.census()
    );
}
