//! The horizon map's tests (plan R10, T8.a): the encoding against WGSL's rounding and its fixture,
//! the Sun's lit fraction through it, the near field on a flat field, a cliff and real terrain
//! against brute force, the far field's reach, its bits at shared points, its independence of the
//! cache and the order of bakes, the survey, and the azimuths; the near field's margin (T8.c): a
//! patch's edges looking out at the ground beside them, and two patches' horizons where they meet,
//! of one level and at a coarse–fine edge.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::f64::consts::PI;

use hyperion_base::units::Metres;
use hyperion_base::units::consts::{METRES_PER_AU, RADIANS_PER_ARCSECOND, SOLAR_RADIUS_M};
use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::bake::tests::{MESH, cells_under, client, code, patch_in, straddling};
use crate::bake::{TerrainBakeOptions, WeightsLayer, bake_terrain};
use crate::cube::SampleGrid;
use crate::field::{CoarseField, Cover, CoverRange, PartialField, cell_at_index, cell_index};
use crate::height::HeightSample;
use crate::synth::test_support::{Recorder, SEED, rim_cells, world};
use crate::testing::{FieldBuilder, SyntheticWorld};
use crate::wire::{decode_payload, encode_payload};

/// Half the encoding's step, 90° ÷ 32,767 ÷ 2, radians.
fn half_step() -> f64 {
    FRAC_PI_2 / SNORM16_MAX / 2.0
}

/// The Sun's angular radius at 1 au, radians: arcsin(R☉ ÷ au), 959.23″, from IAU 2015 Resolution
/// B3's nominal solar radius, 6.957 × 10⁸ m, and IAU 2012 Resolution B2's astronomical unit, as
/// the Astronomical Almanac takes them (2021, Section K, p. K7); the plan's 959.6″ is the older
/// standard 959.63″ (Allen 1976; Haberreiter, Schmutz and Kosovichev 2008, ApJ 675, L53, Table 1).
fn sun_at_one_au() -> f64 {
    math::asin(SOLAR_RADIUS_M / METRES_PER_AU)
}

/// A client's field of `field` holding `cells`, surveyed at 10 m, with their margin.
fn client_of(field: &CoarseField, cells: &BTreeSet<PatchKey>) -> PartialField {
    let mut indices: Vec<u32> = cells.iter().map(|&c| cell_index(c)).collect();
    indices.sort_unstable();
    let cover = Cover::from_ranges(
        indices
            .iter()
            .map(|&i| CoverRange::new(i, i + 1, code(10.0)).unwrap()),
    )
    .unwrap();
    let blocks = decode_payload(&encode_payload(field, &cover, None)).unwrap();
    let mut view = PartialField::new(blocks[0].header().unwrap().clone());
    for block in &blocks {
        view.insert(block).unwrap();
    }
    view
}

/// The coarse cells of `field` whose centre lies within `angle` radians of the unit direction
/// `centre`.
fn cells_within(field: &CoarseField, centre: [f64; 3], angle: f64) -> BTreeSet<PatchKey> {
    let level = field.header().level();
    let near = math::cos(angle);
    (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).unwrap())
        .filter(|c| dot(c.vertex_dir(32, 32), centre) >= near)
        .collect()
}

/// A client's field of `field` holding the cells under `key` and every cell within the far field's
/// reach of it and two cells more, so that the far field reads ground the view holds.
fn client_within_reach(field: &CoarseField, key: PatchKey) -> PartialField {
    let header = field.header();
    let reach = far_reach(&header.figure(), header.realised_relief().value()).value();
    let cell = 1.7 * FRAC_PI_2 / f64::from(1_u32 << header.level().get());
    let mut cells = cells_within(field, key.vertex_dir(32, 32), reach + 2.0 * cell);
    cells.extend(cells_under(field, key));
    client_of(field, &cells)
}

/// The low setting's options with the horizon map and without the weights, which the map does
/// not read.
const HORIZON_ONLY: TerrainBakeOptions = TerrainBakeOptions {
    weights: WeightsLayer::Skipped,
    ..MESH
};

/// `key`'s horizon map at the low setting over `synth`, from `cache`.
fn horizon_of(
    synth: &Synthesiser<'_, PartialField>,
    cache: &mut SynthCache,
    key: PatchKey,
) -> HorizonMap {
    bake_terrain(synth, cache, key, &HORIZON_ONLY, &[])
        .unwrap()
        .layers()
        .horizon()
        .clone()
}

/// The angle between two unit directions, by atan2 of their cross and dot products.
fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    math::atan2(dot(c, c).sqrt(), dot(a, b))
}

/// ⌊½ + 32,767 x⌋ in exact integer arithmetic from `x`'s bits, clamped as WGSL's
/// `pack2x16snorm` clamps: the oracle of [`snorm16`].
fn exact_snorm16(x: f64) -> i16 {
    if x >= 1.0 {
        return 32_767;
    }
    if x <= -1.0 {
        return -32_767;
    }
    let word = bits(x);
    let negative = word >> 63 == 1;
    let exponent = i32::try_from((word >> 52) & 0x7ff).unwrap();
    let fraction = word & ((1 << 52) - 1);
    // |x| = mantissa × 2^−shift with the mantissa an integer below 2^53, and the shift positive
    // since |x| < 1.
    let (mantissa, shift) = if exponent == 0 {
        (fraction, 1074)
    } else {
        (fraction | (1 << 52), 1075 - exponent)
    };
    if shift > 120 {
        // 32,767 × mantissa < 2^68, so 32,767 |x| < 2^−52: the code is 0.
        return 0;
    }
    let scaled = i128::from(mantissa) * 32_767;
    let half = 1_i128 << (shift - 1);
    let whole = 1_i128 << shift;
    let code = if negative {
        (half - scaled).div_euclid(whole)
    } else {
        (scaled + half).div_euclid(whole)
    };
    i16::try_from(code).unwrap()
}

/// The share of a star's disc of angular radius `rho` above a horizon at `horizon`, the star's
/// centre at `sun` (Design note 10): 1 − (arccos x − x √(1 − x²)) ÷ π, x = (`sun` − `horizon`) ÷
/// `rho` clamped to [−1, 1], without limb darkening.
fn lit_fraction(sun: f64, horizon: f64, rho: f64) -> f64 {
    let x = num::min(num::max((sun - horizon) / rho, -1.0), 1.0);
    1.0 - (math::acos(x) - x * (1.0 - x * x).sqrt()) / PI
}

// ---------------------------------------------------------------------------------------------
// The encoding.
// ---------------------------------------------------------------------------------------------

#[test]
fn horizon_round_trip_is_within_half_a_step_at_every_elevation() {
    let bound = 4.95 * RADIANS_PER_ARCSECOND;
    let mut worst = 0.0_f64;
    let mut checked = 0_u32;
    let mut check = |e: f64| {
        let back = decode_horizon(encode_horizon(Radians::new(e))).value();
        let error = (back - e).abs();
        assert!(error <= bound, "{e} rad comes back as {back} rad");
        worst = num::max(worst, error);
        checked += 1;
    };
    let n = 1_000_000_u32;
    for k in 0..=n {
        check(-FRAC_PI_2 + PI * f64::from(k) / f64::from(n));
    }
    for e in [-FRAC_PI_2, -0.0, 0.0, FRAC_PI_2] {
        check(e);
    }
    // Either side of every boundary between two codes, (c + ½) ÷ 32,767 × 90°.
    for c in -32_767..32_767 {
        let boundary = (f64::from(c) + 0.5) / SNORM16_MAX * FRAC_PI_2;
        for e in [boundary.next_down(), boundary, boundary.next_up()] {
            check(e);
        }
    }
    assert!(checked > 1_000_000, "{checked} elevations");
    // The bound is the half step, which the boundaries reach.
    assert!(worst > 0.999 * half_step(), "{worst} reaches the half step");
    assert!(
        worst <= half_step() * (1.0 + 1e-9),
        "{worst} within the half step"
    );
    assert_eq!(encode_horizon(Radians::new(FRAC_PI_2)), 32_767);
    assert_eq!(encode_horizon(Radians::new(-FRAC_PI_2)), -32_767);
    assert_eq!(encode_horizon(Radians::new(0.0)), 0);
    assert_eq!(encode_horizon(Radians::new(-0.0)), 0);
    assert_eq!(encode_horizon(Radians::new(2.0)), 32_767);
    assert_eq!(encode_horizon(Radians::new(-2.0)), -32_767);
}

#[test]
fn horizon_decode_rises_with_the_code() {
    for c in i16::MIN..i16::MAX {
        let (low, high) = (decode_horizon(c).value(), decode_horizon(c + 1).value());
        if c >= NO_HORIZON {
            assert!(high > low, "{c}");
        } else {
            assert_eq!(bits(high), bits(low), "−32,768 reads as −32,767 does");
        }
    }
    assert_eq!(bits(decode_horizon(NO_HORIZON).value()), bits(-FRAC_PI_2));
    assert_eq!(bits(decode_horizon(0).value()), bits(0.0));
    assert_eq!(bits(decode_horizon(32_767).value()), bits(FRAC_PI_2));
}

