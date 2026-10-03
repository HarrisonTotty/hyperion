//! The `FaceDifferences` vertex path: a vertex's position relative to its patch origin, formed
//! from small quantities, in `f64` and as the `f32` reference the WGSL follows operation for
//! operation (plan R05, Design note 4).
//!
//! The vertex shader must never form the absolute position M·d + h·ν in `f32`: at an Earth's
//! radius its step is half a metre. The `BakedOffsets` path bakes each vertex's offset from the
//! origin in `f64` and narrows it ([`super::bake_patch`]); this one rebuilds the offset on the GPU
//! from a patch's terms ([`PatchTerms`]), the shared grid of face offsets and the vertex's height:
//!
//! - **Face coordinates.** Vertex (x, y) of patch (i, j) at level n sits at
//!   s = s₀ + δs with s₀ the patch centre's and δs = (x − 32) ÷ 2ⁿ⁺⁶, exact. The warp's difference
//!   is formed without cancellation: u − u₀ = 4 δs (s + s₀) ÷ 3 where both are at least ½, and
//!   4 δs (2 − s − s₀) ÷ 3 where both are below; a level-0 patch, which straddles s = ½, forms
//!   u(s) − u(s₀) directly, as its precision need is a metre.
//! - **Direction.** With n = a + u e₁ + v e₂ the cube point and Δ = (u − u₀) e₁ + (v − v₀) e₂,
//!   d − d₀ = Δ ÷ |n| + n₀ (|n₀|² − |n|²) ÷ (|n| |n₀| (|n| + |n₀|)), where
//!   |n₀|² − |n|² = −(2 n₀ · Δ + |Δ|²), so no difference of nearly equal numbers is formed.
//! - **Normal.** With m = M⁻¹ d, δ = m − m₀ = M⁻¹ (d − d₀) and the same identity,
//!   ν − ν₀ = δ ÷ |m| + m₀ (|m₀|² − |m|²) ÷ (|m| |m₀| (|m| + |m₀|)).
//! - **Position.** P − P₀ = M (d − d₀) + h (ν − ν₀) + (h − h₀) ν₀, with P₀ the origin at the
//!   centre's own height h₀.
//!
//! A morph target's position is the parent mesh's: at an even vertex the same formula at the
//! morph height h₁; at an odd one the mean of the formula at its two even neighbours on the parent
//! mesh's diagonal, at their morph heights. The vertex on a face edge is formed from its own face's
//! (u, v), not the canonical face's, a difference of an ulp that the skirts cover.

use crate::cube::{Face, PatchKey, st_to_uv};
use crate::spheroid::Spheroid;

/// A patch's terms for the `FaceDifferences` path, in `f64`; [`PatchTerms::narrow`] gives the
/// `f32` uniforms the shader reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatchTerms {
    /// The face's centre axis a, and its u and v axes e₁ and e₂, each a signed unit axis.
    pub axes: [[f64; 3]; 3],
    /// The patch centre's s₀ and t₀.
    pub st0: [f64; 2],
    /// The patch centre's u₀ and v₀.
    pub uv0: [f64; 2],
    /// The step in s of one vertex, 2^−(level + 6).
    pub step: f64,
    /// M's diagonal, (a, a, c), metres.
    pub scale: [f64; 3],
    /// m₀ = M⁻¹ d₀, per metre.
    pub m0: [f64; 3],
    /// ν₀, the spheroid normal at the centre.
    pub nu0: [f64; 3],
    /// h₀, the centre's own height, metres.
    pub h0_m: f64,
    /// Whether the patch straddles s = ½ or t = ½ (level 0 only).
    pub straddles: bool,
}

/// The `f32` uniforms of [`PatchTerms`], field for field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatchTermsF32 {
    /// The face's axes.
    pub axes: [[f32; 3]; 3],
    /// s₀ and t₀.
    pub st0: [f32; 2],
    /// u₀ and v₀.
    pub uv0: [f32; 2],
    /// The vertex step in s.
    pub step: f32,
    /// M's diagonal, metres.
    pub scale: [f32; 3],
    /// m₀, per metre.
    pub m0: [f32; 3],
    /// ν₀.
    pub nu0: [f32; 3],
    /// h₀, metres.
    pub h0_m: f32,
    /// Whether the patch straddles s = ½ or t = ½.
    pub straddles: bool,
}

/// The face's axes a, e₁ and e₂ by S2's convention (see [`crate::cube`]).
#[must_use]
const fn face_axes(face: Face) -> [[f64; 3]; 3] {
    match face {
        Face::PosX => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        Face::PosY => [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        Face::PosZ => [[0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]],
        Face::NegX => [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]],
        Face::NegY => [[0.0, -1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]],
        Face::NegZ => [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
    }
}

#[must_use]
fn narrow(x: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the uniforms are f32 by design; every value is far inside f32's range"
    )]
    let y = x as f32;
    y
}

