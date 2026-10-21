//! A system's summary over a real socket: every star of pinned systems, companions included, and
//! the orbits that hold them, against the sim's own; the refusals; cancellation; and the cache
//! (plan 06, P06.T34, with plan 11's P11.T13 slice); and one system asked in both of plan 12's
//! modes (P12.T6.b).
//!
//! Every universe here has one fixed seed, so the systems are the ones the sim generates for that
//! seed: the test builds the same galaxy and checks the wire against it.

mod common;

use std::sync::OnceLock;

use common::{TestClient, TestServer};
use hyperion_protocol::{
    ErrorCode, GalacticPosition, HierarchyNodeDto, Modelled, ObjectKindDto, ObservedDto, OrbitDto,
    PhaseDto, QueryModeDto, RemnantDto, RequestBody, RequestError, ResponseBody, ServerMessage,
    StarSummaryDto, SystemExistenceDto, SystemIdHex, SystemSummaryDto, SystemSummaryRequest,
    UniverseIdHex, UniverseTime,
};
use hyperion_sim::coords::LyCell;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{CellKey, candidate_count, resolve};
use hyperion_sim::id::{CentreMemberId, SystemId};
use hyperion_sim::observe::{
    CurvatureError, Observer, Retardation, curvature_error, summary_observed,
};
use hyperion_sim::orbit::KeplerElements;
use hyperion_sim::stellar::multiplicity::HierarchyNode;
use hyperion_sim::stellar::remnant::RemnantKind;
use hyperion_sim::stellar::system::{RemnantDetail, SystemExistence, SystemStars};
use hyperion_sim::units::consts::{GM_SUN, METRES_PER_LIGHT_YEAR};
use hyperion_sim::units::{Megayears, Years};
use hyperion_sim::{Seed, time};

/// The seed of every universe these tests create, the other integration tests' own.
const SEED: u64 = 0x4d2;

/// Seconds in a Julian year, the sim's year: the wire carries a time as whole seconds.
const SECONDS_PER_JULIAN_YEAR: i64 = 31_557_600;

/// Pinned systems of layer C at the solar circle, 26,000 ly out on the +y axis: a single K dwarf,
/// a K1 V and M1 V pair, a triple whose primary is a DC white dwarf, and a quadruple of K and M
/// dwarfs.
const PINNED: [u64; 4] = [
    0x4200_2cb2_0000_0001,
    0x4200_2cb2_0000_0000,
    0x4200_2cb2_0000_0005,
    0x4200_acb2_0000_0009,
];

/// A pinned triple of three living main-sequence stars (P11.T13's integration test). Until
/// ruling 74's placement weights it was `0x4200_2cb2_0000_0009`, an F3 IV subgiant with a K7.5 V
/// and an F8.5 V companion, which now draws as a binary; until P15.T4.b's fitted Chabrier scale
/// (ruling 138), `0x4200_2cb2_0000_000d`, whose primary is now a giant. Re-picked by search: the
/// first triple of three dwarfs in the cells along +x from the solar-circle cell.
const TRIPLE: u64 = 0x4200_6cb2_0000_000e;

/// The galaxy the server builds for [`SEED`], to hold its answers to.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
}

/// The galaxy of [`SEED`] as the server builds it, with its full potential and so its velocities,
/// which an observed summary's retardation needs: [`galaxy`] is enough for the stars alone, and
/// cheaper to build.
fn moving_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)).with_full_potential())
}

/// The sim's stars of the system `raw` names.
fn sim_stars(raw: u64) -> SystemStars {
    let id = SystemId::from_raw(raw).expect("a pinned ID is a system ID");
    SystemStars::generate(
        galaxy(),
        &resolve(galaxy(), id).expect("a pinned ID names a system"),
    )
}

fn at_years(years: i64) -> UniverseTime {
    UniverseTime {
        seconds: years * SECONDS_PER_JULIAN_YEAR,
        nanos: 0,
    }
}

fn sim_time(time: UniverseTime) -> time::UniverseTime {
    time::UniverseTime::new(time.seconds, time.nanos).expect("a valid time")
}

fn request(universe: &UniverseIdHex, system: u64, time: UniverseTime) -> RequestBody {
    request_in(universe, system, time, QueryModeDto::Now)
}

fn request_in(
    universe: &UniverseIdHex,
    system: u64,
    time: UniverseTime,
    mode: QueryModeDto,
) -> RequestBody {
    RequestBody::SystemSummary(SystemSummaryRequest {
        universe: universe.clone(),
        system: SystemIdHex::from_u64(system),
        time,
        mode,
    })
}

/// A server, a client that has said hello, and a universe of [`SEED`].
async fn started() -> (TestServer, TestClient, UniverseIdHex) {
    let server = TestServer::start().await;
    let (client, universe) = joined(&server, "Kepler Reach").await;
    (server, client, universe)
}

