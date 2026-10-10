//! The structural octaves' tests (plan R09, T5): each octave's mean and bound, the variance against
//! [`local_variance`] on every synthetic world, the warp's reads, the gradient, the cache and the
//! styles.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::cube::unit_dir;
use crate::field::{ClimateCell, CoarseCrater, CoarseField, FlowDirection, LogArea, LogSteepness};
use crate::field::{SurfaceClass, cell_index};
use crate::synth::{
    BandSpectrumParts, BuildBandSpectrumError, STRUCTURAL_RMS_ICE_RICH, STRUCTURAL_RMS_MOBILE_LID,
    STRUCTURAL_RMS_STAGNANT_LID, SpectrumShape, unresolved_variance,
};
use crate::testing::{FieldBuilder, SyntheticWorld, synthetic_field};

/// The detail seed the tests draw from.
const SEED: DetailSeed = DetailSeed::new(0x7265_6c69_6566);

/// The four synthetic worlds with relief, each built once.
fn world(kind: SyntheticWorld) -> &'static CoarseField {
    static FIELDS: OnceLock<Vec<CoarseField>> = OnceLock::new();
    let fields = FIELDS.get_or_init(|| {
        SyntheticWorld::ALL
            .iter()
            .map(|&k| synthetic_field(k))
            .collect()
    });
    let at = SyntheticWorld::ALL
        .iter()
        .position(|&k| k == kind)
        .expect("every world is listed");
    &fields[at]
}

fn earth() -> &'static CoarseField {
    world(SyntheticWorld::EarthLike)
}

/// `count` directions uniform over the sphere by area (rejection from the unit ball).
fn uniform_points(seed: u64, count: usize) -> Vec<[f64; 3]> {
    let mut rng = Lcg::new(seed);
    let mut points = Vec::with_capacity(count);
    while points.len() < count {
        let p = [0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0);
        let r2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
        if r2 > 1e-6 && r2 <= 1.0 {
            points.push(unit_dir(p));
        }
    }
    points
}

/// `count` directions within about `spread` radians of `centre`.
fn points_near(seed: u64, centre: [f64; 3], spread: f64, count: usize) -> Vec<[f64; 3]> {
    let mut rng = Lcg::new(seed);
    (0..count)
        .map(|_| unit_dir([0, 1, 2].map(|a| centre[a] + (rng.next_f64() * 2.0 - 1.0) * spread)))
        .collect()
}

/// The lattice cache, and the octave index of every noise read through it, in order.
struct Recording {
    cache: LatticeCache,
    reads: Vec<u8>,
}

impl Lattice for Recording {
    fn noise(&mut self, octave: &Octave, x: [f64; 3]) -> (f64, [f64; 3]) {
        self.reads.push(octave.index());
        gradient_noise(x, octave, &mut self.cache)
    }
}

/// The octave terms of `relief` over `field` at `dir`, through its finest octave.
fn terms(relief: &Relief, field: &CoarseField, dir: [f64; 3]) -> Vec<Term> {
    let table = octave_table(relief.seed());
    let mut out = Vec::new();
    relief
        .each_term(
            field,
            dir,
            relief.finest_octave(),
            &table,
            &mut LatticeCache::new(),
            |t| out.push(t),
        )
        .unwrap();
    out
}

#[test]
fn nyquist_degrees_follow_the_level_s_mean_cell() {
    assert_eq!(nyquist_degree(5), 70);
    assert_eq!(nyquist_degree(8), 556);
    for level in 1..=24 {
        let l = nyquist_degree(level);
        // Two mean cells, √(4π ÷ (6 · 4ᴸ)) radians each, are the wavelength 2π ÷ l of a degree
        // between l − 1 and l.
        let cells = 2.0 * (4.0 * PI / 6.0).sqrt() / f64::from(1_u32 << level);
        #[expect(clippy::cast_precision_loss, reason = "a degree below 2^42")]
        let (below, at) = (2.0 * PI / (l - 1) as f64, 2.0 * PI / l as f64);
        assert!(at < cells && cells <= below, "level {level}: {l}");
    }
}

#[test]
fn octaves_span_the_coarse_cell_to_the_band_limit() {
    let finest = |r: f64| finest_octave(Metres::new(r));
    assert_eq!(finest(6.371e6), 22);
    assert_eq!(finest(3.3895e6), 21);
    assert_eq!(finest(1.7374e6), 20);
    assert_eq!(finest(4.697e5), 18);
    for r in [6.371e6, 4.697e5] {
        let m = finest(r);
        let spacing = |m| octave_spacing(Metres::new(r), m).value();
        assert!(spacing(m) >= BAND_LIMIT_M && spacing(m + 1) < BAND_LIMIT_M);
    }
    // 27 km at level 9 and 3.3 m at level 22 on an Earth.
    let earth_spacing = |m| octave_spacing(Metres::new(6.371e6), m).value();
    assert!(
        (earth_spacing(9) - 27_012.1).abs() < 0.1,
        "{}",
        earth_spacing(9)
    );
    assert!(
        (earth_spacing(22) - 3.297).abs() < 1e-3,
        "{}",
        earth_spacing(22)
    );
    assert_eq!(first_octave(earth().header().level()), 9);
}

/// Σ l^−β from `from` to `to` − 1, term by term in increasing order.
fn direct_sum(beta: f64, from: u64, to: u64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "degrees below 10^6")]
    (from..to).map(|l| math::powf(l as f64, -beta)).sum()
}

/// A law of one exponent β and V₁ = 1 m²: its break at degree 1, from which β holds.
fn single(beta: f64) -> BandSpectrum {
    BandSpectrum::new(BandSpectrumParts {
        exponent: beta,
        unit_degree_variance: SquareMetres::new(1.0),
        break_degree: 1.0,
        small_scale_exponent: beta,
    })
    .unwrap()
}

/// A broken law of V₁ = 1 m², β₁ `large`, β₂ `small` and break degree `at`.
fn broken(large: f64, small: f64, at: f64) -> BandSpectrum {
    BandSpectrum::new(BandSpectrumParts {
        exponent: large,
        unit_degree_variance: SquareMetres::new(1.0),
        break_degree: at,
        small_scale_exponent: small,
    })
    .unwrap()
}

#[test]
fn the_spectrum_s_closed_forms_match_its_sums() {
    for beta in [1.1, 1.7, 1.9, 2.05, 3.0, 4.0] {
        let spectrum = single(beta);
        for (from, to) in [
            (1, 300),
            (5, 17),
            (16, 1_000),
            (40, 41),
            (70, 139),
            (556, 200_000),
        ] {
            let closed = spectrum.band_variance(from, to).value();
            let direct = direct_sum(beta, from, to);
            assert!(
                (closed / direct - 1.0).abs() < 1e-11,
                "β {beta}, {from}..{to}: {closed} vs {direct}"
            );
            let tails = spectrum.variance_from_degree(from).value()
                - spectrum.variance_from_degree(to).value();
            assert!((tails / direct - 1.0).abs() < 1e-11, "β {beta}");
        }
        // The tail from degree 1 is ζ(β): ζ(2) = π² ÷ 6, ζ(4) = π⁴ ÷ 90.
        let zeta = spectrum.variance_from_degree(1).value();
        if (beta - 4.0).abs() < 1e-12 {
            assert!((zeta - math::powi(PI, 4) / 90.0).abs() < 1e-12, "{zeta}");
        }
    }
    let two = single(2.0);
    assert!((two.variance_from_degree(1).value() - PI * PI / 6.0).abs() < 1e-12);
    assert_eq!(
        BandSpectrum::new(BandSpectrumParts {
            exponent: 0.9,
            ..BandSpectrumParts {
                exponent: 2.0,
                unit_degree_variance: SquareMetres::new(1.0),
                break_degree: 1.0,
                small_scale_exponent: 2.0,
            }
        }),
        Err(BuildBandSpectrumError::Exponent(0.9))
    );
}

