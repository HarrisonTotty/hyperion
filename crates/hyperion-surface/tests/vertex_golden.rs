//! The `FaceDifferences` vertex path's golden file, `tests/golden/vertex_f32.golden` (plan R05,
//! T4.b): the `f32` reference's inputs and outputs, as bits, so that the client's TypeScript
//! emulation of the WGSL (T11.b) is checked against the same numbers, on native, `wasm32-wasip1`
//! and `wasm32-unknown-unknown`.
//!
//! The header carries [`TEST_PLANET_VERSION`] (Design note 13); where this test fails with the
//! testkit's hint to "bump `GENERATOR_VERSION`", read "bump `TEST_PLANET_VERSION`".
//!
//! # Format
//!
//! Every `f32` is `0x` and its 8 hexadecimal digits of IEEE 754 bits. For each of the twelve
//! patches (`patch <n> <face> <level> <i> <j>`, levels 0, 10 and 19 in turn, two on each face,
//! three of them at cube corners):
//!
//! - `terms <a.x> <a.y> <a.z> <e1.x> … <e2.z> <s0> <t0> <u0> <v0> <step> <scale.x> <scale.y>
//!   <scale.z> <m0.x> <m0.y> <m0.z> <nu0.x> <nu0.y> <nu0.z> <h0> <straddles>`: the uniforms,
//!   `straddles` as 0 or 1;
//! - `vertex <x> <y> <h0> <h1> <ha> <hb> <p.x> <p.y> <p.z> <q.x> <q.y> <q.z>` for x and y each in
//!   0, 1, 31, 32, 33, 63 and 64 (y outer): the inputs, the vertex's own height h0, its morph
//!   height h1, and the morph heights of the two even neighbours (x − 1, y) and (x + 1, y),
//!   (x, y − 1) and (x, y + 1), or (x − 1, y − 1) and (x + 1, y + 1) on the parent mesh's
//!   diagonal that an odd vertex's morph target reads (h1 twice at an even vertex, and at level
//!   0); and the outputs, the own position p and the morph position q relative to the origin,
//!   from `face_difference_position_f32` and `face_difference_morph_f32`.
//!
//! The heights are a smooth fixture's, not the test planet's, so that the file pins the vertex
//! formula alone: h = 100 + 16,000 (x y + 0.3 z²) ÷ a² of the spheroid point, plus the level in
//! metres.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_surface::TEST_PLANET_VERSION;
use hyperion_surface::cube::{Face, PatchKey};
use hyperion_surface::patch::vertex::{
    PatchTerms, face_difference_morph_f32, face_difference_position_f32,
};
use hyperion_surface::spheroid::Spheroid;
use hyperion_testkit::float::bits_f32;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The vertices printed along each axis.
const PRINTED: [u8; 7] = [0, 1, 31, 32, 33, 63, 64];

fn hex(v: f32) -> String {
    format!("0x{:08x}", bits_f32(v))
}

fn narrow(x: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "heights are f32 on the GPU by design"
    )]
    let y = x as f32;
    y
}

/// The fixture height at the vertex (`x`, `y`) of `key` at `level`, metres.
fn height(figure: &Spheroid, key: PatchKey, x: u8, y: u8, level: u8) -> f64 {
    let p = figure.point(key.vertex_dir(x, y));
    let a2 = figure.equatorial_radius_m * figure.equatorial_radius_m;
    100.0 + f64::from(level) + 16_000.0 * (p[0] * p[1] + 0.3 * p[2] * p[2]) / a2
}

/// The morph height of vertex (`x`, `y`): the parent level's at an even vertex, the parent mesh's
/// chord at an odd one (see `hyperion_surface::patch`).
#[expect(
    clippy::manual_midpoint,
    reason = "the bake's own chord, ½ (a + b), in its exact operations"
)]
fn morph_height(figure: &Spheroid, key: PatchKey, x: u8, y: u8) -> f64 {
    let Some(parent) = key.level().checked_sub(1) else {
        return height(figure, key, x, y, 0);
    };
    let at = |px: u8, py: u8| height(figure, key, px, py, parent);
    match (x % 2, y % 2) {
        (0, 0) => at(x, y),
        (1, 0) => 0.5 * (at(x - 1, y) + at(x + 1, y)),
        (0, _) => 0.5 * (at(x, y - 1) + at(x, y + 1)),
        _ => 0.5 * (at(x - 1, y - 1) + at(x + 1, y + 1)),
    }
}

/// The twelve patches.
fn patches() -> Vec<PatchKey> {
    let mut keys = Vec::new();
    for (n, face) in (0_u8..).zip(Face::ALL) {
        for m in 0..2_u8 {
            let level = [0, 10, 19][usize::from((2 * n + m) % 3)];
            let last = (1_u32 << level) - 1;
            // Three cube corners (the first patch of faces 0, 2 and 4), the rest inside the face.
            let (i, j) = if m == 0 && n % 2 == 0 {
                (last, last)
            } else {
                (last / 3, (2 * last) / 3)
            };
            keys.push(PatchKey::new(face, level, i, j).expect("a cell of the level"));
        }
    }
    keys
}

#[test]
fn the_vertex_reference_matches_its_golden() {
    let figure = Spheroid::WGS84;
    let mut w = GoldenWriter::new();
    w.header(TEST_PLANET_VERSION);
    for (n, key) in (0_u32..).zip(patches()) {
        let level = key.level();
        let h0 = height(&figure, key, 32, 32, level);
        let terms = PatchTerms::new(key, &figure, h0).narrow();
        w.line(&format!(
            "patch {n} {} {level} {} {}",
            key.face().index(),
            key.i(),
            key.j()
        ));
        let mut fields: Vec<String> = terms.axes.iter().flatten().map(|v| hex(*v)).collect();
        fields.extend(terms.st0.iter().chain(&terms.uv0).map(|v| hex(*v)));
        fields.push(hex(terms.step));
        fields.extend(
            terms
                .scale
                .iter()
                .chain(&terms.m0)
                .chain(&terms.nu0)
                .map(|v| hex(*v)),
        );
        fields.push(hex(terms.h0_m));
        fields.push(String::from(if terms.straddles { "1" } else { "0" }));
        w.line(&format!("terms {}", fields.join(" ")));
        let h1 = |x: u8, y: u8| narrow(morph_height(&figure, key, x, y));
        for &y in &PRINTED {
            for &x in &PRINTED {
                let own = narrow(height(&figure, key, x, y, level));
                let p = face_difference_position_f32(&terms, x, y, own);
                let q = face_difference_morph_f32(&terms, x, y, h1);
                let (ha, hb) = match (level, x % 2, y % 2) {
                    (0, _, _) | (_, 0, 0) => (h1(x, y), h1(x, y)),
                    (_, 1, 0) => (h1(x - 1, y), h1(x + 1, y)),
                    (_, 0, _) => (h1(x, y - 1), h1(x, y + 1)),
                    _ => (h1(x - 1, y - 1), h1(x + 1, y + 1)),
                };
                w.line(&format!(
                    "vertex {x} {y} {} {} {} {} {} {} {} {} {} {}",
                    hex(own),
                    hex(h1(x, y)),
                    hex(ha),
                    hex(hb),
                    hex(p[0]),
                    hex(p[1]),
                    hex(p[2]),
                    hex(q[0]),
                    hex(q[1]),
                    hex(q[2]),
                ));
            }
        }
    }
    golden!("vertex_f32", &w.finish());
}