#[test]
fn horizon_encoding_is_wgsl_pack2x16snorm_s_rounding_exactly() {
    let mut rng = Lcg::new(0x686f_7269_7a6f_6e31);
    let mut inputs: Vec<f64> = vec![0.0, -0.0, 1.0, -1.0, 1.5, -1.5, f64::MIN_POSITIVE, 5e-324];
    for c in -32_767..32_767 {
        let boundary = (f64::from(c) + 0.5) / SNORM16_MAX;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the f32 nearest a boundary, as a shader's input"
        )]
        let narrow = boundary as f32;
        for x in [boundary.next_down(), boundary, boundary.next_up()] {
            inputs.push(x);
        }
        for x in [narrow.next_down(), narrow, narrow.next_up()] {
            inputs.push(f64::from(x));
        }
    }
    for _ in 0..100_000 {
        inputs.push(2.2 * rng.next_f64() - 1.1);
    }
    for x in inputs {
        assert_eq!(snorm16(x), exact_snorm16(x), "{x:e}");
    }
}

/// The fixture R10.T8.b's TypeScript twin reads: `f32` inputs by their bits and their codes, and
/// pairs of codes packed into a word, as `pack2x16snorm` packs them.
fn encoding_text() -> String {
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    let mut rng = Lcg::new(0x686f_7269_7a6f_6e32);
    let mut inputs: Vec<f32> = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        1.5,
        -1.5,
        f32::MAX,
        f32::MIN,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
    ];
    for c in (-32_767..32_767).step_by(257).chain([-32_767, 32_766]) {
        let boundary = (f64::from(c) + 0.5) / SNORM16_MAX;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the f32 nearest a boundary, as a shader's input"
        )]
        let narrow = boundary as f32;
        inputs.extend([narrow.next_down(), narrow, narrow.next_up()]);
    }
    for _ in 0..64 {
        #[expect(clippy::cast_possible_truncation, reason = "an f32 input")]
        let x = (2.2 * rng.next_f64() - 1.1) as f32;
        inputs.push(x);
    }
    for x in &inputs {
        out.line(&format!(
            "snorm 0x{:08x} {}",
            hyperion_testkit::float::bits_f32(*x),
            snorm16(f64::from(*x))
        ));
    }
    for &[low, high] in inputs.as_chunks::<2>().0.iter().take(24) {
        let codes = [snorm16(f64::from(low)), snorm16(f64::from(high))];
        out.line(&format!(
            "pack {} {} 0x{:08x}",
            codes[0],
            codes[1],
            pack(&codes)[0]
        ));
    }
    out.finish()
}

#[test]
fn horizon_encoding_matches_its_fixture() {
    golden!("horizon/encoding", &encoding_text());
}

#[test]
fn horizon_lit_fraction_of_the_sun_moves_under_a_255th() {
    let rho = sun_at_one_au();
    for (x, f) in [(-1.0, 0.0), (0.0, 0.5), (1.0, 1.0), (-2.0, 0.0), (2.0, 1.0)] {
        assert!((lit_fraction(x * rho, 0.0, rho) - f).abs() < 1e-15, "{x}");
    }
    let mut horizons: Vec<f64> = (0..=10_000)
        .map(|k| -FRAC_PI_2 + PI * f64::from(k) / 10_000.0)
        .collect();
    for c in (-32_767..32_767).step_by(32) {
        let boundary = (f64::from(c) + 0.5) / SNORM16_MAX * FRAC_PI_2;
        horizons.extend([boundary.next_down(), boundary.next_up()]);
    }
    let mut worst = 0.0_f64;
    for h in horizons {
        let decoded = decode_horizon(encode_horizon(Radians::new(h))).value();
        for k in -60..=60 {
            let sun = h + rho * f64::from(k) / 50.0;
            let shift = (lit_fraction(sun, decoded, rho) - lit_fraction(sun, h, rho)).abs();
            worst = num::max(worst, shift);
        }
    }
    // At most 2 δ ÷ (π ρ), 0.33%, for the half step δ (Design note 10).
    let bound = 2.0 * half_step() / (PI * rho);
    assert!(worst <= 1.0 / 255.0, "{worst}");
    assert!(worst <= bound * 1.0001, "{worst} against {bound}");
    assert!(worst > 0.9 * bound, "{worst} against {bound}");
}

// ---------------------------------------------------------------------------------------------
// The near field.
// ---------------------------------------------------------------------------------------------

/// A flat, airless sphere of the Moon's radius, its ground `height_m` metres above the datum.
fn flat_field(height_m: f64) -> CoarseField {
    FieldBuilder::new(Metres::new(1_737_400.0))
        .elevation(move |_| Metres::new(height_m))
        .build()
}

/// The patches of `key`'s level beside it on its face: the eight about it that the face holds.
fn beside(key: PatchKey) -> Vec<PatchKey> {
    let last = (1_u32 << key.level()) - 1;
    let mut out = Vec::new();
    for dj in -1..=1 {
        for di in -1..=1 {
            let (Some(i), Some(j)) = (
                key.i().checked_add_signed(di),
                key.j().checked_add_signed(dj),
            ) else {
                continue;
            };
            if (di, dj) != (0, 0) && i <= last && j <= last {
                out.push(PatchKey::new(key.face(), key.level(), i, j).unwrap());
            }
        }
    }
    out
}

/// The coarse cells of `field` under `key` and under the patches beside it: those its near field
/// and its margin read.
fn cells_about(field: &CoarseField, key: PatchKey) -> BTreeSet<PatchKey> {
    let mut cells = cells_under(field, key);
    for next in beside(key) {
        cells.extend(cells_under(field, next));
    }
    cells
}

/// The unit direction of vertex (`x`, `y`) of `key`'s near grid, its own or its margin's.
fn grid_dir(key: PatchKey, x: i32, y: i32) -> [f64; 3] {
    let (i, j) = NearGrid::of(key).lattice(x, y);
    lattice_vertex_dir(key.face(), key.level(), i, j)
}

/// `key`'s level's height function at its lattice vertex (`i`, `j`) on its face, as the far field
/// and the margin ask it, every one answered.
fn lattice_height_m(
    synth: &Synthesiser<'_, PartialField>,
    cache: &mut SynthCache,
    key: PatchKey,
    (i, j): (u32, u32),
) -> f64 {
    let band = BandLevel::new(key.level()).unwrap();
    synth
        .height_at(
            cache,
            lattice_vertex_dir(key.face(), key.level(), i, j),
            band,
        )
        .unwrap()
        .height_m
}

#[test]
fn horizon_of_a_flat_field_is_the_curvature_drop_and_never_above_zero() {
    let field = flat_field(1_500.0);
    assert_eq!(bits(field.header().realised_relief().value()), bits(0.0));
    let coarse = field.header().level().get();
    let inner = patch_in(
        PatchKey::new(Face::PosX, coarse, 20, 41).unwrap(),
        coarse + 3,
    );
    // A patch on the face's edge s = 0, where its margin stops.
    let rim = PatchKey::new(Face::PosX, coarse + 3, 0, inner.j()).unwrap();
    for key in [inner, rim] {
        let view = client_of(&field, &cells_about(&field, key));
        let synth = Synthesiser::new(&view, SEED);
        let map = horizon_of(&synth, &mut SynthCache::new(), key);
        assert!(
            map.is_complete(),
            "the view holds the flat field beside the patch"
        );
        for y in 0..=64_u8 {
            for x in 0..=64_u8 {
                for (k, &[dx, dy]) in AZIMUTH_STEPS.iter().enumerate() {
                    let (nx, ny) = (i32::from(x) + i32::from(dx), i32::from(y) + i32::from(dy));
                    let near = map.near(x, y, k).value();
                    assert!(near <= 0.0, "{key:?} ({x}, {y}) {k}: {near}");
                    if key.i() == 0 && nx < 0 {
                        assert_eq!(
                            bits(near),
                            bits(-FRAC_PI_2),
                            "{key:?} ({x}, {y}) {k} looks off the face"
                        );
                    } else {
                        // Every point at one height: the nearest stands highest, at −θ ÷ 2, the
                        // patch's own or, from its edge, its margin's.
                        let next = grid_dir(key, nx, ny);
                        let drop = -angle_between(key.vertex_dir(x, y), next) / 2.0;
                        assert!(
                            (near - drop).abs() <= half_step() * 1.001,
                            "{key:?} ({x}, {y}) {k}: {near} against {drop}"
                        );
                    }
                }
            }
        }
        // With no relief the far field's reach is nought: no sample, no horizon.
        for b in 0..=8 {
            for a in 0..=8 {
                for k in 0..AZIMUTHS {
                    assert_eq!(bits(map.far(a, b, k).value()), bits(-FRAC_PI_2));
                }
            }
        }
    }
}

