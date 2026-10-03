//! The patch bake: a patch's heights, morph targets, positions, normals, bounds and skirts, from
//! any [`HeightSource`] (plan R05, Design notes 4 to 6).
//!
//! # The mesh
//!
//! A patch is 65 × 65 vertices, (x, y) for x and y from 0 to 64, at the directions
//! [`PatchKey::vertex_dir`] gives. **Each quad is split from its (0, 0) corner to its (1, 1)
//! corner** in patch index space: quad (x, y) is the triangles (x, y), (x + 1, y), (x + 1, y + 1)
//! and (x, y), (x + 1, y + 1), (x, y + 1). The parent level's mesh, whose vertices are this
//! patch's even ones, is split the same way.
//!
//! # What a vertex carries
//!
//! - its own level's height h₀ above the spheroid (Design note 5), and
//! - its morph target h₁: at a vertex both of whose indices are even, the parent level's height
//!   there; at any other vertex, the parent mesh's own linear interpolation of its even
//!   neighbours, on the parent mesh's diagonal: the mean of (x − 1, y) and (x + 1, y) where only x
//!   is odd, of (x, y − 1) and (x, y + 1) where only y is odd, and of (x − 1, y − 1) and
//!   (x + 1, y + 1) where both are, since the parent draws a chord there. At level 0 there is no
//!   parent, and h₁ = h₀.
//!
//! Positions sit on the spheroid datum, P = M·d + h·ν (Design note 5); the morph target's position
//! q₁ is the parent mesh's point, interpolated in the same way from the parent vertices'
//! positions. Both are formed in `f64` relative to the patch origin, the surface point of vertex
//! (32, 32) at its own height, and narrowed to `f32` only on output.
//!
//! Normals come from the height's analytic gradient, taken in the spheroid's tangent plane and
//! corrected by ρ ÷ (ρ + h), with ρ the spheroid's normal-section radius of curvature in the
//! gradient's direction (Euler's 1/ρ = cos²α ÷ M + sin²α ÷ N), and are stored body-fixed as
//! octahedral pairs ([`normals`]).
//!
//! Skirts hang from every edge of every patch, down along ν by the level's error bound plus the
//! `f32` step of the patch's largest |height| and the caller's margin; they never enter
//! collision.

pub mod collision;
pub mod normals;
pub mod vertex;

use crate::cube::{PATCH_QUADS, PatchKey};
use crate::num;
use crate::spheroid::Spheroid;
use crate::test_planet::HeightSample;

/// Vertices along a patch's side.
pub const PATCH_VERTICES: usize = 65;

/// What the bake, the collision interpolant and selection's bound read.
///
/// The test planet implements it here; R10 implements it over R09's synthesiser (R10 Design note
/// 4), so nothing in the bake names the test planet.
pub trait HeightSource {
    /// The caller's cache, one per bake.
    type Cache;
    /// Why a height could not be computed.
    type Error: std::error::Error + Send + Sync + 'static;

    /// The datum heights are measured from (Design note 5).
    fn figure(&self) -> Spheroid;

    /// The height at the unit direction `dir` (body-fixed) at `level`, and its gradient.
    ///
    /// # Errors
    ///
    /// The source's own.
    fn height(
        &self,
        cache: &mut Self::Cache,
        dir: [f64; 3],
        level: u8,
    ) -> Result<HeightSample, Self::Error>;

    /// The level's hard error bound against the finest level's mesh, metres (Design note 15).
    fn level_bound_m(&self, level: u8) -> f64;

    /// The lowest and highest heights the patch can reach, metres (Design note 8).
    fn height_range_m(&self, key: PatchKey) -> (f64, f64);

    /// Prepares `cache` for the queries of a bake of `key`; the default does nothing. A source's
    /// results never depend on it.
    fn prepare_cache(&self, cache: &mut Self::Cache, key: PatchKey) {
        let _ = (cache, key);
    }
}

/// How the bake forms vertex positions for the GPU (Design note 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexPath {
    /// The worker bakes each vertex's q₀ and q₁ relative to the patch origin, in `f32`.
    BakedOffsets,
    /// The vertex shader forms the position from face-coordinate differences and the heights.
    FaceDifferences,
}

