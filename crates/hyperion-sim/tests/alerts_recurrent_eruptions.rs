//! The recurrent walk's luminous blue variables against the brute force (plan 12, P12.T5.c): plan
//! 06's giant eruptions on a 2,000 ly sphere (a visual sensor whose reach for an eruption's bound
//! in V is 2,000 ly), over a year about an eruption's arrival and over a day while it is in
//! progress. The walk's detections are the brute force's as sets, field by field and bit by bit
//! (`common/observe.rs`), and `scan_cell`'s generic form over every cell of the plan, in reverse
//! with fresh contexts, gives what one call over the sphere does.
//!
//! Two classes are walked:
//!
//! - **Toy luminous blue variables**, not physics: plan 06's 60 M☉ star at [Fe/H] = −1 placed
//!   about an observer on the class's grid, each halfway through its LBV window at the epoch, a
//!   window of some 2 × 10⁵ years in which it erupts once in 12,000. The observer stands at their
//!   centre, and the windows are chosen about an eruption whose light arrives in the clock window.
//! - **The production class** of the galaxy of seed 9 (slow), beside the first of its hosts along
//!   the disc whose eruption's light reaches it inside the clock window.

#[expect(dead_code, reason = "the recurrent alerts use only the alerts' oracle")]
mod common;

use std::f64::consts::PI;

use std::cell::RefCell;

use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, Detection, DetectionLight, DetectionPhase,
    EruptionClass, EruptionHost, GIANT_ERUPTION_BAND_PEAKS, SensorHorizon, StarRecipe,
    TransientKind, giant_eruption_scan_cell, kind_reach, recurrent_scan_plan, scan_cell, scan_plan,
};
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::events::{EventSeries, TimeWindow, tags};
use hyperion_sim::galaxy::catalogue_classes::lbv::LbvProcess;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, CatalogueMember, CatalogueMotion, CatalogueSystem,
    ClassId, ClassProcess,
};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{CatalogueSystemId, EventId, SystemId};
use hyperion_sim::observe::light_time;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::events::lbv::{LbvModel, eruptions_in};
use hyperion_sim::stellar::sse::{PhasePredicate, Track};
use hyperion_sim::time::{CLOCK_WINDOW_H, Span, UniverseTime};
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::units::{
    Dex, HeliumExcess, PerCubicLightYear, SolarMasses, WattsPerSquareMetre, Years,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

use common::observe::{Seen, brute_force_eruption_detections, observer_at};

/// Seconds in a Julian year.
const YEAR: f64 = 31_557_600.0;

/// The observer of the toy.
const CENTRE: [f64; 3] = [0.0, 26_000.0, 20.0];

/// The toy's hosts about [`CENTRE`]: each a direction and a distance, ly.
const HOSTS: [([f64; 3], f64); 24] = [
    ([1.0, 0.0, 0.0], 30.0),
    ([-1.0, 0.0, 0.0], 60.0),
    ([0.0, 1.0, 0.0], 90.0),
    ([0.0, -1.0, 0.0], 120.0),
    ([0.0, 0.0, 1.0], 150.0),
    ([0.0, 0.0, -1.0], 180.0),
    ([0.6, 0.8, 0.0], 220.0),
    ([0.8, -0.6, 0.0], 260.0),
    ([-0.6, 0.8, 0.0], 300.0),
    ([-0.8, -0.6, 0.0], 350.0),
    ([0.0, 0.6, 0.8], 400.0),
    ([0.0, -0.8, 0.6], 450.0),
    ([0.6, 0.0, 0.8], 500.0),
    ([-0.6, 0.0, -0.8], 600.0),
    ([0.48, 0.6, 0.64], 700.0),
    ([-0.48, 0.6, -0.64], 800.0),
    ([0.48, -0.6, 0.64], 900.0),
    ([-0.48, -0.6, -0.64], 1_000.0),
    ([1.0, 0.0, 0.0], 1_200.0),
    ([0.0, 1.0, 0.0], 1_400.0),
    ([0.0, 0.0, 1.0], 1_600.0),
    ([-1.0, 0.0, 0.0], 1_800.0),
    ([0.0, -1.0, 0.0], 1_950.0),
    ([0.6, 0.8, 0.0], 2_300.0),
];

/// The time `seconds` after `t`, or before it for a negative number.
fn shifted(t: UniverseTime, seconds: f64) -> UniverseTime {
    let span = Span::from_seconds_f64(seconds.abs()).unwrap();
    if seconds < 0.0 {
        t.checked_sub(span).unwrap()
    } else {
        t.checked_add(span).unwrap()
    }
}

/// The window of `length` seconds from `start`.
fn window(start: UniverseTime, length: f64) -> TimeWindow {
    TimeWindow::new(start, shifted(start, length)).unwrap()
}

/// The visual sensor whose eruptions' bound in V reaches 2,000 ly, resolving at ten times it.
fn visual() -> SensorHorizon {
    let d = 2_000.0 * METRES_PER_LIGHT_YEAR;
    let limit = GIANT_ERUPTION_BAND_PEAKS[2].value() / (4.0 * PI * d * d);
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(limit), w(10.0 * limit)).unwrap();
    SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap()
}