#[test]
fn horizon_of_a_cliff_of_height_h_at_distance_d_is_its_top_s_elevation() {
    // A cliff 100 m high along a column of a level-14 patch on an Earth-sized sphere, about 10 m
    // between vertices, in the patch (column 40) and on its neighbour, in its margin (column 68):
    // (H − d² ÷ 2R) ÷ d differs from the exact tan e by a part in H ÷ R, at most (H ÷ R) ÷ 2, about
    // 1.6″, in elevation here (at e near 45°).
    let radius = 6_371_000.0;
    let figure = Spheroid::sphere(radius);
    let key = PatchKey::new(Face::PosZ, 14, 5_000, 7_100).unwrap();
    let start = i64::from(key.i() * PATCH_QUADS);
    let h = 100.0;
    for cliff in [40_i32, 68] {
        let ground = |x: i32| if x >= cliff { h } else { 0.0 };
        let heights = PatchHeights::from_queries(key, |q| {
            Ok::<_, std::convert::Infallible>(HeightSample {
                height_m: ground(i32::from(q.x)),
                gradient: [0.0; 3],
            })
        })
        .unwrap();
        let codes = near_codes(key, &figure, &heights, |i, _| {
            ground(i32::try_from(i64::from(i) - start).unwrap())
        });
        let mut seen = 0;
        for y in 0..=64_u8 {
            let top = grid_dir(key, cliff, i32::from(y));
            for x in (0..=64_u8).filter(|&x| i32::from(x) < cliff) {
                let n = usize::from(y) * PATCH_VERTICES + usize::from(x);
                let near = decode_horizon(codes[n * AZIMUTHS]).value();
                let d = radius * angle_between(key.vertex_dir(x, y), top);
                let formula = math::atan((h - d * d / (2.0 * radius)) / d);
                assert!(
                    (near - formula).abs() <= 2.0 * half_step(),
                    "{cliff} ({x}, {y}): {near} against {formula}"
                );
                let from = figure.surface_point(key.vertex_dir(x, y), 0.0);
                let exact = elevation(
                    from,
                    figure.normal(key.vertex_dir(x, y)),
                    figure.surface_point(top, h),
                );
                assert_eq!(
                    codes[n * AZIMUTHS],
                    encode_horizon(exact),
                    "{cliff} ({x}, {y})"
                );
                seen += 1;
            }
        }
        assert_eq!(seen, 65 * cliff.min(65));
        // On level ground, looking on: the curvature drop to the next vertex, the margin's from
        // the patch's edges, on the plateau to +x and below the cliffs to −x.
        let level: &[(u8, u8, usize, i32)] = if cliff == 40 {
            &[(50, 20, 0, 51), (64, 20, 0, 65), (0, 20, 4, -1)]
        } else {
            &[(0, 20, 4, -1), (30, 40, 4, 29)]
        };
        for &(x, y, k, next) in level {
            let n = usize::from(y) * PATCH_VERTICES + usize::from(x);
            let ahead = grid_dir(key, next, i32::from(y));
            let drop = -angle_between(key.vertex_dir(x, y), ahead) / 2.0;
            let flat = decode_horizon(codes[n * AZIMUTHS + k]).value();
            assert!(
                (flat - drop).abs() <= half_step(),
                "{cliff} ({x}, {y}) {k}: {flat} against {drop}"
            );
        }
    }
}

/// The highest elevation over the vertices ahead of `vertex` along azimuth `k`'s grid line, each
/// taken exactly, the patch's own and on to the end of its margin or of the face: the near field
/// by brute force. `position` is each vertex's body-fixed position, `None` past those ends.
fn brute_near(
    position: &dyn Fn(i32, i32) -> Option<[f64; 3]>,
    up: [f64; 3],
    vertex: (u8, u8),
    k: usize,
) -> Option<Radians> {
    let [dx, dy] = AZIMUTH_STEPS[k];
    let (x, y) = (i32::from(vertex.0), i32::from(vertex.1));
    let from = position(x, y).unwrap();
    let mut best: Option<f64> = None;
    for n in 1.. {
        let Some(to) = position(x + n * i32::from(dx), y + n * i32::from(dy)) else {
            break;
        };
        let seen = elevation(from, up, to).value();
        best = Some(best.map_or(seen, |highest| num::max(highest, seen)));
    }
    best.map(Radians::new)
}

/// The body-fixed positions of `key`'s vertices over `figure`, its own from `heights` and, within
/// [`NEAR_MARGIN`] of its edges on its face, its margin's from `margin_m` at their lattice indices,
/// by the patch's vertex indices; `None` past the margin or off the face.
fn near_ground(
    key: PatchKey,
    figure: &Spheroid,
    heights: &PatchHeights,
    mut margin_m: impl FnMut(u32, u32) -> f64,
) -> impl Fn(i32, i32) -> Option<[f64; 3]> {
    let m = i32::from(NEAR_MARGIN);
    let span = -m..=64 + m;
    let side = 65 + 2 * m;
    let last = i64::from(PATCH_QUADS << key.level());
    let (i0, j0) = (
        i64::from(key.i() * PATCH_QUADS),
        i64::from(key.j() * PATCH_QUADS),
    );
    let mut positions = Vec::new();
    for y in span.clone() {
        for x in span.clone() {
            let (i, j) = (i0 + i64::from(x), j0 + i64::from(y));
            let on_face = (0..=last).contains(&i) && (0..=last).contains(&j);
            let own = (u8::try_from(x), u8::try_from(y));
            positions.push(on_face.then(|| match own {
                (Ok(vx), Ok(vy)) if vx <= 64 && vy <= 64 => {
                    figure.surface_point(key.vertex_dir(vx, vy), heights.sample(vx, vy).height_m)
                }
                _ => {
                    let (i, j) = (u32::try_from(i).unwrap(), u32::try_from(j).unwrap());
                    let dir = lattice_vertex_dir(key.face(), key.level(), i, j);
                    figure.surface_point(dir, margin_m(i, j))
                }
            }));
        }
    }
    move |x, y| {
        if !span.contains(&x) || !span.contains(&y) {
            return None;
        }
        positions[usize::try_from((y + m) * side + (x + m)).unwrap()]
    }
}

#[test]
fn horizon_near_field_sweep_is_the_brute_force_horizon_on_rough_ground() {
    for (w, level_above) in [
        (SyntheticWorld::MoonLike, 4),
        (SyntheticWorld::EarthLike, 3),
    ] {
        let field = world(w);
        let coarse = field.header().level().get();
        let key = patch_in(rim_cells(field, 1)[0], coarse + level_above);
        // The view holds the ground beside the patch, so that the margin reads its real heights.
        let view = client(w, &cells_about(field, key), |_| code(20.0));
        let synth = Synthesiser::new(&view, SEED);
        let mut cache = SynthCache::new();
        let heights = synth.bake_patch(&mut cache, key).unwrap();
        let figure = field.header().figure();
        let mut margin = |i, j| lattice_height_m(&synth, &mut cache, key, (i, j));
        let codes = near_codes(key, &figure, &heights, &mut margin);
        let position = near_ground(key, &figure, &heights, &mut margin);
        let (mut above, mut off_by_one) = (0, 0);
        for y in 0..=64_u8 {
            for x in 0..=64_u8 {
                let up = figure.normal(key.vertex_dir(x, y));
                for k in 0..AZIMUTHS {
                    let n = usize::from(y) * PATCH_VERTICES + usize::from(x);
                    let swept = codes[n * AZIMUTHS + k];
                    let brute =
                        brute_near(&position, up, (x, y), k).map_or(NO_HORIZON, encode_horizon);
                    // The sweep's hull lies in the line's plane; where a diagonal or a spheroid's
                    // heights lie a little off it, it may pick a tangent a hair from the exact one.
                    assert!(
                        (i32::from(swept) - i32::from(brute)).abs() <= 1,
                        "{w:?} ({x}, {y}) {k}: {swept} against {brute}"
                    );
                    // Away from the face's edges every vertex has ground ahead, its edges' the
                    // margin's.
                    assert_ne!(swept, NO_HORIZON, "{w:?} ({x}, {y}) {k}");
                    off_by_one += i32::from(swept != brute);
                    above += i32::from(swept > 0);
                }
            }
        }
        assert!(off_by_one < 50, "{w:?}: {off_by_one} codes a step off");
        assert!(above > 1_000, "{w:?}: rough ground has horizons above zero");
    }
}

