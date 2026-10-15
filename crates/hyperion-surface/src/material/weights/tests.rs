//! The class weights' tests (plan R10, T2): the exceedance and the bytes, the class at the finest
//! level, and the coarse weights against the finest level's classes counted over each texel's
//! footprint, on the synthetic worlds.
//!
//! The agreement is the total variation between a texel's weights and the classes counted at a
//! grid of its footprint's points, half the sum over materials of |weight − fraction|: the share of
//! the texel the weights give to the wrong material. A texel at band level m is the square of one
//! vertex spacing about a vertex of the level's lattice, in the face's (s, t).

use std::collections::{BTreeMap, BTreeSet};

use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use hyperion_base::rng::DetailSeed;
use hyperion_base::units::{Kelvin, MetresPerSecondSquared, PerSquareKilometre};

use super::*;
use crate::craters::{
    CraterParams, CraterParamsParts, LUNAR_IMPACT_VELOCITY, LUNAR_PROJECTILE_DENSITY, LUNAR_TARGET,
    SOLAR_SYSTEM_AGE, Screening,
};
use crate::cube::{Face, st_to_uv, uv_to_st, xyz_to_face_uv};
use crate::field::{
    CoarseField, MaterialPalette, MechanicsFamily, PaletteEntry, PaletteRole, cell_at_index,
};
use crate::material::{MaterialClass, material_at};
use crate::substance_key::SubstanceKey;
use crate::synth::relief::ReliefStyle;
use crate::synth::test_support::{Held, SEED, random_cells, random_points_in, rim_cells, world};
use crate::synth::{BandSpectrum, STRUCTURAL_RMS_STAGNANT_LID, SpectrumShape};
use crate::test_sizes::per_target;
use crate::testing::{ClimateSample, FieldBuilder, SyntheticWorld};

/// The cold-trapped field's latitude, poleward of which its cells hold their partial ice: 85°.
const COLD_TRAP_LATITUDE_DEG: f64 = 85.0;

/// An airless, cratered body like the Moon whose cells poleward of 85° hold `share` of water ice
/// (T1.b's test field of that name, illustrative values: a feldspathic lid and water ice, the Moon's
/// radius, spectrum and crater contract, and a still climate of 100 + 150 cos φ kelvin), the
/// synthetic worlds having no partial ice on land.
pub(super) fn cold_trapped(share: f64) -> CoarseField {
    let radius = 1.7374e6;
    let entry = |key: &str, role, density, transition_k, mechanics| PaletteEntry {
        substance: SubstanceKey::new(key).expect("a test key is in the grammar"),
        role,
        normal_albedo_bvr: [0.5; 3],
        phase_row: 0,
        grain_density_kg_m3: density,
        transition: Kelvin::new(transition_k),
        mechanics,
        liquid: None,
    };
    let latitude = |dir: [f64; 3]| math::asin(dir[2].clamp(-1.0, 1.0));
    FieldBuilder::new(Metres::new(radius))
        .palette(
            MaterialPalette::new(vec![
                entry(
                    "feldspathic",
                    PaletteRole::PrimaryCrust,
                    2_900.0,
                    1_373.0,
                    MechanicsFamily::Silicate,
                ),
                entry(
                    "H2O",
                    PaletteRole::Ice,
                    917.0,
                    273.15,
                    MechanicsFamily::WaterIce,
                ),
            ])
            .expect("a test palette is sorted and valid"),
        )
        .crust_palette([None, None, Some(0), None])
        .spectrum(BandSpectrum::anchored(
            SpectrumShape::SILICATE,
            Metres::new(radius),
            STRUCTURAL_RMS_STAGNANT_LID,
        ))
        .crater_params(
            CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::new(0.05),
                screening: Screening::None,
                gravity: MetresPerSecondSquared::new(1.62),
                k_target: 1.0,
                impact_velocity: Some(LUNAR_IMPACT_VELOCITY),
                projectile_density: LUNAR_PROJECTILE_DENSITY,
                target: LUNAR_TARGET,
                system_age: SOLAR_SYSTEM_AGE,
            })
            .expect("the Moon's crater contract is valid"),
        )
        .climate(move |dir| {
            ClimateSample::still(Kelvin::new(100.0 + 150.0 * math::cos(latitude(dir))))
        })
        .ice(move |site, _| {
            if latitude(site.dir).abs() > COLD_TRAP_LATITUDE_DEG.to_radians() {
                share
            } else {
                0.0
            }
        })
        .build()
}

/// The cold-trapped field's polar cells, those holding a partial ice share, by cell index.
pub(super) fn cold_trap_cells(field: &CoarseField) -> Vec<PatchKey> {
    let level = field.header().level();
    (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).expect("a cell of the level"))
        .filter(|&k| field.cell(k).is_some_and(|c| (1..u8::MAX).contains(&c.ice)))
        .collect()
}

/// The worlds with a palette and relief: every synthetic world but the two flat ones.
const BODIES: [SyntheticWorld; 4] = [
    SyntheticWorld::EarthLike,
    SyntheticWorld::MarsLike,
    SyntheticWorld::MoonLike,
    SyntheticWorld::CeresLike,
];

/// The class map's band level (Design note 8).
const CLASS_MAP_LEVEL: u8 = 4;

/// The agreement the plan asks of the coarse weights in the mean over a level's texels: ten
/// percentage points.
const AGREEMENT: f64 = 0.10;