/// The degree variance V(l) term by term over `from` to `to` − 1, in increasing order.
fn direct_band(spectrum: &BandSpectrum, from: u64, to: u64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "degrees below 10^6")]
    (from..to)
        .map(|l| spectrum.degree_variance(l as f64).value())
        .sum()
}

/// The broken law's closed forms equal its direct sums to 10⁻¹¹ (`decision-r09-t5.md` item 2):
/// for the silicate and ice-rich exponents, with the break at an integer and at a non-integer
/// degree, and each band below, across and above it; and the law is continuous at its break.
#[test]
fn the_broken_spectrum_s_closed_forms_match_its_sums_across_the_break() {
    for (large, small) in [(2.0, 3.0), (1.8, 2.6)] {
        for at in [300.0, 300.4, 1.0, 20_015.1] {
            let spectrum = broken(large, small, at);
            let bands: [(u64, u64); 8] = [
                (1, 299),
                (5, 300),
                (299, 302),
                (300, 301),
                (301, 302),
                (70, 139_000),
                (301, 4_000),
                (556, 200_000),
            ];
            for (from, to) in bands {
                let closed = spectrum.band_variance(from, to).value();
                let direct = direct_band(&spectrum, from, to);
                assert!(
                    (closed / direct - 1.0).abs() < 1e-11,
                    "β {large}/{small}, break {at}, {from}..{to}: {closed} vs {direct}"
                );
                let tails = spectrum.variance_from_degree(from).value()
                    - spectrum.variance_from_degree(to).value();
                assert!(
                    (tails / direct - 1.0).abs() < 1e-11,
                    "β {large}/{small}, break {at}, {from}..{to}: tails {tails} vs {direct}"
                );
            }
            // Continuous at the break: the first segment's law reaches the second's there.
            let v1 = spectrum.unit_degree_variance().value();
            let lower = v1 * math::powf(at, -large);
            let upper = spectrum.degree_variance(at).value();
            assert!((upper / lower - 1.0).abs() < 1e-12, "break {at}");
            if at > 1.0 {
                let just_below = spectrum.degree_variance(at * (1.0 - 1e-13)).value();
                assert!((just_below / upper - 1.0).abs() < 1e-12, "break {at}");
            }
        }
    }
    // A break below every degree a field reads is the second law alone, and one beyond them all
    // the first.
    let low = broken(2.0, 3.0, 1.0);
    let three = single(3.0);
    assert_same_bits(
        low.variance_from_degree(70).value(),
        three.variance_from_degree(70).value(),
    );
    let high = broken(2.0, 3.0, 1e300);
    let first = high.band_variance(70, 1 << 40).value();
    let expected = single(2.0).band_variance(70, 1 << 40).value();
    assert!(
        (first / expected - 1.0).abs() < 1e-12,
        "{first} vs {expected}"
    );
}

/// The four synthetic worlds' spectra and a Europa's, anchored as the ruling computes them
/// (`decision-r09-t5.md` item 3, its `out/fixtures.txt`): V₁, the structural RMS above the coarse
/// cell's Nyquist degree, and the first and finest octaves' RMS.
#[test]
fn the_anchored_spectra_carry_their_crust_class_s_rms() {
    // (world, V₁ m², RMS above l_N(L) m, first octave m, finest octave cm)
    let expected = [
        (SyntheticWorld::EarthLike, 2.253e7, 200.0, 142.4, 9.0),
        (SyntheticWorld::MarsLike, 1.079e6, 61.9, 44.1, 2.9),
        (SyntheticWorld::MoonLike, 5.531e5, 62.8, 44.7, 3.0),
        (SyntheticWorld::CeresLike, 1.479e5, 45.6, 32.6, 3.2),
    ];
    for (kind, v1, local_m, first_m, finest_cm) in expected {
        let header = world(kind).header();
        let spectrum = header.spectrum();
        let got = spectrum.unit_degree_variance().value();
        assert!((got / v1 - 1.0).abs() < 1e-3, "{kind:?}: V₁ {got} m²");
        // The break at 2 km, and the anchor's variance from l_72 is the stagnant lid's or the
        // mobile lid's RMS squared.
        let radius = header.radius().value();
        assert_same_bits(spectrum.break_degree(), 2.0 * PI * radius / 2_000.0);
        assert_same_bits(spectrum.small_scale_exponent(), 3.0);
        assert_same_bits(spectrum.exponent(), 2.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a positive degree below 10^4"
        )]
        let l72 = ((2.0 * PI * radius / 72e3).round() as u64).max(2);
        let anchor = if kind == SyntheticWorld::EarthLike {
            STRUCTURAL_RMS_MOBILE_LID
        } else {
            STRUCTURAL_RMS_STAGNANT_LID
        };
        let variance = spectrum.variance_from_degree(l72).value();
        let target = anchor.value() * anchor.value();
        assert!((variance / target - 1.0).abs() < 1e-12, "{kind:?}");
        let local = local_variance(spectrum, header.level()).value().sqrt();
        assert!((local - local_m).abs() < 0.05, "{kind:?}: {local} m");
        let relief = Relief::new(header, SEED);
        let first = relief.octave_rms(relief.first_octave()).value();
        assert!((first - first_m).abs() < 0.05, "{kind:?}: {first} m");
        let finest = relief.octave_rms(relief.finest_octave()).value() * 100.0;
        assert!((finest - finest_cm).abs() < 0.05, "{kind:?}: {finest} cm");
    }
    // For R09.T8: the Earth-like world's finest octave carries 9.0 cm, so its bound is 1.97 m (the
    // ruling's figure, from the rounded 9.0 cm: 2.5 × 8.770 × 9.0 ± 0.05 cm is 1.97 ± 0.011 m).
    let earth_header = earth().header();
    let finest = finest_octave(earth_header.radius());
    let bound = octave_bound(earth_header.spectrum(), finest).value();
    assert!((bound - 1.973).abs() < 0.011, "{bound} m");
    // An ice-rich crust on a Europa: β 1.8 to a break at 1 km, then 2.6, V₁ 1.032 × 10⁵ m² and
    // 49.6 m above degree 139 (level 6).
    let europa = BandSpectrum::anchored(
        SpectrumShape::ICE_RICH,
        Metres::new(1.5608e6),
        STRUCTURAL_RMS_ICE_RICH,
    );
    let v1 = europa.unit_degree_variance().value();
    assert!((v1 / 1.032e5 - 1.0).abs() < 1e-3, "{v1}");
    let local = europa
        .variance_from_degree(nyquist_degree(6))
        .value()
        .sqrt();
    assert!((local - 49.6).abs() < 0.05, "{local} m");
    // An anchor of zero is no relief at all, and the anchor's degree is never below 2.
    let none = BandSpectrum::anchored(SpectrumShape::SILICATE, Metres::new(6.371e6), Metres::ZERO);
    assert_same_bits(none.unit_degree_variance().value(), 0.0);
    let tiny = BandSpectrum::anchored(SpectrumShape::SILICATE, Metres::new(20.0), Metres::new(1.0));
    assert_same_bits(tiny.break_degree(), 1.0);
    assert!((tiny.variance_from_degree(2).value() - 1.0).abs() < 1e-12);
}

