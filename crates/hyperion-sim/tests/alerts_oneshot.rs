//! The one-shot classes' walk against the brute force (plan 12, P12.T5.b): for three observers and
//! windows of a year and a century, the walk's detections are the brute force's, as sets, field by
//! field and bit by bit (`common/observe.rs`).
//!
//! Two classes are walked:
//!
//! - **A dense toy Type Ia class**, not physics, which stands in for P09.T34.a's process until it
//!   lands, on the interface that task documents: entries on the Type Ia grid of 512 ly cells, each
//!   a [`SupernovaEntry`] of a channel with its member 0's motion. Its entries crowd about the
//!   observers, sites up to 3,000 ly from their cells, and their light reaches each observer
//!   within a few years of a time of its own, so that every window holds onsets and events in
//!   progress (fast).
//! - **The production core collapses** of a galaxy, about two of whose recent deaths three
//!   observers stand: above the one in the past to see it go off, beside it to see it in progress,
//!   and 50 ly from the one still to come (slow).
//!
//! The walk also gives each query the same detections whatever it walked before with the same
//! caches.

#[expect(dead_code, reason = "the one-shot alerts use only the alerts' oracle")]
mod common;

use std::cell::RefCell;
use std::collections::BTreeSet;

use common::observe::{KeepCells, Seen, brute_force_detections, observer_at};
use hyperion_sim::Seed;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, ClassSet, DetectionPhase, NoOneShotCells,
    OneShotClass, OneShotEvent, SensorHorizon, TransientKind, one_shot_detections,
};
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::supernova::{SupernovaEntry, SupernovaKind};
use hyperion_sim::galaxy::catalogue_classes::type_ia::IaChannel;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueMember, CatalogueMotion, CatalogueSystem, ClassId, ClassProcess,
};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
use hyperion_sim::galaxy::snr::{ExplosionEnergy, ShellWindow, SiteGas, shell_window};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{CatalogueSystemId, SystemId};
use hyperion_sim::observe::light_time;
use hyperion_sim::rng::{ObjectKey, Stream, tags};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::{
    Dex, HydrogenPerCm3, KelvinPerCm3, PerCubicLightYear, SolarMasses, WattsPerSquareMetre, Years,
};
use hyperion_testkit::order::assert_order_independent;

/// The toy's sites lie up to this far from their cells, ly: within the class grid's 4,096 ly.
const TOY_SCATTER_LY: f64 = 3_000.0;

/// The toy's entries crowd within this distance of an observer, ly.
const TOY_CROWD_LY: f64 = 1_500.0;

/// The toy's entries per cell, in the cells that hold them.
const TOY_PER_CELL: f64 = 6.0;

/// One observer of the toy: where it stands and the time its entries' light reaches it about.
#[derive(Debug, Clone, Copy)]
struct Target {
    position: [f64; 3],
    years: f64,
}

/// The toy's three observers: near the Sun at the epoch, above the plane 600 years before it, and
/// far out in the disc 700 years after it.
const TARGETS: [Target; 3] = [
    Target {
        position: [0.0, 26_000.0, 20.0],
        years: 0.0,
    },
    Target {
        position: [3_000.0, 22_000.0, 900.0],
        years: -600.0,
    },
    Target {
        position: [-15_000.0, 30_000.0, -100.0],
        years: 700.0,
    },
];

/// A toy Type Ia class, not physics (module documentation): candidates only in the cells within
/// [`TOY_CROWD_LY`] + [`TOY_SCATTER_LY`] of a target, all kept, each a supernova whose light
/// reaches its nearest target a few years either side of the target's time, a sixth of them Type
/// Iax.
struct ToyTypeIa {
    window: ShellWindow,
}

impl ToyTypeIa {
    fn new() -> Self {
        let gas = SiteGas::uniform(HydrogenPerCm3::new(1.0), KelvinPerCm3::new(3_800.0)).unwrap();
        Self {
            window: shell_window(&gas, ExplosionEnergy::MEDIAN, Dex::new(0.0)),
        }
    }
}

/// The light-years of `position`.
fn ly(position: &GalacticPosition) -> [f64; 3] {
    position.to_light_years_f64()
}

