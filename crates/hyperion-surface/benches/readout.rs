//! Benchmarks of a view's terrain readout on a height worker (plan R10, T12; R10.T12.d's cost on the
//! Mac's height workers).
//!
//! `readout/<world>-<sight>/<part>` asks a field, as a client's worker holds it, the readout along a
//! line of sight, and its parts apart: `readout`, the whole query (`WorkerFields::ground_readout`),
//! asked again unchanged, as a view standing still asks it at 4 Hz; `march`, the slant range alone
//! (`WorkerFields::slant_range`, `reading.rs`' `march`); `reading` and `material`, the reading over
//! 2 m and the material at the line's foot, which make the rest of the readout's ground
//! (`ground_at`, whose capacity reads the 2 m reading itself); and natively `readout-cold`, the
//! whole query from a cache that holds nothing, its synthesis's parts already made, as the first ask
//! after the view moves. The worlds:
//!
//! - `moon`, `TERRAIN TEST`'s field (its two payloads' blocks, `testing::terrain_test_fixture`, a
//!   level-6 field surveyed about the edge of faces +x and +y), along the scene's sights: `low`,
//!   from 4 km over its ground track (28° N, 47° E) along the track 12° below the horizon, at the
//!   descent's end; and `high`, straight down from 50 km over the same point.
//! - `earth`, the Earth-like synthetic world's level-8 field surveyed whole at 50 m, as the body
//!   inspection surveys a served world, straight down at 268° E, 49° N, the Mac profile's place,
//!   from `orbit`, 3.4 radii from the centre, and `low`, 4 km over the datum: the size of the served
//!   wet world `hyperion-server`'s `served_readout` bench reads, on both targets.
//!
//! Figures taken while other work shares the machine are provisional. CI compiles these and never
//! runs them: `just bench -- readout/`.
//!
//! On the browser target, which has no Criterion, the warm parts run as `wasm-bindgen-test`'s
//! benchmark on the V8 that Electron ships, through R04's `node` shim, as `benches/synth.rs`'s do:
//! `cargo bench --target wasm32-unknown-unknown -p hyperion-surface --features testing --bench
//! readout`.

/// What the benches of both targets share: the fields, their sights and the parts.
mod shared {
    use hyperion_base::math;
    use hyperion_base::rng::DetailSeed;
    use hyperion_base::units::Metres;
    use hyperion_surface::cube::unit_dir;
    use hyperion_surface::field::ResolutionCode;
    use hyperion_surface::field::{BodyRef, Cover, CoverRange, FieldHeader, FieldView};
    use hyperion_surface::testing::{
        SyntheticWorld, TERRAIN_TEST_SEED, synthetic_field, terrain_test_fixture,
    };
    use hyperion_surface::wire::encode_payload;
    use hyperion_surface::worker::{
        GroundReadout, GroundReadoutQuery, SigmaSurvey, SlopeBaseline, WorkerFields,
    };

    /// `TERRAIN TEST`'s ground track, rad: 28° N, 47° E (`terrainScene.ts`).
    const TRACK_LATITUDE_RAD: f64 = 28.0 * core::f64::consts::PI / 180.0;
    const TRACK_LONGITUDE_RAD: f64 = 47.0 * core::f64::consts::PI / 180.0;

    /// How far below the local horizon `TERRAIN TEST`'s seat looks near the ground, rad: 12°.
    const LOW_PITCH_RAD: f64 = 12.0 * core::f64::consts::PI / 180.0;

    /// The Mac profile's place, rad: 268° E, 49° N.
    const PLACE_LONGITUDE_RAD: f64 = 268.0 * core::f64::consts::PI / 180.0;
    const PLACE_LATITUDE_RAD: f64 = 49.0 * core::f64::consts::PI / 180.0;

    /// The Earth-like world's detail seed, R09's `synth` bench's.
    const EARTH_SEED: DetailSeed = DetailSeed::new(0x6368_616e_6e65_6c73);

    /// A world's field as a worker holds it, with its body, seed and sights.
    pub struct World {
        pub name: &'static str,
        pub fields: WorkerFields,
        pub body: BodyRef,
        pub seed: DetailSeed,
        pub sights: Vec<(&'static str, GroundReadoutQuery)>,
    }

