//! The alerts' oracle (plan 12's Test helpers, P12.T5.b and P12.T5.c): every one-shot entry, every
//! magnetar host, every host of plan 11's binary classes and every luminous blue variable, of a box
//! of class cells about the observer, each event evaluated directly, against which the walks'
//! detections are compared as sets.
//!
//! The brute force opens every cell of the box of side 2 × (reach + 4,096 ly) and a cell about the
//! observer, with no sphere test and no reach, and evaluates each entry's event from its own
//! definition: its arrival T + d ÷ c, its phase in the window, its light in each of the sensor's
//! bands (its peak for an onset, its light at the window's start for one in progress: a
//! supernova's template or a merger's binary curve), its flux
//! through plan 07's line of sight, and the band in which it stands highest over the detection
//! limit. A line's transmission is at most 1, so an event whose unextinguished flux misses every
//! band's limit is missed through any line, and no line is marched for it.

use std::collections::BTreeMap;

use hyperion_sim::Seed;
use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, Detection, DetectionPhase, EruptionClass,
    GIANT_ERUPTION_SHARES, LongestDuration, MAGNETAR_BURST_XRAY_SHARE,
    MAGNETAR_GIANT_FLARE_XRAY_SHARE, MagnetarCells, MagnetarClass, MagnetarHost, OneShotCells,
    OneShotClass, OneShotEvent, OneShotLight, TransientKind, binary_luminosity, binary_peak, flux,
    inverse_square_flux, kind_reach, supernova_luminosity, supernova_peak,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::events::TimeWindow;
use hyperion_sim::events::{EventSeries, tags as event_tags};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::binary::{AwdProcess, AwdSide, XrayBinaries};
use hyperion_sim::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use hyperion_sim::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, CatalogueMotion, ClassId,
};
use hyperion_sim::id::{EventId, SystemId};
use hyperion_sim::observe::{Motion, bearing, light_time};
use hyperion_sim::stellar::binary::events::{
    BinaryEvent, BinaryEventKind, BinarySystem, events_in, light_curve, xray_luminosity,
};
use hyperion_sim::stellar::binary::scan::{
    AwdScanMarks, XrbScanMarks, scan_cell as scan_marks_cell,
};
use hyperion_sim::stellar::events::lbv::{
    Eruption, MAX_DURATION as LBV_MAX_DURATION, eruptions_in as lbv_eruptions_in,
};
use hyperion_sim::stellar::events::magnetar::{MAX_BURST_DURATION, MagnetarModel, MagnetarSeries};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::consts::SOLAR_LUMINOSITY_W;
use hyperion_sim::units::consts::SPEED_OF_LIGHT;
use hyperion_sim::units::{LightYears, Watts, Years};
use hyperion_testkit::float::bits;

/// A detection as the walk and the oracle are compared: every field it reports, floats as bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seen {
    pub arrival: UniverseTime,
    pub event: EventId,
    pub host: SystemId,
    pub class: Option<ClassId>,
    pub kind: TransientKind,
    pub emitted: UniverseTime,
    pub phase: DetectionPhase,
    pub band: AlertBand,
    pub flux: u64,
    pub resolvable: bool,
    pub bearing: [u64; 2],
    pub apparent: ([i32; 3], [u64; 3]),
    pub light_age: Span,
    pub distance_now: u64,
    pub velocity_then: [u64; 3],
    pub motion: Motion,
}

impl Seen {
    /// The walk's `detection`.
    #[must_use]
    pub fn of(detection: &Detection) -> Self {
        let retardation = detection.retardation();
        assert_eq!(retardation.observed(), detection.arrival());
        assert_eq!(retardation.emitted(), detection.emitted());
        Self {
            arrival: detection.arrival(),
            event: detection.event(),
            host: detection.host(),
            class: detection.class(),
            kind: detection.kind(),
            emitted: detection.emitted(),
            phase: detection.phase(),
            band: detection.band(),
            flux: bits(detection.flux().value()),
            resolvable: detection.is_resolvable(),
            bearing: [
                bits(detection.bearing().azimuth().value()),
                bits(detection.bearing().elevation().value()),
            ],
            apparent: position_bits(retardation.apparent_position()),
            light_age: retardation.light_age(),
            distance_now: bits(retardation.distance_now().value()),
            velocity_then: retardation.velocity_then().metres_per_second().map(bits),
            motion: retardation.motion(),
        }
    }
}

/// A position's cell and its offset's bits.
fn position_bits(position: &GalacticPosition) -> ([i32; 3], [u64; 3]) {
    (
        position.cell().to_array(),
        position.offset_metres().map(bits),
    )
}

