//! Plan 12's query modes over a real socket (P12.T6): a range query asked `now` and observed from a
//! point finds the same systems, and only the observed answer carries `observed`, which is the
//! sim's own reading of each row at its retarded time; `resolve_system` answers one of those rows
//! by its ID in either mode; and the fields it cannot use are named.

mod common;

use std::num::NonZeroU32;
use std::sync::OnceLock;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    ErrorCode, GalacticPosition, MassLayer, ObjectKindDto, ObservedDto, QueryModeDto, RequestBody,
    RequestError, ResolveSystemRequest, ResolvedSystem, ResponseBody, StellarBriefDto, SystemIdHex,
    SystemRecord, SystemsInRange, SystemsInRangeRequest, UniverseIdHex, UniverseTime,
};
use hyperion_server::limits::MAX_QUERY_CELLS;
use hyperion_sim::coords::LyCell;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::NoCache;
use hyperion_sim::galaxy::query::{QueryMode, RangeQuery, RangeResult, range_query_observed};
use hyperion_sim::id::{CentreMemberId, SystemId};
use hyperion_sim::observe::NoStarsCache;
use hyperion_sim::stellar::ObjectKind;
use hyperion_sim::stellar::system::StellarBrief;
use hyperion_sim::units::LightYears;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::{GENERATOR_VERSION, Seed};

/// The seed of every universe these tests create, the other server tests' own.
const SEED: u64 = 0x4d2;

/// Seconds in a Julian year, the sim's year.
const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

/// The galaxy of [`SEED`] as the server builds it, with its full potential and so its velocities.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)).with_full_potential())
}

/// The centre of a light-year cell, as the wire carries a position.
fn at_cell(cell_ly: [i32; 3]) -> GalacticPosition {
    GalacticPosition {
        cell_ly,
        offset_m: [0.0; 3],
    }
}

/// The instant `years` Julian years from the epoch.
fn at_years(years: i64) -> UniverseTime {
    UniverseTime {
        seconds: years * SECONDS_PER_JULIAN_YEAR,
        nanos: 0,
    }
}

/// The Sun-like point, in the plane 26,000 ly out on the +y axis.
fn sunlike() -> GalacticPosition {
    at_cell([0, 26_000, 0])
}

/// An observer 3,000 ly coreward of the Sun-like point.
fn coreward() -> QueryModeDto {
    QueryModeDto::Observed {
        observer: at_cell([0, 23_000, 0]),
    }
}

/// Twelve light-years of the Sun-like point, a century after the epoch, with briefs, in `mode`.
fn sphere(universe: &UniverseIdHex, mode: QueryModeDto) -> SystemsInRangeRequest {
    SystemsInRangeRequest {
        universe: universe.clone(),
        centre: sunlike(),
        radius_ly: 12.0,
        time: at_years(100),
        min_layer: MassLayer::A,
        limit: 5_000,
        include_stellar: true,
        mode,
    }
}

async fn ask(
    client: &mut TestClient,
    request: SystemsInRangeRequest,
) -> Result<SystemsInRange, RequestError> {
    client
        .request(RequestBody::SystemsInRange(request))
        .await
        .map(|response| match response {
            ResponseBody::SystemsInRange(answer) => answer,
            other => panic!("expected the systems in range, got {other:?}"),
        })
}

async fn resolve_one(
    client: &mut TestClient,
    request: ResolveSystemRequest,
) -> Result<ResolvedSystem, RequestError> {
    client
        .request(RequestBody::ResolveSystem(request))
        .await
        .map(|response| match response {
            ResponseBody::ResolveSystem(answer) => *answer,
            other => panic!("expected the resolved system, got {other:?}"),
        })
}