#[must_use]
fn narrow3(v: [f64; 3]) -> [f32; 3] {
    v.map(narrow)
}

impl PatchTerms {
    /// The terms of `key` on `figure`, for an origin at height `h0_m` metres above the centre vertex.
    #[must_use]
    pub fn new(key: PatchKey, figure: &Spheroid, h0_m: f64) -> Self {
        let level = key.level();
        let cells = f64::from(1_u32 << level);
        let st0 = [
            (f64::from(key.i()) + 0.5) / cells,
            (f64::from(key.j()) + 0.5) / cells,
        ];
        let uv0 = st0.map(st_to_uv);
        let axes = face_axes(key.face());
        let n0 = [0, 1, 2].map(|c| axes[0][c] + uv0[0] * axes[1][c] + uv0[1] * axes[2][c]);
        let len = (n0[0] * n0[0] + n0[1] * n0[1] + n0[2] * n0[2]).sqrt();
        let d0 = n0.map(|c| c / len);
        let scale = [
            figure.equatorial_radius_m,
            figure.equatorial_radius_m,
            figure.polar_radius_m,
        ];
        let m0 = [0, 1, 2].map(|c| d0[c] / scale[c]);
        Self {
            axes,
            st0,
            uv0,
            step: crate::geometry::lattice_step(level),
            scale,
            m0,
            nu0: figure.normal(d0),
            h0_m,
            straddles: level == 0,
        }
    }

    /// The `f32` uniforms.
    #[must_use]
    pub fn narrow(&self) -> PatchTermsF32 {
        PatchTermsF32 {
            axes: self.axes.map(narrow3),
            st0: self.st0.map(narrow),
            uv0: self.uv0.map(narrow),
            step: narrow(self.step),
            scale: narrow3(self.scale),
            m0: narrow3(self.m0),
            nu0: narrow3(self.nu0),
            h0_m: narrow(self.h0_m),
            straddles: self.straddles,
        }
    }
}

/// The vertex's face offset from the patch centre in vertex steps, x − 32, of the shared grid.
#[must_use]
pub fn grid_offset(x: u8) -> f64 {
    f64::from(x) - 32.0
}

/// The warp's difference u(s₀ + δs) − u(s₀), in `f64`.
#[must_use]
fn warp_difference(s0: f64, ds: f64, straddles: bool) -> f64 {
    let s = s0 + ds;
    if straddles {
        st_to_uv(s) - st_to_uv(s0)
    } else if s0 >= 0.5 {
        4.0 * ds * (s + s0) / 3.0
    } else {
        4.0 * ds * (2.0 - s - s0) / 3.0
    }
}

/// The small-difference identity: for p = p₀ + Δ, `p ÷ |p| − p₀ ÷ |p₀|`, in `f64`.
#[must_use]
fn unit_difference(p0: [f64; 3], delta: [f64; 3]) -> [f64; 3] {
    let dot = p0[0] * delta[0] + p0[1] * delta[1] + p0[2] * delta[2];
    let dd = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
    let len0_sq = p0[0] * p0[0] + p0[1] * p0[1] + p0[2] * p0[2];
    let len0 = len0_sq.sqrt();
    let len = (len0_sq + 2.0 * dot + dd).sqrt();
    let diff = -(2.0 * dot + dd);
    let factor = diff / (len * len0 * (len + len0));
    [0, 1, 2].map(|c| delta[c] / len + p0[c] * factor)
}

/// The position of vertex (`x`, `y`) at height `h_m` metres relative to the origin, in `f64`.
#[must_use]
pub fn face_difference_position(terms: &PatchTerms, x: u8, y: u8, h_m: f64) -> [f64; 3] {
    let du = warp_difference(terms.st0[0], grid_offset(x) * terms.step, terms.straddles);
    let dv = warp_difference(terms.st0[1], grid_offset(y) * terms.step, terms.straddles);
    let [a, e1, e2] = terms.axes;
    let n0 = [0, 1, 2].map(|c| a[c] + terms.uv0[0] * e1[c] + terms.uv0[1] * e2[c]);
    let delta = [0, 1, 2].map(|c| du * e1[c] + dv * e2[c]);
    let dd = unit_difference(n0, delta);
    let dm = [0, 1, 2].map(|c| dd[c] / terms.scale[c]);
    let dnu = unit_difference(terms.m0, dm);
    let dh = h_m - terms.h0_m;
    [0, 1, 2].map(|c| terms.scale[c] * dd[c] + h_m * dnu[c] + dh * terms.nu0[c])
}

