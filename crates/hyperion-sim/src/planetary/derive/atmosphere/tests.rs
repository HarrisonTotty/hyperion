use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::composition::FORMATION_ENVELOPE_FLOOR;
use crate::planetary::derive::solar::{
    SMALL_BODIES, SOLAR_AGE, beyond_the_snow_line, derive as golden_derive, historic_sun, orbit,
    placed as golden_placed, placed_about_the_sun, solar_disc, table_bodies,
};
use crate::planetary::derive::{
    BodyHosts, DerivedBody, HostLight, PlacedBody, composition, derive_body, formation_composition,
    radius_chen_kipping,
};
use crate::planetary::disc::DiscProfile;
use crate::stellar::Composition;
use crate::time::UniverseTime;
use crate::units::consts::{EARTH_MASS_KG, SOLAR_MASS_KG};
use crate::units::{EarthFluxes, EarthRadii};

/// A body of `kg` at `a_au` and `e` about the present Sun with its history, at the rank that
/// keeps its radius `km`, derived at `age`.
fn about_the_sun(disc: &DiscProfile, body: (f64, f64, f64, f64), age: Years) -> DerivedBody {
    derive(&placed_about_the_sun(disc, body), disc, age)
}

fn derive(placed: &PlacedBody, disc: &DiscProfile, age: Years) -> DerivedBody {
    let lights = [historic_sun()];
    let hosts = BodyHosts::new(
        Kilograms::new(SOLAR_MASS_KG),
        Composition::SOLAR,
        &lights,
        &[],
    )
    .unwrap();
    derive_body(placed, &hosts, disc, age, UniverseTime::EPOCH).unwrap()
}

/// Every body of the table: the eight planets and the four small bodies.
fn table() -> Vec<(&'static str, DerivedBody)> {
    let disc = solar_disc();
    table_bodies()
        .map(|&(name, kg, km, a, e)| (name, about_the_sun(&disc, (kg, km, a, e), SOLAR_AGE)))
        .collect()
}

fn found<'a>(table: &'a [(&str, DerivedBody)], name: &str) -> &'a DerivedBody {
    &table.iter().find(|(n, _)| *n == name).unwrap().1
}

/// The initial envelope fractions of the enveloped outcomes of a body of `mass` M⊕ formed at 0.1
/// au of a Sun, over 1,999 evenly spaced radius ranks, sorted.
fn envelopes(mass: f64) -> Vec<f64> {
    let disc = solar_disc();
    let o = orbit(0.1, 0.0);
    let mut fractions: Vec<f64> = (1..2_000)
        .filter_map(|i| {
            let rank = UnitUniform::new(f64::from(i) / 2_000.0).unwrap();
            let placed =
                PlacedBody::new(EarthMasses::new(mass), o, o.semi_major_axis(), rank).unwrap();
            let solved = formation_composition(&placed, &disc).unwrap().unwrap();
            (solved.envelope_fraction() > 0.0).then_some(solved.envelope_fraction())
        })
        .collect();
    fractions.sort_by(f64::total_cmp);
    fractions
}

#[test]
fn the_median_envelope_of_five_earth_mass_cores_is_a_few_per_cent() {
    // P14.T13.a: the envelope is T11.c's, from the radius rank. Across the ranks of a 5 M⊕ body
    // formed at 0.1 au of a Sun, the enveloped outcomes' median fraction lies in 1-6%. Over cores
    // above 1.5 M⊕ the distribution is checked against the plan's log-normal about
    // 3% × (M ÷ 5 M⊕)^0.6 with 0.5 dex, which ruling 119.1 makes the formation law: each median
    // and scatter is printed beside the plan's, and asserted below the handover at 10 M⊕.
    for mass in [2.0, 5.0, 10.0] {
        let fractions = envelopes(mass);
        assert!(
            fractions.len() > 200,
            "{} enveloped ranks at {mass}",
            fractions.len()
        );
        let q = |p: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "a quantile's index into a short sorted list"
            )]
            let i = (p * fractions.len() as f64) as usize;
            fractions[i.min(fractions.len() - 1)]
        };
        let median = q(0.5);
        let plan = 0.03 * math::powf(mass / 5.0, 0.6);
        let sigma_dex = (math::log10(q(0.84)) - math::log10(q(0.16))) / 2.0;
        eprintln!(
            "{mass} M_earth: median {median:.4} (plan {plan:.4}), scatter {sigma_dex:.2} dex (plan 0.5)"
        );
        assert!(sigma_dex > 0.0, "scatter {sigma_dex} dex");
        // Ruling 119.1: below the handover to Chen and Kipping the formation law holds, read at
        // formation.
        if mass < 10.0 {
            assert!(
                (median / plan - 1.0).abs() < 0.1,
                "{mass}: median {median} against {plan}"
            );
            assert!(
                (0.3..0.7).contains(&sigma_dex),
                "{mass}: scatter {sigma_dex} dex"
            );
            assert!(fractions[0] >= FORMATION_ENVELOPE_FLOOR - 1e-12);
        }
        if (mass - 5.0).abs() < 1e-9 {
            assert!((0.01..=0.06).contains(&median), "median envelope {median}");
        }
    }
}

#[test]
fn the_handover_to_chen_and_kipping_keeps_the_median_envelope_rising() {
    // Ruling 122.6: the median and 90th-percentile envelope at 5, 10, 14, 20 and 30 M⊕ are
    // reported, and the median rises through the 10-20 M⊕ handover.
    let mut previous = 0.0;
    for mass in [5.0, 10.0, 14.0, 20.0, 30.0] {
        let fractions = envelopes(mass);
        let at = |p: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "a quantile's index into a short sorted list"
            )]
            let i = (p * fractions.len() as f64) as usize;
            fractions[i.min(fractions.len() - 1)]
        };
        let (median, high) = (at(0.5), at(0.9));
        eprintln!("{mass} M_earth: median envelope {median:.4}, 90th percentile {high:.4}");
        assert!(
            median > previous,
            "{mass}: median {median} after {previous}"
        );
        previous = median;
    }
}

#[test]
fn the_inventory_scales_with_the_side_of_the_snow_line() {
    let solved = composition(
        EarthMasses::new(1.0),
        EarthRadii::new(1.0),
        SnowLineSide::Inside,
        EarthFluxes::new(1.0),
    )
    .unwrap();
    let fractions = solved.fractions();
    let at = |side, draws: &VolatileDraws| {
        volatile_inventory(EarthMasses::new(1.0), &fractions, side, draws, SOLAR_AGE)
    };
    let inside = at(SnowLineSide::Inside, &VolatileDraws::MEDIAN);
    let beyond = at(SnowLineSide::Beyond, &VolatileDraws::MEDIAN);
    // Earth's own, at the median.
    assert!(
        (inside.nitrogen().value() / (EARTH_NITROGEN_PER_MASS * EARTH_MASS_KG) - 1.0).abs() < 1e-12
    );
    assert!((inside.water().value() / (EARTH_WATER_PER_MASS * EARTH_MASS_KG) - 1.0).abs() < 1e-12);
    // Beyond the snow line, a hundred times the carbon and nitrogen.
    assert!(
        (beyond.nitrogen().value() / inside.nitrogen().value() - ICY_VOLATILE_BOOST).abs() < 1e-9
    );
    assert!(
        (beyond.carbon_dioxide().value() / inside.carbon_dioxide().value() - ICY_VOLATILE_BOOST)
            .abs()
            < 1e-9
    );
    // An icy body's water is its ice.
    let icy = composition(
        EarthMasses::new(0.02),
        EarthRadii::new(0.4),
        SnowLineSide::Beyond,
        EarthFluxes::new(0.01),
    )
    .unwrap();
    let ice = volatile_inventory(
        EarthMasses::new(0.02),
        &icy.fractions(),
        SnowLineSide::Beyond,
        &VolatileDraws::MEDIAN,
        SOLAR_AGE,
    );
    assert!(icy.fractions().water() > 0.1);
    assert!(
        (ice.water().value() / (icy.fractions().water() * 0.02 * EARTH_MASS_KG) - 1.0).abs()
            < 1e-12
    );
    // One sigma of the rank is half a decade.
    let sigma = UnitUniform::new(0.841_344_746_068_542_9).unwrap();
    let high = at(
        SnowLineSide::Inside,
        &VolatileDraws {
            nitrogen: sigma,
            ..VolatileDraws::MEDIAN
        },
    );
    assert!(
        (high.nitrogen().value() / inside.nitrogen().value() - math::powf(10.0, 0.5)).abs() < 1e-6
    );
    // Argon grows with the age.
    let young = volatile_inventory(
        EarthMasses::new(1.0),
        &fractions,
        SnowLineSide::Inside,
        &VolatileDraws::MEDIAN,
        Years::new(1e9),
    );
    assert!(young.argon() < inside.argon());
}

#[test]
fn the_volatile_draws_read_their_three_words() {
    let system = crate::id::SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let body = BodyId::new(system, 0x0100);
    let seed = Seed::new(3);
    let draws = VolatileDraws::for_body(seed, body);
    let mut stream = Stream::open(seed, tags::PLANET_VOLATILES, ObjectKey::from(body));
    assert_same_bits(draws.water.value(), stream.uniform_open());
    assert_same_bits(draws.carbon.value(), stream.uniform_open());
    assert_same_bits(draws.nitrogen.value(), stream.uniform_open());
}

