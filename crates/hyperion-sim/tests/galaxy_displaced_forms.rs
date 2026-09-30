//! The displaced classes' forms: flared layers and cored power laws, the ballistic and
//! blurred-arm forms, and the own-form mixtures (plan 08, P08.T10), against a test-only form table
//! until plan 15's exists (`displaced_support`).

mod displaced_support;

use displaced_support::{
    AGE_MIDS, OLD_SOURCES, disc_born, old_born, own_shares, scales_and_fields, seed_params,
};
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::displaced::forms::{
    BallisticLayer, CoredPowerLaw, CoredPowerLawParams, DiscBornForm, FlaredLayer,
    FlaredLayerParams, OwnFormMixture, cube_integral, keeps_arm, young_disc,
};
use hyperion_sim::galaxy::displaced::{AGE_BINS, AgeBin, SPEED_BINS};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::math;
use hyperion_testkit::lcg::Lcg;

/// The cube's half-width, ly.
const L: f64 = 65_536.0;

fn age(i: usize) -> AgeBin {
    AgeBin::new(u8::try_from(i).unwrap()).unwrap()
}

/// P08.T10.a: the layer's column density is `exp(−R ÷ h_R) × 2Γ(1 + 1 ÷ β)` to 10⁻⁶, across the
/// table's rows and radii from the centre to the cube's edge.
#[test]
fn the_layers_column_is_its_closed_form() {
    let (scales, _) = scales_and_fields(&GalaxyParams::milky_way_like());
    for speed in 0..SPEED_BINS {
        for a in 0..AGE_BINS {
            let (row, _, _) = disc_born(speed, a);
            let layer = FlaredLayer::new(&row, &scales).unwrap();
            for r in [0.0, 3_000.0, 26_000.0, 60_000.0] {
                let closed = layer.radial(r) * 2.0 * math::gamma(1.0 + 1.0 / row.beta);
                let column = layer.column(r);
                assert!(
                    (column / closed - 1.0).abs() < 1e-6,
                    "bin ({speed}, {a}) at {r} ly: {column} against {closed}"
                );
            }
        }
    }
}

/// The integral over the cube of `f(|x|, |y|, |z|)` by importance sampling: each coordinate is
/// `L u³` with u uniform, whose density `t^(−2/3) ÷ (3 L^(1/3))` follows a cored form's rise to its
/// core, and eight octants.
fn monte_carlo(points: u32, lcg: &mut Lcg, shape: impl Fn(f64, f64, f64) -> f64) -> f64 {
    let mut sum = 0.0;
    let pick = |lcg: &mut Lcg| {
        let u = lcg.next_f64().max(1e-300);
        let coordinate = L * u * u * u;
        let weight = 3.0 * math::cbrt(L) * math::cbrt(coordinate) * math::cbrt(coordinate);
        (coordinate, weight)
    };
    for _ in 0..points {
        let (x, wx) = pick(lcg);
        let (y, wy) = pick(lcg);
        let (z, wz) = pick(lcg);
        sum += shape(x, y, z) * wx * wy * wz;
    }
    8.0 * sum / f64::from(points)
}

/// P08.T10.a: the numerical normalisation of a spheroid agrees with a 10⁷-point Monte Carlo to
/// 0.5%, for a round, a flattened and a steep one.
#[test]
fn a_spheroids_normalisation_agrees_with_a_monte_carlo() {
    let (scales, _) = scales_and_fields(&GalaxyParams::milky_way_like());
    let mut lcg = Lcg::new(0x0810_a000);
    for row in [
        CoredPowerLawParams {
            weight: 1.0,
            a: 1.0,
            q: 1.0,
            gamma: 3.5,
        },
        CoredPowerLawParams {
            weight: 1.0,
            a: 0.4,
            q: 0.3,
            gamma: 5.0,
        },
        CoredPowerLawParams {
            weight: 1.0,
            a: 2.0,
            q: 0.6,
            gamma: 2.5,
        },
    ] {
        let spheroid = CoredPowerLaw::new(&row, &scales).unwrap();
        let sampled = monte_carlo(10_000_000, &mut lcg, |x, y, z| {
            spheroid.shape(math::hypot(x, y), z)
        });
        let integral = 1.0 / spheroid.norm();
        assert!(
            (sampled / integral - 1.0).abs() < 0.005,
            "{row:?}: {sampled} against {integral}"
        );
    }
}

/// Builds every form of the test-only table in the galaxy of `params`, checking each density and
/// normalisation is finite and positive, and returns how many it built.
fn build_every_form(params: &GalaxyParams) -> usize {
    let (scales, fields) = scales_and_fields(params);
    let (young, arm) = young_disc(&fields);
    let mut built = 0;
    for speed in 0..SPEED_BINS {
        for a in 0..AGE_BINS {
            let (layer, spheroid, mean_ut) = disc_born(speed, a);
            let form =
                DiscBornForm::new(&layer, &spheroid, age(a), mean_ut, (young, &arm), &scales)
                    .unwrap_or_else(|e| panic!("bin ({speed}, {a}): {e}"));
            let d = form.density(&PointLy::new(20_000.0, 5_000.0, 300.0));
            assert!(d.is_finite() && d > 0.0, "bin ({speed}, {a}): {d}");
            built += 1;
        }
        for source in 0..OLD_SOURCES {
            let spheroid = CoredPowerLaw::new(&old_born(source, speed), &scales).unwrap();
            assert!(spheroid.norm().is_finite() && spheroid.norm() > 0.0);
            built += 1;
        }
    }
    built
}

