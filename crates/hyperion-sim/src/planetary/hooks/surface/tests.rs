use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::orbit::{Eccentricity, Orientation};
use crate::planetary::derive::atmosphere::tests::water_weight_at_humidity;
use crate::planetary::derive::atmosphere::{
    GreenhouseConstants, NITROGEN_REFERENCE_TEMPERATURE, VolatileDraws,
};
use crate::planetary::derive::solar::{
    PLANETS, SMALL_BODIES, SOLAR_AGE, derive_historic, historic, historic_table, orbit, solar_disc,
};
use crate::planetary::derive::{DerivedBody, PlacedBody};
use crate::planetary::frames::FrameSpin;
use crate::planetary::hooks::figures::cover::PERENNIAL_ICE_TEMPERATURE;
use crate::planetary::hooks::figures::table_figures;
use crate::stellar::draws::UnitUniform;
use crate::units::consts::SECONDS_PER_DAY;
use crate::units::{Degrees, EarthMasses, GravitationalParameter, Years};

/// The table's bodies' own spins: obliquity to the seasonal orbit (degrees), sidereal rotation
/// period and seasonal orbital period (days). NASA's planetary and satellite fact sheets, but:
/// Mercury's 58.646 d is the IAU's rotation (the fact sheet's 1,407.6 h is 58.650 d); Saturn's
/// 10,759.22 d is the fact sheet's before its 2025 update to 10,755.70 d; the Moon's 1.5424° is
/// its equator's tilt to the ecliptic (Cassini's laws; the fact sheet gives 6.68° to its orbit;
/// not re-read); a moon's obliquity is its planet's, since its spin follows its orbit in its
/// planet's equator, and its seasonal orbit is its planet's; Ceres's 4.03° and 9.074 17 h are
/// Park et al. 2016's (Nature 537, 515). Callisto, which the table does not hold, spins for the
/// moons' tests (its synchronous 16.689 d, the Jovian satellite fact sheet's, not re-read).
const SPINS: [(&str, f64, f64, f64); 9] = [
    ("Mercury", 0.034, 58.646, 87.969),
    ("Venus", 177.36, 243.025, 224.701),
    ("Earth", 23.44, 0.997_27, 365.256),
    ("Mars", 25.19, 1.025_96, 686.980),
    ("Moon", 1.542_4, 27.321_7, 365.256),
    ("Ganymede", 3.13, 7.154_6, 4_332.59),
    ("Callisto", 3.13, 16.689, 4_332.59),
    ("Titan", 26.73, 15.945, 10_759.22),
    ("Ceres", 4.03, 9.074_17 / 24.0, 1_680.0),
];

/// The spin of the table's body `name`, its mean solar day 2π ÷ |ω ∓ n| as
/// [`SeasonalSpin::of`] takes it.
pub(crate) fn spin_of(name: &str) -> SeasonalSpin {
    let &(_, degrees, rotation, year) = SPINS.iter().find(|(n, ..)| *n == name).unwrap();
    let obliquity = Radians::from(Degrees::new(degrees));
    let tau = 2.0 * core::f64::consts::PI;
    let (rotation, year) = (rotation * SECONDS_PER_DAY, year * SECONDS_PER_DAY);
    let solar = if degrees <= 90.0 {
        (tau / rotation - tau / year).abs()
    } else {
        tau / rotation + tau / year
    };
    SeasonalSpin {
        obliquity,
        rotation_period: Seconds::new(rotation),
        solar_day: Some(Seconds::new(tau / solar)),
    }
}

/// The inputs of the derived body `body` under the spin `spin`, its material of its own mass
/// fractions and its heat flow and ocean weight its own about the present Sun ([`heat_flow_of`],
/// [`ocean_weight_of`]).
pub(crate) fn inputs_of(body: &DerivedBody, spin: SeasonalSpin) -> SurfaceInputs<'_> {
    SurfaceInputs {
        atmosphere: body.atmosphere(),
        equilibrium_temperature: body.equilibrium_temperature(),
        surface_gravity: body.surface_gravity(),
        radius: Metres::from(body.radius()),
        material: SurfaceMaterial::of(&body.fractions()),
        lid: None,
        stellar_flux: WattsPerSquareMetre::from(body.flux()),
        xuv_flux: body.xuv_flux(),
        spin,
        heat_flow: heat_flow_of(body),
        ocean_weight: ocean_weight_of(body),
    }
}

/// The heat flow of the derived body `body` about the present Sun at its age (P14.T24.b), zero for
/// a gas envelope, which has none.
fn heat_flow_of(body: &DerivedBody) -> WattsPerSquareMetre {
    table_figures(body).map_or(WattsPerSquareMetre::new(0.0), |f| f.heat_flow())
}

/// The ocean weight θ of the derived body `body` about the present Sun at its age (P14.T24.b), none
/// for a gas envelope.
fn ocean_weight_of(body: &DerivedBody) -> OceanWeight {
    table_figures(body).map_or(OceanWeight::NONE, |f| f.ocean_weight())
}

/// The conditions of the derived body `body` under the spin `spin`.
pub(crate) fn conditions_of(body: &DerivedBody, spin: SeasonalSpin) -> Option<SurfaceConditions> {
    SurfaceConditions::derive(&inputs_of(body, spin))
}

/// The column and ocean weight the conditions of the derived body `body` read, under the spin
/// `spin`.
fn column_of(body: &DerivedBody, spin: SeasonalSpin) -> (Column, OceanWeight) {
    let inputs = inputs_of(body, spin);
    let surface = Surface::of(&inputs).unwrap();
    (surface.column, inputs.ocean_weight)
}

/// The dayside's and nightside's temperatures of the derived body `body` under the spin `spin`,
/// over the floor of its own heat flow, as `SurfaceConditions::derive` takes them.
fn day_and_night_of(body: &DerivedBody, spin: SeasonalSpin) -> (Kelvin, Kelvin) {
    let (column, _) = column_of(body, spin);
    day_and_night(
        &column,
        &spin,
        GroundInertia::of_material(SurfaceMaterial::of(&body.fractions())),
        night_floor(heat_flow_of(body)),
    )
}

/// Every body of the table, under P14.T24.f's rules, with its own spin.
pub(crate) fn table() -> Vec<(&'static str, DerivedBody, SurfaceConditions)> {
    historic_table()
        .into_iter()
        .map(|(name, body)| {
            let conditions = conditions_of(&body, spin_of(name)).unwrap();
            (name, body, conditions)
        })
        .collect()
}

fn found<'a>(
    table: &'a [(&'static str, DerivedBody, SurfaceConditions)],
    name: &str,
) -> &'a (&'static str, DerivedBody, SurfaceConditions) {
    table.iter().find(|(n, ..)| *n == name).unwrap()
}

/// The mole fraction of `gas` among `conditions`' gases; zero where it is not listed.
fn fraction(conditions: &SurfaceConditions, gas: Gas) -> f64 {
    conditions
        .gases()
        .iter()
        .find(|&&(id, _)| id == gas.substance())
        .map_or(0.0, |&(_, x)| x)
}

#[test]
fn earth_has_liquid_water_and_mars_and_venus_do_not() {
    let table = table();
    for (name, body, conditions) in &table {
        let (coldest, warmest) = conditions.climate_range();
        let (day, night) = day_and_night_of(body, spin_of(name));
        eprintln!(
            "{name}: {:?}, {:.2} K, ep {:.2} K, dn {:.3} K (day {:.2} K, night {:.2} K), range \
             [{:.1}, {:.1}] K, {:.4e} Pa, mu {:?}, cp/R {:?}, liquid {}, {:?}, flux {:.1} W/m2, \
             xuv {:.3e} W/m2",
            body.atmosphere().state(),
            conditions.mean_surface_temperature().value(),
            conditions.equator_pole_contrast_k(),
            conditions.day_night_contrast_k(),
            day.value(),
            night.value(),
            coldest.value(),
            warmest.value(),
            conditions.surface_pressure().value(),
            conditions.mean_molar_mass_g_per_mol(),
            conditions.heat_capacity_over_r(),
            conditions.liquid_water(),
            conditions.radiation(),
            conditions.stellar_flux().value(),
            body.xuv_flux().value(),
        );
        for &(id, x) in conditions.gases() {
            eprintln!("    {} {x:.6e}", id.substance().key().as_str());
        }
    }
    assert!(found(&table, "Earth").2.liquid_water());
    assert!(!found(&table, "Mars").2.liquid_water());
    assert!(!found(&table, "Venus").2.liquid_water());
}

/// Ice Ih melts under pressure on water's phase diagram: at 9.2 MPa, Venus's surface pressure, at
/// 272.463 K, 0.70 K below the triple temperature (IAPWS R14-08, eq. 1), so a range that ends at
/// 272.6 K holds liquid water there, and under 1 kPa, where the ice sublimes, none.
#[test]
fn a_range_below_the_triple_point_holds_liquid_water_where_pressure_melts_the_ice() {
    let range = (Kelvin::new(250.0), Kelvin::new(272.6));
    assert!(liquid_water(range, Pascals::new(9.2e6)));
    assert!(!liquid_water(range, Pascals::new(1e3)));
}

/// A gas at or above its critical temperature at the surface adds nothing to the greenhouse
/// gain, under either rules: carbon dioxide held at its cap (5 bar of an inventory of 10, over 5
/// bar of nitrogen) feeds back at 300 K, below its 304.1282 K, and not at it or at 310 K.
#[test]
fn the_greenhouse_gain_takes_nothing_from_a_gas_at_or_above_its_critical_temperature() {
    let (co2, n2) = (Gas::CarbonDioxide, Gas::Nitrogen);
    let weights = |carbon_dioxide: f64| {
        PartialPressures::from_fn(|gas| match gas {
            Gas::CarbonDioxide => Pascals::new(carbon_dioxide),
            Gas::Nitrogen => Pascals::new(5e5),
            _ => Pascals::ZERO,
        })
    };
    let (air, inventory) = (weights(5e5), weights(1e6));
    let moles = |gas: Gas| 5e5 / gas.molar_mass_g_per_mol();
    let x = moles(co2) / (moles(co2) + moles(n2));
    let gain = |t: f64| {
        greenhouse_gain(
            Kelvin::new(t),
            &air,
            1.0,
            &[(n2.substance(), 1.0 - x), (co2.substance(), x)],
            Kelvin::new(250.0),
            &inventory,
        )
    };
    let critical = co2
        .substance()
        .substance()
        .phase_diagram()
        .unwrap()
        .critical_temperature()
        .value();
    assert!(gain(300.0) > 0.0);
    for t in [critical, 310.0] {
        assert_same_bits(gain(t), 0.0);
    }
}

#[test]
fn a_locked_airless_body_s_day_night_contrast_exceeds_300_k_and_venus_s_is_under_10_k() {
    let table = table();
    let (_, mercury, _) = found(&table, "Mercury");
    assert_eq!(mercury.atmosphere().state(), SurfaceState::Airless);
    // Mercury locked 1:1 to the Sun, its day–night contrast about its substellar axis: its night
    // is the floor.
    let locked = conditions_of(
        mercury,
        SeasonalSpin {
            solar_day: None,
            ..spin_of("Mercury")
        },
    )
    .unwrap();
    assert!(
        locked.day_night_contrast_k() > 300.0,
        "{}",
        locked.day_night_contrast_k()
    );
    // The Moon, locked to Earth, turns under the Sun once in 29.5 days, and its regolith carries
    // some of the day into the night (science-p14-t24a-climate.md §2: 237.6 K).
    let (_, moon, moon_conditions) = found(&table, "Moon");
    assert_eq!(moon.atmosphere().state(), SurfaceState::Airless);
    let moon_dn = moon_conditions.day_night_contrast_k();
    assert!(
        (225.0..250.0).contains(&moon_dn),
        "the Moon's day–night contrast is {moon_dn} K"
    );
    let venus = found(&table, "Venus").2.day_night_contrast_k();
    assert!(
        (0.0..10.0).contains(&venus),
        "Venus's day–night contrast is {venus} K"
    );
    // A locked Venus would be as uniform: its thick air redistributes almost everything.
    let (_, venus_body, _) = found(&table, "Venus");
    let locked_venus = conditions_of(
        venus_body,
        SeasonalSpin {
            solar_day: None,
            ..spin_of("Venus")
        },
    )
    .unwrap();
    assert!(locked_venus.day_night_contrast_k() < 10.0);
    // Titan's air, at ε near 1, leaves under a kelvin.
    let titan = found(&table, "Titan").2.day_night_contrast_k();
    assert!((0.0..1.0).contains(&titan), "Titan's is {titan} K");
}

#[test]
fn the_published_fractions_are_mole_fractions() {
    // Equal weights of N₂ and CO₂ give x(N₂) = μ_CO₂ ÷ (μ_CO₂ + μ_N₂), with the registry's molar
    // masses.
    let (n2, co2) = (Gas::Nitrogen, Gas::CarbonDioxide);
    let gases = mole_fractions([
        (co2.substance(), Pascals::new(3e4)),
        (n2.substance(), Pascals::new(3e4)),
    ])
    .unwrap();
    let (m_n2, m_co2) = (n2.molar_mass_g_per_mol(), co2.molar_mass_g_per_mol());
    assert_eq!(gases[0].0, n2.substance());
    assert!((gases[0].1 - m_co2 / (m_co2 + m_n2)).abs() < 1e-12);
    assert!((gases[1].1 - m_n2 / (m_co2 + m_n2)).abs() < 1e-12);
    // Every body's μ equals p_s ÷ Σ(p_j ÷ μ_j), its list sums to 1 and runs largest first.
    let table = table();
    for (name, body, conditions) in &table {
        let sum = conditions.gases().iter().fold(0.0, |sum, &(_, x)| sum + x);
        let Some(mu) = conditions.mean_molar_mass_g_per_mol() else {
            assert!(conditions.gases().is_empty(), "{name}");
            continue;
        };
        assert!((sum - 1.0).abs() < 1e-9, "{name}'s fractions sum to {sum}");
        let partials = body.atmosphere().partial_pressures();
        let moles = Gas::ALL.iter().fold(0.0, |sum, &gas| {
            sum + partials.of(gas).value() / gas.molar_mass_g_per_mol()
        });
        let by_weight = partials.total().value() / moles;
        assert!(
            (mu - by_weight).abs() < 1e-12,
            "{name}: {mu} against {by_weight}"
        );
        assert_eq!(
            conditions.surface_pressure(),
            body.atmosphere().surface_pressure().unwrap()
        );
        for pair in conditions.gases().windows(2) {
            assert!(pair[0].1 >= pair[1].1, "{name}'s gases run largest first");
        }
    }
    // The table's Venus carries N₂ at its weight over its molar mass: about 1.65% by number from
    // 0.61 of 57.8 bar, not the 1.05% its weight is.
    let (_, venus, conditions) = found(&table, "Venus");
    let partials = venus.atmosphere().partial_pressures();
    let by_mass = partials.of(n2).value() / partials.total().value();
    let by_number = fraction(conditions, n2);
    eprintln!("Venus's N2: {by_number:.5} by number, {by_mass:.5} by mass");
    assert!((0.0160..0.0170).contains(&by_number), "{by_number}");
    assert!((0.0100..0.0110).contains(&by_mass), "{by_mass}");
}

#[test]
fn equal_fractions_are_ordered_by_substance() {
    // Weights in the ratio of the molar masses hold equal moles: the lower row comes first.
    let (water, methane) = (Gas::Water, Gas::Methane);
    let gases = mole_fractions([
        (
            methane.substance(),
            Pascals::new(methane.molar_mass_g_per_mol()),
        ),
        (
            water.substance(),
            Pascals::new(water.molar_mass_g_per_mol()),
        ),
    ])
    .unwrap();
    assert_same_bits(gases[0].1, gases[1].1);
    assert!(water.substance() < methane.substance());
    assert_eq!(gases[0].0, water.substance());
    // A weight of nothing is no gas, and no weight at all is an empty list.
    assert_eq!(
        mole_fractions([(water.substance(), Pascals::ZERO)]).unwrap(),
        []
    );
}

#[test]
fn a_weight_without_a_molar_mass_or_not_finite_is_refused() {
    // Every row of the registry so far has a molar mass, so a lookup that gives none stands in
    // for a later row that has none.
    let (water, methane) = (Gas::Water.substance(), Gas::Methane.substance());
    let unweighable = |id: SubstanceId| (id != methane).then(|| Gas::Water.molar_mass_g_per_mol());
    assert_eq!(
        mole_fractions_by(
            [(water, Pascals::new(1.0)), (methane, Pascals::new(1.0))],
            unweighable
        ),
        Err(CountMolesError::NoMolarMass(methane))
    );
    // A row of no weight is not counted, so it needs no molar mass.
    assert_eq!(
        mole_fractions_by([(methane, Pascals::ZERO)], unweighable),
        Ok(Vec::new())
    );
    for weight in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert_eq!(
            mole_fractions([(water, Pascals::new(1.0)), (methane, Pascals::new(weight))]),
            Err(CountMolesError::NotFinite(methane)),
            "{weight}"
        );
    }
}

#[test]
fn an_airless_body_has_no_gases_and_no_mean_molar_mass() {
    let mut airless = Vec::new();
    for (name, body, conditions) in table() {
        if body.atmosphere().state() != SurfaceState::Airless {
            continue;
        }
        airless.push(name);
        assert!(conditions.gases().is_empty(), "{name}");
        assert_eq!(conditions.mean_molar_mass_g_per_mol(), None, "{name}");
        assert_eq!(conditions.heat_capacity_over_r(), None, "{name}");
        assert_eq!(conditions.surface_pressure(), Pascals::ZERO, "{name}");
        assert!(!conditions.liquid_water(), "{name}");
    }
    // Mercury, the Moon, Ganymede and Ceres.
    assert_eq!(airless.len(), 4, "{airless:?}");
    for name in ["Mercury", "Moon", "Ganymede", "Ceres"] {
        assert!(airless.contains(&name), "{name}: {airless:?}");
    }
}

#[test]
fn the_mean_molar_mass_and_heat_capacity_are_the_registry_s_over_the_published_order() {
    for (name, _, conditions) in table() {
        assert_eq!(
            conditions.mean_molar_mass_g_per_mol(),
            mean_molar_mass_g_per_mol(conditions.gases()),
            "{name}"
        );
        assert_eq!(
            conditions.heat_capacity_over_r(),
            mixture_heat_capacity_over_r_at(
                conditions.gases(),
                conditions.mean_surface_temperature()
            ),
            "{name}"
        );
    }
}

