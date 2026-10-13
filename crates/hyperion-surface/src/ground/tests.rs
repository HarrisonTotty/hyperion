//! The authoritative ground on the synthetic worlds and on analytic fields (plan R10, T3).

use std::cell::RefCell;
use std::collections::BTreeSet;

use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::cube::{
    Edge, Face, FaceUv, PATCH_QUADS, face_uv_to_xyz, st_to_uv, unit_dir, uv_to_st, xyz_to_face_uv,
};
use crate::field::{CoarseField, FieldView};
use crate::geometry::finest_level;
use crate::patch::collision::{Quad, finest_surface_height, quad_of};
use crate::patch::fixture::Fixture;
use crate::synth::level_bound_m;
use crate::synth::test_support::{
    Held, SEED, king_distances, point_in, random_cells, random_points_in, rim_cells, sample_bits,
    world,
};
use crate::testing::SyntheticWorld;

/// A world's synthesiser and its finest level.
fn synth_of(w: SyntheticWorld) -> (Synthesiser<'static, CoarseField>, u8) {
    let field = world(w);
    let finest = finest_level(field.header().figure().equatorial_radius_m);
    (Synthesiser::new(field, SEED), finest)
}

/// The height of R09's synthesis at `dir` at the finest level `finest`.
fn synth_height<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    dir: [f64; 3],
    finest: u8,
) -> f64 {
    synth
        .height_at(cache, dir, BandLevel::new(finest).unwrap())
        .unwrap()
        .height_m
}

/// `a · b`.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Points of `w` to sample the ground at: at random over the sphere and in finest patches at a
/// cube corner and on face edges, and in the cells under coarse craters' rims, with their
/// altitudes above the datum, in metres, from −500 to 5,000.
fn sampled_points(w: SyntheticWorld, count: usize, rng: &mut Lcg) -> Vec<[f64; 3]> {
    let field = world(w);
    let figure = field.header().figure();
    let finest = finest_level(figure.equatorial_radius_m);
    let mut dirs: Vec<[f64; 3]> = (0..count)
        .map(|_| unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0)))
        .collect();
    for cell in random_cells(finest, 3, rng) {
        dirs.extend(random_points_in(cell, 2, rng));
    }
    for cell in rim_cells(field, 3) {
        dirs.extend(random_points_in(cell, 1, rng));
    }
    dirs.into_iter()
        .map(|d| figure.surface_point(d, rng.next_f64() * 5_500.0 - 500.0))
        .collect()
}

#[test]
fn at_a_vertex_the_ground_is_height_at_there_bit_for_bit() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_0001);
    for &w in SyntheticWorld::ALL {
        let (synth, finest) = synth_of(w);
        let figure = HeightSource::figure(&synth);
        let mut cache = SynthCache::new();
        for key in random_cells(finest, 5, &mut rng) {
            let free = u8::try_from(rng.next_below(65)).unwrap();
            for (x, y) in [(0, 0), (64, 64), (64, 0), (0, 64), (32, free), (free, 7)] {
                let dir = key.vertex_dir(x, y);
                let expected = synth_height(&synth, &mut cache, dir, finest);
                for altitude in [-300.0, 0.0, 2_500.0] {
                    let p = figure.surface_point(dir, altitude);
                    let ground = ground_at(&synth, &mut cache, &p).unwrap();
                    assert_eq!(
                        bits(ground.height().value()),
                        bits(expected),
                        "{w:?} {key:?} vertex ({x}, {y}) at {altitude} m"
                    );
                    assert!((ground.point_height().value() - altitude).abs() < 1e-6);
                    let triangle = ground.triangle();
                    let corners = triangle
                        .vertices()
                        .map(|(a, b)| triangle.key().vertex_dir(a, b).map(bits));
                    assert!(corners.contains(&dir.map(bits)), "{w:?} {key:?} ({x}, {y})");
                }
            }
        }
    }
}

/// A plane in the finest lattice's coordinates of one face, h = 1,000 m + 0.05 m (S − S₀) +
/// 0.03 m (T − T₀), S and T the direction's lattice coordinates there and (S₀, T₀) a patch's
/// corner: the interpolant's own form, which it reproduces to rounding.
struct LatticePlane {
    figure: Spheroid,
    face: Face,
    corner: (f64, f64),
}