/// A client of `server` that has said hello, and a universe of [`SEED`] of its own, named `name`,
/// which no other universe of the server may have.
async fn joined(server: &TestServer, name: &str) -> (TestClient, UniverseIdHex) {
    let mut client = server.connected().await;
    let universe = client.create_universe(name, SEED).await.id;
    (client, universe)
}

async fn summary(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    system: u64,
    time: UniverseTime,
) -> SystemSummaryDto {
    summary_in(client, universe, system, time, QueryModeDto::Now).await
}

async fn summary_in(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    system: u64,
    time: UniverseTime,
    mode: QueryModeDto,
) -> SystemSummaryDto {
    match client
        .request(request_in(universe, system, time, mode))
        .await
    {
        Ok(ResponseBody::SystemSummary(summary)) => summary,
        other => panic!("expected the summary of {system:#018x}, got {other:?}"),
    }
}

async fn refused(client: &mut TestClient, body: RequestBody) -> RequestError {
    match client.request(body).await {
        Err(error) => error,
        Ok(body) => panic!("expected a refusal, got {body:?}"),
    }
}

/// Holds a float on the wire to the sim's, bit for bit: `serde_json` writes the shortest decimal
/// that round-trips, and the workspace builds it with `float_roundtrip` (ruling 64.7), so its
/// parser reads that decimal back to the same bits, as a browser's correctly rounded `JSON.parse`
/// does. A summary that matches only to a tolerance is a summary that differs.
fn assert_bits(what: &str, wire: f64, sim: f64) {
    assert_eq!(
        wire.to_bits(),
        sim.to_bits(),
        "{what}: the wire says {wire}, the sim {sim}"
    );
}

/// The wire's string for a value whose serde form is its variant's name in snake case, as every
/// enum of the stellar DTOs is.
fn wire_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .expect("an enum serialises")
        .as_str()
        .expect("a unit variant serialises as a string")
        .to_owned()
}

/// `CamelCase` as `snake_case`: the sim's variant names as the wire writes them.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Holds an orbit on the wire to the sim's elements.
fn assert_orbit_is_the_sims(wire: &OrbitDto, sim: &KeplerElements) {
    assert_bits("period", wire.period_s, sim.period().value());
    assert_bits(
        "semi-major axis",
        wire.semi_major_axis_m,
        sim.semi_major_axis().value(),
    );
    assert_bits(
        "eccentricity",
        wire.eccentricity,
        sim.eccentricity().value(),
    );
    assert_bits(
        "inclination",
        wire.inclination_rad,
        sim.inclination().value(),
    );
    assert_bits(
        "ascending node",
        wire.ascending_node_rad,
        sim.ascending_node().value(),
    );
    assert_bits(
        "argument of periapsis",
        wire.argument_of_periapsis_rad,
        sim.argument_of_periapsis().value(),
    );
    assert_bits(
        "mean anomaly at the epoch",
        wire.mean_anomaly_at_epoch_rad,
        sim.mean_anomaly_at_epoch().value(),
    );
    assert_bits("mu", wire.mu_m3_s2, sim.gravitational_parameter().value());
}

