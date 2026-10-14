//! The class weights' goldens, `tests/golden/weights/` (plan R10, T2): [`weights_at`] at 250 points
//! of each synthetic world at six band levels (`<world>.golden`), at 40 points of a cold-trapped
//! pole's partial ice (`cold_trap.golden`, whose cut no synthetic world's land reads), and the
//! kernels' floats bit for bit (`kernels.golden`), blessed natively and compared on `wasm32-wasip1`
//! and `wasm32-unknown-unknown` through the testkit's embedded arm.
//!
//! A change to the weights alone moves only these, and is a client change (the plan's Generator
//! version section): it re-blesses them at the current version with no bump, the commit saying so
//! (the plan's Risks, "Deviations in T2, as built"). R09's closed forms and the classifier move
//! them too, so R09.T7.h re-blesses them.
//!
//! The client's WebAssembly workers bake the weights a patch draws, so a split between the targets
//! is ground drawn in one blend here and another there. The weights are presentation, not generated
//! output, but they read the height, the classifier and R09's closed forms, every one of which is
//! pinned, so the goldens carry `GENERATOR_VERSION` (main's ruling of 2026-10-13: header 21, in a
//! subdirectory of their own, as the material goldens are, which `cube_golden.rs`'s check of the
//! test planet's version does not read). They live in the library's tests for the reason the
//! material goldens do: the synthetic worlds are the `testing` feature's.
//!
//! # Format
//!
//! `<face> <u> <v> <level> <slots>`: the point's face and its (u, v), each float `0x` and its 16
//! hexadecimal digits of IEEE 754 bits; the band level; and each held slot as
//! `<code>:<entry>=<byte>`, the class's code, its palette entry or `-`, and its weight. The points
//! are up to 30 each in cells on mountain belts on dry land, in cells of partial ice, in the
//! shallowest cells under water and under the coarse craters' rims, one point a cell, where a world
//! has them, then random points over the sphere to 250; the levels are the finest, n − 1, n − 3 and
//! n − 6 from it, n − 10, and the class map's, level 4.

use std::fmt::Write as _;

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_base::rng::DetailSeed;

use super::tests::{cold_trap_cells, cold_trapped};
use super::*;
use crate::cube::xyz_to_face_uv;
use crate::field::{CoarseField, cell_at_index};
use crate::synth::relief::ReliefStyle;
use crate::synth::test_support::{SEED, random_points_in, rim_cells, world};
use crate::testing::SyntheticWorld;

/// The points each world's golden pins.
const POINTS: usize = 250;

/// The points of each kind of cell a golden pins, at most.
const EACH: usize = 30;

/// The class map's band level (Design note 8).
const CLASS_MAP_LEVEL: u8 = 4;

/// The points the golden of `field` pins (the module documentation).
fn pinned_points(field: &CoarseField) -> Vec<FaceUv> {
    let mut rng = Lcg::new(0x7765_6967_6874_7367);
    let level = field.header().level();
    let cells: Vec<PatchKey> = (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).expect("a cell of the level"))
        .collect();
    let record = |k: PatchKey| field.cell(k).expect("a whole field holds every cell");
    let spread = |keys: Vec<PatchKey>| {
        let stride = (keys.len() / EACH).max(1);
        keys.into_iter()
            .step_by(stride)
            .take(EACH)
            .collect::<Vec<_>>()
    };
    let belts = cells
        .iter()
        .copied()
        .filter(|&k| {
            let c = record(k);
            ReliefStyle::of(c).belt() >= 0.5 && !c.is_under_water() && c.ice == 0
        })
        .collect();
    let ice = cells
        .iter()
        .copied()
        .filter(|&k| (1..u8::MAX).contains(&record(k).ice))
        .collect();
    let mut sea: Vec<(i32, PatchKey)> = cells
        .iter()
        .filter_map(|&k| {
            let c = record(k);
            c.is_under_water()
                .then_some((c.water_surface_mm - c.elevation_mm, k))
        })
        .collect();
    sea.sort_unstable();
    let shallow = sea.into_iter().take(EACH).map(|(_, k)| k).collect();
    let mut points: Vec<FaceUv> = [belts, ice, shallow, rim_cells(field, EACH)]
        .into_iter()
        .flat_map(spread)
        .map(|cell| xyz_to_face_uv(random_points_in(cell, 1, &mut rng)[0]))
        .collect();
    while points.len() < POINTS {
        let dir = unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0));
        points.push(xyz_to_face_uv(dir));
    }
    points
}

/// The golden text of `w`.
fn weights_text(w: SyntheticWorld) -> String {
    let field = world(w);
    weights_lines(&Synthesiser::new(field, SEED), &pinned_points(field))
}

/// The golden text of the cold-trapped pole's partial ice at 0.2 (`tests::cold_trapped`), whose
/// weights read the ice's cut, which no synthetic world's land does: 40 points in its polar cells,
/// one a cell, spread through them.
fn cold_trap_text() -> String {
    let field = cold_trapped(0.2);
    let cells = cold_trap_cells(&field);
    let mut rng = Lcg::new(0x0063_6f6c_6474_7261);
    let points: Vec<FaceUv> = (0..40)
        .map(|k| cells[(k * cells.len()) / 40])
        .map(|cell| xyz_to_face_uv(random_points_in(cell, 1, &mut rng)[0]))
        .collect();
    weights_lines(&Synthesiser::new(&field, DetailSeed::new(17)), &points)
}

