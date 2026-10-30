//! The alerts' oracle (plan 12's Test helpers, P12.T5.b): every one-shot entry of a box of class
//! cells about the observer, each event evaluated directly, against which the walk's detections are
//! compared as sets.
//!
//! The brute force opens every cell of the box of side 2 × (reach + 4,096 ly) and a cell about the
//! observer, with no sphere test and no reach, and evaluates each entry's event from its own
//! definition: its arrival T + d ÷ c, its phase in the window, its light in each of the sensor's
//! bands (its peak for an onset, its light at the window's start for one in progress), its flux
//! through plan 07's line of sight, and the band in which it stands highest over the detection
//! limit. A line's transmission is at most 1, so an event whose unextinguished flux misses every
//! band's limit is missed through any line, and no line is marched for it.

use std::collections::BTreeMap;

use hyperion_sim::alerts::{
    AlertBand, AlertContext, AlertQuery, Detection, DetectionPhase, OneShotCells, OneShotClass,
    OneShotEvent, TransientKind, flux, inverse_square_flux, kind_reach, supernova_luminosity,
    supernova_peak,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, CatalogueClassCell, ClassId};
use hyperion_sim::id::{EventId, SystemId};
use hyperion_sim::observe::{bearing, light_time};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::Years;
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
    /// The events of `process`'s cell `key` in `galaxy`, generated on first asking.
    pub fn events<P: OneShotClass>(
        &mut self,
        galaxy: &Galaxy,
        process: &P,
        key: CatalogueCellKey,
    ) -> &[OneShotEvent] {
        self.0.entry(key).or_insert_with(|| {
            CatalogueClassCell::generate(galaxy, process, key)
                .entries()
                .iter()
                .map(|entry| process.event(entry.value()))
                .collect()
        })
    }
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
    let window = query.window();
    let mut seen = Vec::new();
    for x in cx - half..=cx + half {
        for y in cy - half..=cy + half {
            for z in cz - half..=cz + half {
                let Ok(key) = CatalogueCellKey::new(class, [x, y, z]) else {
                    continue;
                };
                for event in cells.events(galaxy, process, key).to_vec() {
                    let site = event.site();
                    let distance = site.distance_to(observer);
                    let arrival = event.emitted().checked_add(light_time(distance)).unwrap();
                    let phase = if window.start() <= arrival && arrival < window.end() {
                        DetectionPhase::Onset
                    } else if arrival < window.start() {
                        DetectionPhase::InProgress
                    } else {
                        continue;
                    };
                    let light = |band| match phase {
                        DetectionPhase::Onset => supernova_peak(event.supernova(), band),
                        DetectionPhase::InProgress => {
                            let age = window.start().checked_since(arrival).unwrap();
                            supernova_luminosity(
                                event.supernova(),
                                Years::new(age.as_julian_years_f64()),
                                band,
                            )
                        }
                    };
                    let bare = query.sensor().bands().any(|(band, limits)| {
                        inverse_square_flux(light(band), distance)
                            .is_some_and(|f| f >= limits.detect())
                    });
                    if !bare {
                        continue;
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
                    let Some((band, f, _)) = best else {
                        continue;
                    };
                    let resolve = query.sensor().limits(band).unwrap().resolve();
                    let to = bearing(observer, &site).unwrap();
                    seen.push(Seen {
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
                    });
                }
            }
        }
    }
    seen.sort();
    seen
}
