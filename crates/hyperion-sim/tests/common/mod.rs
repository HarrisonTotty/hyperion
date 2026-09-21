//! Checks shared by the galaxy tests: every parameter inside its range, and the derived
//! quantities consistent with one another (plan 02, P02.T5.a and P02.T5.b).
//!
//! The ranges are written out here from plan 02's table, independently of the sim's own
//! constants, so that a wrong constant in the sim fails a test instead of moving the bracket.

use hyperion_sim::galaxy::params::{
    ArmCount, GalaxyParams, HaloComponentKind, HaloComponentParams, ProgenitorKind,
};
use hyperion_sim::galaxy::{POPULATIONS, Population};
use hyperion_sim::math;
use hyperion_sim::units::Degrees;

const GYR: f64 = 1e9;

/// Asserts `low ≤ value ≤ high`, naming the value.
#[track_caller]
pub fn assert_within(what: &str, value: f64, low: f64, high: f64) {
    assert!(
        (low..=high).contains(&value),
        "{what} = {value} lies outside [{low}, {high}]"
    );
}

/// Asserts `|actual − expected| ≤ tolerance × |expected|`.
#[track_caller]
pub fn assert_relative(what: &str, actual: f64, expected: f64, tolerance: f64) {
    let error = ((actual - expected) / expected).abs();
    assert!(
        error <= tolerance,
        "{what}: {actual} is {error:e} from {expected}, over {tolerance:e}"
    );
}

/// Every getter of `p` lies inside its range from plan 02's table. `max_sigmas` bounds the
/// normal scatters: 9 is the builder's own limit, a smaller count suits a small sample of seeds.
#[expect(
    clippy::too_many_lines,
    reason = "one assertion per parameter of the table, in its order"
)]
pub fn assert_params_in_ranges(p: &GalaxyParams, max_sigmas: f64) {
    let share = |pop| p.population_share(pop);
    assert_within("stellar mass", p.stellar_mass().value(), 3e10, 1e11);
    assert_within("thick share", share(Population::ThickDisc), 0.08, 0.14);
    let bulge_bar = share(Population::Bulge) + share(Population::LongBar);
    assert_within("bulge and bar share", bulge_bar, 0.20, 0.35);
    assert_within("bar of bulge", p.bar_share_of_bulge(), 0.30, 0.40);
    assert_relative(
        "bar share",
        share(Population::LongBar),
        bulge_bar * p.bar_share_of_bulge(),
        1e-12,
    );
    assert_within("nuclear share", share(Population::NuclearDisc), 0.01, 0.025);
    assert_within("halo share", share(Population::Halo), 0.007, 0.014);
    let thin = share(Population::YoungThinDisc) + share(Population::OldThinDisc);
    assert_within("thin share", thin, 0.47, 0.70);
    assert_within(
        "young part of the thin disc",
        share(Population::YoungThinDisc) / thin,
        0.0030,
        0.0056,
    );
    let total: f64 = POPULATIONS.iter().map(|&pop| share(pop)).sum();
    assert_relative("shares", total, 1.0, 1e-14);
    assert_within("timescale", p.sfh_timescale().value(), 5.0 * GYR, 9.0 * GYR);

    let thin_disc = p.thin_disc();
    assert_within("thin length", thin_disc.length().value(), 7_000.0, 11_500.0);
    assert_within(
        "thin mean height",
        thin_disc.height().value(),
        850.0,
        1_150.0,
    );
    assert_within(
        "young height",
        p.young_disc().height().value(),
        130.0,
        200.0,
    );
    assert_relative(
        "young length",
        p.young_disc().length().value(),
        thin_disc.length().value(),
        0.0,
    );
    assert_within(
        "thick length ratio",
        p.thick_disc().length() / thin_disc.length(),
        0.7,
        0.9,
    );
    assert_within(
        "thick height ratio",
        p.thick_disc().height() / thin_disc.height(),
        2.7,
        3.3,
    );

    let bulge = p.bulge();
    assert_within("bulge a", bulge.scale_x().value(), 1_700.0, 3_000.0);
    assert_within("bulge b ÷ a", bulge.scale_y() / bulge.scale_x(), 0.5, 0.7);
    assert_within("bulge c ÷ a", bulge.scale_z() / bulge.scale_x(), 0.3, 0.4);
    assert_within("bulge boxiness", bulge.boxiness(), 3.0, 4.0);

    let bar = p.bar();
    assert_within(
        "bar half-length",
        bar.half_length().value(),
        10_000.0,
        18_000.0,
    );
    assert_within(
        "bar width ratio",
        bar.width() / bar.half_length(),
        0.08,
        0.12,
    );
    assert_within("bar height", bar.height().value(), 500.0, 700.0);
    assert_within("bar corotation ratio", bar.corotation_ratio(), 1.0, 1.4);

    let nuclear = p.nuclear_disc();
    assert_within("nuclear length", nuclear.length().value(), 200.0, 400.0);
    assert_within(
        "nuclear height ratio",
        nuclear.height() / nuclear.length(),
        0.3,
        0.5,
    );
    let cluster_ratio =
        p.nuclear_cluster().mass().value() / p.population_mass(Population::NuclearDisc).value();
    let spread = math::exp10(max_sigmas * 0.2);
    assert_within(
        "nuclear cluster ratio",
        cluster_ratio,
        0.024 / spread,
        0.024 * spread,
    );

    let arms = p.arms();
    assert!(matches!(arms.count(), ArmCount::Two | ArmCount::Four));
    let degrees = Degrees::from(arms.pitch()).value();
    assert_within("pitch", degrees, 10.0 - 1e-12, 18.0 + 1e-12);
    assert_within("young arm width", arms.young_width().value(), 250.0, 500.0);
    assert_within("young arm fraction", arms.young_fraction(), 0.7, 0.9);
    assert_within("old arm amplitude", arms.old_amplitude(), 0.10, 0.30);

    let thin_mass = p.population_mass(Population::YoungThinDisc).value()
        + p.population_mass(Population::OldThinDisc).value();
    let gas = p.gas_disc();
    assert_within("gas fraction", gas.mass().value() / thin_mass, 0.10, 0.20);
    assert_within(
        "gas length ratio",
        gas.length() / thin_disc.length(),
        1.5,
        2.0,
    );
    assert_within("gas height", gas.height().value(), 400.0, 400.0);

    let dark = p.dark_halo();
    assert_within("f★", dark.f_star(), 0.12, 0.45);
    assert_relative(
        "M200",
        dark.m200().value(),
        p.stellar_mass().value() / (0.157 * dark.f_star()),
        1e-14,
    );
    assert_within("concentration", dark.concentration(), 2.0, 40.0);
    assert_within("r200", dark.r200().value(), 400_000.0, 1_400_000.0);
    // The fixture's one scatter is the Milky Way's measured offset from M–σ, 1.1 times the
    // relation's 0.38 dex (plan 02, Risks, R13), so this bound never falls below 1.2 of it.
    let bh_sigmas = max_sigmas.max(1.2);
    assert_within(
        "black hole scatter",
        p.black_hole().scatter().value(),
        -bh_sigmas * 0.38,
        bh_sigmas * 0.38,
    );
    assert_within(
        "metallicity gradient",
        p.metallicity_gradient().value(),
        -0.07,
        -0.04,
    );

    assert_halo_in_ranges(p);
    assert_accretion_in_ranges(p);
}

