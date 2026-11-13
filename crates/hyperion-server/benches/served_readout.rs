//! A view's terrain readout on a served world (rendering plan R10, T12; R10.T12.d's cost on the
//! Mac's height workers): the readout a client's height worker answers over the field the surface
//! service serves, surveyed whole, as the body inspection surveys it.
//!
//! `readout/served-<body>-<sight>/<part>` asks the body's field, every coarse cell surveyed at 50 m
//! (the inspection's `orbital` pass) from the payload a server would send, the readout along a line
//! of sight straight down from the inspection's place, 268° east and 49° north, and its parts apart,
//! as `hyperion-surface`'s `readout` bench does on `TERRAIN TEST`: `readout`, the whole query, asked
//! again unchanged, as a view standing still asks it at 4 Hz; `march`, the slant range alone;
//! `reading` and `material` at the line's foot; and `readout-cold`, the whole query from a cache that
//! holds nothing, the synthesis's parts and the march's ceiling already made. The sights are the inspection's three windows
//! (the owner's Mac profile, 2026-11-01): `orbit`, 22.0 Mm from the centre; `close`, 6.76 Mm; and
//! `low`, 4 km over the datum. The world is the wet world R10.T15.b's runs fly,
//! `41ffecb200000002.0400` of the `Dev Fixture` universe, as `served_bake` computes it, but read
//! through [`CoarseInputs::for_body`] alone, as [`RecordInputs`](hyperion_server::surface::RecordInputs)
//! reads it from the serve gate's version (25) on: below it the server answers every generated body
//! not modelled. CI compiles these and never runs them: `cargo bench -p hyperion-server --bench
//! served_readout`.

use std::hint::black_box;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hyperion_server::compute::{CpuPool, GalaxyKey};
use hyperion_server::limits::{BULK_QUEUE_CAPACITY, INTERACTIVE_QUEUE_CAPACITY};
use hyperion_server::surface::{FieldKey, InputsSource, RECORD_TIME, SurfaceService};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::id::BodyId;
use hyperion_sim::math;
use hyperion_sim::planetary::record::{BodyRecord, Section};
use hyperion_sim::planetary::surface::{CoarseInputs, ReadCoarseInputsError};
use hyperion_sim::planetary::{BodyIndex, SystemContext, generate};
use hyperion_sim::rng::DetailSeed;
use hyperion_sim::units::Metres;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_surface::field::{BodyRef, ResolutionCode};
use hyperion_surface::field::{CoarseField, Cover, CoverRange, FieldHeader, FieldView};
use hyperion_surface::wire::encode_payload;
use hyperion_surface::worker::{
    GroundReadout, GroundReadoutQuery, SigmaSurvey, SlopeBaseline, WorkerFields,
};

/// The `Dev Fixture` universe's seed, `just place-ship`'s default.
const UNIVERSE_SEED: u64 = 0x4d2;

/// The wet world R10.T15.b's runs fly.
const BODY: &str = "41ffecb200000002.0400";

/// The inspection's place, rad: 268° east, 49° north (the Mac profile's `POSITION`).
const LONGITUDE_RAD: f64 = 268.0 * core::f64::consts::PI / 180.0;
const LATITUDE_RAD: f64 = 49.0 * core::f64::consts::PI / 180.0;

/// A body's coarse-pass inputs read from its record ([`CoarseInputs::for_body`]), with no serve gate.
#[derive(Debug)]
struct UngatedInputs;

