//! The drawn ground's golden files, `tests/golden/drawn_ground/<world>.golden` (plan R10, T15.a,
//! decision-r10-t15a-scope): the fixture of the client's check that collision agrees with what is
//! drawn, `drawnGround.test.ts` beside the client's `gpu/vertexEmulation.ts`.
//!
//! On the Earth-like and Moon-like synthetic worlds, through one client's field holding every cell
//! under them, three finest-level patches (one inside a face under a coarse crater's rim, one on a
//! face edge and one at a cube corner) are each baked once with [`bake_terrain`] on the
//! `BakedOffsets` path. That one bake carries both vertex paths' inputs: the `f32` own heights from
//! which `FaceDifferences` forms its positions, and the `f32` own offsets q₀ that `BakedOffsets`
//! draws. At each sample point [`ground_at`] gives the authoritative ground, the height collision
//! reads, with its triangle; the client forms that triangle's three vertices as the GPU does on each
//! path and holds the plane through them to the ground's height along the same normal.
//!
//! Each patch has 69 sample points: 64 drawn by the testkit's [`Lcg`] strictly inside the patch,
//! then a vertex, a point on a quad's diagonal, one on a quad's edge, one on the patch's edge (its
//! v-min edge, inside the face, whose quads are the patch's) and one in the far corner quad
//! (63, 63), which on the edge patch touches the face edge and on the corner patch holds the cube
//! corner. The diagonal's point comes back from its direction with fa and fb equal, or a few
//! 10⁻⁹ of a quad apart, so its half is the rounding's, where both halves give the same height to
//! nanometres. Each is a body-fixed point a few metres above the ground at its direction, as a
//! craft asks, in `ground/golden.rs`'s form, and its foot d on the datum is the ground's
//! [`dir`](crate::ground::GroundPoint::dir). Before a sample is written the writer checks that the
//! ground's triangle lies in the patch and that (fa, fb) are R05's weights there: the half is the
//! upper exactly where fa < fb, and R05's interpolant at them is the ground's height bit for bit.
//!
//! The edge patch is on face −y's v-max edge, whose vertices are evaluated on face +x, their
//! canonical face, and the corner patch on face −z at the corner of faces +x, +y and −z, evaluated
//! on +x (`PatchKey::vertex_dir`), while the client forms them from their own face's (u, v).
//!
//! They live in the library's tests, as R10.T3's and T6.a's goldens over the synthetic worlds do,
//! since the `testing` feature is on there, and carry `GENERATOR_VERSION`: a change to R09's
//! heights re-blesses them with R09's own goldens.
//!
//! # Format
//!
//! Every `f32` is `0x` and its 8 hexadecimal digits of IEEE 754 bits, and every `f64` its 16.
//!
//! - `figure <a> <c>`: the datum's equatorial and polar radii, metres (`f64`).
//! - For each patch, `patch <case> <face> <level> <i> <j>`, the case `inside`, `edge` or `corner`;
//!   then
//!   - `origin <x> <y> <z> <h0>`: the bake's origin, body-fixed metres, and its height, metres
//!     (`f64`);
//!   - `range <lowest> <highest>`: the bake's height range over both channels, metres (`f32`);
//!   - `vertex <x> <y> <h> <q0.x> <q0.y> <q0.z>` for each vertex a sample's triangle uses, once
//!     each, in row order (y, then x): its own height and its own offset from the origin, metres, as
//!     the bake narrowed them (`f32`);
//!   - `sample <kind> <x> <y> <half> <fa> <fb> <h_g> <Md.x> <Md.y> <Md.z> <nu.x> <nu.y> <nu.z>` for
//!     each point, in the order above: its kind (`random`, `vertex`, `diagonal`, `quad_edge`,
//!     `patch_edge` or `far_quad`); the ground's quad (x, y) and half (`upper` or `lower`); the
//!     point's weights across and up the quad, each 0 to 1; the ground's height `h_g`, metres; and
//!     the foot's datum point M·d, body-fixed metres, and its unit normal ν(d) (`f64`).

use std::collections::BTreeSet;