#[test]
fn horizon_near_field_at_a_patch_s_edge_sees_the_neighbour_s_rising_ground() {
    for (w, level_above) in [
        (SyntheticWorld::MoonLike, 4),
        (SyntheticWorld::EarthLike, 3),
    ] {
        let field = world(w);
        let coarse = field.header().level().get();
        let key = patch_in(rim_cells(field, 1)[0], coarse + level_above);
        let view = client(w, &cells_about(field, key), |_| code(20.0));
        let synth = Synthesiser::new(&view, SEED);
        // The bake's map, its margin from the far field's ground.
        let map = horizon_of(&synth, &mut SynthCache::new(), key);
        let mut cache = SynthCache::new();
        let heights = synth.bake_patch(&mut cache, key).unwrap();
        let figure = field.header().figure();
        let grid = NearGrid::of(key);
        let (mut outward, mut rising) = (0, 0);
        for y in 0..=64_u8 {
            for x in (0..=64_u8).filter(|&x| x == 0 || x == 64 || y == 0 || y == 64) {
                let from =
                    figure.surface_point(key.vertex_dir(x, y), heights.sample(x, y).height_m);
                let up = figure.normal(key.vertex_dir(x, y));
                for (k, &[dx, dy]) in AZIMUTH_STEPS.iter().enumerate() {
                    let (nx, ny) = (i32::from(x) + i32::from(dx), i32::from(y) + i32::from(dy));
                    if NearGrid::own(nx, ny).is_some() {
                        continue;
                    }
                    outward += 1;
                    let near = map.near(x, y, k).value();
                    assert!(
                        near > -FRAC_PI_2,
                        "{w:?} ({x}, {y}) {k} looks out at the neighbour's ground"
                    );
                    // The neighbour's vertex a step on: the horizon is at least as high, to the
                    // sweep's code and the encoding's half step.
                    let next_m = lattice_height_m(&synth, &mut cache, key, grid.lattice(nx, ny));
                    let next = figure.surface_point(grid_dir(key, nx, ny), next_m);
                    let first = elevation(from, up, next).value();
                    assert!(
                        near >= first - 3.0 * half_step() * 1.001,
                        "{w:?} ({x}, {y}) {k}: {near} below the next vertex's {first}"
                    );
                    if first > 0.0 {
                        rising += 1;
                        assert!(near > 0.0, "{w:?} ({x}, {y}) {k}");
                    }
                }
            }
        }
        // Three azimuths out of each edge vertex, five out of a corner.
        assert_eq!(outward, 4 * 63 * 3 + 4 * 5, "{w:?}");
        assert!(
            rising > 50,
            "{w:?}: {rising} azimuths look out at rising ground"
        );
    }
}

/// The vertices two patches of one level share along an edge, and the edge's azimuths.
struct SharedEdge {
    /// Each shared vertex's indices in the first patch and in the second.
    pairs: Vec<[(u8, u8); 2]>,
    /// The two azimuths along the edge.
    along: [usize; 2],
    /// The three azimuths out of the first patch, across the edge; their opposites are out of
    /// the second.
    out_of_a: [usize; 3],
}

impl SharedEdge {
    /// The edge `a` shares with `b`, its neighbour of its level on its face.
    fn of(a: PatchKey, b: PatchKey) -> Self {
        let all = 0..=64_u8;
        let step = (
            b.i().cast_signed() - a.i().cast_signed(),
            b.j().cast_signed() - a.j().cast_signed(),
        );
        let (pairs, along, out_of_a) = match step {
            (1, 0) => (all.map(|y| [(64, y), (0, y)]).collect(), [2, 6], [7, 0, 1]),
            (-1, 0) => (all.map(|y| [(0, y), (64, y)]).collect(), [2, 6], [3, 4, 5]),
            (0, 1) => (all.map(|x| [(x, 64), (x, 0)]).collect(), [0, 4], [1, 2, 3]),
            (0, -1) => (all.map(|x| [(x, 0), (x, 64)]).collect(), [0, 4], [5, 6, 7]),
            other => panic!("{a:?} and {b:?} share no edge: {other:?}"),
        };
        Self {
            pairs,
            along,
            out_of_a,
        }
    }
}

/// The differences between `a`'s and `b`'s horizons, radians, at the vertices they share along
/// `edge`: none along the edge, where both read the same line of vertices; and looking across it,
/// where the side looking in sees everything the side looking out sees and more, the excess of the
/// horizon looking in over the horizon looking out, at each azimuth's knot.
fn shared_edge_excess(a: &HorizonMap, b: &HorizonMap, edge: &SharedEdge) -> Vec<f64> {
    let mut excess = Vec::new();
    for &[(xa, ya), (xb, yb)] in &edge.pairs {
        for k in 0..AZIMUTHS {
            #[expect(clippy::cast_precision_loss, reason = "eight azimuths")]
            let phi = Radians::new(FRAC_PI_4 * k as f64);
            let (ea, eb) = (
                a.elevation_at(xa, ya, phi).value(),
                b.elevation_at(xb, yb, phi).value(),
            );
            if edge.along.contains(&k) {
                assert_eq!(
                    bits(a.near(xa, ya, k).value()),
                    bits(b.near(xb, yb, k).value()),
                    "({xa}, {ya}) {k}: one line along the edge"
                );
                // The far field, the same bits at shared points, is lerped to the edge by each.
                assert!(
                    (ea - eb).abs() <= 1e-12,
                    "({xa}, {ya}) {k}: {ea} against {eb}"
                );
                continue;
            }
            let (inward, outward) = if edge.out_of_a.contains(&k) {
                (eb, ea)
            } else {
                (ea, eb)
            };
            // Each sweep within a code of the brute force over the same vertices and more.
            assert!(
                inward >= outward - 4.0 * half_step() - 1e-12,
                "({xa}, {ya}) {k}: {inward} looking in below {outward} looking out"
            );
            excess.push(inward - outward);
        }
    }
    excess
}

#[test]
fn horizon_at_an_edge_two_patches_share_differs_only_by_the_ground_beyond_the_margin() {
    for (w, level_above) in [
        (SyntheticWorld::MoonLike, 4),
        (SyntheticWorld::EarthLike, 3),
    ] {
        let field = world(w);
        let coarse = field.header().level().get();
        let a = patch_in(rim_cells(field, 1)[0], coarse + level_above);
        let next = |di: u32, dj: u32| PatchKey::new(a.face(), a.level(), a.i() + di, a.j() + dj);
        let (east, north) = (next(1, 0).unwrap(), next(0, 1).unwrap());
        let view = client_within_reach(field, a);
        let synth = Synthesiser::new(&view, SEED);
        let mut cache = SynthCache::new();
        let m_a = horizon_of(&synth, &mut cache, a);
        // The margin's heights are the neighbour's own, bit for bit.
        let heights = synth.bake_patch(&mut cache, east).unwrap();
        let grid = NearGrid::of(a);
        for y in 0..=64_u8 {
            for x in 1..=NEAR_MARGIN {
                let at = grid.lattice(64 + i32::from(x), i32::from(y));
                assert_eq!(
                    bits(lattice_height_m(&synth, &mut cache, a, at)),
                    bits(heights.sample(x, y).height_m),
                    "{w:?} ({x}, {y})"
                );
            }
        }
        let mut excess = Vec::new();
        for b in [east, north] {
            let m_b = horizon_of(&synth, &mut cache, b);
            // Its margin read from `a`'s kept band, it is a cold bake's bits.
            assert_eq!(
                m_b,
                horizon_of(&synth, &mut SynthCache::new(), b),
                "{w:?} {b:?}: warm against cold"
            );
            excess.extend(shared_edge_excess(&m_a, &m_b, &SharedEdge::of(a, b)));
        }
        assert_shared_edge_excess(&format!("{w:?}"), &excess);
    }
}

#[test]
fn horizon_at_a_coarse_fine_edge_the_parent_s_differs_from_the_coarse_neighbour_s_by_as_much() {
    // A fine patch at k = 1 draws its parent's ground and takes its parent's horizon (R10.T8.b's
    // blend), beside a coarse neighbour of its parent's level that takes its own: the two meet as
    // two patches of one level do, along the half of the parent's edge the child's lies on.
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let coarse = field.header().level().get();
    let child = patch_in(rim_cells(field, 1)[0], coarse + 3);
    let parent = child.parent().unwrap();
    let (qx, qy) = (child.i() & 1, child.j() & 1);
    let neighbour = PatchKey::new(
        parent.face(),
        parent.level(),
        if qx == 1 {
            parent.i() + 1
        } else {
            parent.i() - 1
        },
        parent.j(),
    )
    .unwrap();
    let view = client_within_reach(field, parent);
    let synth = Synthesiser::new(&view, SEED);
    let mut cache = SynthCache::new();
    let (m_parent, m_neighbour) = (
        horizon_of(&synth, &mut cache, parent),
        horizon_of(&synth, &mut cache, neighbour),
    );
    let mut edge = SharedEdge::of(parent, neighbour);
    // The child's even vertices on its edge, at the parent's (32 qy + y ÷ 2).
    let half = u8::try_from(32 * qy).unwrap();
    edge.pairs
        .retain(|&[(_, y), _]| (half..=half + 32).contains(&y));
    assert_eq!(edge.pairs.len(), 33);
    let excess = shared_edge_excess(&m_parent, &m_neighbour, &edge);
    assert_shared_edge_excess("the coarse–fine edge", &excess);
}