#[test]
fn the_solar_system_keeps_its_atmospheres_where_it_does() {
    // P14.T13.b: Earth, Venus, Mars and Titan keep atmospheres; Mercury, the Moon, Ganymede and
    // Ceres do not.
    let table = table();
    for (name, body) in &table {
        let air = body.atmosphere();
        eprintln!(
            "{name}: {:?} at {:.1} K, {:?} Pa, CO2 {:.3e} Pa, N2 {:.3e} Pa, T_eq {:.1} K, T_exo {:.0} K",
            air.state(),
            air.surface_temperature().value(),
            air.surface_pressure().map(Pascals::value),
            air.partial_pressures().of(Gas::CarbonDioxide).value(),
            air.partial_pressures().of(Gas::Nitrogen).value(),
            body.equilibrium_temperature().value(),
            air.exobase_temperature().value(),
        );
    }
    for name in ["Earth", "Venus", "Mars", "Titan"] {
        let air = found(&table, name).atmosphere();
        assert!(air.keeps_atmosphere(), "{name}: {air:?}");
    }
    for name in ["Mercury", "Moon", "Ganymede", "Ceres"] {
        let air = found(&table, name).atmosphere();
        assert_eq!(air.state(), SurfaceState::Airless, "{name}: {air:?}");
    }
    // The states the table's atmospheres are in.
    assert_eq!(
        found(&table, "Venus").atmosphere().state(),
        SurfaceState::RunawayGreenhouse
    );
    assert_eq!(
        found(&table, "Earth").atmosphere().state(),
        SurfaceState::Temperate
    );
    assert_eq!(
        found(&table, "Mars").atmosphere().state(),
        SurfaceState::Temperate
    );
    assert_eq!(
        found(&table, "Titan").atmosphere().state(),
        SurfaceState::Snowball
    );
    // Earth keeps its nitrogen and water and loses its hydrogen and helium.
    let earth = found(&table, "Earth").atmosphere().retention();
    assert!(earth.retains(Gas::Nitrogen) && earth.retains(Gas::Water));
    assert!(!earth.retains(Gas::Hydrogen) && !earth.retains(Gas::Helium));
    // Earth's air is nitrogen with a trace of carbon dioxide, near one bar.
    let air = found(&table, "Earth").atmosphere();
    let bars = air.surface_pressure().unwrap().value() / 1e5;
    assert!((0.7..1.1).contains(&bars), "Earth at {bars} bar");
    let co2 = air.partial_pressures().of(Gas::CarbonDioxide).value();
    assert!((20.0..60.0).contains(&co2), "Earth's CO2 at {co2} Pa");
    // Venus's is tens of bars of carbon dioxide, and Titan's over a bar of nitrogen.
    let venus = found(&table, "Venus").atmosphere();
    assert!(venus.partial_pressures().of(Gas::CarbonDioxide).value() > 3e6);
    assert_eq!(venus.partial_pressures().of(Gas::Water), Pascals::ZERO);
    // Mars keeps 0.11 bar, nearly all nitrogen: its real 6 mbar needs the non-thermal loss this
    // model has not (ruling 119.4, pinned).
    let mars = found(&table, "Mars").atmosphere();
    let mars_bar = mars.surface_pressure().unwrap().value() / 1e5;
    assert!((0.10..0.12).contains(&mars_bar), "Mars at {mars_bar} bar");
    let titan = found(&table, "Titan").atmosphere();
    assert!(titan.partial_pressures().of(Gas::Nitrogen).value() > 1e5);
}

#[test]
fn the_table_s_surface_temperatures_are_within_eight_per_cent() {
    // P14.T13.c: Venus 735 K, Earth 288 K, Mars 215 K, Titan 94 K.
    let table = table();
    for (name, kelvin) in [
        ("Venus", 735.0),
        ("Earth", 288.0),
        ("Mars", 215.0),
        ("Titan", 94.0),
    ] {
        let t = found(&table, name).surface_temperature().value();
        assert!(
            ((t - kelvin) / kelvin).abs() < 0.08,
            "{name} at {t} K against {kelvin}"
        );
    }
    // The albedo each takes is its state's, and the equilibrium temperature is at it: Earth's
    // 0.294 gives 255 K (254 K at the 0.306 before decision-p11-t4k-faults).
    let earth = found(&table, "Earth");
    assert_same_bits(earth.albedo().value(), 0.294);
    let earth_eq = earth.equilibrium_temperature().value();
    assert!((earth_eq - 255.0).abs() < 1.0, "Earth's T_eq {earth_eq} K");
    let venus = found(&table, "Venus");
    assert_same_bits(venus.albedo().value(), 0.76);
    assert!((venus.equilibrium_temperature().value() - 229.0).abs() < 1.0);
    // An airless body sits at its equilibrium temperature.
    let moon = found(&table, "Moon");
    assert_same_bits(
        moon.surface_temperature().value(),
        moon.equilibrium_temperature().value(),
    );
}

#[test]
fn the_greenhouse_settles_on_its_fixed_point() {
    // The fixed three passes and the 32 greenhouse steps settle: one more step moves no surface
    // temperature of the table by a part in a billion, and the passes agree on the albedo.
    for (name, body) in table() {
        let air = body.atmosphere();
        assert_same_bits(air.albedo().value(), body.albedo().value());
        if air.state() == SurfaceState::Airless {
            continue;
        }
        let t = air.surface_temperature();
        let again = grey_surface_temperature(body.equilibrium_temperature(), air.optical_depth());
        assert!(
            ((again.value() - t.value()) / t.value()).abs() < 1e-9,
            "{name}: {t:?} against {again:?}"
        );
    }
}

/// A 5 M⊕ body at `a_au` of the Sun, formed there, at the rank whose envelope is 2% of its mass.
fn two_per_cent_envelope(disc: &DiscProfile, a_au: f64) -> PlacedBody {
    let o = orbit(a_au, 0.0);
    let at = |rank: f64| {
        PlacedBody::new(
            EarthMasses::new(5.0),
            o,
            o.semi_major_axis(),
            UnitUniform::new(rank).unwrap(),
        )
        .unwrap()
    };
    let envelope = |rank: f64| {
        formation_composition(&at(rank), disc)
            .unwrap()
            .unwrap()
            .envelope_fraction()
    };
    let (mut lo, mut hi) = (1e-9, 1.0 - 1e-9);
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if envelope(mid) < 0.02 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let placed = at(hi);
    assert!((envelope(hi) - 0.02).abs() < 1e-4);
    placed
}

#[test]
fn a_close_in_envelope_is_stripped_and_a_distant_one_is_not() {
    // P14.T13.b: a 5 M⊕ core with a 2% envelope at 0.05 au of a Sun is stripped by 1 Gyr; the same
    // body at 0.5 au is not.
    let disc = solar_disc();
    let close = derive(&two_per_cent_envelope(&disc, 0.05), &disc, Years::new(1e9));
    assert_same_bits(close.fractions().envelope(), 0.0);
    assert!(close.envelope_lost().value() > 0.09);
    assert_eq!(close.class(), crate::planetary::derive::PlanetClass::Rocky);
    // Stripped, it takes Zeng et al.'s radius of its core.
    assert!(close.radius().value() < 1.8, "{:?}", close.radius());
    let far = derive(&two_per_cent_envelope(&disc, 0.5), &disc, Years::new(1e9));
    assert!(far.fractions().envelope() > 0.018, "{far:?}");
    assert!(far.radius().value() > 2.0);
}

#[test]
fn the_envelope_fraction_is_monotone_and_continuous_in_time() {
    let disc = solar_disc();
    for a in [0.03, 0.05, 0.08, 0.15] {
        let placed = two_per_cent_envelope(&disc, a);
        let mut previous: Option<(f64, f64)> = None;
        let mut age = 1e6;
        while age < 1.2e10 {
            let body = derive(&placed, &disc, Years::new(age));
            let (f, m) = (body.fractions().envelope(), body.mass().value());
            if let Some((f0, m0)) = previous {
                assert!(f <= f0, "the envelope grows at {a} au, {age} yr");
                assert!(m <= m0);
                // A step of 1% in age takes at most 2% of the initial envelope.
                assert!(
                    f0 - f <= 0.02 * 0.02,
                    "a jump at {a} au, {age} yr: {f0} to {f}"
                );
            }
            previous = Some((f, m));
            age *= 1.01;
        }
    }
}

#[test]
fn no_surface_is_hotter_than_its_hottest_host() {
    // Bodies from 0.005 to 50 au of a Sun, from a Ceres to 20 M⊕, and one about a cool dwarf.
    let disc = solar_disc();
    let mut hottest_seen = 0.0_f64;
    for i in 0..40 {
        let a = 0.005 * math::exp10(f64::from(i) / 10.0);
        for m in [1e-3, 0.1, 1.0, 5.0, 20.0] {
            for rank in [0.1, 0.5, 0.9] {
                let o = orbit(a, 0.0);
                let placed = PlacedBody::new(
                    EarthMasses::new(m),
                    o,
                    o.semi_major_axis(),
                    UnitUniform::new(rank).unwrap(),
                )
                .unwrap();
                let body = derive(&placed, &disc, SOLAR_AGE);
                let t = body.surface_temperature().value();
                hottest_seen = hottest_seen.max(t);
                assert!(t <= 5_772.0, "{m} M⊕ at {a} au: {t} K");
            }
        }
    }
    assert!(hottest_seen > SILICATE_SOLIDUS.value());
    // The saturation itself: a thick greenhouse about a cool host stops at its temperature.
    let inputs = AtmosphereInputs {
        mass: Kilograms::new(5.0 * EARTH_MASS_KG),
        radius: Metres::new(1.6 * crate::units::consts::EARTH_RADIUS_M),
        material: SurfaceMaterial::Rock,
        envelope_fraction: 0.0,
        inventory: VolatileInventory::new(
            Kilograms::ZERO,
            Kilograms::new(1e24),
            Kilograms::ZERO,
            Kilograms::ZERO,
        ),
        formed: SnowLineSide::Inside,
        equilibrium: Kelvin::new(1_500.0),
        worst_equilibrium: Kelvin::new(1_500.0),
        heated: Kelvin::new(1_500.0),
        xuv_fluence: JoulesPerSquareMetre::ZERO,
        saturated_xuv_flux: WattsPerSquareMetre::ZERO,
        insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(3_000.0),
    };
    let air = atmosphere(&inputs);
    assert_same_bits(air.surface_temperature().value(), 3_000.0);
    assert_eq!(air.state(), SurfaceState::MagmaOcean);
}