use hyperion_testkit::float::{bits, bits_f32};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::tests::{cells_under, client, code, patch_in};
use super::*;
use crate::cube::{Face, uv_to_st, xyz_to_face_uv};
use crate::field::FieldView;
use crate::geometry::finest_level;
use crate::ground::{FinestTriangle, GroundPoint, ground_at};
use crate::patch::collision::{Quad, quad_of};
use crate::patch::{NormalScale, VertexPath};
use crate::synth::test_support::{SEED, point_in, rim_cells, world};
use crate::testing::SyntheticWorld;

/// The bake's options: the `BakedOffsets` path, the high setting's, whose bake holds the own
/// heights that `FaceDifferences` reads beside the offsets. Nothing here reads the normals'
/// resolution, the skirts, the weights or the horizon map.
const BOTH_PATHS: TerrainBakeOptions = TerrainBakeOptions {
    patch: BakeOptions {
        vertex_path: VertexPath::BakedOffsets,
        normals: NormalScale::Mesh,
        skirt_m: 0.0,
    },
    weights: WeightsLayer::Skipped,
    horizon: HorizonLayer::Skipped,
};

/// The sample points drawn at random in each patch.
const RANDOM_SAMPLES: u32 = 64;

/// How close to a lattice line, in quads, R05's interpolant takes a query to be on it:
/// `patch::collision`'s `SNAP_QUADS`, private there. A sample whose weights the two would snap
/// differently fails the writer's check that the weights give the ground's height bit for bit.
const SNAP_QUADS: f64 = 1e-6;

/// The `f64` `x` as the golden writes it.
#[must_use]
fn hex64(x: f64) -> String {
    format!("0x{:016x}", bits(x))
}

/// The `f32` `x` as the golden writes it.
#[must_use]
fn hex32(x: f32) -> String {
    format!("0x{:08x}", bits_f32(x))
}

/// The three `f64` of `v` as the golden writes them.
#[must_use]
fn hex64s(v: [f64; 3]) -> String {
    v.map(hex64).join(" ")
}

