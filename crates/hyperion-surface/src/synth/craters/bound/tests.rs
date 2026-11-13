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
            // Its count is its density over the area its craters reach, never past the widest,
            // with the figure's allowance for the ground a chord disc holds (R09.T8.d).
            let widest = f64::midpoint(1.0, MAX_EXTERIOR_WIDTH) * upper;
            let density_per_m2 = octave.density().value() * 1e-6;
            let allowance = chord_area_allowance(&figure);
            assert!(
                mark.expected
                    <= density_per_m2 * core::f64::consts::PI * widest * widest * allowance
            );
            assert!(mark.expected > 0.0 || density_per_m2 <= 0.0);
        }
    }
}

/// Asserts two lists of rims' marks are the same to the bit.
fn assert_same_marks(a: &[RimMarks], b: &[RimMarks]) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert_same_bits(x.expected, y.expected);
        for n in 0..crate::synth::BAND_LEVELS {
            assert_same_bits(x.expected_at[n], y.expected_at[n]);
            assert_same_bits(x.height_m[n], y.height_m[n]);
            assert_same_bits(x.change_m[n], y.change_m[n]);
        }
    }
}

/// Each band level's fillet half-width on `figure`, metres.
fn bands_of(figure: &crate::spheroid::Spheroid) -> [f64; crate::synth::BAND_LEVELS] {
    core::array::from_fn(|n| {
        let level = crate::synth::BandLevel::new(u8::try_from(n).unwrap()).unwrap();
        super::super::band_half_width(figure, level).value()
    })
}

/// `header`'s coarse marks sampled by `sampling`, with its coarse band and bands.
fn rim_marks_of(header: &crate::field::FieldHeader, sampling: RimSampling) -> Vec<RimMarks> {
    let figure = header.figure();
    let coarse_band = super::super::rims::coarse_band_half_width(&figure, header.level()).value();
    coarse_marks_sampled(
        header.craters(),
        coarse_band,
        header.boundary_diameter().value(),
        header.radius().value(),
        &bands_of(&figure),
        chord_area_allowance(&figure),
        sampling,
    )
}

#[test]
fn the_coarse_rims_marks_bound_their_sharpenings_changes() {
    for kind in [SyntheticWorld::MoonLike, SyntheticWorld::OneCrater] {
        let header = world(kind).header();
        let figure = header.figure();
        let bands = bands_of(&figure);
        let coarse_band =
            super::super::rims::coarse_band_half_width(&figure, header.level()).value();
        let (d_b, radius_m) = (header.boundary_diameter().value(), header.radius().value());
        let allowance = chord_area_allowance(&figure);
        let marks = coarse_marks(
            header.craters(),
            coarse_band,
            d_b,
            radius_m,
            &bands,
            allowance,
        );
        assert_same_marks(&marks, &rim_marks_of(header, RimSampling::BOUND));
        // Octaves of D_b × 2^k up to π R.
        assert_ne!(marks, []);
        let mut lower = d_b;
        for mark in &marks {
            assert!(lower < core::f64::consts::PI * radius_m);
            // At saturation, over the area where the coarse fillet changes the octave's craters,
            // and over the level's own band's area for the change, never past the widest reach of
            // the octave's largest crater (R09.T8.d), each with the figure's allowance.
            let capped = crate::craters::saturation(Metres::new(lower)).value()
                - crate::craters::saturation(Metres::new(2.0 * lower)).value();
            let count = |band: f64| capped * 1e-6 * filleted_area_m2(lower, band, allowance);
            assert_same_bits(mark.expected, count(coarse_band));
            let widest = f64::midpoint(1.0, MAX_EXTERIOR_WIDTH) * (2.0 * lower);
            let reach = capped * 1e-6 * core::f64::consts::PI * widest * widest * allowance;
            assert!(mark.expected > 0.0 && mark.expected <= reach * (1.0 + 1e-12));
            for (n, &band) in bands.iter().enumerate() {
                assert_same_bits(mark.expected_at[n], count(band.min(coarse_band)));
                assert!(mark.expected_at[n] <= mark.expected);
                // No sharpening at or beyond the coarse band.
                if band >= coarse_band {
                    assert_same_bits(mark.height_m[n], 0.0);
                }
            }
            // A crater between the samples keeps its sharpening, and its change to every finer
            // level, within the marks at every band, held to the coarse band as the sharpening is.
            for along in [0.13, 0.452, 0.871] {
                let diameter = lower * math::powf(2.0, along);
                let shape = crater_shape(header.craters(), Metres::new(diameter));
                let coarse = CraterProfile::filleted(shape, Metres::new(coarse_band));
                let at_band = |n: usize| {
                    CraterProfile::filleted(shape, Metres::new(bands[n].min(coarse_band)))
                };
                for n in [0_usize, 2, 4, 8, 12, 16, 20, 24] {
                    let fine = at_band(n);
                    for i in 0..=800 {
                        let r = (1.0 + shape.exterior_width()) * f64::from(i) / 800.0;
                        let at = |p: &CraterProfile| p.at(r).height_m;
                        assert!(
                            (at(&fine) - at(&coarse)).abs() <= mark.height_m[n] + 1e-9,
                            "{kind:?} D {diameter} level {n} at {r}"
                        );
                        for m in n + 1..crate::synth::BAND_LEVELS {
                            if m % 3 == 0 || m == n + 1 {
                                let change = (at(&fine) - at(&at_band(m))).abs();
                                assert!(change <= mark.change_m[n] + 1e-9, "{kind:?} {n} {m}");
                            }
                        }
                    }
                }
            }
            lower *= 2.0;
        }
        assert!(lower >= core::f64::consts::PI * radius_m);
    }
}