#[test]
fn local_variance_is_the_sum_of_every_octave_s_band() {
    for kind in [
        SyntheticWorld::EarthLike,
        SyntheticWorld::MarsLike,
        SyntheticWorld::MoonLike,
        SyntheticWorld::CeresLike,
    ] {
        let header = world(kind).header();
        let spectrum = header.spectrum();
        let level = header.level();
        let mut bands = 0.0;
        for m in first_octave(level)..=40 {
            bands += spectrum.level_variance(m).value();
        }
        bands += spectrum.variance_from_degree(nyquist_degree(40)).value();
        let local = local_variance(spectrum, level).value();
        assert!(
            (bands / local - 1.0).abs() < 1e-12,
            "{kind:?}: {bands} vs {local}"
        );
        // The octaves stop at the band limit, far below the break, leaving out under 10⁻⁶ of it.
        let relief = Relief::new(header, SEED);
        let mut carried = 0.0;
        for m in relief.first_octave()..=relief.finest_octave() {
            let rms = relief.octave_rms(m).value();
            carried += rms * rms;
        }
        assert!(
            carried < local && carried > (1.0 - 1e-6) * local,
            "{kind:?}: {}",
            1.0 - carried / local
        );
    }
    // The Earth-like world's local share is its anchor, 200 m RMS, since its coarse cell's
    // Nyquist degree, 556, is the anchor's degree.
    let spectrum = earth().header().spectrum();
    let local = local_variance(spectrum, earth().header().level()).value();
    assert!((local.sqrt() - 200.0).abs() < 1e-9, "{}", local.sqrt());
}

/// Each octave's n̂ and r̂ over `points` of an Earth-sized body in `style`: their sums and sums of
/// squares, by octave from the first, with the octave's largest |t| at four mixes.
fn octave_moments(points: &[[f64; 3]], style: ReliefStyle) -> Vec<(u8, [f64; 4], f64)> {
    let header = earth().header();
    let relief = Relief::new(header, SEED);
    let table = octave_table(SEED);
    let (first, last) = (relief.first_octave(), relief.finest_octave());
    let mut moments = vec![(0_u8, [0.0; 4], 0.0_f64); usize::from(last - first) + 1];
    let mixes = [0.0, FRAC_PI_2 / 2.0, 0.96, FRAC_PI_2].map(math::sin_cos);
    let mut cache = LatticeCache::new();
    for &dir in points {
        relief.octave_terms(
            &table,
            header.figure().point(dir),
            &StyleSample::uniform(style),
            last,
            &mut cache,
            |t| {
                let slot = &mut moments[usize::from(t.octave - first)];
                slot.0 = t.octave;
                let (n, r) = (t.plain.value, t.ridged.value);
                slot.1[0] += n;
                slot.1[1] += n * n;
                slot.1[2] += r;
                slot.1[3] += r * r;
                for (sin, cos) in mixes {
                    slot.2 = slot.2.max((cos * n + sin * r).abs());
                }
            },
        );
    }
    moments
}

/// The plan's test: each octave's mean is under 1% of its amplitude, plain and ridged (the
/// multifractal's weighted term), warped and not, and no octave's term leaves its stated bound.
///
/// Over 2 × 10⁵ points (the plan's 10⁵ raised so that the 1% is 4.5 standard errors of a unit-RMS
/// mean, a two-sided p of 8 × 10⁻⁶ a check and under 10⁻³ over the 56 checks, `stats::ALPHA`).
#[test]
fn each_octave_s_mean_is_under_a_hundredth_of_its_amplitude() {
    let points = uniform_points(0x6d65_616e, 200_000);
    #[expect(clippy::cast_precision_loss, reason = "2 × 10^5 points")]
    let count = points.len() as f64;
    let plain = ReliefStyle::REFERENCE;
    let warped = ReliefStyle {
        shear: 1.0,
        ..plain
    };
    for style in [plain, warped] {
        for (m, [n, n2, r, r2], largest) in octave_moments(&points, style) {
            let (mean_n, mean_r) = (n / count, r / count);
            let (rms_n, rms_r) = ((n2 / count).sqrt(), (r2 / count).sqrt());
            assert!(
                mean_n.abs() < 0.01,
                "octave {m} {style:?}: plain mean {mean_n}"
            );
            assert!(
                mean_r.abs() < 0.01,
                "octave {m} {style:?}: ridged mean {mean_r}"
            );
            assert!((rms_n - 1.0).abs() < 0.03, "octave {m}: plain RMS {rms_n}");
            assert!((rms_r - 1.0).abs() < 0.03, "octave {m}: ridged RMS {rms_r}");
            assert!(largest <= t_max(), "octave {m}: {largest} > {}", t_max());
        }
    }
}

#[test]
fn the_stated_bound_is_reached_only_at_the_noise_s_extreme() {
    // At n = −B with the weight 1 both terms are negative, and the mix at tan θ = r̃ ÷ n̂ reaches
    // √(n̂² + r̃²).
    let n = -NOISE_BOUND / NOISE_RMS;
    let r = ridged(-NOISE_BOUND) / RIDGE_WEIGHT_RMS;
    assert!(n < 0.0 && r < 0.0);
    let theta = math::atan(r / n);
    let (sin, cos) = math::sin_cos(theta);
    assert!(((cos * n + sin * r).abs() - t_max()).abs() < 1e-12);
    assert!((t_max() - 8.770).abs() < 1e-3, "{}", t_max());
    // Every other value gives less, at every mix and weight, the first octave's unweighted term
    // included (the weight 1 without the division is smaller still).
    for k in 0..=2_000 {
        let v = NOISE_BOUND * (f64::from(k) / 1_000.0 - 1.0);
        for j in 0..=64 {
            let (sin, cos) = math::sin_cos(FRAC_PI_2 * f64::from(j) / 64.0);
            for w in [0.0, 0.25, 0.5, 1.0] {
                for divisor in [RIDGE_WEIGHT_RMS, 1.0] {
                    let t = cos * v / NOISE_RMS + sin * w * ridged(v) / divisor;
                    assert!(t.abs() <= t_max() + 1e-12, "{v}, {j}, {w}: {t}");
                }
            }
        }
    }
}

/// The weight is 1 on the coarser octave's crest and 0 where its ridge is 0 or below, continuous
/// in value and slope there.
#[test]
fn the_multifractal_weight_runs_from_trough_to_crest() {
    let (crest, _) = ridge_weight(ridge(0.0).0);
    assert_same_bits(crest, 1.0);
    assert_same_bits(ridge_weight(-0.01).0, 0.0);
    assert_same_bits(ridge_weight(-0.01).1, 0.0);
    let (w, dw) = ridge_weight(1e-9);
    assert!(w < 1e-17 && dw < 1e-8, "{w}, {dw}");
    for k in 0..=100 {
        let r = f64::from(k) / 100.0 * (1.0 - RIDGE_EPSILON);
        let h = 1e-6;
        let numeric = (ridge_weight(r + h).0 - ridge_weight(r - h).0) / (2.0 * h);
        assert!((numeric - ridge_weight(r).1).abs() < 1e-6, "{r}");
    }
}

#[test]
fn no_octave_on_a_synthetic_world_exceeds_its_stated_bound() {
    for &kind in SyntheticWorld::ALL {
        let field = world(kind);
        let header = field.header();
        let relief = Relief::new(header, SEED);
        for dir in uniform_points(0x626f_756e, 1_500) {
            for t in terms(&relief, field, dir) {
                let bound = octave_bound(header.spectrum(), t.octave).value();
                assert!(
                    t.height.value.abs() <= bound,
                    "{kind:?} octave {}: {} > {bound}",
                    t.octave,
                    t.height.value
                );
            }
        }
    }
}