/// The test's cache of a class's cells: every cell it is asked for, for one galaxy, which the walk
/// borrows through its context and the oracle reads directly.
#[derive(Debug, Default)]
pub struct KeepCells(BTreeMap<CatalogueCellKey, Vec<OneShotEvent>>);

impl KeepCells {
    /// The events of `process`'s cell `key` in `galaxy`, generated on first asking from the cell's
    /// entries, each entry's own event, whatever the class's walk reads them from.
    pub fn events<P: OneShotClass>(
        &mut self,
        galaxy: &Galaxy,
        process: &P,
        key: CatalogueCellKey,
    ) -> &[OneShotEvent] {
        self.0
            .entry(key)
            .or_insert_with(|| entry_events(galaxy, process, key))
    }
}

/// The events of `process`'s cell `key` in `galaxy` from its entries, in index order: each
/// entry's [`OneShotClass::event`].
#[must_use]
pub fn entry_events<P: OneShotClass>(
    galaxy: &Galaxy,
    process: &P,
    key: CatalogueCellKey,
) -> Vec<OneShotEvent> {
    CatalogueClassCell::generate(galaxy, process, key)
        .entries()
        .iter()
        .filter_map(|entry| process.event(galaxy, entry.value()))
        .collect()
}

impl OneShotCells for KeepCells {
    fn with_events(
        &mut self,
        key: CatalogueCellKey,
        generate: &mut dyn FnMut() -> Vec<OneShotEvent>,
        visit: &mut dyn FnMut(&[OneShotEvent]),
    ) {
        visit(self.0.entry(key).or_insert_with(generate));
    }
}

/// The observer's position `ly` light-years from the origin, galactic.
#[must_use]
pub fn observer_at(ly: [f64; 3]) -> GalacticPosition {
    GalacticPosition::from_light_years(ly).expect("the observer is in the root cube")
}

/// Every detection of `process`'s class by `query`'s sensor, by walking the box of cells about the
/// observer (module documentation), sorted.
///
/// # Panics
///
/// If the observer's cell is not a cell of the class's grid, which no point of the cube is not.
pub fn brute_force_detections<P: OneShotClass>(
    galaxy: &Galaxy,
    context: &mut AlertContext<'_>,
    cells: &mut KeepCells,
    process: &P,
    query: &AlertQuery,
) -> Vec<Seen> {
    let class = process.class();
    if !query.classes().contains(class) {
        return Vec::new();
    }
    let kind = process.kind();
    let reach = kind_reach(kind, query.sensor()).value();
    let observer = query.observer_position();
    let centre = CatalogueCellKey::containing(class, observer).expect("a cell of the cube");
    let edge = f64::from(centre.edge_ly());
    let half = ((reach + LIFETIME_REACH.value()) / edge).ceil();
    assert!(half < 1e3, "a box of {half} cells about the observer");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number under 1,000"
    )]
    let half = half as i32 + 1;
    let [cx, cy, cz] = centre.cell();
    let mut seen = Vec::new();
    for x in cx - half..=cx + half {
        for y in cy - half..=cy + half {
            for z in cz - half..=cz + half {
                // A cell of the box outside the root cube is no cell of the class.
                let Ok(key) = CatalogueCellKey::new(class, [x, y, z]) else {
                    continue;
                };
                for event in cells.events(galaxy, process, key).to_vec() {
                    seen.extend(evaluated(galaxy, context, query, class, kind, &event));
                }
            }
        }
    }
    seen.sort();
    seen
}