/// The resolution of a patch's normals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NormalScale {
    /// One normal a vertex, 65 × 65.
    Mesh,
    /// Twice the mesh's resolution, 129 × 129 (Design note 25).
    Double,
}

impl NormalScale {
    /// The sample grid the normals are baked on.
    #[must_use]
    pub const fn grid(self) -> crate::cube::SampleGrid {
        match self {
            Self::Mesh => crate::cube::SampleGrid::Mesh,
            Self::Double => crate::cube::SampleGrid::Double,
        }
    }

    /// Normals along a side: 65 or 129.
    #[must_use]
    pub const fn side(self) -> usize {
        match self {
            Self::Mesh => PATCH_VERTICES,
            Self::Double => 2 * PATCH_VERTICES - 1,
        }
    }
}

/// What a bake makes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BakeOptions {
    /// The vertex path.
    pub vertex_path: VertexPath,
    /// The normals' resolution.
    pub normals: NormalScale,
    /// A margin added to every skirt's depth, metres, at least 0.
    pub skirt_m: f64,
}

/// A baked patch, the arrays the worker hands the GPU.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchBake {
    /// The patch.
    pub key: PatchKey,
    /// The patch origin, the surface point of vertex (32, 32) at its own height, body-fixed
    /// metres.
    pub origin: [f64; 3],
    /// The height of the origin, vertex (32, 32)'s own height, metres: the `FaceDifferences`
    /// path's h₀.
    pub origin_height_m: f64,
    /// Per vertex, row by row (y outer, x inner), its own height h₀ and morph target h₁, metres.
    pub heights: Vec<f32>,
    /// [`VertexPath::BakedOffsets`] only: per vertex, q₀ then q₁ relative to the origin, metres.
    pub offsets: Option<Vec<f32>>,
    /// Per normal sample, row by row, the octahedral pair of the unit normal, body-fixed.
    pub normals: Vec<f32>,
    /// The lowest and highest of the baked heights, both channels, metres.
    pub height_range_m: (f32, f32),
    /// The radius about the origin that holds every vertex, morph target and skirt, metres.
    pub bounding_radius_m: f64,
    /// How far the skirts hang below the edge vertices along ν, metres.
    pub skirt_depth_m: f64,
}

/// One vertex in `f64`: direction, own height, morph height, and the positions q₀ and q₁ relative
/// to the origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BakedVertex {
    pub(crate) dir: [f64; 3],
    pub(crate) h0: f64,
    pub(crate) h1: f64,
    pub(crate) q0: [f64; 3],
    pub(crate) q1: [f64; 3],
}

/// The vertices of `key` in `f64`, row by row: the bake's authoritative values before narrowing.
pub(crate) fn bake_vertices<S: HeightSource>(
    source: &S,
    key: PatchKey,
    cache: &mut S::Cache,
) -> Result<BakedVertices, S::Error> {
    let figure = source.figure();
    let quads = u8::try_from(PATCH_QUADS).expect("64 quads fit a u8");
    let centre = key.vertex_dir(quads / 2, quads / 2);
    let origin_height = num::assert_finite(source.height(cache, centre, key.level())?.height_m);
    let origin = figure.surface_point(centre, origin_height);
    let parent_level = key.level().checked_sub(1);
    let mut vertices = Vec::with_capacity(PATCH_VERTICES * PATCH_VERTICES);
    for y in 0..=quads {
        for x in 0..=quads {
            let dir = key.vertex_dir(x, y);
            let h0 = num::assert_finite(source.height(cache, dir, key.level())?.height_m);
            let p0 = figure.surface_point(dir, h0);
            let q0 = sub(p0, origin);
            vertices.push(BakedVertex {
                dir,
                h0,
                h1: h0,
                q0,
                q1: q0,
            });
        }
    }
    if let Some(parent) = parent_level {
        // The parent level's height and position at the even vertices first.
        let side = PATCH_VERTICES;
        let mut parent_h = vec![0.0; side * side];
        let mut parent_q = vec![[0.0; 3]; side * side];
        for y in (0..side).step_by(2) {
            for x in (0..side).step_by(2) {
                let v = &vertices[y * side + x];
                let h = num::assert_finite(source.height(cache, v.dir, parent)?.height_m);
                parent_h[y * side + x] = h;
                parent_q[y * side + x] = sub(figure.surface_point(v.dir, h), origin);
            }
        }
        for y in 0..side {
            for x in 0..side {
                let (a, b) = match (x % 2, y % 2) {
                    (0, 0) => (y * side + x, y * side + x),
                    (1, 0) => (y * side + x - 1, y * side + x + 1),
                    (0, _) => ((y - 1) * side + x, (y + 1) * side + x),
                    _ => ((y - 1) * side + x - 1, (y + 1) * side + x + 1),
                };
                let v = &mut vertices[y * side + x];
                if a == b {
                    v.h1 = parent_h[a];
                    v.q1 = parent_q[a];
                } else {
                    v.h1 = mean(parent_h[a], parent_h[b]);
                    v.q1 = [0, 1, 2].map(|n| mean(parent_q[a][n], parent_q[b][n]));
                }
            }
        }
    }
    Ok(BakedVertices {
        origin,
        origin_height,
        vertices,
    })
}

