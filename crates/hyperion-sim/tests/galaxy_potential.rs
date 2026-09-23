//! The potential (plan 02, P02.T6): the fitted expansions, the Gaussian components against closed
//! forms, the mass model's tables against direct evaluation, the black hole and the golden file.
//!
//! The checks over many seeds run here on 32 seeds, and over 10³ in `galaxy_sweeps.rs` under
//! `just test-slow` (plan 02, P02.T11). The (R, z) grid costs seconds to build, so its test is
//! slow too.

#[expect(
    dead_code,
    reason = "the potential tests use only the shared assertions"
)]
mod common;

use common::{assert_relative, assert_within};
use hyperion_sim::galaxy::PointLy;
use hyperion_sim::galaxy::consts::{G, LIGHT_YEARS_PER_KILOPARSEC, LIGHT_YEARS_PER_YEAR_PER_KM_S};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::{GalaxyParams, GalaxyParamsBuilder};
use hyperion_sim::galaxy::potential::mge::{
    Gaussian, bar_disc, double_exponential, spheroidal_exponential,
};
use hyperion_sim::galaxy::potential::sigma::{black_hole_mass, bulge_dispersion};
use hyperion_sim::galaxy::potential::spherical::SphericalMass;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::math;
use hyperion_sim::tables::mge::{MGE_BAR, MGE_EXP};
use hyperion_sim::units::{Dex, KilometresPerSecond, LightYears, Metres, SolarMasses};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

const PI: f64 = core::f64::consts::PI;

fn ly(x: f64) -> LightYears {
    LightYears::new(x)
}

fn seeds() -> impl Iterator<Item = Seed> {
    (0..32_u64).map(|n| Seed::new(0x0206_5eed_0000_0000 | n))
}

/// The three pinned seeds of the golden files.
const PINNED: [u64; 3] = [
    0x0000_0000_0000_0001,
    0x5eed_0000_c0ff_ee00,
    0xdead_beef_cafe_f00d,
];

fn expansion(table: &[(f64, f64)], x: f64) -> f64 {
    table.iter().fold(0.0, |sum, &(w, s)| {
        sum + w * math::exp(-x * x / (2.0 * s * s))
    })
}

// P02.T6.a: the tables, checked independently of the tool that fitted them.

#[test]
fn the_expansions_have_non_negative_weights_and_ascending_widths() {
    for table in [&MGE_EXP, &MGE_BAR] {
        assert!(table.iter().all(|&(w, s)| w >= 0.0 && s > 0.0));
        assert!(table.windows(2).all(|pair| pair[0].1 < pair[1].1));
    }
}

/// `MGE_EXP` reproduces `e^(−s)` to 1% on 0.05–8, and the mass of the spherical exponential,
/// `Σ w (2π)^(3÷2) σ³` against `8π`, to 0.1%.
#[test]
fn mge_exp_reproduces_the_exponential() {
    for i in 0..=4_000 {
        let s = 0.05 * math::exp(f64::from(i) / 4_000.0 * math::ln(160.0));
        let error = expansion(&MGE_EXP, s) / math::exp(-s) - 1.0;
        assert!(error.abs() < 0.01, "at s = {s}: {error:e}");
    }
    let two_pi = 2.0 * PI;
    let mass = MGE_EXP.iter().fold(0.0, |sum, &(w, s)| {
        sum + w * two_pi * two_pi.sqrt() * s * s * s
    });
    assert_relative("spherical exponential mass", mass, 8.0 * PI, 1e-3);
}

