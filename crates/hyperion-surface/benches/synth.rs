//! Benchmarks of the local synthesis's cost per point (plan R09, T6.b; T8 extends them).
//!
//! `surface/channels/through-<k>` evaluates the channels' incision with its gradient at the 65 × 65
//! vertices of one patch of the Earth-like synthetic world's level L + 4 (12), through each level
//! from the coarse level L (8) to L + 4, so that each level's cost is the difference from the one
//! before: the per-level breakdown. `cached` reads a cache a first bake of the patch filled, as a
//! worker's next query near it does; `bake` starts from an empty cache each iteration, so it
//! includes building the patch's cells, their grooves and their parent cells' means. The per-point
//! time is the reported time ÷ 4,225. Design note 14's target is 3.5 µs a point in WebAssembly,
//! which runs about 1.2–1.5 times native. Figures taken while other work shares the machine are
//! provisional. CI compiles these and never runs them.
//!
//! The browser target has no Criterion; there this file is an empty program.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod bench {
    use std::hint::black_box;

    use criterion::Criterion;
    use hyperion_base::rng::DetailSeed;
    use hyperion_surface::cube::PatchKey;
    use hyperion_surface::field::{CoarseField, FieldView, cell_at_index};
    use hyperion_surface::synth::channels::{ChannelBody, deepest_level};
    use hyperion_surface::synth::{Channels, SynthCache};
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
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_group!(benches, bench::channels);
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_main!(benches);

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn main() {}
