//! P14.T48.d's climate regime on the Solar System table and on its laws' cases.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::solar::{
    PLANETS, SOLAR_AGE, derive_historic, placed_about_the_sun, solar_disc,
};
use crate::planetary::hooks::figures::cover::PERENNIAL_ICE_TEMPERATURE;
use crate::planetary::hooks::figures::{VolatileHistory, table_figures};
use crate::planetary::hooks::surface::tests::{conditions_of, inputs_of, spin_of, table};
use crate::planetary::hooks::surface::{
    Column, OceanWeight, SeasonalSpin, SurfaceInputs, ZonalProfile, climate_range, day_and_night,
    diurnal_response, heat_redistribution, night_floor, specific_heat_j_per_kg_k,
};
use crate::substance::heat_capacity_over_r;
use crate::units::consts::{SECONDS_PER_DAY, SOLAR_CONSTANT_W_PER_M2, STEFAN_BOLTZMANN};
use crate::units::{Degrees, Metres, MetresPerSecondSquared, Radians};

/// The regime of the derived body `body` under its conditions `conditions`, with the ice its own
/// figures publish over them (P14.T24.b's second stage, with no volatile history) and from a
/// colder state where `from_colder`, as `hooks_at` classifies it.
pub(crate) fn regime_of(
    body: &DerivedBody,
    conditions: &SurfaceConditions,
    from_colder: bool,
) -> ClimateRegime {
    let figures = table_figures(body)
        .expect("the table's bodies have figures")
        .with_surface(conditions, VolatileHistory::NONE);
    let reservoirs = surface_reservoirs(body.atmosphere());
    ClimateRegime::of(
        conditions,
        &ClimateInputs {
            mass: body.mass(),
            reservoirs: &reservoirs,
            from_colder,
            ice_fraction: figures.ice_fraction(),
            ice_threshold: figures.ice_threshold(),
        },
    )
}

/// Pins the climate regime `regime` under `name`: its thermal regime, forcing, condensable by
/// registry key ("None" for none) and coarse model, or "None" for conditions before the second
/// stage.
pub(crate) fn pin_regime(
    w: &mut hyperion_testkit::golden::GoldenWriter,
    name: &str,
    regime: Option<ClimateRegime>,
) {
    let Some(regime) = regime else {
        w.line(&format!("{name}_climate_regime = None"));
        return;
    };
    w.line(&format!("{name}_thermal_regime = {:?}", regime.thermal()));
    w.line(&format!("{name}_forcing = {:?}", regime.forcing()));
    w.line(&format!(
        "{name}_condensable = {}",
        regime.condensable().map_or_else(
            || "None".to_owned(),
            |id| id.substance().key().as_str().to_owned()
        )
    ));
    w.line(&format!("{name}_climate_model = {:?}", regime.model()));
}

