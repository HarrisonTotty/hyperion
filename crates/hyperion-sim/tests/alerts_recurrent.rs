//! The recurrent classes' walk against the brute force (plan 12, P12.T5.c), so far the core
//! collapses' magnetars: on a 2,000 ly sphere (an X-ray sensor whose reach for a magnetar's
//! brightest event is 2,000 ly), for three observers and windows of a year and a century, the
//! walk's detections are the brute force's, as sets, field by field and bit by bit
//! (`common/observe.rs`); and `scan_cell` over every cell of the plan gives what one call over the
//! sphere does.
//!
//! Two classes are walked:
//!
//! - **Toy magnetars**, not physics: placed young magnetars of plan 06's model on the core
//!   collapses' grid, strong enough that the windows hold events. One observer stands a tenth of a
//!   light-year from one to see its short bursts, two stand some hundreds of light-years from groups of them to see
//!   their giant flares, and a few lie beyond the reach (fast for a year, slow for a century).
//! - **The production core collapses** of a galaxy, about two points of the solar circle and a
//!   tenth of a light-year from its strongest magnetar active at the epoch (slow).
//!
//! The walk also gives each query the same detections whatever it walked before with the same
//! caches, and with none; and a golden pins the near observer's year.

#[expect(dead_code, reason = "the recurrent alerts use only the alerts' oracle")]
mod common;

use std::cell::RefCell;

use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, Detection, DetectionLight, DetectionPhase,
    MagnetarCells, MagnetarClass, MagnetarHost, NoMagnetarCells, SensorHorizon, StarRecipe,
    TransientKind, magnetar_detections, magnetar_scan_cell, magnetar_scan_plan, scan_cell,
    scan_plan,
};
use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueMember, CatalogueMotion, CatalogueSystem, ClassId, ClassProcess,
};
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::id::{CatalogueSystemId, SystemId};
use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use hyperion_sim::stellar::system::StarModel;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::{Gauss, PerCubicLightYear, SolarMasses, WattsPerSquareMetre, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};

use common::observe::{KeepMagnetarCells, Seen, brute_force_magnetar_detections, observer_at};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

/// The X-ray sensor's detection limit, W m⁻²: at it the row's bound, a giant flare's spike at the
/// top of its law in 2–10 keV, 1.8 × 10³⁸ W, reaches 2,000 ly.
const LIMIT: f64 = 0.040_02;

/// One observer of the toy: where it stands, the time its magnetars' light reaches it about, and
/// its magnetars, each a direction, a distance (ly), a birth field (G) and its neutron star's age
/// (years) when the light that reaches the observer at that time left it.
#[derive(Debug, Clone, Copy)]
struct Target {
    position: [f64; 3],
    years: f64,
    magnetars: &'static [([f64; 3], f64, f64, f64)],
}

/// The toy's three observers (module documentation).
const TARGETS: [Target; 3] = [
    // A tenth of a light-year from a young magnetar of 8 × 10¹⁵ G, 0.3 ly from an older one, and
    // one beyond the reach.
    Target {
        position: [0.0, 26_000.0, 20.0],
        years: 0.0,
        magnetars: &[
            ([1.0, 0.0, 0.0], 0.1, 8e15, 50.0),
            ([0.0, 0.0, 1.0], 0.3, 4e15, 400.0),
            ([0.0, 1.0, 0.0], 2_300.0, 8e15, 100.0),
        ],
    },
    // Some hundreds of light-years from eight, above the plane, 600 years before the epoch.
    Target {
        position: [3_000.0, 22_000.0, 900.0],
        years: -600.0,
        magnetars: &[
            ([1.0, 0.0, 0.0], 150.0, 6e15, 80.0),
            ([-1.0, 0.0, 0.0], 220.0, 4e15, 300.0),
            ([0.0, 1.0, 0.0], 180.0, 8e15, 60.0),
            ([0.0, -1.0, 0.0], 260.0, 3e15, 500.0),
            ([0.0, 0.0, 1.0], 120.0, 5e15, 200.0),
            ([0.0, 0.0, -1.0], 300.0, 7e15, 120.0),
            ([0.6, 0.8, 0.0], 90.0, 2e15, 900.0),
            ([0.0, 0.6, -0.8], 1_900.0, 9e15, 50.0),
        ],
    },
    // Far out in the disc 700 years after the epoch, and one beyond the reach.
    Target {
        position: [-15_000.0, 30_000.0, -100.0],
        years: 700.0,
        magnetars: &[
            ([1.0, 0.0, 0.0], 110.0, 6e15, 90.0),
            ([0.0, -1.0, 0.0], 160.0, 9e15, 40.0),
            ([0.0, 0.0, 1.0], 240.0, 4e15, 250.0),
            ([-0.8, 0.6, 0.0], 2_600.0, 9e15, 40.0),
        ],
    },
];

