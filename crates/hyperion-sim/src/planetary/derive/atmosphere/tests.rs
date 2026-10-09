use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::composition::FORMATION_ENVELOPE_FLOOR;
use crate::planetary::derive::solar::{
    PLANETS, SOLAR_AGE, historic_sun, orbit, rank_for, solar_disc,
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

/// The moons and the dwarf planet of the table, each placed about the Sun at its parent's
/// distance, where its light comes from: name, mass (kg), mean radius (km), semi-major axis (au)
/// and eccentricity (NASA's fact sheets).
const SMALL_BODIES: [(&str, f64, f64, f64, f64); 4] = [
    ("Moon", 7.342e22, 1_737.4, 1.000_001, 0.016_709),
    ("Ganymede", 1.481_9e23, 2_634.1, 5.203_8, 0.048_9),
    ("Titan", 1.345_2e23, 2_574.7, 9.582_6, 0.056_5),
    ("Ceres", 9.39e20, 469.7, 2.767_5, 0.075_8),
];

/// A body of `kg` at `a_au` and `e` about the present Sun with its history, at the rank that
/// keeps its radius `km`, derived at `age`.
fn about_the_sun(
    disc: &DiscProfile,
    (kg, km, a, e): (f64, f64, f64, f64),
    age: Years,
) -> DerivedBody {
    let rank = rank_for(disc, kg, km, a, e);
    assert!(
        rank > 0.0 && rank < 1.0,
        "a radius of {km} km is inside its window"
    );
    let orbit = orbit(a, e);
    let placed = PlacedBody::new(
        EarthMasses::new(kg / EARTH_MASS_KG),
        orbit,
        orbit.semi_major_axis(),
        UnitUniform::new(rank).unwrap(),
    )
    .unwrap();
    derive(&placed, disc, age)
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
    PLANETS
        .iter()
        .filter(|(name, ..)| !matches!(*name, "Jupiter" | "Saturn" | "Uranus" | "Neptune"))
        .chain(SMALL_BODIES.iter())
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
        equilibrium: Kelvin::new(1_500.0),
        worst_equilibrium: Kelvin::new(1_500.0),
        heated: Kelvin::new(1_500.0),
        xuv_fluence: JoulesPerSquareMetre::ZERO,
        insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
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

#[test]
fn the_saturation_pressures_meet_their_triple_points() {
    for c in [
        &WATER_CONDENSATION,
        &CARBON_DIOXIDE_CONDENSATION,
        &NITROGEN_CONDENSATION,
        &ARGON_CONDENSATION,
    ] {
        let t = c.triple_temperature;
        let below = c.saturation_pressure(t * (1.0 - 1e-9));
        let above = c.saturation_pressure(t);
        assert!((below / c.triple_pressure - 1.0).abs() < 1e-6);
        assert!((above / c.triple_pressure - 1.0).abs() < 1e-12);
        assert!(c.saturation_pressure(c.critical_temperature).is_infinite());
        assert_same_bits(c.saturation_pressure(0.0), 0.0);
    }
    // Water boils near 373 K at one atmosphere, and carbon dioxide sublimes near 195 K.
    assert!((WATER_CONDENSATION.saturation_pressure(373.15) / 101_325.0 - 1.0).abs() < 0.1);
    assert!(
        (CARBON_DIOXIDE_CONDENSATION.saturation_pressure(194.7) / 101_325.0 - 1.0).abs() < 0.15
    );
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