/// Holds one star on the wire to the sim's summary of it.
fn assert_star_is_the_sims(
    wire: &StarSummaryDto,
    stars: &SystemStars,
    sim: &hyperion_sim::stellar::system::StarSummary,
) {
    let state = sim.state();
    let model = &stars.stars()[usize::from(wire.body_index)];
    assert_eq!(wire.body_index, sim.body().body_index());
    assert_eq!(wire_name(&wire.kind), snake(&format!("{:?}", sim.kind())));
    assert_eq!(
        wire_name(&wire.phase),
        snake(&format!("{:?}", state.phase()))
    );
    assert_eq!(wire.class, sim.classification().to_string());
    assert_bits(
        "initial mass",
        wire.initial_mass_msun,
        model.initial_mass().value(),
    );
    assert_bits("mass", wire.mass_msun, state.mass().value());
    assert_bits("core mass", wire.core_mass_msun, state.core_mass().value());
    assert_bits(
        "luminosity",
        wire.luminosity_lsun,
        state.luminosity().value(),
    );
    assert_bits("radius", wire.radius_rsun, state.radius().value());
    assert_bits(
        "mass-loss rate",
        wire.mass_loss_rate_msun_per_yr,
        state.mass_loss_rate().value(),
    );
    match wire.teff_k {
        Some(teff) => assert_bits("T_eff", teff, state.effective_temperature().value()),
        None => assert_eq!(
            state.luminosity().value().to_bits(),
            0,
            "only the dark have none"
        ),
    }
    assert_eq!(
        wire.absolute_v_mag.is_some(),
        sim.absolute_magnitude_v().is_some()
    );
    if let (Some(wire), Some(sim)) = (wire.absolute_v_mag, sim.absolute_magnitude_v()) {
        assert_bits("M_V", wire, sim.value());
    }
    assert_eq!(wire.colour_b_v_mag.is_some(), sim.colour_b_v().is_some());
    if let (Some(wire), Some(sim)) = (wire.colour_b_v_mag, sim.colour_b_v()) {
        assert_bits("B-V", wire, sim.value());
    }
    match (&wire.remnant, sim.remnant()) {
        (None, None) => {}
        (
            Some(RemnantDto::WhiteDwarf {
                cooling_age_myr, ..
            }),
            Some(remnant),
        ) => {
            assert_eq!(remnant.kind(), RemnantKind::WhiteDwarf);
            let died = model
                .death()
                .expect("a white dwarf's star died")
                .age()
                .value();
            assert_bits(
                "cooling age",
                *cooling_age_myr,
                Megayears::from(Years::new(state.age().value() - died)).value(),
            );
        }
        (Some(RemnantDto::NeutronStar { .. }), Some(remnant)) => {
            assert_eq!(remnant.kind(), RemnantKind::NeutronStar);
        }
        (Some(RemnantDto::BlackHole { .. }), Some(remnant)) => {
            assert_eq!(remnant.kind(), RemnantKind::BlackHole);
        }
        (Some(RemnantDto::NoRemnant), Some(remnant)) => {
            assert_eq!(remnant.kind(), RemnantKind::None);
        }
        (wire, sim) => panic!("the wire's remnant {wire:?} is not the sim's {sim:?}"),
    }
    assert_eq!(
        wire.death_time.map(sim_time),
        sim.death_in_window().map(|(when, _)| when)
    );
    assert_details_are_the_sims(wire, sim);
}

/// Holds a star's variability, rotation and activity on the wire to the sim's (P06.T25–T26).
fn assert_details_are_the_sims(
    wire: &StarSummaryDto,
    sim: &hyperion_sim::stellar::system::StarSummary,
) {
    // P06.T21–T22: a neutron star's pulsar and a black hole's spin.
    match &wire.remnant {
        Some(RemnantDto::NeutronStar { pulsar, .. }) => {
            let Some(RemnantDetail::NeutronStar(sim_pulsar)) = sim.remnant_detail() else {
                panic!("a neutron star's summary carries its pulsar");
            };
            let pulsar = pulsar.expect("the wire carries the pulsar (P06.T21)");
            assert_bits("P", pulsar.spin_period_s, sim_pulsar.period().value());
            assert_bits(
                "Pdot",
                pulsar.period_derivative_s_per_s,
                sim_pulsar.period_derivative(),
            );
            assert_bits("B", pulsar.magnetic_field_g, sim_pulsar.field().value());
            assert_eq!(pulsar.alive, sim_pulsar.is_radio_alive());
            assert_eq!(pulsar.magnetar, sim_pulsar.is_magnetar());
        }
        Some(RemnantDto::BlackHole {
            dimensionless_spin, ..
        }) => {
            let Some(RemnantDetail::BlackHole(hole)) = sim.remnant_detail() else {
                panic!("a black hole's summary carries its spin");
            };
            assert_bits(
                "spin",
                dimensionless_spin.expect("the wire carries the spin (P06.T22)"),
                hole.spin(),
            );
        }
        Some(RemnantDto::WhiteDwarf { .. } | RemnantDto::NoRemnant) | None => {}
    }
    // P06.T26.a–c: variability, a value or `null`, never absent.
    match (&wire.variability, sim.variability()) {
        (Modelled::Value(wire), Some(sim)) => {
            assert_bits("variable period", wire.period_d, sim.period().value());
            assert_bits("amplitude", wire.amplitude_mag, sim.amplitude().value());
        }
        (Modelled::Null, None) => {}
        (wire, sim) => panic!("the wire's variability {wire:?} is not the sim's {sim:?}"),
    }
    // P06.T16.b: the planetary nebula, a value or `null`, never absent.
    match (&wire.planetary_nebula, sim.planetary_nebula()) {
        (Modelled::Value(wire), Some(sim)) => {
            assert_bits("nebula radius", wire.radius_ly, sim.radius().value());
            assert_bits(
                "expansion speed",
                wire.expansion_speed_km_s,
                sim.expansion_speed().value(),
            );
            assert_bits("nebula age", wire.age_yr, sim.age().value());
            assert_bits(
                "ionised mass",
                wire.ionised_mass_msun,
                sim.ionised_mass().value(),
            );
            assert_eq!(wire.excitation_class, sim.excitation_class());
        }
        (Modelled::Null, None) => {}
        (wire, sim) => panic!("the wire's nebula {wire:?} is not the sim's {sim:?}"),
    }
    // P06.T25: a living star's rotation and a cool dwarf's activity are the sim's.
    if let Some(spin) = sim.rotation() {
        let Modelled::Value(period) = wire.rotation_period_d else {
            panic!("{:?} for {spin:?}", wire.rotation_period_d);
        };
        assert_bits("rotation", period, spin.period().value());
    }
    match (&wire.activity_log_lx_lbol, sim.activity()) {
        (Modelled::Value(wire), Some(sim)) => assert_bits("activity", *wire, sim.log_lx_lbol()),
        (Modelled::Null | Modelled::NotModelled, None) => {}
        (wire, sim) => panic!("the wire's activity {wire:?} is not the sim's {sim:?}"),
    }
}