/// The regime of the table's body `name`, under its own spin.
fn table_regime(
    table: &[(&'static str, DerivedBody, SurfaceConditions)],
    name: &str,
) -> ClimateRegime {
    let (_, body, conditions) = table.iter().find(|(n, ..)| *n == name).unwrap();
    regime_of(body, conditions, false)
}

/// The registry key of a condensable, or "none".
fn key_of(condensable: Option<SubstanceId>) -> String {
    condensable.map_or_else(
        || "none".to_owned(),
        |id| id.substance().key().as_str().to_owned(),
    )
}

#[test]
fn the_table_s_bodies_take_their_regimes() {
    let table = table();
    for (name, body, conditions) in &table {
        let regime = regime_of(body, conditions, false);
        eprintln!(
            "{name}: {:?}, {:?}, {}, {:?} (ε {:.4}, storage {:?}, transport {:.3})",
            regime.thermal(),
            regime.forcing(),
            key_of(regime.condensable()),
            regime.model(),
            conditions.air.redistribution,
            conditions.air.diurnal_storage,
            conditions.air.meridional_transport,
        );
    }
    // Earth takes the seasonal energy-balance model with water.
    let earth = table_regime(&table, "Earth");
    assert_eq!(earth.thermal(), ThermalRegime::Efficient);
    assert_eq!(earth.forcing(), Forcing::Seasonal);
    assert_eq!(earth.condensable(), Some(Gas::Water.substance()));
    assert_eq!(earth.model(), ClimateModelKind::EnergyBalance);
    // Venus the isothermal surface, with no condensable: its carbon dioxide is supercritical and
    // its trace of water above its critical point.
    let venus = table_regime(&table, "Venus");
    assert_eq!(venus.thermal(), ThermalRegime::Efficient);
    assert_eq!(venus.forcing(), Forcing::SlowRotator);
    assert_eq!(venus.condensable(), None);
    assert_eq!(venus.model(), ClimateModelKind::Isothermal);
    // The Moon radiative equilibrium: airless, turning once a month under the Sun.
    let moon = table_regime(&table, "Moon");
    assert_eq!(moon.thermal(), ThermalRegime::AirlessLike);
    assert_eq!(moon.forcing(), Forcing::SlowRotator);
    assert_eq!(moon.condensable(), None);
    assert_eq!(moon.model(), ClimateModelKind::RadiativeEquilibrium);
    // Titan takes methane, which its surface holds a reservoir of, though its air is undersaturated
    // over the whole of its narrow range; its nitrogen does not condense at 94 K and 1.5 bar.
    let titan = table_regime(&table, "Titan");
    assert_eq!(titan.condensable(), Some(Gas::Methane.substance()));
    // Every airless body is airless-like, in radiative equilibrium, with no condensable.
    for (name, body, conditions) in &table {
        if body.atmosphere().state() == SurfaceState::Airless {
            let regime = regime_of(body, conditions, false);
            assert_eq!(regime.thermal(), ThermalRegime::AirlessLike, "{name}");
            assert_eq!(
                regime.model(),
                ClimateModelKind::RadiativeEquilibrium,
                "{name}"
            );
            assert_eq!(regime.condensable(), None, "{name}");
        }
    }
}

#[test]
fn a_locked_temperate_world_takes_the_body_fixed_model() {
    let table = table();
    let (_, earth, _) = table.iter().find(|(n, ..)| *n == "Earth").unwrap();
    let locked = conditions_of(
        earth,
        SeasonalSpin {
            solar_day: None,
            ..spin_of("Earth")
        },
    )
    .unwrap();
    let regime = regime_of(earth, &locked, false);
    eprintln!("a locked Earth: {regime:?}");
    assert_eq!(regime.forcing(), Forcing::Locked);
    assert_eq!(regime.model(), ClimateModelKind::LockedEnergyBalance);
    assert_eq!(regime.condensable(), Some(Gas::Water.substance()));
    // Its air redistributes as Koll's scaling has it, with no rotation to damp its contrast.
    assert_eq!(locked.air.diurnal_storage, None);
    assert_ne!(regime.thermal(), ThermalRegime::AirlessLike);
}

#[test]
fn the_slow_rotator_onset_lies_inside_yang_et_al_s_brackets() {
    let day = SECONDS_PER_DAY;
    let flux = |s: f64| WattsPerSquareMetre::new(s * SOLAR_CONSTANT_W_PER_M2);
    let onset = |s: f64| slow_rotator_onset(flux(s)).value() / day;
    // The plan's brackets, in sidereal rotation period at 1.40 and 1.92 S⊕.
    let brackets = [(1.40, 8.0, 16.0), (1.92, 32.0, 48.0)];
    for (s, low, high) in brackets {
        eprintln!("the onset at {s} S⊕: {:.2} d", onset(s));
        assert!(onset(s) > low && onset(s) < high, "{s}: {}", onset(s));
    }
    // At the source's own fluxes, 1,900 and 2,615 W m⁻², each bracket's geometric mean.
    for &(at, low, high) in &SLOW_ROTATOR_BRACKETS {
        let mean = (low.value() * high.value()).sqrt();
        let got = slow_rotator_onset(at).value() / day;
        assert!(
            (got / mean - 1.0).abs() < 1e-12,
            "{at:?}: {got} against {mean}"
        );
    }
    let mut last = 0.0;
    for step in 0..=200 {
        let s = 0.5 + 0.01 * f64::from(step);
        assert!(onset(s) >= last, "{s}");
        last = onset(s);
    }
    assert!(onset(1.6) > onset(1.5));
    // An Earth at each bracket's flux, turning at its ends at Yang et al.'s 225 d orbit, is a fast
    // rotator at the bracket's foot and a slow one at its head.
    let table = table();
    let (_, earth, _) = table.iter().find(|(n, ..)| *n == "Earth").unwrap();
    let orbit = 225.0 * day;
    for (s, low, high) in brackets {
        for (days, expected) in [(low, Forcing::Seasonal), (high, Forcing::SlowRotator)] {
            let rotation = days * day;
            let solar = 1.0 / (1.0 / rotation - 1.0 / orbit);
            let spin = SeasonalSpin {
                obliquity: Radians::from(Degrees::new(23.44)),
                rotation_period: Seconds::new(rotation),
                solar_day: Some(Seconds::new(solar)),
            };
            let conditions = SurfaceConditions::derive(&SurfaceInputs {
                stellar_flux: flux(s),
                ..inputs_of(earth, spin)
            })
            .unwrap();
            assert_eq!(
                regime_of(earth, &conditions, false).forcing(),
                expected,
                "{s} S⊕ at {days} d"
            );
        }
    }
}

/// An Earth moved out from the Sun to `a_au`, about the present Sun at its age, under the batch's
/// carbon and oxygen rules.
fn earth_at(a_au: f64) -> DerivedBody {
    let disc = solar_disc();
    let &(_, kg, km, _, e) = PLANETS.iter().find(|(n, ..)| *n == "Earth").unwrap();
    let placed = placed_about_the_sun(&disc, (kg, km, a_au, e));
    derive_historic(&placed, &disc, SOLAR_AGE)
}

#[test]
fn a_ninety_degree_world_started_cold_has_an_equatorial_belt_and_one_started_warm_has_none() {
    // An Earth at 1.1 au, tilted to 90°: a temperate world at 270.8 K, whose equator's annual mean,
    // 262.7 K, lies under the perennial ice edge and whose poles, at 286.2 K, lie above it. It is
    // a test of the model's own profile: at 0.83 S⊕ it lies under the fluxes at which Kilic et
    // al. 2018 found their belts (0.93–1.10 of the solar constant).
    let spin = SeasonalSpin {
        obliquity: Radians::from(Degrees::new(90.0)),
        ..spin_of("Earth")
    };
    let body = earth_at(1.1);
    let conditions = conditions_of(&body, spin).unwrap();
    let figures = table_figures(&body)
        .unwrap()
        .with_surface(&conditions, VolatileHistory::NONE);
    eprintln!(
        "an Earth at 1.1 au tilted to 90°: {:?}, T_s {:.1} K, equator {:.1} K, pole {:.1} K, ice {:.3}",
        body.atmosphere().state(),
        conditions.mean_surface_temperature().value(),
        conditions.zonal_temperature(0.0).value(),
        conditions.zonal_temperature(1.0).value(),
        figures.ice_fraction(),
    );
    assert_eq!(body.atmosphere().state(), SurfaceState::Temperate);
    assert!(conditions.zonal_temperature(0.0) < PERENNIAL_ICE_TEMPERATURE);
    assert!(conditions.zonal_temperature(1.0) > PERENNIAL_ICE_TEMPERATURE);
    assert!(figures.ice_fraction() > 0.0 && figures.ice_fraction() < 1.0);
    let cold = regime_of(&body, &conditions, true);
    let warm = regime_of(&body, &conditions, false);
    assert_eq!(cold.forcing(), Forcing::IceBelt);
    assert_eq!(warm.forcing(), Forcing::Seasonal);
    // The belt is the profile's cold equator, with no extra hysteresis: the cover is the same
    // either way (science-p14-t24b-laws §4.3), and only the forcing reads the history.
    assert_eq!(cold.thermal(), warm.thermal());
    assert_eq!(cold.condensable(), warm.condensable());
    assert_eq!(cold.model(), warm.model());
    // A cover whose threshold lies above the poles' annual mean ices the poles too: caps and a
    // band together are no belt, whatever the history.
    let reservoirs = surface_reservoirs(body.atmosphere());
    let iced_poles = ClimateRegime::of(
        &conditions,
        &ClimateInputs {
            mass: body.mass(),
            reservoirs: &reservoirs,
            from_colder: true,
            ice_fraction: 0.5,
            ice_threshold: Kelvin::new(290.0),
        },
    );
    assert_eq!(iced_poles.forcing(), Forcing::Seasonal);
    // At Earth's own obliquity the same world, started cold, has caps and no belt.
    let upright = conditions_of(&body, spin_of("Earth")).unwrap();
    assert_eq!(
        regime_of(&body, &upright, true).forcing(),
        Forcing::Seasonal
    );
}

/// The measured Mars, by hand, as P14.T24.a's test of it takes it: 6.1 mbar of CO₂ at 210 K,
/// 586.2 W m⁻² at a Bond albedo of 0.25, 3.71 m s⁻², 3,389.5 km and 43.34 g mol⁻¹ (NASA's Mars
/// fact sheet before its revision of 2025, which gives 6.36 mbar, about 214 K, 3.73 m s⁻² and
/// 43.49 g mol⁻¹; no outcome here depends on the difference), and its 25.19° and 24.62 h.
fn measured_mars() -> Column {
    let co2 = Gas::CarbonDioxide.substance();
    let mu = 43.34;
    let cp = heat_capacity_over_r(&[(co2, 1.0)]).unwrap();
    let equilibrium = math::powf(0.75 * 586.2 / 4.0 / STEFAN_BOLTZMANN, 0.25);
    let (pressure, gravity) = (610.0, 3.71);
    Column {
        surface_temperature: Kelvin::new(210.0),
        equilibrium_temperature: Kelvin::new(equilibrium),
        surface_pressure: Pascals::new(pressure),
        air_mass_kg_per_m2: pressure / gravity,
        specific_heat_j_per_kg_k: specific_heat_j_per_kg_k(cp, mu),
        molar_mass_g_per_mol: mu,
        surface_gravity: MetresPerSecondSquared::new(gravity),
        optical_depth: (math::powi(210.0 / equilibrium, 4) - 1.0) / 0.75,
        greenhouse_gain: 0.0,
        radius: Metres::new(3_389.5e3),
    }
}

#[test]
fn a_mars_given_its_measured_air_by_hand_takes_carbon_dioxide_not_water() {
    let mars = measured_mars();
    let spin = spin_of("Mars");
    let floor = night_floor(WattsPerSquareMetre::new(0.019));
    let zonal = ZonalProfile::of(
        &mars,
        &spin,
        OceanWeight::NONE,
        crate::planetary::derive::atmosphere::SnapshotRules::Held,
    );
    let (day, night) = day_and_night(&mars, &spin, SurfaceMaterial::Rock, floor);
    let range = climate_range(mars.surface_temperature, zonal.ends, day, night, floor);
    eprintln!(
        "the measured Mars's range: [{:.1}, {:.1}] K",
        range.0.value(),
        range.1.value()
    );
    // Its air by number (NASA's fact sheet, with its 210 ppm of water), and its polar caps' water
    // and carbon dioxide ices.
    let (co2, water) = (Gas::CarbonDioxide.substance(), Gas::Water.substance());
    let gases = [
        (co2, 0.951),
        (Gas::Nitrogen.substance(), 0.0259),
        (Gas::Argon.substance(), 0.0194),
        (Gas::Oxygen.substance(), 0.0016),
        (water, 2.1e-4),
    ];
    let pressure = mars.surface_pressure;
    assert_eq!(
        climate_condensable(&gases, pressure, range, &[water, co2]),
        Some(co2)
    );
    // Its carbon dioxide freezes out near 148 K, below its orbit-mean climate's coldest: it is a
    // condensable by its caps alone, and its water's vapour changes phase inside the range.
    let frost = saturation_pressure(co2, range.0).unwrap();
    assert!(frost.value() > 0.951 * pressure.value(), "{frost:?}");
    assert_eq!(
        climate_condensable(&gases, pressure, range, &[]),
        Some(water)
    );
    // It is a thin air: transitional, its meridional transport under its restoring, so in
    // radiative equilibrium, as rendering plan R09's reference Mars takes it.
    let response = diurnal_response(&mars, &spin, SurfaceMaterial::Rock, floor);
    let air = AirTransport {
        redistribution: heat_redistribution(&mars),
        diurnal_storage: response.and_then(|r| r.storage),
        meridional_transport: zonal.transport_over_restoring(),
    };
    let thermal = thermal_of(&air, || false);
    eprintln!("the measured Mars: {air:?}, {thermal:?}");
    assert_eq!(thermal, ThermalRegime::Transitional);
    assert!(air.meridional_transport < 1.0);
    assert_eq!(
        model_of(thermal, Forcing::Seasonal, air.meridional_transport, range),
        ClimateModelKind::RadiativeEquilibrium
    );
}

#[test]
fn the_condensable_is_the_largest_partial_pressure_by_registry_order() {
    let (water, methane, nitrogen, co2) = (
        Gas::Water.substance(),
        Gas::Methane.substance(),
        Gas::Nitrogen.substance(),
        Gas::CarbonDioxide.substance(),
    );
    let range = (Kelvin::new(90.0), Kelvin::new(95.0));
    let pressure = Pascals::new(1.5e5);
    // A Titan-like air: nitrogen does not condense at 1.4 bar in 90–95 K, and methane's reservoir
    // makes it the condensable.
    let titan = [(nitrogen, 0.94), (methane, 0.06)];
    assert_eq!(climate_condensable(&titan, pressure, range, &[]), None);
    assert_eq!(
        climate_condensable(&titan, pressure, range, &[methane]),
        Some(methane)
    );
    // Equal partial pressures, both reservoirs: the earlier row, water, wins.
    let even = [(water, 0.5), (co2, 0.5)];
    assert_eq!(
        climate_condensable(&even, pressure, range, &[co2, water]),
        Some(water)
    );
    // A row the air does not hold is none, whatever the surface holds.
    assert_eq!(
        climate_condensable(&[], Pascals::ZERO, range, &[water]),
        None
    );
    // The range's ends are inclusive: a partial pressure at the coldest's saturation pressure is
    // a candidate.
    let at = saturation_pressure(nitrogen, range.0).unwrap();
    let edge = [(nitrogen, 1.0)];
    assert_eq!(climate_condensable(&edge, at, range, &[]), Some(nitrogen));
    assert_eq!(
        climate_condensable(
            &edge,
            Pascals::new(at.value() * 0.999),
            (range.0, range.0),
            &[]
        ),
        None
    );
}

#[test]
fn the_reservoirs_are_the_gases_held_at_their_caps() {
    let table = table();
    for (name, body, _) in &table {
        let reservoirs = surface_reservoirs(body.atmosphere());
        eprintln!(
            "{name}: {:?}",
            reservoirs
                .iter()
                .map(|id| id.substance().key().as_str().to_owned())
                .collect::<Vec<_>>()
        );
    }
    let found = |name: &str| {
        let (_, body, _) = table.iter().find(|(n, ..)| *n == name).unwrap();
        surface_reservoirs(body.atmosphere())
    };
    assert!(found("Earth").contains(&Gas::Water.substance()));
    assert!(found("Titan").contains(&Gas::Methane.substance()));
    assert!(!found("Titan").contains(&Gas::Nitrogen.substance()));
    assert_eq!(found("Venus"), []);
}

/// An air of mole fractions `gases` at `pressure` Pa over a night at `night` K, its surface holding
/// reservoirs of `reservoirs`.
fn night_air<'a>(
    gases: &'a [(SubstanceId, f64)],
    pressure: f64,
    night: f64,
    reservoirs: &'a [SubstanceId],
) -> NightAir<'a> {
    NightAir {
        gases,
        pressure: Pascals::new(pressure),
        reservoirs,
        night: Kelvin::new(night),
    }
}

