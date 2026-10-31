use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::condense::NotFormed;
use crate::substance::by_key;
use crate::units::consts::EARTH_RADIUS_M;

/// Turbet et al. 2019's Earth ocean as vapour, Pa: "270 bars (Equivalent water vapour atmospheric
/// pressure if the entire water reservoir is vaporized in the atmosphere)" (their §3).
const EARTH_OCEAN_PA: f64 = 2.7e7;

/// The post-runaway Earth's insolation in Turbet et al. 2019's Fig. 2, W m⁻²: 1.07 times the
/// present Earth's, in Turbet et al. 2020's unit.
const POST_RUNAWAY_FLUX: f64 = 1.07 * 1_366.0;

/// Turbet et al. 2019's threshold surface temperature of a post-runaway Earth holding `oceans`
/// Earth oceans, K: their Fig. 3B's fit, `T_ε` = 80 (log₁₀ W)² + 700 log₁₀ W + 1,700.
fn threshold_temperature(oceans: f64) -> Kelvin {
    let w = math::log10(oceans);
    Kelvin::new(80.0 * w * w + 700.0 * w + 1_700.0)
}

/// An Earth of 9.81 m s⁻² under `oceans` Earth oceans of steam at the post-runaway insolation and
/// its threshold temperature.
fn post_runaway_earth(oceans: f64) -> SteamInflationInputs {
    SteamInflationInputs {
        water_column: Pascals::new(oceans * EARTH_OCEAN_PA),
        radius: Metres::new(EARTH_RADIUS_M),
        gravity: MetresPerSecondSquared::new(9.81),
        stellar_flux: WattsPerSquareMetre::new(POST_RUNAWAY_FLUX),
        surface_temperature: threshold_temperature(oceans),
    }
}

/// At the published points of Turbet et al.'s model, the post-runaway Earths of the 2019 paper
/// whose 1-D model the 2020 grid extends (inside that grid), the fit meets their figures within
/// 10%, the 2020 fit's largest residual in `T_eff` (Turbet et al. 2020, App. B: "mean error ∼2.5%;
/// max error ∼10%"), which Eq. 2 carries into z by R ÷ (R − L H), 1.05–1.21 here: one ocean's
/// atmosphere "from almost zero up to 500 km" (Turbet et al. 2019, §2, Fig. 2), and a net radius
/// change "from ∼ 300 km (for 0.1× Earth ocean water content ...) up to ∼ 1100 km (for 100 Earth
/// ocean water content)" (their §3, Fig. 5A) above the condensed layer, Earth's 2.7 km global
/// equivalent layer per ocean scaled linearly, which their Eq. 1 subtracts.
#[test]
fn the_transit_inflation_meets_turbet_et_al_s_post_runaway_earths() {
    for (oceans, published_m) in [(0.1, 300.0e3 + 0.27e3), (1.0, 500.0e3), (100.0, 1_370.0e3)] {
        let z = steam_transit_inflation(&post_runaway_earth(oceans))
            .expect("inside the fit's grid")
            .value();
        eprintln!(
            "{oceans} Earth oceans: {:.1} km against {:.1} km",
            z / 1e3,
            published_m / 1e3
        );
        assert!(
            (z / published_m - 1.0).abs() < 0.1,
            "{oceans}: {z} m against {published_m} m"
        );
    }
}

/// The fit is Turbet et al. 2020's Eqs. 2 and 3 with their Table 2, as an independent evaluation
/// gives it (Python's `math`), within 10⁻¹²: z at one ocean, 508,568.095 m, and `T_eff` at a water
/// fraction of 2.2 × 10⁻⁴, 9.81 m s⁻² and 1.07 Earth insolations, 511.110 K.
#[test]
fn the_transit_inflation_is_turbet_et_al_s_equations() {
    let z = steam_transit_inflation(&post_runaway_earth(1.0))
        .unwrap()
        .value();
    assert!((z / 508_568.095_419_079_5 - 1.0).abs() < 1e-12, "{z}");
    let t = steam_effective_temperature(2.2e-4, MetresPerSecondSquared::new(9.81), 1.07).value();
    assert!((t / 511.109_865_622_151_7 - 1.0).abs() < 1e-12, "{t}");
}