#[test]
fn the_rims_sampled_shares_hold_a_ten_times_denser_sampling() {
    // Ten times the diameters and the steps across each span, unwidened: every sampled share
    // stays within the bound's, whose margin covers what the samples miss (R09.T8.d).
    let dense = RimSampling::new(10 * (DIAMETER_SAMPLES - 1) + 1, 10 * FILLET_SAMPLES, 1.0);
    // Every world natively and on wasip1, the Moon-like alone on the browser target
    // (`crate::test_sizes`).
    let worlds = crate::test_sizes::per_target(SyntheticWorld::ALL, &[SyntheticWorld::MoonLike]);
    for &kind in worlds {
        let header = world(kind).header();
        let marks = rim_marks_of(header, RimSampling::BOUND);
        let denser = rim_marks_of(header, dense);
        assert_eq!(marks.len(), denser.len());
        for (k, (mark, finer)) in marks.iter().zip(&denser).enumerate() {
            for n in 0..crate::synth::BAND_LEVELS {
                assert!(
                    finer.height_m[n] <= mark.height_m[n],
                    "{kind:?} octave {k} level {n}: height {} over {}",
                    finer.height_m[n],
                    mark.height_m[n]
                );
                assert!(
                    finer.change_m[n] <= mark.change_m[n],
                    "{kind:?} octave {k} level {n}: change {} over {}",
                    finer.change_m[n],
                    mark.change_m[n]
                );
            }
        }
    }
}

#[test]
fn every_drawn_cone_is_narrower_than_the_widest_peak_fraction() {
    // 12,000 diameters, even in ln D, from half the transition diameter, below the central peak's
    // onset, to π R, at least 10⁴ of them from the transition on, on the Moon-like and the
    // Earth-like contracts: every cone base the generator draws is below the bound the rims'
    // count reads (R09.T8.d), whatever the crater's morphology.
    for kind in [SyntheticWorld::MoonLike, SyntheticWorld::EarthLike] {
        let header = world(kind).header();
        let params = header.craters();
        let transition = crate::craters::transition_diameter(params).value();
        let (from, to) = (0.5 * transition, PI * header.radius().value());
        let count = 12_000_u32;
        let mut cones = 0_u32;
        let mut complex = 0_u32;
        for i in 0..count {
            let along = f64::from(i) / f64::from(count - 1);
            let d = from * math::powf(to / from, along);
            if d >= transition {
                complex += 1;
            }
            let shape = crater_shape(params, Metres::new(d));
            let cone = shape.peak_fraction();
            assert!(
                (0.0..WIDEST_PEAK_FRACTION).contains(&cone),
                "{kind:?} D {d} m: cone {cone}"
            );
            if cone > 0.0 {
                cones += 1;
            }
        }
        assert!(
            complex >= 10_000,
            "{kind:?}: {complex} diameters from the transition on"
        );
        // The sweep crosses the central-peak craters.
        assert!(cones > 1_000, "{kind:?}: {cones}");
    }
}

