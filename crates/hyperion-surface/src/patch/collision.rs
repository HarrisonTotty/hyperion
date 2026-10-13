//! The collision interpolant: the surface the ground is, where anything touches it (plan R05,
//! Design note 6).
//!
//! It reads the finest level's vertex heights around the query direction, and only those, and
//! interpolates them on the mesh's own diagonal (quads split from (0, 0) to (1, 1); see
//! [`crate::patch`]), in `f64` throughout. Wherever the morph is held at zero, which is everywhere
//! near a grounded body, that is exactly the mesh drawn there, so collision and the picture agree
//! to the `f32` step of the drawn heights, about 2 mm at ±20 km.
//!
//! The interpolation weights are the query's position within its quad in the face's cell
//! coordinates (s, t), the lattice the vertices are placed on. The drawn triangle is flat between
//! the same three vertices, and the two differ by the warp's curvature across one quad, second
//! order in a spacing of a third of a metre: far below the `f32` step (T4.c as built).
//!
//! # The triangle
//!
//! `mesh_point` is the same interpolant with the triangle it read (plan R10, T3): the triangle's
//! patch, its quad there and the quad's half ([`MeshTriangle`]), and the unit normal of the flat
//! triangle through its three vertices' surface points, which R10's
//! [`ground_at`](crate::ground::ground_at) returns. [`mesh_height`] is its height, bit for bit,
//! and `finest_surface_point` and [`finest_surface_height`] are the two at the finest level; the
//! two triangle queries are the crate's, and `ground_at` is their public face.
//!
//! A query belongs to the quad whose lattice cell holds it, a query on the face's far edge to the
//! last quad, and to the quad's lower half where its weight across, `fa`, is at least its weight
//! up, `fb`; so a query on a quad's edge or its diagonal, or at a vertex, has one triangle, and
//! that triangle has the vertex among its three. Across a cube-face edge the query's face is
//! [`xyz_to_face_uv`]'s.

use super::HeightSource;
use crate::cube::{Face, PATCH_QUADS, PatchKey, unit_dir, uv_to_st, xyz_to_face_uv};
use crate::geometry::finest_level;
use crate::num::assert_finite;

/// A triangle of a level's mesh: the patch it lies in, its quad (`x`, `y`) there, each 0 to 63,
/// and the quad's half (see [`crate::patch`]'s split).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeshTriangle {
    key: PatchKey,
    x: u8,
    y: u8,
    upper: bool,
}

impl MeshTriangle {
    /// The patch the triangle lies in, at the mesh's level.
    #[must_use]
    pub const fn key(self) -> PatchKey {
        self.key
    }

    /// The quad's index across the patch, 0 to 63.
    #[must_use]
    pub const fn x(self) -> u8 {
        self.x
    }

    /// The quad's index up the patch, 0 to 63.
    #[must_use]
    pub const fn y(self) -> u8 {
        self.y
    }

    /// Whether it is the quad's upper half, above the diagonal in patch index space: vertices
    /// (x, y), (x + 1, y + 1) and (x, y + 1). The lower half is (x, y), (x + 1, y) and
    /// (x + 1, y + 1).
    #[must_use]
    pub const fn upper(self) -> bool {
        self.upper
    }

    /// Its three vertices' indices in the patch, each 0 to 64, in the order
    /// [`upper`](Self::upper) lists them: counter-clockwise seen from outside the body, since
    /// every face's (u, v) and its outward normal are right-handed.
    #[must_use]
    pub const fn vertices(self) -> [(u8, u8); 3] {
        let (x, y) = (self.x, self.y);
        if self.upper {
            [(x, y), (x + 1, y + 1), (x, y + 1)]
        } else {
            [(x, y), (x + 1, y), (x + 1, y + 1)]
        }
    }
}

/// A point of a level's mesh: its height, the triangle it lies in, and that triangle's normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MeshPoint {
    height_m: f64,
    triangle: MeshTriangle,
    normal: [f64; 3],
}