#[test]
fn methane_and_oxygen_reach_the_fractions_through_the_speciated_derivation() {
    // P14.T24.f's Titan carries Huygens's 5.65% CH₄ by number to 10% (5.96% as built, at
    // 1.473 bar).
    let table = table();
    let (_, titan, conditions) = found(&table, "Titan");
    let methane = fraction(conditions, Gas::Methane);
    assert!(((methane - 0.0565) / 0.0565).abs() < 0.10, "{methane}");
    assert_eq!(conditions.gases()[0].0, Gas::Nitrogen.substance());
    assert_eq!(conditions.gases()[1].0, Gas::Methane.substance());
    assert!(titan.atmosphere().partial_pressures().of(Gas::Methane) > Pascals::ZERO);
    // The wet runaway world of `derive_body`'s golden keeps its lost oceans' oxygen.
    let wet = VolatileDraws {
        water: UnitUniform::new(0.99).unwrap(),
        ..VolatileDraws::MEDIAN
    };
    let o = orbit(0.6, 0.0);
    let runaway = PlacedBody::new(
        EarthMasses::new(1.0),
        o,
        o.semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap()
    .with_volatiles(wet);
    let body = derive_historic(&runaway, &solar_disc(), SOLAR_AGE);
    let conditions = conditions_of(&body, spin_of("Venus")).unwrap();
    let oxygen = fraction(&conditions, Gas::Oxygen);
    eprintln!("the wet runaway world: O2 {oxygen:.4} by number");
    assert!(oxygen > 0.5, "{oxygen}");
    assert_eq!(conditions.gases()[0].0, Gas::Oxygen.substance());
}

/// Earth's measured state as [`EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K`] and
/// [`WET_ALBEDO_GRADIENT`] are fitted to: 288 K over 240.25 W m⁻² absorbed, the grey τ that holds
/// it, Earth's column of dry air under g₀, its radius, obliquity and sidereal day, and the gain of
/// the batch's measured Earth ([`earth_gain`]; decision-pre-23-review item 20).
fn earth_column() -> (Column, SeasonalSpin) {
    let absorbed = (1.0 - 0.294) * crate::units::consts::SOLAR_CONSTANT_W_PER_M2 / 4.0;
    let surface = Kelvin::new(288.0);
    let equilibrium = math::powf(absorbed / STEFAN_BOLTZMANN, 0.25);
    let tau = (math::powi(surface.value() / equilibrium, 4) - 1.0) / 0.75;
    let pressure = EARTH_COLUMN_MASS_KG_PER_M2 * STANDARD_GRAVITY.value();
    let column = Column {
        surface_temperature: surface,
        equilibrium_temperature: Kelvin::new(equilibrium),
        surface_pressure: Pascals::new(pressure),
        air_mass_kg_per_m2: EARTH_COLUMN_MASS_KG_PER_M2,
        specific_heat_j_per_kg_k: EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K,
        molar_mass_g_per_mol: EARTH_AIR_MOLAR_MASS_G_PER_MOL,
        surface_gravity: STANDARD_GRAVITY,
        optical_depth: tau,
        greenhouse_gain: earth_gain(surface, tau),
        radius: Metres::new(6_371.0e3),
    };
    let spin = SeasonalSpin {
        obliquity: Radians::from(Degrees::new(23.44)),
        rotation_period: EARTH_SIDEREAL_DAY,
        solar_day: Some(Seconds::new(SECONDS_PER_DAY)),
    };
    (column, spin)
}

/// The batch's measured Earth's air, as P14.T57.a's water fit takes it (decision-p14-t57a-fits
/// §1.3), derived once: the table's Earth's partial pressures under the batch, its carbon dioxide
/// and nitrogen Earth's per unit mass (about 42 Pa and 0.747 bar by weight), and its airborne
/// inventory as column weights, whose water lies above any weight its air holds.
fn batch_earth_air() -> &'static (PartialPressures, PartialPressures) {
    static AIR: OnceLock<(PartialPressures, PartialPressures)> = OnceLock::new();
    AIR.get_or_init(|| {
        let earth = historic("Earth");
        let air = earth.atmosphere();
        (*air.partial_pressures(), *air.inventory_weights())
    })
}

/// The air `air` with its water at a surface at `surface` as P14.T51.a's partition holds a minor
/// condensable: its partial pressure, its mole fraction times the summed weights, at
/// [`WATER_SURFACE_HUMIDITY`](crate::planetary::chem::partition::WATER_SURFACE_HUMIDITY) of its
/// saturation pressure on water's phase diagram, its column weight the atmosphere's tests'
/// [`water_weight_at_humidity`], which P14.T57.a's water fit takes. With water's mole fraction.
fn with_water_at_humidity(air: &PartialPressures, surface: Kelvin) -> (PartialPressures, f64) {
    let water = Gas::Water;
    let moles = Gas::ALL
        .into_iter()
        .filter(|&gas| gas != water)
        .fold(0.0, |n, gas| {
            n + air.of(gas).value() / gas.molar_mass_g_per_mol()
        });
    let diagram = water.substance().substance().phase_diagram().unwrap();
    let w = water_weight_at_humidity(air, diagram.saturation_pressure(surface).value());
    let moles_w = w / water.molar_mass_g_per_mol();
    let wet = PartialPressures::from_fn(|gas| {
        if gas == water {
            Pascals::new(w)
        } else {
            air.of(gas)
        }
    });
    (wet, moles_w / (moles + moles_w))
}

/// The greenhouse gain of the batch's measured Earth, by hand (decision-pre-23-review item 20;
/// decision-p14-t57a-fits §1.3): its air ([`batch_earth_air`]) with its water at the partition's
/// humidity at `surface` ([`with_water_at_humidity`]), under P14.T57.a's constants, water's 4.47
/// ([`GreenhouseConstants::FITTED`]), in a column of grey optical
/// depth `tau`. The water counts as the batch's rule counts a gas the partition holds at its cap,
/// at its curve's slope at `surface` on water's phase diagram (44.47 kJ mol⁻¹ at 288 K).
fn earth_gain(surface: Kelvin, tau: f64) -> f64 {
    let water = Gas::Water;
    let (air, x_w) = with_water_at_humidity(&batch_earth_air().0, surface);
    let diagram = water.substance().substance().phase_diagram().unwrap();
    let t = surface.value();
    let tau_w = GreenhouseConstants::FITTED.water() * (air.of(water).value() / 1e5).sqrt();
    let enthalpy = diagram
        .clausius_clapeyron_enthalpy_j_per_mol(surface)
        .unwrap();
    let slope = (1.0 - x_w) * 0.5 * tau_w * enthalpy / (MOLAR_GAS_CONSTANT * t * t);
    t / 4.0 * 0.75 * slope / (1.0 + 0.75 * tau)
}

/// The batch's measured Earth's gain, [`earth_gain`]'s by hand, is the code's under the batch's
/// rule (decision-pre-23-review item 20): of its air ([`batch_earth_air`]) only its water counts,
/// held at its cap by the partition, under P14.T57.a's constants. It is the 0.4311, and its
/// `B_eff` the 1.898 W m⁻² K⁻¹, that [`greenhouse_gain`]'s and
/// [`EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K`]'s documentation state.
#[test]
fn the_measured_earth_s_gain_is_the_batch_s_rule() {
    let (column, _) = earth_column();
    let (air, inventory) = batch_earth_air();
    let t_s = column.surface_temperature;
    let (wet, x_w) = with_water_at_humidity(air, t_s);
    let rule = greenhouse_gain(
        t_s,
        &wet,
        column.optical_depth,
        &[(Gas::Water.substance(), x_w)],
        column.equilibrium_temperature,
        inventory,
    );
    let diagram = Gas::Water.substance().substance().phase_diagram().unwrap();
    let water = wet.of(Gas::Water).value();
    eprintln!(
        "the batch's measured Earth: water {water:.3} Pa ({:.4} of p_sat), x {x_w:.5}, inventory \
         {:.4e} Pa, gain {:.9} ({rule:.9} by the rule)",
        water / diagram.saturation_pressure(t_s).value(),
        inventory.of(Gas::Water).value(),
        column.greenhouse_gain,
    );
    assert!(
        (rule - column.greenhouse_gain).abs() < 1e-12,
        "{rule} against {}",
        column.greenhouse_gain
    );
    let effective = column.effective_restoring_w_per_m2_k();
    assert!((rule - 0.4311).abs() < 5e-5, "{rule}");
    assert!((effective - 1.898).abs() < 5e-4, "{effective}");
}

/// P14.T24.a's Earth fits by North, Cahalan and Coakley's procedure (decision-p14-t54a-bands
/// §2.6, amending decision-pre-23-review item 20(2)), which measured B and the forcing and fitted
/// D, "left as an adjustable parameter":
///
/// 1. Φ⊕ in their measured balance: the measured Earth's column ([`earth_column`]) with its gain
///    set so that its `B_eff` is their B, 2.09, and D = 0.310 × 2.09, by bisection to the 42 K
///    contrast on the zonal solver, with the 64-step bisection of each end, version 22's solver
///    kept for the tests ([`zonal_ends_bisected`]), as its oracle;
/// 2. D⊕ at the batch's measured Earth ([`earth_gain`], `B_eff` 1.898), at that Φ⊕, by bisection
///    to 42 K, with the same oracle; the last of plan 14's order (N₂ on Titan, CO₂ on Venus, water
///    on Earth, then D⊕ on Earth's contrast).
///
/// Each is held to its constant within 10⁻⁷. Then 6D⊕ ÷ `B_eff`⊕ is 2.14, against their 1.86 at
/// their B, the contrast at Φ = 1 is 28.4 K, and Φ⊕ lies within 1.40–1.60, about the Earth
/// Radiation Budget's gradient, 1.52–1.54. D⊕'s peak transport, 5.57 PW against the 5.5–6
/// measured ([`earth_s_restoring_and_transport_are_the_measured_ones`]), and its diffusivity, 2.71
/// × 10⁶ m² s⁻¹ against Siler, Roe and Armour's 2.70 × 10⁶, are its documentation's; and
/// [`the_measured_earth_s_fitted_profile_meets_its_checks`] holds the fitted profile's own.
#[test]
fn earth_s_measured_state_takes_north_cahalan_and_coakley_s_partition() {
    let (column, spin) = earth_column();
    let planck = column.restoring_w_per_m2_k();
    let effective = column.effective_restoring_w_per_m2_k();
    let weak = weak_gradient_transport(&column, &spin) * planck;
    let contrast_under = |solver: ZonalEnds, effective: f64, conductance: f64, albedo: f64| {
        let transport = ZonalTransport {
            planck,
            effective,
            conductance,
            weak,
            albedo,
        };
        let (equator, pole) = solver(
            column.surface_temperature.value(),
            &transport,
            spin.obliquity,
        );
        equator - pole
    };
    let (newton, oracle): (ZonalEnds, ZonalEnds) = (zonal_ends, zonal_ends_bisected);
    // 200 bisections of `range` for the value at which `contrast` is 42 K, rising in it where
    // `rising` and falling otherwise.
    let bisect = |range: (f64, f64), rising: bool, contrast: &dyn Fn(f64) -> f64| {
        let (mut low, mut high) = range;
        for _ in 0..200 {
            let middle = f64::midpoint(low, high);
            if (contrast(middle) > EARTH_EQUATOR_POLE_CONTRAST_K) == rising {
                high = middle;
            } else {
                low = middle;
            }
        }
        f64::midpoint(low, high)
    };
    // 1. Φ⊕ in North, Cahalan and Coakley's measured balance: B_eff their B, D 0.310 of it.
    let balance = Column {
        greenhouse_gain: 1.0 - EARTH_OUTGOING_LONGWAVE_SLOPE_W_PER_M2_K / planck,
        ..column
    };
    let measured = balance.effective_restoring_w_per_m2_k();
    let partition = EARTH_TRANSPORT_PARTITION * measured;
    let albedo_under = |solver: ZonalEnds| {
        bisect((1.0, 3.0), true, &|albedo| {
            contrast_under(solver, measured, partition, albedo)
        })
    };
    let (albedo, albedo_oracle) = (albedo_under(newton), albedo_under(oracle));
    // 2. D⊕ at the batch's measured Earth, at that Φ⊕: the contrast falls as D grows.
    let conductance_under = |solver: ZonalEnds| {
        bisect((0.0, 3.0), false, &|conductance| {
            contrast_under(solver, effective, conductance, albedo)
        })
    };
    let (conductance, conductance_oracle) = (conductance_under(newton), conductance_under(oracle));
    let ratio = 6.0 * conductance / effective;
    eprintln!(
        "the measured balance: B_eff {measured:.9} W/m2/K, D {partition:.10} W/m2/K, Phi \
         {albedo:.10} (the bisection's {albedo_oracle:.10}); the batch's measured Earth: B_P \
         {planck:.9} W/m2/K, gain {:.9}, B_eff {effective:.9} W/m2/K, D {conductance:.10} W/m2/K \
         (the bisection's {conductance_oracle:.10}), 6D/B_eff {ratio:.4}, tau {:.6}, at Phi 1: \
         {:.6} K",
        column.greenhouse_gain,
        column.optical_depth,
        contrast_under(newton, effective, conductance, 1.0)
    );
    for (solver, name) in [(newton, "Newton"), (oracle, "bisection")] {
        let balanced = contrast_under(solver, measured, partition, albedo);
        let at = contrast_under(solver, effective, conductance, albedo);
        for contrast in [balanced, at] {
            assert!(
                (contrast - EARTH_EQUATOR_POLE_CONTRAST_K).abs() < 1e-9,
                "{name}: {contrast} K"
            );
        }
    }
    assert!(
        (albedo / albedo_oracle - 1.0).abs() < 1e-9,
        "{albedo} against {albedo_oracle}"
    );
    assert!(
        (conductance / conductance_oracle - 1.0).abs() < 1e-9,
        "{conductance} against {conductance_oracle}"
    );
    assert!(
        (albedo / WET_ALBEDO_GRADIENT - 1.0).abs() < 1e-7,
        "{albedo}"
    );
    assert!(
        (conductance / EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K - 1.0).abs() < 1e-7,
        "{conductance}"
    );
    assert!((ratio - 2.14).abs() < 0.005, "{ratio}");
    // Band 2 (decision-p14-t54a-bands §2.6): Φ⊕ about the Earth Radiation Budget's gradient,
    // H₂ ÷ (S₂ a₀) = 1.52–1.54, which lies 2% above it.
    assert!((1.40..1.60).contains(&albedo), "Φ⊕ {albedo}");
}

/// The measured Earth's profile at the fitted D⊕ and Φ⊕ ([`earth_column`];
/// decision-p14-t54a-bands §2.5 and §4.3): every scaling of its conductance is 1, Kaspi and
/// Showman's law governs it rather than Koll's weak gradient, its contrast is 42 K, its ends are
/// 302.5 and 260.5 K, within 1 K of North, Cahalan and Coakley's, its dry contrast is 43.4 K, and
/// its cover's ice, the cold area below 263.15 K, is 0.035, inside 0.02–0.07.
#[test]
fn the_measured_earth_s_fitted_profile_meets_its_checks() {
    let (column, spin) = earth_column();
    let weak = weak_gradient_transport(&column, &spin) * column.restoring_w_per_m2_k();
    // Every scaling of the conductance is 1 on the measured Earth.
    assert!(
        (meridional_conductance_w_per_m2_k(&column, &spin)
            - EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K)
            .abs()
            < 1e-15
    );
    // Kaspi and Showman's law governs Earth, not Koll's weak gradient.
    eprintln!(
        "W B_P {weak} against 6D {}",
        6.0 * EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K
    );
    assert!(weak < 6.0 * EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K);
    let contrast = equator_pole_contrast_k(&column, &spin, OceanWeight::FULL);
    assert!(
        (contrast - EARTH_EQUATOR_POLE_CONTRAST_K).abs() < 1e-5,
        "Earth's contrast is {contrast} K"
    );
    // Its ends lie within 1 K of North, Cahalan and Coakley's P₂ fit, 302.1 and 260.1 K.
    let (equator, pole) = zonal_ends_k(&column, &spin, OceanWeight::FULL);
    eprintln!("the measured Earth's ends: {equator} and {pole} K");
    assert!((equator - 302.1).abs() < 1.0 && (pole - 260.1).abs() < 1.0);
    // Dry, the same Earth keeps half its transport under a uniform albedo, and a larger contrast:
    // 43.4 K against 42.0.
    let dry = equator_pole_contrast_k(&column, &spin, OceanWeight::NONE);
    eprintln!("dry: {dry} K");
    assert!(dry > contrast, "{dry}");
    // The cover's rule on the measured Earth's profile, the cold area below 263.15 K
    // (decision-p14-t54a-bands §4.3): 0.035, against Earth's perennial ice, 0.045–0.051 of the
    // globe, and its annual mean, 0.068–0.078 (Fretwell et al. 2013, The Cryosphere 7, 375, Table
    // 7; IPCC AR5 WGI Ch. 4, Tables 4.1–4.2; the NSIDC Sea Ice Index, v4, 1981–2010).
    let profile = ZonalProfile::of(&column, &spin, OceanWeight::FULL);
    let cover = area_below(&profile.points(), PERENNIAL_ICE_TEMPERATURE);
    eprintln!("the measured Earth's cover: {cover}");
    assert!((0.02..=0.07).contains(&cover), "{cover}");
    // The annual insolation, by quadrature of its definitions at 23.44°: 1.2213 at the equator and
    // 0.5065 at a pole, each over S ÷ 4.
    let (equator, pole) = annual_insolation(spin.obliquity);
    assert!((equator - 1.221_269).abs() < 1e-6, "{equator}");
    assert!((pole - 0.506_480).abs() < 1e-6, "{pole}");
}

/// The measured Earth's restoring and transport against what they stand for
/// (decision-p14-t54a-bands §1 and §2.6): its peak diffusive transport, 5.57 PW, within 4.5–6.5
/// PW of the 5.5–6 measured, and its `B_eff`⊕, 1.898 W m⁻² K⁻¹, within 1.7–2.3, the fixed relative
/// humidity Planck feedback's.
#[test]
fn earth_s_restoring_and_transport_are_the_measured_ones() {
    let (column, _) = earth_column();
    let effective = column.effective_restoring_w_per_m2_k();
    // The peak diffusive transport, 2πR² D (2 ÷ √3) |T₂| at T₂ = −28 K: 5.5–6 PW measured.
    let r = column.radius.value();
    let peak = 2.0
        * core::f64::consts::PI
        * r
        * r
        * EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K
        * (2.0 / 3.0_f64.sqrt())
        * 28.0
        / 1e15;
    eprintln!("B_eff {effective} W/m2/K, peak transport {peak} PW");
    assert!((4.5..6.5).contains(&peak), "{peak} PW");
    // Band 1 (decision-p14-t54a-bands §1): the fixed relative humidity Planck feedback, with the
    // relative humidity, the lapse rate and the cloud fixed, as the model's B_eff is:
    // - 1.75 ± 0.01 (Held and Shell 2012, J. Climate 25, 2578, eq. 15);
    // - 1.86 (IPCC AR6 WGI Ch. 7: the Planck feedback's 3.22 less fixed-RH water vapour's 1.36);
    // - about 1.9 (Zelinka et al. 2020, GRL 47, e2019GL085782);
    // - 1.90–1.92 (Dessler 2013, J. Climate 26, 333, Table 1);
    // - up to 2.16–2.26 clear-sky and in a single column (Koll and Cronin 2018's 2.218; Koll,
    //   Jeevanjee and Lutsko 2023's −1.8 to −2.2; Ramanathan and Coakley 1978, via North, Cahalan
    //   and Coakley 1981).
    // North, Cahalan and Coakley's 2.09 is a zonal all-sky regression that ignores the cloud.
    assert!((1.7..2.3).contains(&effective), "B_eff⊕ {effective} W/m2/K");
}