/// The toy magnetars' star: 15 M☉ at solar composition with the median draws but the neutron
/// star's, born with the field `field` and a 0.2 s period, `age_at_epoch` old at the epoch.
fn recipe(field: f64, age_at_epoch: Years) -> StarRecipe {
    let z = |value: f64| StandardNormal::new(value).unwrap();
    // log₁₀ B₀ = 12.95 + 0.6 z and P₀ = 0.25 + 0.1 z (plan 06's neutron star).
    let draws = StarDraws::from_parts(StarDrawsParts {
        ns_field: z((math::log10(field) - 12.95) / 0.6),
        ns_spin: [z(-0.5); 8],
        ..StarDrawsParts::MEDIAN
    });
    StarRecipe::new(
        SolarMasses::new(15.0),
        Composition::SOLAR,
        draws,
        age_at_epoch,
    )
    .unwrap()
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

/// Toy magnetars, not physics (module documentation): the core collapses' grid holding, in each
/// cell, candidates 0, 1, … for the toy's magnetars whose sites lie in it (fifty expected per cell,
/// so that every one is all but certain to have its candidate), each a host of plan 06's model.
struct ToyMagnetars {
    /// Each magnetar's site, birth field (G), birth, and its star's age at the epoch.
    hosts: Vec<(GalacticPosition, f64, UniverseTime, Years)>,
}

impl ToyMagnetars {
    fn new() -> Self {
        let lifetime = StarModel::new(
            SolarMasses::new(15.0),
            Composition::SOLAR,
            StarDraws::from_parts(StarDrawsParts::MEDIAN),
            Years::new(1e9),
        )
        .unwrap()
        .lifetime()
        .unwrap()
        .value();
        let mut hosts = Vec::new();
        for target in TARGETS {
            for &(direction, away, field, age) in target.magnetars {
                let norm = direction.iter().map(|d| d * d).sum::<f64>().sqrt();
                let site =
                    [0, 1, 2].map(|axis| target.position[axis] + direction[axis] / norm * away);
                // The light that reaches the target at its time left at its NS age `age`.
                let birth = target.years - away - age;
                hosts.push((
                    observer_at(site),
                    field,
                    at_years(birth),
                    Years::new(lifetime - birth),
                ));
            }
        }
        Self { hosts }
    }

    /// The toy's magnetars in `cell`, in the toy's order.
    fn in_cell(&self, cell: &CatalogueCellKey) -> Vec<usize> {
        (0..self.hosts.len())
            .filter(|&i| {
                CatalogueCellKey::containing(ClassId::CORE_COLLAPSE, &self.hosts[i].0)
                    == Some(*cell)
            })
            .collect()
    }
}

impl ClassProcess for ToyMagnetars {
    type Candidate = (CatalogueSystemId, usize);
    type Entry = MagnetarHost;

    fn class(&self) -> ClassId {
        ClassId::CORE_COLLAPSE
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

    fn test(&self, _: &Galaxy, (id, host): Self::Candidate) -> Option<MagnetarHost> {
        let (site, field, birth, age) = self.hosts[host];
        // The remnant kicked to 400 km/s along +x, its progenitor at rest.
        let kicked = GalacticVelocity::new([4e5, 0.0, 0.0]);
        let motion = CatalogueMotion::broken(site, birth, GalacticVelocity::default(), kicked);
        MagnetarHost::new(
            SystemId::from(id),
            birth,
            Gauss::new(field),
            motion,
            recipe(field, age),
        )
        .ok()
    }

    fn members(&self, _: &MagnetarHost) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<MagnetarHost>) -> CatalogueSystem {
        let host = member.entry().value();
        let record = SystemRecord::from_parts(
            SystemId::from(member.id()),
            host.motion().position_at(UniverseTime::EPOCH),
            SystemOrigin::CatalogueSystem(ClassId::CORE_COLLAPSE),
            Population::YoungThinDisc,
            SolarMasses::new(15.0),
            Years::new(2e7),
        );
        CatalogueSystem::new(record, *host.motion())
    }
}

impl MagnetarClass for ToyMagnetars {
    fn magnetar_host(&self, _: &Galaxy, entry: &MagnetarHost) -> Option<MagnetarHost> {
        Some(entry.clone())
    }
}

/// The 2,000 ly sphere's X-ray sensor, resolving at ten times its limit.
fn xray() -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(LIMIT), w(10.0 * LIMIT)).unwrap();
    SensorHorizon::new([(AlertBand::XRay, limits)]).unwrap()
}