/// The probability outside a disc, from a polar sum independent of [`exceedance`]'s: ∫₀ᵗ r ÷ v
/// exp(−(r − m)² ÷ 2v) e^(−rm/v) I₀(rm ÷ v) dr, by Simpson's rule in r over 4,000 steps, and the
/// scaled Bessel function by the trapezoid rule over its angle, 1,000 steps, exact to rounding for a
/// periodic integrand this smooth.
fn polar_exceedance(mean: f64, variance: f64, threshold: f64) -> f64 {
    let scaled_i0 = |x: f64| {
        let n = 1_000_u32;
        let step = core::f64::consts::PI / f64::from(n);
        (0..=n)
            .map(|k| {
                let w = if k == 0 || k == n { 0.5 } else { 1.0 };
                w * math::exp(x * (math::cos(f64::from(k) * step) - 1.0))
            })
            .sum::<f64>()
            / f64::from(n)
    };
    let density = |r: f64| {
        r / variance
            * math::exp(-(r - mean) * (r - mean) / (2.0 * variance))
            * scaled_i0(r * mean / variance)
    };
    let n = 4_000_u32;
    let h = threshold / f64::from(n);
    let mut sum = density(0.0) + density(threshold);
    for k in 1..n {
        let w = if k % 2 == 1 { 4.0 } else { 2.0 };
        sum += w * density(f64::from(k) * h);
    }
    1.0 - sum * h / 3.0
}

#[test]
fn material_weights_the_rayleigh_tail_at_a_zero_mean() {
    for (variance, threshold) in [(0.01, 0.84), (0.3, 0.84), (1.0, 0.1), (4.0, 1.73)] {
        let expected = math::exp(-threshold * threshold / (2.0 * variance));
        let got = exceedance(0.0, variance, threshold);
        assert!(
            (got - expected).abs() < 1e-9,
            "{variance} {threshold}: {got} against {expected}"
        );
    }
}

#[test]
fn material_weights_the_exceedance_matches_an_independent_polar_sum() {
    // Each case's reference sums 4 × 10⁶ exponentials. The browser target takes 24 of the 70
    // cases: the means either side of the cliff's tangent and the grid's ends, at the variances'
    // ends and middle (`crate::test_sizes`).
    let means: &[f64] = per_target(
        &[0.05, 0.3, 0.8, 0.839, 0.85, 1.2, 2.0],
        &[0.05, 0.839, 0.85, 2.0],
    );
    let variances: &[f64] = per_target(&[1e-4, 1e-3, 0.01, 0.1, 1.0], &[1e-4, 0.01, 1.0]);
    let mut worst: f64 = 0.0;
    for &mean in means {
        for &variance in variances {
            for threshold in [
                math::tan(CLIFF_SLOPE.value()),
                math::tan(SNOW_SHED_SLOPE.value()),
            ] {
                let got = exceedance(mean, variance, threshold);
                let expected = polar_exceedance(mean, variance, threshold);
                worst = worst.max((got - expected).abs());
                assert!(
                    (got - expected).abs() < 1e-6,
                    "m {mean}, v {variance}, t {threshold}: {got} against {expected}"
                );
            }
        }
    }
    assert!(worst < 1e-6, "{worst}");
}

#[test]
fn material_weights_the_exceedance_falls_with_the_threshold_and_rises_with_the_mean() {
    for variance in [1e-3, 0.05, 0.5] {
        let mut last = 1.0;
        for k in 1..=40 {
            let p = exceedance(0.5, variance, 0.05 * f64::from(k));
            assert!(p <= last + 1e-9, "{variance} at {k}: {p} after {last}");
            last = p;
        }
        let mut last = 0.0;
        for k in 0..=40 {
            let p = exceedance(0.05 * f64::from(k), variance, 0.84);
            assert!(p >= last - 1e-9, "{variance} at {k}: {p} after {last}");
            last = p;
        }
    }
}

#[test]
fn material_weights_the_blend_is_linear_across_each_threshold() {
    for (degrees, expected) in [
        (20.0, [1.0, 0.0, 0.0]),
        (35.0, [1.0, 0.0, 0.0]),
        (37.0, [0.8, 0.2, 0.0]),
        (40.0, [0.5, 0.5, 0.0]),
        (44.0, [0.1, 0.9, 0.0]),
        (50.0, [0.0, 1.0, 0.0]),
        (58.0, [0.0, 0.7, 0.3]),
        (60.0, [0.0, 0.5, 0.5]),
        (70.0, [0.0, 0.0, 1.0]),
    ] {
        let shares = blend(f64::to_radians(degrees));
        for ((steepness, share), want) in shares.into_iter().zip(expected) {
            assert!(
                (share - want).abs() < 1e-9,
                "{degrees}°, {steepness:?}: {share} against {want}"
            );
        }
    }
}

/// A material of class `class` and entry `entry`.
fn ground(class: MaterialClass, entry: Option<u8>) -> GroundMaterial {
    GroundMaterial::of(class, entry)
}

