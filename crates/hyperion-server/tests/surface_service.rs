//! The surface service through its public API (rendering plan R09, R09.T17): the field it serves
//! from the reference worlds' inputs is the sim's coarse pass on the same inputs, bit for bit, with
//! R10.T10.a's albedo scale in its header, and with the production source, the serve gate open
//! from generator version 25 (P14.T54.a), a generated moon is served the sim's pass of its record's
//! inputs, bit for bit, read with its planet's record (R09.T10.f), and a giant answers that it has
//! no solid surface. A system's prefetch computes its bodies with a solid surface, nearest first,
//! to the same bits.

mod common;

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use common::surface::{
    ReferenceWorld, SEED, moon_system, reference_inputs, served_field, surface_system,
};
use common::{patience, patiently};
use hyperion_server::compute::{CpuPool, GalaxyKey};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_server::surface::{
    FieldKey, InputsSource, RECORD_TIME, RecordInputs, SurfaceJobError, SurfaceService,
    field_albedo_scale,
};
use hyperion_sim::id::BodyId;
use hyperion_sim::planetary::hooks::{detail_seed, surface_seed};
use hyperion_sim::planetary::record::{BodyRecord, Section};
use hyperion_sim::planetary::surface::{
    CoarseInputs, ReadCoarseInputsError, SurfaceInputsError, coarse_pass,
};
use hyperion_sim::planetary::{BodyIndex, SystemContext};
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

