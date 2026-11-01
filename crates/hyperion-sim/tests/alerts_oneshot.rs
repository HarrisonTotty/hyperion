//! The one-shot classes' walk against the brute force (plan 12, P12.T5.b): for three observers and
//! windows of a year and a century, the walk's detections are the brute force's, as sets, field by
//! field and bit by bit (`common/observe.rs`).
//!
//! These classes are walked:
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
//! - **A lone Type Ia**, a class of one entry, to see a slow riser at its onset and not while it
//!   rises below the limit at a window's start.
//! - **The production Type Ia, stellar mergers and neutron-star mergers** of the same galaxy,
//!   each about two of its own events, a year and a century, and the year split in two (slow). The
//!   walk reads the mergers' scan marks, and the brute force their entries, on caches of their own,
//!   and a fast test holds the two to the same events.
//!
//! The walk also gives each query the same detections whatever it walked before with the same
//! caches.

#[expect(dead_code, reason = "the one-shot alerts use only the alerts' oracle")]
mod common;

use std::cell::RefCell;
use std::collections::BTreeSet;

use common::observe::{KeepCells, Seen, brute_force_detections, entry_events, observer_at};
use hyperion_sim::Seed;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, ClassSet, DetectionLight, DetectionPhase,
    NoOneShotCells, OneShotClass, OneShotEvent, OneShotLight, SensorHorizon, TransientKind,
    inverse_square_flux, one_shot_detections, supernova_luminosity, supernova_peak,
};
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::catalogue_classes::binary::{NeutronStarMergers, StellarMergers};
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::supernova::{SupernovaEntry, SupernovaKind};
use hyperion_sim::galaxy::catalogue_classes::type_ia::{IaChannel, TypeIaProcess};
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, CatalogueMember, CatalogueMotion, CatalogueSystem,
    ClassId, ClassProcess,
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
use hyperion_sim::stellar::binary::light_curves::BinaryLightCurve;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::{
    Dex, HydrogenPerCm3, KelvinPerCm3, PerCubicLightYear, SolarMasses, WattsPerSquareMetre, Years,
};
use hyperion_testkit::float::assert_same_bits;
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

    fn event(&self, _: &Galaxy, entry: &Self::Entry) -> Option<OneShotEvent> {
        Some(OneShotEvent::of_supernova(&entry.0, entry.1))
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

/// The window from `years` before `at` to `years` after it, and its halves, which meet at `at`
/// exactly.
fn halves(at: f64, years: f64) -> [TimeWindow; 3] {
    let (start, middle, end) = (at_years(at - years), at_years(at), at_years(at + years));
    [(start, end), (start, middle), (middle, end)].map(|(a, b)| TimeWindow::new(a, b).unwrap())
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

/// Each detection's light (P12.T8.b.1) is its template's in its band through its line of sight:
/// zero before its arrival, finite and non-negative at its knots, and the walk's flux at the
/// detection's own time, an onset's peak and an event in progress's window start.
#[test]
fn one_shot_alerts_lights_are_the_walks_flux_at_their_own_times() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0b00));
    let process = ToyTypeIa::new();
    let sensor = sensor(&[
        (AlertBand::Photometric(Band::V), 1.25e-4),
        (AlertBand::Photometric(Band::B), 2.2e-4),
        (AlertBand::Photometric(Band::K), 1.5e-5),
    ]);
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut cells = KeepCells::default();
    let mut phases = BTreeSet::new();
    for target in TARGETS {
        let window = window(target.years, -0.5, 1.0);
        let query = AlertQuery::builder(observer_at(target.position), window, sensor.clone())
            .build()
            .unwrap();
        let mut context =
            AlertContext::new(&mut noise, &NoModifiers).with_one_shot_cells(&mut cells);
        let mut out = Vec::new();
        one_shot_detections(&galaxy, &mut context, &query, &process, &mut out);
        for d in &out {
            let DetectionLight::Lit(light) = d.light() else {
                panic!("a supernova gives light: {d:?}");
            };
            assert_eq!(light.band(), d.band());
            let knots = light.knots();
            assert!(knots.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(knots[0], d.arrival());
            let before = d.arrival().checked_sub(Span::new(0, 1).unwrap()).unwrap();
            assert!(light.flux_at(before).value().abs() < f64::MIN_POSITIVE);
            let fluxes: Vec<f64> = knots.iter().map(|&t| light.flux_at(t).value()).collect();
            assert!(fluxes.iter().all(|f| f.is_finite() && *f >= 0.0), "{d:?}");
            let flux = d.flux().value();
            match d.phase() {
                DetectionPhase::Onset => {
                    let peak = fluxes.iter().copied().fold(0.0, f64::max);
                    assert!((peak - flux).abs() <= 1e-9 * flux, "{d:?}: its peak {peak}");
                }
                DetectionPhase::InProgress => {
                    let opening = light.flux_at(window.start()).value();
                    assert_same_bits(opening, flux);
                }
            }
            phases.insert(d.phase());
        }
    }
    assert_eq!(phases.len(), 2, "onsets and events in progress");
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
    let [whole, early, late] = halves(target.years, 3.0).map(&mut run);
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
    // What the late half sees in progress arrived before it.
    let middle = at_years(target.years);
    for seen in late
        .iter()
        .filter(|s| s.phase == DetectionPhase::InProgress)
    {
        assert!(seen.arrival < middle, "{seen:?}");
    }
}