/// `event` as `query`'s sensor sees it, evaluated directly (module documentation), or `None`.
fn evaluated(
    galaxy: &Galaxy,
    context: &mut AlertContext<'_>,
    query: &AlertQuery,
    class: ClassId,
    kind: TransientKind,
    event: &OneShotEvent,
) -> Option<Seen> {
    let observer = query.observer_position();
    let window = query.window();
    let site = event.site();
    let distance = site.distance_to(observer);
    let arrival = event.emitted().checked_add(light_time(distance)).unwrap();
    let phase = if window.start() <= arrival && arrival < window.end() {
        DetectionPhase::Onset
    } else if arrival < window.start() {
        DetectionPhase::InProgress
    } else {
        return None;
    };
    let light = |band| match (phase, *event.light()) {
        (DetectionPhase::Onset, OneShotLight::Supernova(kind)) => supernova_peak(kind, band),
        (DetectionPhase::Onset, OneShotLight::Binary(curve)) => binary_peak(&curve, band),
        (DetectionPhase::InProgress, light) => {
            let age = window.start().checked_since(arrival).unwrap();
            match light {
                OneShotLight::Supernova(kind) => {
                    supernova_luminosity(kind, Years::new(age.as_julian_years_f64()), band)
                }
                OneShotLight::Binary(curve) => binary_luminosity(&curve, age, band),
            }
        }
    };
    let bare = query.sensor().bands().any(|(band, limits)| {
        inverse_square_flux(light(band), distance).is_some_and(|f| f >= limits.detect())
    });
    if !bare {
        return None;
    }
    let line = context.sightline(galaxy, &site, observer);
    let mut best = None;
    for band in AlertBand::ALL {
        let Some(limits) = query.sensor().limits(band) else {
            continue;
        };
        let Some(f) = flux(light(band), distance, &line, band) else {
            continue;
        };
        let over = f.value() / limits.detect().value();
        let better = match best {
            None => over >= 1.0,
            Some((_, _, most)) => over > most,
        };
        if better {
            best = Some((band, f, over));
        }
    }
    let (band, f, _) = best?;
    let resolve = query.sensor().limits(band).unwrap().resolve();
    let to = bearing(observer, &site).unwrap();
    let present = event.motion().position_at(arrival);
    Some(Seen {
        arrival,
        event: event.event(),
        host: event.host(),
        class: Some(class),
        kind,
        emitted: event.emitted(),
        phase,
        band,
        flux: bits(f.value()),
        resolvable: f >= resolve,
        bearing: [bits(to.azimuth().value()), bits(to.elevation().value())],
        apparent: position_bits(&site),
        light_age: light_time(distance),
        distance_now: bits(LightYears::from(observer.distance_to(&present)).value()),
        velocity_then: event
            .motion()
            .velocity_at(event.emitted())
            .metres_per_second()
            .map(bits),
        motion: Motion::Drift,
    })
}

/// The test's cache of a class's magnetar hosts: every cell it is asked for, for one galaxy.
#[derive(Debug, Default)]
pub struct KeepMagnetarCells(BTreeMap<CatalogueCellKey, Vec<MagnetarHost>>);

impl KeepMagnetarCells {
    /// The hosts of `process`'s cell `key` in `galaxy`, generated on first asking.
    pub fn hosts<P: MagnetarClass>(
        &mut self,
        galaxy: &Galaxy,
        process: &P,
        key: CatalogueCellKey,
    ) -> &[MagnetarHost] {
        self.0.entry(key).or_insert_with(|| {
            CatalogueClassCell::generate(galaxy, process, key)
                .entries()
                .iter()
                .filter_map(|entry| process.magnetar_host(galaxy, entry.value()))
                .collect()
        })
    }
}

impl MagnetarCells for KeepMagnetarCells {
    fn with_hosts(
        &mut self,
        key: CatalogueCellKey,
        generate: &mut dyn FnMut() -> Vec<MagnetarHost>,
        visit: &mut dyn FnMut(&[MagnetarHost]),
    ) {
        visit(self.0.entry(key).or_insert_with(generate));
    }
}

/// Every magnetar detection of `process`'s class by `query`'s sensor, by walking the box of cells
/// about the observer and listing each host's events over an emission interval wide enough by its
/// speed alone, each evaluated directly (P12.T5.c), sorted.
///
/// The interval is the window less the light time from the host's position at the window's start,
/// widened by β (s + window + D) ÷ (1 ∓ β) for its fastest speed, a day either side and the longest
/// event twice over, from bounds alone: no root on the light cone. The line of sight is the walk's
/// rule, from the remnant's position at the start of the Julian year that holds each onset, so that
/// fluxes compare bit for bit.
///
/// # Panics
///
/// As [`brute_force_detections`].
pub fn brute_force_magnetar_detections<P: MagnetarClass>(
    galaxy: &Galaxy,
    context: &mut AlertContext<'_>,
    cells: &mut KeepMagnetarCells,
    process: &P,
    query: &AlertQuery,
) -> Vec<Seen> {
    let class = process.class();
    if !query.classes().contains(class) {
        return Vec::new();
    }
    let reach = kind_reach(TransientKind::MagnetarBurst, query.sensor()).value();
    let observer = query.observer_position();
    let centre = CatalogueCellKey::containing(class, observer).expect("a cell of the cube");
    let edge = f64::from(centre.edge_ly());
    let half = ((reach + LIFETIME_REACH.value()) / edge).ceil();
    assert!(half < 1e3, "a box of {half} cells about the observer");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number under 1,000"
    )]
    let half = half as i32 + 1;
    let [cx, cy, cz] = centre.cell();
    let mut seen = Vec::new();
    for x in cx - half..=cx + half {
        for y in cy - half..=cy + half {
            for z in cz - half..=cz + half {
                // A cell of the box outside the root cube is no cell of the class.
                let Ok(key) = CatalogueCellKey::new(class, [x, y, z]) else {
                    continue;
                };
                for host in cells.hosts(galaxy, process, key).to_vec() {
                    seen.extend(magnetar_seen(galaxy, context, query, class, &host));
                }
            }
        }
    }
    seen.sort();
    seen
}

