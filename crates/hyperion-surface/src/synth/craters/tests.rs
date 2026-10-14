//! The small craters' tests (plan R09, T7.b): the octaves and their levels, the density capped at
//! saturation and the ages' windows, the cells' true areas and the placement, the realised
//! size–frequency, the 3 × 3 search across face edges, the profile's fillet and its volume balance
//! at every level, the height's gradient, order independence and the closed forms.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, chi_square_gof};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_testkit::lcg::Lcg;

use super::*;
use crate::craters::{CraterParamsParts, TargetClass};
use crate::cube::xyz_to_face_uv;
use crate::field::{CoarseField, PartialField};
use crate::geometry::finest_level;
use crate::synth::test_support::{random_cells, random_points_in};
use crate::testing::{SyntheticWorld, synthetic_field};

/// The detail seed the tests draw from.
const SEED: DetailSeed = DetailSeed::new(0x7363_7261_7465_7273);

/// The cratered synthetic worlds: airless and at the lunar reference, screened by a thin
/// atmosphere and mapped, icy and oblate, and screened by a thick one.
pub(super) const WORLDS: [SyntheticWorld; 4] = [
    SyntheticWorld::MoonLike,
    SyntheticWorld::MarsLike,
    SyntheticWorld::CeresLike,
    SyntheticWorld::EarthLike,
];

/// Each cratered synthetic world, built once.
pub(super) fn world(kind: SyntheticWorld) -> &'static CoarseField {
    static FIELDS: OnceLock<Vec<CoarseField>> = OnceLock::new();
    let fields = FIELDS.get_or_init(|| WORLDS.iter().map(|&k| synthetic_field(k)).collect());
    let at = WORLDS
        .iter()
        .position(|&k| k == kind)
        .expect("a cratered world");
    &fields[at]
}

/// The small craters of `kind`, from [`SEED`].
fn small(kind: SyntheticWorld) -> SmallCraters {
    SmallCraters::new(world(kind).header(), SEED)
}

/// Every octave, from the largest.
const ALL: u8 = u8::MAX;