/// [`warp_difference`] in `f32`, in the WGSL's order.
#[must_use]
fn warp_difference_f32(s0: f32, ds: f32, straddles: bool) -> f32 {
    let s = s0 + ds;
    if straddles {
        st_to_uv_f32(s) - st_to_uv_f32(s0)
    } else if s0 >= 0.5 {
        4.0 * ds * (s + s0) / 3.0
    } else {
        4.0 * ds * (2.0 - s - s0) / 3.0
    }
}

/// The warp in `f32`, in [`st_to_uv`]'s order.
#[must_use]
fn st_to_uv_f32(s: f32) -> f32 {
    if s >= 0.5 {
        (4.0 * s * s - 1.0) / 3.0
    } else {
        let r = 1.0 - s;
        (1.0 - 4.0 * r * r) / 3.0
    }
}

/// [`unit_difference`] in `f32`, in the WGSL's order.
#[must_use]
fn unit_difference_f32(p0: [f32; 3], delta: [f32; 3]) -> [f32; 3] {
    let dot = p0[0] * delta[0] + p0[1] * delta[1] + p0[2] * delta[2];
    let dd = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
    let len0_sq = p0[0] * p0[0] + p0[1] * p0[1] + p0[2] * p0[2];
    let len0 = len0_sq.sqrt();
    let len = (len0_sq + 2.0 * dot + dd).sqrt();
    let diff = -(2.0 * dot + dd);
    let factor = diff / (len * len0 * (len + len0));
    [0, 1, 2].map(|c| delta[c] / len + p0[c] * factor)
}

/// The `f32` reference of [`face_difference_position`]: exactly the operations the WGSL performs,
/// in the same order, on the `f32` uniforms, the grid offsets and the `f32` height.
#[must_use]
pub fn face_difference_position_f32(terms: &PatchTermsF32, x: u8, y: u8, h_m: f32) -> [f32; 3] {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a grid offset of −32 to 32 is exact in f32"
    )]
    let offset = |n: u8| grid_offset(n) as f32;
    let du = warp_difference_f32(terms.st0[0], offset(x) * terms.step, terms.straddles);
    let dv = warp_difference_f32(terms.st0[1], offset(y) * terms.step, terms.straddles);
    let [a, e1, e2] = terms.axes;
    let n0 = [0, 1, 2].map(|c| a[c] + terms.uv0[0] * e1[c] + terms.uv0[1] * e2[c]);
    let delta = [0, 1, 2].map(|c| du * e1[c] + dv * e2[c]);
    let dd = unit_difference_f32(n0, delta);
    let dm = [0, 1, 2].map(|c| dd[c] / terms.scale[c]);
    let dnu = unit_difference_f32(terms.m0, dm);
    let dh = h_m - terms.h0_m;
    [0, 1, 2].map(|c| terms.scale[c] * dd[c] + h_m * dnu[c] + dh * terms.nu0[c])
}

/// The `f32` morph target's position of vertex (`x`, `y`): at an even vertex the formula at its
/// morph height, at an odd one the mean of the formula at its two even neighbours on the parent
/// mesh's diagonal, at their morph heights, `h1(x, y)`.
///
/// # Panics
///
/// If `x` or `y` is above 64.
#[must_use]
pub fn face_difference_morph_f32(
    terms: &PatchTermsF32,
    x: u8,
    y: u8,
    h1: impl Fn(u8, u8) -> f32,
) -> [f32; 3] {
    assert!(x <= 64 && y <= 64, "vertex ({x}, {y}) is outside a patch");
    let at = |px: u8, py: u8| face_difference_position_f32(terms, px, py, h1(px, py));
    // A level-0 patch (the one that straddles s = ½) has no parent: its morph target is itself.
    if terms.straddles {
        return at(x, y);
    }
    let (a, b) = match (x % 2, y % 2) {
        (0, 0) => return at(x, y),
        (1, 0) => (at(x - 1, y), at(x + 1, y)),
        (0, _) => (at(x, y - 1), at(x, y + 1)),
        _ => (at(x - 1, y - 1), at(x + 1, y + 1)),
    };
    #[expect(
        clippy::manual_midpoint,
        reason = "the exact operations are fixed for bit-for-bit agreement with the client"
    )]
    let mean = |c: usize| 0.5 * (a[c] + b[c]);
    [mean(0), mean(1), mean(2)]
}

