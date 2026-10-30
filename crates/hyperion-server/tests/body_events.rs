//! A system's events over a real socket (plan 14, P14.T36.b): pinned systems' events against the
//! sim's own `events_between`, the window's and the ID's refusals, and the cache of event models.
//!
//! P14.T31.c's events are held until generator version 25, below which `body_events` is answered
//! `unsupported`, as `system_bodies.rs` checks. So every test of this binary first puts them on the
//! record for the whole process ([`wire_body_events_in_this_process`]): the server answers on its
//! pool's threads, which no thread's switch reaches. The other 25 holds stay held, so the events read version 24's
//! records, in the server and in the sim's systems the tests hold the answers to alike.

mod common;

use common::bodies::{
    PINNED, SECONDS_PER_JULIAN_YEAR, at_years, galaxy, joined, refused, sim_system, sim_time,
};
use common::{TestClient, TestServer};
use hyperion_protocol::{
    BodyEventDto, BodyEventsDto, BodyEventsRequest, BodyIdHex, CometEventDto, ErrorCode,
    OrbitHostDto, RequestBody, ResponseBody, StormBandDto, SystemIdHex, UniverseIdHex,
    UniverseTime,
};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, candidate_count};
use hyperion_sim::id::SystemId;
use hyperion_sim::orbit::Orbit;
use hyperion_sim::planetary::events::{BodyEvent, StormBand};
use hyperion_sim::planetary::placement::OrbitHost;
use hyperion_sim::planetary::testing::wire_body_events_in_this_process;
use hyperion_sim::planetary::{SystemContext, generate};
use hyperion_sim::{Seed, time};

/// The seed of the sim's golden systems (`tests/planetary_golden.rs`'s `SYSTEMS_SEED`), whose
/// solar-like system has a cometary halo.
const GOLDEN_SYSTEMS_SEED: u64 = 0x5eed_0000_0014_0032;

fn events_request(
    universe: &UniverseIdHex,
    system: u64,
    from: UniverseTime,
    to: UniverseTime,
) -> RequestBody {
    RequestBody::BodyEvents(BodyEventsRequest {
        universe: universe.clone(),
        system: SystemIdHex::from_u64(system),
        from,
        to,
    })
}

async fn events(client: &mut TestClient, body: RequestBody) -> BodyEventsDto {
    match client.request(body).await {
        Ok(ResponseBody::BodyEvents(answer)) => answer,
        other => panic!("expected a system's events, got {other:?}"),
    }
}

/// What a comet goes round, as the wire says it.
fn host_dto(host: OrbitHost) -> OrbitHostDto {
    match host {
        OrbitHost::Star(n) => OrbitHostDto::Star {
            body_index: u16::from(n),
        },
        OrbitHost::Pair(k) => OrbitHostDto::Pair {
            key_body_index: u16::from(k),
        },
        OrbitHost::Barycentre => OrbitHostDto::Barycentre,
        OrbitHost::Body(_) => panic!("a halo goes round a star, a pair or the barycentre"),
    }
}