/// `host`'s events as `query`'s sensor sees them, evaluated directly.
fn magnetar_seen(
    galaxy: &Galaxy,
    context: &mut AlertContext<'_>,
    query: &AlertQuery,
    class: ClassId,
    host: &MagnetarHost,
) -> Vec<Seen> {
    let Some(limits) = query.sensor().limits(AlertBand::XRay) else {
        return Vec::new();
    };
    let observer = query.observer_position();
    let window = query.window();
    let motion = host.motion();
    let interval = wide_interval(host, observer, window);
    let star = host.star().to_star();
    let model = MagnetarModel::of(&star).expect("a host leaves its magnetar");
    let series = MagnetarSeries::new(galaxy.seed(), host.body());
    let (mut bursts, mut flares) = (Vec::new(), Vec::new());
    model.bursts_in(&series, interval, &mut bursts);
    model.giant_flares_in(&series, interval, &mut flares);
    let mut events: Vec<_> = bursts
        .iter()
        .map(|b| {
            (
                b.onset(),
                b.id(),
                b.end(),
                b.mean_luminosity_watts() * MAGNETAR_BURST_XRAY_SHARE,
            )
        })
        .chain(flares.iter().map(|f| {
            let share = MAGNETAR_GIANT_FLARE_XRAY_SHARE;
            (
                f.onset(),
                f.id(),
                f.end(),
                f.mean_luminosity_watts() * share,
            )
        }))
        .collect();
    events.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut lines = BTreeMap::new();
    let mut seen = Vec::new();
    for (onset, id, end, xray) in events {
        let site = motion.position_at(onset);
        let distance = site.distance_to(observer);
        let arrival = onset.checked_add(light_time(distance)).unwrap();
        let end_arrival = end
            .checked_add(light_time(motion.position_at(end).distance_to(observer)))
            .unwrap();
        let phase = if window.start() <= arrival && arrival < window.end() {
            DetectionPhase::Onset
        } else if arrival < window.start() && window.start() < end_arrival {
            DetectionPhase::InProgress
        } else {
            continue;
        };
        let watts = Watts::new(xray);
        if !inverse_square_flux(watts, distance).is_some_and(|f| f >= limits.detect()) {
            continue;
        }
        // The walk's rule: the line from the remnant's position at the start of the Julian year
        // that holds the onset.
        let year = onset.seconds().div_euclid(31_557_600) * 31_557_600;
        let line = lines.entry(year).or_insert_with(|| {
            let from = motion.position_at(UniverseTime::new(year, 0).unwrap());
            context.sightline(galaxy, &from, observer)
        });
        let f = flux(watts, distance, line, AlertBand::XRay).unwrap();
        if f < limits.detect() {
            continue;
        }
        let to = bearing(observer, &site).unwrap();
        let present = motion.position_at(arrival);
        seen.push(Seen {
            arrival,
            event: id,
            host: host.host(),
            class: Some(class),
            kind: TransientKind::MagnetarBurst,
            emitted: onset,
            phase,
            band: AlertBand::XRay,
            flux: bits(f.value()),
            resolvable: f >= limits.resolve(),
            bearing: [bits(to.azimuth().value()), bits(to.elevation().value())],
            apparent: position_bits(&site),
            light_age: light_time(distance),
            distance_now: bits(LightYears::from(observer.distance_to(&present)).value()),
            velocity_then: motion.velocity_at(onset).metres_per_second().map(bits),
            motion: Motion::Drift,
        });
    }
    seen
}