/// The three finest patches of `w` by case: inside a face, under its first listed crater's rim;
/// on face −y's v-max edge, a third of the way along it; and at the corner of faces +x, +y and −z.
#[must_use]
fn patches(w: SyntheticWorld) -> [(&'static str, PatchKey); 3] {
    let field = world(w);
    let finest = finest_level(field.header().figure().equatorial_radius_m);
    let last = (1_u32 << finest) - 1;
    [
        ("inside", patch_in(rim_cells(field, 1)[0], finest)),
        (
            "edge",
            PatchKey::new(Face::NegY, finest, last / 3, last).expect("a face edge's patch"),
        ),
        (
            "corner",
            PatchKey::new(Face::NegZ, finest, last, last).expect("a cube corner's patch"),
        ),
    ]
}

/// The sample directions of `key`, by kind, the random ones from `rng` (see the
/// [module documentation](self)).
fn sample_dirs(key: PatchKey, rng: &mut Lcg) -> Vec<(&'static str, [f64; 3])> {
    let quads = f64::from(PATCH_QUADS);
    // The direction at (x, y) in the patch's quads, each 0 to 64.
    let at = |x: f64, y: f64| point_in(key, x / quads, y / quads);
    // A coordinate strictly inside the patch, a thousandth of it from either edge.
    let inside = |rng: &mut Lcg| quads * (0.001 + 0.998 * rng.next_f64());
    let mut dirs: Vec<(&'static str, [f64; 3])> = (0..RANDOM_SAMPLES)
        .map(|_| {
            let x = inside(rng);
            ("random", at(x, inside(rng)))
        })
        .collect();
    dirs.extend([
        ("vertex", key.vertex_dir(17, 43)),
        ("diagonal", at(40.3, 21.3)),
        ("quad_edge", at(9.0, 50.7)),
        ("patch_edge", at(29.6, 0.0)),
        ("far_quad", at(63.62, 63.41)),
    ]);
    dirs
}

/// A sample point: its kind, the ground under it and its weights (fa, fb) in the ground's quad.
struct Sample {
    kind: &'static str,
    ground: GroundPoint,
    weights: (f64, f64),
}

/// `c`, a lattice coordinate, moved onto the nearest lattice line within [`SNAP_QUADS`] of it, as
/// R05's interpolant moves it.
#[must_use]
fn snapped(c: f64) -> f64 {
    let nearest = c.round();
    if (c - nearest).abs() <= SNAP_QUADS && nearest >= 0.0 {
        nearest
    } else {
        c
    }
}

/// The weights (fa, fb) of the unit direction `dir` across and up `triangle`'s quad, in R05's
/// operations: its lattice coordinates at the triangle's level, from the face's (u, v) through the
/// inverse warp, snapped, less the quad's corner.
#[must_use]
fn weights(triangle: FinestTriangle, dir: [f64; 3]) -> (f64, f64) {
    let key = triangle.key();
    let on_face = xyz_to_face_uv(dir);
    assert_eq!(
        on_face.face,
        key.face(),
        "the foot is on the triangle's face"
    );
    let side = f64::from(PATCH_QUADS << key.level());
    let corner = |cell: u32, quad: u8| f64::from(cell * PATCH_QUADS + u32::from(quad));
    (
        snapped(uv_to_st(on_face.u) * side) - corner(key.i(), triangle.x()),
        snapped(uv_to_st(on_face.v) * side) - corner(key.j(), triangle.y()),
    )
}

/// The ground under a point a few metres above the ground at the unit direction `dir`, a sample of
/// kind `kind` in `key`, with the checks the [module documentation](self) lists.
///
/// # Panics
///
/// If the view lacks a cell a query reads, or a check fails.
fn sample<F: SurveyView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    (kind, dir): (&'static str, [f64; 3]),
    rng: &mut Lcg,
) -> Sample {
    let held = "the view holds every cell under the patch";
    let level = band(key.level());
    let below = synth.height_at(cache, dir, level).expect(held).height_m;
    let point = synth
        .field()
        .header()
        .figure()
        .surface_point(dir, below + 1.0 + 9.0 * rng.next_f64());
    let ground = ground_at(synth, cache, &point).expect(held);
    let t = ground.triangle();
    assert_eq!(
        t.key(),
        key,
        "{kind}: the ground's triangle lies in the patch"
    );
    let quad = |cell: u32, x: u8| u64::from(cell) * u64::from(PATCH_QUADS) + u64::from(x);
    let (a, b) = (quad(key.i(), t.x()), quad(key.j(), t.y()));
    let (face, located) = quad_of(ground.dir(), key.level());
    assert_eq!(
        (face, located.a(), located.b()),
        (key.face(), a, b),
        "{kind}: the foot's quad is the ground's"
    );
    let (fa, fb) = weights(t, ground.dir());
    assert!(
        (0.0..=1.0).contains(&fa) && (0.0..=1.0).contains(&fb),
        "{kind}: weights ({fa}, {fb}) outside the quad"
    );
    assert_eq!(
        t.upper(),
        fa < fb,
        "{kind}: R05's diagonal rule at ({fa}, {fb})"
    );
    let [first, second, third] = t.vertices().map(|(x, y)| {
        synth
            .height_at(cache, key.vertex_dir(x, y), level)
            .expect(held)
            .height_m
    });
    // `vertices` lists (x, y), (x + 1, y + 1), (x, y + 1) in the upper half and (x, y), (x + 1, y),
    // (x + 1, y + 1) in the lower; the interpolant takes (x, y), (x + 1, y + 1), then the third.
    let (h00, h11, other) = if t.upper() {
        (first, second, third)
    } else {
        (first, third, second)
    };
    assert_eq!(
        bits(Quad::new(a, b, fa, fb, t.upper()).interpolate(h00, h11, other)),
        bits(ground.height().value()),
        "{kind}: R05's interpolant at ({fa}, {fb}) is the ground's height"
    );
    Sample {
        kind,
        ground,
        weights: (fa, fb),
    }
}

/// Writes the lines of `bake`, the case `case`, and of its `samples` on `figure` to `out`.
fn write_patch(
    out: &mut GoldenWriter,
    case: &str,
    figure: &Spheroid,
    bake: &TerrainBake,
    samples: &[Sample],
) {
    let p = bake.patch();
    let k = p.key;
    out.line(&format!(
        "patch {case} {} {} {} {}",
        k.face().index(),
        k.level(),
        k.i(),
        k.j()
    ));
    out.line(&format!(
        "origin {} {}",
        hex64s(p.origin),
        hex64(p.origin_height_m)
    ));
    out.line(&format!(
        "range {} {}",
        hex32(p.height_range_m.0),
        hex32(p.height_range_m.1)
    ));
    let offsets = p
        .offsets
        .as_deref()
        .expect("a BakedOffsets bake carries its offsets");
    // Row order: y, then x.
    let used: BTreeSet<(u8, u8)> = samples
        .iter()
        .flat_map(|s| s.ground.triangle().vertices())
        .map(|(x, y)| (y, x))
        .collect();
    for (y, x) in used {
        let n = usize::from(y) * PATCH_VERTICES + usize::from(x);
        let q0: Vec<String> = offsets[6 * n..6 * n + 3]
            .iter()
            .map(|&q| hex32(q))
            .collect();
        out.line(&format!(
            "vertex {x} {y} {} {}",
            hex32(p.heights[2 * n]),
            q0.join(" ")
        ));
    }
    for s in samples {
        let t = s.ground.triangle();
        let d = s.ground.dir();
        out.line(&format!(
            "sample {} {} {} {} {} {} {} {} {}",
            s.kind,
            t.x(),
            t.y(),
            if t.upper() { "upper" } else { "lower" },
            hex64(s.weights.0),
            hex64(s.weights.1),
            hex64(s.ground.height().value()),
            hex64s(figure.point(d)),
            hex64s(figure.normal(d)),
        ));
    }
}

/// The golden text of `w`.
#[must_use]
fn drawn_ground_text(w: SyntheticWorld) -> String {
    let field = world(w);
    let figure = field.header().figure();
    let patches = patches(w);
    let cells: BTreeSet<PatchKey> = patches
        .iter()
        .flat_map(|&(_, key)| cells_under(field, key))
        .collect();
    let view = client(w, &cells, |_| code(100.0));
    let synth = Synthesiser::new(&view, SEED);
    let mut cache = SynthCache::new();
    let mut rng = Lcg::new(0x6472_6177_6e5f_6764);
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    out.line(&format!(
        "figure {} {}",
        hex64(figure.equatorial_radius_m),
        hex64(figure.polar_radius_m)
    ));
    for (case, key) in patches {
        let bake = bake_terrain(&synth, &mut cache, key, &BOTH_PATHS, &[])
            .expect("the view holds every cell under the patch");
        let layers = bake.layers();
        assert!(
            layers.is_complete() && layers.survey_mask().iter().all(|&m| m == SURVEYED),
            "{w:?} {case}: the bake is surveyed whole"
        );
        let samples: Vec<Sample> = sample_dirs(key, &mut rng)
            .into_iter()
            .map(|at| sample(&synth, &mut cache, key, at, &mut rng))
            .collect();
        write_patch(&mut out, case, &figure, &bake, &samples);
    }
    out.finish()
}

#[test]
fn the_drawn_ground_matches_its_goldens() {
    golden!(
        "drawn_ground/earth_like",
        &drawn_ground_text(SyntheticWorld::EarthLike)
    );
    golden!(
        "drawn_ground/moon_like",
        &drawn_ground_text(SyntheticWorld::MoonLike)
    );
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::{Path, PathBuf};

    /// The golden files of `tests/golden/drawn_ground/`, in name order.
    fn goldens() -> Vec<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("drawn_ground");
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("the drawn ground's golden directory is readable")
            .map(|entry| entry.expect("a directory entry is readable").path())
            .collect();
        paths.sort();
        paths
    }

    /// Every golden file of `tests/golden/drawn_ground/` carries `GENERATOR_VERSION`, and there are
    /// two.
    #[test]
    fn every_drawn_ground_golden_carries_the_generator_version() {
        let expected = format!("# generator_version = {}", crate::generator_version());
        let paths = goldens();
        for path in &paths {
            let text = std::fs::read_to_string(path).expect("a golden file is readable");
            assert_eq!(
                text.lines().next(),
                Some(expected.as_str()),
                "{}",
                path.display()
            );
        }
        assert_eq!(paths.len(), 2);
    }

    /// The two golden files stay under 200 kB together (decision-r10-t15a-scope §3).
    #[test]
    fn the_drawn_ground_goldens_stay_under_200_kb() {
        let bytes: u64 = goldens()
            .iter()
            .map(|path| {
                std::fs::metadata(path)
                    .expect("a golden file's metadata is readable")
                    .len()
            })
            .sum();
        assert!(bytes < 200_000, "{bytes} B");
    }
}