/// A patch's vertices in `f64` with the origin they are relative to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BakedVertices {
    /// The surface point of vertex (32, 32) at its own height, body-fixed metres.
    pub(crate) origin: [f64; 3],
    /// Vertex (32, 32)'s own height, metres.
    pub(crate) origin_height: f64,
    /// The vertices, row by row.
    pub(crate) vertices: Vec<BakedVertex>,
}

/// The parent mesh's chord at its midpoint, ½ (a + b), in that form: the sum then the halving,
/// which no value here can overflow, as the TypeScript side reproduces it.
#[expect(
    clippy::manual_midpoint,
    reason = "the exact operations are fixed for bit-for-bit agreement with the client"
)]
#[must_use]
fn mean(a: f64, b: f64) -> f64 {
    0.5 * (a + b)
}

/// `a − b`.
#[must_use]
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `|a|`.
#[must_use]
fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// The `f32` nearest `x`.
#[must_use]
fn narrow(x: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "narrowing to f32 is the bake's output format; every value is a finite height or \
                  offset far inside f32's range"
    )]
    let y = x as f32;
    y
}

/// The spacing of `f32` values at |x|, an upper bound on the narrowing step, metres.
#[must_use]
fn f32_step(x: f64) -> f64 {
    // 2⁻²³ of the magnitude bounds the spacing of f32 values at or below it (ulp ≤ |x| 2⁻²³).
    x.abs() * f64::from(f32::EPSILON)
}

/// Bakes `key` from `source` (see the module documentation).
///
/// `cache` is the caller's, one per bake; the bake is independent of what it holds.
///
/// # Errors
///
/// The first error `source` returns, unchanged.
///
/// # Panics
///
/// If a height is not finite (a bug in the source), or `opts.skirt_m` is negative or not finite.
pub fn bake_patch<S: HeightSource>(
    source: &S,
    key: PatchKey,
    opts: &BakeOptions,
    cache: &mut S::Cache,
) -> Result<PatchBake, S::Error> {
    assert!(
        opts.skirt_m.is_finite() && opts.skirt_m >= 0.0,
        "a skirt margin must be finite and non-negative, got {}",
        opts.skirt_m
    );
    source.prepare_cache(cache, key);
    let figure = source.figure();
    let BakedVertices {
        origin,
        origin_height,
        vertices,
    } = bake_vertices(source, key, cache)?;

    let mut heights = Vec::with_capacity(2 * vertices.len());
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    let mut largest_h = 0.0_f64;
    let mut radius = 0.0_f64;
    for v in &vertices {
        for h in [v.h0, v.h1] {
            let narrowed = narrow(h);
            heights.push(narrowed);
            low = if narrowed < low { narrowed } else { low };
            high = if narrowed > high { narrowed } else { high };
            largest_h = num::max(largest_h, h.abs());
        }
        radius = num::max(radius, num::max(norm(v.q0), norm(v.q1)));
    }

    let skirt_depth_m = source.level_bound_m(key.level()) + f32_step(largest_h) + opts.skirt_m;
    // The skirts' lowest points: every edge vertex lowered along ν by the skirt depth.
    let side = PATCH_VERTICES;
    for (n, v) in vertices.iter().enumerate() {
        let (x, y) = (n % side, n / side);
        if x == 0 || y == 0 || x == side - 1 || y == side - 1 {
            for h in [v.h0, v.h1] {
                let bottom = sub(figure.surface_point(v.dir, h - skirt_depth_m), origin);
                radius = num::max(radius, norm(bottom));
            }
        }
    }

    let offsets = match opts.vertex_path {
        VertexPath::BakedOffsets => Some(
            vertices
                .iter()
                .flat_map(|v| v.q0.into_iter().chain(v.q1))
                .map(narrow)
                .collect(),
        ),
        VertexPath::FaceDifferences => None,
    };
    let normals = normals::bake_normals(source, key, opts.normals, cache)?;

    Ok(PatchBake {
        key,
        origin,
        origin_height_m: origin_height,
        heights,
        offsets,
        normals,
        height_range_m: (low, high),
        bounding_radius_m: radius,
        skirt_depth_m,
    })
}