/// The emission interval wide enough, by `host`'s fastest speed alone, to hold every event whose
/// light reaches `observer` in `window` or is arriving at its start (`brute_force_magnetar_detections`).
fn wide_interval(
    host: &MagnetarHost,
    observer: &GalacticPosition,
    window: TimeWindow,
) -> TimeWindow {
    let motion = host.motion();
    let speed = [
        motion.velocity_at(motion.at()),
        motion.velocity_at(UniverseTime::EPOCH),
    ]
    .iter()
    .map(|v| v.speed().value())
    .fold(0.0, f64::max);
    let beta = speed / SPEED_OF_LIGHT;
    let s0 = motion
        .position_at(window.start())
        .distance_to(observer)
        .value()
        / SPEED_OF_LIGHT;
    let width = window
        .end()
        .checked_since(window.start())
        .unwrap()
        .as_seconds_f64();
    let longest = 2.0 * MAX_BURST_DURATION.as_seconds_f64() + 86_400.0;
    let reach = beta * (s0 + width + longest);
    let (s_hi, s_lo) = (
        (s0 + reach) / (1.0 - beta),
        ((s0 - reach) / (1.0 + beta)).max(0.0),
    );
    let at = |t: UniverseTime, seconds: f64| {
        let span = Span::from_seconds_f64(seconds.abs()).unwrap();
        if seconds < 0.0 {
            t.checked_sub(span)
        } else {
            t.checked_add(span)
        }
        .unwrap()
    };
    TimeWindow::new(
        at(window.start(), -(longest + s_hi + 86_400.0)),
        at(window.end(), -(s_lo - 86_400.0)),
    )
    .unwrap()
}

/// A host of plan 11's binary classes as the oracle reads it: its scan marks.
#[derive(Debug, Clone, Copy)]
enum BinaryHost {
    Awd(AwdScanMarks),
    Xrb(XrbScanMarks),
}

impl BinaryHost {
    fn system(&self, seed: Seed) -> BinarySystem<'_> {
        match self {
            Self::Awd(marks) => BinarySystem::accreting_white_dwarf_scan(seed, marks),
            Self::Xrb(marks) => BinarySystem::xray_binary(seed, marks),
        }
    }

    fn id(&self) -> SystemId {
        match self {
            Self::Awd(marks) => marks.id(),
            Self::Xrb(marks) => marks.id(),
        }
    }

    fn motion(&self) -> CatalogueMotion {
        match self {
            Self::Awd(marks) => CatalogueMotion::straight(*marks.position(), marks.velocity()),
            Self::Xrb(marks) => CatalogueMotion::straight(*marks.position(), marks.velocity()),
        }
    }
}

/// The registry's kind of plan 11's `kind`, for the binary classes the oracle walks.
fn binary_kind(kind: BinaryEventKind) -> TransientKind {
    match kind {
        BinaryEventKind::Nova => TransientKind::Nova,
        BinaryEventKind::XrayTransient => TransientKind::XrayTransient,
        BinaryEventKind::XrayBurst => TransientKind::XrayBurst,
        BinaryEventKind::BeXGiantOutburst => TransientKind::BeXOutburst,
        kind @ (BinaryEventKind::DwarfNova
        | BinaryEventKind::LuminousRedNova
        | BinaryEventKind::Kilonova) => panic!("{kind:?} is not walked with its class"),
    }
}

/// The kinds the recurrent walk finds over `class`'s hosts, plan 11's binary classes'.
fn binary_kinds(class: ClassId) -> &'static [TransientKind] {
    if class == ClassId::XRAY_BINARY {
        &[
            TransientKind::XrayTransient,
            TransientKind::XrayBurst,
            TransientKind::BeXOutburst,
        ]
    } else {
        assert!(
            [
                ClassId::ACCRETING_WHITE_DWARF,
                ClassId::ACCRETING_WHITE_DWARF_SLOW
            ]
            .contains(&class),
            "{class} is no binary class of the walk"
        );
        &[TransientKind::Nova]
    }
}

