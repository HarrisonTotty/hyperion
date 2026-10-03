//! The patch bake's golden file, `tests/golden/bake.golden` (plan R05, T5): three baked patches
//! of the test planet, each array's `hyperion_testkit::golden::f32_digest` and the scalars' bits,
//! the same on native, `wasm32-wasip1` and `wasm32-unknown-unknown` (the last under Electron's V8,
//! as the height workers run it).
//!
//! The header carries [`TEST_PLANET_VERSION`] (Design note 13); where this test fails with the
//! testkit's hint to "bump `GENERATOR_VERSION`", read "bump `TEST_PLANET_VERSION`".
//!
//! # Format
//!
//! For each patch, `bake <face> <level> <i> <j> <path> <normals> <ridges>`, then
//! `heights <n> <digest>`, `offsets <n> <digest>` (or `offsets none`), `normals <n> <digest>`,
//! each array's length and FNV-1a 64 digest in hexadecimal, and the scalars `origin`,
//! `origin_height_m`, `height_range_m`, `bounding_radius_m` and `skirt_depth_m` as the testkit's
//! `f64` lines (bits, then the decimal); `height_range_m`'s two `f32` ends are printed widened to
//! `f64`, which is exact.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_surface::TEST_PLANET_VERSION;
use hyperion_surface::cube::{Face, PatchKey};
use hyperion_surface::noise::LatticeCache;
use hyperion_surface::patch::{BakeOptions, NormalScale, VertexPath, bake_patch};
use hyperion_surface::test_planet::{Ridges, TEST_PLANET};
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f32_digest};

#[test]
fn three_bakes_match_their_golden() {
    let cases = [
        (
            PatchKey::new(Face::PosZ, 4, 3, 12).expect("a level-4 cell"),
            VertexPath::BakedOffsets,
            NormalScale::Mesh,
            Ridges::Off,
        ),
        (
            PatchKey::new(Face::NegX, 13, 8_191, 0).expect("a level-13 cell"),
            VertexPath::FaceDifferences,
            NormalScale::Double,
            Ridges::On,
        ),
        (
            PatchKey::new(Face::PosY, 19, 300_000, 400_000).expect("a level-19 cell"),
            VertexPath::BakedOffsets,
            NormalScale::Double,
            Ridges::Off,
        ),
    ];
    let mut w = GoldenWriter::new();
    w.header(TEST_PLANET_VERSION);
    for (key, path, normals, ridges) in cases {
        let opts = BakeOptions {
            vertex_path: path,
            normals,
            skirt_m: 0.0,
        };
        let planet = TEST_PLANET.with_ridges(ridges);
        let mut cache = LatticeCache::new();
        let bake = match bake_patch(&planet, key, &opts, &mut cache) {
            Ok(bake) => bake,
            Err(never) => match never {},
        };
        w.line(&format!(
            "bake {} {} {} {} {path:?} {normals:?} {ridges:?}",
            key.face().index(),
            key.level(),
            key.i(),
            key.j()
        ));
        w.line(&format!(
            "heights {} 0x{:016x}",
            bake.heights.len(),
            f32_digest(&bake.heights)
        ));
        match &bake.offsets {
            Some(offsets) => w.line(&format!(
                "offsets {} 0x{:016x}",
                offsets.len(),
                f32_digest(offsets)
            )),
            None => w.line("offsets none"),
        }
        w.line(&format!(
            "normals {} 0x{:016x}",
            bake.normals.len(),
            f32_digest(&bake.normals)
        ));
        for (axis, value) in ["x", "y", "z"].into_iter().zip(bake.origin) {
            w.f64(&format!("origin.{axis}"), value);
        }
        w.f64("origin_height_m", bake.origin_height_m);
        w.f64("height_range_m.low", f64::from(bake.height_range_m.0));
        w.f64("height_range_m.high", f64::from(bake.height_range_m.1));
        w.f64("bounding_radius_m", bake.bounding_radius_m);
        w.f64("skirt_depth_m", bake.skirt_depth_m);
    }
    golden!("bake", &w.finish());
}
