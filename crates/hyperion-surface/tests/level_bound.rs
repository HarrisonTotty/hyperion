//! The level bound's slow test (plan R05, T6): no sampled distance between a level's mesh and the
//! finest level's exceeds `ε_n`, and the figures R10.T4's rule reads are recorded
//! (decisions-r05.md item 6).
//!
//! Per level n below the finest, 16 random level-n patches each take 625 random points inside
//! them, 10⁴ samples a level, and the distance |`S_n(p`) − `S_f(p)|` is measured with the finest mesh
//! through `finest_surface_height` and level n's through `mesh_height` on the same diagonal rule.
//! Printed per level: the bound, the largest sample, the 99.9th percentile and its ratio to the
//! bound (a ratio below a quarter is a finding, not a failure), `σ_n` (the RMS of the omitted
//! octaves, √`Σσ_k²`), the 99.9th percentile ÷ `4σ_n`, the largest per-patch maximum ÷ `4σ_n` and the
//! share of patches whose maximum is within 1.25 × `4σ_n`, and the implied patch-to-distance ratio
//! `k_n` = `ε_n` ÷ (`S_n` τ `θ_px`) at τ = 1 px, 1080p and a 60° field of view, with `S_n` the level's patch
//! size (64 times its mean vertex spacing).

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_surface::cube::{Face, FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use hyperion_surface::geometry::{finest_level, vertex_spacing};
use hyperion_surface::noise::LatticeCache;
use hyperion_surface::patch::collision::{finest_surface_height, mesh_height};
use hyperion_surface::test_planet::{Ridges, TEST_PLANET};
use hyperion_testkit::lcg::Lcg;

/// A random direction inside `key`.
fn point_in(key: PatchKey, rng: &mut Lcg) -> [f64; 3] {
    let cells = f64::from(1_u32 << key.level());
    let s = (f64::from(key.i()) + rng.next_f64()) / cells;
    let t = (f64::from(key.j()) + rng.next_f64()) / cells;
    unit_dir(face_uv_to_xyz(FaceUv {
        face: key.face(),
        u: st_to_uv(s),
        v: st_to_uv(t),
    }))
}

#[test]
#[ignore = "slow: 10^4 mesh-to-mesh distances at every level, both ridge settings"]
fn level_bound_holds() {
    let theta_px = 2.0 * hyperion_base::math::tan(30.0_f64.to_radians()) / 1920.0;
    for ridges in [Ridges::Off, Ridges::On] {
        let planet = TEST_PLANET.with_ridges(ridges);
        let a = planet.figure().equatorial_radius_m;
        let finest = finest_level(a);
        let all = planet.octaves_at(finest).count();
        let mut rng = Lcg::new(0x0062_6f75_6e64);
        let mut cache = LatticeCache::new();
        println!("ridges {ridges:?}");
        println!(
            "level  bound_m  max_m  p999_m  p999/bound  sigma_m  p999/4s  patchmax/4s  within1.25  k_n"
        );
        for level in 0..finest {
            let bound = planet.level_bound_m(level);
            let own = planet.octaves_at(level).count();
            let mut variance = 0.0;
            for k in own..all {
                variance += planet.sigma_m(k) * planet.sigma_m(k);
            }
            let sigma = variance.sqrt();
            let last = (1_u64 << level) - 1;
            let mut samples = Vec::with_capacity(10_000);
            let mut patch_maxima = Vec::with_capacity(16);
            for _ in 0..16 {
                let face = Face::from_index(u8::try_from(rng.next_below(6)).unwrap()).unwrap();
                let i = u32::try_from(rng.next_below(last + 1)).unwrap();
                let j = u32::try_from(rng.next_below(last + 1)).unwrap();
                let key = PatchKey::new(face, level, i, j).unwrap();
                let mut patch_max = 0.0_f64;
                for _ in 0..625 {
                    let dir = point_in(key, &mut rng);
                    let fine = match finest_surface_height(&planet, dir, &mut cache) {
                        Ok(h) => h,
                        Err(never) => match never {},
                    };
                    let coarse = match mesh_height(&planet, dir, level, &mut cache) {
                        Ok(h) => h,
                        Err(never) => match never {},
                    };
                    let d = (fine - coarse).abs();
                    assert!(
                        d <= bound,
                        "level {level}: |S_n − S_f| = {d} m exceeds the bound {bound} m at {dir:?}"
                    );
                    patch_max = patch_max.max(d);
                    samples.push(d);
                }
                patch_maxima.push(patch_max);
            }
            samples.sort_by(f64::total_cmp);
            let p999 = samples[samples.len() * 999 / 1000];
            let max = samples[samples.len() - 1];
            let four_sigma = 4.0 * sigma;
            let worst_patch = patch_maxima.iter().copied().fold(0.0, f64::max);
            #[expect(clippy::cast_precision_loss, reason = "at most 16 patches")]
            let within = patch_maxima
                .iter()
                .filter(|m| **m <= 1.25 * four_sigma)
                .count() as f64
                / 16.0;
            let patch_size = 64.0 * vertex_spacing(a, level).mean_m;
            let k_n = bound / (patch_size * theta_px);
            // A level with every octave omits none, so σ_n is 0 and its ratios are undefined.
            let ratio = |x: f64| {
                if sigma > 0.0 {
                    format!("{:.3}", x / four_sigma)
                } else {
                    String::from("n/a")
                }
            };
            println!(
                "{level:5}  {bound:.4e}  {max:.4e}  {p999:.4e}  {:.4}  {sigma:.4e}  {}  {}  {within:.3}  {k_n:.3}",
                p999 / bound,
                ratio(p999),
                ratio(worst_patch),
            );
            if p999 / bound < 0.25 {
                println!("  finding: the 99.9th percentile is under a quarter of the bound");
            }
        }
    }
}