/// Every recurrent detection of plan 11's binary `class`, an accreting white dwarf class or the
/// X-ray binaries, by `query`'s sensor (P12.T5.c), sorted: every host of the box of cells about the
/// observer by its scan marks, its events listed by plan 11 over an interval wide enough by its
/// speed alone and the class's longest event, each evaluated directly from plan 11's light curve
/// functions (`light_curve`, `xray_luminosity`).
///
/// An event is an onset where its light arrives in the window, seen by its peak, and in progress
/// where it arrived before the window and is not over at its start (plan 11's `duration`), seen by
/// its light then. The line of sight is the walk's rule, from the host's position at the start of
/// the Julian year that holds each onset, so that fluxes compare bit for bit. No slow host is
/// passed over by the eruptions its marks carry.
///
/// # Panics
///
/// As [`brute_force_detections`], and for a class that is no binary class of the walk.
pub fn brute_force_binary_detections(
    galaxy: &Galaxy,
    context: &mut AlertContext<'_>,
    class: ClassId,
    query: &AlertQuery,
) -> Vec<Seen> {
    if !query.classes().contains(class) {
        return Vec::new();
    }
    let kinds = binary_kinds(class);
    let reach = kinds
        .iter()
        .map(|&kind| kind_reach(kind, query.sensor()).value())
        .fold(0.0, f64::max);
    if reach <= 0.0 {
        return Vec::new();
    }
    let longest = kinds
        .iter()
        .map(|&kind| match kind.row().duration() {
            Some(LongestDuration::AtMost(span)) => span.as_seconds_f64(),
            other => panic!("{kind:?} lasts {other:?}"),
        })
        .fold(0.0, f64::max);
    let observer = query.observer_position();
    let centre = CatalogueCellKey::containing(class, observer).expect("a cell of the cube");
    let edge = f64::from(centre.edge_ly());
    let half = ((reach + LIFETIME_REACH.value()) / edge).ceil();
    assert!(half < 1e3, "a box of {half} cells about the observer");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number under 1,000"
    )]
    let half = half as i32 + 1;
    let [cx, cy, cz] = centre.cell();
    let (fast, slow, xrb) = (
        AwdProcess::new(galaxy, AwdSide::Fast),
        AwdProcess::new(galaxy, AwdSide::Slow),
        XrayBinaries::new(galaxy),
    );
    let mut seen = Vec::new();
    let mut events = Vec::new();
    for x in cx - half..=cx + half {
        for y in cy - half..=cy + half {
            for z in cz - half..=cz + half {
                let Ok(key) = CatalogueCellKey::new(class, [x, y, z]) else {
                    continue;
                };
                let hosts: Vec<BinaryHost> = if class == ClassId::XRAY_BINARY {
                    let mut marks = Vec::new();
                    scan_marks_cell(galaxy, &xrb, key, &mut marks);
                    marks.into_iter().map(BinaryHost::Xrb).collect()
                } else {
                    let process = if class == ClassId::ACCRETING_WHITE_DWARF {
                        &fast
                    } else {
                        &slow
                    };
                    let mut marks = Vec::new();
                    scan_marks_cell(galaxy, process, key, &mut marks);
                    marks.into_iter().map(BinaryHost::Awd).collect()
                };
                for host in &hosts {
                    let motion = host.motion();
                    let interval = wide_line_interval(&motion, observer, query.window(), longest);
                    events.clear();
                    events_in(&host.system(galaxy.seed()), interval, &mut events);
                    seen.extend(binary_seen(
                        galaxy,
                        context,
                        query,
                        class,
                        host.id(),
                        &motion,
                        &events,
                    ));
                }
            }
        }
    }
    seen.sort();
    seen
}

/// `events` of `host` moving by `motion` as `query`'s sensor sees them, evaluated directly.
fn binary_seen(
    galaxy: &Galaxy,
    context: &mut AlertContext<'_>,
    query: &AlertQuery,
    class: ClassId,
    host: SystemId,
    motion: &CatalogueMotion,
    events: &[BinaryEvent],
) -> Vec<Seen> {
    let observer = query.observer_position();
    let window = query.window();
    let mut lines = BTreeMap::new();
    let mut seen = Vec::new();
    for event in events {
        let onset = event.onset();
        let site = motion.position_at(onset);
        let distance = site.distance_to(observer);
        let arrival = onset.checked_add(light_time(distance)).unwrap();
        let (phase, age) = if window.start() <= arrival && arrival < window.end() {
            (DetectionPhase::Onset, event.curve().peak())
        } else if arrival < window.start()
            && window.start().checked_since(arrival).unwrap() < event.duration()
        {
            (
                DetectionPhase::InProgress,
                window.start().checked_since(arrival).unwrap(),
            )
        } else {
            continue;
        };
        let light = |band| match band {
            AlertBand::Photometric(band) => light_curve(event, age, band),
            AlertBand::XRay => xray_luminosity(event, age),
        };
        let bare = query.sensor().bands().any(|(band, limits)| {
            inverse_square_flux(light(band), distance).is_some_and(|f| f >= limits.detect())
        });
        if !bare {
            continue;
        }
        let year = onset.seconds().div_euclid(31_557_600) * 31_557_600;
        let line = lines.entry(year).or_insert_with(|| {
            let from = motion.position_at(UniverseTime::new(year, 0).unwrap());
            context.sightline(galaxy, &from, observer)
        });
        let mut best = None;
        for band in AlertBand::ALL {
            let Some(limits) = query.sensor().limits(band) else {
                continue;
            };
            let Some(f) = flux(light(band), distance, line, band) else {
                continue;
            };
            let over = f.value() / limits.detect().value();
            let better = match best {
                None => over >= 1.0,
                Some((_, _, most)) => over > most,
            };
            if better {
                best = Some((band, f, over));
            }
        }
        let Some((band, f, _)) = best else {
            continue;
        };
        let resolve = query.sensor().limits(band).unwrap().resolve();
        let to = bearing(observer, &site).unwrap();
        let present = motion.position_at(arrival);
        seen.push(Seen {
            arrival,
            event: event.id(),
            host,
            class: Some(class),
            kind: binary_kind(event.kind()),
            emitted: onset,
            phase,
            band,
            flux: bits(f.value()),
            resolvable: f >= resolve,
            bearing: [bits(to.azimuth().value()), bits(to.elevation().value())],
            apparent: position_bits(&site),
            light_age: light_time(distance),
            distance_now: bits(LightYears::from(observer.distance_to(&present)).value()),
            velocity_then: motion.velocity_at(onset).metres_per_second().map(bits),
            motion: Motion::Drift,
        });
    }
    seen
}