impl MeshPoint {
    /// The mesh's height there, metres above the spheroid along its normal: [`mesh_height`]'s.
    #[must_use]
    pub(crate) const fn height_m(&self) -> f64 {
        self.height_m
    }

    /// The triangle the point lies in.
    #[must_use]
    pub(crate) const fn triangle(&self) -> MeshTriangle {
        self.triangle
    }

    /// The outward unit normal, body-fixed, of the flat triangle through its three vertices'
    /// surface points, P = M·d + h·ν (Design note 5).
    #[must_use]
    pub(crate) const fn normal(&self) -> [f64; 3] {
        self.normal
    }
}

/// The height of the collision surface at the unit direction `dir` (body-fixed), metres above the
/// spheroid: the finest level's mesh, interpolated on its diagonal.
///
/// # Errors
///
/// The first error `source` returns, unchanged.
///
/// # Panics
///
/// If `dir` is zero or not finite.
pub fn finest_surface_height<S: HeightSource>(
    source: &S,
    dir: [f64; 3],
    cache: &mut S::Cache,
) -> Result<f64, S::Error> {
    let level = finest_level(source.figure().equatorial_radius_m);
    mesh_height(source, dir, level, cache)
}

/// The point of the collision surface at the unit direction `dir` (body-fixed): the finest level's
/// mesh there with the triangle it read, [`mesh_point`] at the finest level. Its height is
/// [`finest_surface_height`]'s, bit for bit.
///
/// # Errors
///
/// The first error `source` returns, unchanged.
///
/// # Panics
///
/// If `dir` is zero or not finite, or a height is not finite (a bug in the source).
pub(crate) fn finest_surface_point<S: HeightSource>(
    source: &S,
    dir: [f64; 3],
    cache: &mut S::Cache,
) -> Result<MeshPoint, S::Error> {
    let level = finest_level(source.figure().equatorial_radius_m);
    mesh_point(source, dir, level, cache)
}

/// The height of level `level`'s mesh at `dir`, interpolated on its diagonal as
/// [`finest_surface_height`] does the finest's: what the level bound's test measures a coarser
/// level against (T6).
///
/// At a vertex it reads that vertex alone, and elsewhere its triangle's three.
///
/// # Errors
///
/// The first error `source` returns, unchanged.
///
/// # Panics
///
/// If `dir` is zero or not finite, or `level` is above [`crate::cube::MAX_LEVEL`].
pub fn mesh_height<S: HeightSource>(
    source: &S,
    dir: [f64; 3],
    level: u8,
    cache: &mut S::Cache,
) -> Result<f64, S::Error> {
    let at = Located::new(dir, level);
    // At a vertex, its own height, bit for bit.
    if let (Lattice::On(ga), Lattice::On(gb)) = (at.s, at.t) {
        return Ok(assert_finite(at.vertex(source, cache, ga, gb)?.1));
    }
    let quad = at.quad();
    let h00 = assert_finite(at.vertex(source, cache, quad.a, quad.b)?.1);
    let h11 = assert_finite(at.vertex(source, cache, quad.a + 1, quad.b + 1)?.1);
    let (ga, gb) = quad.third();
    let third = assert_finite(at.vertex(source, cache, ga, gb)?.1);
    Ok(quad.interpolate(h00, h11, third))
}

