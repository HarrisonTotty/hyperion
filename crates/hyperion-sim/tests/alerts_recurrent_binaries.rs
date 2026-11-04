//! The recurrent walk's binary classes against the brute force (plan 12, P12.T5.c): the accreting
//! white dwarfs' novae, fast and slow, and the X-ray binaries' transients, bursts and Be/X
//! outbursts, on a 2,000 ly sphere (a sensor whose reach for each class's brightest event in its
//! band is 2,000 ly), for observers about the Sun and beside hosts whose events their windows
//! hold, over a year, a few days and (slow) a century. The walk's detections are the brute
//! force's as sets, field by field and bit by bit (`common/observe.rs`); `scan_cell` over every
//! cell of the plan, in reverse with fresh contexts, gives what one call over the sphere does; and
//! neither runs plan 11's binary engine (`scan::engine_calls`).
//!
//! The production classes of the galaxy of seed 9 are walked, read through plan 11's scan marks
//! alone; the brute force lists every host's events over an interval wide enough by its speed, so
//! it passes over no slow host by the eruptions its marks carry. A golden pins what the observers
//! beside the hosts see, which the brute force, built on plan 11's same curves, cannot show moving.

#![cfg(feature = "testing")]

#[expect(dead_code, reason = "the recurrent alerts use only the alerts' oracle")]
mod common;

use std::f64::consts::PI;

use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, BandLimits, Detection, DetectionLight, DetectionPhase,
    NOVA_BAND_PEAKS, SensorHorizon, TransientKind, XRAY_TRANSIENT_BAND_PEAKS, kind_reach,
    recurrent_detections, scan_cell, scan_plan,
};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::binary::{AwdProcess, AwdSide, XrayBinaries};
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::id::EventId;
use hyperion_sim::stellar::binary::events::{BinaryEventKind, BinarySystem, events_in};
use hyperion_sim::stellar::binary::scan::{engine_calls, scan_cell as scan_marks_cell};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::WattsPerSquareMetre;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

use common::observe::{Seen, brute_force_binary_detections, observer_at};

/// The galaxy's seed.
const SEED: u64 = 9;

/// The sphere's radius, light-years.
const SPHERE_LY: f64 = 2_000.0;

/// The detection limit, W m⁻², at which `bound` reaches [`SPHERE_LY`].
fn limit_reaching(bound: f64) -> f64 {
    let d = SPHERE_LY * METRES_PER_LIGHT_YEAR;
    bound / (4.0 * PI * d * d)
}

/// A sensor of one band whose limit puts `bound` at [`SPHERE_LY`], resolving at ten times it.
fn sensor(band: AlertBand, bound: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limit = limit_reaching(bound);
    let limits = BandLimits::new(w(limit), w(10.0 * limit)).unwrap();
    SensorHorizon::new([(band, limits)]).unwrap()
}

/// The visual sensor for the novae: their bound in V reaches 2,000 ly.
fn visual() -> SensorHorizon {
    let v = AlertBand::Photometric(Band::V);
    sensor(v, NOVA_BAND_PEAKS[2].value())
}

/// The X-ray sensor for the X-ray binaries: the transients' bound in 2–10 keV reaches 2,000 ly.
fn xray() -> SensorHorizon {
    sensor(AlertBand::XRay, XRAY_TRANSIENT_BAND_PEAKS[10].value())
}

/// The time `seconds` after `t`, or before it for a negative number.
fn shifted(t: UniverseTime, seconds: f64) -> UniverseTime {
    let span = Span::from_seconds_f64(seconds.abs()).unwrap();
    if seconds < 0.0 {
        t.checked_sub(span).unwrap()
    } else {
        t.checked_add(span).unwrap()
    }
}

/// Seconds in a Julian year.
const YEAR: f64 = 31_557_600.0;

/// The window of `length` seconds from `start`.
fn window(start: UniverseTime, length: f64) -> TimeWindow {
    TimeWindow::new(start, shifted(start, length)).unwrap()
}

