//! The cube sphere: S2's six faces, its per-face (u, v) axes and its quadratic warp.
//!
//! A body's surface is addressed through a cube projected onto the sphere, as Google's S2 geometry
//! library does (`s2coords.h`), so that S2's published cell statistics apply unchanged (plan R05,
//! Design note 2). A direction maps to one face and a point (u, v) on it, with u and v in
//! [−1, 1]; the warp between (u, v) and the cell coordinates (s, t) in [0, 1] is S2's quadratic
//! projection, which evens the cells' areas to a largest-to-smallest ratio of about 2.09 (Design
//! note 3), against 5.2 for the unwarped cube.
//!
//! # Conventions
//!
//! - Faces 0 to 5 are +x, +y, +z, −x, −y, −z, with S2's axes: a face's point is
//!   face 0 (1, u, v), face 1 (−u, 1, v), face 2 (−u, −v, 1), face 3 (−1, −v, −u),
//!   face 4 (v, −1, −u), face 5 (v, u, −1) (`S2::FaceUVtoXYZ`).
//! - The warp is u = (4s² − 1) ÷ 3 for s ≥ ½ and (1 − 4(1 − s)²) ÷ 3 otherwise, whose inverse is
//!   s = ½√(1 + 3u) for u ≥ 0 and 1 − ½√(1 − 3u) otherwise (`S2_QUADRATIC_PROJECTION`'s
//!   `STtoUV` and `UVtoST`). S2 multiplies by the rounded `1.0 / 3`; this crate divides by 3,
//!   one correctly rounded operation, so the TypeScript mirror reproduces it bit for bit (Design
//!   note 1). Only `+ − × ÷` and `sqrt` appear, which IEEE 754 rounds exactly on every target.
//! - A point on a face edge or a cube corner belongs to the face of largest |axis|, ties going to
//!   the **lowest face index** (S2 sends ties to the highest axis instead), so that every direction
//!   has exactly one face and the rule agrees with the canonical face of [`PatchKey::vertex_dir`]
//!   (Design note 2).
//!
//! Sources: S2 Geometry, `src/s2/s2coords.h` (the face axes, `STtoUV`, `UVtoST`, `GetFace`,
//! `ValidFaceXYZtoUV`) and `src/s2/s2metrics.cc` (`kMinArea` = 8√2 ÷ 9 and `kMaxArea` = 2.635799,
//! for the quadratic projection), github.com/google/s2geometry.

/// One of the cube's six faces, in S2's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Face {
    /// Face 0, centred on +x.
    PosX,
    /// Face 1, centred on +y.
    PosY,
    /// Face 2, centred on +z.
    PosZ,
    /// Face 3, centred on −x.
    NegX,
    /// Face 4, centred on −y.
    NegY,
    /// Face 5, centred on −z.
    NegZ,
}

impl Face {
    /// The six faces in index order.
    pub const ALL: [Self; 6] = [
        Self::PosX,
        Self::PosY,
        Self::PosZ,
        Self::NegX,
        Self::NegY,
        Self::NegZ,
    ];

    /// The face's S2 index, 0 to 5.
    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Self::PosX => 0,
            Self::PosY => 1,
            Self::PosZ => 2,
            Self::NegX => 3,
            Self::NegY => 4,
            Self::NegZ => 5,
        }
    }

    /// The face of S2 index `index`, or `None` above 5.
    #[must_use]
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::PosX),
            1 => Some(Self::PosY),
            2 => Some(Self::PosZ),
            3 => Some(Self::NegX),
            4 => Some(Self::NegY),
            5 => Some(Self::NegZ),
            _ => None,
        }
    }
}

/// A point on a face: the face and its (u, v), each in [−1, 1] for a point on the cube.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceUv {
    /// The face.
    pub face: Face,
    /// The face's first coordinate, in [−1, 1].
    pub u: f64,
    /// The face's second coordinate, in [−1, 1].
    pub v: f64,
}

/// S2's quadratic warp from a cell coordinate s in [0, 1] to a face coordinate u in [−1, 1].
#[must_use]
pub fn st_to_uv(s: f64) -> f64 {
    if s >= 0.5 {
        (4.0 * s * s - 1.0) / 3.0
    } else {
        let r = 1.0 - s;
        (1.0 - 4.0 * r * r) / 3.0
    }
}

