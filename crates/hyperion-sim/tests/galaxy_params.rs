//! The galaxy's parameters (plan 02, P02.T5): ranges, derived quantities, the builder's
//! validation, the Milky Way fixture and the golden file.
//!
//! The checks over many seeds run here on 32 seeds, and over 10⁴ in `galaxy_sweeps.rs` under
//! `just test-slow` (plan 02, P02.T11).

mod common;

use common::{assert_derived_consistent, assert_params_in_ranges, assert_relative, assert_within};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::{
    ArmCount, BuildGalaxyParamsError, GalaxyParams, GalaxyParamsBuilder, HaloComponentKind,
    LesserProgenitorInput, Orbit, RecentProgenitorInput,
};
use hyperion_sim::galaxy::{POPULATIONS, Population};
use hyperion_sim::math;
use hyperion_sim::units::{
    Degrees, Dex, DexPerKiloparsec, LightYears, Radians, SolarMasses, Years,
};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

const GYR: f64 = 1e9;

/// The seeds of the fast checks.
fn fast_seeds() -> impl Iterator<Item = Seed> {
    (0..32_u64).map(|n| Seed::new(0x0200_5eed_0000_0000 | n))
}

fn params(seed: Seed) -> GalaxyParams {
    GalaxyParams::from_seed(seed, MassFunctionKind::Kroupa)
}

#[test]
fn every_getter_lies_in_its_range_over_32_seeds() {
    for seed in fast_seeds() {
        let p = params(seed);
        // 32 draws of each normal scatter stay within 4.5 standard deviations but for a chance
        // of about 10⁻⁴.
        assert_params_in_ranges(&p, 4.5);
        assert_derived_consistent(&p);
    }
}

#[test]
fn galaxy_params_are_a_pure_function_of_the_seed() {
    let seeds: Vec<Seed> = fast_seeds().take(6).collect();
    assert_order_independent(&seeds, |&seed| format!("{:?}", params(seed)));
    assert_ne!(params(seeds[0]), params(seeds[1]));
}

/// With no scatter, each coupled size grows exactly as the cube root of the mass it holds.
#[test]
fn sizes_follow_their_masses_as_the_cube_root() {
    let build = |stellar_mass: f64| {
        GalaxyParamsBuilder::new()
            .stellar_mass(SolarMasses::new(stellar_mass))
            .thin_length_scatter(Dex::new(0.0))
            .bulge_length_scatter(Dex::new(0.0))
            .bar_length_scatter(Dex::new(0.0))
            .nuclear_length_scatter(Dex::new(0.0))
            .build()
            .unwrap()
    };
    let (light, heavy) = (build(5.0e10), build(7.0e10));
    let cube_root_of_ratio = |p: Population| {
        math::cbrt(heavy.population_mass(p).value() / light.population_mass(p).value())
    };
    let thin = |g: &GalaxyParams| {
        g.population_mass(Population::YoungThinDisc).value()
            + g.population_mass(Population::OldThinDisc).value()
    };
    assert_relative(
        "thin length",
        heavy.thin_disc().length() / light.thin_disc().length(),
        math::cbrt(thin(&heavy) / thin(&light)),
        1e-12,
    );
    assert_relative(
        "bulge",
        heavy.bulge().scale_x() / light.bulge().scale_x(),
        cube_root_of_ratio(Population::Bulge),
        1e-12,
    );
    assert_relative(
        "bar",
        heavy.bar().half_length() / light.bar().half_length(),
        cube_root_of_ratio(Population::LongBar),
        1e-12,
    );
    assert_relative(
        "nuclear disc",
        heavy.nuclear_disc().length() / light.nuclear_disc().length(),
        cube_root_of_ratio(Population::NuclearDisc),
        1e-12,
    );
    // At the Milky Way's masses the laws give the Milky Way's sizes, P02.T5.b's references.
    let mw = build(6.0e10);
    assert_relative(
        "thin length at 3.4 × 10¹⁰",
        mw.thin_disc().length().value(),
        8_480.0 * math::cbrt(thin(&mw) / 3.4e10),
        1e-12,
    );
    assert_relative(
        "bar at 5.6 × 10⁹",
        mw.bar().half_length().value(),
        16_000.0 * math::cbrt(mw.population_mass(Population::LongBar).value() / 5.6e9),
        1e-12,
    );
    // A size is clamped to its range.
    let tiny = GalaxyParamsBuilder::new()
        .stellar_mass(SolarMasses::new(3.0e10))
        .nuclear_disc_share(0.01)
        .nuclear_length_scatter(Dex::new(-0.3))
        .build()
        .unwrap();
    assert_relative(
        "clamped nuclear disc",
        tiny.nuclear_disc().length().value(),
        200.0,
        0.0,
    );
}

