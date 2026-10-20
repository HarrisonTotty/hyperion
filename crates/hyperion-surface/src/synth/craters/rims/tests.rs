//! The coarse craters' rims' tests (plan R09, T7.c): the coarse band, nothing added at and beyond
//! it, the sharpening's volume balance over its reach at every level and on the ground of a
//! sphere, its continuity across the coarse cells' edges and the faces', its gradient, and its
//! refusal of ground the field does not hold.

use std::sync::OnceLock;

use hyperion_base::math;
use hyperion_testkit::float::assert_same_bits;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::super::tests::{WORLDS, dot, quadrature, tangents, world};
use super::*;
use crate::cube::{Edge, MAX_LEVEL, PATCH_QUADS, unit_dir};
use crate::field::{CoarseField, PartialField, cell_at_index};
use crate::geometry::finest_level;
use crate::testing::{SyntheticWorld, synthetic_field};

/// The single crater of 300 km on the edge between faces 0 and 2 of a flat sphere at level 6.
fn one_crater() -> &'static CoarseField {
    static FIELD: OnceLock<CoarseField> = OnceLock::new();
    FIELD.get_or_init(|| synthetic_field(SyntheticWorld::OneCrater))
}

/// Every field with coarse craters: the four cratered worlds and the single crater.
fn fields() -> Vec<&'static CoarseField> {
    let mut all: Vec<&'static CoarseField> = WORLDS.iter().map(|&k| world(k)).collect();
    all.push(one_crater());
    all
}

/// Every band level.
fn levels() -> impl Iterator<Item = BandLevel> {
    (0..=MAX_LEVEL).map(|n| BandLevel::new(n).unwrap())
}

/// The unit direction `psi` radians from the unit direction `d` along the great circle towards
/// `azimuth`, for any `psi` from 0 to π: cos ψ d + sin ψ t.
fn geodesic(d: [f64; 3], psi: f64, azimuth: f64) -> [f64; 3] {
    let (e1, e2) = tangents(d);
    let (s, c) = math::sin_cos(azimuth);
    let (sin_psi, cos_psi) = math::sin_cos(psi);
    unit_dir([0, 1, 2].map(|k| cos_psi * d[k] + sin_psi * (c * e1[k] + s * e2[k])))
}

/// The direction `x` rim radii from `crater`'s centre towards `azimuth`, on a body of volumetric
/// radius `radius_m`: an arc of the chord's length, near enough for a sample point.
fn near(crater: &CoarseCrater, x: f64, azimuth: f64, radius_m: f64) -> [f64; 3] {
    let rim_m = 0.5 * crater.diameter.value();
    geodesic(crater.centre, x * rim_m / radius_m, azimuth)
}

fn sample_bits(sample: HeightSample) -> [u64; 4] {
    let [x, y, z] = sample.gradient.map(hyperion_testkit::float::bits);
    [hyperion_testkit::float::bits(sample.height_m), x, y, z]
}

#[test]
fn the_coarse_band_is_the_band_of_the_patch_level_whose_vertex_spacing_is_the_coarse_cell_edge() {
    for field in fields() {
        let figure = field.header().figure();
        for l in CoarseLevel::MIN.get()..=CoarseLevel::MAX.get() {
            let coarse = coarse_band_half_width(&figure, CoarseLevel::new(l).unwrap()).value();
            if l >= 6 {
                let patch = band_half_width(&figure, BandLevel::new(l - 6).unwrap());
                assert_same_bits(coarse, patch.value());
            } else {
                // Level 5's cells are twice level 0's patch vertices apart.
                let patch = band_half_width(&figure, BandLevel::new(0).unwrap());
                assert_same_bits(coarse, 2.0 * patch.value());
            }
        }
        // The rims take their own field's level.
        let rims = CoarseRims::new(field.header());
        assert_same_bits(
            rims.coarse_band().value(),
            coarse_band_half_width(&figure, field.header().level()).value(),
        );
    }
    // On a sphere it is the boundary diameter, 92.6 km on the Moon at level 6.
    let header = one_crater().header();
    let coarse = CoarseRims::new(header).coarse_band().value();
    let d_b = header.boundary_diameter().value();
    assert!(
        (coarse / d_b - 1.0).abs() < 1e-12,
        "{coarse} m against {d_b} m"
    );
    assert!((coarse - 92.6e3).abs() < 0.1e3, "{coarse} m");
}

