//! The collision interpolant's golden file, `tests/golden/collision.golden` (plan R05, T4.c): the
//! drawn finest mesh's height at fixed directions, ridges off and on, which the wasm module's
//! `surfaceHeightM` reproduces bit for bit (`heightWasm.test.ts`), on native, `wasm32-wasip1` and
//! `wasm32-unknown-unknown`.
//!
//! The header carries [`TEST_PLANET_VERSION`] (Design note 13); where this test fails with the
//! testkit's hint to "bump `GENERATOR_VERSION`", read "bump `TEST_PLANET_VERSION`".
//!
//! # Format
//!
//! `height <ridges> <x> <y> <z> <h>`: the direction as given (not normalised) and the height,
//! metres above the spheroid, every float as `0x` and its 16 hexadecimal digits of IEEE 754 bits.
//! The directions are 24 drawn uniformly from a fixed test generator, a finest-level vertex on a
//! face edge and one inside a face (the snap's case), and a cube corner.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_surface::TEST_PLANET_VERSION;
use hyperion_surface::cube::{Face, PatchKey};
use hyperion_surface::noise::LatticeCache;
use hyperion_surface::patch::collision::finest_surface_height;
use hyperion_surface::test_planet::{Ridges, TEST_PLANET};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

fn hex(value: f64) -> String {
    format!("0x{:016x}", bits(value))
}

/// The directions the golden pins.
fn directions() -> Vec<[f64; 3]> {
    let mut rng = Lcg::new(0x636f_6c6c_6964_6500);
    let mut out: Vec<[f64; 3]> = (0..24)
        .map(|_| [0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0))
        .collect();
    let edge = PatchKey::new(Face::PosX, 19, (1 << 19) - 1, 12_345).expect("a level-19 cell");
    out.push(edge.vertex_dir(64, 17));
    let inner = PatchKey::new(Face::NegY, 19, 271_828, 314_159).expect("a level-19 cell");
    out.push(inner.vertex_dir(13, 51));
    out.push([1.0, 1.0, 1.0]);
    out
}

#[test]
fn the_collision_interpolant_matches_its_golden() {
    let mut w = GoldenWriter::new();
    w.header(TEST_PLANET_VERSION);
    for (ridges, name) in [(Ridges::Off, "off"), (Ridges::On, "on")] {
        let planet = TEST_PLANET.with_ridges(ridges);
        for dir in directions() {
            let mut cache = LatticeCache::new();
            let h = match finest_surface_height(&planet, dir, &mut cache) {
                Ok(h) => h,
                Err(never) => match never {},
            };
            w.line(&format!(
                "height {name} {} {} {} {}",
                hex(dir[0]),
                hex(dir[1]),
                hex(dir[2]),
                hex(h)
            ));
        }
    }
    golden!("collision", &w.finish());
}