/// The plan's test: over each synthetic world, the sampled variance of the relief is within 5% of
/// `local_variance` times the mean square of the interpolated amplitude at the same points (1 on a
/// world of the reference style alone). 6 × 10⁴ points a world, so that the 5% is at least the
/// two-sided z of `stats::ALPHA` (3.29) standard errors of the difference, measured from the
/// samples as if they were independent (the coarsest octave's few thousand lattice cells over a
/// small body make them less so).
#[test]
fn local_variance_matches_the_sampled_variance_on_each_synthetic_world() {
    for (kind, seed) in [
        (SyntheticWorld::EarthLike, 0x6561_7274),
        (SyntheticWorld::MarsLike, 0x6d61_7273),
        (SyntheticWorld::MoonLike, 0x6d6f_6f6e),
        (SyntheticWorld::CeresLike, 0x6365_7265),
    ] {
        let field = world(kind);
        let header = field.header();
        let relief = Relief::new(header, SEED);
        let mut cache = SynthCache::new();
        let points = uniform_points(seed, 60_000);
        let local = local_variance(header.spectrum(), header.level()).value();
        let (mut sum, mut sum_sq, mut a_sq) = (0.0, 0.0, 0.0);
        let mut differences = Vec::with_capacity(points.len());
        for &dir in &points {
            let h = relief.at(field, dir, 31, &mut cache).unwrap().height_m;
            let a = smooth(field, dir, Component::Amplitude).unwrap().value;
            sum += h;
            sum_sq += h * h;
            a_sq += a * a;
            differences.push(h * h - local * a * a);
        }
        #[expect(clippy::cast_precision_loss, reason = "6 × 10^4 points")]
        let count = points.len() as f64;
        let mean = sum / count;
        let sampled = sum_sq / count - mean * mean;
        let expected = local * a_sq / count;
        let centre = differences.iter().sum::<f64>() / count;
        let spread = differences
            .iter()
            .map(|d| (d - centre) * (d - centre))
            .sum::<f64>();
        let standard_error = (spread / (count - 1.0)).sqrt() / count.sqrt();
        let z = math::normal_quantile(1.0 - stats::ALPHA / 2.0);
        assert!(
            0.05 * expected >= z * standard_error,
            "{kind:?}: 5% of {expected} m² is under {z} standard errors of {standard_error} m²"
        );
        println!(
            "{kind:?}: sampled {sampled:.1} m², expected {expected:.1} m², ratio {:.4}, \
             mean a² {:.4}",
            sampled / expected,
            a_sq / count
        );
        assert!(
            (sampled / expected - 1.0).abs() < 0.05,
            "{kind:?}: sampled {sampled} m², expected {expected} m² (mean a² {})",
            a_sq / count
        );
    }
}

/// The structural relief's adirectional RMS slope in `style` over each of `baselines` metres, on
/// the body of `field`, from `count` area-uniform points: atan(√2 ν(b) ÷ b) in degrees, with
/// ν(b) the RMS height difference over b along a random tangent at each point, through the finest
/// octave (R10's readout, R10 Design note 7, is the RMS gradient magnitude over b, √2 times a
/// profile's on isotropic ground). Each slope comes with the standard error of tan of it, from
/// the samples' spread.
fn sampled_slopes<const N: usize>(
    field: &CoarseField,
    style: ReliefStyle,
    baselines: [f64; N],
    seed: u64,
    count: usize,
) -> [(f64, f64); N] {
    let header = field.header();
    let relief = Relief::new(header, SEED);
    let table = octave_table(SEED);
    let finest = relief.finest_octave();
    let uniform = StyleSample::uniform(style);
    let mut cache = LatticeCache::new();
    let mut height = |point: [f64; 3]| {
        let mut h = 0.0;
        relief.octave_terms(&table, point, &uniform, finest, &mut cache, |t| {
            h += t.height.value;
        });
        h
    };
    let mut rng = Lcg::new(seed ^ 0x7461_6e67);
    let mut sums = [(0.0_f64, 0.0_f64); N];
    for dir in uniform_points(seed, count) {
        let p = header.figure().point(dir);
        let r = [0, 1, 2].map(|_| rng.next_f64() * 2.0 - 1.0);
        let along = r[0] * dir[0] + r[1] * dir[1] + r[2] * dir[2];
        let tangent = unit_dir([0, 1, 2].map(|k| r[k] - along * dir[k]));
        let here = height(p);
        for (sum, &b) in sums.iter_mut().zip(&baselines) {
            let d = height([0, 1, 2].map(|k| p[k] + b * tangent[k])) - here;
            sum.0 += d * d;
            sum.1 += d * d * d * d;
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "under 10^6 points")]
    let n = count as f64;
    let mut out = [(0.0, 0.0); N];
    for ((slot, (d2, d4)), &b) in out.iter_mut().zip(sums).zip(&baselines) {
        let nu2 = d2 / n;
        // tan = √2 ν ÷ b, so its relative error is half ν²'s.
        let tan = (2.0 * nu2).sqrt() / b;
        let relative = ((d4 / n - nu2 * nu2) / n).sqrt() / nu2 / 2.0;
        *slot = (math::atan(tan).to_degrees(), tan * relative);
    }
    out
}

/// The structural relief's adirectional RMS slopes at 1, 10 and 100 m at the reference style,
/// against the ruling's (`decision-r09-t5.md` item 2, "Expected RMS slopes", from its octave
/// model of the noise's measured structure function): 18.0°, 15.6° and 12.0° on the Earth-like
/// world's mobile lid, 5.5°, 4.8° and 3.6° on each stagnant lid, and 14.7°, 10.2° and 5.6° on an
/// ice-rich Europa. The single law V₁ l^−1.9 gave 56° along a profile at 1 m.
///
/// Each slope's tangent is held within 5% of the ruling's: its figures are rounded to 0.1°, 1.4%
/// at 3.6°, and its model interpolates the noise's structure function linearly between measured
/// lags, a few per cent at the finest octaves; and the 5% is at least the two-sided z of
/// `stats::ALPHA` standard errors of the sample, 2 × 10⁴ points a body.
#[test]
fn the_reference_style_s_slopes_are_the_ruling_s() {
    let europa = FieldBuilder::new(Metres::new(1.5608e6))
        .spectrum(BandSpectrum::anchored(
            SpectrumShape::ICE_RICH,
            Metres::new(1.5608e6),
            STRUCTURAL_RMS_ICE_RICH,
        ))
        .build();
    let cases: [(&CoarseField, &str, [f64; 3], u64); 5] = [
        (earth(), "Earth-like", [18.0, 15.6, 12.0], 0x736c_6f31),
        (
            world(SyntheticWorld::MarsLike),
            "Mars-like",
            [5.5, 4.8, 3.6],
            0x736c_6f32,
        ),
        (
            world(SyntheticWorld::MoonLike),
            "Moon-like",
            [5.5, 4.8, 3.6],
            0x736c_6f33,
        ),
        (
            world(SyntheticWorld::CeresLike),
            "Ceres-like",
            [5.5, 4.8, 3.6],
            0x736c_6f34,
        ),
        (
            &europa,
            "an ice-rich Europa",
            [14.7, 10.2, 5.6],
            0x736c_6f35,
        ),
    ];
    let z = math::normal_quantile(1.0 - stats::ALPHA / 2.0);
    for (field, name, expected, seed) in cases {
        let measured = sampled_slopes(
            field,
            ReliefStyle::REFERENCE,
            [1.0, 10.0, 100.0],
            seed,
            20_000,
        );
        println!(
            "{name}: {:.2}° / {:.2}° / {:.2}° at 1 / 10 / 100 m (the ruling's {} / {} / {})",
            measured[0].0, measured[1].0, measured[2].0, expected[0], expected[1], expected[2]
        );
        for (((slope, error), want), b) in measured.into_iter().zip(expected).zip([1, 10, 100]) {
            let target = math::tan(want.to_radians());
            let tan = math::tan(slope.to_radians());
            assert!(
                0.05 * target >= z * error,
                "{name} at {b} m: 5% of tan {want}° is under {z} standard errors of {error}"
            );
            assert!(
                (tan / target - 1.0).abs() < 0.05,
                "{name} at {b} m: {slope}° against the ruling's {want}°"
            );
        }
    }
    // A collision belt's full style (amplitude 2.5, all ridged), against the ruling's 39.0°,
    // 35.0° and 28.1°, which scale the plain noise's slopes by the amplitude alone: recorded, for
    // R09.T8 and R10, since the ridged term is steeper than the plain one at the same RMS height.
    let belt = ReliefStyle {
        amplitude: BELT_AMPLITUDE,
        belt: 1.0,
        shear: 0.0,
    };
    let reference = sampled_slopes(
        earth(),
        ReliefStyle::REFERENCE,
        [1.0, 10.0, 100.0],
        0x6265_6c74,
        20_000,
    );
    let ridged = sampled_slopes(earth(), belt, [1.0, 10.0, 100.0], 0x6265_6c74, 20_000);
    println!(
        "Earth-like belt: {:.2}° / {:.2}° / {:.2}° at 1 / 10 / 100 m (the ruling's 39.0 / 35.0 / \
         28.1, the plain noise's at a = 2.5)",
        ridged[0].0, ridged[1].0, ridged[2].0
    );
    for ((plain, _), (steep, _)) in reference.into_iter().zip(ridged) {
        assert!(steep > plain, "{steep}° against {plain}°");
    }
}