/// The point of level `level`'s mesh at `dir`: [`mesh_height`]'s height, bit for bit, with the
/// triangle it lies in and that triangle's normal (see the [module documentation](self)).
///
/// It reads its triangle's three vertices, at a vertex too, in [`mesh_height`]'s order: (x, y),
/// then (x + 1, y + 1), then the third.
///
/// # Errors
///
/// The first error `source` returns, unchanged.
///
/// # Panics
///
/// If `dir` is zero or not finite, `level` is above [`crate::cube::MAX_LEVEL`], or a height is
/// not finite (a bug in the source).
pub(crate) fn mesh_point<S: HeightSource>(
    source: &S,
    dir: [f64; 3],
    level: u8,
    cache: &mut S::Cache,
) -> Result<MeshPoint, S::Error> {
    let at = Located::new(dir, level);
    let quad = at.quad();
    let (third_a, third_b) = quad.third();
    let (d00, h00) = at.vertex(source, cache, quad.a, quad.b)?;
    let (d11, h11) = at.vertex(source, cache, quad.a + 1, quad.b + 1)?;
    let (d_third, third) = at.vertex(source, cache, third_a, third_b)?;
    let (h00, h11, third) = (assert_finite(h00), assert_finite(h11), assert_finite(third));
    let height_m = match (at.s, at.t) {
        (Lattice::On(ga), Lattice::On(gb)) => {
            if (ga, gb) == (quad.a, quad.b) {
                h00
            } else if (ga, gb) == (quad.a + 1, quad.b + 1) {
                h11
            } else {
                assert_eq!(
                    (ga, gb),
                    (third_a, third_b),
                    "a vertex is a corner of the triangle the split gives it"
                );
                third
            }
        }
        _ => quad.interpolate(h00, h11, third),
    };
    // The triangle's vertices in its own order, counter-clockwise seen from outside.
    let corners = if quad.upper {
        [(d00, h00), (d11, h11), (d_third, third)]
    } else {
        [(d00, h00), (d_third, third), (d11, h11)]
    };
    let figure = source.figure();
    let [p0, p1, p2] = corners.map(|(d, h)| figure.surface_point(d, h));
    let e1 = [0, 1, 2].map(|k| p1[k] - p0[k]);
    let e2 = [0, 1, 2].map(|k| p2[k] - p0[k]);
    // The normal's form, these edges and this cross product in this order, is pinned output: R10's
    // ground goldens carry its bits.
    let normal = unit_dir([
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ]);
    let per = u64::from(PATCH_QUADS);
    let patch = |n: u64| u32::try_from(n / per).expect("a patch index fits a u32");
    let within = |n: u64| u8::try_from(n % per).expect("a quad index is below 64");
    let triangle = MeshTriangle {
        key: PatchKey::new(at.face, level, patch(quad.a), patch(quad.b))
            .expect("the patch is on the level's face"),
        x: within(quad.a),
        y: within(quad.b),
        upper: quad.upper,
    };
    Ok(MeshPoint {
        height_m,
        triangle,
        normal,
    })
}

/// A query's place on a level's lattice: its face and its two lattice coordinates, each snapped
/// onto a lattice line within [`SNAP_QUADS`] of one.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Located {
    face: Face,
    level: u8,
    quads: u64,
    s: Lattice,
    t: Lattice,
}

impl Located {
    /// The place of `dir` on `level`'s lattice.
    #[must_use]
    fn new(dir: [f64; 3], level: u8) -> Self {
        let on_face = xyz_to_face_uv(dir);
        let quads = u64::from(PATCH_QUADS) << level;
        #[expect(
            clippy::cast_precision_loss,
            reason = "the lattice has at most 2^30 quads a side, exact in f64"
        )]
        let side = quads as f64;
        Self {
            face: on_face.face,
            level,
            quads,
            s: snap(uv_to_st(on_face.u) * side),
            t: snap(uv_to_st(on_face.v) * side),
        }
    }

    /// The quad it falls in, its weights there and its half.
    #[must_use]
    fn quad(&self) -> Quad {
        let (a, fa) = split(self.s.coordinate(), self.quads);
        let (b, fb) = split(self.t.coordinate(), self.quads);
        Quad {
            a,
            b,
            fa,
            fb,
            upper: fa < fb,
        }
    }

    /// Lattice vertex (`ga`, `gb`)'s unit direction and its height at the level.
    fn vertex<S: HeightSource>(
        &self,
        source: &S,
        cache: &mut S::Cache,
        ga: u64,
        gb: u64,
    ) -> Result<([f64; 3], f64), S::Error> {
        // The vertex's patch and index; a vertex on the face's far edge is the last patch's 64.
        let per = u64::from(PATCH_QUADS);
        let (i, j) = (ga.min(self.quads - 1) / per, gb.min(self.quads - 1) / per);
        let key = PatchKey::new(
            self.face,
            self.level,
            u32::try_from(i).expect("a patch index fits a u32"),
            u32::try_from(j).expect("a patch index fits a u32"),
        )
        .expect("the patch is on the level's face");
        let dir = key.vertex_dir(
            u8::try_from(ga - i * per).expect("a vertex index is at most 64"),
            u8::try_from(gb - j * per).expect("a vertex index is at most 64"),
        );
        Ok((dir, source.height(cache, dir, self.level)?.height_m))
    }
}

