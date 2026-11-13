//! The bake on real terrain's golden files, `tests/golden/terrain_bake/<world>.golden` (plan R10,
//! T6.a): [`bake_terrain`] of pinned patches over a client's field, blessed natively and compared
//! on `wasm32-wasip1` and `wasm32-unknown-unknown` through the testkit's embedded arm, so that the
//! client's workers bake what the native build bakes.
//!
//! The heights, gradients and weights under them are pinned by R09's and R10.T2's goldens; these
//! pin what the bake makes of them: R05's arrays over the synthesis (its octahedral normals, the
//! offsets, origin, range, radius and skirt), the survey mask at cells' edges (`PatchKey::containing`
//! of each vertex), the reference surface where a view cannot answer, the palette and its bytes,
//! the bound and σ. They live in the library's tests, as R10.T3's and T2's do, because the
//! synthetic worlds are the `testing` feature's, and carry `GENERATOR_VERSION` in a subdirectory
//! of their own. The bake is presentation, so a change to it alone re-blesses them at the version
//! in force with no bump, as the weights' goldens are (R10.T2's record); a change to R09's heights
//! or closed forms re-blesses them with R09's.
//!
//! # Format
//!
//! Each patch opens with `patch <case> <face> <level> <i> <j>`, then each array's FNV-1a digest of
//! its IEEE 754 bits (`heights`, `offsets`, empty where none, `normals`) or of its bytes
//! (`weights`, `mask`), each float scalar's bits (the origin, its height, the height range, the
//! bounding radius, the skirt depth, the bound and σ), the palette's slots as `<class code>:<entry>`
//! (`-` for no entry, `none` for an empty slot) and whether the bake is complete.

use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f32_digest, f64_digest};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::tests::{DOUBLE, MESH, cells_under, client, code, patch_in, straddling};
use super::*;
use crate::field::FieldView;
use crate::synth::test_support::{SEED, rim_cells, world};
use crate::testing::SyntheticWorld;

/// A digest of `bytes` in the golden's form: each as an `f64`, which holds it exactly.
fn byte_digest(bytes: impl IntoIterator<Item = u8>) -> u64 {
    f64_digest(&bytes.into_iter().map(f64::from).collect::<Vec<_>>())
}

/// Writes `bake`'s lines, as the case `case`, to `out`.
fn write_bake(out: &mut GoldenWriter, case: &str, bake: &TerrainBake) {
    let (p, l) = (bake.patch(), bake.layers());
    let k = p.key;
    out.line(&format!(
        "patch {case} {} {} {} {}",
        k.face().index(),
        k.level(),
        k.i(),
        k.j()
    ));
    out.u64_hex("heights", f32_digest(&p.heights));
    out.u64_hex(
        "offsets",
        f32_digest(p.offsets.as_deref().unwrap_or_default()),
    );
    out.u64_hex("normals", f32_digest(&p.normals));
    for (axis, x) in ["x", "y", "z"].into_iter().zip(p.origin) {
        out.f64(&format!("origin_{axis}"), x);
    }
    out.f64("origin_height", p.origin_height_m);
    out.f64("lowest", f64::from(p.height_range_m.0));
    out.f64("highest", f64::from(p.height_range_m.1));
    out.f64("bounding_radius", p.bounding_radius_m);
    out.f64("skirt_depth", p.skirt_depth_m);
    let palette: Vec<String> = l
        .palette()
        .iter()
        .map(|slot| {
            slot.map_or_else(
                || "none".to_owned(),
                |m| {
                    let entry = m.entry().map_or_else(|| "-".to_owned(), |e| e.to_string());
                    format!("{}:{entry}", u8::from(m.class()))
                },
            )
        })
        .collect();
    out.line(&format!("palette {}", palette.join(" ")));
    out.u64_hex(
        "weights",
        byte_digest(l.weights().iter().flatten().copied()),
    );
    out.u64_hex("mask", byte_digest(l.survey_mask().iter().copied()));
    out.f64("bound", l.bound().value());
    out.f64("sigma", l.sigma().value());
    out.line(&format!("complete {}", l.is_complete()));
}

/// The Moon-like world's golden: the level-2 patch over the 2 × 2 survey's edge, incomplete, at
/// the low setting; a level-8 patch inside the survey at the high setting; and a level-8 patch on
/// a coarse crater's rim, surveyed whole, at the low setting.
fn moon_text() -> String {
    let (w, edge, surveyed) = straddling();
    let field = world(w);
    let coarse = field.header().level().get();
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let inside = patch_in(*surveyed.first().expect("a surveyed cell"), coarse + 2);
    for (case, key, opts) in [("edge", edge, MESH), ("inside", inside, DOUBLE)] {
        let bake = bake_terrain(&synth, &mut SynthCache::new(), key, &opts, &[]).expect("held");
        write_bake(&mut out, case, &bake);
    }
    let rim = patch_in(rim_cells(field, 1)[0], coarse + 2);
    let view = client(w, &cells_under(field, rim), |_| code(20.0));
    let synth = Synthesiser::new(&view, SEED);
    let bake = bake_terrain(&synth, &mut SynthCache::new(), rim, &MESH, &[]).expect("held");
    write_bake(&mut out, "rim", &bake);
    out.finish()
}

/// The Earth-like world's golden: a patch at the coarse level plus three on a coarse crater's rim
/// and one at its finest level, each surveyed whole, at the low setting.
fn earth_text() -> String {
    let w = SyntheticWorld::EarthLike;
    let field = world(w);
    let coarse = field.header().level().get();
    let finest = crate::geometry::finest_level(field.header().figure().equatorial_radius_m);
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    let rim = rim_cells(field, 1)[0];
    for (case, level) in [("rim", coarse + 3), ("finest", finest)] {
        let key = patch_in(rim, level);
        let view = client(w, &cells_under(field, key), |_| code(100.0));
        let synth = Synthesiser::new(&view, SEED);
        let bake = bake_terrain(&synth, &mut SynthCache::new(), key, &MESH, &[]).expect("held");
        write_bake(&mut out, case, &bake);
    }
    out.finish()
}

#[test]
fn bake_matches_its_goldens() {
    golden!("terrain_bake/moon_like", &moon_text());
    golden!("terrain_bake/earth_like", &earth_text());
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    /// Every golden file of `tests/golden/terrain_bake/` carries `GENERATOR_VERSION`, and there
    /// are two.
    #[test]
    fn every_terrain_bake_golden_carries_the_generator_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("terrain_bake");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the bake's golden directory is readable") {
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
        assert_eq!(seen, 2);
    }
}