/// The sim's own observed answer to `request`, over a galaxy built here and no cache of cells or
/// stars.
fn sim_observed(request: &SystemsInRangeRequest) -> RangeResult {
    let position = |wire: &GalacticPosition| {
        hyperion_sim::coords::GalacticPosition::new(LyCell::new(wire.cell_ly), wire.offset_m)
            .expect("the test's positions are canonical")
    };
    let QueryModeDto::Observed { observer } = &request.mode else {
        panic!("an observed request")
    };
    let query = RangeQuery::builder(
        position(&request.centre),
        LightYears::new(request.radius_ly),
    )
    .time(
        hyperion_sim::time::UniverseTime::new(request.time.seconds, request.time.nanos)
            .expect("a well-formed time"),
    )
    .limit(NonZeroU32::new(request.limit).expect("a limit above zero"))
    .cell_budget(MAX_QUERY_CELLS)
    .mode(QueryMode::ObservedFrom(position(observer)))
    .build()
    .expect("the test's query is valid");
    range_query_observed(
        galaxy(),
        &mut NoCache::new(),
        &mut NoStarsCache,
        &[],
        &query,
    )
    .expect("the grid places no centre member")
}

/// Whether `a` and `b` agree to a relative `1e-15`, since `serde_json`'s own parser can read a
/// float a last bit out.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-15 * a.abs().max(b.abs())
}

fn wire_kind(kind: ObjectKind) -> ObjectKindDto {
    match kind {
        ObjectKind::Protostar => ObjectKindDto::Protostar,
        ObjectKind::PreMainSequence => ObjectKindDto::PreMainSequence,
        ObjectKind::Dwarf => ObjectKindDto::Dwarf,
        ObjectKind::Subgiant => ObjectKindDto::Subgiant,
        ObjectKind::Giant => ObjectKindDto::Giant,
        ObjectKind::Supergiant => ObjectKindDto::Supergiant,
        ObjectKind::WolfRayet => ObjectKindDto::WolfRayet,
        ObjectKind::HotSubdwarf => ObjectKindDto::HotSubdwarf,
        ObjectKind::WhiteDwarf => ObjectKindDto::WhiteDwarf,
        ObjectKind::NeutronStar => ObjectKindDto::NeutronStar,
        ObjectKind::BlackHole => ObjectKindDto::BlackHole,
        ObjectKind::NoRemnant => ObjectKindDto::NoRemnant,
        ObjectKind::Substellar => ObjectKindDto::Substellar,
    }
}

/// The wire's brief of the sim's, as the server writes it: f32s rounded from the sim's f64s.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the wire's f32s are the sim's f64s rounded, which is what is checked"
)]
fn wire_brief(brief: &StellarBrief) -> StellarBriefDto {
    let lit = brief.log_luminosity();
    StellarBriefDto {
        kind: wire_kind(brief.kind()),
        class: brief.class().to_string(),
        log_luminosity_lsun: lit.map(|l| l.value() as f32),
        teff_k: lit.map(|_| brief.effective_temperature().value() as f32),
        star_count: brief.star_count(),
    }
}

/// Holds a wire observation to the sim's.
fn assert_observed_is_the_sims(
    wire: &ObservedDto,
    sim: &hyperion_sim::observe::ObservedSystem,
    what: &str,
) {
    let r = sim.retardation();
    assert_eq!(
        (wire.emitted.seconds, wire.emitted.nanos),
        (r.emitted().seconds(), r.emitted().subsec_nanos()),
        "{what}"
    );
    assert!(
        close(wire.light_age_yr, r.light_age().as_julian_years_f64()),
        "{what}"
    );
    assert_eq!(
        wire.apparent_position.cell_ly,
        r.apparent_position().cell().to_array(),
        "{what}"
    );
    for (wire, sim) in wire
        .apparent_position
        .offset_m
        .iter()
        .zip(r.apparent_position().offset_metres())
    {
        assert!(close(*wire, sim), "{what}: offset {wire} against {sim}");
    }
    assert!(
        close(wire.curvature_error_ly, sim.error().length().value()),
        "{what}"
    );
    assert!(
        close(wire.curvature_error_arcsec, sim.error().angle().value()),
        "{what}"
    );
}