pub(super) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A unit tangent basis at the unit direction `d`.
pub(super) fn tangents(d: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let helper = if d[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let e1 = unit_dir(cross(d, helper));
    (e1, cross(d, e1))
}

/// The direction about `arc` radians from the unit direction `d` towards azimuth `azimuth`.
pub(super) fn towards(d: [f64; 3], arc: f64, azimuth: f64) -> [f64; 3] {
    let (e1, e2) = tangents(d);
    let (s, c) = math::sin_cos(azimuth);
    let t = math::tan(arc);
    unit_dir([0, 1, 2].map(|k| d[k] + t * (c * e1[k] + s * e2[k])))
}

/// `n` pseudo-random unit directions from `seed`, uniform on the sphere.
fn random_dirs(seed: u64, n: usize) -> Vec<[f64; 3]> {
    let mut stream = DetailSeed::new(seed).stream(
        SURFACE_CRATER,
        ObjectKey::surface_cell(0, 0, 0, 0, 7).expect("a surface cell"),
    );
    (0..n)
        .map(|_| {
            let z = 2.0 * stream.uniform() - 1.0;
            let phi = 2.0 * core::f64::consts::PI * stream.uniform();
            let r = (1.0 - z * z).sqrt();
            let (sine, cosine) = math::sin_cos(phi);
            [r * cosine, r * sine, z]
        })
        .collect()
}

#[test]
fn the_octaves_halve_from_the_boundary_diameter_to_the_band_limit() {
    // 16 octaves on an Earth (the last from 1.3 m, keeping 2 to 2.6 m), the Moon and Mars, and 15
    // on Ceres.
    for (kind, count) in WORLDS.into_iter().zip([16, 16, 15, 16]) {
        let header = world(kind).header();
        let octaves = CraterOctaves::new(header);
        let octaves = octaves.octaves();
        assert_eq!(octaves.len(), count, "{kind:?}");
        let d_b = header.boundary_diameter().value();
        let figure = header.figure();
        let smallest_m = num::min(figure.equatorial_radius_m, figure.polar_radius_m);
        let mut lower = d_b;
        for (k, octave) in (0_u8..).zip(octaves) {
            lower *= 0.5;
            assert_eq!(octave.index(), k);
            assert_same_bits(octave.diameters().lower().value(), lower);
            // Every octave keeps its whole span but the last, which straddles the band limit and
            // keeps from it up.
            let last = usize::from(k) + 1 == octaves.len();
            if last {
                assert!(
                    lower < SMALLEST_DIAMETER.value() && 2.0 * lower > SMALLEST_DIAMETER.value()
                );
                assert_eq!(octave.smallest(), SMALLEST_DIAMETER);
            } else {
                assert!(lower >= SMALLEST_DIAMETER.value());
                assert_eq!(octave.smallest(), octave.diameters().lower());
            }
            // The level is the deepest whose narrowest chord holds the octave's reach.
            let reach = (1.0 + MAX_EXTERIOR_WIDTH) * lower;
            let chord = |level: u8| {
                let width = min_rate() / f64::from(1_u32 << level);
                SEARCH_MARGIN * 2.0 * smallest_m * math::sin(0.5 * width)
            };
            assert!(reach <= chord(octave.level()), "{kind:?} octave {k}");
            assert!(reach > chord(octave.level() + 1), "{kind:?} octave {k}");
            if k > 0 {
                assert_eq!(octave.level(), octaves[usize::from(k) - 1].level() + 1);
            }
        }
        // The octave after the last would keep nothing.
        assert!(lower < SMALLEST_DIAMETER.value(), "{kind:?}");
    }
    // The flat world's craters have a density of zero: its octaves draw nothing.
    let flat = synthetic_field(SyntheticWorld::Flat);
    let none = SmallCraters::new(flat.header(), SEED);
    let mut cache = CraterCache::new();
    for octave in none.octaves().octaves() {
        assert_same_bits(octave.density().value(), 0.0);
        assert_same_bits(octave.variance().value(), 0.0);
        let cell = PatchKey::containing(octave.level(), [0.6, 0.0, 0.8]).unwrap();
        assert!(none.craters_in(octave.index(), cell, &mut cache).is_empty());
    }
    let h = none.at(
        [0.6, 0.0, 0.8],
        BandLevel::new(19).unwrap(),
        ALL,
        &mut cache,
    );
    assert_same_bits(h.height_m, 0.0);
}

#[test]
fn an_octave_s_density_is_its_production_capped_at_saturation() {
    let mut saturated = 0;
    let mut open = 0;
    for kind in WORLDS {
        let header = world(kind).header();
        let params = header.craters();
        let system = params.system_age();
        let surface = header.surface_age();
        let n_surface = lunar_chronology(surface, system).value();
        for octave in CraterOctaves::new(header).octaves() {
            let (lo, hi) = (octave.diameters().lower(), octave.diameters().upper());
            let production =
                cumulative_density(params, lo).value() - cumulative_density(params, hi).value();
            let capped = saturation(lo).value() - saturation(hi).value();
            assert_same_bits(octave.production().value(), production);
            assert_same_bits(octave.saturation().value(), capped);
            assert_same_bits(octave.drawn_density().value(), num::min(production, capped));
            let kept = if production > 0.0 {
                (cumulative_density(params, octave.smallest()).value()
                    - cumulative_density(params, hi).value())
                    / production
            } else {
                1.0
            };
            assert_same_bits(
                octave.density().value(),
                num::min(production, capped) * kept,
            );
            if octave.is_saturated() {
                saturated += 1;
                // The window holds the youngest share N_sat ÷ N_prod of the surface's craters.
                let window = octave.window();
                assert!(window.value() < surface.value(), "{kind:?}");
                let held = lunar_chronology(window, system).value();
                let share = capped / production;
                assert!((held / (n_surface * share) - 1.0).abs() < 1e-9, "{kind:?}");
            } else {
                open += 1;
                assert_eq!(octave.window(), surface, "{kind:?}");
            }
        }
    }
    // The Moon's small octaves are saturated and its large ones are not.
    assert!(
        saturated >= 5 && open >= 5,
        "{saturated} saturated, {open} open"
    );
    let moon = CraterOctaves::new(world(SyntheticWorld::MoonLike).header());
    assert!(!moon.octaves()[0].is_saturated());
    assert!(moon.octaves().last().unwrap().is_saturated());
}

/// The closed-form area of an oblate spheroid of radii `a` ≥ `c`: 2πa² (1 + ((1 − e²) ÷ e)
/// atanh e), e² = 1 − c² ÷ a², and 4πa² for a sphere.
fn spheroid_area(figure: &Spheroid) -> f64 {
    let (a, c) = (figure.equatorial_radius_m, figure.polar_radius_m);
    let e2 = 1.0 - (c * c) / (a * a);
    if e2 <= 0.0 {
        return 4.0 * core::f64::consts::PI * a * a;
    }
    let e = e2.sqrt();
    2.0 * core::f64::consts::PI * a * a * (1.0 + (1.0 - e2) / e * math::atanh(e))
}

/// The solid angle of `cell`, steradians, by the gnomonic rectangle's closed form over its corners.
fn solid_angle(cell: PatchKey) -> f64 {
    let side = f64::from(1_u32 << cell.level());
    let u = |i: u32| st_to_uv(f64::from(i) / side);
    let (u0, u1, v0, v1) = (u(cell.i()), u(cell.i() + 1), u(cell.j()), u(cell.j() + 1));
    let f = |u: f64, v: f64| math::atan2(u * v, (1.0 + u * u + v * v).sqrt());
    f(u1, v1) - f(u0, v1) - f(u1, v0) + f(u0, v0)
}

#[test]
fn cells_have_their_true_areas_on_the_spheroid() {
    // A sphere: each cell's area is its solid angle times R².
    let sphere = Spheroid::sphere(1.0e6);
    for level in 0..=10_u8 {
        let side = 1_u32 << level;
        for (i, j) in [
            (0, 0),
            (side / 3, side / 2),
            (side - 1, side / 5),
            (side / 2, side - 1),
        ] {
            for face in Face::ALL {
                let cell = PatchKey::new(face, level, i, j).unwrap();
                let exact = solid_angle(cell) * 1e12;
                let area = cell_area(cell, &sphere).value();
                assert!(
                    (area / exact - 1.0).abs() < 1e-9,
                    "{cell:?}: {area} {exact}"
                );
            }
        }
    }
    // The cells of a level tile the spheroid, an oblate Ceres-like one among them.
    for figure in [
        Spheroid::sphere(1.0e6),
        world(SyntheticWorld::CeresLike).header().figure(),
        Spheroid::WGS84,
    ] {
        let exact = spheroid_area(&figure);
        for level in [0_u8, 1, 3] {
            let side = 1_u32 << level;
            let mut total = 0.0;
            for face in Face::ALL {
                for i in 0..side {
                    for j in 0..side {
                        total +=
                            cell_area(PatchKey::new(face, level, i, j).unwrap(), &figure).value();
                    }
                }
            }
            assert!(
                (total / exact - 1.0).abs() < 1e-9,
                "{figure:?} level {level}: {total} against {exact}"
            );
        }
    }
}

#[test]
fn a_placement_s_acceptance_rises_towards_one_on_fine_cells() {
    // The cell's mean area element over the bound, the chance a candidate is accepted, on every
    // face of the oblate Ceres-like figure (f 0.075): measured 0.276–0.288 on level-1 cells,
    // 0.847–0.924 at level 5 and 0.924–0.995 at level 12, where the spheroid's bound, its
    // polar radius against the equatorial, c ÷ a = 0.925, sets the least.
    let figure = world(SyntheticWorld::CeresLike).header().figure();
    for (level, least) in [(1_u8, 0.25), (5, 0.8), (12, 0.9)] {
        let side = 1_u32 << level;
        let span_st = 1.0 / f64::from(side * side);
        for face in Face::ALL {
            for (i, j) in [(0, 0), (side / 2, side / 3), (side - 1, side - 1)] {
                let cell = PatchKey::new(face, level, i, j).unwrap();
                let rate = cell_area(cell, &figure).value()
                    / (area_density_bound(cell, &figure) * span_st);
                assert!(rate > least && rate <= 1.0, "{cell:?}: {rate}");
            }
        }
    }
}

#[test]
fn craters_are_placed_uniformly_in_the_spheroid_s_area() {
    // Level-1 cells, half a face each, across which the area per unit of s and t varies most; on
    // the oblate Ceres-like figure. Each child's share of the placements is its share of the area.
    let figure = world(SyntheticWorld::CeresLike).header().figure();
    for cell in [
        PatchKey::new(Face::PosX, 1, 1, 0).unwrap(),
        PatchKey::new(Face::PosZ, 1, 0, 1).unwrap(),
    ] {
        let mut stream = SEED.stream(
            SURFACE_CRATER,
            ObjectKey::surface_cell(cell.face().index(), 1, cell.i(), cell.j(), 9).unwrap(),
        );
        let bound = area_density_bound(cell, &figure);
        let children = cell.children();
        let mut observed = [0_u64; 4];
        let draws = 40_000;
        for _ in 0..draws {
            let dir = place(cell, &figure, bound, &mut stream);
            let child = PatchKey::containing(2, dir).unwrap();
            let at = children
                .iter()
                .position(|&c| c == child)
                .expect("in the cell");
            observed[at] += 1;
        }
        let whole = cell_area(cell, &figure).value();
        let expected = children.map(|c| f64::from(draws) * cell_area(c, &figure).value() / whole);
        let fit = chi_square_gof(&observed, &expected);
        assert_p_value(&format!("{cell:?}"), fit.p_value, ALPHA);
        // Uniform in (s, t) would put a quarter in each, which the test would refuse.
        let flat = f64::from(draws) / 4.0;
        assert!(
            expected.iter().any(|e| (e - flat).abs() > 1_000.0),
            "{expected:?}"
        );
    }
}

/// The cells of `octave`'s level a size–frequency test counts over, and their total true area in
/// km²: the cells under the cell of the level `depth` above it about a fixed direction, for the
/// least depth (at most 6) whose expected count is at least 400, or every cell of the body when
/// the octave's level is 6 or less and none is.
fn region(octave: &CraterOctave, figure: &Spheroid) -> (Vec<PatchKey>, f64) {
    let level = octave.level();
    let anchor = unit_dir([0.31, -0.52, 0.79]);
    let density = octave.density().value();
    let area_km2 = |cells: &[PatchKey]| -> f64 {
        cells
            .iter()
            .map(|&c| cell_area(c, figure).value() * 1e-6)
            .sum()
    };
    let under = |root: PatchKey| -> Vec<PatchKey> {
        let depth = level - root.level();
        let side = 1_u32 << depth;
        let mut cells = Vec::new();
        for i in 0..side {
            for j in 0..side {
                cells.push(
                    PatchKey::new(root.face(), level, root.i() * side + i, root.j() * side + j)
                        .unwrap(),
                );
            }
        }
        cells
    };
    for depth in 0..=level.min(6) {
        let root = PatchKey::containing(level - depth, anchor).unwrap();
        let root_km2 = cell_area(root, figure).value() * 1e-6;
        if density * root_km2 >= 400.0 || depth == level.min(6) && level > 6 {
            let cells = under(root);
            let area = area_km2(&cells);
            return (cells, area);
        }
    }
    let cells: Vec<PatchKey> = Face::ALL
        .into_iter()
        .flat_map(|face| under(PatchKey::root(face)))
        .collect();
    let area = area_km2(&cells);
    (cells, area)
}

#[test]
fn the_realised_size_frequency_matches_the_capped_production() {
    let tests: usize = WORLDS
        .iter()
        .map(|&k| small(k).octaves().octaves().len())
        .sum();
    #[expect(
        clippy::cast_precision_loss,
        reason = "a test count is small and exact in f64"
    )]
    let alpha = ALPHA / tests as f64;
    let mut counted = 0_u64;
    for kind in WORLDS {
        let craters = small(kind);
        let figure = world(kind).header().figure();
        let mut cache = CraterCache::new();
        for octave in craters.octaves().octaves() {
            let (cells, area_km2) = region(octave, &figure);
            let mut count = 0_u64;
            for &cell in &cells {
                let side = f64::from(1_u32 << cell.level());
                for crater in craters.craters_in(octave.index(), cell, &mut cache) {
                    count += 1;
                    assert_eq!(crater.octave(), octave.index());
                    let d = crater.diameter();
                    assert!(d >= octave.smallest() && d < octave.diameters().upper());
                    // Its centre lies in its own cell.
                    let FaceUv { face, u, v } = xyz_to_face_uv(crater.centre());
                    let (s, t) = (
                        crate::cube::uv_to_st(u) * side,
                        crate::cube::uv_to_st(v) * side,
                    );
                    let inside =
                        |x: f64, i: u32| x >= f64::from(i) - 1e-9 && x <= f64::from(i + 1) + 1e-9;
                    assert!(
                        face == cell.face() && inside(s, cell.i()) && inside(t, cell.j()),
                        "{kind:?}: a crater outside {cell:?}"
                    );
                }
            }
            let mean = octave.density().value() * area_km2;
            assert_poisson_count(
                &format!("{kind:?} octave {}", octave.index()),
                count,
                mean,
                alpha,
            );
            counted += count;
        }
    }
    assert!(counted > 20_000, "{counted} craters counted");
}