/// Toy luminous blue variables, not physics (module documentation): the class's grid holding, in
/// each cell, candidates 0, 1, … for the toy's hosts whose sites lie in it, each a host of plan
/// 06's star.
struct ToyEruptions {
    /// Each host's site and its star's recipe, and the window its star is one in.
    hosts: Vec<(GalacticPosition, StarRecipe, (UniverseTime, UniverseTime))>,
}

impl ToyEruptions {
    fn new() -> Self {
        let (mass, poor, draws) = (
            SolarMasses::new(60.0),
            Composition::from_fe_h(Dex::new(-1.0), HeliumExcess::new(0.0)),
            StarDraws::median(),
        );
        let ages = Track::full(mass, &poor, &draws)
            .window_where(PhasePredicate::Lbv)
            .expect("a 60 M☉ star at [Fe/H] = −1 is a luminous blue variable");
        let middle = f64::midpoint(ages.start().value(), ages.end().value());
        let recipe = StarRecipe::new(mass, poor, draws, Years::new(middle)).unwrap();
        let window = recipe
            .to_star()
            .lbv_window()
            .expect("a window about the epoch");
        let hosts = HOSTS
            .iter()
            .map(|&(direction, away)| {
                let norm = direction.iter().map(|d| d * d).sum::<f64>().sqrt();
                let site = [0, 1, 2].map(|axis| CENTRE[axis] + direction[axis] / norm * away);
                (observer_at(site), recipe.clone(), window)
            })
            .collect();
        Self { hosts }
    }

    /// The toy's hosts in `cell`, in the toy's order.
    fn in_cell(&self, cell: &CatalogueCellKey) -> Vec<usize> {
        (0..self.hosts.len())
            .filter(|&i| {
                CatalogueCellKey::containing(ClassId::LUMINOUS_BLUE_VARIABLE, &self.hosts[i].0)
                    == Some(*cell)
            })
            .collect()
    }
}

impl ClassProcess for ToyEruptions {
    type Candidate = (CatalogueSystemId, usize);
    type Entry = EruptionHost;

    fn class(&self) -> ClassId {
        ClassId::LUMINOUS_BLUE_VARIABLE
    }

    fn bound(&self, _: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let here = !self.in_cell(cell).is_empty();
        PerCubicLightYear::new(if here { 50.0 / cell.volume_ly3() } else { 0.0 })
    }

    fn candidate(
        &self,
        _: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        _: PerCubicLightYear,
    ) -> Option<Self::Candidate> {
        let index = usize::try_from(id.index()).unwrap();
        self.in_cell(cell).get(index).map(|&host| (id, host))
    }