fn assert_halo_in_ranges(p: &GalaxyParams) {
    let halo = p.halo();
    let components = halo.components();
    assert_within(
        "halo components",
        f64::from(u32::try_from(components.len()).unwrap()),
        5.0,
        8.0,
    );
    let total: f64 = components.iter().map(HaloComponentParams::share).sum();
    assert_relative("halo shares", total, 1.0, 1e-14);
    assert_eq!(components[0].kind(), HaloComponentKind::InSitu);
    assert_eq!(components[1].kind(), HaloComponentKind::DominantMerger);
    assert_eq!(
        components[components.len() - 1].kind(),
        HaloComponentKind::GlobularDebris
    );
    for (n, c) in (1_u8..).zip(&components[2..components.len() - 1]) {
        assert_eq!(c.kind(), HaloComponentKind::Lesser(n));
    }
    for c in components {
        let what = format!("{:?}", c.kind());
        let (slope, flattening, core, cut) = match c.kind() {
            HaloComponentKind::InSitu => ((3.3, 3.7), (0.45, 0.55), (1_500.0, 3_000.0), 50_000.0),
            HaloComponentKind::DominantMerger => {
                ((3.3, 3.7), (0.6, 0.8), (2_000.0, 5_000.0), 65_000.0)
            }
            HaloComponentKind::Lesser(_) => ((3.3, 3.7), (0.6, 1.0), (3_000.0, 3_000.0), 65_000.0),
            HaloComponentKind::GlobularDebris => {
                ((4.0, 4.5), (1.0, 1.0), (3_000.0, 5_000.0), 65_000.0)
            }
        };
        assert_within(&what, c.slope(), slope.0, slope.1);
        assert_within(&what, c.flattening(), flattening.0, flattening.1);
        assert_within(&what, c.core().value(), core.0, core.1);
        assert_within(&what, c.cut_radius().value(), cut, cut);
        assert_within(&what, c.ages().min().value(), 10.0 * GYR, 12.0 * GYR);
        // Stars formed before the event that put them in the halo: the last major merger heated
        // the in-situ component and quenched the dominant one.
        let merger = p.accretion().last_major_merger().value();
        match c.kind() {
            HaloComponentKind::InSitu | HaloComponentKind::DominantMerger => {
                assert!(
                    c.ages().min().value() >= merger,
                    "{what}: younger than the merger"
                );
            }
            HaloComponentKind::Lesser(_) | HaloComponentKind::GlobularDebris => {}
        }
        assert_within(&what, c.ages().max().value(), 11.0 * GYR, 13.0 * GYR);
        assert_relative(&what, (c.ages().max() - c.ages().min()).value(), GYR, 1e-12);
        assert_within(&what, c.feh_mean().value(), -2.0, -0.6);
        match c.outer_break() {
            Some(b) => {
                assert_eq!(c.kind(), HaloComponentKind::DominantMerger);
                assert_within("break radius", b.radius().value(), 40_000.0, 90_000.0);
                assert_within("break steepening", b.steepening(), 1.0, 2.0);
            }
            None => assert_ne!(c.kind(), HaloComponentKind::DominantMerger),
        }
    }
    assert_within("discrete share", halo.discrete_share(), 0.02, 0.15);
}