/// The ground within each chord of `radii` of the point at parametric latitude `latitude`
/// (longitude 0) on the spheroid of equatorial radius 1 and polar radius `polar`, by the midpoint
/// rule on a `steps` × 2`steps` grid in parametric latitude u and longitude, whose area element is
/// cos u √(c² cos² u + sin² u), c the polar radius.
fn chord_cap_areas(polar: f64, latitude: f64, radii: &[f64], steps: u32) -> Vec<f64> {
    let centre = [math::cos(latitude), 0.0, polar * math::sin(latitude)];
    let step = PI / f64::from(steps);
    let mut areas = vec![0.0; radii.len()];
    for i in 0..steps {
        let at = -0.5 * PI + (f64::from(i) + 0.5) * step;
        let (cos_at, sin_at) = (math::cos(at), math::sin(at));
        let element =
            cos_at * (polar * polar * cos_at * cos_at + sin_at * sin_at).sqrt() * step * step;
        for j in 0..2 * steps {
            let longitude = -PI + (f64::from(j) + 0.5) * step;
            let gap = [
                cos_at * math::cos(longitude) - centre[0],
                cos_at * math::sin(longitude),
                polar * sin_at - centre[2],
            ];
            let gap2 = gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2];
            for (area, chord) in areas.iter_mut().zip(radii) {
                if gap2 <= chord * chord {
                    *area += element;
                }
            }
        }
    }
    areas
}

#[test]
fn chord_discs_and_annuli_hold_within_the_allowance() {
    // On a sphere the ground within a chord r is π r² (Archimedes). On an oblate spheroid of
    // flattening f, k = (a ÷ c)² bounds it, k π r² for a disc and π (k r₂² − r₁²) for an annulus
    // (R09.T8.d; decision-r09-t8d-reach §1 and its follow-up 2). The worst disc is the whole body
    // from a pole, at r = 2c while f ≤ 0.29 and 1.40029 a at f = 0.3, and the thin rings are
    // worst near the pole's chord 2c. The sphere's quadrature error, at the same point and chord,
    // is taken off each area.
    let (n, sphere_tolerance) = crate::test_sizes::per_target((720, 0.01), (240, 0.03));
    let centres = [0.5 * PI, 0.25 * PI, 0.0];
    for f in [0.0, 0.1, 0.2, 0.3] {
        let c = 1.0 - f;
        let k = chord_area_allowance(&crate::spheroid::Spheroid {
            equatorial_radius_m: 1.0,
            polar_radius_m: c,
        });
        let worst_r = if f <= 0.29 { 2.0 * c } else { 1.400_29 };
        let radii = [0.5, 1.0, 1.2, 1.4, worst_r - 0.05, worst_r, 1.8, 1.9];
        let mut worst = 0.0_f64;
        for u0 in centres {
            let sphere = chord_cap_areas(1.0, u0, &radii, n);
            let areas = chord_cap_areas(c, u0, &radii, n);
            let corrected: Vec<f64> = areas
                .iter()
                .zip(&sphere)
                .zip(radii)
                .map(|((area, s), r)| {
                    let exact = PI * r * r;
                    assert!(
                        (s / exact - 1.0).abs() < sphere_tolerance,
                        "sphere: r {r} at {u0}: {s}"
                    );
                    area - (s - exact)
                })
                .collect();
            for (area, r) in corrected.iter().zip(radii) {
                worst = worst.max(area / (PI * r * r));
                // A part in 10⁹ for the rounding of the sphere's correction.
                assert!(
                    *area <= k * PI * r * r * (1.0 + 1e-9),
                    "f {f}: disc r {r} at {u0}: {area} over k {k}"
                );
            }
            // Each ring between consecutive chords, the thin one at the worst chord among them.
            for (j, (inner, outer)) in radii.iter().zip(radii.iter().skip(1)).enumerate() {
                let ring = corrected[j + 1] - corrected[j];
                let bound = PI * (k * outer * outer - inner * inner) + 1e-9 * PI * outer * outer;
                assert!(
                    ring <= bound,
                    "f {f}: ring {inner}–{outer} at {u0}: {ring} over {bound}"
                );
            }
        }
        eprintln!("f {f}: worst disc {worst:.4} against k {k:.4}");
    }
}

/// The radii, metres, at which `fillet`'s profile differs from `fresh`'s on 4,001 even steps out to
/// the reach: the least and the greatest, or `None` where it differs nowhere.
fn changed_radii(fresh: &CraterProfile, fillet: &CraterProfile, rim_m: f64) -> Option<(f64, f64)> {
    let reach = 1.0 + fresh.shape().exterior_width();
    let scale = fresh.shape().depth().value();
    let mut found: Option<(f64, f64)> = None;
    for i in 0..=4000 {
        let x = reach * f64::from(i) / 4000.0;
        let change = (fillet.at(x).height_m - fresh.at(x).height_m).abs();
        if change > 1e-12 * scale {
            let r = x * rim_m;
            found = Some(found.map_or((r, r), |(lo, hi)| (lo.min(r), hi.max(r))));
        }
    }
    found
}