#[cfg(test)]
pub(crate) mod fixture;

#[cfg(test)]
mod tests {
    use super::fixture::{Fixture, Refused};
    use super::*;
    use crate::cube::{Edge, Face};
    use crate::noise::LatticeCache;
    use crate::test_planet::TEST_PLANET;
    use hyperion_testkit::float::bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    const OPTS: BakeOptions = BakeOptions {
        vertex_path: VertexPath::BakedOffsets,
        normals: NormalScale::Mesh,
        skirt_m: 0.0,
    };

    fn vertices<S: HeightSource>(source: &S, key: PatchKey, cache: &mut S::Cache) -> BakedVertices
    where
        S::Error: std::fmt::Debug,
    {
        bake_vertices(source, key, cache).unwrap()
    }

    /// The vertex indices along `edge`, in increasing x or y.
    fn edge_indices(edge: Edge) -> Vec<usize> {
        let last = PATCH_VERTICES - 1;
        (0..PATCH_VERTICES)
            .map(|n| match edge {
                Edge::UMin => n * PATCH_VERTICES,
                Edge::UMax => n * PATCH_VERTICES + last,
                Edge::VMin => n,
                Edge::VMax => last * PATCH_VERTICES + n,
            })
            .collect()
    }

    #[test]
    fn shared_edges_have_bitwise_equal_heights_across_faces_too() {
        let mut cache = LatticeCache::new();
        let keys = [
            PatchKey::new(Face::PosX, 9, 511, 200).unwrap(),
            PatchKey::new(Face::NegZ, 9, 0, 511).unwrap(),
            PatchKey::new(Face::PosY, 9, 100, 101).unwrap(),
        ];
        for key in keys {
            let ours = vertices(&TEST_PLANET, key, &mut cache).vertices;
            for edge in Edge::ALL {
                let (neighbour, back) = key.edge_neighbour_and_back(edge);
                let theirs = vertices(&TEST_PLANET, neighbour, &mut cache).vertices;
                let a: Vec<_> = edge_indices(edge)
                    .into_iter()
                    .map(|i| (ours[i].dir.map(bits), bits(ours[i].h0), bits(ours[i].h1)))
                    .collect();
                let mut b: Vec<_> = edge_indices(back)
                    .into_iter()
                    .map(|i| {
                        (
                            theirs[i].dir.map(bits),
                            bits(theirs[i].h0),
                            bits(theirs[i].h1),
                        )
                    })
                    .collect();
                if a[0].0 != b[0].0 {
                    b.reverse();
                }
                assert_eq!(a, b, "{key:?} {edge:?}");
            }
        }
    }