/// The largest excess of the horizon looking in over the horizon looking out at a vertex two maps
/// of one level share, degrees ([`shared_edge_excess`]): the ground past the margin, which the
/// side looking out sees only through the far field, lerped between its points eight vertices
/// apart. The study's estimate (R10.T8.c) is about Δh ÷ D for ground of relief Δh seen from D, at
/// least nine vertex spacings out: 1–3° on rough ground. Measured (2026-10-20), at most 2.29° on
/// the Moon-like world's rim at L + 4 (level 10; 99% of knots under 2.06°), 0.42° on the Earth-like
/// world's at L + 3, and 0.48° at the Moon-like coarse–fine edge, its parent at L + 2; at L + 6,
/// 0.79° and 0.15°. With no margin, 10.4°, 4.9° and 4.4° (3.0° and 2.3° at L + 6). The bound is
/// half as much again as the largest.
const SHARED_EDGE_EXCESS_DEG: f64 = 3.5;

/// Asserts the excesses `excess` of `case` within [`SHARED_EDGE_EXCESS_DEG`], and that some knot
/// looks across the edge.
fn assert_shared_edge_excess(case: &str, excess: &[f64]) {
    assert!(!excess.is_empty(), "{case}: knots looking across the edge");
    let worst = excess.iter().copied().fold(0.0, num::max);
    assert!(
        worst.to_degrees() <= SHARED_EDGE_EXCESS_DEG,
        "{case}: {}° looking in over looking out",
        worst.to_degrees()
    );
}

// ---------------------------------------------------------------------------------------------
// The far field.
// ---------------------------------------------------------------------------------------------

#[test]
fn horizon_far_march_stops_at_the_reach() {
    // The plan's Earth with 20 km of relief: about 505 km.
    let earth = Spheroid::sphere(6_371_000.0);
    let reach = far_reach(&earth, 20_000.0).value();
    let metres = reach * 6_371_000.0;
    assert!(
        (metres - (2.0 * 6_371_000.0 * 20_000.0_f64).sqrt()).abs() < 1e-6,
        "{metres}"
    );
    assert!((metres - 505_000.0).abs() < 1_000.0, "{metres}");
    for level in [0, 2, 3, 8, 12, 19, 24] {
        let steps = far_steps(level, &earth, 20_000.0);
        let first = vertex_spacing(1.0, level).mean_m;
        if first > reach {
            assert!(steps.is_empty(), "{level}");
            continue;
        }
        assert_eq!(bits(steps[0].theta_rad), bits(first), "{level}");
        let last = steps.last().unwrap();
        assert!(
            last.theta_rad <= reach,
            "{level}: the last step within the reach"
        );
        assert!(
            last.theta_rad * GROWTH[1] > reach * (1.0 - 1e-12),
            "{level}: the next step past it"
        );
        for (k, step) in steps.iter().enumerate() {
            if k >= 4 {
                assert_eq!(
                    bits(step.theta_rad),
                    bits(2.0 * steps[k - 4].theta_rad),
                    "{level} {k}"
                );
            }
            if k > 0 {
                let growth = step.theta_rad / steps[k - 1].theta_rad;
                assert!((growth - GROWTH[1]).abs() < 1e-12, "{level} {k}: {growth}");
                assert!(
                    step.level <= steps[k - 1].level,
                    "{level} {k}: levels never finer"
                );
            }
            let expected = level.saturating_sub(u8::try_from((k / 4).saturating_sub(2)).unwrap());
            assert_eq!(step.level, expected, "{level} {k}");
            // The level's vertex spacing is about the step's length, θ (2^¼ − 1).
            if step.level > 0 && step.level < level {
                let spacing = vertex_spacing(1.0, step.level).mean_m;
                let ratio = spacing / (step.theta_rad * (GROWTH[1] - 1.0));
                assert!((0.6..1.7).contains(&ratio), "{level} {k}: {ratio}");
            }
        }
        // A level's steps are its parent's, four steps on, to the bit.
        if level > 0 {
            let parent = far_steps(level - 1, &earth, 20_000.0);
            for (k, step) in parent.iter().enumerate() {
                assert_eq!(
                    bits(steps[k + 4].theta_rad),
                    bits(step.theta_rad),
                    "{level} {k}"
                );
            }
        }
    }
    assert!(far_steps(12, &earth, 0.0).is_empty(), "no relief, no march");
    // A small, rough body: the reach is held to a quarter turn.
    assert_eq!(
        bits(far_reach(&Spheroid::sphere(10_000.0), 30_000.0).value()),
        bits(FRAC_PI_2)
    );
}

/// A sphere of Earth's radius with a basin `radius` radians across about `centre`, 0 within and
/// `height_m` metres beyond, blended by the cells' spline.
fn basin_field(centre: [f64; 3], radius: f64, height_m: f64) -> CoarseField {
    let rim = math::cos(radius);
    FieldBuilder::new(Metres::new(6_371_000.0))
        .elevation(move |site| {
            Metres::new(if dot(site.dir, centre) < rim {
                height_m
            } else {
                0.0
            })
        })
        .build()
}

#[test]
fn horizon_far_field_sees_a_wall_of_height_h_at_distance_d() {
    let radius = 6_371_000.0;
    let centre = unit_dir([0.3, 0.2, 1.0]);
    let (wall, h) = (150_000.0 / radius, 10_000.0);
    let field = basin_field(centre, wall, h);
    let coarse = field.header().level().get();
    let key = PatchKey::containing(coarse + 4, centre).unwrap();
    let view = client_within_reach(&field, key);
    let synth = Synthesiser::new(&view, SEED);
    let map = horizon_of(&synth, &mut SynthCache::new(), key);
    assert!(map.is_complete(), "the view holds the reach");
    let figure = field.header().figure();
    // The cells blend the step over about two cells either side of the wall.
    let cell = 1.7 * FRAC_PI_2 / f64::from(1_u32 << coarse);
    let from = figure.surface_point(centre, 0.0);
    let up = figure.normal(centre);
    let at = |theta: f64| {
        let towards = unit_dir([1.0, 0.0, 0.0]);
        let t = unit_dir(sub(towards, scale(centre, dot(towards, centre))));
        let (s, c) = math::sin_cos(theta);
        unit_dir(add(scale(centre, c), scale(t, s)))
    };
    let seen = |theta: f64| elevation(from, up, figure.surface_point(at(theta), h)).value();
    // No ground stands higher than a wall of H at the blend's inner edge, and the march meets the
    // plateau itself within a step past its outer edge.
    let highest = seen(wall - 2.0 * cell);
    let lowest = seen((wall + 2.0 * cell) * GROWTH[1]);
    let formula = {
        let d = wall * radius;
        math::atan((h - d * d / (2.0 * radius)) / d)
    };
    assert!(lowest < formula, "{lowest} below the wall's {formula}");
    assert!(formula < highest, "{formula} below the bound {highest}");
    for b in 2..=6 {
        for a in 2..=6 {
            for k in 0..AZIMUTHS {
                let far = map.far(a, b, k).value();
                assert!(
                    far >= lowest - half_step() && far <= highest + half_step(),
                    "({a}, {b}) {k}: {far} outside [{lowest}, {highest}]"
                );
            }
        }
    }
}

#[test]
fn horizon_far_field_is_the_same_bits_at_points_adjacent_patches_share() {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let coarse = field.header().level().get();
    let level = coarse + 3;
    let a = patch_in(rim_cells(field, 1)[0], level);
    let next = |di: u32, dj: u32| PatchKey::new(a.face(), level, a.i() + di, a.j() + dj).unwrap();
    let (east, north, beyond) = (next(1, 0), next(0, 1), next(1, 1));
    let mut cells = BTreeSet::new();
    for key in [a, east, north, beyond] {
        cells.extend(cells_under(field, key));
    }
    let view = client(w, &cells, |_| code(20.0));
    let synth = Synthesiser::new(&view, SEED);
    let shared = RefCell::new(SynthCache::new());
    let map = |key| horizon_of(&synth, &mut shared.borrow_mut(), key);
    let (m_beyond, m_north, m_a, m_east) = (map(beyond), map(north), map(a), map(east));
    for k in 0..AZIMUTHS {
        for n in 0..=8 {
            assert_eq!(
                bits(m_a.far(8, n, k).value()),
                bits(m_east.far(0, n, k).value()),
                "east {n} {k}"
            );
            assert_eq!(
                bits(m_a.far(n, 8, k).value()),
                bits(m_north.far(n, 0, k).value()),
                "north {n} {k}"
            );
        }
        assert_eq!(
            bits(m_a.far(8, 8, k).value()),
            bits(m_beyond.far(0, 0, k).value()),
            "corner {k}"
        );
    }
    // The lattice's points are the same points at every level that holds them.
    for (i, j) in [(0, 0), (3, 5), (8, 8), (77, 1_000), (1_024, 1_024)] {
        let p = far_field_point(Face::NegY, 7, i, j);
        let q = far_field_point(Face::NegY, 8, 2 * i, 2 * j);
        assert_eq!((bits(p.u), bits(p.v)), (bits(q.u), bits(q.v)));
    }
    // And they are the patch's every eighth vertex.
    let p = far_field_point(a.face(), level, 8 * a.i() + 3, 8 * a.j() + 5);
    let dir = unit_dir(face_uv_to_xyz(p));
    let off = angle_between(dir, a.vertex_dir(24, 40));
    assert!(off < 1e-15, "the far point is the vertex, {off} rad off");
}

