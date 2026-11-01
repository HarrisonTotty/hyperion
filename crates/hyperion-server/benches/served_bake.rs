//! The bake on a served world (rendering plan R10, T6.a's bench after the serve gate): one patch of
//! a generated body's coarse field, as the surface service serves it, baked as the client's height
//! workers bake it.
//!
//! `bake/served/<body>-<level>/<case>` bakes the patch over a client's field holding the coarse
//! cells under its samples with their margin, surveyed at 10 m, from the payload the server would
//! send (`hyperion_surface::bake::bake_terrain`), each case warm from a first bake of the patch, as
//! a worker's next bake near it: `weights` and `no-weights`, the low setting's mesh normals with
//! the class weights baked or skipped; `double`, the high setting's baked offsets and normals at
//! twice the mesh's resolution, with the weights; and, for the breakdown, `heights`, R09's bake of
//! the patch's heights alone (`Synthesiser::bake_patch`) over the same client's field. Every case
//! skips the horizon map, as `hyperion-surface`'s `bake/patch_65` does, whose Earth-like world's
//! cases these stand beside.
//!
//! The world is the wet world R10.T15.b's runs fly, `41ffecb200000002.0400` of the `Dev Fixture`
//! universe (seed `0x4d2`, `just place-ship`'s default): a temperate level-8 planet with a sea. Its
//! field is computed once, before the timing, by a surface service on a pool of the machine's
//! threads less one, from the body's record at the epoch (`RecordInputs`, the server's source), so
//! the bench's field is the one `surface_field` serves. The patch is at the coarse level plus four
//! (12), at the same place as the Earth-like world's: inside the first coarse cell, in cell-index
//! order, whose channels are evaluated to L + 4. CI compiles these and never runs them:
//! `just bench -- bake/`, or this bench alone with
//! `cargo bench -p hyperion-server --bench served_bake`.

use std::collections::BTreeSet;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use hyperion_server::compute::{CpuPool, GalaxyKey};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_server::surface::{FieldKey, RECORD_TIME, RecordInputs, SurfaceService};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::id::BodyId;
use hyperion_sim::planetary::record::Section;
use hyperion_sim::planetary::{BodyIndex, SystemContext, generate};
use hyperion_sim::rng::DetailSeed;
use hyperion_sim::units::Metres;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_surface::bake::{
    HorizonLayer, NormalsLayer, TerrainBakeOptions, WeightsLayer, bake_terrain,
};
use hyperion_surface::cube::{PatchKey, SampleGrid};
use hyperion_surface::field::{
    CoarseField, Cover, CoverRange, FieldView, PartialField, ResolutionCode, cell_at_index,
    cell_index,
};
use hyperion_surface::patch::{BakeOptions, NormalScale, VertexPath};
use hyperion_surface::synth::channels::{ChannelBody, deepest_level};
use hyperion_surface::synth::{SynthCache, Synthesiser};
use hyperion_surface::wire::{decode_payload, encode_payload};

/// The `Dev Fixture` universe's seed, `just place-ship`'s default.
const UNIVERSE_SEED: u64 = 0x4d2;

/// The wet world R10.T15.b's runs fly.
const BODY: &str = "41ffecb200000002.0400";

/// The low setting's bake: mesh normals and no offsets.
const LOW: BakeOptions = BakeOptions {
    vertex_path: VertexPath::FaceDifferences,
    normals: NormalScale::Mesh,
    skirt_m: 0.0,
};

/// The high setting's: baked offsets and normals at twice the mesh's resolution.
const HIGH: BakeOptions = BakeOptions {
    vertex_path: VertexPath::BakedOffsets,
    normals: NormalScale::Double,
    skirt_m: 0.0,
};

