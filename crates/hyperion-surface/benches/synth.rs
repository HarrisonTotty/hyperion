//! Benchmarks of the local synthesis's cost per point and per patch (plan R09, T6.b and T8).
//!
//! `surface/channels/through-<k>` evaluates the channels' incision with its gradient at the 65 × 65
//! vertices of one patch of the Earth-like synthetic world's level L + 4 (12), through each level
//! from the coarse level L (8) to L + 4, so that each level's cost is the difference from the one
//! before: the per-level breakdown. `cached` reads a cache a first bake of the patch filled, as a
//! worker's next query near it does; `bake` starts from an empty cache each iteration, so it
//! includes building the patch's cells, their grooves and their parent cells' means. The per-point
//! time is the reported time ÷ 4,225. Design note 14's target is 3.5 µs a point in WebAssembly,
//! which runs about 1.2–1.5 times native.
//!
//! `surface/point/<world>/<part>-<n>` (R09.T8) evaluates each part of the whole height function,
//! and the whole, at the 65 × 65 vertices of a patch of a world at level n, from a cache a first
//! bake filled: the base elevation, the structural octaves, the channels' incision, the small
//! craters (also `craters-uncached`, each point from an empty crater cache, the cell draw) and the
//! coarse craters' rims through the level's set, and `height` their sum, against Design note 14's
//! targets in WebAssembly (interpolation 0.2 µs, relief 1.5 µs, channels 3.5 µs, craters 2 µs)
//! and the brainstorm's 10 µs a point. `surface/patch/<world>/<n>/{warm,cold}` bakes the patch's
//! heights, parent-band heights and gradients (`Synthesiser::bake_patch`), 4,225 queries at its
//! level and 1,089 at its parent's, from the warm cache and from an empty one, against the
//! brainstorm's 40 ms a patch. The worlds are the Earth-like, at the coarse level plus four (12)
//! and its band limit's level (19), the deepest any patch needs, and the Moon-like on a coarse
//! crater's rim, at 10 and 17. Figures taken while other work shares the machine are provisional.
//! CI compiles these and never runs them.
//!
//! The browser target has no Criterion; there this file is an empty program.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod bench {
    use std::hint::black_box;

    use criterion::measurement::WallTime;
    use criterion::{BenchmarkGroup, Criterion};
    use hyperion_base::math;
    use hyperion_base::rng::DetailSeed;
    use hyperion_surface::cube::{PatchKey, unit_dir};
    use hyperion_surface::field::{CoarseField, FieldView, cell_at_index};
    use hyperion_surface::synth::channels::{ChannelBody, deepest_level};
    use hyperion_surface::synth::craters::CraterCache;
    use hyperion_surface::synth::interp::base_elevation;
    use hyperion_surface::synth::{BandLevel, Channels, SynthCache, Synthesiser};
    use hyperion_surface::testing::{SyntheticWorld, synthetic_field};

    /// The detail seed the bench draws from, the channels' tests' own.
    const SEED: DetailSeed = DetailSeed::new(0x6368_616e_6e65_6c73);

    /// The patch: a level L + 4 cell inside the first coarse cell, in cell-index order, whose
    /// channels are evaluated to L + 4.
    fn key(field: &CoarseField) -> PatchKey {
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

    fn directions(key: PatchKey) -> Vec<[f64; 3]> {
        (0..=64_u8)
            .flat_map(|y| (0..=64_u8).map(move |x| key.vertex_dir(x, y)))
            .collect()
    }

    pub fn channels(c: &mut Criterion) {
        let field = synthetic_field(SyntheticWorld::EarthLike);
        let channels = Channels::new(field.header(), SEED);
        let first = channels.first_level();
        let dirs = directions(key(&field));
        let mut group = c.benchmark_group("surface");
        for through in first..=first + 4 {
            let mut warm = SynthCache::new();
            for d in &dirs {
                channels
                    .incision_at(&field, *d, through, &mut warm)
                    .expect("the whole field is held");
            }
            group.bench_function(format!("channels/through-{through}/cached"), |b| {
                b.iter(|| {
                    let mut sum = 0.0;
                    for d in &dirs {
                        sum += channels
                            .incision_at(&field, *d, through, &mut warm)
                            .expect("the whole field is held")
                            .height_m;
                    }
                    black_box(sum)
                });
            });
            group.bench_function(format!("channels/through-{through}/bake"), |b| {
                b.iter(|| {
                    let mut cache = SynthCache::new();
                    let mut sum = 0.0;
                    for d in &dirs {
                        sum += channels
                            .incision_at(&field, *d, through, &mut cache)
                            .expect("the whole field is held")
                            .height_m;
                    }
                    black_box(sum)
                });
            });
        }
        group.finish();
    }

    /// The patch of `level` (L + 4 or finer) inside [`key`]'s cell, at the same place in it.
    fn patch_at(field: &CoarseField, level: u8) -> PatchKey {
        let at_l4 = key(field);
        let down = level - at_l4.level();
        let offset = |n: u32| if down == 0 { 0 } else { n % (1 << down) };
        PatchKey::new(
            at_l4.face(),
            level,
            (at_l4.i() << down) + offset(7),
            (at_l4.j() << down) + offset(11),
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

    /// Each part of the height function, and the whole, at the vertices of `patch` of `field`
    /// from a cache a first bake filled, the small craters' also from an empty crater cache (the
    /// uncached cell draw), then the patch's bake from that cache and from an empty one.
    fn parts_and_patch(
        group: &mut BenchmarkGroup<'_, WallTime>,
        world: &str,
        field: &CoarseField,
        patch: PatchKey,
    ) {
        let synth = Synthesiser::new(field, SEED);
        let level = patch.level();
        let band = BandLevel::new(level).expect("a band level");
        let set = *synth.level_set(band);
        let dirs = directions(patch);
        let mut warm = SynthCache::new();
        synth
            .bake_patch(&mut warm, patch)
            .expect("the whole field is held");
        let mut part = |name: &str, f: &mut dyn FnMut(&mut SynthCache, [f64; 3]) -> f64| {
            group.bench_function(format!("point/{world}/{name}-{level}"), |b| {
                b.iter(|| {
                    let mut sum = 0.0;
                    for d in &dirs {
                        sum += f(&mut warm, *d);
                    }
                    black_box(sum)
                });
            });
        };
        part("base", &mut |_, d| {
            base_elevation(field, d)
                .expect("the whole field is held")
                .height_m
        });
        if let Some(through) = set.relief_through() {
            part("relief", &mut |cache, d| {
                synth
                    .relief()
                    .at(field, d, through, cache)
                    .expect("the whole field is held")
                    .height_m
            });
        }
        if let Some(through) = set.channels_through() {
            part("channels", &mut |cache, d| {
                synth
                    .channels()
                    .incision_at(field, d, through, cache)
                    .expect("the whole field is held")
                    .height_m
            });
        }
        if let Some(through) = set.craters_through() {
            let mut craters = CraterCache::new();
            part("craters", &mut |_, d| {
                synth.craters().at(d, band, through, &mut craters).height_m
            });
            part("craters-uncached", &mut |_, d| {
                synth
                    .craters()
                    .at(d, band, through, &mut CraterCache::new())
                    .height_m
            });
        }
        part("rims", &mut |_, d| {
            synth
                .rims()
                .at(field, d, band)
                .expect("the whole field is held")
                .height_m
        });
        part("height", &mut |cache, d| {
            synth
                .height_at(cache, d, band)
                .expect("the whole field is held")
                .height_m
        });
        group.bench_function(format!("patch/{world}/{level}/warm"), |b| {
            b.iter(|| black_box(synth.bake_patch(&mut warm, patch)));
        });
        group.bench_function(format!("patch/{world}/{level}/cold"), |b| {
            b.iter(|| black_box(synth.bake_patch(&mut SynthCache::new(), patch)));
        });
    }

    pub fn point_and_patch(c: &mut Criterion) {
        let mut group = c.benchmark_group("surface");
        let earth = synthetic_field(SyntheticWorld::EarthLike);
        let coarse = earth.header().level().get();
        for level in [coarse + 4, 19] {
            parts_and_patch(&mut group, "earth", &earth, patch_at(&earth, level));
        }
        let moon = synthetic_field(SyntheticWorld::MoonLike);
        let rim = on_a_rim(&moon);
        let coarse = moon.header().level().get();
        for level in [coarse + 4, 17] {
            let patch = PatchKey::containing(level, rim).expect("a level is a level");
            parts_and_patch(&mut group, "moon", &moon, patch);
        }
        group.finish();
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_group!(benches, bench::channels, bench::point_and_patch);
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_main!(benches);

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn main() {}