/// The emission interval wide enough, by the speed of a host moving by `motion` alone, to hold
/// every event of at most `longest` seconds whose light reaches `observer` in `window` or is
/// arriving at its start: the window less the light time from the host's position at the window's
/// start, widened by β (s + window + 2 × longest + 2 days) ÷ (1 ∓ β), twice the longest event and
/// a day either side, from bounds alone.
fn wide_line_interval(
    motion: &CatalogueMotion,
    observer: &GalacticPosition,
    window: TimeWindow,
    longest: f64,
) -> TimeWindow {
    let beta = motion.velocity_at(UniverseTime::EPOCH).speed().value() / SPEED_OF_LIGHT;
    let s0 = motion
        .position_at(window.start())
        .distance_to(observer)
        .value()
        / SPEED_OF_LIGHT;
    let width = window
        .end()
        .checked_since(window.start())
        .unwrap()
        .as_seconds_f64();
    let reach = beta * (s0 + width + 2.0 * longest + 2.0 * 86_400.0);
    let (s_hi, s_lo) = (
        (s0 + reach) / (1.0 - beta),
        ((s0 - reach) / (1.0 + beta)).max(0.0),
    );
    let at = |t: UniverseTime, seconds: f64| {
        let span = Span::from_seconds_f64(seconds.abs()).unwrap();
        if seconds < 0.0 {
            t.checked_sub(span)
        } else {
            t.checked_add(span)
        }
        .unwrap()
    };
    TimeWindow::new(
        at(window.start(), -(2.0 * longest + s_hi + 2.0 * 86_400.0)),
        at(window.end(), -(s_lo - 2.0 * 86_400.0)),
    )
    .unwrap()
}

/// Every giant eruption of `process`'s class, luminous blue variables, that `query`'s sensor sees
/// (P12.T5.c), sorted: every host of the box of cells about the observer, its star built whatever
/// its window, its eruptions listed by plan 06 over an interval wide enough by its speed alone and
/// the longest eruption, each evaluated directly: its own light, (factor − 1) times the star's
/// quiescent luminosity at its onset, times the eruptions' share in the band.
///
/// An eruption is an onset where its light arrives in the window, seen by its peak, and in progress
/// where it arrived before the window and is not over at its start, seen by its light then. The
/// line of sight is the walk's rule, as for the binary classes.
///
/// # Panics
///
/// As [`brute_force_detections`].
pub fn brute_force_eruption_detections<P: EruptionClass>(
    galaxy: &Galaxy,
    context: &mut AlertContext<'_>,
    process: &P,
    query: &AlertQuery,
) -> Vec<Seen> {
    let class = process.class();
    if !query.classes().contains(class) {
        return Vec::new();
    }
    let reach = kind_reach(TransientKind::GiantEruption, query.sensor()).value();
    if reach <= 0.0 {
        return Vec::new();
    }
    let observer = query.observer_position();
    let window = query.window();
    let centre = CatalogueCellKey::containing(class, observer).expect("a cell of the cube");
    let edge = f64::from(centre.edge_ly());
    let half = ((reach + LIFETIME_REACH.value()) / edge).ceil();
    assert!(half < 1e3, "a box of {half} cells about the observer");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number under 1,000"
    )]
    let half = half as i32 + 1;
    let [cx, cy, cz] = centre.cell();
    let mut seen = Vec::new();
    let mut eruptions = Vec::new();
    for x in cx - half..=cx + half {
        for y in cy - half..=cy + half {
            for z in cz - half..=cz + half {
                let Ok(key) = CatalogueCellKey::new(class, [x, y, z]) else {
                    continue;
                };
                for entry in CatalogueClassCell::generate(galaxy, process, key).entries() {
                    let Some(host) = process.eruption_host(galaxy, entry.value()) else {
                        continue;
                    };
                    let motion = *host.motion();
                    let star = host.star().to_star();
                    let interval = wide_line_interval(
                        &motion,
                        observer,
                        window,
                        LBV_MAX_DURATION.as_seconds_f64(),
                    );
                    let series = EventSeries::new(
                        galaxy.seed(),
                        event_tags::STAR_LBV_ERUPTION,
                        host.body().into(),
                    );
                    eruptions.clear();
                    lbv_eruptions_in(&star, &series, interval, &mut eruptions);
                    for eruption in &eruptions {
                        seen.extend(eruption_seen(
                            galaxy,
                            context,
                            query,
                            class,
                            host.host(),
                            &motion,
                            eruption,
                        ));
                    }
                }
            }
        }
    }
    seen.sort();
    seen
}