#[test]
fn the_collapse_pressure_is_wordsworth_s_eq_43() {
    // His abstract: "at Earth's stellar flux, the critical pressures are about five times higher
    // (0.14 bar) than previous gray gas estimates".
    let flux = WattsPerSquareMetre::new(1_366.0);
    let earth = carbon_dioxide_collapse_pressure(flux, EarthMasses::new(1.0)).unwrap();
    let by_hand = 1_366.0_f64;
    let log = ((-0.84e-10 * by_hand + 0.73e-6) * by_hand - 0.0022) * by_hand + 6.01;
    assert!((earth.value() / math::exp10(log) - 1.0).abs() < 1e-12);
    assert!((earth.value() / 1.4e4 - 1.0).abs() < 0.05, "{earth:?}");
    let heavy = carbon_dioxide_collapse_pressure(flux, EarthMasses::new(10.0)).unwrap();
    let between =
        carbon_dioxide_collapse_pressure(flux, EarthMasses::new(10.0_f64.sqrt())).unwrap();
    assert!(heavy > between && between > earth);
    assert!(
        (math::log10(between.value())
            - f64::midpoint(math::log10(earth.value()), math::log10(heavy.value())))
        .abs()
            < 1e-12
    );
    // It falls with flux over the fit, and holds only inside it.
    let at = |f: f64| {
        carbon_dioxide_collapse_pressure(WattsPerSquareMetre::new(f), EarthMasses::new(1.0))
    };
    assert!(
        at(300.0).unwrap() > at(1_366.0).unwrap() && at(1_366.0).unwrap() > at(4_000.0).unwrap()
    );
    assert_eq!(at(200.0), None);
    assert_eq!(at(5_000.0), None);
    assert_eq!(
        carbon_dioxide_collapse_pressure(flux, EarthMasses::new(0.9)),
        None
    );
    assert_eq!(
        carbon_dioxide_collapse_pressure(flux, EarthMasses::new(11.0)),
        None
    );
    // A pure CO₂ air on a locked Earth collapses below it and stands above it.
    let co2 = [(Gas::CarbonDioxide.substance(), 1.0)];
    let mass = EarthMasses::new(1.0);
    let air = night_air;
    assert!(collapsed(&air(&co2, 1e4, 200.0, &[]), flux, mass));
    assert!(!collapsed(&air(&co2, 2e4, 200.0, &[]), flux, mass));
    // Outside the fit the bulk gas's own condensation at the night's temperature decides: a
    // nitrogen air of half a bar freezes out on a 60 K night and stands on a 90 K one.
    let nitrogen = [(Gas::Nitrogen.substance(), 1.0)];
    assert!(collapsed(&air(&nitrogen, 5e4, 60.0, &[]), flux, mass));
    assert!(!collapsed(&air(&nitrogen, 5e4, 90.0, &[]), flux, mass));
    // Over its own reservoir the same air is held at its saturation pressure, and its night's frost
    // is its cycle, not a collapse.
    let held = [Gas::Nitrogen.substance()];
    assert!(!collapsed(&air(&nitrogen, 5e4, 60.0, &held), flux, mass));
    // A collapsed air in Koll's own regime is airless-like.
    let thick = AirTransport {
        redistribution: 0.9,
        diurnal_storage: None,
        meridional_transport: 2.0,
    };
    assert_eq!(thermal_of(&thick, || true), ThermalRegime::AirlessLike);
    assert_eq!(thermal_of(&thick, || false), ThermalRegime::Transitional);
}