/// The greenhouse gain is the vapour feedback of the gases the surface holds at their cap. On the
/// table's bodies an airless body has none, and T13.c's terms sum to its τ to the bit; no gas of
/// Venus's or Titan's bulk is held, so neither feeds back (decision-pre-23-review item 20;
/// decision-p14-t54a-bands §3):
///
/// - the generated Earth's water is held at its cap, below its inventory (some of it condensed),
///   and its gain is that water's feedback by hand under P14.T57.a's constants;
/// - the table's Earth gives 0.4545 (`B_eff` 1.841 W m⁻² K⁻¹) through the passes at their stand-in
///   surface, at 290.57 K, the figures [`greenhouse_gain`]'s documentation states (version 22's
///   rules gave 0.3997, `B_eff` 1.987);
/// - band 3: the batch's generated Earth restores as the measured Earth does, its `B_eff` within
///   the fixed relative humidity Planck feedback's 1.7–2.3 W m⁻² K⁻¹ and within 0.10 of the
///   measured Earth's `B_eff`⊕ ([`earth_column`]), 1.841 against 1.898 at the stand-ins (printed
///   again at Batch C's real inputs);
/// - a wet world's gain lies within (0, 1) and rises to 330 K
///   ([`a_wet_world_s_gain_lies_within_zero_and_one_and_rises_to_330_k`], band 4).
#[test]
fn the_greenhouse_gain_is_the_held_gases_feedback() {
    let table = table();
    for (name, body, _) in &table {
        let (column, _) = column_of(body, spin_of(name));
        let air = body.atmosphere();
        if air.state() == SurfaceState::Airless {
            assert_same_bits(column.greenhouse_gain, 0.0);
            continue;
        }
        // T13.c's terms sum to its τ to the bit, at the derivation's equilibrium temperature.
        let terms = GreenhouseConstants::FITTED
            .terms(air.partial_pressures(), body.equilibrium_temperature());
        let sum = terms.iter().fold(0.0, |sum, &(_, term)| sum + term);
        assert_same_bits(sum, air.optical_depth());
        let effective = column.effective_restoring_w_per_m2_k();
        eprintln!(
            "{name}: gain {:.4}, B_P {:.4}, B_eff {effective:.4} W/m2/K",
            column.greenhouse_gain,
            column.restoring_w_per_m2_k(),
        );
    }
    // The generated Earth holds its water at its cap: below its inventory, some of it condensed.
    // By hand, as `earth_gain` takes the measured state, at its own temperature, depth and water
    // fraction, under P14.T57.a's constants, its curve's slope at T_s on water's phase diagram.
    let spin = spin_of("Earth");
    let (_, earth, conditions) = found(&table, "Earth");
    let (column, _) = column_of(earth, spin);
    let air = earth.atmosphere();
    let t_s = air.surface_temperature();
    let water = Gas::Water;
    let diagram = water.substance().substance().phase_diagram().unwrap();
    let p_w = air.partial_pressures().of(water);
    let inventory_w = air.inventory_weights().of(water);
    assert!(
        p_w < inventory_w,
        "water {p_w:?} against its inventory's {inventory_w:?}"
    );
    let fitted = GreenhouseConstants::FITTED;
    let t = t_s.value();
    let tau_w = fitted.water() * (p_w.value() / 1e5).sqrt();
    let enthalpy = diagram.clausius_clapeyron_enthalpy_j_per_mol(t_s).unwrap();
    let slope =
        (1.0 - fraction(conditions, water)) * 0.5 * tau_w * enthalpy / (MOLAR_GAS_CONSTANT * t * t);
    let by_hand = t / 4.0 * 0.75 * slope / (1.0 + 0.75 * air.optical_depth());
    let effective = column.effective_restoring_w_per_m2_k();
    eprintln!(
        "the generated Earth: T_s {t:.3} K, gain {:.4} ({by_hand:.4} by hand), B_P {:.4}, B_eff \
         {effective:.4} W/m2/K",
        column.greenhouse_gain,
        column.restoring_w_per_m2_k()
    );
    assert!(
        (column.greenhouse_gain - by_hand).abs() < 1e-12,
        "{} against {by_hand}",
        column.greenhouse_gain
    );
    assert!((by_hand - 0.4545).abs() < 5e-5, "{by_hand}");
    assert!((effective - 1.841).abs() < 5e-4, "{effective}");
    // No gas of Venus's or Titan's bulk is held at saturation: neither feeds back.
    for name in ["Venus", "Titan"] {
        let (_, body, _) = found(&table, name);
        let (column, _) = column_of(body, spin_of(name));
        assert!(
            column.greenhouse_gain.abs() < 1e-3,
            "{name}: {}",
            column.greenhouse_gain
        );
    }
    // Band 3 (decision-p14-t54a-bands §3.2): the generated Earth's B_eff, 1.841 W m⁻² K⁻¹ at the
    // stand-ins, within band 1's 1.7–2.3 and within 0.10 of the measured Earth's, 1.898.
    let measured = earth_column().0.effective_restoring_w_per_m2_k();
    assert!((1.7..2.3).contains(&effective), "B_eff {effective} W/m2/K");
    assert!(
        (effective - measured).abs() <= 0.10,
        "B_eff {effective} against the measured Earth's {measured} W/m2/K"
    );
}

/// Band 4 (decision-p14-t54a-bands §3.3): a wet world's greenhouse gain, 0.75 bar of N₂ and 42 Pa
/// of CO₂ over a surface in equilibrium at 255 K, its water at the partition's humidity, under
/// P14.T57.a's constants (science-p14-t24a-climate.md §1), lies within (0, 1) from 260 to 360 K,
/// the feedback never reversing the restoring, and rises from 260 K through 330 K.
///
/// At 260, 273.17, 288, 300, 310, 320, 330, 340, 350 and 360 K it is 0.217, 0.303, 0.432, 0.538,
/// 0.618, 0.679, 0.712, 0.711, 0.672 and 0.602: the peak, 0.712, at 330 K, and the fall above it
/// the (1 − x) weight's at water's mole fraction of 0.2–0.4 (P14.T53.a's). Radiative–convective
/// models' λ falls from about 2 W m⁻² K⁻¹ to about 1 by 320–330 K, and to 0 near 340–350 K
/// (Goldblatt et al. 2013, Nat. Geosci. 6, 661; Koll and Cronin 2018, PNAS 115, 10293; Kluft et
/// al. 2021, GRL 48, e2021GL094649, "more negative than −1 … as high as 330 K"), where the model's
/// local restoring is 1.01 at 330 K; no band holds that magnitude. Version 22's g < 0.57 was the
/// (1 − x) weight's own bound at its saturated water, not a physical one, and is gone.
#[test]
fn a_wet_world_s_gain_lies_within_zero_and_one_and_rises_to_330_k() {
    let water = Gas::Water;
    let diagram = water.substance().substance().phase_diagram().unwrap();
    let fitted = GreenhouseConstants::FITTED;
    let dry_air = PartialPressures::from_fn(|gas| match gas {
        Gas::Nitrogen => Pascals::new(75_000.0),
        Gas::CarbonDioxide => Pascals::new(42.0),
        _ => Pascals::ZERO,
    });
    let dry_depth = fitted.carbon_dioxide() * (42.0_f64 / 1e5).sqrt()
        + fitted.nitrogen()
            * math::powi(NITROGEN_REFERENCE_TEMPERATURE.value() / 255.0, 2)
            * 0.75_f64.sqrt();
    let gains = [
        260.0, 273.17, 288.0, 300.0, 310.0, 320.0, 330.0, 340.0, 350.0, 360.0,
    ]
    .map(|t| {
        let surface = Kelvin::new(t);
        let (wet, x_w) = with_water_at_humidity(&dry_air, surface);
        let w = wet.of(water).value();
        let tau_w = fitted.water() * (w / 1e5).sqrt();
        let enthalpy = diagram
            .clausius_clapeyron_enthalpy_j_per_mol(surface)
            .unwrap();
        let slope = (1.0 - x_w) * 0.5 * tau_w * enthalpy / (MOLAR_GAS_CONSTANT * t * t);
        let gain = grey_gain(surface, tau_w + dry_depth, slope);
        eprintln!("{t} K: water {w:.1} Pa, x {x_w:.4}, gain {gain:.4}");
        (t, gain)
    });
    assert!(
        gains.iter().all(|&(_, gain)| gain > 0.0 && gain < 1.0),
        "{gains:?}"
    );
    let rising = gains.iter().take_while(|&&(t, _)| t <= 330.0);
    let rising: Vec<f64> = rising.map(|&(_, gain)| gain).collect();
    assert_eq!(rising.len(), 7, "{gains:?}");
    assert!(rising.windows(2).all(|pair| pair[1] > pair[0]), "{gains:?}");
}

/// P14.T51.a's partition holds a minor condensable's partial pressure at its humidity, below its
/// saturation pressure: Earth's water at 0.77 of it, a column weight of 0.496 of it. So the
/// greenhouse gain counts a gas where the partition holds it at its cap, on either branch, as the
/// partition's own state says (some of its inventory condensed: its airborne weight below its
/// inventory's), not where its weight reaches its saturation pressure, version 22's rule, under
/// which the batch's Earth lost its water's feedback (decision-p14-t57a-fits §5). The Earth keeps
/// it, by hand as `the_greenhouse_gain_is_the_held_gases_feedback` takes it, with P14.T57.a's
/// constants, the ones its τ was taken under; Venus's and Titan's bulk, which nothing condenses,
/// give none.
#[test]
fn under_the_batch_the_greenhouse_gain_counts_the_water_the_partition_holds() {
    let spin = spin_of("Earth");
    let earth = historic("Earth");
    let conditions = conditions_of(&earth, spin).unwrap();
    let (column, _) = column_of(&earth, spin);
    let air = earth.atmosphere();
    let water = Gas::Water;
    let diagram = water.substance().substance().phase_diagram().unwrap();
    let t_s = air.surface_temperature();
    let p_w = air.partial_pressures().of(water);
    // Version 22's rule would not count it.
    assert!(p_w < diagram.saturation_pressure(t_s), "{p_w:?}");
    let t = t_s.value();
    let tau_w = GreenhouseConstants::FITTED.water() * (p_w.value() / 1e5).sqrt();
    let slope = (1.0 - fraction(&conditions, water))
        * 0.5
        * tau_w
        * diagram.clausius_clapeyron_enthalpy_j_per_mol(t_s).unwrap()
        / (MOLAR_GAS_CONSTANT * t * t);
    let by_hand = t / 4.0 * 0.75 * slope / (1.0 + 0.75 * air.optical_depth());
    eprintln!(
        "Earth's greenhouse gain: {:.4} ({:.4} by hand)",
        column.greenhouse_gain, by_hand
    );
    assert!(
        (column.greenhouse_gain - by_hand).abs() < 1e-12,
        "{} against {by_hand}",
        column.greenhouse_gain
    );
    assert!(column.greenhouse_gain > 0.3, "{}", column.greenhouse_gain);
    for name in ["Venus", "Titan"] {
        let body = historic(name);
        let (column, _) = column_of(&body, spin_of(name));
        assert!(
            column.greenhouse_gain.abs() < 1e-3,
            "{name}: {}",
            column.greenhouse_gain
        );
    }
}

/// Asserts that the model's contrast relative to Earth's, `model`, lies within `tolerance` of the
/// GCM's, `gcm`, at the point `at` of Kaspi and Showman's figure `figure`.
fn against_gcm(figure: &str, at: f64, model: f64, gcm: f64, tolerance: f64) {
    eprintln!(
        "{figure} {at}: GCM {gcm:.3}, model {model:.3} ({:+.1}%)",
        (model / gcm - 1.0) * 100.0
    );
    assert!(
        (model / gcm - 1.0).abs() < tolerance,
        "{figure} at {at}: {model} against {gcm}"
    );
}

#[test]
fn the_transport_follows_kaspi_and_showman_s_sweeps() {
    // Kaspi and Showman 2015 (ApJ 804, 60), their contrasts read off the rendered figures to
    // ±1 K by science-p14-t24a-climate.md, each sweep from an Earth with τ held. Each ratio to
    // Earth's lies within 15% of Fig. 8a's (40 K at Earth's rate; its plateau at Ω⊕ ÷ 4 is the
    // GCM's, −13.0% at D⊕'s fit) and within 10% of Figs. 15c's, 18a's and 20b's (the tightest,
    // +9.9% at 0.2 bar, the column law's taper; decision-p14-t54a-bands §2.5).
    let (earth, spin) = earth_column();
    let contrast = |column: &Column, spin: &SeasonalSpin| {
        equator_pole_contrast_k(column, spin, OceanWeight::FULL)
    };
    let base = contrast(&earth, &spin);
    // Fig. 8a, the rotation rate over Earth's.
    for (rate, gcm) in [
        (1.0 / 24.0, 23.0),
        (1.0 / 8.0, 28.0),
        (1.0 / 4.0, 36.0),
        (1.0 / 2.0, 36.0),
        (2.0, 48.0),
        (4.0, 58.0),
        (8.0, 70.0),
        (12.0, 77.0),
    ] {
        let turned = SeasonalSpin {
            rotation_period: Seconds::new(EARTH_SIDEREAL_DAY.value() / rate),
            ..spin
        };
        against_gcm(
            "8a",
            rate,
            contrast(&earth, &turned) / base,
            gcm / 40.0,
            0.15,
        );
    }
    // Fig. 15c, the column's mass in Earth's (bar), at Earth's gravity: 42 K at 1 bar.
    for (bar, gcm) in [
        (0.2, 47.0),
        (0.5, 44.0),
        (2.0, 40.0),
        (3.0, 37.0),
        (5.0, 35.0),
        (10.0, 30.5),
        (20.0, 26.5),
        (30.0, 24.0),
        (50.0, 22.0),
    ] {
        let massive = Column {
            air_mass_kg_per_m2: earth.air_mass_kg_per_m2 * bar,
            surface_pressure: Pascals::new(earth.surface_pressure.value() * bar),
            ..earth
        };
        against_gcm(
            "15c",
            bar,
            contrast(&massive, &spin) / base,
            gcm / 42.0,
            0.10,
        );
    }
    // Fig. 18a, the mean density (kg m⁻³) at Earth's radius and column mass, g ∝ ρ: 42.5 K at
    // 5,520.
    for (density, gcm) in [
        (500.0, 20.0),
        (1_000.0, 24.0),
        (2_000.0, 31.0),
        (3_000.0, 35.5),
        (4_000.0, 39.5),
        (5_500.0, 42.5),
        (6_500.0, 45.0),
        (8_000.0, 47.5),
        (9_000.0, 49.5),
        (10_000.0, 50.5),
        (11_200.0, 52.0),
    ] {
        let g = STANDARD_GRAVITY.value() * density / 5_520.0;
        let dense = Column {
            surface_gravity: MetresPerSecondSquared::new(g),
            surface_pressure: Pascals::new(earth.air_mass_kg_per_m2 * g),
            ..earth
        };
        against_gcm(
            "18a",
            density,
            contrast(&dense, &spin) / base,
            gcm / 42.5,
            0.10,
        );
    }
    // Fig. 20b, the radius (km) at Earth's density and column mass, g ∝ R: 42.5 K at 6,371 km.
    for (km, gcm) in [
        (1_000.0, 22.0),
        (2_000.0, 28.0),
        (3_000.0, 33.0),
        (5_000.0, 40.0),
        (8_000.0, 46.0),
        (10_000.0, 49.0),
        (20_000.0, 58.0),
        (30_000.0, 63.0),
        (50_000.0, 67.0),
    ] {
        let g = STANDARD_GRAVITY.value() * km / 6_371.0;
        let sized = Column {
            radius: Metres::new(km * 1e3),
            surface_gravity: MetresPerSecondSquared::new(g),
            surface_pressure: Pascals::new(earth.air_mass_kg_per_m2 * g),
            ..earth
        };
        against_gcm("20b", km, contrast(&sized, &spin) / base, gcm / 42.5, 0.10);
    }
}

#[test]
fn the_generated_venus_and_titan_are_near_isothermal() {
    let table = table();
    // Venus's slow spin leaves Koll's weak gradient to govern its transport (science-p14-t24a-
    // climate.md §1: 2.8 K).
    let (_, venus, conditions) = found(&table, "Venus");
    let (column, weight) = column_of(venus, spin_of("Venus"));
    assert_eq!(weight, OceanWeight::NONE);
    let spin = spin_of("Venus");
    let weak = weak_gradient_transport(&column, &spin) * column.restoring_w_per_m2_k();
    let diffusive = 6.0 * meridional_conductance_w_per_m2_k(&column, &spin) * DRY_TRANSPORT_SHARE;
    eprintln!(
        "Venus: {} K, W B_P {weak} against 6D {diffusive}, X {}, k {}",
        conditions.equator_pole_contrast_k(),
        redistribution_parameter(&column),
        redistribution_scale(&column)
    );
    assert!(weak > diffusive);
    assert!(
        (0.0..5.0).contains(&conditions.equator_pole_contrast_k()),
        "{}",
        conditions.equator_pole_contrast_k()
    );
    // Titan's is far under its measured 1–4 K, which its seasons and methane set; any annual law
    // keeps it under 4 K.
    let titan = found(&table, "Titan").2.equator_pole_contrast_k();
    assert!((0.0..4.0).contains(&titan), "{titan}");
}

#[test]
fn a_mars_given_its_measured_state_lies_within_30_to_70_k() {
    // The measured Mars, by hand: 6.1 mbar of CO₂ at 210 K, 586.2 W m⁻² at a Bond albedo of 0.25,
    // 3.71 m s⁻², 3,389.5 km and 43.34 g mol⁻¹ (NASA's Mars fact sheet), the registry's c_p ÷ R
    // for CO₂ at 210 K, 25.19° and 24.62 h. Its measured annual contrast is some 50–65 K, short by
    // its CO₂ caps' brightening, which is P14.T51.a's.
    let co2 = Gas::CarbonDioxide.substance();
    let mu = 43.34;
    let cp = mixture_heat_capacity_over_r_at(&[(co2, 1.0)], Kelvin::new(210.0)).unwrap();
    let equilibrium = math::powf(0.75 * 586.2 / 4.0 / STEFAN_BOLTZMANN, 0.25);
    let (pressure, gravity) = (610.0, 3.71);
    let mars = Column {
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
    };
    let spin = spin_of("Mars");
    let contrast = equator_pole_contrast_k(&mars, &spin, OceanWeight::NONE);
    let six_d_over_b = 6.0 * meridional_conductance_w_per_m2_k(&mars, &spin) * DRY_TRANSPORT_SHARE
        / mars.restoring_w_per_m2_k();
    eprintln!("the measured Mars: {contrast} K, 6D/B {six_d_over_b}");
    assert!((30.0..70.0).contains(&contrast), "{contrast}");
    // Its thin air's transport is weak, from the taper below 0.2 of Earth's column.
    assert!(six_d_over_b < 0.15, "{six_d_over_b}");
}

#[test]
fn the_transport_is_continuous_across_its_ends() {
    let (earth, spin) = earth_column();
    let conductance = |mass: f64, g: f64| {
        meridional_conductance_w_per_m2_k(
            &Column {
                air_mass_kg_per_m2: mass * EARTH_COLUMN_MASS_KG_PER_M2,
                surface_gravity: MetresPerSecondSquared::new(g * STANDARD_GRAVITY.value()),
                ..earth
            },
            &spin,
        )
    };
    let close = |a: f64, b: f64| (a / b - 1.0).abs() < 1e-8;
    // The taper at a fifth of Earth's column, and the ends of the column's and the gravity's
    // ranges.
    let (low, high) = COLUMN_TRANSPORT_RANGE;
    for mass in [low, high] {
        assert!(close(
            conductance(mass * (1.0 - 1e-10), 1.0),
            conductance(mass * (1.0 + 1e-10), 1.0)
        ));
    }
    assert_same_bits(conductance(high * 2.0, 1.0), conductance(high, 1.0));
    assert!(conductance(1e-6, 1.0) < 1e-5 * conductance(1.0, 1.0));
    assert_same_bits(conductance(0.0, 1.0), 0.0);
    let (g_low, g_high) = GRAVITY_TRANSPORT_RANGE;
    for g in [g_low, g_high] {
        assert!(close(
            conductance(1.0, g * (1.0 - 1e-10)),
            conductance(1.0, g * (1.0 + 1e-10))
        ));
    }
    assert_same_bits(conductance(1.0, g_low / 2.0), conductance(1.0, g_low));
    assert_same_bits(conductance(1.0, g_high * 2.0), conductance(1.0, g_high));
    // Through the weak gradient's weight and the rotation law's ends: a dry Earth, its period
    // swept from an hour to ten thousand days in steps of 0.1%, moves its contrast by under
    // 0.25% a step (the weak gradient's weight goes as the period squared), and a body that does
    // not turn takes the slowest one's.
    let dry = |period: f64| {
        equator_pole_contrast_k(
            &earth,
            &SeasonalSpin {
                rotation_period: Seconds::new(period),
                ..spin
            },
            OceanWeight::NONE,
        )
    };
    let mut period = 3_600.0;
    let mut last = dry(period);
    while period < 1e4 * SECONDS_PER_DAY {
        period *= 1.001;
        let next = dry(period);
        assert!(
            (next / last - 1.0).abs() < 2.5e-3,
            "{period} s: {last} to {next}"
        );
        last = next;
    }
    assert!((dry(f64::INFINITY) / dry(1e15) - 1.0).abs() < 1e-6);
}
#[test]
fn the_annual_insolation_is_its_definitions_mean() {
    // The year's mean of the daily-mean insolation at the equator, (4 ÷ π) cos δ over S ÷ 4, and
    // at a pole, 4 sin δ in its summer, by the midpoint rule over the sun's longitude.
    let steps = 20_000;
    for degrees in [0.0, 1.0, 23.44, 53.9, 90.0, 120.0, 177.36] {
        let beta = Radians::from(Degrees::new(degrees)).value();
        let (mut equator, mut pole) = (0.0, 0.0);
        for i in 0..steps {
            let lambda = (f64::from(i) + 0.5) / f64::from(steps) * 2.0 * core::f64::consts::PI;
            let sin_delta = math::sin(beta) * math::sin(lambda);
            equator += 4.0 / core::f64::consts::PI * (1.0 - sin_delta * sin_delta).sqrt();
            pole += 4.0 * sin_delta.max(0.0);
        }
        let (equator, pole) = (equator / f64::from(steps), pole / f64::from(steps));
        let (closed_equator, closed_pole) = annual_insolation(Radians::new(beta));
        assert!(
            (closed_equator - equator).abs() < 1e-8,
            "{degrees}°: {closed_equator} against {equator}"
        );
        assert!(
            (closed_pole - pole).abs() < 1e-6,
            "{degrees}°: {closed_pole} against {pole}"
        );
    }
    // E(0) = π ÷ 2 and E(1) = 1, and the global mean of the zonal insolation is 1.
    assert_same_bits(complete_elliptic_e(0.0), core::f64::consts::FRAC_PI_2);
    assert_same_bits(complete_elliptic_e(1.0), 1.0);
    assert!((complete_elliptic_e(1.0 - 1e-12) - 1.0).abs() < 1e-9);
}