/// Holds a whole summary on the wire to the sim's summary of the same system at `time`: the
/// request's in `now`, and in observed mode the time its light left.
fn assert_summary_is_the_sims(
    wire: &SystemSummaryDto,
    stars: &SystemStars,
    time: time::UniverseTime,
) {
    let sim = stars.summary_at(time);
    assert_eq!(
        wire.existence,
        match sim.existence() {
            SystemExistence::NotYetBorn => SystemExistenceDto::NotYetBorn,
            SystemExistence::Exists => SystemExistenceDto::Exists,
        }
    );
    assert_bits(
        "age",
        wire.age_myr,
        Megayears::from(stars.record().age_at(time)).value(),
    );
    assert_bits("[Fe/H]", wire.fe_h_dex, sim.composition().fe_h().value());
    assert_eq!(wire.stars.len(), sim.stars().len());
    for (star, summary) in wire.stars.iter().zip(sim.stars()) {
        assert_star_is_the_sims(star, stars, summary);
    }
    let Some(hierarchy) = sim.hierarchy() else {
        assert_eq!(wire.hierarchy.nodes, []);
        return;
    };
    assert_eq!(wire.hierarchy.nodes.len(), hierarchy.nodes().len());
    for (node, sim) in wire.hierarchy.nodes.iter().zip(hierarchy.nodes()) {
        match (node, sim) {
            (
                HierarchyNodeDto::Star {
                    body_index,
                    mass_msun,
                },
                HierarchyNode::Star(index),
            ) => {
                let slot = hierarchy.star(*index);
                assert_eq!(*body_index, slot.body().body_index());
                assert_bits("star node mass", *mass_msun, slot.initial_mass().value());
            }
            (
                HierarchyNodeDto::Pair {
                    inner,
                    outer,
                    orbit,
                },
                HierarchyNode::Pair {
                    inner: sim_inner,
                    outer: sim_outer,
                    orbit: sim_orbit,
                },
            ) => {
                assert_eq!((*inner, *outer), (sim_inner.get(), sim_outer.get()));
                assert_orbit_is_the_sims(orbit, sim_orbit);
            }
            (node, sim) => panic!("the wire's node {node:?} is not the sim's {sim:?}"),
        }
    }
}

/// Pinned systems' summaries on one server: the refusals, every star of pinned systems with their
/// companions, a pinned triple's three stars and two orbits, and one system asked in both of plan
/// 12's modes, all against the sim's, and the observed mode's refusals.
///
/// Nextest runs each test in its own process, and each server builds its galaxy, 4.4 s with its
/// full potential in a test build, so the five share one server: each takes a client and a
/// universe of its own, as it did with a server of its own, and its assertions and their messages
/// are unchanged. The refusals come first, so that the count of systems they read is what a fresh
/// server gives them; the others read no count. The cancellation, which needs the galaxy's build
/// still running, and the cache's test, which counts from a fresh server too, keep a server each. A
/// server holds one universe of a name, so each property's universe is named apart (no assertion
/// reads a name), the first keeping its test's.
#[tokio::test]
async fn pinned_summaries_are_the_sims_and_refusals_name_their_field() {
    let server = TestServer::start().await;
    a_bad_time_or_an_id_that_names_no_system_is_refused_naming_the_field(&server).await;
    a_pinned_systems_summary_is_the_sims_companions_included(&server).await;
    a_pinned_triple_returns_three_stars_and_two_orbits(&server).await;
    a_summary_asked_in_both_modes_is_the_sims_now_and_when_its_light_left(&server).await;
    an_observed_summary_names_the_mode_for_a_bad_observer_and_the_system_for_the_centre(&server)
        .await;
    server.stop().await;
}

/// Whether two floats agree to 1e-15 relative, as `tests/observed.rs` holds a row's observation.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-15 * a.abs().max(b.abs())
}

