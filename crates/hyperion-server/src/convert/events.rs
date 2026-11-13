//! The `body_events` request and its answer (plan 14, P14.T36.b): the checks of the request's
//! window, and the wire form of a system's events (P14.T31's `BodyEvent`).
//!
//! The events are the sim's ([`SystemEvents::between`], the merge of P14.T31.c's
//! `events_between`), each converted as it is: nothing is added, dropped or reordered here. A
//! comet's track, which the wire carries because a client does not propagate an open orbit, is
//! sampled from the comet's own orbit ([`COMET_TRACK_POINTS`]).

use hyperion_protocol::{
    BodyEventDto, BodyEventsDto, BodyEventsRequest, BodyIdHex, CometEventDto, CometTrackPointDto,
    EventIdHex, OrbitHostDto, RequestError, StormBandDto,
};
use hyperion_sim::events::TimeWindow;
use hyperion_sim::id::{EventId, SystemId};
use hyperion_sim::orbit::Orbit;
use hyperion_sim::planetary::PlanetarySystem;
use hyperion_sim::planetary::events::{BodyEvent, Comet, StormBand};
use hyperion_sim::planetary::placement::OrbitHost;
use hyperion_sim::planetary::system::SystemEvents;
use hyperion_sim::time::{Span, UniverseTime};

use super::planetary::{body_id, orbit_host};
use super::{ConvertRequestError, clock_time, unknown_system, wire_time};

/// The longest window a `body_events` request may ask about: 1,000 Julian years (plan 14,
/// P14.T36.b). A young system lists up to about 2 × 10⁵ events a century (P14.T31.c's golden
/// `subgiant`), so even this window's answer can run to hundreds of megabytes.
pub(crate) const MAX_EVENTS_WINDOW: Span = Span::from_seconds(1_000 * 31_557_600);

/// The most events one answer lists: 25,000, some 8 MB of JSON at an impact's 330 bytes, inside
/// the 16 MiB a connection queues ([`OUTBOUND_BYTES`](crate::limits::OUTBOUND_BYTES)). A comet,
/// with its 33-point track, takes about 4 KB, but a halo sends in no more than 64 a year, so a
/// list of mostly comets is a 1,000-year window's of a halo at that rate, about 100 MB, which is
/// still sent, as any frame larger than the queue is.
///
/// P14.T31's rates fill a window fast: a giant's impacts are counted above the energy at which a
/// year holds 64, and a young rocky planet's eruptions saturate at 64 a week, so a century of a
/// young system lists 10⁴–10⁵ events, some 34 MB of JSON for one pinned Sun-like system, and a
/// 1,000-year window several hundred. A window that holds more is listed over the widest span
/// about its middle that holds no more ([`SystemEvents::listed_about_middle`]).
pub(crate) const MAX_LISTED_EVENTS: usize = 25_000;

/// The points of a comet's track on each side of its perihelion ([`comet_track`]).
const TRACK_SIDE: u32 = 16;

/// The most points a comet's track holds: [`TRACK_SIDE`] either side of its perihelion and the
/// perihelion itself, closer together about it ([`comet_track`]).
pub(crate) const COMET_TRACK_POINTS: usize = 33;

const _: () = assert!(COMET_TRACK_POINTS == 2 * TRACK_SIDE as usize + 1);

/// A `body_events` request, checked: the system it names, decoded, and its window inside the
/// clock window and at most [`MAX_EVENTS_WINDOW`] long.
///
/// The checks run in the order of the survey pass's, whose window is the same kind (rendering plan
/// R09): the universe before this, then `from` and `to`, each inside the clock window
/// (`bad_request` naming it), then `to` no earlier than `from` and the window no longer than
/// [`MAX_EVENTS_WINDOW`] (`bad_request` naming `to`), then `system`, whose bits must be a system
/// ID (`unknown_system`); the ID is resolved on the pool job that generates the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct EventsRequest {
    system: SystemId,
    window: TimeWindow,
}

impl EventsRequest {
    /// The system asked about, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn system(self) -> SystemId {
        self.system
    }

    /// The window asked about, `from` inclusive and `to` exclusive.
    #[must_use]
    pub(crate) fn window(self) -> TimeWindow {
        self.window
    }
}