#[test]
fn the_thermal_thresholds_are_koll_s_factor_s() {
    let f = |ratio: f64| 2.0 / 3.0 - 5.0 / 12.0 * (ratio / (1.0 + ratio));
    // Airless-like within 5% of a bare rock's 2/3, efficient within 10% of a uniform planet's ¼.
    assert!((f(KOLL_AIRLESS_LIKE_RATIO) / (2.0 / 3.0) - 0.95).abs() < 1e-4);
    assert!((f(KOLL_EFFICIENT_RATIO) / 0.25 - 1.1).abs() < 1e-3);
    assert_same_bits(ISOTHERMAL_FLUX_SPREAD, f(KOLL_EFFICIENT_RATIO) / 0.25 - 1.0);
    let epsilon = |ratio: f64| ratio / (1.0 + ratio);
    let at = |ratio: f64| by_redistribution(epsilon(ratio));
    assert_eq!(
        at(KOLL_AIRLESS_LIKE_RATIO * 0.99),
        ThermalRegime::AirlessLike
    );
    assert_eq!(
        at(KOLL_AIRLESS_LIKE_RATIO * 1.01),
        ThermalRegime::Transitional
    );
    assert_eq!(at(KOLL_EFFICIENT_RATIO * 0.99), ThermalRegime::Transitional);
    assert_eq!(at(KOLL_EFFICIENT_RATIO * 1.01), ThermalRegime::Efficient);
    assert_eq!(by_redistribution(f64::NAN), ThermalRegime::AirlessLike);
    // A locked world's is Koll's own; a rotator's storage damps its day–night contrast, which
    // raises its redistribution, and past Koll's regime no collapse is read.
    let locked = AirTransport {
        redistribution: 0.05,
        diurnal_storage: None,
        meridional_transport: 0.1,
    };
    assert_same_bits(effective_redistribution(&locked), 0.05);
    assert_eq!(thermal_of(&locked, || false), ThermalRegime::AirlessLike);
    let spun = AirTransport {
        diurnal_storage: Some(100.0),
        ..locked
    };
    assert_eq!(thermal_of(&spun, || true), ThermalRegime::Efficient);
}