/// Holds a summary's observation on the wire to the sim's retardation and stated error, as
/// `tests/observed.rs` holds a row's: the emitted time exactly, the floats to 1e-15 relative.
fn assert_observed_is_the_sims(wire: &ObservedDto, r: &Retardation, error: CurvatureError) {
    assert_eq!(
        (wire.emitted.seconds, wire.emitted.nanos),
        (r.emitted().seconds(), r.emitted().subsec_nanos())
    );
    assert!(
        close(wire.light_age_yr, r.light_age().as_julian_years_f64()),
        "light age {} against {:?}",
        wire.light_age_yr,
        r.light_age()
    );
    assert_eq!(
        wire.apparent_position.cell_ly,
        r.apparent_position().cell().to_array()
    );
    for (wire, sim) in wire
        .apparent_position
        .offset_m
        .iter()
        .zip(r.apparent_position().offset_metres())
    {
        assert!(close(*wire, sim), "offset {wire} against {sim}");
    }
    assert!(
        close(wire.curvature_error_ly, error.length().value()),
        "{} ly against {error:?}",
        wire.curvature_error_ly
    );
    assert!(
        close(wire.curvature_error_arcsec, error.angle().value()),
        "{} arcsec against {error:?}",
        wire.curvature_error_arcsec
    );
}

/// The light-year cell 3,000 ly coreward of the pinned systems, the observer of plan 12's tests.
const COREWARD: [i32; 3] = [0, 23_000, 0];

/// Observed mode from the centre of the light-year cell `cell_ly`.
fn observed_from(cell_ly: [i32; 3]) -> QueryModeDto {
    QueryModeDto::Observed {
        observer: GalacticPosition {
            cell_ly,
            offset_m: [0.0; 3],
        },
    }
}

/// P12.T6.b: one pinned system's summary asked in both modes, at one time and from 3,000 ly
/// coreward, so that its light left before the clock window's start and the source horizon is
/// read. Both answers echo the request's `system` and `time`, only the observed one has
/// `observed`, the `now` answer is the sim's summary at `time`, and the observed answer's state is
/// the sim's `summary_observed` at the emitted time, bit for bit, with the sim's retardation and
/// stated error.
///
/// Run by [`pinned_summaries_are_the_sims_and_refusals_name_their_field`] on its shared server.
async fn a_summary_asked_in_both_modes_is_the_sims_now_and_when_its_light_left(
    server: &TestServer,
) {
    let (mut client, universe) = joined(server, "Kepler Reach: as observed").await;
    let raw = PINNED[1];
    let at = at_years(100);
    let now = summary_in(&mut client, &universe, raw, at, QueryModeDto::Now).await;
    let seen = summary_in(&mut client, &universe, raw, at, observed_from(COREWARD)).await;
    assert_eq!(
        (&seen.universe, &seen.system, seen.time),
        (&now.universe, &now.system, now.time)
    );
    assert_eq!((&now.system, now.time), (&SystemIdHex::from_u64(raw), at));
    assert_eq!(now.observed, None);
    let Some(light) = seen.observed else {
        panic!("an observed summary carries its observation: {seen:?}");
    };

    // The present, as plan 06's summary has it.
    assert_summary_is_the_sims(&now, &sim_stars(raw), sim_time(at));

    // As observed: the sim's own reading over the server's galaxy, whose systems move.
    let galaxy = moving_galaxy();
    let record = resolve(
        galaxy,
        SystemId::from_raw(raw).expect("a pinned ID is a system ID"),
    )
    .expect("a pinned ID names a system");
    let stars = SystemStars::generate(galaxy, &record);
    let observer = Observer::new(
        hyperion_sim::coords::GalacticPosition::new(LyCell::new(COREWARD), [0.0; 3])
            .expect("a cell's centre is canonical"),
        sim_time(at),
    )
    .expect("the observer is in the cube and its time in the clock window");
    let (then, retardation) = summary_observed(galaxy, &record, &stars, &observer)
        .expect("a grid system's motion is traced");
    let error = curvature_error(galaxy, observer.position(), &retardation);
    assert_eq!(then.time(), retardation.emitted());
    assert_eq!(then, stars.summary_at(retardation.emitted()));
    assert_summary_is_the_sims(&seen, &stars, then.time());
    assert_observed_is_the_sims(&light, &retardation, error);
    // The light left before the clock window's start, which is never checked.
    assert!(
        light.emitted.seconds < -1_000 * SECONDS_PER_JULIAN_YEAR,
        "{:?}",
        light.emitted
    );
    assert!(
        (2_900.0..3_100.0).contains(&light.light_age_yr),
        "{} years",
        light.light_age_yr
    );
    // The age is already the emitted time's: the present's less the light's age.
    assert!(
        ((now.age_myr - seen.age_myr) - light.light_age_yr / 1e6).abs() < 1e-9,
        "{} Myr now, {} Myr then, {} years of light",
        now.age_myr,
        seen.age_myr,
        light.light_age_yr
    );
    assert_eq!(seen.fe_h_dex.to_bits(), now.fe_h_dex.to_bits());
    client.close().await;
}