/// The dark halo follows Dutton and Macciò (2014) with h = 0.671 and r₂₀₀ from 200 `ρ_crit`.
#[test]
fn the_dark_halo_follows_its_relations() {
    let p = GalaxyParams::milky_way_like();
    let dark = p.dark_halo();
    assert_relative("M200", dark.m200().value(), 6.0e10 / (0.157 * 0.32), 1e-14);
    let log_c = 0.905 - 0.101 * math::log10(dark.m200().value() * 0.671 / 1e12);
    assert_relative("c200", dark.concentration(), math::exp10(log_c), 1e-14);
    // ρ_crit = 3 H₀² ÷ 8πG = 2.775 366 × 10¹¹ h² M☉ Mpc⁻³, and the mean density inside r₂₀₀
    // is 200 times it.
    let r200 = dark.r200().value();
    let mean_density =
        dark.m200().value() / (4.0 / 3.0 * std::f64::consts::PI * r200 * r200 * r200);
    let ly_per_mpc = 3.261_563_777e6;
    let critical = 2.775_366e11 * 0.671 * 0.671 / (ly_per_mpc * ly_per_mpc * ly_per_mpc);
    assert_relative("200 ρ_crit", mean_density, 200.0 * critical, 1e-5);
    // About 225 kpc for the Milky Way (McMillan 2017's is 237 kpc at 1.3 × 10¹²).
    assert_within("r200 in kpc", r200 / 3_261.563_777, 200.0, 240.0);
    let scatter = GalaxyParamsBuilder::new()
        .dark_concentration_scatter(Dex::new(0.11))
        .build()
        .unwrap();
    assert_relative(
        "scatter",
        scatter.dark_halo().concentration() / dark.concentration(),
        math::exp10(0.11),
        1e-13,
    );
}