/// The inverse of [`st_to_uv`], from u in [−1, 1] back to s in [0, 1], with a square root alone.
#[must_use]
pub fn uv_to_st(u: f64) -> f64 {
    if u >= 0.0 {
        0.5 * (1.0 + 3.0 * u).sqrt()
    } else {
        1.0 - 0.5 * (1.0 - 3.0 * u).sqrt()
    }
}

/// The point (u, v) of `f`'s face on the cube of half-side 1, in body-fixed axes; not normalised
/// ([`unit_dir`] normalises).
#[must_use]
pub fn face_uv_to_xyz(f: FaceUv) -> [f64; 3] {
    let FaceUv { face, u, v } = f;
    match face {
        Face::PosX => [1.0, u, v],
        Face::PosY => [-u, 1.0, v],
        Face::PosZ => [-u, -v, 1.0],
        Face::NegX => [-1.0, -v, -u],
        Face::NegY => [v, -1.0, -u],
        Face::NegZ => [v, u, -1.0],
    }
}

/// The face of the direction `p` and its (u, v) there.
///
/// The face is that of `p`'s largest |component|, ties going to the lowest face index (see the
/// module documentation); `p` need not be normalised.
///
/// # Panics
///
/// If `p` is zero or has a component that is not finite: such a vector has no direction.
#[must_use]
pub fn xyz_to_face_uv(p: [f64; 3]) -> FaceUv {
    let face = face_of(p);
    let [x, y, z] = p;
    let (first, second) = match face {
        Face::PosX => (y / x, z / x),
        Face::PosY => (-x / y, z / y),
        Face::PosZ => (-x / z, -y / z),
        Face::NegX => (z / x, y / x),
        Face::NegY => (z / y, -x / y),
        Face::NegZ => (-y / z, -x / z),
    };
    FaceUv {
        face,
        u: first,
        v: second,
    }
}

/// The face of the direction `p`: its largest |component|, ties to the lowest face index.
///
/// # Panics
///
/// If `p` is zero or has a component that is not finite.
#[must_use]
#[expect(
    clippy::float_cmp,
    reason = "the tie rule needs exact equality: a magnitude ties the largest or it does not"
)]
pub fn face_of(p: [f64; 3]) -> Face {
    assert!(
        p.iter().all(|c| c.is_finite()) && p.iter().any(|&c| c != 0.0),
        "a direction must be finite and non-zero, got {p:?}"
    );
    let largest = p[0].abs().max(p[1].abs()).max(p[2].abs());
    // Among the axes that reach the largest magnitude, the face of lowest index. The axis's face
    // is its index for a positive component and its index plus 3 for a negative one (a component
    // of largest magnitude is never zero here).
    let mut best = u8::MAX;
    for (axis, &c) in (0_u8..).zip(p.iter()) {
        if c.abs() == largest {
            let face = if c > 0.0 { axis } else { axis + 3 };
            best = best.min(face);
        }
    }
    Face::from_index(best).expect("a face of largest magnitude exists for a non-zero direction")
}