    fn test(&self, _: &Galaxy, (id, host): Self::Candidate) -> Option<EruptionHost> {
        let (site, recipe, window) = self.hosts[host].clone();
        // Drifting at 30 km/s along +y.
        let motion = CatalogueMotion::straight(site, GalacticVelocity::new([0.0, 3e4, 0.0]));
        EruptionHost::new(SystemId::from(id), window, motion, recipe).ok()
    }

    fn members(&self, _: &EruptionHost) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<EruptionHost>) -> CatalogueSystem {
        let host = member.entry().value();
        let record = SystemRecord::from_parts(
            SystemId::from(member.id()),
            host.motion().position_at(UniverseTime::EPOCH),
            SystemOrigin::CatalogueSystem(ClassId::LUMINOUS_BLUE_VARIABLE),
            Population::YoungThinDisc,
            SolarMasses::new(60.0),
            Years::new(4e6),
        );
        CatalogueSystem::new(record, *host.motion())
    }
}

impl EruptionClass for ToyEruptions {
    fn eruption_host(&self, _: &Galaxy, entry: &EruptionHost) -> Option<EruptionHost> {
        Some(entry.clone())
    }
}

/// The walk over `process`'s cells within the reach, merged and sorted; the same cell by cell in
/// reverse with fresh contexts, which must be it; and the brute force's.
fn walked_and_brute<P: EruptionClass>(
    galaxy: &Galaxy,
    noise: &mut NoiseCache,
    process: &P,
    query: &AlertQuery,
) -> (Vec<Detection>, Vec<Seen>, Vec<Seen>) {
    let cells = recurrent_scan_plan(process.class(), query);
    let mut out = Vec::new();
    {
        let mut context = AlertContext::new(noise, &NoModifiers);
        for &cell in &cells {
            giant_eruption_scan_cell(galaxy, &mut context, query, process, cell, &mut out);
        }
    }
    out.sort_by_key(|d| (d.arrival(), d.event()));
    let mut by_cell = Vec::new();
    for &cell in cells.iter().rev() {
        let mut fresh = NoiseCache::with_capacity(64);
        let mut context = AlertContext::new(&mut fresh, &NoModifiers);
        giant_eruption_scan_cell(galaxy, &mut context, query, process, cell, &mut by_cell);
    }
    let as_seen = |detections: &[Detection]| {
        let mut seen: Vec<Seen> = detections.iter().map(Seen::of).collect();
        seen.sort();
        seen
    };
    let walk = as_seen(&out);
    assert_eq!(
        as_seen(&by_cell),
        walk,
        "scan_cell over the plan is the walk"
    );
    let mut context = AlertContext::new(noise, &NoModifiers);
    let oracle = brute_force_eruption_detections(galaxy, &mut context, process, query);
    (out, walk, oracle)
}

/// An eruption to see: an observer at `position`, the eruption, and when its light arrives there.
#[derive(Debug, Clone, Copy)]
struct Target {
    position: [f64; 3],
    event: EventId,
    arrival: UniverseTime,
    duration: Span,
}

