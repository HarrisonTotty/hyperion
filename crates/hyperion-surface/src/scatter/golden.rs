//! The authoritative rocks' golden files, `tests/golden/scatter/<name>.golden` (plan R11, T3.b and
//! T3.d), placed from generator version 33: blessed natively and compared on `wasm32-wasip1` and
//! `wasm32-unknown-unknown` through the testkit's embedded arm, each golden named by a literal.
//!
//! They live in the library's tests, not in `tests/`, because the synthetic worlds are the
//! `testing` feature's, which `cfg(test)` turns on here and which `ci`'s wasip1 selection does not
//! enable for the crate's integration tests, as the ground's goldens do (`ground/golden.rs`). They
//! carry `GENERATOR_VERSION`; 33's bump re-blessed their headers alone.
//!
//! # Format
//!
//! `rocks_<world>`: for each of twelve pinned site cells, at the largest octave's level, one inside
//! each face and one at each face's first corner, a cube corner, `site <face> <level> <i> <j> k
//! <k>`, then each rock of every octave's cells under it, octave by octave, each octave's cells in
//! key order and each cell's rocks in draw order: `rock <octave> <face> <level> <i> <j> <index> dir
//! <d> ground <h> diameter <D> ratio <h ÷ D> aspect <b ÷ a> yaw <ψ> exponent <n>`. Every float is
//! `0x` and its 16 hexadecimal digits of IEEE 754 bits.
//!
//! `contact_<world>`: `height <p> ground <g> with <h>`, R10's ground and
//! [`surface_height_with_rocks`] at the body-fixed point p: at each rock of the pinned site cells'
//! axis and at four points about it, two inside its outline and two past it, then at sixteen
//! points drawn over the sphere, a cube corner and a pole, from 2 m below the ground to 3 m above
//! it.
//!
//! `octaves_moon_like`: the rocks of every octave at a stated k, [`MAX_ROCK_ABUNDANCE`], through
//! [`rocks_in_cell_at`], so that the larger octaves, which the worlds' sites hold too few of, are
//! pinned too: for each octave, its cells of a row-major block on face 3 in turn, until four rocks
//! or 20,000 cells, each rock's `rock` line as above and `top <p> <h>` of its top at its axis and
//! at two points inside its outline, and `top <p> none` at one past it.

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::contact::{RockCache, surface_height_with_rocks};
use super::law::{MAX_ROCK_ABUNDANCE, RockAbundance};
use super::place::{
    ROCK_OCTAVE_COUNT, RockCellKey, rock_octave_levels, rocks_in_cell, rocks_in_cell_at,
    site_abundance,
};
use super::shape::Rock;
use crate::cube::{Face, PatchKey, unit_dir};
use crate::field::FieldView;
use crate::ground::ground_at;
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
                out.line(&rock_line(octave, cell, index, rock));
            }
        }
    }
    out.finish()
}

/// The points the contact golden of `w` pins (see the [module documentation](self)).
fn contact_points(w: SyntheticWorld) -> Vec<[f64; 3]> {
    let field = world(w);
    let figure = field.header().figure();
    let synth = Synthesiser::new(field, SEED);
    let mut cache = SynthCache::new();
    let levels = rock_octave_levels(&figure);
    let mut points = Vec::new();
    for site in pinned_sites(levels[ROCK_OCTAVE_COUNT - 1]) {
        for (_, cell) in cells_under(site, levels) {
            let mut rocks = Vec::new();
            rocks_in_cell(&synth, &mut cache, RockCellKey::new(cell), &mut rocks)
                .expect("a whole field holds every cell");
            for rock in rocks {
                let o = rock.base_centre();
                let up = figure.normal(rock.dir());
                // Two tangents, from the axis's components alone.
                let t1 = unit_dir([-up[1], up[0], 0.0]);
                let t2 = [
                    up[1] * t1[2] - up[2] * t1[1],
                    up[2] * t1[0] - up[0] * t1[2],
                    up[0] * t1[1] - up[1] * t1[0],
                ];
                let r = rock.reach().value();
                points.push(o);
                for (f1, f2) in [(0.2, 0.1), (-0.1, 0.25), (1.2, 0.0), (0.0, -1.3)] {
                    points.push([0, 1, 2].map(|k| o[k] + f1 * r * t1[k] + f2 * r * t2[k]));
                }
            }
        }
    }
    let mut rng = Lcg::new(0x726f_636b_7363_6f6e);
    let mut dirs: Vec<[f64; 3]> = (0..16)
        .map(|_| unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0)))
        .collect();
    dirs.push(unit_dir([1.0, -1.0, 1.0]));
    dirs.push([0.0, 0.0, 1.0]);
    for dir in dirs {
        let ground = crate::patch::collision::finest_surface_height(&synth, dir, &mut cache)
            .expect("a whole field");
        points.push(figure.surface_point(dir, ground + rng.next_f64() * 5.0 - 2.0));
    }
    points
}

