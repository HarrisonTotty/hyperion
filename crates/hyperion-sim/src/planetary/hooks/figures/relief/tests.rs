//! P14.T48.a's relief, continents and volcanism level on the Solar System table, and their laws'
//! cases (science-p14-t24b-laws §4.1 and §4.5).

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

use super::*;
use crate::id::SystemId;
use crate::planetary::hooks::figures::tests::{
    TABLE_CONTINENTS, body_of, figures_from, figures_of, figures_table, inputs_of,
    the_giant_impact_moon,
};
use crate::planetary::hooks::figures::{FiguresInputs, GlobalFigures, heat_at, interior_of};

/// The table's Ceres with McCord and Sotin 2005's lowest water, 17% of its mass (J. Geophys. Res.
/// 110, E05009), whose crust is then ice-rich (P14.T48.f's test): the table's own keeps 78 m and
/// is silicate (plan 14's Risks, deferred).
fn wet_ceres() -> GlobalFigures {
    let table = figures_table();
    let ceres = body_of(&table, "Ceres");
    let water = 0.17 * crate::units::Kilograms::from(ceres.mass()).value();
    figures_from(&FiguresInputs {
        inventory_water: crate::units::Kilograms::new(water),
        ..inputs_of(ceres, "Ceres")
    })
    .unwrap()
}

/// What `GlobalFigures::derive` hands the volcanism level of the body `inputs` describe, whose
/// figures are `figures`: its crust, surface age and iron share, and its heat flow at a system age
/// under today's state.
fn volcanism_of<'a>(
    inputs: &'a FiguresInputs<'a>,
    figures: &GlobalFigures,
) -> VolcanismInputs<impl Fn(Years) -> WattsPerSquareMetre + 'a> {
    let air = inputs.atmosphere;
    let heat = inputs.heat();
    let interior = interior_of(&heat).expect("a body with figures has an interior");
    let state = air.state();
    VolcanismInputs {
        crust: figures.impactors().crust(),
        surface_age: figures.surface_age(),
        age: inputs.age,
        surface_temperature: air.surface_temperature(),
        surface_gravity: inputs.surface_gravity,
        radius: inputs.radius,
        iron_share: heat_at(&heat, interior, inputs.age, state).iron_share,
        heat_flow_at: move |t: Years| heat_at(&heat, interior, t, state).heat_flow,
    }
}

/// [`MARS_ERUPTED_MELT_W_GYR_PER_M2`] and [`MARS_PLUME_VIGOUR_M4_PER_S2`] are the table's Mars's
/// own, refitted here to 10⁻¹², so that its volcanism level is 1 (science-p14-t24b-laws §4.1).
#[test]
fn the_volcanism_level_is_fitted_on_the_table_s_mars() {
    let table = figures_table();
    let mars = body_of(&table, "Mars");
    let inputs = inputs_of(mars, "Mars");
    let figures = figures_of(&table, "Mars");
    let volcanism = volcanism_of(&inputs, figures);
    let melt = erupted_melt_w_gyr_per_m2(&volcanism);
    let vigour = plume_vigour_m4_per_s2(
        volcanism.surface_gravity,
        volcanism.radius,
        volcanism.iron_share,
    );
    eprintln!(
        "the table's Mars: I {melt:?} W m⁻² Gyr over {} Gyr, g D_m³ {vigour:?} m⁴ s⁻², iron {}",
        figures.surface_age().value() / 1e9,
        volcanism.iron_share
    );
    assert!(
        (melt / MARS_ERUPTED_MELT_W_GYR_PER_M2 - 1.0).abs() < 1e-12,
        "{melt:?}"
    );
    assert!(
        (vigour / MARS_PLUME_VIGOUR_M4_PER_S2 - 1.0).abs() < 1e-12,
        "{vigour:?}"
    );
    assert!(
        (figures.volcanism() - 1.0).abs() < 1e-12,
        "{}",
        figures.volcanism()
    );
    // The ruling's replica's I♂ is about 0.0335 W m⁻² Gyr.
    assert!((0.025..0.045).contains(&melt), "{melt}");
}