/// A window split in two: the first half's events in progress are the whole window's, bit for
/// bit, since both are judged at the window's start, and the whole's detections are the first
/// half's with the second half's onsets (Design note 6: onsets once, and each window's own events
/// in progress at its start).
#[test]
fn one_shot_alerts_in_progress_in_a_first_half_are_the_whole_windows() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0b00));
    let process = ToyTypeIa::new();
    let sensor = sensor(&[(AlertBand::Photometric(Band::V), 1.25e-4)]);
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut cells = KeepCells::default();
    let mut late_counts = [0; 2];
    for target in TARGETS {
        let at = observer_at(target.position);
        let mut run = |window| {
            let query = AlertQuery::builder(at, window, sensor.clone())
                .build()
                .unwrap();
            walked(&galaxy, &mut noise, &mut cells, &process, &query)
        };
        let [whole, early, late] = halves(target.years, 0.5).map(&mut run);
        let in_progress = |seen: &[Seen]| -> Vec<Seen> {
            seen.iter()
                .filter(|s| s.phase == DetectionPhase::InProgress)
                .copied()
                .collect()
        };
        assert_ne!(in_progress(&whole).len(), 0, "{target:?}");
        assert_eq!(in_progress(&early), in_progress(&whole), "{target:?}");
        // The second halves have onsets and events in progress of their own.
        for (count, phase) in late_counts
            .iter_mut()
            .zip([DetectionPhase::Onset, DetectionPhase::InProgress])
        {
            *count += late.iter().filter(|s| s.phase == phase).count();
        }
        let mut joined: Vec<_> = early
            .iter()
            .chain(late.iter().filter(|s| s.phase == DetectionPhase::Onset))
            .copied()
            .collect();
        joined.sort();
        assert_eq!(joined, whole, "{target:?}");
    }
    assert!(late_counts.iter().all(|&n| n > 0), "{late_counts:?}");
}

/// One Type Ia, not physics: the Type Ia grid's one entry, candidate 0 of the cell holding `site`
/// (fifty expected there, so that candidate 0 is all but certain to exist, none elsewhere), a
/// normal Type Ia exploding at `explosion` and standing still.
struct Lone {
    site: GalacticPosition,
    explosion: UniverseTime,
    window: ShellWindow,
}

impl ClassProcess for Lone {
    type Candidate = CatalogueSystemId;
    type Entry = (SupernovaEntry, CatalogueMotion);

    fn class(&self) -> ClassId {
        ClassId::TYPE_IA
    }

    fn bound(&self, _: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let home = CatalogueCellKey::containing(ClassId::TYPE_IA, &self.site);
        PerCubicLightYear::new(if home == Some(*cell) {
            50.0 / cell.volume_ly3()
        } else {
            0.0
        })
    }

    fn candidate(
        &self,
        _: &Galaxy,
        _: &CatalogueCellKey,
        id: CatalogueSystemId,
        _: PerCubicLightYear,
    ) -> Option<CatalogueSystemId> {
        (id.index() == 0).then_some(id)
    }