    /// The fields after `blocks` are posted in turn, the first starting the body's field.
    fn fields_of<'b>(body: BodyRef, blocks: impl IntoIterator<Item = &'b [u8]>) -> WorkerFields {
        let mut fields = WorkerFields::new();
        for (n, block) in blocks.into_iter().enumerate() {
            if n == 0 {
                fields.init(body, block, SigmaSurvey::Skipped)
            } else {
                fields.chunk(body, block)
            }
            .expect("the blocks are taken");
        }
        fields
    }

    /// `bytes` cut at each block's length, the `u32` at offset 6 of its frame.
    fn blocks(bytes: &[u8]) -> Vec<&[u8]> {
        let mut out = Vec::new();
        let mut rest = bytes;
        while !rest.is_empty() {
            let length = u32::from_le_bytes(rest[6..10].try_into().expect("a block's frame"));
            let (block, after) = rest.split_at(usize::try_from(length).expect("a block's length"));
            out.push(block);
            rest = after;
        }
        out
    }

    /// The readout's query along `direction` from `origin_m`, body-fixed, at the display's
    /// settings: 2 m, the reference pad, no season.
    fn query(header: &FieldHeader, origin_m: [f64; 3], direction: [f64; 3]) -> GroundReadoutQuery {
        let from_centre = origin_m.iter().map(|x| x * x).sum::<f64>().sqrt();
        GroundReadoutQuery {
            origin_m,
            direction,
            max: Metres::new(
                from_centre
                    + header.figure().equatorial_radius_m
                    + header.realised_relief().value(),
            ),
            slope_baseline: SlopeBaseline::TwoMetres,
            pad_width: Metres::new(1.0),
            season: None,
        }
    }

    /// The unit direction of latitude `lat` and east longitude `lon`, rad.
    fn direction_of(lat: f64, lon: f64) -> [f64; 3] {
        let (sin_lat, cos_lat) = math::sin_cos(lat);
        let (sin_lon, cos_lon) = math::sin_cos(lon);
        [cos_lat * cos_lon, cos_lat * sin_lon, sin_lat]
    }

    /// `TERRAIN TEST`'s field, `moon`, and its sights `low` and `high`.
    pub fn moon() -> World {
        let field = synthetic_field(SyntheticWorld::MoonLike);
        let fixture = terrain_test_fixture(&field);
        let body = fixture.body();
        let fields = fields_of(body, fixture.first_then_delta());
        let header = field.header();
        let radial = direction_of(TRACK_LATITUDE_RAD, TRACK_LONGITUDE_RAD);
        let (sin_lon, cos_lon) = math::sin_cos(TRACK_LONGITUDE_RAD);
        let west = [sin_lon, -cos_lon, 0.0];
        let figure = header.figure();
        let (sin_p, cos_p) = math::sin_cos(LOW_PITCH_RAD);
        let along = [0, 1, 2].map(|k| cos_p * west[k] - sin_p * radial[k]);
        let down = radial.map(|x| -x);
        World {
            name: "moon",
            fields,
            body,
            seed: TERRAIN_TEST_SEED,
            sights: vec![
                (
                    "low",
                    query(header, figure.surface_point(radial, 4_000.0), along),
                ),
                (
                    "high",
                    query(header, figure.surface_point(radial, 50_000.0), down),
                ),
            ],
        }
    }

    /// The Earth-like world's field surveyed whole at 50 m, `earth`, and its sights `orbit` and
    /// `low`.
    pub fn earth() -> World {
        let field = synthetic_field(SyntheticWorld::EarthLike);
        let header = field.header();
        let code =
            ResolutionCode::from_resolution(Metres::new(50.0)).expect("50 m is a resolution");
        let cover = Cover::from_ranges([
            CoverRange::new(0, header.level().cell_count(), code).expect("every cell")
        ])
        .expect("one range");
        let bytes = encode_payload(&field, &cover, None);
        let body = header.body();
        let fields = fields_of(body, blocks(&bytes));
        let radial = direction_of(PLACE_LATITUDE_RAD, PLACE_LONGITUDE_RAD);
        let down = radial.map(|x| -x);
        let figure = header.figure();
        let radius = figure.equatorial_radius_m;
        World {
            name: "earth",
            fields,
            body,
            seed: EARTH_SEED,
            sights: vec![
                (
                    "orbit",
                    query(header, radial.map(|x| 3.4 * radius * x), down),
                ),
                (
                    "low",
                    query(header, figure.surface_point(radial, 4_000.0), down),
                ),
            ],
        }
    }

