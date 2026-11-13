//! The authoritative rocks' golden files, `tests/golden/scatter/<name>.golden` (plan R11, T3.b and
//! T3.d), under the rules of 27 ([`with_rocks_placed`]): blessed natively and compared on
//! `wasm32-wasip1` and `wasm32-unknown-unknown` through the testkit's embedded arm, each golden named
//! by a literal.
//!
//! They live in the library's tests, not in `tests/`, because the synthetic worlds are the
//! `testing` feature's, which `cfg(test)` turns on here and which `ci`'s wasip1 selection does not
//! enable for the crate's integration tests, as the ground's goldens do (`ground/golden.rs`). They
//! carry `GENERATOR_VERSION`; 27's bump re-blesses their headers alone.
//!
//! # Format
//!
//! `rocks_<world>`: for each of twelve pinned site cells, at the largest octave's level, one inside
//! each face and one at each face's first corner, a cube corner, `site <face> <level> <i> <j> k
//! <k>`, then each rock of every octave's cells under it, octave by octave, each octave's cells in
//! key order and each cell's rocks in draw order: `rock <octave> <face> <level> <i> <j> <index> dir
//! <d> ground <h> diameter <D> ratio <h ÷ D> aspect <b ÷ a> yaw <ψ> exponent <n>`. Every float is
//! `0x` and its 16 hexadecimal digits of IEEE 754 bits.

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::hold::with_rocks_placed;
use super::place::{
    ROCK_OCTAVE_COUNT, RockCellKey, rock_octave_levels, rocks_in_cell, site_abundance,
};
use crate::cube::{Face, PatchKey};
use crate::field::FieldView;
use crate::synth::test_support::{SEED, world};
use crate::synth::{SynthCache, Synthesiser};
use crate::testing::SyntheticWorld;

fn hex(value: f64) -> String {
    format!("0x{:016x}", bits(value))
}

fn hex3(v: [f64; 3]) -> String {
    v.map(hex).join(" ")
}

/// The twelve pinned site cells at `level`: on each face, one inside it and the one at its first
/// corner, which is a cube corner.
pub(crate) fn pinned_sites(level: u8) -> Vec<PatchKey> {
    let side = 1_u32 << level;
    let mut out = Vec::new();
    for face in Face::ALL {
        let inside = (side / 8) * 3 + u32::from(face.index());
        out.push(PatchKey::new(face, level, inside, side / 2 + 5).expect("a site cell"));
        out.push(PatchKey::new(face, level, 0, 0).expect("a corner cell"));
    }
    out
}

/// The cells of every octave under the site cell `site`, octave by octave, each octave's in key
/// order.
pub(crate) fn cells_under(
    site: PatchKey,
    levels: [u8; ROCK_OCTAVE_COUNT],
) -> Vec<(usize, PatchKey)> {
    let mut out = Vec::new();
    for (octave, &level) in levels.iter().enumerate() {
        let down = level - site.level();
        let span = 1_u32 << down;
        for di in 0..span {
            for dj in 0..span {
                let cell = PatchKey::new(
                    site.face(),
                    level,
                    (site.i() << down) + di,
                    (site.j() << down) + dj,
                )
                .expect("a descendant is a patch key");
                out.push((octave, cell));
            }
        }
    }
    out.sort_by_key(|&(octave, cell)| (octave, cell));
    out
}

/// The golden text of the rocks of `w`'s pinned cells.
fn rocks_text(w: SyntheticWorld) -> String {
    with_rocks_placed(|| {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let levels = rock_octave_levels(&field.header().figure());
        let mut out = GoldenWriter::new();
        out.header(crate::generator_version());
        for site in pinned_sites(levels[ROCK_OCTAVE_COUNT - 1]) {
            let k = site_abundance(&synth, &mut cache, site).expect("a whole field");
            out.line(&format!(
                "site {} {} {} {} k {}",
                site.face().index(),
                site.level(),
                site.i(),
                site.j(),
                hex(k.k())
            ));
            for (octave, cell) in cells_under(site, levels) {
                let mut rocks = Vec::new();
                rocks_in_cell(&synth, &mut cache, RockCellKey::new(cell), &mut rocks)
                    .expect("a whole field holds every cell");
                for (index, rock) in rocks.iter().enumerate() {
                    let shape = rock.shape();
                    out.line(&format!(
                        "rock {octave} {} {} {} {} {index} dir {} ground {} diameter {} ratio {} \
                         aspect {} yaw {} exponent {}",
                        cell.face().index(),
                        cell.level(),
                        cell.i(),
                        cell.j(),
                        hex3(rock.dir()),
                        hex(rock.ground_height().value()),
                        hex(rock.diameter().value()),
                        hex(shape.height_ratio()),
                        hex(shape.aspect()),
                        hex(shape.yaw()),
                        hex(shape.exponent()),
                    ));
                }
            }
        }
        out.finish()
    })
}

#[test]
fn the_rocks_match_their_goldens() {
    golden!(
        "scatter/rocks_earth_like",
        &rocks_text(SyntheticWorld::EarthLike)
    );
    golden!(
        "scatter/rocks_mars_like",
        &rocks_text(SyntheticWorld::MarsLike)
    );
    golden!(
        "scatter/rocks_moon_like",
        &rocks_text(SyntheticWorld::MoonLike)
    );
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    /// Every golden file of `tests/golden/scatter/` carries `GENERATOR_VERSION`.
    #[test]
    fn every_scatter_golden_carries_the_generator_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("scatter");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the scatter golden directory is readable") {
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
        assert!(seen >= 3, "{seen} scatter goldens");
    }
}