/// The window of `years` Julian years from `offset` years after `at`.
fn window(at: f64, offset: f64, years: f64) -> TimeWindow {
    TimeWindow::new(at_years(at + offset), at_years(at + offset + years)).unwrap()
}

/// The walk's detections and the brute force's, as the oracle sees them, sorted; and the walk's
/// cell by cell through `scan_cell`'s generic form, in reverse order and each with a fresh context
/// and no cache, merged, which must be the walk's. The brute force generates its hosts in a cache
/// of its own, `oracle_cells`.
fn walked_and_brute<P: MagnetarClass>(
    galaxy: &Galaxy,
    noise: &mut NoiseCache,
    cells: &mut KeepMagnetarCells,
    oracle_cells: &mut KeepMagnetarCells,
    process: &P,
    query: &AlertQuery,
) -> (Vec<Seen>, Vec<Seen>) {
    let mut out = Vec::new();
    {
        let mut context = AlertContext::new(noise, &NoModifiers).with_magnetar_cells(cells);
        magnetar_detections(galaxy, &mut context, query, process, &mut out);
    }
    assert!(
        out.windows(2)
            .all(|pair| (pair[0].arrival(), pair[0].event()) < (pair[1].arrival(), pair[1].event())),
        "by arrival, then by event"
    );
    let mut by_cell = Vec::new();
    for cell in magnetar_scan_plan(process.class(), query).into_iter().rev() {
        let mut fresh = NoiseCache::with_capacity(64);
        let mut context = AlertContext::new(&mut fresh, &NoModifiers);
        magnetar_scan_cell(galaxy, &mut context, query, process, cell, &mut by_cell);
    }
    let as_seen = |detections: &[_]| {
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
    let oracle =
        brute_force_magnetar_detections(galaxy, &mut context, oracle_cells, process, query);
    (walk, oracle)
}

/// The toy's magnetars over `windows` of each target: the walk is the brute force's, and the
/// detections are the magnetars'. Returns each target's count.
fn toy_magnetars_are_the_brute_forces(windows: &[(f64, f64)]) -> [usize; 3] {
    let galaxy = Galaxy::new(Seed::new(0x0125_0c00));
    let process = ToyMagnetars::new();
    let mut noise = NoiseCache::with_capacity(4_096);
    let (mut cells, mut oracle_cells) =
        (KeepMagnetarCells::default(), KeepMagnetarCells::default());
    let mut counts = [0; 3];
    for (count, target) in counts.iter_mut().zip(TARGETS) {
        for &(offset, years) in windows {
            let window = window(target.years, offset, years);
            let query = AlertQuery::builder(observer_at(target.position), window, xray())
                .build()
                .unwrap();
            let (walk, oracle) = walked_and_brute(
                &galaxy,
                &mut noise,
                &mut cells,
                &mut oracle_cells,
                &process,
                &query,
            );
            assert_eq!(walk, oracle, "{target:?} over {window:?}");
            for seen in &walk {
                assert_eq!(seen.class, Some(ClassId::CORE_COLLAPSE));
                assert_eq!(seen.kind, TransientKind::MagnetarBurst);
                assert_eq!(seen.band, AlertBand::XRay);
                assert!(window.contains(seen.arrival) || seen.phase == DetectionPhase::InProgress);
            }
            *count += walk.len();
            eprintln!("{} detections for {target:?} over {window:?}", walk.len());
        }
    }
    counts
}

/// On the 2,000 ly sphere, for the toy's three observers over a year about each one's time, the
/// magnetar walk is the brute force's and `scan_cell` over the plan's cells is the walk; the year
/// holds hundreds of the near magnetars' short bursts.
#[test]
fn magnetar_alerts_of_toy_magnetars_are_the_brute_forces_over_a_year() {
    let [near, _, _] = toy_magnetars_are_the_brute_forces(&[(-0.5, 1.0)]);
    assert!(near >= 100, "{near} detections near");
}

/// Each magnetar detection's light (P12.T8.b.1) is a top hat in X-rays: zero before its onset
/// arrives and from its end's, and the walk's flux at the detection's own time, its arrival for an
/// onset and the window's start for an event in progress, which a window opening half-way through
/// an onset's light makes one.
#[test]
fn magnetar_alerts_lights_are_the_walks_flux_at_their_own_times() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0c00));
    let process = ToyMagnetars::new();
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut cells = KeepMagnetarCells::default();
    let near = &TARGETS[0];
    let mut walk = |window: TimeWindow| {
        let query = AlertQuery::builder(observer_at(near.position), window, xray())
            .build()
            .unwrap();
        let mut context =
            AlertContext::new(&mut noise, &NoModifiers).with_magnetar_cells(&mut cells);
        let mut out = Vec::new();
        magnetar_detections(&galaxy, &mut context, &query, &process, &mut out);
        out
    };
    let lit = |d: &Detection| match d.light() {
        DetectionLight::Lit(light) => *light,
        DetectionLight::Unlit => panic!("a magnetar's outburst gives light: {d:?}"),
    };
    let nanosecond = Span::new(0, 1).unwrap();
    let year = window(near.years, -0.5, 1.0);
    let onsets = walk(year);
    assert!(onsets.len() >= 100, "{} detections", onsets.len());
    for d in &onsets {
        let light = lit(d);
        assert_eq!(light.band(), AlertBand::XRay);
        let knots = light.knots();
        assert_eq!(knots.len(), 2, "{d:?}");
        assert_eq!(knots[0], d.arrival());
        let at = |t: UniverseTime| light.flux_at(t).value();
        assert!(at(knots[0].checked_sub(nanosecond).unwrap()).abs() < f64::MIN_POSITIVE);
        assert!(at(knots[1]).abs() < f64::MIN_POSITIVE, "gone at its end");
        let own = match d.phase() {
            DetectionPhase::Onset => d.arrival(),
            DetectionPhase::InProgress => year.start(),
        };
        assert_same_bits(at(own), d.flux().value());
        assert_same_bits(
            at(knots[1].checked_sub(nanosecond).unwrap()),
            d.flux().value(),
        );
    }
    // The longest of the year's onsets, seen from a window that opens half-way through its light.
    let longest = onsets
        .iter()
        .filter(|d| d.phase() == DetectionPhase::Onset)
        .max_by_key(|d| lit(d).knots()[1].checked_since(d.arrival()).unwrap())
        .unwrap();
    let knots = lit(longest).knots();
    let half =
        Span::from_seconds_f64(knots[1].checked_since(knots[0]).unwrap().as_seconds_f64() / 2.0)
            .unwrap();
    let opening = knots[0].checked_add(half).unwrap();
    let later = TimeWindow::new(
        opening,
        opening.checked_add(Span::from_seconds(3_600)).unwrap(),
    )
    .unwrap();
    let in_progress = walk(later)
        .into_iter()
        .find(|d| d.event() == longest.event())
        .expect("the onset is in progress half-way through its light");
    assert_eq!(in_progress.phase(), DetectionPhase::InProgress);
    assert_eq!(in_progress.light(), longest.light(), "the same light");
    assert_same_bits(
        lit(&in_progress).flux_at(opening).value(),
        in_progress.flux().value(),
    );
}