#[test]
fn material_weights_the_authoritative_class_takes_the_largest_weight_ties_included() {
    let soil = ground(MaterialClass::Soil, Some(0));
    let bedrock = ground(MaterialClass::Bedrock, Some(0));
    // An even blend at the threshold: bedrock's code is the lower, and soil decides.
    let mut shares = Shares::default();
    shares.add(bedrock, 0.5);
    shares.add(soil, 0.5);
    let weights = shares.into_weights(First::Authoritative(soil));
    assert_eq!(weights.largest(), soil);
    assert_eq!(weights.weights(), [128, 127, 0, 0, 0, 0, 0, 0]);
    // Without an authoritative class the tie goes to the lower code.
    let mut shares = Shares::default();
    shares.add(soil, 0.5);
    shares.add(bedrock, 0.5);
    let weights = shares.into_weights(First::ByDiscriminant);
    assert_eq!(weights.largest(), bedrock);
    assert_eq!(weights.palette()[..2], [Some(bedrock), Some(soil)]);
    // An authoritative class with the smaller share still takes the largest byte.
    let mut shares = Shares::default();
    shares.add(bedrock, 0.7);
    shares.add(soil, 0.3);
    let weights = shares.into_weights(First::Authoritative(soil));
    assert_eq!(weights.largest(), soil);
    assert_eq!(weights.weights()[..2], [128, 127]);
    // Three even shares.
    let sand = ground(MaterialClass::Sand, Some(1));
    let mut shares = Shares::default();
    for m in [bedrock, soil, sand] {
        shares.add(m, 1.0 / 3.0);
    }
    let weights = shares.into_weights(First::Authoritative(sand));
    assert_eq!(weights.largest(), sand);
    assert_eq!(weights.weights()[..3], [86, 85, 84]);
}

#[test]
fn material_weights_keep_the_eight_likeliest_and_sum_to_255() {
    let mut shares = Shares::default();
    let classes = &MaterialClass::ALL[..11];
    for (k, &class) in (1_u32..).zip(classes) {
        shares.add(ground(class, None), f64::from(k));
    }
    let weights = shares.into_weights(First::ByDiscriminant);
    assert_eq!(weights.len(), MAX_CLASSES);
    assert_eq!(
        weights.weights().iter().map(|&w| u32::from(w)).sum::<u32>(),
        u32::from(WEIGHT_TOTAL)
    );
    let held: Vec<MaterialClass> = weights.iter().map(|(m, _)| m.class()).collect();
    assert_eq!(held, classes[3..].iter().rev().copied().collect::<Vec<_>>());
    assert!(weights.weights().windows(2).all(|w| w[0] >= w[1]));
    // A material whose share rounds to no byte is not held.
    let mut shares = Shares::default();
    shares.add(ground(MaterialClass::Soil, None), 1.0);
    shares.add(ground(MaterialClass::Sand, None), 1e-6);
    let weights = shares.into_weights(First::ByDiscriminant);
    assert_eq!(weights.len(), 1);
    assert_eq!(weights.weights()[0], WEIGHT_TOTAL);
}