/// The Earth-like world's ocean holds each of the sea floor's regimes over at least a tenth of its
/// cells (`decision-r09-t5.md` item 5): hills rougher than the reference (amplitude at least 1,
/// the slow ridges' floor), smoother draped hills (between 0.2 and 1, the fast ridges' floor and
/// the floor its drape has frozen at half its relief), and plains (at most 0.2, R09.T12.a's
/// measure); and the cells' mean squared amplitude, over the ocean and the body (the ruled ⟨a²⟩,
/// of the interpolated amplitude at area-uniform points, is the variance test's "mean a²").
#[test]
fn the_earth_like_ocean_holds_hills_draped_hills_and_plains() {
    let field = earth();
    let (mut ocean, mut hills, mut draped, mut plains) = (0_u32, 0_u32, 0_u32, 0_u32);
    let (mut a2_ocean, mut a2_all) = (0.0, 0.0);
    for cell in field.synthesis() {
        let a = ReliefStyle::of(cell).amplitude();
        a2_all += a * a;
        if cell.crust != Crust::Oceanic {
            continue;
        }
        ocean += 1;
        a2_ocean += a * a;
        if a <= 0.2 {
            plains += 1;
        } else if a < 1.0 {
            draped += 1;
        } else {
            hills += 1;
        }
    }
    let share = |n: u32| f64::from(n) / f64::from(ocean);
    #[expect(clippy::cast_precision_loss, reason = "under 10^6 cells")]
    let cells = field.synthesis().len() as f64;
    println!(
        "Earth-like ocean: hills {:.3}, draped hills {:.3}, plains {:.3}; the cells' mean a² \
         {:.3} over the ocean and {:.3} over the body",
        share(hills),
        share(draped),
        share(plains),
        a2_ocean / f64::from(ocean),
        a2_all / cells
    );
    for (name, n) in [
        ("hills", hills),
        ("draped hills", draped),
        ("plains", plains),
    ] {
        assert!(share(n) >= 0.1, "{name} cover {} of the ocean", share(n));
    }
}

#[test]
fn a_world_with_no_spectrum_has_no_relief() {
    for kind in [SyntheticWorld::Flat, SyntheticWorld::OneCrater] {
        let field = world(kind);
        let header = field.header();
        assert_eq!(
            local_variance(header.spectrum(), header.level()),
            SquareMetres::ZERO
        );
        let relief = Relief::new(header, SEED);
        let mut cache = SynthCache::new();
        for dir in uniform_points(0x666c_6174, 200) {
            let h = relief.at(field, dir, 31, &mut cache).unwrap();
            assert_eq!(bits(h.height_m), 0, "{kind:?}");
            assert!(h.gradient.iter().all(|&g| bits(g) == 0), "{kind:?}");
        }
    }
}

/// The plan's instrumented test: evaluating through octave n reads each octave from the first to
/// n once, in order, and none finer, so no warp offset reads an octave finer than the one it
/// warps; and the octaves through n are the same terms, bit for bit, as when finer ones follow.
#[test]
fn a_warp_never_reads_an_octave_finer_than_the_one_it_warps() {
    let field = earth();
    let relief = Relief::new(field.header(), SEED);
    let table = octave_table(SEED);
    let (first, finest) = (relief.first_octave(), relief.finest_octave());
    for dir in uniform_points(0x7761_7270, 40) {
        let mut deepest = Vec::new();
        relief
            .each_term(field, dir, finest, &table, &mut LatticeCache::new(), |t| {
                deepest.push(t);
            })
            .unwrap();
        for n in first..=finest {
            let mut lattice = Recording {
                cache: LatticeCache::new(),
                reads: Vec::new(),
            };
            let mut reads_at_each = Vec::new();
            let mut shallow = Vec::new();
            relief
                .each_term(field, dir, n, &table, &mut lattice, |t| shallow.push(t))
                .unwrap();
            reads_at_each.extend(lattice.reads.iter().copied());
            assert_eq!(
                reads_at_each,
                (first..=n).collect::<Vec<_>>(),
                "through {n}"
            );
            assert_eq!(shallow[..], deepest[..shallow.len()], "through {n}");
        }
    }
    // Below the first octave nothing is read, not even a cell.
    let held = Holding {
        inner: field,
        held: BTreeSet::new(),
    };
    let none = relief.at(&held, [1.0, 0.0, 0.0], first - 1, &mut SynthCache::new());
    assert_eq!(bits(none.unwrap().height_m), 0);
}

/// The finite difference of H(P) = h(M⁻¹P ÷ |M⁻¹P|), the relief through `through` as a function
/// of direction, along each body-fixed axis at the spheroid point of `dir`, `step` metres each
/// way.
fn central_difference(
    relief: &Relief,
    field: &CoarseField,
    dir: [f64; 3],
    through: u8,
    step: f64,
) -> [f64; 3] {
    let figure = field.header().figure();
    let p = figure.point(dir);
    let radii = [
        figure.equatorial_radius_m,
        figure.equatorial_radius_m,
        figure.polar_radius_m,
    ];
    let mut cache = SynthCache::new();
    [0, 1, 2].map(|k| {
        let mut at = |sign: f64| {
            let mut q = p;
            q[k] += sign * step;
            let d = unit_dir([0, 1, 2].map(|a| q[a] / radii[a]));
            relief.at(field, d, through, &mut cache).unwrap().height_m
        };
        (at(1.0) - at(-1.0)) / (2.0 * step)
    })
}