/// P12.T6.b: an observer outside the root cube or off the position grid is `bad_request` naming
/// `mode`, with a time outside the clock window named first; and the central black hole asked from
/// a valid observer is `unknown_system` naming `system`, as it is in `now`.
///
/// Run by [`pinned_summaries_are_the_sims_and_refusals_name_their_field`] on its shared server.
async fn an_observed_summary_names_the_mode_for_a_bad_observer_and_the_system_for_the_centre(
    server: &TestServer,
) {
    let (mut client, universe) = joined(server, "Kepler Reach: observed refusals").await;
    let pinned = PINNED[0];
    let outside = observed_from([70_000, 0, 0]);
    let off_grid = QueryModeDto::Observed {
        observer: GalacticPosition {
            cell_ly: COREWARD,
            offset_m: [2.0 * METRES_PER_LIGHT_YEAR, 0.0, 0.0],
        },
    };
    for mode in [outside, off_grid] {
        let error = refused(
            &mut client,
            request_in(&universe, pinned, at_years(0), mode),
        )
        .await;
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("mode")),
            "{mode:?}: {error:?}"
        );
        let error = refused(
            &mut client,
            request_in(&universe, pinned, at_years(1_001), mode),
        )
        .await;
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("time")),
            "{mode:?}: {error:?}"
        );
    }
    let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).raw();
    for mode in [QueryModeDto::Now, observed_from(COREWARD)] {
        let error = refused(
            &mut client,
            request_in(&universe, black_hole, at_years(0), mode),
        )
        .await;
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::UnknownSystem, Some("system")),
            "{mode:?}: {error:?}"
        );
    }
    // Refusals are answers, and the connection carries on.
    let answer = summary_in(
        &mut client,
        &universe,
        pinned,
        at_years(0),
        observed_from(COREWARD),
    )
    .await;
    assert!(answer.observed.is_some(), "{answer:?}");
    client.close().await;
}

/// Run by [`pinned_summaries_are_the_sims_and_refusals_name_their_field`] on its shared server.
async fn a_pinned_systems_summary_is_the_sims_companions_included(server: &TestServer) {
    let (mut client, universe) = joined(server, "Kepler Reach: pinned systems").await;
    let mut shapes = Vec::new();
    for raw in PINNED {
        let stars = sim_stars(raw);
        for years in [0, -500, 731] {
            let answer = summary(&mut client, &universe, raw, at_years(years)).await;
            assert_eq!(answer.universe, universe);
            assert_eq!(answer.system, SystemIdHex::from_u64(raw));
            assert_eq!(answer.time, at_years(years));
            assert_eq!(answer.existence, SystemExistenceDto::Exists);
            assert_eq!(answer.stars.len(), usize::from(stars.star_count()));
            assert_summary_is_the_sims(&answer, &stars, sim_time(answer.time));
            // Nothing this generator version does not compute is sent as a value.
            for star in &answer.stars {
                assert!(star.active_events.is_none());
            }
        }
        shapes.push(stars.star_count());
    }
    assert_eq!(
        shapes,
        [1, 2, 3, 4],
        "the pinned systems are what their names say"
    );
    client.close().await;
}

/// A node's total mass, M☉, summed from its stars' masses as a client sums them.
fn mass_under(nodes: &[HierarchyNodeDto], index: u8) -> f64 {
    match &nodes[usize::from(index)] {
        HierarchyNodeDto::Star { mass_msun, .. } => *mass_msun,
        HierarchyNodeDto::Pair { inner, outer, .. } => {
            mass_under(nodes, *inner) + mass_under(nodes, *outer)
        }
    }
}