#[test]
fn a_young_crust_is_a_magma_ocean_and_a_giant_is_an_envelope() {
    let disc = solar_disc();
    let o = orbit(1.0, 0.0167);
    let earth = PlacedBody::new(
        EarthMasses::new(1.0),
        o,
        o.semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap()
    .with_magma_ocean_until(Years::new(5e7));
    let young = derive(&earth, &disc, Years::new(3e7));
    assert_eq!(young.atmosphere().state(), SurfaceState::MagmaOcean);
    assert!(young.surface_temperature() >= SILICATE_SOLIDUS);
    let old = derive(&earth, &disc, Years::new(6e7));
    assert_ne!(old.atmosphere().state(), SurfaceState::MagmaOcean);
    let jupiter = PlacedBody::new(
        EarthMasses::new(317.8),
        orbit(5.2, 0.0489),
        orbit(5.2, 0.0489).semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap();
    let jupiter = derive(&jupiter, &disc, SOLAR_AGE);
    let air = jupiter.atmosphere();
    assert_eq!(air.state(), SurfaceState::GasEnvelope);
    assert_eq!(air.surface_pressure(), None);
    assert_same_bits(jupiter.albedo().value(), 0.34);
    // Jupiter's 110 K at its albedo, and its surface the temperature it radiates at.
    assert!((jupiter.equilibrium_temperature().value() - 110.0).abs() < 2.0);
    assert_same_bits(
        air.surface_temperature().value(),
        jupiter
            .effective_temperature()
            .expect("a giant radiates its own")
            .value(),
    );
}

#[test]
fn escape_and_retention_follow_their_formulae() {
    // λ of Earth's nitrogen at 1,000 K, by hand.
    let lambda = jeans_parameter(
        Kilograms::new(EARTH_MASS_KG),
        Metres::new(6.371e6),
        Gas::Nitrogen,
        Kelvin::new(1_000.0),
    );
    let by_hand = GRAVITATIONAL_CONSTANT * EARTH_MASS_KG * 28.014 * ATOMIC_MASS_CONSTANT_KG
        / (BOLTZMANN_CONSTANT * 1_000.0 * 6.371e6);
    assert!((lambda / by_hand - 1.0).abs() < 1e-12);
    assert!(
        jeans_parameter(
            Kilograms::new(1.0),
            Metres::new(1.0),
            Gas::Water,
            Kelvin::ZERO
        )
        .is_infinite()
    );
    // Heavier species are held first.
    let retention = Retention::of(
        Kilograms::new(0.1 * EARTH_MASS_KG),
        Metres::new(3.4e6),
        Kelvin::new(2_000.0),
    );
    assert!(retention.retains(Gas::CarbonDioxide));
    assert!(!retention.retains(Gas::Hydrogen));
    // The exobase takes the worst temperature and the X-rays beyond Earth's.
    let earth = earth_xuv_fluence();
    assert!((earth.value() / 2.57e15 - 1.0).abs() < 0.01, "{earth:?}");
    assert_same_bits(
        exobase_temperature(Kelvin::new(250.0), earth).value(),
        EXOBASE_MULTIPLE * 250.0,
    );
    let quadruple = JoulesPerSquareMetre::new(4.0 * earth.value());
    assert!(
        (exobase_temperature(Kelvin::new(250.0), quadruple).value() / (2.0 * 5.0 * 250.0) - 1.0)
            .abs()
            < 1e-12
    );
    // Erkaev's factor tends to 1 far inside the lobe and falls towards it.
    assert!((roche_lobe_factor(Metres::new(1e9), Metres::new(1.0)) - 1.0).abs() < 1e-8);
    assert!(roche_lobe_factor(Metres::new(3.0), Metres::new(1.0)) < 0.6);
    assert!(roche_lobe_factor(Metres::new(0.5), Metres::new(1.0)) > 0.0);
    // Energy-limited loss by hand.
    let loss = energy_limited_loss(
        Kilograms::new(3e25),
        Metres::new(1.5e7),
        JoulesPerSquareMetre::new(1e17),
        Metres::new(1e15),
    );
    let k = roche_lobe_factor(Metres::new(1e15), Metres::new(1.5e7));
    let expected = 0.1 * core::f64::consts::PI * math::powi(1.5e7, 3) * 1e17
        / (GRAVITATIONAL_CONSTANT * 3e25 * k);
    assert!((loss.value() / expected - 1.0).abs() < 1e-12);
}

/// P14.T13.c's private Clausius–Clapeyron constants, frozen here as they were before the substance
/// registry took them (P14.T49.a): triple temperature (K) and pressure (Pa), the enthalpies of
/// sublimation and vaporisation (J mol⁻¹) and the critical temperature (K).
const T13_CONDENSATION: [(Gas, [f64; 5]); 4] = [
    (Gas::Water, [273.16, 611.657, 51_059.0, 43_500.0, 647.1]),
    (
        Gas::CarbonDioxide,
        [216.58, 518_500.0, 26_100.0, 15_300.0, 304.13],
    ),
    (Gas::Nitrogen, [63.15, 12_520.0, 6_900.0, 5_570.0, 126.19]),
    (Gas::Argon, [83.81, 68_890.0, 7_800.0, 6_430.0, 150.69]),
];

/// P14.T13.c's private saturation pressure, verbatim, on the constants `c` of
/// [`T13_CONDENSATION`].
fn t13_saturation_pressure(c: [f64; 5], t: f64) -> f64 {
    let [
        triple_temperature,
        triple_pressure,
        sublimation,
        vaporisation,
        critical_temperature,
    ] = c;
    if t >= critical_temperature {
        return f64::INFINITY;
    }
    if t.is_nan() || t <= 0.0 {
        return 0.0;
    }
    let enthalpy = if t < triple_temperature {
        sublimation
    } else {
        vaporisation
    };
    // The molar gas constant as P14.T13.c wrote it, frozen too.
    let molar_gas_constant = 8.314_462_618_153_24;
    triple_pressure
        * math::exp(-(enthalpy / molar_gas_constant) * (1.0 / t - 1.0 / triple_temperature))
}

/// The registry's saturation pressures of the four are P14.T13.c's to the bit, at 200 temperatures
/// from a fifth of the triple point to past the critical point, and at the edges.
#[test]
fn the_registry_s_saturation_pressures_are_t13_s_to_the_bit() {
    for (gas, c) in T13_CONDENSATION {
        let (low, high) = (0.2 * c[0], 1.1 * c[4]);
        let mut temperatures: Vec<f64> = (0..200)
            .map(|i| low + (high - low) * f64::from(i) / 199.0)
            .collect();
        temperatures.extend([0.0, -1.0, f64::NAN, c[0], c[4], f64::INFINITY]);
        for t in temperatures {
            let registry = saturation_pressure(gas.substance(), Kelvin::new(t))
                .expect("the gas has phase data")
                .value();
            assert_same_bits(registry, t13_saturation_pressure(c, t));
        }
        let phase = gas.substance().substance().phase().unwrap();
        assert_same_bits(phase.triple_temperature().value(), c[0]);
        assert_same_bits(phase.triple_pressure().value(), c[1]);
        assert_same_bits(phase.sublimation_enthalpy_j_per_mol(), c[2]);
        assert_same_bits(phase.vaporisation_enthalpy_j_per_mol(), c[3]);
        assert_same_bits(phase.critical_temperature().value(), c[4]);
    }
}

#[test]
fn a_dark_host_leaves_a_body_frozen_and_airless() {
    let disc = solar_disc();
    let dark = [HostLight::new(
        crate::units::SolarLuminosities::ZERO,
        Kelvin::ZERO,
        crate::units::SolarRadii::ZERO,
    )
    .unwrap()];
    let hosts = BodyHosts::new(
        Kilograms::new(SOLAR_MASS_KG),
        Composition::SOLAR,
        &dark,
        &[],
    )
    .unwrap();
    let o = orbit(1.0, 0.0);
    let placed = PlacedBody::new(
        EarthMasses::new(1.0),
        o,
        o.semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap();
    let body = derive_body(&placed, &hosts, &disc, SOLAR_AGE, UniverseTime::EPOCH).unwrap();
    assert_eq!(body.atmosphere().state(), SurfaceState::Airless);
    assert_eq!(body.surface_temperature(), Kelvin::ZERO);
    // And the radius rank at the median still reads Chen and Kipping's scatter.
    assert!(radius_chen_kipping(EarthMasses::new(1.0), UnitUniform::HALF).value() > 0.9);
}

// P14.T24.f: carbon speciation and oxygen, in force from the 21 → 22 batch's bump.

/// The mole fraction of `gas` in the air `air`, (p ÷ μ) ÷ Σ(pⱼ ÷ μⱼ): the partial pressures are
/// the gases' column weights, which count mass, not molecules (signoff-3.md, item 1.7).
fn mole_fraction(air: &Atmosphere, gas: Gas) -> f64 {
    let moles = |gas: Gas| air.partial_pressures().of(gas).value() / gas.molar_mass_g_per_mol();
    moles(gas) / Gas::ALL.iter().fold(0.0, |sum, &g| sum + moles(g))
}

/// Methane's partial pressure in the air `air`, Pa: its mole fraction times the surface pressure.
fn methane_in_the_air(air: &Atmosphere) -> f64 {
    mole_fraction(air, Gas::Methane) * air.partial_pressures().total().value()
}

#[test]
fn titan_carries_methane_at_its_measured_share() {
    // P14.T24.f: the table's Titan carries CH₄ at 5.65% of its surface gas by number to 10%
    // (Niemann et al. 2010), held over its reservoir at the cited humidity, not fitted, which caps
    // its partial pressure in the air, x p_s (signoff-3.md, item 1.7).
    let table = table();
    let titan = found(&table, "Titan");
    let air = titan.atmosphere();
    let x = mole_fraction(air, Gas::Methane);
    eprintln!(
        "Titan: {:.3} K, {:.4} bar, CH4 {:.4} by number ({:.4} by mass), N2 {:.4}",
        air.surface_temperature().value(),
        air.surface_pressure().unwrap().value() / 1e5,
        x,
        air.partial_pressures().of(Gas::Methane).value() / air.partial_pressures().total().value(),
        mole_fraction(air, Gas::Nitrogen),
    );
    assert_eq!(air.state(), SurfaceState::Snowball);
    assert_eq!(air.carbon_carrier(), Some(CarbonCarrier::Methane));
    assert!(((x - 0.0565) / 0.0565).abs() < 0.10, "Titan's CH4 at {x}");
    assert_eq!(
        air.partial_pressures().of(Gas::CarbonDioxide),
        Pascals::ZERO
    );
    // Its carbon exceeds what its air holds: the air sits at the humidity over the reservoir.
    let saturated = saturation_pressure(Gas::Methane.substance(), air.surface_temperature())
        .unwrap()
        .value();
    let in_the_air = methane_in_the_air(air);
    assert!(
        (in_the_air / (METHANE_RELATIVE_HUMIDITY * saturated) - 1.0).abs() < 1e-12,
        "{in_the_air} Pa against {saturated} Pa saturated"
    );
    let weight = titan.surface_gravity().value()
        / (4.0 * core::f64::consts::PI * math::powi(Metres::from(titan.radius()).value(), 2));
    let carbon = titan.inventory().carbon_dioxide().value() * Gas::Methane.molar_mass_g_per_mol()
        / Gas::CarbonDioxide.molar_mass_g_per_mol();
    assert!(carbon * weight > 10.0 * air.partial_pressures().of(Gas::Methane).value());
    // Its methane is kept with its nitrogen, though its own lighter molecule fails Jeans's test
    // at the exobase multiple.
    assert!(air.retention().retains(Gas::Nitrogen));
    assert!(air.retention().retains(Gas::Methane));
    let mass = Kilograms::from(titan.mass());
    let radius = Metres::from(titan.radius());
    assert!(!Retention::of(mass, radius, air.exobase_temperature()).retains(Gas::Methane));
    // The grey greenhouse reads no methane, and Titan stays within T13.c's 8% of 94 K.
    let t = air.surface_temperature().value();
    assert!(((t - 94.0) / 94.0).abs() < 0.08, "Titan at {t} K");
}

#[test]
fn earth_venus_and_mars_keep_their_carbon_as_carbon_dioxide() {
    // P14.T24.f: the inner planets formed inside the snow line keep their carbon as CO₂: Venus's
    // water was lost, but its sinks have taken all its oxygen. Methane's retention follows
    // nitrogen's (science-p14-t24f §4).
    let speciated = table();
    for name in ["Mercury", "Venus", "Earth", "Mars", "Moon"] {
        let air = found(&speciated, name).atmosphere();
        assert_eq!(
            air.carbon_carrier(),
            Some(CarbonCarrier::CarbonDioxide),
            "{name}"
        );
        assert_eq!(
            air.partial_pressures().of(Gas::Methane),
            Pascals::ZERO,
            "{name}"
        );
        assert_eq!(
            air.retention().retains(Gas::Methane),
            air.retention().retains(Gas::Nitrogen),
            "{name}"
        );
    }
    for name in ["Venus", "Earth", "Mars"] {
        let air = found(&speciated, name).atmosphere();
        assert!(air.partial_pressures().of(Gas::CarbonDioxide) > Pascals::ZERO);
    }
    // Ceres, at 2.77 au, formed beyond the generated Sun's snow line, at 2.26 au, and lies beyond
    // the maximum-greenhouse limit: its carrier is methane (science-p14-t24f §5), though it keeps
    // no nitrogen and so no methane.
    let ceres = found(&speciated, "Ceres");
    assert_eq!(ceres.formed(), SnowLineSide::Beyond);
    let air = ceres.atmosphere();
    assert_eq!(air.state(), SurfaceState::Airless);
    assert_eq!(air.carbon_carrier(), Some(CarbonCarrier::Methane));
    assert!(!air.retention().retains(Gas::Nitrogen));
    assert_eq!(air.partial_pressures().of(Gas::Methane), Pascals::ZERO);
    // Biotic oxygen is a gap for the owner: no plan produces life, so the generated Earth has no
    // O₂, and no ozone follows from it (plan 14, Risks, the rendering plans' asks, question 1).
    let earth = found(&speciated, "Earth").atmosphere();
    assert_eq!(earth.partial_pressures().of(Gas::Oxygen), Pascals::ZERO);
}

#[test]
fn venus_s_oxygen_is_under_a_thousandth_of_its_carbon_dioxide_by_its_sinks() {
    // P14.T24.f: Venus lost its ocean, and its O₂ stays under 10⁻³ of its CO₂, which the sinks
    // must give: the oxygen its water freed, less only what escaped with the hydrogen, would be
    // more than its carbon dioxide. Venus's measured O₂ is under 0.3 ppmv above 58 km (Trauger
    // and Lunine 1983, as Marcq et al. 2018, Space Sci. Rev. 214, 10, give it), and the
    // generated Venus's is none.
    let table = table();
    let venus = found(&table, "Venus");
    let air = venus.atmosphere();
    assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
    let (o2, co2) = (
        air.partial_pressures().of(Gas::Oxygen).value(),
        air.partial_pressures().of(Gas::CarbonDioxide).value(),
    );
    assert!(
        o2 < 1e-3 * co2,
        "Venus's O2 at {o2} Pa against {co2} Pa of CO2"
    );
    // Its oxygen is not lost to Jeans escape: its sinks take it.
    assert!(air.retention().retains(Gas::Oxygen));
    let lost = venus.inventory().water();
    let (mass, radius) = (Kilograms::from(venus.mass()), Metres::from(venus.radius()));
    let freed = lost.value() * Gas::Oxygen.molar_mass_g_per_mol()
        / (2.0 * Gas::Water.molar_mass_g_per_mol());
    let lights = [historic_sun()];
    let eta = oxygen_escape_parameter(
        mass,
        radius,
        lights[0]
            .xuv()
            .saturated_flux(Metres::new(0.723_332 * METRES_PER_AU), 0.006_772),
    );
    let weight = venus.surface_gravity().value()
        / (4.0 * core::f64::consts::PI * math::powi(radius.value(), 2));
    eprintln!(
        "Venus: water lost {:.3e} kg, eta {eta:.3}, O2 before its sinks {:.1} bar, CO2 {:.1} bar",
        lost.value(),
        freed * (1.0 - eta) * weight / 1e5,
        co2 / 1e5
    );
    assert!(lost > Kilograms::ZERO);
    assert!(freed * (1.0 - eta) * weight > co2);
}

/// A runaway Earth that lost the water `water` (kg), at `age`, with the saturated X-ray and
/// ultraviolet flux `xuv` (W m⁻²), in equilibrium at `equilibrium` (K).
fn runaway_earth(water: f64, age: f64, xuv: f64, equilibrium: f64) -> Atmosphere {
    atmosphere(&AtmosphereInputs {
        mass: Kilograms::new(EARTH_MASS_KG),
        radius: Metres::new(crate::units::consts::EARTH_RADIUS_M),
        material: SurfaceMaterial::Rock,
        envelope_fraction: 0.0,
        inventory: VolatileInventory::new(
            Kilograms::new(water),
            Kilograms::new(3.6e20),
            Kilograms::new(3.9e18),
            Kilograms::new(6.6e16),
        ),
        formed: SnowLineSide::Inside,
        equilibrium: Kelvin::new(equilibrium),
        worst_equilibrium: Kelvin::new(equilibrium),
        heated: Kelvin::new(equilibrium),
        xuv_fluence: earth_xuv_fluence(),
        saturated_xuv_flux: WattsPerSquareMetre::new(xuv),
        insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: Years::new(age),
        hottest_host: Kelvin::new(5_772.0),
    })
}

#[test]
fn a_runaway_world_carries_the_oxygen_of_the_water_it_lost_less_its_sinks() {
    // P14.T24.f: the oxygen of the water lost, less what escapes with the hydrogen (Luger and
    // Barnes 2015, eqs. 5, 11 and 12) and what the surface takes up at Earth's rate per unit area
    // (Catling 2014, through Luger and Barnes's §2.5.1), by hand.
    let (water, age, xuv) = (1.4e22, 5e8, 1.0);
    let air = runaway_earth(water, age, xuv, 255.0);
    assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
    assert!(air.retention().retains(Gas::Oxygen));
    let (mass, radius) = (
        Kilograms::new(EARTH_MASS_KG),
        Metres::new(crate::units::consts::EARTH_RADIUS_M),
    );
    let hydrogen = 1.008 * ATOMIC_MASS_CONSTANT_KG;
    let g = GRAVITATIONAL_CONSTANT * mass.value() / math::powi(radius.value(), 2);
    let reference =
        0.30 * xuv * radius.value() / (4.0 * GRAVITATIONAL_CONSTANT * mass.value() * hydrogen);
    let x = BOLTZMANN_CONSTANT * 400.0 * reference
        / (10.0 * 4.8e19 * math::powf(400.0, 0.75) * g * hydrogen);
    let eta = (x - 1.0) / (x + 8.0);
    assert!(
        (oxygen_escape_parameter(mass, radius, WattsPerSquareMetre::new(xuv)) / eta - 1.0).abs()
            < 1e-12
    );
    assert!(eta > 0.1 && eta < 0.9, "η {eta}");
    let freed = water * 31.998 / (2.0 * 18.015);
    let taken = 2.21e13 * age * 31.998e-3;
    let kept = freed * (1.0 - eta) - taken;
    let by_paper = runaway_oxygen(
        Kilograms::new(water),
        mass,
        radius,
        WattsPerSquareMetre::new(xuv),
        Years::ZERO,
        Years::new(age),
    );
    assert!(
        (by_paper.value() / kept - 1.0).abs() < 1e-12,
        "{by_paper:?} against {kept}"
    );
    let weight = g / (4.0 * core::f64::consts::PI * math::powi(radius.value(), 2));
    let o2 = air.partial_pressures().of(Gas::Oxygen).value();
    assert!((o2 / (kept * weight) - 1.0).abs() < 1e-12, "{o2} Pa");
    eprintln!(
        "ten oceans lost: {:.0} bar of O2 at {:.1} Gyr, η {eta:.3}",
        o2 / 1e5,
        age / 1e9
    );
    // Oxygen never condenses, and the air holds it with the carbon dioxide.
    assert!(o2 > air.partial_pressures().of(Gas::CarbonDioxide).value());
    // One that lost no water carries none.
    let dry = runaway_earth(0.0, age, xuv, 255.0);
    assert_eq!(dry.partial_pressures().of(Gas::Oxygen), Pascals::ZERO);
    // Its surface takes it all up in time, and a magma ocean at once.
    let old = runaway_earth(water, 2e10, xuv, 255.0);
    assert_eq!(old.partial_pressures().of(Gas::Oxygen), Pascals::ZERO);
    let molten = runaway_earth(water, age, xuv, 1_400.0);
    assert_eq!(molten.state(), SurfaceState::MagmaOcean);
    assert_eq!(molten.partial_pressures().of(Gas::Oxygen), Pascals::ZERO);
    // Below the critical flux no oxygen escapes, and with more of it more does.
    assert!(oxygen_escape_parameter(mass, radius, WattsPerSquareMetre::new(0.17)) <= 0.0);
    let more = runaway_earth(water, age, 10.0, 255.0);
    assert!(more.partial_pressures().of(Gas::Oxygen).value() < o2);
}

#[test]
fn oxygen_escapes_above_luger_and_barnes_s_critical_flux() {
    // Luger and Barnes's eq. 9: F_crit = 180 (M ÷ M⊕)² (R ÷ R⊕)⁻³ erg cm⁻² s⁻¹ at ε 0.30, which
    // the constants give to within their rounding (178 for Earth).
    for (m, r) in [(1.0, 1.0), (5.0, 1.5), (0.815, 0.95), (0.107, 0.532)] {
        let mass = Kilograms::new(m * EARTH_MASS_KG);
        let radius = Metres::new(r * crate::units::consts::EARTH_RADIUS_M);
        let critical = 0.18 * m * m / (r * r * r);
        let eta = |f: f64| oxygen_escape_parameter(mass, radius, WattsPerSquareMetre::new(f));
        assert!(eta(0.97 * critical) <= 0.0, "{m} M⊕");
        assert!(eta(1.03 * critical) > 0.0, "{m} M⊕");
        // η rises towards 1, the water's own proportion, far above it.
        assert!(eta(10.0 * critical) > eta(2.0 * critical));
        assert!((eta(1e4 * critical) - 1.0).abs() < 1e-3);
    }
    let earth = (
        Kilograms::new(EARTH_MASS_KG),
        Metres::new(crate::units::consts::EARTH_RADIUS_M),
    );
    assert!(oxygen_escape_parameter(earth.0, earth.1, WattsPerSquareMetre::new(f64::NAN)) <= 0.0);
    assert!(
        oxygen_escape_parameter(Kilograms::ZERO, earth.1, WattsPerSquareMetre::new(1.0)) <= 0.0
    );
    assert_same_bits(
        oxygen_escape_parameter(earth.0, earth.1, WattsPerSquareMetre::new(f64::INFINITY)),
        1.0,
    );
}

/// An Earth ocean, kg, as the runaway tests count oceans.
const OCEAN: f64 = 1.4e21;

/// The saturated X-ray and ultraviolet flux at Venus's orbit about the Sun with its history.
fn venus_saturated_flux() -> WattsPerSquareMetre {
    historic_sun()
        .xuv()
        .saturated_flux(Metres::new(0.723_332 * METRES_PER_AU), 0.0)
}

/// The rate at which a runaway world of `mass` and `radius` loses its water under the saturated
/// flux `xuv`, kg s⁻¹, by hand: Luger and Barnes's eq. A9, 9 ÷ (1 + 8η) times the energy-limited
/// rate 0.30 π R³ F ÷ (G M).
fn water_loss_rate(mass: Kilograms, radius: Metres, xuv: WattsPerSquareMetre) -> f64 {
    let eta = oxygen_escape_parameter(mass, radius, xuv);
    let r = radius.value();
    let energy_limited = 0.30 * core::f64::consts::PI * r * r * r * xuv.value()
        / (GRAVITATIONAL_CONSTANT * mass.value());
    9.0 * energy_limited / (1.0 + 8.0 * eta)
}

#[test]
fn a_young_melt_keeps_the_oxygen_of_the_water_lost_while_it_lasted() {
    // Science-p14-t24f §3: the water lost while the young magma ocean lasted gave its oxygen to
    // the melt, which keeps it (Schaefer et al. 2016, §4.2), at T24.f's eq. A9 rate.
    let (mass, radius) = (
        Kilograms::new(EARTH_MASS_KG),
        Metres::new(crate::units::consts::EARTH_RADIUS_M),
    );
    let xuv = venus_saturated_flux();
    let eta = oxygen_escape_parameter(mass, radius, xuv);
    assert!(eta > 0.1 && eta < 0.3, "η {eta} at Venus's saturated flux");
    let kept = |water: f64, melt: f64, age: f64| {
        runaway_oxygen(
            Kilograms::new(water),
            mass,
            radius,
            xuv,
            Years::new(melt),
            Years::new(age),
        )
        .value()
    };
    // With no melt it is the as-built figure, bit for bit, for the hand-computed cases.
    for (water, flux, age) in [(1.4e22, 1.0, 5e8), (OCEAN, 1.0, 1e8), (OCEAN, 1.0, 1e9)] {
        let flux = WattsPerSquareMetre::new(flux);
        let freed =
            water * Gas::Oxygen.molar_mass_g_per_mol() / (2.0 * Gas::Water.molar_mass_g_per_mol());
        let before = freed * (1.0 - oxygen_escape_parameter(mass, radius, flux))
            - EARTH_OXYGEN_SINK_MOL_PER_YEAR * age * Gas::Oxygen.molar_mass_g_per_mol() * 1e-3;
        let at_zero = runaway_oxygen(
            Kilograms::new(water),
            mass,
            radius,
            flux,
            Years::ZERO,
            Years::new(age),
        );
        assert_same_bits(at_zero.value(), before.max(0.0));
    }
    // Six oceans and a 31.6 Myr melt keep less, by the melt's share of the loss, by hand.
    let (water, melt) = (6.0 * OCEAN, 3.16e7);
    let lost_into_the_melt = water_loss_rate(mass, radius, xuv) * melt * 31_557_600.0;
    assert!(lost_into_the_melt < water);
    let difference = lost_into_the_melt * 31.998 / (2.0 * 18.015) * (1.0 - eta);
    let (none, molten) = (kept(water, 0.0, 4.57e9), kept(water, melt, 4.57e9));
    assert!(molten > 0.0);
    assert!(
        ((none - molten) / difference - 1.0).abs() < 1e-12,
        "{none} − {molten} against {difference}"
    );
    // A bar over Earth's surface weighs 5.20 × 10¹⁸ kg.
    let bar = 4.0 * core::f64::consts::PI * math::powi(radius.value(), 2) * 1e5
        / (GRAVITATIONAL_CONSTANT * mass.value() / math::powi(radius.value(), 2));
    eprintln!(
        "six oceans at Venus's flux, η {eta:.3}: {:.0} bar of O2 kept, {:.0} with a 31.6 Myr melt, \
         an ocean lost in {:.1} Myr",
        none / bar,
        molten / bar,
        OCEAN / water_loss_rate(mass, radius, xuv) / 31_557_600.0 / 1e6,
    );
    // A melt that outlasts the whole loss keeps none.
    assert_same_bits(kept(water, 1e9, 4.57e9), 0.0);
    assert_same_bits(kept(water, 1e9, 1e9), 0.0);
    // The oxygen kept is non-increasing and continuous in the melt's duration: a step of 1 Myr
    // takes at most that step's share of the loss.
    let step = water_loss_rate(mass, radius, xuv) * 1e6 * 31_557_600.0 * 31.998 / (2.0 * 18.015)
        * (1.0 - eta);
    let mut previous = kept(water, 0.0, 1e9);
    for i in 1..=300 {
        let now = kept(water, 1e6 * f64::from(i), 1e9);
        assert!(now <= previous, "the oxygen rises at a {i} Myr melt");
        assert!(
            previous - now <= step * (1.0 + 1e-9),
            "a jump at a {i} Myr melt"
        );
        previous = now;
    }
    assert_same_bits(previous, 0.0);
    // A rate that is not a number takes nothing into the melt.
    assert_same_bits(
        runaway_oxygen(
            Kilograms::new(water),
            mass,
            radius,
            WattsPerSquareMetre::new(f64::NAN),
            Years::new(melt),
            Years::new(1e9),
        )
        .value(),
        runaway_oxygen(
            Kilograms::new(water),
            mass,
            radius,
            WattsPerSquareMetre::new(f64::NAN),
            Years::ZERO,
            Years::new(1e9),
        )
        .value(),
    );
}

/// The wet runaway world of `derive_body`'s golden: an Earth mass at 0.6 au with water rank 0.99,
/// whose young magma ocean, if any, lasted `melt` years.
fn wet_runaway(melt: Option<f64>) -> PlacedBody {
    let wet = VolatileDraws {
        water: UnitUniform::new(0.99).unwrap(),
        ..VolatileDraws::MEDIAN
    };
    let o = orbit(0.6, 0.0);
    let placed = PlacedBody::new(
        EarthMasses::new(1.0),
        o,
        o.semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap()
    .with_volatiles(wet);
    match melt {
        Some(melt) => placed.with_magma_ocean_until(Years::new(melt)),
        None => placed,
    }
}

/// The young melt of [`a_runaway_world_with_a_young_melt_keeps_less_oxygen`], yr: 31.6 Myr, the
/// geometric middle of P14.T28.a's 10–100 Myr.
const YOUNG_MELT: f64 = 3.16e7;

#[test]
fn a_runaway_world_with_a_young_melt_keeps_less_oxygen() {
    // Science-p14-t24f §3, through the derivation: the wet runaway world of `derive_body`'s golden,
    // about the Sun with its history, whose young magma ocean lasted 31.6 Myr (P14.T28.a's
    // `molten_until`), keeps less O₂ than the same world with none, by the melt's share, by hand.
    let disc = solar_disc();
    let melt = YOUNG_MELT;
    let none = derive(&wet_runaway(None), &disc, SOLAR_AGE);
    let molten = derive(&wet_runaway(Some(melt)), &disc, SOLAR_AGE);
    for body in [&none, &molten] {
        assert_eq!(body.atmosphere().state(), SurfaceState::RunawayGreenhouse);
    }
    let (o2, o2_molten) = (
        none.atmosphere()
            .partial_pressures()
            .of(Gas::Oxygen)
            .value(),
        molten
            .atmosphere()
            .partial_pressures()
            .of(Gas::Oxygen)
            .value(),
    );
    assert!(
        o2_molten > 0.0 && o2_molten < o2,
        "{o2_molten} Pa against {o2}"
    );
    let (mass, radius) = (Kilograms::from(none.mass()), Metres::from(none.radius()));
    let xuv = historic_sun()
        .xuv()
        .saturated_flux(Metres::new(0.6 * METRES_PER_AU), 0.0);
    let eta = oxygen_escape_parameter(mass, radius, xuv);
    let weight = GRAVITATIONAL_CONSTANT * mass.value()
        / math::powi(radius.value(), 2)
        / (4.0 * core::f64::consts::PI * math::powi(radius.value(), 2));
    let difference = water_loss_rate(mass, radius, xuv) * melt * 31_557_600.0 * 31.998
        / (2.0 * 18.015)
        * (1.0 - eta)
        * weight;
    assert!(
        ((o2 - o2_molten) / difference - 1.0).abs() < 1e-9,
        "{o2} − {o2_molten} Pa against {difference}"
    );
    eprintln!(
        "the wet runaway world: {:.0} bar of O2, {:.0} with a 31.6 Myr melt",
        o2 / 1e5,
        o2_molten / 1e5
    );
    // Nothing else of its air moves.
    for gas in Gas::ALL.into_iter().filter(|&gas| gas != Gas::Oxygen) {
        assert_eq!(
            none.atmosphere().partial_pressures().of(gas),
            molten.atmosphere().partial_pressures().of(gas),
            "{gas:?}"
        );
    }
}

/// A Titan-like cold moon formed on the side `formed` of the snow line, with nitrogen and the
/// carbon `carbon` (kg, counted as carbon dioxide) in its inventory, in equilibrium at
/// `equilibrium` (K), its crust `crust`.
fn cold_moon(carbon: f64, equilibrium: f64, formed: SnowLineSide, crust: Crust) -> Atmosphere {
    atmosphere(&AtmosphereInputs {
        mass: Kilograms::new(1.345e23),
        radius: Metres::new(2.575e6),
        material: SurfaceMaterial::Ice,
        envelope_fraction: 0.0,
        inventory: VolatileInventory::new(
            Kilograms::new(1e22),
            Kilograms::new(carbon),
            Kilograms::new(9e18),
            Kilograms::ZERO,
        ),
        formed,
        equilibrium: Kelvin::new(equilibrium),
        worst_equilibrium: Kelvin::new(82.0),
        heated: Kelvin::new(equilibrium),
        xuv_fluence: JoulesPerSquareMetre::ZERO,
        saturated_xuv_flux: WattsPerSquareMetre::ZERO,
        insolation: Insolation::BeyondMaximumGreenhouse,
        crust,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
    })
}

#[test]
fn methane_is_held_at_its_humidity_only_over_a_reservoir() {
    // A large carbon inventory leaves a reservoir on the surface and the air at
    // METHANE_RELATIVE_HUMIDITY of saturation; a small one is all airborne, subsaturated.
    let rich = cold_moon(1e21, 75.6, SnowLineSide::Beyond, Crust::Solid);
    let t = rich.surface_temperature();
    let saturated = saturation_pressure(Gas::Methane.substance(), t)
        .unwrap()
        .value();
    assert_eq!(rich.carbon_carrier(), Some(CarbonCarrier::Methane));
    // The humidity caps its partial pressure in the air, not its column weight (signoff-3.md,
    // item 1.7): in nitrogen the partial pressure is the larger, by μ̄ ÷ μ_CH₄.
    let in_the_air = methane_in_the_air(&rich);
    assert!(
        (in_the_air / (METHANE_RELATIVE_HUMIDITY * saturated) - 1.0).abs() < 1e-12,
        "{in_the_air} Pa against {saturated} Pa saturated"
    );
    assert!(rich.partial_pressures().of(Gas::Methane).value() < in_the_air);
    let poor = cold_moon(1e15, 75.6, SnowLineSide::Beyond, Crust::Solid);
    let weight = GRAVITATIONAL_CONSTANT * 1.345e23
        / math::powi(2.575e6, 2)
        / (4.0 * core::f64::consts::PI * math::powi(2.575e6, 2));
    let methane = 1e15 * 16.043 / 44.009 * weight;
    let p = poor.partial_pressures().of(Gas::Methane).value();
    assert!(
        (p / methane - 1.0).abs() < 1e-12,
        "{p} Pa against {methane}"
    );
    assert!(methane_in_the_air(&poor) < METHANE_RELATIVE_HUMIDITY * saturated);
    // Formed inside the snow line, the same moon keeps its carbon as frozen carbon dioxide.
    let inner = cold_moon(1e21, 75.6, SnowLineSide::Inside, Crust::Solid);
    assert_eq!(inner.carbon_carrier(), Some(CarbonCarrier::CarbonDioxide));
    assert_eq!(inner.partial_pressures().of(Gas::Methane), Pascals::ZERO);
    // A young magma ocean holds its carbon as carbon dioxide, however cold its greenhouse.
    let molten = cold_moon(1e21, 75.6, SnowLineSide::Beyond, Crust::Molten);
    assert_eq!(molten.state(), SurfaceState::MagmaOcean);
    assert_eq!(molten.carbon_carrier(), Some(CarbonCarrier::CarbonDioxide));
    assert_eq!(molten.partial_pressures().of(Gas::Methane), Pascals::ZERO);
}

#[test]
fn the_humidity_caps_methane_s_partial_pressure_in_the_air() {
    // Signoff-3.md, item 1.7: the weight w that meets the cap solves w² + b w − c = 0, and its
    // partial pressure in the air, x p_s, is then the cap to 10⁻¹², in both of the root's forms.
    let air_of = |nitrogen: f64, argon: f64| {
        let mut p = [Pascals::ZERO; 9];
        p[Gas::Nitrogen.index()] = Pascals::new(nitrogen);
        p[Gas::Argon.index()] = Pascals::new(argon);
        p
    };
    // The cap's weight, Pa, among `others`, with an inventory of `inventory` Pa and a cap of `cap`.
    let capped = |inventory: f64, cap: f64, others: &[Pascals; 9]| {
        capped_methane_weight(Pascals::new(inventory), Pascals::new(cap), others).value()
    };
    let in_the_air = |w: f64, others: &[Pascals; 9]| {
        let moles = |gas: Gas, p: f64| p / gas.molar_mass_g_per_mol();
        let (mut total, mut all) = (w, moles(Gas::Methane, w));
        for gas in Gas::ALL {
            total += others[gas.index()].value();
            all += moles(gas, others[gas.index()].value());
        }
        moles(Gas::Methane, w) / all * total
    };
    for (nitrogen, argon, cap) in [
        (1.4e5, 0.0, 1.0e4),
        (1.4e5, 2.0e3, 8.0e3),
        (1.0e3, 0.0, 1.0e4),
        (5.0e2, 1.0e2, 3.0e5),
    ] {
        let others = air_of(nitrogen, argon);
        let w = capped(1e12, cap, &others);
        assert!(w > 0.0 && w < 1e12);
        let x_p = in_the_air(w, &others);
        assert!(
            (x_p / cap - 1.0).abs() < 1e-12,
            "{nitrogen} Pa of N2: {x_p} Pa against {cap}"
        );
        // In nitrogen the weight is below the cap, by μ_CH₄ ÷ μ̄.
        assert!(w < cap);
        // An inventory below the root is all airborne.
        assert_same_bits(capped(0.5 * w, cap, &others), 0.5 * w);
    }
    // Alone, methane's weight is its partial pressure.
    let none = air_of(0.0, 0.0);
    assert!((capped(1e12, 1e4, &none) / 1e4 - 1.0).abs() < 1e-15);
    // No cap above the critical point, and no methane where the cap is none.
    let nitrogen = air_of(1.4e5, 0.0);
    assert_same_bits(capped(7.0, f64::INFINITY, &nitrogen), 7.0);
    assert_same_bits(capped(7.0, 0.0, &nitrogen), 0.0);
    assert_same_bits(capped(7.0, f64::NAN, &nitrogen), 0.0);
    assert_same_bits(capped(7.0, 0.0, &none), 0.0);
    // The weight rises continuously with the cap, through the change of the root's form at
    // b = 0, where the cap equals the other gases' weight.
    let mut previous = 0.0;
    for i in 1..=4_000 {
        let cap = 1e2 * f64::from(i);
        let w = capped(1e12, cap, &nitrogen);
        assert!(w > previous, "at a cap of {cap} Pa");
        assert!(
            w - previous <= 1e2 * (1.0 + 1e-9),
            "a jump at a cap of {cap} Pa"
        );
        previous = w;
    }
}

#[test]
fn a_gas_envelope_and_a_body_of_no_mass_have_no_carbon_carrier() {
    let inputs = |envelope_fraction: f64, mass: Kilograms| AtmosphereInputs {
        mass,
        radius: Metres::new(2.5e7),
        material: SurfaceMaterial::Ice,
        envelope_fraction,
        inventory: VolatileInventory::new(
            Kilograms::new(1e22),
            Kilograms::new(1e21),
            Kilograms::new(1e20),
            Kilograms::ZERO,
        ),
        formed: SnowLineSide::Beyond,
        equilibrium: Kelvin::new(60.0),
        worst_equilibrium: Kelvin::new(60.0),
        heated: Kelvin::new(60.0),
        xuv_fluence: JoulesPerSquareMetre::ZERO,
        saturated_xuv_flux: WattsPerSquareMetre::ZERO,
        insolation: Insolation::BeyondMaximumGreenhouse,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
    };
    let envelope = atmosphere(&inputs(0.1, Kilograms::new(15.0 * EARTH_MASS_KG)));
    assert_eq!(envelope.state(), SurfaceState::GasEnvelope);
    assert_eq!(envelope.carbon_carrier(), None);
    let nothing = atmosphere(&inputs(0.0, Kilograms::ZERO));
    assert_eq!(nothing.state(), SurfaceState::Airless);
    assert_eq!(nothing.carbon_carrier(), None);
}

#[test]
fn the_speciated_atmosphere_is_the_same_twice() {
    // The rules the bump puts in force are pure: a methane world and an oxygen world derived twice
    // are the same.
    let first = table();
    let again = table();
    for ((name, a), (_, b)) in first.iter().zip(&again) {
        assert_eq!(a, b, "{name}");
    }
    let wet = runaway_earth(1.4e22, 5e8, 1.0, 255.0);
    let wet_again = runaway_earth(1.4e22, 5e8, 1.0, 255.0);
    assert_eq!(wet, wet_again);
    // A runaway world whose young melt took some of its oxygen, derived twice.
    let disc = solar_disc();
    let molten = || *derive(&wet_runaway(Some(YOUNG_MELT)), &disc, SOLAR_AGE).atmosphere();
    let (melt, melt_again) = (molten(), molten());
    assert!(melt.partial_pressures().of(Gas::Oxygen) > Pascals::ZERO);
    assert_eq!(melt, melt_again);
    for gas in Gas::ALL {
        assert_same_bits(
            wet.partial_pressures().of(gas).value(),
            wet_again.partial_pressures().of(gas).value(),
        );
    }
}

/// Whether a body about the Sun with its history, receiving the flux `flux`, lies beyond the
/// maximum-greenhouse limit (Kopparapu et al. 2013), as the derivation's `Sky` judges it.
fn beyond_the_maximum_greenhouse(flux: EarthFluxes) -> bool {
    let limit = crate::planetary::derive::habitable_zone::HabitableLimit::MaximumGreenhouse.flux(
        Kelvin::new(crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K),
    );
    flux.value() / limit.value() < 1.0
}

/// An Earth mass formed at 5 au of the Sun, beyond its snow line, now at `a_au`.
fn migrated_from_beyond_the_snow_line(a_au: f64) -> PlacedBody {
    PlacedBody::new(
        EarthMasses::new(1.0),
        orbit(a_au, 0.0),
        Metres::new(5.0 * METRES_PER_AU),
        UnitUniform::HALF,
    )
    .unwrap()
}

#[test]
fn the_carbon_changes_carrier_only_where_its_insolation_or_crust_does() {
    // Science-p14-t24f §5: the carrier reads a recorded state, so it changes only where the climate
    // does. An Earth mass formed beyond the snow line, moved from 1.3 to 2.3 au of the Sun at steps
    // of 0.1%, carries methane exactly where it lies beyond the maximum-greenhouse limit; its
    // gases move continuously but where the carrier and the state change together.
    let disc = solar_disc();
    let mut previous: Option<(bool, Atmosphere)> = None;
    let (mut changes, mut a) = (0, 1.3);
    while a < 2.3 {
        let body = derive(&migrated_from_beyond_the_snow_line(a), &disc, SOLAR_AGE);
        assert_eq!(body.formed(), SnowLineSide::Beyond);
        let beyond = beyond_the_maximum_greenhouse(body.flux());
        let air = *body.atmosphere();
        let methane = air.carbon_carrier() == Some(CarbonCarrier::Methane);
        assert_eq!(methane, beyond, "{a} au");
        assert_eq!(
            air.state(),
            if beyond {
                SurfaceState::Snowball
            } else {
                SurfaceState::Temperate
            },
            "{a} au"
        );
        if let Some((was_beyond, before)) = previous {
            if before.carbon_carrier() == air.carbon_carrier() {
                assert_eq!(was_beyond, beyond, "{a} au");
                let total = air.partial_pressures().total().value();
                for gas in Gas::ALL {
                    let jump = (air.partial_pressures().of(gas).value()
                        - before.partial_pressures().of(gas).value())
                    .abs();
                    assert!(jump < 1e-2 * total, "{gas:?} jumps by {jump} Pa at {a} au");
                }
            } else {
                assert_ne!(was_beyond, beyond, "{a} au");
                assert_ne!(before.state(), air.state(), "{a} au");
                changes += 1;
            }
        }
        previous = Some((beyond, air));
        a *= 1.001;
    }
    assert_eq!(changes, 1, "the carrier changes once, at the limit");
    // As its young magma ocean freezes, the carrier changes with the crust: carbon dioxide while
    // it is molten, methane once it has formed.
    let placed = migrated_from_beyond_the_snow_line(2.0).with_magma_ocean_until(Years::new(1e8));
    let mut age = 5e7;
    while age < 2e8 {
        let air = *derive(&placed, &disc, Years::new(age)).atmosphere();
        let molten = age < 1e8;
        assert_eq!(air.state() == SurfaceState::MagmaOcean, molten, "{age} yr");
        assert_eq!(
            air.carbon_carrier(),
            Some(if molten {
                CarbonCarrier::CarbonDioxide
            } else {
                CarbonCarrier::Methane
            }),
            "{age} yr"
        );
        age *= 1.01;
    }
}

#[test]
fn no_body_inside_the_snow_line_or_the_maximum_greenhouse_limit_takes_methane() {
    // Bodies from 0.05 to 39.7 au of a Sun, from a Ceres to 5 M⊕, formed where they lie, under
    // P14.T24.f's rules: no methane inside the snow line or inside the maximum-greenhouse limit,
    // and methane the carrier of every solid body formed beyond the snow line and beyond the limit
    // (science-p14-t24f §5). Methane's retention is nitrogen's (§4); every body's fractions sum to
    // 1 to 10⁻⁹.
    let disc = solar_disc();
    let mut methane_worlds = 0;
    for i in 0..30 {
        let a = 0.05 * math::exp10(f64::from(i) / 10.0);
        for m in [1e-3, 0.02, 0.1, 1.0, 5.0] {
            for rank in [0.1, 0.5, 0.9] {
                let o = orbit(a, 0.0);
                let placed = PlacedBody::new(
                    EarthMasses::new(m),
                    o,
                    o.semi_major_axis(),
                    UnitUniform::new(rank).unwrap(),
                )
                .unwrap();
                let body = derive(&placed, &disc, SOLAR_AGE);
                let air = body.atmosphere();
                let methane = air.partial_pressures().of(Gas::Methane);
                let beyond = beyond_the_maximum_greenhouse(body.flux());
                if air.state() == SurfaceState::GasEnvelope {
                    assert_eq!(air.carbon_carrier(), None, "{m} M⊕ at {a} au");
                } else if body.formed() == SnowLineSide::Beyond && beyond {
                    assert_eq!(
                        air.carbon_carrier(),
                        Some(CarbonCarrier::Methane),
                        "{m} M⊕ at {a} au"
                    );
                    assert_eq!(
                        air.partial_pressures().of(Gas::CarbonDioxide),
                        Pascals::ZERO,
                        "{m} M⊕ at {a} au"
                    );
                } else {
                    assert_eq!(methane, Pascals::ZERO, "{m} M⊕ at {a} au");
                    assert_eq!(
                        air.carbon_carrier(),
                        Some(CarbonCarrier::CarbonDioxide),
                        "{m} M⊕ at {a} au"
                    );
                }
                assert_eq!(
                    air.retention().retains(Gas::Methane),
                    air.retention().retains(Gas::Nitrogen),
                    "{m} M⊕ at {a} au"
                );
                if methane > Pascals::ZERO && air.keeps_atmosphere() {
                    methane_worlds += 1;
                }
                if let Some(total) = air.surface_pressure().filter(|p| p.value() > 0.0) {
                    let sum = Gas::ALL.iter().fold(0.0, |sum, &gas| {
                        sum + air.partial_pressures().of(gas).value() / total.value()
                    });
                    assert!((sum - 1.0).abs() < 1e-9, "{m} M⊕ at {a} au: {sum}");
                }
            }
        }
    }
    eprintln!("{methane_worlds} methane worlds with air");
    assert!(methane_worlds > 0);
}

#[test]
fn icy_worlds_beyond_the_maximum_greenhouse_hold_no_carbon_dioxide_greenhouse() {
    // Science-p14-t24f §5: `derive_body`'s golden `icy` and `beyond_dry` bodies carry methane, and
    // only their nitrogen warms them, as on Titan: each surface lies within 2% of the
    // nitrogen-only grey temperature by hand, frozen. Under P14.T13.c's rules, before the 21 → 22
    // batch's bump, they were carbon-dioxide greenhouses of 580 and 706 K, which no carbon-dioxide
    // air can be beyond the limit (Kopparapu et al. 2013).
    let disc = solar_disc();
    for (name, placed, kelvin) in [
        ("icy", golden_placed(EARTH_MASS_KG, 5.0, 0.05, 0.9), 157.0),
        ("beyond_dry", beyond_the_snow_line(&disc, true), 180.0),
        ("beyond_watery", beyond_the_snow_line(&disc, false), 175.0),
    ] {
        // As `derive_body`'s golden derives its bodies: about the present Sun with no X-ray
        // history.
        let body = golden_derive(&placed, &disc).unwrap();
        assert_eq!(body.formed(), SnowLineSide::Beyond, "{name}");
        let air = body.atmosphere();
        assert_eq!(air.state(), SurfaceState::Snowball, "{name}");
        assert_eq!(air.carbon_carrier(), Some(CarbonCarrier::Methane), "{name}");
        assert_eq!(
            air.partial_pressures().of(Gas::CarbonDioxide),
            Pascals::ZERO,
            "{name}"
        );
        assert!(air.retention().retains(Gas::Nitrogen), "{name}");
        // The nitrogen-only grey temperature, by hand.
        let (mass, r) = (
            Kilograms::from(body.mass()).value(),
            Metres::from(body.radius()).value(),
        );
        let weight =
            GRAVITATIONAL_CONSTANT * mass / (r * r) / (4.0 * core::f64::consts::PI * r * r);
        let nitrogen = body.inventory().nitrogen().value() * weight;
        let t_eq = body.equilibrium_temperature().value();
        let tau = 1.55 * (75.6 / t_eq) * (75.6 / t_eq) * (nitrogen / 1e5).sqrt();
        let by_hand = t_eq * math::powf(1.0 + 0.75 * tau, 0.25);
        let t = air.surface_temperature().value();
        eprintln!(
            "{name}: {t:.1} K (by hand {by_hand:.1} K), N2 {:.1} bar, CH4 {:.2} bar, \
             {:.1} bar in all, T_eq {t_eq:.1} K",
            air.partial_pressures().of(Gas::Nitrogen).value() / 1e5,
            air.partial_pressures().of(Gas::Methane).value() / 1e5,
            air.partial_pressures().total().value() / 1e5,
        );
        assert!(
            ((t - by_hand) / by_hand).abs() < 0.02,
            "{name}: {t} K against {by_hand}"
        );
        assert!(
            ((t - kelvin) / kelvin).abs() < 0.05,
            "{name}: {t} K against {kelvin}"
        );
        assert!(t < 273.16, "{name}: {t} K");
    }
}

#[test]
fn every_table_body_s_fractions_sum_to_one() {
    for (name, body) in table() {
        let air = body.atmosphere();
        let Some(total) = air.surface_pressure().filter(|p| p.value() > 0.0) else {
            continue;
        };
        let sum = Gas::ALL.iter().fold(0.0, |sum, &gas| {
            sum + air.partial_pressures().of(gas).value() / total.value()
        });
        assert!((sum - 1.0).abs() < 1e-9, "{name}: {sum}");
    }
}

#[test]
fn the_gases_are_continuous_in_time_but_at_a_recorded_state_change() {
    // P14.T24.f: Titan's methane and a water-rich runaway world's oxygen, at steps of 0.1% in age
    // over 0.2–10 Gyr: no partial pressure jumps by 10⁻³ of the surface pressure but where the
    // state, the retained gases or the carbon's carrier change.
    let disc = solar_disc();
    let (_, kg, km, a, e) = SMALL_BODIES[2];
    let titan = placed_about_the_sun(&disc, (kg, km, a, e));
    let wet = VolatileDraws {
        water: UnitUniform::new(0.99).unwrap(),
        ..VolatileDraws::MEDIAN
    };
    let runaway = {
        let o = orbit(0.6, 0.0);
        PlacedBody::new(
            EarthMasses::new(2.0),
            o,
            o.semi_major_axis(),
            UnitUniform::HALF,
        )
        .unwrap()
        .with_volatiles(wet)
    };
    let mut oxygen_seen = false;
    for placed in [titan, runaway] {
        let mut previous: Option<Atmosphere> = None;
        let mut age = 2e8;
        while age < 1e10 {
            let air = *derive(&placed, &disc, Years::new(age)).atmosphere();
            oxygen_seen |= air.partial_pressures().of(Gas::Oxygen) > Pascals::ZERO;
            if let Some(before) = previous {
                let recorded = before.state() != air.state()
                    || before.retention() != air.retention()
                    || before.carbon_carrier() != air.carbon_carrier();
                if !recorded {
                    let total = air.partial_pressures().total().value();
                    for gas in Gas::ALL {
                        let jump = (air.partial_pressures().of(gas).value()
                            - before.partial_pressures().of(gas).value())
                        .abs();
                        assert!(
                            jump < 1e-3 * total,
                            "{gas:?} jumps by {jump} Pa at {age} yr"
                        );
                    }
                }
            }
            previous = Some(air);
            age *= 1.001;
        }
    }
    assert!(oxygen_seen, "the runaway world keeps oxygen for a while");
}

/// P14.T24.f's figures, in force from the 21 → 22 bump, pinned bit for bit: the table's bodies'
/// gases and carriers about the Sun with its X-ray history, a runaway Earth's oxygen, a cold moon's
/// methane, and the closed forms at a few inputs, a young melt's among them.
#[test]
fn the_speciated_figures_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    let gases = |w: &mut GoldenWriter, name: &str, air: &Atmosphere| {
        w.line(&format!("{name}_surface_state = {:?}", air.state()));
        w.f64(
            &format!("{name}_surface_temperature"),
            air.surface_temperature().value(),
        );
        for gas in Gas::ALL {
            let gas_name = format!("{gas:?}").to_lowercase();
            w.f64(
                &format!("{name}_partial_pressure_{gas_name}"),
                air.partial_pressures().of(gas).value(),
            );
        }
        w.line(&format!(
            "{name}_carbon_carrier = {:?}",
            air.carbon_carrier()
        ));
    };
    for (name, body) in table() {
        gases(&mut w, &name.to_lowercase(), body.atmosphere());
    }
    for (name, age, xuv) in [
        ("runaway_young", 5e8, 1.0),
        ("runaway_bright", 5e8, 10.0),
        ("runaway_old", 1.5e9, 1.0),
    ] {
        let air = runaway_earth(1.4e22, age, xuv, 255.0);
        gases(&mut w, name, &air);
    }
    for (name, carbon) in [("cold_moon_rich", 1e21), ("cold_moon_poor", 1e15)] {
        let air = cold_moon(carbon, 75.6, SnowLineSide::Beyond, Crust::Solid);
        gases(&mut w, name, &air);
    }
    // Science-p14-t24f §3: the wet runaway world about the Sun with its history, with no young
    // melt and with one of 31.6 Myr, which took some of its oxygen.
    let disc = solar_disc();
    for (name, melt) in [
        ("wet_runaway_historic", None),
        ("wet_runaway_historic_young_melt", Some(YOUNG_MELT)),
    ] {
        let body = derive(&wet_runaway(melt), &disc, SOLAR_AGE);
        gases(&mut w, name, body.atmosphere());
    }
    let (mass, radius) = (
        Kilograms::new(EARTH_MASS_KG),
        Metres::new(crate::units::consts::EARTH_RADIUS_M),
    );
    for flux in [0.1, 0.19, 1.0, 10.0, 100.0] {
        w.f64(
            &format!("earth_oxygen_escape_parameter_at_{flux}"),
            oxygen_escape_parameter(mass, radius, WattsPerSquareMetre::new(flux)),
        );
    }
    for age in [1e8, 1e9, 1.5e9] {
        w.f64(
            &format!("earth_runaway_oxygen_of_an_ocean_at_{age:e}"),
            runaway_oxygen(
                Kilograms::new(1.4e21),
                mass,
                radius,
                WattsPerSquareMetre::new(1.0),
                Years::ZERO,
                Years::new(age),
            )
            .value(),
        );
    }
    // Science-p14-t24f §3: six oceans lost at Venus's saturated flux, with a young melt.
    for melt in [1e7, 3.16e7, 1e8] {
        w.f64(
            &format!("earth_runaway_oxygen_of_six_oceans_at_venus_with_a_melt_of_{melt:e}"),
            runaway_oxygen(
                Kilograms::new(6.0 * OCEAN),
                mass,
                radius,
                venus_saturated_flux(),
                Years::new(melt),
                SOLAR_AGE,
            )
            .value(),
        );
    }
    let sun = historic_sun();
    w.f64(
        "sun_saturated_flux_at_1_au",
        sun.xuv()
            .saturated_flux(Metres::new(METRES_PER_AU), 0.0)
            .value(),
    );
    golden!("planetary/atmosphere_speciation", w.as_str());
}