/// Holds the wire's `event` to the sim's `sim`, of the system `raw` whose halo goes round
/// `halo_host`, to the bit.
fn assert_event_is_the_sims(
    raw: u64,
    halo_host: Option<OrbitHost>,
    wire: &BodyEventDto,
    sim: &BodyEvent,
) {
    let body = BodyIdHex::from_parts(raw, sim.body().get());
    let id = sim.id().to_string();
    let time = |t: time::UniverseTime| UniverseTime {
        seconds: t.seconds(),
        nanos: t.subsec_nanos(),
    };
    match (wire, sim) {
        (
            BodyEventDto::Impact {
                id: wire_id,
                body: wire_body,
                time: at,
                projectile_diameter_m,
                energy_j,
                impact_velocity_m_per_s,
                latitude_rad,
                longitude_rad,
            },
            BodyEvent::Impact { impact, .. },
        ) => {
            assert_eq!((wire_id.as_str(), wire_body), (id.as_str(), &body));
            assert_eq!(*at, time(impact.time()));
            assert_eq!(
                [
                    *projectile_diameter_m,
                    *energy_j,
                    *impact_velocity_m_per_s,
                    *latitude_rad,
                    *longitude_rad,
                ],
                [
                    impact.diameter().value(),
                    impact.energy_joules(),
                    impact.velocity().value(),
                    impact.site().latitude().value(),
                    impact.site().longitude().value(),
                ],
                "{id}"
            );
        }
        (
            BodyEventDto::Eruption {
                id: wire_id,
                body: wire_body,
                onset,
                duration_s,
            },
            BodyEvent::Eruption { eruption, .. },
        ) => {
            assert_eq!((wire_id.as_str(), wire_body), (id.as_str(), &body));
            assert_eq!(*onset, time(eruption.onset()));
            assert_eq!(*duration_s, eruption.duration().as_seconds_f64(), "{id}");
        }
        (
            BodyEventDto::Storm {
                id: wire_id,
                body: wire_body,
                onset,
                duration_s,
                band,
            },
            BodyEvent::Storm { storm, .. },
        ) => {
            assert_eq!((wire_id.as_str(), wire_body), (id.as_str(), &body));
            assert_eq!(*onset, time(storm.onset()));
            assert_eq!(*duration_s, storm.duration().as_seconds_f64(), "{id}");
            let expected = match storm.band() {
                StormBand::Equatorial => StormBandDto::Equatorial,
                StormBand::MidLatitude => StormBandDto::MidLatitude,
                StormBand::Polar => StormBandDto::Polar,
            };
            assert_eq!(*band, expected, "{id}");
        }
        (
            BodyEventDto::DustStorm {
                id: wire_id,
                body: wire_body,
                onset,
                duration_s,
                onset_true_anomaly_rad,
            },
            BodyEvent::DustStorm { dust_storm, .. },
        ) => {
            assert_eq!((wire_id.as_str(), wire_body), (id.as_str(), &body));
            assert_eq!(*onset, time(dust_storm.onset()));
            assert_eq!(
                [*duration_s, *onset_true_anomaly_rad],
                [
                    dust_storm.duration().as_seconds_f64(),
                    dust_storm.onset_true_anomaly().value()
                ],
                "{id}"
            );
        }
        (BodyEventDto::Comet(wire), BodyEvent::Comet { comet, .. }) => {
            assert_eq!((wire.id.as_str(), &wire.halo), (id.as_str(), &body));
            assert_comet_is_the_sims(halo_host, wire, comet);
        }
        (wire, sim) => panic!("the wire's {wire:?} is not the sim's {sim:?}"),
    }
}

/// Holds the wire's comet `wire` to the sim's `comet`, whose halo goes round `halo_host`, to the
/// bit, with its track.
fn assert_comet_is_the_sims(
    halo_host: Option<OrbitHost>,
    wire: &CometEventDto,
    comet: &hyperion_sim::planetary::events::Comet,
) {
    let id = wire.id.as_str();
    let time = |t: time::UniverseTime| UniverseTime {
        seconds: t.seconds(),
        nanos: t.subsec_nanos(),
    };
    assert_eq!(
        wire.host,
        host_dto(halo_host.expect("a system with comets has a halo"))
    );
    assert_eq!(wire.perihelion_time, time(comet.perihelion_time()));
    let mu = match comet.orbit() {
        Orbit::Bound(elements) => elements.gravitational_parameter(),
        Orbit::Open(open) => open.gravitational_parameter(),
    };
    assert_eq!(
        [
            wire.perihelion_m,
            wire.eccentricity,
            wire.mu_m3_s2,
            wire.nucleus_diameter_m
        ],
        [
            comet.perihelion().value(),
            comet.eccentricity(),
            mu.value(),
            comet.nucleus_diameter().value()
        ],
        "{id}"
    );
    assert_eq!(
        (wire.listed_from, wire.listed_to, wire.plunges),
        (
            time(comet.listed().start()),
            time(comet.listed().end()),
            comet.plunges()
        )
    );
    // The track runs over the listing in time order, each point the comet's own position.
    assert!(!wire.track.is_empty() && wire.track.len() <= 33, "{id}");
    assert_eq!(
        wire.track.first().map(|point| point.time),
        Some(wire.listed_from)
    );
    assert_eq!(
        wire.track.last().map(|point| point.time),
        Some(wire.listed_to)
    );
    assert!(
        wire.track
            .windows(2)
            .all(|pair| pair[0].time < pair[1].time)
    );
    for point in &wire.track {
        assert_eq!(
            point.position_m,
            comet.position_at(sim_time(point.time)).metres(),
            "{id}"
        );
    }
}

/// The most events an answer lists (the server's `MAX_LISTED_EVENTS`).
const MAX_LISTED_EVENTS: usize = 25_000;

