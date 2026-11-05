//! The planet viewer's kinds over a real socket (rendering plan R15): a planet generated from its
//! inputs and then answered by the system's kinds as a galaxy system is, its refusals, the store's
//! eviction, `RANDOMIZE`'s draw and a served planet's inputs read back.
//!
//! One server serves every property, as `system_bodies.rs` shares one: each server builds its
//! galaxy, which costs seconds in a test build.

mod common;

use common::surface::field_via_wire;
use common::{TestClient, TestServer};
use hyperion_protocol::{
    BodyDetailRequest, BodyIdHex, BodyStateDto, DetailLevelDto, ErrorCode, PlanetInputsDto,
    RequestBody, ResponseBody, SectionDto, SeedHex, SurfaceFieldRequest, SurveyScopeDto,
    SystemBodiesRequest, SystemIdHex, UniverseIdHex, UniverseTime, ViewerInputsRequest,
    ViewerPlanetDto, ViewerPlanetRequest, ViewerRandomizeRequest,
};
use hyperion_sim::planetary::viewer::{PlanetInputs, RawPlanetInputs, ViewerSystem};

/// The seed of the universe, the other integration tests' own.
const SEED: u64 = 0x4d2;

/// An Earth about a Sun, its day and tilt given, everything else drawn.
fn earth() -> PlanetInputsDto {
    PlanetInputsDto {
        seed: SeedHex::from_u64(0x0e15_0002),
        star_mass_msun: 1.0,
        fe_h_dex: 0.0,
        age_gyr: 4.57,
        mass_mearth: 1.0,
        semi_major_axis_au: 1.0,
        eccentricity: 0.0167,
        formed_at_au: None,
        day_hours: Some(24.0),
        tilt_deg: Some(23.4),
        water_rank: None,
        carbon_rank: None,
        nitrogen_rank: None,
    }
}

/// The epoch.
fn epoch() -> UniverseTime {
    UniverseTime {
        seconds: 0,
        nanos: 0,
    }
}

async fn generated(
    client: &mut TestClient,
    universe: &UniverseIdHex,
    inputs: PlanetInputsDto,
) -> ViewerPlanetDto {
    match client
        .request(RequestBody::ViewerPlanet(ViewerPlanetRequest {
            universe: universe.clone(),
            inputs,
        }))
        .await
    {
        Ok(ResponseBody::ViewerPlanet(planet)) => *planet,
        other => panic!("expected the planet, got {other:?}"),
    }
}

fn bodies_request(universe: &UniverseIdHex, system: &SystemIdHex) -> RequestBody {
    RequestBody::SystemBodies(SystemBodiesRequest {
        universe: universe.clone(),
        system: system.clone(),
        time: epoch(),
        detail: DetailLevelDto::Full,
    })
}

#[tokio::test]
async fn the_planet_viewer_s_kinds_serve_a_planet_from_its_inputs() {
    let server = TestServer::start().await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Kepler Reach viewer", SEED).await.id;
    a_planet_s_system_is_answered_as_a_galaxy_system_is(&mut client, &universe).await;
    inputs_out_of_range_are_refused_naming_them(&mut client, &universe).await;
    an_evicted_planet_is_unknown_until_it_is_generated_again(&mut client, &universe).await;
    a_draw_is_repeatable_and_its_planet_s_inputs_read_back(&mut client, &universe).await;
    server.stop().await;
}

/// `viewer_planet` answers the sim's planet, and `system_bodies`, `body_detail` and
/// `surface_field` then answer its system.
async fn a_planet_s_system_is_answered_as_a_galaxy_system_is(
    client: &mut TestClient,
    universe: &UniverseIdHex,
) {
    let planet = generated(client, universe, earth()).await;
    let raw = RawPlanetInputs {
        seed: 0x0e15_0002,
        star_mass_msun: 1.0,
        fe_h_dex: 0.0,
        age_gyr: 4.57,
        mass_mearth: 1.0,
        semi_major_axis_au: 1.0,
        eccentricity: 0.0167,
        formed_at_au: None,
        day_hours: Some(24.0),
        tilt_deg: Some(23.4),
        water_rank: None,
        carbon_rank: None,
        nitrogen_rank: None,
    };
    let sim = ViewerSystem::generate(PlanetInputs::try_from(raw).unwrap()).unwrap();
    assert_eq!(planet.system, SystemIdHex::from_u64(sim.id().raw()));
    assert_eq!(planet.body.to_parts(), (sim.id().raw(), 0x0100));
    assert_eq!(planet.inputs, earth());
    assert_eq!(planet.drawn.day_hours, Some(24.0));
    assert_eq!(planet.barycentre.cell_ly, [0, 26_000, 0]);

    let Ok(ResponseBody::SystemBodies(bodies)) = client
        .request(bodies_request(universe, &planet.system))
        .await
    else {
        panic!("the system's bodies");
    };
    assert_eq!(bodies.hosts.stars.len(), 1, "one synthetic star");
    let body = bodies
        .bodies
        .iter()
        .find(|body| body.id == planet.body)
        .expect("the planet among its system's bodies");
    assert_eq!(body.state, BodyStateDto::Present);

    let Ok(ResponseBody::BodyDetail(detail)) = client
        .request(RequestBody::BodyDetail(BodyDetailRequest {
            universe: universe.clone(),
            body: planet.body.clone(),
            time: epoch(),
            detail: DetailLevelDto::Full,
        }))
        .await
    else {
        panic!("the planet's record");
    };
    assert!(
        matches!(detail.record.bulk, SectionDto::Ok(_)),
        "the planet's bulk is derived"
    );

    let field = field_via_wire(
        client,
        SurfaceFieldRequest {
            universe: universe.clone(),
            body: planet.body.clone(),
            have_revision: None,
            scope: SurveyScopeDto::Training,
        },
    )
    .await;
    assert!(
        field.header.level > 0,
        "the planet's coarse field is served"
    );

    // Its inputs read back are the inputs it was generated from, each drawn one stated.
    let Ok(ResponseBody::ViewerInputs(read)) = client
        .request(RequestBody::ViewerInputs(ViewerInputsRequest {
            universe: universe.clone(),
            body: planet.body.clone(),
        }))
        .await
    else {
        panic!("the planet's inputs");
    };
    assert!(!read.multiple_stars);
    assert_eq!(read.inputs.mass_mearth, 1.0);
    assert_eq!(read.inputs.day_hours, Some(24.0));
    assert_eq!(read.inputs.water_rank, Some(planet.drawn.water_rank));
}