impl LatticePlane {
    /// The plane at the unit direction `dir`, on the plane's face.
    fn at(&self, dir: [f64; 3]) -> f64 {
        let on_face = xyz_to_face_uv(dir);
        assert_eq!(on_face.face, self.face, "the plane is defined on one face");
        let quads = f64::from(PATCH_QUADS << finest_level(self.figure.equatorial_radius_m));
        let (s, t) = (uv_to_st(on_face.u) * quads, uv_to_st(on_face.v) * quads);
        1_000.0 + 0.05 * (s - self.corner.0) + 0.03 * (t - self.corner.1)
    }
}

impl HeightSource for LatticePlane {
    type Cache = ();
    type Error = std::convert::Infallible;

    fn figure(&self) -> Spheroid {
        self.figure
    }

    fn height(
        &self,
        _cache: &mut (),
        dir: [f64; 3],
        _level: u8,
    ) -> Result<HeightSample, Self::Error> {
        Ok(HeightSample {
            height_m: self.at(dir),
            gradient: [0.0; 3],
        })
    }

    fn level_bound_m(&self, _: u8) -> f64 {
        0.0
    }

    fn height_range_m(&self, _: PatchKey) -> (f64, f64) {
        (0.0, 2_000.0)
    }
}

#[test]
fn on_an_analytic_plane_it_is_exact() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_0002);
    for figure in [Spheroid::WGS84, Spheroid::sphere(1.7374e6)] {
        let finest = finest_level(figure.equatorial_radius_m);
        for key in [
            PatchKey::new(Face::NegY, finest, 1_000, 77).unwrap(),
            PatchKey::new(Face::PosZ, finest, 4_321, 2_222).unwrap(),
        ] {
            let corner = (
                f64::from(key.i() * PATCH_QUADS),
                f64::from(key.j() * PATCH_QUADS),
            );
            let plane = LatticePlane {
                figure,
                face: key.face(),
                corner,
            };
            for dir in random_points_in(key, 200, &mut rng) {
                let p = figure.surface_point(dir, 1_000.0);
                let ground = ground_over(&plane, &mut (), &p).unwrap();
                let exact = plane.at(ground.dir());
                assert!(
                    (ground.height().value() - exact).abs() < 1e-9,
                    "{key:?}: {} against the plane's {exact}",
                    ground.height().value()
                );
            }
        }
        // A level ground, the datum shell at a height, is its height everywhere, bit for bit.
        let level = Fixture::constant(figure, 1_234.5);
        let mut points = sampled_points(SyntheticWorld::Flat, 40, &mut rng);
        points.push([0.0, 0.0, -2e6]);
        for p in points {
            let ground = ground_over(&level, &mut (), &p).unwrap();
            assert_eq!(bits(ground.height().value()), bits(1_234.5), "{p:?}");
        }
    }
}

/// The unit direction of lattice vertex (`ga`, `gb`) of `face` at `level`, as the interpolant
/// reads it: from the patch that holds it, a vertex on the face's far edge being the last
/// patch's 64.
fn lattice_vertex(face: Face, level: u8, ga: u64, gb: u64) -> [f64; 3] {
    let quads = u64::from(PATCH_QUADS) << level;
    let per = u64::from(PATCH_QUADS);
    let (i, j) = (ga.min(quads - 1) / per, gb.min(quads - 1) / per);
    let key = PatchKey::new(
        face,
        level,
        u32::try_from(i).unwrap(),
        u32::try_from(j).unwrap(),
    )
    .unwrap();
    key.vertex_dir(
        u8::try_from(ga - i * per).unwrap(),
        u8::try_from(gb - j * per).unwrap(),
    )
}

/// A lattice index as a coordinate.
fn lattice(n: u64) -> f64 {
    f64::from(u32::try_from(n).unwrap())
}

/// The unit direction at lattice coordinates (`s`, `t`) of `face` at `level`, in quads.
fn lattice_dir(face: Face, level: u8, s: f64, t: f64) -> [f64; 3] {
    let quads = f64::from(PATCH_QUADS << level);
    unit_dir(face_uv_to_xyz(FaceUv {
        face,
        u: st_to_uv(s / quads),
        v: st_to_uv(t / quads),
    }))
}