/// The strength limit of the constructional share is a bound on V (science-p14-t24b-laws §4.x.1):
/// `V_s` = h♂ ÷ (4 `C₀`), 21.9 km over four times 2.76 km, and the law holds every level to
/// [0, `V_s`], a NaN to 0.
#[test]
fn the_volcanism_level_is_held_to_its_strength_limit() {
    let saturation = TALLEST_EDIFICE_RELIEF / (4.0 * CONSTRUCTIONAL_RELIEF);
    assert!(
        (VOLCANISM_SATURATION - saturation).abs() < 1e-15,
        "{saturation}"
    );
    assert_same_bits(bounded_volcanism(1e3), VOLCANISM_SATURATION);
    assert_same_bits(bounded_volcanism(f64::INFINITY), VOLCANISM_SATURATION);
    assert_same_bits(bounded_volcanism(f64::NAN), 0.0);
    assert_same_bits(bounded_volcanism(-1.0), 0.0);
    assert_same_bits(bounded_volcanism(0.5), 0.5);
    // A massive old stagnant lid erupting for 10 Gyr saturates.
    let age = 10e9;
    let big = VolcanismInputs {
        crust: Crust::Silicate,
        surface_age: Years::new(9.9e9),
        age: Years::new(age),
        surface_temperature: Kelvin::new(266.0),
        surface_gravity: MetresPerSecondSquared::new(12.6),
        radius: Metres::new(8.12e6),
        iron_share: 0.3,
        heat_flow_at: |t: Years| WattsPerSquareMetre::new(0.08 - 0.065 * t.value() / age),
    };
    assert_same_bits(volcanism_level(&big), VOLCANISM_SATURATION);
}

/// The volcanism level within the `σ_h` calibrations' bounds (science-p14-t24b-laws §4.1 and
/// §4.5): the Moon at most 0.11, Mercury at most 0.05 and Ceres, ice-rich, 0; Venus's and Earth's
/// from the law put their `σ_h` within 5% of 0.94 and 2.51 km.
#[test]
fn the_volcanism_level_keeps_the_calibrations() {
    let table = figures_table();
    for (name, _, figures) in &table {
        eprintln!(
            "{name}: V {:.4}, σ_h {:.0} m (structural {:.0}, basin {:.0}, constructional {:.0}), \
             f_c {:.3}, {:?}, {:?}",
            figures.volcanism(),
            figures.sigma_h().value(),
            figures.relief().structural().value(),
            figures.relief().basin().value(),
            figures.relief().constructional().value(),
            figures.continental_fraction(),
            figures.tectonic_regime(),
            figures.impactors().crust(),
        );
    }
    let moon = the_giant_impact_moon();
    eprintln!(
        "the generated Moon: V {:.4}, σ_h {:.0} m",
        moon.volcanism(),
        moon.sigma_h().value()
    );
    let v = |name: &str| figures_of(&table, name).volcanism();
    assert!(v("Moon") <= 0.11, "{}", v("Moon"));
    assert!(moon.volcanism() <= 0.11, "{}", moon.volcanism());
    assert!(v("Mercury") <= 0.05, "{}", v("Mercury"));
    let ceres = wet_ceres();
    assert_eq!(ceres.impactors().crust(), Crust::IceRich);
    assert_same_bits(ceres.volcanism(), 0.0);
    let sigma = |name: &str| figures_of(&table, name).sigma_h().value();
    assert!(
        (sigma("Earth") / 2_510.0 - 1.0).abs() < 0.05,
        "{}",
        sigma("Earth")
    );
    assert!(
        (sigma("Venus") / 940.0 - 1.0).abs() < 0.05,
        "{}",
        sigma("Venus")
    );
    // The ruling's Venus and Earth, about 1.05 and 1.23.
    assert!((0.5..2.0).contains(&v("Venus")), "{}", v("Venus"));
    assert!((0.6..2.5).contains(&v("Earth")), "{}", v("Earth"));
}