#[test]
fn horizon_far_field_is_independent_of_the_cache_and_the_order_of_bakes() {
    let (w, edge, surveyed) = straddling();
    let field = world(w);
    let coarse = field.header().level().get();
    let inside = patch_in(*surveyed.first().unwrap(), coarse + 2);
    let beside = PatchKey::new(inside.face(), inside.level(), inside.i() + 1, inside.j()).unwrap();
    let keys = [
        inside,
        beside,
        inside.parent().unwrap(),
        inside.children()[3],
        edge,
    ];
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let shared = RefCell::new(SynthCache::new());
    assert_order_independent(&keys, |&key| {
        horizon_of(&synth, &mut shared.borrow_mut(), key)
    });
    assert_order_independent(&keys, |&key| {
        horizon_of(&synth, &mut SynthCache::new(), key)
    });
    // The warm cache's maps are the cold ones.
    for key in keys {
        assert_eq!(
            horizon_of(&synth, &mut shared.borrow_mut(), key),
            horizon_of(&synth, &mut SynthCache::new(), key),
            "{key:?}"
        );
    }
    assert!(
        shared.borrow_mut().far_heights_mut().len() > 0,
        "the cache kept far heights"
    );
}

#[test]
fn horizon_far_field_from_a_cache_warmed_under_a_smaller_view_is_a_cold_bake_s() {
    // A worker's view only grows: a cache filled while the far field read the reference surface
    // beyond the survey answers as a fresh one once the view holds that ground.
    let (w, _, surveyed) = straddling();
    let field = world(w);
    let coarse = field.header().level().get();
    let inside = patch_in(*surveyed.first().unwrap(), coarse + 2);
    let small = client(w, &surveyed, |_| code(50.0));
    let mut warm = SynthCache::new();
    let before = horizon_of(&Synthesiser::new(&small, SEED), &mut warm, inside);
    assert!(
        !before.is_complete(),
        "the small view lacks the far field's ground"
    );
    let header = field.header();
    let reach = far_reach(&header.figure(), header.realised_relief().value()).value();
    let cell = 1.7 * FRAC_PI_2 / f64::from(1_u32 << coarse);
    let mut cells = cells_within(field, inside.vertex_dir(32, 32), reach + 2.0 * cell);
    cells.extend(surveyed.iter().copied());
    let grown = client_of(field, &cells);
    let synth = Synthesiser::new(&grown, SEED);
    let after = horizon_of(&synth, &mut warm, inside);
    assert!(after.is_complete(), "the grown view holds the reach");
    assert_eq!(after, horizon_of(&synth, &mut SynthCache::new(), inside));
    assert_ne!(
        after, before,
        "the far field reads the ground the view gained"
    );
}

// ---------------------------------------------------------------------------------------------
// The survey and the layer.
// ---------------------------------------------------------------------------------------------

#[test]
fn horizon_is_incomplete_where_the_far_field_reaches_beyond_the_view() {
    let (w, _, surveyed) = straddling();
    let field = world(w);
    let coarse = field.header().level().get();
    let inside = patch_in(*surveyed.first().unwrap(), coarse + 2);
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let bake = bake_terrain(&synth, &mut SynthCache::new(), inside, &HORIZON_ONLY, &[]).unwrap();
    // The bake is complete, its far field not: the survey is two cells, 85 km, across, and the far
    // field reaches some hundreds of kilometres.
    assert!(bake.layers().is_complete(), "the bake is complete");
    assert!(
        !bake.layers().horizon().is_complete(),
        "its far field reaches past the survey"
    );
    let whole = client_within_reach(field, inside);
    let synth = Synthesiser::new(&whole, SEED);
    let bake = bake_terrain(&synth, &mut SynthCache::new(), inside, &HORIZON_ONLY, &[]).unwrap();
    assert!(
        bake.layers().horizon().is_complete(),
        "the far field holds within the view"
    );
}

#[test]
fn horizon_names_the_cells_its_far_field_was_refused_for() {
    let (w, _, surveyed) = straddling();
    let field = world(w);
    let coarse = field.header().level().get();
    let inside = patch_in(*surveyed.first().unwrap(), coarse + 2);
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let map = horizon_of(&synth, &mut SynthCache::new(), inside);
    let refused = map.refused_cells().to_vec();
    assert!(!map.is_complete());
    assert!(!refused.is_empty(), "the far field reaches past the survey");
    assert!(refused.is_sorted_by(|a, b| a < b), "rising, each once");
    let header = field.header();
    let reach = far_reach(&header.figure(), header.realised_relief().value()).value();
    // A cell's centre lies within half its diagonal, about 1.2 of its mean side, of any point in it.
    let cell = 1.7 * FRAC_PI_2 / f64::from(1_u32 << coarse);
    for &index in &refused {
        let c = cell_at_index(coarse, index).unwrap();
        assert_eq!(
            view.resolution(c),
            None,
            "a refused cell is one the view lacks"
        );
        let off = angle_between(c.vertex_dir(32, 32), inside.vertex_dir(32, 32));
        assert!(
            off <= reach + 2.0 * cell,
            "{c:?} lies {off} rad off, past the reach"
        );
    }
    // A view that gains every cell named answers each height that named one: none is named again.
    let mut cells = surveyed.clone();
    cells.extend(refused.iter().map(|&i| cell_at_index(coarse, i).unwrap()));
    let grown = client_of(field, &cells);
    let again = horizon_of(
        &Synthesiser::new(&grown, SEED),
        &mut SynthCache::new(),
        inside,
    );
    assert!(
        again
            .refused_cells()
            .iter()
            .all(|i| refused.binary_search(i).is_err()),
        "a cell held is never named"
    );
    assert_ne!(again, map);
    // A worker's cache serves the field as it grows: warmed under the smaller view, it bakes the
    // grown view's map, its refused cells too, as a cold cache does.
    let mut warm = SynthCache::new();
    assert_eq!(horizon_of(&synth, &mut warm, inside), map);
    assert_eq!(
        horizon_of(&Synthesiser::new(&grown, SEED), &mut warm, inside),
        again
    );
    // A view within the reach names none, and the map is complete; a map skipped names none.
    let whole = client_within_reach(field, inside);
    let complete = horizon_of(
        &Synthesiser::new(&whole, SEED),
        &mut SynthCache::new(),
        inside,
    );
    assert!(complete.is_complete());
    assert_eq!(complete.refused_cells(), [] as [u32; 0]);
    assert_eq!(HorizonMap::default().refused_cells(), [] as [u32; 0]);
}

/// The coarse cells the height function reads over `field`, every cell held, at the unit
/// directions `dirs` at band level `level`.
fn cells_read(
    field: &CoarseField,
    level: u8,
    dirs: impl IntoIterator<Item = [f64; 3]>,
) -> BTreeSet<PatchKey> {
    let view = Recorder::new(field);
    let synth = Synthesiser::new(&view, SEED);
    let band = BandLevel::new(level).unwrap();
    let mut cache = SynthCache::new();
    view.take();
    for dir in dirs {
        synth.height_at(&mut cache, dir, band).unwrap();
    }
    view.take().into_iter().collect()
}

/// The coarse cells `key`'s bake reads over `field`, every cell held: its vertices and its
/// normals' samples at its level, and its even vertices at its parent's.
fn bake_reads(field: &CoarseField, key: PatchKey) -> BTreeSet<PatchKey> {
    let mut own = cells_read(
        field,
        key.level(),
        (0..=128).flat_map(|y| (0..=128).map(move |x| key.sample_dir(x, y, SampleGrid::Double))),
    );
    own.extend(cells_read(
        field,
        key.level() - 1,
        (0..=32_u8).flat_map(|y| (0..=32_u8).map(move |x| key.vertex_dir(2 * x, 2 * y))),
    ));
    own
}

/// The unit directions of the vertices of `key`'s near field's margin, row by row.
fn margin_dirs(key: PatchKey) -> Vec<[f64; 3]> {
    let grid = NearGrid::of(key);
    (grid.y0..=grid.y1)
        .flat_map(|y| (grid.x0..=grid.x1).map(move |x| (x, y)))
        .filter(|&(x, y)| NearGrid::own(x, y).is_none())
        .map(|(x, y)| grid_dir(key, x, y))
        .collect()
}