#[test]
fn points_on_a_shared_edge_get_the_same_height_from_either_triangle() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_0003);
    for &w in SyntheticWorld::ALL {
        let (synth, finest) = synth_of(w);
        let mut cache = SynthCache::new();
        let quads = u64::from(PATCH_QUADS) << finest;
        for p in sampled_points(w, 12, &mut rng) {
            let (dir, _) = datum_foot(&HeightSource::figure(&synth), &p);
            let (face, q) = quad_of(dir, finest);
            let (qa, qb) = (q.a(), q.b());
            // Keep clear of the face's far edges, where the quads beyond are another face's.
            if qa + 2 >= quads || qb + 2 >= quads {
                continue;
            }
            let mut h = |da: u64, db: u64| {
                synth_height(
                    &synth,
                    &mut cache,
                    lattice_vertex(face, finest, qa + da, qb + db),
                    finest,
                )
            };
            let (h00, h10, h01, h11) = (h(0, 0), h(1, 0), h(0, 1), h(1, 1));
            let (h21, h12) = (h(2, 1), h(1, 2));
            let quad =
                |da: u64, db: u64, fa: f64, fb: f64| Quad::new(qa + da, qb + db, fa, fb, fa < fb);
            for f in [0.125, 0.5, 0.875] {
                let lower = quad(0, 0, f, f).interpolate(h00, h11, h10);
                // The upper half's own expression at the same point.
                let upper = Quad::new(qa, qb, f, f, true).interpolate(h00, h11, h01);
                let left = quad(0, 0, 1.0, f).interpolate(h00, h11, h10);
                let right = quad(1, 0, 0.0, f).interpolate(h10, h21, h11);
                let below = quad(0, 0, f, 1.0).interpolate(h00, h11, h01);
                let above = quad(0, 1, f, 0.0).interpolate(h01, h12, h11);
                for (a, b, edge) in [
                    (lower, upper, "diagonal"),
                    (left, right, "right edge"),
                    (below, above, "top edge"),
                ] {
                    // The two expressions' rounding, a few ulps of a height of kilometres
                    // (about 10⁻¹² m each), well inside 10⁻⁹ m.
                    assert!((a - b).abs() < 1e-9, "{w:?} {edge} at {f}: {a} against {b}");
                }
            }
            // Through `ground_at`: either side of the quad's diagonal, 10⁻⁷ of a quad off it (the
            // lattice coordinate's own rounding is under 10⁻⁸ of a quad), the two triangles'
            // heights differ by the slope over that step, a few hundredths of a micrometre.
            let figure = HeightSource::figure(&synth);
            let side = (0..2).map(|k| {
                let (fa, fb) = if k == 0 {
                    (0.5 + 1e-7, 0.5)
                } else {
                    (0.5, 0.5 + 1e-7)
                };
                let dir = lattice_dir(face, finest, lattice(qa) + fa, lattice(qb) + fb);
                ground_at(&synth, &mut cache, &figure.point(dir)).unwrap()
            });
            let [low, high]: [GroundPoint; 2] = side.collect::<Vec<_>>().try_into().unwrap();
            assert!(!low.triangle().upper() && high.triangle().upper());
            assert!(
                (low.height().value() - high.height().value()).abs() < 1e-6,
                "{w:?}: {:?} against {:?} across the diagonal",
                low.height(),
                high.height()
            );
        }
    }
}