fn assert_accretion_in_ranges(p: &GalaxyParams) {
    let accretion = p.accretion();
    let merger = accretion.last_major_merger().value();
    assert_within("last major merger", merger, 6.0 * GYR, 11.0 * GYR);
    let progenitors = accretion.progenitors();
    assert_eq!(progenitors[0].kind(), ProgenitorKind::DominantMerger);
    assert_relative(
        "dominant accreted",
        progenitors[0].accreted().value(),
        merger,
        0.0,
    );
    let lesser = p.halo().components().len() - 3;
    let halo_mass = p.population_mass(Population::Halo).value();
    for (i, progenitor) in progenitors.iter().enumerate() {
        let what = format!("{:?}", progenitor.kind());
        let accreted = progenitor.accreted().value();
        match progenitor.kind() {
            ProgenitorKind::DominantMerger => assert_eq!(i, 0),
            ProgenitorKind::Lesser(n) => {
                assert_eq!(usize::from(n), i);
                assert_within(&what, accreted, 6.0 * GYR, 12.0 * GYR);
                assert!(progenitor.mass().value() < halo_mass);
                // Its component's youngest stars predate its accretion.
                let component = &p.halo().components()[i + 1];
                assert_eq!(component.kind(), HaloComponentKind::Lesser(n));
                assert!(
                    component.ages().min().value() >= accreted,
                    "{what}: stars younger than the accretion"
                );
            }
            ProgenitorKind::Recent(j) => {
                assert_eq!(usize::try_from(j).unwrap() + lesser + 1, i);
                assert_within(&what, accreted, 0.0, 6.0 * GYR);
                assert_within(&what, progenitor.mass().value(), 1e5, 3.162_277_660_168_4e9);
            }
        }
        let orbit = progenitor.orbit();
        let apocentre = orbit.apocentre().value();
        let pericentre = orbit.pericentre().value();
        assert_within(&what, apocentre, 20_000.0, 200_000.0);
        if progenitor.kind() == ProgenitorKind::DominantMerger {
            // Radial, like the Gaia Sausage (Belokurov et al. 2018).
            let e = (apocentre - pericentre) / (apocentre + pericentre);
            assert_within("dominant eccentricity", e, 0.85 - 1e-12, 0.95 + 1e-12);
        } else {
            assert_within(&what, pericentre / apocentre, 0.05 - 1e-12, 0.6 + 1e-12);
        }
        assert_within(
            &what,
            orbit.inclination().value(),
            0.0,
            core::f64::consts::PI,
        );
        let two_pi = 2.0 * core::f64::consts::PI;
        assert_within(&what, orbit.node().value(), 0.0, two_pi);
        assert_within(&what, orbit.phase().value(), 0.0, two_pi);
    }
    let dominant = progenitors[0].mass().value();
    assert!(dominant > 0.0 && dominant < halo_mass);
    let count = accretion.globular_count();
    assert!((80..=800).contains(&count), "globular count {count}");
}

/// The derived quantities agree with one another (P02.T5.b).
pub fn assert_derived_consistent(p: &GalaxyParams) {
    let stellar = p.stellar_mass().value();
    let masses: f64 = POPULATIONS
        .iter()
        .map(|&pop| p.population_mass(pop).value())
        .sum();
    assert_relative("population masses", masses, stellar, 1e-12);
    let per_system: f64 = POPULATIONS
        .iter()
        .map(|&pop| p.population_share(pop) * p.mean_system_mass(pop).value())
        .sum();
    assert_relative(
        "system count",
        p.system_count(),
        stellar / per_system,
        1e-12,
    );
    assert_within("system count", p.system_count(), 0.5e11, 2.1e11);
    let m200 = p.dark_halo().m200().value();
    // Plan 02's "1.4–5.3 × 10¹²" is this range rounded: 10¹¹ M☉ ÷ (0.157 f★) at the ends of f★'s
    // range, 0.45 and 0.12, is 1.415 and 5.308 × 10¹², so the exact ends are checked.
    let per_1e11 = |f_star: f64| 1e11 / (0.157 * f_star);
    assert_within(
        "M200 per 10¹¹ M☉ of stars",
        m200 / (stellar / 1e11),
        per_1e11(0.45) * (1.0 - 1e-12),
        per_1e11(0.12) * (1.0 + 1e-12),
    );
    let formed = p.mean_formed_mass().value();
    for pop in POPULATIONS {
        let mean = p.mean_system_mass(pop).value();
        assert!(mean < formed, "{pop:?}: {mean} ≥ {formed}");
        match pop {
            Population::YoungThinDisc => assert_within("young mean mass", mean, 0.65, 0.75),
            _ => assert_within(pop.name(), mean, 0.45, 0.52),
        }
    }
}