    fn test(&self, _: &Galaxy, id: CatalogueSystemId) -> Option<Self::Entry> {
        let entry = SupernovaEntry::new(
            SystemId::from(id),
            self.explosion,
            SupernovaKind::TypeIa(IaChannel::Merger),
            self.window,
        );
        let still = GalacticVelocity::default();
        let motion = CatalogueMotion::broken(self.site, self.explosion, still, still);
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

impl OneShotClass for Lone {
    fn kind(&self) -> TransientKind {
        TransientKind::TypeIa
    }

    fn event(&self, _: &Galaxy, entry: &Self::Entry) -> Option<OneShotEvent> {
        Some(OneShotEvent::of_supernova(&entry.0, entry.1))
    }
}

/// A slow riser whose first light arrives below the limit: a Type Ia gives no light at its onset
/// and peaks 15 days on, here at twice a V sensor's limit 500 ly above it, out of the plane.
///
/// - A window that holds its onset sees it, at its peak, though its first light is below the
///   limit.
/// - A window that opens a day after its onset, while it is still rising below the limit, sees
///   nothing, though it passes the limit inside the window: "in progress" is judged at the
///   window's start (Design note 6), and the onset was the earlier window's.
/// - A window that opens 20 days after its onset, past its peak and still above the limit, sees it
///   in progress, at its light then: through the same line of sight, the two fluxes stand as the
///   peak to the light 20 days on.
///
/// The brute force agrees each time.
#[test]
fn one_shot_alerts_see_a_slow_riser_at_its_onset_not_while_it_rises_below_the_limit() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0b01));
    let site = observer_at([0.0, 26_000.0, 2_000.0]);
    let at = observer_at([0.0, 26_000.0, 2_500.0]);
    let gas = SiteGas::uniform(HydrogenPerCm3::new(0.1), KelvinPerCm3::new(3_000.0)).unwrap();
    let process = Lone {
        site,
        explosion: at_years(-500.0),
        window: shell_window(&gas, ExplosionEnergy::MEDIAN, Dex::new(0.0)),
    };
    let ia = SupernovaKind::TypeIa(IaChannel::Merger);
    let v = AlertBand::Photometric(Band::V);
    let distance = site.distance_to(&at);
    let bare = |watts| inverse_square_flux(watts, distance).unwrap().value();
    let day_in_years = 1.0 / 365.25;
    let at_day = |days: f64| bare(supernova_luminosity(ia, Years::new(days * day_in_years), v));
    let peak = bare(supernova_peak(ia, v));
    let limit = 0.5 * peak;
    let sensor = sensor(&[(v, limit)]);
    let onset = process
        .explosion
        .checked_add(light_time(distance))
        .unwrap()
        .since_epoch()
        .as_julian_years_f64();
    assert!(
        at_day(0.0) < limit && at_day(1.0) < limit && at_day(20.0) > limit,
        "{:e}, {:e} and {:e} W m⁻² at 0, 1 and 20 days against {limit:e}",
        at_day(0.0),
        at_day(1.0),
        at_day(20.0)
    );
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut cells = KeepCells::default();
    let mut run = |window| {
        let query = AlertQuery::builder(at, window, sensor.clone())
            .build()
            .unwrap();
        let walk = walked(&galaxy, &mut noise, &mut cells, &process, &query);
        let oracle = brute(&galaxy, &mut noise, &mut cells, &process, &query);
        assert_eq!(walk, oracle, "{window:?}");
        walk
    };

    let seen = run(window(onset, -0.5, 1.0));
    let [riser] = seen.as_slice() else {
        panic!("one detection: {seen:?}");
    };
    assert_eq!(riser.phase, DetectionPhase::Onset);
    let flux = f64::from_bits(riser.flux);
    assert!(flux >= limit && flux <= peak, "{flux:e} against {limit:e}");

    let rising = run(window(onset, day_in_years, 1.0));
    assert_eq!(rising, [], "still below the limit a day on");

    let seen = run(window(onset, 20.0 * day_in_years, 1.0));
    let [past] = seen.as_slice() else {
        panic!("one detection: {seen:?}");
    };
    assert_eq!(past.phase, DetectionPhase::InProgress);
    assert_eq!(past.event, riser.event);
    let flux = f64::from_bits(past.flux);
    assert!(flux >= limit, "{flux:e}");
    let ratio = f64::from_bits(riser.flux) / flux / (peak / at_day(20.0));
    assert!((ratio - 1.0).abs() < 1e-9, "{ratio}");
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

