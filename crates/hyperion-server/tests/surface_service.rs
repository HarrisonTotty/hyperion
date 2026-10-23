//! The surface service through its public API (rendering plan R09, R09.T17): the field it serves
//! from the reference worlds' inputs is the sim's coarse pass on the same inputs, bit for bit, with
//! R10.T10.a's albedo scale in its header, and with the production source a generated body answers
//! that its surface is not modelled, a moon too once given its planet's record (R09.T10.f).

mod common;

use std::num::NonZeroUsize;
use std::sync::Arc;

use common::patience;
use common::surface::{
    ReferenceWorld, SEED, moon_system, reference_inputs, served_field, surface_system,
};
use hyperion_server::compute::{CpuPool, GalaxyKey};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_server::surface::{
    FieldKey, InputsSource, RECORD_TIME, RecordInputs, SurfaceJobError, SurfaceService,
};
use hyperion_sim::planetary::BodyIndex;
use hyperion_sim::planetary::hooks::{detail_seed, surface_seed};
use hyperion_sim::planetary::record::{RecordSection, Section};
use hyperion_sim::planetary::surface::{
    CoarseInputs, ReadCoarseInputsError, SurfaceInputsError, coarse_pass,
};
use hyperion_sim::units::Metres;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_surface::field::{CoarseField, Cover, CoverRange, FieldView, ResolutionCode};
use hyperion_surface::material::{albedo_scale, class_fractions};
use hyperion_surface::synth::{SynthCache, Synthesiser};
use hyperion_surface::wire::encode_payload;
use tokio::time::timeout;

fn pool() -> Arc<CpuPool> {
    Arc::new(
        CpuPool::new(
            NonZeroUsize::new(2).expect("two workers"),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
        )
        .expect("the pool starts"),
    )
}

/// A service on `pool` of the given source, running one pass at a time, the default bound on the
/// pool's two workers.
fn service(pool: &Arc<CpuPool>, source: impl InputsSource + 'static) -> SurfaceService {
    SurfaceService::new(
        Arc::clone(pool),
        Arc::new(source),
        64 << 20,
        NonZeroUsize::MIN,
    )
}

fn key(body: hyperion_sim::id::BodyId) -> FieldKey {
    FieldKey::new(GalaxyKey::new(SEED, GENERATOR_VERSION), body)
}

/// The whole field as the wire carries it, every cell surveyed at 1 km: the encoder writes each
/// value's bits, so two fields that encode alike are one field, bit for bit.
fn payload(field: &CoarseField) -> Vec<u8> {
    let cells = field.header().level().cell_count();
    let code = ResolutionCode::from_resolution(Metres::new(1_000.0)).expect("1 km has a code");
    let all = Cover::from_ranges([CoverRange::new(0, cells, code).expect("a level has cells")])
        .expect("one range is sorted");
    encode_payload(field, &all, None)
}