/// A query's quad (`a`, `b`) on its face's lattice, its weights `fa` across and `fb` up the quad,
/// and the half they put it in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Quad {
    a: u64,
    b: u64,
    fa: f64,
    fb: f64,
    upper: bool,
}

impl Quad {
    /// Quad (`a`, `b`) with the weights `fa` and `fb` and the half `upper`, for the tests, which
    /// evaluate a half where the split would not put the point: on the diagonal, the upper half.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn new(a: u64, b: u64, fa: f64, fb: f64, upper: bool) -> Self {
        Self {
            a,
            b,
            fa,
            fb,
            upper,
        }
    }

    /// The quad's lattice index across the face.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn a(&self) -> u64 {
        self.a
    }

    /// The quad's lattice index up the face.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn b(&self) -> u64 {
        self.b
    }

    /// The triangle's vertex besides (a, b) and (a + 1, b + 1): (a, b + 1) in the upper half and
    /// (a + 1, b) in the lower.
    #[must_use]
    pub(crate) const fn third(&self) -> (u64, u64) {
        if self.upper {
            (self.a, self.b + 1)
        } else {
            (self.a + 1, self.b)
        }
    }

    /// The height on the triangle from its vertices' heights, `h00` at (a, b), `h11` at
    /// (a + 1, b + 1) and `third` at [`third`](Self::third), in R05's exact operations.
    ///
    /// # Panics
    ///
    /// If the height is not finite.
    #[must_use]
    pub(crate) fn interpolate(&self, h00: f64, h11: f64, third: f64) -> f64 {
        let (fa, fb) = (self.fa, self.fb);
        assert_finite(if self.upper {
            h00 + fb * (third - h00) + fa * (h11 - third)
        } else {
            h00 + fa * (third - h00) + fb * (h11 - third)
        })
    }
}

/// The face and quad of `dir` on `level`'s lattice, for the tests that hold the triangles'
/// interpolants to each other and to the drawn heights.
#[cfg(test)]
#[must_use]
pub(crate) fn quad_of(dir: [f64; 3], level: u8) -> (Face, Quad) {
    let at = Located::new(dir, level);
    (at.face, at.quad())
}

/// How close to a lattice line, in quads, a query is taken to be on it: a direction computed from
/// a vertex comes back through the face's division and the warp's square root a few ulps off its
/// exact lattice coordinate, and snapping it makes the query at a vertex return that vertex's
/// height bit for bit. It moves a query by at most 10⁻⁶ of a quad, a third of a micrometre at an
/// Earth's finest level.
const SNAP_QUADS: f64 = 1e-6;

/// A lattice coordinate: on a lattice line, by its index, or between two.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Lattice {
    /// On line n.
    On(u64),
    /// Strictly between two lines.
    Between(f64),
}

impl Lattice {
    /// The coordinate as a number.
    #[must_use]
    fn coordinate(self) -> f64 {
        match self {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a lattice index of at most 2^30 is exact in f64"
            )]
            Self::On(n) => n as f64,
            Self::Between(c) => c,
        }
    }
}

/// `c` (in [0, 2^30]) moved onto the nearest lattice line if it is within [`SNAP_QUADS`] of it.
#[must_use]
fn snap(c: f64) -> Lattice {
    let nearest = c.round();
    if (c - nearest).abs() <= SNAP_QUADS && nearest >= 0.0 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole, non-negative lattice coordinate of at most 2^30"
        )]
        let index = nearest as u64;
        Lattice::On(index)
    } else {
        Lattice::Between(c)
    }
}