#[test]
fn material_weights_the_height_cases_sum_to_one_each_strictly_inside_its_own_band() {
    let breaks = [-120.0, 0.0, 1_500.0];
    // The band a height lies strictly inside, or `None` at a break.
    let band = |h: f64| {
        if breaks.iter().any(|&b| (h - b).abs() < 1e-9) {
            None
        } else {
            Some(breaks.iter().filter(|&&b| b < h).count())
        }
    };
    for (mean, variance) in [(-500.0, 1e4), (10.0, 4e4), (1_499.0, 1.0), (3_000.0, 1e6)] {
        let cases = height_cases(&breaks, mean, variance);
        let total: f64 = cases.iter().map(|&(_, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-12, "{mean}: {total}");
        let bands: Vec<Option<usize>> = cases.iter().map(|&(h, _)| band(h)).collect();
        assert!(
            bands.iter().all(Option::is_some),
            "{mean}: {cases:?} at a break"
        );
        let distinct: BTreeSet<usize> = bands.iter().flatten().copied().collect();
        assert_eq!(distinct.len(), cases.len(), "{mean}: two cases in one band");
    }
    // The mean's own band is asked at the mean.
    let cases = height_cases(&breaks, 10.0, 4e4);
    assert!(cases.iter().any(|&(h, _)| (h - 10.0).abs() < 1e-12));
    assert_eq!(height_cases(&breaks, 10.0, 0.0), vec![(10.0, 1.0)]);
}

/// The cells a test of a world's sites reads: its hard cells and some at random.
fn site_cells(field: &CoarseField, synth: &Synthesiser<'_, CoarseField>) -> Vec<PatchKey> {
    let mut rng = Lcg::new(0x0073_6974_6573);
    let mut cells = hard_cells(field, synth, 6);
    cells.extend(random_cells(field.header().level().get(), 24, &mut rng));
    cells
}

#[test]
fn material_weights_a_rule_on_the_height_changes_only_across_a_listed_break() {
    // From 12 km below the datum to 12 km above it, in 10 m steps: every pair of neighbouring
    // heights whose materials differ, at any steepness and either ice, has a break between them,
    // to a centimetre.
    let mut changes = 0;
    for &w in &BODIES {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        for key in site_cells(field, &synth) {
            let cell = field.cell(key).expect("a whole field holds every cell");
            let site = Site::of(field, key, cell).expect("held");
            let breaks = site.height_breaks();
            assert!(breaks.windows(2).all(|b| b[0] < b[1]), "{breaks:?}");
            for steepness in [Steepness::Loose, Steepness::Cliff, Steepness::Shed] {
                for ice in [false, true] {
                    let at = |h: f64| site.facts(h, steepness, ice, false).decide().1;
                    let mut last = at(-12_000.0);
                    for k in 1..=2_400 {
                        let h = -12_000.0 + 10.0 * f64::from(k);
                        let now = at(h);
                        if now != last {
                            changes += 1;
                            assert!(
                                breaks.iter().any(|&b| b > h - 10.01 && b <= h + 0.01),
                                "{w:?} {key:?} {steepness:?} ice {ice}: {last:?} to {now:?} \
                                 between {} and {h} m, breaks {breaks:?}",
                                h - 10.0
                            );
                        }
                        last = now;
                    }
                }
            }
        }
    }
    assert!(
        changes > 0,
        "some rule on the height changes on these sites"
    );
}

#[test]
fn material_weights_the_steepness_cases_sum_to_one_and_tend_to_the_level_s_own() {
    let mut rng = Lcg::new(0x0073_7465_6570);
    for &w in &BODIES {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let header = field.header();
        let level = BandLevel::new(finest_band(header).get() - 6).expect("a band level");
        for at in finest_points(field, &synth, 60, &mut rng) {
            let dir = unit_dir(face_uv_to_xyz(at));
            let sample = synth.height_at(&mut cache, dir, level).expect("held");
            let own = Steepness::of(cos_slope(header, dir, &sample));
            for slope2 in [1e-14, 1e-3, 0.1, 1.0] {
                let cases = steepness_cases(header, dir, &sample, slope2);
                let total: f64 = cases.iter().map(|&(_, p)| p).sum();
                assert!((total - 1.0).abs() < 1e-12, "{w:?} at {at:?}: {total}");
                assert!(cases.iter().all(|&(_, p)| p >= 0.0), "{cases:?}");
            }
            // As the omitted slopes vanish, the level's own steepness takes all of it, unless the
            // level's slope is within a hair of a threshold.
            let tan = |c: f64| (1.0 - c * c).sqrt() / c;
            let slope = tan(cos_slope(header, dir, &sample));
            let near = [CLIFF_SLOPE, SNOW_SHED_SLOPE]
                .iter()
                .any(|t| (slope - math::tan(t.value())).abs() < 1e-5);
            if !near {
                let cases = steepness_cases(header, dir, &sample, 1e-14);
                let p = cases
                    .iter()
                    .find(|&&(s, _)| s == own)
                    .map_or(0.0, |&(_, p)| p);
                assert!(
                    p > 1.0 - 1e-9,
                    "{w:?} at {at:?}: {own:?} takes {p}, {cases:?}"
                );
            }
        }
    }
}

/// The face and the (s, t) of the unit direction `dir`.
fn st_of(dir: [f64; 3]) -> (Face, f64, f64) {
    let at = xyz_to_face_uv(dir);
    (at.face, uv_to_st(at.u), uv_to_st(at.v))
}

/// The point of `face` at (`s`, `t`).
fn uv_at(face: Face, s: f64, t: f64) -> FaceUv {
    FaceUv {
        face,
        u: st_to_uv(s),
        v: st_to_uv(t),
    }
}

/// One texel of a band level: the vertex it is centred on and its side in the face's s and t.
#[derive(Debug, Clone, Copy)]
struct Texel {
    face: Face,
    s: f64,
    t: f64,
    side: f64,
    level: BandLevel,
}

impl Texel {
    /// The texel of `level` whose vertex is nearest the unit direction `dir`.
    fn nearest(dir: [f64; 3], level: u8) -> Self {
        let (face, s, t) = st_of(dir);
        let per_face = f64::from(64_u32 << level);
        let snap = |x: f64| ((x * per_face).round().clamp(1.0, per_face - 1.0)) / per_face;
        Self {
            face,
            s: snap(s),
            t: snap(t),
            side: 1.0 / per_face,
            level: BandLevel::new(level).expect("a band level"),
        }
    }

    /// The texel's centre.
    fn at(&self) -> FaceUv {
        uv_at(self.face, self.s, self.t)
    }

    /// The `n` × `n` points at the centres of the texel's equal steps.
    fn points(&self, n: u32) -> impl Iterator<Item = FaceUv> + '_ {
        let step = self.side / f64::from(n);
        (0..n).flat_map(move |a| {
            (0..n).map(move |b| {
                let s = self.s - 0.5 * self.side + (f64::from(a) + 0.5) * step;
                let t = self.t - 0.5 * self.side + (f64::from(b) + 0.5) * step;
                uv_at(self.face, s, t)
            })
        })
    }
}

/// The total variation between `weights` and the classes [`material_at`] gives at the texel's
/// `n` × `n` points, ½ Σ |weight ÷ 255 − fraction| over the materials of either, and whether the
/// texel is mixed: more than one material among the weights or the points.
fn total_variation(
    synth: &Synthesiser<'_, CoarseField>,
    cache: &mut SynthCache,
    texel: &Texel,
    weights: &ClassWeights,
    n: u32,
) -> (f64, bool) {
    let mut counts: BTreeMap<GroundMaterial, u32> = BTreeMap::new();
    for at in texel.points(n) {
        let m = material_at(synth, cache, at).expect("a whole field holds every cell");
        *counts.entry(m).or_default() += 1;
    }
    let all = f64::from(n * n);
    let materials: BTreeSet<GroundMaterial> = counts
        .keys()
        .copied()
        .chain(weights.iter().map(|(m, _)| m))
        .collect();
    let tv = 0.5
        * materials
            .iter()
            .map(|&m| {
                let w = f64::from(weights.weight(m)) / f64::from(WEIGHT_TOTAL);
                let f = f64::from(counts.get(&m).copied().unwrap_or(0)) / all;
                (w - f).abs()
            })
            .sum::<f64>();
    (tv, counts.len() > 1 || weights.len() > 1)
}