/// P12.T6: the same sphere asked in both modes gets the same IDs, census and present positions,
/// with `observed` only in the observed answer, where each row is the sim's own
/// `range_query_observed` row, its brief the primary's when the light left it; a repeat of the
/// observed query builds no star, since the server lends its system cache as the stars cache.
#[tokio::test]
async fn one_sphere_asked_in_both_modes_finds_the_same_systems_and_only_one_is_observed() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Parallax", SEED).await;

    let now = ask(&mut client, sphere(&universe.id, QueryModeDto::Now))
        .await
        .unwrap();
    let request = sphere(&universe.id, coreward());
    let seen = ask(&mut client, request.clone()).await.unwrap();
    assert!(now.systems.len() > 10, "{} systems", now.systems.len());
    assert_eq!(seen.census, now.census);
    assert_eq!(
        (&seen.universe, seen.centre, seen.radius_ly, seen.time),
        (&now.universe, now.centre, now.radius_ly, now.time)
    );
    assert_eq!(seen.systems.len(), now.systems.len());
    for (observed, present) in seen.systems.iter().zip(&now.systems) {
        assert_eq!(observed.id, present.id, "the same IDs in the same order");
        assert!(present.observed.is_none(), "{present:?}");
        assert!(observed.observed.is_some(), "{observed:?}");
        // Everything but the observation and the brief is the present's.
        assert_eq!(
            SystemRecord {
                stellar: None,
                observed: None,
                ..observed.clone()
            },
            SystemRecord {
                stellar: None,
                ..present.clone()
            }
        );
    }

    let sim = sim_observed(&request);
    assert_eq!(sim.observed().len(), seen.systems.len());
    for (row, sim_row) in seen.systems.iter().zip(sim.observed()) {
        assert_eq!(row.id.to_u64(), sim_row.hit().id().raw());
        let observed = row.observed.as_ref().expect("an observed row");
        assert_observed_is_the_sims(observed, sim_row, &row.designation);
        // 3,000 ly away, give or take the sphere and the century's drift.
        assert!(
            (observed.light_age_yr - 3_000.0).abs() < 15.0,
            "{}: {} yr",
            row.designation,
            observed.light_age_yr
        );
        assert_eq!(
            row.stellar,
            sim_row.brief_then().as_ref().map(wire_brief),
            "{}",
            row.designation
        );
    }

    // The observed rows' stars are the system cache's now: asking again builds none.
    let before = server.stats().systems();
    let again = ask(&mut client, request).await.unwrap();
    assert_eq!(again, seen);
    let after = server.stats().systems();
    assert_eq!(after.misses(), before.misses(), "a repeat builds no star");
    assert!(after.hits() >= before.hits() + u64::try_from(seen.systems.len()).unwrap());

    client.close().await;
    server.stop().await;
}

/// P12.T6: an observer outside the root cube, or off the position grid, is `bad_request` naming
/// `mode`, for the range query and for `resolve_system`, and a request's other fields are checked
/// first.
#[tokio::test]
async fn an_observer_outside_the_cube_is_a_bad_request_naming_the_mode() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Parallax", SEED).await;
    let outside = QueryModeDto::Observed {
        observer: at_cell([70_000, 0, 0]),
    };
    let off_grid = QueryModeDto::Observed {
        observer: GalacticPosition {
            cell_ly: [0, 23_000, 0],
            offset_m: [2.0 * METRES_PER_LIGHT_YEAR, 0.0, 0.0],
        },
    };
    for mode in [outside, off_grid] {
        let error = ask(&mut client, sphere(&universe.id, mode))
            .await
            .unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("mode")),
            "{error:?}"
        );
        let error = resolve_one(
            &mut client,
            ResolveSystemRequest {
                universe: universe.id.clone(),
                system: SystemIdHex::from_u64(
                    SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).raw(),
                ),
                time: at_years(0),
                mode,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("mode")),
            "{error:?}"
        );
    }
    // The time is checked first, then the rest in plan 04's order, and the mode last.
    let mut late = sphere(&universe.id, outside);
    late.time = at_years(1_001);
    let error = ask(&mut client, late).await.unwrap_err();
    assert_eq!(error.field.as_deref(), Some("time"));
    let mut wide = sphere(&universe.id, outside);
    wide.radius_ly = 0.0;
    let error = ask(&mut client, wide).await.unwrap_err();
    assert_eq!(error.field.as_deref(), Some("radius_ly"));

    client.close().await;
    server.stop().await;
}