/// `p` scaled to unit length.
///
/// # Panics
///
/// If `p` is zero or not finite, which has no direction.
#[must_use]
pub fn unit_dir(p: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = p;
    let norm = (x * x + y * y + z * z).sqrt();
    assert!(
        norm.is_finite() && norm > 0.0,
        "a direction must be finite and non-zero, got {p:?}"
    );
    [x / norm, y / norm, z / norm]
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_base::math;
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn ulps_apart(a: f64, b: f64) -> u64 {
        hyperion_testkit::float::ulps_apart(a, b)
    }

    #[test]
    fn the_warp_inverts_exactly_at_the_quarters_and_to_one_ulp_elsewhere() {
        for s in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_same_bits(uv_to_st(st_to_uv(s)), s);
        }
        assert_same_bits(st_to_uv(0.0), -1.0);
        assert_same_bits(st_to_uv(0.5), 0.0);
        assert_same_bits(st_to_uv(1.0), 1.0);
        let n = 10_000_u32;
        for i in 0..=n {
            let s = f64::from(i) / f64::from(n);
            let back = uv_to_st(st_to_uv(s));
            // Below ½ the warp works in 1 − s, so its one ulp is the spacing of 1 − s, 2⁻⁵³ in
            // [½, 1), not the far finer spacing of s itself near 0 (plan R05, T1.a as built).
            let within = if s >= 0.5 {
                ulps_apart(back, s) <= 1
            } else {
                (back - s).abs() <= f64::EPSILON / 2.0
            };
            assert!(within, "s = {s}, back = {back}");
        }
    }

    fn random_face_uv(rng: &mut Lcg, margin: f64) -> FaceUv {
        let face = Face::from_index(u8::try_from(rng.next_below(6)).unwrap()).unwrap();
        let span = 2.0 * (1.0 - margin);
        FaceUv {
            face,
            u: rng.next_f64() * span - (1.0 - margin),
            v: rng.next_f64() * span - (1.0 - margin),
        }
    }

    #[test]
    fn face_coordinates_round_trip_away_from_the_edges() {
        let mut rng = Lcg::new(0x5eed_c0be);
        for _ in 0..10_000 {
            let f = random_face_uv(&mut rng, 1e-6);
            let back = xyz_to_face_uv(face_uv_to_xyz(f));
            assert_eq!(back.face, f.face);
            assert!((back.u - f.u).abs() <= 1e-15, "{f:?} -> {back:?}");
            assert!((back.v - f.v).abs() <= 1e-15, "{f:?} -> {back:?}");
            // The scale of the vector does not matter.
            let p = face_uv_to_xyz(f);
            let unit = xyz_to_face_uv(unit_dir(p));
            assert_eq!(unit.face, f.face);
            assert!((unit.u - f.u).abs() <= 1e-15 && (unit.v - f.v).abs() <= 1e-15);
        }
    }

    #[test]
    fn every_face_centre_and_axis_is_s2s() {
        let centres = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, -1.0],
        ];
        for (face, centre) in Face::ALL.into_iter().zip(centres) {
            assert_eq!(Face::from_index(face.index()), Some(face));
            let xyz = face_uv_to_xyz(FaceUv {
                face,
                u: 0.0,
                v: 0.0,
            });
            // Equal as numbers: S2's negated axes give −0 where the centre has +0.
            for (got, want) in xyz.into_iter().zip(centre) {
                assert!((got - want).abs() <= 0.0, "{face:?}: {xyz:?}");
            }
            assert_eq!(face_of(centre), face);
        }
        assert_eq!(Face::from_index(6), None);
    }

    #[test]
    fn edges_and_corners_go_to_the_lowest_face_index() {
        // Each of the twelve edges' midpoints and the eight corners, by sign pattern.
        let signs = [-1.0, 0.0, 1.0];
        for &x in &signs {
            for &y in &signs {
                for &z in &signs {
                    let p = [x, y, z];
                    if p.iter().all(|&c| c == 0.0) {
                        continue;
                    }
                    let mut expected = u8::MAX;
                    for (axis, c) in (0_u8..).zip(p) {
                        if c != 0.0 {
                            expected = expected.min(if c > 0.0 { axis } else { axis + 3 });
                        }
                    }
                    let face = face_of(p);
                    assert_eq!(face.index(), expected, "{p:?}");
                    // The point lies on the face it is given to, with |u|, |v| ≤ 1.
                    let f = xyz_to_face_uv(p);
                    assert!(f.u.abs() <= 1.0 && f.v.abs() <= 1.0, "{p:?} -> {f:?}");
                }
            }
        }
    }

    #[test]
    fn every_direction_has_exactly_one_face() {
        let mut rng = Lcg::new(0xface);
        for _ in 0..10_000 {
            let p = [
                rng.next_f64() - 0.5,
                rng.next_f64() - 0.5,
                rng.next_f64() - 0.5,
            ];
            let f = xyz_to_face_uv(p);
            // The direction is on its face's square and maps back to itself.
            assert!(f.u.abs() <= 1.0 && f.v.abs() <= 1.0);
            let q = face_uv_to_xyz(f);
            let cross = cross(p, q);
            assert!(cross.iter().all(|c| c.abs() < 1e-15), "{p:?} -> {q:?}");
            assert!(dot(p, q) > 0.0);
            // No other face contains it in its interior.
            for other in Face::ALL {
                if other != f.face {
                    let a = face_uv_to_xyz(FaceUv {
                        face: other,
                        u: 0.0,
                        v: 0.0,
                    });
                    assert!(dot(p, a) <= p[0].abs().max(p[1].abs()).max(p[2].abs()));
                }
            }
        }
    }

    #[test]
    fn lines_of_constant_u_are_great_circles() {
        for face in Face::ALL {
            for i in 0..=8 {
                let fixed = f64::from(i) / 4.0 - 1.0;
                let normal_u = cross(
                    face_uv_to_xyz(FaceUv {
                        face,
                        u: fixed,
                        v: -0.9,
                    }),
                    face_uv_to_xyz(FaceUv {
                        face,
                        u: fixed,
                        v: 0.3,
                    }),
                );
                let normal_v = cross(
                    face_uv_to_xyz(FaceUv {
                        face,
                        u: -0.9,
                        v: fixed,
                    }),
                    face_uv_to_xyz(FaceUv {
                        face,
                        u: 0.3,
                        v: fixed,
                    }),
                );
                for j in 0..=10 {
                    let free = f64::from(j) / 5.0 - 1.0;
                    let on_u = unit_dir(face_uv_to_xyz(FaceUv {
                        face,
                        u: fixed,
                        v: free,
                    }));
                    assert!(dot(normal_u, on_u).abs() < 1e-15, "{face:?} u = {fixed}");
                    // And of constant v, through the centre too.
                    let on_v = unit_dir(face_uv_to_xyz(FaceUv {
                        face,
                        u: free,
                        v: fixed,
                    }));
                    assert!(dot(normal_v, on_v).abs() < 1e-15, "{face:?} v = {fixed}");
                }
            }
        }
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    /// The solid angle of the spherical triangle on unit vectors a, b, c, by Van Oosterom and
    /// Strackee (1983): tan(E ÷ 2) = |a · (b × c)| ÷ (1 + a·b + b·c + c·a).
    fn triangle_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
        let num = dot(a, cross(b, c)).abs();
        let den = 1.0 + dot(a, b) + dot(b, c) + dot(c, a);
        2.0 * math::atan2(num, den)
    }

    /// The exact area of cell (i, j) of face 0 at `level`, over two triangles.
    fn cell_area(level: u32, i: u32, j: u32) -> f64 {
        let n = f64::from(1_u32 << level);
        let corner = |a: u32, b: u32| {
            unit_dir(face_uv_to_xyz(FaceUv {
                face: Face::PosX,
                u: st_to_uv(f64::from(a) / n),
                v: st_to_uv(f64::from(b) / n),
            }))
        };
        let (p00, p10, p11, p01) = (
            corner(i, j),
            corner(i + 1, j),
            corner(i + 1, j + 1),
            corner(i, j + 1),
        );
        triangle_area(p00, p10, p11) + triangle_area(p00, p11, p01)
    }

    /// The largest and smallest cell areas of a level and where they lie, over one quadrant of
    /// one face, which the face's symmetries map onto all of it.
    fn area_extremes(level: u32) -> ((f64, u32, u32), (f64, u32, u32)) {
        let half = 1_u32 << (level - 1);
        let mut max = (0.0_f64, 0, 0);
        let mut min = (f64::INFINITY, 0, 0);
        for i in 0..half {
            for j in 0..half {
                let a = cell_area(level, i, j);
                if a > max.0 {
                    max = (a, i, j);
                }
                if a < min.0 {
                    min = (a, i, j);
                }
            }
        }
        (max, min)
    }

    #[test]
    fn the_cell_area_ratio_at_level_10_is_s2s() {
        let ((max, _, _), (min, mi, mj)) = area_extremes(10);
        let ratio = max / min;
        // Design note 3: 2.0917 at level 10, against 2.087 at 9 and 2.094 at 11.
        assert!(
            ((ratio - 2.0917) / 2.0917).abs() < 1e-4,
            "ratio at level 10 = {ratio}"
        );
        // Below S2's limit, kMaxArea ÷ kMinArea = 2.635799 ÷ (8√2 ÷ 9).
        let limit = 2.635_799 / (8.0 * 2.0_f64.sqrt() / 9.0);
        assert!(ratio < limit, "{ratio} ≥ {limit}");
        // The smallest cell is at an edge's midpoint: on the face's edge (i = 0 or j = 0 in this
        // quadrant, whose corner (0, 0) is the cube's) and next to its middle (index 511).
        let mid = (1_u32 << 9) - 1;
        assert!(
            (mi == 0 && mj == mid) || (mj == 0 && mi == mid),
            "smallest cell at ({mi}, {mj})"
        );
    }
}