/// The walk's detections for `query` over `class`, checked sorted; `scan_cell` over the plan's
/// cells in reverse, each with a fresh context, which must be them; and the brute force's. Neither
/// runs the binary engine.
fn walked_and_brute(
    galaxy: &Galaxy,
    noise: &mut NoiseCache,
    class: ClassId,
    query: &AlertQuery,
) -> (Vec<Detection>, Vec<Seen>, Vec<Seen>) {
    let before = engine_calls();
    let mut out = Vec::new();
    {
        let mut context = AlertContext::new(noise, &NoModifiers);
        recurrent_detections(galaxy, &mut context, class, query, &mut out);
    }
    assert!(
        out.windows(2)
            .all(|pair| (pair[0].arrival(), pair[0].event()) < (pair[1].arrival(), pair[1].event())),
        "by arrival, then by event"
    );
    let mut by_cell = Vec::new();
    for cell in scan_plan(galaxy, class, query).into_iter().rev() {
        let mut fresh = NoiseCache::with_capacity(64);
        let mut context = AlertContext::new(&mut fresh, &NoModifiers);
        scan_cell(galaxy, &mut context, class, cell, query, &mut by_cell);
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
    let oracle = brute_force_binary_detections(galaxy, &mut context, class, query);
    assert_eq!(engine_calls(), before, "no binary engine runs");
    (out, walk, oracle)
}

/// An observer beside a host of `class` and a window that sees one of its events: where it stands,
/// the event, and its arrival there.
#[derive(Debug, Clone, Copy)]
struct Target {
    position: [f64; 3],
    event: EventId,
    arrival: UniverseTime,
}

/// The first host of `class` among the cells `cells` whose events of `kind` include one with an
/// onset in `span` (asked of `listing`'s interval for each host), and an observer `away` ly above
/// the plane from where the host is at that onset.
fn target(
    galaxy: &Galaxy,
    class: ClassId,
    cells: impl IntoIterator<Item = [i32; 3]>,
    kind: BinaryEventKind,
    span: TimeWindow,
    away: f64,
) -> Target {
    let mut events = Vec::new();
    for cell in cells {
        let key = CatalogueCellKey::new(class, cell).unwrap();
        let found = if class == ClassId::XRAY_BINARY {
            let mut hosts = Vec::new();
            scan_marks_cell(galaxy, &XrayBinaries::new(galaxy), key, &mut hosts);
            hosts.iter().find_map(|host| {
                events.clear();
                events_in(
                    &BinarySystem::xray_binary(galaxy.seed(), host),
                    span,
                    &mut events,
                );
                let event = events.iter().find(|e| e.kind() == kind)?;
                Some((*event, host.position_at(event.onset())))
            })
        } else {
            let side = if class == ClassId::ACCRETING_WHITE_DWARF {
                AwdSide::Fast
            } else {
                AwdSide::Slow
            };
            let mut hosts = Vec::new();
            scan_marks_cell(galaxy, &AwdProcess::new(galaxy, side), key, &mut hosts);
            hosts.iter().find_map(|host| {
                events.clear();
                let system = BinarySystem::accreting_white_dwarf_scan(galaxy.seed(), host);
                events_in(&system, span, &mut events);
                let event = events.iter().find(|e| e.kind() == kind)?;
                Some((*event, host.position_at(event.onset())))
            })
        };
        if let Some((event, site)) = found {
            let [x, y, z] = site.to_light_years_f64();
            let position = [x, y, z + away];
            return Target {
                position,
                event: event.id(),
                arrival: shifted(event.onset(), away * YEAR),
            };
        }
    }
    panic!("no host of {class} has a {kind:?} in {span:?}")
}

/// The span of `years` Julian years either side of the epoch, inside the clock window.
fn about_the_epoch(years: f64) -> TimeWindow {
    TimeWindow::new(
        shifted(UniverseTime::EPOCH, -years * YEAR),
        shifted(UniverseTime::EPOCH, years * YEAR),
    )
    .unwrap()
}

/// The cells of a band of the solar circle, `xs` along it and `zs` off the plane, at y = 50.
fn solar_circle(xs: std::ops::Range<i32>, zs: std::ops::Range<i32>) -> Vec<[i32; 3]> {
    xs.flat_map(|x| zs.clone().map(move |z| [x, 50, z]))
        .collect()
}

/// The cells about the centre, where the X-ray binaries crowd.
fn about_the_centre() -> Vec<[i32; 3]> {
    (-12..12)
        .flat_map(|y| (-12..12).flat_map(move |x| [-1, 0].map(move |z| [x, y, z])))
        .collect()
}

/// The cells of the disc within 51,200 ly of the centre along x and y, in and just below the
/// plane, x first: from 27 the refit gives the galaxy's catalogue about four Be/X binaries
/// (P15.T10.b), and seed 9's five lie in the disc, none about the centre.
fn across_the_disc() -> Vec<[i32; 3]> {
    (-100..100)
        .flat_map(|x| (-100..100).flat_map(move |y| [-1, 0].map(move |z| [x, y, z])))
        .collect()
}

/// `query`'s walk over `class` is the brute force's; returns the walk's detections.
fn assert_walk_is_the_brute_force(
    galaxy: &Galaxy,
    noise: &mut NoiseCache,
    class: ClassId,
    query: &AlertQuery,
) -> Vec<Detection> {
    let (out, walk, oracle) = walked_and_brute(galaxy, noise, class, query);
    assert_eq!(
        walk,
        oracle,
        "{class} from {:?} over {:?}",
        query.observer_position(),
        query.window()
    );
    for seen in &walk {
        assert_eq!(seen.class, Some(class));
        assert!(query.window().contains(seen.arrival) || seen.phase == DetectionPhase::InProgress);
    }
    eprintln!(
        "{} detections of {class} over {:?}",
        walk.len(),
        query.window()
    );
    out
}

/// The query from `position` over `window` for `sensor`.
fn query(position: [f64; 3], window: TimeWindow, sensor: SensorHorizon) -> AlertQuery {
    AlertQuery::builder(observer_at(position), window, sensor)
        .build()
        .unwrap()
}

/// The fast and slow accreting white dwarfs' novae on the 2,000 ly sphere: about the Sun over a
/// year, and beside a host of each class over a year that holds one of its novae's arrival and
/// over a day two days later, when the nova is in progress. The walk is the brute force's, and
/// sees each target's nova begin and then in progress.
#[test]
fn nova_alerts_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let mut noise = NoiseCache::with_capacity(4_096);
    for class in [
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
    ] {
        let sun = query(
            [0.0, 26_000.0, 20.0],
            window(UniverseTime::EPOCH, YEAR),
            visual(),
        );
        assert_walk_is_the_brute_force(&galaxy, &mut noise, class, &sun);
        let target = target(
            &galaxy,
            class,
            solar_circle(-40..40, -1..1),
            BinaryEventKind::Nova,
            about_the_epoch(900.0),
            30.0,
        );
        let year = query(
            target.position,
            window(shifted(target.arrival, -0.5 * YEAR), YEAR),
            visual(),
        );
        let seen = assert_walk_is_the_brute_force(&galaxy, &mut noise, class, &year);
        let onset = seen
            .iter()
            .find(|d| d.event() == target.event)
            .expect("its nova");
        assert_eq!(onset.phase(), DetectionPhase::Onset);
        assert_eq!(onset.kind(), TransientKind::Nova);
        let later = query(
            target.position,
            window(shifted(target.arrival, 2.0 * 86_400.0), 86_400.0),
            visual(),
        );
        let seen = assert_walk_is_the_brute_force(&galaxy, &mut noise, class, &later);
        let running = seen
            .iter()
            .find(|d| d.event() == target.event)
            .expect("its nova");
        assert_eq!(running.phase(), DetectionPhase::InProgress);
        assert_eq!(running.emitted(), onset.emitted());
    }
}