/// The fit is published only inside its grid, each edge included, and never for an unbound
/// atmosphere: its scale's span at or beyond the radius.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "each of the grid's eight edges, inside and outside"
)]
fn the_transit_inflation_is_not_modelled_outside_turbet_et_al_s_grid() {
    let earth = post_runaway_earth(1.0);
    let g = STANDARD_GRAVITY.value();
    let insolation = STEAM_FIT_EARTH_INSOLATION.value();
    let cases = [
        (
            "the least insolation",
            SteamInflationInputs {
                stellar_flux: WattsPerSquareMetre::new(STEAM_FIT_INSOLATION_EARTHS.0 * insolation),
                ..earth
            },
            SteamInflationInputs {
                stellar_flux: WattsPerSquareMetre::new(0.99 * insolation),
                ..earth
            },
        ),
        (
            "the most insolation",
            SteamInflationInputs {
                stellar_flux: WattsPerSquareMetre::new(STEAM_FIT_INSOLATION_EARTHS.1 * insolation),
                ..earth
            },
            SteamInflationInputs {
                stellar_flux: WattsPerSquareMetre::new(30.01 * insolation),
                ..earth
            },
        ),
        (
            "the least gravity",
            SteamInflationInputs {
                gravity: MetresPerSecondSquared::new(STEAM_FIT_GRAVITY_EARTH_G.0 * g),
                ..earth
            },
            SteamInflationInputs {
                gravity: MetresPerSecondSquared::new(0.199 * g),
                ..earth
            },
        ),
        (
            "the most gravity",
            SteamInflationInputs {
                gravity: MetresPerSecondSquared::new(STEAM_FIT_GRAVITY_EARTH_G.1 * g),
                ..earth
            },
            SteamInflationInputs {
                gravity: MetresPerSecondSquared::new(6.01 * g),
                ..earth
            },
        ),
        (
            "the least water",
            SteamInflationInputs {
                water_column: STEAM_FIT_PRESSURE.0,
                ..earth
            },
            SteamInflationInputs {
                water_column: Pascals::new(2.69e5),
                ..earth
            },
        ),
        (
            "the most water",
            SteamInflationInputs {
                water_column: STEAM_FIT_PRESSURE.1,
                ..earth
            },
            SteamInflationInputs {
                water_column: Pascals::new(2.71e9),
                ..earth
            },
        ),
        (
            "the coolest surface",
            SteamInflationInputs {
                surface_temperature: STEAM_FIT_SURFACE_TEMPERATURE.0,
                ..earth
            },
            SteamInflationInputs {
                surface_temperature: Kelvin::new(299.0),
                ..earth
            },
        ),
        (
            "the hottest surface",
            SteamInflationInputs {
                surface_temperature: STEAM_FIT_SURFACE_TEMPERATURE.1,
                ..earth
            },
            SteamInflationInputs {
                surface_temperature: Kelvin::new(4_301.0),
                ..earth
            },
        ),
    ];
    for (edge, inside, outside) in cases {
        let z = steam_transit_inflation(&inside);
        assert!(
            z.is_some_and(|z| z.value().is_finite() && z.value() > 0.0),
            "{edge}: {z:?}"
        );
        assert_eq!(steam_transit_inflation(&outside), None, "beyond {edge}");
    }
    // A small, light body under the grid's most water is unbound: its span, about 4,900 km (`T_eff`
    // 881 K, H 204 km), lies beyond its 1,000 km radius.
    let unbound = SteamInflationInputs {
        radius: Metres::new(1.0e6),
        gravity: MetresPerSecondSquared::new(2.0),
        water_column: STEAM_FIT_PRESSURE.1,
        ..earth
    };
    assert_eq!(steam_transit_inflation(&unbound), None);
    let nan = SteamInflationInputs {
        radius: Metres::new(f64::NAN),
        ..earth
    };
    assert_eq!(steam_transit_inflation(&nan), None);
}

/// The plan's quarter-water Earth's published air (P14.T53.b part 3's test, rounded): nearly pure
/// water at 2,475 bar and 1,154 K, with a hundredth of carbon dioxide, under 9.82 m s⁻² at 300 K
/// of equilibrium, its top 100 bar of grey depth 40.
fn steam_air(gases: &[(SubstanceId, f64)]) -> SupercriticalAtmosphereInputs<'_> {
    SupercriticalAtmosphereInputs {
        gases,
        surface_pressure: Pascals::new(2.475e8),
        surface_temperature: Kelvin::new(1_154.0),
        reference: (Pascals::new(1.0e7), 40.0),
        equilibrium_temperature: Kelvin::new(300.0),
        gravity: MetresPerSecondSquared::new(9.82),
    }
}