/// The largest height difference between points just either side of `w`'s cube-face edges, on
/// two faces, and between points just inside each face at its cube corners and the corner itself,
/// metres; each point a quarter of the interpolant's snap from the edge or corner, so that each is
/// read on its own face's lattice line or vertex.
fn seam_gap(w: SyntheticWorld, rng: &mut Lcg) -> f64 {
    let (synth, finest) = synth_of(w);
    let figure = HeightSource::figure(&synth);
    let mut cache = SynthCache::new();
    let last = (1_u32 << finest) - 1;
    // A quarter of the snap, 10⁻⁶ of a quad, in arc on the unit sphere: a quad's arc is at least
    // 0.94 ÷ (quads a side), at the middle of a face's edge.
    let eps = 0.25e-6 * 0.9 / f64::from(PATCH_QUADS << finest);
    let nudge = |from: [f64; 3], towards: [f64; 3], by: f64| {
        let step = [0, 1, 2].map(|k| towards[k] - from[k]);
        let len = dot(step, step).sqrt();
        unit_dir([0, 1, 2].map(|k| from[k] + by * step[k] / len))
    };
    let mut worst = 0.0_f64;
    for face in Face::ALL {
        for edge in Edge::ALL {
            let free = u32::try_from(rng.next_below(u64::from(last) + 1)).unwrap();
            let along = u8::try_from(rng.next_below(64)).unwrap();
            let (i, j, start, end, inward) = match edge {
                Edge::UMin => (0, free, (0, along), (0, along + 1), (1, along)),
                Edge::UMax => (last, free, (64, along), (64, along + 1), (63, along)),
                Edge::VMin => (free, 0, (along, 0), (along + 1, 0), (along, 1)),
                Edge::VMax => (free, last, (along, 64), (along + 1, 64), (along, 63)),
            };
            let key = PatchKey::new(face, finest, i, j).unwrap();
            let (start, end) = (
                key.vertex_dir(start.0, start.1),
                key.vertex_dir(end.0, end.1),
            );
            let mid = unit_dir([0, 1, 2].map(|k| start[k] + end[k]));
            let inside = key.vertex_dir(inward.0, inward.1);
            let near_in = nudge(mid, inside, eps);
            let near_out = nudge(mid, inside, -eps);
            let g_in = ground_at(&synth, &mut cache, &figure.point(near_in)).unwrap();
            let g_out = ground_at(&synth, &mut cache, &figure.point(near_out)).unwrap();
            assert_ne!(
                g_in.triangle().key().face(),
                g_out.triangle().key().face(),
                "{w:?} {face:?} {edge:?}: the two points are on one face"
            );
            worst = worst.max((g_in.height().value() - g_out.height().value()).abs());
        }
    }
    for corner in 0..8_u8 {
        let sign = |k: usize| if corner >> k & 1 == 1 { -1.0 } else { 1.0 };
        let c = unit_dir([sign(0), sign(1), sign(2)]);
        let at_corner = ground_at(&synth, &mut cache, &figure.point(c)).unwrap();
        let mut faces = BTreeSet::new();
        for axis in 0..3 {
            // Into the face of `axis`: that component grows and the other two shrink.
            let towards = [0, 1, 2].map(|k| {
                if k == axis {
                    c[k] + 2.0 * sign(k)
                } else {
                    c[k] - sign(k)
                }
            });
            let g = ground_at(&synth, &mut cache, &figure.point(nudge(c, towards, eps))).unwrap();
            faces.insert(g.triangle().key().face());
            worst = worst.max((g.height().value() - at_corner.height().value()).abs());
        }
        assert_eq!(faces.len(), 3, "{w:?} corner {corner}: {faces:?}");
    }
    worst
}

#[test]
fn either_side_of_a_cube_face_edge_and_at_a_cube_corner_it_agrees_to_a_micrometre() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_0004);
    for &w in SyntheticWorld::ALL {
        let gap = seam_gap(w, &mut rng);
        assert!(gap < 1e-6, "{w:?}: {gap} m across a seam");
    }
}

/// The surface points of `triangle`'s vertices over `synth` at the finest level `finest`.
fn triangle_corners<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    triangle: FinestTriangle,
    finest: u8,
) -> [[f64; 3]; 3] {
    let figure = HeightSource::figure(synth);
    triangle.vertices().map(|(x, y)| {
        let d = triangle.key().vertex_dir(x, y);
        figure.surface_point(d, synth_height(synth, cache, d, finest))
    })
}

#[test]
fn the_normal_is_the_triangle_s() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_0005);
    for &w in SyntheticWorld::ALL {
        let (synth, finest) = synth_of(w);
        let figure = HeightSource::figure(&synth);
        let mut cache = SynthCache::new();
        for p in sampled_points(w, 24, &mut rng) {
            let ground = ground_at(&synth, &mut cache, &p).unwrap();
            let [p0, p1, p2] = triangle_corners(&synth, &mut cache, ground.triangle(), finest);
            let n = ground.normal();
            assert!(
                (dot(n, n) - 1.0).abs() < 1e-15,
                "{w:?}: |n|² = {}",
                dot(n, n)
            );
            // Perpendicular to both edges, to the rounding of coordinates of thousands of
            // kilometres against an edge of a third of a metre.
            for e in [
                [0, 1, 2].map(|k| p1[k] - p0[k]),
                [0, 1, 2].map(|k| p2[k] - p0[k]),
            ] {
                let cos = dot(e, n) / dot(e, e).sqrt();
                assert!(
                    cos.abs() < 1e-8,
                    "{w:?}: an edge's cosine with the normal is {cos}"
                );
            }
            assert!(
                dot(n, figure.normal(ground.dir())) > 0.0,
                "{w:?}: the normal points into the body"
            );
        }
    }
}