#[test]
fn a_ninety_degree_obliquity_world_s_equator_pole_contrast_is_negative() {
    let (column, spin) = earth_column();
    let tilted = |column: &Column, moisture: OceanWeight, degrees: f64| {
        let spin = SeasonalSpin {
            obliquity: Radians::from(Degrees::new(degrees)),
            ..spin
        };
        equator_pole_contrast_k(column, &spin, moisture)
    };
    let at = |degrees: f64| tilted(&column, OceanWeight::FULL, degrees);
    assert!(at(90.0) < 0.0);
    // On an airless body (the table's Moon) the contrast is the radiative one. At 90° a pole takes
    // 4 ÷ π of the mean light and the equator 8 ÷ π², so the warm pole's contrast is
    // (8 ÷ π²)^¼ − (4 ÷ π)^¼ over the cold pole's (4 ÷ π)^¼ at 0°, by hand; it changes sign
    // where the equator's and the poles' light are equal, at 53.90° and 126.10°.
    let table = table();
    let (_, moon, _) = found(&table, "Moon");
    let (bare, dry) = column_of(moon, spin_of("Moon"));
    let airless = |degrees: f64| tilted(&bare, dry, degrees);
    let pi = core::f64::consts::PI;
    let by_hand = (math::powf(8.0 / (pi * pi), 0.25) - math::powf(4.0 / pi, 0.25))
        / math::powf(4.0 / pi, 0.25);
    assert!(
        (airless(90.0) / airless(0.0) - by_hand).abs() < 1e-12,
        "{}",
        airless(90.0) / airless(0.0)
    );
    assert!(
        airless(53.8) > 0.0 && airless(54.0) < 0.0,
        "{} {}",
        airless(53.8),
        airless(54.0)
    );
    assert!(airless(126.0) < 0.0 && airless(126.2) > 0.0);
    // Under transport the zero moves toward 54.7°, where P₂(cos β) vanishes: the measured Earth's
    // lies near 54.45° (science-p14-t24a-climate.md §7). The contrast is symmetric about 90°.
    assert!(
        at(54.0) > 0.0 && at(54.8) < 0.0,
        "{} {}",
        at(54.0),
        at(54.8)
    );
    assert!(
        at(125.2) < 0.0 && at(126.0) > 0.0,
        "{} {}",
        at(125.2),
        at(126.0)
    );
    assert!((at(30.0) - at(150.0)).abs() < 1e-9);
    // Through the derivation: the table's Earth tilted to 90°.
    let (_, earth, conditions) = found(&table, "Earth");
    assert!(conditions.equator_pole_contrast_k() > 0.0);
    let tilted = conditions_of(
        earth,
        SeasonalSpin {
            obliquity: Radians::from(Degrees::new(90.0)),
            ..spin_of("Earth")
        },
    )
    .unwrap();
    assert!(tilted.equator_pole_contrast_k() < 0.0);
}

#[test]
fn a_low_obliquity_world_s_contrast_is_its_transport_s() {
    // The table's Earth at 0° and at 3° of obliquity: its contrast within 45–70 K, where damping
    // the whole radiative contrast at P₂'s rate gave 217 and 113 K (science-p14-t24a-climate.md
    // §7: its narrow polar cap lies in modes that any transport erases).
    let table = table();
    let (_, earth, _) = found(&table, "Earth");
    for degrees in [0.0, 3.0] {
        let conditions = conditions_of(
            earth,
            SeasonalSpin {
                obliquity: Radians::from(Degrees::new(degrees)),
                ..spin_of("Earth")
            },
        )
        .unwrap();
        let contrast = conditions.equator_pole_contrast_k();
        eprintln!("Earth at {degrees}°: {contrast} K");
        assert!((45.0..70.0).contains(&contrast), "{degrees}°: {contrast} K");
    }
}

#[test]
fn the_zonal_ends_follow_a_nonlinear_diffusive_balance() {
    // Science-p14-t24a-climate.md §7's nonlinear one-dimensional diffusive balance, T_s = 1,
    // B = 4, dry and at fixed τ (`sweep7.json`): (obliquity in degrees, 6D ÷ B, T₀, T₉₀).
    const BALANCE: [(f64, f64, f64, f64); 24] = [
        (0.5, 0.01, 1.0619, 0.6183),
        (0.5, 0.1, 1.0585, 0.7577),
        (0.5, 1.0, 1.0351, 0.9053),
        (0.5, 15.0, 1.0046, 0.9893),
        (4.0, 0.01, 1.0616, 0.6396),
        (4.0, 0.1, 1.0582, 0.7633),
        (4.0, 1.0, 1.0349, 0.9064),
        (4.0, 15.0, 1.0046, 0.9894),
        (10.0, 0.01, 1.0599, 0.7088),
        (10.0, 0.1, 1.0566, 0.7873),
        (10.0, 1.0, 1.0337, 0.9121),
        (10.0, 15.0, 1.0044, 0.9899),
        (23.44, 0.01, 1.0509, 0.8461),
        (23.44, 0.1, 1.0478, 0.8661),
        (23.44, 1.0, 1.0279, 0.9358),
        (23.44, 15.0, 1.0036, 0.9924),
        (45.0, 0.01, 1.0227, 0.9741),
        (45.0, 0.1, 1.0206, 0.9745),
        (45.0, 1.0, 1.0108, 0.9836),
        (45.0, 15.0, 1.0013, 0.9978),
        (90.0, 0.01, 0.9503, 1.0619),
        (90.0, 0.1, 0.9571, 1.0586),
        (90.0, 1.0, 0.9786, 1.0356),
        (90.0, 15.0, 0.9974, 1.0047),
    ];
    let transport = |six_d_over_b: f64| ZonalTransport {
        planck: 4.0,
        effective: 4.0,
        conductance: six_d_over_b * 4.0 / 6.0,
        weak: 0.0,
        albedo: 1.0,
    };
    for (degrees, six_d_over_b, equator, pole) in BALANCE {
        let obliquity = Radians::from(Degrees::new(degrees));
        let (modal_equator, modal_pole) = zonal_ends(1.0, &transport(six_d_over_b), obliquity);
        let contrast = (modal_equator - modal_pole) / (equator - pole) - 1.0;
        assert!(
            contrast.abs() < 0.06
                && (modal_equator - equator).abs() < 0.002
                && (modal_pole - pole).abs() < 0.02,
            "{degrees}° at {six_d_over_b}: ({modal_equator}, {modal_pole}) against ({equator}, \
             {pole})"
        );
        // With no transport the ends are the radiative ones, to the bit.
        let (q_eq, q_pole) = annual_insolation(obliquity);
        let (bare_equator, bare_pole) = zonal_ends(1.0, &transport(0.0), obliquity);
        assert_same_bits(bare_equator, math::powf(q_eq, 0.25));
        assert_same_bits(bare_pole, math::powf(q_pole, 0.25));
    }
    // The adopted values at two of the rows (the ruling's `zonal7.py`).
    let at = |degrees: f64, six_d_over_b: f64| {
        zonal_ends(
            1.0,
            &transport(six_d_over_b),
            Radians::from(Degrees::new(degrees)),
        )
    };
    for (degrees, six_d_over_b, adopted_equator, adopted_pole) in
        [(0.5, 0.1, 1.0587, 0.7732), (23.44, 1.0, 1.0287, 0.9390)]
    {
        let (equator, pole) = at(degrees, six_d_over_b);
        assert!(
            (equator - adopted_equator).abs() < 1e-4 && (pole - adopted_pole).abs() < 1e-4,
            "{degrees}° at {six_d_over_b}: {equator} {pole}"
        );
    }
}

#[test]
fn the_insolation_modes_are_the_annual_insolation_s() {
    // The modes: q₂ = −(5/8) P₂(cos β) and q₄ = −(9/64) P₄(cos β), and their partial sums approach
    // the ends' insolation, within 10⁻³ at 23.44°, while the pole's tail at 0.5° is some −0.028,
    // which the tail term carries.
    for degrees in [0.5, 23.44, 60.0, 90.0] {
        let obliquity = Radians::from(Degrees::new(degrees));
        let c = math::cos(obliquity.value());
        let modes = insolation_modes(obliquity);
        let p_2 = (3.0 * c * c - 1.0) / 2.0;
        let p_4 = (35.0 * math::powi(c, 4) - 30.0 * c * c + 3.0) / 8.0;
        assert!(
            (modes[0].coefficient + 5.0 / 8.0 * p_2).abs() < 1e-15,
            "{degrees}°"
        );
        assert!(
            (modes[1].coefficient + 9.0 / 64.0 * p_4).abs() < 1e-15,
            "{degrees}°"
        );
        assert_same_bits(modes[0].degree, 2.0);
        assert_same_bits(modes[ZONAL_MODES - 1].degree, 32.0);
    }
    let tails = |degrees: f64| {
        let obliquity = Radians::from(Degrees::new(degrees));
        let modes = insolation_modes(obliquity);
        let (q_eq, q_pole) = annual_insolation(obliquity);
        let sum = |legendre: &ModeValues| {
            modes
                .iter()
                .zip(legendre)
                .fold(1.0, |sum, (mode, &at)| sum + mode.coefficient * at)
        };
        let at_equator = modes.map(|mode| mode.at_equator);
        (q_eq - sum(&at_equator), q_pole - sum(&[1.0; ZONAL_MODES]))
    };
    let (equator, pole) = tails(23.44);
    assert!(
        equator.abs() < 1e-3 && pole.abs() < 1e-3,
        "{equator} {pole}"
    );
    let (_, pole) = tails(0.5);
    assert!((-0.035..-0.02).contains(&pole), "{pole}");
}

#[test]
fn a_moon_s_contrasts_read_its_obliquity_to_its_planet_s_orbit() {
    // A Titan: its orbit in its planet's equator, 26.73° from the planet's orbit, and its spin
    // along its own orbit's normal. Its seasons are its planet's at that tilt.
    let tilt = Radians::from(Degrees::new(26.73));
    let saturn = |mu: GravitationalParameter, a: f64, inclination: Radians| {
        KeplerElements::from_semi_major_axis(
            Metres::new(a),
            mu,
            Eccentricity::new(0.0).unwrap(),
            Orientation::new(inclination, Radians::ZERO, Radians::ZERO).unwrap(),
            Radians::ZERO,
        )
        .unwrap()
    };
    let planet_orbit = saturn(
        GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
        9.58 * METRES_PER_AU,
        Radians::ZERO,
    );
    let moon_orbit = saturn(GravitationalParameter::new(3.793_1e16), 1.221_87e9, tilt);
    let frame = BodyFixedFrame::new(
        &moon_orbit,
        &FrameSpin {
            obliquity: Radians::ZERO,
            pole_azimuth: Radians::ZERO,
            primordial_period: Seconds::new(15.945 * SECONDS_PER_DAY),
            locking_time: Seconds::new(1e10),
            phase_at_epoch: Radians::ZERO,
        },
        Years::new(4.57e9),
    )
    .unwrap();
    let t = UniverseTime::EPOCH;
    let seasonal = SeasonalSpin::of(&frame, t, &planet_orbit, SeasonsFrom::Planet);
    assert!(
        (seasonal.obliquity.value() - tilt.value()).abs() < 1e-9,
        "{:?}",
        seasonal.obliquity
    );
    // Its own orbit would give it none, and no seasons' contrast.
    let own = SeasonalSpin::of(&frame, t, &moon_orbit, SeasonsFrom::Own);
    assert!(own.obliquity.value().abs() < 1e-12);
    // Locked to its planet, it still turns under the Sun, once in about its orbit.
    let day = seasonal.solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((day - 15.97).abs() < 0.05, "{day} d");
    assert_eq!(own.solar_day, None);
    let (column, _) = earth_column();
    let tilted = equator_pole_contrast_k(&column, &seasonal, OceanWeight::FULL);
    let untilted = equator_pole_contrast_k(&column, &own, OceanWeight::FULL);
    let at_tilt = SeasonalSpin {
        obliquity: tilt,
        ..own
    };
    assert!((tilted - equator_pole_contrast_k(&column, &at_tilt, OceanWeight::FULL)).abs() < 1e-9);
    assert!(
        untilted - tilted > 1.0,
        "{untilted} K untilted, {tilted} K tilted"
    );
}

#[test]
fn the_solar_day_is_the_sun_s_rate_about_the_pole() {
    // Venus, turning retrograde once in 243.025 d on a 224.701 d orbit: 116.75 d, through a frame
    // tilted 177.36° and through the table's spin.
    let venus_orbit = KeplerElements::from_period(
        Seconds::new(224.701 * SECONDS_PER_DAY),
        GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
        Eccentricity::new(0.0068).unwrap(),
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
        Radians::ZERO,
    )
    .unwrap();
    let venus_frame = BodyFixedFrame::new(
        &venus_orbit,
        &FrameSpin {
            obliquity: Radians::from(Degrees::new(177.36)),
            pole_azimuth: Radians::ZERO,
            primordial_period: Seconds::new(243.025 * SECONDS_PER_DAY),
            locking_time: Seconds::new(f64::INFINITY),
            phase_at_epoch: Radians::ZERO,
        },
        Years::new(4.57e9),
    )
    .unwrap();
    let retrograde = SeasonalSpin::of(
        &venus_frame,
        UniverseTime::EPOCH,
        &venus_orbit,
        SeasonsFrom::Own,
    );
    let day = retrograde.solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((day - 116.75).abs() < 0.02, "{day} d");
    let venus = spin_of("Venus").solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((venus - 116.75).abs() < 0.02, "{venus} d");
    let mercury = spin_of("Mercury").solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((mercury - 175.94).abs() < 0.02, "{mercury} d");
    let earth = spin_of("Earth").solar_day.unwrap().value() / SECONDS_PER_DAY;
    assert!((earth - 1.0).abs() < 1e-4, "{earth} d");
    // A planet locked 1:1 to its star has none.
    let orbit = orbit(0.05, 0.0);
    let frame = BodyFixedFrame::new(
        &orbit,
        &FrameSpin {
            obliquity: Radians::from(Degrees::new(5.0)),
            pole_azimuth: Radians::ZERO,
            primordial_period: Seconds::new(SECONDS_PER_DAY),
            locking_time: Seconds::new(1e13),
            phase_at_epoch: Radians::ZERO,
        },
        Years::new(4.57e9),
    )
    .unwrap();
    assert_eq!(
        frame.state_at(UniverseTime::EPOCH),
        SpinState::Locked(SpinOrbitResonance::Synchronous)
    );
    let locked = SeasonalSpin::of(&frame, UniverseTime::EPOCH, &orbit, SeasonsFrom::Own);
    assert_eq!(locked.solar_day, None);
    assert!((locked.rotation_period.value() / orbit.period().value() - 1.0).abs() < 1e-12);
}

#[test]
fn the_rotation_factor_follows_kaspi_and_showman() {
    let at =
        |ratio: f64| rotation_transport_factor(Seconds::new(EARTH_SIDEREAL_DAY.value() / ratio));
    assert!((at(1.0) - 1.0).abs() < 1e-15);
    // R09's transport law: about 1.7 at Ω⊕ ÷ 8 and 0.52 at 4 Ω⊕.
    assert!((at(1.0 / 8.0) - 1.68).abs() < 0.01, "{}", at(1.0 / 8.0));
    assert!((at(4.0) - 0.5).abs() < 1e-12);
    // Held beyond the runs' range, and continuous through Earth's rate.
    assert_same_bits(at(1.0 / 100.0), at(1.0 / 24.0));
    assert_same_bits(at(100.0), at(12.0));
    assert!((at(1.0 - 1e-9) - at(1.0 + 1e-9)).abs() < 1e-8);
    assert_same_bits(
        rotation_transport_factor(Seconds::new(f64::INFINITY)),
        at(1.0 / 24.0),
    );
}

#[test]
fn the_redistribution_is_koll_s() {
    // His eq. 9's k at his three planets (Koll 2022, Table 1: radius in R⊕, gravity in m s⁻²),
    // under N₂ at the registry's molar mass: 1.23, 1.89 and 2.35 (science-p14-t24a-climate.md
    // §4) against his 1.2, 1.9 and 2.3. LHS 3844b's 2.353 would round to 2.4, so his own
    // evaluation sits some 2% lower, far inside χ's factor of two: within 3% of each.
    let (earth, _) = earth_column();
    let n2 = Gas::Nitrogen.molar_mass_g_per_mol();
    for (planet, radius, g, k) in [
        ("TRAPPIST-1b", 1.12, 7.95, 1.2),
        ("GJ1132b", 1.16, 11.8, 1.9),
        ("LHS3844b", 1.32, 12.9, 2.3),
    ] {
        let column = Column {
            radius: Metres::new(radius * 6.371e6),
            surface_gravity: MetresPerSecondSquared::new(g),
            molar_mass_g_per_mol: n2,
            ..earth
        };
        let scale = redistribution_scale(&column);
        eprintln!("{planet}: k {scale}");
        assert!((scale / k - 1.0).abs() < 0.03, "{planet}: {scale}");
    }
    // By hand: X = (1.5τ)^⅓ (p ÷ 1 bar)^⅔ (T_eq ÷ 600 K)^−4/3, and ε = X ÷ (k + X).
    let x = math::cbrt(1.5 * earth.optical_depth)
        * math::powf(earth.surface_pressure.value() / 1e5, 2.0 / 3.0)
        * math::powf(earth.equilibrium_temperature.value() / 600.0, -4.0 / 3.0);
    assert!((redistribution_parameter(&earth) - x).abs() < 1e-15);
    let k = redistribution_scale(&earth);
    let epsilon = heat_redistribution(&earth);
    assert!((epsilon - x / (k + x)).abs() < 1e-15);
    eprintln!("Earth: X {x}, k {k}, eps {epsilon}");
    assert!((1.2..1.5).contains(&k), "{k}");
    // No air or no greenhouse redistributes nothing; a deep air nearly all.
    let bare = Column {
        surface_pressure: Pascals::ZERO,
        ..earth
    };
    assert_same_bits(heat_redistribution(&bare), 0.0);
    let clear = Column {
        optical_depth: 0.0,
        ..earth
    };
    assert_same_bits(heat_redistribution(&clear), 0.0);
    let deep = Column {
        surface_pressure: Pascals::new(9e6),
        optical_depth: 100.0,
        ..earth
    };
    assert!(heat_redistribution(&deep) > 0.99);
    // A lighter air redistributes more: k goes as μ^4/3.
    let light = Column {
        molar_mass_g_per_mol: 2.016,
        ..earth
    };
    let ratio = redistribution_scale(&light) / k;
    assert!(
        (ratio - math::powf(2.016 / EARTH_AIR_MOLAR_MASS_G_PER_MOL, 4.0 / 3.0)).abs() < 1e-12,
        "{ratio}"
    );
    // The generated bodies' own k (science-p14-t24a-climate.md §4): Earth 1.34, Venus 2.11 and
    // Titan 0.073.
    let table = table();
    for (name, k) in [("Earth", 1.34), ("Venus", 2.11), ("Titan", 0.073)] {
        let (_, body, _) = found(&table, name);
        let (column, _) = column_of(body, spin_of(name));
        let scale = redistribution_scale(&column);
        eprintln!("{name}: k {scale}, X {}", redistribution_parameter(&column));
        assert!(((scale - k) / k).abs() < 0.05, "{name}: {scale}");
    }
}

