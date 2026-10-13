//! The craters' bound terms' tests (plan R09, T8): Chernoff's bound against exact Poisson tails,
//! and each octave's marks against its craters' own profiles.

use hyperion_testkit::float::assert_same_bits;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::field::FieldView;
use crate::synth::levels::marks_of;
use crate::synth::test_support::world;
use crate::testing::SyntheticWorld;

/// P(N ≥ n) for N Poisson of mean `mu`, summed from the top of a long series.
fn poisson_at_least(mu: f64, n: u32) -> f64 {
    let mut term = math::exp(-mu);
    let mut below = 0.0;
    for k in 0..n {
        below += term;
        term *= mu / f64::from(k + 1);
    }
    (1.0 - below).max(0.0)
}

#[test]
fn the_chernoff_bound_holds_against_exact_poisson_tails() {
    let ln_tail = -math::ln(1e-9);
    for mu in [1e-4, 0.01, 0.1, 0.5, 1.0, 4.0, 20.0] {
        let t = poisson_tail_bound(&[(mu, 1.0)], ln_tail);
        // With unit marks the sum is the count, so t bounds its 10⁻⁹ quantile, and the bound is
        // never more than a few counts loose.
        let count = (0_u32..10_000).find(|&n| f64::from(n) >= t).unwrap();
        assert!(
            poisson_at_least(mu, count) <= 1e-9,
            "at μ {mu}: P(N ≥ {count}) = {}",
            poisson_at_least(mu, count)
        );
        let exact = (0..200).find(|&n| poisson_at_least(mu, n) <= 1e-9).unwrap();
        assert!(
            t >= f64::from(exact) - 1.0 && t <= 2.0 * f64::from(exact) + 4.0,
            "μ {mu}: {t}"
        );
        assert!(t >= mu, "the bound is at least the mean");
    }
    // Nothing adds nothing; marks scale it; more terms raise it.
    assert_same_bits(poisson_tail_bound(&[], ln_tail), 0.0);
    assert_same_bits(poisson_tail_bound(&[(0.0, 5.0), (3.0, 0.0)], ln_tail), 0.0);
    let one = poisson_tail_bound(&[(0.3, 2.0)], ln_tail);
    assert!((poisson_tail_bound(&[(0.3, 6.0)], ln_tail) / one - 3.0).abs() < 1e-6);
    assert!(poisson_tail_bound(&[(0.3, 2.0), (0.3, 1.0)], ln_tail) > one);
    assert!(poisson_tail_bound(&[(0.3, 2.0)], -math::ln(1e-12)) > one);
}

#[test]
fn an_octave_s_marks_bound_its_craters_profiles() {
    for kind in [SyntheticWorld::MoonLike, SyntheticWorld::MarsLike] {
        let header = world(kind).header();
        let octaves = CraterOctaves::new(header);
        let marks = marks_of(header);
        assert_eq!(marks.len(), octaves.octaves().len());
        let figure = header.figure();
        for (octave, mark) in octaves.octaves().iter().zip(&marks) {
            assert!(
                mark.expected >= 0.0 && mark.fresh_m > 0.0,
                "{kind:?} {octave:?}"
            );
            assert!(mark.expected.is_finite() && mark.fresh_m.is_finite());
            // A crater between the samples, at every level's band, keeps within its octave's
            // marks: its height within the fresh mark and the fillet's, its change within the
            // level's fillet mark.
            let (smallest, upper) = kept_span(octave);
            let diameter = (smallest * upper).sqrt() * 1.013;
            let shape = crater_shape(&octaves.inputs.params, Metres::new(diameter));
            let fresh = CraterProfile::filleted(shape, Metres::ZERO);
            for n in [0_u8, 6, 10, 14, 18, 24] {
                let band = super::super::band_half_width(
                    &figure,
                    crate::synth::BandLevel::new(n).unwrap(),
                );
                let filleted = CraterProfile::filleted(shape, band);
                for i in 0..=400 {
                    let r = 2.6 * f64::from(i) / 400.0;
                    let h = filleted.at(r).height_m;
                    let change = (h - fresh.at(r).height_m).abs();
                    let fillet_mark = mark.fillet_m[usize::from(n)];
                    assert!(change <= fillet_mark + 1e-9, "{kind:?} D {diameter} at {r}");
                    assert!(h.abs() <= mark.fresh_m + fillet_mark + 1e-9);
                }
            }
            // Its count is its density over the area its craters reach, never past the widest.
            let widest = f64::midpoint(1.0, MAX_EXTERIOR_WIDTH) * upper;
            let density_per_m2 = octave.density().value() * 1e-6;
            assert!(mark.expected <= density_per_m2 * core::f64::consts::PI * widest * widest);
            assert!(mark.expected > 0.0 || density_per_m2 <= 0.0);
        }
    }
}

#[test]
fn the_coarse_rims_marks_bound_their_sharpenings_changes() {
    for kind in [SyntheticWorld::MoonLike, SyntheticWorld::OneCrater] {
        let header = world(kind).header();
        let figure = header.figure();
        let bands: [f64; crate::synth::BAND_LEVELS] = core::array::from_fn(|n| {
            let level = crate::synth::BandLevel::new(u8::try_from(n).unwrap()).unwrap();
            super::super::band_half_width(&figure, level).value()
        });
        let coarse_band =
            super::super::rims::coarse_band_half_width(&figure, header.level()).value();
        let (d_b, radius_m) = (header.boundary_diameter().value(), header.radius().value());
        let marks = coarse_marks(header.craters(), coarse_band, d_b, radius_m, &bands);
        // Octaves of D_b × 2^k up to π R.
        assert!(!marks.is_empty());
        let mut lower = d_b;
        for mark in &marks {
            assert!(lower < core::f64::consts::PI * radius_m);
            // At saturation, over the widest reach of the octave's largest crater.
            let capped = crate::craters::saturation(Metres::new(lower)).value()
                - crate::craters::saturation(Metres::new(2.0 * lower)).value();
            let widest = f64::midpoint(1.0, MAX_EXTERIOR_WIDTH) * (2.0 * lower);
            let expected = capped * 1e-6 * core::f64::consts::PI * widest * widest;
            assert!((mark.expected / expected - 1.0).abs() < 1e-12);
            // A crater between the samples keeps its changes within the marks at every band, held
            // to the coarse band as the sharpening is.
            let diameter = lower * 1.37;
            let shape = crater_shape(header.craters(), Metres::new(diameter));
            let fresh = CraterProfile::filleted(shape, Metres::ZERO);
            let coarse = CraterProfile::filleted(shape, Metres::new(coarse_band));
            for n in [0_usize, 4, 8, 12, 16, 20, 24] {
                let fine = CraterProfile::filleted(shape, Metres::new(bands[n].min(coarse_band)));
                for i in 0..=800 {
                    let r = (1.0 + shape.exterior_width()) * f64::from(i) / 800.0;
                    let at = |p: &CraterProfile| p.at(r).height_m;
                    assert!((at(&fine) - at(&fresh)).abs() <= mark.fillet_m[n] + 1e-9);
                    assert!((at(&coarse) - at(&fresh)).abs() <= mark.coarse_m + 1e-9);
                }
            }
            lower *= 2.0;
        }
        assert!(lower >= core::f64::consts::PI * radius_m);
    }
}