impl InputsSource for UngatedInputs {
    fn inputs(
        &self,
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError> {
        CoarseInputs::for_body(record, planet, ctx)
    }
}

/// The body's coarse field as the surface service serves it past its gate, and its detail seed:
/// `served_bake`'s.
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
    let service = SurfaceService::new(pool, Arc::new(UngatedInputs), 64 << 20, NonZeroUsize::MIN);
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

/// A client's worker fields holding `field` surveyed whole at 50 m, from the payload a server would
/// send, each block posted in turn.
fn surveyed_whole(field: &CoarseField) -> WorkerFields {
    let cells = field.header().level().cell_count();
    let code = ResolutionCode::from_resolution(Metres::new(50.0)).expect("50 m is a resolution");
    let cover = Cover::from_ranges([CoverRange::new(0, cells, code).expect("every cell")])
        .expect("one range");
    let bytes = encode_payload(field, &cover, None);
    let body = field.header().body();
    let mut fields = WorkerFields::new();
    let mut rest = bytes.as_slice();
    let mut first = true;
    while !rest.is_empty() {
        // A block's length is the `u32` at offset 6 of its frame.
        let length = u32::from_le_bytes(rest[6..10].try_into().expect("a block's frame"));
        let (block, after) = rest.split_at(usize::try_from(length).expect("a block's length"));
        if first {
            fields.init(body, block, SigmaSurvey::Skipped)
        } else {
            fields.chunk(body, block)
        }
        .expect("the payload's blocks are taken");
        first = false;
        rest = after;
    }
    fields
}

/// The readout's query from `from_centre_m` straight down at the inspection's place, at the
/// display's settings: 2 m, the reference pad, no season.
fn query(header: &FieldHeader, from_centre_m: f64) -> GroundReadoutQuery {
    let (sin_lat, cos_lat) = math::sin_cos(LATITUDE_RAD);
    let (sin_lon, cos_lon) = math::sin_cos(LONGITUDE_RAD);
    let radial = [cos_lat * cos_lon, cos_lat * sin_lon, sin_lat];
    let figure = header.figure();
    GroundReadoutQuery {
        origin_m: radial.map(|x| from_centre_m * x),
        direction: radial.map(|x| -x),
        max: Metres::new(
            from_centre_m + figure.equatorial_radius_m + header.realised_relief().value(),
        ),
        slope_baseline: SlopeBaseline::TwoMetres,
        pad_width: Metres::new(1.0),
        season: None,
    }
}

fn served_readout(c: &mut Criterion) {
    let (field, seed) = served();
    let header = field.header().clone();
    let body: BodyRef = header.body();
    let mut fields = surveyed_whole(&field);
    drop(field);
    // The parts made, the cache empty: the cold readout's start, from a sight off the body.
    let away = query(&header, 4.0 * header.figure().equatorial_radius_m);
    let away = GroundReadoutQuery {
        direction: away.direction.map(|x| -x),
        ..away
    };
    fields.ground_readout(body, seed, &away).expect("held");
    let primed = fields.clone();
    let mut group = c.benchmark_group("readout");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));
    let datum_m = header.figure().equatorial_radius_m;
    let index = BODY.rsplit('.').next().expect("a body index");
    for (sight, from_centre_m) in [
        ("orbit", 22.0e6),
        ("close", 6.76e6),
        ("low", datum_m + 4_000.0),
    ] {
        let name = format!("served-{index}-{sight}");
        let q = query(&header, from_centre_m);
        let first = fields.ground_readout(body, seed, &q).expect("held");
        group.bench_function(format!("{name}/readout"), |b| {
            b.iter(|| black_box(fields.ground_readout(body, seed, &q)));
        });
        group.bench_function(format!("{name}/march"), |b| {
            b.iter(|| black_box(fields.slant_range(body, seed, &q.origin_m, &q.direction, q.max)));
        });
        if let GroundReadout::Ground(ground) = first {
            let at = ground.sight().at();
            group.bench_function(format!("{name}/reading"), |b| {
                b.iter(|| {
                    black_box(fields.reading(body, seed, at, SlopeBaseline::TwoMetres.metres()))
                });
            });
            group.bench_function(format!("{name}/material"), |b| {
                b.iter(|| black_box(fields.material(body, seed, at)));
            });
        }
        group.bench_function(format!("{name}/readout-cold"), |b| {
            b.iter_batched_ref(
                || primed.clone(),
                |f| black_box(f.ground_readout(body, seed, &q)),
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, served_readout);
criterion_main!(benches);