#[test]
fn a_rotating_column_damps_the_day_night_contrast() {
    let (earth, spin) = earth_column();
    let floor = COSMIC_BACKGROUND_TEMPERATURE;
    let at = |days: Option<f64>| {
        let (day, night) = day_and_night(
            &earth,
            &SeasonalSpin {
                solar_day: days.map(|d| Seconds::new(d * SECONDS_PER_DAY)),
                ..spin
            },
            GroundInertia::of_material(SurfaceMaterial::Rock),
            floor,
        );
        day.value() - night.value()
    };
    let locked = at(None);
    // Cowan and Agol's contrast at Koll's ε and the floor's, by hand: a locked world's ground
    // stores nothing.
    let epsilon = heat_redistribution(&earth)
        + math::powi(floor.value() / earth.equilibrium_temperature.value(), 4);
    let by_hand =
        288.0 * (math::powf(8.0 / 3.0 - 5.0 / 3.0 * epsilon, 0.25) - math::powf(epsilon, 0.25));
    assert!(
        (locked - by_hand).abs() < 1e-9,
        "{locked} against {by_hand}"
    );
    // On a rotator each departure is damped by 1 ÷ √(1 + (ω C (1 − ε) ÷ B_P)²), at the ε of the
    // air's and the ground's in series.
    let rotating = Some(Seconds::new(SECONDS_PER_DAY));
    let daily = SeasonalSpin {
        solar_day: rotating,
        ..spin
    };
    let ground = ground_redistribution(
        &earth,
        &daily,
        GroundInertia::of_material(SurfaceMaterial::Rock),
    );
    let series = 1.0 - (1.0 - heat_redistribution(&earth)) * (1.0 - ground)
        + math::powi(floor.value() / earth.equilibrium_temperature.value(), 4);
    let omega_tau = core::f64::consts::TAU / SECONDS_PER_DAY
        * earth.heat_capacity_j_per_m2_k()
        * (1.0 - series)
        / earth.restoring_w_per_m2_k();
    let undamped =
        288.0 * (math::powf(8.0 / 3.0 - 5.0 / 3.0 * series, 0.25) - math::powf(series, 0.25));
    assert!((at(Some(1.0)) - undamped / math::hypot(1.0, omega_tau)).abs() < 1e-9);
    eprintln!(
        "Earth: locked {locked} K, a day's {} K, ground eps {ground}",
        at(Some(1.0))
    );
    // Longer days damp less, and the locked contrast is the limit of an ever longer day, whose
    // ground stores ever less.
    let mut last = 0.0;
    for days in [0.1, 1.0, 10.0, 100.0, 1e4, 1e8] {
        let contrast = at(Some(days));
        assert!(
            contrast > last && contrast < locked,
            "{days} d: {contrast} K"
        );
        last = contrast;
    }
    assert!((at(Some(1e14)) / locked - 1.0).abs() < 1e-6);
    // Venus's and Titan's thick airs leave under 10 K and under 1 K.
    let table = table();
    for (name, limit) in [("Venus", 10.0), ("Titan", 1.0)] {
        let contrast = found(&table, name).2.day_night_contrast_k();
        assert!((0.0..limit).contains(&contrast), "{name}: {contrast} K");
    }
}

#[test]
fn an_airless_night_is_its_regolith_s() {
    // The ruling's five pairs of its constant-inertia model (science-p14-t24a-climate.md §2),
    // within 3%.
    for (theta, epsilon) in [
        (0.0051, 0.0053),
        (0.028, 0.0246),
        (0.152, 0.103),
        (0.506, 0.254),
        (1.52, 0.480),
    ] {
        let fitted = regolith_redistribution(theta);
        assert!(
            (fitted / epsilon - 1.0).abs() < 0.03,
            "Θ {theta}: {fitted} against {epsilon}"
        );
    }
    // The thermal parameter by hand, Γ √ω ÷ (ε_r σ T_ss³), on the table's Moon.
    let table = table();
    let (_, moon, _) = found(&table, "Moon");
    let spin = spin_of("Moon");
    let (column, _) = column_of(moon, spin);
    assert_eq!(
        SurfaceMaterial::of(&moon.fractions()),
        SurfaceMaterial::Rock
    );
    let subsolar = column.equilibrium_temperature.value() * math::powf(4.0 / 0.95, 0.25);
    let omega = core::f64::consts::TAU / spin.solar_day.unwrap().value();
    let theta = 55.0 * omega.sqrt() / (0.95 * STEFAN_BOLTZMANN * math::powi(subsolar, 3));
    eprintln!("the Moon's Θ {theta}");
    assert!((theta - 0.028).abs() < 0.002, "{theta}");
    assert!(
        (ground_redistribution(
            &column,
            &spin,
            GroundInertia::of_material(SurfaceMaterial::Rock)
        ) - regolith_redistribution(theta))
        .abs()
            < 1e-15
    );
    // The Moon's night (Diviner: about 95 K before dawn, a night mean of some 100–110 K) and
    // Mercury's (Mariner 10: 100–130 K), each the hemisphere's.
    for (name, low, high) in [("Moon", 90.0, 125.0), ("Mercury", 90.0, 130.0)] {
        let (_, body, _) = found(&table, name);
        let (_, night) = day_and_night_of(body, spin_of(name));
        eprintln!("{name}'s night: {} K", night.value());
        assert!((low..high).contains(&night.value()), "{name}: {night:?}");
    }
    // An icy body stores less at its lower inertia.
    let ice = ground_redistribution(
        &column,
        &spin,
        GroundInertia::of_material(SurfaceMaterial::Ice),
    );
    assert!(
        ice < ground_redistribution(
            &column,
            &spin,
            GroundInertia::of_material(SurfaceMaterial::Rock)
        )
    );
    assert!((ice - regolith_redistribution(theta * 20.0 / 55.0)).abs() < 1e-15);
}

#[test]
fn a_locked_airless_body_s_night_is_its_floor() {
    let table = table();
    let (_, mercury, _) = found(&table, "Mercury");
    let locked = SeasonalSpin {
        solar_day: None,
        ..spin_of("Mercury")
    };
    let (column, _) = column_of(mercury, locked);
    assert_same_bits(
        ground_redistribution(
            &column,
            &locked,
            GroundInertia::of_material(SurfaceMaterial::Rock),
        ),
        0.0,
    );
    // Its night is its heat flow's floor (P14.T24.b; science-p14-t24b-laws §1.5): about 21 K at
    // Mercury's 10.8 mW m⁻², to 10⁻¹².
    let floor = night_floor(heat_flow_of(mercury));
    eprintln!("the locked Mercury's night: {} K", floor.value());
    assert!((20.0..23.0).contains(&floor.value()), "{floor:?}");
    let (_, night) = day_and_night_of(mercury, locked);
    assert!(
        (night.value() / floor.value() - 1.0).abs() < 1e-12,
        "{night:?}"
    );
    // With no internal heat it is the cosmic background's, and a warmer floor is its night too.
    for floor in [COSMIC_BACKGROUND_TEMPERATURE, Kelvin::new(30.0)] {
        let (_, dark) = day_and_night(
            &column,
            &locked,
            GroundInertia::of_material(SurfaceMaterial::Rock),
            floor,
        );
        assert!(
            (dark.value() / floor.value() - 1.0).abs() < 1e-12,
            "{dark:?}"
        );
    }
    let conditions = conditions_of(mercury, locked).unwrap();
    assert_same_bits(conditions.climate_range().0.value(), floor.value());
}

#[test]
fn the_night_floor_is_the_heat_flow_s() {
    // Science-p14-t24a-climate.md §2: (T_CMB⁴ + F ÷ ε_r σ)^¼, the cosmic background's with no
    // heat. Its §6's 26.1 K and 24.0 K are withdrawn (science-p14-t24b-laws §1.5): the floors
    // follow from each body's own F, the Moon's 11.9 mW m⁻² at 21.7 K and Mercury's 11.0 at
    // 21.2 K. The table's Moon and Mercury lie in 20–23 K.
    assert_eq!(
        night_floor(WattsPerSquareMetre::new(0.0)),
        COSMIC_BACKGROUND_TEMPERATURE
    );
    for f in [-1.0, f64::NAN] {
        assert_eq!(
            night_floor(WattsPerSquareMetre::new(f)),
            COSMIC_BACKGROUND_TEMPERATURE
        );
    }
    for (flow, kelvin) in [(0.0119, 21.7), (0.0110, 21.2)] {
        let floor = night_floor(WattsPerSquareMetre::new(flow)).value();
        assert!((floor - kelvin).abs() < 0.1, "{flow}: {floor}");
    }
    let table = table();
    for name in ["Moon", "Mercury"] {
        let floor = night_floor(heat_flow_of(&found(&table, name).1)).value();
        eprintln!("the table's {name}'s floor: {floor} K");
        assert!((20.0..23.0).contains(&floor), "{name}: {floor}");
    }
    let by_hand = math::powf(
        math::powi(COSMIC_BACKGROUND_TEMPERATURE.value(), 4)
            + 0.0926 / (REGOLITH_EMISSIVITY * STEFAN_BOLTZMANN),
        0.25,
    );
    assert_same_bits(
        night_floor(WattsPerSquareMetre::new(0.0926)).value(),
        by_hand,
    );
    // Continuous as the heat goes, and rising with it.
    let tiny = night_floor(WattsPerSquareMetre::new(1e-30)).value();
    assert!((tiny - COSMIC_BACKGROUND_TEMPERATURE.value()).abs() < 1e-12);
    let mut last = COSMIC_BACKGROUND_TEMPERATURE.value();
    for step in 1..=100 {
        let floor = night_floor(WattsPerSquareMetre::new(0.001 * f64::from(step))).value();
        assert!(floor > last, "{step}");
        last = floor;
    }
}

#[test]
fn the_night_is_continuous_across_the_airless_threshold() {
    // An Earth whose air thins to nothing joins the bare rock's day and night: the air's ε and
    // its storage vanish with its pressure.
    let (earth, spin) = earth_column();
    let floor = COSMIC_BACKGROUND_TEMPERATURE;
    let thinned = |scale: f64| Column {
        surface_pressure: Pascals::new(earth.surface_pressure.value() * scale),
        air_mass_kg_per_m2: earth.air_mass_kg_per_m2 * scale,
        ..earth
    };
    let (bare_day, bare_night) = day_and_night(
        &thinned(0.0),
        &spin,
        GroundInertia::of_material(SurfaceMaterial::Rock),
        floor,
    );
    let mut last = f64::INFINITY;
    for scale in [1e-2, 1e-4, 1e-6, 1e-8, 1e-10] {
        let (day, night) = day_and_night(
            &thinned(scale),
            &spin,
            GroundInertia::of_material(SurfaceMaterial::Rock),
            floor,
        );
        let apart =
            (day.value() - bare_day.value()).abs() + (night.value() - bare_night.value()).abs();
        assert!(apart < last, "{scale}: {apart} K");
        last = apart;
    }
    assert!(last < 1e-3, "{last} K");
}

#[test]
fn a_solar_day_that_is_not_positive_is_none() {
    // The fields are public: a day of zero, less or not a number is taken as no solar day, as a
    // world locked 1:1 to its seasonal host has, and gives finite conditions.
    let table = table();
    for (name, body, _) in &table {
        let base = spin_of(name);
        let locked = conditions_of(
            body,
            SeasonalSpin {
                solar_day: None,
                ..base
            },
        )
        .unwrap();
        for day in [0.0, -1.0, f64::NAN] {
            let odd = conditions_of(
                body,
                SeasonalSpin {
                    solar_day: Some(Seconds::new(day)),
                    ..base
                },
            )
            .unwrap();
            assert!(odd.day_night_contrast_k().is_finite(), "{name} at {day} s");
            assert_eq!(odd, locked, "{name} at {day} s");
        }
    }
}

#[test]
fn every_wet_world_s_water_feeds_back() {
    // T13.c holds a wet world's water at its saturation pressure at its surface temperature, so
    // its gain is positive: Earth-like bodies of 0.5 to 4 M⊕ from 0.90 to 1.30 au of the Sun, each
    // of them with an ocean weight (P14.T24.b's θ).
    let disc = solar_disc();
    let mut wet = 0_u32;
    for mass in [0.5, 1.0, 2.0, 4.0] {
        for step in 0..=40 {
            let a = 0.9 + 0.01 * f64::from(step);
            let o = orbit(a, 0.0);
            let placed = PlacedBody::new(
                EarthMasses::new(mass),
                o,
                o.semi_major_axis(),
                UnitUniform::HALF,
            )
            .unwrap();
            let body = derive_historic(&placed, &disc, SOLAR_AGE);
            if body.atmosphere().state() == SurfaceState::GasEnvelope {
                continue;
            }
            let (column, weight) = column_of(&body, spin_of("Earth"));
            if weight.get() > 0.0 {
                wet += 1;
                assert!(
                    column.greenhouse_gain > 0.0,
                    "{mass} M⊕ at {a} au: {:?}",
                    body.atmosphere().state()
                );
            }
        }
    }
    eprintln!("{wet} wet worlds");
    assert!(wet > 5, "{wet}");
}

#[test]
fn no_light_gives_no_contrast() {
    let (earth, spin) = earth_column();
    let dark = Column {
        equilibrium_temperature: Kelvin::ZERO,
        ..earth
    };
    assert_same_bits(
        equator_pole_contrast_k(&dark, &spin, OceanWeight::FULL),
        0.0,
    );
    let (day, night) = day_and_night(
        &dark,
        &spin,
        GroundInertia::of_material(SurfaceMaterial::Rock),
        COSMIC_BACKGROUND_TEMPERATURE,
    );
    assert_eq!(
        (day, night),
        (dark.surface_temperature, dark.surface_temperature)
    );
    assert_same_bits(heat_redistribution(&dark), 0.0);
    assert_same_bits(
        ground_redistribution(
            &dark,
            &spin,
            GroundInertia::of_material(SurfaceMaterial::Rock),
        ),
        0.0,
    );
    assert_same_bits(weak_gradient_transport(&dark, &spin), 0.0);
}

#[test]
fn the_table_s_climate_ranges_are_the_ruling_s() {
    // Science-p14-t24a-climate.md §6's ranges (item 1's zonal ends; the table's Ceres is rock,
    // item 2's erratum), the bodies with air at §7's modal ends, with the airless floors at their
    // own heat flows' (P14.T24.b; science-p14-t24b-laws §1.5): 21.1 K on Mercury and 20.0 K on the
    // table's Moon, which the table derives as a planet with Earth's core fraction.
    let table = table();
    for name in ["Mercury", "Moon"] {
        let (_, body, conditions) = found(&table, name);
        assert_same_bits(
            conditions.climate_range().0.value(),
            night_floor(heat_flow_of(body)).value(),
        );
    }
    // Under the batch the table's Venus is the median draw at P14.T57.a's constants, 696.3 K
    // (decision-p14-t57a-fits §3), where the ruling's 733.5–736.1 K was version 22's 735.1 K.
    // Mars and Ganymede are printed: at the passes' stand-ins Mars ices over (196.6 K;
    // decision-p14-t51-batch E3) until P14.T54.a's Batch C, and Ganymede, airless, lies under its
    // frost's albedo at 22.3–98.6 K (`T_eq` 78 K against 109.6 K at its measured 0.35, P14.T49.m's;
    // decision-p14-t54a-ganymede §1.5); the ruling's [203.3, 221.2] and [17.9, 142.8] K were
    // version 22's.
    for name in ["Mars", "Ganymede"] {
        let (cold, warm) = found(&table, name).2.climate_range();
        eprintln!(
            "{name} at the passes' stand-ins: [{:.2}, {:.2}] K",
            cold.value(),
            warm.value()
        );
    }
    for (name, coldest, warmest) in [
        ("Earth", 261.7, 305.1),
        ("Mercury", 21.1, 585.2),
        ("Moon", 20.0, 361.0),
        // The batch's own Venus, as P14.T54.a's B3 record and its bless note measure it.
        ("Venus", 694.9, 697.2),
        ("Ceres", 56.7, 202.9),
    ] {
        let (cold, warm) = found(&table, name).2.climate_range();
        eprintln!("{name}: [{:.2}, {:.2}] K", cold.value(), warm.value());
        assert!(
            (cold.value() - coldest).abs() < 1.5,
            "{name}'s coldest: {cold:?}"
        );
        assert!(
            (warm.value() - warmest).abs() < 1.5,
            "{name}'s warmest: {warm:?}"
        );
    }
    let (cold, warm) = found(&table, "Earth").2.climate_range();
    assert!((255.0..265.0).contains(&cold.value()) && (300.0..310.0).contains(&warm.value()));
    for (name, material) in [
        ("Ceres", SurfaceMaterial::Rock),
        ("Ganymede", SurfaceMaterial::Ice),
    ] {
        assert_eq!(
            SurfaceMaterial::of(&found(&table, name).1.fractions()),
            material
        );
    }
    // No body's coldest lies below its heat flow's floor: the table's, locked, and tilted to 90°.
    for (name, body, conditions) in &table {
        let base = spin_of(name);
        let locked = conditions_of(
            body,
            SeasonalSpin {
                solar_day: None,
                ..base
            },
        )
        .unwrap();
        let tilted = conditions_of(
            body,
            SeasonalSpin {
                obliquity: Radians::from(Degrees::new(90.0)),
                ..base
            },
        )
        .unwrap();
        let floor = night_floor(heat_flow_of(body));
        for c in [conditions, &locked, &tilted] {
            let (cold, warm) = c.climate_range();
            assert!(cold >= floor, "{name}: {cold:?}");
            assert!(warm >= cold, "{name}: {cold:?} to {warm:?}");
            assert!(
                cold <= c.mean_surface_temperature() && warm >= c.mean_surface_temperature(),
                "{name}"
            );
        }
    }
    // By hand: the zonal profile's ends, with the day's excess and the night's deficit.
    let (_, earth, conditions) = found(&table, "Earth");
    let spin = spin_of("Earth");
    let (day, night) = day_and_night_of(earth, spin);
    let (column, moisture) = column_of(earth, spin);
    let t_s = conditions.mean_surface_temperature().value();
    let (equator, pole) = zonal_ends_k(&column, &spin, moisture);
    assert_same_bits(equator - pole, conditions.equator_pole_contrast_k());
    let (cold, warm) = conditions.climate_range();
    assert!((cold.value() - (pole - (t_s - night.value()))).abs() < 1e-9);
    assert!((warm.value() - (equator + (day.value() - t_s))).abs() < 1e-9);
    // A warm-pole world's coldest is its equator and its warmest a pole; a colder end above the
    // mean is held at the mean.
    let t = Kelvin::new(300.0);
    let (cold, warm) = climate_range(t, (290.0, 320.0), t, t, COSMIC_BACKGROUND_TEMPERATURE);
    assert!((cold.value() - 290.0).abs() < 1e-12 && (warm.value() - 320.0).abs() < 1e-12);
    let (cold, _) = climate_range(t, (301.0, 302.0), t, t, COSMIC_BACKGROUND_TEMPERATURE);
    assert!((cold.value() - 300.0).abs() < 1e-12, "{cold:?}");
}