/// The quad index of lattice coordinate `c` in [0, `quads`] and the fraction within it; the far
/// edge belongs to the last quad.
#[must_use]
fn split(c: f64, quads: u64) -> (u64, f64) {
    let floor = c.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "c is in [0, 2^30], so its floor is a small non-negative integer"
    )]
    let index = (floor.max(0.0) as u64).min(quads - 1);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a quad index below 2^30 is exact in f64"
    )]
    let fraction = c - index as f64;
    (index, fraction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube::{Edge, Face, face_uv_to_xyz, st_to_uv, unit_dir};
    use crate::noise::LatticeCache;
    use crate::patch::fixture::Fixture;
    use crate::patch::{
        BakeOptions, HeightSource, NormalScale, PATCH_VERTICES, VertexPath, bake_patch,
        bake_vertices,
    };
    use crate::spheroid::Spheroid;
    use crate::test_planet::TEST_PLANET;
    use hyperion_testkit::float::bits;
    use hyperion_testkit::lcg::Lcg;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// 20 finest-level (19) patches over the six faces, their edges and corners.
    fn finest_patches() -> Vec<PatchKey> {
        let last = (1_u32 << 19) - 1;
        let mut rng = Lcg::new(0x636f_6c6c);
        (0..20_u8)
            .map(|n| {
                let face = Face::from_index(n % 6).unwrap();
                let mut free = || u32::try_from(rng.next_below(u64::from(last) + 1)).unwrap();
                let (i, j) = match n % 4 {
                    0 => (0, 0),
                    1 => (last, free()),
                    2 => (free(), last),
                    _ => (free(), free()),
                };
                PatchKey::new(face, 19, i, j).unwrap()
            })
            .collect()
    }

    #[test]
    fn at_every_vertex_it_is_the_bakes_height() {
        let source = Fixture::relief(Spheroid::WGS84, 3_000.0);
        let opts = BakeOptions {
            vertex_path: VertexPath::FaceDifferences,
            normals: NormalScale::Mesh,
            skirt_m: 0.0,
        };
        for key in finest_patches() {
            let vertices = bake_vertices(&source, key, &mut ()).unwrap().vertices;
            let bake = bake_patch(&source, key, &opts, &mut ()).unwrap();
            for (n, v) in vertices.iter().enumerate() {
                let h = finest_surface_height(&source, v.dir, &mut ()).unwrap();
                assert_eq!(bits(h), bits(v.h0), "{key:?} vertex {n}");
                // The drawn f32 height is within its own f32 step.
                let drawn = f64::from(bake.heights[2 * n]);
                assert!((drawn - h).abs() <= h.abs() * f64::from(f32::EPSILON));
            }
        }
    }

    #[test]
    fn the_test_planets_vertices_agree_too() {
        let mut cache = LatticeCache::new();
        let key = PatchKey::new(Face::PosX, 19, 262_144, 12_345).unwrap();
        let vertices = bake_vertices(&TEST_PLANET, key, &mut cache)
            .unwrap()
            .vertices;
        for v in vertices.iter().step_by(97) {
            let h = finest_surface_height(&TEST_PLANET, v.dir, &mut cache).unwrap();
            assert_eq!(bits(h), bits(v.h0));
        }
    }

    /// The direction at lattice coordinates (`a`, `b`) of face 0's finest level.
    fn lattice_point(a: f64, b: f64) -> [f64; 3] {
        let quads = f64::from(64_u32 << 19);
        unit_dir(face_uv_to_xyz(crate::cube::FaceUv {
            face: Face::PosX,
            u: st_to_uv(a / quads),
            v: st_to_uv(b / quads),
        }))
    }

    #[test]
    fn inside_a_quad_it_interpolates_on_the_stated_diagonal() {
        let source = Fixture::relief(Spheroid::WGS84, 3_000.0);
        let height =
            |a: f64, b: f64| finest_surface_height(&source, lattice_point(a, b), &mut ()).unwrap();
        let (a0, b0) = (1_000_000.0, 2_000_000.0);
        let (h00, h10, h01, h11) = (
            height(a0, b0),
            height(a0 + 1.0, b0),
            height(a0, b0 + 1.0),
            height(a0 + 1.0, b0 + 1.0),
        );
        for (fa, fb) in [
            (0.25, 0.125),
            (0.75, 0.5),
            (0.125, 0.5),
            (0.5, 0.875),
            (0.5, 0.5),
        ] {
            let got = height(a0 + fa, b0 + fb);
            let expected = if fa >= fb {
                h00 + fa * (h10 - h00) + fb * (h11 - h10)
            } else {
                h00 + fb * (h01 - h00) + fa * (h11 - h01)
            };
            assert!(
                (got - expected).abs() < 1e-9,
                "({fa}, {fb}): {got} vs {expected}"
            );
        }
        // Off the (0, 0)–(1, 1) diagonal the other split would differ: the relief is not planar.
        let other = h00 + 0.25 * (h10 - h00) + 0.75 * (h01 - h00);
        assert!((height(a0 + 0.25, b0 + 0.75) - other).abs() > 0.0);
    }

    #[test]
    fn on_a_shared_edge_it_is_the_same_from_either_patch() {
        // Along a face edge, approached from both faces' sides: the surface is continuous across
        // it, to the f64 rounding of the two faces' interpolation weights.
        let mut cache = LatticeCache::new();
        let key = PatchKey::new(Face::PosX, 19, (1 << 19) - 1, 4_000).unwrap();
        let (neighbour, _) = key.edge_neighbour_and_back(Edge::UMax);
        assert_ne!(neighbour.face(), key.face());
        let on_edge = key.vertex_dir(64, 17);
        let next = key.vertex_dir(64, 18);
        let mid = unit_dir([0, 1, 2].map(|n| on_edge[n] + next[n]));
        let inward = key.vertex_dir(63, 17);
        let outward = neighbour.vertex_dir(1, 17);
        let h = finest_surface_height(&TEST_PLANET, mid, &mut cache).unwrap();
        for side in [inward, outward] {
            // A step of 10⁻⁹ of the way towards a vertex off the edge on each side.
            let near = unit_dir([0, 1, 2].map(|n| mid[n] + 1e-9 * (side[n] - mid[n])));
            let hn = finest_surface_height(&TEST_PLANET, near, &mut cache).unwrap();
            assert!(
                (hn - h).abs() < 1e-3,
                "{hn} against {h} across the face edge"
            );
        }
        let a = finest_surface_height(&TEST_PLANET, on_edge, &mut cache).unwrap();
        let vertices = bake_vertices(&TEST_PLANET, neighbour, &mut cache)
            .unwrap()
            .vertices;
        let shared = vertices
            .iter()
            .find(|v| v.dir.map(bits) == on_edge.map(bits))
            .expect("the neighbour shares the edge vertex");
        assert_eq!(bits(a), bits(shared.h0));
        assert_eq!(PATCH_VERTICES, 65);
    }

    #[test]
    fn it_reads_only_the_finest_level() {
        let source = Fixture::relief(Spheroid::WGS84, 10.0);
        let mut rng = Lcg::new(0x6c76_6c73);
        for _ in 0..200 {
            let p = [0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0);
            let _ = finest_surface_height(&source, p, &mut ()).unwrap();
        }
        assert!(source.levels.borrow().iter().all(|&l| l == 19));
        // A smaller body's finest level is its own.
        let moon = Fixture::relief(Spheroid::sphere(1.7374e6), 10.0);
        let _ = finest_surface_height(&moon, [0.0, 0.6, 0.8], &mut ()).unwrap();
        assert!(moon.levels.borrow().iter().all(|&l| l == 17));
    }

    /// Directions over every face, at random, on lattice lines, at vertices, on face edges and at
    /// cube corners, of a body whose finest level is 19.
    fn sampled_directions() -> Vec<[f64; 3]> {
        let mut rng = Lcg::new(0x7472_6961);
        let mut out: Vec<[f64; 3]> = (0..200)
            .map(|_| [0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0))
            .collect();
        for key in finest_patches() {
            for (x, y) in [(0, 0), (64, 64), (64, 0), (0, 64), (17, 43), (64, 31)] {
                out.push(key.vertex_dir(x, y));
            }
            // On a lattice line between two vertices, and on a quad's diagonal.
            let (a, b) = (key.vertex_dir(5, 9), key.vertex_dir(5, 10));
            out.push(unit_dir([0, 1, 2].map(|n| a[n] + b[n])));
            let c = key.vertex_dir(6, 10);
            out.push(unit_dir([0, 1, 2].map(|n| a[n] + c[n])));
        }
        // A cube corner and a face edge's midpoint, as unit vectors, as their vertices are.
        out.extend([[1.0, 1.0, 1.0], [-1.0, 1.0, -1.0], [1.0, 0.0, 1.0]].map(unit_dir));
        out
    }

    #[test]
    fn the_triangle_query_s_height_is_mesh_height_s_bit_for_bit() {
        let fixture = Fixture::relief(Spheroid::WGS84, 3_000.0);
        let mut cache = LatticeCache::new();
        for dir in sampled_directions() {
            let point = finest_surface_point(&fixture, dir, &mut ()).unwrap();
            let h = finest_surface_height(&fixture, dir, &mut ()).unwrap();
            assert_eq!(bits(point.height_m()), bits(h), "{dir:?}");
            let point = mesh_point(&TEST_PLANET, dir, 14, &mut cache).unwrap();
            let h = mesh_height(&TEST_PLANET, dir, 14, &mut cache).unwrap();
            assert_eq!(bits(point.height_m()), bits(h), "{dir:?} at level 14");
        }
    }

    #[test]
    fn the_triangle_holds_the_query_and_a_vertex_is_one_of_its_corners() {
        let fixture = Fixture::relief(Spheroid::WGS84, 3_000.0);
        let quads = f64::from(64_u32 << 19);
        for dir in sampled_directions() {
            let triangle = finest_surface_point(&fixture, dir, &mut ())
                .unwrap()
                .triangle();
            let key = triangle.key();
            assert_eq!(key.level(), 19);
            let on_face = xyz_to_face_uv(dir);
            assert_eq!(key.face(), on_face.face, "{dir:?}");
            // The query's lattice coordinates relative to the quad's corner, within a snap of it.
            let s = uv_to_st(on_face.u) * quads - f64::from(key.i() * 64 + u32::from(triangle.x()));
            let t = uv_to_st(on_face.v) * quads - f64::from(key.j() * 64 + u32::from(triangle.y()));
            let slack = 2.0 * SNAP_QUADS;
            assert!(
                (-slack..=1.0 + slack).contains(&s) && (-slack..=1.0 + slack).contains(&t),
                "{dir:?}: ({s}, {t}) is outside quad {triangle:?}"
            );
            assert!(
                if triangle.upper() {
                    t >= s - slack
                } else {
                    s >= t - slack
                },
                "{dir:?}: ({s}, {t}) is in the other half of {triangle:?}"
            );
            // At a vertex, the vertex is one of the triangle's three.
            let corners = triangle
                .vertices()
                .map(|(x, y)| key.vertex_dir(x, y).map(bits));
            if let Some(vertex) = sampled_vertex(dir) {
                assert!(
                    corners.contains(&vertex.map(bits)),
                    "{dir:?} in {triangle:?}"
                );
            }
        }
    }

    /// `dir` if it is one of [`sampled_directions`]'s vertices: the lattice coordinates of a
    /// direction computed from a vertex are whole to within the snap.
    fn sampled_vertex(dir: [f64; 3]) -> Option<[f64; 3]> {
        let quads = f64::from(64_u32 << 19);
        let on_face = xyz_to_face_uv(dir);
        let (s, t) = (uv_to_st(on_face.u) * quads, uv_to_st(on_face.v) * quads);
        ((s - s.round()).abs() <= SNAP_QUADS && (t - t.round()).abs() <= SNAP_QUADS).then_some(dir)
    }

    #[test]
    fn the_normal_is_the_triangle_s_outward_unit_normal() {
        let fixture = Fixture::relief(Spheroid::WGS84, 3_000.0);
        let figure = fixture.figure;
        for dir in sampled_directions() {
            let point = finest_surface_point(&fixture, dir, &mut ()).unwrap();
            let n = point.normal();
            let triangle = point.triangle();
            let corners = triangle.vertices().map(|(x, y)| {
                let d = triangle.key().vertex_dir(x, y);
                figure.surface_point(d, fixture.height(&mut (), d, 19).unwrap().height_m)
            });
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-15, "{dir:?}: |n| = {len}");
            // Perpendicular to both edges: their cosines with it are at the rounding of a
            // coordinate of 6.4 × 10⁶ m against an edge of a third of a metre.
            for k in [1, 2] {
                let e = [0, 1, 2].map(|a| corners[k][a] - corners[0][a]);
                let e_len = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
                let cos = (e[0] * n[0] + e[1] * n[1] + e[2] * n[2]) / e_len;
                assert!(cos.abs() < 1e-8, "{dir:?}: edge {k}'s cosine {cos}");
            }
            let nu = figure.normal(unit_dir(dir));
            assert!(
                n[0] * nu[0] + n[1] * nu[1] + n[2] * nu[2] > 0.99,
                "{dir:?}: the normal points inwards or is far from the datum's"
            );
        }
    }

    #[test]
    fn either_triangle_gives_a_shared_edge_the_same_height() {
        // Each pair of triangles that share an edge, both interpolants evaluated on it: the
        // diagonal of quad (a, b), its right edge with quad (a + 1, b)'s left, and its top edge
        // with quad (a, b + 1)'s bottom, across a patch's edge too (a = 63 + 64 n).
        let fixture = Fixture::relief(Spheroid::WGS84, 3_000.0);
        let height = |a: u32, b: u32| -> f64 {
            fixture
                .height(&mut (), lattice_point(f64::from(a), f64::from(b)), 19)
                .unwrap()
                .height_m
        };
        for (a0, b0) in [
            (1_000_000_u32, 2_000_000_u32),
            (127, 4_095),
            (64 * 3 - 1, 9),
        ] {
            let h = |da: u32, db: u32| height(a0 + da, b0 + db);
            let quad = |da: u32, db: u32, fa: f64, fb: f64| Quad {
                a: u64::from(a0 + da),
                b: u64::from(b0 + db),
                fa,
                fb,
                upper: fa < fb,
            };
            for f in [0.125, 0.5, 0.875] {
                // The diagonal: the lower half at fa = fb, and the upper half's own expression.
                let lower = quad(0, 0, f, f).interpolate(h(0, 0), h(1, 1), h(1, 0));
                let upper = Quad {
                    upper: true,
                    ..quad(0, 0, f, f)
                }
                .interpolate(h(0, 0), h(1, 1), h(0, 1));
                assert_close(lower, upper, "the diagonal");
                // The right edge: quad (a, b)'s lower half at fa = 1, quad (a + 1, b)'s upper at 0.
                let left = quad(0, 0, 1.0, f).interpolate(h(0, 0), h(1, 1), h(1, 0));
                let right = quad(1, 0, 0.0, f).interpolate(h(1, 0), h(2, 1), h(1, 1));
                assert_close(left, right, "the right edge");
                // The top edge: quad (a, b)'s upper half at fb = 1, quad (a, b + 1)'s lower at 0.
                let below = quad(0, 0, f, 1.0).interpolate(h(0, 0), h(1, 1), h(0, 1));
                let above = quad(0, 1, f, 0.0).interpolate(h(0, 1), h(1, 2), h(1, 1));
                assert_close(below, above, "the top edge");
            }
        }
    }

    /// Two interpolants of one point agree to a few ulps of a height of kilometres.
    fn assert_close(a: f64, b: f64, what: &str) {
        assert!((a - b).abs() <= 1e-11, "{what}: {a} against {b}");
    }
}