impl TryFrom<&BodyEventsRequest> for EventsRequest {
    type Error = RequestError;

    /// Checks `from` and `to`, then the window, then decodes `system`.
    fn try_from(request: &BodyEventsRequest) -> Result<Self, Self::Error> {
        let from = clock_time("from", &request.from)?;
        let to = clock_time("to", &request.to)?;
        let window = TimeWindow::new(from, to).map_err(|_| {
            ConvertRequestError::new("to", format!("{to} is before the window's start, {from}"))
        })?;
        let longest = UniverseTime::checked_add(from, MAX_EVENTS_WINDOW);
        if longest.is_some_and(|longest| to > longest) {
            return Err(ConvertRequestError::new(
                "to",
                format!(
                    "the window from {from} to {to} is longer than the {} Julian years a request \
                     may ask about: ask for a longer span as several requests",
                    MAX_EVENTS_WINDOW.as_julian_years_f64()
                ),
            )
            .into());
        }
        let system = SystemId::from_raw(request.system.to_u64())
            .map_err(|error| unknown_system(&request.system, error))?;
        Ok(Self { system, window })
    }
}

/// The `body_events` answer to `request`, checked as `wanted`: the events of the system
/// `planets`, whose models are `models`, in its window, or in the span about its middle that
/// [`SystemEvents::listed_about_middle`] gives under [`MAX_LISTED_EVENTS`], in the sim's order, by
/// time and then by event ID.
#[must_use]
pub(crate) fn body_events(
    request: BodyEventsRequest,
    wanted: EventsRequest,
    planets: &PlanetarySystem,
    models: &SystemEvents,
) -> BodyEventsDto {
    let system = wanted.system();
    // A comet's host is its halo's, which a system with comets has.
    let halo_host = planets.halo_host();
    let listed = models.listed_about_middle(wanted.window(), MAX_LISTED_EVENTS);
    BodyEventsDto {
        universe: request.universe,
        system: request.system,
        from: request.from,
        to: request.to,
        listed_from: wire_time(listed.span().start()),
        listed_to: wire_time(listed.span().end()),
        events: listed
            .events()
            .iter()
            .map(|event| event_dto(system, halo_host, event))
            .collect(),
    }
}

/// Nanoseconds in a second.
const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// `span` in whole nanoseconds.
#[must_use]
fn nanos_of(span: Span) -> i128 {
    i128::from(span.seconds()) * NANOS_PER_SECOND + i128::from(span.subsec_nanos())
}

/// The span of `nanos` nanoseconds, not negative.
///
/// # Panics
///
/// Never for a part of a window, which the clock window bounds.
#[must_use]
fn span_of_nanos(nanos: i128) -> Span {
    let seconds = i64::try_from(nanos.div_euclid(NANOS_PER_SECOND))
        .expect("a part of a window is a span of the clock");
    let nanos = u32::try_from(nanos.rem_euclid(NANOS_PER_SECOND))
        .expect("a remainder of a second is below 10⁹");
    Span::new(seconds, nanos).expect("whole seconds and a remainder below a second are a span")
}

/// `event`, an event of the system `system` whose halo goes round `halo_host`, as the wire says it.
///
/// # Panics
///
/// If `event` is a comet and the system has no halo, which no system's models give.
#[must_use]
fn event_dto(system: SystemId, halo_host: Option<OrbitHost>, event: &BodyEvent) -> BodyEventDto {
    let id = event_id(event.id());
    let body = body_id(system, event.body());
    match event {
        BodyEvent::Impact { impact, .. } => BodyEventDto::Impact {
            id,
            body,
            time: wire_time(impact.time()),
            projectile_diameter_m: impact.diameter().value(),
            energy_j: impact.energy_joules(),
            impact_velocity_m_per_s: impact.velocity().value(),
            latitude_rad: impact.site().latitude().value(),
            longitude_rad: impact.site().longitude().value(),
        },
        BodyEvent::Eruption { eruption, .. } => BodyEventDto::Eruption {
            id,
            body,
            onset: wire_time(eruption.onset()),
            duration_s: eruption.duration().as_seconds_f64(),
        },
        BodyEvent::Storm { storm, .. } => BodyEventDto::Storm {
            id,
            body,
            onset: wire_time(storm.onset()),
            duration_s: storm.duration().as_seconds_f64(),
            band: storm_band(storm.band()),
        },
        BodyEvent::DustStorm { dust_storm, .. } => BodyEventDto::DustStorm {
            id,
            body,
            onset: wire_time(dust_storm.onset()),
            duration_s: dust_storm.duration().as_seconds_f64(),
            onset_true_anomaly_rad: dust_storm.onset_true_anomaly().value(),
        },
        BodyEvent::Comet { comet, .. } => {
            let host = halo_host.expect("a system whose models list comets has a halo");
            BodyEventDto::Comet(Box::new(comet_dto(
                id,
                body,
                orbit_host(system, host),
                comet,
            )))
        }
    }
}