/// The merger classes' walk reads each cell's events from its scan marks, and they are its
/// entries' own events, event for event: the stellar mergers of a stretch of the solar circle and
/// every neutron-star merger cell of the galaxy, where a black hole's merger that ejects nothing
/// lists none.
#[test]
fn the_merger_classes_scan_marks_give_their_entries_events() {
    let galaxy = Galaxy::new(Seed::new(9));
    let mergers = StellarMergers::new(&galaxy);
    let mut seen = 0;
    for x in -2..2 {
        let key = CatalogueCellKey::new(ClassId::STELLAR_MERGER, [x, 6, 0]).unwrap();
        let events = mergers.cell_events(&galaxy, key);
        assert_eq!(events, entry_events(&galaxy, &mergers, key), "{key:?}");
        assert!(events.iter().all(|event| matches!(
            event.light(),
            OneShotLight::Binary(BinaryLightCurve::LuminousRedNova(_))
        )));
        seen += events.len();
    }
    assert!(seen > 100, "{seen} red novae");
    let neutron_stars = NeutronStarMergers::new(&galaxy);
    let (mut events, mut entries) = (0, 0);
    for x in -16..16 {
        for y in -16..16 {
            for z in -16..16 {
                let key = CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, [x, y, z]).unwrap();
                let listed = neutron_stars.cell_events(&galaxy, key);
                assert_eq!(
                    listed,
                    entry_events(&galaxy, &neutron_stars, key),
                    "{key:?}"
                );
                events += listed.len();
                entries += CatalogueClassCell::generate(&galaxy, &neutron_stars, key)
                    .entries()
                    .len();
            }
        }
    }
    eprintln!("{events} kilonovae of {entries} neutron-star mergers");
    assert!(events > 0 && events <= entries);
}

/// One observer of a production class's event: its direction from the event's site, how far, its
/// two windows about the event's arrival there (each an offset and a length, Julian years), and how
/// it sees the event.
#[derive(Debug, Clone, Copy)]
struct Watch {
    direction: [f64; 3],
    away: f64,
    windows: [(f64, f64); 2],
    phase: DetectionPhase,
}

/// The observer `away` light-years from `event`'s site along `direction`.
fn placed(event: &OneShotEvent, direction: [f64; 3], away: f64) -> GalacticPosition {
    let site = ly(&event.site());
    observer_at([0, 1, 2].map(|axis| site[axis] + direction[axis] * away))
}

/// When `event`'s light reaches `at`, Julian years from the epoch.
fn arrival_years(event: &OneShotEvent, at: &GalacticPosition) -> f64 {
    let light = light_time(event.site().distance_to(at));
    event
        .emitted()
        .checked_add(light)
        .unwrap()
        .since_epoch()
        .as_julian_years_f64()
}

/// The walk's detections of `query` in full, each with its light: its light's knots strictly
/// increase from its arrival, its flux is zero before them, and it is the walk's flux at the
/// detection's own time, an onset's peak within 10⁻⁹ and an event in progress's window start bit
/// for bit.
fn assert_lights_are_the_walks<P: OneShotClass>(
    galaxy: &Galaxy,
    noise: &mut NoiseCache,
    cells: &mut KeepCells,
    process: &P,
    query: &AlertQuery,
) {
    let mut context = AlertContext::new(noise, &NoModifiers).with_one_shot_cells(cells);
    let mut out = Vec::new();
    one_shot_detections(galaxy, &mut context, query, process, &mut out);
    for d in &out {
        let DetectionLight::Lit(light) = d.light() else {
            panic!("a one-shot event gives light: {d:?}");
        };
        let knots = light.knots();
        assert!(knots.windows(2).all(|pair| pair[0] < pair[1]), "{d:?}");
        assert_eq!(knots[0], d.arrival());
        let before = d.arrival().checked_sub(Span::new(0, 1).unwrap()).unwrap();
        assert_same_bits(light.flux_at(before).value(), 0.0);
        let flux = d.flux().value();
        match d.phase() {
            DetectionPhase::Onset => {
                let peak = knots
                    .iter()
                    .map(|&t| light.flux_at(t).value())
                    .fold(0.0, f64::max);
                assert!((peak - flux).abs() <= 1e-9 * flux, "{d:?}: its peak {peak}");
            }
            DetectionPhase::InProgress => {
                assert_same_bits(light.flux_at(query.window().start()).value(), flux);
            }
        }
    }
}