/// P11.T13's test: a pinned triple returns three stars and two orbits, each orbit with its whole
/// element set and its μ, and each star's mass, so that a client can place the stars about each
/// barycentre as the server does.
///
/// Run by [`pinned_summaries_are_the_sims_and_refusals_name_their_field`] on its shared server.
async fn a_pinned_triple_returns_three_stars_and_two_orbits(server: &TestServer) {
    let (mut client, universe) = joined(server, "Kepler Reach: the triple").await;
    let answer = summary(&mut client, &universe, TRIPLE, at_years(0)).await;
    assert_eq!(answer.stars.len(), 3);
    assert_eq!(
        answer
            .stars
            .iter()
            .map(|star| (star.body_index, star.kind))
            .collect::<Vec<_>>(),
        [
            (0, ObjectKindDto::Dwarf),
            (1, ObjectKindDto::Dwarf),
            (2, ObjectKindDto::Dwarf)
        ]
    );
    assert!(answer.stars.iter().all(|s| s.phase != PhaseDto::Substellar));
    let nodes = &answer.hierarchy.nodes;
    assert_eq!(nodes.len(), 5, "three stars and two pairs: {nodes:?}");
    let mut pairs = 0;
    for node in nodes {
        match node {
            HierarchyNodeDto::Star {
                body_index,
                mass_msun,
            } => {
                let star = &answer.stars[usize::from(*body_index)];
                assert_eq!(star.body_index, *body_index);
                assert_eq!(mass_msun.to_bits(), star.initial_mass_msun.to_bits());
            }
            HierarchyNodeDto::Pair {
                inner,
                outer,
                orbit,
            } => {
                pairs += 1;
                assert!(inner < outer, "the inner member comes first: {node:?}");
                // μ is G times the pair's two members' masses, to the rounding of its sum.
                let mass = mass_under(nodes, *inner) + mass_under(nodes, *outer);
                assert!(
                    (orbit.mu_m3_s2 / (GM_SUN * mass) - 1.0).abs() < 1e-14,
                    "μ = {} for {mass} M☉",
                    orbit.mu_m3_s2
                );
                // Kepler's third law holds between the period, the axis and μ.
                let a = orbit.semi_major_axis_m;
                let period = std::f64::consts::TAU * (a * a * a / orbit.mu_m3_s2).sqrt();
                assert!((period / orbit.period_s - 1.0).abs() < 1e-12);
                assert!((0.0..0.9999).contains(&orbit.eccentricity));
                assert!((0.0..=std::f64::consts::PI).contains(&orbit.inclination_rad));
                for angle in [
                    orbit.ascending_node_rad,
                    orbit.argument_of_periapsis_rad,
                    orbit.mean_anomaly_at_epoch_rad,
                ] {
                    assert!((0.0..std::f64::consts::TAU).contains(&angle));
                }
            }
        }
    }
    assert_eq!(pairs, 2);
    assert_summary_is_the_sims(&answer, &sim_stars(TRIPLE), sim_time(answer.time));
    client.close().await;
}

/// Run first by [`pinned_summaries_are_the_sims_and_refusals_name_their_field`] on its shared
/// server, so that the count of systems it reads is its own.
async fn a_bad_time_or_an_id_that_names_no_system_is_refused_naming_the_field(server: &TestServer) {
    let (mut client, universe) = joined(server, "Kepler Reach").await;
    let pinned = PINNED[0];

    // A time outside the clock window, either way, or with too many nanoseconds.
    for time in [
        at_years(1_001),
        at_years(-1_001),
        UniverseTime {
            seconds: 0,
            nanos: 1_000_000_000,
        },
    ] {
        let error = refused(&mut client, request(&universe, pinned, time)).await;
        assert_eq!(error.code, ErrorCode::BadRequest, "{error:?}");
        assert_eq!(error.field.as_deref(), Some("time"), "{error:?}");
    }

    // Well-formed IDs that name no system: bits that are no system ID at all, the index after a
    // cell's last candidate, and a brown dwarf's layer, which this generator version does not
    // place.
    let cell = CellKey::of(SystemId::from_raw(pinned).expect("a system ID")).expect("a grid ID");
    let beyond = cell
        .candidate_id(candidate_count(galaxy(), cell))
        .expect("a cell at the solar circle has room for another index");
    for raw in [u64::MAX, beyond.raw()] {
        let error = refused(&mut client, request(&universe, raw, at_years(0))).await;
        assert_eq!(
            error.code,
            ErrorCode::UnknownSystem,
            "{raw:#018x}: {error:?}"
        );
        assert_eq!(error.field.as_deref(), Some("system"), "{error:?}");
    }

    // A time at fault is named before the system.
    let error = refused(&mut client, request(&universe, u64::MAX, at_years(2_000))).await;
    assert_eq!(error.field.as_deref(), Some("time"));

    // A malformed ID does not parse: plan 04's `bad_request`, for the request's own ID.
    client
        .send_raw(&format!(
            r#"{{"type":"request","id":90,"body":{{"kind":"system_summary","universe":"{}","system":"4200","time":{{"seconds":0,"nanos":0}}}}}}"#,
            universe.as_str()
        ))
        .await;
    match client.next_message().await {
        ServerMessage::RequestError { id, error } => {
            assert_eq!(id.0, 90);
            assert_eq!(error.code, ErrorCode::BadRequest, "{error:?}");
        }
        other => panic!("expected a bad request, got {other:?}"),
    }

    // A universe that does not exist is refused before anything else is read.
    let error = refused(
        &mut client,
        request(&UniverseIdHex::from_u64(0xdead), u64::MAX, at_years(2_000)),
    )
    .await;
    assert_eq!(error.code, ErrorCode::UnknownUniverse);

    // Refusals are answers, and the connection carries on.
    let answer = summary(&mut client, &universe, pinned, at_years(0)).await;
    assert_eq!(answer.stars.len(), 1);
    assert_eq!(
        server.stats().systems().entries(),
        1,
        "no refused ID was cached"
    );
    client.close().await;
}