/// The earliest eruption of the toy's hosts whose light reaches [`CENTRE`] inside the clock
/// window, more than a year from its ends.
fn toy_target(galaxy: &Galaxy, process: &ToyEruptions) -> Target {
    let observer = observer_at(CENTRE);
    let h = CLOCK_WINDOW_H.as_seconds_f64() - YEAR;
    let mut best: Option<Target> = None;
    for key in (-1..=1).flat_map(|x| (5..=8).flat_map(move |y| (-1..=1).map(move |z| [x, y, z]))) {
        let key = CatalogueCellKey::new(ClassId::LUMINOUS_BLUE_VARIABLE, key).unwrap();
        for entry in CatalogueClassCell::generate(galaxy, process, key).entries() {
            let host = entry.value();
            let star = host.star().to_star();
            let series =
                EventSeries::new(galaxy.seed(), tags::STAR_LBV_ERUPTION, host.body().into());
            let span = window(
                shifted(UniverseTime::EPOCH, -h - 2_500.0 * YEAR),
                2.0 * h + 2_500.0 * YEAR,
            );
            let mut eruptions = Vec::new();
            eruptions_in(&star, &series, span, &mut eruptions);
            for eruption in eruptions {
                let site = host.motion().position_at(eruption.onset());
                let arrival = eruption
                    .onset()
                    .checked_add(light_time(site.distance_to(&observer)))
                    .unwrap();
                let inside = shifted(UniverseTime::EPOCH, -h) < arrival
                    && arrival < shifted(UniverseTime::EPOCH, h);
                if inside && best.is_none_or(|b| arrival < b.arrival) {
                    best = Some(Target {
                        position: CENTRE,
                        event: eruption.id(),
                        arrival,
                        duration: eruption.duration(),
                    });
                }
            }
        }
    }
    best.expect("the toy's hosts erupt into the clock window")
}

/// The query from `position` over `window`.
fn query(position: [f64; 3], window: TimeWindow) -> AlertQuery {
    AlertQuery::builder(observer_at(position), window, visual())
        .build()
        .unwrap()
}

/// The toy's eruptions on the 2,000 ly sphere, over a year about the earliest eruption arriving in
/// the clock window, over a day halfway through it, and over the whole clock window: the walk is
/// the brute force's, and sees the eruption begin and then in progress; the production dispatch
/// walks the class's own process.
#[test]
fn eruption_alerts_of_toy_luminous_blue_variables_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0c0e));
    let process = ToyEruptions::new();
    let mut noise = NoiseCache::with_capacity(4_096);
    let target = toy_target(&galaxy, &process);
    let year = query(
        target.position,
        window(shifted(target.arrival, -0.5 * YEAR), YEAR),
    );
    let (out, walk, oracle) = walked_and_brute(&galaxy, &mut noise, &process, &year);
    assert_eq!(walk, oracle);
    let onset = out
        .iter()
        .find(|d| d.event() == target.event)
        .expect("the eruption");
    assert_eq!(onset.phase(), DetectionPhase::Onset);
    assert_eq!(onset.kind(), TransientKind::GiantEruption);
    assert_eq!(onset.class(), Some(ClassId::LUMINOUS_BLUE_VARIABLE));
    let halfway = shifted(target.arrival, target.duration.as_seconds_f64() / 2.0);
    let later = query(target.position, window(halfway, 86_400.0));
    let (out, walk, oracle) = walked_and_brute(&galaxy, &mut noise, &process, &later);
    assert_eq!(walk, oracle);
    let running = out
        .iter()
        .find(|d| d.event() == target.event)
        .expect("in progress");
    assert_eq!(running.phase(), DetectionPhase::InProgress);
    eprintln!("{} detections a year about {}", walk.len(), target.arrival);
    // The whole clock window, a year short of each end: some four onsets of the 23 hosts within
    // reach at one in 12,000 years, and those in progress at its start.
    let h = CLOCK_WINDOW_H.as_seconds_f64() - YEAR;
    let clock = query(CENTRE, window(shifted(UniverseTime::EPOCH, -h), 2.0 * h));
    let (_, walk, oracle) = walked_and_brute(&galaxy, &mut noise, &process, &clock);
    assert_eq!(walk, oracle);
    let onsets = walk
        .iter()
        .filter(|s| s.phase == DetectionPhase::Onset)
        .count();
    eprintln!(
        "{} detections over the clock window, {onsets} onsets",
        walk.len()
    );
    assert!(onsets >= 2, "{onsets} onsets");
    // The dispatch by class reads the production class, whose hosts are none of the toy's.
    assert_eq!(
        scan_plan(&galaxy, ClassId::LUMINOUS_BLUE_VARIABLE, &year),
        recurrent_scan_plan(ClassId::LUMINOUS_BLUE_VARIABLE, &year)
    );
}