/// The cells of `field` whose ground the weights find hardest: belts on dry land free of ice,
/// partial ice, the shallow sea's edge, the channels' trunks and the coarse craters' rims, up to
/// `each` of each, spread through the field.
fn hard_cells(
    field: &CoarseField,
    synth: &Synthesiser<'_, CoarseField>,
    each: usize,
) -> Vec<PatchKey> {
    let level = field.header().level();
    let all: Vec<PatchKey> = (0..level.cell_count())
        .map(|n| cell_at_index(level.get(), n).expect("a cell of the level"))
        .collect();
    let spread = |keys: Vec<PatchKey>| {
        let stride = (keys.len() / each.max(1)).max(1);
        keys.into_iter()
            .step_by(stride)
            .take(each)
            .collect::<Vec<_>>()
    };
    let record = |k: PatchKey| field.cell(k).expect("a whole field holds every cell");
    let belts = all
        .iter()
        .copied()
        .filter(|&k| {
            let c = record(k);
            ReliefStyle::of(c).belt() >= 0.5 && !c.is_under_water() && c.ice == 0
        })
        .collect();
    let ice = all
        .iter()
        .copied()
        .filter(|&k| (1..u8::MAX).contains(&record(k).ice))
        .collect();
    let coast = all
        .iter()
        .copied()
        .filter(|&k| {
            let c = record(k);
            c.is_under_water() && c.water_surface_mm - c.elevation_mm < 300_000
        })
        .collect();
    let mut network = synth.channels().network(field);
    let trunks = all
        .iter()
        .copied()
        .filter(|&k| {
            !record(k).is_under_water()
                && network
                    .segment(k)
                    .expect("a whole field holds every cell")
                    .is_some()
        })
        .collect();
    let rims = rim_cells(field, each);
    [belts, ice, coast, trunks, rims]
        .into_iter()
        .flat_map(spread)
        .collect()
}

/// `count` texels of band level `level` on `field`: half in its hard cells ([`hard_cells`]), at
/// random points of them, and half at random over the sphere.
fn texels(
    field: &CoarseField,
    synth: &Synthesiser<'_, CoarseField>,
    level: u8,
    count: usize,
    rng: &mut Lcg,
) -> Vec<Texel> {
    let hard = hard_cells(field, synth, count);
    let mut out: Vec<Texel> = Vec::with_capacity(count);
    for k in 0..count / 2 {
        if hard.is_empty() {
            break;
        }
        let cell = hard[(k * hard.len()) / (count / 2).max(1) % hard.len()];
        let dir = random_points_in(cell, 1, rng)[0];
        out.push(Texel::nearest(dir, level));
    }
    while out.len() < count {
        let dir = unit_dir([0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0));
        out.push(Texel::nearest(dir, level));
    }
    out
}

/// The agreement at one band level of one world: the mean and worst total variation over its
/// texels, the mean over its mixed texels alone, and the worst texels with their cells, worst
/// first, so that a failure names where the weights miss.
#[derive(Debug, Clone)]
struct Agreement {
    level: u8,
    texels: usize,
    mean: f64,
    worst: f64,
    mixed: usize,
    mixed_mean: f64,
    worst_texels: Vec<(f64, Texel, PatchKey)>,
}

/// The worst texels an [`Agreement`] keeps.
const WORST_KEPT: usize = 5;

/// The agreement of `world`'s weights with its classes at the levels n − 1 to n − 6 from its finest
/// level n, over `per_level` texels of 8 × 8 points each, and at the class map's level over
/// `class_map` texels of `map_side` × `map_side` points, each level given to `report` as it is
/// measured.
fn agreements(
    world_: SyntheticWorld,
    per_level: usize,
    (class_map, map_side): (usize, u32),
    mut report: impl FnMut(&Agreement),
) {
    let field = world(world_);
    let synth = Synthesiser::new(field, SEED);
    let finest = finest_band(field.header()).get();
    let mut rng = Lcg::new(0x7765_6967_6874_7300 ^ u64::from(finest));
    let class_map_level = (CLASS_MAP_LEVEL, class_map, map_side);
    let levels = (1..=6_u8)
        .map(|k| (finest - k, per_level, 8))
        .chain([class_map_level]);
    for (level, count, side) in levels {
        let (mut sum, mut mixed_sum, mut mixed) = (0.0, 0.0, 0_u32);
        let mut worst: f64 = 0.0;
        let mut worst_texels: Vec<(f64, Texel, PatchKey)> = Vec::new();
        let set = texels(field, &synth, level, count, &mut rng);
        for texel in &set {
            // A cache a texel, as a worker drops its own between descents: the channels' part is
            // never pruned, and a thousand texels about a body would hold every cell's grooves.
            let mut cache = SynthCache::new();
            let at = texel.at();
            let weights =
                weights_at(&synth, &mut cache, at, texel.level).expect("a whole field holds it");
            assert_eq!(
                weights.weights().iter().map(|&b| u32::from(b)).sum::<u32>(),
                u32::from(WEIGHT_TOTAL),
                "{world_:?} level {level} at {at:?}"
            );
            let (tv, is_mixed) = total_variation(&synth, &mut cache, texel, &weights, side);
            sum += tv;
            worst = worst.max(tv);
            if is_mixed {
                mixed += 1;
                mixed_sum += tv;
            }
            let (cell, _) = synth
                .cell_at(unit_dir(face_uv_to_xyz(at)))
                .expect("a whole field holds every cell");
            worst_texels.push((tv, *texel, cell));
            worst_texels.sort_by(|a, b| b.0.total_cmp(&a.0));
            worst_texels.truncate(WORST_KEPT);
        }
        #[expect(clippy::cast_precision_loss, reason = "a few thousand texels")]
        let mean = sum / set.len() as f64;
        report(&Agreement {
            level,
            texels: set.len(),
            mean,
            worst,
            mixed: usize::try_from(mixed).expect("a count fits a usize"),
            mixed_mean: if mixed > 0 {
                mixed_sum / f64::from(mixed)
            } else {
                0.0
            },
            worst_texels,
        });
    }
}