#[test]
fn an_airless_body_s_warmest_lies_between_its_dayside_and_its_subsolar_point() {
    // Science-p14-t24a-climate.md §6, item 1: the range is the orbit-mean climate's, so an
    // airless body's warmest is a dayside, never above its orbit-mean subsolar temperature,
    // T_eq (4 ÷ ε_r)^¼: at its own spin, locked, tilted to 90° and turning in an hour. Its zonal
    // ends are its radiative ones, and every body's differ by its contrast.
    let table = table();
    for name in ["Mercury", "Moon", "Ganymede", "Ceres"] {
        let (_, body, _) = found(&table, name);
        assert_eq!(body.atmosphere().state(), SurfaceState::Airless, "{name}");
        let base = spin_of(name);
        for spin in [
            base,
            SeasonalSpin {
                solar_day: None,
                ..base
            },
            SeasonalSpin {
                obliquity: Radians::from(Degrees::new(90.0)),
                ..base
            },
            SeasonalSpin {
                solar_day: Some(Seconds::new(3_600.0)),
                ..base
            },
        ] {
            let conditions = conditions_of(body, spin).unwrap();
            let (column, moisture) = column_of(body, spin);
            let (day, _) = day_and_night_of(body, spin);
            let subsolar = column.equilibrium_temperature.value()
                * math::powf(4.0 / REGOLITH_EMISSIVITY, 0.25);
            let (_, warm) = conditions.climate_range();
            assert!(
                warm >= day && warm.value() < subsolar,
                "{name}: {warm:?}, {day:?}, {subsolar} K"
            );
            let t_s = column.surface_temperature.value();
            let (q_eq, q_pole) = annual_insolation(spin.obliquity);
            let (equator, pole) = zonal_ends_k(&column, &spin, moisture);
            assert!(
                (equator - t_s * math::powf(q_eq, 0.25)).abs() < 1e-9 * t_s,
                "{name}"
            );
            assert!(
                (pole - t_s * math::powf(q_pole, 0.25)).abs() < 1e-9 * t_s,
                "{name}"
            );
        }
    }
    for (name, body, conditions) in &table {
        let spin = spin_of(name);
        let (column, moisture) = column_of(body, spin);
        let (equator, pole) = zonal_ends_k(&column, &spin, moisture);
        assert!(
            (equator - pole - conditions.equator_pole_contrast_k()).abs() < 1e-9,
            "{name}"
        );
    }
}

#[test]
fn the_zonal_ends_are_continuous_in_the_transport() {
    // The generated Mars's column, its mass and surface pressure scaled together from 10⁻⁶ to 50
    // times its own in steps of 1% (science-p14-t24a-climate.md §6, item 1): at the smallest its
    // ends are its no-transport ones to 10⁻⁴ T_s (the weak gradient falls only as p^⅔), no step
    // moves either end by more than 0.002 T_s, and each end's distance from T_s falls
    // monotonically. The no-transport ends are the radiative ones with the emitted flux's
    // departures amplified by the column's own vapour feedback, `B_P` ÷ `B_eff` (Mars's water is
    // held at saturation, g = 0.025), and are the radiative ones exactly where there is none.
    let table = table();
    let (_, mars, _) = found(&table, "Mars");
    let spin = spin_of("Mars");
    let (column, weight) = column_of(mars, spin);
    // The generated Mars's 20 m of surface water weighs a few hundredths (0.046; the ruling's
    // estimate, about a tenth, science-p14-t24b-laws §4.4). The transport's limits are read dry,
    // under a uniform albedo, where the no-transport ends are the radiative ones.
    assert!((0.02..0.2).contains(&weight.get()), "{weight:?}");
    let moisture = OceanWeight::NONE;
    assert!(column.greenhouse_gain > 0.0);
    let t_s = column.surface_temperature.value();
    let scaled = |scale: f64| Column {
        air_mass_kg_per_m2: column.air_mass_kg_per_m2 * scale,
        surface_pressure: Pascals::new(column.surface_pressure.value() * scale),
        ..column
    };
    let (q_eq, q_pole) = annual_insolation(spin.obliquity);
    let feedback = column.restoring_w_per_m2_k() / column.effective_restoring_w_per_m2_k();
    let unmoved = |q: f64| t_s * math::powf(1.0 + feedback * (q - 1.0), 0.25);
    let (equator, pole) = zonal_ends_k(&scaled(1e-6), &spin, moisture);
    assert!(
        (equator - unmoved(q_eq)).abs() < 1e-4 * t_s,
        "{equator} K against {}",
        unmoved(q_eq)
    );
    assert!(
        (pole - unmoved(q_pole)).abs() < 1e-4 * t_s,
        "{pole} K against {}",
        unmoved(q_pole)
    );
    let bare = Column {
        greenhouse_gain: 0.0,
        ..scaled(1e-6)
    };
    let (equator, _) = zonal_ends_k(&bare, &spin, moisture);
    assert!((equator - t_s * math::powf(q_eq, 0.25)).abs() < 1e-4 * t_s);
    let (equator, pole) = zonal_ends_k(&scaled(1e-6), &spin, moisture);
    let (mut last_equator, mut last_pole) = (equator, pole);
    let mut scale = 1e-6;
    while scale < 50.0 {
        scale *= 1.01;
        let (equator, pole) = zonal_ends_k(&scaled(scale), &spin, moisture);
        assert!(
            (equator - last_equator).abs() < 0.002 * t_s && (pole - last_pole).abs() < 0.002 * t_s,
            "{scale}: {last_equator} to {equator} K, {last_pole} to {pole} K"
        );
        assert!(
            (equator - t_s).abs() <= (last_equator - t_s).abs()
                && (pole - t_s).abs() <= (last_pole - t_s).abs(),
            "{scale}: {equator} and {pole} K"
        );
        (last_equator, last_pole) = (equator, pole);
    }
}

#[test]
fn the_radiation_class_bands_are_decades_of_earth_s() {
    let earth = earth_xuv_flux().value();
    // The generated Sun's present X-ray and ultraviolet flux at 1 au, about 9.75 × 10⁻⁴ W m⁻².
    assert!((earth - 9.75e-4).abs() < 0.01e-4, "{earth}");
    let class = |ratio: f64| RadiationClass::of(WattsPerSquareMetre::new(ratio * earth));
    assert_eq!(
        RadiationClass::of(WattsPerSquareMetre::ZERO),
        RadiationClass::Dark
    );
    assert_eq!(class(f64::NAN), RadiationClass::Dark);
    assert_eq!(class(1.0), RadiationClass::Quiet);
    assert_eq!(class(10.0), RadiationClass::Active);
    assert_eq!(class(99.9), RadiationClass::Active);
    assert_eq!(class(100.0), RadiationClass::Intense);
    assert_eq!(class(1_000.0), RadiationClass::Extreme);
    // The saturated young Sun at 1 au: some 300 times today's.
    let young = XuvHistory::new(SolarMasses::new(1.0), REFERENCE_SUN_ZAMS_LUMINOSITY).flux(
        Years::new(5e7),
        Metres::new(METRES_PER_AU),
        0.0,
    );
    assert_eq!(RadiationClass::of(young), RadiationClass::Intense);
    // The table's planets today are quiet.
    for (name, _, conditions) in table() {
        assert_eq!(conditions.radiation(), RadiationClass::Quiet, "{name}");
    }
}

#[test]
fn a_gas_envelope_has_no_surface_conditions() {
    let disc = solar_disc();
    let &(_, kg, _, a, e) = PLANETS.iter().find(|(n, ..)| *n == "Jupiter").unwrap();
    let jupiter = crate::planetary::derive::solar::derive(
        &crate::planetary::derive::solar::placed(kg, a, e, 0.5),
        &disc,
    )
    .unwrap();
    assert_eq!(jupiter.atmosphere().state(), SurfaceState::GasEnvelope);
    assert_eq!(conditions_of(&jupiter, spin_of("Earth")), None);
}

#[test]
fn the_conditions_are_continuous_in_obliquity() {
    let table = table();
    for (name, body, _) in &table {
        let base = spin_of(name);
        let mut last: Option<SurfaceConditions> = None;
        for step in 0..=1_800 {
            let spin = SeasonalSpin {
                obliquity: Radians::from(Degrees::new(f64::from(step) * 0.1)),
                ..base
            };
            let conditions = conditions_of(body, spin).unwrap();
            if let Some(previous) = &last {
                // Two degrees and more from 0° and 180°, where a pole's light rises as
                // (sin β)^¼, a tenth of a degree moves the contrast by under a percent of the
                // mean temperature.
                let jump = (conditions.equator_pole_contrast_k()
                    - previous.equator_pole_contrast_k())
                .abs();
                let degrees = f64::from(step) * 0.1;
                if (2.0..=178.0).contains(&degrees) {
                    assert!(
                        jump < 0.01 * conditions.mean_surface_temperature().value(),
                        "{name} at {degrees}°: {jump} K"
                    );
                }
                assert_same_bits(
                    conditions.day_night_contrast_k(),
                    previous.day_night_contrast_k(),
                );
                // The range's ends follow the contrast, through 53.9° too.
                let (cold, warm) = conditions.climate_range();
                let (last_cold, last_warm) = previous.climate_range();
                if (2.0..=178.0).contains(&degrees) {
                    let mean = conditions.mean_surface_temperature().value();
                    assert!(
                        (cold.value() - last_cold.value()).abs() < 0.01 * mean
                            && (warm.value() - last_warm.value()).abs() < 0.01 * mean,
                        "{name} at {degrees}°: {cold:?} to {warm:?}"
                    );
                }
            }
            last = Some(conditions);
        }
        // At 0° the contrast is continuous too, its step shrinking as the step's fourth root.
        let at = |degrees: f64| {
            conditions_of(
                body,
                SeasonalSpin {
                    obliquity: Radians::from(Degrees::new(degrees)),
                    ..base
                },
            )
            .unwrap()
            .equator_pole_contrast_k()
        };
        let mut last_jump = f64::INFINITY;
        for degrees in [1e-2, 1e-4, 1e-6, 1e-8] {
            let jump = (at(degrees) - at(0.0)).abs();
            assert!(jump < last_jump, "{name} at {degrees}°: {jump} K");
            last_jump = jump;
        }
        assert!(
            last_jump < 0.01 * at(0.0).abs().max(1.0),
            "{name}: {last_jump} K"
        );
    }
}

#[test]
fn the_conditions_are_the_same_twice() {
    let first = table();
    let second = table();
    assert_eq!(first.len(), second.len());
    for ((name, _, a), (_, _, b)) in first.iter().zip(&second) {
        assert_eq!(a, b, "{name}");
    }
}

#[test]
fn the_small_bodies_of_the_table_are_spun() {
    for (name, ..) in SMALL_BODIES {
        assert!(SPINS.iter().any(|(n, ..)| *n == name), "{name}");
    }
}

#[test]
fn the_conditions_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    let pin = |w: &mut GoldenWriter, name: &str, c: &SurfaceConditions| {
        w.f64(
            &format!("{name}_mean_surface_temperature"),
            c.mean_surface_temperature().value(),
        );
        w.f64(
            &format!("{name}_equator_pole_contrast_k"),
            c.equator_pole_contrast_k(),
        );
        w.f64(
            &format!("{name}_day_night_contrast_k"),
            c.day_night_contrast_k(),
        );
        let (coldest, warmest) = c.climate_range();
        w.f64(&format!("{name}_coldest"), coldest.value());
        w.f64(&format!("{name}_warmest"), warmest.value());
        w.f64(
            &format!("{name}_surface_pressure"),
            c.surface_pressure().value(),
        );
        for &(id, x) in c.gases() {
            w.f64(
                &format!("{name}_mole_fraction_{}", id.substance().key().as_str()),
                x,
            );
        }
        for (label, value) in [
            ("mean_molar_mass_g_per_mol", c.mean_molar_mass_g_per_mol()),
            ("heat_capacity_over_r", c.heat_capacity_over_r()),
        ] {
            match value {
                Some(value) => w.f64(&format!("{name}_{label}"), value),
                None => w.line(&format!("{name}_{label} = None")),
            }
        }
        w.f64(&format!("{name}_stellar_flux"), c.stellar_flux().value());
        w.line(&format!("{name}_radiation = {:?}", c.radiation()));
        w.line(&format!("{name}_liquid_water = {}", c.liquid_water()));
    };
    let table = table();
    for (name, body, conditions) in &table {
        let name = name.to_lowercase();
        pin(&mut w, &name, conditions);
        w.f64(&format!("{name}_xuv_flux"), body.xuv_flux().value());
        // Its climate regime (P14.T48.d), at its own figures' ice and not from a colder state.
        let regime = regime::tests::regime_of(body, conditions, false);
        regime::tests::pin_regime(&mut w, &name, Some(regime));
    }
    // Locked 1:1, a world keeps only its air's redistribution, undamped, over the floor.
    for name in ["Earth", "Venus"] {
        let (_, body, _) = found(&table, name);
        let locked = conditions_of(
            body,
            SeasonalSpin {
                solar_day: None,
                ..spin_of(name)
            },
        )
        .unwrap();
        let name = format!("{}_locked", name.to_lowercase());
        pin(&mut w, &name, &locked);
        let regime = regime::tests::regime_of(body, &locked, false);
        regime::tests::pin_regime(&mut w, &name, Some(regime));
    }
    let (_, earth, _) = found(&table, "Earth");
    let tilted = conditions_of(
        earth,
        SeasonalSpin {
            obliquity: Radians::from(Degrees::new(90.0)),
            ..spin_of("Earth")
        },
    )
    .unwrap();
    pin(&mut w, "earth_at_90_degrees", &tilted);
    let regime = regime::tests::regime_of(earth, &tilted, false);
    regime::tests::pin_regime(&mut w, "earth_at_90_degrees", Some(regime));
    w.f64("earth_xuv_flux", earth_xuv_flux().value());
    let young = XuvHistory::new(SolarMasses::new(1.0), REFERENCE_SUN_ZAMS_LUMINOSITY).flux(
        Years::new(5e7),
        Metres::new(METRES_PER_AU),
        0.0,
    );
    w.f64("young_sun_xuv_flux_at_1_au", young.value());
    w.line(&format!(
        "young_sun_radiation_at_1_au = {:?}",
        RadiationClass::of(young)
    ));
    golden!("planetary/surface_conditions", w.as_str());
}

/// Europa at its measured mean of 102 K, airless at 3.13° (Jupiter's obliquity), lies under 110 K
/// at every latitude: its radiative profile, whose warmest point is 108.3 K, is wholly cold
/// (science-p14-t24b-laws §4.3, "Europa all ice").
#[test]
fn a_europa_at_102_k_is_cold_everywhere() {
    let t = Kelvin::new(102.0);
    let column = Column {
        surface_temperature: t,
        equilibrium_temperature: t,
        surface_pressure: Pascals::ZERO,
        air_mass_kg_per_m2: 0.0,
        specific_heat_j_per_kg_k: 0.0,
        molar_mass_g_per_mol: 0.0,
        surface_gravity: MetresPerSecondSquared::new(1.315),
        optical_depth: 0.0,
        greenhouse_gain: 0.0,
        radius: Metres::new(1.560_8e6),
    };
    let spin = SeasonalSpin {
        obliquity: Radians::from(Degrees::new(3.13)),
        rotation_period: Seconds::new(3.551 * SECONDS_PER_DAY),
        solar_day: Some(Seconds::new(3.554 * SECONDS_PER_DAY)),
    };
    let profile = ZonalProfile::of(&column, &spin, OceanWeight::NONE);
    let points = profile.points();
    let warmest = points.iter().fold(0.0_f64, |high, &(_, t)| high.max(t));
    assert!((warmest - 108.3).abs() < 0.1, "{warmest} K");
    assert_same_bits(area_below(&points, Kelvin::new(110.0)), 1.0);
}

/// The eyeball's dayside profile, `E_n` + (3/2)(`E_d` − `E_n`) μ, emits what the world absorbs
/// at every redistribution and its μ-weighted dayside is Cowan and Agol's `T_d`
/// (science-p14-t24b-laws §4.x.3): with their eqs. 4–5 in `σT_s⁴`'s units, `E_d` = 8/3 − 5ε/3
/// and `E_n` = ε, the sphere's mean is 1 and the day's μ-weighted mean `E_d`, to 10⁻¹².
#[test]
fn the_eyeball_s_profile_conserves_the_absorbed_flux() {
    use crate::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};
    // Gauss–Legendre on μ in [0, 1], exact for the profile and its μ-weighted moment.
    let integral = |f: &dyn Fn(f64) -> f64| {
        GL8_NODES
            .iter()
            .zip(&GL8_WEIGHTS)
            .fold(0.0, |sum, (&x, &w)| {
                sum + 0.5 * w * f(f64::midpoint(x, 1.0))
            })
    };
    for step in 0..=20 {
        let epsilon = f64::from(step) / 20.0;
        let (day, night) = (8.0 / 3.0 - 5.0 / 3.0 * epsilon, epsilon);
        let flux = |mu: f64| night + EYEBALL_SLOPE * (day - night) * mu;
        // Area is uniform in μ over the dayside, half the sphere.
        let mean = 0.5 * integral(&flux) + 0.5 * night;
        assert!((mean - 1.0).abs() < 1e-12, "ε {epsilon}: {mean}");
        let weighted = integral(&|mu| flux(mu) * mu) / integral(&|mu| mu);
        assert!(
            (weighted - day).abs() < 1e-12,
            "ε {epsilon}: {weighted} against {day}"
        );
    }
}

/// The ocean weight θ moves the contrasts continuously (science-p14-t24a-climate.md §1;
/// science-p14-t24b-laws §4.4): the measured Earth's contrast runs from its dry value at θ = 0 to
/// its wet one at 1, by under 0.1 K a step of 10⁻³, the moist share's transport and the
/// brightening growing together.
#[test]
fn the_ocean_weight_moves_the_contrasts_continuously() {
    let (earth, spin) = earth_column();
    let at = |theta: f64| {
        equator_pole_contrast_k(
            &earth,
            &spin,
            OceanWeight::of_potential_ocean(theta * FULL_OCEAN_FRACTION),
        )
    };
    let (dry, wet) = (
        equator_pole_contrast_k(&earth, &spin, OceanWeight::NONE),
        equator_pole_contrast_k(&earth, &spin, OceanWeight::FULL),
    );
    assert_same_bits(at(0.0), dry);
    assert!((at(1.0) - wet).abs() < 1e-9, "{} against {wet}", at(1.0));
    let mut last = dry;
    for step in 1..=1_000 {
        let theta = f64::from(step) / 1_000.0;
        let contrast = at(theta);
        assert!(
            (contrast - last).abs() < 0.1,
            "{theta}: {last} to {contrast} K"
        );
        last = contrast;
    }
    // Its share and brightening are the switch's at either end.
    assert_same_bits(OceanWeight::NONE.transport_share(), DRY_TRANSPORT_SHARE);
    assert_same_bits(OceanWeight::FULL.transport_share(), 1.0);
    assert_same_bits(OceanWeight::NONE.albedo_gradient(), 1.0);
    assert_same_bits(OceanWeight::FULL.albedo_gradient(), WET_ALBEDO_GRADIENT);
}