#[test]
fn the_sharpening_is_the_level_s_profile_less_the_coarse_one_times_the_kept_relief() {
    let mut checked = 0;
    for field in fields() {
        let header = field.header();
        let rims = CoarseRims::new(header);
        let coarse_band = rims.coarse_band();
        for crater in field.craters() {
            let g = kept_relief(crater);
            assert_same_bits(g, 1.0 - f64::from(crater.degradation) / 255.0);
            let shape = crater_shape(header.craters(), crater.diameter);
            assert_eq!(
                rims.coarse_profile(crater.diameter),
                CraterProfile::filleted(shape, coarse_band)
            );
            for level in levels() {
                let band = band_half_width(&header.figure(), level);
                let s = rims.sharpening(crater, level);
                assert_eq!(s, RimSharpening::new(shape, band, coarse_band, g));
                assert_same_bits(s.relief(), g);
                assert_eq!(*s.coarse(), rims.coarse_profile(crater.diameter));
                let held = Metres::new(num::min(band.value(), coarse_band.value()));
                assert_eq!(*s.fine(), CraterProfile::filleted(shape, held));
                for k in 0..=40_u8 {
                    let x = f64::from(k) * 0.065;
                    let got = s.at(x);
                    let (fine, coarse) = (s.fine().at(x), s.coarse().at(x));
                    if s.is_zero() {
                        assert_same_bits(got.height_m, 0.0);
                        assert_same_bits(got.slope_m_per_rim_radius, 0.0);
                    } else {
                        assert_same_bits(got.height_m, g * (fine.height_m - coarse.height_m));
                        assert_same_bits(
                            got.slope_m_per_rim_radius,
                            g * (fine.slope_m_per_rim_radius - coarse.slope_m_per_rim_radius),
                        );
                    }
                    // The coarse elevation's g × F(w_c) and the sharpening make the level's g ×
                    // F(w′) (R09.T12.d's contract), to rounding.
                    let whole = g * coarse.height_m + got.height_m;
                    let scale = shape.depth().value();
                    assert!(
                        (whole - g * fine.height_m).abs() <= 1e-12 * scale,
                        "{whole} m against {} m",
                        g * fine.height_m
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 50_000, "{checked} samples");
}

#[test]
fn the_sharpening_adds_nothing_at_and_beyond_the_coarse_band() {
    let mut zero_levels = 0;
    let mut sharpened = 0;
    for field in fields() {
        let header = field.header();
        let figure = header.figure();
        let radius_m = figure.volumetric_radius_m();
        let rims = CoarseRims::new(header);
        let coarse_band = rims.coarse_band();
        for crater in field.craters().iter().take(12) {
            let shape = crater_shape(header.craters(), crater.diameter);
            // At the coarse band itself and beyond it, and with no relief left.
            for w in [1.0, 1.5, 100.0] {
                let band = Metres::new(w * coarse_band.value());
                assert!(RimSharpening::new(shape, band, coarse_band, 1.0).is_zero());
            }
            assert!(RimSharpening::new(shape, Metres::new(1.0), coarse_band, 0.0).is_zero());
            for level in levels() {
                let s = rims.sharpening(crater, level);
                let coarser = band_half_width(&figure, level).value() >= coarse_band.value();
                if coarser {
                    assert!(s.is_zero(), "{level:?} of {crater:?}");
                    zero_levels += 1;
                    // Nothing at all, to the bit, from any crater the point reaches.
                    for (k, x) in [0.0, 0.3, 0.97, 1.0, 1.04, 1.8, 2.4]
                        .into_iter()
                        .enumerate()
                    {
                        let dir = near(
                            crater,
                            x,
                            0.7 * f64::from(u8::try_from(k).unwrap()),
                            radius_m,
                        );
                        let sample = rims.at(field, dir, level).unwrap();
                        assert_eq!(sample_bits(sample), [0; 4], "{level:?} at {x}");
                    }
                } else if !s.is_zero() {
                    sharpened += 1;
                    let most = (0..=50_u8)
                        .map(|k| s.at(0.05 * f64::from(k)).height_m.abs())
                        .fold(0.0, num::max);
                    assert!(most > 0.0, "{level:?} of {crater:?} adds nothing");
                }
            }
        }
    }
    assert!(
        zero_levels > 40,
        "{zero_levels} levels at or beyond the coarse band"
    );
    assert!(sharpened > 500, "{sharpened} sharpenings");
    // A crater too narrow for either band to fillet less than its rim allows adds nothing: at
    // D_b the coarse fillet is held at min(1, a) rim radii, and so is a band of a rim radius.
    let header = one_crater().header();
    let shape = crater_shape(header.craters(), header.boundary_diameter());
    let coarse_band = CoarseRims::new(header).coarse_band();
    let rim = header.boundary_diameter() * 0.5;
    assert!(RimSharpening::new(shape, rim, coarse_band, 1.0).is_zero());
    assert!(!RimSharpening::new(shape, rim * 0.5, coarse_band, 1.0).is_zero());
}

/// The sharpening's volume over its reach, cubic metres: ∫ h 2x dx in rim radii times π R², by
/// the independent five-point rule on every piece of both its profiles.
fn volume_m3(s: &RimSharpening) -> f64 {
    let mut points: Vec<f64> = s
        .fine()
        .breakpoints()
        .into_iter()
        .chain(s.coarse().breakpoints())
        .collect();
    points.sort_by(f64::total_cmp);
    let mut volume = 0.0;
    for pair in points.windows(2) {
        if pair[1] > pair[0] {
            volume += quadrature(pair[0], pair[1], |x| s.height_and_slope(x).0);
        }
    }
    let rim_m = 0.5 * s.coarse().shape().diameter().value();
    core::f64::consts::PI * rim_m * rim_m * volume
}

#[test]
fn every_coarse_crater_s_sharpening_integrates_to_zero_over_its_reach_at_every_level() {
    let mut balanced = 0;
    for field in fields() {
        let header = field.header();
        let rims = CoarseRims::new(header);
        let d_b = header.boundary_diameter().value();
        // The field's own craters, and a span of diameters from D_b with no wear and with some.
        let mut cases: Vec<(CraterShape, f64)> = field
            .craters()
            .iter()
            .map(|c| (crater_shape(header.craters(), c.diameter), kept_relief(c)))
            .collect();
        for ratio in [1.0, 1.2, 1.5, 2.0, 3.0, 4.0, 8.0, 16.0, 32.0] {
            let shape = crater_shape(header.craters(), Metres::new(ratio * d_b));
            cases.push((shape, 1.0));
            cases.push((shape, 0.6));
        }
        for (shape, g) in cases {
            let reach = 1.0 + shape.exterior_width();
            for level in levels() {
                let band = band_half_width(&header.figure(), level);
                let s = RimSharpening::new(shape, band, rims.coarse_band(), g);
                let volume = volume_m3(&s);
                assert!(
                    volume.abs() <= 1e-6 * shape.cavity_volume_m3(),
                    "{shape:?} at {level:?}: {volume} m³ of {}",
                    shape.cavity_volume_m3()
                );
                for x in [reach, 1.5 * reach] {
                    let at = s.at(x);
                    assert_same_bits(at.height_m, 0.0);
                    assert_same_bits(at.slope_m_per_rim_radius, 0.0);
                }
                if !s.is_zero() {
                    balanced += 1;
                }
            }
        }
    }
    assert!(balanced > 2_000, "{balanced} sharpenings balanced");
}

/// The sharpening of every coarse crater of `field` at `dir` and `level`, from the whole list in
/// list order rather than the field's index: what [`CoarseRims::at`] must give.
fn from_the_whole_list(field: &CoarseField, dir: [f64; 3], level: BandLevel) -> HeightSample {
    let rims = CoarseRims::new(field.header());
    let figure = field.header().figure();
    let p = figure.point(dir);
    let mut height = 0.0;
    let mut gradient = [0.0; 3];
    if band_half_width(&figure, level).value() < rims.coarse_band().value() {
        for crater in field.craters() {
            let s = rims.sharpening(crater, level);
            let centre = figure.point(crater.centre);
            let delta = [0, 1, 2].map(|k| p[k] - centre[k]);
            let d2 = dot(delta, delta);
            let reach = s.coarse().shape().reach().value();
            if d2 >= reach * reach || s.is_zero() {
                continue;
            }
            let distance = d2.sqrt();
            let rim_m = 0.5 * crater.diameter.value();
            let (h, slope) = s.height_and_slope(distance / rim_m);
            height += h;
            if distance > 0.0 {
                let along = slope / (rim_m * distance);
                for (g, d) in gradient.iter_mut().zip(delta) {
                    *g += along * d;
                }
            }
        }
    }
    HeightSample {
        height_m: height,
        gradient: direction_gradient(&figure, dir, gradient),
    }
}

#[test]
fn a_coarse_crater_s_sharpening_is_continuous_across_the_coarse_cell_edges_it_spans() {
    let mut on_edges = 0;
    let mut across_faces = 0;
    for field in [
        one_crater(),
        world(SyntheticWorld::MoonLike),
        world(SyntheticWorld::CeresLike),
    ] {
        let header = field.header();
        let rims = CoarseRims::new(header);
        let level_l = header.level().get();
        let finest = finest_level(header.figure().equatorial_radius_m);
        let sharpening_levels =
            [level_l - 4, level_l + 2, finest].map(|n| BandLevel::new(n).unwrap());
        for crater in field.craters().iter().take(4) {
            let cells: Vec<u32> = crater.reach.cells().collect();
            let step = cells.len().div_ceil(40);
            for &index in cells.iter().step_by(step) {
                let cell = cell_at_index(level_l, index).unwrap();
                for edge in Edge::ALL {
                    let neighbour = cell.edge_neighbour(edge);
                    for k in (0..=PATCH_QUADS).step_by(8) {
                        let k = u8::try_from(k).unwrap();
                        let last = u8::try_from(PATCH_QUADS).unwrap();
                        let (x, y) = match edge {
                            Edge::UMin => (0, k),
                            Edge::UMax => (last, k),
                            Edge::VMin => (k, 0),
                            Edge::VMax => (k, last),
                        };
                        // The edge's own direction, the same from either cell.
                        let dir = cell.vertex_dir(x, y);
                        for level in sharpening_levels {
                            let here = rims.sum_in(field, cell, dir, level);
                            let there = rims.sum_in(field, neighbour, dir, level);
                            assert_eq!(
                                sample_bits(here),
                                sample_bits(there),
                                "{cell:?} and {neighbour:?} at {dir:?}"
                            );
                            assert_eq!(
                                sample_bits(here),
                                sample_bits(from_the_whole_list(field, dir, level)),
                                "the index of {cell:?} misses a crater at {dir:?}"
                            );
                            if here.height_m.abs() > 1.0 {
                                on_edges += 1;
                                if cell.face() != neighbour.face() {
                                    across_faces += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        on_edges > 2_000,
        "{on_edges} sharpened samples on cell edges"
    );
    assert!(
        across_faces > 20,
        "{across_faces} sharpened samples on face edges"
    );
}

#[test]
fn a_query_sums_the_craters_reaching_it_in_list_order() {
    for field in [
        world(SyntheticWorld::MoonLike),
        world(SyntheticWorld::MarsLike),
        one_crater(),
    ] {
        let header = field.header();
        let rims = CoarseRims::new(header);
        let radius_m = header.figure().volumetric_radius_m();
        let level = BandLevel::new(header.level().get() + 3).unwrap();
        // Samples across each crater's reach, and across its coarse fillet's span, where its
        // sharpening lies (R09.T7.i), so that most of the latter are sharpened.
        let (mut sharpened, mut samples) = ([0, 0], [0, 0]);
        for crater in field.craters().iter().take(8) {
            let w = rims.sharpening(crater, level).coarse().fillet;
            for k in 0..24_u8 {
                let across = 0.11 * f64::from(k);
                let on_span = 1.0 - w + 2.0 * w * (f64::from(k) + 0.5) / 24.0;
                for (n, x) in [across, on_span].into_iter().enumerate() {
                    let dir = near(crater, x, 0.37 * f64::from(k), radius_m);
                    let sample = rims.at(field, dir, level).unwrap();
                    assert_eq!(
                        sample_bits(sample),
                        sample_bits(from_the_whole_list(field, dir, level))
                    );
                    samples[n] += 1;
                    if sample.height_m.abs() > 1.0 {
                        sharpened[n] += 1;
                    }
                }
            }
        }
        assert!(
            sharpened[0] > 0 && 2 * sharpened[1] > samples[1],
            "{sharpened:?} of {samples:?} samples sharpened, across the reaches and on the spans"
        );
    }
}

#[test]
fn on_a_sphere_the_sharpening_integrates_to_zero_over_the_ground_it_touches() {
    // On a sphere the ground within a chord c of a point is π c² (Archimedes), so a profile that
    // balances in the chord balances on the ground: ∫∫ h(c(ψ)) ρ² sin ψ dψ dφ about the crater's
    // centre, in geodesic polar coordinates, through the field's query.
    let field = one_crater();
    let header = field.header();
    let figure = header.figure();
    let rho = figure.equatorial_radius_m;
    assert_same_bits(figure.polar_radius_m, rho);
    let rims = CoarseRims::new(header);
    let crater = &field.craters()[0];
    let rim_m = 0.5 * crater.diameter.value();
    let finest = finest_level(rho);
    let azimuths = 8_u8;
    for n in [1, 3, 6, 10, 14, finest] {
        let level = BandLevel::new(n).unwrap();
        let s = rims.sharpening(crater, level);
        assert!(!s.is_zero(), "level {n}");
        let mut chords: Vec<f64> = s
            .fine()
            .breakpoints()
            .into_iter()
            .chain(s.coarse().breakpoints())
            .map(|x| x * rim_m)
            .collect();
        chords.sort_by(f64::total_cmp);
        let angles: Vec<f64> = chords
            .iter()
            .map(|&c| 2.0 * math::asin(0.5 * c / rho))
            .collect();
        let mut volume = 0.0;
        let mut magnitude = 0.0;
        for j in 0..azimuths {
            let azimuth = 2.0 * core::f64::consts::PI * (f64::from(j) + 0.5) / f64::from(azimuths);
            let mut ray = 0.0;
            let mut ray_magnitude = 0.0;
            for pair in angles.windows(2) {
                if pair[1] <= pair[0] {
                    continue;
                }
                let panels = 16_u8;
                let width = (pair[1] - pair[0]) / f64::from(panels);
                for p in 0..panels {
                    let mid = pair[0] + width * (f64::from(p) + 0.5);
                    for (node, weight) in super::super::tests::GAUSS_LEGENDRE_5 {
                        let psi = mid + 0.5 * width * node;
                        let dir = geodesic(crater.centre, psi, azimuth);
                        let h = rims.at(field, dir, level).unwrap().height_m;
                        let area = rho * rho * math::sin(psi);
                        ray += 0.5 * width * weight * h * area;
                        ray_magnitude += 0.5 * width * weight * h.abs() * area;
                    }
                }
            }
            volume += ray * 2.0 * core::f64::consts::PI / f64::from(azimuths);
            magnitude += ray_magnitude * 2.0 * core::f64::consts::PI / f64::from(azimuths);
        }
        let cavity = s.coarse().shape().cavity_volume_m3();
        assert!(
            volume.abs() <= 1e-6 * cavity,
            "level {n}: {volume} m³ of {cavity} m³ (|h| integrates to {magnitude} m³)"
        );
        assert!(magnitude > 1e-3 * cavity, "level {n}: {magnitude} m³");
    }
}

#[test]
fn the_sharpening_s_gradient_is_its_derivative_along_the_surface() {
    let mut sloped = 0;
    for field in [
        world(SyntheticWorld::MoonLike),
        world(SyntheticWorld::CeresLike),
        one_crater(),
    ] {
        let header = field.header();
        let figure = header.figure();
        let radius_m = figure.volumetric_radius_m();
        let rims = CoarseRims::new(header);
        for n in [header.level().get() - 2, header.level().get() + 4] {
            let level = BandLevel::new(n).unwrap();
            for crater in field.craters().iter().take(6) {
                for k in 0..10_u8 {
                    // Off the profiles' breakpoints, which a millimetre step might straddle.
                    let x = 0.053 + 0.247 * f64::from(k);
                    let dir = near(crater, x, 1.3 * f64::from(k), radius_m);
                    let sample = rims.at(field, dir, level).unwrap();
                    let (e1, e2) = tangents(dir);
                    for e in [e1, e2] {
                        // A step of 10 cm along the surface: the chord's rounding, about a
                        // nanometre at a body's radius, is then no part of the difference.
                        let eps = 0.1 / radius_m;
                        let moved =
                            |sign: f64| unit_dir([0, 1, 2].map(|i| dir[i] + sign * eps * e[i]));
                        let up = rims.at(field, moved(1.0), level).unwrap().height_m;
                        let down = rims.at(field, moved(-1.0), level).unwrap().height_m;
                        let numeric = (up - down) / (2.0 * eps);
                        let analytic = dot(sample.gradient, figure.point(e));
                        assert!(
                            (numeric - analytic).abs() <= 1e-4 * analytic.abs().max(1e-2),
                            "at {dir:?}, level {n}: {analytic} against {numeric}"
                        );
                        if analytic.abs() > 1e-2 {
                            sloped += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(sloped > 200, "only {sloped} sloped samples");
}

#[test]
fn a_query_on_ground_the_field_does_not_hold_is_refused() {
    let field = one_crater();
    let header = field.header();
    let rims = CoarseRims::new(header);
    let crater = &field.craters()[0];
    let level = BandLevel::new(10).unwrap();
    let empty = PartialField::new(header.clone());
    let cell = PatchKey::containing(header.level().get(), crater.centre).unwrap();
    assert_eq!(
        rims.at(&empty, crater.centre, level),
        Err(QueryHeightError::NotSurveyed(cell))
    );
    assert!(rims.at(field, crater.centre, level).is_ok());
}

/// The ground integral of `crater`'s sharpening at `level` over its reach on `field`'s figure,
/// cubic metres, and of its magnitude: in geodesic polar coordinates (ψ, φ) on the unit sphere
/// of directions about its centre, with the spheroid's area element a² c |M⁻¹d| sin ψ, each ray
/// split where its chord crosses a breakpoint of either profile (found by bisection), five-point
/// Gauss–Legendre on 8 panels a piece and `azimuths` azimuths.
fn ground_integral(
    field: &CoarseField,
    crater: &CoarseCrater,
    level: BandLevel,
    azimuths: u16,
) -> (f64, f64) {
    let figure = field.header().figure();
    let (equatorial_m, polar_m) = (figure.equatorial_radius_m, figure.polar_radius_m);
    let rims = CoarseRims::new(field.header());
    let sharpening = rims.sharpening(crater, level);
    let rim_m = 0.5 * crater.diameter.value();
    let centre = figure.point(crater.centre);
    let chord = |dir: [f64; 3]| {
        let point = figure.point(dir);
        let delta = [0, 1, 2].map(|k| point[k] - centre[k]);
        dot(delta, delta).sqrt()
    };
    let mut breakpoints: Vec<f64> = sharpening
        .fine()
        .breakpoints()
        .into_iter()
        .chain(sharpening.coarse().breakpoints())
        .map(|x| x * rim_m)
        .collect();
    breakpoints.sort_by(f64::total_cmp);
    let reach_m = *breakpoints.last().unwrap();
    let (mut volume, mut magnitude) = (0.0, 0.0);
    for j in 0..azimuths {
        let azimuth = 2.0 * core::f64::consts::PI * (f64::from(j) + 0.5) / f64::from(azimuths);
        // The angle at which the ray's chord reaches `target`, by bisection from a bracket.
        let angle_at = |target: f64| {
            let (mut lo, mut hi) = (0.0, core::f64::consts::PI);
            for _ in 0..80 {
                let mid = f64::midpoint(lo, hi);
                if chord(geodesic(crater.centre, mid, azimuth)) < target {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            f64::midpoint(lo, hi)
        };
        let angles: Vec<f64> = breakpoints
            .iter()
            .map(|&b| {
                if b <= 0.0 {
                    0.0
                } else {
                    angle_at(b.min(reach_m))
                }
            })
            .collect();
        let (mut ray, mut ray_magnitude) = (0.0, 0.0);
        for pair in angles.windows(2) {
            if pair[1] <= pair[0] {
                continue;
            }
            let panels = 8_u8;
            let width = (pair[1] - pair[0]) / f64::from(panels);
            for p in 0..panels {
                let mid = pair[0] + width * (f64::from(p) + 0.5);
                for (node, weight) in super::super::tests::GAUSS_LEGENDRE_5 {
                    let psi = mid + 0.5 * width * node;
                    let dir = geodesic(crater.centre, psi, azimuth);
                    let h = sharpening.at(chord(dir) / rim_m).height_m;
                    let inverse = [
                        dir[0] / equatorial_m,
                        dir[1] / equatorial_m,
                        dir[2] / polar_m,
                    ];
                    let area = equatorial_m
                        * equatorial_m
                        * polar_m
                        * dot(inverse, inverse).sqrt()
                        * math::sin(psi);
                    ray += 0.5 * width * weight * h * area;
                    ray_magnitude += 0.5 * width * weight * h.abs() * area;
                }
            }
        }
        let share = 2.0 * core::f64::consts::PI / f64::from(azimuths);
        volume += ray * share;
        magnitude += ray_magnitude * share;
    }
    (volume, magnitude)
}

#[test]
fn on_an_oblate_figure_the_ground_integral_is_off_by_a_small_residual() {
    // A profile balanced in the chord is balanced on a sphere's ground, but not quite on an oblate
    // one's, whose ground within a chord is not π c² (R09.T7.c's deferred finding): about
    // f² (c ÷ ρ)² + f (c ÷ ρ)⁴ (the science check). Over every listed crater it stays within the
    // task's 10⁻⁶ of the cavity on the Moon-, Mars- and Earth-like figures (flattening 0.0012,
    // 0.0059 and 0.0034), but on the Ceres-like world's 0.075 it is larger: pinned here as it
    // stands. Under T7.b's compensation spread over the whole reach the worst were 2.5 × 10⁻⁷ (the
    // Moon-like 2,500 km basin) and 3.1 × 10⁻⁵ (the Ceres-like 280 km crater, whose reach spans
    // 0.76 of the body's radius); with it on the fillet's span (R09.T7.i), 7.3 × 10⁻⁹ (the
    // Moon-like 323 km crater) and 7.9 × 10⁻⁶ (the Ceres-like 107 km one). The sharpening's own
    // magnitude, which keeps the test from being vacuous, is then the rim's alone, down to
    // 2.9 × 10⁻³ of the cavity for the Moon-like 2,500 km basin, whose coarse fillet is 0.07 of its
    // rim radius.
    let field = one_crater();
    let crater = &field.craters()[0];
    let (volume, magnitude) = ground_integral(field, crater, BandLevel::new(8).unwrap(), 16);
    let cavity = crater_shape(field.header().craters(), crater.diameter).cavity_volume_m3();
    assert!(magnitude > 0.1 * cavity, "{magnitude} m³ of {cavity} m³");
    assert!(
        volume.abs() < 1e-12 * cavity,
        "{volume} m³ of {cavity} m³ on a sphere"
    );
    for kind in WORLDS {
        let field = world(kind);
        let bound = if kind == SyntheticWorld::CeresLike {
            5e-5
        } else {
            1e-6
        };
        let level = BandLevel::new(field.header().level().get() + 2).unwrap();
        for crater in field.craters() {
            let (volume, magnitude) = ground_integral(field, crater, level, 64);
            let cavity = crater_shape(field.header().craters(), crater.diameter).cavity_volume_m3();
            assert!(
                magnitude > 1e-3 * cavity,
                "{kind:?}: {magnitude} m³ of {cavity} m³"
            );
            assert!(
                volume.abs() <= bound * cavity,
                "{kind:?}, {:?}: {volume} m³ of {cavity} m³",
                crater.diameter
            );
        }
    }
}

/// ∫ f(x) 2x dx over the reach of two profiles of one shape, in rim radii: five-point
/// Gauss–Legendre on four equal panels of each piece between both profiles' breakpoints, exact to
/// rounding for f a product of two of their heights (degree 8 at most, and 9 with x), and
/// independent of the closed forms' eight-point rule.
fn by_pieces(first: &CraterProfile, second: &CraterProfile, f: impl Fn(f64) -> f64) -> f64 {
    let mut points: Vec<f64> = first
        .breakpoints()
        .into_iter()
        .chain(second.breakpoints())
        .collect();
    points.sort_by(f64::total_cmp);
    let mut total = 0.0;
    for pair in points.windows(2) {
        if pair[1] <= pair[0] {
            continue;
        }
        let width = (pair[1] - pair[0]) / 4.0;
        for k in 0..4_u8 {
            let mid = pair[0] + width * (f64::from(k) + 0.5);
            for (node, weight) in super::super::tests::GAUSS_LEGENDRE_5 {
                let x = mid + 0.5 * width * node;
                total += 0.5 * width * weight * f(x) * 2.0 * x;
            }
        }
    }
    total
}

/// ∫ F(x) x² 2x dx over `profile`'s pieces, metres: its ∫F r² dA over π R⁴, the long-wavelength
/// moment that every isotropic low-pass of a zero-volume profile keeps.
fn second_moment(profile: &CraterProfile) -> f64 {
    profile
        .breakpoints()
        .windows(2)
        .filter(|p| p[1] > p[0])
        .map(|p| quadrature(p[0], p[1], |x| profile.at(x).height_m * x * x))
        .sum()
}

/// The reference Moon's 113 km crater over its coarse band (92.6 km, which holds its fillet at one
/// rim radius) is a depression in the coarse profile (R09.T7.i): −(d − h) at its centre, about
/// −3,330 m (+2,165 m as built, the cone's slope carried into the Hermite and the volume put back
/// as a dome), and below its rim and exterior; its profile at 0, 0.2, 0.6, 1, 1.5 and 2 R, beside
/// main's Table A, and ∫F r² dA beside the fresh crater's (about 0.69 of it under the fillet
/// through the peak, 0.555 as built), are printed, with that ratio for the first octave's other
/// craters.
#[test]
fn the_reference_moon_s_113_km_crater_is_a_depression_over_its_coarse_band() {
    let (params, figure, level) = super::super::tests::reference_moon();
    let band = coarse_band_half_width(&figure, level);
    assert!((band.value() / 92.6e3 - 1.0).abs() < 1e-3, "{band:?}");
    let shape = crater_shape(&params, Metres::new(113e3));
    let coarse = CraterProfile::filleted(shape, band);
    let fresh = CraterProfile::filleted(shape, Metres::ZERO);
    let row = |p: &CraterProfile| {
        [0.0, 0.2, 0.6, 1.0, 1.5, 2.0].map(|x| format!("{:+.0}", p.at(x).height_m))
    };
    eprintln!(
        "113 km on the reference Moon (d {:.0} m, h {:.0} m, d − h {:.0} m, peak {:.0} m on a \
         base of {:.4} R, a {:.4}), filleted over {:.1} m ({} R):",
        shape.depth().value(),
        shape.rim_height().value(),
        shape.floor_depth().value(),
        shape.peak_relief().value(),
        shape.peak_fraction(),
        shape.exterior_width(),
        band.value(),
        coarse.fillet
    );
    eprintln!("  fresh  at 0, 0.2, 0.6, 1, 1.5, 2 R: {:?}", row(&fresh));
    eprintln!("  coarse at 0, 0.2, 0.6, 1, 1.5, 2 R: {:?}", row(&coarse));
    for d_km in [95.0, 113.0, 150.0, 185.0] {
        let shape = crater_shape(&params, Metres::new(d_km * 1e3));
        let ratio = second_moment(&CraterProfile::filleted(shape, band))
            / second_moment(&CraterProfile::filleted(shape, Metres::ZERO));
        eprintln!("  {d_km} km: ∫F r² dA ÷ the fresh crater's {ratio:.4}");
    }
    assert_same_bits(coarse.fillet, 1.0);
    let centre = coarse.at(0.0).height_m;
    assert_same_bits(centre, -shape.floor_depth().value());
    for k in 0..=200 {
        let x = 1.0 + shape.exterior_width() * f64::from(k) / 200.0;
        assert!(centre < coarse.at(x).height_m, "at {x} R");
    }
}

/// The per-crater forms over the whole ground (main's ruling R2 on R09.T12.e, 2026-10-19): a
/// crater's sharpening finer than a wavelength, over the area of its reach's disc, is the share
/// [`unresolved_rims`] sums for it, to 10⁻¹²; it is zero at the band limit and below and for a
/// crater whose relief is gone; and both forms scale as g².
#[test]
fn a_crater_s_sharpening_over_its_reach_is_its_share() {
    let mut checked = 0;
    for field in fields() {
        let rims = CoarseRims::new(field.header());
        for crater in field.craters() {
            let shape = crater_shape(field.header().craters(), crater.diameter);
            let rim_m = 0.5 * crater.diameter.value();
            let reach = 1.0 + shape.exterior_width();
            let disc_m2 = core::f64::consts::PI * (reach * rim_m) * (reach * rim_m);
            for lambda_m in [3.0, 400.0, 2.0e4, 1.0e5, 1.0e7] {
                let finer_than = Metres::new(lambda_m);
                let form = rims.sharpening_moment_m4(crater, finer_than);
                let crater_share = match rims.unresolved_bands(finer_than) {
                    Some((finest, band)) => rims.share(crater, finest, band).variance,
                    None => 0.0,
                };
                let over_disc = form / disc_m2;
                assert!(
                    (over_disc - crater_share).abs() <= 1e-12 * crater_share.abs(),
                    "{:?} at {lambda_m} m: {over_disc} against {crater_share}",
                    crater.diameter
                );
                checked += usize::from(crater_share > 0.0);
            }
            assert_same_bits(rims.sharpening_moment_m4(crater, Metres::new(2.0)), 0.0);
            let worn = CoarseCrater {
                degradation: 255,
                ..crater.clone()
            };
            assert_same_bits(rims.sharpening_moment_m4(&worn, Metres::new(1e5)), 0.0);
            assert_same_bits(rims.increase_m4(&worn), 0.0);
            let fresh = CoarseCrater {
                degradation: 0,
                ..crater.clone()
            };
            let part = CoarseCrater {
                degradation: 51,
                ..crater.clone()
            };
            let g = kept_relief(&part);
            for (whole, scaled) in [
                (
                    rims.sharpening_moment_m4(&fresh, Metres::new(1e5)),
                    rims.sharpening_moment_m4(&part, Metres::new(1e5)),
                ),
                (rims.increase_m4(&fresh), rims.increase_m4(&part)),
            ] {
                assert!(
                    (scaled - g * g * whole).abs() <= 1e-12 * whole.abs(),
                    "{scaled} against {}",
                    g * g * whole
                );
            }
        }
    }
    assert!(checked > 20, "{checked} shares checked");
}

/// A crater's increase is g² ∫ (F(w₂)² − F(`w_c`)²) dA to 10⁻¹⁰ of an independent sum, and is the
/// sharpening's own ∫ (F(w₂) − F(`w_c`))² dA, its whole moment at a wavelength of twice the coarse
/// band or more, plus twice the sharpening's product with the coarse profile.
#[test]
fn a_crater_s_increase_is_its_fine_square_less_its_coarse_one() {
    for field in fields() {
        let rims = CoarseRims::new(field.header());
        for crater in field.craters() {
            let shape = crater_shape(field.header().craters(), crater.diameter);
            let rim_m = 0.5 * crater.diameter.value();
            let disc = core::f64::consts::PI * rim_m * rim_m;
            let fine = CraterProfile::filleted(shape, Metres::new(0.5 * BAND_LIMIT_M));
            let coarse = CraterProfile::filleted(shape, rims.coarse_band());
            let g = kept_relief(crater);
            let heights = |x: f64| (fine.at(x).height_m, coarse.at(x).height_m);
            let squares = by_pieces(&fine, &coarse, |x| {
                let (f, c) = heights(x);
                f * f - c * c
            });
            let expected = g * g * squares * disc;
            let increase = rims.increase_m4(crater);
            let scale = g * g * disc * by_pieces(&fine, &coarse, |x| heights(x).0 * heights(x).0);
            assert!(
                (increase - expected).abs() <= 1e-10 * scale,
                "{:?}: {increase} m⁴ against {expected}",
                crater.diameter
            );
            let whole = Metres::new(2.0 * rims.coarse_band().value());
            let sharpening = rims.sharpening_moment_m4(crater, whole);
            let product = by_pieces(&fine, &coarse, |x| {
                let (f, c) = heights(x);
                (f - c) * c
            });
            let cross = 2.0 * g * g * product * disc;
            assert!(
                (sharpening + cross - increase).abs() <= 1e-10 * scale,
                "{:?}: {sharpening} + {cross} against {increase}",
                crater.diameter
            );
        }
    }
}