fn key(body: BodyId) -> FieldKey {
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

/// With the production source, the body's record, a generated moon with a solid surface is served
/// its field: the sim's coarse pass of its surface seed on the inputs `CoarseInputs::for_body`
/// reads from its record and its planet's, the one `CoarseInputs::planet_of` names, which the
/// service fetches beside it (R09.T10.f), with the albedo scale its record gives, bit for bit;
/// without its planet's record the reading refuses the moon before its crust. A giant answers that
/// it has no solid surface and runs no pass, under the production source and the reference inputs
/// alike.
#[tokio::test]
async fn a_generated_surface_is_served_the_sim_s_pass_of_its_record() {
    let pool = pool();
    let (system, moons) = (surface_system(), moon_system());
    let production = service(&pool, RecordInputs);
    let served = timeout(patience(), production.field(key(moons.moon), &moons.system))
        .await
        .expect("timed out computing a field")
        .expect("a generated moon the sim reads is served its field");
    let (ctx, planets) = moons.system.as_ref();
    let at = |body: BodyId| {
        let index = BodyIndex::try_from(body.body_index()).expect("a body's index");
        planets
            .body_at(ctx, index, RECORD_TIME)
            .expect("a body of the system resolves")
    };
    let moon = at(moons.moon);
    assert_eq!(
        CoarseInputs::planet_of(&moon).map(|planet| planet.body_id(planets.system())),
        Some(moons.planet)
    );
    assert_eq!(
        RecordInputs.inputs(&moon, None, ctx),
        Err(ReadCoarseInputsError::PlanetMissing(moons.planet))
    );
    let inputs = CoarseInputs::for_body(&moon, Some(&at(moons.planet)), ctx)
        .expect("the sim reads the moon with its planet's record");
    let own = coarse_pass(surface_seed(Seed::new(SEED), moons.moon), &inputs);
    let scale = field_albedo_scale(
        &own,
        moon.photometry(),
        detail_seed(Seed::new(SEED), moons.moon),
    );
    assert_eq!(
        served.header().albedo_scale().map(f64::to_bits),
        scale.map(f64::to_bits),
        "the served field's albedo scale"
    );
    let own = own
        .with_albedo_scale(scale)
        .expect("a solved scale is finite and positive");
    let (served, own) = (payload(&served), payload(&own));
    assert_eq!(served.len(), own.len(), "the payloads' lengths");
    assert_eq!(
        served.iter().zip(&own).position(|(a, b)| a != b),
        None,
        "the payloads' first differing byte"
    );

    let reference = service(&pool, reference_inputs(ReferenceWorld::Ceres));
    for (source, service) in [("production", &production), ("reference", &reference)] {
        let answer = timeout(
            patience(),
            service.field(key(system.gaseous), &system.system),
        )
        .await
        .expect("timed out reading a body's inputs");
        assert_eq!(
            answer.unwrap_err(),
            SurfaceJobError::Inputs(SurfaceInputsError::NoSolidSurface.into()),
            "the {source} source"
        );
    }
    let counters = production.counters();
    assert_eq!(
        (counters.computed(), counters.cache().entries()),
        (1, 1),
        "the moon's pass alone, kept: {counters:?}"
    );
    timeout(patience(), pool.shutdown()).await.unwrap().unwrap();
}

/// The reference Ceres's inputs for every body with a solid surface, a giant refused, recording
/// each body asked for, in order.
#[derive(Debug)]
struct Recording {
    asked: Arc<Mutex<Vec<BodyIndex>>>,
}

impl InputsSource for Recording {
    fn inputs(
        &self,
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError> {
        self.asked
            .lock()
            .expect("no test thread panics holding it")
            .push(record.index());
        reference_inputs(ReferenceWorld::Ceres).inputs(record, planet, ctx)
    }
}

/// A system's prefetch from beside its solid body reads every body with a surface seed for its
/// plan, nearest first, then computes the solid body's field first and each body with a solid
/// surface after it, nearest first, and none for a giant, a ring or a belt. The prefetched field
/// is the coarse pass on the same inputs bit for bit, and asked for once the prefetch has had it,
/// it costs one cache read (rendering plan R09, R09.T17).
#[tokio::test]
async fn a_surface_prefetch_of_a_system_computes_its_nearest_solid_bodies_bit_for_bit() {
    let pool = Arc::new(
        CpuPool::new(
            NonZeroUsize::new(3).expect("three workers"),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
        )
        .expect("the pool starts"),
    );
    let system = surface_system();
    let (ctx, planets) = system.system.as_ref();
    let solid = BodyIndex::try_from(system.solid.body_index()).expect("a body's index");
    let near = planets
        .position_at(ctx, solid, RECORD_TIME)
        .expect("the solid body resolves")
        .expect("the solid body is present at the record's time");
    let mut by_distance: Vec<(f64, BodyIndex)> = planets
        .indices()
        .into_iter()
        .filter(|&index| matches!(planets.surface_seed(index), Ok(Some(_))))
        .filter_map(|index| {
            let at = planets.position_at(ctx, index, RECORD_TIME).ok()??;
            Some((near.displacement_to(&at).length().value(), index))
        })
        .collect();
    by_distance.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let planned: Vec<BodyIndex> = by_distance.iter().map(|&(_, index)| index).collect();
    let grounded: Vec<BodyIndex> = planned
        .iter()
        .copied()
        .filter(|&index| {
            let record = planets
                .body_at(ctx, index, RECORD_TIME)
                .expect("a planned body resolves");
            !matches!(record.surface(), Section::NotApplicable)
        })
        .collect();
    let gaseous = BodyIndex::try_from(system.gaseous.body_index()).expect("a body's index");
    assert!(planned.contains(&gaseous) && !grounded.contains(&gaseous));
    assert_eq!(grounded.first(), Some(&solid));

    let asked = Arc::new(Mutex::new(Vec::new()));
    // A bound of two: the prefetch runs one pass at a time, in its order.
    let service = SurfaceService::new(
        Arc::clone(&pool),
        Arc::new(Recording {
            asked: Arc::clone(&asked),
        }),
        64 << 20,
        NonZeroUsize::new(2).expect("two passes"),
    );
    let galaxy = GalaxyKey::new(SEED, GENERATOR_VERSION);
    let _prefetch = service.prefetch_system(galaxy, &system.system, near, RECORD_TIME);
    let all = u64::try_from(grounded.len()).expect("a count of bodies");
    patiently("prefetching the system's fields", async {
        while service.counters().computed() < all {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let asked = asked
        .lock()
        .expect("no test thread panics holding it")
        .clone();
    assert_eq!(
        asked[..planned.len()],
        planned[..],
        "the plan reads each body with a surface seed once, nearest first"
    );
    assert_eq!(
        asked[planned.len()..],
        grounded[..],
        "then a pass for each body with a solid surface, nearest first"
    );

    let before = service.counters();
    let prefetched = patiently(
        "asking for a prefetched field",
        service.field(key(system.solid), &system.system),
    )
    .await
    .expect("a reference world has a field");
    let after = service.counters();
    assert_eq!(
        (
            after.cache().hits() - before.cache().hits(),
            after.cache().misses() - before.cache().misses(),
            after.computed()
        ),
        (1, 0, all),
        "one cache read"
    );
    let (served, own) = (
        payload(&prefetched),
        payload(&served_field(ReferenceWorld::Ceres, system.solid)),
    );
    assert_eq!(served.len(), own.len(), "the payloads' lengths");
    assert_eq!(
        served.iter().zip(&own).position(|(a, b)| a != b),
        None,
        "the payloads' first differing byte"
    );
    timeout(patience(), pool.shutdown()).await.unwrap().unwrap();
}