#[test]
fn the_thermal_regime_is_continuous_across_koll_s_validity() {
    // "main"'s ruling of 2026-10-13: one effective redistribution, 1 − κ(1 − ε), on both sides of
    // ω C (1 − ε) ÷ B_P = 1, where only the collapse test switches off.
    for step in 0..=1_000 {
        let epsilon = f64::from(step) * 0.001;
        let air = |storage: f64| AirTransport {
            redistribution: epsilon,
            diurnal_storage: Some(storage),
            meridional_transport: 1.0,
        };
        let (below, above) = (air(1.0 - 1e-9), air(1.0 + 1e-9));
        let jump = effective_redistribution(&above) - effective_redistribution(&below);
        assert!(jump.abs() < 1e-9, "{epsilon}: {jump}");
        // Away from a threshold, where 1 − (1 − ε) ÷ √2 meets X ÷ (k + X), the label holds across
        // the switch.
        let near = |ratio: f64| {
            ((1.0 - epsilon) - core::f64::consts::SQRT_2 * (1.0 - ratio / (1.0 + ratio))).abs()
                < 1e-6
        };
        if !(near(KOLL_AIRLESS_LIKE_RATIO) || near(KOLL_EFFICIENT_RATIO)) {
            assert_eq!(
                thermal_of(&below, || false),
                thermal_of(&above, || false),
                "{epsilon}"
            );
        }
        // As the storage vanishes the rotator's is the locked world's, Koll's own.
        let still = AirTransport {
            diurnal_storage: None,
            ..air(0.0)
        };
        assert!(
            (effective_redistribution(&air(1e-9)) - effective_redistribution(&still)).abs() < 1e-12
        );
    }
}