/// The fixture carries plan 02's values (P02.T5.c).
#[test]
fn the_milky_way_fixture_has_the_plan_values() {
    let p = GalaxyParams::milky_way_like();
    assert_params_in_ranges(&p, 1e-9);
    assert_derived_consistent(&p);
    assert_relative("M★", p.stellar_mass().value(), 6.0e10, 0.0);
    assert_relative(
        "thick",
        p.population_share(Population::ThickDisc),
        0.10,
        0.0,
    );
    let bulge_bar = p.population_share(Population::Bulge) + p.population_share(Population::LongBar);
    assert_relative("bulge and bar", bulge_bar, 0.31, 1e-15);
    assert_relative("bar", p.population_share(Population::LongBar), 0.093, 1e-14);
    assert_relative(
        "nuclear",
        p.population_share(Population::NuclearDisc),
        0.0175,
        0.0,
    );
    assert_relative("halo", p.population_share(Population::Halo), 0.01, 0.0);
    assert_relative("τ", p.sfh_timescale().value(), 7.0 * GYR, 0.0);
    assert_relative("thin length", p.thin_disc().length().value(), 8_480.0, 0.0);
    assert_relative("thin height", p.thin_disc().height().value(), 1_000.0, 0.0);
    let bulge = p.bulge();
    assert_relative("bulge a", bulge.scale_x().value(), 2_280.0, 0.0);
    assert_relative("bulge b", bulge.scale_y().value(), 1_440.0, 1e-15);
    assert_relative("bulge c", bulge.scale_z().value(), 820.0, 1e-15);
    assert_relative("boxiness", bulge.boxiness(), 3.5, 0.0);
    assert_relative("bar", p.bar().half_length().value(), 16_000.0, 0.0);
    assert_relative("bar height", p.bar().height().value(), 590.0, 0.0);
    assert_relative("corotation", p.bar().corotation_ratio(), 1.2, 0.0);
    assert_relative(
        "nuclear length",
        p.nuclear_disc().length().value(),
        290.0,
        0.0,
    );
    assert_relative(
        "nuclear height",
        p.nuclear_disc().height().value(),
        93.0,
        1e-15,
    );
    assert_eq!(p.arms().count(), ArmCount::Four);
    assert_relative(
        "pitch",
        Degrees::from(p.arms().pitch()).value(),
        12.0,
        1e-15,
    );
    assert_relative("f★", p.dark_halo().f_star(), 0.32, 0.0);
    // The Milky Way's own offset from M–σ, which makes its black hole Sgr A*'s mass.
    assert_same_bits(p.black_hole().scatter().value(), -0.421);
    // The Milky Way's system count, "about 10¹¹", and its mean present-day mass per system, "0.48
    // M☉ under Kroupa's function" (brainstorm, "Galaxy parameters"), with plan 02's ±0.03.
    assert_within("system count", p.system_count(), 1.0e11, 1.3e11);
    assert_within(
        "mean mass per system",
        p.stellar_mass().value() / p.system_count(),
        0.45,
        0.51,
    );
    // Its nuclear disc holds about 10⁹ M☉ (Launhardt et al. 2002; Sormani et al. 2022), and its
    // nuclear cluster 2.5 × 10⁷ M☉ (Schödel et al. 2014).
    let nuclear_mass = p.population_mass(Population::NuclearDisc).value();
    assert_within("nuclear disc mass", nuclear_mass, 0.9e9, 1.2e9);
    assert_relative(
        "nuclear cluster",
        p.nuclear_cluster().mass().value(),
        0.024 * nuclear_mass,
        1e-15,
    );
    // M₂₀₀ ÷ 6.5 × 10⁹ M☉ without scatter, rounded.
    assert_eq!(p.accretion().globular_count(), 184);
    assert_eq!(p.halo().components().len(), 6);
}

/// Formed mass exceeds the present mass of every population of the fixture (P02.T4).
#[test]
fn the_fixture_has_lost_mass_in_every_population() {
    let p = GalaxyParams::milky_way_like();
    for pop in POPULATIONS {
        assert!(p.mean_system_mass(pop) < p.mean_formed_mass(), "{pop:?}");
    }
    let old = p.mean_system_mass(Population::OldThinDisc).value();
    let young = p.mean_system_mass(Population::YoungThinDisc).value();
    assert_within("young ÷ old thin disc", young / old, 1.35, 1.45);
}