/// The distance between two points given in light-years.
fn distance_ly(from: [f64; 3], to: [f64; 3]) -> f64 {
    let gap = [0, 1, 2].map(|axis| to[axis] - from[axis]);
    gap.iter().map(|g| g * g).sum::<f64>().sqrt()
}

/// The time `years` Julian years from the epoch.
fn at_years(years: f64) -> UniverseTime {
    let span = Span::from_seconds_f64(years.abs() * 31_557_600.0).unwrap();
    if years < 0.0 {
        UniverseTime::EPOCH.checked_sub(span).unwrap()
    } else {
        UniverseTime::EPOCH.checked_add(span).unwrap()
    }
}

impl ClassProcess for ToyTypeIa {
    type Candidate = (
        CatalogueSystemId,
        GalacticPosition,
        UniverseTime,
        IaChannel,
        CatalogueMotion,
    );
    type Entry = (SupernovaEntry, CatalogueMotion);

    fn class(&self) -> ClassId {
        ClassId::TYPE_IA
    }

    fn bound(&self, _: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let edge = f64::from(cell.edge_ly());
        let middle = cell.origin_ly().map(|c| f64::from(c) + edge / 2.0);
        let near = TARGETS.iter().any(|target| {
            distance_ly(middle, target.position) < TOY_CROWD_LY + TOY_SCATTER_LY + edge
        });
        PerCubicLightYear::new(if near {
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
        let home = ly(&cell.position_from_words([
            stream.next_u64(),
            stream.next_u64(),
            stream.next_u64(),
        ]));
        // A point in the ball of the scatter, by rejection from its cube.
        let offset = loop {
            let offset = [0, 1, 2].map(|_| (2.0 * stream.uniform() - 1.0) * TOY_SCATTER_LY);
            if distance_ly(offset, [0.0; 3]) <= TOY_SCATTER_LY {
                break offset;
            }
        };
        let site = [0, 1, 2].map(|axis| home[axis] + offset[axis]);
        let target = TARGETS
            .iter()
            .min_by(|a, b| distance_ly(site, a.position).total_cmp(&distance_ly(site, b.position)))
            .unwrap();
        // Two fifths arrive within three years of the target's time, a fifth in the 40 days
        // before a year about it opens, a fifth in the year and a half before a century about it
        // opens, and a fifth over the sixty years before it.
        let u = stream.uniform();
        let lead = if u < 0.4 {
            6.0 * stream.uniform() - 3.0
        } else if u < 0.6 {
            -0.5 - 0.11 * stream.uniform()
        } else if u < 0.8 {
            -50.0 - 1.5 * stream.uniform()
        } else {
            -60.0 * stream.uniform()
        };
        let explosion = at_years(target.years + lead - distance_ly(site, target.position));
        let channel = if stream.uniform() < 1.0 / 6.0 {
            IaChannel::Iax
        } else {
            IaChannel::Merger
        };
        let before = GalacticVelocity::new([0, 1, 2].map(|_| stream.normal(0.0, 3e4)));
        let after = before + GalacticVelocity::new([stream.normal(0.0, 3e5), 0.0, 0.0]);
        let site = GalacticPosition::from_light_years(site)?;
        let motion = CatalogueMotion::broken(site, explosion, before, after);
        Some((id, site, explosion, channel, motion))
    }

    fn test(&self, _: &Galaxy, candidate: Self::Candidate) -> Option<Self::Entry> {
        let (id, _, explosion, channel, motion) = candidate;
        let entry = SupernovaEntry::new(
            SystemId::from(id),
            explosion,
            SupernovaKind::TypeIa(channel),
            self.window,
        );
        Some((entry, motion))
    }

    fn members(&self, _: &Self::Entry) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<Self::Entry>) -> CatalogueSystem {
        let (_, motion) = member.entry().value();
        let record = SystemRecord::from_parts(
            SystemId::from(member.id()),
            motion.position_at(UniverseTime::EPOCH),
            SystemOrigin::CatalogueSystem(ClassId::TYPE_IA),
            Population::OldThinDisc,
            SolarMasses::new(1.0),
            Years::new(5e9),
        );
        CatalogueSystem::new(record, *motion)
    }
}

impl OneShotClass for ToyTypeIa {
    fn kind(&self) -> TransientKind {
        TransientKind::TypeIa
    }