/// Holds `answer`'s listed span inside its window and about its middle, as far either side where
/// the window allows, and its events inside the span, by time, no more than an answer lists.
fn assert_listed_about_the_middle(answer: &BodyEventsDto) {
    let nanos = |t: UniverseTime| i128::from(t.seconds) * 1_000_000_000 + i128::from(t.nanos);
    let (from, to) = (nanos(answer.from), nanos(answer.to));
    let (start, end) = (nanos(answer.listed_from), nanos(answer.listed_to));
    assert!(from <= start && start <= end && end <= to, "{answer:?}");
    let middle = from + (to - from) / 2;
    assert!(
        start <= middle && middle <= end,
        "the span holds the middle"
    );
    if from < start && end < to {
        assert_eq!(middle - start, end - middle, "as far either side");
    }
    assert!(answer.events.len() <= MAX_LISTED_EVENTS);
    let time = |event: &BodyEventDto| match event {
        BodyEventDto::Impact { time, .. } => nanos(*time),
        BodyEventDto::Eruption { onset, .. }
        | BodyEventDto::Storm { onset, .. }
        | BodyEventDto::DustStorm { onset, .. } => nanos(*onset),
        BodyEventDto::Comet(comet) => nanos(comet.perihelion_time),
    };
    assert!(
        answer
            .events
            .iter()
            .all(|event| (start..end).contains(&time(event)))
    );
    assert!(
        answer
            .events
            .windows(2)
            .all(|pair| time(&pair[0]) <= time(&pair[1]))
    );
}

/// A system's events and the cache on one server: pinned systems' centuries against the sim's,
/// a repeat and another window from the cached models, and no event of a body gone before the
/// epoch.
///
/// Nextest runs each test in its own process, and each server builds its galaxy, so these share
/// one server, as `system_bodies.rs`'s do. The cache's counts come first.
#[tokio::test]
async fn a_systems_events_are_the_sims_and_cached() {
    wire_body_events_in_this_process();
    let server = TestServer::start().await;
    let (mut client, universe) = joined(&server, "Kepler Reach").await;
    let (from, to) = (at_years(-50), at_years(50));

    // The first asks generate the system and build its models once; a repeat reads both.
    let raw = PINNED[0];
    let first = events(&mut client, events_request(&universe, raw, from, to)).await;
    let counters = server.stats().bodies();
    assert_eq!((counters.generated(), counters.modelled()), (1, 1));
    let again = events(&mut client, events_request(&universe, raw, from, to)).await;
    assert_eq!(again, first);
    let counters = server.stats().bodies();
    assert_eq!((counters.generated(), counters.modelled()), (1, 1));
    assert_eq!(counters.events().hits(), 1);
    assert!(counters.events().bytes() <= counters.events().budget());

    // Every pinned system's century, and a later decade, are the sim's to the bit.
    for raw in PINNED {
        let (ctx, planets) = sim_system(raw);
        for (from, to) in [(from, to), (at_years(500), at_years(510))] {
            let answer = events(&mut client, events_request(&universe, raw, from, to)).await;
            assert_eq!(
                (answer.system.to_u64(), answer.from, answer.to),
                (raw, from, to)
            );
            assert_listed_about_the_middle(&answer);
            // The listed span's events are the sim's, every one; a window that holds no more
            // than an answer lists is listed whole.
            let sim = planets.events_between(
                &ctx,
                sim_time(answer.listed_from),
                sim_time(answer.listed_to),
            );
            assert_eq!(answer.events.len(), sim.len(), "{raw:#018x}");
            let whole = planets.events_between(&ctx, sim_time(from), sim_time(to));
            if whole.len() <= MAX_LISTED_EVENTS {
                assert_eq!((answer.listed_from, answer.listed_to), (from, to));
            }
            for (wire, sim) in answer.events.iter().zip(&sim) {
                assert_event_is_the_sims(raw, planets.halo_host(), wire, sim);
            }
        }
    }
    // The first system's events are its rocky planet's and its giant's, more in a century than an
    // answer lists; the triple's planet engulfed before the epoch has none.
    assert_eq!(first.events.len().min(1), 1);
    assert_ne!((first.listed_from, first.listed_to), (from, to));
    let (ctx, planets) = sim_system(PINNED[2]);
    let snapshot = planets.snapshot_at(&ctx, time::UniverseTime::EPOCH);
    let gone: Vec<BodyIdHex> = snapshot
        .bodies()
        .iter()
        .filter(|record| {
            record.identity().state() != hyperion_sim::planetary::fate::BodyState::Present
        })
        .map(|record| BodyIdHex::from_parts(PINNED[2], record.index().get()))
        .collect();
    assert!(
        !gone.is_empty(),
        "the triple has a body gone before the epoch"
    );
    let triple = events(&mut client, events_request(&universe, PINNED[2], from, to)).await;
    assert!(triple.events.iter().all(|event| {
        let body = match event {
            BodyEventDto::Impact { body, .. }
            | BodyEventDto::Eruption { body, .. }
            | BodyEventDto::Storm { body, .. }
            | BodyEventDto::DustStorm { body, .. } => body,
            BodyEventDto::Comet(comet) => &comet.halo,
        };
        !gone.contains(body)
    }));
    // The golden solar-like system of the sim's goldens, in a universe of their seed, sends in
    // comets from its halo, each with its orbit and track.
    let golden = 0x4200_6cba_0000_0009;
    let seeded = client
        .create_universe("Golden Reach", GOLDEN_SYSTEMS_SEED)
        .await
        .id;
    let answer = events(
        &mut client,
        events_request(&seeded, golden, at_years(-5), at_years(5)),
    )
    .await;
    let comets = answer
        .events
        .iter()
        .filter(|event| matches!(event, BodyEventDto::Comet(_)))
        .count();
    assert!(comets > 0, "{} events, no comet", answer.events.len());
    let galaxy = Galaxy::new(Seed::new(GOLDEN_SYSTEMS_SEED));
    let id = SystemId::from_raw(golden).expect("a pinned ID is a system ID");
    let ctx = SystemContext::for_system(&galaxy, id).expect("a pinned ID names a system");
    let planets = generate(Seed::new(GOLDEN_SYSTEMS_SEED), &ctx);
    let sim = planets.events_between(
        &ctx,
        sim_time(answer.listed_from),
        sim_time(answer.listed_to),
    );
    assert_eq!(answer.events.len(), sim.len());
    for (wire, sim) in answer.events.iter().zip(&sim) {
        assert_event_is_the_sims(golden, planets.halo_host(), wire, sim);
    }
    client.close().await;
    server.stop().await;
}