/// A cancelled summary ends with exactly one terminal message, and nothing follows it.
///
/// `create_universe` does not build the galaxy (only `open_universe` warms it), so the summary
/// first waits on a build of about a second in a test build (130 ms in a release one), and the
/// `cancel` is sent once the server's counters show that build running: the one terminal message is
/// then `cancelled`. A machine slow enough to finish the build first would see the `response`
/// instead, which is still one terminal message, so the test holds either and counts accordingly.
/// The `ping` sent after the `cancel` is answered after the request's one terminal message, and a
/// second `ping`, sent once nothing is in flight, is the next message, so no late answer follows.
#[tokio::test]
async fn a_cancelled_summary_ends_with_exactly_one_terminal_message() {
    let (server, mut client, universe) = started().await;
    let id = client
        .send_request(request(&universe, PINNED[1], at_years(0)))
        .await;
    server
        .stats_until("the galaxy's build is running", |stats| {
            stats.pool().running() > 0
        })
        .await;
    client.cancel(id).await;
    client
        .send(&hyperion_protocol::ClientMessage::Ping { nonce: 1 })
        .await;
    let mut terminal = Vec::new();
    loop {
        match client.next_message().await {
            ServerMessage::Pong { nonce: 1 } => break,
            ServerMessage::RequestError {
                id: answered,
                error,
            } if answered == id => {
                terminal.push(Some(error.code));
            }
            ServerMessage::Response { id: answered, .. } if answered == id => terminal.push(None),
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(
        matches!(terminal[..], [Some(ErrorCode::Cancelled) | None]),
        "exactly one terminal message, the `cancelled` or the answer: {terminal:?}"
    );
    server
        .stats_until("nothing is in flight", |stats| {
            stats.requests().in_flight() == 0
        })
        .await;
    assert_eq!(client.ping(2).await, ServerMessage::Pong { nonce: 2 });
    // The connection is healthy, and the same summary is answered once asked again.
    let answer = summary(&mut client, &universe, PINNED[1], at_years(0)).await;
    assert_eq!(answer.stars.len(), 2);
    let requests = server.stats().requests();
    let cancelled = u64::from(terminal[0].is_some());
    assert_eq!(
        (requests.cancelled(), requests.responded()),
        (cancelled, 3 - cancelled)
    );
    client.close().await;
    server.stop().await;
}

/// The frame of a response, from its body on: what two answers to one request share.
fn body_of(frame: &str) -> &str {
    frame.split_once(r#""body":"#).map_or_else(
        || panic!("a response frame has a body: {frame}"),
        |(_, body)| body,
    )
}

/// The same request twice gives the same frame, byte for byte but for its ID, the first built and
/// the second from the cache; a request at another time reuses the cached system too, since the
/// cache holds the system's state at the epoch.
#[tokio::test]
async fn the_same_request_twice_is_identical_whether_cached_or_not() {
    let (server, mut client, universe) = started().await;
    let mut frames = Vec::new();
    for _ in 0..2 {
        let id = client
            .send_request(request(&universe, TRIPLE, at_years(250)))
            .await;
        let frame = client.next_text().await;
        assert!(
            frame.starts_with(&format!(r#"{{"type":"response","id":{}"#, id.0)),
            "{frame}"
        );
        frames.push(frame);
    }
    assert_eq!(body_of(&frames[0]), body_of(&frames[1]));
    let systems = server.stats().systems();
    assert_eq!(
        (systems.misses(), systems.hits(), systems.entries()),
        (1, 1, 1)
    );
    assert!(systems.bytes() > 0 && systems.bytes() <= systems.budget());

    let earlier = summary(&mut client, &universe, TRIPLE, at_years(-250)).await;
    assert_summary_is_the_sims(&earlier, &sim_stars(TRIPLE), sim_time(earlier.time));
    assert_eq!(server.stats().systems().hits(), 2);

    // A second universe of the same seed shares the entry (plan 04, design note 23).
    let twin = client.create_universe("Kepler Reach II", SEED).await.id;
    let id = client
        .send_request(request(&twin, TRIPLE, at_years(250)))
        .await;
    let frame = client.next_text().await;
    assert!(frame.contains(&format!(r#""id":{}"#, id.0)), "{frame}");
    assert_eq!(
        body_of(&frame).replace(twin.as_str(), universe.as_str()),
        body_of(&frames[0])
    );
    assert_eq!(server.stats().systems().hits(), 3);
    client.close().await;
    server.stop().await;
}