/// Holds `world_`'s agreement to [`AGREEMENT`] at every level, and prints each as it is measured;
/// a failure names the level's worst texels and their cells.
fn assert_agreement(world_: SyntheticWorld, per_level: usize, class_map: (usize, u32)) {
    agreements(world_, per_level, class_map, |a| {
        println!(
            "{world_:?} level {}: mean {:.4}, worst {:.4} over {} texels; mean {:.4} over the {} \
             mixed",
            a.level, a.mean, a.worst, a.texels, a.mixed_mean, a.mixed
        );
        assert!(
            a.mean <= AGREEMENT,
            "{world_:?} at level {}: the weights are {:.3} from the classes in the mean; the worst \
             texels (total variation, texel, cell): {:?}",
            a.level,
            a.mean,
            a.worst_texels
        );
    });
}

/// The class map's texels in `ci`'s agreement on each world, and their points a side: 64, and 32 on
/// the browser target, where those points were most of the Mars-like world's time and both of its
/// texels are still mixed (`crate::test_sizes`).
const CLASS_MAP: (usize, u32) = (2, per_target(64, 32));

#[test]
fn material_weights_agree_with_the_classes_counted_on_the_earth_like_world() {
    assert_agreement(SyntheticWorld::EarthLike, 24, CLASS_MAP);
}

#[test]
fn material_weights_agree_with_the_classes_counted_on_the_mars_like_world() {
    assert_agreement(SyntheticWorld::MarsLike, 24, CLASS_MAP);
}

#[test]
fn material_weights_agree_with_the_classes_counted_on_the_moon_like_world() {
    assert_agreement(SyntheticWorld::MoonLike, 24, CLASS_MAP);
}

#[test]
fn material_weights_agree_with_the_classes_counted_on_the_ceres_like_world() {
    assert_agreement(SyntheticWorld::CeresLike, 24, CLASS_MAP);
}

/// The plan's measure: a thousand texels a level, sixty-four points a texel at n − 1 to n − 6 and
/// 4,096 at the class map's level (R10.T2).
#[test]
#[ignore = "slow: 10^3 texels a level, 4 x 10^6 finest points at the class map's level"]
fn material_weights_agree_with_the_classes_counted_over_a_thousand_texels_earth_like() {
    assert_agreement(SyntheticWorld::EarthLike, 1_000, (1_000, 64));
}

#[test]
#[ignore = "slow: 10^3 texels a level, 4 x 10^6 finest points at the class map's level"]
fn material_weights_agree_with_the_classes_counted_over_a_thousand_texels_mars_like() {
    assert_agreement(SyntheticWorld::MarsLike, 1_000, (1_000, 64));
}

#[test]
#[ignore = "slow: 10^3 texels a level, 4 x 10^6 finest points at the class map's level"]
fn material_weights_agree_with_the_classes_counted_over_a_thousand_texels_moon_like() {
    assert_agreement(SyntheticWorld::MoonLike, 1_000, (1_000, 64));
}

#[test]
#[ignore = "slow: 10^3 texels a level, 4 x 10^6 finest points at the class map's level"]
fn material_weights_agree_with_the_classes_counted_over_a_thousand_texels_ceres_like() {
    assert_agreement(SyntheticWorld::CeresLike, 1_000, (1_000, 64));
}

/// Points of `field` for the finest level: random points in its hard cells and over the sphere.
fn finest_points(
    field: &CoarseField,
    synth: &Synthesiser<'_, CoarseField>,
    count: usize,
    rng: &mut Lcg,
) -> Vec<FaceUv> {
    let mut points: Vec<FaceUv> = hard_cells(field, synth, count / 8)
        .into_iter()
        .flat_map(|cell| random_points_in(cell, 2, rng))
        .map(xyz_to_face_uv)
        .collect();
    while points.len() < count {
        points.push(xyz_to_face_uv(unit_dir(
            [0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0),
        )));
    }
    points
}

#[test]
fn material_weights_at_the_finest_level_the_largest_is_the_class() {
    let mut rng = Lcg::new(0x6669_6e65_7374);
    let mut blended = 0;
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let finest = finest_band(field.header());
        let deeper = BandLevel::new(finest.get() + 1).expect("a band level");
        // Two points in each of up to 50 hard cells of each kind, then random ones to 400 a world;
        // on the browser target up to 12 of each kind, to 100 (`crate::test_sizes`).
        for at in finest_points(field, &synth, per_target(400, 100), &mut rng) {
            let class = material_at(&synth, &mut cache, at).expect("a whole field holds it");
            for level in [finest, deeper] {
                let weights = weights_at(&synth, &mut cache, at, level).expect("held");
                assert_eq!(weights.largest(), class, "{w:?} at {at:?}, level {level:?}");
                assert_eq!(
                    weights.weights().iter().map(|&b| u32::from(b)).sum::<u32>(),
                    u32::from(WEIGHT_TOTAL),
                    "{w:?} at {at:?}"
                );
                assert!(
                    weights
                        .iter()
                        .skip(1)
                        .all(|(_, b)| b < weights.weights()[0]),
                    "{w:?} at {at:?}: {weights:?}"
                );
            }
            if weights_at(&synth, &mut cache, at, finest)
                .expect("held")
                .len()
                > 1
            {
                blended += 1;
            }
        }
    }
    println!("{blended} of the finest points blend two materials");
}