#[cfg(test)]
/// The naive `f32` position M·d + h·ν less the origin, as a shader must not form it: what the
/// tests show both paths are needed against.
#[must_use]
pub fn naive_position_f32(
    figure: &Spheroid,
    dir: [f64; 3],
    h_m: f32,
    origin_m: [f64; 3],
) -> [f32; 3] {
    let p = figure.point(dir).map(narrow);
    let nu = figure.normal(dir).map(narrow);
    let o = origin_m.map(narrow);
    [0, 1, 2].map(|c| p[c] + h_m * nu[c] - o[c])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::fixture::Fixture;
    use crate::patch::{PATCH_VERTICES, bake_vertices};
    use hyperion_testkit::lcg::Lcg;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// 100 patches at `level` over the six faces, a third of them at faces' corners and edges.
    fn patches(level: u8) -> Vec<PatchKey> {
        let last = (1_u32 << level) - 1;
        let mut rng = Lcg::new(0x7665_7274 + u64::from(level));
        (0..100_u8)
            .map(|n| {
                let face = Face::from_index(n % 6).unwrap();
                let mut free = || u32::try_from(rng.next_below(u64::from(last) + 1)).unwrap();
                let (i, j) = match n % 3 {
                    0 => (
                        [0, last][usize::from(n % 2)],
                        [0, last][usize::from(n / 2 % 2)],
                    ),
                    1 => (free(), [0, last][usize::from(n % 2)]),
                    _ => (free(), free()),
                };
                PatchKey::new(face, level, i, j).unwrap()
            })
            .collect()
    }

    fn distance_f32(a: [f32; 3], b: [f64; 3]) -> f64 {
        let d = [0, 1, 2].map(|c| f64::from(a[c]) - b[c]);
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    /// The largest error of the baked offsets, the face-difference reference and the naive form
    /// against the f64 positions, own-level and morph targets, over `keys` on `figure`, metres.
    fn worst_errors(figure: Spheroid, keys: &[PatchKey]) -> (f64, f64, f64) {
        let source = Fixture::relief(figure, 16_000.0);
        let (mut baked, mut faces, mut naive) = (0.0_f64, 0.0_f64, 0.0_f64);
        for &key in keys {
            let b = bake_vertices(&source, key, &mut ()).unwrap();
            let terms = PatchTerms::new(key, &figure, b.origin_height).narrow();
            let h1 = |x: u8, y: u8| {
                let v = &b.vertices[usize::from(y) * PATCH_VERTICES + usize::from(x)];
                narrow(v.h1)
            };
            for y in 0..=64_u8 {
                for x in 0..=64_u8 {
                    let v = &b.vertices[usize::from(y) * PATCH_VERTICES + usize::from(x)];
                    baked = baked.max(distance_f32(v.q0.map(narrow), v.q0));
                    let own = face_difference_position_f32(&terms, x, y, narrow(v.h0));
                    let morph = face_difference_morph_f32(&terms, x, y, h1);
                    faces = faces
                        .max(distance_f32(own, v.q0))
                        .max(distance_f32(morph, v.q1));
                    let n = naive_position_f32(&figure, v.dir, narrow(v.h0), b.origin);
                    naive = naive.max(distance_f32(n, v.q0));
                }
            }
        }
        (baked, faces, naive)
    }

    #[test]
    fn both_paths_hold_a_millimetre_at_level_19_and_the_naive_form_does_not() {
        for figure in [Spheroid::WGS84, Spheroid::sphere(6.371e6)] {
            let (baked, faces, naive) = worst_errors(figure, &patches(19));
            println!("{figure:?}: baked {baked} m, face differences {faces} m, naive {naive} m");
            assert!(baked < 1e-3, "baked offsets: {baked} m");
            assert!(faces < 1e-3, "face differences: {faces} m");
            assert!(
                naive > 1e-3,
                "the naive form held {naive} m, so the test cannot fail"
            );
        }
    }

    #[test]
    fn both_paths_hold_a_metre_at_level_0() {
        let keys: Vec<PatchKey> = Face::ALL.into_iter().map(PatchKey::root).collect();
        let (baked, faces, _) = worst_errors(Spheroid::WGS84, &keys);
        assert!(baked < 1.0 && faces < 1.0, "{baked} m, {faces} m");
    }

    #[test]
    #[expect(
        clippy::many_single_char_names,
        reason = "a bake, its vertex, its index and coordinates, as the formula names them"
    )]
    fn the_f64_formula_is_the_spheroid_position() {
        let figure = Spheroid::WGS84;
        let source = Fixture::relief(figure, 16_000.0);
        for key in patches(10).into_iter().take(12) {
            let b = bake_vertices(&source, key, &mut ()).unwrap();
            let terms = PatchTerms::new(key, &figure, b.origin_height);
            for (n, v) in b.vertices.iter().enumerate().step_by(31) {
                let (x, y) = (n % PATCH_VERTICES, n / PATCH_VERTICES);
                let p = face_difference_position(
                    &terms,
                    u8::try_from(x).unwrap(),
                    u8::try_from(y).unwrap(),
                    v.h0,
                );
                let d = [0, 1, 2].map(|c| p[c] - v.q0[c]);
                assert!(
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < 1e-6,
                    "{key:?} {n}"
                );
            }
        }
    }
}