/// The X-ray binaries' events on the 2,000 ly sphere, about the centre where they crowd: beside a
/// burster over ten days, beside a transient over a year that holds an outburst's arrival and
/// over a day a month later, and beside a Be/X binary of the disc ([`across_the_disc`]) over a
/// year that holds a giant outburst's arrival. The walk is the brute force's, and each observer
/// sees its host's events.
#[test]
fn x_ray_binary_alerts_are_the_brute_forces() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let mut noise = NoiseCache::with_capacity(4_096);
    let class = ClassId::XRAY_BINARY;
    let day = 86_400.0;
    let burster = target(
        &galaxy,
        class,
        about_the_centre(),
        BinaryEventKind::XrayBurst,
        window(UniverseTime::EPOCH, day),
        1.0,
    );
    let bursts = query(
        burster.position,
        window(burster.arrival, 10.0 * day),
        xray(),
    );
    let seen = assert_walk_is_the_brute_force(&galaxy, &mut noise, class, &bursts);
    assert!(
        seen.iter()
            .any(|d| d.event() == burster.event && d.kind() == TransientKind::XrayBurst),
        "the burster's bursts"
    );
    let transient = target(
        &galaxy,
        class,
        about_the_centre(),
        BinaryEventKind::XrayTransient,
        about_the_epoch(900.0),
        50.0,
    );
    let outburst = query(
        transient.position,
        window(shifted(transient.arrival, -0.5 * YEAR), YEAR),
        xray(),
    );
    let seen = assert_walk_is_the_brute_force(&galaxy, &mut noise, class, &outburst);
    assert!(
        seen.iter().any(|d| d.event() == transient.event),
        "its outburst"
    );
    let month_on = query(
        transient.position,
        window(shifted(transient.arrival, 30.0 * day), day),
        xray(),
    );
    let seen = assert_walk_is_the_brute_force(&galaxy, &mut noise, class, &month_on);
    assert!(
        seen.iter()
            .any(|d| d.event() == transient.event && d.phase() == DetectionPhase::InProgress),
        "its outburst in progress"
    );
    let bex = target(
        &galaxy,
        class,
        across_the_disc(),
        BinaryEventKind::BeXGiantOutburst,
        window(UniverseTime::EPOCH, 100.0 * YEAR),
        10.0,
    );
    let year = query(
        bex.position,
        window(shifted(bex.arrival, -0.5 * YEAR), YEAR),
        xray(),
    );
    let seen = assert_walk_is_the_brute_force(&galaxy, &mut noise, class, &year);
    assert!(
        seen.iter().any(|d| d.event() == bex.event),
        "its giant outburst"
    );
}