    fn event(&self, entry: &Self::Entry) -> OneShotEvent {
        OneShotEvent::of_supernova(&entry.0, entry.1)
    }
}

/// A sensor with these bands' detection limits, W m⁻², each resolving at ten times its limit.
fn sensor(bands: &[(AlertBand, f64)]) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    SensorHorizon::new(
        bands
            .iter()
            .map(|&(band, detect)| (band, BandLimits::new(w(detect), w(10.0 * detect)).unwrap())),
    )
    .unwrap()
}

/// The window of `years` Julian years that starts `offset` years from `at`.
fn window(at: f64, offset: f64, years: f64) -> TimeWindow {
    TimeWindow::new(at_years(at + offset), at_years(at + offset + years)).unwrap()
}

/// The walk's detections, as the oracle sees them, sorted.
fn walked<P: OneShotClass>(
    galaxy: &Galaxy,
    noise: &mut NoiseCache,
    cells: &mut KeepCells,
    process: &P,
    query: &AlertQuery,
) -> Vec<Seen> {
    let mut context = AlertContext::new(noise, &NoModifiers).with_one_shot_cells(cells);
    let mut out = Vec::new();
    one_shot_detections(galaxy, &mut context, query, process, &mut out);
    assert!(
        out.windows(2)
            .all(|pair| (pair[0].arrival(), pair[0].event()) < (pair[1].arrival(), pair[1].event())),
        "by arrival, then by event"
    );
    let mut seen: Vec<_> = out.iter().map(Seen::of).collect();
    seen.sort();
    seen
}

/// The oracle's detections.
fn brute<P: OneShotClass>(
    galaxy: &Galaxy,
    noise: &mut NoiseCache,
    cells: &mut KeepCells,
    process: &P,
    query: &AlertQuery,
) -> Vec<Seen> {
    let mut context = AlertContext::new(noise, &NoModifiers);
    brute_force_detections(galaxy, &mut context, cells, process, query)
}

/// The toy class's walk is the brute force's for three observers and windows of a year and a
/// century, at times across the clock window, through V, B and K limits that reach some 1,000,
/// 1,040 and 750 ly, and an X-ray band the class gives no light in. The windows hold onsets and
/// events in progress, seen in each of the three bands, resolvable and not. The walk without a
/// cache gives the same, and a query that does not ask about the class finds nothing.
#[test]
fn one_shot_alerts_of_a_toy_class_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0b00));
    let process = ToyTypeIa::new();
    // At these limits a Type Ia, at SN 2011fe's colours, stands highest over B's through less
    // than about 0.3 mag of A_V, over K's through more than about 0.7, and over V's between.
    let sensor = sensor(&[
        (AlertBand::Photometric(Band::V), 1.25e-4),
        (AlertBand::Photometric(Band::B), 2.2e-4),
        (AlertBand::Photometric(Band::K), 1.5e-5),
        (AlertBand::XRay, 1e-15),
    ]);
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut cells = KeepCells::default();
    let (mut onsets, mut in_progress, mut bands, mut resolvable, mut total) =
        (0, 0, BTreeSet::new(), 0, 0);
    for target in TARGETS {
        for window in [
            window(target.years, -0.5, 1.0),
            window(target.years, -50.0, 100.0),
        ] {
            let query = AlertQuery::builder(observer_at(target.position), window, sensor.clone())
                .build()
                .unwrap();
            let walk = walked(&galaxy, &mut noise, &mut cells, &process, &query);
            let oracle = brute(&galaxy, &mut noise, &mut cells, &process, &query);
            assert_eq!(walk, oracle, "{target:?} over {window:?}");
            for seen in &walk {
                match seen.phase {
                    DetectionPhase::Onset => onsets += 1,
                    DetectionPhase::InProgress => in_progress += 1,
                }
                bands.insert(seen.band);
                resolvable += usize::from(seen.resolvable);
                assert_eq!(seen.class, Some(ClassId::TYPE_IA));
                assert_eq!(seen.kind, TransientKind::TypeIa);
            }
            total += walk.len();
        }
    }
    eprintln!(
        "{total} detections: {onsets} onsets, {in_progress} in progress, {resolvable} resolvable, \
         bands {bands:?}"
    );
    assert!(
        onsets >= 100 && in_progress >= 20,
        "{onsets} onsets, {in_progress} in progress"
    );
    assert_eq!(bands.len(), 3, "{bands:?}");
    assert!(
        resolvable > 0 && resolvable < total,
        "{resolvable} of {total}"
    );

    // Without a cache, the same.
    let target = TARGETS[0];
    let year = window(target.years, -0.5, 1.0);
    let query = AlertQuery::builder(observer_at(target.position), year, sensor.clone())
        .build()
        .unwrap();
    let mut context = AlertContext::new(&mut noise, &NoModifiers).with_one_shot_cells(&mut cells);
    let mut cached = Vec::new();
    one_shot_detections(&galaxy, &mut context, &query, &process, &mut cached);
    let mut nothing = NoOneShotCells;
    let mut context = AlertContext::new(&mut noise, &NoModifiers).with_one_shot_cells(&mut nothing);
    let mut uncached = Vec::new();
    one_shot_detections(&galaxy, &mut context, &query, &process, &mut uncached);
    let seen = |detections: &[_]| detections.iter().map(Seen::of).collect::<Vec<_>>();
    assert_eq!(seen(&cached), seen(&uncached));
    assert_ne!(cached.len(), 0, "the observer sees some");

    // A query that does not ask about the class finds nothing.
    let other = AlertQuery::builder(observer_at(target.position), year, sensor)
        .classes(ClassSet::of(ClassId::CORE_COLLAPSE))
        .build()
        .unwrap();
    let mut context = AlertContext::new(&mut noise, &NoModifiers);
    let mut none = Vec::new();
    one_shot_detections(&galaxy, &mut context, &other, &process, &mut none);
    assert_eq!(none, []);
}