    #[test]
    fn at_morph_one_a_child_lies_on_its_parents_mesh() {
        let mut cache = LatticeCache::new();
        let parent = PatchKey::new(Face::NegX, 6, 17, 40).unwrap();
        let parent_baked = vertices(&TEST_PLANET, parent, &mut cache);
        let (parent_v, parent_origin) = (parent_baked.vertices, parent_baked.origin);
        let at = |x: usize, y: usize| {
            let v = &parent_v[y * PATCH_VERTICES + x];
            ([0, 1, 2].map(|n| v.q0[n] + parent_origin[n]), v.h0)
        };
        for (c, child) in parent.children().into_iter().enumerate() {
            let child_baked = vertices(&TEST_PLANET, child, &mut cache);
            let (child_v, origin) = (child_baked.vertices, child_baked.origin);
            let (ox, oy) = (32 * (c % 2), 32 * (c / 2));
            for y in 0..PATCH_VERTICES {
                for x in 0..PATCH_VERTICES {
                    let v = &child_v[y * PATCH_VERTICES + x];
                    let (px, py) = (ox + x / 2, oy + y / 2);
                    let (p, h) = match (x % 2, y % 2) {
                        (0, 0) => at(px, py),
                        (1, 0) => mid(at(px, py), at(px + 1, py)),
                        (0, _) => mid(at(px, py), at(px, py + 1)),
                        _ => mid(at(px, py), at(px + 1, py + 1)),
                    };
                    let q = [0, 1, 2].map(|n| v.q1[n] + origin[n]);
                    let gap = norm(sub(q, p));
                    // Both sides form the point from the same parent positions, relative to two
                    // origins: a few ulps of a 6.4 × 10⁶ m coordinate (about 10⁻⁹ m each).
                    assert!(
                        gap < 1e-8,
                        "child {c} ({x}, {y}): {gap} m off the parent's mesh"
                    );
                    if x % 2 == 0 && y % 2 == 0 {
                        assert_eq!(bits(v.h1), bits(h));
                    } else {
                        assert_eq!(bits(v.h1), bits(h), "child {c} ({x}, {y})");
                    }
                }
            }
        }
    }