/// The annual insolation at a latitude ([`annual_insolation_at`], McGehee and Lehman's integral)
/// is the closed forms at the equator and the poles to 10⁻¹² over obliquities from 0° to 177°, and
/// within 10⁻⁶ of a 20,000-step quadrature of its integral at every latitude, the kink at the polar
/// circle included; its area-weighted mean over x is 1.
#[test]
fn the_annual_insolation_at_a_latitude_meets_the_closed_forms() {
    for degrees in [0.034, 3.13, 23.44, 25.19, 45.0, 60.0, 89.0, 98.0, 177.36] {
        let obliquity = Radians::from(Degrees::new(degrees));
        let (equator, pole) = annual_insolation(obliquity);
        assert!(
            (annual_insolation_at(obliquity, 0.0) - equator).abs() < 1e-12,
            "{degrees}°"
        );
        assert!(
            (annual_insolation_at(obliquity, 1.0) - pole).abs() < 1e-12,
            "{degrees}°"
        );
        let (sin_b, cos_b) = math::sin_cos(obliquity.value());
        let dense = |x: f64| {
            let across = (1.0 - x * x).sqrt();
            let n = 20_000_u32;
            let step = core::f64::consts::PI / f64::from(n);
            let f = |j: u32| {
                let a = across * sin_b * math::cos(step * f64::from(j)) - x * cos_b;
                (1.0 - a * a).max(0.0).sqrt()
            };
            let sum = (1..n).fold(f64::midpoint(f(0), f(n)), |sum, j| sum + f(j));
            4.0 * step / (core::f64::consts::PI * core::f64::consts::PI) * sum
        };
        let mut mean = 0.0;
        for k in 0..256 {
            let x = (f64::from(k) + 0.5) / 256.0;
            let q = annual_insolation_at(obliquity, x);
            assert!((q - dense(x)).abs() < 1e-6, "{degrees}° at x = {x}: {q}");
            mean += q / 256.0;
        }
        assert!((mean - 1.0).abs() < 1e-4, "{degrees}°: {mean}");
    }
}

/// T13.c's passes read the record's own range (decision-p14-t51a-partition §6, condition 1): for
/// every body of the table, the passes' climate at the body's own surface temperature and partial
/// pressures is the record's [`SurfaceConditions::climate_range`], bit for bit, about its mean.
#[test]
fn the_passes_climate_is_the_record_s_range() {
    let mut compared = 0;
    for (name, body) in historic_table() {
        let spin = spin_of(name);
        let Some(conditions) = SurfaceConditions::derive(&inputs_of(&body, spin)) else {
            continue;
        };
        let air = body.atmosphere();
        let passes = step_climate(
            &climate_body(&body, spin),
            ocean_weight_of(&body),
            air.surface_temperature(),
            air.partial_pressures(),
        );
        let (cold, warm) = conditions.climate_range();
        let mean = conditions.mean_surface_temperature();
        assert_same_bits(passes.mean().value(), mean.value());
        // The ends are the record's, widened to the mean only where the range does not hold it.
        assert_same_bits(passes.coldest().value(), cold.value().min(mean.value()));
        assert_same_bits(passes.warmest().value(), warm.value().max(mean.value()));
        compared += 1;
    }
    assert!(compared >= 8, "{compared} bodies compared");
}

/// What the passes' climate reads of the table's derived body `body` under the spin `spin`, its
/// greenhouse's inventory its atmosphere's ([`Atmosphere::inventory_weights`]).
fn climate_body(body: &DerivedBody, spin: SeasonalSpin) -> ClimateBody {
    ClimateBody {
        equilibrium_temperature: body.equilibrium_temperature(),
        surface_gravity: body.surface_gravity(),
        radius: Metres::from(body.radius()),
        ground: GroundInertia::of_material(SurfaceMaterial::of(&body.fractions())),
        spin,
        heat_flow: heat_flow_of(body),
        inventory: *body.atmosphere().inventory_weights(),
    }
}

/// The cost of the passes' climate, against a partition step's (P14.T51.a's second part,
/// decision-p14-t51a-part2 §1.3: at most 5 µs a call with air under the Newton steps): run by name,
/// in release, and recorded in plan 14's Risks.
#[test]
#[ignore = "a timing, run by name in release"]
fn time_the_passes_climate() {
    use crate::planetary::chem::partition::{AirborneWeights, ColumnInventory, Ground};
    for (name, body) in historic_table() {
        let spin = spin_of(name);
        if conditions_of(&body, spin).is_none() {
            continue;
        }
        let air = body.atmosphere();
        let climate = climate_body(&body, spin);
        let weight = ocean_weight_of(&body);
        let n = 2_000;
        let mut sink = 0.0;
        let start = std::time::Instant::now();
        for i in 0..n {
            let t = Kelvin::new(air.surface_temperature().value() + 1e-9 * f64::from(i));
            sink += step_climate(&climate, weight, t, air.partial_pressures())
                .coldest()
                .value();
        }
        let each = start.elapsed().as_secs_f64() / f64::from(n) * 1e6;
        let partials = air.partial_pressures();
        let inv = ColumnInventory::new(Gas::ALL.iter().filter_map(|&g| {
            let p = partials.of(g);
            (p.value() > 0.0).then(|| (g.substance(), Pascals::new(p.value() * 1.5)))
        }))
        .unwrap();
        let passes = step_climate(&climate, weight, air.surface_temperature(), partials);
        let mut w = AirborneWeights::new();
        let ground = Ground::without_basins(body.surface_gravity());
        let start = std::time::Instant::now();
        for _ in 0..n {
            w.solve(&passes, &ground, &inv);
            sink += w.weights().first().map_or(0.0, |p| p.value());
        }
        let solve = start.elapsed().as_secs_f64() / f64::from(n) * 1e6;
        eprintln!("{name}: climate {each:.2} µs; partition step {solve:.2} µs ({sink:e})");
    }
}