/// The walk gives each of six queries, the three observers' years and centuries, the same
/// detections whatever it walked before it with the same cell and noise caches, forwards,
/// backwards, shuffled and alone.
#[test]
fn one_shot_alerts_do_not_depend_on_what_was_walked_before() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0b00));
    let process = ToyTypeIa::new();
    let sensor = sensor(&[
        (AlertBand::Photometric(Band::V), 1.25e-4),
        (AlertBand::Photometric(Band::K), 1.0e-5),
    ]);
    let queries: Vec<_> = TARGETS
        .iter()
        .flat_map(|target| {
            [(-0.5, 1.0), (-50.0, 100.0)].map(|(offset, years)| {
                AlertQuery::builder(
                    observer_at(target.position),
                    window(target.years, offset, years),
                    sensor.clone(),
                )
                .build()
                .unwrap()
            })
        })
        .collect();
    let cells = RefCell::new(KeepCells::default());
    let noise = RefCell::new(NoiseCache::with_capacity(4_096));
    assert_order_independent(&queries, |query| {
        let (mut noise, mut cells) = (noise.borrow_mut(), cells.borrow_mut());
        walked(&galaxy, &mut noise, &mut cells, &process, query)
    });
}

/// A window split in two finds each onset of the whole once, in the half its light arrives in, and
/// an event the first half saw begin is in progress at the second's start if it is still above the
/// limit then (Design note 6).
#[test]
fn one_shot_alerts_of_a_split_window_find_each_onset_once() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0b00));
    let process = ToyTypeIa::new();
    let sensor = sensor(&[(AlertBand::Photometric(Band::V), 1.25e-4)]);
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut cells = KeepCells::default();
    let target = TARGETS[1];
    let at = observer_at(target.position);
    let mut run = |window| {
        let query = AlertQuery::builder(at, window, sensor.clone())
            .build()
            .unwrap();
        walked(&galaxy, &mut noise, &mut cells, &process, &query)
    };
    let whole = run(window(target.years, -3.0, 6.0));
    let early = run(window(target.years, -3.0, 3.0));
    let late = run(window(target.years, 0.0, 3.0));
    let onsets = |seen: &[Seen]| -> BTreeSet<_> {
        seen.iter()
            .filter(|s| s.phase == DetectionPhase::Onset)
            .map(|s| s.event)
            .collect()
    };
    let (whole_onsets, early_onsets, late_onsets) = (onsets(&whole), onsets(&early), onsets(&late));
    assert!(early_onsets.is_disjoint(&late_onsets));
    assert_eq!(
        whole_onsets,
        early_onsets.union(&late_onsets).copied().collect()
    );
    assert!(early_onsets.len() >= 5 && late_onsets.len() >= 5);
    // What the late half sees in progress began before it: in the early half, or before both.
    for seen in late
        .iter()
        .filter(|s| s.phase == DetectionPhase::InProgress)
    {
        let before = whole.iter().any(|s| s.event == seen.event);
        assert!(before, "{seen:?}");
    }
}

