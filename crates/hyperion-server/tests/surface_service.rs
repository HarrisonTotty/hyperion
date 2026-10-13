//! The surface service through its public API (rendering plan R09, R09.T17): the field it serves
//! from the reference worlds' inputs is the sim's coarse pass on the same inputs, bit for bit, and
//! with the production source a generated body answers that its surface is not modelled.

mod common;

use std::num::NonZeroUsize;
use std::sync::Arc;

use common::patience;
use common::surface::{ReferenceWorld, SEED, reference_inputs, surface_system};
use hyperion_server::compute::{CpuPool, GalaxyKey};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_server::surface::{
    FieldKey, InputsSource, RecordInputs, SurfaceJobError, SurfaceService,
};
use hyperion_sim::planetary::hooks::surface_seed;
use hyperion_sim::planetary::record::RecordSection;
use hyperion_sim::planetary::surface::{SurfaceInputsError, coarse_pass};
use hyperion_sim::units::Metres;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_surface::field::{CoarseField, Cover, CoverRange, FieldView, ResolutionCode};
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

fn service(pool: &Arc<CpuPool>, source: impl InputsSource + 'static) -> SurfaceService {
    SurfaceService::new(Arc::clone(pool), Arc::new(source), 64 << 20)
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
/// Ceres-like and Moon-like worlds; one pass each, kept.
#[tokio::test]
async fn the_surface_service_serves_the_coarse_pass_bit_for_bit() {
    let pool = pool();
    let system = surface_system();
    for world in [ReferenceWorld::Ceres, ReferenceWorld::Moon] {
        let service = service(&pool, reference_inputs(world));
        let served = timeout(patience(), service.field(key(system.solid), &system.system))
            .await
            .expect("timed out computing a field")
            .expect("a reference world has a field");
        let own = coarse_pass(surface_seed(Seed::new(SEED), system.solid), &world.inputs());
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
/// surface; neither runs a pass or keeps anything. The reference inputs refuse the giant alike.
#[tokio::test]
async fn a_generated_surface_is_not_modelled_naming_its_section() {
    let pool = pool();
    let system = surface_system();
    let production = service(&pool, RecordInputs);
    for (body, refusal) in [
        (
            system.solid,
            SurfaceInputsError::NotModelled(RecordSection::Surface),
        ),
        (system.gaseous, SurfaceInputsError::NoSolidSurface),
    ] {
        let answer = timeout(patience(), production.field(key(body), &system.system))
            .await
            .expect("timed out reading a body's inputs");
        assert_eq!(
            answer.unwrap_err(),
            SurfaceJobError::Inputs(refusal),
            "{body}"
        );
    }
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
        SurfaceJobError::Inputs(SurfaceInputsError::NoSolidSurface)
    );
    timeout(patience(), pool.shutdown()).await.unwrap().unwrap();
}
