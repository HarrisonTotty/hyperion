//! Benchmarks of the bake on real terrain (plan R10, T6.a), and its horizon map (T8.a).
//!
//! `bake/patch_65/<world>-<level>/<case>` bakes one patch of a synthetic world over a client's
//! field holding the cells under it with their margin (`hyperion_surface::bake::bake_terrain`):
//! R05's arrays from R09's heights, parent-band heights and gradients, and this plan's layers.
//! The cases are `weights` and `no-weights`, the class weights baked or skipped, each from a cache
//! a first bake of the patch filled, as a worker's next bake near it; `weights-cold`, from an
//! empty cache each iteration; and `double`, the high setting's baked offsets and normals at twice
//! the mesh's resolution, with the weights, warm. The low setting's other cases bake mesh normals
//! and no offsets. Beside them, for the breakdown, `heights` is R09's bake of the patch's heights
//! alone (`Synthesiser::bake_patch`) over the same client's field, and `heights-whole` the same
//! over the server's whole field, both warm. The patches are R09's `surface/patch` ones: the
//! Earth-like world's at the coarse
//! level plus four (12) inside the first coarse cell whose channels reach it, and at its finest
//! level (19), and the Moon-like world's at the coarse level plus four (10) on its first crater's
//! rim, against the brainstorm's 40 ms a patch on two of the UHD 620 laptop's cores (about 10 µs a
//! point with its gradient; the plan's Risks: the desktop records its own and fails only if over
//! it). Figures taken while other work shares the machine are provisional. CI compiles these and
//! never runs them: `just bench -- bake/patch`. Every `bake/patch_65` case skips the horizon map,
//! as T6.a measured it.
//!
//! `bake/horizon/<world>-<level>/<case>` bakes the same patches with the horizon map
//! (`HorizonLayer::Baked`) and without it, with the weights, at the low setting, so that the map's
//! cost is the difference, against the plan's quarter of the 40 ms (R10.T8.a): `without` and
//! `warm`, each from a cache a first bake of the patch with its map filled, so that the far field's
//! vertex heights are kept and the map costs its sweep and its march; `neighbour`, from a cache
//! that the bakes of the patch's eight neighbours filled, as a worker's next bake beside them;
//! and `cold` and `cold-without`, from an empty cache each iteration, where the far field asks
//! every vertex height it reads. The client's field holds every coarse cell within the far field's
//! reach of the patch, so that the far field reads real ground throughout. `just bench --
//! bake/horizon`.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod bench {
    use std::collections::BTreeSet;
    use std::hint::black_box;
    use std::time::Duration;

    use criterion::BatchSize;
    use criterion::Criterion;
    use hyperion_base::math;
    use hyperion_base::rng::DetailSeed;
    use hyperion_surface::bake::{HorizonLayer, TerrainBakeOptions, WeightsLayer, bake_terrain};
    use hyperion_surface::cube::{PatchKey, SampleGrid, unit_dir};
    use hyperion_surface::field::{
        CoarseField, Cover, CoverRange, FieldView, PartialField, ResolutionCode, cell_at_index,
        cell_index,
    };
    use hyperion_surface::patch::{BakeOptions, NormalScale, VertexPath};
    use hyperion_surface::synth::channels::{ChannelBody, deepest_level};
    use hyperion_surface::synth::{SynthCache, Synthesiser};
    use hyperion_surface::testing::{SyntheticWorld, synthetic_field};
    use hyperion_surface::wire::{decode_payload, encode_payload};

    /// The detail seed, R09's `synth` bench's.
    const SEED: DetailSeed = DetailSeed::new(0x6368_616e_6e65_6c73);

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

    /// R09's `synth` bench's patch: a level L + 4 cell inside the first coarse cell, in cell-index
    /// order, whose channels are evaluated to L + 4.
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
            .expect("the Earth-like world has a cell evaluated to L + 4");
        PatchKey::new(
            coarse.face(),
            first + 4,
            (coarse.i() << 4) + 5,
            (coarse.j() << 4) + 9,
        )
        .expect("a cell below a coarse cell")
    }

    /// The patch of `level` inside `at`'s cell, at the same place in it, as R09's bench takes it.
    fn below(at: PatchKey, level: u8) -> PatchKey {
        let down = level - at.level();
        let offset = |n: u32| if down == 0 { 0 } else { n % (1 << down) };
        PatchKey::new(
            at.face(),
            level,
            (at.i() << down) + offset(7),
            (at.j() << down) + offset(11),
        )
        .expect("a cell below a coarse cell")
    }

    /// A direction on the rim of `field`'s first listed crater, a rim radius's chord from its
    /// centre.
    fn on_a_rim(field: &CoarseField) -> [f64; 3] {
        let crater = &field.craters()[0];
        let c = crater.centre;
        let e = if c[2].abs() < 0.9 {
            [-c[1], c[0], 0.0]
        } else {
            [0.0, -c[2], c[1]]
        };
        let n = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
        let half_chord = 0.25 * crater.diameter.value() / field.header().radius().value();
        let (sin, cos) = math::sin_cos(2.0 * math::asin(half_chord.min(1.0)));
        unit_dir([0, 1, 2].map(|k| cos * c[k] + sin * e[k] / n))
    }

    /// A client's field of `field` holding the coarse cells under `key`'s samples, surveyed at
    /// 10 m, with their margin, from the payload a server would send.
    fn client(field: &CoarseField, key: PatchKey) -> PartialField {
        let level = field.header().level().get();
        let cells: BTreeSet<u32> = (0..=128)
            .flat_map(|y| (0..=128).map(move |x| (x, y)))
            .map(|(x, y)| {
                let dir = key.sample_dir(x, y, SampleGrid::Double);
                cell_index(PatchKey::containing(level, dir).expect("a coarse level"))
            })
            .collect();
        let code = ResolutionCode::from_resolution(hyperion_base::units::Metres::new(10.0))
            .expect("10 m is a resolution");
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

    /// The worlds and patches: the Earth-like world at L + 4 and 19, and the Moon-like on its first
    /// crater's rim at L + 4.
    fn cases() -> Vec<(&'static str, CoarseField, PatchKey)> {
        let earth = synthetic_field(SyntheticWorld::EarthLike);
        let at = channelled(&earth);
        let moon = synthetic_field(SyntheticWorld::MoonLike);
        let rim = on_a_rim(&moon);
        let moon_level = moon.header().level().get() + 4;
        let moon_patch = PatchKey::containing(moon_level, rim).expect("a level");
        vec![
            ("earth", earth.clone(), at),
            ("earth", earth, below(at, 19)),
            ("moon", moon, moon_patch),
        ]
    }

    pub fn patch_65(c: &mut Criterion) {
        let mut group = c.benchmark_group("bake/patch_65");
        group.sample_size(20);
        group.measurement_time(Duration::from_secs(10));
        for (k, (world, field, key)) in cases().into_iter().enumerate() {
            let view = client(&field, key);
            let synth = Synthesiser::new(&view, SEED);
            let name = format!("{world}-{}", key.level());
            let mut runs = vec![
                ("weights", LOW, WeightsLayer::Baked),
                ("no-weights", LOW, WeightsLayer::Skipped),
            ];
            if k == 0 {
                runs.push(("double", HIGH, WeightsLayer::Baked));
            }
            for (case, patch, weights) in runs {
                let opts = TerrainBakeOptions {
                    patch,
                    weights,
                    horizon: HorizonLayer::Skipped,
                };
                let mut warm = SynthCache::new();
                let first = bake_terrain(&synth, &mut warm, key, &opts, &[]).expect("held");
                assert!(first.layers().is_complete(), "the patch is surveyed");
                group.bench_function(format!("{name}/{case}"), |b| {
                    b.iter(|| black_box(bake_terrain(&synth, &mut warm, key, &opts, &[])));
                });
            }
            {
                let mut warm = SynthCache::new();
                synth.bake_patch(&mut warm, key).expect("held");
                group.bench_function(format!("{name}/heights"), |b| {
                    b.iter(|| black_box(synth.bake_patch(&mut warm, key)));
                });
                let whole = Synthesiser::new(&field, SEED);
                let mut warm = SynthCache::new();
                whole.bake_patch(&mut warm, key).expect("held");
                group.bench_function(format!("{name}/heights-whole"), |b| {
                    b.iter(|| black_box(whole.bake_patch(&mut warm, key)));
                });
            }
            if k == 0 {
                let opts = TerrainBakeOptions {
                    patch: LOW,
                    weights: WeightsLayer::Baked,
                    horizon: HorizonLayer::Skipped,
                };
                group.bench_function(format!("{name}/weights-cold"), |b| {
                    b.iter(|| {
                        black_box(bake_terrain(
                            &synth,
                            &mut SynthCache::new(),
                            key,
                            &opts,
                            &[],
                        ))
                    });
                });
            }
        }
        group.finish();
    }

    /// The patch's eight neighbours at its level on its face, those that exist.
    fn neighbours(key: PatchKey) -> Vec<PatchKey> {
        let last = (1_i64 << key.level()) - 1;
        let mut out = Vec::new();
        for dj in -1_i64..=1 {
            for di in -1_i64..=1 {
                let (i, j) = (i64::from(key.i()) + di, i64::from(key.j()) + dj);
                if (di, dj) != (0, 0) && (0..=last).contains(&i) && (0..=last).contains(&j) {
                    let index = |n: i64| u32::try_from(n).expect("an index on the face");
                    out.push(
                        PatchKey::new(key.face(), key.level(), index(i), index(j))
                            .expect("a patch on the face"),
                    );
                }
            }
        }
        out
    }

    /// A client's field holding the cells under `keys`' samples, as [`client`] makes it, and every
    /// coarse cell whose centre lies within the far field's reach of `keys[0]`'s centre, √(2 Δh ÷
    /// a), and two cells' widths more, so that the far field reads ground the view holds.
    fn client_within_reach(field: &CoarseField, keys: &[PatchKey]) -> PartialField {
        let header = field.header();
        let level = header.level().get();
        let figure = header.figure();
        let reach = (2.0 * header.realised_relief().value() / figure.equatorial_radius_m).sqrt();
        let cell_width = 1.7 * std::f64::consts::FRAC_PI_2 / f64::from(1_u32 << level);
        let centre = keys[0].vertex_dir(32, 32);
        let near = math::cos(reach + 2.0 * cell_width);
        let mut cells: BTreeSet<u32> = (0..header.level().cell_count())
            .filter(|&n| {
                let cell = cell_at_index(level, n).expect("a cell of the level");
                let d = cell.vertex_dir(32, 32);
                d[0] * centre[0] + d[1] * centre[1] + d[2] * centre[2] >= near
            })
            .collect();
        for &key in keys {
            for y in 0..=128 {
                for x in 0..=128 {
                    let dir = key.sample_dir(x, y, SampleGrid::Double);
                    cells.insert(cell_index(
                        PatchKey::containing(level, dir).expect("a coarse level"),
                    ));
                }
            }
        }
        let code = ResolutionCode::from_resolution(hyperion_base::units::Metres::new(10.0))
            .expect("10 m is a resolution");
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

    pub fn horizon(c: &mut Criterion) {
        let mut group = c.benchmark_group("bake/horizon");
        group.sample_size(10);
        group.measurement_time(Duration::from_secs(10));
        let with = TerrainBakeOptions {
            patch: LOW,
            weights: WeightsLayer::Baked,
            horizon: HorizonLayer::Baked,
        };
        let without = TerrainBakeOptions {
            horizon: HorizonLayer::Skipped,
            ..with
        };
        for (world, field, key) in cases() {
            let mut keys = vec![key];
            keys.extend(neighbours(key));
            let view = client_within_reach(&field, &keys);
            let synth = Synthesiser::new(&view, SEED);
            let name = format!("{world}-{}", key.level());
            let mut warm = SynthCache::new();
            let first = bake_terrain(&synth, &mut warm, key, &with, &[]).expect("held");
            assert!(first.layers().is_complete(), "the patch is surveyed");
            assert!(
                first.layers().horizon().is_complete(),
                "the view holds the far field's reach"
            );
            for (case, opts) in [("without", without), ("warm", with)] {
                group.bench_function(format!("{name}/{case}"), |b| {
                    b.iter(|| black_box(bake_terrain(&synth, &mut warm, key, &opts, &[])));
                });
            }
            let mut beside = SynthCache::new();
            for &n in keys.iter().filter(|&&n| n != key) {
                bake_terrain(&synth, &mut beside, n, &with, &[]).expect("held");
            }
            group.bench_function(format!("{name}/neighbour"), |b| {
                b.iter_batched(
                    || beside.clone(),
                    |mut cache| black_box(bake_terrain(&synth, &mut cache, key, &with, &[])),
                    BatchSize::LargeInput,
                );
            });
            for (case, opts) in [("cold", with), ("cold-without", without)] {
                group.bench_function(format!("{name}/{case}"), |b| {
                    b.iter(|| {
                        black_box(bake_terrain(
                            &synth,
                            &mut SynthCache::new(),
                            key,
                            &opts,
                            &[],
                        ))
                    });
                });
            }
        }
        group.finish();
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_group!(benches, bench::patch_65, bench::horizon);
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_main!(benches);

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn main() {}