#[test]
fn the_rims_count_area_holds_every_crater_s_changed_ground() {
    // Wherever a fillet changes a coarse crater's fresh profile, the crater's centre lies within
    // the area its octave's count takes about the point, at every band a level fillets with
    // (R09.T8.d): the annulus of the fillet's span, or the disc where it reaches the cone.
    for kind in [
        SyntheticWorld::MoonLike,
        SyntheticWorld::EarthLike,
        SyntheticWorld::CeresLike,
    ] {
        let header = world(kind).header();
        let figure = header.figure();
        let coarse_band =
            super::super::rims::coarse_band_half_width(&figure, header.level()).value();
        let (d_b, radius_m) = (header.boundary_diameter().value(), header.radius().value());
        let mut lower = d_b;
        while lower < PI * radius_m {
            for along in [0.0, 0.11, 0.37, 0.5, 0.73, 0.999] {
                let diameter = lower * math::powf(2.0, along);
                let rim_m = 0.5 * diameter;
                let shape = crater_shape(header.craters(), Metres::new(diameter));
                let fresh = CraterProfile::filleted(shape, Metres::ZERO);
                for n in 0..crate::synth::BAND_LEVELS {
                    let level = crate::synth::BandLevel::new(u8::try_from(n).unwrap()).unwrap();
                    let band = super::super::band_half_width(&figure, level)
                        .value()
                        .min(coarse_band);
                    let fillet = CraterProfile::filleted(shape, Metres::new(band));
                    let Some((inner, outer)) = changed_radii(&fresh, &fillet, rim_m) else {
                        continue;
                    };
                    // The fillet's half-width in metres is at most min(band, R), and the change
                    // lies within it of the rim, or within its outer end of the centre where it
                    // reaches the cone; the area the count takes holds either.
                    let half = band.min(rim_m);
                    let slack = 1e-9 * rim_m;
                    let counted = filleted_area_m2(lower, band, 1.0);
                    assert!(
                        outer <= rim_m + half + slack,
                        "{kind:?} D {diameter} m, level {n}"
                    );
                    let disc = PI * (rim_m + half) * (rim_m + half);
                    let annulus = 4.0 * PI * rim_m * half;
                    if inner >= rim_m - half - slack {
                        assert!(
                            annulus <= counted * (1.0 + 1e-12),
                            "{kind:?} D {diameter} m, level {n}: {annulus} m² over {counted} m²"
                        );
                    } else {
                        // Only the faded cone reaches inside the span.
                        assert!(
                            inner < WIDEST_PEAK_FRACTION * rim_m,
                            "{kind:?} D {diameter} m, level {n}: changed from {inner} m"
                        );
                        assert!(
                            disc <= counted * (1.0 + 1e-12),
                            "{kind:?} D {diameter} m, level {n}: {disc} m² over {counted} m²"
                        );
                    }
                }
            }
            lower *= 2.0;
        }
    }
}

#[test]
fn the_rims_count_area_shrinks_with_the_band_and_never_passes_the_reach() {
    for lower in [5e4, 9.26e4, 3.7e5, 2.9e6] {
        let reach = (1.0 + MAX_EXTERIOR_WIDTH) * lower;
        let mut previous = 0.0;
        for band in [
            0.0,
            1.0,
            1e3,
            0.1 * lower,
            0.25 * lower,
            0.5 * lower,
            lower,
            4.0 * lower,
        ] {
            let area = filleted_area_m2(lower, band, 1.0);
            assert!(area >= previous, "{lower} m at {band} m");
            assert!(area <= PI * reach * reach);
            // A flattened figure's allowance only adds ground, and its reach cap takes it too.
            let flattened = filleted_area_m2(lower, band, 1.5);
            assert!(flattened >= area && flattened <= 1.5 * PI * reach * reach);
            previous = area;
        }
        assert_same_bits(filleted_area_m2(lower, 0.0, 1.5), 0.0);
        // A fillet a tenth of the least rim radius wide reaches no cone: on a sphere the annulus,
        // 4π R w, and on a figure of allowance k, π (k (R + w)² − (R − w)²).
        let band = 0.05 * lower;
        assert!(
            (filleted_area_m2(lower, band, 1.0) / (4.0 * PI * lower * band) - 1.0).abs() < 1e-12
        );
        let (outer, inner) = (lower + band, lower - band);
        let annulus = PI * (1.5 * outer * outer - inner * inner);
        assert!((filleted_area_m2(lower, band, 1.5) / annulus - 1.0).abs() < 1e-12);
    }
}