    #[expect(
        clippy::manual_midpoint,
        reason = "the bake's own chord, ½ (a + b), in its exact operations"
    )]
    fn mid(a: ([f64; 3], f64), b: ([f64; 3], f64)) -> ([f64; 3], f64) {
        (
            [0, 1, 2].map(|n| 0.5 * (a.0[n] + b.0[n])),
            0.5 * (a.1 + b.1),
        )
    }

    #[test]
    fn decoded_normals_agree_with_the_gradients() {
        let mut cache = LatticeCache::new();
        let key = PatchKey::new(Face::PosZ, 12, 2_000, 3_000).unwrap();
        for scale in [NormalScale::Mesh, NormalScale::Double] {
            let opts = BakeOptions {
                normals: scale,
                ..OPTS
            };
            let bake = bake_patch(&TEST_PLANET, key, &opts, &mut cache).unwrap();
            let side = scale.side();
            assert_eq!(bake.normals.len(), 2 * side * side);
            let per = scale.grid().quads();
            for y in (0..=per).step_by(7) {
                for x in (0..=per).step_by(5) {
                    let dir = key.sample_dir(x, y, scale.grid());
                    let s = TEST_PLANET.height(dir, key.level(), &mut cache);
                    let expected =
                        normals::surface_normal(&TEST_PLANET.figure(), dir, s.height_m, s.gradient);
                    let n = usize::try_from(y).unwrap() * side + usize::try_from(x).unwrap();
                    let got = normals::decode_octahedral([
                        f64::from(bake.normals[2 * n]),
                        f64::from(bake.normals[2 * n + 1]),
                    ]);
                    let cos = got[0] * expected[0] + got[1] * expected[1] + got[2] * expected[2];
                    let angle = hyperion_base::math::acos(cos.min(1.0));
                    assert!(angle < 1e-4, "({x}, {y}) at {scale:?}: {angle} rad");
                }
            }
        }
    }

    #[test]
    fn octahedral_pairs_round_trip() {
        for n in [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [0.6, -0.8, 0.0],
            [-0.36, 0.48, -0.8],
            [1.0, 0.0, 0.0],
        ] {
            let back = normals::decode_octahedral(normals::encode_octahedral(n));
            for a in 0..3 {
                assert!((back[a] - n[a]).abs() < 1e-15, "{n:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn the_height_range_holds_every_baked_height_and_the_radius_every_point() {
        let mut cache = LatticeCache::new();
        for key in [
            PatchKey::new(Face::PosY, 3, 2, 5).unwrap(),
            PatchKey::new(Face::NegY, 15, 9_000, 77).unwrap(),
        ] {
            let bake = bake_patch(&TEST_PLANET, key, &OPTS, &mut cache).unwrap();
            let (low, high) = bake.height_range_m;
            assert!(bake.heights.iter().all(|h| low <= *h && *h <= high));
            let offsets = bake.offsets.as_ref().unwrap();
            for q in offsets.chunks(3) {
                let r = norm([0, 1, 2].map(|n| f64::from(q[n])));
                assert!(r <= bake.bounding_radius_m * (1.0 + 1e-6));
            }
            assert!(bake.skirt_depth_m >= TEST_PLANET.level_bound_m(key.level()));
            assert_eq!(bake.heights.len(), 2 * PATCH_VERTICES * PATCH_VERTICES);
            assert_eq!(offsets.len(), 6 * PATCH_VERTICES * PATCH_VERTICES);
        }
    }

    #[test]
    fn the_bake_is_independent_of_the_caches_fill_order() {
        let keys = [
            PatchKey::new(Face::NegZ, 10, 300, 301).unwrap(),
            PatchKey::new(Face::NegZ, 10, 301, 301).unwrap(),
            PatchKey::new(Face::NegZ, 11, 602, 603).unwrap(),
        ];
        let warm = std::cell::RefCell::new(LatticeCache::new());
        hyperion_testkit::order::assert_order_independent(&keys, |k| {
            bake_patch(&TEST_PLANET, *k, &OPTS, &mut warm.borrow_mut()).unwrap()
        });
        for k in &keys {
            assert_eq!(
                bake_patch(&TEST_PLANET, *k, &OPTS, &mut warm.borrow_mut()).unwrap(),
                bake_patch(&TEST_PLANET, *k, &OPTS, &mut LatticeCache::new()).unwrap()
            );
        }
    }

    #[test]
    fn a_constant_height_lies_on_its_shell() {
        for figure in [Spheroid::WGS84, Spheroid::sphere(6.371e6)] {
            let h = 1_234.5;
            let source = Fixture::constant(figure, h);
            for key in [
                PatchKey::new(Face::PosZ, 0, 0, 0).unwrap(),
                PatchKey::new(Face::NegX, 19, 123_456, 7_890).unwrap(),
            ] {
                let baked = vertices(&source, key, &mut ());
                let origin = baked.origin;
                for v in baked.vertices {
                    let p = [0, 1, 2].map(|n| v.q0[n] + origin[n]);
                    let shell = figure.surface_point(v.dir, h);
                    // The f64 rounding of q₀ and of adding the origin back: a few ulps of a.
                    let ulps = norm(sub(p, shell)) / (f64::EPSILON * figure.equatorial_radius_m);
                    assert!(ulps < 8.0, "{key:?}: {ulps} ulps off the shell");
                    assert_eq!(bits(v.h0), bits(h));
                }
            }
        }
    }

    #[test]
    fn a_sources_error_is_returned() {
        let mut source = Fixture::relief(Spheroid::WGS84, 50.0);
        source.refuse_z_above = Some(0.99);
        let key = PatchKey::root(Face::PosZ);
        assert_eq!(bake_patch(&source, key, &OPTS, &mut ()), Err(Refused));
        let fine = PatchKey::root(Face::NegZ);
        assert!(bake_patch(&source, fine, &OPTS, &mut ()).is_ok());
    }

    #[test]
    fn level_zero_has_no_morph_and_face_differences_bake_no_offsets() {
        let source = Fixture::relief(Spheroid::WGS84, 500.0);
        let key = PatchKey::root(Face::NegY);
        for v in vertices(&source, key, &mut ()).vertices {
            assert_eq!(bits(v.h0), bits(v.h1));
        }
        let opts = BakeOptions {
            vertex_path: VertexPath::FaceDifferences,
            ..OPTS
        };
        assert!(
            bake_patch(&source, key, &opts, &mut ())
                .unwrap()
                .offsets
                .is_none()
        );
    }
}