/// As over a year, over a century about the Sun and beside the fast novae's target.
#[test]
#[ignore = "slow: lists a century of the novae of the accreting white dwarfs within 6,100 ly"]
fn nova_alerts_are_the_brute_forces_over_a_century() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let mut noise = NoiseCache::with_capacity(4_096);
    for class in [
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
    ] {
        let target = target(
            &galaxy,
            class,
            solar_circle(-40..40, -1..1),
            BinaryEventKind::Nova,
            about_the_epoch(900.0),
            30.0,
        );
        for position in [[0.0, 26_000.0, 20.0], target.position] {
            let century = query(
                position,
                window(shifted(target.arrival, -50.0 * YEAR), 100.0 * YEAR),
                visual(),
            );
            assert_walk_is_the_brute_force(&galaxy, &mut noise, class, &century);
        }
    }
}

/// A window split in two finds the whole's onsets, each once, in the part its light arrives in,
/// and its first part the whole's events in progress (Design note 6): beside the burster, over a
/// day split at its middle.
#[test]
fn a_split_window_finds_each_onset_once() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let mut noise = NoiseCache::with_capacity(4_096);
    let class = ClassId::XRAY_BINARY;
    let day = 86_400.0;
    let burster = target(
        &galaxy,
        class,
        about_the_centre(),
        BinaryEventKind::XrayBurst,
        window(UniverseTime::EPOCH, day),
        1.0,
    );
    let mut walk = |window: TimeWindow| {
        let mut context = AlertContext::new(&mut noise, &NoModifiers);
        let mut out = Vec::new();
        let q = query(burster.position, window, xray());
        recurrent_detections(&galaxy, &mut context, class, &q, &mut out);
        out
    };
    let start = shifted(burster.arrival, -0.25 * day);
    let whole = walk(window(start, day));
    let first = walk(window(start, 0.5 * day));
    let second = walk(window(shifted(start, 0.5 * day), 0.5 * day));
    let onsets = |detections: &[Detection]| {
        detections
            .iter()
            .filter(|d| d.phase() == DetectionPhase::Onset)
            .map(Seen::of)
            .collect::<Vec<_>>()
    };
    let in_progress = |detections: &[Detection]| {
        detections
            .iter()
            .filter(|d| d.phase() == DetectionPhase::InProgress)
            .map(Seen::of)
            .collect::<Vec<_>>()
    };
    let mut parts = onsets(&first);
    parts.extend(onsets(&second));
    assert!(!parts.is_empty(), "the burster bursts in a day");
    assert_eq!(parts, onsets(&whole));
    assert_eq!(in_progress(&first), in_progress(&whole));
}