/// The largest difference, metres, between the ground's face-coordinate interpolant and the
/// spatial one, where the datum's normal line through the point's foot meets the flat triangle
/// through the three vertices' surface points, at points of `w`.
fn spatial_gap(w: SyntheticWorld, count: usize, rng: &mut Lcg) -> f64 {
    let (synth, finest) = synth_of(w);
    let figure = HeightSource::figure(&synth);
    let mut cache = SynthCache::new();
    let mut worst = 0.0_f64;
    for p in sampled_points(w, count, rng) {
        let ground = ground_at(&synth, &mut cache, &p).unwrap();
        let [p0, _, _] = triangle_corners(&synth, &mut cache, ground.triangle(), finest);
        let n = ground.normal();
        let foot = figure.point(ground.dir());
        let nu = figure.normal(ground.dir());
        let to_plane = [0, 1, 2].map(|k| p0[k] - foot[k]);
        let spatial = dot(to_plane, n) / dot(nu, n);
        worst = worst.max((spatial - ground.height().value()).abs());
    }
    worst
}

#[test]
fn face_coordinate_and_spatial_interpolants_agree_to_a_micrometre() {
    // Design note 4: the chord's sagitta, D² ÷ 8R on a world's finest diagonal D, is about 5 nm on
    // an Earth and 18 nm at the Moon's radius; the rest is slope-weighted, the rounding of the
    // lattice coordinates and the warp's curvature.
    let mut rng = Lcg::new(0x6772_6f75_6e64_0006);
    for &w in SyntheticWorld::ALL {
        let gap = spatial_gap(w, 40, &mut rng);
        assert!(gap < 1e-6, "{w:?}: the interpolants differ by {gap} m");
    }
}

/// The `f32` nearest `x`, as the bake narrows a height for the GPU, back in `f64`.
fn narrowed(x: f64) -> f64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the bake's own narrowing of a finite height to the drawn f32"
    )]
    let y = x as f32;
    f64::from(y)
}

/// The largest difference, metres, between the ground and the drawn finest mesh, its vertices'
/// heights narrowed to `f32` as the bake narrows them and interpolated with the ground's own
/// weights, at points of three finest patches of `w`; and the largest of its ratio to the `f32`
/// step of each patch's largest |height|, |h| 2⁻²³ (R05's skirt term).
fn drawn_gap(w: SyntheticWorld, per_patch: usize, rng: &mut Lcg) -> (f64, f64) {
    let (synth, finest) = synth_of(w);
    let figure = HeightSource::figure(&synth);
    let mut cache = SynthCache::new();
    let (mut worst, mut worst_ratio) = (0.0_f64, 0.0_f64);
    for key in random_cells(finest, 3, rng) {
        let bake = synth.bake_patch(&mut cache, key).unwrap();
        let largest = bake
            .samples()
            .iter()
            .map(|s| s.height_m.abs())
            .fold(0.0, f64::max);
        let step = largest * f64::from(f32::EPSILON);
        for dir in random_points_in(key, per_patch, rng) {
            let ground = ground_at(&synth, &mut cache, &figure.point(dir)).unwrap();
            let triangle = ground.triangle();
            if triangle.key() != key {
                continue;
            }
            let (_, quad) = quad_of(ground.dir(), finest);
            let drawn_at = |x: u8, y: u8| narrowed(bake.sample(x, y).height_m);
            let (x, y) = (triangle.x(), triangle.y());
            let third = if triangle.upper() {
                drawn_at(x, y + 1)
            } else {
                drawn_at(x + 1, y)
            };
            let drawn = quad.interpolate(drawn_at(x, y), drawn_at(x + 1, y + 1), third);
            let gap = (drawn - ground.height().value()).abs();
            worst = worst.max(gap);
            if step > 0.0 {
                worst_ratio = worst_ratio.max(gap / step);
            }
        }
    }
    (worst, worst_ratio)
}