/// The production core collapses: three observers about two recent deaths of the galaxy of seed 9
/// found along the solar circle, with windows of a year and a century, through V and K limits that
/// reach some 1,600 ly; the walk is the brute force's, and each observer sees its death.
#[test]
#[ignore = "slow: generates the core-collapse class's cells about three observers, a few seconds of cells each"]
fn one_shot_alerts_of_the_core_collapses_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = CoreCollapseProcess::new(&galaxy);
    let mut cells = KeepCells::default();
    // The deaths whose light can reach an observer within ±H, by the solar circle.
    let mut deaths = Vec::new();
    for y in 44..56 {
        for x in -6..6 {
            for z in [-1, 0] {
                let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [x, y, z]).unwrap();
                for event in cells.events(&galaxy, &process, key) {
                    let t = event.emitted().since_epoch().as_julian_years_f64();
                    if (-2_000.0..990.0).contains(&t) {
                        deaths.push((t, *event));
                    }
                }
            }
        }
    }
    deaths.sort_by(|a, b| a.0.total_cmp(&b.0));
    eprintln!("{} recent deaths: {deaths:?}", deaths.len());
    let past = deaths
        .iter()
        .find(|(t, _)| *t < -400.0)
        .expect("a death in the past");
    let future = deaths
        .iter()
        .rev()
        .find(|(t, _)| *t > 400.0)
        .expect("a death to come");
    let sensor = sensor(&[
        (AlertBand::Photometric(Band::V), 7.6e-6),
        (AlertBand::Photometric(Band::K), 1.4e-6),
    ]);
    // The observer `away` light-years from `event`'s site along `direction`.
    let place = |event: &OneShotEvent, direction: [f64; 3], away: f64| {
        let site = ly(&event.site());
        observer_at([0, 1, 2].map(|axis| site[axis] + direction[axis] * away))
    };
    let arrival_of = |event: &OneShotEvent, at: &GalacticPosition| -> f64 {
        let light = light_time(event.site().distance_to(at));
        event
            .emitted()
            .checked_add(light)
            .unwrap()
            .since_epoch()
            .as_julian_years_f64()
    };
    let (_, past) = *past;
    let (_, future) = *future;
    // Above the past death, 1,200 ly up, to see it go off; beside it, 800 ly coreward in the
    // plane, to see it in progress 20 days on; 50 ly from the death to come.
    let above = place(&past, [0.0, 0.0, 1.0], 1_200.0);
    let beside = place(&past, [0.0, -1.0, 0.0], 800.0);
    let near = place(&future, [1.0, 0.0, 0.0], 50.0);
    let day = 1.0 / 365.25;
    let cases = [
        (
            above,
            past,
            [(-0.25, 1.0), (-60.0, 100.0)],
            DetectionPhase::Onset,
        ),
        (
            beside,
            past,
            [(20.0 * day, 1.0), (40.0 * day, 100.0)],
            DetectionPhase::InProgress,
        ),
        (
            near,
            future,
            [(-0.5, 1.0), (-50.0, 100.0)],
            DetectionPhase::Onset,
        ),
    ];
    let mut noise = NoiseCache::with_capacity(4_096);
    for (at, death, windows, phase) in cases {
        let arrival = arrival_of(&death, &at);
        for (offset, years) in windows {
            let window = window(arrival, offset, years);
            let query = AlertQuery::builder(at, window, sensor.clone())
                .build()
                .unwrap();
            let walk = walked(&galaxy, &mut noise, &mut cells, &process, &query);
            let oracle = brute(&galaxy, &mut noise, &mut cells, &process, &query);
            eprintln!("{} detections over {window:?}: {walk:?}", walk.len());
            assert_eq!(walk, oracle, "{at:?} over {window:?}");
            assert!(
                walk.iter()
                    .any(|seen| seen.event == death.event() && seen.phase == phase),
                "{death:?} is seen as {phase:?} from {at:?} over {window:?}"
            );
        }
    }
}