/// The golden lines of `points` on `synth`'s field: each at the finest level, n − 1, n − 3, n − 6
/// and n − 10 from it, and the class map's level (the module documentation's format).
fn weights_lines(synth: &Synthesiser<'_, CoarseField>, points: &[FaceUv]) -> String {
    let mut cache = SynthCache::new();
    let finest = finest_band(synth.field().header()).get();
    let levels = [
        finest,
        finest - 1,
        finest - 3,
        finest - 6,
        finest - 10,
        CLASS_MAP_LEVEL,
    ];
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    let mut line = String::new();
    for &at in points {
        for level in levels {
            let band = BandLevel::new(level).expect("a band level");
            let weights =
                weights_at(synth, &mut cache, at, band).expect("a whole field holds every cell");
            line.clear();
            write!(
                line,
                "{} 0x{:016x} 0x{:016x} {level}",
                at.face.index(),
                bits(at.u),
                bits(at.v)
            )
            .expect("a string takes any write");
            for (m, byte) in weights.iter() {
                write!(line, " {}:", u8::from(m.class())).expect("a string takes any write");
                match m.entry() {
                    Some(entry) => write!(line, "{entry}"),
                    None => write!(line, "-"),
                }
                .expect("a string takes any write");
                write!(line, "={byte}").expect("a string takes any write");
            }
            out.line(&line);
        }
    }
    out.finish()
}

#[test]
fn material_weights_match_their_goldens() {
    golden!(
        "weights/earth_like",
        &weights_text(SyntheticWorld::EarthLike)
    );
    golden!("weights/mars_like", &weights_text(SyntheticWorld::MarsLike));
    golden!("weights/moon_like", &weights_text(SyntheticWorld::MoonLike));
    golden!(
        "weights/ceres_like",
        &weights_text(SyntheticWorld::CeresLike)
    );
    golden!("weights/flat", &weights_text(SyntheticWorld::Flat));
    golden!(
        "weights/one_crater",
        &weights_text(SyntheticWorld::OneCrater)
    );
    golden!("weights/cold_trap", &cold_trap_text());
}

/// The kernels' golden text: the floats the weights' probabilities are made of, bit for bit, which
/// the bytes of the other goldens show only where they round across a byte. The Rice tail
/// ([`exceedance`]) over a grid of means, variances and thresholds (with the reach's early return
/// and the panels' bound among them), the normal distribution at a grid of deviates, the height
/// cases on fixed breaks, and the blend at a grid of slopes.
fn kernels_text() -> String {
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line("exceedance");
    for mean in [0.0, 0.05, 0.3, 0.8, 0.839, 0.85, 1.2, 2.0, 5.0] {
        for variance in [1e-8, 9e-10, 1e-6, 1e-4, 1e-2, 0.1, 1.0, 4.0] {
            for threshold in [0.1, 0.839_099_631_177_280, 1.732_050_807_568_877] {
                out.f64(
                    &format!("exceedance({mean}, {variance}, {threshold})"),
                    exceedance(mean, variance, threshold),
                );
            }
        }
    }
    out.line("normal_cdf");
    for k in -40..=40 {
        let z = f64::from(k) / 5.0;
        out.f64(&format!("normal_cdf({z})"), normal_cdf(z));
    }
    out.line("height_cases");
    let breaks = [-120.0, 0.0, 1_500.0];
    for (mean, variance) in [(-500.0, 1e4), (10.0, 4e4), (1_499.0, 1.0), (3_000.0, 1e6)] {
        for (k, (h, p)) in height_cases(&breaks, mean, variance)
            .into_iter()
            .enumerate()
        {
            out.f64(&format!("height_cases({mean}, {variance})[{k}].h"), h);
            out.f64(&format!("height_cases({mean}, {variance})[{k}].p"), p);
        }
    }
    out.line("blend");
    for degrees in (30..=70).step_by(2) {
        for (steepness, share) in blend(f64::from(degrees).to_radians()) {
            out.f64(&format!("blend({degrees}°).{steepness:?}"), share);
        }
    }
    out.finish()
}

#[test]
fn material_weights_kernels_match_their_golden() {
    golden!("weights/kernels", &kernels_text());
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    /// The weights' goldens: one for each synthetic world, the cold-trapped pole's and the
    /// kernels'.
    const WEIGHTS_GOLDENS: usize = 8;

    /// Every golden file of `tests/golden/weights/` carries `GENERATOR_VERSION`, not R05's
    /// `TEST_PLANET_VERSION`, which heads the files directly under `tests/golden/`.
    #[test]
    fn material_weights_every_golden_carries_the_generator_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("weights");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the weights golden directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            if path.extension().is_some_and(|e| e == "golden") {
                let text = std::fs::read_to_string(&path).expect("a golden file is readable");
                assert_eq!(
                    text.lines().next(),
                    Some(expected.as_str()),
                    "{}",
                    path.display()
                );
                seen += 1;
            }
        }
        assert_eq!(seen, WEIGHTS_GOLDENS, "the goldens in {}", dir.display());
    }
}