/// Each binary detection's light (P12.T8.b.1) is its event's curve at the observer: zero before
/// its onset arrives and from the end of its event, the walk's flux at the detection's own time,
/// its peak's arrival for an onset and the window's start for one in progress.
#[test]
fn binary_alerts_lights_are_the_walks_flux_at_their_own_times() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let mut noise = NoiseCache::with_capacity(4_096);
    let day = 86_400.0;
    let transient = target(
        &galaxy,
        ClassId::XRAY_BINARY,
        about_the_centre(),
        BinaryEventKind::XrayTransient,
        about_the_epoch(900.0),
        50.0,
    );
    let fast = target(
        &galaxy,
        ClassId::ACCRETING_WHITE_DWARF,
        solar_circle(-40..40, -1..1),
        BinaryEventKind::Nova,
        about_the_epoch(900.0),
        30.0,
    );
    let nanosecond = Span::new(0, 1).unwrap();
    let mut seen = 0;
    for (class, target, sensor) in [
        (ClassId::XRAY_BINARY, transient, xray()),
        (ClassId::ACCRETING_WHITE_DWARF, fast, visual()),
    ] {
        for start in [
            shifted(target.arrival, -day),
            shifted(target.arrival, 3.0 * day),
        ] {
            let q = query(target.position, window(start, 2.0 * day), sensor.clone());
            let mut context = AlertContext::new(&mut noise, &NoModifiers);
            let mut out = Vec::new();
            recurrent_detections(&galaxy, &mut context, class, &q, &mut out);
            let d = out
                .iter()
                .find(|d| d.event() == target.event)
                .expect("the target's event");
            let DetectionLight::Lit(light) = d.light() else {
                panic!("a binary event gives light: {d:?}");
            };
            assert_eq!(light.band(), d.band());
            let knots = light.knots();
            assert_eq!(knots.len(), 3, "{d:?}");
            assert_eq!(knots[0], d.arrival());
            let at = |t: UniverseTime| light.flux_at(t).value();
            assert_same_bits(at(knots[0].checked_sub(nanosecond).unwrap()), 0.0);
            assert_same_bits(at(knots[2]), 0.0);
            assert!(at(knots[2].checked_sub(nanosecond).unwrap()) > 0.0);
            let own = match d.phase() {
                DetectionPhase::Onset => knots[1],
                DetectionPhase::InProgress => q.window().start(),
            };
            assert_same_bits(at(own), d.flux().value());
            seen += 1;
        }
    }
    assert_eq!(seen, 4);
}