/// Each eruption's light (P12.T8.b.1) is its own at the observer: zero before its onset arrives
/// and from its end's, the walk's flux at its peak's arrival for an onset and at the window's
/// start for one in progress.
#[test]
fn eruption_alerts_lights_are_the_walks_flux_at_their_own_times() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0c0e));
    let process = ToyEruptions::new();
    let mut noise = NoiseCache::with_capacity(4_096);
    let target = toy_target(&galaxy, &process);
    let nanosecond = Span::new(0, 1).unwrap();
    let halfway = shifted(target.arrival, target.duration.as_seconds_f64() / 2.0);
    for start in [shifted(target.arrival, -86_400.0), halfway] {
        let q = query(target.position, window(start, 2.0 * 86_400.0));
        let (out, ..) = walked_and_brute(&galaxy, &mut noise, &process, &q);
        let d = out
            .iter()
            .find(|d| d.event() == target.event)
            .expect("the eruption");
        let DetectionLight::Lit(light) = d.light() else {
            panic!("an eruption gives light: {d:?}");
        };
        let knots = light.knots();
        assert_eq!(knots.len(), 3);
        assert_eq!(knots[0], d.arrival());
        let at = |t: UniverseTime| light.flux_at(t).value();
        assert_same_bits(at(knots[0].checked_sub(nanosecond).unwrap()), 0.0);
        assert_same_bits(at(knots[2]), 0.0);
        let own = match d.phase() {
            DetectionPhase::Onset => knots[1],
            DetectionPhase::InProgress => q.window().start(),
        };
        assert_same_bits(at(own), d.flux().value());
    }
}

/// The walk gives each of the toy's queries, its target's year and day and the clock window, the
/// same detections whatever it walked before with the same noise cache, forwards, backwards,
/// shuffled and alone.
#[test]
fn eruption_alerts_do_not_depend_on_what_was_walked_before() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0c0e));
    let process = ToyEruptions::new();
    let target = toy_target(&galaxy, &process);
    let h = CLOCK_WINDOW_H.as_seconds_f64() - YEAR;
    let halfway = shifted(target.arrival, target.duration.as_seconds_f64() / 2.0);
    let queries = [
        query(CENTRE, window(shifted(target.arrival, -0.5 * YEAR), YEAR)),
        query(CENTRE, window(halfway, 86_400.0)),
        query(CENTRE, window(shifted(UniverseTime::EPOCH, -h), 2.0 * h)),
    ];
    let noise = RefCell::new(NoiseCache::with_capacity(4_096));
    assert_order_independent(&queries, |q| {
        let mut noise = noise.borrow_mut();
        let mut context = AlertContext::new(&mut noise, &NoModifiers);
        let mut out = Vec::new();
        for cell in recurrent_scan_plan(ClassId::LUMINOUS_BLUE_VARIABLE, q) {
            giant_eruption_scan_cell(&galaxy, &mut context, q, &process, cell, &mut out);
        }
        let mut seen: Vec<Seen> = out.iter().map(Seen::of).collect();
        seen.sort();
        seen
    });
}

/// Pins the toy's eruptions over the clock window from its centre: each detection's event,
/// arrival, phase, band and flux, so that a change to plan 06's eruptions, the shares, plan 07's
/// lines or the walk shows, which the brute force, built on the same, cannot.
#[test]
fn eruption_alerts_golden() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0c0e));
    let process = ToyEruptions::new();
    let h = CLOCK_WINDOW_H.as_seconds_f64() - YEAR;
    let q = query(CENTRE, window(shifted(UniverseTime::EPOCH, -h), 2.0 * h));
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut context = AlertContext::new(&mut noise, &NoModifiers);
    let mut out = Vec::new();
    for cell in recurrent_scan_plan(ClassId::LUMINOUS_BLUE_VARIABLE, &q) {
        giant_eruption_scan_cell(&galaxy, &mut context, &q, &process, cell, &mut out);
    }
    out.sort_by_key(|d| (d.arrival(), d.event()));
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(&format!("{} detections", out.len()));
    for d in &out {
        w.f64(
            &format!(
                "{} arriving {} {:?} in {:?}",
                d.event(),
                d.arrival(),
                d.phase(),
                d.band()
            ),
            d.flux().value(),
        );
    }
    golden!("alerts/recurrent_eruptions", w.as_str());
}