/// The bar's azimuthally averaged surface density, by brute force: `(2 ÷ π) ∫₀^(π÷2) L(u cos θ)
/// exp(−u² sin²θ ÷ 2w²) dθ` by Simpson's rule with 4,000 intervals, `w = 0.1`, `L` level to 0.85
/// with a Gaussian end of 0.15 (P02.T6.a).
fn bar_profile(u: f64) -> f64 {
    let length = |x: f64| {
        if x <= 0.85 {
            1.0
        } else {
            math::exp(-0.5 * ((x - 0.85) / 0.15) * ((x - 0.85) / 0.15))
        }
    };
    let f = |theta: f64| {
        let (sin, cos) = math::sin_cos(theta);
        length(u * cos) * math::exp(-0.5 * (u * sin / 0.1) * (u * sin / 0.1))
    };
    let n = 4_000;
    let h = 0.5 * PI / f64::from(n);
    let mut sum = f(0.0) + f(0.5 * PI);
    for i in 1..n {
        sum += f(h * f64::from(i)) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    sum * h / 3.0 * 2.0 / PI
}

/// `MGE_BAR` reproduces the bar's profile to 3% of its central value everywhere, and its mass to
/// 0.5%; the mass inside each radius, which is what the rotation curve reads, stays within 7%.
///
/// A sum of centred Gaussians with non-negative weights is log-convex in `u²`, and the bar's
/// Gaussian end is log-concave, so no such sum follows the end in relative terms: by linear
/// programming over 14 or 16 log-spaced widths, the best achievable maximum relative error is 12%
/// out to one half-length and 30% out to 1.1, and holding the level part to even 10% forces the
/// total mass off by more than 0.5%. So the 3% is read against the central value (plan 02,
/// Risks, R14). The mass inside a radius is off by up to 6.6% just beyond the half-length; the
/// best any such fit could do there with the other two checks held is 4.4%.
#[test]
fn mge_bar_reproduces_the_bar_profile() {
    let profile: Vec<f64> = (0..=250)
        .map(|i| bar_profile(0.01 * f64::from(i)))
        .collect();
    for (i, &p) in (0..=250).zip(&profile) {
        let u = 0.01 * f64::from(i);
        let error = expansion(&MGE_BAR, u) - p;
        assert!(error.abs() < 0.03, "at u = {u}: {error}");
    }
    let mass = MGE_BAR
        .iter()
        .fold(0.0, |sum, &(w, s)| sum + w * 2.0 * PI * s * s);
    let exact = (2.0 * PI).sqrt() * 0.1 * 2.0 * (0.85 + 0.15 * (0.5 * PI).sqrt());
    assert_relative("bar mass", mass, exact, 5e-3);
    // The mass inside u, 2π ∫ Σ u du: Simpson's rule on the samples above for the profile, in
    // closed form for the expansion.
    let mut inside = 0.0;
    for pair in 0..125_u32 {
        let (i, u) = (2 * pair, 0.01 * f64::from(2 * pair));
        let at = |k: u32| {
            let index = usize::try_from(i + k).unwrap();
            (u + 0.01 * f64::from(k)) * profile[index]
        };
        inside += 2.0 * PI * 0.01 / 3.0 * (at(0) + 4.0 * at(1) + at(2));
        let edge = u + 0.02;
        let expanded = MGE_BAR.iter().fold(0.0, |sum, &(w, s)| {
            sum + w * 2.0 * PI * s * s * (1.0 - math::exp(-edge * edge / (2.0 * s * s)))
        });
        if edge >= 0.05 {
            assert_relative(&format!("mass inside {edge}"), expanded, inside, 0.07);
        }
    }
    assert_relative("mass inside 2.5", inside, exact, 1e-3);
}

// P02.T6.b: Gaussian components.

/// A spherical Gaussian against `M(<r) = M [erf(x ÷ √2) − √(2 ÷ π) x e^(−x²÷2)]`, `v_c² = G M(<r)
/// ÷ r` and `Φ = −G M erf(x ÷ √2) ÷ r`.
#[test]
fn a_spherical_gaussian_matches_its_closed_forms() {
    let (m, sigma) = (3e9, 700.0);
    let g = Gaussian::new(SolarMasses::new(m), ly(sigma), 1.0).unwrap();
    for x in [0.01, 0.3, 1.0, 2.5, 6.0, 20.0] {
        let r = x * sigma;
        let enclosed =
            m * (math::erf(x / 2.0_f64.sqrt()) - (2.0 / PI).sqrt() * x * math::exp(-0.5 * x * x));
        assert_relative("M(<r)", g.enclosed_mass(ly(r)).value(), enclosed, 1e-10);
        assert_relative("v_c²", g.v_circ_sq(ly(r)), G * enclosed / r, 1e-10);
        let phi = -G * m * math::erf(x / 2.0_f64.sqrt()) / r;
        assert_relative("Φ", g.potential(ly(r), ly(0.0)), phi, 1e-10);
        // Off the plane too, at the same spherical radius.
        let (rc, z) = (0.6 * r, 0.8 * r);
        assert_relative("Φ off the plane", g.potential(ly(rc), ly(z)), phi, 1e-10);
    }
}

/// A spherical exponential built from `MGE_EXP` against `M(<r) = M [1 − e^(−x)(1 + x + x² ÷ 2)]`,
/// through `v_c² = G M(<r) ÷ r`, to 1% on 0.1–8 scale lengths.
#[test]
fn a_spherical_exponential_from_the_expansion_matches_its_enclosed_mass() {
    let (m, a) = (1e10, 2_000.0);
    let gaussians = spheroidal_exponential(SolarMasses::new(m), ly(a), ly(a)).unwrap();
    let total = gaussians.iter().fold(0.0, |sum, g| sum + g.mass().value());
    assert_relative("mass", total, m, 1e-12);
    for i in 0..=40 {
        let x = 0.1 * math::exp(f64::from(i) / 40.0 * math::ln(80.0));
        let r = x * a;
        let v2 = gaussians
            .iter()
            .fold(0.0, |sum, g| sum + g.v_circ_sq(ly(r)));
        let enclosed = m * (1.0 - math::exp(-x) * (1.0 + x + 0.5 * x * x));
        assert_relative(&format!("v_c² at {x} scales"), v2, G * enclosed / r, 0.01);
    }
}

fn thin_disc(height_ratio: f64) -> (Vec<Gaussian>, f64, f64) {
    let (m, length) = (5e10, 10_000.0);
    let gaussians =
        double_exponential(SolarMasses::new(m), ly(length), ly(height_ratio * length)).unwrap();
    (gaussians, m, length)
}

fn sum_v2(gaussians: &[Gaussian], r: f64) -> f64 {
    gaussians
        .iter()
        .fold(0.0, |sum, g| sum + g.v_circ_sq(ly(r)))
}

/// A thin exponential disc's rotation curve peaks at 2.1–2.3 scale lengths: 2.15 for a razor-thin
/// one (Freeman 1970, ApJ 160, 811).
#[test]
fn a_thin_double_exponential_peaks_near_two_point_two_scale_lengths() {
    let (gaussians, _, length) = thin_disc(0.01);
    let mut best = (0.0, 0.0);
    for i in 0..=300 {
        let x = 1.5 + 0.005 * f64::from(i);
        let v2 = sum_v2(&gaussians, x * length);
        if v2 > best.1 {
            best = (x, v2);
        }
    }
    assert_within("peak in scale lengths", best.0, 2.1, 2.3);
}

/// Just above a thin disc, `K_z` is the field of an infinite sheet of its local surface density,
/// `2πGΣ(R)` with `Σ(R) = M e^(−R ÷ L) ÷ 2πL²`, inside a few scale lengths.
#[test]
fn the_vertical_force_above_a_thin_disc_is_two_pi_g_sigma() {
    let (gaussians, m, length) = thin_disc(0.002);
    for x in [0.5, 1.0, 2.0, 3.0] {
        let r = x * length;
        // Ten scale heights up: all but e^(−10) of the column below, 2% of a scale length.
        let z = 0.02 * length;
        let kz = gaussians
            .iter()
            .fold(0.0, |sum, g| sum + g.vertical_force(ly(r), ly(z)));
        let sheet = 2.0 * PI * G * m * math::exp(-x) / (2.0 * PI * length * length);
        assert_relative(&format!("K_z at {x} scale lengths"), kz, sheet, 0.03);
    }
}

/// The radial derivative of `v_c²` and the vertical force against finite differences, and Φ →
/// −GM ÷ r far away, for every expansion.
#[test]
fn forces_are_the_derivatives_of_the_potential() {
    let m = SolarMasses::new(1e10);
    let expansions = [
        double_exponential(m, ly(9_000.0), ly(1_000.0)).unwrap(),
        double_exponential(m, ly(290.0), ly(93.0)).unwrap(),
        spheroidal_exponential(m, ly(1_800.0), ly(800.0)).unwrap(),
        bar_disc(m, ly(16_000.0), ly(590.0)).unwrap(),
    ];
    for gaussians in &expansions {
        let total = gaussians.iter().fold(0.0, |sum, g| sum + g.mass().value());
        assert_relative("mass", total, 1e10, 1e-12);
        for r in [50.0, 700.0, 6_000.0, 30_000.0] {
            let h = 1e-4 * r;
            let v2_slope = gaussians
                .iter()
                .fold(0.0, |sum, g| sum + g.v_circ_sq_derivative(ly(r)));
            let numeric = (sum_v2(gaussians, r + h) - sum_v2(gaussians, r - h)) / (2.0 * h);
            assert!(
                (v2_slope - numeric).abs() < 1e-6 * sum_v2(gaussians, r) / r,
                "dv²/dR at {r}: {v2_slope} against {numeric}"
            );
            // v_c² = R ∂Φ ÷ ∂R in the plane.
            let phi = |rr: f64, z: f64| {
                gaussians
                    .iter()
                    .fold(0.0, |sum, g| sum + g.potential(ly(rr), ly(z)))
            };
            let numeric = r * (phi(r + h, 0.0) - phi(r - h, 0.0)) / (2.0 * h);
            assert_relative("v_c² = R ∂Φ ÷ ∂R", sum_v2(gaussians, r), numeric, 1e-6);
            for z in [30.0, 400.0, 5_000.0] {
                let dz = 1e-4 * z;
                let kz = gaussians
                    .iter()
                    .fold(0.0, |sum, g| sum + g.vertical_force(ly(r), ly(z)));
                let numeric = (phi(r, z + dz) - phi(r, z - dz)) / (2.0 * dz);
                assert_relative(&format!("K_z at ({r}, {z})"), kz, numeric, 1e-5);
            }
        }
        for (r, z) in [(3e7, 0.0), (0.0, 3e7), (2e7, 2e7)] {
            let phi = gaussians
                .iter()
                .fold(0.0, |sum, g| sum + g.potential(ly(r), ly(z)));
            let kepler = -G * 1e10 / math::hypot(r, z);
            assert_relative("far field", phi, kepler, 1e-6);
        }
    }
}

// P02.T6.c and P02.T6.d: the mass model and its tables.

fn models() -> Vec<(String, GalaxyParams)> {
    let mut all = vec![("milky_way".to_owned(), GalaxyParams::milky_way_like())];
    all.extend(PINNED.map(|s| {
        let seed = Seed::new(s);
        (
            seed.to_string(),
            GalaxyParams::from_seed(seed, MassFunctionKind::default()),
        )
    }));
    all
}

/// Radii log-spaced from `lo` to `hi`, placed off the tables' grid.
fn radii(n: u32, lo: f64, hi: f64) -> impl Iterator<Item = f64> {
    (0..n).map(move |i| {
        let t = (f64::from(i) + 0.37) / f64::from(n);
        lo * math::exp(t * math::ln(hi / lo))
    })
}

/// `dv_c² ÷ dR` summed directly over every component.
fn direct_derivative(model: &MassModel, r: f64) -> f64 {
    let extended = model
        .gaussians()
        .iter()
        .fold(0.0, |sum, g| sum + g.v_circ_sq_derivative(ly(r)));
    extended
        + model.dark_halo().v_circ_sq_derivative(ly(r))
        + model.nuclear_cluster().v_circ_sq_derivative(ly(r))
        + model.black_hole().v_circ_sq_derivative(ly(r))
}

/// The tables against direct evaluation at 500 off-grid radii from 0.01 ly to 2¹⁸ ly: `v_c`, Ω,
/// κ and Φ to 10⁻³.
#[test]
fn the_tables_match_direct_evaluation() {
    let (_, params) = &models()[0];
    let model = MassModel::new(params);
    let tables = PotentialTables::in_plane(&model);
    for r in radii(500, 0.01, 262_144.0) {
        let what = format!("at {r} ly");
        let v2 = model.v_circ_sq(ly(r));
        assert_relative(&what, tables.v_circ(ly(r)).value(), v2.sqrt(), 1e-3);
        let omega = v2.sqrt() / r * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        assert_relative(&what, tables.omega(ly(r)).value(), omega, 1e-3);
        let kappa_sq = direct_derivative(&model, r) / r + 2.0 * v2 / (r * r);
        let kappa = kappa_sq.sqrt() * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        assert_relative(&what, tables.kappa(ly(r)).value(), kappa, 1e-3);
        let phi = model.potential(ly(r), ly(0.0));
        assert_relative(&what, tables.potential_in_plane(ly(r)), phi, 1e-3);
    }
}

/// κ lies between Ω and 2Ω everywhere outside 1 ly (the rotation curve falls no faster than
/// Keplerian and rises no faster than solid-body), and tends to Ω near the black hole.
#[test]
fn kappa_lies_between_omega_and_twice_omega() {
    for (name, params) in models() {
        let tables = PotentialTables::in_plane(&MassModel::new(&params));
        for r in radii(400, 1.0, 262_144.0) {
            let ratio = tables.kappa(ly(r)).value() / tables.omega(ly(r)).value();
            assert_within(&format!("{name}: κ ÷ Ω at {r} ly"), ratio, 1.0, 2.0);
        }
        let near = tables.kappa(ly(0.01)).value() / tables.omega(ly(0.01)).value();
        assert_relative(&format!("{name}: κ ÷ Ω at 0.01 ly"), near, 1.0, 1e-3);
    }
}

/// Near the black hole the tidal radius is the point-mass one, `R (m ÷ 3M)^⅓`, and at 26,000 ly
/// it is a few light-years for a solar mass (brainstorm: about 4).
#[test]
fn the_tidal_radius_near_the_black_hole_is_the_point_mass_one() {
    let params = GalaxyParams::milky_way_like();
    let tables = PotentialTables::in_plane(&MassModel::new(&params));
    let bh = params.black_hole().mass().value();
    for r in [0.003, 0.01] {
        let p = PointLy::new(0.6 * r, -0.8 * r, 0.0);
        let rt = LightYears::from(tables.tidal_radius(SolarMasses::new(1.0), &p)).value();
        assert_relative(
            &format!("at {r} ly"),
            rt,
            r * math::cbrt(1.0 / (3.0 * bh)),
            1e-3,
        );
    }
    let sun = PointLy::new(26_000.0, 0.0, 0.0);
    let rt = LightYears::from(tables.tidal_radius(SolarMasses::new(1.0), &sun)).value();
    assert_within("tidal radius at 26,000 ly", rt, 3.0, 6.0);
    assert_eq!(
        tables.tidal_radius(SolarMasses::new(1.0), &PointLy::default()),
        Metres::ZERO
    );
}

/// The dark halo in the model is the parameters' NFW halo: M₂₀₀ inside r₂₀₀.
#[test]
fn the_model_holds_the_parameters_masses() {
    for (name, params) in models() {
        let model = MassModel::new(&params);
        let halo = params.dark_halo();
        assert_relative(
            &name,
            model.dark_halo().enclosed_mass(halo.r200()).value(),
            halo.m200().value(),
            1e-12,
        );
        let expanded = model
            .gaussians()
            .iter()
            .fold(0.0, |sum, g| sum + g.mass().value());
        let stars: f64 = hyperion_sim::galaxy::POPULATIONS
            .iter()
            .filter(|&&p| p != hyperion_sim::galaxy::Population::Halo)
            .map(|&p| params.population_mass(p).value())
            .sum();
        assert_relative(
            &name,
            expanded,
            stars + params.gas_disc().mass().value(),
            1e-12,
        );
        assert_relative(
            &name,
            model.enclosed_mass(ly(1e12)).value(),
            model.dark_halo().enclosed_mass(ly(1e12)).value()
                + params.nuclear_cluster().mass().value()
                + params.black_hole().mass().value(),
            1e-9,
        );
        let bar = model.bar_corotation().value() / params.bar().half_length().value();
        assert_relative(&name, bar, params.bar().corotation_ratio(), 1e-15);
    }
}

// P02.T6.e: the black hole.

/// The Milky Way fixture's bulge dispersion is 95–125 km/s, and its black hole lies within a
/// factor of 2.5 of Sgr A*'s 4.3 × 10⁶ M☉ (plan 02, P02.T6.e).
///
/// The fixture's scatter is the Milky Way's own offset from the relation, −0.512 dex, set so that
/// the estimator's 123.8 km/s gives 4.30 × 10⁶ M☉ (plan 02, Risks, R13 and R22). The factor of
/// 2.5 then lets σ move by 7% before the test fails, which is what it guards: the relation itself,
/// without the offset, gives 3.3 times the measured mass at this σ, a real galaxy 1.35 times the
/// relation's intrinsic scatter below it. The offset's check reaches −0.55 dex, not the −0.5 it
/// held at R13's −0.421: P02.T11's tuning raised σ from 119.3 km/s, and the offset follows σ,
/// since what it is fixed by is Sgr A*'s measured mass.
///
/// That is also why the black hole itself is held to Sgr A*'s (4.297 ± 0.012) × 10⁶ M☉ (GRAVITY
/// Collaboration 2022, A&A 657, L12) to 1%: the offset is a measured fact about the Milky Way only
/// through the mass it reproduces. Without this, σ could fall 4% and the black hole with it to
/// 3.4 × 10⁶ M☉ with no check but the goldens noticing, since the checks below stop a rise of 1%
/// but a fall only at 8%, and the slow enclosed mass at 1 pc at 5.4% (plan 02, Risks, R23). A σ
/// that moves by more than 0.2% now fails here until the offset is re-set.
#[test]
fn the_fixture_black_hole_follows_m_sigma() {
    let params = GalaxyParams::milky_way_like();
    let bh = params.black_hole();
    let sigma = bh.bulge_dispersion();
    assert_relative("black hole ÷ Sgr A*'s", bh.mass().value(), 4.297e6, 0.01);
    assert_within("σ", sigma.value(), 95.0, 125.0);
    assert_relative(
        "M–σ with the fixture's offset",
        bh.mass().value(),
        black_hole_mass(sigma, bh.scatter()).value(),
        1e-12,
    );
    assert_within("offset, dex", bh.scatter().value(), -0.55, -0.35);
    let ratio = bh.mass().value() / 4.3e6;
    assert_within("black hole ÷ 4.3 × 10⁶ M☉", ratio, 1.0 / 2.5, 2.5);
    let on_relation = black_hole_mass(sigma, Dex::new(0.0)).value() / 4.3e6;
    assert_within("the relation alone ÷ 4.3 × 10⁶ M☉", on_relation, 2.0, 3.5);
    assert_relative(
        "σ is the estimator's",
        bh.bulge_dispersion().value(),
        bulge_dispersion(&params).value(),
        0.0,
    );
}

/// The drawn scatter moves the mass by exactly its dex, and σ not at all.
#[test]
fn the_scatter_moves_the_black_hole_mass_only() {
    let plain = GalaxyParams::milky_way_like();
    let scattered = GalaxyParamsBuilder::new()
        .black_hole_scatter(Dex::new(0.38))
        .build()
        .unwrap();
    assert_relative(
        "σ",
        scattered.black_hole().bulge_dispersion().value(),
        plain.black_hole().bulge_dispersion().value(),
        0.0,
    );
    assert_relative(
        "mass",
        scattered.black_hole().mass().value() / plain.black_hole().mass().value(),
        math::exp10(0.38 - plain.black_hole().scatter().value()),
        1e-12,
    );
    let at_200 = black_hole_mass(KilometresPerSecond::new(200.0), Dex::new(0.0));
    assert_relative("10^8.32", at_200.value(), math::exp10(8.32), 1e-14);
}

/// Over 32 seeds σ stays within 80–170 km/s; the 10³-seed distribution is in `galaxy_sweeps.rs`.
#[test]
fn the_bulge_dispersion_over_32_seeds() {
    for seed in seeds() {
        let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let sigma = params.black_hole().bulge_dispersion().value();
        assert_within(&format!("σ for {seed}"), sigma, 80.0, 170.0);
        assert!(params.black_hole().mass().value() > 0.0);
    }
}

// Golden values.

fn write_potential(w: &mut GoldenWriter, label: &str, params: &GalaxyParams) {
    let model = MassModel::new(params);
    let tables = PotentialTables::in_plane(&model);
    let mut f = |name: &str, value: f64| w.f64(&format!("{label}.{name}"), value);
    f("bh.sigma", params.black_hole().bulge_dispersion().value());
    f("bh.mass", params.black_hole().mass().value());
    f(
        "gaussians",
        f64::from(u32::try_from(model.gaussians().len()).unwrap()),
    );
    for r in [
        0.1,
        3.0,
        100.0,
        1_000.0,
        3_261.563_777,
        8_000.0,
        26_000.0,
        60_000.0,
        200_000.0,
    ] {
        let x = ly(r);
        f(&format!("v_circ({r})"), tables.v_circ(x).value());
        f(&format!("omega({r})"), tables.omega(x).value());
        f(&format!("kappa({r})"), tables.kappa(x).value());
        f(&format!("phi({r})"), tables.potential_in_plane(x));
        f(
            &format!("escape({r})"),
            tables.escape_speed_in_plane(x).value(),
        );
        f(&format!("direct_v_circ_sq({r})"), model.v_circ_sq(x));
    }
    f("bar_corotation", tables.bar_corotation().value());
    f("bar_pattern_speed", tables.bar_pattern_speed().value());
    for p in [
        PointLy::new(1.0, 0.0, 0.0),
        PointLy::new(-18_000.0, 7_000.0, 300.0),
        PointLy::new(0.0, 26_000.0, -100.0),
    ] {
        let rt = tables.tidal_radius(SolarMasses::new(1.0), &p).value();
        f(&format!("tidal({}, {}, {})", p.x, p.y, p.z), rt);
    }
    for (r, z) in [(0.0, 500.0), (8_000.0, 1_000.0), (26_000.0, 3_000.0)] {
        f(
            &format!("direct_phi({r}, {z})"),
            model.potential(ly(r), ly(z)),
        );
        f(
            &format!("vertical_force({r}, {z})"),
            model.vertical_force(ly(r), ly(z)),
        );
    }
}

/// The potential of three pinned seeds and the fixture, bit for bit.
#[test]
fn galaxy_potential_is_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (name, params) in models() {
        w.line(&format!("# {name}"));
        write_potential(&mut w, &name, &params);
    }
    golden!("galaxy_potential", w.as_str());
}