/// As over a year, over a century about each observer's time, which holds giant flares from some
/// hundreds of light-years for the two far observers.
#[test]
#[ignore = "slow: lists a century of short bursts and giant flares of fifteen strong magnetars"]
fn magnetar_alerts_of_toy_magnetars_are_the_brute_forces_over_a_century() {
    let counts = toy_magnetars_are_the_brute_forces(&[(-50.0, 100.0)]);
    assert!(counts.iter().all(|&n| n > 0), "{counts:?}");
}

/// The walk gives each of the toy's six queries, its observers' years and the near observer's
/// windows a quarter of a year apart, the same detections whatever it walked before with the same
/// cell and noise caches, forwards, backwards, shuffled and alone; and with no cache.
#[test]
fn magnetar_alerts_do_not_depend_on_what_was_walked_before() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0c00));
    let process = ToyMagnetars::new();
    let mut queries: Vec<_> = TARGETS
        .iter()
        .map(|target| (target.position, window(target.years, -0.5, 1.0)))
        .collect();
    queries
        .extend([0.25, 0.5, 0.75].map(|offset| (TARGETS[0].position, window(0.0, offset, 0.25))));
    let queries: Vec<_> = queries
        .into_iter()
        .map(|(position, window)| {
            AlertQuery::builder(observer_at(position), window, xray())
                .build()
                .unwrap()
        })
        .collect();
    let walk =
        |query: &AlertQuery, noise: &mut NoiseCache, cells: Option<&mut KeepMagnetarCells>| {
            let mut out = Vec::new();
            let mut nothing = NoMagnetarCells;
            let cells: &mut dyn MagnetarCells = match cells {
                Some(cells) => cells,
                None => &mut nothing,
            };
            let mut context = AlertContext::new(noise, &NoModifiers).with_magnetar_cells(cells);
            magnetar_detections(&galaxy, &mut context, query, &process, &mut out);
            out.iter().map(Seen::of).collect::<Vec<_>>()
        };
    let cells = RefCell::new(KeepMagnetarCells::default());
    let noise = RefCell::new(NoiseCache::with_capacity(4_096));
    assert_order_independent(&queries, |query| {
        walk(
            query,
            &mut noise.borrow_mut(),
            Some(&mut cells.borrow_mut()),
        )
    });
    let mut cold = NoiseCache::with_capacity(64);
    assert_eq!(
        walk(&queries[0], &mut cold, None),
        walk(
            &queries[0],
            &mut noise.borrow_mut(),
            Some(&mut cells.borrow_mut())
        )
    );
}