/// An event's ID on the wire: plan 01's text form.
///
/// # Panics
///
/// Never: plan 01's text form of an event ID is the wire's.
#[must_use]
fn event_id(id: EventId) -> EventIdHex {
    EventIdHex::try_from(id.to_string()).expect("an event ID's text form is the wire's")
}

/// A storm's band as the wire says it.
#[must_use]
fn storm_band(band: StormBand) -> StormBandDto {
    match band {
        StormBand::Equatorial => StormBandDto::Equatorial,
        StormBand::MidLatitude => StormBandDto::MidLatitude,
        StormBand::Polar => StormBandDto::Polar,
    }
}

/// `comet`, of event ID `id` from the halo `halo` about `host`, as the wire says it, with its
/// sampled track.
#[must_use]
fn comet_dto(id: EventIdHex, halo: BodyIdHex, host: OrbitHostDto, comet: &Comet) -> CometEventDto {
    let orientation = comet.orientation();
    let mu = match comet.orbit() {
        Orbit::Bound(elements) => elements.gravitational_parameter(),
        Orbit::Open(open) => open.gravitational_parameter(),
    };
    CometEventDto {
        id,
        halo,
        host,
        perihelion_time: wire_time(comet.perihelion_time()),
        perihelion_m: comet.perihelion().value(),
        eccentricity: comet.eccentricity(),
        inclination_rad: orientation.inclination().value(),
        ascending_node_rad: orientation.ascending_node().value(),
        argument_of_periapsis_rad: orientation.argument_of_periapsis().value(),
        mu_m3_s2: mu.value(),
        nucleus_diameter_m: comet.nucleus_diameter().value(),
        listed_from: wire_time(comet.listed().start()),
        listed_to: wire_time(comet.listed().end()),
        plunges: comet.plunges(),
        track: comet_track(comet)
            .into_iter()
            .map(|t| CometTrackPointDto {
                time: wire_time(t),
                position_m: comet.position_at(t).metres(),
            })
            .collect(),
    }
}

/// The instants of `comet`'s track, in time order: its listing's start and end, its perihelion
/// held inside the listing (a plunging comet's listing ends before it), and 15 more on each side,
/// placed by the square of their rank from the perihelion, so that they crowd about it, where the
/// comet moves fastest. A side of the listing that holds no time gives no points beyond the
/// perihelion's, so a track has at most [`COMET_TRACK_POINTS`] points.
#[must_use]
fn comet_track(comet: &Comet) -> Vec<UniverseTime> {
    let listed = comet.listed();
    let centre = comet.perihelion_time().clamp(listed.start(), listed.end());
    let before = centre.checked_since(listed.start()).unwrap_or_default();
    let after = listed.end().checked_since(centre).unwrap_or_default();
    // The share of a side's span at rank k of `TRACK_SIDE`, (k ÷ TRACK_SIDE)², in whole
    // nanoseconds, so that the last point is the listing's end exactly.
    let at = |span: Span, k: u32| {
        span_of_nanos(nanos_of(span) * i128::from(k * k) / i128::from(TRACK_SIDE * TRACK_SIDE))
    };
    let mut times = Vec::with_capacity(COMET_TRACK_POINTS);
    if before > Span::default() {
        times.extend(
            (1..=TRACK_SIDE)
                .rev()
                .filter_map(|k| centre.checked_sub(at(before, k))),
        );
    }
    times.push(centre);
    if after > Span::default() {
        times.extend((1..=TRACK_SIDE).filter_map(|k| centre.checked_add(at(after, k))));
    }
    times
}