/// P12.T6: `resolve_system` answers one range row by its ID, the same row the range query gives in
/// either mode, less the brief it does not ask for.
#[tokio::test]
async fn resolve_system_answers_the_range_row_in_either_mode() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Parallax", SEED).await;
    let now = ask(&mut client, sphere(&universe.id, QueryModeDto::Now))
        .await
        .unwrap();
    let seen = ask(&mut client, sphere(&universe.id, coreward()))
        .await
        .unwrap();
    // A few rows, from the nearest to the farthest.
    let rows = now.systems.len();
    for index in [0, rows / 2, rows - 1] {
        for (mode, answer) in [(QueryModeDto::Now, &now), (coreward(), &seen)] {
            let row = &answer.systems[index];
            let resolved = resolve_one(
                &mut client,
                ResolveSystemRequest {
                    universe: universe.id.clone(),
                    system: row.id.clone(),
                    time: at_years(100),
                    mode,
                },
            )
            .await
            .unwrap();
            assert_eq!(resolved.universe, universe.id);
            assert_eq!(resolved.time, at_years(100));
            assert_eq!(
                resolved.record,
                SystemRecord {
                    stellar: None,
                    fe_h_dex: None,
                    ..row.clone()
                },
                "{mode:?}"
            );
            assert_eq!(
                resolved.record.observed.is_some(),
                mode != QueryModeDto::Now
            );
        }
    }

    client.close().await;
    server.stop().await;
}

/// `resolve_system` refuses an ID that names no system as `system_summary` does, `unknown_system`
/// naming `system`, and a time outside the clock window naming `time`, before the ID.
#[tokio::test]
async fn resolve_system_names_the_system_or_the_time_it_cannot_use() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Parallax", SEED).await;
    let request = |system: u64, time: UniverseTime| ResolveSystemRequest {
        universe: universe.id.clone(),
        system: SystemIdHex::from_u64(system),
        time,
        mode: coreward(),
    };
    for system in [u64::MAX, 0x0200_0800_2000_ffff] {
        let error = resolve_one(&mut client, request(system, at_years(0)))
            .await
            .unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::UnknownSystem, Some("system")),
            "{system:016x}: {error:?}"
        );
    }
    let error = resolve_one(&mut client, request(u64::MAX, at_years(-1_001)))
        .await
        .unwrap_err();
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("time"))
    );

    client.close().await;
    server.stop().await;
}

/// The galactic centre's black hole is refused in either mode with a typed error, `unknown_system`
/// naming `system` as `system_summary` refuses it, rather than a panic: its members have no line
/// until plan 09's P09.T28 builds the centre's orbits (P12.T2 as built), and plan 08's velocity draw
/// is a grid record's.
#[tokio::test]
async fn the_central_black_hole_is_refused_until_its_orbit_is_built() {
    assert_eq!(GENERATOR_VERSION.get(), 16, "the version this was seen at");
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Parallax", SEED).await;
    let black_hole =
        SystemIdHex::from_u64(SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).raw());
    for mode in [QueryModeDto::Now, coreward()] {
        let error = resolve_one(
            &mut client,
            ResolveSystemRequest {
                universe: universe.id.clone(),
                system: black_hole.clone(),
                time: at_years(0),
                mode,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::UnknownSystem, Some("system")),
            "{mode:?}: {error:?}"
        );
        assert!(
            error.message.contains("cannot be placed yet"),
            "{}",
            error.message
        );
        assert!(error.message.contains("not built yet"), "{}", error.message);
    }
    // The server is well and answers the next request.
    let seen = ask(&mut client, sphere(&universe.id, coreward())).await;
    assert!(seen.is_ok(), "{seen:?}");

    client.close().await;
    server.stop().await;
}