/// Pins the near observer's year of the toy's magnetars: each detection's event, arrival, phase
/// and flux, so that a change to plan 06's events, plan 07's lines or the shares shows, which the
/// brute force, built on the same, cannot.
#[test]
fn magnetar_alerts_golden() {
    let galaxy = Galaxy::new(Seed::new(0x0125_0c00));
    let process = ToyMagnetars::new();
    let target = TARGETS[0];
    let query = AlertQuery::builder(observer_at(target.position), window(0.0, -0.5, 1.0), xray())
        .build()
        .unwrap();
    let mut noise = NoiseCache::with_capacity(4_096);
    let mut context = AlertContext::new(&mut noise, &NoModifiers);
    let mut out = Vec::new();
    magnetar_detections(&galaxy, &mut context, &query, &process, &mut out);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line(&format!("{} detections", out.len()));
    for detection in &out {
        w.f64(
            &format!(
                "{} arriving {} {:?}",
                detection.event(),
                detection.arrival(),
                detection.phase()
            ),
            detection.flux().value(),
        );
    }
    golden!("alerts/recurrent", w.as_str());
}

/// The production core collapses of the galaxy of seed 9, on the 2,000 ly sphere, over a year and a
/// century: about two points of the solar circle, and a tenth of a light-year from the remnant of
/// the strongest magnetar active at the epoch among the class's entries within some 10,000 ly of
/// the Sun in the plane. The walk is the brute force's, `scan_cell`, the production dispatch by
/// class, is the generic walk's, and the near observer sees the magnetar's events.
#[test]
#[ignore = "slow: generates the core-collapse class's cells within 6,100 ly of three observers"]
fn magnetar_alerts_of_the_core_collapses_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(9));
    let process = CoreCollapseProcess::new(&galaxy);
    let mut noise = NoiseCache::with_capacity(4_096);
    let (mut cells, mut oracle_cells) =
        (KeepMagnetarCells::default(), KeepMagnetarCells::default());
    // The magnetars active at the epoch within some 10,000 ly of the Sun in the plane, by their
    // field then.
    let mut active = Vec::new();
    for y in 30..70 {
        for x in -20..20 {
            for z in [-1, 0] {
                let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [x, y, z]).unwrap();
                for host in cells.hosts(&galaxy, &process, key) {
                    let star = host.star().to_star();
                    let model = hyperion_sim::stellar::events::magnetar::MagnetarModel::of(&star)
                        .expect("a host leaves a magnetar");
                    if let Some(pulsar) = model.pulsar_at(UniverseTime::EPOCH)
                        && pulsar.is_magnetar()
                    {
                        active.push((pulsar.field().value(), host.clone()));
                    }
                }
            }
        }
    }
    active.sort_by(|a, b| a.0.total_cmp(&b.0));
    eprintln!(
        "{} magnetars active at the epoch: {:?}",
        active.len(),
        active.iter().map(|a| a.0).collect::<Vec<_>>()
    );
    let (_, strongest) = active
        .last()
        .expect("an active magnetar by the solar circle");
    let remnant = strongest
        .motion()
        .position_at(UniverseTime::EPOCH)
        .to_light_years_f64();
    let near = [remnant[0] + 0.1, remnant[1], remnant[2]];
    let mut near_seen = 0;
    for (position, is_near) in [
        ([0.0, 26_000.0, 20.0], false),
        ([2_000.0, 24_500.0, -30.0], false),
        (near, true),
    ] {
        for (at, years) in [(0.0, 1.0), (-50.0, 100.0)] {
            let window = window(at, 0.0, years);
            let query = AlertQuery::builder(observer_at(position), window, xray())
                .build()
                .unwrap();
            let (walk, oracle) = walked_and_brute(
                &galaxy,
                &mut noise,
                &mut cells,
                &mut oracle_cells,
                &process,
                &query,
            );
            assert_eq!(walk, oracle, "{position:?} over {window:?}");
            let mut dispatched = Vec::new();
            let mut context = AlertContext::new(&mut noise, &NoModifiers);
            for cell in scan_plan(&galaxy, ClassId::CORE_COLLAPSE, &query) {
                scan_cell(
                    &galaxy,
                    &mut context,
                    ClassId::CORE_COLLAPSE,
                    cell,
                    &query,
                    &mut dispatched,
                );
            }
            let mut seen: Vec<Seen> = dispatched.iter().map(Seen::of).collect();
            seen.sort();
            assert_eq!(seen, walk, "{position:?} over {window:?}");
            eprintln!(
                "{} detections from {position:?} over {window:?}",
                walk.len()
            );
            if is_near {
                near_seen += walk.iter().filter(|s| s.host == strongest.host()).count();
            }
        }
    }
    assert!(near_seen > 0, "the near observer sees its magnetar");
}