/// `σ_h` on the Solar System table (P14.T48.a's test): Earth 2.51 km at `f_c` 0.405, Venus, the
/// Moon, Mars and Ceres their calibrating values within 5%, and Mercury, the one body out of the
/// sample, within 25%; no figure published is a greatest relief.
#[test]
fn the_table_s_relief_is_its_calibrations() {
    let table = figures_table();
    let earth = figures_of(&table, "Earth");
    assert_same_bits(earth.continental_fraction(), EARTH_CONTINENTAL_FRACTION);
    assert!((earth.relief().structural().value() - 2_507.5).abs() < 1.0);
    for (name, measured, within) in [
        ("Earth", 2_510.0, 0.05),
        ("Venus", 940.0, 0.05),
        ("Moon", 2_239.0, 0.05),
        ("Mars", 2_900.0, 0.05),
        ("Mercury", 1_048.0, 0.25),
    ] {
        let sigma = figures_of(&table, name).sigma_h().value();
        assert!(
            (sigma / measured - 1.0).abs() < within,
            "{name}: {sigma} m against {measured}"
        );
    }
    // Ceres calibrates `k_comp` on its ice-rich crust (Dawn's 2.077 km by R09's fit).
    let ceres = wet_ceres().sigma_h().value();
    assert!((ceres / 2_077.0 - 1.0).abs() < 0.05, "Ceres: {ceres} m");
    // Mars's relief exceeds Earth's (P14.T24.b's test).
    assert!(figures_of(&table, "Mars").sigma_h() > earth.sigma_h());
}

/// The basin share is capped at 0.02 R, the cap Vesta calibrates: a Vesta of 262.7 km and
/// 0.25 m s⁻² at saturation reaches it, and the Moon is far under it.
#[test]
fn the_basin_share_is_capped_at_a_fiftieth_of_the_radius() {
    let vesta = Relief::of(&ReliefInputs {
        regime: TectonicRegime::StagnantLid,
        continental_fraction: 0.0,
        crust: Crust::Silicate,
        surface_gravity: MetresPerSecondSquared::new(0.25),
        radius: Metres::new(262.7e3),
        crater_density: PerSquareKilometre::new(0.1),
        volcanism: 0.0,
        lithosphere: Metres::new(1e6),
    });
    assert_same_bits(vesta.basin().value(), 0.02 * 262.7e3);
    assert!(
        (vesta.sigma_h().value() - (math::powi(900.0, 2) + math::powi(5_254.0, 2)).sqrt()).abs()
            < 1e-6
    );
    let table = figures_table();
    let moon = figures_of(&table, "Moon").relief();
    assert!(moon.basin().value() < 0.02 * 1_737.4e3 / 10.0);
}

/// Each share follows its form (P14.T48.a): the structural share has no gravity term, the basin
/// share grows as √N to saturation and falls as 1 ÷ g, an ice-rich crust's is 0.16 of a silicate
/// one's, and the constructional share is gated by the lithosphere to 70 km.
#[test]
fn the_shares_follow_their_forms() {
    let base = ReliefInputs {
        regime: TectonicRegime::StagnantLid,
        continental_fraction: 0.0,
        crust: Crust::Silicate,
        surface_gravity: MetresPerSecondSquared::new(1.62),
        radius: Metres::new(1.737e6),
        crater_density: PerSquareKilometre::new(0.0045),
        volcanism: 1.0,
        lithosphere: Metres::new(35e3),
    };
    let relief = Relief::of(&base);
    assert_same_bits(relief.structural().value(), 900.0);
    assert!((relief.basin().value() - 2_050.0 * 0.5).abs() < 1e-9);
    let constructional = 2_760.0 * (3.71 / 1.62) * 0.5;
    assert!((relief.constructional().value() - constructional).abs() < 1e-9);
    let heavier = Relief::of(&ReliefInputs {
        surface_gravity: MetresPerSecondSquared::new(3.24),
        ..base
    });
    assert_same_bits(heavier.structural().value(), 900.0);
    assert!((heavier.basin().value() * 2.0 - relief.basin().value()).abs() < 1e-9);
    let icy = Relief::of(&ReliefInputs {
        crust: Crust::IceRich,
        ..base
    });
    assert!((icy.basin().value() - 0.16 * relief.basin().value()).abs() < 1e-9);
    let saturated = Relief::of(&ReliefInputs {
        crater_density: PerSquareKilometre::new(1.0),
        ..base
    });
    assert!((saturated.basin().value() - 2_050.0).abs() < 1e-9);
    let held = Relief::of(&ReliefInputs {
        lithosphere: Metres::new(200e3),
        ..base
    });
    assert!((held.constructional().value() - 2.0 * constructional).abs() < 1e-9);
    // On a mobile lid the structural share is Earth2014's mixture, 2.51 km at `f_c` = 0.405, and
    // the two populations' mixture has σ_h² as its variance.
    let earth = Relief::of(&ReliefInputs {
        regime: TectonicRegime::MobileLid,
        continental_fraction: 0.405,
        ..base
    });
    assert!((earth.structural().value() - 2_507.5).abs() < 1.0);
    let Hypsometry::TwoPopulations(populations) = earth.hypsometry() else {
        panic!("a mobile lid has two populations");
    };
    let mean = populations
        .iter()
        .fold(0.0, |sum, p| sum + p.weight * p.mean.value());
    let variance = populations.iter().fold(0.0, |sum, p| {
        sum + p.weight * (math::powi(p.width.value(), 2) + math::powi(p.mean.value(), 2))
    });
    assert!(mean.abs() < 1e-9, "{mean}");
    assert!((variance.sqrt() / earth.sigma_h().value() - 1.0).abs() < 1e-12);
}