/// `process`'s walk is the brute force's for each of `watches`, an observer about one of the
/// class's events: over the watch's two windows, and its first split in two at its middle, as
/// sets, with the walk and the oracle on caches of their own, and the walk's lights its flux at
/// their own times. Each observer sees its event in its watch's phase over both windows, and the
/// halves find the first window's onsets, each once.
fn assert_the_walk_about_its_events<P: OneShotClass>(
    galaxy: &Galaxy,
    process: &P,
    sensor: &SensorHorizon,
    watches: &[(OneShotEvent, Watch)],
) {
    let mut noise = NoiseCache::with_capacity(4_096);
    let (mut walk_cells, mut oracle_cells) = (KeepCells::default(), KeepCells::default());
    let mut run = |at: GalacticPosition, window: TimeWindow| {
        let query = AlertQuery::builder(at, window, sensor.clone())
            .build()
            .unwrap();
        let walk = walked(galaxy, &mut noise, &mut walk_cells, process, &query);
        let oracle = brute(galaxy, &mut noise, &mut oracle_cells, process, &query);
        eprintln!("{} detections over {window:?}: {walk:?}", walk.len());
        assert_eq!(walk, oracle, "{at:?} over {window:?}");
        assert_lights_are_the_walks(galaxy, &mut noise, &mut walk_cells, process, &query);
        walk
    };
    let onsets = |seen: &[Seen]| -> BTreeSet<_> {
        seen.iter()
            .filter(|s| s.phase == DetectionPhase::Onset)
            .map(|s| s.event)
            .collect()
    };
    for (event, watch) in watches {
        let at = placed(event, watch.direction, watch.away);
        let arrival = arrival_years(event, &at);
        for (k, (offset, years)) in watch.windows.into_iter().enumerate() {
            let whole = window(arrival, offset, years);
            let seen = run(at, whole);
            assert!(
                seen.iter()
                    .any(|s| s.event == event.event() && s.phase == watch.phase),
                "{event:?} is seen as {:?} from {at:?} over {whole:?}",
                watch.phase
            );
            if k == 0 {
                let middle = at_years(arrival + offset + years / 2.0);
                let early = run(at, TimeWindow::new(whole.start(), middle).unwrap());
                let late = run(at, TimeWindow::new(middle, whole.end()).unwrap());
                let (early, late) = (onsets(&early), onsets(&late));
                assert!(early.is_disjoint(&late));
                assert_eq!(onsets(&seen), early.union(&late).copied().collect());
            }
        }
    }
}

/// Above and beside one event to see it go off and in progress, and near another to see it go
/// off: the three watches of a production class's slow test, at `above`, `beside` and `near`
/// light-years, an event in progress seen `late` and twice `late` years after its onset arrives.
fn three_watches(
    first: OneShotEvent,
    second: OneShotEvent,
    (above, beside, near): (f64, f64, f64),
    late: f64,
) -> [(OneShotEvent, Watch); 3] {
    let onset = [(-0.25, 1.0), (-60.0, 100.0)];
    [
        (
            first,
            Watch {
                direction: [0.0, 0.0, 1.0],
                away: above,
                windows: onset,
                phase: DetectionPhase::Onset,
            },
        ),
        (
            first,
            Watch {
                direction: [0.0, -1.0, 0.0],
                away: beside,
                windows: [(late, 1.0), (2.0 * late, 100.0)],
                phase: DetectionPhase::InProgress,
            },
        ),
        (
            second,
            Watch {
                direction: [1.0, 0.0, 0.0],
                away: near,
                windows: onset,
                phase: DetectionPhase::Onset,
            },
        ),
    ]
}