/// The sum of the magnitudes of the octaves' gradients at `dir` through `through`, each as a
/// function of direction: the scale of a finite difference's error, which each octave makes in
/// proportion to its own slope, however the octaves' slopes cancel in their sum.
fn octave_slopes(relief: &Relief, field: &CoarseField, dir: [f64; 3], through: u8) -> f64 {
    let table = octave_table(relief.seed());
    let mut total = 0.0;
    relief
        .each_term(field, dir, through, &table, &mut LatticeCache::new(), |t| {
            let g = as_function_of_direction(field.header(), dir, t.height.gradient);
            total += (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
        })
        .unwrap();
    total
}

#[test]
fn the_gradient_matches_a_central_difference() {
    // Through octave 15 (422 m on an Earth) with a 10 cm step, and through the finest (3.3 m)
    // with a 1 mm step. The difference errs by about (step ÷ λ)² of each octave's own slope, and
    // the noise, C² at its lattice planes, by that much again where a step crosses one: under
    // 10⁻⁶ of the octaves' slopes, the larger of their sum's magnitude and the sum of theirs,
    // well inside the 10⁻⁵ asserted. The sum of theirs is the scale where the octaves' slopes
    // cancel: the broken law (`decision-r09-t5.md` item 2) leaves the finest octaves centimetres
    // and the total slope in places a hundredth, against which the finest octave's own error is
    // no longer small.
    for (kind, seed) in [
        (SyntheticWorld::EarthLike, 0x6772_6164),
        (SyntheticWorld::CeresLike, 0x6365_7273),
    ] {
        let field = world(kind);
        let relief = Relief::new(field.header(), SEED);
        let radius = field.header().radius().value();
        for (through, step) in [(15, 0.1), (relief.finest_octave(), 1e-3)] {
            let step = step * (radius / 6.371e6).max(0.1);
            let mut cache = SynthCache::new();
            for dir in uniform_points(seed, 300) {
                let g = relief.at(field, dir, through, &mut cache).unwrap().gradient;
                let size = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
                let scale = size.max(octave_slopes(&relief, field, dir, through));
                let fd = central_difference(&relief, field, dir, through, step);
                let error = [0, 1, 2].map(|k| fd[k] - g[k]);
                let error =
                    (error[0] * error[0] + error[1] * error[1] + error[2] * error[2]).sqrt();
                assert!(
                    error <= 1e-5 * scale + 1e-9,
                    "{kind:?} through {through} at {dir:?}: {g:?} vs {fd:?} (scale {scale})"
                );
            }
        }
    }
}

#[test]
fn the_relief_does_not_depend_on_the_order_of_queries_or_the_cache() {
    let field = earth();
    let relief = Relief::new(field.header(), SEED);
    let centre = unit_dir([0.3, -0.5, 0.8]);
    let points = points_near(0x6f72_6465, centre, 2e-6, 300);
    for through in [12, relief.finest_octave()] {
        let mut warm = SynthCache::new();
        relief.cover(field, centre, Metres::new(40.0), through, &mut warm);
        let warm = RefCell::new(warm);
        let query = |cache: &mut SynthCache, dir: [f64; 3]| {
            let h = relief.at(field, dir, through, cache).unwrap();
            [h.height_m, h.gradient[0], h.gradient[1], h.gradient[2]].map(bits)
        };
        assert_order_independent(&points, |&d| query(&mut warm.borrow_mut(), d));
        assert_order_independent(&points, |&d| query(&mut SynthCache::new(), d));
        for &d in &points {
            assert_eq!(
                query(&mut warm.borrow_mut(), d),
                query(&mut SynthCache::new(), d)
            );
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_relief_and_another_another() {
    let field = earth();
    let a = Relief::new(field.header(), SEED);
    let b = Relief::new(field.header(), SEED);
    let other = Relief::new(field.header(), DetailSeed::new(SEED.get() ^ 1));
    let mut differ = 0;
    for dir in uniform_points(0x7365_6564, 50) {
        let at = |r: &Relief| {
            let h = r.at(field, dir, 31, &mut SynthCache::new()).unwrap();
            [h.height_m, h.gradient[0], h.gradient[1], h.gradient[2]].map(bits)
        };
        assert_eq!(at(&a), at(&b));
        if at(&a) != at(&other) {
            differ += 1;
        }
    }
    assert_eq!(differ, 50);
}

/// One cache passed between two bodies' seeds and the test planet, back and forth, gives each its
/// own heights, as a fresh cache does.
#[test]
fn a_cache_shared_by_two_seeds_and_the_test_planet_gives_each_its_own_heights() {
    use crate::test_planet::TEST_PLANET;
    let field = earth();
    let a = Relief::new(field.header(), SEED);
    let b = Relief::new(field.header(), DetailSeed::new(0x006f_7468_6572));
    let centre = unit_dir([-0.4, 0.2, 0.9]);
    let points = points_near(0x7368_6172, centre, 2e-6, 60);
    let fresh = |r: &Relief, d| {
        let h = r.at(field, d, 31, &mut SynthCache::new()).unwrap();
        [h.height_m, h.gradient[0], h.gradient[1], h.gradient[2]].map(bits)
    };
    let mut shared = SynthCache::new();
    a.cover(field, centre, Metres::new(40.0), 31, &mut shared);
    for &d in &points {
        for r in [&a, &b, &a] {
            let h = r.at(field, d, 31, &mut shared).unwrap();
            let got = [h.height_m, h.gradient[0], h.gradient[1], h.gradient[2]].map(bits);
            assert_eq!(got, fresh(r, d));
        }
        let planet = TEST_PLANET.height(d, 19, shared.lattice_mut());
        assert_eq!(planet, TEST_PLANET.height(d, 19, &mut LatticeCache::new()));
        b.cover(field, d, Metres::new(10.0), 31, &mut shared);
    }
}

/// The relief's stored octave RMS is the direct computation's, bit for bit, so that heights and
/// `octave_bound` read one value.
#[test]
fn the_stored_octave_rms_is_the_direct_one() {
    for &kind in SyntheticWorld::ALL {
        let header = world(kind).header();
        let relief = Relief::new(header, SEED);
        for m in relief.first_octave()..=relief.finest_octave() {
            assert_same_bits(
                relief.octave_rms(m).value(),
                octave_rms(header.spectrum(), m).value(),
            );
        }
    }
}

/// A body too small for any octave finer than its coarse cells has no unresolved relief.
#[test]
fn a_body_too_small_for_octaves_has_no_unresolved_relief() {
    let tiny = crate::testing::FieldBuilder::new(Metres::new(20.0))
        .spectrum(BandSpectrum::anchored(
            SpectrumShape::SILICATE,
            Metres::new(20.0),
            Metres::new(1.0),
        ))
        .build();
    let header = tiny.header();
    assert!(finest_octave(header.radius()) < first_octave(header.level()));
    let cell = PatchKey::containing(header.level().get(), [0.0, 0.0, 1.0]).unwrap();
    let variance = unresolved_variance(&tiny, cell, Metres::new(1e3)).unwrap();
    assert_same_bits(variance.relief().value(), 0.0);
    let relief = Relief::new(header, SEED);
    let h = relief
        .at(&tiny, [0.0, 0.0, 1.0], 31, &mut SynthCache::new())
        .unwrap();
    assert_same_bits(h.height_m, 0.0);
}

/// A view of `inner` that holds only the cells of `held`.
struct Holding<'a> {
    inner: &'a CoarseField,
    held: BTreeSet<PatchKey>,
}

impl FieldView for Holding<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.held
            .contains(&cell)
            .then(|| self.inner.cell(cell))
            .flatten()
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.held
            .contains(&cell)
            .then(|| self.inner.climate(cell))
            .flatten()
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        let held = self.held.contains(&cell);
        self.inner.craters_reaching(cell).filter(move |_| held)
    }
}

/// A view of `inner` that records every cell asked of it.
struct Reading<'a> {
    inner: &'a CoarseField,
    reads: RefCell<BTreeSet<PatchKey>>,
}

impl FieldView for Reading<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.reads.borrow_mut().insert(cell);
        self.inner.cell(cell)
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.reads.borrow_mut().insert(cell);
        self.inner.climate(cell)
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        self.reads.borrow_mut().insert(cell);
        self.inner.craters_reaching(cell)
    }
}

/// The relief reads exactly the cells the base elevation reads, so it adds nothing to the margin
/// (Design note 15); and a cell not held is not surveyed, the first one missing named.
#[test]
fn the_relief_reads_the_base_elevation_s_cells_and_no_others() {
    let field = earth();
    let relief = Relief::new(field.header(), SEED);
    for dir in uniform_points(0x7265_6164, 200) {
        let base = Reading {
            inner: field,
            reads: RefCell::new(BTreeSet::new()),
        };
        interp::base_elevation(&base, dir).unwrap();
        let styles = Reading {
            inner: field,
            reads: RefCell::new(BTreeSet::new()),
        };
        relief.at(&styles, dir, 31, &mut SynthCache::new()).unwrap();
        assert_eq!(styles.reads.take(), base.reads.take(), "{dir:?}");
        let mut held = Reading {
            inner: field,
            reads: RefCell::new(BTreeSet::new()),
        };
        interp::base_elevation(&held, dir).unwrap();
        let mut cells = held.reads.get_mut().clone();
        let missing = *cells.iter().nth(cells.len() / 2).unwrap();
        cells.remove(&missing);
        let partial = Holding {
            inner: field,
            held: cells,
        };
        let refused = relief.at(&partial, dir, 31, &mut SynthCache::new());
        assert_eq!(
            refused.err().map(|e| e.cell),
            interp::base_elevation(&partial, dir).err().map(|e| e.cell)
        );
    }
}