// The (R, z) grid, slow to build.

/// The grid against direct evaluation at 300 off-grid points from 0.1 to 2¹⁸ ly in R and |z|:
/// Φ and the escape speed to 10⁻³; and the in-plane limit.
#[test]
#[ignore = "slow: builds the (R, z) grid, about 3 s in release"]
fn the_grid_matches_direct_evaluation() {
    let params = GalaxyParams::milky_way_like();
    let model = MassModel::new(&params);
    let tables = PotentialTables::full(&model);
    assert!(tables.has_grid());
    let rs: Vec<f64> = radii(20, 0.1, 262_144.0).collect();
    let zs: Vec<f64> = radii(15, 0.1, 262_144.0).collect();
    for &r in &rs {
        for &z in &zs {
            let direct = model.potential(ly(r), ly(z));
            let grid = tables.potential(ly(r), ly(z)).unwrap();
            assert_relative(&format!("Φ({r}, {z})"), grid, direct, 1e-3);
            let escape = tables.escape_speed(ly(r), ly(-z)).unwrap().value();
            assert_relative(
                &format!("escape({r}, {z})"),
                escape,
                (-2.0 * direct).sqrt(),
                1e-3,
            );
        }
        let plane = tables.potential(ly(r), ly(0.0)).unwrap();
        assert_relative("z = 0", plane, tables.potential_in_plane(ly(r)), 1e-9);
    }
    let kpc = LIGHT_YEARS_PER_KILOPARSEC;
    let far = tables.potential(ly(300_000.0), ly(400_000.0)).unwrap();
    assert_relative(
        "beyond the grid",
        far,
        model.potential(ly(300_000.0), ly(400_000.0)),
        1e-3,
    );
    assert!(tables.escape_speed(ly(8.0 * kpc), ly(0.0)).unwrap().value() > 400.0);
}