/// The events of `process`'s cells in `cells` that went off in `years`, Julian years from the
/// epoch, by time.
fn events_between<P: OneShotClass>(
    galaxy: &Galaxy,
    process: &P,
    cells: impl IntoIterator<Item = [i32; 3]>,
    years: std::ops::Range<f64>,
) -> Vec<OneShotEvent> {
    let mut found = Vec::new();
    for cell in cells {
        let Ok(key) = CatalogueCellKey::new(process.class(), cell) else {
            continue;
        };
        for event in process.cell_events(galaxy, key) {
            let t = event.emitted().since_epoch().as_julian_years_f64();
            if years.contains(&t) {
                found.push(event);
            }
        }
    }
    found.sort_by_key(OneShotEvent::emitted);
    found
}

/// The production Type Ia of seed 9's galaxy: two of the explosions of the disc within 30,000 ly
/// of the centre in the last millennium or so and the next 700 years, the earliest and the latest,
/// seen from 200 ly above the first go off, from 150 ly beside it in progress 20 and 40 days on,
/// and from 50 ly beside the second go off, through V and K limits that reach some 1,100 and 650
/// ly; the walk is the brute force's.
#[test]
#[ignore = "slow: generates the Type Ia class's cells of the disc and about three observers"]
fn one_shot_alerts_of_the_type_ia_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = TypeIaProcess::new(&galaxy);
    let disc = (-60..60).flat_map(|y| (-60..60).flat_map(move |x| (-2..2).map(move |z| [x, y, z])));
    let found = events_between(&galaxy, &process, disc, -1_050.0..700.0);
    eprintln!("{} Type Ia: {found:?}", found.len());
    let (Some(&first), Some(&second)) = (found.first(), found.last()) else {
        panic!("two Type Ia in the disc");
    };
    assert_ne!(first.event(), second.event());
    let sensor = sensor(&[
        (AlertBand::Photometric(Band::V), 1.0e-4),
        (AlertBand::Photometric(Band::K), 2.0e-5),
    ]);
    let watches = three_watches(first, second, (200.0, 150.0, 50.0), 20.0 / 365.25);
    assert_the_walk_about_its_events(&galaxy, &process, &sensor, &watches);
}

/// The production stellar mergers of seed 9's galaxy: the two heaviest of a stretch of the solar
/// circle that merge within the last millennium or so and the next 700 years, seen from 200 ly
/// above the first go off, from 150 ly beside it on its plateau 20 and 40 days on, and from 50 ly
/// beside the second go off, through V and K limits of 10⁻⁹ W m⁻². The red novae's bound, at the
/// class's heaviest merger (P12.T5.b), takes the reach to the diagonal: the walk reads every
/// merger's scan marks and the brute force every entry, and they agree.
#[test]
#[ignore = "slow: every stellar merger of the galaxy, as scan marks and as entries"]
fn one_shot_alerts_of_the_stellar_mergers_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = StellarMergers::new(&galaxy);
    let stretch = (5..8).flat_map(|y| (-2..2).flat_map(move |x| [[x, y, -1], [x, y, 0]]));
    let mut found = events_between(&galaxy, &process, stretch, -1_050.0..700.0);
    let peak = |event: &OneShotEvent| match event.light() {
        OneShotLight::Binary(curve) => curve.peak_luminosity().value(),
        OneShotLight::Supernova(_) => unreachable!("a merger's red nova"),
    };
    found.sort_by(|a, b| peak(b).total_cmp(&peak(a)));
    eprintln!("{} red novae: {found:?}", found.len());
    let [first, second, ..] = found.as_slice() else {
        panic!("two red novae by the solar circle");
    };
    let sensor = sensor(&[
        (AlertBand::Photometric(Band::V), 1.0e-9),
        (AlertBand::Photometric(Band::K), 1.0e-9),
    ]);
    let watches = three_watches(*first, *second, (200.0, 150.0, 50.0), 20.0 / 365.25);
    assert_the_walk_about_its_events(&galaxy, &process, &sensor, &watches);
}