/// The window's refusals name their field, in the order they are checked, and a system ID that
/// names no system is `unknown_system` naming `system`.
#[tokio::test]
async fn a_bad_window_or_an_unknown_system_is_refused_naming_the_field() {
    wire_body_events_in_this_process();
    let server = TestServer::start().await;
    let (mut client, universe) = joined(&server, "Kepler Reach").await;
    let pinned = PINNED[0];
    let beyond_window = at_years(1_001);
    for (from, to, field) in [
        // An end outside the clock window, the start first.
        (beyond_window, at_years(0), "from"),
        (at_years(0), beyond_window, "to"),
        // A window that ends before it starts.
        (at_years(10), at_years(-10), "to"),
        // Longer than 1,000 Julian years, by a second.
        (
            at_years(-500),
            UniverseTime {
                seconds: 500 * SECONDS_PER_JULIAN_YEAR + 1,
                nanos: 0,
            },
            "to",
        ),
    ] {
        let error = refused(&mut client, events_request(&universe, pinned, from, to)).await;
        assert_eq!(error.code, ErrorCode::BadRequest, "{error:?}");
        assert_eq!(error.field.as_deref(), Some(field), "{error:?}");
    }
    // A window of exactly 1,000 years is served, about its middle, and so is an empty one.
    let longest = events(
        &mut client,
        events_request(&universe, pinned, at_years(-500), at_years(500)),
    )
    .await;
    assert_listed_about_the_middle(&longest);
    assert_eq!(longest.events.len().min(1), 1);
    let empty = events(
        &mut client,
        events_request(&universe, pinned, at_years(3), at_years(3)),
    )
    .await;
    assert_eq!(empty.events, Vec::new());

    // Bits that are no system ID, and the index after a cell's last candidate.
    let cell = CellKey::of(SystemId::from_raw(pinned).expect("a system ID")).expect("a grid ID");
    let beyond = cell
        .candidate_id(candidate_count(galaxy(), cell))
        .expect("a cell at the solar circle has room for another index")
        .raw();
    for raw in [u64::MAX, beyond] {
        let error = refused(
            &mut client,
            events_request(&universe, raw, at_years(0), at_years(1)),
        )
        .await;
        assert_eq!(
            error.code,
            ErrorCode::UnknownSystem,
            "{raw:#018x}: {error:?}"
        );
        assert_eq!(error.field.as_deref(), Some("system"), "{error:?}");
    }
    // Nothing refused was modelled.
    assert_eq!(server.stats().bodies().modelled(), 1);
    client.close().await;
    server.stop().await;
}