/// The safeguarded Newton steps ([`zonal_point_newton`], [`ZONAL_NEWTON_STEPS`]) solve every zonal
/// point within 10⁻¹² `T_s` of the 64-step bisection, version 22's solver, which the tests keep as
/// their oracle (`zonal_point`) (decision-p14-t51a-part2 §1.4): at the equator, the pole and every
/// eighth area point, over obliquities of 0–180°, mean temperatures of 40–737 K, 6D ÷ `B_eff` of
/// 10⁻⁴ to 50, Koll's W of 0 or `B_P`, `B_eff` ÷ `B_P` of 1 or 0.3, and Φ of 1 or Φ⊕. The dark pole
/// at 0° and weak transport, where E → 0, is among them. Two steps fewer still hold 10⁻¹³ `T_s`:
/// the count is the least that does, plus two.
#[test]
fn the_newton_zonal_points_are_the_bisection_s() {
    let (mut worst, mut fewer) = (0.0_f64, 0.0_f64);
    let cosines = hour_cosines();
    for degrees in [
        0.0, 10.0, 23.44, 45.0, 54.74, 60.0, 90.0, 98.0, 120.0, 180.0,
    ] {
        let obliquity = Radians::from(Degrees::new(degrees));
        let modes = insolation_modes(obliquity);
        let (q_eq, q_pole) = annual_insolation(obliquity);
        let mut points = vec![
            (q_eq, modes.map(|mode| mode.at_equator)),
            (q_pole, [1.0; ZONAL_MODES]),
        ];
        for k in (0..ZONAL_AREA_POINTS).step_by(8) {
            let x = (f64::from(k) + 0.5) / f64::from(ZONAL_AREA_POINTS);
            points.push((insolation_with(obliquity, x, &cosines), legendre_at(x)));
        }
        for t_s in [40.0, 94.0, 214.0, 288.0, 737.0] {
            for six_d in [1e-4, 1e-2, 1.0, 50.0] {
                for weak in [0.0, 1.0] {
                    for effective in [1.0, 0.3] {
                        for albedo in [1.0, WET_ALBEDO_GRADIENT] {
                            let transport = ZonalTransport {
                                planck: 1.0,
                                effective,
                                conductance: six_d * effective / 6.0,
                                weak,
                                albedo,
                            };
                            for (q, legendre) in &points {
                                let bisection = zonal_point(t_s, &transport, &modes, *q, legendre);
                                let newton =
                                    zonal_point_newton(t_s, &transport, &modes, *q, legendre);
                                let (at, (low, high)) = newton_point(
                                    t_s,
                                    &transport,
                                    &modes,
                                    (*q, legendre),
                                    ZONAL_NEWTON_STEPS,
                                );
                                assert_same_bits(at, newton);
                                assert!(low <= newton && newton <= high, "{low} {newton} {high}");
                                let miss = (newton - bisection).abs() / t_s;
                                worst = worst.max(miss);
                                assert!(
                                    miss <= 1e-12,
                                    "{degrees}° {t_s} K 6D/B {six_d} W {weak} B_eff {effective} \
                                     Φ {albedo} q {q}: {newton} against {bisection}"
                                );
                                let (short, _) = newton_point(
                                    t_s,
                                    &transport,
                                    &modes,
                                    (*q, legendre),
                                    ZONAL_NEWTON_STEPS - 2,
                                );
                                fewer = fewer.max((short - bisection).abs() / t_s);
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(fewer <= 1e-13, "two steps fewer miss by {fewer:e} T_s");
    eprintln!(
        "the Newton zonal points miss the bisection by at most {worst:e} T_s, and by {fewer:e} \
         two steps fewer"
    );
}

/// The Newton zonal points against the bisection over a wider sweep (decision-p14-t51a-part2 §1.4):
/// every area point, obliquities every 5° from 0 to 180°, 6D ÷ `B_eff` of 10⁻⁴ to 50 by decades,
/// W of 0, ½ or 1 `B_P`, `B_eff` ÷ `B_P` of 1, 0.6, 0.3 or 0.1 and Φ of 1, 1.25 or Φ⊕, at 40 and
/// 288 K, within 10⁻¹² `T_s`, and within 10⁻¹³ two steps fewer: the count is this sweep's least,
/// plus two. Slow, run by name in release; its worst is [`ZONAL_NEWTON_STEPS`]'s doc's.
#[test]
#[ignore = "slow: a wider sweep, run by name in release"]
fn the_newton_zonal_points_are_the_bisection_s_over_a_wider_sweep() {
    let (mut worst, mut fewer) = (0.0_f64, 0.0_f64);
    let cosines = hour_cosines();
    for step in 0..=36 {
        let obliquity = Radians::from(Degrees::new(5.0 * f64::from(step)));
        let modes = insolation_modes(obliquity);
        let (q_eq, q_pole) = annual_insolation(obliquity);
        let mut points = vec![
            (q_eq, modes.map(|mode| mode.at_equator)),
            (q_pole, [1.0; ZONAL_MODES]),
        ];
        for k in 0..ZONAL_AREA_POINTS {
            let x = (f64::from(k) + 0.5) / f64::from(ZONAL_AREA_POINTS);
            points.push((insolation_with(obliquity, x, &cosines), legendre_at(x)));
        }
        for t_s in [40.0, 288.0] {
            for six_d in [1e-4, 1e-3, 1e-2, 0.1, 1.0, 10.0, 50.0] {
                for weak in [0.0, 0.5, 1.0] {
                    for effective in [1.0, 0.6, 0.3, 0.1] {
                        for albedo in [1.0, 1.25, WET_ALBEDO_GRADIENT] {
                            let transport = ZonalTransport {
                                planck: 1.0,
                                effective,
                                conductance: six_d * effective / 6.0,
                                weak,
                                albedo,
                            };
                            for (q, legendre) in &points {
                                let bisection = zonal_point(t_s, &transport, &modes, *q, legendre);
                                let newton =
                                    zonal_point_newton(t_s, &transport, &modes, *q, legendre);
                                let miss = (newton - bisection).abs() / t_s;
                                worst = worst.max(miss);
                                assert!(miss <= 1e-12, "{step} {t_s} {six_d} {weak} {effective}");
                                let (short, _) = newton_point(
                                    t_s,
                                    &transport,
                                    &modes,
                                    (*q, legendre),
                                    ZONAL_NEWTON_STEPS - 2,
                                );
                                fewer = fewer.max((short - bisection).abs() / t_s);
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(fewer <= 1e-13, "two steps fewer miss by {fewer:e} T_s");
    eprintln!(
        "over the wider sweep the Newton zonal points miss by at most {worst:e} T_s, and by \
         {fewer:e} two steps fewer"
    );
}

/// The zonal ends of a transport `transport` at the obliquity `obliquity` about the mean
/// `surface_k`, K, as one solver gives them ([`zonal_ends`]'s signature).
type ZonalEnds = fn(f64, &ZonalTransport, Radians) -> (f64, f64);

/// [`zonal_ends`] with each end solved by the 64-step bisection (`zonal_point`), version 22's
/// solver, which the tests keep as the Newton steps' oracle.
fn zonal_ends_bisected(
    surface_k: f64,
    transport: &ZonalTransport,
    obliquity: Radians,
) -> (f64, f64) {
    if !restored(transport) {
        return (surface_k, surface_k);
    }
    let modes = insolation_modes(obliquity);
    let (equator, pole) = annual_insolation(obliquity);
    let at_equator = modes.map(|mode| mode.at_equator);
    (
        zonal_point(surface_k, transport, &modes, equator, &at_equator),
        zonal_point(surface_k, transport, &modes, pole, &[1.0; ZONAL_MODES]),
    )
}

/// The points of the profile `profile` ([`ZonalProfile::points`]), each solved by the 64-step
/// bisection (`zonal_point`), the Newton steps' oracle.
fn bisected_points(profile: &ZonalProfile) -> Vec<(f64, f64)> {
    let Some((transport, obliquity)) = profile.modal else {
        return profile.points();
    };
    let (modes, cosines) = (insolation_modes(obliquity), hour_cosines());
    let n = f64::from(ZONAL_AREA_POINTS);
    let surface_k = profile.surface_k;
    let inner = (0..ZONAL_AREA_POINTS).map(|k| {
        let x = (f64::from(k) + 0.5) / n;
        let insolation = insolation_with(obliquity, x, &cosines);
        (
            x,
            zonal_point(surface_k, &transport, &modes, insolation, &legendre_at(x)),
        )
    });
    let (equator, pole) = zonal_ends_bisected(surface_k, &transport, obliquity);
    core::iter::once((0.0, equator))
        .chain(inner)
        .chain(core::iter::once((1.0, pole)))
        .collect()
}

/// The table's bodies on the Newton steps: their zonal ends and contrasts within 10⁻¹² `T_s` of
/// the 64-step bisection's, version 22's solver, and their cold areas below 263.15 K within 10⁻⁹
/// (decision-p14-t51a-part2 §1.4).
#[test]
fn the_batch_s_zonal_solver_moves_the_table_only_at_its_last_bits() {
    let mut compared = 0;
    for (name, body) in historic_table() {
        let inputs = inputs_of(&body, spin_of(name));
        let Some(conditions) = SurfaceConditions::derive(&inputs) else {
            continue;
        };
        let profile = conditions.zonal;
        let t_s = conditions.mean_surface_temperature().value();
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-12 * t_s;
        let bisected = match profile.modal {
            Some((transport, obliquity)) => {
                zonal_ends_bisected(profile.surface_k, &transport, obliquity)
            }
            None => profile.ends,
        };
        let ends = (profile.ends, bisected);
        assert!(
            near(ends.0.0, ends.1.0)
                && near(ends.0.1, ends.1.1)
                && near(
                    conditions.equator_pole_contrast_k(),
                    bisected.0 - bisected.1
                ),
            "{name}: {ends:?}"
        );
        let threshold = PERENNIAL_ICE_TEMPERATURE;
        let (a, b) = (
            area_below(&profile.points(), threshold),
            area_below(&bisected_points(&profile), threshold),
        );
        assert!((a - b).abs() <= 1e-9, "{name}: {a} against {b}");
        compared += 1;
    }
    assert!(compared >= 8, "{compared}");
}

// P14.T54.a, lever (h): a body's zonal insolation table (decision-p14-t54a-passes §C.3).

/// The obliquities, degrees, at which the table's tests read it: none, the Solar System's
/// (Mercury's, Ganymede's, Earth's, Venus's), and either side of the trapezoid's kinks, the polar
/// circle's and the ends' near 90°, to 180°.
const TABLE_OBLIQUITIES_DEG: [f64; 11] = [
    0.0, 0.034, 3.13, 23.44, 45.0, 60.0, 89.9, 90.0, 120.0, 177.36, 180.0,
];

/// Asserts two profiles' points equal bit for bit, `(x, T)` by `(x, T)`.
fn same_points(read: &[(f64, f64)], own: &[(f64, f64)], what: &str) {
    assert_eq!(read.len(), own.len(), "{what}");
    for (&(x, t), &(x_own, t_own)) in read.iter().zip(own) {
        assert_same_bits(x, x_own);
        assert_same_bits(t, t_own);
    }
}

/// The points of the profile `profile` as each profile computed them before its body's table:
/// its own modes, cosines, insolations and Legendre values (lever (h)'s oracle).
fn unlevered_points(profile: &ZonalProfile) -> Vec<(f64, f64)> {
    let inner = match profile.modal {
        Some((transport, obliquity)) => profile.inner_points_unlevered(&transport, obliquity),
        None => profile.flat_points(),
    };
    profile.with_ends(inner)
}

/// Lever (h): the table holds, at each area point, the point's own insolation
/// (`insolation_with` over `hour_cosines`, and `annual_insolation_at`) and Legendre values
/// (`legendre_at`), with `insolation_modes`'s modes, bit for bit; and a call chain's cosines are
/// `hour_cosines`'.
#[test]
fn the_zonal_insolation_table_is_each_point_s_own_insolation() {
    let cosines = HourCosines::new();
    let own = hour_cosines();
    assert_eq!(cosines.0.len(), own.len());
    for (&shared, &fresh) in cosines.0.iter().zip(&own) {
        assert_same_bits(shared, fresh);
    }
    let n = f64::from(ZONAL_AREA_POINTS);
    for degrees in TABLE_OBLIQUITIES_DEG {
        let obliquity = Radians::from(Degrees::new(degrees));
        let table = ZonalInsolation::of(obliquity, &cosines);
        assert!(table.is_at(obliquity), "{degrees}°");
        for (mode, own) in table.modes.iter().zip(&insolation_modes(obliquity)) {
            assert_same_bits(mode.degree, own.degree);
            assert_same_bits(mode.coefficient, own.coefficient);
            assert_same_bits(mode.at_equator, own.at_equator);
        }
        assert_eq!(table.points.len(), 64, "{degrees}°");
        for (k, (x, insolation, legendre)) in (0..ZONAL_AREA_POINTS).zip(&table.points) {
            let x_k = (f64::from(k) + 0.5) / n;
            assert_same_bits(*x, x_k);
            assert_same_bits(*insolation, insolation_with(obliquity, x_k, &own));
            assert_same_bits(*insolation, annual_insolation_at(obliquity, x_k));
            for (&value, &own) in legendre.iter().zip(&legendre_at(x_k)) {
                assert_same_bits(value, own);
            }
        }
    }
}

/// Lever (h): a profile read off its body's table is the profile's own, bit for bit: the table's
/// bodies under their own spins and at the table's obliquities, with modes or flat, rotating or
/// locked 1:1, in their points, cold profiles and cold areas. A flat profile reads no table.
#[test]
fn a_profile_off_its_body_s_table_is_its_own_to_the_bit() {
    let thresholds = [
        Kelvin::new(110.0),
        PERENNIAL_ICE_TEMPERATURE,
        Kelvin::new(273.15),
    ];
    let (mut modal, mut locked) = (0, 0);
    for (name, body) in historic_table() {
        let own = spin_of(name);
        let obliquities = core::iter::once(own.obliquity)
            .chain(TABLE_OBLIQUITIES_DEG.map(|d| Radians::from(Degrees::new(d))));
        for obliquity in obliquities {
            for solar_day in [own.solar_day, None] {
                let spin = SeasonalSpin {
                    obliquity,
                    solar_day,
                    ..own
                };
                let Some(conditions) = conditions_of(&body, spin) else {
                    continue;
                };
                let what = format!("{name} at {obliquity:?}, solar day {solar_day:?}");
                let table = ZonalInsolation::of(obliquity, &HourCosines::new());
                let profile = conditions.zonal;
                let unlevered = unlevered_points(&profile);
                same_points(&profile.points_over(&table), &unlevered, &what);
                same_points(&profile.points(), &unlevered, &what);
                assert_eq!(
                    format!("{:?}", conditions.cold_profile_over(&table)),
                    format!("{:?}", conditions.cold_profile()),
                    "{what}"
                );
                for threshold in thresholds {
                    let read = conditions.cold_area_over(threshold, &table);
                    assert_same_bits(read, conditions.cold_area(threshold));
                    if !conditions.locked {
                        assert_same_bits(read, area_below(&unlevered, threshold));
                    }
                }
                assert_eq!(conditions.zonal_insolation(), table, "{what}");
                modal += u32::from(profile.modal.is_some());
                locked += u32::from(conditions.locked);
            }
        }
    }
    eprintln!("{modal} profiles with modes, {locked} locked conditions");
    assert!(modal >= 100 && locked >= 50, "{modal}, {locked}");
    // A flat profile, at its mean everywhere, whatever table it is given.
    let flat = ZonalProfile {
        surface_k: 95.0,
        modal: None,
        ends: (95.0, 95.0),
    };
    let elsewhere = ZonalInsolation::of(Radians::from(Degrees::new(60.0)), &HourCosines::new());
    same_points(
        &flat.points_over(&elsewhere),
        &unlevered_points(&flat),
        "flat",
    );
}

/// Lever (h): a profile with modes refuses a table of another obliquity, a broken invariant.
#[test]
#[should_panic(expected = "a zonal profile reads the insolation table of its own obliquity")]
fn a_profile_refuses_another_obliquity_s_table() {
    let (column, spin) = earth_column();
    let profile = ZonalProfile::of(&column, &spin, OceanWeight::FULL);
    let tilted = Radians::new(spin.obliquity.value().next_up());
    let _ = profile.points_over(&ZonalInsolation::of(tilted, &HourCosines::new()));
}

/// The record path on the table (P14.T51.a's second part; decision-p14-t51a-part2 §3.7 and
/// §4.4): the wet worlds hold surface water, Earth's published water ice is T24.b's `ice_fraction`
/// to the bit at the same surface water, within 0.01–0.07 (decision-p14-t54a-bands §4.3), Earth
/// holds liquid water and the regime's reservoirs are the partition's.
#[test]
fn the_batch_publishes_the_table_s_condensates() {
    use crate::planetary::chem::partition::CondensatePhase;
    use crate::planetary::hooks::cold_traps::{ColdTrapRules, RecordColdTraps, lunar_sky};
    use crate::planetary::hooks::figures::{GlobalFigures, VolatileHistory, WaterIce, cover};

    let water = Gas::Water.substance();
    let sky = lunar_sky();
    let mut wet = 0;
    let mut earth_ice = None;
    for (name, body) in historic_table() {
        let Some(batch) = table_figures(&body) else {
            continue;
        };
        if batch.surface_water().value() > 0.0 {
            wet += 1;
        }
        let inputs = SurfaceInputs {
            heat_flow: batch.heat_flow(),
            ocean_weight: batch.ocean_weight(),
            ..inputs_of(&body, spin_of(name))
        };
        let Some(conditions) = SurfaceConditions::derive(&inputs) else {
            continue;
        };
        let gravity = body.surface_gravity();
        let atmosphere = body.atmosphere();
        // P14.T51.a's published condensates, with the cold traps held (P14.T51.f).
        let table = conditions.zonal_insolation();
        let under = || {
            let traps = RecordColdTraps::at_the_albedo_of(ColdTrapRules::Held, &sky, atmosphere);
            batch.condensates_under(atmosphere, &conditions, gravity, &traps, &table)
        };
        let (partition, published) = under();
        let (history, off_table) = (VolatileHistory::NONE, WaterIce::OffTable);
        let second = |figures: GlobalFigures| {
            figures.with_surface_under(&conditions, history, Some(&published), &table, off_table)
        };
        if let Some(ice) = published.entry(water, CondensatePhase::Solid) {
            let t24b = cover::ice_fraction(&cover::IceInputs {
                conditions: &conditions,
                state: atmosphere.state(),
                crust: batch.impactors().crust(),
                surface_water: batch.surface_water(),
            });
            assert_same_bits(ice.area_fraction(), t24b);
        }
        // The same twice (the sim-determinism skill).
        let again = under();
        assert_eq!(
            format!("{:?}", (&partition, &published)),
            format!("{again:?}")
        );
        // T24.b's gate on the state holds: an airless body has no surface water.
        if body.atmosphere().state() == SurfaceState::Airless {
            assert_same_bits(batch.surface_water().value(), 0.0);
        }
        assert_an_ice_bulk_lists_its_crust(name, &body, &partition, &published);
        // Water's ice where the condensates hold none is T24.b's, appended to the partition's ices
        // and divided with them by their total where it exceeds the surface (W2).
        if published.entry(water, CondensatePhase::Solid).is_none() {
            let figures = second(batch.clone());
            let t24b = cover::ice_fraction(&cover::IceInputs {
                conditions: &conditions,
                state: atmosphere.state(),
                crust: batch.impactors().crust(),
                surface_water: batch.surface_water(),
            });
            let &(_, listed) = figures
                .surface_ices()
                .iter()
                .find(|&&(id, _)| id == water)
                .unwrap_or_else(|| panic!("{name} lists its water ice"));
            let total = published
                .surface_ices()
                .iter()
                .fold(0.0, |sum, &(_, f)| sum + f)
                + t24b;
            let expected = if total > 1.0 { t24b / total } else { t24b };
            eprintln!(
                "{name}: water ice {listed}, T24.b's {t24b} over a total of {total}, beside {:?}",
                published.surface_ices()
            );
            assert_same_bits(listed, expected);
        }
        if name == "Earth" {
            let ocean = published.entry(water, CondensatePhase::Liquid);
            assert!(ocean.is_some(), "{published:?}");
            earth_ice = published
                .entry(water, CondensatePhase::Solid)
                .map(crate::planetary::chem::partition::CondensateEntry::area_fraction);
            assert!(
                conditions
                    .clone()
                    .with_condensates(&published)
                    .liquid_water()
            );
            assert_eq!(partition.reservoirs(), [water]);
            let figures = second(batch);
            assert_same_bits(figures.ocean_fraction(), batch_ocean(&published));
        }
    }
    assert!(wet >= 2, "{wet} wet worlds");
    // Band 5 (decision-p14-t54a-bands §4.3): the generated Earth's perennial ice within 0.01–0.07,
    // its figure printed, on the batch's table Earth at the passes' stand-ins. Earth's perennial
    // ice is 0.045–0.051 of the globe and its annual mean 0.068–0.078 (Fretwell et al. 2013, The
    // Cryosphere 7, 375, Table 7; IPCC AR5 WGI Ch. 4, Tables 4.1–4.2; the NSIDC Sea Ice Index, v4,
    // 1981–2010).
    let ice = earth_ice.unwrap_or(0.0);
    eprintln!("the generated Earth's ice: {ice}");
    assert!((0.01..=0.07).contains(&ice), "Earth's ice {ice}");
}

/// Decision-p14-t54a-ganymede, C1 follow-up §C1.4: the table's body `name`, derived as `body`,
/// whose partition is `partition`, lists its crust's frozen water itself among its published
/// condensates `published` (at T24.b's area, which its caller holds to the bit) where its bulk is
/// ice and its partition holds frozen water, as the airless table Ganymede and the table Titan do,
/// whose ice holds the water Jeans's test fails.
fn assert_an_ice_bulk_lists_its_crust(
    name: &str,
    body: &DerivedBody,
    partition: &crate::planetary::chem::partition::Partition,
    published: &crate::planetary::chem::partition::Condensates,
) {
    use crate::planetary::chem::partition::CondensatePhase;
    use crate::planetary::derive::atmosphere::SurfaceMaterial;
    let water = Gas::Water.substance();
    let ice_bulk = SurfaceMaterial::of(&body.fractions()) == SurfaceMaterial::Ice;
    let frozen = partition
        .condensate(water)
        .is_some_and(|c| c.phase() == CondensatePhase::Solid);
    if ice_bulk && frozen {
        assert!(
            published.entry(water, CondensatePhase::Solid).is_some(),
            "{name}: {published:?}"
        );
    }
    if matches!(name, "Ganymede" | "Titan") {
        assert!(ice_bulk && frozen, "{name}: {partition:?}");
    }
}

/// The ocean fraction the published condensates `published` give: their liquids' areas summed.
fn batch_ocean(published: &crate::planetary::chem::partition::Condensates) -> f64 {
    published
        .surface_liquids()
        .iter()
        .fold(0.0, |sum, &(_, f)| sum + f)
}

/// The conditions' gases are one computation with `mole_fractions` (the sim-determinism skill's
/// precomputed path): `GasFractions`, which the passes' columns and the record's share, gives the
/// generic form's fractions, mean molar mass and `c_p` ÷ R at the mean surface temperature bit for
/// bit on every atmosphere of the table, and on a single gas.
#[test]
fn the_conditions_gases_are_the_mole_fractions() {
    for (_, body) in historic_table() {
        let partials = body.atmosphere().partial_pressures();
        let generic =
            mole_fractions(Gas::ALL.map(|gas| (gas.substance(), partials.of(gas)))).unwrap();
        let ours = GasFractions::of(partials, false).unwrap();
        assert_eq!(format!("{:?}", ours.as_slice()), format!("{generic:?}"));
        let t = body.atmosphere().surface_temperature();
        for (a, b) in [
            (
                ours.mean_molar_mass_g_per_mol,
                mean_molar_mass_g_per_mol(&generic),
            ),
            (
                ours.heat_capacity_over_r_at(t),
                mixture_heat_capacity_over_r_at(&generic, t),
            ),
        ] {
            match (a, b) {
                (Some(a), Some(b)) => assert_same_bits(a, b),
                (a, b) => assert_eq!(a.is_none(), b.is_none()),
            }
        }
        assert_eq!(GasFractions::of(partials, true).unwrap().as_slice(), []);
    }
}

/// Lever (a) on the table (decision-p14-t54a-passes, "Cost escalation (after Batch B)"): the
/// record's second stage, given T24.b's water ice as its partition's cover read it off the profile
/// it built (`WaterIce::Covered`, from `condensates_and_ice_under`), publishes the same figures, bit
/// for bit, as when it reads the ice off the table again (`WaterIce::OffTable`), rotating and locked
/// 1:1, after the conditions take the liquid-water flag the record sets between the two stages.
/// Each ice the second stage reads is checked to the bit (`IceReads::Checked`).
#[test]
fn the_record_s_second_stage_reads_its_partition_s_ice_to_the_bit() {
    use crate::planetary::chem::partition::CondensatePhase;
    use crate::planetary::hooks::cold_traps::{ColdTrapRules, RecordColdTraps, lunar_sky};
    use crate::planetary::hooks::figures::{IceReads, VolatileHistory, WaterIce, with_ice_reads};

    let sky = lunar_sky();
    let (mut surfaces, mut read) = (0_u64, 0_u64);
    let ((), checked) = with_ice_reads(IceReads::Checked, || {
        for (name, body) in historic_table() {
            let Some(batch) = table_figures(&body) else {
                continue;
            };
            let rotating = spin_of(name);
            let locked = SeasonalSpin {
                solar_day: None,
                ..rotating
            };
            for spin in [rotating, locked] {
                let inputs = SurfaceInputs {
                    heat_flow: batch.heat_flow(),
                    ocean_weight: batch.ocean_weight(),
                    ..inputs_of(&body, spin)
                };
                let Some(conditions) = SurfaceConditions::derive(&inputs) else {
                    continue;
                };
                let atmosphere = body.atmosphere();
                let table = conditions.zonal_insolation();
                let traps =
                    RecordColdTraps::at_the_albedo_of(ColdTrapRules::Held, &sky, atmosphere);
                let (_, published, ice) = batch.condensates_and_ice_under(
                    atmosphere,
                    &conditions,
                    body.surface_gravity(),
                    &traps,
                    &table,
                );
                assert!(matches!(ice, WaterIce::Covered(_)), "{name}: {ice:?}");
                let conditions = conditions.with_condensates(&published);
                let second = |water_ice| {
                    batch.clone().with_surface_under(
                        &conditions,
                        VolatileHistory::NONE,
                        Some(&published),
                        &table,
                        water_ice,
                    )
                };
                assert_eq!(
                    format!("{:?}", second(ice)),
                    format!("{:?}", second(WaterIce::OffTable)),
                    "{name}, {spin:?}"
                );
                surfaces += 1;
                if published
                    .entry(Gas::Water.substance(), CondensatePhase::Solid)
                    .is_none()
                {
                    read += 1;
                }
            }
        }
    });
    eprintln!("{surfaces} surfaces, {read} of them reading T24.b's ice, {checked} checked");
    assert_eq!(read, checked);
    assert!(surfaces >= 10 && read >= 4, "{surfaces} {read}");
}

/// The ground's inertia is P14.T49.j's law at its lid, with its fallbacks (P14.T54.a;
/// decision-pre-23-review item 7; decision-p14-t54a-passes §3.5): Mars's `basalt_fe` at 214 K
/// takes the law, about 48.1, the reference form's to the bit; `graphite`, with no thermal columns,
/// rock's 55; `cometary`, of the `Organic` family, which has no form, ice's 20; `H2O` above its
/// 273.16 K triple point, with no solid, its family's 20, and `basalt` above its solidus rock's 55,
/// whatever the bulk material; and no lid the bulk material's constant, as P14.T24.a reads it, to
/// the bit. Mars's and Titan's figures at the table's equilibrium temperatures are printed for the
/// bless note, against their measured 180–250 and about 236, which pore gas gives
/// (decision-pre-23-review item 7; P14.T49.l).
#[test]
fn the_ground_s_inertia_is_the_law_at_its_lid_with_its_fallbacks() {
    let at = |lid: CrustLithology, material: SurfaceMaterial, t: f64| {
        ground_inertia(Some(lid), material, Kelvin::new(t))
    };
    let law = |lid: CrustLithology, t: f64| {
        reference_regolith_thermal_inertia(lid.substance(), Kelvin::new(t))
            .unwrap_or_else(|e| panic!("{lid:?} at {t} K: {e}"))
            .inertia_j_per_m2_k_sqrt_s()
    };
    let assert_ground = |ground: GroundInertia, source: InertiaSource, gamma: f64| {
        assert_eq!(ground.source(), source, "{ground:?}");
        assert_same_bits(ground.inertia_j_per_m2_k_sqrt_s(), gamma);
    };
    let mars = at(CrustLithology::IronRichBasalt, SurfaceMaterial::Rock, 214.0);
    assert_ground(
        mars,
        InertiaSource::Law,
        law(CrustLithology::IronRichBasalt, 214.0),
    );
    assert!(
        (mars.inertia_j_per_m2_k_sqrt_s() - 48.1).abs() < 0.05,
        "{mars:?}"
    );
    let (rock, ice) = (
        ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S,
        ICE_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S,
    );
    for material in [SurfaceMaterial::Rock, SurfaceMaterial::Ice] {
        assert_ground(
            at(CrustLithology::Graphite, material, 214.0),
            InertiaSource::NoThermalColumns,
            rock,
        );
        assert_ground(
            at(CrustLithology::Cometary, material, 40.0),
            InertiaSource::NoRegolithForm,
            ice,
        );
        assert_ground(
            at(CrustLithology::Water, material, 280.0),
            InertiaSource::NoSolid,
            ice,
        );
        assert_ground(
            at(CrustLithology::Basalt, material, 2_000.0),
            InertiaSource::NoSolid,
            rock,
        );
    }
    for (material, gamma) in [(SurfaceMaterial::Rock, rock), (SurfaceMaterial::Ice, ice)] {
        let none = ground_inertia(None, material, Kelvin::new(214.0));
        assert_ground(none, InertiaSource::NoLid, gamma);
        assert_eq!(none, GroundInertia::of_material(material));
    }
    // Below its triple point, water ice takes the law.
    let europa = at(CrustLithology::Water, SurfaceMaterial::Ice, 102.0);
    assert_ground(
        europa,
        InertiaSource::Law,
        law(CrustLithology::Water, 102.0),
    );
    let table = table();
    let t_eq = |name: &str| found(&table, name).1.equilibrium_temperature().value();
    let (mars_t, titan_t) = (t_eq("Mars"), t_eq("Titan"));
    eprintln!(
        "Γ: basalt_fe at 214 K {:.2}, at Mars's table T_eq {mars_t:.2} K {:.2}; H2O at 102 K {:.2}, \
         at Titan's table T_eq {titan_t:.2} K {:.2}, at its 94 K surface {:.2}",
        mars.inertia_j_per_m2_k_sqrt_s(),
        law(CrustLithology::IronRichBasalt, mars_t),
        europa.inertia_j_per_m2_k_sqrt_s(),
        law(CrustLithology::Water, titan_t),
        law(CrustLithology::Water, 94.0),
    );
}

/// Every lid T51.c can name takes the law or a named fallback at any positive temperature, never
/// the unreachable one: each row has condensed columns, which `ground_inertia`'s type rests on
/// (P14.T54.a; the C1 review's RC1-b and RC1-c).
#[test]
fn every_lid_takes_the_law_or_a_named_fallback() {
    for lid in CrustLithology::ALL {
        assert!(
            lid.substance().substance().condensed().is_some(),
            "{lid:?} has condensed columns"
        );
        for t in [20.0, 40.0, 100.0, 214.0, 300.0, 1_000.0, 3_000.0] {
            for material in [SurfaceMaterial::Rock, SurfaceMaterial::Ice] {
                let ground = ground_inertia(Some(lid), material, Kelvin::new(t));
                assert_ne!(
                    ground.source(),
                    InertiaSource::Unreachable,
                    "{lid:?} at {t} K"
                );
                let gamma = ground.inertia_j_per_m2_k_sqrt_s();
                assert!(
                    gamma.is_finite() && gamma > 0.0,
                    "{lid:?} at {t} K: {gamma}"
                );
            }
        }
    }
}

/// A column under no light, its equilibrium temperature not positive, stores nothing by day: its
/// ground takes its material's constant, labelled, and never reaches the law's debug assertion.
#[test]
fn an_unlit_ground_takes_its_material_s_constant() {
    for t in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let ground = GroundInertia::at_equilibrium(
            Some(CrustLithology::Basalt),
            SurfaceMaterial::Rock,
            Kelvin::new(t),
        );
        assert_eq!(ground.source(), InertiaSource::Unlit, "{t}");
        assert_same_bits(
            ground.inertia_j_per_m2_k_sqrt_s(),
            ROCK_REGOLITH_THERMAL_INERTIA_J_PER_M2_K_SQRT_S,
        );
    }
    let lit = GroundInertia::at_equilibrium(
        Some(CrustLithology::Basalt),
        SurfaceMaterial::Rock,
        Kelvin::new(214.0),
    );
    assert_eq!(
        lit,
        ground_inertia(
            Some(CrustLithology::Basalt),
            SurfaceMaterial::Rock,
            Kelvin::new(214.0)
        )
    );
}

/// What a lid's row at a positive, finite temperature never gives, a temperature that is not one,
/// fails the fallback's debug assertion: a bug, which a release build reads as the bulk material's
/// constant.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "a lid's row at a positive, finite temperature never gives it")]
fn an_unreachable_inertia_fails_its_debug_assertion() {
    let ground = ground_inertia(
        Some(CrustLithology::Basalt),
        SurfaceMaterial::Rock,
        Kelvin::new(f64::NAN),
    );
    assert_eq!(ground.source(), InertiaSource::Unreachable);
}