/// The walk gives each of five queries beside the burster, its ten days, a day's two halves and two
/// days a week on, the same detections whatever it walked before with the same noise cache,
/// forwards, backwards, shuffled and alone.
#[test]
fn binary_alerts_do_not_depend_on_what_was_walked_before() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let day = 86_400.0;
    let burster = target(
        &galaxy,
        ClassId::XRAY_BINARY,
        about_the_centre(),
        BinaryEventKind::XrayBurst,
        window(UniverseTime::EPOCH, day),
        1.0,
    );
    let queries: Vec<_> = [
        (0.0, 10.0 * day),
        (0.0, 0.5 * day),
        (0.5 * day, 0.5 * day),
        (7.0 * day, 2.0 * day),
        (-day, day),
    ]
    .map(|(from, length)| {
        query(
            burster.position,
            window(shifted(burster.arrival, from), length),
            xray(),
        )
    })
    .into_iter()
    .collect();
    let noise = std::cell::RefCell::new(NoiseCache::with_capacity(4_096));
    assert_order_independent(&queries, |q| {
        let mut noise = noise.borrow_mut();
        let mut context = AlertContext::new(&mut noise, &NoModifiers);
        let mut out = Vec::new();
        recurrent_detections(&galaxy, &mut context, ClassId::XRAY_BINARY, q, &mut out);
        out.iter().map(Seen::of).collect::<Vec<_>>()
    });
}

/// The reach of the sensors is the sphere's, to a part in 10¹²: the novae's in V, the X-ray
/// binaries' transients' in 2–10 keV.
#[test]
fn the_sensors_reach_the_sphere() {
    for (kind, sensor) in [
        (TransientKind::Nova, visual()),
        (TransientKind::XrayTransient, xray()),
    ] {
        let reach = kind_reach(kind, &sensor).value();
        assert!((reach / SPHERE_LY - 1.0).abs() < 1e-12, "{kind:?}: {reach}");
    }
}

/// Pins what the observers beside the hosts see: each detection's event, arrival, phase, band and
/// flux, so that a change to plan 11's curves or clocks, plan 07's lines or the walk shows, which
/// the brute force, built on the same, cannot.
#[test]
fn binary_alerts_golden() {
    let galaxy = Galaxy::new(Seed::new(SEED));
    let mut noise = NoiseCache::with_capacity(4_096);
    let day = 86_400.0;
    let fast = target(
        &galaxy,
        ClassId::ACCRETING_WHITE_DWARF,
        solar_circle(-40..40, -1..1),
        BinaryEventKind::Nova,
        about_the_epoch(900.0),
        30.0,
    );
    let burster = target(
        &galaxy,
        ClassId::XRAY_BINARY,
        about_the_centre(),
        BinaryEventKind::XrayBurst,
        window(UniverseTime::EPOCH, day),
        1.0,
    );
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (name, class, q) in [
        (
            "fast nova",
            ClassId::ACCRETING_WHITE_DWARF,
            query(
                fast.position,
                window(shifted(fast.arrival, -0.5 * YEAR), YEAR),
                visual(),
            ),
        ),
        (
            "burster",
            ClassId::XRAY_BINARY,
            query(burster.position, window(burster.arrival, day), xray()),
        ),
    ] {
        let mut context = AlertContext::new(&mut noise, &NoModifiers);
        let mut out = Vec::new();
        recurrent_detections(&galaxy, &mut context, class, &q, &mut out);
        w.line(&format!("{name}: {} detections", out.len()));
        for d in &out {
            w.f64(
                &format!(
                    "{} {:?} arriving {} {:?} in {:?}",
                    d.event(),
                    d.kind(),
                    d.arrival(),
                    d.phase(),
                    d.band()
                ),
                d.flux().value(),
            );
        }
    }
    golden!("alerts/recurrent_binaries", w.as_str());
}