#[test]
fn material_weights_where_a_level_leaves_nothing_out_they_are_the_class() {
    let mut rng = Lcg::new(0x6261_6e64);
    let mut checked = 0;
    for &w in &BODIES {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let finest = finest_band(field.header()).get();
        // Every level from the band limit's evaluates the finest level's ground to the bit.
        for level in (0..finest).rev() {
            let band = BandLevel::new(level).expect("a band level");
            if synth.level_set(band).shortest_wavelength().value() > crate::geometry::BAND_LIMIT_M {
                break;
            }
            for at in finest_points(field, &synth, 40, &mut rng) {
                let class = material_at(&synth, &mut cache, at).expect("held");
                let weights = weights_at(&synth, &mut cache, at, band).expect("held");
                assert_eq!(
                    weights.len(),
                    1,
                    "{w:?} level {level} at {at:?}: {weights:?}"
                );
                assert_eq!(weights.largest(), class, "{w:?} level {level} at {at:?}");
                checked += 1;
            }
        }
    }
    assert!(
        checked > 0,
        "some world has a level coarser than its finest at the band limit"
    );
}

#[test]
fn material_weights_are_order_and_cache_independent() {
    let field = world(SyntheticWorld::EarthLike);
    let synth = Synthesiser::new(field, SEED);
    let finest = finest_band(field.header()).get();
    let mut rng = Lcg::new(0x006f_7264_6572);
    let queries: Vec<(FaceUv, BandLevel)> = finest_points(field, &synth, 48, &mut rng)
        .into_iter()
        .zip(
            [finest, finest - 1, finest - 4, finest - 9, CLASS_MAP_LEVEL]
                .into_iter()
                .cycle(),
        )
        .map(|(at, l)| (at, BandLevel::new(l).expect("a band level")))
        .collect();
    let shared = std::cell::RefCell::new(SynthCache::new());
    assert_order_independent(&queries, |&(at, level)| {
        weights_at(&synth, &mut shared.borrow_mut(), at, level).expect("held")
    });
    assert_order_independent(&queries, |&(at, level)| {
        weights_at(&synth, &mut SynthCache::new(), at, level).expect("held")
    });
}

/// An answer of the interleaved order test: a class or a point's weights.
#[derive(Debug, PartialEq)]
enum Answer {
    Class(GroundMaterial),
    Weights(ClassWeights),
}

/// On a cold-trapped pole the coarse weights read the cell's cut, which [`material_at`] keeps in
/// the same cache under the same key: the two interleaved, in any order and from any cache, give
/// the same answers.
#[test]
fn material_weights_and_the_class_share_the_cut_in_any_order() {
    let field = cold_trapped(0.2);
    let synth = Synthesiser::new(&field, DetailSeed::new(17));
    let finest = finest_band(field.header()).get();
    let mut rng = Lcg::new(0x0063_7574_7300);
    let cells = cold_trap_cells(&field);
    assert!(!cells.is_empty(), "the pole holds partial ice");
    // Three points in each of four cells, or of two on the browser target, where the six still
    // take every kind of query in the cycle, in both cells (`crate::test_sizes`).
    let queries: Vec<(FaceUv, Option<BandLevel>)> = cells
        .iter()
        .take(per_target(4, 2))
        .flat_map(|&cell| random_points_in(cell, 3, &mut rng))
        .map(xyz_to_face_uv)
        .zip(
            [
                None,
                Some(finest - 1),
                None,
                Some(finest - 6),
                Some(CLASS_MAP_LEVEL),
            ]
            .into_iter()
            .cycle(),
        )
        .map(|(at, l)| (at, l.map(|l| BandLevel::new(l).expect("a band level"))))
        .collect();
    let answer = |cache: &mut SynthCache, &(at, level): &(FaceUv, Option<BandLevel>)| match level {
        None => Answer::Class(material_at(&synth, cache, at).expect("held")),
        Some(level) => Answer::Weights(weights_at(&synth, cache, at, level).expect("held")),
    };
    let shared = std::cell::RefCell::new(SynthCache::new());
    assert_order_independent(&queries, |q| answer(&mut shared.borrow_mut(), q));
    assert_order_independent(&queries, |q| answer(&mut SynthCache::new(), q));
}

/// The cold-trapped pole's partial ice, which no synthetic world's land holds: at n − 1 to n − 6
/// the weights agree with the classes counted over its polar cells' texels, every one of them
/// mixed, to the plan's ten points in the mean; and at every level, the class map's too, the ice's
/// weight in the mean is its counted share in the mean, also to ten points, since at the class
/// map's level, where the level resolves nothing below the cells, a texel's own count scatters
/// about the cell's share.
#[test]
fn material_weights_agree_with_the_classes_counted_on_a_cold_trapped_pole() {
    for share in [0.2, 0.6] {
        let field = cold_trapped(share);
        let synth = Synthesiser::new(&field, DetailSeed::new(17));
        let mut cache = SynthCache::new();
        let finest = finest_band(field.header()).get();
        let cells = cold_trap_cells(&field);
        let mut rng = Lcg::new(0x0070_6f6c_6500);
        // A texel costs its cell's cut, 256 heights once a cell, the cell's omitted index at the
        // level, 512, and two heights at each of its points: 16 texels a level and 24 at the class
        // map's, of 16 × 16 points, and 8 and 12 of 8 × 8 on the browser target, where 8 at the
        // class map's left the thick ice's weight 0.105 from its count (`crate::test_sizes`).
        let (level_texels, class_map_texels) = (per_target(16, 8), per_target(24, 12));
        for (level, count, side) in [
            (finest - 1, level_texels, 8),
            (finest - 3, level_texels, 8),
            (finest - 6, level_texels, 8),
            (CLASS_MAP_LEVEL, class_map_texels, per_target(16, 8)),
        ] {
            let (mut tv, mut weight, mut counted) = (0.0, 0.0, 0.0);
            let mut iced = 0;
            for k in 0..count {
                let cell = cells[(k * cells.len()) / count];
                let texel = Texel::nearest(random_points_in(cell, 1, &mut rng)[0], level);
                let weights = weights_at(&synth, &mut cache, texel.at(), texel.level)
                    .expect("a whole field holds every cell");
                let ice: f64 = weights
                    .iter()
                    .filter(|(m, _)| m.entry() == Some(1))
                    .map(|(_, b)| f64::from(b) / f64::from(WEIGHT_TOTAL))
                    .sum();
                if ice > 0.0 {
                    iced += 1;
                }
                weight += ice;
                let points = texel.points(side).count();
                let lying = texel
                    .points(side)
                    .filter(|&at| {
                        material_at(&synth, &mut cache, at)
                            .expect("a whole field holds every cell")
                            .entry()
                            == Some(1)
                    })
                    .count();
                counted += f64::from(u32::try_from(lying).expect("a count fits a u32"))
                    / f64::from(u32::try_from(points).expect("a count fits a u32"));
                tv += total_variation(&synth, &mut cache, &texel, &weights, side).0;
            }
            let n = f64::from(u32::try_from(count).expect("a count fits a u32"));
            let (tv, weight, counted) = (tv / n, weight / n, counted / n);
            println!(
                "cold trap {share}: level {level}, total variation {tv:.4}, the ice's weight {weight:.3} \
                 against {counted:.3} counted, {iced} of {count} iced"
            );
            if level != CLASS_MAP_LEVEL {
                assert!(
                    tv <= AGREEMENT,
                    "cold trap {share}, level {level}: the weights are {tv:.3} from the classes"
                );
            }
            assert!(
                (weight - counted).abs() <= AGREEMENT,
                "cold trap {share}, level {level}: the ice weighs {weight:.3} against {counted:.3}"
            );
            assert!(
                iced > 0,
                "cold trap {share}, level {level}: no texel weighs the ice"
            );
        }
    }
}