/// The sensor's reach for an eruption is the sphere's.
#[test]
fn the_sensor_reaches_the_sphere() {
    let reach = kind_reach(TransientKind::GiantEruption, &visual()).value();
    assert!((reach / 2_000.0 - 1.0).abs() < 1e-12, "{reach}");
}

/// The production class of the galaxy of seed 9: beside the first host along the disc whose
/// eruption's light reaches a point a light-year above it inside the clock window, over a year
/// about its arrival. The walk is the brute force's, the production dispatch the generic walk's,
/// and the observer sees the eruption.
#[test]
#[ignore = "slow: generates the luminous blue variables' cells within 6,100 ly of a host"]
fn eruption_alerts_of_the_luminous_blue_variables_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = LbvProcess::new(&galaxy);
    let mut noise = NoiseCache::with_capacity(4_096);
    let h = CLOCK_WINDOW_H.as_seconds_f64() - YEAR;
    let clock = window(shifted(UniverseTime::EPOCH, -h - YEAR), 2.0 * h);
    let mut target = None;
    'search: for y in 1..12 {
        for x in [0, -1, 1] {
            for z in [0, -1] {
                let key =
                    CatalogueCellKey::new(ClassId::LUMINOUS_BLUE_VARIABLE, [x, y, z]).unwrap();
                for entry in CatalogueClassCell::generate(&galaxy, &process, key).entries() {
                    let host = process.eruption_host(&galaxy, entry.value()).unwrap();
                    let star = host.star().to_star();
                    let Some(model) = LbvModel::of(&star) else {
                        continue;
                    };
                    let series = EventSeries::new(
                        galaxy.seed(),
                        tags::STAR_LBV_ERUPTION,
                        host.body().into(),
                    );
                    let mut eruptions = Vec::new();
                    model.eruptions_in(&series, clock, &mut eruptions);
                    if let Some(eruption) = eruptions.first() {
                        let [x, y, z] = host
                            .motion()
                            .position_at(eruption.onset())
                            .to_light_years_f64();
                        target = Some(Target {
                            position: [x, y, z + 1.0],
                            event: eruption.id(),
                            arrival: shifted(eruption.onset(), YEAR),
                            duration: eruption.duration(),
                        });
                        break 'search;
                    }
                }
            }
        }
    }
    let target = target.expect("an eruption along the disc inside the clock window");
    let year = query(
        target.position,
        window(shifted(target.arrival, -0.5 * YEAR), YEAR),
    );
    let (out, walk, oracle) = walked_and_brute(&galaxy, &mut noise, &process, &year);
    assert_eq!(walk, oracle);
    assert!(
        out.iter().any(|d| d.event() == target.event),
        "the observer sees its eruption"
    );
    let mut dispatched = Vec::new();
    let mut context = AlertContext::new(&mut noise, &NoModifiers);
    for cell in scan_plan(&galaxy, ClassId::LUMINOUS_BLUE_VARIABLE, &year) {
        scan_cell(
            &galaxy,
            &mut context,
            ClassId::LUMINOUS_BLUE_VARIABLE,
            cell,
            &year,
            &mut dispatched,
        );
    }
    let mut seen: Vec<Seen> = dispatched.iter().map(Seen::of).collect();
    seen.sort();
    assert_eq!(seen, walk);
    eprintln!("{} eruptions seen beside a host", walk.len());
}