/// `eruption` of `host` moving by `motion` as `query`'s sensor sees it, evaluated directly.
fn eruption_seen(
    galaxy: &Galaxy,
    context: &mut AlertContext<'_>,
    query: &AlertQuery,
    class: ClassId,
    host: SystemId,
    motion: &CatalogueMotion,
    eruption: &Eruption,
) -> Option<Seen> {
    let observer = query.observer_position();
    let window = query.window();
    let onset = eruption.onset();
    let site = motion.position_at(onset);
    let distance = site.distance_to(observer);
    let arrival = onset.checked_add(light_time(distance)).unwrap();
    let (phase, emitted) = if window.start() <= arrival && arrival < window.end() {
        (DetectionPhase::Onset, eruption.peak())
    } else if arrival < window.start()
        && window.start().checked_since(arrival).unwrap() < eruption.duration()
    {
        let age = window.start().checked_since(arrival).unwrap();
        (DetectionPhase::InProgress, onset.checked_add(age).unwrap())
    } else {
        return None;
    };
    let quiescent = eruption.quiescent_luminosity().value() * SOLAR_LUMINOSITY_W;
    let own = (eruption.luminosity_factor_at(emitted) - 1.0) * quiescent;
    let light = |band: AlertBand| {
        let i = AlertBand::ALL.iter().position(|&b| b == band).unwrap();
        Watts::new(own * GIANT_ERUPTION_SHARES[i])
    };
    let bare = query.sensor().bands().any(|(band, limits)| {
        inverse_square_flux(light(band), distance).is_some_and(|f| f >= limits.detect())
    });
    if !bare {
        return None;
    }
    let year = onset.seconds().div_euclid(31_557_600) * 31_557_600;
    let from = motion.position_at(UniverseTime::new(year, 0).unwrap());
    let line = context.sightline(galaxy, &from, observer);
    let mut best = None;
    for band in AlertBand::ALL {
        let Some(limits) = query.sensor().limits(band) else {
            continue;
        };
        let Some(f) = flux(light(band), distance, &line, band) else {
            continue;
        };
        let over = f.value() / limits.detect().value();
        let better = match best {
            None => over >= 1.0,
            Some((_, _, most)) => over > most,
        };
        if better {
            best = Some((band, f, over));
        }
    }
    let (band, f, _) = best?;
    let resolve = query.sensor().limits(band).unwrap().resolve();
    let to = bearing(observer, &site).unwrap();
    let present = motion.position_at(arrival);
    Some(Seen {
        arrival,
        event: eruption.id(),
        host,
        class: Some(class),
        kind: TransientKind::GiantEruption,
        emitted: onset,
        phase,
        band,
        flux: bits(f.value()),
        resolvable: f >= resolve,
        bearing: [bits(to.azimuth().value()), bits(to.elevation().value())],
        apparent: position_bits(&site),
        light_age: light_time(distance),
        distance_now: bits(LightYears::from(observer.distance_to(&present)).value()),
        velocity_then: motion.velocity_at(onset).metres_per_second().map(bits),
        motion: Motion::Drift,
    })
}