#[test]
fn a_crater_straddling_a_face_edge_is_found_from_both_faces() {
    let mut straddling = 0;
    for kind in [SyntheticWorld::MoonLike, SyntheticWorld::CeresLike] {
        let craters = small(kind);
        let figure = world(kind).header().figure();
        let mut cache = CraterCache::new();
        for octave in craters.octaves().octaves().iter().skip(2).take(8) {
            let level = octave.level();
            let last = (1_u32 << level) - 1;
            // The cells of face 0 along its four edges, and its four corners.
            let step = (last / 16).max(1);
            let mut cells = Vec::new();
            for n in (0..=last).step_by(usize::try_from(step).unwrap()) {
                for (i, j) in [(0, n), (last, n), (n, 0), (n, last)] {
                    cells.push(PatchKey::new(Face::PosX, level, i, j).unwrap());
                }
            }
            for cell in cells {
                let drawn = craters
                    .craters_in(octave.index(), cell, &mut cache)
                    .to_vec();
                for crater in drawn {
                    let reach = crater.shape().reach().value();
                    let radius = figure.volumetric_radius_m();
                    for step in 0..8 {
                        let azimuth = f64::from(step) * core::f64::consts::PI / 4.0;
                        let dir = towards(crater.centre(), 0.6 * reach / radius, azimuth);
                        let p = figure.point(dir);
                        let q = crater.point_m();
                        let chord = [0, 1, 2].map(|k| p[k] - q[k]);
                        if dot(chord, chord).sqrt() >= reach {
                            continue;
                        }
                        let found = craters
                            .craters_reaching(dir, octave.index(), &mut cache)
                            .iter()
                            .any(|c| c.centre().map(bits) == crater.centre().map(bits));
                        assert!(found, "{kind:?}: a crater of {cell:?} not found at {dir:?}");
                        if xyz_to_face_uv(dir).face != cell.face() {
                            straddling += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(
        straddling >= 50,
        "only {straddling} points across a face edge"
    );
}

#[test]
fn the_three_by_three_search_finds_every_crater_within_reach() {
    for kind in [SyntheticWorld::MoonLike, SyntheticWorld::CeresLike] {
        let craters = small(kind);
        let figure = world(kind).header().figure();
        let mut cache = CraterCache::new();
        let mut dirs = random_dirs(0x3333, 30);
        // Beside a face edge and a cube corner.
        dirs.push(unit_dir([1.0, 1.0, 0.3]));
        dirs.push(unit_dir([1.0, 0.999_99, 0.999_98]));
        dirs.push(unit_dir([-1.0, 0.2, 1.0]));
        let mut found = 0;
        for dir in dirs {
            let p = figure.point(dir);
            let searched: BTreeSet<[u64; 3]> = craters
                .craters_reaching(dir, ALL, &mut cache)
                .iter()
                .map(|c| c.centre().map(bits))
                .collect();
            // Every crater of the 5 × 5 about the point's cell that reaches it.
            let mut wide = BTreeSet::new();
            for octave in craters.octaves().octaves() {
                let home = PatchKey::containing(octave.level(), dir).unwrap();
                let (near, n) = neighbourhood(home);
                let mut cells = BTreeSet::new();
                for &cell in &near[..n] {
                    let (outer, m) = neighbourhood(cell);
                    cells.extend(outer[..m].iter().copied());
                }
                for cell in cells {
                    for crater in craters.craters_in(octave.index(), cell, &mut cache) {
                        let q = crater.point_m();
                        let chord = [0, 1, 2].map(|k| p[k] - q[k]);
                        let reach = crater.shape().reach().value();
                        if dot(chord, chord) < reach * reach {
                            wide.insert(crater.centre().map(bits));
                        }
                    }
                }
            }
            assert_eq!(searched, wide, "{kind:?} at {dir:?}");
            found += searched.len();
        }
        assert!(
            found > 40,
            "{kind:?}: only {found} craters reach the points"
        );
    }
}

/// The crater contracts of the worlds' bodies, and a porous one.
fn contracts() -> Vec<CraterParams> {
    let mut all: Vec<CraterParams> = WORLDS
        .iter()
        .map(|&k| *world(k).header().craters())
        .collect();
    let mut porous: CraterParamsParts = *all[0].parts();
    porous.target.class = TargetClass::Porous;
    all.push(CraterParams::new(porous).unwrap());
    all
}

/// The diameters the profile tests take, metres: bowls of the regolith's sizes, transitional and
/// complex craters to multi-ring basins on the Moon.
const DIAMETERS_M: [f64; 13] = [
    2.5, 10.0, 63.0, 150.0, 400.0, 1e3, 3e3, 8e3, 15e3, 25e3, 60e3, 200e3, 400e3,
];

/// The five-point Gauss–Legendre rule on [−1, 1], its nodes and weights (Abramowitz and Stegun,
/// Table 25.4, n = 5): exact for polynomials to degree 9, and independent of the eight-point rule
/// the code integrates with.
pub(super) const GAUSS_LEGENDRE_5: [(f64, f64); 5] = [
    (-0.906_179_845_938_664, 0.236_926_885_056_189_1),
    (-0.538_469_310_105_683_1, 0.478_628_670_499_366_5),
    (0.0, 0.568_888_888_888_888_9),
    (0.538_469_310_105_683_1, 0.478_628_670_499_366_5),
    (0.906_179_845_938_664, 0.236_926_885_056_189_1),
];

/// ∫ f(x) 2x dx over [`lo`, `hi`] by the five-point Gauss–Legendre rule on each of 16 equal
/// panels: exact to rounding where f x is a polynomial of degree at most 9, as every piece of a
/// profile is.
pub(super) fn quadrature(lo: f64, hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    let panels = 16;
    let width = (hi - lo) / f64::from(panels);
    let mut total = 0.0;
    for k in 0..panels {
        let a = lo + width * f64::from(k);
        let mid = a + 0.5 * width;
        let mut sum = 0.0;
        for (node, weight) in GAUSS_LEGENDRE_5 {
            let x = mid + 0.5 * width * node;
            sum += weight * f(x) * 2.0 * x;
        }
        total += 0.5 * width * sum;
    }
    total
}

/// Every profile the balance tests check: each contract's shapes at [`DIAMETERS_M`], filleted at
/// every level of an Earth's figure for craters of 0, 0.3 and 4 Gyr.
fn profiles() -> Vec<CraterProfile> {
    let mut all = Vec::new();
    for params in contracts() {
        for d in DIAMETERS_M {
            let shape = crater_shape(&params, Metres::new(d));
            for age in [0.0, 0.3, 4.0] {
                let diffusion = diffusion_length(Gigayears::new(age));
                for level in 0..=MAX_LEVEL {
                    let band = band_half_width(&Spheroid::WGS84, BandLevel::new(level).unwrap());
                    all.push(CraterProfile::new(shape, band, diffusion));
                }
            }
        }
    }
    all
}

#[test]
fn every_crater_integrates_to_zero_at_every_level() {
    let mut filleted = 0;
    for profile in profiles() {
        let points = profile.breakpoints();
        let mut volume = 0.0;
        for pair in points.windows(2) {
            if pair[1] > pair[0] {
                volume += quadrature(pair[0], pair[1], |x| profile.height_and_slope(x).0);
            }
        }
        let shape = profile.shape();
        let rim_m = 0.5 * shape.diameter().value();
        let volume_m3 = core::f64::consts::PI * rim_m * rim_m * volume;
        assert!(
            volume_m3.abs() <= 1e-6 * shape.cavity_volume_m3(),
            "{shape:?} filleted over {:?}: {volume_m3} m³ of {}",
            profile.fillet_half_width(),
            shape.cavity_volume_m3()
        );
        // Zero at and beyond the reach.
        let reach = 1.0 + shape.exterior_width();
        for x in [reach, reach * 1.5] {
            let (h, slope) = profile.height_and_slope(x);
            assert_same_bits(h, 0.0);
            assert_same_bits(slope, 0.0);
        }
        if profile.fillet > 0.0 {
            filleted += 1;
        }
    }
    assert!(filleted > 4_000, "{filleted} filleted profiles");
}

#[test]
fn the_fillet_s_closed_form_is_its_quadrature() {
    for profile in profiles() {
        if profile.fillet <= 0.0 {
            continue;
        }
        let shape = profile.shape();
        let (x0, x1) = (1.0 - profile.fillet, 1.0 + profile.fillet);
        let scale = shape.rim_height().value() * (0.8 + 0.2 * shape.exterior_width());
        // The fresh profile's moment, piece by piece, against quadrature between its pieces.
        let mut cuts = vec![x0, x1];
        for x in [shape.peak_fraction(), shape.floor_fraction(), 1.0] {
            if x > x0 && x < x1 {
                cuts.push(x);
            }
        }
        cuts.sort_by(f64::total_cmp);
        let fresh: f64 = cuts
            .windows(2)
            .map(|p| quadrature(p[0], p[1], |x| shape.relief_and_slope(x).0))
            .sum();
        assert!(
            (shape.moment(x0, x1) - fresh).abs() <= 1e-9 * scale,
            "{shape:?}: {} against {fresh}",
            shape.moment(x0, x1)
        );
        // The Hermite's moment, and so the compensation.
        let hermite_volume = quadrature(x0, x1, |x| {
            hermite(x0, x1 - x0, profile.inner, profile.outer, x).0
        });
        let change = hermite_volume - fresh;
        let reach = 1.0 + shape.exterior_width();
        let expected = -3.0 * change / (reach * reach);
        assert!(
            (profile.compensation().value() - expected).abs() <= 1e-9 * scale,
            "{shape:?}: compensation {:?} against {expected}",
            profile.compensation()
        );
        // The compensating term's own volume, c r₁² ÷ 3.
        let c = profile.compensation().value();
        let term = quadrature(0.0, reach, |x| {
            let b = 1.0 - (x / reach) * (x / reach);
            c * b * b
        });
        assert!((term - c * reach * reach / 3.0).abs() <= 1e-12 * scale.max(c.abs()));
    }
}

#[test]
fn a_profile_without_a_fillet_is_the_fresh_profile() {
    for params in contracts() {
        for d in DIAMETERS_M {
            let shape = crater_shape(&params, Metres::new(d));
            let plain = CraterProfile::filleted(shape, Metres::ZERO);
            assert_same_bits(plain.compensation().value(), 0.0);
            assert_same_bits(plain.fillet_half_width().value(), 0.0);
            for k in 0..=300 {
                let x = f64::from(k) / 100.0;
                assert_same_bits(plain.at(x).height_m, shape.relief(x).value());
                assert_same_bits(
                    profile(x, &shape, Metres::ZERO, Metres::ZERO).height_m,
                    shape.relief(x).value(),
                );
            }
        }
    }
}

#[test]
fn the_profile_is_continuous_and_its_slope_is_its_derivative() {
    for profile in profiles() {
        let shape = profile.shape();
        let scale = shape.depth().value();
        let reach = 1.0 + shape.exterior_width();
        // Continuous in height and slope across the fillet's ends.
        if profile.fillet > 0.0 {
            // A step of a thousand-millionth of the fillet, across which the profile barely moves.
            let step = 1e-9 * profile.fillet;
            let ends = [1.0 - profile.fillet, 1.0 + profile.fillet];
            for x in ends.into_iter().filter(|&x| x > step) {
                let (below, above) = (
                    profile.height_and_slope(x - step),
                    profile.height_and_slope(x + step),
                );
                assert!(
                    (below.0 - above.0).abs() <= 1e-6 * scale,
                    "{shape:?} at {x}"
                );
                assert!(
                    (below.1 - above.1).abs() <= 1e-5 * scale,
                    "{shape:?} at {x}"
                );
            }
        }
        // The slope is the height's derivative, by a step well inside the fillet, away from the
        // cone's kinks and the pieces' ends, where the curvature jumps.
        let (x0, x1) = (1.0 - profile.fillet, 1.0 + profile.fillet);
        let h = num::min(1e-6, 1e-3 * profile.fillet);
        let ends = [
            0.0,
            shape.peak_fraction(),
            shape.floor_fraction(),
            x0,
            x1,
            reach,
        ];
        for k in 1..200 {
            let x = reach * f64::from(k) / 200.0;
            if ends.iter().any(|&end| (x - end).abs() < 2.0 * h) {
                continue;
            }
            let numeric =
                (profile.height_and_slope(x + h).0 - profile.height_and_slope(x - h).0) / (2.0 * h);
            let slope = profile.height_and_slope(x).1;
            assert!(
                (numeric - slope).abs() <= 1e-5 * scale.max(slope.abs()),
                "{shape:?} filleted over {:?} at {x}: {slope} against {numeric}",
                profile.fillet_half_width()
            );
        }
    }
}

#[test]
fn the_fillet_is_held_inside_the_crater() {
    let moon = *world(SyntheticWorld::MoonLike).header().craters();
    for d in [10.0, 1e3, 30e3] {
        let shape = crater_shape(&moon, Metres::new(d));
        let wide = CraterProfile::filleted(shape, Metres::new(1e9));
        let held = num::min(1.0, shape.exterior_width());
        assert_same_bits(wide.fillet, held);
        assert_same_bits(
            wide.fillet_half_width().value(),
            shape.diameter().value() * (0.5 * held),
        );
        // Wider fillets take more of the rim, so the compensation grows with the width.
        let narrow = CraterProfile::filleted(shape, Metres::new(0.01 * d));
        assert!(narrow.compensation().value() > 0.0);
        assert!(wide.compensation().value() > narrow.compensation().value());
        // A filleted rim is lower than the fresh crest.
        assert!(narrow.at(1.0).height_m < shape.rim_height().value());
    }
}

#[test]
fn the_band_sets_the_fillet_and_the_age_the_relief() {
    let earth = Spheroid::WGS84;
    let finest = finest_level(earth.equatorial_radius_m);
    assert_eq!(finest, 19);
    assert_same_bits(
        band_half_width(&earth, BandLevel::new(finest).unwrap()).value(),
        1.0,
    );
    let coarse = band_half_width(&earth, BandLevel::new(5).unwrap()).value();
    assert!((coarse / 10_500.0 - 1.0).abs() < 0.02, "{coarse}");
    let mut previous = f64::INFINITY;
    for level in 0..=MAX_LEVEL {
        let w = band_half_width(&earth, BandLevel::new(level).unwrap()).value();
        assert!(w <= previous && w >= 1.0, "{level}: {w}");
        previous = w;
    }
    // √(2κt): about 204 m at 3.8 Gyr, none at no age.
    let l = diffusion_length(Gigayears::new(3.8)).value();
    assert!((l - (2.0 * 5.5 * 3_800.0_f64).sqrt()).abs() < 1e-9, "{l}");
    assert_same_bits(diffusion_length(Gigayears::ZERO).value(), 0.0);
    // The band alone sets the fillet; the age sets the relief.
    let shape = crater_shape(
        world(SyntheticWorld::MoonLike).header().craters(),
        Metres::new(1e3),
    );
    let profile = CraterProfile::new(shape, Metres::new(20.0), Metres::new(150.0));
    assert_same_bits(profile.fillet_half_width().value(), 20.0);
    assert_same_bits(
        profile.relief_factor(),
        relief_factor(Metres::new(500.0), Metres::new(150.0)),
    );
}

#[test]
fn the_relief_factor_is_minton_s_depth_law() {
    // g = (b ÷ (K ÷ R² + b))², K = κt = L² ÷ 2: 1 when fresh, ¼ where K ÷ R² is b.
    assert_same_bits(relief_factor(Metres::new(150.0), Metres::ZERO), 1.0);
    let rim = 150.0;
    let at_b = (2.0 * DEGRADATION_B).sqrt() * rim;
    assert!((relief_factor(Metres::new(rim), Metres::new(at_b)) - 0.25).abs() < 1e-12);
    let mut previous = 1.0;
    for k in 1..=40 {
        let g = relief_factor(Metres::new(rim), Metres::new(10.0 * f64::from(k)));
        assert!(g < previous && g > 0.0, "{k}: {g}");
        previous = g;
    }
    // At 3 Gyr, K = 16,500 m²: a 300 m crater (K ÷ R² = 0.73, extrapolated beyond Minton et
    // al.'s fitted 0 to 0.3) keeps 2.9% of its relief, against Fassett and Thomson 2014's ~7%,
    // and a 1 km one (0.066) 48%, against their ~52%.
    let diffusion = diffusion_length(Gigayears::new(3.0));
    let small = relief_factor(Metres::new(150.0), diffusion);
    let large = relief_factor(Metres::new(500.0), diffusion);
    assert!((small - 0.0288).abs() < 5e-4, "{small}");
    assert!((large - 0.482).abs() < 5e-3, "{large}");
}

#[test]
fn a_degraded_crater_scales_its_filleted_profile_by_its_relief_factor() {
    // At every distance, every crater and every age: the band's fillet times g, so the volume
    // stays zero and the reach unchanged; the closed form's ∫h² carries g².
    let moon = *world(SyntheticWorld::MoonLike).header().craters();
    for d in DIAMETERS_M {
        let shape = crater_shape(&moon, Metres::new(d));
        let band = Metres::new(1.0);
        let filleted = CraterProfile::filleted(shape, band);
        for age in [0.0, 0.3, 3.0] {
            let diffusion = diffusion_length(Gigayears::new(age));
            let degraded = CraterProfile::new(shape, band, diffusion);
            let g = degraded.relief_factor();
            for k in 0..=300 {
                let x = f64::from(k) / 100.0;
                let (h, slope) = degraded.height_and_slope(x);
                let (h0, slope0) = filleted.height_and_slope(x);
                assert_same_bits(h, g * h0);
                assert_same_bits(slope, g * slope0);
            }
            let ratio = degraded.square_moment() / filleted.square_moment();
            assert!((ratio / (g * g) - 1.0).abs() < 1e-12, "{d} m, {age} Gyr");
        }
    }
    // A 300 m bowl at 3 Gyr, filleted at an Earth's finest band: its relief from rim to floor
    // falls to a few per cent of the fresh bowl's (Fassett and Thomson 2014: ~7%; exact
    // diffusion of this bowl: 5.2%).
    let bowl = crater_shape(&moon, Metres::new(300.0));
    let degraded = CraterProfile::new(
        bowl,
        Metres::new(1.0),
        diffusion_length(Gigayears::new(3.0)),
    );
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    for k in 0..=2_000 {
        let h = degraded.at(2.6 * f64::from(k) / 2_000.0).height_m;
        low = low.min(h);
        high = high.max(h);
    }
    let kept = (high - low) / bowl.depth().value();
    assert!((0.01..0.06).contains(&kept), "{kept}");
}

#[test]
fn a_shape_s_cavity_volume_is_its_interior_s_and_its_exterior_s() {
    for params in contracts() {
        for d in DIAMETERS_M {
            let shape = crater_shape(&params, Metres::new(d));
            let rim = 0.5 * d;
            let area = core::f64::consts::PI * rim * rim;
            let a = shape.exterior_width();
            let cavity = shape.cavity_volume_m3();
            assert!(cavity > 0.0);
            let interior = -area * shape.moment(0.0, 1.0);
            let exterior = area * shape.moment(1.0, 1.0 + a);
            assert!((interior / cavity - 1.0).abs() < 1e-9, "{shape:?}");
            assert!((exterior / cavity - 1.0).abs() < 1e-12, "{shape:?}");
        }
    }
}

#[test]
fn the_height_s_gradient_is_its_derivative_along_the_surface() {
    let field = world(SyntheticWorld::MoonLike);
    let craters = small(SyntheticWorld::MoonLike);
    let figure = field.header().figure();
    let level = BandLevel::new(finest_level(figure.equatorial_radius_m)).unwrap();
    let radius = figure.volumetric_radius_m();
    let mut cache = CraterCache::new();
    // Points near craters of every octave, at varied distances from their centres.
    let mut points = Vec::new();
    let mut stream = SEED.stream(
        SURFACE_CRATER,
        ObjectKey::surface_cell(0, 0, 0, 0, 11).unwrap(),
    );
    for octave in craters.octaves().octaves() {
        for dir in random_dirs(0x6772_6164 + u64::from(octave.index()), 4) {
            let home = PatchKey::containing(octave.level(), dir).unwrap();
            let drawn = craters
                .craters_in(octave.index(), home, &mut cache)
                .to_vec();
            for crater in drawn.iter().take(2) {
                let rim = 0.5 * crater.diameter().value();
                let x = 2.5 * stream.uniform();
                let azimuth = 2.0 * core::f64::consts::PI * stream.uniform();
                points.push(towards(crater.centre(), x * rim / radius, azimuth));
            }
        }
    }
    assert!(points.len() > 40, "{} points", points.len());
    let mut sloped = 0;
    for dir in points {
        let sample = craters.at(dir, level, ALL, &mut cache);
        let (e1, e2) = tangents(dir);
        for e in [e1, e2] {
            // A step of a millimetre along the surface.
            let eps = 0.001 / radius;
            let moved = |sign: f64| unit_dir([0, 1, 2].map(|k| dir[k] + sign * eps * e[k]));
            let up = craters.at(moved(1.0), level, ALL, &mut cache).height_m;
            let down = craters.at(moved(-1.0), level, ALL, &mut cache).height_m;
            let numeric = (up - down) / (2.0 * eps);
            let along = figure.point(e);
            let analytic = dot(sample.gradient, along);
            assert!(
                (numeric - analytic).abs() <= 1e-4 * analytic.abs().max(1e-3),
                "at {dir:?}: {analytic} against {numeric}"
            );
            if analytic.abs() > 1e-3 {
                sloped += 1;
            }
        }
    }
    assert!(sloped > 40, "only {sloped} sloped samples");
}

#[test]
fn the_same_seed_draws_the_same_craters_and_another_seed_others() {
    let header = world(SyntheticWorld::MoonLike).header();
    let a = SmallCraters::new(header, SEED);
    let b = SmallCraters::new(header, SEED);
    let other = SmallCraters::new(header, DetailSeed::new(SEED.get() ^ 1));
    let octave = &a.octaves().octaves()[6];
    let cell = PatchKey::containing(octave.level(), [0.2, 0.9, -0.4]).unwrap();
    let mine = a
        .craters_in(octave.index(), cell, &mut CraterCache::new())
        .to_vec();
    assert!(!mine.is_empty());
    assert_eq!(
        mine,
        b.craters_in(octave.index(), cell, &mut CraterCache::new())
    );
    assert_ne!(
        mine.as_slice(),
        other.craters_in(octave.index(), cell, &mut CraterCache::new())
    );
}

#[test]
fn the_last_octave_keeps_its_drawn_craters_of_two_metres_and_wider() {
    // A cell of the Moon's last octave drawn twice: as built, keeping 2 m and wider, and as if it
    // kept its whole octave. The kept craters are, bit for bit, the whole draw's of 2 m and wider,
    // so a dropped crater still takes its words and moves none after it.
    let craters = small(SyntheticWorld::MoonLike);
    let last = *craters.octaves().octaves().last().unwrap();
    assert!(last.smallest() > last.diameters().lower());
    let mut whole = craters.clone();
    let octave = whole.octaves.octaves.last_mut().unwrap();
    octave.smallest = octave.diameters.lower();
    let mut dropped = 0;
    for dir in random_dirs(0x6c61_7374, 40) {
        let cell = PatchKey::containing(last.level(), dir).unwrap();
        let kept = craters
            .craters_in(last.index(), cell, &mut CraterCache::new())
            .to_vec();
        let all = whole
            .craters_in(last.index(), cell, &mut CraterCache::new())
            .to_vec();
        let wide: Vec<SmallCrater> = all
            .iter()
            .copied()
            .filter(|c| c.diameter() >= SMALLEST_DIAMETER)
            .collect();
        assert_eq!(kept, wide, "{cell:?}");
        dropped += all.len() - kept.len();
    }
    assert!(dropped > 20, "only {dropped} craters under 2 m drawn");
}

#[test]
fn a_full_cache_starts_afresh_and_moves_no_crater() {
    // The Moon's last octave, whose cells are the most numerous: one cell, then 16,384 others,
    // the last of which finds the cache full and clears it; the first cell's craters, drawn
    // again, are those a fresh cache draws.
    let craters = small(SyntheticWorld::MoonLike);
    let octave = *craters.octaves().octaves().last().unwrap();
    let level = octave.level();
    let first = PatchKey::containing(level, [0.0, 0.6, 0.8]).unwrap();
    let expected = craters
        .craters_in(octave.index(), first, &mut CraterCache::new())
        .to_vec();
    let mut cache = CraterCache::new();
    let _ = craters.craters_in(octave.index(), first, &mut cache);
    let root = PatchKey::containing(level - 7, [0.4, 0.4, 0.82]).unwrap();
    assert_ne!(
        root,
        PatchKey::containing(level - 7, [0.0, 0.6, 0.8]).unwrap()
    );
    let side = 1_u32 << 7;
    let mut drawn = 1;
    for i in 0..side {
        for j in 0..side {
            let cell = PatchKey::new(root.face(), level, root.i() * side + i, root.j() * side + j)
                .unwrap();
            let _ = craters.craters_in(octave.index(), cell, &mut cache);
            drawn += 1;
        }
    }
    assert_eq!(drawn, CACHE_CELLS + 1);
    assert_eq!(cache.len(), 1, "the cache cleared once full");
    assert_eq!(
        craters.craters_in(octave.index(), first, &mut cache),
        expected.as_slice()
    );
    assert_eq!(cache.len(), 2);
}

#[test]
fn the_cache_moves_no_height() {
    let craters = small(SyntheticWorld::MoonLike);
    let mars = small(SyntheticWorld::MarsLike);
    let level = BandLevel::new(17).unwrap();
    let points = random_dirs(0x6361_6368, 24);
    let shared = RefCell::new(CraterCache::new());
    // Warm the shared cache on another body first: it starts afresh for this one.
    let _ = mars.at(points[0], level, ALL, &mut shared.borrow_mut());
    assert_order_independent(&points, |&dir| {
        let h = craters.at(dir, level, ALL, &mut shared.borrow_mut());
        (bits(h.height_m), h.gradient.map(bits))
    });
    for &dir in &points {
        let fresh = craters.at(dir, level, ALL, &mut CraterCache::new());
        let warm = craters.at(dir, level, ALL, &mut shared.borrow_mut());
        assert_eq!(bits(fresh.height_m), bits(warm.height_m));
        assert_eq!(fresh.gradient.map(bits), warm.gradient.map(bits));
    }
}

#[test]
fn the_unresolved_variance_falls_with_the_wavelength_to_zero_at_the_band_limit() {
    for kind in WORLDS {
        let field = world(kind);
        let header = field.header();
        let octaves = CraterOctaves::new(header);
        // Beyond every octave's diameters, the whole ground at its fillet (R09.T7.h).
        let whole: f64 = octaves
            .octaves()
            .iter()
            .map(|o| o.ground_variance().value())
            .sum();
        assert!(whole > 0.0, "{kind:?}");
        let at = |w: f64| octaves.unresolved_variance(Metres::new(w)).value();
        assert!((at(1e9) / whole - 1.0).abs() < 1e-12, "{kind:?}");
        // Non-increasing as the wavelength shortens, in steps of a fifth: the kept octaves'
        // sharpening narrows with the band, and an octave's sharpening is no more than its ground.
        let mut previous = f64::INFINITY;
        let mut w = 1e9;
        while w > 0.5 {
            let v = at(w);
            assert!(v <= previous, "{kind:?} at {w} m: {v} after {previous}");
            previous = v;
            w *= 0.8;
        }
        assert_same_bits(at(2.0), 0.0);
        assert_same_bits(at(0.5), 0.0);
        // The field's per-contribution variance carries it, in every cell the view holds.
        let level = header.level().get();
        let cell = PatchKey::containing(level, [0.0, 0.6, 0.8]).unwrap();
        let parts = crate::synth::unresolved_variance(field, cell, Metres::new(1e3)).unwrap();
        assert_eq!(
            parts.craters(),
            octaves.unresolved_variance(Metres::new(1e3))
        );
        assert_eq!(
            unresolved_variance(field, cell, Metres::new(1e3)),
            Some(parts.craters())
        );
        let empty = PartialField::new(header.clone());
        assert_eq!(unresolved_variance(&empty, cell, Metres::new(1e3)), None);
        assert_eq!(
            structure_function(&empty, cell, Metres::new(1e3), Metres::new(10.0)),
            None
        );
    }
}

#[test]
fn the_structure_function_rises_from_zero_to_twice_the_variance() {
    for kind in WORLDS {
        let field = world(kind);
        let octaves = CraterOctaves::new(field.header());
        for w in [1e9, 1e4, 300.0] {
            let finer = Metres::new(w);
            let variance = octaves.unresolved_variance(finer).value();
            assert_same_bits(octaves.structure_function(finer, Metres::ZERO).value(), 0.0);
            let far = octaves.structure_function(finer, Metres::new(1e9)).value();
            assert!(
                (far / (2.0 * variance) - 1.0).abs() < 0.01,
                "{kind:?} {w}: {far}"
            );
            for lag in [1.0, 10.0, 100.0, 1e3, 1e4] {
                let d = octaves.structure_function(finer, Metres::new(lag)).value();
                // 1 − J₀ is at most 1.403.
                assert!(d >= 0.0 && d <= 2.81 * variance, "{kind:?} {w} {lag}: {d}");
            }
        }
        let level = field.header().level().get();
        let cell = PatchKey::containing(level, [0.0, 0.6, 0.8]).unwrap();
        assert_eq!(
            structure_function(field, cell, Metres::new(1e4), Metres::new(50.0)),
            Some(octaves.structure_function(Metres::new(1e4), Metres::new(50.0)))
        );
    }
}

/// The height octave `k` alone adds at `dir`, at `level`.
fn octave_height(
    craters: &SmallCraters,
    k: u8,
    dir: [f64; 3],
    level: BandLevel,
    cache: &mut CraterCache,
) -> f64 {
    let figure = craters.octaves().inputs.figure;
    let band = band_half_width(&figure, level);
    let point = figure.point(dir);
    let octave = craters.octaves().octaves()[usize::from(k)];
    let home = PatchKey::containing(octave.level(), dir).unwrap();
    let (cells, count) = neighbourhood(home);
    let mut height = 0.0;
    for &cell in &cells[..count] {
        for crater in craters.craters_in(k, cell, cache) {
            let centre = crater.point_m();
            let chord = [0, 1, 2].map(|a| point[a] - centre[a]);
            let distance = dot(chord, chord).sqrt();
            if distance < crater.shape().reach().value() {
                let profile = CraterProfile::new(*crater.shape(), band, crater.diffusion());
                height += profile
                    .at(distance / (0.5 * crater.diameter().value()))
                    .height_m;
            }
        }
    }
    height
}

#[test]
fn an_octave_s_closed_form_variance_is_near_its_sampled_one() {
    // An open octave of the Moon, sampled at 6,000 points over a region of some 500 craters:
    // Campbell's theorem over the octave's sizes and ages holds the sampled variance within 1.5
    // either way (measured 1.05). A saturated octave's variance is carried by its few youngest
    // craters (on the Moon's octave 9, one crater of 31 Myr is half of its region's 3,000 craters'
    // ∫h² dA), so its sample is too noisy for the check, which the dense quadrature test makes
    // instead.
    let craters = small(SyntheticWorld::MoonLike);
    let figure = world(SyntheticWorld::MoonLike).header().figure();
    let level = BandLevel::new(finest_level(figure.equatorial_radius_m)).unwrap();
    let mut cache = CraterCache::new();
    for k in [3_u8] {
        let octave = craters.octaves().octaves()[usize::from(k)];
        assert!(!octave.is_saturated());
        let root = PatchKey::containing(octave.level() - 5, [0.31, -0.52, 0.79]).unwrap();
        let mut stream = SEED.stream(
            SURFACE_CRATER,
            ObjectKey::surface_cell(0, 0, 0, 0, 13).unwrap(),
        );
        let side = f64::from(1_u32 << root.level());
        let draws = 6_000;
        let mut total = 0.0;
        let mut squares = 0.0;
        for _ in 0..draws {
            let s = (f64::from(root.i()) + stream.uniform()) / side;
            let t = (f64::from(root.j()) + stream.uniform()) / side;
            let dir = area_density(root.face(), s, t, &figure).0;
            let h = octave_height(&craters, k, dir, level, &mut cache);
            total += h;
            squares += h * h;
        }
        let n = f64::from(draws);
        let sampled = squares / n - (total / n) * (total / n);
        let ratio = sampled / octave.variance().value();
        assert!(
            (0.67..=1.5).contains(&ratio),
            "octave {k}: sampled {sampled} m² against {:?}, ratio {ratio}",
            octave.variance()
        );
    }
}

/// The mean of g² over `octave`'s window for craters of rim radius `rim_m`, densely: Σ G(t) ΔN
/// over 8,000 panels of the window, geometric in t from 10⁻¹² of it, by the chronology's own
/// differences.
fn dense_mean_square_relief(rim_m: f64, octave: &CraterOctave, system: Gigayears) -> f64 {
    let tau = octave.window().value();
    let n = |t: f64| lunar_chronology(Gigayears::new(t), system).value();
    let panels = 8_000;
    let first = 1e-12 * tau;
    let ratio = math::powf(tau / first, 1.0 / f64::from(panels));
    let g2 = |t: f64| {
        let g = relief_factor(Metres::new(rim_m), diffusion_length(Gigayears::new(t)));
        g * g
    };
    let mut total = g2(0.5 * first) * n(first);
    let mut lo = first;
    for _ in 0..panels {
        let hi = num::min(lo * ratio, tau);
        total += g2((lo * hi).sqrt()) * (n(hi) - n(lo));
        lo = hi;
    }
    total / octave.window_density.value()
}

#[test]
fn the_closed_form_variance_is_its_dense_quadrature() {
    // The closed form's mean of g² over the ages against a dense sum, to 0.1%; and its octave
    // variance against 64 diameters drawn by the octave's own inversion at the midpoints of its
    // shares, each with that dense mean, to 2% (measured within 1.1%, the last, thinned octaves
    // the farthest). The same holds for the moments beside it (R09.T7.h): its ground variance, to
    // 2%; and its sharpening's variance and slopes at bands of a quarter and a half of its smallest
    // diameter, where the levels that keep it fillet it, to 4%. Every octave's sharpening was
    // measured within 0.4% but the widest, complex octaves' at half their smallest diameter, where
    // the fillet's inner end crosses the floor's edge as the diameter varies, a kink between the
    // eight nodes: 3.4% high on the Mars-like world's octave 0, 2.5% on the Ceres-like world's and
    // 1.1% low on the Moon-like world's octaves 0 and 1 (plan R09, Risks, "Deferred findings
    // (2026-10-13 rule)" of T7.h).
    for kind in [
        SyntheticWorld::MoonLike,
        SyntheticWorld::MarsLike,
        SyntheticWorld::CeresLike,
    ] {
        let header = world(kind).header();
        let params = header.craters();
        let system = params.system_age();
        let octaves = CraterOctaves::new(header);
        let all = octaves.octaves();
        for octave in [
            all[0],
            all[all.len() / 3],
            all[2 * all.len() / 3],
            all[all.len() - 1],
        ] {
            if octave.density().value() <= 0.0 {
                continue;
            }
            let bands = [0.25, 0.5].map(|f| Metres::new(f * octave.smallest().value()));
            let mut mean = 0.0;
            let mut ground_mean = 0.0;
            let mut sharpening = [(0.0, 0.0); 2];
            let mut kept = 0.0;
            for k in 0..64 {
                let midpoint = (f64::from(k) + 0.5) / 64.0;
                let d = diameter_in_octave(
                    params,
                    octave.diameters(),
                    UnitUniform::new(midpoint).unwrap(),
                );
                if d < octave.smallest() {
                    continue;
                }
                let rim = 0.5 * d.value();
                let closed_g2 = mean_square_relief(rim, &octave, system);
                let dense_g2 = dense_mean_square_relief(rim, &octave, system);
                assert!(
                    (closed_g2 / dense_g2 - 1.0).abs() < 1e-3,
                    "{kind:?} octave {}: {closed_g2} against {dense_g2}",
                    octave.index()
                );
                let shape = crater_shape(params, d);
                let fresh = CraterProfile::filleted(shape, Metres::ZERO).square_moment();
                mean += core::f64::consts::PI * rim * rim * fresh * dense_g2;
                let ground = CraterProfile::filleted(shape, GROUND_FILLET);
                ground_mean +=
                    core::f64::consts::PI * rim * rim * ground.square_moment() * dense_g2;
                for (sum, band) in sharpening.iter_mut().zip(bands) {
                    let (heights, slopes) = sharpening_moments(&ground, band);
                    sum.0 += core::f64::consts::PI * rim * rim * heights * dense_g2;
                    sum.1 += core::f64::consts::PI * 0.5 * slopes * dense_g2;
                }
                kept += 1.0;
            }
            let per_m2 = octave.density().value() * 1e-6 / kept;
            let at = format!("{kind:?} octave {}", octave.index());
            for (what, closed, dense) in [
                ("variance", octave.variance().value(), per_m2 * mean),
                (
                    "ground variance",
                    octave.ground_variance().value(),
                    per_m2 * ground_mean,
                ),
            ] {
                assert!(
                    (closed / dense - 1.0).abs() < 0.02,
                    "{at}'s {what}: {closed} against {dense}"
                );
            }
            let k = usize::from(octave.index());
            for (band, (heights, slopes)) in bands.into_iter().zip(sharpening) {
                let closed = octaves.sharpening(k, band);
                if heights <= 0.0 {
                    // The last octave's smallest craters keep the ground's fillet at both bands.
                    assert_same_bits(closed.variance_m2, 0.0);
                    continue;
                }
                for (what, closed, dense) in [
                    ("sharpening", closed.variance_m2, per_m2 * heights),
                    ("sharpening's slopes", closed.slope2, per_m2 * slopes),
                ] {
                    assert!(
                        (closed / dense - 1.0).abs() < 0.04,
                        "{at}'s {what} at a band of {band:?}: {closed} against {dense}"
                    );
                }
            }
        }
    }
}

/// ∫ f(x) 2x dx over [0, `reach`] by the midpoint rule on `panels` equal panels, blind to where a
/// profile's pieces meet: the radial brute force.
fn midpoint_radial(reach: f64, panels: u32, f: impl Fn(f64) -> f64) -> f64 {
    let h = reach / f64::from(panels);
    let mut total = 0.0;
    for k in 0..panels {
        let x = (f64::from(k) + 0.5) * h;
        total += f(x) * 2.0 * x;
    }
    total * h
}

/// One crater's sharpening, `ground` less `level`, two fillets of one shape, over the plane: its
/// ∫∫ h² dA, square metres, and ∫∫ (∂h ÷ ∂x)² dA along one axis, square metres, the slope by
/// central differences of the two profiles' heights, by the midpoint rule on a `side` × `side`
/// grid over the square that holds the crater's reach.
fn planar_moments(ground: &CraterProfile, level: &CraterProfile, side: u32) -> (f64, f64) {
    let shape = ground.shape();
    let rim_m = 0.5 * shape.diameter().value();
    let reach = 1.0 + shape.exterior_width();
    let h = |x: f64, y: f64| {
        let r = (x * x + y * y).sqrt();
        ground.at(r).height_m - level.at(r).height_m
    };
    let step = 2.0 * reach / f64::from(side);
    let eps = 1e-6;
    let (mut heights, mut slopes) = (0.0, 0.0);
    for i in 0..side {
        let x = -reach + (f64::from(i) + 0.5) * step;
        for j in 0..side {
            let y = -reach + (f64::from(j) + 0.5) * step;
            let here = h(x, y);
            // Metres per rim radius.
            let along = (h(x + eps, y) - h(x - eps, y)) / (2.0 * eps);
            heights += here * here;
            slopes += along * along;
        }
    }
    // A grid cell is step² rim radii², step² R² m²; a slope per metre is along ÷ R.
    (heights * step * step * rim_m * rim_m, slopes * step * step)
}

#[test]
fn one_crater_s_sharpening_is_its_two_profiles_sampled() {
    // One crater's sharpening moments, the ground's fillet against a level's, against brute force
    // over the two profiles. Radially, by the midpoint rule on 2 × 10⁵ panels blind to their
    // pieces: to 10⁻⁶ in the heights, and to 10⁻⁴ in the slopes, whose difference jumps where a
    // level's fillet passes a floor's or a peak's edge. Over the plane, on a 400 × 400 grid with
    // the slope along one axis by central differences, so that half the radial slope moment is a
    // random direction's: to 5 × 10⁻³, the grid's own error on a fillet 16 cells wide (measured
    // 2.2 × 10⁻³ at worst; R09.T7.h).
    let params = *world(SyntheticWorld::MoonLike).header().craters();
    let mut sharpened = 0;
    // Each diameter and band, and whether the plane is sampled there too.
    for (d, plane_d) in [(2.5, false), (63.0, true), (1e3, true), (25e3, false)] {
        let shape = crater_shape(&params, Metres::new(d));
        let rim_m = 0.5 * d;
        let reach = 1.0 + shape.exterior_width();
        let ground = CraterProfile::filleted(shape, GROUND_FILLET);
        for (band_over_rim, plane_band) in [(0.02, false), (0.1, true), (0.5, true), (3.0, false)] {
            let band = Metres::new(band_over_rim * rim_m);
            let level = CraterProfile::filleted(shape, band);
            let (heights, slopes) = sharpening_moments(&ground, band);
            let at = format!("{d} m filleted over {band_over_rim} rim radii");
            if level.fillet <= ground.fillet {
                assert_same_bits(heights, 0.0);
                assert_same_bits(slopes, 0.0);
                continue;
            }
            let dh = |x: f64| ground.at(x).height_m - level.at(x).height_m;
            let ds =
                |x: f64| ground.at(x).slope_m_per_rim_radius - level.at(x).slope_m_per_rim_radius;
            let radial_h = midpoint_radial(reach, 200_000, |x| dh(x) * dh(x));
            let radial_s = midpoint_radial(reach, 200_000, |x| ds(x) * ds(x));
            eprintln!(
                "{at}: radial ratios {:.3e} {:.3e}",
                heights / radial_h - 1.0,
                slopes / radial_s - 1.0
            );
            assert!(
                (heights / radial_h - 1.0).abs() < 1e-6,
                "{at}: {heights} against {radial_h}"
            );
            assert!(
                (slopes / radial_s - 1.0).abs() < 1e-4,
                "{at}: {slopes} against {radial_s}"
            );
            if plane_d && plane_band {
                let (area_h, area_s) = planar_moments(&ground, &level, 400);
                let closed_h = core::f64::consts::PI * rim_m * rim_m * heights;
                let closed_s = core::f64::consts::PI * 0.5 * slopes;
                eprintln!(
                    "{at}: planar ratios {:.3e} {:.3e}",
                    closed_h / area_h - 1.0,
                    closed_s / area_s - 1.0
                );
                assert!(
                    (closed_h / area_h - 1.0).abs() < 5e-3,
                    "{at}: {closed_h} m⁴ against {area_h}"
                );
                assert!(
                    (closed_s / area_s - 1.0).abs() < 5e-3,
                    "{at}: {closed_s} m² against {area_s}"
                );
            }
            sharpened += 1;
        }
    }
    assert!(sharpened >= 10, "{sharpened} sharpened cases");
}

#[test]
fn an_octave_s_sharpening_grows_with_its_band_to_no_more_than_its_ground_variance() {
    // Zero to the bit at the ground's fillet and finer; then non-decreasing as the band widens, and
    // never above the octave's ground variance, out past its widest crater's rim radius, where every
    // fillet is held at min(1, a) rim radii: the condition that keeps the wavelength-keyed variance
    // non-decreasing in the wavelength (decision-r10-t5-closed-forms.md §2).
    for kind in WORLDS {
        let octaves = CraterOctaves::new(world(kind).header());
        for (k, octave) in octaves.octaves().iter().enumerate() {
            let ground = octave.ground_variance().value();
            for band in [0.0, 0.5, 1.0] {
                let none = octaves.sharpening(k, Metres::new(band));
                assert_same_bits(none.variance_m2, 0.0);
                assert_same_bits(none.slope2, 0.0);
            }
            let mut previous = 0.0;
            let mut band = 1.0;
            while band < octave.diameters().upper().value() {
                let s = octaves.sharpening(k, Metres::new(band));
                let at = format!("{kind:?} octave {k} at a band of {band} m");
                assert!(
                    s.variance_m2 >= previous,
                    "{at}: {} after {previous}",
                    s.variance_m2
                );
                assert!(
                    s.variance_m2 <= ground,
                    "{at}: {} over {ground}",
                    s.variance_m2
                );
                assert!(
                    s.slope2 >= 0.0 && s.slope2.is_finite(),
                    "{at}: {}",
                    s.slope2
                );
                previous = s.variance_m2;
                band *= 1.25;
            }
        }
    }
}

/// `kind`'s band limit's level: the shallowest whose shortest wavelength is 2 m.
fn band_limit_level(kind: SyntheticWorld) -> u8 {
    let figure = world(kind).header().figure();
    (0..=MAX_LEVEL)
        .find(|&n| {
            crate::synth::levels::shortest_wavelength(&figure, BandLevel::new(n).unwrap()).value()
                <= BAND_LIMIT_M
        })
        .unwrap()
}

#[test]
fn both_forms_are_their_octaves_sharpening_and_ground_and_vanish_at_the_band_limit() {
    // The level-keyed form is Σ Φ_k(w_n) over the octaves the level keeps and Σ G_k over the rest,
    // zero from the band limit's level; the wavelength-keyed one is Σ [s_k G_k + (1 − s_k)
    // Φ_k(max(½ λ, 1 m))], zero at 2 m and below; and their structure functions take the rims' form
    // for what is kept and the J₀ ring for what is left out (R09.T7.h).
    for kind in WORLDS {
        let header = world(kind).header();
        let figure = header.figure();
        let octaves = CraterOctaves::new(header);
        let bounds = crate::synth::LevelBounds::of(header, &octaves);
        let limit = band_limit_level(kind);
        let all = octaves.octaves();
        let ring = |o: &CraterOctave, b: f64| {
            2.0 * o.ground_variance().value()
                * (1.0 - math::j0(2.0 * core::f64::consts::PI * b / octave_wavelength(o)))
        };
        let rims = |s: Sharpening, b: f64| {
            if s.variance_m2 > 0.0 {
                2.0 * s.variance_m2 * -math::exp_m1(-s.slope2 * b * b / (2.0 * s.variance_m2))
            } else {
                0.0
            }
        };
        for n in 0..=MAX_LEVEL {
            let level = BandLevel::new(n).unwrap();
            let set = bounds.set(level);
            let band = band_half_width(&figure, level);
            let lags = [0.0, 2.0, 5.0, set.shortest_wavelength().value(), 1e4];
            let (mut variance, mut structure) = (0.0, [0.0; 5]);
            for (k, octave) in all.iter().enumerate() {
                if set.keeps_crater_octave(octave.index()) {
                    let s = octaves.sharpening(k, band);
                    variance += s.variance_m2;
                    for (d, &b) in structure.iter_mut().zip(&lags) {
                        *d += rims(s, b);
                    }
                } else {
                    variance += octave.ground_variance().value();
                    for (d, &b) in structure.iter_mut().zip(&lags) {
                        *d += ring(octave, b);
                    }
                }
            }
            let at = format!("{kind:?} at level {n}");
            assert_same_bits(octaves.omitted_variance(set).value(), variance);
            for (&b, d) in lags.iter().zip(structure) {
                let got = octaves
                    .omitted_structure_function(set, Metres::new(b))
                    .value();
                assert!(
                    (got - d).abs() <= 1e-12 * d.abs(),
                    "{at} at a lag of {b} m: {got} against {d}"
                );
            }
            if n >= limit {
                assert_same_bits(variance, 0.0);
                assert_same_bits(
                    octaves
                        .omitted_structure_function(set, Metres::new(5.0))
                        .value(),
                    0.0,
                );
            }
        }
        for lambda in [3e4, 1e3, 100.0, 10.0, 3.0, 2.0, 1.0] {
            let band = Metres::new(num::max(0.5 * lambda, 1.0));
            let lags = [2.0, 50.0, 1e3];
            let (mut variance, mut structure) = (0.0, [0.0; 3]);
            for (k, octave) in all.iter().enumerate() {
                // An octave whose diameters straddle λ is part kept and part left out.
                let share = band_share(lambda, octave);
                let s = octaves.sharpening(k, band);
                if share > 0.0 {
                    variance += share * octave.ground_variance().value();
                }
                if share < 1.0 {
                    variance += (1.0 - share) * s.variance_m2;
                }
                for (d, &b) in structure.iter_mut().zip(&lags) {
                    *d += share * ring(octave, b) + (1.0 - share) * rims(s, b);
                }
            }
            let got = octaves.unresolved_variance(Metres::new(lambda)).value();
            assert_same_bits(got, variance);
            for (&b, d) in lags.iter().zip(structure) {
                let got = octaves
                    .structure_function(Metres::new(lambda), Metres::new(b))
                    .value();
                assert!(
                    (got - d).abs() <= 1e-12 * d.abs(),
                    "{kind:?} finer than {lambda} m at a lag of {b} m: {got} against {d}"
                );
            }
            if lambda <= BAND_LIMIT_M {
                assert_same_bits(got, 0.0);
                for b in [2.0, 100.0] {
                    let d = octaves.structure_function(Metres::new(lambda), Metres::new(b));
                    assert_same_bits(d.value(), 0.0);
                }
            }
        }
    }
}

/// The realised small craters' part of what a level leaves out of the finest level's ground,
/// against the closed forms: at the levels of a 100 m and a 1 km survey on each cratered world,
/// the octaves the level keeps, sharpened from its band to the band limit's, and the octaves it
/// leaves out, over 400 cells × 25 points (R09.T7.h's recorded measurement; release:
/// `--release` or `--profile slow-test`, `--ignored --nocapture`).
#[test]
#[ignore = "measurement, by hand: cargo test -p hyperion-surface --profile slow-test --lib synth::craters::tests::measure_the_kept_octaves_sharpening_and_the_octaves_left_out -- --ignored --nocapture"]
fn measure_the_kept_octaves_sharpening_and_the_octaves_left_out() {
    for kind in WORLDS {
        let header = world(kind).header();
        let figure = header.figure();
        let craters = small(kind);
        let octaves = craters.octaves();
        let bounds = crate::synth::LevelBounds::of(header, octaves);
        let limit = BandLevel::new(band_limit_level(kind)).unwrap();
        let every = bounds
            .set(limit)
            .craters_through()
            .expect("the band limit keeps every octave");
        let coarse = header.level().get();
        for resolution in [100.0, 1e3] {
            // The deepest level whose shortest wavelength is the resolution or longer.
            let n = (0..=MAX_LEVEL)
                .rev()
                .find(|&n| {
                    bounds
                        .set(BandLevel::new(n).unwrap())
                        .shortest_wavelength()
                        .value()
                        >= resolution
                })
                .unwrap();
            let level = BandLevel::new(n).unwrap();
            let set = bounds.set(level);
            let band = band_half_width(&figure, level);
            let kept = set.craters_through();
            let (mut closed_kept, mut closed_left) = (0.0, 0.0);
            for (k, octave) in octaves.octaves().iter().enumerate() {
                if set.keeps_crater_octave(octave.index()) {
                    closed_kept += octaves.sharpening(k, band).variance_m2;
                } else {
                    closed_left += octave.ground_variance().value();
                }
            }
            let mut rng = Lcg::new(0x7368_6172_7000 + u64::from(n));
            let mut cache = CraterCache::new();
            // Σ x, Σ x² and Σ x⁴ of the sharpening and of the octaves left out.
            let mut sums = [[0.0; 3]; 2];
            let mut points = 0.0;
            for cell in random_cells(coarse, 400, &mut rng) {
                for dir in random_points_in(cell, 25, &mut rng) {
                    let sharp =
                        kept.map_or(0.0, |k| craters.at(dir, limit, k, &mut cache).height_m);
                    let blunt =
                        kept.map_or(0.0, |k| craters.at(dir, level, k, &mut cache).height_m);
                    let whole = craters.at(dir, limit, every, &mut cache).height_m;
                    for (sum, x) in sums.iter_mut().zip([sharp - blunt, whole - sharp]) {
                        sum[0] += x;
                        sum[1] += x * x;
                        sum[2] += x * x * x * x;
                    }
                    points += 1.0;
                }
            }
            eprintln!(
                "{kind:?} at {resolution} m: level {n}, λ_n {:.2} m, band {:.2} m, octaves kept \
                 through {kept:?} of {every}, {points} points",
                set.shortest_wavelength().value(),
                band.value()
            );
            for (what, sum, closed) in [
                ("kept octaves' sharpening", sums[0], closed_kept),
                ("octaves left out", sums[1], closed_left),
            ] {
                let mean_square = sum[1] / points;
                // The standard error of the mean square, from the fourth moment.
                let error = ((sum[2] / points - mean_square * mean_square) / points).sqrt();
                eprintln!(
                    "  {what:25} realised {mean_square:.4} ± {error:.4} m² (mean {:.4} m), \
                     closed {closed:.4} m², realised ÷ closed {:.3}",
                    sum[0] / points,
                    mean_square / closed
                );
            }
        }
    }
}