/// The body's coarse field as the surface service serves it, and its detail seed.
fn served() -> (CoarseField, DetailSeed) {
    let body: BodyId = BODY.parse().expect("a body ID");
    let seed = Seed::new(UNIVERSE_SEED);
    let galaxy = Galaxy::new(seed);
    let ctx = SystemContext::for_system(&galaxy, body.system()).expect("the system exists");
    let planets = generate(seed, &ctx);
    let index = BodyIndex::try_from(body.body_index()).expect("a body index");
    let record = planets
        .body_at(&ctx, index, RECORD_TIME)
        .expect("the body resolves");
    let Section::Ok(hooks) = record.hooks() else {
        panic!("{BODY} has a surface, and so its hooks");
    };
    let detail = hooks.detail_seed();
    let workers = std::thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .saturating_sub(1)
        .max(1);
    let pool = Arc::new(
        CpuPool::new(
            NonZeroUsize::new(workers).expect("at least one worker"),
            INTERACTIVE_QUEUE_CAPACITY,
            BULK_QUEUE_CAPACITY,
        )
        .expect("the pool starts"),
    );
    let service = SurfaceService::new(pool, Arc::new(RecordInputs), 64 << 20, NonZeroUsize::MIN);
    let key = FieldKey::new(GalaxyKey::new(UNIVERSE_SEED, GENERATOR_VERSION), body);
    let system = Arc::new((ctx, planets));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("a runtime");
    let field = runtime
        .block_on(service.field(key, &system))
        .expect("the body's field is served");
    (Arc::unwrap_or_clone(field), detail)
}

/// The patch at L + 4 inside the first coarse cell, in cell-index order, whose channels are
/// evaluated to L + 4, at the place `bake/patch_65` takes in it.
fn channelled(field: &CoarseField) -> PatchKey {
    let header = field.header();
    let first = header.level().get();
    let coarse = (0..header.level().cell_count())
        .map(|n| cell_at_index(first, n).expect("a cell of the level"))
        .find(|&c| {
            field
                .cell(c)
                .is_some_and(|cell| deepest_level(cell, ChannelBody::of(header)) >= first + 4)
        })
        .expect("the wet world has a cell evaluated to L + 4");
    PatchKey::new(
        coarse.face(),
        first + 4,
        (coarse.i() << 4) + 5,
        (coarse.j() << 4) + 9,
    )
    .expect("a cell below a coarse cell")
}

/// A client's field of `field` holding the coarse cells under `key`'s samples, surveyed at 10 m,
/// with their margin, from the payload a server would send.
fn client(field: &CoarseField, key: PatchKey) -> PartialField {
    let level = field.header().level().get();
    let cells: BTreeSet<u32> = (0..=128)
        .flat_map(|y| (0..=128).map(move |x| (x, y)))
        .map(|(x, y)| {
            let dir = key.sample_dir(x, y, SampleGrid::Double);
            cell_index(PatchKey::containing(level, dir).expect("a coarse level"))
        })
        .collect();
    let code = ResolutionCode::from_resolution(Metres::new(10.0)).expect("10 m is a resolution");
    let cover = Cover::from_ranges(
        cells
            .iter()
            .map(|&i| CoverRange::new(i, i + 1, code).expect("a cell's range")),
    )
    .expect("ranges in order");
    let blocks = decode_payload(&encode_payload(field, &cover, None)).expect("a payload");
    let header = blocks[0]
        .header()
        .expect("block 0 carries the header")
        .clone();
    let mut view = PartialField::new(header);
    for block in &blocks {
        view.insert(block).expect("a block of the payload");
    }
    view
}

fn served_bake(c: &mut Criterion) {
    let (field, seed) = served();
    let key = channelled(&field);
    let view = client(&field, key);
    let synth = Synthesiser::new(&view, seed);
    let mut group = c.benchmark_group("bake/served");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));
    let name = format!(
        "{}-{}",
        BODY.rsplit('.').next().expect("a body index"),
        key.level()
    );
    for (case, patch, weights) in [
        ("weights", LOW, WeightsLayer::Baked),
        ("no-weights", LOW, WeightsLayer::Skipped),
        ("double", HIGH, WeightsLayer::Baked),
    ] {
        let opts = TerrainBakeOptions {
            patch,
            weights,
            horizon: HorizonLayer::Skipped,
            normals: NormalsLayer::Baked,
        };
        let mut warm = SynthCache::new();
        let first = bake_terrain(&synth, &mut warm, key, &opts, &[]).expect("held");
        assert!(first.layers().is_complete(), "the patch is surveyed");
        group.bench_function(format!("{name}/{case}"), |b| {
            b.iter(|| black_box(bake_terrain(&synth, &mut warm, key, &opts, &[])));
        });
    }
    let mut warm = SynthCache::new();
    synth.bake_patch(&mut warm, key).expect("held");
    group.bench_function(format!("{name}/heights"), |b| {
        b.iter(|| black_box(synth.bake_patch(&mut warm, key)));
    });
    group.finish();
}

criterion_group!(benches, served_bake);
criterion_main!(benches);