/// The Milky Way fixture's seed, the galaxy plan 11's nova rate was measured in (P11.T9.b).
const MILKY_WAY_SEED: u64 = 0x0b17_09b0_0000_0002;

/// The V band's light of a star of V = 0, W m⁻²: Vega's 3.631 × 10⁻⁸ W m⁻² µm⁻¹ at V (Bessell,
/// Castelli and Plez 1998, A&A 333, 231, Table A2) through the band's 0.084 µm top hat (plan 12's
/// `alerts::light`).
const V_ZERO_POINT: f64 = 3.631e-8 * 0.084;

/// The galaxy's novae a year that a visual sensor at the Sun sees begin, to V = 12, over a decade
/// from the epoch, through plan 07's dust: every cell of both accreting white dwarf classes the
/// scan plans, the whole cube, walked on eight threads. Plan 11 generates 46.0 a year in this
/// galaxy (P11.T9.b), and 9.8 a year are seen; about ten a year are found in the Milky Way, most
/// of them brighter than twelfth magnitude, against 26–50 a year in all (Shafter 2017, ApJ 834,
/// 196; De et al. 2021, ApJ 912, 19; Kawash et al. 2022, ApJ 937, 64; recalled, not read at
/// source). Pinned as measured.
#[test]
#[ignore = "slow: walks every accreting white dwarf cell of the cube for a decade"]
// Its eight threads: wasip1 spawns none.
#[cfg(not(target_family = "wasm"))]
fn the_galaxys_novae_seen_from_the_sun_a_year() {
    let galaxy = Galaxy::from_params(
        Seed::new(MILKY_WAY_SEED),
        hyperion_sim::galaxy::params::GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let w = WattsPerSquareMetre::new;
    let limit = V_ZERO_POINT * hyperion_sim::math::exp10(-0.4 * 12.0);
    let limits = BandLimits::new(w(limit), w(limit)).unwrap();
    let sensor = SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap();
    let decade = window(UniverseTime::EPOCH, 10.0 * YEAR);
    let sun = query([0.0, 26_000.0, 20.0], decade, sensor);
    let mut onsets = Vec::new();
    for class in [
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
    ] {
        let cells = scan_plan(&galaxy, class, &sun);
        let chunk = cells.len().div_ceil(8);
        let found: Vec<Detection> = std::thread::scope(|scope| {
            let handles: Vec<_> = cells
                .chunks(chunk)
                .map(|cells| {
                    let (galaxy, sun) = (&galaxy, &sun);
                    scope.spawn(move || {
                        let mut noise = NoiseCache::with_capacity(4_096);
                        let mut context = AlertContext::new(&mut noise, &NoModifiers);
                        let mut out = Vec::new();
                        for &cell in cells {
                            scan_cell(galaxy, &mut context, class, cell, sun, &mut out);
                        }
                        out
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().unwrap())
                .collect()
        });
        eprintln!("{class}: {} cells, {} detections", cells.len(), found.len());
        onsets.extend(
            found
                .into_iter()
                .filter(|d| d.phase() == DetectionPhase::Onset),
        );
    }
    onsets.sort_by_key(|d| (d.arrival(), d.event()));
    for d in &onsets {
        let v = -2.5 * hyperion_sim::math::log10(d.flux().value() / V_ZERO_POINT);
        eprintln!(
            "{} arriving {} at V = {v:.2}, {:.0} ly",
            d.event(),
            d.arrival(),
            d.retardation().light_age().as_julian_years_f64()
        );
    }
    #[expect(clippy::cast_precision_loss, reason = "a count of tens")]
    let a_year = onsets.len() as f64 / 10.0;
    eprintln!("{a_year} novae a year seen to V = 12");
    // Measured 2026-10-31: 98 in the decade, 87 of them of fast hosts, from 2.1 to 12.0 mag at
    // 3,051 to 50,149 ly, the median 21,515 ly.
    assert_eq!(onsets.len(), 98, "pinned as measured: {a_year} a year");
}