/// A builder with one value outside its range, and the parameter the error names.
type Case = (&'static str, fn(GalaxyParamsBuilder) -> GalaxyParamsBuilder);

fn orbit(apocentre: f64, pericentre: f64) -> Orbit {
    Orbit::new(
        LightYears::new(apocentre),
        LightYears::new(pericentre),
        Radians::new(1.0),
        Radians::new(1.0),
        Radians::new(1.0),
    )
}

fn lesser(flattening: f64, feh: f64) -> LesserProgenitorInput {
    LesserProgenitorInput::new(
        1.0,
        flattening,
        3.5,
        Years::new(11.5 * GYR),
        Dex::new(feh),
        Years::new(9.0 * GYR),
        orbit(60_000.0, 20_000.0),
    )
}

#[expect(clippy::too_many_lines, reason = "one case per setter")]
fn out_of_range_cases() -> Vec<Case> {
    vec![
        ("stellar_mass", |b| b.stellar_mass(SolarMasses::new(2.9e10))),
        ("stellar_mass", |b| b.stellar_mass(SolarMasses::new(1.1e11))),
        ("share.thick", |b| b.thick_share(0.15)),
        ("share.bulge_bar", |b| b.bulge_bar_share(0.19)),
        ("share.bar_of_bulge", |b| b.bar_share_of_bulge(0.41)),
        ("share.nuclear_disc", |b| b.nuclear_disc_share(0.009)),
        ("share.halo", |b| b.halo_share(0.015)),
        ("sfh.timescale", |b| b.sfh_timescale(Years::new(4.9 * GYR))),
        ("thin.length", |b| b.thin_length(LightYears::new(6_999.0))),
        ("thin.length.scatter", |b| {
            b.thin_length_scatter(Dex::new(0.46))
        }),
        ("thin.mean_height", |b| {
            b.thin_mean_height(LightYears::new(1_151.0))
        }),
        ("young.height", |b| b.young_height(LightYears::new(129.0))),
        ("thick.length_ratio", |b| b.thick_length_ratio(0.95)),
        ("thick.height_ratio", |b| b.thick_height_ratio(2.6)),
        ("bulge.length", |b| b.bulge_length(LightYears::new(3_001.0))),
        ("bulge.length.scatter", |b| {
            b.bulge_length_scatter(Dex::new(-0.55))
        }),
        ("bulge.b_over_a", |b| b.bulge_b_over_a(0.45)),
        ("bulge.c_over_a", |b| b.bulge_c_over_a(0.41)),
        ("bulge.boxiness", |b| b.bulge_boxiness(2.9)),
        ("bar.length", |b| {
            b.bar_half_length(LightYears::new(9_000.0))
        }),
        ("bar.length.scatter", |b| {
            b.bar_length_scatter(Dex::new(0.5))
        }),
        ("bar.width_ratio", |b| b.bar_width_ratio(0.13)),
        ("bar.height", |b| b.bar_height(LightYears::new(710.0))),
        ("bar.corotation_ratio", |b| b.bar_corotation_ratio(0.99)),
        ("nuclear.length", |b| {
            b.nuclear_length(LightYears::new(401.0))
        }),
        ("nuclear.length.scatter", |b| {
            b.nuclear_length_scatter(Dex::new(0.37))
        }),
        ("nuclear.height_ratio", |b| b.nuclear_height_ratio(0.29)),
        ("nuclear_cluster.mass.scatter", |b| {
            b.nuclear_cluster_mass_scatter(Dex::new(1.9))
        }),
        ("arms.pitch", |b| b.arm_pitch(Degrees::new(9.0))),
        ("arms.young_width", |b| {
            b.arm_young_width(LightYears::new(501.0))
        }),
        ("arms.young_fraction", |b| b.arm_young_fraction(0.69)),
        ("arms.old_amplitude", |b| b.arm_old_amplitude(0.31)),
        ("gas.mass_fraction", |b| b.gas_mass_fraction(0.21)),
        ("gas.length_ratio", |b| b.gas_length_ratio(1.4)),
        ("dark.f_star", |b| b.dark_f_star(0.46)),
        ("dark.concentration.scatter", |b| {
            b.dark_concentration_scatter(Dex::new(-1.0))
        }),
        ("bh.scatter", |b| b.black_hole_scatter(Dex::new(3.5))),
        ("metallicity.gradient", |b| {
            b.metallicity_gradient(DexPerKiloparsec::new(-0.03))
        }),
        ("halo.in_situ.share", |b| {
            b.halo_in_situ(
                0.31,
                0.5,
                LightYears::new(2_000.0),
                3.5,
                Years::new(11.5 * GYR),
            )
        }),
        ("halo.in_situ.flattening", |b| {
            b.halo_in_situ(
                0.2,
                0.6,
                LightYears::new(2_000.0),
                3.5,
                Years::new(11.5 * GYR),
            )
        }),
        ("halo.in_situ.core", |b| {
            b.halo_in_situ(
                0.2,
                0.5,
                LightYears::new(1_000.0),
                3.5,
                Years::new(11.5 * GYR),
            )
        }),
        ("halo.component.slope", |b| {
            b.halo_in_situ(
                0.2,
                0.5,
                LightYears::new(2_000.0),
                3.8,
                Years::new(11.5 * GYR),
            )
        }),
        ("halo.component.age", |b| {
            b.halo_in_situ(
                0.2,
                0.5,
                LightYears::new(2_000.0),
                3.5,
                Years::new(12.6 * GYR),
            )
        }),
        ("halo.dominant.share", |b| {
            b.halo_dominant(
                0.3,
                0.7,
                LightYears::new(3_000.0),
                3.5,
                Years::new(11.5 * GYR),
            )
        }),
        ("halo.dominant.flattening", |b| {
            b.halo_dominant(
                0.5,
                0.9,
                LightYears::new(3_000.0),
                3.5,
                Years::new(11.5 * GYR),
            )
        }),
        ("halo.dominant.core", |b| {
            b.halo_dominant(
                0.5,
                0.7,
                LightYears::new(6_000.0),
                3.5,
                Years::new(11.5 * GYR),
            )
        }),
        ("halo.dominant.break_radius", |b| {
            b.halo_dominant_break_radius(LightYears::new(30_000.0))
        }),
        ("halo.dominant.break_steepening", |b| {
            b.halo_dominant_break_steepening(2.1)
        }),
        ("halo.lesser.share_total", |b| {
            b.halo_lesser_share_total(0.26)
        }),
        ("halo.lesser.flattening", |b| {
            b.halo_lesser_progenitors(vec![lesser(0.5, -1.5), lesser(0.8, -1.5)])
        }),
        ("halo.component.feh", |b| {
            b.halo_lesser_progenitors(vec![lesser(0.8, -1.5), lesser(0.8, -0.5)])
        }),
        ("halo.debris.share", |b| {
            b.halo_debris(0.2, LightYears::new(4_000.0), 4.2, Years::new(11.5 * GYR))
        }),
        ("halo.debris.core", |b| {
            b.halo_debris(0.1, LightYears::new(2_000.0), 4.2, Years::new(11.5 * GYR))
        }),
        ("halo.debris.slope", |b| {
            b.halo_debris(0.1, LightYears::new(4_000.0), 3.9, Years::new(11.5 * GYR))
        }),
        ("halo.discrete_share", |b| b.halo_discrete_share(0.01)),
        ("accretion.last_major_merger", |b| {
            b.last_major_merger(Years::new(12.0 * GYR))
        }),
        ("accretion.progenitor.orbit", |b| {
            b.dominant_orbit(orbit(60_000.0, 70_000.0))
        }),
        ("accretion.progenitor.orbit", |b| {
            b.dominant_orbit(orbit(300_000.0, 70_000.0))
        }),
        // The dominant merger's orbit must be radial: an eccentricity of 0.5 is not.
        ("accretion.progenitor.orbit", |b| {
            b.dominant_orbit(orbit(60_000.0, 20_000.0))
        }),
        // Stars no younger than the merger that heated or brought them: an age centre of 11 Gyr
        // leaves the youngest stars at 10.5 Gyr, after a merger 10.8 Gyr ago.
        ("halo.component.age", |b| {
            b.last_major_merger(Years::new(10.8 * GYR)).halo_in_situ(
                0.2,
                0.5,
                LightYears::new(2_000.0),
                3.5,
                Years::new(11.0 * GYR),
            )
        }),
        ("halo.component.age", |b| {
            b.last_major_merger(Years::new(10.8 * GYR)).halo_dominant(
                0.5,
                0.7,
                LightYears::new(3_000.0),
                3.5,
                Years::new(11.2 * GYR),
            )
        }),
        ("accretion.progenitor.time", |b| {
            b.halo_lesser_progenitors(vec![
                lesser(0.8, -1.5),
                LesserProgenitorInput::new(
                    1.0,
                    0.8,
                    3.5,
                    Years::new(11.5 * GYR),
                    Dex::new(-1.5),
                    Years::new(11.2 * GYR),
                    orbit(60_000.0, 20_000.0),
                ),
            ])
        }),
        ("accretion.progenitor.mass", |b| {
            b.recent_progenitors(vec![RecentProgenitorInput::new(
                SolarMasses::new(1e10),
                Years::new(GYR),
                orbit(60_000.0, 20_000.0),
            )])
        }),
        ("accretion.progenitor.time", |b| {
            b.recent_progenitors(vec![RecentProgenitorInput::new(
                SolarMasses::new(1e8),
                Years::new(7.0 * GYR),
                orbit(60_000.0, 20_000.0),
            )])
        }),
        ("accretion.globular_count.scatter", |b| {
            b.globular_count_scatter(Dex::new(-1.9))
        }),
        ("stellar_mass", |b| {
            b.stellar_mass(SolarMasses::new(f64::NAN))
        }),
    ]
}

#[test]
fn the_builder_rejects_each_out_of_range_value() {
    for (expected, case) in out_of_range_cases() {
        match case(GalaxyParamsBuilder::new()).build() {
            Err(BuildGalaxyParamsError::OutOfRange { parameter, .. }) => {
                assert_eq!(parameter, expected);
            }
            other => panic!("{expected}: {other:?}"),
        }
    }
    let one = GalaxyParamsBuilder::new().halo_lesser_progenitors(vec![lesser(0.8, -1.5)]);
    assert_eq!(
        one.build(),
        Err(BuildGalaxyParamsError::LesserProgenitorCount { count: 1 })
    );
    let six = GalaxyParamsBuilder::new().halo_lesser_progenitors(vec![lesser(0.8, -1.5); 6]);
    assert_eq!(
        six.build(),
        Err(BuildGalaxyParamsError::LesserProgenitorCount { count: 6 })
    );
    let weightless = LesserProgenitorInput::new(
        0.0,
        0.8,
        3.5,
        Years::new(11.5 * GYR),
        Dex::new(-1.5),
        Years::new(9.0 * GYR),
        orbit(60_000.0, 20_000.0),
    );
    assert_eq!(
        GalaxyParamsBuilder::new()
            .halo_lesser_progenitors(vec![weightless; 2])
            .build(),
        Err(BuildGalaxyParamsError::LesserWeightsZero)
    );
    assert_eq!(
        BuildGalaxyParamsError::LesserProgenitorCount { count: 6 }.to_string(),
        "a halo has 2–5 lesser progenitors, not 6"
    );
}

/// A builder's values reach the parameters, and the halo's shares are renormalised.
#[test]
fn the_builder_sets_what_it_is_given() {
    let p = GalaxyParamsBuilder::new()
        .mass_function(MassFunctionKind::Chabrier)
        .arm_count(ArmCount::Two)
        .halo_lesser_progenitors(vec![lesser(0.8, -1.5), lesser(0.9, -1.1)])
        .build()
        .unwrap();
    assert_eq!(p.mass_function(), MassFunctionKind::Chabrier);
    assert_eq!(p.arms().count(), ArmCount::Two);
    let components = p.halo().components();
    assert_eq!(components.len(), 5);
    assert_eq!(components[3].kind(), HaloComponentKind::Lesser(2));
    assert_relative("lesser flattening", components[3].flattening(), 0.9, 0.0);
    assert_relative("lesser [Fe/H]", components[3].feh_mean().value(), -1.1, 0.0);
    // Shares before renormalising: 0.225, 0.475, 0.175 split in two, 0.115; total 0.99.
    assert_relative("in-situ share", components[0].share(), 0.225 / 0.99, 1e-14);
    assert_relative("lesser share", components[2].share(), 0.0875 / 0.99, 1e-14);
    // Chabrier's mean masses are heavier (brainstorm: 0.55–0.60 against Kroupa's 0.48).
    let kroupa = GalaxyParams::milky_way_like();
    assert!(p.system_count() < kroupa.system_count());
}

/// Every parameter of one galaxy, in the order of the getters.
fn write_params(w: &mut GoldenWriter, label: &str, p: &GalaxyParams) {
    let mut f = |name: &str, value: f64| w.f64(&format!("{label}.{name}"), value);
    f("stellar_mass", p.stellar_mass().value());
    f("sfh_timescale", p.sfh_timescale().value());
    f("bar_share_of_bulge", p.bar_share_of_bulge());
    for pop in POPULATIONS {
        let name = pop.name();
        f(&format!("{name}.share"), p.population_share(pop));
        f(
            &format!("{name}.mean_mass"),
            p.mean_system_mass(pop).value(),
        );
        f(&format!("{name}.mass"), p.population_mass(pop).value());
    }
    f("system_count", p.system_count());
    f("mean_formed_mass", p.mean_formed_mass().value());
    f("thin.length", p.thin_disc().length().value());
    f("thin.height", p.thin_disc().height().value());
    f("young.length", p.young_disc().length().value());
    f("young.height", p.young_disc().height().value());
    f("thick.length", p.thick_disc().length().value());
    f("thick.height", p.thick_disc().height().value());
    f("bulge.scale_x", p.bulge().scale_x().value());
    f("bulge.scale_y", p.bulge().scale_y().value());
    f("bulge.scale_z", p.bulge().scale_z().value());
    f("bulge.boxiness", p.bulge().boxiness());
    f("bar.half_length", p.bar().half_length().value());
    f("bar.width", p.bar().width().value());
    f("bar.height", p.bar().height().value());
    f("bar.corotation_ratio", p.bar().corotation_ratio());
    f("nuclear.length", p.nuclear_disc().length().value());
    f("nuclear.height", p.nuclear_disc().height().value());
    f("nuclear_cluster.mass", p.nuclear_cluster().mass().value());
    f("arms.count", f64::from(p.arms().count().get()));
    f("arms.pitch", p.arms().pitch().value());
    f("arms.young_width", p.arms().young_width().value());
    f("arms.young_fraction", p.arms().young_fraction());
    f("arms.old_amplitude", p.arms().old_amplitude());
    f("gas.mass", p.gas_disc().mass().value());
    f("gas.length", p.gas_disc().length().value());
    f("dark.f_star", p.dark_halo().f_star());
    f("dark.m200", p.dark_halo().m200().value());
    f("dark.concentration", p.dark_halo().concentration());
    f("dark.r200", p.dark_halo().r200().value());
    f("bh.scatter", p.black_hole().scatter().value());
    f("bh.sigma", p.black_hole().bulge_dispersion().value());
    f("bh.mass", p.black_hole().mass().value());
    f("metallicity_gradient", p.metallicity_gradient().value());
    f("halo.discrete_share", p.halo().discrete_share());
    for c in p.halo().components() {
        let name = format!("halo.{:?}", c.kind())
            .to_lowercase()
            .replace(['(', ')'], "");
        f(&format!("{name}.share"), c.share());
        f(&format!("{name}.slope"), c.slope());
        f(&format!("{name}.core"), c.core().value());
        f(&format!("{name}.flattening"), c.flattening());
        f(&format!("{name}.cut_radius"), c.cut_radius().value());
        f(&format!("{name}.age_min"), c.ages().min().value());
        f(&format!("{name}.age_max"), c.ages().max().value());
        f(&format!("{name}.feh_mean"), c.feh_mean().value());
        if let Some(b) = c.outer_break() {
            f(&format!("{name}.break_radius"), b.radius().value());
            f(&format!("{name}.break_steepening"), b.steepening());
        }
    }
    let accretion = p.accretion();
    f(
        "accretion.last_major_merger",
        accretion.last_major_merger().value(),
    );
    f(
        "accretion.globular_count",
        f64::from(accretion.globular_count()),
    );
    for progenitor in accretion.progenitors() {
        let name = format!("progenitor.{}", progenitor.kind().number());
        let orbit = progenitor.orbit();
        f(&format!("{name}.mass"), progenitor.mass().value());
        f(&format!("{name}.accreted"), progenitor.accreted().value());
        f(&format!("{name}.apocentre"), orbit.apocentre().value());
        f(&format!("{name}.pericentre"), orbit.pericentre().value());
        f(&format!("{name}.inclination"), orbit.inclination().value());
        f(&format!("{name}.node"), orbit.node().value());
        f(&format!("{name}.phase"), orbit.phase().value());
    }
}

/// Three pinned seeds under Kroupa's function, one under Chabrier's, and the fixture: every
/// parameter, bit for bit.
#[test]
fn galaxy_params_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for seed in [
        0x0000_0000_0000_0001_u64,
        0x5eed_0000_c0ff_ee00,
        0xdead_beef_cafe_f00d,
    ] {
        let seed = Seed::new(seed);
        w.line(&format!("# seed {seed}"));
        write_params(&mut w, &seed.to_string(), &params(seed));
    }
    let seed = Seed::new(0x5eed_0000_c0ff_ee00);
    w.line(&format!("# seed {seed}, Chabrier"));
    write_params(
        &mut w,
        "chabrier",
        &GalaxyParams::from_seed(seed, MassFunctionKind::Chabrier),
    );
    w.line("# milky_way_like");
    write_params(&mut w, "milky_way", &GalaxyParams::milky_way_like());
    golden!("galaxy_params", w.as_str());
}