/// P08.T10.a: all 56 + 40 forms normalise for 200 seeds.
#[test]
#[ignore = "slow: 200 galaxies' parameters, mass models and fields, and 96 cube integrals each"]
fn every_form_normalises_for_200_seeds() {
    for i in 0..200 {
        assert_eq!(build_every_form(&seed_params(i)), 96, "seed {i}");
    }
}

/// P08.T10.a at the Milky Way fixture alone, for the fast suite.
#[test]
fn every_form_normalises_at_milky_way_values() {
    assert_eq!(build_every_form(&GalaxyParams::milky_way_like()), 96);
}

/// P08.T10.b: as `⟨uτ⟩` → 0 the ballistic form tends to the young disc's density, normalised over
/// the cube, to 10⁻⁶, arm included.
#[test]
fn the_ballistic_form_tends_to_the_young_disc() {
    let (scales, fields) = scales_and_fields(&GalaxyParams::milky_way_like());
    let (young, arm) = young_disc(&fields);
    let young_norm = 1.0 / cube_integral(|r, z| young.envelope(r, z));
    for mean_ut in [0.0, 1e-9] {
        let layer = BallisticLayer::new(young, &arm, age(0), mean_ut, &scales).unwrap();
        assert!(layer.arm().is_some());
        for p in [
            PointLy::new(26_000.0, 0.0, 0.0),
            PointLy::new(-8_000.0, 12_000.0, 150.0),
            PointLy::new(3_000.0, -40_000.0, -600.0),
        ] {
            let expected = young_norm * young.density(&p);
            let got = layer.density(&p);
            assert!(
                (got / expected - 1.0).abs() < 1e-6,
                "⟨uτ⟩ {mean_ut} at {p:?}: {got} against {expected}"
            );
        }
    }
    // A ballistic layer widens with ⟨uτ⟩ and keeps its column.
    let wide = BallisticLayer::new(young, &arm, age(1), 0.2, &scales).unwrap();
    let h = young.height().value();
    let expected = math::hypot(h, 1.1 * 0.2 * scales.r_d().value());
    assert!((wide.height() / expected - 1.0).abs() < 1e-12);
}

/// P08.T10.b: the arm factor is present exactly when τ < 1 and `⟨uτ⟩ ≤ 0.4`.
#[test]
fn the_arm_is_kept_exactly_for_young_slow_classes() {
    let (scales, fields) = scales_and_fields(&GalaxyParams::milky_way_like());
    let (young, arm) = young_disc(&fields);
    for speed in 0..SPEED_BINS {
        for (a, &mid) in AGE_MIDS.iter().enumerate() {
            let (layer, spheroid, mean_ut) = disc_born(speed, a);
            let form =
                DiscBornForm::new(&layer, &spheroid, age(a), mean_ut, (young, &arm), &scales)
                    .unwrap();
            let young_bin = mid < 1.0;
            let expected = young_bin && mean_ut <= 0.4;
            assert_eq!(
                form.arm().is_some(),
                expected,
                "bin ({speed}, {a}), ⟨uτ⟩ {mean_ut}"
            );
            assert_eq!(keeps_arm(age(a), mean_ut), expected);
            if let Some(blurred) = form.arm() {
                let width = math::hypot(arm.width().value(), 0.8 * mean_ut * scales.r_d().value());
                assert!((blurred.width().value() / width - 1.0).abs() < 1e-12);
            }
        }
    }
}

/// `∫ R dR ∫ dφ` of `column(R) × arm(R, φ)` over the disc of radius L, and without the arm: the
/// radial rule on 24 doubling panels of 8 Gauss–Legendre nodes, the azimuthal one a trapezoid of
/// 4,096 points, exact for a smooth periodic factor.
fn in_plane_with_and_without_arm(
    column: impl Fn(f64) -> f64,
    arm: &hyperion_sim::galaxy::fields::arms::SharpArm,
) -> (f64, f64) {
    let edges: Vec<f64> = core::iter::once(0.0)
        .chain((0..24).rev().map(|k| L * math::powi(0.5, k)))
        .collect();
    let nodes = hyperion_sim::tables::gauss_legendre::GL8_NODES;
    let weights = hyperion_sim::tables::gauss_legendre::GL8_WEIGHTS;
    let (mut with, mut without) = (0.0, 0.0);
    for w in edges.windows(2) {
        let (half, mid) = (0.5 * (w[1] - w[0]), f64::midpoint(w[0], w[1]));
        for (&x, &weight) in nodes.iter().zip(&weights) {
            let r = mid + half * x;
            let c = column(r) * r * weight * half;
            let mut ring = 0.0;
            for j in 0..4_096 {
                let phi = 2.0 * core::f64::consts::PI * f64::from(j) / 4_096.0;
                let (s, co) = math::sin_cos(phi);
                ring += arm.factor(&arm.geometry().point(r * co, r * s));
            }
            with += c * ring / 4_096.0;
            without += c;
        }
    }
    (with, without)
}