/// The golden text of the ground with its rocks at `w`'s pinned points.
fn contact_text(w: SyntheticWorld) -> String {
    let points = contact_points(w);
    let synth = Synthesiser::new(world(w), SEED);
    let (mut synth_cache, mut cache) = (SynthCache::new(), RockCache::new());
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    for p in points {
        let ground = ground_at(&synth, &mut synth_cache, &p).expect("a whole field");
        let with = surface_height_with_rocks(&synth, &mut synth_cache, &mut cache, &p)
            .expect("a whole field holds every cell");
        out.line(&format!(
            "height {} ground {} with {}",
            hex3(p),
            hex(ground.height().value()),
            hex(with.value())
        ));
    }
    out.finish()
}

/// The `rock` line of `rock`, the `index`th of `cell` of octave `octave`.
fn rock_line(octave: usize, cell: PatchKey, index: usize, rock: &Rock) -> String {
    let shape = rock.shape();
    format!(
        "rock {octave} {} {} {} {} {index} dir {} ground {} diameter {} ratio {} aspect {} yaw {} \
         exponent {}",
        cell.face().index(),
        cell.level(),
        cell.i(),
        cell.j(),
        hex3(rock.dir()),
        hex(rock.ground_height().value()),
        hex(rock.diameter().value()),
        hex(shape.height_ratio()),
        hex(shape.aspect()),
        hex(shape.yaw().value()),
        hex(shape.exponent()),
    )
}

/// The golden text of every octave's rocks at the largest k on the Moon-like world.
fn octaves_text() -> String {
    let field = world(SyntheticWorld::MoonLike);
    let figure = field.header().figure();
    let synth = Synthesiser::new(field, SEED);
    let mut cache = SynthCache::new();
    let levels = rock_octave_levels(&figure);
    let k = RockAbundance::new(MAX_ROCK_ABUNDANCE);
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    for (octave, &level) in levels.iter().enumerate() {
        let side = 1_u32 << level;
        let (i0, j0) = (side / 3, side / 5);
        let mut found = 0;
        for n in 0..20_000_u32 {
            let cell = PatchKey::new(Face::PosY, level, i0 + n % 128, j0 + n / 128)
                .expect("a cell of the block");
            let mut rocks = Vec::new();
            rocks_in_cell_at(&synth, &mut cache, RockCellKey::new(cell), k, &mut rocks)
                .expect("a whole field holds every cell");
            for (index, rock) in rocks.iter().enumerate() {
                out.line(&rock_line(octave, cell, index, rock));
                let up = figure.normal(rock.dir());
                let t1 = unit_dir([-up[1], up[0], 0.0]);
                let o = rock.base_centre();
                let r = rock.reach().value();
                for f in [0.0, 0.15, -0.3, 1.2] {
                    let p = [0, 1, 2].map(|i| o[i] + f * r * t1[i]);
                    let top = rock.top_above(&figure, &p);
                    out.line(&format!(
                        "top {} {}",
                        hex3(p),
                        top.map_or_else(|| "none".to_owned(), |h| hex(h.value()))
                    ));
                }
            }
            found += rocks.len();
            if found >= 4 {
                break;
            }
        }
        assert!(found >= 4, "octave {octave} drew {found} rocks");
    }
    out.finish()
}

#[test]
fn every_octaves_rocks_match_their_golden() {
    golden!("scatter/octaves_moon_like", &octaves_text());
}

#[test]
fn the_ground_with_its_rocks_matches_its_goldens() {
    golden!(
        "scatter/contact_earth_like",
        &contact_text(SyntheticWorld::EarthLike)
    );
    golden!(
        "scatter/contact_moon_like",
        &contact_text(SyntheticWorld::MoonLike)
    );
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
        assert!(seen >= 6, "{seen} scatter goldens");
    }
}
