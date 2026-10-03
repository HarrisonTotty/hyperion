//! Benchmarks of the test planet's cost per point (plan R05, T3.c).
//!
//! `test_planet/cached` and `test_planet/uncached` each evaluate the height and its gradient at
//! the 65 × 65 vertices of one finest-level (19) patch, the points of one bake's heights, with a
//! lattice cache covering the patch and with none; the per-point time is the reported time ÷
//! 4,225. The research estimate is about 2 µs a point cached and 6 µs uncached, and the budget
//! 10 µs; above it is a finding for T16 (Design notes 13 and 25). Figures taken while other work
//! shares the machine are provisional (Design note 27). CI compiles these and never runs them.
//!
//! The browser target has no Criterion; there this file is an empty program.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod bench {
    use std::hint::black_box;

    use criterion::Criterion;
    use hyperion_surface::cube::{Face, PatchKey};
    use hyperion_surface::noise::LatticeCache;
    use hyperion_surface::test_planet::TEST_PLANET;

    /// The level-19 patch the bench bakes, mid-face on −y.
    fn key() -> PatchKey {
        PatchKey::new(Face::NegY, 19, 271_828, 314_159).expect("a level-19 cell")
    }

    fn directions(key: PatchKey) -> Vec<[f64; 3]> {
        (0..=64_u8)
            .flat_map(|y| (0..=64_u8).map(move |x| key.vertex_dir(x, y)))
            .collect()
    }

    pub fn test_planet(c: &mut Criterion) {
        let key = key();
        let dirs = directions(key);
        let mut group = c.benchmark_group("test_planet");
        group.bench_function("cached", |b| {
            b.iter(|| {
                let mut cache = LatticeCache::new();
                TEST_PLANET.cover_patch(&mut cache, key);
                let mut sum = 0.0;
                for d in &dirs {
                    sum += TEST_PLANET.height(*d, 19, &mut cache).height_m;
                }
                black_box(sum)
            });
        });
        group.bench_function("uncached", |b| {
            b.iter(|| {
                let mut cache = LatticeCache::new();
                let mut sum = 0.0;
                for d in &dirs {
                    sum += TEST_PLANET.height(*d, 19, &mut cache).height_m;
                }
                black_box(sum)
            });
        });
        group.finish();
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_group!(benches, bench::test_planet);
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
criterion::criterion_main!(benches);

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn main() {}