/// P08.T10.b: the arm factor leaves the normalisation unchanged to 10⁻⁴, for a ballistic class
/// and a fitted one.
#[test]
fn the_arm_does_not_move_the_normalisation() {
    let (scales, fields) = scales_and_fields(&GalaxyParams::milky_way_like());
    let (young, arm) = young_disc(&fields);
    let ballistic = BallisticLayer::new(young, &arm, age(1), 0.1, &scales).unwrap();
    let blurred = ballistic.arm().unwrap();
    let (with, without) = in_plane_with_and_without_arm(|r| ballistic.envelope(r, 0.0), blurred);
    assert!(
        (with / without - 1.0).abs() < 1e-4,
        "ballistic: {with} against {without}"
    );
    let (row, _, _) = disc_born(1, 2);
    let layer = FlaredLayer::new(&row, &scales).unwrap();
    let fitted_arm =
        hyperion_sim::galaxy::displaced::forms::blurred_arm(&arm, 0.3, &scales).unwrap();
    let (with, without) = in_plane_with_and_without_arm(|r| layer.column(r), &fitted_arm);
    assert!(
        (with / without - 1.0).abs() < 1e-4,
        "fitted: {with} against {without}"
    );
}

/// P08.T10.c: own share and spheroid share reproduce the brainstorm's figures at a corotation
/// ratio of 1.2 (bar 0.95, 0.61, 0.20, 0.05; bulge 0.91, 0.76, 0.55, 0.25; nuclear disc 0.88,
/// 0.67, 0.43) and interpolate monotonically between the nodes.
#[test]
fn own_form_mixtures_reproduce_the_brainstorm_and_interpolate_monotonically() {
    let brainstorm: [(usize, &[f64]); 3] = [
        (3, &[0.95, 0.61, 0.20, 0.05]),
        (2, &[0.91, 0.76, 0.55, 0.25]),
        (4, &[0.88, 0.67, 0.43]),
    ];
    for (source, figures) in brainstorm {
        let table = own_shares(source);
        let mut last = f64::INFINITY;
        for (speed, nodes) in table.iter().enumerate() {
            let mixture = OwnFormMixture::new(*nodes).unwrap();
            let own = mixture.own_share(1.2);
            if let Some(&want) = figures.get(speed) {
                assert!(
                    (own - want).abs() < 1e-12,
                    "source {source} bin {speed}: {own}"
                );
            }
            assert!((own + mixture.spheroid_share(1.2) - 1.0).abs() < 1e-15);
            assert!(own <= last, "shares fall with speed");
            last = own;
            let samples: Vec<f64> = (0..=40).map(|k| 1.0 + 0.01 * f64::from(k)).collect();
            let values: Vec<f64> = samples.iter().map(|&r| mixture.own_share(r)).collect();
            let rising = values.windows(2).all(|w| w[1] >= w[0]);
            let falling = values.windows(2).all(|w| w[1] <= w[0]);
            assert!(rising || falling, "source {source} bin {speed}: {values:?}");
            for (k, node) in [1.0, 1.2, 1.4].iter().enumerate() {
                assert!((mixture.own_share(*node) - nodes[k]).abs() < 1e-15);
            }
            assert!((mixture.own_share(0.8) - nodes[0]).abs() < 1e-15);
            assert!((mixture.own_share(1.6) - nodes[2]).abs() < 1e-15);
            let d = mixture.density(1.2, 2.0, 1.0);
            assert!((d - (own * 2.0 + (1.0 - own))).abs() < 1e-15);
        }
    }
}

/// A layer's form is exactly the flared layer of its row: weights, heights and flare as given.
#[test]
fn a_flared_layer_has_its_rows_shape() {
    let (scales, _) = scales_and_fields(&GalaxyParams::milky_way_like());
    let r_d = scales.r_d().value();
    let row = FlaredLayerParams {
        weight: 1.0,
        h_r: 1.5,
        h_0: 0.05,
        r_flare: 4.0,
        beta: 1.2,
    };
    let layer = FlaredLayer::new(&row, &scales).unwrap();
    let r = 20_000.0;
    let h = 0.05 * r_d * math::exp(r / (4.0 * r_d));
    assert!((layer.scale_height(r) / h - 1.0).abs() < 1e-12);
    let z = 700.0;
    let shape = math::exp(-r / (1.5 * r_d)) * math::exp(-math::powf(z / h, 1.2)) / h;
    assert!((layer.shape(r, z) / shape - 1.0).abs() < 1e-12);
    assert!((layer.density(&PointLy::new(r, 0.0, z)) / (layer.norm() * shape) - 1.0).abs() < 1e-12);
    assert!(FlaredLayer::new(&FlaredLayerParams { beta: 0.0, ..row }, &scales).is_err());
}