    /// One part of the readout, asked of the fields.
    pub type Part<'p> = &'p mut dyn FnMut(&mut WorkerFields);

    /// Hands `bench` each part of `world`'s readout along `query` by name: the whole query, the
    /// march, and where it meets surveyed ground, the reading and the material at its foot.
    pub fn each_part(
        world: &mut World,
        query: &GroundReadoutQuery,
        mut bench: impl FnMut(&str, &mut WorkerFields, Part<'_>),
    ) {
        let (body, seed) = (world.body, world.seed);
        let fields = &mut world.fields;
        let first = fields
            .ground_readout(body, seed, query)
            .expect("the field is held");
        bench("readout", fields, &mut |f| {
            std::hint::black_box(f.ground_readout(body, seed, query).expect("held"));
        });
        bench("march", fields, &mut |f| {
            std::hint::black_box(
                f.slant_range(body, seed, &query.origin_m, &query.direction, query.max)
                    .expect("held"),
            );
        });
        if let GroundReadout::Ground(ground) = first {
            let at = ground.sight().at();
            bench("reading", fields, &mut |f| {
                std::hint::black_box(
                    f.reading(body, seed, at, SlopeBaseline::TwoMetres.metres())
                        .expect("held"),
                );
            });
            bench("material", fields, &mut |f| {
                std::hint::black_box(f.material(body, seed, at).expect("held"));
            });
        }
    }

    /// `world`'s fields with the synthesis's parts made and the cache empty, the cold readout's
    /// start: a readout along a line from far off the body pointing away builds the parts and
    /// reads no height.
    pub fn primed(world: &World) -> WorkerFields {
        let mut fields = world.fields.clone();
        let header = fields.field(world.body).expect("held").header().clone();
        let out = unit_dir([1.0, 1.0, 0.8]);
        let away = query(
            &header,
            out.map(|x| 4.0 * header.figure().equatorial_radius_m * x),
            out,
        );
        fields
            .ground_readout(world.body, world.seed, &away)
            .expect("held");
        fields
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod bench {
    use std::time::Duration;

    use criterion::{BatchSize, Criterion};

    use super::shared::{each_part, earth, moon, primed};

    pub fn readout(c: &mut Criterion) {
        let mut group = c.benchmark_group("readout");
        group.sample_size(20);
        group.measurement_time(Duration::from_secs(5));
        for mut world in [moon(), earth()] {
            let primed = primed(&world);
            let (body, seed) = (world.body, world.seed);
            for (sight, query) in world.sights.clone() {
                let name = format!("{}-{sight}", world.name);
                each_part(&mut world, &query, |part, fields, ask| {
                    group.bench_function(format!("{name}/{part}"), |b| {
                        b.iter(|| ask(fields));
                    });
                });
                group.bench_function(format!("{name}/readout-cold"), |b| {
                    b.iter_batched(
                        || primed.clone(),
                        |mut f| f.ground_readout(body, seed, &query),
                        BatchSize::LargeInput,
                    );
                });
            }
        }
        group.finish();
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_group!(benches, bench::readout);
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_main!(benches);

/// `readout` on the browser target, as `wasm-bindgen-test`'s benchmark (see the module
/// documentation).
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod browser {
    use wasm_bindgen_test::{Criterion, wasm_bindgen_bench};

    use super::shared::{each_part, earth, moon};

    /// The samples a part takes.
    const SAMPLES: usize = 20;

    /// Each part of the readout along each sight, warm, as the native `readout`.
    #[wasm_bindgen_bench]
    fn readout(c: &mut Criterion) {
        *c = std::mem::take(c).sample_size(SAMPLES);
        for mut world in [moon(), earth()] {
            for (sight, query) in world.sights.clone() {
                let name = format!("readout/{}-{sight}", world.name);
                each_part(&mut world, &query, |part, fields, ask| {
                    c.bench_function(&format!("{name}/{part}"), |b| {
                        b.iter(|| ask(fields));
                    });
                });
            }
        }
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn main() {}