/// The lithosphere: a silicate crust's k (870 K − `T_s`) ÷ F, an icy one's ice form, (a ÷ F)
/// ln(`T_ice` ÷ `T_s`), each zero at or above its isotherm (P14.T48.a's test).
#[test]
fn the_lithosphere_follows_its_crust() {
    let f = WattsPerSquareMetre::new(0.02);
    let silicate = lithosphere_thickness(Crust::Silicate, Kelvin::new(210.0), f);
    assert!((silicate.value() - 3.1 * 660.0 / 0.02).abs() < 1e-6);
    let ice = lithosphere_thickness(Crust::IceRich, Kelvin::new(100.0), f);
    assert!((ice.value() - 651.0 * math::ln(1.7) / 0.02).abs() < 1e-6);
    for (crust, at) in [(Crust::Silicate, 870.0), (Crust::IceRich, 170.0)] {
        assert_same_bits(
            lithosphere_thickness(crust, Kelvin::new(at), f).value(),
            0.0,
        );
        assert_same_bits(
            lithosphere_thickness(crust, Kelvin::new(at + 100.0), f).value(),
            0.0,
        );
        let below = lithosphere_thickness(crust, Kelvin::new(at - 1e-9), f).value();
        assert!((0.0..1e-3).contains(&below), "{crust:?}: {below}");
    }
    assert!(
        lithosphere_thickness(
            Crust::Silicate,
            Kelvin::new(288.0),
            WattsPerSquareMetre::new(0.0)
        )
        .value()
        .is_infinite()
    );
}

/// The continental fraction (P14.T48.a's tests): zero before `t_p`, on every lid but a mobile one
/// and on a world that never entered the window; rising monotonically and continuously to f∞.
#[test]
fn the_continental_fraction_grows_to_its_asymptote() {
    let grown = |window| Continents::Grown {
        asymptote: 0.6,
        window,
    };
    let entered = grown(WindowEntry::Entered);
    let at = |continents, regime, gyr: f64| {
        continental_fraction(continents, regime, Years::new(gyr * 1e9))
    };
    let mobile = TectonicRegime::MobileLid;
    assert_same_bits(at(entered, mobile, 0.5), 0.0);
    assert_same_bits(at(entered, mobile, 0.4), 0.0);
    assert_same_bits(at(grown(WindowEntry::Never), mobile, 4.5), 0.0);
    for regime in [
        TectonicRegime::StagnantLid,
        TectonicRegime::HeatPipe,
        TectonicRegime::Episodic,
        TectonicRegime::IceShell,
    ] {
        assert_same_bits(at(entered, regime, 4.5), 0.0);
        assert_same_bits(at(TABLE_CONTINENTS, regime, 4.5), 0.0);
    }
    let mut last = 0.0;
    for step in 0..=1_000 {
        let gyr = 0.5 + 0.02 * f64::from(step);
        let f = at(entered, mobile, gyr);
        assert!(f >= last && f < 0.6, "{gyr} Gyr: {f}");
        assert!(f - last < 0.6 * 0.02 / 1.1 + 1e-12, "{gyr} Gyr: {f}");
        last = f;
    }
    assert!((last - 0.6).abs() < 1e-7, "{last}");
    // Earth's 0.405 at 4.5 Gyr is f∞ = 0.416 (R09.T0.b).
    let earth = Continents::Grown {
        asymptote: 0.416,
        window: WindowEntry::Entered,
    };
    assert!((at(earth, mobile, 4.5) - 0.405).abs() < 0.001);
    assert_same_bits(
        at(TABLE_CONTINENTS, mobile, 4.5),
        EARTH_CONTINENTAL_FRACTION,
    );
}