#[test]
fn horizon_names_the_cells_its_margin_alone_was_refused_for() {
    // A client's view holds its survey's cells and their margin of `SYNTHESIS_MARGIN_CELLS`, five
    // cells, which holds every cell the near field's margin of a patch at the field's level or
    // finer reads, beside every cell its own vertices read. A patch three levels coarser, eight
    // cells a side, has a margin a cell deep, whose outer vertices read a column of cells its own
    // do not: a survey whose margin's edge falls between the two refuses the near field's margin
    // alone.
    let field = flat_field(1_500.0);
    let header = field.header();
    let coarse = header.level().get();
    let key = PatchKey::new(Face::PosX, coarse - 3, 3, 3).unwrap();
    // With no relief the far field's reach is nought: its march takes no step and reads nothing.
    assert!(
        far_steps(
            key.level(),
            &header.figure(),
            header.realised_relief().value()
        )
        .is_empty(),
        "the far march takes no step"
    );
    // What the bake reads, and what its near field's margin reads.
    let own = bake_reads(&field, key);
    let margin = margin_dirs(key);
    let margin_reads = cells_read(&field, key.level(), margin.iter().copied());
    assert!(
        own.iter()
            .chain(&margin_reads)
            .all(|c| c.face() == Face::PosX),
        "the patch and its margin read the face's cells alone"
    );
    let west = own.iter().map(|c| c.i()).min().unwrap();
    assert!(
        margin_reads.iter().any(|c| c.i() < west),
        "the margin reads further toward −s than the patch's own vertices"
    );
    // A survey from five cells east of the bake's westmost column, so that its margin stops at that
    // column, and reaching far enough every other way.
    let (i0, j0) = (key.i() << 3, key.j() << 3);
    let surveyed: BTreeSet<PatchKey> = (j0 - 14..=j0 + 21)
        .flat_map(|j| {
            (west + 5..=i0 + 21).map(move |i| PatchKey::new(Face::PosX, coarse, i, j).unwrap())
        })
        .collect();
    let view = client_of(&field, &surveyed);
    assert!(
        own.iter().all(|&c| view.resolution(c).is_some()),
        "the view holds every cell the bake reads"
    );
    let synth = Synthesiser::new(&view, SEED);
    let bake = bake_terrain(&synth, &mut SynthCache::new(), key, &HORIZON_ONLY, &[]).unwrap();
    assert!(
        bake.layers().is_complete(),
        "the view holds the patch's own ground"
    );
    let map = bake.layers().horizon();
    assert!(!map.is_complete(), "the margin reaches past the view");
    // Each margin height the view refuses names the first cell its reads lacked, as the height
    // function names it, and the map names those and no other.
    let band = BandLevel::new(key.level()).unwrap();
    let mut cache = SynthCache::new();
    let expected: BTreeSet<u32> = margin
        .iter()
        .filter_map(|&dir| match synth.height_at(&mut cache, dir, band) {
            Ok(_) => None,
            Err(QueryHeightError::NotSurveyed(cell)) => Some(cell_index(cell)),
        })
        .collect();
    assert!(!expected.is_empty(), "the view refuses margin heights");
    assert_eq!(
        map.refused_cells(),
        expected.iter().copied().collect::<Vec<_>>().as_slice()
    );
    let next = beside(key);
    for &index in &expected {
        let c = cell_at_index(coarse, index).unwrap();
        assert_eq!(view.resolution(c), None, "{c:?} is one the view lacks");
        assert!(c.i() < west, "{c:?} lies past what the bake reads");
        let under = PatchKey::containing(key.level(), c.vertex_dir(32, 32)).unwrap();
        assert!(next.contains(&under), "{c:?} lies under a patch beside it");
    }
    // A view grown by the cells named answers each height that named one: none is named again (a
    // height whose reads lacked more than one cell may name the next).
    let mut cells = surveyed;
    cells.extend(expected.iter().map(|&i| cell_at_index(coarse, i).unwrap()));
    let again = horizon_of(
        &Synthesiser::new(&client_of(&field, &cells), SEED),
        &mut SynthCache::new(),
        key,
    );
    assert!(
        again.refused_cells().iter().all(|i| !expected.contains(i)),
        "a cell held is never named"
    );
    // A view grown by every cell the margin reads holds the margin's ground: the map is complete.
    cells.extend(margin_reads);
    let grown = client_of(&field, &cells);
    let synth_grown = Synthesiser::new(&grown, SEED);
    let complete = horizon_of(&synth_grown, &mut SynthCache::new(), key);
    assert!(complete.is_complete(), "the grown view holds the margin");
    assert_eq!(complete.refused_cells(), [] as [u32; 0]);
    // A cache warmed under the smaller view, which kept no refused height, bakes the grown view's
    // map as a cold one does.
    let mut warm = SynthCache::new();
    assert_eq!(&horizon_of(&synth, &mut warm, key), map);
    assert_eq!(horizon_of(&synth_grown, &mut warm, key), complete);
}

#[test]
fn horizon_skipped_leaves_the_map_empty_and_the_rest_as_baked() {
    let (w, edge, surveyed) = straddling();
    let view = client(w, &surveyed, |_| code(50.0));
    let synth = Synthesiser::new(&view, SEED);
    let skipped = TerrainBakeOptions {
        horizon: HorizonLayer::Skipped,
        ..MESH
    };
    let with = bake_terrain(&synth, &mut SynthCache::new(), edge, &MESH, &[]).unwrap();
    let without = bake_terrain(&synth, &mut SynthCache::new(), edge, &skipped, &[]).unwrap();
    let map = without.layers().horizon();
    assert!(map.is_empty(), "a skipped map is empty");
    assert!(map.is_complete(), "and reads nothing");
    assert_eq!(with.layers().horizon().near_words().len(), 65 * 65 * 4);
    assert_eq!(with.layers().horizon().far_words().len(), 9 * 9 * 4);
    assert_eq!(without.patch(), with.patch());
    assert_eq!(without.layers().weights(), with.layers().weights());
    assert_eq!(without.layers().survey_mask(), with.layers().survey_mask());
    assert_eq!(without.layers().is_complete(), with.layers().is_complete());
}

// ---------------------------------------------------------------------------------------------
// The azimuths.
// ---------------------------------------------------------------------------------------------

#[test]
fn horizon_grid_azimuth_is_k_eighths_of_a_turn_along_the_grid_s_steps() {
    let mut rng = Lcg::new(0x686f_7269_7a6f_6e33);
    for face in Face::ALL {
        for _ in 0..20 {
            let level = 16;
            let last = (1_u32 << level) - 1;
            let pick = |rng: &mut Lcg| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a patch index below 2^16"
                )]
                let n = (rng.next_f64() * f64::from(last)) as u32;
                n.clamp(1, last - 1)
            };
            let key = PatchKey::new(face, level, pick(&mut rng), pick(&mut rng)).unwrap();
            let at = key.vertex_dir(32, 32);
            for (k, &[dx, dy]) in AZIMUTH_STEPS.iter().enumerate() {
                // The chord to the vertex a step on, against the grid's analytic tangents.
                let to = |x: i8, y: i8| {
                    key.vertex_dir(
                        32_u8.saturating_add_signed(x),
                        32_u8.saturating_add_signed(y),
                    )
                };
                let chord = sub(to(dx, dy), to(-dx, -dy));
                let phi = grid_azimuth(face, at, chord).unwrap().value();
                #[expect(clippy::cast_precision_loss, reason = "eight azimuths")]
                let expected = FRAC_PI_4 * k as f64;
                let off = (phi - expected + PI).rem_euclid(TAU) - PI;
                assert!(off.abs() < 1e-6, "{face:?} {k}: {phi} against {expected}");
            }
            for towards in [[0.0; 3], at, scale(at, -3.0)] {
                assert_eq!(
                    grid_azimuth(face, at, towards),
                    None,
                    "{towards:?}: no azimuth"
                );
            }
        }
    }
}

#[test]
fn horizon_azimuth_interpolation_is_continuous() {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let coarse = field.header().level().get();
    let key = patch_in(rim_cells(field, 1)[0], coarse + 3);
    let view = client(w, &cells_under(field, key), |_| code(20.0));
    let synth = Synthesiser::new(&view, SEED);
    let map = horizon_of(&synth, &mut SynthCache::new(), key);
    let steps = 4_000_u32;
    let dphi = TAU / f64::from(steps);
    for (x, y) in [(0, 0), (8, 16), (13, 51), (32, 32), (64, 7), (64, 64)] {
        let at = |phi: f64| map.elevation_at(x, y, Radians::new(phi)).value();
        // The near field's and the far field's steepest change between two azimuths.
        let mut jump = 0.0_f64;
        for k in 0..AZIMUTHS {
            let k1 = (k + 1) % AZIMUTHS;
            jump = num::max(jump, (map.near(x, y, k1) - map.near(x, y, k)).value().abs());
            for b in 0..=8 {
                for a in 0..=8 {
                    jump = num::max(jump, (map.far(a, b, k1) - map.far(a, b, k)).value().abs());
                }
            }
        }
        let bound = jump * dphi / FRAC_PI_4 * 1.000_001 + 1e-15;
        let mut previous = at(0.0);
        for n in 1..=steps {
            let now = at(dphi * f64::from(n));
            assert!((now - previous).abs() <= bound, "({x}, {y}) at {n}");
            previous = now;
        }
        // Round the turn and back to the start, and on the azimuths themselves.
        assert!(
            (at(TAU - 1e-12) - at(0.0)).abs() <= bound,
            "({x}, {y}) round the turn"
        );
        assert!(
            (at(-dphi) - at(TAU - dphi)).abs() <= 1e-12,
            "({x}, {y}) a turn back"
        );
        for k in 0..AZIMUTHS {
            #[expect(clippy::cast_precision_loss, reason = "eight azimuths")]
            let phi = FRAC_PI_4 * k as f64;
            let near = map.near(x, y, k).value();
            assert!(at(phi) >= near - 1e-15, "({x}, {y}) {k}");
        }
    }
    // At a far point, on an azimuth, the lookup is the larger of the two fields there.
    for k in 0..AZIMUTHS {
        #[expect(clippy::cast_precision_loss, reason = "eight azimuths")]
        let phi = FRAC_PI_4 * k as f64;
        let expected = num::max(map.near(24, 40, k).value(), map.far(3, 5, k).value());
        assert_eq!(
            bits(map.elevation_at(24, 40, Radians::new(phi)).value()),
            bits(expected)
        );
    }
}