/// The field the service serves for a body, from a reference world's inputs, is the sim's coarse
/// pass of that body's surface seed in the universe's seed on the same inputs, bit for bit, on the
/// Ceres-like and Moon-like worlds, its header carrying the albedo scale that makes the airless
/// body's materials give its record's geometric albedo (R10.T10.a): `albedo_scale` of its class
/// fractions under its detail seed; one pass each, kept.
#[tokio::test]
async fn the_surface_service_serves_the_coarse_pass_bit_for_bit() {
    let pool = pool();
    let system = surface_system();
    let (ctx, planets) = system.system.as_ref();
    let index = BodyIndex::try_from(system.solid.body_index()).expect("a body's index");
    let record = planets
        .body_at(ctx, index, RECORD_TIME)
        .expect("the solid body resolves");
    let Section::Ok(photometry) = record.photometry() else {
        panic!("a solid body's photometry is modelled");
    };
    let p = photometry.geometric_albedo();
    for world in [ReferenceWorld::Ceres, ReferenceWorld::Moon] {
        let service = service(&pool, reference_inputs(world));
        let served = timeout(patience(), service.field(key(system.solid), &system.system))
            .await
            .expect("timed out computing a field")
            .expect("a reference world has a field");
        let unscaled = coarse_pass(surface_seed(Seed::new(SEED), system.solid), &world.inputs());
        let synth = Synthesiser::new(&unscaled, detail_seed(Seed::new(SEED), system.solid));
        let fractions = class_fractions(&synth, &mut SynthCache::new()).expect("every cell held");
        let solved = albedo_scale(&fractions, unscaled.header().palette(), [p.b, p.v, p.r])
            .expect("a reference palette names the ground");
        assert_eq!(
            served.header().albedo_scale().map(f64::to_bits),
            Some(solved.scale().to_bits()),
            "{world:?}: the served field's albedo scale"
        );
        let own = served_field(world, system.solid);
        assert_eq!(
            own.header().albedo_scale().map(f64::to_bits),
            Some(solved.scale().to_bits()),
            "{world:?}"
        );
        assert_eq!(served.header().level(), own.header().level(), "{world:?}");
        let (served, own) = (payload(&served), payload(&own));
        assert_eq!(served.len(), own.len(), "{world:?}: the payloads' lengths");
        assert_eq!(
            served.iter().zip(&own).position(|(a, b)| a != b),
            None,
            "{world:?}: the payloads' first differing byte"
        );
        let counters = service.counters();
        assert_eq!(
            (counters.computed(), counters.cache().entries()),
            (1, 1),
            "{world:?}: {counters:?}"
        );
    }
    timeout(patience(), pool.shutdown()).await.unwrap().unwrap();
}

/// With the production source, the body's record, a generated body with a solid surface answers
/// that its surface section is not modelled, naming the section, and a giant that it has no solid
/// surface; neither runs a pass or keeps anything. A generated moon answers as a planet does: the
/// service fetches its planet's record, the one `CoarseInputs::planet_of` names, which `for_body`
/// checks before its crust (R09.T10.f). The reference inputs refuse the giant alike.
#[tokio::test]
async fn a_generated_surface_is_not_modelled_naming_its_section() {
    let pool = pool();
    let (system, moons) = (surface_system(), moon_system());
    let production = service(&pool, RecordInputs);
    let not_modelled = SurfaceInputsError::NotModelled(RecordSection::Surface);
    for (body, of, refusal) in [
        (system.solid, &system.system, not_modelled),
        (
            system.gaseous,
            &system.system,
            SurfaceInputsError::NoSolidSurface,
        ),
        (moons.moon, &moons.system, not_modelled),
    ] {
        let answer = timeout(patience(), production.field(key(body), of))
            .await
            .expect("timed out reading a body's inputs");
        assert_eq!(
            answer.unwrap_err(),
            SurfaceJobError::Inputs(ReadCoarseInputsError::Section(refusal)),
            "{body}"
        );
    }
    // The moon's planet is the one `planet_of` names, and without its record the reading refuses
    // the moon before its crust.
    let (ctx, planets) = moons.system.as_ref();
    let index = BodyIndex::try_from(moons.moon.body_index()).expect("a body's index");
    let moon = planets
        .body_at(ctx, index, RECORD_TIME)
        .expect("the moon resolves");
    assert_eq!(
        CoarseInputs::planet_of(&moon).map(|planet| planet.body_id(planets.system())),
        Some(moons.planet)
    );
    assert_eq!(
        RecordInputs.inputs(&moon, None, ctx),
        Err(ReadCoarseInputsError::PlanetMissing(moons.planet))
    );
    let counters = production.counters();
    assert_eq!(
        (counters.computed(), counters.cache().entries()),
        (0, 0),
        "{counters:?}"
    );
    let reference = service(&pool, reference_inputs(ReferenceWorld::Ceres));
    let answer = timeout(
        patience(),
        reference.field(key(system.gaseous), &system.system),
    )
    .await
    .expect("timed out reading a body's inputs");
    assert_eq!(
        answer.unwrap_err(),
        SurfaceJobError::Inputs(SurfaceInputsError::NoSolidSurface.into())
    );
    timeout(patience(), pool.shutdown()).await.unwrap().unwrap();
}