/// The steam air's gases: 0.99 of water and 0.01 of carbon dioxide by number.
fn steam_gases() -> [(SubstanceId, f64); 2] {
    [
        (by_key("H2O").expect("a row"), 0.99),
        (by_key("CO2").expect("a row"), 0.01),
    ]
}

/// A steam envelope's profile takes the air's own grey depth at its reference level, joins its
/// adiabat above its surface and meets the published surface temperature there; it is monotone in
/// pressure; every figure is finite; and no deck is based at or above its condensable's critical
/// point, water itself labelled the bulk gas, never trapped.
#[test]
fn a_steam_envelope_s_profile_meets_its_published_surface() {
    let gases = steam_gases();
    let inputs = steam_air(&gases);
    let air = EnvelopeAtmosphere::supercritical(&inputs).expect("a steam envelope's model");
    let profile = air.profile();
    let (level, depth) = inputs.reference;
    let tau = profile.grey().optical_depth(level, inputs.gravity);
    assert!((tau / depth - 1.0).abs() < 1e-12, "{tau}");
    let at_surface = profile.temperature_at(inputs.surface_pressure).value();
    assert!(
        (at_surface / inputs.surface_temperature.value() - 1.0).abs() < 1e-9,
        "{at_surface}"
    );
    assert!(profile.join_pressure().value() < inputs.surface_pressure.value());
    let t_int = air.internal_temperature().value();
    assert!(t_int.is_finite() && t_int > 0.0, "{t_int}");
    assert_same_bits(
        air.irradiation_temperature().value(),
        irradiation_temperature(inputs.equilibrium_temperature).value(),
    );
    assert_monotone(profile, inputs.surface_pressure);
    let total: f64 = air.composition().iter().map(|&(_, x)| x).sum();
    assert!((total - 1.0).abs() < 1e-9, "{total}");
    assert!(air.mean_molar_mass_g_per_mol().is_finite());
    assert!(air.heat_capacity_over_r().is_finite() && air.heat_capacity_over_r() > 0.0);
    assert_no_deck_at_or_above_the_critical_point(&air);
    let water = by_key("H2O").unwrap();
    assert!(
        air.not_formed()
            .iter()
            .any(|&(id, why)| id == water && why == NotFormed::BulkGas),
        "{:?}",
        air.not_formed()
    );
    assert_same_bits(air.haze_optical_depth(), 0.0);
    assert!(air.absorbers().is_empty(), "{:?}", air.absorbers());
    eprintln!(
        "the steam envelope: T_int {t_int:.2} K, T_irr {:.2} K, κ₁ {:.4e} m² kg⁻¹, join {:.2} bar \
         at {:.1} K, the adiabat to {:.1} bar at {:.1} K, the skin {:.1} K, {} decks, not formed \
         {:?}",
        air.irradiation_temperature().value(),
        profile.grey().thermal_opacity_m2_per_kg(),
        profile.join_pressure().value() / 1e5,
        profile.nodes()[0].temperature().value(),
        profile.nodes().last().unwrap().pressure().value() / 1e5,
        profile.nodes().last().unwrap().temperature().value(),
        profile.skin_temperature().value(),
        air.decks().len(),
        air.not_formed()
    );
}

/// The model gives the same bits twice.
#[test]
fn a_steam_envelope_s_model_is_the_same_twice() {
    let gases = steam_gases();
    let inputs = steam_air(&gases);
    assert_eq!(
        EnvelopeAtmosphere::supercritical(&inputs),
        EnvelopeAtmosphere::supercritical(&inputs)
    );
}

