//! The test planet's golden file, `tests/golden/test_planet.golden` (plan R05, T3.c).
//!
//! It pins `height` and its gradient at 500 fixed directions and levels, with ridges off and on,
//! as hexadecimal bits, on native, `wasm32-wasip1` and `wasm32-unknown-unknown`, so that the
//! server's and the client's test planet agree bit for bit.
//!
//! The header carries [`TEST_PLANET_VERSION`], not the generator version: the test planet belongs
//! to no universe (Design note 13). Where this test fails with the testkit's hint to "bump
//! `GENERATOR_VERSION`", read "bump `TEST_PLANET_VERSION`" in `src/lib.rs`, then `just bless`.
//!
//! # Format
//!
//! After the header, `height <n> <ridges> <level> <dx> <dy> <dz> <h> <gx> <gy> <gz>` for n from 0
//! to 499 and ridges `off` then `on`: the unit direction, the height in metres and its gradient,
//! every float as `0x` and its 16 hexadecimal digits of IEEE 754 bits; the level is decimal.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_surface::TEST_PLANET_VERSION;
use hyperion_surface::cube::{MAX_LEVEL, unit_dir};
use hyperion_surface::noise::LatticeCache;
use hyperion_surface::test_planet::{Ridges, TEST_PLANET};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;

fn hex(value: f64) -> String {
    format!("0x{:016x}", bits(value))
}

/// 500 directions, uniform on the sphere by rejection from a fixed test generator, each with a
/// level from 0 to 24 in turn.
fn samples() -> Vec<([f64; 3], u8)> {
    let mut rng = Lcg::new(0x7465_7374_706c_616e);
    let mut out = Vec::with_capacity(500);
    let mut level = 0_u8;
    while out.len() < 500 {
        let p = [0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0);
        let r2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
        if r2 > 1e-6 && r2 <= 1.0 {
            out.push((unit_dir(p), level));
            level = (level + 1) % (MAX_LEVEL + 1);
        }
    }
    out
}

#[test]
fn the_test_planet_matches_its_golden() {
    let mut w = GoldenWriter::new();
    w.header(TEST_PLANET_VERSION);
    let mut cache = LatticeCache::new();
    for (ridges, name) in [(Ridges::Off, "off"), (Ridges::On, "on")] {
        let planet = TEST_PLANET.with_ridges(ridges);
        for (n, (dir, level)) in (0_u32..).zip(samples()) {
            let s = planet.height(dir, level, &mut cache);
            w.line(&format!(
                "height {n} {name} {level} {} {} {} {} {} {} {}",
                hex(dir[0]),
                hex(dir[1]),
                hex(dir[2]),
                hex(s.height_m),
                hex(s.gradient[0]),
                hex(s.gradient[1]),
                hex(s.gradient[2]),
            ));
        }
    }
    golden!("test_planet", &w.finish());
}