#[test]
fn material_weights_a_cell_not_held_is_not_surveyed() {
    let field = world(SyntheticWorld::MoonLike);
    let level = field.header().level().get();
    let cell = cell_at_index(level, 1_000).expect("a cell of the level");
    let view = Held {
        inner: field,
        held: BTreeSet::from([cell]),
    };
    let synth = Synthesiser::new(&view, SEED);
    let at = xyz_to_face_uv(cell.vertex_dir(32, 32));
    for l in [CLASS_MAP_LEVEL, 12, 17] {
        let result = weights_at(
            &synth,
            &mut SynthCache::new(),
            at,
            BandLevel::new(l).expect("a band level"),
        );
        assert!(
            matches!(result, Err(QueryHeightError::NotSurveyed(_))),
            "level {l}: {result:?}"
        );
    }
}

#[test]
fn material_weights_the_slope_lag_is_short_enough() {
    let mut rng = Lcg::new(0x006c_6167);
    for &w in &BODIES {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let finest = finest_band(field.header()).get();
        let coarse = field.header().level().get();
        for cell in random_cells(coarse, 8, &mut rng) {
            for level in [CLASS_MAP_LEVEL, finest - 6, finest - 1] {
                let band = BandLevel::new(level).expect("a band level");
                let at = |lag: f64| {
                    synth
                        .omitted_structure_function(cell, band, Metres::new(lag))
                        .expect("held")
                        .value()
                        / (lag * lag)
                };
                let (half, whole) = (at(0.5 * SLOPE_LAG.value()), at(SLOPE_LAG.value()));
                assert!(
                    (whole - half).abs() <= 0.01 * half.max(1e-12),
                    "{w:?} {cell:?} level {level}: {whole} at the lag, {half} at half of it"
                );
            }
        }
    }
}

/// The materials of the weights at every vertex of `patch` at its own level.
fn patch_palette(
    synth: &Synthesiser<'_, CoarseField>,
    cache: &mut SynthCache,
    patch: PatchKey,
) -> BTreeSet<GroundMaterial> {
    let level = BandLevel::new(patch.level()).expect("a band level");
    let mut out = BTreeSet::new();
    for y in 0..=64 {
        for x in 0..=64 {
            let at = xyz_to_face_uv(patch.vertex_dir(x, y));
            let weights = weights_at(synth, cache, at, level).expect("held");
            assert_eq!(
                weights.weights().iter().map(|&b| u32::from(b)).sum::<u32>(),
                u32::from(WEIGHT_TOTAL),
                "{patch:?} at {at:?}"
            );
            out.extend(weights.iter().map(|(m, _)| m));
        }
    }
    out
}

#[test]
fn material_weights_a_patch_needs_no_more_than_eight_classes() {
    let mut rng = Lcg::new(0x0070_6174_6368);
    let mut most = 0;
    for &w in &BODIES {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let finest = finest_band(field.header()).get();
        let mut hard = hard_cells(field, &synth, 2);
        if hard.is_empty() {
            hard = random_cells(field.header().level().get(), 4, &mut rng);
        }
        for level in [finest - 1, finest - 3, finest - 6, CLASS_MAP_LEVEL] {
            let pick = rng.next_below(u64::try_from(hard.len()).expect("a count fits a u64"));
            let cell = hard[usize::try_from(pick).expect("an index fits a usize")];
            let dir = random_points_in(cell, 1, &mut rng)[0];
            let patch = PatchKey::containing(level, dir).expect("a patch of the level");
            let palette = patch_palette(&synth, &mut cache, patch);
            most = most.max(palette.len());
            assert!(
                palette.len() <= MAX_CLASSES,
                "{w:?}'s patch {patch:?} needs {} classes: {palette:?}",
                palette.len()
            );
        }
    }
    println!("the most classes a patch needed: {most}");
}