/// Over 10⁴ bodies the asymptotes are uniform on [0.2, 0.7] (Kolmogorov–Smirnov), each the open
/// uniform of word 0 of its `body.continents` stream and no other word.
#[test]
fn the_asymptotes_are_uniform_and_read_word_0_alone() {
    let seed = Seed::new(0x0014_0048);
    let system = SystemId::from_raw(0x0200_0800_2000_0048).unwrap();
    let mut asymptotes: Vec<f64> = (0..10_000_u16)
        .map(|i| {
            let body = BodyId::new(system, 0x0100 + i);
            let f = continental_asymptote(seed, body);
            let mut stream = Stream::open(seed, tags::BODY_CONTINENTS, ObjectKey::from(body));
            assert_same_bits(f, 0.2 + 0.5 * stream.uniform_open());
            assert_eq!(stream.position(), 1);
            f
        })
        .collect();
    let ks = ks_one_sample(&mut asymptotes, |x| ((x - 0.2) / 0.5).clamp(0.0, 1.0));
    assert_p_value("the continental asymptotes", ks.p_value, ALPHA);
    assert!(asymptotes.iter().all(|f| (0.2..0.7).contains(f)));
}

/// The volcanism level is zero on an ice-rich crust, a surface age of zero and a surface at or
/// above 870 K, and continuous in time: over presents a thousand years apart the table's Mars,
/// Earth and Venus move it by under 10⁻⁵ of itself.
#[test]
fn the_volcanism_level_is_zero_where_nothing_builds_and_continuous_in_time() {
    let table = figures_table();
    for name in ["Mars", "Earth", "Venus"] {
        let body = body_of(&table, name);
        let inputs = inputs_of(body, name);
        let figures = figures_of(&table, name);
        let base = volcanism_of(&inputs, figures);
        let level = volcanism_level(&base);
        assert!(level > 0.0, "{name}");
        let later = volcanism_level(&VolcanismInputs {
            age: Years::new(inputs.age.value() + 1e3),
            ..volcanism_of(&inputs, figures)
        });
        assert!(
            (later / level - 1.0).abs() < 1e-5,
            "{name}: {level} to {later}"
        );
        let icy = volcanism_level(&VolcanismInputs {
            crust: Crust::IceRich,
            ..volcanism_of(&inputs, figures)
        });
        assert_same_bits(icy, 0.0);
        let fresh = volcanism_level(&VolcanismInputs {
            surface_age: Years::ZERO,
            ..volcanism_of(&inputs, figures)
        });
        assert_same_bits(fresh, 0.0);
        let molten = volcanism_level(&VolcanismInputs {
            surface_temperature: Kelvin::new(870.0),
            ..volcanism_of(&inputs, figures)
        });
        assert_same_bits(molten, 0.0);
    }
}

/// `σ_h` is continuous in time (P14.T48.a's test): the table's Earth, Mars and Moon, a thousand
/// years later, move it by under 10⁻⁵ of itself.
#[test]
fn sigma_h_is_continuous_in_time() {
    let table = figures_table();
    for name in ["Earth", "Mars", "Moon", "Venus", "Ceres"] {
        let body = body_of(&table, name);
        let inputs = inputs_of(body, name);
        let now = figures_from(&inputs).unwrap();
        let later = figures_from(&FiguresInputs {
            age: Years::new(inputs.age.value() + 1e3),
            ..inputs
        })
        .unwrap();
        let (a, b) = (now.sigma_h().value(), later.sigma_h().value());
        assert!((b / a - 1.0).abs() < 1e-5, "{name}: {a} to {b}");
    }
}
