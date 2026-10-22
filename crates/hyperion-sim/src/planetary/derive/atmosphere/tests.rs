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
use crate::substance::saturation_pressure;
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
    // Formed at 1 au and at 5 au of the zero-age Sun: inside its snow line and beyond its
    // ammonia line, so that neither lies between the two lines, where P14.T50.c's ammonia line
    // grades the nitrogen.
    let formed_at = |side| {
        let snow = solar_disc().snow_line();
        let au = match side {
            SnowLineSide::Inside => 1.0,
            SnowLineSide::Beyond => 5.0,
        };
        formation_temperature(Metres::new(au * METRES_PER_AU), snow)
    };
    let at = |side, draws: &VolatileDraws| {
        volatile_inventory(
            EarthMasses::new(1.0),
            &fractions,
            side,
            formed_at(side),
            draws,
            SOLAR_AGE,
        )
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
        formed_at(SnowLineSide::Beyond),
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
        formed_at(SnowLineSide::Inside),
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
    // Ceres do not. The table's Ganymede, a body at 5.2 au, is airless under the frost's albedo
    // because its exobase reads the irradiation, not the surface (decision-p14-t54a-ganymede §1.4).
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
    // P14.T13.c: Venus 737 K (decision-p14-t57a-fits §3), Earth 288 K, Mars 215 K, Titan 94 K.
    // Venus is taken at its measured inventory, given by hand (P14.T57.a): the table's Venus is a
    // median draw. The measured one is 737 K (778 K under version 22's constants).
    let table = table();
    let measured = measured_venus();
    for (name, kelvin) in [("Venus", 737.0), ("Earth", 288.0), ("Titan", 94.0)] {
        let body = if name == "Venus" {
            &measured
        } else {
            found(&table, name)
        };
        let t = body.surface_temperature().value();
        assert!(
            ((t - kelvin) / kelvin).abs() < 0.08,
            "{name} at {t} K against {kelvin}"
        );
    }
    // Mars at the passes' stand-ins ices over, all its condensed water at its surface, at a Bond
    // albedo near 0.5 (decision-p14-t51-batch E3): its 196.6 K lies 8.6% under 215 K. Printed until
    // P14.T54.a's Batch C gives the passes its water share, where T13.c's 8% is asserted again.
    let mars = found(&table, "Mars");
    eprintln!(
        "Mars at the passes' stand-ins: {:.3} K at a Bond albedo of {:.5}, against 215 K",
        mars.surface_temperature().value(),
        mars.albedo().value()
    );
    // The albedo each takes is its area mix's (P14.T51.b), and the equilibrium temperature is at
    // it: Earth's within 0.02 of 0.294, which gives 255 K, at 0.67 of cloud and with no sea until
    // P14.T54.a's Batch C gives the passes basins.
    let earth = found(&table, "Earth");
    assert!(
        (earth.albedo().value() - 0.294).abs() < 0.02,
        "{:?}",
        earth.albedo()
    );
    let earth_eq = earth.equilibrium_temperature().value();
    assert!((earth_eq - 255.0).abs() < 2.0, "Earth's T_eq {earth_eq} K");
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
    // The passes and the 32 greenhouse steps settle: one more step moves no surface temperature of
    // the table by a part in a billion, and the albedo the published pass read is the one its
    // atmosphere gives, to the passes' convergence: T_eq ∝ (1 − A)^¼, so a step of
    // `ATMOSPHERE_CONVERGENCE` (10⁻⁹) of the equilibrium temperature is one of at most 4 × 10⁻⁹ in
    // the albedo.
    let bound = 4.0 * crate::planetary::derive::ATMOSPHERE_CONVERGENCE;
    for (name, body) in table() {
        let air = body.atmosphere();
        let (read, gave) = (body.albedo().value(), air.albedo().value());
        assert!(
            (read - gave).abs() <= bound,
            "{name}: read {read}, gave {gave}"
        );
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
        class: PlanetClass::Rocky,
        inventory: VolatileInventory::new(
            Kilograms::ZERO,
            Kilograms::new(1e24),
            Kilograms::ZERO,
            Kilograms::ZERO,
        ),
        formed: SnowLineSide::Inside,
        equilibrium: Kelvin::new(1_500.0),
        irradiation_temperature: Kelvin::new(1_500.0),
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
    // Its albedo is class I's of Sudarsky, Burrows and Pinto about a Sun under its hazes, 0.48
    // (P14.T51.b's area mix, decision-p14-t54a-ganymede §2; T13.c's table gave 0.34), its
    // equilibrium temperature that albedo's (about 103.6 K, where its measured 0.503 ± 0.012, Li et
    // al. 2018, Nat. Commun. 9, 3709, gives 102.4 K, and Hanel et al. 1981's superseded 0.343 gives
    // 110 K), and its surface the temperature it radiates at.
    assert!(
        (jupiter.albedo().value() - 0.48).abs() < 0.05,
        "{:?}",
        jupiter.albedo()
    );
    assert_same_bits(
        jupiter.equilibrium_temperature().value(),
        equilibrium_temperature(jupiter.flux(), jupiter.albedo()).value(),
    );
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
    // The exobase takes the irradiation at the worst and the X-rays beyond Earth's.
    let earth = earth_xuv_fluence();
    assert!((earth.value() / 2.57e15 - 1.0).abs() < 0.01, "{earth:?}");
    assert_same_bits(
        exobase_temperature(Kelvin::new(250.0), earth).value(),
        EXOBASE_MULTIPLE * 250.0,
    );
    let quadruple = JoulesPerSquareMetre::new(4.0 * earth.value());
    assert!(
        (exobase_temperature(Kelvin::new(250.0), quadruple).value()
            / (2.0 * EXOBASE_MULTIPLE * 250.0)
            - 1.0)
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
/// ultraviolet flux `xuv` (W m⁻²), in equilibrium at `equilibrium` (K), which its exobase reads as
/// its irradiation.
fn runaway_earth(water: f64, age: f64, xuv: f64, equilibrium: f64) -> Atmosphere {
    atmosphere(&AtmosphereInputs {
        mass: Kilograms::new(EARTH_MASS_KG),
        radius: Metres::new(crate::units::consts::EARTH_RADIUS_M),
        material: SurfaceMaterial::Rock,
        envelope_fraction: 0.0,
        class: PlanetClass::Rocky,
        inventory: VolatileInventory::new(
            Kilograms::new(water),
            Kilograms::new(3.6e20),
            Kilograms::new(3.9e18),
            Kilograms::new(6.6e16),
        ),
        formed: SnowLineSide::Inside,
        equilibrium: Kelvin::new(equilibrium),
        irradiation_temperature: Kelvin::new(equilibrium),
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

/// Titan's irradiation temperature: the equilibrium temperature at zero albedo at Saturn's
/// 9.5826 au from the present Sun, 278.33 K ÷ √9.5826. The distance is an edition of NASA's Saturn
/// fact sheet before its update of 18 March 2025 (1,433.53 × 10⁶ km), which gives 1,432.041 ×
/// 10⁶ km, 9.5726 au; the value is kept.
const TITAN_IRRADIATION: Kelvin = Kelvin::new(89.91);

/// A Titan-like cold moon formed on the side `formed` of the snow line, with nitrogen and the
/// carbon `carbon` (kg, counted as carbon dioxide) in its inventory, in equilibrium at
/// `equilibrium` (K), its crust `crust`, its exobase at Titan's irradiation ([`TITAN_IRRADIATION`]).
fn cold_moon(carbon: f64, equilibrium: f64, formed: SnowLineSide, crust: Crust) -> Atmosphere {
    atmosphere(&AtmosphereInputs {
        mass: Kilograms::new(1.345e23),
        radius: Metres::new(2.575e6),
        material: SurfaceMaterial::Ice,
        envelope_fraction: 0.0,
        class: PlanetClass::Icy,
        inventory: VolatileInventory::new(
            Kilograms::new(1e22),
            Kilograms::new(carbon),
            Kilograms::new(9e18),
            Kilograms::ZERO,
        ),
        formed,
        equilibrium: Kelvin::new(equilibrium),
        irradiation_temperature: TITAN_IRRADIATION,
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
fn a_gas_envelope_and_a_body_of_no_mass_have_no_carbon_carrier() {
    let inputs = |envelope_fraction: f64, mass: Kilograms, class: PlanetClass| AtmosphereInputs {
        mass,
        radius: Metres::new(2.5e7),
        material: SurfaceMaterial::Ice,
        envelope_fraction,
        class,
        inventory: VolatileInventory::new(
            Kilograms::new(1e22),
            Kilograms::new(1e21),
            Kilograms::new(1e20),
            Kilograms::ZERO,
        ),
        formed: SnowLineSide::Beyond,
        equilibrium: Kelvin::new(60.0),
        irradiation_temperature: Kelvin::new(60.0),
        heated: Kelvin::new(60.0),
        xuv_fluence: JoulesPerSquareMetre::ZERO,
        saturated_xuv_flux: WattsPerSquareMetre::ZERO,
        insolation: Insolation::BeyondMaximumGreenhouse,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
    };
    let envelope = atmosphere(&inputs(
        0.1,
        Kilograms::new(15.0 * EARTH_MASS_KG),
        PlanetClass::IceGiant,
    ));
    assert_eq!(envelope.state(), SurfaceState::GasEnvelope);
    assert_eq!(envelope.carbon_carrier(), None);
    let nothing = atmosphere(&inputs(0.0, Kilograms::ZERO, PlanetClass::Icy));
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
    // air can be beyond the limit (Kopparapu et al. 2013). The two `beyond` bodies formed at 4 au,
    // between the zero-age Sun's snow line and its ammonia line (4.31 au), so their nitrogen is
    // Earth's per unit mass (P14.T50.c): 0.90 and 0.66 bar of N₂ hold them at 128.7 and 127.2 K,
    // where version 22's hundredfold nitrogen, 90 and 66 bar, held them at 180.1 and 174.5 K.
    let disc = solar_disc();
    for (name, placed, kelvin) in [
        ("icy", golden_placed(EARTH_MASS_KG, 5.0, 0.05, 0.9), 157.0),
        ("beyond_dry", beyond_the_snow_line(&disc, true), 128.7),
        ("beyond_watery", beyond_the_snow_line(&disc, false), 127.2),
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

/// Ranks of a body's water, carbon and nitrogen to try, together: the median, both one-sigma
/// ranks and the extremes, with its other ranks at their medians.
fn nitrogen_ranks() -> Vec<VolatileDraws> {
    [
        0.5,
        0.158_655_253_931_457_05,
        0.841_344_746_068_542_9,
        1e-6,
        0.999_999,
    ]
    .iter()
    .map(|&u| {
        let r = UnitUniform::new(u).unwrap();
        VolatileDraws {
            water: r,
            carbon: r,
            nitrogen: r,
            ..VolatileDraws::MEDIAN
        }
    })
    .collect()
}

/// Earth's nitrogen per unit mass at the rank `draws` gives, times `kg`: the inventory a body
/// that takes no icy boost holds.
fn earth_nitrogen(draws: &VolatileDraws, kg: f64) -> f64 {
    let multiple =
        math::exp10(VOLATILE_SCATTER_DEX * math::normal_quantile(draws.nitrogen.value()));
    EARTH_NITROGEN_PER_MASS * multiple * kg
}

/// Each of the four carriers of `a` and `b`, bit for bit.
fn assert_same_inventory(a: &VolatileInventory, b: &VolatileInventory, what: &str) {
    for (x, y, carrier) in [
        (a.water(), b.water(), "water"),
        (a.carbon_dioxide(), b.carbon_dioxide(), "carbon"),
        (a.nitrogen(), b.nitrogen(), "nitrogen"),
        (a.argon(), b.argon(), "argon"),
    ] {
        assert_eq!(
            hyperion_testkit::float::bits(x.value()),
            hyperion_testkit::float::bits(y.value()),
            "{what}: {carrier} {} against {}",
            x.value(),
            y.value()
        );
    }
}

/// An Earth mass formed at `formed_au` of the zero-age Sun and now at `a_au`, at the median ranks.
fn formed_at_now_at(formed_au: f64, a_au: f64) -> PlacedBody {
    PlacedBody::new(
        EarthMasses::new(1.0),
        orbit(a_au, 0.0),
        Metres::new(formed_au * METRES_PER_AU),
        UnitUniform::HALF,
    )
    .unwrap()
}

/// P14.T50.c: half of nitrogen is in its icy carrier, ammonia monohydrate, by 123 K, the element
/// registry's 50% condensation temperature of nitrogen (decision-ammonia-line; the constant's
/// source is on it), against the snow line's 170 K, and the boost holds at or below it. At the
/// hydrate's appearance temperature, 131 K, it does not.
#[test]
fn nitrogen_s_carrier_holds_half_of_it_by_123_k() {
    assert_same_bits(AMMONIA_HYDRATE_CONDENSATION.value(), 123.0);
    assert_same_bits(
        AMMONIA_HYDRATE_CONDENSATION.value(),
        crate::substance::Element::N
            .condensation_temperature()
            .value(),
    );
    assert!((SNOW_LINE_TEMPERATURE.value() - 170.0).abs() < 1e-12);
    assert!(nitrogen_boosted(AMMONIA_HYDRATE_CONDENSATION));
    assert!(nitrogen_boosted(Kelvin::new(40.0)));
    assert!(!nitrogen_boosted(Kelvin::new(123.000_001)));
    assert!(!nitrogen_boosted(Kelvin::new(131.0)));
    assert!(!nitrogen_boosted(SNOW_LINE_TEMPERATURE));
    // Outside the valid range: a formation distance of +0 is infinitely hot, a negative distance
    // or snow line gives a NaN, and none of them boosts.
    let au = Metres::new(METRES_PER_AU);
    assert!(
        formation_temperature(Metres::ZERO, au)
            .value()
            .is_infinite()
    );
    assert!(formation_temperature(-au, au).value().is_nan());
    assert!(formation_temperature(au, -au).value().is_nan());
    for t in [
        formation_temperature(Metres::ZERO, au),
        formation_temperature(-au, au),
        formation_temperature(au, -au),
        Kelvin::new(f64::NAN),
    ] {
        assert!(!nitrogen_boosted(t), "{} K", t.value());
    }
}

/// P14.T50.c: the ammonia line lies at (170 ÷ 123)² = 1.910 times the snow line, where the disc is
/// at 123 K: 4.32 ± 0.01 au about a zero-age Sun of 0.7 L☉ (decision-ammonia-line's figure), and
/// 4.31 ± 0.01 au about the generated zero-age Sun's disc, of 0.6977 L☉ (plan 06's zero-age main
/// sequence), whose snow line lies at 2.255 au; about a host of zero-age luminosity L it lies at
/// 5.16 √L au.
#[test]
fn the_zero_age_sun_s_ammonia_line_lies_at_4_3_au() {
    let disc_snow = solar_disc().snow_line();
    let exact_snow = crate::planetary::disc::snow_line(SolarLuminosities::new(0.7));
    for (snow, expected_au) in [(disc_snow, 4.31), (exact_snow, 4.32)] {
        let line = ammonia_line(snow);
        let ratio = line / snow;
        assert!((ratio - 1.910).abs() < 5e-4, "{ratio} snow lines");
        let au = line.value() / METRES_PER_AU;
        assert!(
            (au - expected_au).abs() <= 0.01,
            "the ammonia line at {au} au, against {expected_au}"
        );
        assert!((formation_temperature(snow, snow).value() - 170.0).abs() < 1e-12);
        assert!((formation_temperature(line, snow).value() - 123.0).abs() < 1e-9);
        assert!(!nitrogen_boosted(formation_temperature(
            line * (1.0 - 1e-9),
            snow
        )));
        assert!(nitrogen_boosted(formation_temperature(
            line * (1.0 + 1e-9),
            snow
        )));
    }
    for l in [0.01, 0.1, 1.0, 4.0, 30.0] {
        let at = ammonia_line(crate::planetary::disc::snow_line(SolarLuminosities::new(l)));
        let per_root_l = at.value() / METRES_PER_AU / l.sqrt();
        assert!(
            (per_root_l - 5.16).abs() < 5e-3,
            "{per_root_l} √L au at {l} L☉"
        );
    }
}

/// P14.T50.c: a body formed at 3 au of the zero-age Sun, between its snow line and its ammonia
/// line, has Earth's nitrogen per unit mass at its rank, and one formed at 5 au a hundred times
/// it. Their water, carbon and argon do not read the formation temperature, so they are the same
/// at both distances, bit for bit.
#[test]
fn nitrogen_takes_the_icy_boost_only_beyond_the_ammonia_line() {
    let snow = solar_disc().snow_line();
    let icy = composition(
        EarthMasses::new(0.02),
        EarthRadii::new(0.4),
        SnowLineSide::Beyond,
        EarthFluxes::new(0.01),
    )
    .unwrap()
    .fractions();
    let rocky = composition(
        EarthMasses::new(1.0),
        EarthRadii::new(1.0),
        SnowLineSide::Inside,
        EarthFluxes::new(1.0),
    )
    .unwrap()
    .fractions();
    for (mass, fractions) in [
        (EarthMasses::new(0.02), icy),
        (EarthMasses::new(1.0), rocky),
    ] {
        let kg = mass.value() * EARTH_MASS_KG;
        for draws in nitrogen_ranks() {
            let formed_at = |formed_au: f64| {
                let formed_at = formation_temperature(Metres::new(formed_au * METRES_PER_AU), snow);
                volatile_inventory(
                    mass,
                    &fractions,
                    SnowLineSide::Beyond,
                    formed_at,
                    &draws,
                    SOLAR_AGE,
                )
            };
            let (between, beyond) = (formed_at(3.0), formed_at(5.0));
            let earth = earth_nitrogen(&draws, kg);
            for (formed_au, inventory, expected) in [
                (3.0, &between, earth),
                (5.0, &beyond, ICY_VOLATILE_BOOST * earth),
            ] {
                assert!(
                    (inventory.nitrogen().value() / expected - 1.0).abs() < 1e-12,
                    "{} M⊕ formed at {formed_au} au: {} kg of nitrogen against {expected}",
                    mass.value(),
                    inventory.nitrogen().value()
                );
            }
            for (x, y) in [
                (between.water(), beyond.water()),
                (between.carbon_dioxide(), beyond.carbon_dioxide()),
                (between.argon(), beyond.argon()),
            ] {
                assert_same_bits(x.value(), y.value());
            }
        }
    }
}

/// P14.T50.c: a body formed inside the snow line takes no nitrogen boost whatever formation
/// temperature it is given, at 10 au's 80 K too: at every rank its inventory is the same at every
/// one, bit for bit, and its nitrogen is Earth's per unit mass at its rank.
#[test]
fn an_inventory_formed_inside_the_snow_line_takes_no_nitrogen_boost() {
    let snow = solar_disc().snow_line();
    let fractions = composition(
        EarthMasses::new(1.0),
        EarthRadii::new(1.0),
        SnowLineSide::Inside,
        EarthFluxes::new(1.0),
    )
    .unwrap()
    .fractions();
    for draws in nitrogen_ranks() {
        let formed_at = |formed_au: f64| {
            let formed_at = formation_temperature(Metres::new(formed_au * METRES_PER_AU), snow);
            volatile_inventory(
                EarthMasses::new(1.0),
                &fractions,
                SnowLineSide::Inside,
                formed_at,
                &draws,
                SOLAR_AGE,
            )
        };
        let at_1_au = formed_at(1.0);
        let earth = earth_nitrogen(&draws, EARTH_MASS_KG);
        assert!(
            (at_1_au.nitrogen().value() / earth - 1.0).abs() < 1e-12,
            "{} kg of nitrogen against {earth}",
            at_1_au.nitrogen().value()
        );
        for formed_au in [0.05, 0.4, 2.0, 2.25, 10.0] {
            assert_same_inventory(
                &formed_at(formed_au),
                &at_1_au,
                &format!("formed inside at {formed_au} au"),
            );
        }
    }
}

/// P14.T50.c: the atmosphere table's nitrogen. The four giants, and Ganymede and Titan placed at
/// their parents' distances, all beyond the zero-age Sun's ammonia line (4.31 au), hold a hundred
/// times Earth's nitrogen per unit mass at the median ranks; Mercury, Venus, Earth, Mars and the
/// Moon, formed inside its snow line (2.26 au), and Ceres, formed at 2.77 au between the two
/// lines, the only one there, hold Earth's. Ceres, at a hundredth of version 22's, stays airless
/// with no nitrogen in its air.
#[test]
fn the_table_s_nitrogen_takes_the_boost_only_beyond_the_ammonia_line() {
    let disc = solar_disc();
    let (snow, ammonia) = (disc.snow_line(), ammonia_line(disc.snow_line()));
    let bodies = table();
    for (name, body) in &bodies {
        let name = *name;
        let &(_, _, _, a_au, _) = table_bodies().find(|(n, ..)| *n == name).unwrap();
        let formed = Metres::new(a_au * METRES_PER_AU);
        let boost = if formed >= ammonia {
            ICY_VOLATILE_BOOST
        } else {
            1.0
        };
        let kg = Kilograms::from(body.mass()).value();
        let nitrogen = body.inventory().nitrogen().value();
        let expected = boost * earth_nitrogen(&VolatileDraws::MEDIAN, kg);
        assert!(
            (nitrogen / expected - 1.0).abs() < 1e-12,
            "{name}: {nitrogen} kg of nitrogen against {expected}"
        );
        let between = snow <= formed && formed < ammonia;
        assert_eq!(between, name == "Ceres", "{name}");
    }
    let ceres = found(&bodies, "Ceres").atmosphere();
    assert_eq!(ceres.state(), SurfaceState::Airless);
    assert_same_bits(ceres.partial_pressures().of(Gas::Nitrogen).value(), 0.0);
}

/// P14.T50.c: the boost is a step in formation distance only. A body keeps the same nitrogen per
/// unit mass at every age and wherever it has migrated to, so its inventory stays continuous in
/// time; across the ammonia line in formation distance its nitrogen steps a hundredfold.
#[test]
fn the_ammonia_line_is_a_step_in_formation_distance_only() {
    let line_au = ammonia_line(solar_disc().snow_line()).value() / METRES_PER_AU;
    let per_mass = |body: &DerivedBody| {
        body.inventory().nitrogen().value() / Kilograms::from(body.mass()).value()
    };
    for formed_au in [3.0, 5.0] {
        let first = per_mass(&derive(
            &formed_at_now_at(formed_au, formed_au),
            &solar_disc(),
            SOLAR_AGE,
        ));
        for a_au in [0.8, 1.5, formed_au, 6.0] {
            for age in [3e8, 1e9, 4.57e9, 8e9] {
                let body = derive(
                    &formed_at_now_at(formed_au, a_au),
                    &solar_disc(),
                    Years::new(age),
                );
                let n = per_mass(&body);
                assert!(
                    (n / first - 1.0).abs() < 1e-12,
                    "formed at {formed_au} au, at {a_au} au and {age} yr: {n} against {first}"
                );
            }
        }
    }
    let across = |factor: f64| {
        per_mass(&derive(
            &formed_at_now_at(line_au * factor, 1.0),
            &solar_disc(),
            SOLAR_AGE,
        ))
    };
    let (inside, beyond) = (across(1.0 - 1e-6), across(1.0 + 1e-6));
    assert!(
        (beyond / inside - ICY_VOLATILE_BOOST).abs() < 1e-9,
        "{beyond} against {inside}"
    );
}

// P14.T51.a: the condensate partition in T13.c's passes, held at 22 by the lane's hold.

use crate::planetary::chem::partition::{ClimateSpan, Ground};
use crate::planetary::hooks::surface::{
    ClimateBody, GroundInertia, OceanWeight, SeasonalSpin, step_climate,
};
use crate::units::{Degrees, MetresPerSecondSquared, Radians, Seconds};

/// An Earth's inputs, with its median inventory at its median ranks (the planets' own,
/// [`volatile_inventory`] at Earth's mass and fractions), at the equilibrium temperature
/// `equilibrium`, which its exobase reads as its irradiation, and against the habitable zone as
/// `insolation` puts it.
fn earth_inputs(equilibrium: f64, insolation: Insolation) -> AtmosphereInputs {
    let mass = EARTH_MASS_KG;
    AtmosphereInputs {
        mass: Kilograms::new(mass),
        radius: Metres::new(crate::units::consts::EARTH_RADIUS_M),
        material: SurfaceMaterial::Rock,
        envelope_fraction: 0.0,
        class: PlanetClass::Rocky,
        inventory: VolatileInventory::new(
            Kilograms::new(EARTH_WATER_PER_MASS * mass),
            Kilograms::new(EARTH_CARBON_DIOXIDE_PER_MASS * mass),
            Kilograms::new(EARTH_NITROGEN_PER_MASS * mass),
            Kilograms::new(6.6e16),
        ),
        formed: SnowLineSide::Inside,
        equilibrium: Kelvin::new(equilibrium),
        irradiation_temperature: Kelvin::new(equilibrium),
        heated: Kelvin::new(equilibrium),
        xuv_fluence: earth_xuv_fluence(),
        saturated_xuv_flux: WattsPerSquareMetre::new(0.3),
        insolation,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
    }
}

/// The span of the climate T24.a gives the body `inputs` describes for a column of partial
/// pressures `partials` over a mean surface at `t`, with Earth's spin (23.44° of obliquity, a
/// sidereal day of 86,164.1 s and a solar day of 24 h), Earth's heat flow, 0.0925 W m⁻², and no
/// ocean: the passes' climate ([`step_climate`], T24.a's own range, its greenhouse over the body's
/// airborne inventory, as `state_and_greenhouse`'s steps 2 and 3 leave it), as P14.T54.a's wiring
/// gives it from a body's own spin.
fn t24a_span(inputs: &AtmosphereInputs, t: Kelvin, partials: &PartialPressures) -> ClimateSpan {
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    let exobase = exobase_temperature(inputs.irradiation_temperature, inputs.xuv_fluence);
    let (retention, held) = retained(inputs, exobase);
    let mut airborne = Airborne::kept(inputs, retention, held);
    airborne.divide(inputs, retention, held);
    let weight = GRAVITATIONAL_CONSTANT * m / (r * r) / (4.0 * core::f64::consts::PI * r * r);
    let body = ClimateBody {
        equilibrium_temperature: inputs.equilibrium,
        surface_gravity: MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r)),
        radius: inputs.radius,
        ground: GroundInertia::of_material(inputs.material),
        spin: SeasonalSpin {
            obliquity: Radians::from(Degrees::new(23.44)),
            rotation_period: Seconds::new(86_164.1),
            solar_day: Some(Seconds::new(86_400.0)),
        },
        heat_flow: WattsPerSquareMetre::new(0.0925),
        inventory: airborne.column_weights(weight),
    };
    step_climate(&body, OceanWeight::NONE, t, partials)
}

/// A Titan's inputs: its mass and radius, formed beyond the snow line, beyond the
/// maximum-greenhouse limit, with the cold moon's inventory and irradiation of [`cold_moon`].
fn titan_inputs() -> AtmosphereInputs {
    let mut titan = earth_inputs(75.6, Insolation::BeyondMaximumGreenhouse);
    titan.mass = Kilograms::new(1.345e23);
    titan.radius = Metres::new(2.575e6);
    titan.material = SurfaceMaterial::Ice;
    titan.class = PlanetClass::Icy;
    titan.formed = SnowLineSide::Beyond;
    titan.irradiation_temperature = TITAN_IRRADIATION;
    titan.inventory = VolatileInventory::new(
        Kilograms::new(1e22),
        Kilograms::new(1e21),
        Kilograms::new(9e18),
        Kilograms::ZERO,
    );
    titan
}

/// The passes hold each condensable as the partition does, over T24.a's climate of each step's
/// column: Earth's water, a minor condensable, at Manabe and Wetherald's 0.77 of its saturation at
/// the mean, and Titan's methane on the minor branch at its humidity, 0.5 (plan 14's test keeps
/// Titan minor), both on the registry's fitted curves.
///
/// Version 22 held Earth's water at 1, its column weight at the saturation pressure, so under
/// version 22's constants the partition cooled this Earth some 5 K. P14.T57.a's water refit takes
/// that up: this Earth, at 255 K, about the generator's irradiation for Earth's measured 0.294
/// (255.14 K), lies within 1 K of the refit's 288 K.
#[test]
fn the_batch_holds_earth_s_water_and_titan_s_methane_on_the_minor_branch() {
    let earth = earth_inputs(255.0, Insolation::Habitable);
    let climate = |t: Kelvin, p: &PartialPressures| t24a_span(&earth, t, p);
    let air = atmosphere_under(
        &earth,
        &PassSurface {
            climate: PassClimate::Given(&climate),
            ..PassSurface::stand_in(PassLight::HottestHost)
        },
    );
    assert_eq!(air.state(), SurfaceState::Temperate);
    let water = mole_fraction(&air, Gas::Water) * air.surface_pressure().unwrap().value();
    let held = crate::planetary::chem::partition::WATER_SURFACE_HUMIDITY
        * crate::substance::diagram::saturation_pressure(
            Gas::Water.substance(),
            air.surface_temperature(),
        )
        .unwrap()
        .value();
    assert!(
        (water / held - 1.0).abs() < 1e-9,
        "{water} Pa against {held}"
    );
    let t = air.surface_temperature().value();
    eprintln!("this Earth under the batch: {t:.3} K");
    assert!((t - EARTH_SURFACE_TEMPERATURE).abs() < 1.0, "{t} K");
    let titan = titan_inputs();
    let climate = |t: Kelvin, p: &PartialPressures| t24a_span(&titan, t, p);
    let batch = atmosphere_under(
        &titan,
        &PassSurface {
            climate: PassClimate::Given(&climate),
            ..PassSurface::stand_in(PassLight::HottestHost)
        },
    );
    let saturated = saturation_pressure(Gas::Methane.substance(), batch.surface_temperature())
        .unwrap()
        .value();
    let in_the_air = methane_in_the_air(&batch);
    eprintln!(
        "this Titan under the batch: {:.3} K, methane {in_the_air:.3} Pa in the air",
        batch.surface_temperature().value()
    );
    assert!(
        (in_the_air / (METHANE_RELATIVE_HUMIDITY * saturated) - 1.0).abs() < 1e-9,
        "{in_the_air} Pa against {saturated} Pa saturated"
    );
}

/// A dry Earth formed inside the snow line at 2 au of a Sun, beyond the maximum-greenhouse limit,
/// stores no carbon: its 70 bar of carbon dioxide ran T13.c's √p greenhouse away to some 570 K
/// (science-p14-t24f, finding 1), and P14.T51.a's cold trap was to hold it at `p_sat`(`T_cold`).
/// Under the partition, with T24.a's own contrasts of each step's column (Earth's spin, no ocean),
/// it does not: from its equilibrium temperature to 273 K, every surface holding its carbon dioxide
/// at its cold trap is warmed above itself by T13.c's grey greenhouse, so no state below freezing
/// holds itself and the iteration runs to the whole inventory's.
///
/// The states that fail hold 18 Pa to 0.2 bar of carbon dioxide, two to three orders of magnitude
/// below the some 8 bar at which Kopparapu et al. (2013, arXiv:1301.6674v2, §3.3, read) put the
/// maximum greenhouse, `S_eff` 0.325 at 1.70 au, so the cause is that the √p term, whose CO₂
/// constant Venus fixes, overstates a thin carbon dioxide air: given Mars's measured 607 Pa it
/// gives τ = 1.44 and 252 K at the fact sheet's 209.8 K black-body temperature, where Mars has
/// 214 K (NASA's Mars fact sheet). This test pins the finding until its fix, a carbon dioxide
/// greenhouse that holds Venus, Earth and Mars together, P14.T53.a's, takes the plan's test over
/// (decision-p14-t51a-partition §1).
#[test]
fn a_dry_earth_beyond_the_maximum_greenhouse_still_runs_away_past_its_cold_trap() {
    let earth = earth_inputs(165.6, Insolation::BeyondMaximumGreenhouse);
    let climate = |t: Kelvin, p: &PartialPressures| t24a_span(&earth, t, p);
    let air = atmosphere_under(
        &earth,
        &PassSurface {
            climate: PassClimate::Given(&climate),
            ..PassSurface::stand_in(PassLight::HottestHost)
        },
    );
    assert!(air.surface_temperature().value() > 500.0, "{air:?}");
    // No state below freezing holds itself: at each, with its column held by the partition over
    // its own climate, the greenhouse warms the surface above it.
    let retention = air.retention();
    let held = IceHeld::of(&earth, retention);
    let mut airborne = Airborne::kept(&earth, retention, held);
    assert_eq!(
        airborne.divide(&earth, retention, held),
        SurfaceState::Snowball
    );
    let (m, r) = (earth.mass.value(), earth.radius.value());
    let weight_gravity = GRAVITATIONAL_CONSTANT * m / (r * r);
    let weight = weight_gravity / (4.0 * core::f64::consts::PI * r * r);
    let inventory = airborne.column(weight).unwrap();
    let mut weights = AirborneWeights::new();
    for step in 0..=107 {
        let t = Kelvin::new(166.0 + f64::from(step));
        // From the whole column, as the greenhouse starts.
        let mut partials = whole_column(&inventory);
        for _ in 0..8 {
            let span = t24a_span(&earth, t, &partials);
            let ground = Ground::without_basins(MetresPerSecondSquared::new(weight_gravity));
            partials = partitioned_partials(&inventory, &span, &ground, &mut weights);
        }
        let warmed = grey_surface_temperature(
            earth.equilibrium,
            GreenhouseConstants::FITTED.optical_depth(&partials, earth.equilibrium),
        );
        assert!(warmed > t, "{t:?} holds itself at {warmed:?}: {partials:?}");
    }
}

// P14.T51.b: the area-mix Bond albedo in the passes.

use crate::planetary::derive::passes::Convergence;
use crate::planetary::derive::photometry::area_mix::{
    Regolith, airless_silicate_stand_in, solid_bond, sun_bands,
};
use crate::planetary::derive::{GivenBulk, GivenFractions, equilibrium_temperature};
use crate::planetary::moons::regular::moon_radius;
use crate::planetary::params::ICY_WATER_FRACTION;

/// `placed` derived in the zero-age Sun's disc `disc` about the present Sun with its history at
/// the Solar System's age.
fn derived(placed: &PlacedBody, disc: &DiscProfile) -> DerivedBody {
    crate::planetary::derive::solar::derive_historic(placed, disc, SOLAR_AGE)
}

/// P14.T51.h: a body whose batch passes settle within the plain passes keeps P14.T51.b's bits, its
/// production wiring (the pass, the albedo it gives, the 10⁻⁹ test, the start) held against
/// T51.b's loop as it was built, `PassDriver::T51b`: every body of the table, Jupiter, an icy
/// belt member as recorded and as placed, and a Pluto, every field of their derivations the same,
/// the count and the outcome included (the table's three-pass bodies among them, so T57.a's fits
/// do not move).
#[test]
fn a_body_that_settles_keeps_t51b_s_bits() {
    use crate::planetary::derive::pass_record::PassRecord;
    use crate::planetary::derive::{PassDriver, derive_body_driven};

    let disc = solar_disc();
    let lights = [historic_sun()];
    let hosts = BodyHosts::new(
        Kilograms::new(SOLAR_MASS_KG),
        Composition::SOLAR,
        &lights,
        &[],
    )
    .unwrap();
    let mut placed: Vec<(&str, PlacedBody)> = table_bodies()
        .map(|&(name, kg, km, a, e)| (name, placed_about_the_sun(&disc, (kg, km, a, e))))
        .collect();
    placed.push((
        "Jupiter",
        crate::planetary::derive::solar::placed(1_898.13e24, 5.203_8, 0.048_9, 0.5),
    ));
    let member = crate::planetary::derive::solar::placed(1e21, 43.0, 0.05, 0.05);
    let bulk = GivenBulk::new(
        moon_radius(member.mass(), 0.35),
        GivenFractions::Exactly(crate::planetary::derive::MassFractions::solid(
            0.0, 0.65, 0.35,
        )),
    );
    placed.push(("an icy member as recorded", member.with_bulk(bulk)));
    placed.push(("an icy member as placed", member));
    let rank = crate::planetary::derive::solar::rank_for(&disc, 1.303e22, 1_188.3, 39.48, 0.248_8)
        .clamp(0.001, 0.999);
    placed.push((
        "Pluto",
        crate::planetary::derive::solar::placed(1.303e22, 39.48, 0.248_8, rank),
    ));
    for (name, body) in &placed {
        let derive = |driver| {
            derive_body_driven(
                driver,
                body,
                &hosts,
                &disc,
                SOLAR_AGE,
                UniverseTime::EPOCH,
                PassRecord::StandIn,
            )
            .unwrap()
        };
        let (solved, t51b) = (derive(PassDriver::Solve), derive(PassDriver::T51b));
        assert_eq!(solved.convergence(), Convergence::Converged, "{name}");
        assert_eq!(format!("{solved:?}"), format!("{t51b:?}"), "{name}");
    }
}

/// Decision-p14-t54a-ganymede §1.4: [`EXOBASE_MULTIPLE`] is T13.c's 5 `T_eq` at the measured
/// Ganymede's Bond albedo, 5 × (1 − 0.35)^¼, to the bit, and it lies inside its window at the
/// measured moons' radii, 4.15–5.23 `T₀`: the measured Ganymede's nitrogen is lost (λ 23.1) and
/// Titan's kept (λ 29.1), each at the irradiation of its giant's semi-major axis about the present
/// Sun (JPL's GM and mean radii, JUP365 and SAT441, Archinal et al. 2018; NASA's fact sheets,
/// Saturn's 9.5826 au from an edition before the update of 18 March 2025, which gives 9.5726 au).
#[test]
fn the_exobase_multiple_loses_ganymede_s_nitrogen_and_keeps_titan_s() {
    assert_same_bits(EXOBASE_MULTIPLE, 5.0 * math::powf(1.0 - 0.35, 0.25));
    let nitrogen = |gm: f64, km: f64, a_au: f64| {
        let irradiation = equilibrium_temperature(
            EarthFluxes::new(1.0 / (a_au * a_au)),
            BondAlbedo::from_fraction(0.0),
        );
        let exobase = exobase_temperature(irradiation, JoulesPerSquareMetre::ZERO);
        assert_same_bits(exobase.value(), EXOBASE_MULTIPLE * irradiation.value());
        let (mass, radius) = (
            Kilograms::new(gm / GRAVITATIONAL_CONSTANT),
            Metres::new(km * 1e3),
        );
        let lambda = jeans_parameter(mass, radius, Gas::Nitrogen, exobase);
        let retained = Retention::of(mass, radius, exobase).retains(Gas::Nitrogen);
        (lambda, retained)
    };
    let (ganymede, ganymede_kept) = nitrogen(9_887.832_75e9, 2_631.20, 5.203_8);
    let (titan, titan_kept) = nitrogen(8_978.137_10e9, 2_574.76, 9.582_6);
    eprintln!("λ(N₂) at {EXOBASE_MULTIPLE} T₀: Ganymede {ganymede:.3}, Titan {titan:.3}");
    assert!(
        !ganymede_kept && (ganymede - 23.1).abs() < 0.05,
        "Ganymede: {ganymede}"
    );
    assert!(titan_kept && (titan - 29.1).abs() < 0.05, "Titan: {titan}");
    // The window: the multiples at which each sits on the threshold.
    let (lower, upper) = (
        EXOBASE_MULTIPLE * ganymede / JEANS_RETENTION_THRESHOLD,
        EXOBASE_MULTIPLE * titan / JEANS_RETENTION_THRESHOLD,
    );
    eprintln!("the window at the measured radii: {lower:.4}–{upper:.4} T₀");
    assert!((lower - 4.15).abs() < 0.005 && (upper - 5.23).abs() < 0.005);
    assert!(lower < EXOBASE_MULTIPLE && EXOBASE_MULTIPLE < upper);
}

/// Decision-p14-t54a-ganymede §1.4: a body's exobase and its bulk-air retention, Jeans's test's
/// verdict, are the same bits at every albedo a pass reads, its passes' published ones included,
/// and its exobase is [`exobase_temperature`] of its zero-albedo irradiation temperature: every
/// body of the table, Jupiter, an icy belt member as recorded and as placed, each through one
/// pass at albedos from 0 to 0.95, while the equilibrium temperature each pass reads moves with
/// its albedo. The ice-held water and nitrogen of an ice crust read that temperature, as a
/// volatile ice's vapour does (the ruling's C0 follow-up, rule (b)), so they alone may differ
/// between passes: the icy member as recorded, 10²¹ kg at 43 au, holds its N₂ (and the methane
/// beside it) at 0.95, at 20 K, and not at 0.832, at 27 K, nor in its solved pass; every other
/// body's ice holds the same at every albedo.
#[test]
fn bulk_air_retention_is_independent_of_the_albedo_a_pass_reads() {
    use crate::planetary::derive::pass_record::PassRecord;
    use crate::planetary::derive::{PassDriver, derive_body_driven};

    // The bulk-air verdict of each gas: Jeans's test's, not the ice's.
    let bulk_air = |air: &Atmosphere| Gas::ALL.map(|gas| air.retains_against_escape(gas));

    let disc = solar_disc();
    let lights = [historic_sun()];
    let hosts = BodyHosts::new(
        Kilograms::new(SOLAR_MASS_KG),
        Composition::SOLAR,
        &lights,
        &[],
    )
    .unwrap();
    let mut placed: Vec<(&str, PlacedBody)> = table_bodies()
        .map(|&(name, kg, km, a, e)| (name, placed_about_the_sun(&disc, (kg, km, a, e))))
        .collect();
    placed.push((
        "Jupiter",
        crate::planetary::derive::solar::placed(1_898.13e24, 5.203_8, 0.048_9, 0.5),
    ));
    let member = crate::planetary::derive::solar::placed(1e21, 43.0, 0.05, 0.05);
    let bulk = GivenBulk::new(
        moon_radius(member.mass(), 0.35),
        GivenFractions::Exactly(crate::planetary::derive::MassFractions::solid(
            0.0, 0.65, 0.35,
        )),
    );
    placed.push(("an icy member as recorded", member.with_bulk(bulk)));
    placed.push(("an icy member as placed", member));
    for (name, body) in &placed {
        let derive = |driver| {
            derive_body_driven(
                driver,
                body,
                &hosts,
                &disc,
                SOLAR_AGE,
                UniverseTime::EPOCH,
                PassRecord::StandIn,
            )
            .unwrap()
        };
        let solved = derive(PassDriver::Solve);
        let air = solved.atmosphere();
        // The present Sun is its brightest, so the flux now is the flux at the peak.
        let irradiation = equilibrium_temperature(solved.flux(), BondAlbedo::from_fraction(0.0));
        assert_same_bits(
            air.exobase_temperature().value(),
            exobase_temperature(irradiation, solved.xuv_fluence()).value(),
        );
        let mut equilibria = Vec::new();
        for albedo in [0.0, 0.11, 0.3, 0.35, 0.536, 0.832, 0.95] {
            let once = derive(PassDriver::Once(BondAlbedo::from_fraction(albedo)));
            let at = once.atmosphere();
            assert_same_bits(
                at.exobase_temperature().value(),
                air.exobase_temperature().value(),
            );
            assert_eq!(bulk_air(at), bulk_air(air), "{name} at {albedo}");
            let t_eq = once.equilibrium_temperature().value();
            if *name == "an icy member as recorded" && albedo > 0.9 {
                // At 0.95, 20 K: its N₂ ice's vapour is too thin to escape over the age.
                assert!((19.5..20.5).contains(&t_eq), "{name} at {albedo}: {t_eq} K");
                for gas in [Gas::Nitrogen, Gas::Methane] {
                    assert!(at.ice_held().retains(gas), "{name} at {albedo}: {at:?}");
                    assert!(!air.ice_held().retains(gas), "{name}: {air:?}");
                }
            } else {
                // At 0.832, 27 K, as in the solved pass, it escapes.
                assert_eq!(at.ice_held(), air.ice_held(), "{name} at {albedo}");
            }
            if *name == "an icy member as recorded" && (albedo - 0.832).abs() < 1e-9 {
                assert!((26.5..27.5).contains(&t_eq), "{name} at {albedo}: {t_eq} K");
                assert!(
                    !at.ice_held().retains(Gas::Nitrogen),
                    "{name} at {albedo}: {at:?}"
                );
            }
            equilibria.push(t_eq);
        }
        assert_same_bits(equilibria[0], irradiation.value());
        assert!(
            equilibria.windows(2).all(|pair| pair[1] < pair[0]),
            "{name}: {equilibria:?}"
        );
    }
}

/// P14.T51.i: the bare regolith's Bond albedo the cold traps read comes from the passes: the
/// table's Moon, Mercury and Ceres take the Moon's 0.11 to the bit, as their area mix does (their
/// cold-trap figures move by nothing), and Ganymede, icy, its frost's.
#[test]
fn the_regolith_albedo_is_the_batch_s_bare_ground() {
    let batch = table();
    let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
    for name in ["Moon", "Mercury", "Ceres"] {
        let body = found(&batch, name);
        assert_same_bits(body.regolith_albedo(), moon);
        assert_same_bits(body.albedo().value(), moon);
    }
    let frost = Regolith::of_material(SurfaceMaterial::Ice)
        .bond(SurfaceState::Airless, &sun_bands())
        .bond;
    let ganymede = found(&batch, "Ganymede").regolith_albedo();
    assert!(
        (ganymede - frost).abs() < 1e-3,
        "{ganymede} against {frost}"
    );
}

/// P14.T51.b, the table through the passes (their stand-in surface: no basins, a
/// cover linear across an isothermal climate, the regolith's lid not known):
///
/// - Venus keeps its 0.76 exactly, by its full deck;
/// - the Moon keeps its 0.11 exactly, by the space-weathering stand-in: an airless rocky body whose
///   lid the passes do not know, as on the stand-in, takes the Moon's 0.11 (the material's rule,
///   which P14.T54.a keeps where no lid is known; a known lid of the `Silicate` family is capped at
///   it as well), so Mercury and Ceres keep 0.11 too (recorded), where `basalt`'s fresh row is
///   0.0519 about a Sun; Mercury's 0.088 and its 0.068 in V are P14.T54.b's
///   (decision-p14-t51d-weathering §3);
/// - Earth takes its area mix within 0.02 of 0.294 under Earth's 0.67 of cloud, its value
///   recorded: with no basins its water fills no sea, and its land is `basalt`'s;
/// - Jupiter, class I of Sudarsky, Burrows and Pinto about a Sun under its hazes, lies within 0.05
///   of 0.48 (decision-p14-t54a-ganymede §2), recorded against Li et al. 2018's measured
///   0.503 ± 0.012 (Nat. Commun. 9, 3709), which supersedes Hanel et al. 1981's 0.343 ± 0.032;
/// - every body's equilibrium temperature is its albedo's, and its passes converge within the plain
///   passes (`Converged`, P14.T51.h); the rest are recorded.
#[test]
fn under_the_batch_the_table_takes_the_area_mix() {
    let batch = table();
    let venus = found(&batch, "Venus");
    assert_eq!(venus.atmosphere().state(), SurfaceState::RunawayGreenhouse);
    assert_same_bits(venus.albedo().value(), 0.76);
    assert_same_bits(venus.atmosphere().cloud_fraction(), 1.0);
    let moon = found(&batch, "Moon");
    assert_eq!(moon.atmosphere().state(), SurfaceState::Airless);
    assert_same_bits(
        moon.albedo().value(),
        SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value(),
    );
    let basalt = solid_bond(crate::substance::by_key("basalt").unwrap(), &sun_bands()).bond;
    eprintln!(
        "basalt's fresh Bond albedo about a Sun {basalt:.5}, its stand-in under a known lid {:.5}",
        airless_silicate_stand_in(basalt)
    );
    let earth = found(&batch, "Earth");
    assert_eq!(earth.atmosphere().state(), SurfaceState::Temperate);
    assert_same_bits(earth.atmosphere().cloud_fraction(), 0.67);
    assert!(
        (earth.albedo().value() - 0.294).abs() < 0.02,
        "{:?}",
        earth.albedo()
    );
    let disc = solar_disc();
    let jupiter = crate::planetary::derive::solar::placed(1_898.13e24, 5.203_8, 0.048_9, 0.5);
    let jupiter = derived(&jupiter, &disc);
    assert_eq!(jupiter.atmosphere().state(), SurfaceState::GasEnvelope);
    assert!(
        (jupiter.albedo().value() - 0.48).abs() < 0.05,
        "{:?}",
        jupiter.albedo()
    );
    eprintln!(
        "Jupiter: Bond albedo {:.5} at its effective temperature {:?}, {} passes",
        jupiter.albedo().value(),
        jupiter.effective_temperature(),
        jupiter.atmosphere_passes()
    );
    for (name, body) in &batch {
        let passes = body.atmosphere_passes();
        assert_eq!(
            body.convergence(),
            Convergence::Converged,
            "{name}: {passes} passes"
        );
        assert_same_bits(
            body.equilibrium_temperature().value(),
            equilibrium_temperature(body.flux(), body.albedo()).value(),
        );
        eprintln!(
            "{name}: {:?}, Bond albedo {:.5} under {:.4} of cloud, T_eq {:.2} K, T_s {:.2} K, \
             {passes} passes",
            body.atmosphere().state(),
            body.albedo().value(),
            body.atmosphere().cloud_fraction(),
            body.equilibrium_temperature().value(),
            body.atmosphere().surface_temperature().value(),
        );
    }
}

/// An icy belt member whose placement holds under 10% water (a Kuiper belt body of 10²¹ kg at 43
/// au, its Chen and Kipping rank dense) takes the albedo of its record's bulk, ice
/// (`icy_member_bulk`'s 0.35 at its lowest rank, at `moon_radius`'s radius), given to its
/// derivation as its bulk (`PlanetarySystem`'s `with_record_bulk`): its fresh frost's, and so its
/// equilibrium temperature; its placement's rock takes the Moon's 0.11, its lid not known.
#[test]
fn an_icy_member_takes_the_albedo_of_its_record_s_ice() {
    let disc = solar_disc();
    let kg = 1e21;
    let placed = [0.02, 0.05, 0.1, 0.2]
        .into_iter()
        .map(|rank| crate::planetary::derive::solar::placed(kg, 43.0, 0.05, rank))
        .find(|placed| derived(placed, &disc).fractions().water() < ICY_WATER_FRACTION)
        .expect("a dense rank holds under 10% water");
    let ice = 0.35;
    let mass = placed.mass();
    let bulk = GivenBulk::new(
        moon_radius(mass, ice),
        GivenFractions::Exactly(crate::planetary::derive::MassFractions::solid(
            0.0,
            1.0 - ice,
            ice,
        )),
    );
    let as_placed = derived(&placed, &disc);
    let as_recorded = derived(&placed.with_bulk(bulk), &disc);
    assert_eq!(as_recorded.atmosphere().state(), SurfaceState::Airless);
    let frost = Regolith::of_material(SurfaceMaterial::Ice)
        .bond(SurfaceState::Airless, &sun_bands())
        .bond;
    let rock = Regolith::of_material(SurfaceMaterial::Rock)
        .bond(SurfaceState::Airless, &sun_bands())
        .bond;
    assert!(
        as_recorded.albedo().value() > 0.5 && (as_recorded.albedo().value() - frost).abs() < 0.05,
        "{:?} against its frost's {frost}",
        as_recorded.albedo()
    );
    assert!(
        (as_placed.albedo().value() - rock).abs() < 1e-9,
        "{:?}",
        as_placed.albedo()
    );
    assert!(as_recorded.equilibrium_temperature() < as_placed.equilibrium_temperature());
    assert_same_bits(
        as_recorded.equilibrium_temperature().value(),
        equilibrium_temperature(as_recorded.flux(), as_recorded.albedo()).value(),
    );
    eprintln!(
        "an icy member of {kg:e} kg at 43 au: as placed {:.5} at {:.2} K, as recorded {:.5} at \
         {:.2} K",
        as_placed.albedo().value(),
        as_placed.equilibrium_temperature().value(),
        as_recorded.albedo().value(),
        as_recorded.equilibrium_temperature().value()
    );
}

/// A Pluto at its own inventory (NASA's Pluto fact sheet's mass, 1.303 × 10²² kg,
/// and mean radius, 1,188.3 km, from memory, at its 39.48 au and e 0.2488): airless with no N₂ until
/// P14.T53.c, its albedo recorded, not asserted (decision-triton-and-e101 §1.6). Its cheap
/// invariants are: the airless state, a finite albedo in [0, 1), and passes that converge within
/// the plain passes (`Converged`, P14.T51.h).
#[test]
fn a_pluto_at_its_own_inventory_is_recorded() {
    let disc = solar_disc();
    let (kg, km, a, e) = (1.303e22, 1_188.3, 39.48, 0.248_8);
    let rank = crate::planetary::derive::solar::rank_for(&disc, kg, km, a, e).clamp(0.001, 0.999);
    let placed = crate::planetary::derive::solar::placed(kg, a, e, rank);
    let batch = derived(&placed, &disc);
    assert_eq!(batch.atmosphere().state(), SurfaceState::Airless);
    let albedo = batch.albedo().value();
    assert!(
        albedo.is_finite() && (0.0..1.0).contains(&albedo),
        "{albedo}"
    );
    assert_eq!(
        batch.convergence(),
        Convergence::Converged,
        "{} passes",
        batch.atmosphere_passes()
    );
    eprintln!(
        "Pluto at its own inventory: {:?}, Bond albedo {:.5}, T_eq {:.2} K, N2 {:.3e} Pa, water \
         {:.3}",
        batch.atmosphere().state(),
        batch.albedo().value(),
        batch.equilibrium_temperature().value(),
        batch
            .atmosphere()
            .partial_pressures()
            .of(Gas::Nitrogen)
            .value(),
        batch.fractions().water()
    );
}

/// P14.T51.b's cost: each table body's derivation, its passes with their area mix, run by name in
/// release and recorded in plan 14's Risks. A timing alone, asserting nothing.
#[test]
#[ignore = "a timing, run by name in release"]
fn time_the_batch_passes() {
    let disc = solar_disc();
    let n = 20;
    let mut sink = 0.0;
    for &(name, kg, km, a, e) in table_bodies() {
        let placed = placed_about_the_sun(&disc, (kg, km, a, e));
        let start = std::time::Instant::now();
        for _ in 0..n {
            sink += derived(&placed, &disc).albedo().value();
        }
        let each = start.elapsed().as_secs_f64() / f64::from(n) * 1e3;
        let passes = derived(&placed, &disc).atmosphere_passes();
        eprintln!(
            "{name}: derive_body {each:.3} ms ({passes} passes, {:.3} ms a pass)",
            each / f64::from(passes)
        );
    }
    eprintln!("({sink})");
}

// P14.T57.a: the greenhouse's calibration at Venus's measured state, then water at Earth.

use crate::planetary::chem::partition::WATER_SURFACE_HUMIDITY;

/// The mass of Venus's air, kg: about 4.8 × 10²⁰ (NASA's Venus fact sheet, Williams, updated
/// 2024; [`GREENHOUSE_CARBON_DIOXIDE`]).
const VENUS_AIR_KG: f64 = 4.8e20;
/// Venus's carbon dioxide's share of its air by volume, 96.5 ± 0.8% below 45 km (von Zahn et al.
/// 1983, in _Venus_, University of Arizona Press, Table II, as Johnson and de Oliveira 2019 compile
/// it).
const VENUS_CARBON_DIOXIDE_BY_VOLUME: f64 = 0.965;
/// Venus's nitrogen's share of its air by volume, 3.5 ± 0.8% below 45 km (von Zahn et al. 1983,
/// Table II, as Johnson and de Oliveira 2019 compile it).
const VENUS_NITROGEN_BY_VOLUME: f64 = 0.035;

/// Venus's carbon dioxide and nitrogen as measured, kg: [`VENUS_AIR_KG`] divided by mass at its
/// shares by volume and the registry's molar masses, 4.69 × 10²⁰ and 1.08 × 10¹⁹ kg.
fn venus_measured_kg() -> (f64, f64) {
    let co2 = VENUS_CARBON_DIOXIDE_BY_VOLUME * Gas::CarbonDioxide.molar_mass_g_per_mol();
    let n2 = VENUS_NITROGEN_BY_VOLUME * Gas::Nitrogen.molar_mass_g_per_mol();
    (
        VENUS_AIR_KG * co2 / (co2 + n2),
        VENUS_AIR_KG * n2 / (co2 + n2),
    )
}

/// The table's Venus given its measured inventory by hand: its carbon's and nitrogen's ranks those
/// at which T13.a's log-normal gives [`venus_measured_kg`] (z = log₁₀(M ÷ `M_median`) ÷ 0.5 dex,
/// ranked by Φ through `math::erfc`), derived about the Sun.
fn measured_venus() -> DerivedBody {
    let disc = solar_disc();
    let &(_, kg, km, a, e) = table_bodies().find(|(name, ..)| *name == "Venus").unwrap();
    let placed = placed_about_the_sun(&disc, (kg, km, a, e));
    let mass = placed.mass().value() * EARTH_MASS_KG;
    let rank = |measured: f64, median: f64| {
        let z = math::log10(measured / median) / VOLATILE_SCATTER_DEX;
        UnitUniform::new(0.5 * math::erfc(-z * core::f64::consts::FRAC_1_SQRT_2)).unwrap()
    };
    let (co2, n2) = venus_measured_kg();
    let draws = VolatileDraws {
        carbon: rank(co2, EARTH_CARBON_DIOXIDE_PER_MASS * mass),
        nitrogen: rank(n2, EARTH_NITROGEN_PER_MASS * mass),
        ..VolatileDraws::MEDIAN
    };
    derived(&placed.with_volatiles(draws), &disc)
}

/// Venus's measured surface temperature, the fit's target: 737 K, the mean over a sphere of its
/// 6,051.8 km mean radius: the Venus International Reference Atmosphere's 735.3 K at its altitude
/// zero (Seiff et al. 1985, Adv. Space Res. 5(11), 3, read in transcription), which Seiff et al.
/// put at 6,052 km as a search excerpt gives it (lower confidence), carried 0.2 km down at its
/// 7.6 K km⁻¹: NASA's fact sheet's 737 K (decision-p14-t57a-fits §3;
/// [`GREENHOUSE_CARBON_DIOXIDE`]).
const VENUS_SURFACE_TEMPERATURE: f64 = 737.0;

/// Earth's surface temperature, the water fit's target: 288.0 K, the pre-industrial Earth's surface
/// emission temperature, which the grey `T_s` is (decision-p14-t57a-fits §2; [`GREENHOUSE_WATER`]).
const EARTH_SURFACE_TEMPERATURE: f64 = 288.0;

/// Titan's surface temperature, 94 K: the Huygens probe's 93.65 ± 0.25 K at 1,467 hPa (Fulchignoni
/// et al. 2005, Nature 438, 785).
const TITAN_SURFACE_TEMPERATURE: f64 = 94.0;

/// The grey optical depth a surface at `surface` K over an equilibrium at `equilibrium` K needs:
/// τ = ((`T_s` ÷ `T_eq`)⁴ − 1) ÷ ¾, the inverse of [`grey_surface_temperature`].
fn needed_depth(surface: f64, equilibrium: f64) -> f64 {
    (math::powi(surface / equilibrium, 4) - 1.0) / 0.75
}

/// The column weight, Pa, at which water's partial pressure, its mole fraction times the summed
/// weights, is [`WATER_SURFACE_HUMIDITY`] of its saturation pressure `saturation` among the other
/// gases of `partials` (P14.T51.a's minor branch, `weight_meeting_cap`): the
/// positive root of w² + b w − c = 0, b = `W_o` − h `p_sat`, c = h `p_sat` `μ_w` `Σ_o` (`w_o` ÷ `μ_o`).
pub(crate) fn water_weight_at_humidity(partials: &PartialPressures, saturation: f64) -> f64 {
    let others = Gas::ALL.into_iter().filter(|&gas| gas != Gas::Water);
    let (weight, moles) = others.fold((0.0, 0.0), |(w, n), gas| {
        let p = partials.of(gas).value();
        (w + p, n + p / gas.molar_mass_g_per_mol())
    });
    let held = WATER_SURFACE_HUMIDITY * saturation;
    let b = weight - held;
    let c = held * Gas::Water.molar_mass_g_per_mol() * moles;
    2.0 * c / (b + (b * b + 4.0 * c).sqrt())
}

/// Water's saturation pressure at `t` K on the registry's curve, Pa, as the partition reads it.
fn water_saturation(t: f64) -> f64 {
    crate::substance::diagram::saturation_pressure(Gas::Water.substance(), Kelvin::new(t))
        .unwrap()
        .value()
}

/// The equilibrium temperature the generator's irradiation gives `body` at the Bond albedo of the
/// state `state` on rock, its measured one: Venus's 0.76 for the runaway state, Earth's 0.294 for
/// the temperate, the albedos the area mix's clouds are calibrated to (`water_cloud_albedo`;
/// decision-p14-t57a-fits §1.3).
fn at_measured_albedo(body: &DerivedBody, state: SurfaceState) -> Kelvin {
    equilibrium_temperature(body.flux(), state.albedo(SurfaceMaterial::Rock))
}

/// The table's Earth's inputs at the equilibrium temperature `equilibrium`, its
/// inventory and bulk as derived: what its passes give its greenhouse, its exobase at the
/// irradiation its passes read, with the hosts' light and history Earth's ([`earth_inputs`]).
fn table_earth_inputs(earth: &DerivedBody, equilibrium: Kelvin) -> AtmosphereInputs {
    AtmosphereInputs {
        mass: Kilograms::from(earth.mass()),
        radius: Metres::from(earth.radius()),
        material: SurfaceMaterial::of(&earth.fractions()),
        envelope_fraction: earth.fractions().envelope(),
        class: earth.class(),
        inventory: *earth.inventory(),
        equilibrium,
        irradiation_temperature: equilibrium_temperature(
            earth.flux(),
            BondAlbedo::from_fraction(0.0),
        ),
        heated: equilibrium,
        ..earth_inputs(equilibrium.value(), Insolation::Habitable)
    }
}

/// P14.T57.a: the grey greenhouse's constants are their fits, in their order, each at its body's
/// measured inventory and measured Bond albedo under the generator's own irradiation
/// (decision-p14-t57a-fits §1.3–§1.4), recomputed here from the code, so that a change to what the
/// fits read fails here with the refit:
///
/// - carbon dioxide on Venus at its measured inventory, given by hand ([`measured_venus`]), at the
///   irradiation for 0.76, which the passes give it: its τ at [`VENUS_SURFACE_TEMPERATURE`]
///   less the nitrogen term as built, over √p(CO₂), 14.862;
/// - then water on the table's Earth, whose carbon dioxide and nitrogen are Earth's, at the
///   irradiation for 0.294: its water held by the partition on the minor branch at Manabe and
///   Wetherald's humidity (checked at its own `T_s`), solved at
///   [`EARTH_SURFACE_TEMPERATURE`], and its τ there less the carbon dioxide and nitrogen terms, over
///   √p(H₂O), 4.469.
///
/// Each constant is held to half a unit in its second decimal. The fits read no input P14.T54.a's
/// wiring moves. Titan's nitrogen is not refitted (decision-p14-t51-batch E1;
/// `under_the_batch_titan_is_94_k`).
#[test]
fn the_greenhouse_constants_are_their_fits() {
    let fitted = GreenhouseConstants::FITTED;
    let root = |p: f64| (p / PASCALS_PER_BAR).sqrt();
    let nitrogen_term = |partials: &PartialPressures, equilibrium: f64| {
        fitted.nitrogen()
            * math::powi(NITROGEN_REFERENCE_TEMPERATURE.value() / equilibrium, 2)
            * root(partials.of(Gas::Nitrogen).value())
    };

    let venus = measured_venus();
    let (co2_kg, n2_kg) = venus_measured_kg();
    assert!((venus.inventory().carbon_dioxide().value() / co2_kg - 1.0).abs() < 1e-9);
    assert!((venus.inventory().nitrogen().value() / n2_kg - 1.0).abs() < 1e-9);
    let air = venus.atmosphere();
    assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
    let partials = air.partial_pressures();
    assert_eq!(partials.of(Gas::Water), Pascals::ZERO);
    let t_eq = at_measured_albedo(&venus, SurfaceState::RunawayGreenhouse);
    assert_same_bits(t_eq.value(), venus.equilibrium_temperature().value());
    let t_eq = t_eq.value();
    let carbon_dioxide = (needed_depth(VENUS_SURFACE_TEMPERATURE, t_eq)
        - nitrogen_term(partials, t_eq))
        / root(partials.of(Gas::CarbonDioxide).value());
    eprintln!(
        "Venus measured: {:.3} bar of CO2 and {:.4} of N2 by weight, T_eq {t_eq:.4} K: k(CO2) \
         {carbon_dioxide:.4}",
        partials.of(Gas::CarbonDioxide).value() / PASCALS_PER_BAR,
        partials.of(Gas::Nitrogen).value() / PASCALS_PER_BAR,
    );
    assert!(
        (GREENHOUSE_CARBON_DIOXIDE - carbon_dioxide).abs() <= 0.005,
        "refit GREENHOUSE_CARBON_DIOXIDE at Venus's measured state: {carbon_dioxide:.3}"
    );

    let batch = table();
    let earth = found(&batch, "Earth");
    let air = earth.atmosphere();
    assert_eq!(air.state(), SurfaceState::Temperate);
    let partials = air.partial_pressures();
    // The partition holds Earth's water at its humidity on the minor branch, as the fit takes it.
    let own = water_weight_at_humidity(
        partials,
        water_saturation(air.surface_temperature().value()),
    );
    let water = partials.of(Gas::Water).value();
    assert!(
        (water / own - 1.0).abs() < 1e-6,
        "{water} Pa against {own} Pa"
    );
    let t_eq = at_measured_albedo(earth, SurfaceState::Temperate).value();
    let at_target = water_weight_at_humidity(partials, water_saturation(EARTH_SURFACE_TEMPERATURE));
    let water_constant = (needed_depth(EARTH_SURFACE_TEMPERATURE, t_eq)
        - fitted.carbon_dioxide() * root(partials.of(Gas::CarbonDioxide).value())
        - nitrogen_term(partials, t_eq))
        / root(at_target);
    eprintln!(
        "Earth: {:.3} Pa of CO2 and {:.5} bar of N2, water {at_target:.3} Pa at \
         {EARTH_SURFACE_TEMPERATURE} K, T_eq {t_eq:.4} K: k(H2O) {water_constant:.4}",
        partials.of(Gas::CarbonDioxide).value(),
        partials.of(Gas::Nitrogen).value() / PASCALS_PER_BAR,
    );
    assert!(
        (GREENHOUSE_WATER - water_constant).abs() <= 0.005,
        "refit GREENHOUSE_WATER on Earth after carbon dioxide: {water_constant:.3}"
    );
}

/// P14.T57.a: Venus at its measured inventory, given by hand ([`measured_venus`]), is 737 K ± 2%
/// under P14.T57.a's constants; under version 22's, fitted at the median, it was 778 K.
#[test]
fn the_measured_venus_reaches_737_k_under_the_batch() {
    let batch = measured_venus();
    let t = batch.atmosphere().surface_temperature().value();
    eprintln!("the measured Venus: {t:.3} K");
    assert!(
        ((t - VENUS_SURFACE_TEMPERATURE) / VENUS_SURFACE_TEMPERATURE).abs() < 0.02,
        "{t} K"
    );
}

/// P14.T57.a: the measured Earth, the table's Earth's inputs at the generator's
/// irradiation for Earth's measured Bond albedo, 0.294, through the passes' greenhouse and
/// partition (`atmosphere_under` on the passes' stand-in surface), is 288 ± 2 K: the solver's fixed
/// point against the fit's closed form, which puts it at 288.0 K. Its carbon dioxide and nitrogen
/// are the table's Earth's.
///
/// The table's Earth through the batch's passes, at their stand-in surface (no basins, so no sea),
/// is recorded, not asserted (about 290.6 K at a Bond albedo of 0.2798): P14.T54.a asserts it at its
/// real inputs (decision-p14-t57a-fits §1.5).
#[test]
fn the_measured_earth_holds_288_k_under_the_batch() {
    let batch = table();
    let earth = found(&batch, "Earth");
    let equilibrium = at_measured_albedo(earth, SurfaceState::Temperate);
    let measured = atmosphere_under(
        &table_earth_inputs(earth, equilibrium),
        &PassSurface::stand_in(PassLight::HottestHost),
    );
    assert_eq!(measured.state(), SurfaceState::Temperate);
    for gas in [Gas::CarbonDioxide, Gas::Nitrogen] {
        let (p, table) = (
            measured.partial_pressures().of(gas).value(),
            earth.atmosphere().partial_pressures().of(gas).value(),
        );
        assert!(
            (p / table - 1.0).abs() < 1e-12,
            "{gas:?}: {p} Pa against {table}"
        );
    }
    let t = measured.surface_temperature().value();
    let generated = earth.atmosphere().surface_temperature().value();
    eprintln!(
        "the measured Earth: {t:.3} K at T_eq {:.4} K, water {:.3} Pa; the table's Earth through \
         the batch's passes {generated:.3} K at {:.5} (T_eq {:.4} K)",
        equilibrium.value(),
        measured.partial_pressures().of(Gas::Water).value(),
        earth.albedo().value(),
        earth.equilibrium_temperature().value(),
    );
    assert!(
        (t - EARTH_SURFACE_TEMPERATURE).abs() < 2.0,
        "the measured Earth at {t} K"
    );
}

/// P14.T57.a: the table's Venus is a median draw of the inventory, 55–60 bar, and under the
/// batch's constants it reaches 680–710 K, about 696 K, in place of the 735 K that version 22's
/// constant was fitted to give it (decision-p14-moons §4.2; decision-p14-t57a-fits §3).
#[test]
fn the_table_s_median_venus_holds_55_to_60_bar_and_reaches_680_to_710_k() {
    let batch = table();
    let air = found(&batch, "Venus").atmosphere();
    assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
    let bar = air.surface_pressure().unwrap().value() / PASCALS_PER_BAR;
    let t = air.surface_temperature().value();
    eprintln!("the table's Venus under the batch: {bar:.3} bar at {t:.3} K");
    assert!((55.0..60.0).contains(&bar), "{bar} bar");
    assert!((680.0..710.0).contains(&t), "{t} K");
}

/// P14.T57.a: under the batch the table's Titan is 94 ± 3 K. It is a check only: no constant is
/// refitted on Titan, whose Bond albedo lacks its haze until P14.T24.h (decision-p14-t51-batch
/// §9.5, E1), and a miss here is recorded for T24.h, not cured by moving the nitrogen constant.
/// Under the batch at 23's stand-ins it reaches 92.8 K at a Bond albedo of 0.536 (measured 0.265).
#[test]
fn under_the_batch_titan_is_94_k() {
    let batch = table();
    let titan = found(&batch, "Titan");
    let t = titan.atmosphere().surface_temperature().value();
    eprintln!(
        "under the batch: Titan {t:.3} K at a Bond albedo of {:.4}",
        titan.albedo().value()
    );
    assert!(
        (t - TITAN_SURFACE_TEMPERATURE).abs() < 3.0,
        "Titan at {t} K"
    );
}

// Decision-p14-t54a-ganymede, C0 follow-up: the runaway's water (rule (a)) and the ice-held
// volatiles (rule (b)).

/// Rule (a): a runaway world loses its whole water by photolysis and hydrogen escape, not by H₂O's
/// molecular Jeans test. A Mars-sized world inside the runaway limit at an irradiation of 305 K,
/// whose exobase of 1,369 K fails its water (λ ≈ 20) and passes its O₂ (λ ≈ 35.5), keeps
/// [`runaway_oxygen`]'s O₂ of its whole drawn water, less its sinks, in its air.
#[test]
fn a_runaway_world_whose_water_fails_its_jeans_test_keeps_the_oxygen_it_freed() {
    let (mass, radius) = (Kilograms::new(6.417e23), Metres::new(3.389_5e6));
    let (water, age, xuv) = (1e21, 1e8, 1.0);
    let air = atmosphere(&AtmosphereInputs {
        mass,
        radius,
        material: SurfaceMaterial::Rock,
        envelope_fraction: 0.0,
        class: PlanetClass::Rocky,
        inventory: VolatileInventory::new(
            Kilograms::new(water),
            Kilograms::new(1e19),
            Kilograms::new(1e17),
            Kilograms::new(1e16),
        ),
        formed: SnowLineSide::Inside,
        equilibrium: Kelvin::new(250.0),
        irradiation_temperature: Kelvin::new(305.0),
        heated: Kelvin::new(250.0),
        xuv_fluence: earth_xuv_fluence(),
        saturated_xuv_flux: WattsPerSquareMetre::new(xuv),
        insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: Years::new(age),
        hottest_host: Kelvin::new(5_772.0),
    });
    let exobase = air.exobase_temperature();
    let lambda = |gas: Gas| jeans_parameter(mass, radius, gas, exobase);
    eprintln!(
        "a runaway Mars at a {:.1} K exobase: λ(H₂O) {:.3}, λ(O₂) {:.3}",
        exobase.value(),
        lambda(Gas::Water),
        lambda(Gas::Oxygen)
    );
    assert!(lambda(Gas::Water) < JEANS_RETENTION_THRESHOLD);
    assert!(lambda(Gas::Oxygen) >= JEANS_RETENTION_THRESHOLD);
    assert!(!air.retention().retains(Gas::Water));
    assert!(air.retention().retains(Gas::Oxygen));
    assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
    let kept = runaway_oxygen(
        Kilograms::new(water),
        mass,
        radius,
        WattsPerSquareMetre::new(xuv),
        Years::ZERO,
        Years::new(age),
    );
    assert!(kept > Kilograms::ZERO, "{kept:?}");
    let g = GRAVITATIONAL_CONSTANT * mass.value() / math::powi(radius.value(), 2);
    let weight = g / (4.0 * core::f64::consts::PI * math::powi(radius.value(), 2));
    let o2 = air.partial_pressures().of(Gas::Oxygen).value();
    eprintln!("its O₂: {:.3} bar", o2 / PASCALS_PER_BAR);
    assert!(
        (o2 / (kept.value() * weight) - 1.0).abs() < 1e-12,
        "{o2} Pa"
    );
    assert_same_bits(air.partial_pressures().of(Gas::Water).value(), 0.0);
}

/// Rule (b)'s escape, [`ice_vapour_escape`], is Jeans's flux at the ice's saturated vapour over the
/// whole surface and the age (Schaller and Brown 2007's criterion, the exobase at the surface):
///
/// - at Triton (JPL's GM, 1,428.5 km³ s⁻², and mean radius, 1,352.6 km) and 32.5 K its N₂ ice's
///   vapour is about 0.04 Pa (0.0435, the registry's Fray and Schmitt α-N₂; the ruling's "some
///   0.02 Pa" is half of it) and λ about 109.5 (the ruling's), and the loss is the closed form to
///   10⁻¹², a negligible share of any inventory;
/// - it holds nothing where the vapour would be a bulk air: N₂ at 70 K, above its triple point,
///   and at 130 K, above its critical point, and water at 300 K; nor at a temperature that is not
///   positive;
/// - a flux that underflows, of N₂ on an Earth at 30 K (λ ≈ 7,000), loses nothing, nor does water
///   at the least subnormal temperature, whose λ is infinite.
#[test]
fn the_ice_s_vapour_escapes_by_jeans_s_flux_with_the_exobase_at_the_surface() {
    let (mass, radius) = (
        Kilograms::new(1_428.495e9 / GRAVITATIONAL_CONSTANT),
        Metres::new(1_352.6e3),
    );
    let t = Kelvin::new(32.5);
    let vapour = saturation_pressure(Gas::Nitrogen.substance(), t)
        .unwrap()
        .value();
    let lambda = jeans_parameter(mass, radius, Gas::Nitrogen, t);
    let loss = ice_vapour_escape(mass, radius, Gas::Nitrogen, t, SOLAR_AGE)
        .unwrap()
        .value();
    let m = Gas::Nitrogen.molecular_mass().value();
    let kt = BOLTZMANN_CONSTANT * t.value();
    let flux = vapour / kt * (2.0 * kt / m).sqrt() * (1.0 + lambda) * math::exp(-lambda)
        / (2.0 * core::f64::consts::PI.sqrt());
    let by_hand = 4.0
        * core::f64::consts::PI
        * math::powi(radius.value(), 2)
        * m
        * SOLAR_AGE.value()
        * SECONDS_PER_JULIAN_YEAR
        * flux;
    eprintln!(
        "Triton at 32.5 K: N₂'s vapour {vapour:.4} Pa, λ {lambda:.2}, loss {loss:.3e} kg over \
         the age"
    );
    assert!((0.04..0.05).contains(&vapour), "{vapour} Pa");
    assert!((lambda - 109.5).abs() < 0.5, "{lambda}");
    assert!(
        (loss / by_hand - 1.0).abs() < 1e-12,
        "{loss} against {by_hand}"
    );
    assert!(loss < 1e-20 * mass.value(), "{loss}");
    for (gas, k) in [
        (Gas::Nitrogen, 70.0),
        (Gas::Nitrogen, 130.0),
        (Gas::Water, 300.0),
        (Gas::Nitrogen, 0.0),
        (Gas::Nitrogen, f64::NAN),
    ] {
        let held = ice_vapour_escape(mass, radius, gas, Kelvin::new(k), SOLAR_AGE);
        assert_eq!(held, None, "{gas:?} at {k} K");
    }
    let earth = ice_vapour_escape(
        Kilograms::new(EARTH_MASS_KG),
        Metres::new(crate::units::consts::EARTH_RADIUS_M),
        Gas::Nitrogen,
        Kelvin::new(30.0),
        SOLAR_AGE,
    );
    assert_same_bits(earth.unwrap().value(), 0.0);
    // 2⁻¹⁰⁷⁴, the least subnormal.
    let subnormal = Kelvin::new(f64::MIN_POSITIVE * f64::EPSILON);
    assert!(jeans_parameter(mass, radius, Gas::Water, subnormal).is_infinite());
    let bound = ice_vapour_escape(mass, radius, Gas::Water, subnormal, SOLAR_AGE);
    assert_same_bits(bound.unwrap().value(), 0.0);
}

/// Rule (b) leaves small bodies their loss (Schaller and Brown 2007, Fig. 1): a Kuiper-belt body
/// 500 km in radius at 1,900 kg m⁻³, an ice crust at 40 K, has λ(N₂) about 11 at its surface,
/// so its N₂ ice's vapour, some pascals, escapes many times its whole inventory over the age, and
/// it keeps no N₂, nor the methane that follows it. Its water ice, whose vapour at 40 K is nil,
/// stays.
#[test]
fn a_500_km_body_at_40_k_keeps_no_nitrogen() {
    let r = 5e5;
    let kg = 4.0 / 3.0 * core::f64::consts::PI * r * r * r * 1_900.0;
    let (mass, radius) = (Kilograms::new(kg), Metres::new(r));
    let t = Kelvin::new(40.0);
    let nitrogen = Kilograms::new(100.0 * EARTH_NITROGEN_PER_MASS * kg);
    let air = atmosphere(&AtmosphereInputs {
        mass,
        radius,
        material: SurfaceMaterial::Ice,
        envelope_fraction: 0.0,
        class: PlanetClass::Icy,
        inventory: VolatileInventory::new(
            Kilograms::new(0.4 * kg),
            Kilograms::new(100.0 * EARTH_CARBON_DIOXIDE_PER_MASS * kg),
            nitrogen,
            Kilograms::ZERO,
        ),
        formed: SnowLineSide::Beyond,
        equilibrium: t,
        irradiation_temperature: Kelvin::new(42.0),
        heated: t,
        xuv_fluence: JoulesPerSquareMetre::ZERO,
        saturated_xuv_flux: WattsPerSquareMetre::ZERO,
        insolation: Insolation::BeyondMaximumGreenhouse,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
    });
    let loss = ice_vapour_escape(mass, radius, Gas::Nitrogen, t, SOLAR_AGE).unwrap();
    let vapour = saturation_pressure(Gas::Nitrogen.substance(), t).unwrap();
    let lambda = jeans_parameter(mass, radius, Gas::Nitrogen, t);
    eprintln!(
        "a 500 km body at 40 K: N₂'s vapour {:.3} Pa, λ {lambda:.2}, loss {:.3e} of {:.3e} kg",
        vapour.value(),
        loss.value(),
        nitrogen.value()
    );
    assert!((10.0..13.0).contains(&lambda), "{lambda}");
    assert!(loss.value() > 1e3 * nitrogen.value(), "{loss:?}");
    assert!(!air.retention().retains(Gas::Nitrogen));
    assert!(!air.ice_held().retains(Gas::Nitrogen));
    assert!(!air.retention().retains(Gas::Methane));
    assert_same_bits(air.inventory_weights().of(Gas::Nitrogen).value(), 0.0);
    assert_same_bits(air.inventory_weights().of(Gas::Methane).value(), 0.0);
    assert!(air.ice_held().retains(Gas::Water));
    assert_eq!(air.state(), SurfaceState::Airless);
}

// Decision-p14-t54a-ganymede, "Remnant ice-bulk bodies' water" (2026-10-21): a past runaway
// leaves an ice crust the water its ice holds.

/// The inputs of an ice bulk like the sample's 41ffecb200000005.e605, a belt member of
/// 2.34 × 10⁻³ M⊕ and 0.189 R⊕ about a stellar remnant, at 5 Gyr and the equilibrium temperature
/// `equilibrium` K. It lay inside the runaway limit at its host's peak, an irradiation of 370 K
/// (the ruling's 3.1 S⊕, 40 au from 5 × 10³ L☉), so that its exobase of some 1,660 K fails its
/// water (λ ≈ 1.0) and its O₂ (λ ≈ 1.8). Its bulk is 37% water, as the member's is.
fn remnant_ice_bulk(equilibrium: f64) -> AtmosphereInputs {
    let kg = 2.34e-3 * EARTH_MASS_KG;
    AtmosphereInputs {
        mass: Kilograms::new(kg),
        radius: Metres::new(0.189 * crate::units::consts::EARTH_RADIUS_M),
        material: SurfaceMaterial::Ice,
        envelope_fraction: 0.0,
        class: PlanetClass::Icy,
        inventory: VolatileInventory::new(
            Kilograms::new(0.37 * kg),
            Kilograms::new(100.0 * EARTH_CARBON_DIOXIDE_PER_MASS * kg),
            Kilograms::new(100.0 * EARTH_NITROGEN_PER_MASS * kg),
            Kilograms::ZERO,
        ),
        formed: SnowLineSide::Beyond,
        equilibrium: Kelvin::new(equilibrium),
        irradiation_temperature: Kelvin::new(370.0),
        heated: Kelvin::new(equilibrium),
        xuv_fluence: earth_xuv_fluence(),
        saturated_xuv_flux: WattsPerSquareMetre::new(1.0),
        insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: Years::new(5e9),
        hottest_host: Kelvin::new(10_000.0),
    }
}

/// The figures of the body `inputs` describe, whose atmosphere is `air`, turning as Ceres does,
/// its crater contract a planet's on a circular orbit at 1 au under the Solar System's sources,
/// as `table_figures` gives the table's bodies theirs.
fn remnant_figures(
    inputs: &AtmosphereInputs,
    air: &Atmosphere,
    spin: &crate::planetary::hooks::surface::SeasonalSpin,
) -> crate::planetary::hooks::figures::GlobalFigures {
    use crate::planetary::hooks::figures::relief::EARTH_CONTINENTAL_FRACTION;
    use crate::planetary::hooks::figures::tests::solar_sources;
    use crate::planetary::hooks::figures::{
        Continents, CraterInputs, FiguresInputs, GlobalFigures, HostOrbit, HostOrbiter,
        ImpactorOrbit, Subsurface,
    };
    use crate::units::{Dex, KilogramsPerCubicMetre};
    let (kg, r) = (inputs.mass.value(), inputs.radius.value());
    let mass = EarthMasses::new(kg / EARTH_MASS_KG);
    GlobalFigures::derive(&FiguresInputs {
        atmosphere: air,
        mass,
        radius: inputs.radius,
        surface_gravity: MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * kg / (r * r)),
        class: PlanetClass::Icy,
        fractions: MassFractions::solid(0.0, 0.63, 0.37),
        inventory_water: inputs.inventory.water(),
        rotation_period: spin.rotation_period,
        fe_h: Dex::new(0.0),
        age: inputs.age,
        tidal_heat_flow: WattsPerSquareMetre::new(0.0),
        subsurface_ocean: Subsurface::NotComputed,
        craters: CraterInputs {
            orbit: ImpactorOrbit::Host(HostOrbiter {
                orbit: HostOrbit {
                    semi_major_axis: Metres::new(METRES_PER_AU),
                    eccentricity: 0.0,
                    inclination: Radians::ZERO,
                },
                mass,
                belt: None,
            }),
            density: KilogramsPerCubicMetre::new(
                kg / (4.0 / 3.0 * core::f64::consts::PI * r * r * r),
            ),
            sources: solar_sources(),
            molten_until: None,
            last_giant_impact: None,
        },
        continents: Continents::Given(EARTH_CONTINENTAL_FRACTION),
    })
    .expect("the remnant's ice bulk is in range")
    .expect("an airless ice bulk has a surface")
}

/// What the record's path gives a body ([`remnant_record`]).
struct RemnantRecord {
    figures: crate::planetary::hooks::figures::GlobalFigures,
    conditions: crate::planetary::hooks::surface::SurfaceConditions,
    partition: crate::planetary::chem::partition::Partition,
    published: crate::planetary::chem::partition::Condensates,
}

/// The record's path for the body `inputs` describe, whose atmosphere is `air`, turning as Ceres
/// does: its figures ([`remnant_figures`]), its conditions under the flux that holds its
/// equilibrium temperature at its albedo, and its partition and published condensates
/// (`condensates_under`, the cold traps held, under the Moon's sky).
fn remnant_record(inputs: &AtmosphereInputs, air: &Atmosphere) -> RemnantRecord {
    use crate::planetary::hooks::cold_traps::{ColdTrapRules, RecordColdTraps, lunar_sky};
    use crate::planetary::hooks::surface::tests::spin_of;
    use crate::planetary::hooks::surface::{SurfaceConditions, SurfaceInputs};
    use crate::units::consts::STEFAN_BOLTZMANN;
    let spin = spin_of("Ceres");
    let figures = remnant_figures(inputs, air, &spin);
    let r = inputs.radius.value();
    let gravity =
        MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * inputs.mass.value() / (r * r));
    let albedo = air.albedo().value();
    let flux = 4.0 * STEFAN_BOLTZMANN * math::powi(inputs.equilibrium.value(), 4) / (1.0 - albedo);
    let conditions = SurfaceConditions::derive(&SurfaceInputs {
        atmosphere: air,
        equilibrium_temperature: inputs.equilibrium,
        surface_gravity: gravity,
        radius: inputs.radius,
        material: SurfaceMaterial::Ice,
        lid: None,
        stellar_flux: WattsPerSquareMetre::new(flux),
        xuv_flux: WattsPerSquareMetre::ZERO,
        spin,
        heat_flow: figures.heat_flow(),
        ocean_weight: figures.ocean_weight(),
    })
    .expect("it has a surface");
    let table = conditions.zonal_insolation();
    let sky = lunar_sky();
    let traps = RecordColdTraps::at_the_albedo_of(ColdTrapRules::Held, &sky, air);
    let (partition, published) =
        figures.condensates_under(air, &conditions, gravity, &traps, &table);
    RemnantRecord {
        figures,
        conditions,
        partition,
        published,
    }
}

/// A past runaway takes an ice crust's surface layer, not its bulk (Jura and Xu 2010; Malamud and
/// Perets 2016): an ice bulk like e605 at 3.2 K, inside the runaway limit at its remnant host's
/// peak, whose water fails Jeans's test, keeps the water its ice holds (rule (b)). Its column holds
/// its inventory less its ice's vapour's escape, and on the record's path (its figures and
/// conditions and `condensates_under`) its partition's frozen water is that column over its
/// gravity, all of it, to the bit, listed as its `H2O` solid at the figures' T24.b ice area. Its
/// state is the one its lost water gave it, `Airless`, at the same temperature, and its O₂, which
/// fails Jeans's test, stays unretained (rule (a)'s oxygen reads the whole inventory still).
#[test]
fn a_past_runaway_leaves_an_ice_bulk_the_water_its_ice_holds() {
    use crate::planetary::chem::partition::CondensatePhase;
    use crate::planetary::hooks::figures::cover;
    let inputs = remnant_ice_bulk(3.2);
    let (mass, radius) = (inputs.mass, inputs.radius);
    let air = atmosphere(&inputs);
    let exobase = air.exobase_temperature();
    let lambda = |gas: Gas| jeans_parameter(mass, radius, gas, exobase);
    let water = inputs.inventory.water().value();
    let loss = ice_vapour_escape(mass, radius, Gas::Water, inputs.equilibrium, inputs.age)
        .expect("water ice's vapour at 3.2 K is no air");
    let kept = water - loss.value();
    eprintln!(
        "a remnant's ice bulk at {:.1} K: exobase {:.1} K, λ(H₂O) {:.3}, λ(O₂) {:.3}; water \
         {water:.4e} kg, its ice's loss {:.3e} kg",
        inputs.equilibrium.value(),
        exobase.value(),
        lambda(Gas::Water),
        lambda(Gas::Oxygen),
        loss.value()
    );
    assert!(lambda(Gas::Water) < JEANS_RETENTION_THRESHOLD);
    assert!(lambda(Gas::Oxygen) < JEANS_RETENTION_THRESHOLD);
    assert!(!Retention::of(mass, radius, exobase).retains(Gas::Water));
    assert!(air.ice_held().retains(Gas::Water), "{air:?}");
    assert!(air.retention().retains(Gas::Water), "{air:?}");
    assert!(!air.retention().retains(Gas::Oxygen), "{air:?}");
    assert!(kept > 0.0, "{kept}");
    let (m, r) = (mass.value(), radius.value());
    let g = GRAVITATIONAL_CONSTANT * m / (r * r);
    let weight = g / (4.0 * core::f64::consts::PI * r * r);
    assert_same_bits(
        air.inventory_weights().of(Gas::Water).value(),
        kept * weight,
    );
    assert_same_bits(air.inventory_weights().of(Gas::Oxygen).value(), 0.0);
    // The state is the one the lost water gave: that of the same body with no water.
    let mut dry = inputs;
    dry.inventory = VolatileInventory::new(
        Kilograms::ZERO,
        inputs.inventory.carbon_dioxide(),
        inputs.inventory.nitrogen(),
        inputs.inventory.argon(),
    );
    let dry = atmosphere(&dry);
    assert_eq!(air.state(), SurfaceState::Airless, "{air:?}");
    assert_eq!(air.state(), dry.state());
    assert_same_bits(
        air.surface_temperature().value(),
        dry.surface_temperature().value(),
    );

    // The record's path.
    let RemnantRecord {
        figures,
        conditions,
        partition,
        published,
    } = remnant_record(&inputs, &air);
    let ice = cover::ice_fraction(&cover::IceInputs {
        conditions: &conditions,
        state: air.state(),
        crust: figures.impactors().crust(),
        surface_water: figures.surface_water(),
    });
    let at = format!(
        "at a mean {:.2} K: T24.b's ice {ice}, {:?}",
        conditions.mean_surface_temperature().value(),
        published.entries()
    );
    eprintln!("{at}");
    let frozen = partition
        .condensate(Gas::Water.substance())
        .unwrap_or_else(|| panic!("{at}: the partition holds the crust's water"));
    assert_eq!(frozen.phase(), CondensatePhase::Solid, "{at}");
    assert_same_bits(frozen.reservoir().value(), kept * weight / g);
    let entry = published
        .entry(Gas::Water.substance(), CondensatePhase::Solid)
        .unwrap_or_else(|| panic!("{at}: no H2O solid entry"));
    assert_same_bits(entry.reservoir().value(), frozen.reservoir().value());
    assert_same_bits(entry.area_fraction(), ice);
    assert!(ice > 0.0, "{at}");
    assert!(
        published
            .entry(Gas::Water.substance(), CondensatePhase::Liquid)
            .is_none(),
        "{at}"
    );
    // T24.b's surface water stays on water's Jeans verdict: none.
    assert_same_bits(figures.surface_water().value(), 0.0);
}

/// The ice holds no water whose vapour would be an air: the same ice bulk inside the runaway limit
/// at an equilibrium temperature of 300 K, where ice Ih's vapour is some 3.5 kPa, keeps none, as
/// before the remnant ruling, and its water's column is nil.
#[test]
fn a_warm_ice_bulk_inside_the_runaway_limit_keeps_no_water() {
    let inputs = remnant_ice_bulk(300.0);
    let air = atmosphere(&inputs);
    let held = ice_vapour_escape(
        inputs.mass,
        inputs.radius,
        Gas::Water,
        inputs.equilibrium,
        inputs.age,
    );
    assert_eq!(held, None);
    assert!(!air.ice_held().retains(Gas::Water), "{air:?}");
    assert!(!air.retention().retains(Gas::Water), "{air:?}");
    assert_same_bits(air.inventory_weights().of(Gas::Water).value(), 0.0);
    assert_same_bits(air.partial_pressures().of(Gas::Water).value(), 0.0);
    assert_eq!(air.state(), SurfaceState::Airless, "{air:?}");
}

// Lever (p) (decision-p14-t54a-passes, "Cost escalation (after Batch B)"): the greenhouse's
// bit-exact exit, and a column with nothing to condense reading no step climate.

/// Lever (p)'s exit from the greenhouse steps: they stop after a step whose temperature and
/// partial pressures are the step before's, bit for bit, and the greenhouse is the full count's to
/// the bit. Stub airs (a temperate Earth, a dry Earth beyond the maximum-greenhouse limit, Titan, a
/// cold Titan-sized moon, a runaway Earth and the remnant ice bulk at 3.2 and 300 K) and the
/// table's bodies, derived through their passes, their pass counts included, are the same with
/// the exits and without, every greenhouse checked to the bit as it is made (`Exits::Checked`).
#[test]
fn the_greenhouse_stops_at_its_bit_exact_fixed_point() {
    use crate::planetary::chem::partition::{ExitCounts, Exits, with_exits};

    let run = || -> Vec<String> {
        let airs = [
            atmosphere(&earth_inputs(255.0, Insolation::Habitable)),
            atmosphere(&earth_inputs(165.6, Insolation::BeyondMaximumGreenhouse)),
            atmosphere(&titan_inputs()),
            cold_moon(1e19, 94.0, SnowLineSide::Beyond, Crust::Solid),
            runaway_earth(1.4e21, 4.57e9, 1.0, 300.0),
            atmosphere(&remnant_ice_bulk(3.2)),
            atmosphere(&remnant_ice_bulk(300.0)),
        ];
        let mut out: Vec<String> = airs.iter().map(|air| format!("{air:?}")).collect();
        out.extend(
            table()
                .iter()
                .map(|(name, body)| format!("{name}: {body:?}")),
        );
        out
    };
    let (checked, counts) = with_exits(Exits::Checked, run);
    let (unlevered, uncounted) = with_exits(Exits::Unlevered, run);
    assert_eq!(uncounted, ExitCounts::default());
    assert_eq!(checked.len(), unlevered.len());
    for (with, without) in checked.iter().zip(&unlevered) {
        assert_eq!(with, without);
    }
    let most = counts.greenhouses * u64::from(GREENHOUSE_STEPS + 1);
    eprintln!(
        "{} greenhouses checked, {} evaluations of {most} at most: {counts:?}",
        counts.checked_greenhouses, counts.evaluations
    );
    assert_eq!(counts.checked_greenhouses, counts.greenhouses);
    assert!(counts.greenhouses > 20, "{counts:?}");
    assert!(counts.evaluations < most, "{counts:?}");
}

/// Lever (p)'s third part: a column with nothing that condenses is partitioned the same over every
/// climate, so its greenhouse reads none and takes the mean's isothermal span in its place, and the
/// atmosphere is the same, bit for bit, as when every step reads its climate. The remnant ice bulk
/// at 300 K keeps nothing, so all 33 of its step climates go and only the area mix's remain. An
/// Earth, whose column condenses, reads its climate at every step it takes.
#[test]
fn a_column_with_nothing_to_condense_reads_no_step_climate() {
    use crate::planetary::chem::partition::{Exits, with_exits};

    let calls = core::cell::Cell::new(0_u32);
    let climate = |t: Kelvin, _: &PartialPressures| {
        calls.set(calls.get() + 1);
        ClimateSpan::isothermal(t)
    };
    let surface = PassSurface {
        climate: PassClimate::Given(&climate),
        ..PassSurface::stand_in(PassLight::HottestHost)
    };
    let read = |inputs: &AtmosphereInputs, exits: Exits| {
        calls.set(0);
        let (air, counts) = with_exits(exits, || atmosphere_under(inputs, &surface));
        (format!("{air:?}"), calls.get(), counts)
    };
    let empty = remnant_ice_bulk(300.0);
    let (taken, taken_calls, counts) = read(&empty, Exits::Taken);
    let (unlevered, unlevered_calls, _) = read(&empty, Exits::Unlevered);
    eprintln!(
        "the empty column: {taken_calls} climates read with the exits, {unlevered_calls} without; \
         {counts:?}"
    );
    assert_eq!(taken, unlevered);
    assert_eq!(counts.greenhouses, 1);
    assert_eq!(counts.climates, 0);
    assert_eq!(unlevered_calls, taken_calls + GREENHOUSE_STEPS + 1);
    let earth = earth_inputs(255.0, Insolation::Habitable);
    let (taken, taken_calls, counts) = read(&earth, Exits::Taken);
    let (unlevered, unlevered_calls, _) = read(&earth, Exits::Unlevered);
    eprintln!(
        "an Earth: {taken_calls} climates read with the exits, {unlevered_calls} without; \
         {counts:?}"
    );
    assert_eq!(taken, unlevered);
    assert_eq!(counts.climates, counts.evaluations);
    assert!(taken_calls <= unlevered_calls);
}
