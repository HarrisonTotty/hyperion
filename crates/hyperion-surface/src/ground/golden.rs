//! The ground's golden files, `tests/golden/ground/<world>.golden` (plan R10, T3): [`ground_at`]
//! at pinned points of each synthetic world, blessed natively and compared on `wasm32-wasip1` and
//! `wasm32-unknown-unknown` through the testkit's embedded arm, each golden named by a literal.
//!
//! They live in the library's tests, not in `tests/`, because the synthetic worlds are the
//! `testing` feature's, which `cfg(test)` turns on here and which `ci`'s wasip1 selection does not
//! enable for the crate's integration tests; R09's wire goldens, and R09.T9's golden heights, live
//! in the library for the same reason. They carry `GENERATOR_VERSION`, in a subdirectory that
//! `cube_golden.rs`'s check of the test planet's version does not read. Until R09.T9's golden
//! heights land these are the only goldens over `height_at` on the synthetic worlds; once they do,
//! a change here without one there is a change to the interpolant or to the point's foot, and a
//! `GENERATOR_VERSION` bump re-blesses these with them.
//!
//! # Format
//!
//! `ground <p> height <h> normal <n> triangle <face> <level> <i> <j> <x> <y> <half> dir <d> above
//! <h_p>`: the body-fixed point p, metres; the ground's height; its triangle's unit normal; the
//! triangle's patch, quad and half (`upper` or `lower`); the point's foot d on the datum; and the
//! point's own height above it. Every float is `0x` and its 16 hexadecimal digits of IEEE 754 bits.
//! The points are, at altitudes from −500 m to 5 km, sixteen drawn at random over the sphere, two
//! in each of three finest patches at the cube's seams (a cube corner's and two on face edges), one
//! under each of up to three coarse craters' rims, a finest vertex, a cube corner and a pole: 28 on
//! each cratered world, 26 on the one-crater world and 25 on the flat one.

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::cube::{Face, unit_dir};
use crate::geometry::finest_level;
use crate::synth::test_support::{SEED, random_cells, random_points_in, rim_cells, world};
use crate::testing::SyntheticWorld;

fn hex(value: f64) -> String {
    format!("0x{:016x}", bits(value))
}

fn hex3(v: [f64; 3]) -> String {
    v.map(hex).join(" ")
}

/// The points the golden of `w` pins.
fn pinned_points(w: SyntheticWorld) -> Vec<[f64; 3]> {
    let field = world(w);
    let figure = field.header().figure();
    let finest = finest_level(figure.equatorial_radius_m);
    let mut rng = Lcg::new(0x6772_6f75_6e64_6764);
    let mut dirs: Vec<[f64; 3]> = (0..16)
        .map(|_| unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0)))
        .collect();
    for cell in random_cells(finest, 3, &mut rng) {
        dirs.extend(random_points_in(cell, 2, &mut rng));
    }
    for cell in rim_cells(field, 3) {
        dirs.extend(random_points_in(cell, 1, &mut rng));
    }
    let vertex = PatchKey::new(Face::NegY, finest, 1_234, 4_321).expect("a finest patch");
    dirs.push(vertex.vertex_dir(13, 51));
    dirs.push(unit_dir([1.0, -1.0, 1.0]));
    dirs.push([0.0, 0.0, -1.0]);
    dirs.into_iter()
        .map(|d| figure.surface_point(d, rng.next_f64() * 5_500.0 - 500.0))
        .collect()
}

/// The golden text of `w`.
fn ground_text(w: SyntheticWorld) -> String {
    let synth = Synthesiser::new(world(w), SEED);
    let mut cache = SynthCache::new();
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    for p in pinned_points(w) {
        let g = ground_at(&synth, &mut cache, &p).expect("a whole field holds every cell");
        let t = g.triangle();
        out.line(&format!(
            "ground {} height {} normal {} triangle {} {} {} {} {} {} {} dir {} above {}",
            hex3(p),
            hex(g.height().value()),
            hex3(g.normal()),
            t.key().face().index(),
            t.key().level(),
            t.key().i(),
            t.key().j(),
            t.x(),
            t.y(),
            if t.upper() { "upper" } else { "lower" },
            hex3(g.dir()),
            hex(g.point_height().value()),
        ));
    }
    out.finish()
}

#[test]
fn the_ground_matches_its_goldens() {
    golden!("ground/earth_like", &ground_text(SyntheticWorld::EarthLike));
    golden!("ground/mars_like", &ground_text(SyntheticWorld::MarsLike));
    golden!("ground/moon_like", &ground_text(SyntheticWorld::MoonLike));
    golden!("ground/ceres_like", &ground_text(SyntheticWorld::CeresLike));
    golden!("ground/flat", &ground_text(SyntheticWorld::Flat));
    golden!("ground/one_crater", &ground_text(SyntheticWorld::OneCrater));
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    /// Every golden file of `tests/golden/ground/` carries `GENERATOR_VERSION`, one for each
    /// synthetic world.
    #[test]
    fn every_ground_golden_carries_the_generator_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("ground");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the ground's golden directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            let text = std::fs::read_to_string(&path).expect("a golden file is readable");
            assert_eq!(
                text.lines().next(),
                Some(expected.as_str()),
                "{}",
                path.display()
            );
            seen += 1;
        }
        assert_eq!(seen, crate::testing::SyntheticWorld::ALL.len());
    }
}