#[test]
fn the_regime_comes_with_the_second_stage() {
    let table = table();
    let (_, earth, conditions) = table.iter().find(|(n, ..)| *n == "Earth").unwrap();
    assert_eq!(conditions.climate_regime(), None);
    let figures = table_figures(earth)
        .unwrap()
        .with_surface(conditions, VolatileHistory::NONE);
    let reservoirs = surface_reservoirs(earth.atmosphere());
    let inputs = ClimateInputs {
        mass: earth.mass(),
        reservoirs: &reservoirs,
        from_colder: false,
        ice_fraction: figures.ice_fraction(),
        ice_threshold: figures.ice_threshold(),
    };
    let classified = conditions.clone().with_climate(&inputs);
    assert_eq!(
        classified.climate_regime(),
        Some(ClimateRegime::of(conditions, &inputs))
    );
    assert_eq!(&classified.without_climate(), conditions);
}

#[test]
fn the_pass_reads_the_table_s_regimes_as_its_reference_worlds_take_them() {
    use crate::planetary::surface::reference::{ceres_like, earth_like, moon_like};
    let table = table();
    for (name, reference) in [
        ("Earth", earth_like()),
        ("Moon", moon_like()),
        ("Ceres", ceres_like()),
    ] {
        let regime = table_regime(&table, name);
        assert_eq!(
            crate::planetary::surface::ClimateRegime::from(&regime),
            *reference.climate(),
            "{name}"
        );
    }
}