/// An input outside its range, and an orbit inside its star, are `bad_request` naming the input.
async fn inputs_out_of_range_are_refused_naming_them(
    client: &mut TestClient,
    universe: &UniverseIdHex,
) {
    for (inputs, field) in [
        (
            PlanetInputsDto {
                eccentricity: 0.95,
                ..earth()
            },
            "inputs.eccentricity",
        ),
        (
            PlanetInputsDto {
                semi_major_axis_au: 0.002,
                ..earth()
            },
            "inputs.semi_major_axis_au",
        ),
    ] {
        let error = client
            .request(RequestBody::ViewerPlanet(ViewerPlanetRequest {
                universe: universe.clone(),
                inputs,
            }))
            .await
            .expect_err("refused");
        assert_eq!(error.code, ErrorCode::BadRequest, "{error:?}");
        assert_eq!(error.field.as_deref(), Some(field));
    }
}

/// The server keeps the last 16 planets of a universe's galaxy; the first of 17 is then unknown,
/// and generating it again serves it again.
async fn an_evicted_planet_is_unknown_until_it_is_generated_again(
    client: &mut TestClient,
    universe: &UniverseIdHex,
) {
    let inputs = |k: u64| PlanetInputsDto {
        seed: SeedHex::from_u64(0x0e15_1000 + k),
        ..earth()
    };
    let first = generated(client, universe, inputs(0)).await;
    for k in 1..=16 {
        generated(client, universe, inputs(k)).await;
    }
    let error = client
        .request(bodies_request(universe, &first.system))
        .await
        .expect_err("evicted");
    assert_eq!(error.code, ErrorCode::UnknownSystem, "{error:?}");
    let again = generated(client, universe, inputs(0)).await;
    assert_eq!(again, first, "the same inputs make the same planet");
    assert!(
        client
            .request(bodies_request(universe, &first.system))
            .await
            .is_ok()
    );
}

/// `viewer_randomize` draws the same planet for the same seed, a real one whose inputs
/// `viewer_inputs` reads back, and the drawn inputs make a planet.
async fn a_draw_is_repeatable_and_its_planet_s_inputs_read_back(
    client: &mut TestClient,
    universe: &UniverseIdHex,
) {
    let draw = |seed: u64| {
        RequestBody::ViewerRandomize(ViewerRandomizeRequest {
            universe: universe.clone(),
            draw_seed: SeedHex::from_u64(seed),
        })
    };
    let Ok(ResponseBody::ViewerRandomize(first)) = client.request(draw(3)).await else {
        panic!("a draw");
    };
    let Ok(ResponseBody::ViewerRandomize(again)) = client.request(draw(3)).await else {
        panic!("a draw");
    };
    assert_eq!(first, again);
    assert_eq!(first.inputs.day_hours, None, "the day is left to its draw");
    let Ok(ResponseBody::ViewerInputs(read)) = client
        .request(RequestBody::ViewerInputs(ViewerInputsRequest {
            universe: universe.clone(),
            body: first.source.clone(),
        }))
        .await
    else {
        panic!("the source planet's inputs");
    };
    assert_eq!(read.inputs.mass_mearth, first.inputs.mass_mearth);
    assert_eq!(
        read.inputs.semi_major_axis_au,
        first.inputs.semi_major_axis_au
    );
    assert_eq!(read.inputs.star_mass_msun, first.inputs.star_mass_msun);
    let planet = generated(client, universe, first.inputs.clone()).await;
    assert_ne!(planet.body, first.source, "a planet of its own");
    // A body that is no planet is refused naming it.
    let (system, _) = first.source.to_parts();
    let error = client
        .request(RequestBody::ViewerInputs(ViewerInputsRequest {
            universe: universe.clone(),
            body: BodyIdHex::from_parts(system, 0x0000),
        }))
        .await
        .expect_err("a star's index");
    assert_eq!(error.field.as_deref(), Some("body"), "{error:?}");
}