// ---------------------------------------------------------------------------------------------
// R10.T8.b's fixtures: the grid azimuth and a parent's and child's maps.
// ---------------------------------------------------------------------------------------------

/// A direction's bits, three 16-digit words.
fn dir_bits(v: [f64; 3]) -> String {
    format!(
        "{:016x} {:016x} {:016x}",
        bits(v[0]),
        bits(v[1]),
        bits(v[2])
    )
}

/// The fixture of [`grid_azimuth`] that R10.T8.b's TypeScript twin `gridAzimuth` reads
/// (decision-r10-t8a §2): `azimuth <face> <at ×3> <towards ×3> <φ>` by bits, φ `none` where the
/// direction has no tangential part, over every face, the centres of faces 2 and 5 (the poles), a
/// cube corner, both sides of s = ½, a point just past a face's edge (a skirt's) and the zenith.
fn grid_azimuth_text() -> String {
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    let mut rng = Lcg::new(0x6772_6964_617a_6930);
    let unit = |rng: &mut Lcg| loop {
        let v = [
            2.0 * rng.next_f64() - 1.0,
            2.0 * rng.next_f64() - 1.0,
            2.0 * rng.next_f64() - 1.0,
        ];
        let n = dot(v, v).sqrt();
        if n > 0.1 && n <= 1.0 {
            break scale(v, 1.0 / n);
        }
    };
    let half_less = st_to_uv(0.5 - 1e-6);
    let half_more = st_to_uv(0.5 + 1e-6);
    // (u, v) on each face: two general points, the centre, a corner, both sides of s = ½ in u and
    // in v, and a point just past the edge u = 1.
    let places = [
        (0.3, -0.4),
        (-0.7, 0.6),
        (0.0, 0.0),
        (1.0, 1.0),
        (half_less, 0.2),
        (half_more, 0.2),
        (-0.5, half_less),
        (-0.5, half_more),
        (1.02, -0.3),
    ];
    for face in Face::ALL {
        for &(u, v) in &places {
            let at = face_uv_to_xyz(FaceUv { face, u, v });
            let p = unit_dir(at);
            let mut towards = vec![unit(&mut rng), unit(&mut rng), unit(&mut rng)];
            // A sun low over the ground and one high, both in a general azimuth.
            let t = unit_dir(sub(towards[0], scale(p, dot(towards[0], p))));
            towards.push(add(scale(p, math::sin(0.05)), scale(t, math::cos(0.05))));
            towards.push(add(scale(p, math::sin(1.4)), scale(t, math::cos(1.4))));
            // The zenith and the nadir have none.
            towards.extend([p, scale(p, -2.0)]);
            for w in towards {
                let phi = grid_azimuth(face, at, w).map_or_else(
                    || "none".to_owned(),
                    |a| format!("{:016x}", bits(a.value())),
                );
                out.line(&format!(
                    "azimuth {} {} {} {phi}",
                    face.index(),
                    dir_bits(at),
                    dir_bits(w)
                ));
            }
        }
    }
    out.finish()
}

#[test]
fn horizon_grid_azimuth_matches_its_fixture() {
    golden!("horizon/grid_azimuth", &grid_azimuth_text());
}

/// The child's vertices the blend fixture samples: its even vertices every eighth (its parent's
/// vertices), and a few odd ones.
fn blend_samples() -> Vec<(u8, u8)> {
    let mut samples: Vec<(u8, u8)> = (0..=8_u8)
        .flat_map(|b| (0..=8_u8).map(move |a| (8 * a, 8 * b)))
        .collect();
    samples.extend([(5, 3), (13, 51), (63, 1), (34, 18)]);
    samples
}

/// The azimuths the blend fixture's lookups are taken at, radians: on knots and between, and
/// round the turn.
const BLEND_AZIMUTHS: [f64; 5] = [0.0, 0.3, 1.9, PI, 5.9];

/// The words of `map`'s texel `texel` of `words`, as a fixture line's tail.
fn texel_words(words: &[u32], texel: usize) -> String {
    words[texel * WORDS_PER_TEXEL..(texel + 1) * WORDS_PER_TEXEL]
        .iter()
        .map(|w| format!("{w:08x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The fixture of a child's and its parent's maps that R10.T8.b's blend twin reads
/// (decision-r10-t8a §1): `patch <child|parent> <face> <level> <i> <j>`, each map's far field
/// whole as `far <map> <a> <b> <words ×4>`, its near field at the sampled vertices as `near <map>
/// <x> <y> <words ×4>` (the child's at [`blend_samples`], the parent's at the parent vertices under
/// the child's even ones), and `elevation <map> <x> <y> <azimuth> <elevation>` by bits,
/// [`HorizonMap::elevation_at`] there, at [`BLEND_AZIMUTHS`].
fn blend_text() -> String {
    let w = SyntheticWorld::MoonLike;
    let field = world(w);
    let coarse = field.header().level().get();
    let child = patch_in(rim_cells(field, 1)[0], coarse + 3);
    let parent = child.parent().unwrap();
    let view = client_within_reach(field, child);
    let synth = Synthesiser::new(&view, SEED);
    let mut cache = SynthCache::new();
    let maps = [
        ("child", child, horizon_of(&synth, &mut cache, child)),
        ("parent", parent, horizon_of(&synth, &mut cache, parent)),
    ];
    let (qx, qy) = (child.i() & 1, child.j() & 1);
    let to_parent = |(x, y): (u8, u8)| {
        let q = |q: u32, n: u8| u8::try_from(u32::midpoint(64 * q, u32::from(n))).unwrap();
        (q(qx, x), q(qy, y))
    };
    let mut out = GoldenWriter::new();
    out.header(crate::generator_version());
    for (name, key, map) in &maps {
        out.line(&format!(
            "patch {name} {} {} {} {}",
            key.face().index(),
            key.level(),
            key.i(),
            key.j()
        ));
        let vertices: Vec<(u8, u8)> = if *name == "child" {
            blend_samples()
        } else {
            blend_samples()
                .into_iter()
                .filter(|&(x, y)| x % 2 == 0 && y % 2 == 0)
                .map(to_parent)
                .collect()
        };
        for b in 0..=8_u8 {
            for a in 0..=8_u8 {
                let texel = usize::from(b) * FAR_SIDE + usize::from(a);
                out.line(&format!(
                    "far {name} {a} {b} {}",
                    texel_words(map.far_words(), texel)
                ));
            }
        }
        for &(x, y) in &vertices {
            let texel = usize::from(y) * PATCH_VERTICES + usize::from(x);
            out.line(&format!(
                "near {name} {x} {y} {}",
                texel_words(map.near_words(), texel)
            ));
        }
        for &(x, y) in &vertices {
            for phi in BLEND_AZIMUTHS {
                let e = map.elevation_at(x, y, Radians::new(phi)).value();
                out.line(&format!(
                    "elevation {name} {x} {y} {:016x} {:016x}",
                    bits(phi),
                    bits(e)
                ));
            }
        }
    }
    out.finish()
}

#[test]
fn horizon_blend_fixture_matches() {
    golden!("horizon/blend", &blend_text());
}

/// The tests that read files, which the browser target cannot (plan R04, Design note 12).
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::Path;

    /// Every golden file of `tests/golden/horizon/` carries `GENERATOR_VERSION`, and there are
    /// three: the encoding's, the grid azimuth's and the blend's.
    #[test]
    fn every_horizon_golden_carries_the_generator_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("horizon");
        let expected = format!("# generator_version = {}", crate::generator_version());
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("the horizon's golden directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            let text = std::fs::read_to_string(&path).expect("a golden file is readable");
            assert_eq!(
                text.lines().next(),
                Some(expected.as_str()),
                "{}",
                path.display()
            );
            seen += 1;
        }
        assert_eq!(seen, 3);
    }
}