#[test]
fn the_drawn_f32_mesh_agrees_with_the_ground_to_half_its_f32_step() {
    // Each vertex rounds to nearest, half a step at most, and the interpolant's weights are a
    // convex combination of the three; the rest is f64 rounding.
    let mut rng = Lcg::new(0x6772_6f75_6e64_0007);
    for &w in SyntheticWorld::ALL {
        let (gap, ratio) = drawn_gap(w, 40, &mut rng);
        assert!(
            ratio <= 0.5 + 1e-6,
            "{w:?}: {gap} m, {ratio} of the f32 step"
        );
    }
}

#[test]
#[ignore = "measurement, run by hand with --nocapture: the agreement figures of R10.T3's record"]
fn the_agreement_figures() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_0008);
    for &w in SyntheticWorld::ALL {
        let (gap, ratio) = drawn_gap(w, 400, &mut rng);
        let spatial = spatial_gap(w, 400, &mut rng);
        let seam = seam_gap(w, &mut rng);
        println!(
            "{w:?}: drawn f32 mesh {gap:.3e} m ({ratio:.3} of the f32 step); \
             face against spatial {spatial:.3e} m; seams {seam:.3e} m"
        );
    }
}

#[test]
fn the_ground_is_finite_everywhere_sampled_and_refuses_cells_not_held() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_0009);
    for &w in SyntheticWorld::ALL {
        let (synth, _) = synth_of(w);
        let mut cache = SynthCache::new();
        for p in sampled_points(w, 60, &mut rng) {
            let g = ground_at(&synth, &mut cache, &p).unwrap();
            assert!(g.height().value().is_finite() && g.point_height().value().is_finite());
            assert!(g.normal().iter().chain(&g.dir()).all(|x| x.is_finite()));
        }
    }
    // A view of the cells within six king moves of one: a point in it answers as the whole field
    // does, and a point a quarter of the way round is refused, naming a cell the view lacks.
    let field = world(SyntheticWorld::MarsLike);
    let level = field.header().level().get();
    let centre_cell = PatchKey::new(Face::PosZ, level, 40, 70).unwrap();
    let held: BTreeSet<PatchKey> = king_distances(centre_cell, 6).into_keys().collect();
    let view = Held {
        inner: field,
        held: held.clone(),
    };
    let part = Synthesiser::new(&view, SEED);
    let whole = Synthesiser::new(field, SEED);
    let figure = field.header().figure();
    let inside = figure.point(point_in(centre_cell, 0.3, 0.6));
    let mut cache = SynthCache::new();
    assert_eq!(
        ground_at(&part, &mut cache, &inside).unwrap(),
        ground_at(&whole, &mut SynthCache::new(), &inside).unwrap()
    );
    let away = figure.point(point_in(
        PatchKey::new(Face::PosX, level, 3, 3).unwrap(),
        0.5,
        0.5,
    ));
    match ground_at(&part, &mut cache, &away) {
        Err(QueryHeightError::NotSurveyed(cell)) => assert!(!held.contains(&cell)),
        other => panic!("a point outside the view gave {other:?}"),
    }
}

#[test]
fn as_r05_s_height_source_the_synthesiser_forwards_r09_s_values() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_000a);
    for w in [SyntheticWorld::EarthLike, SyntheticWorld::MoonLike] {
        let (synth, finest) = synth_of(w);
        let header = synth.field().header();
        assert_eq!(HeightSource::figure(&synth), header.figure());
        let coarse = header.level().get();
        let mut cache = SynthCache::new();
        let cell = PatchKey::new(Face::NegX, 3, 2, 5).unwrap();
        for level in [0, coarse, coarse + 4, finest] {
            let band = BandLevel::new(level).unwrap();
            for dir in random_points_in(cell, 4, &mut rng) {
                let ours = HeightSource::height(&synth, &mut cache, dir, level).unwrap();
                let theirs = synth.height_at(&mut cache, dir, band).unwrap();
                assert_eq!(
                    sample_bits(ours),
                    sample_bits(theirs),
                    "{w:?} level {level}"
                );
            }
            let bound = HeightSource::level_bound_m(&synth, level);
            assert_eq!(bits(bound), bits(synth.level_bound_m(band)));
            assert_eq!(bits(bound), bits(level_bound_m(header, band)));
            let key = PatchKey::containing(level, [0.2, -0.7, 0.4]).unwrap();
            let (low, high) = HeightSource::height_range_m(&synth, key);
            let (l, h) = synth.height_range_m(key);
            assert_eq!((bits(low), bits(high)), (bits(l), bits(h)));
        }
        // The ground is R05's interpolant over this source at the point's foot, bit for bit.
        for p in sampled_points(w, 6, &mut rng) {
            let ground = ground_at(&synth, &mut cache, &p).unwrap();
            let h = finest_surface_height(&synth, ground.dir(), &mut cache).unwrap();
            assert_eq!(bits(ground.height().value()), bits(h));
        }
    }
}