/// The sea floor byte of hills of relief `hills_m` under `ponded_m` of ponded sediment.
fn seafloor(hills_m: f64, ponded_m: f64) -> u8 {
    SynthesisCell::pack_seafloor(
        SynthesisCell::quantise_hill_relief(Metres::new(hills_m)).unwrap(),
        SynthesisCell::quantise_ponded_sediment(Metres::new(ponded_m)).unwrap(),
    )
    .unwrap()
}

/// A cell of crust `crust` at `distance_km` from a boundary of kind `boundary` and obliquity
/// `obliquity` (in 255ths of 90°), its sea floor the reference hills where it is oceanic.
fn cell(crust: Crust, boundary: BoundaryKind, distance_km: i16, obliquity: u8) -> SynthesisCell {
    SynthesisCell {
        elevation_mm: 0,
        boundary_distance_km: distance_km,
        plate: 0,
        crust,
        boundary,
        boundary_obliquity: obliquity,
        flow: FlowDirection::Terminal,
        drainage: LogArea::ZERO,
        steepness: LogSteepness::ZERO,
        water_surface_mm: 0,
        ice: 0,
        substances: SynthesisCell::NO_SUBSTANCES,
        class: SurfaceClass::UNCLASSIFIED,
        crater_state: 0,
        seafloor: match crust {
            Crust::Oceanic => seafloor(H_REF.value(), 0.0),
            Crust::Continental | Crust::Lid | Crust::Province => SynthesisCell::NO_SEAFLOOR,
        },
    }
}

#[test]
fn styles_follow_the_crust_and_the_nearest_boundary() {
    let none = SynthesisCell::NO_BOUNDARY_KM;
    let style = |c, b, d, o| ReliefStyle::of(&cell(c, b, d, o));
    // A stagnant lid's crust is the reference, and its provinces a quarter of it.
    assert_eq!(
        style(Crust::Lid, BoundaryKind::Absent, none, 0),
        ReliefStyle::REFERENCE
    );
    let province = style(Crust::Province, BoundaryKind::Absent, none, 0);
    assert_same_bits(province.amplitude(), PROVINCE_AMPLITUDE);
    // A collision belt is ridged and rough on both sides, and fades by 250 km.
    let belt = style(Crust::Continental, BoundaryKind::Collision, -50, 0);
    assert_same_bits(belt.belt(), 1.0);
    assert_same_bits(belt.amplitude(), BELT_AMPLITUDE);
    assert_same_bits(
        style(Crust::Continental, BoundaryKind::Collision, 250, 0).belt(),
        0.0,
    );
    let half = style(Crust::Continental, BoundaryKind::Collision, 175, 0).belt();
    assert!((half - 0.5).abs() < 1e-12, "{half}");
    let slow_floor = seafloor(232.0, 0.0);
    // An oblique boundary shears, over a narrower zone on oceanic crust; a head-on one does not.
    let oblique = style(Crust::Continental, BoundaryKind::Transform, 50, 255);
    assert_same_bits(oblique.shear(), 1.0);
    assert_same_bits(oblique.belt(), 0.0);
    assert_same_bits(
        style(Crust::Oceanic, BoundaryKind::Transform, 50, 255).shear(),
        0.0,
    );
    assert_same_bits(
        style(Crust::Oceanic, BoundaryKind::Transform, 10, 255).shear(),
        1.0,
    );
    assert_same_bits(
        style(Crust::Continental, BoundaryKind::Divergent, 50, 0).shear(),
        0.0,
    );
    // A continent is rough in its deformation zone and smooth in its interior.
    let rift = style(Crust::Continental, BoundaryKind::Divergent, 100, 0);
    assert_same_bits(rift.amplitude(), ACTIVE_AMPLITUDE);
    let interior = style(Crust::Continental, BoundaryKind::Transform, 2_000, 0);
    assert_same_bits(interior.amplitude(), INTERIOR_AMPLITUDE);
    // Oceanic crust's amplitude is its sea floor's, wherever its boundary is.
    let floor = |byte: u8, d: i16| {
        ReliefStyle::of(&SynthesisCell {
            seafloor: byte,
            ..cell(Crust::Oceanic, BoundaryKind::Divergent, d, 0)
        })
        .amplitude()
    };
    assert_same_bits(floor(slow_floor, 100), floor(slow_floor, 3_000));
}

/// A subduction arc rises behind the boundary on the overriding plate only, in a band set by the
/// overriding crust (`decision-r09-t5.md` item 6), and the forearc before it keeps its crust's own
/// style.
#[test]
fn arcs_lie_in_bands_behind_their_trenches_set_by_the_overriding_crust() {
    let style = |c, b, d, o| ReliefStyle::of(&cell(c, b, d, o));
    // Full 200 km behind an oceanic trench and 300 km behind a continental one, nothing 50 km
    // behind either or on the subducting plate.
    for (crust, full) in [(Crust::Oceanic, 200), (Crust::Continental, 300)] {
        assert_same_bits(style(crust, BoundaryKind::Subduction, full, 0).belt(), 1.0);
        assert_same_bits(style(crust, BoundaryKind::Subduction, 50, 0).belt(), 0.0);
        assert_same_bits(style(crust, BoundaryKind::Subduction, -150, 0).belt(), 0.0);
    }
    // The bands' edges: an island arc is gone by 330 km, a cordillera by 600, and each rises
    // over its ramp.
    for (crust, d, belt) in [
        (Crust::Oceanic, 90, 0.0),
        (Crust::Oceanic, 140, 1.0),
        (Crust::Oceanic, 250, 1.0),
        (Crust::Oceanic, 330, 0.0),
        (Crust::Continental, 130, 0.0),
        (Crust::Continental, 180, 1.0),
        (Crust::Continental, 450, 1.0),
        (Crust::Continental, 600, 0.0),
    ] {
        assert_same_bits(style(crust, BoundaryKind::Subduction, d, 0).belt(), belt);
    }
    let rising = style(Crust::Oceanic, BoundaryKind::Subduction, 115, 0).belt();
    assert!((rising - 0.5).abs() < 1e-12, "{rising}");
    let falling = style(Crust::Continental, BoundaryKind::Subduction, 525, 0).belt();
    assert!((falling - 0.5).abs() < 1e-12, "{falling}");
    // The forearc keeps its crust's own style: the deformation zone on a continent, the sea floor
    // on an oceanic plate.
    let forearc = style(Crust::Continental, BoundaryKind::Subduction, 60, 0);
    assert_same_bits(forearc.amplitude(), ACTIVE_AMPLITUDE);
    let slow_floor = seafloor(232.0, 0.0);
    let island_forearc = ReliefStyle::of(&SynthesisCell {
        seafloor: slow_floor,
        ..cell(Crust::Oceanic, BoundaryKind::Subduction, 60, 0)
    });
    let open_floor = ReliefStyle::of(&SynthesisCell {
        seafloor: slow_floor,
        ..cell(Crust::Oceanic, BoundaryKind::Divergent, 2_000, 0)
    });
    assert_same_bits(island_forearc.belt(), 0.0);
    assert_same_bits(island_forearc.amplitude(), open_floor.amplitude());
}

/// Every style lies within the bounds the octaves state, on every sea floor, and is the one its
/// stored codes give ([`ReliefStyle::from_codes`]).
#[test]
fn every_style_lies_within_the_octaves_bounds_and_comes_from_its_codes() {
    let none = SynthesisCell::NO_BOUNDARY_KM;
    for &crust in Crust::ALL {
        for &boundary in BoundaryKind::ALL {
            for d in [-2_000, -300, -100, 0, 100, 300, 2_000, none] {
                for o in [0, 128, 255] {
                    for byte in [0x10, 0x80, 0xF0, 0x1F, 0xFF, 0x87] {
                        let at = SynthesisCell {
                            seafloor: if crust == Crust::Oceanic {
                                byte
                            } else {
                                SynthesisCell::NO_SEAFLOOR
                            },
                            ..cell(crust, boundary, d, o)
                        };
                        let s = ReliefStyle::of(&at);
                        assert!(
                            s.amplitude() > 0.0 && s.amplitude() <= MAX_AMPLITUDE,
                            "{s:?}"
                        );
                        assert!(
                            (0.0..=1.0).contains(&s.belt()) && (0.0..=1.0).contains(&s.shear())
                        );
                        // `of` reads exactly the codes `from_codes` takes.
                        assert_eq!(
                            s,
                            ReliefStyle::from_codes(
                                at.crust,
                                at.boundary,
                                at.boundary_distance_km,
                                at.boundary_obliquity,
                                at.seafloor,
                            )
                        );
                    }
                }
            }
        }
    }
}