/// A published surface no profile meets, cooler than the irradiated isotherm at depth, and inputs
/// out of range are refused, never given a NaN.
#[test]
fn a_surface_no_profile_meets_and_inputs_out_of_range_are_refused() {
    let gases = steam_gases();
    let inputs = steam_air(&gases);
    let refused = |inputs: SupercriticalAtmosphereInputs<'_>| {
        EnvelopeAtmosphere::supercritical(&inputs).expect_err("refused")
    };
    assert_eq!(
        refused(SupercriticalAtmosphereInputs {
            surface_temperature: Kelvin::new(300.0),
            ..inputs
        }),
        DeriveSupercriticalAtmosphereError::NoProfileMeetsTheSurface
    );
    assert!(matches!(
        refused(SupercriticalAtmosphereInputs {
            surface_pressure: Pascals::new(1.0e5),
            ..inputs
        }),
        DeriveSupercriticalAtmosphereError::SurfacePressure(_)
    ));
    assert!(matches!(
        refused(SupercriticalAtmosphereInputs {
            reference: (Pascals::new(3.0e8), 40.0),
            ..inputs
        }),
        DeriveSupercriticalAtmosphereError::Reference(..)
    ));
    assert!(matches!(
        refused(SupercriticalAtmosphereInputs {
            surface_temperature: Kelvin::new(f64::NAN),
            ..inputs
        }),
        DeriveSupercriticalAtmosphereError::SurfaceTemperature(_)
    ));
    assert!(matches!(
        refused(SupercriticalAtmosphereInputs {
            gravity: MetresPerSecondSquared::new(0.0),
            ..inputs
        }),
        DeriveSupercriticalAtmosphereError::Gravity(_)
    ));
    assert!(matches!(
        refused(SupercriticalAtmosphereInputs {
            equilibrium_temperature: Kelvin::new(f64::NAN),
            ..inputs
        }),
        DeriveSupercriticalAtmosphereError::EquilibriumTemperature(_)
    ));
    // A depth whose opacity overflows is no grey profile's.
    assert!(matches!(
        refused(SupercriticalAtmosphereInputs {
            reference: (Pascals::new(1.0e7), 1e308),
            ..inputs
        }),
        DeriveSupercriticalAtmosphereError::Grey(_)
    ));
}

/// A published surface just above the irradiated isotherm needs so little internal heat that the
/// profile joins its adiabat below the surface: refused, since no adiabat lies above the ground.
#[test]
fn a_profile_that_joins_below_the_surface_is_refused() {
    let gases = steam_gases();
    let inputs = SupercriticalAtmosphereInputs {
        surface_temperature: Kelvin::new(340.0),
        ..steam_air(&gases)
    };
    assert!(
        matches!(
            EnvelopeAtmosphere::supercritical(&inputs),
            Err(DeriveSupercriticalAtmosphereError::JoinBelowTheSurface { .. })
        ),
        "{:?}",
        EnvelopeAtmosphere::supercritical(&inputs).map(|air| air.profile().join_pressure())
    );
}

/// The profile's nodes ascend in pressure and do not cool, and its temperature does not fall with
/// pressure on a grid of 400 levels evenly in ln p from 1 Pa to `surface`.
pub(crate) fn assert_monotone(profile: &EnvelopeProfile, surface: Pascals) {
    for pair in profile.nodes().windows(2) {
        assert!(
            pair[1].pressure().value() > pair[0].pressure().value(),
            "{pair:?}"
        );
        assert!(
            pair[1].temperature().value() >= pair[0].temperature().value(),
            "{pair:?}"
        );
    }
    let span = math::ln(surface.value());
    let mut last = f64::NEG_INFINITY;
    for k in 0..=400_u32 {
        let p = Pascals::new(math::exp(span * f64::from(k) / 400.0));
        let t = profile.temperature_at(p).value();
        assert!(t.is_finite() && t > 0.0, "{p:?}: {t}");
        assert!(t >= last, "{p:?}: {t} under {last}");
        last = t;
    }
}

/// No deck of `air` is based at or above its condensable's critical temperature, and each base's
/// pressure and temperature are finite and positive.
pub(crate) fn assert_no_deck_at_or_above_the_critical_point(air: &EnvelopeAtmosphere) {
    for deck in air.decks() {
        let (p, t) = (deck.base().value(), deck.base_temperature().value());
        assert!(
            p.is_finite() && p > 0.0 && t.is_finite() && t > 0.0,
            "{deck:?}"
        );
        if let Some(diagram) = deck.condensate().substance().phase_diagram() {
            assert!(
                t < diagram.critical_temperature().value(),
                "{deck:?} based at or above its critical point"
            );
        }
    }
}
