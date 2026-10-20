//! The aerosol extinction's golden (plan 14, P14.T49.e).
//!
//! `tests/golden/substance/aerosol.golden` pins, by their bits at the generator version, the mass
//! extinction `κ_ext`, the extinction efficiency `Q̄_ext` and the single-scattering albedo ω that
//! [`aerosol_optics`] interpolates from the committed `condensate_extinction` tables between their
//! nodes, with each value's basis and stand-in flag: for each of the gamma distribution and the
//! log-normal, at a point near the grid's lower corner, one inside a cell and one near its upper
//! corner, none of them a node; for a lossless file (aluminium oxide's, on the sphere fallback,
//! its index outside the hexahedra's kernels), a measured absorbing one on its phase table (mars
//! dust's), water ice on Yang et al.'s, iron, whose longwave table ends at 23 µm, a stand-in on
//! spheres (the ice-giant haze), a stand-in on the sphere fallback (ammonia ice), and Venus's four
//! deck modes of the 75 wt% acid (Pollack et al. 1993's modal radii and widths, as
//! `substance::aerosol`'s Venus test takes them). Nothing on
//! the record reads the tables until P14.T54.b (decision-bump-plan-after-22 §1.3), so until then a
//! refit re-blesses this golden at the version in force.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::math;
use hyperion_sim::substance::{
    CondensedPhase, FileVariant, SizeDistribution, aerosol_optics, by_key,
};
use hyperion_sim::units::{Kelvin, Metres};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The files pinned: a key, its phase, its variant, a temperature at which its grain density is
/// read, K, and a label.
const FILES: [(&str, CondensedPhase, Option<&str>, f64, &str); 6] = [
    ("Al2O3", CondensedPhase::Solid, None, 1_800.0, "Al2O3 solid"),
    (
        "mars_dust",
        CondensedPhase::Solid,
        None,
        210.0,
        "mars_dust solid",
    ),
    ("H2O", CondensedPhase::Solid, None, 220.0, "H2O solid"),
    ("Fe", CondensedPhase::Solid, None, 1_500.0, "Fe solid"),
    (
        "ice_giant_haze",
        CondensedPhase::Solid,
        None,
        80.0,
        "ice_giant_haze solid",
    ),
    ("NH3", CondensedPhase::Solid, None, 120.0, "NH3 solid"),
];

/// The off-node points of the gamma distribution: (`r_eff` m, `v_eff`), near the lower corner of
/// the grid (1 nm, 0.04), inside a cell, and near the upper corner (101 µm, 0.45).
const GAMMA_POINTS: [(f64, f64); 3] = [(1.05e-9, 0.0415), (3.7e-7, 0.137), (9.9e-5, 0.44)];

/// The log-normal's: near (1 nm, 0.04), inside a cell, and near (33 µm, 0.65).
const LOG_NORMAL_POINTS: [(f64, f64); 3] = [(1.05e-9, 0.0415), (3.7e-7, 0.137), (3.2e-5, 0.64)];

/// Venus's deck modes: modal radius, m, and ln σ (the module's remarks).
const VENUS_MODES: [(f64, f64); 4] = [
    (0.30e-6, 0.44),
    (1.00e-6, 0.25),
    (1.40e-6, 0.21),
    (3.65e-6, 0.25),
];

/// Pins the optics of row `key` in `phase` and `variant` at `t` K for `mode` under `label`, or
/// its refusal.
fn pin(
    w: &mut GoldenWriter,
    label: &str,
    (key, phase, variant, t): (&str, CondensedPhase, Option<&str>, f64),
    mode: &SizeDistribution,
) {
    let id = by_key(key).expect("the golden's rows are registry rows");
    match aerosol_optics(id, phase, FileVariant::from(variant), mode, Kelvin::new(t)) {
        Ok(optics) => {
            w.line(&format!(
                "{label}: basis {}, stand-in {}",
                optics.extinction_basis().name(),
                optics.is_stand_in()
            ));
            w.f64(
                &format!("{label}: mass_extinction_m2_per_kg"),
                optics.mass_extinction_m2_per_kg(),
            );
            w.f64(
                &format!("{label}: extinction_efficiency"),
                optics.extinction_efficiency(),
            );
            w.f64(
                &format!("{label}: single_scattering_albedo"),
                optics.single_scattering_albedo(),
            );
        }
        Err(refusal) => w.line(&format!("{label}: refused: {refusal}")),
    }
}

#[test]
fn aerosol_optics_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("[gamma]");
    for (key, phase, variant, t, name) in FILES {
        for (radius, variance) in GAMMA_POINTS {
            let mode = SizeDistribution::gamma(Metres::new(radius), variance)
                .expect("the points are gamma distributions");
            let label = format!("{name} gamma r_eff {radius:e} m v_eff {variance}");
            pin(&mut w, &label, (key, phase, variant, t), &mode);
        }
    }
    w.line("[log-normal]");
    for (key, phase, variant, t, name) in FILES {
        for (radius, variance) in LOG_NORMAL_POINTS {
            let mode = SizeDistribution::log_normal(Metres::new(radius), variance)
                .expect("the points are log-normals");
            let label = format!("{name} log-normal r_eff {radius:e} m v_eff {variance}");
            pin(&mut w, &label, (key, phase, variant, t), &mode);
        }
    }
    w.line("[venus]");
    for (n, (modal_radius_m, ln_sigma)) in VENUS_MODES.into_iter().enumerate() {
        let s2 = ln_sigma * ln_sigma;
        let mode = SizeDistribution::log_normal(
            Metres::new(modal_radius_m * math::exp(2.5 * s2)),
            math::exp_m1(s2),
        )
        .expect("Venus's modes are log-normals");
        let label = format!("H2SO4 75wt% liquid Venus mode {}", n + 1);
        pin(
            &mut w,
            &label,
            ("H2SO4", CondensedPhase::Liquid, Some("75wt%"), 250.0),
            &mode,
        );
    }
    let text = w.finish();
    golden!("substance/aerosol", &text);
}