/// The sea floor's amplitude (`decision-r09-t5.md` item 5): the hills' (H ÷ 95 m)^0.6, 1.71 on a
/// slow ridge's 232 m, 0.72 on a fast one's 55 m and 0.48 draped to 28 m, capped at the belts';
/// unchanged while the ponded sediment is under half the hills' relief, a plain's from three
/// times it, and continuous between.
#[test]
fn the_sea_floor_s_amplitude_follows_its_hills_and_their_burial() {
    let amplitude = |byte: u8| {
        ReliefStyle::of(&SynthesisCell {
            seafloor: byte,
            ..cell(Crust::Oceanic, BoundaryKind::Divergent, 500, 0)
        })
        .amplitude()
    };
    let hills = |code: u8| {
        let byte = SynthesisCell::pack_seafloor(code, 0).unwrap();
        let h = crate::field::hill_relief_of(byte).unwrap().value();
        (byte, h)
    };
    // The ruling's three floors, exactly and through the hill code nearest each, which is within
    // half the code's step, 2^(1 ÷ 6) in relief, so 2^(0.6 ÷ 6) in amplitude.
    for (relief_m, expected) in [(232.0, 1.71), (55.0, 0.72), (28.0, 0.48)] {
        let exact = math::powf(relief_m / H_REF.value(), HILL_AMPLITUDE_EXPONENT);
        assert!((exact - expected).abs() < 0.005, "{relief_m} m: {exact}");
        let a = amplitude(seafloor(relief_m, 0.0));
        assert!(
            math::ln(a / exact).abs() <= HILL_AMPLITUDE_EXPONENT * core::f64::consts::LN_2 / 6.0,
            "{relief_m} m: {a} against {exact}"
        );
    }
    // Every hill code's amplitude, undrowned, is its relief's law, capped at the belts'.
    for code in 1..=15 {
        let (byte, h) = hills(code);
        let law = math::powf(h / H_REF.value(), HILL_AMPLITUDE_EXPONENT);
        assert_same_bits(amplitude(byte), num::min(law, BELT_AMPLITUDE));
    }
    assert_same_bits(amplitude(0xF0), BELT_AMPLITUDE);
    // The ruling's law, a_h^(1 − t) PLAIN_AMPLITUDE^t with t = clamp(ln(2 S_t ÷ H) ÷ ln 6, 0, 1),
    // which is continuous in the sediment: at the codes' depths below, and at depths no code holds
    // about both ends of its ramp.
    let law = |h: f64, s: f64| {
        let a_h = num::min(
            math::powf(h / H_REF.value(), HILL_AMPLITUDE_EXPONENT),
            BELT_AMPLITUDE,
        );
        let (onset, buried) = PONDING;
        let t = (math::ln(s / h / onset) / math::ln(buried / onset)).clamp(0.0, 1.0);
        math::powf(a_h, 1.0 - t) * math::powf(PLAIN_AMPLITUDE, t)
    };
    for h in [28.0, 95.0, 232.0] {
        for s in [h / 2.0, 3.0 * h] {
            let below = law(h, s * (1.0 - 1e-9));
            let above = law(h, s * (1.0 + 1e-9));
            assert!((below - above).abs() < 1e-8, "{h} m at {s} m");
        }
    }
    // Every byte's amplitude is the law's at its codes' relief and sediment: the hills' own a_h at
    // or below H ÷ 2, a plain's at or above 3H, the ramp between, falling as the sediment deepens.
    for code in 1..=15 {
        let (byte, h) = hills(code);
        let a_h = amplitude(byte);
        let mut previous = a_h;
        for ponded in 0..=15_u8 {
            let s = crate::field::ponded_sediment_of(ponded).value();
            let a = amplitude(byte | ponded);
            if s <= h / 2.0 {
                assert_same_bits(a, a_h);
            } else if s >= 3.0 * h {
                assert_same_bits(a, PLAIN_AMPLITUDE);
            } else {
                assert_same_bits(a, law(h, s));
            }
            assert!(a <= previous + 1e-15, "hills {code}, ponded {ponded}: {a}");
            previous = a;
        }
    }
}

#[test]
fn unresolved_variance_falls_with_the_wavelength_to_zero_at_the_band_limit() {
    let field = earth();
    let header = field.header();
    let level = header.level().get();
    let mut checked = 0;
    for dir in uniform_points(0x756e_7265, 60) {
        let cell = PatchKey::containing(level, dir).unwrap();
        let a = ReliefStyle::of(field.cell(cell).unwrap()).amplitude();
        let at = |w: f64| {
            unresolved_variance(field, cell, Metres::new(w))
                .unwrap()
                .relief()
                .value()
        };
        // Every wavelength the coarse cell resolves: the whole of the octaves' bands.
        let relief = Relief::new(header, SEED);
        let mut carried = 0.0;
        for m in relief.first_octave()..=relief.finest_octave() {
            let rms = relief.octave_rms(m).value();
            carried += rms * rms;
        }
        assert!((at(1e9) / (a * a * carried) - 1.0).abs() < 1e-12);
        let mut previous = f64::INFINITY;
        for w in [1e7, 1e5, 3e4, 1e4, 1e3, 1e2, 10.0, 5.0] {
            let v = at(w);
            assert!(v <= previous && v > 0.0, "{w} m: {v}");
            previous = v;
        }
        assert_same_bits(at(2.0), 0.0);
        assert_same_bits(at(0.5), 0.0);
        assert_eq!(
            unresolved_variance(field, cell, Metres::new(1e3))
                .unwrap()
                .total(),
            unresolved_variance(field, cell, Metres::new(1e3))
                .unwrap()
                .relief()
        );
        checked += 1;
    }
    assert_eq!(checked, 60);
    // A cell the view does not hold has none.
    let held = Holding {
        inner: field,
        held: BTreeSet::new(),
    };
    let cell = PatchKey::containing(level, [0.0, 0.0, 1.0]).unwrap();
    assert_eq!(unresolved_variance(&held, cell, Metres::new(1e3)), None);
    assert!(cell_index(cell) < header.level().cell_count());
}

/// The ridged multifractal's weight's RMS, `RIDGE_WEIGHT_RMS`, measured over 10⁷ points of one
/// octave's noise far apart (its value depends on the noise's distribution alone).
#[test]
#[ignore = "slow: measures the ridge weight's RMS over 10^7 points"]
fn the_ridge_weight_rms_is_measured() {
    let octave = Octave::new(
        4,
        1.0,
        noise::rotation(4),
        [0.31, 0.47, 0.73],
        NoiseKey::Relief(SEED),
    );
    let mut rng = Lcg::new(0x7765_6967);
    let mut cache = LatticeCache::new();
    let n = 10_000_000_u32;
    let mut sum_sq = 0.0;
    for _ in 0..n {
        let p = [0, 1, 2].map(|_| (rng.next_f64() * 2.0 - 1.0) * 1.0e6);
        let v = gradient_noise(p, &octave, &mut cache).0;
        let w = ridge_weight(1.0 - (v * v + RIDGE_EPSILON * RIDGE_EPSILON).sqrt()).0;
        sum_sq += w * w;
    }
    let rms = (sum_sq / f64::from(n)).sqrt();
    println!("ridge weight RMS {rms}");
    assert!(
        (rms - RIDGE_WEIGHT_RMS).abs() < 2e-4,
        "{rms} vs {RIDGE_WEIGHT_RMS}"
    );
}