/// A member of band C of the first feature with members within 3,000 ly of the Sun-like point.
fn a_feature_member(galaxy: &Galaxy) -> hyperion_sim::galaxy::features::members::MemberRecord {
    use hyperion_sim::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
    use hyperion_sim::galaxy::features::members::FeatureInterior;
    use hyperion_sim::galaxy::imf::MassBand;

    let sun = hyperion_sim::coords::GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
        .expect("in the root cube");
    let interior = FeatureCatalogue::near(galaxy, &sun, LightYears::new(3_000.0), &NoFeatureCache)
        .find_map(|feature| FeatureInterior::of(galaxy, &feature))
        .expect("a feature with members lies within 3,000 ly of the Sun");
    let mut members = Vec::new();
    interior
        .grid()
        .owned_cells()
        .find_map(|cell| {
            interior.members_in_cell(galaxy, MassBand::C, cell, &mut members);
            members.first().map(|(member, _)| *member)
        })
        .expect("the feature has a member in band C")
}

/// A catalogue feature's member resolves on its own line, its cluster's motion and its own, not
/// plan 08's draw for a grid record, and is observed as the sim's `observe_hit` reads it with its
/// member record's stars.
#[tokio::test]
#[ignore = "slow: a feature interior in a full-potential galaxy"]
async fn resolve_system_places_a_feature_member_on_its_own_line() {
    use hyperion_sim::galaxy::query::SystemHit;
    use hyperion_sim::observe::{Drift, Observer, Trajectory, observe_hit};

    let galaxy = galaxy();
    let member = a_feature_member(galaxy);
    let record = *member.record();
    let line = Drift::of_member(&member);
    let time = hyperion_sim::time::UniverseTime::from_julian_years(100).expect("in the window");

    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Parallax", SEED).await;
    let id = SystemIdHex::from_u64(record.id().raw());
    let now = resolve_one(
        &mut client,
        ResolveSystemRequest {
            universe: universe.id.clone(),
            system: id.clone(),
            time: at_years(100),
            mode: QueryModeDto::Now,
        },
    )
    .await
    .unwrap();
    let present = line.position_at(time);
    assert_eq!(now.record.id, id);
    assert_eq!(now.record.position.cell_ly, present.cell().to_array());
    for (wire, sim) in now
        .record
        .position
        .offset_m
        .iter()
        .zip(present.offset_metres())
    {
        assert!(close(*wire, sim), "offset {wire} against {sim}");
    }
    for (wire, sim) in now
        .record
        .velocity_km_s
        .iter()
        .zip(line.velocity().metres_per_second())
    {
        assert!(
            (wire * 1e3 - sim).abs() <= 1e-9 * sim.abs().max(1.0),
            "{wire} km/s against {sim} m/s"
        );
    }

    let seen = resolve_one(
        &mut client,
        ResolveSystemRequest {
            universe: universe.id.clone(),
            system: id,
            time: at_years(100),
            mode: coreward(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        SystemRecord {
            observed: None,
            ..seen.record.clone()
        },
        now.record
    );
    let QueryModeDto::Observed { observer } = coreward() else {
        unreachable!("an observed mode")
    };
    let observer = Observer::new(
        hyperion_sim::coords::GalacticPosition::new(
            LyCell::new(observer.cell_ly),
            observer.offset_m,
        )
        .expect("a canonical position"),
        time,
    )
    .expect("an observer");
    let sim = observe_hit(
        galaxy,
        &SystemHit::new(record, present, LightYears::ZERO),
        &member.stars(galaxy),
        &observer,
    )
    .expect("a feature member's motion is traced");
    assert_observed_is_the_sims(
        seen.record.observed.as_ref().expect("an observed row"),
        &sim,
        &seen.record.designation,
    );

    client.close().await;
    server.stop().await;
}
