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

use super::HeightSource;
use crate::cube::{PATCH_QUADS, PatchKey, uv_to_st, xyz_to_face_uv};
use crate::geometry::finest_level;
use crate::num::assert_finite;

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

/// The height of level `level`'s mesh at `dir`, interpolated on its diagonal as
/// [`finest_surface_height`] does the finest's: what the level bound's test measures a coarser
/// level against (T6).
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
    let on_face = xyz_to_face_uv(dir);
    let quads = u64::from(PATCH_QUADS) << level;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the lattice has at most 2^30 quads a side, exact in f64"
    )]
    let side = quads as f64;
    let (s, t) = (
        snap(uv_to_st(on_face.u) * side),
        snap(uv_to_st(on_face.v) * side),
    );
    let height_at = |cache: &mut S::Cache, ga: u64, gb: u64| -> Result<f64, S::Error> {
        // The vertex's patch and index; a vertex on the face's far edge is the last patch's 64.
        let per = u64::from(PATCH_QUADS);
        let (i, j) = (ga.min(quads - 1) / per, gb.min(quads - 1) / per);
        let key = PatchKey::new(
            on_face.face,
            level,
            u32::try_from(i).expect("a patch index fits a u32"),
            u32::try_from(j).expect("a patch index fits a u32"),
        )
        .expect("the patch is on the finest level's face");
        let dir = key.vertex_dir(
            u8::try_from(ga - i * per).expect("a vertex index is at most 64"),
            u8::try_from(gb - j * per).expect("a vertex index is at most 64"),
        );
        Ok(source.height(cache, dir, level)?.height_m)
    };
    // At a vertex, its own height, bit for bit.
    if let (Lattice::On(ga), Lattice::On(gb)) = (s, t) {
        return height_at(cache, ga, gb).map(assert_finite);
    }
    let (a, fa) = split(s.coordinate(), quads);
    let (b, fb) = split(t.coordinate(), quads);
    let h00 = assert_finite(height_at(cache, a, b)?);
    let h11 = assert_finite(height_at(cache, a + 1, b + 1)?);
    Ok(assert_finite(if fa >= fb {
        let h10 = assert_finite(height_at(cache, a + 1, b)?);
        h00 + fa * (h10 - h00) + fb * (h11 - h10)
    } else {
        let h01 = assert_finite(height_at(cache, a, b + 1)?);
        h00 + fb * (h01 - h00) + fa * (h11 - h01)
    }))
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
        BakeOptions, NormalScale, PATCH_VERTICES, VertexPath, bake_patch, bake_vertices,
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
        // A direction on a face edge, reached from both faces' sides of it, and one on a patch
        // edge inside a face.
        let mut cache = LatticeCache::new();
        let key = PatchKey::new(Face::PosX, 19, (1 << 19) - 1, 4_000).unwrap();
        let (neighbour, _) = key.edge_neighbour_and_back(Edge::UMax);
        assert_ne!(neighbour.face(), key.face());
        let on_edge = key.vertex_dir(64, 17);
        let mid = unit_dir([0, 1, 2].map(|n| on_edge[n] + key.vertex_dir(64, 18)[n]));
        let h = finest_surface_height(&TEST_PLANET, mid, &mut cache).unwrap();
        let flipped =
            finest_surface_height(&TEST_PLANET, mid.map(|c| c * 3.0), &mut cache).unwrap();
        assert_eq!(bits(h), bits(flipped));
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
}