#[test]
fn a_point_s_foot_on_the_datum_inverts_its_surface_point() {
    let mut rng = Lcg::new(0x6772_6f75_6e64_000b);
    // A body flattened by 0.2, beside an Earth's and a sphere: its evolute reaches 450 km from
    // the centre along the axis, inside every point sampled here.
    let flattened = Spheroid {
        equatorial_radius_m: 1.0e6,
        polar_radius_m: 0.8e6,
    };
    for figure in [Spheroid::WGS84, Spheroid::sphere(4.697e5), flattened] {
        let (a, c) = (figure.equatorial_radius_m, figure.polar_radius_m);
        let mut dirs: Vec<[f64; 3]> = (0..200)
            .map(|_| unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0)))
            .collect();
        dirs.extend([
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.6, -0.8, 0.0],
        ]);
        for d in dirs {
            for h in [-0.3 * c, -1e4, 0.0, 1e3, 1e5, a] {
                let p = figure.surface_point(d, h);
                let (foot, height) = datum_foot(&figure, &p);
                let gap = [0, 1, 2].map(|k| foot[k] - d[k]);
                assert!(
                    dot(gap, gap).sqrt() < 1e-14,
                    "{figure:?} {d:?} at {h} m: {foot:?}"
                );
                // The rounding of coordinates of up to 2a.
                let slack = 8.0 * f64::EPSILON * (a + h.abs());
                assert!(
                    (height - h).abs() < slack,
                    "{figure:?} {d:?}: {height} m for {h} m"
                );
            }
        }
    }
    // The geocentric direction is not the foot: at 45° on an Earth they part by about f ÷ 2.
    let earth = Spheroid::WGS84;
    let p = earth.surface_point(unit_dir([1.0, 0.0, 1.0]), 0.0);
    let (foot, _) = datum_foot(&earth, &p);
    let geocentric = unit_dir(p);
    let gap = [0, 1, 2].map(|k| foot[k] - geocentric[k]);
    assert!(
        dot(gap, gap).sqrt() > 1e-3,
        "{foot:?} against {geocentric:?}"
    );
}

/// The bits of everything `ground` holds.
fn ground_bits(ground: GroundPoint) -> ([u64; 8], FinestTriangle) {
    let [n0, n1, n2] = ground.normal().map(bits);
    let [d0, d1, d2] = ground.dir().map(bits);
    (
        [
            bits(ground.height().value()),
            n0,
            n1,
            n2,
            d0,
            d1,
            d2,
            bits(ground.point_height().value()),
        ],
        ground.triangle(),
    )
}

#[test]
fn the_ground_is_independent_of_the_cache_and_of_the_order_of_queries() {
    // Forwards, backwards and interleaved through one cache that every query fills, and each
    // query from an empty cache: the server and a client ask their points in different orders.
    let mut rng = Lcg::new(0x6772_6f75_6e64_000c);
    for w in [
        SyntheticWorld::EarthLike,
        SyntheticWorld::MarsLike,
        SyntheticWorld::OneCrater,
    ] {
        let (synth, _) = synth_of(w);
        let points = sampled_points(w, 10, &mut rng);
        let shared = RefCell::new(SynthCache::new());
        let through =
            |p: &[f64; 3]| ground_bits(ground_at(&synth, &mut shared.borrow_mut(), p).unwrap());
        assert_order_independent(&points, through);
        for p in &points {
            assert_eq!(
                through(p),
                ground_bits(ground_at(&synth, &mut SynthCache::new(), p).unwrap()),
                "{w:?} {p:?}"
            );
        }
    }
}