/// The two latest kilonovae of seed 9's galaxy, some ten neutron-star mergers in the source
/// horizon, all but a few long ago, whose light reaches the clock window from tens of thousands of
/// light-years: observers out of the plane above and below the first, to see it go off and in
/// progress 5 and 10 days on, and one toward the cube's farthest corner from the second, to see it
/// go off, each a century after the epoch; and a K limit of 2 × 10⁻¹¹ W m⁻², which reaches the
/// diagonal.
fn kilonova_watches(
    galaxy: &Galaxy,
    process: &NeutronStarMergers,
) -> ([(OneShotEvent, Watch); 3], SensorHorizon) {
    let every =
        (-16..16).flat_map(|x| (-16..16).flat_map(move |y| (-16..16).map(move |z| [x, y, z])));
    let found = events_between(galaxy, process, every, -1e9..1e9);
    eprintln!("{} kilonovae: {found:?}", found.len());
    let [.., second, first] = found.as_slice() else {
        panic!("two kilonovae in the horizon");
    };
    let away = |event: &OneShotEvent| 100.0 - event.emitted().since_epoch().as_julian_years_f64();
    let site = ly(&second.site());
    let corner = [0, 1, 2].map(|axis| {
        if site[axis] < 0.0 {
            65_000.0
        } else {
            -65_000.0
        }
    });
    let span = distance_ly(site, corner);
    assert!(span > away(second) + 1_000.0, "{span} ly to the corner");
    let toward = [0, 1, 2].map(|axis| (corner[axis] - site[axis]) / span);
    let onset = [(-0.25, 1.0), (-60.0, 100.0)];
    let day = 1.0 / 365.25;
    let watch = |direction, away, windows, phase| Watch {
        direction,
        away,
        windows,
        phase,
    };
    let watches = [
        (
            *first,
            watch([0.0, 0.0, 1.0], away(first), onset, DetectionPhase::Onset),
        ),
        (
            *first,
            watch(
                [0.0, 0.0, -1.0],
                away(first),
                [(5.0 * day, 1.0), (10.0 * day, 100.0)],
                DetectionPhase::InProgress,
            ),
        ),
        (
            *second,
            watch(toward, away(second), onset, DetectionPhase::Onset),
        ),
    ];
    (
        watches,
        sensor(&[(AlertBand::Photometric(Band::K), 2.0e-11)]),
    )
}

/// The production neutron-star mergers of seed 9's galaxy, about their two latest kilonovae
/// ([`kilonova_watches`]); the walk is the brute force's.
#[test]
#[ignore = "slow: every neutron-star merger cell of the galaxy, and lines of sight across it"]
fn one_shot_alerts_of_the_neutron_star_mergers_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = NeutronStarMergers::new(&galaxy);
    let (watches, sensor) = kilonova_watches(&galaxy, &process);
    assert_the_walk_about_its_events(&galaxy, &process, &sensor, &watches);
}

/// A merger class's walk, its cells' events read from their scan marks behind the cache, gives
/// each of six queries about seed 9's two latest kilonovae ([`kilonova_watches`]), a year and a
/// century from each observer, the same detections whatever it walked before with the same cell
/// and noise caches, forwards, backwards, shuffled and alone; and the queries see some.
#[test]
fn one_shot_alerts_of_a_merger_class_do_not_depend_on_what_was_walked_before() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = NeutronStarMergers::new(&galaxy);
    let (watches, sensor) = kilonova_watches(&galaxy, &process);
    let queries: Vec<_> = watches
        .iter()
        .flat_map(|(event, watch)| {
            let at = placed(event, watch.direction, watch.away);
            let arrival = arrival_years(event, &at);
            watch.windows.map(|(offset, years)| {
                AlertQuery::builder(at, window(arrival, offset, years), sensor.clone())
                    .build()
                    .unwrap()
            })
        })
        .collect();
    let cells = RefCell::new(KeepCells::default());
    let noise = RefCell::new(NoiseCache::with_capacity(4_096));
    let seen = RefCell::new(0);
    assert_order_independent(&queries, |query| {
        let (mut noise, mut cells) = (noise.borrow_mut(), cells.borrow_mut());
        let walk = walked(&galaxy, &mut noise, &mut cells, &process, query);
        *seen.borrow_mut() += walk.len();
        walk
    });
    assert!(*seen.borrow() > 0, "the queries see kilonovae");
}
