//! The level table's golden file, `tests/golden/level_table.golden` (plan R05, T6): what the wasm
//! module's `levelTable` hands the client's selection, for the test planet without and with
//! ridges, on native, `wasm32-wasip1` and `wasm32-unknown-unknown`. The client's test pins its
//! parsed table against this file (T7.a).
//!
//! The header carries [`TEST_PLANET_VERSION`] (Design note 13); where this test fails with the
//! testkit's hint to "bump `GENERATOR_VERSION`", read "bump `TEST_PLANET_VERSION`".
//!
//! # Format
//!
//! `level <ridges> <n> <bound> <low> <high> <spacing>` for ridges `off` then `on` and n from 0 to
//! 24: the level bound `ε_n`, the lowest and highest height, and the largest vertex spacing, metres,
//! each as `0x` and its 16 hexadecimal digits of IEEE 754 bits, in the table's order.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_surface::TEST_PLANET_VERSION;
use hyperion_surface::test_planet::{Ridges, TEST_PLANET};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

#[test]
fn the_level_table_matches_its_golden() {
    let mut w = GoldenWriter::new();
    w.header(TEST_PLANET_VERSION);
    for (ridges, name) in [(Ridges::Off, "off"), (Ridges::On, "on")] {
        let table = TEST_PLANET.with_ridges(ridges).level_table();
        assert_eq!(table.len(), 100);
        for (n, row) in (0_u32..).zip(table.chunks(4)) {
            let hex: Vec<String> = row.iter().map(|v| format!("0x{:016x}", bits(*v))).collect();
            w.line(&format!("level {name} {n} {}", hex.join(" ")));
        }
    }
    golden!("level_table", &w.finish());
}
