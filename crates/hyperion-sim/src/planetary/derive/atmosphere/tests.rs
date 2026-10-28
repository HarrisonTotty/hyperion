use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::lcg::Lcg;

use super::*;
use crate::planetary::context::XUV_SATURATION_FGK;
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
        present_insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(3_000.0),
        impact_speed: MetresPerSecond::ZERO,
        xuv_saturation_time: XUV_SATURATION_FGK,
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(Kelvin::new(1_500.0)),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(Kelvin::new(1_500.0))),
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
        present_insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: Years::new(age),
        hottest_host: Kelvin::new(5_772.0),
        impact_speed: MetresPerSecond::ZERO,
        xuv_saturation_time: XUV_SATURATION_FGK,
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(Kelvin::new(equilibrium)),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(Kelvin::new(
            equilibrium,
        ))),
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
        present_insolation: Insolation::BeyondMaximumGreenhouse,
        crust,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
        impact_speed: MetresPerSecond::ZERO,
        xuv_saturation_time: XUV_SATURATION_FGK,
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(TITAN_IRRADIATION),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(TITAN_IRRADIATION)),
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
        present_insolation: Insolation::BeyondMaximumGreenhouse,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
        impact_speed: MetresPerSecond::ZERO,
        xuv_saturation_time: XUV_SATURATION_FGK,
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(Kelvin::new(60.0)),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(Kelvin::new(60.0))),
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
/// The bolometric flux of a body's irradiation temperature `irradiation`, 4σT⁴ (the flux at which
/// a black body's equilibrium temperature is `irradiation`), for hand inputs whose flux now is the
/// one their exobase reads ([`AtmosphereInputs::flux`]).
fn irradiation_flux(irradiation: Kelvin) -> EarthFluxes {
    EarthFluxes::from(WattsPerSquareMetre::new(
        4.0 * STEFAN_BOLTZMANN * math::powi(irradiation.value(), 4),
    ))
}

/// The ratio of the flux `flux` from a Sun-like host to its maximum-greenhouse limit's, Kopparapu
/// et al.'s `S_eff,max` at the Sun's effective temperature, for hand inputs about the Sun
/// ([`AtmosphereInputs::maximum_greenhouse_flux_ratio`]).
fn sun_maximum_ratio(flux: EarthFluxes) -> f64 {
    let limit = crate::planetary::derive::habitable_zone::HabitableLimit::MaximumGreenhouse.flux(
        Kelvin::new(crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K),
    );
    flux.value() / limit.value()
}

/// [`volatile_inventory`] at Earth's mass and fractions), at the equilibrium temperature
/// `equilibrium`, which its exobase reads as its irradiation, and against the habitable zone as
/// `insolation` puts it.
pub(crate) fn earth_inputs(equilibrium: f64, insolation: Insolation) -> AtmosphereInputs {
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
        present_insolation: insolation,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
        impact_speed: MetresPerSecond::ZERO,
        xuv_saturation_time: XUV_SATURATION_FGK,
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(Kelvin::new(equilibrium)),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(Kelvin::new(
            equilibrium,
        ))),
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
    // The stand-in's base is the larger of `T_eq` and the background, `T_eq` for every body here.
    let (retention, held) = retained(
        inputs,
        exobase,
        inputs.equilibrium,
        WattsPerSquareMetre::ZERO,
    );
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
        ceiling: GreenhouseCeiling::of(inputs),
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
    let held = IceHeld::of(
        &earth,
        retention,
        earth.equilibrium,
        WattsPerSquareMetre::ZERO,
    );
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
            GreenhouseConstants::FITTED.optical_depth(
                &partials,
                earth.equilibrium,
                MetresPerSecondSquared::new(weight_gravity),
            ),
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
        flux: earth.flux(),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(earth.flux()),
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

// P14.T53.a: the greenhouse's broadened carbon dioxide term and its refits, behind the escape's
// hold (`ESCAPE_VERSION`).

use crate::planetary::chem::escape::{EscapeRules, with_rules};
use crate::planetary::hooks::surface::tests::earth_conductance_fit;
use crate::planetary::hooks::surface::{
    BROADENED_EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K, EARTH_EQUATOR_POLE_CONTRAST_K,
    STANDARD_GRAVITY, WET_ALBEDO_GRADIENT,
};
use crate::units::consts::STEFAN_BOLTZMANN;

/// Mars's surface pressure, Pa: 636, the 6.36 mbar at its mean radius of NASA's Mars fact sheet
/// (Williams), the weight of its column, as the generator's partial pressures are weights.
const MARS_SURFACE_PRESSURE_PA: f64 = 636.0;

/// Mars's air by volume, NASA's Mars fact sheet: 95.1% carbon dioxide, 2.59% nitrogen, 1.94% argon
/// and 210 ppm water. Its 0.16% of oxygen and 0.06% of carbon monoxide, which enter no term, are
/// left out.
const MARS_AIR_BY_VOLUME: [(Gas, f64); 4] = [
    (Gas::CarbonDioxide, 0.951),
    (Gas::Nitrogen, 0.0259),
    (Gas::Argon, 0.0194),
    (Gas::Water, 210e-6),
];

/// Mars's measured Bond albedo, 0.250 (NASA's Mars fact sheet), at which the fit takes the
/// generator's irradiation (decision-p14-t57a-fits §1.3's rule).
const MARS_BOND_ALBEDO: f64 = 0.250;

/// Mars's surface temperature, the band constant's anchor: 214 K, NASA's Mars fact sheet's "~214
/// K", the emission mean ⟨T⁴⟩^¼, against Haberle's (2013) 202 K arithmetic mean, as Méndez and
/// Rivera-Valentín (2017, `ApJL` 837, L1, p. 4) quote them. It is the same kind of quantity as
/// Earth's 288 K ([`EARTH_SURFACE_TEMPERATURE`]), which the grey `T_s` is.
const MARS_SURFACE_TEMPERATURE: f64 = 214.0;

/// The effective radiative forcing of doubled carbon dioxide, W m⁻²: 3.93 ± 0.47, IPCC AR6 WG1
/// ch. 7, §7.3.2.1 ("The ERF from doubling CO2 (2×CO2) from the 1750 level (278 ppm …) is assessed
/// to be 3.93 ± 0.47 W m–2"), the band exponent's anchor at the measured Earth, whose air holds
/// that 278 ppm.
const DOUBLED_CARBON_DIOXIDE_FORCING_W_PER_M2: f64 = 3.93;

/// Water vapour's share of water's greenhouse, the rest its cloud's: 2 ÷ 3, Schmidt et al.'s (2010,
/// JGR 115, D20106) "water vapour … ∼50%" against "clouds (∼25%)" of the all-sky greenhouse
/// (decision-bump-plan-after-22, "T53.a's CO₂ form", C2).
const WATER_VAPOUR_SHARE: f64 = 2.0 / 3.0;

/// The greenhouse terms of P14.T53.a's water, vapour and cloud.
const WATER_TERMS: [Absorber; 2] = [Absorber::WaterVapour, Absorber::WaterCloud];

/// A measured state a greenhouse constant is fitted on: its partial pressures as column weights,
/// its equilibrium temperature under the generator's irradiation at its measured Bond albedo, its
/// surface gravity, and its measured surface temperature, K.
#[derive(Debug, Clone, Copy)]
struct MeasuredState {
    partials: PartialPressures,
    equilibrium: Kelvin,
    gravity: MetresPerSecondSquared,
    target: f64,
}

impl MeasuredState {
    /// The τ its target needs less every term but those of `absorbers` under `constants`.
    fn remainder(&self, constants: &GreenhouseConstants, absorbers: &[Absorber]) -> f64 {
        let others = constants
            .terms(&self.partials, self.equilibrium, self.gravity)
            .iter()
            .filter(|term| !absorbers.contains(&term.absorber()))
            .fold(0.0, |sum, term| sum + term.depth());
        needed_depth(self.target, self.equilibrium.value()) - others
    }

    /// The depth of its term of `absorber` under `constants`.
    fn depth_of(&self, constants: &GreenhouseConstants, absorber: Absorber) -> f64 {
        constants
            .terms(&self.partials, self.equilibrium, self.gravity)
            .iter()
            .find(|term| term.absorber() == absorber)
            .map_or(0.0, GreyTerm::depth)
    }

    /// The broadened term's band and pairs' variables, X and Y ([`broadened_variable`],
    /// [`collision_variable`]).
    fn variables(&self) -> (f64, f64) {
        (
            broadened_variable(&self.partials, self.gravity),
            collision_variable(&self.partials, self.gravity),
        )
    }

    /// The same state with its carbon dioxide doubled.
    fn doubled(&self) -> Self {
        Self {
            partials: PartialPressures::from_fn(|gas| {
                let p = self.partials.of(gas);
                if gas == Gas::CarbonDioxide {
                    Pascals::new(2.0 * p.value())
                } else {
                    p
                }
            }),
            ..*self
        }
    }
}

/// The measured Mars, given by hand: its column weights, `p_s` `x_i` `M_i` ÷ Σ `x_j` `M_j` over
/// [`MARS_AIR_BY_VOLUME`]'s gases at the registry's molar masses, which lay the whole of its
/// [`MARS_SURFACE_PRESSURE_PA`] on them, with the generator's irradiation for the table's Mars
/// `mars` at its measured Bond albedo ([`MARS_BOND_ALBEDO`]) and its gravity, at
/// [`MARS_SURFACE_TEMPERATURE`].
fn measured_mars(mars: &DerivedBody) -> MeasuredState {
    let weight = |&(gas, x): &(Gas, f64)| x * gas.molar_mass_g_per_mol();
    let total = MARS_AIR_BY_VOLUME
        .iter()
        .fold(0.0, |sum, share| sum + weight(share));
    MeasuredState {
        partials: PartialPressures::from_fn(|gas| {
            let share = MARS_AIR_BY_VOLUME.iter().find(|&&(g, _)| g == gas);
            Pascals::new(share.map_or(0.0, |share| {
                MARS_SURFACE_PRESSURE_PA * weight(share) / total
            }))
        }),
        equilibrium: equilibrium_temperature(
            mars.flux(),
            BondAlbedo::from_fraction(MARS_BOND_ALBEDO),
        ),
        gravity: mars.surface_gravity(),
        target: MARS_SURFACE_TEMPERATURE,
    }
}

/// The band's and the pairs' constants (a, b) at the band exponent `exponent` that hold the
/// anchors `venus` and `mars`, each its τ less its other terms A with its variables X and Y: the
/// linear solve of a Xⁿ + b Y = A at both.
fn band_and_collisions(
    (a_v, (x_v, y_v)): (f64, (f64, f64)),
    (a_m, (x_m, y_m)): (f64, (f64, f64)),
    exponent: f64,
) -> (f64, f64) {
    let (p_v, p_m) = (math::powf(x_v, exponent), math::powf(x_m, exponent));
    let det = p_m * y_v - p_v * y_m;
    ((a_m * y_v - a_v * y_m) / det, (p_m * a_v - p_v * a_m) / det)
}

/// The grey forcing of doubled carbon dioxide on the measured Earth `earth` under `constants`, W
/// m⁻²: σ `T_s`⁴ [1 ÷ (1 + ¾τ) − 1 ÷ (1 + ¾τ₂)] at its fixed `T_s`, τ its measured state's (the τ
/// its `T_s` needs) and τ₂ that with carbon dioxide's term at doubled `P_CO₂`, whose surface
/// pressure holds the added gas.
fn doubled_forcing(earth: &MeasuredState, constants: &GreenhouseConstants) -> f64 {
    let tau = needed_depth(earth.target, earth.equilibrium.value());
    let doubled = tau + earth.doubled().depth_of(constants, Absorber::CarbonDioxide)
        - earth.depth_of(constants, Absorber::CarbonDioxide);
    STEFAN_BOLTZMANN
        * math::powi(earth.target, 4)
        * (1.0 / (1.0 + 0.75 * tau) - 1.0 / (1.0 + 0.75 * doubled))
}

/// The broadened term's a, n and b on its three anchors at water's constants `water`
/// ([`BROADENED_CARBON_DIOXIDE_BAND`]): n by 200 bisections, so that the measured Earth's forcing of
/// doubled carbon dioxide is [`DOUBLED_CARBON_DIOXIDE_FORCING_W_PER_M2`], and at each n, a and b
/// that hold the measured Mars and the measured Venus.
fn carbon_dioxide_fit(
    [venus, mars, earth]: &[MeasuredState; 3],
    water: (f64, f64),
) -> (f64, f64, f64) {
    let trial = GreenhouseConstants::broadened((0.0, 0.5, 0.0), water);
    let anchor = |state: &MeasuredState| {
        (
            state.remainder(&trial, &[Absorber::CarbonDioxide]),
            state.variables(),
        )
    };
    let (at_venus, at_mars) = (anchor(venus), anchor(mars));
    assert!(
        at_venus.0 > 0.0 && at_mars.0 > 0.0,
        "no carbon dioxide τ to fit: Venus {at_venus:?}, Mars {at_mars:?}"
    );
    let at = |exponent: f64| {
        let (band, collisions) = band_and_collisions(at_venus, at_mars, exponent);
        let constants = GreenhouseConstants::broadened((band, exponent, collisions), water);
        (band, collisions, doubled_forcing(earth, &constants))
    };
    // The forcing rises with n: Earth's X lies above Mars's, so its band term grows with n.
    let (mut low, mut high) = (1e-3, 0.9);
    assert!(
        at(low).2 < DOUBLED_CARBON_DIOXIDE_FORCING_W_PER_M2
            && at(high).2 > DOUBLED_CARBON_DIOXIDE_FORCING_W_PER_M2,
        "the forcing does not bracket its anchor: {:?} to {:?}",
        at(low),
        at(high)
    );
    for _ in 0..200 {
        let middle = f64::midpoint(low, high);
        if at(middle).2 > DOUBLED_CARBON_DIOXIDE_FORCING_W_PER_M2 {
            high = middle;
        } else {
            low = middle;
        }
    }
    let exponent = f64::midpoint(low, high);
    let (band, collisions, _) = at(exponent);
    (band, exponent, collisions)
}

/// Water's constants fitted on the measured Earth `earth`, its water at its target's humidity,
/// under carbon dioxide's (a, n, b) `carbon_dioxide`: the τ it needs less the other terms, split
/// [`WATER_VAPOUR_SHARE`] to the vapour, over √(`p_w` ÷ 1 bar), and the rest to the cloud, over
/// its availability ([`water_cloud_availability`], 1 at Earth).
fn water_fit(earth: &MeasuredState, carbon_dioxide: (f64, f64, f64)) -> (f64, f64) {
    let trial = GreenhouseConstants::broadened(carbon_dioxide, (0.0, 0.0));
    let residual = earth.remainder(&trial, &WATER_TERMS);
    (
        WATER_VAPOUR_SHARE * residual
            / (earth.partials.of(Gas::Water).value() / PASCALS_PER_BAR).sqrt(),
        (1.0 - WATER_VAPOUR_SHARE) * residual
            / water_cloud_availability(&earth.partials, earth.gravity),
    )
}

/// The measured Venus, Mars and Earth under the rules in force, as the broadened term's chain
/// fits on them: the measured Venus ([`measured_venus`]) through the passes, at the irradiation
/// for 0.76, no water; the measured Mars by hand ([`measured_mars`]); and the table's Earth, its
/// water at the partition's humidity at [`EARTH_SURFACE_TEMPERATURE`] (checked at its own `T_s`),
/// at the irradiation for 0.294. Each at its own gravity.
fn escape_measured_states() -> [MeasuredState; 3] {
    let batch = table();
    let venus = measured_venus();
    let air = venus.atmosphere();
    assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
    assert_eq!(air.partial_pressures().of(Gas::Water), Pascals::ZERO);
    let venus_eq = at_measured_albedo(&venus, SurfaceState::RunawayGreenhouse);
    assert_same_bits(venus_eq.value(), venus.equilibrium_temperature().value());
    let venus = MeasuredState {
        partials: *air.partial_pressures(),
        equilibrium: venus_eq,
        gravity: venus.surface_gravity(),
        target: VENUS_SURFACE_TEMPERATURE,
    };
    let mars = measured_mars(found(&batch, "Mars"));
    let earth = found(&batch, "Earth");
    let air = earth.atmosphere();
    assert_eq!(air.state(), SurfaceState::Temperate);
    let own = water_weight_at_humidity(
        air.partial_pressures(),
        water_saturation(air.surface_temperature().value()),
    );
    let water = air.partial_pressures().of(Gas::Water).value();
    assert!(
        (water / own - 1.0).abs() < 1e-6,
        "{water} Pa against {own} Pa"
    );
    let at_target = water_weight_at_humidity(
        air.partial_pressures(),
        water_saturation(EARTH_SURFACE_TEMPERATURE),
    );
    let earth = MeasuredState {
        partials: PartialPressures::from_fn(|gas| {
            if gas == Gas::Water {
                Pascals::new(at_target)
            } else {
                air.partial_pressures().of(gas)
            }
        }),
        equilibrium: at_measured_albedo(earth, SurfaceState::Temperate),
        gravity: earth.surface_gravity(),
        target: EARTH_SURFACE_TEMPERATURE,
    };
    [venus, mars, earth]
}

/// The broadened term's (a, n, b) and water's (`k_v`, `τ_c`), fitted in their order on the measured
/// states `states` ([`escape_measured_states`]) and iterated to their fixed point, since Mars's
/// water terms read water's constants.
fn escape_chain(states: &[MeasuredState; 3]) -> ((f64, f64, f64), (f64, f64)) {
    let mut water = (
        BROADENED_GREENHOUSE_WATER_VAPOUR,
        BROADENED_GREENHOUSE_WATER_CLOUD,
    );
    let mut carbon_dioxide = carbon_dioxide_fit(states, water);
    for _ in 0..16 {
        let refit = water_fit(&states[2], carbon_dioxide);
        let settled = (refit.0 - water.0).abs() < 1e-12 && (refit.1 - water.1).abs() < 1e-12;
        water = refit;
        carbon_dioxide = carbon_dioxide_fit(states, water);
        if settled {
            break;
        }
    }
    (carbon_dioxide, water)
}

/// States each of the measured states `states` under `constants`, term by term.
fn state_the_measured(states: &[MeasuredState; 3], constants: &GreenhouseConstants) {
    for (name, state) in ["Venus", "Mars", "Earth"].into_iter().zip(states) {
        let terms = constants.terms(&state.partials, state.equilibrium, state.gravity);
        let (x, y) = state.variables();
        eprintln!(
            "the measured {name}: CO2 {:.6e} Pa, N2 {:.6e} Pa, H2O {:.6e} Pa, p_s {:.6e} Pa, g \
             {:.4} m/s2, X {x:.6e}, Y {y:.6e}, A_w {:.6}, T_eq {:.4} K, target {} K: tau {:.6} \
             (terms {:?})",
            state.partials.of(Gas::CarbonDioxide).value(),
            state.partials.of(Gas::Nitrogen).value(),
            state.partials.of(Gas::Water).value(),
            state.partials.total().value(),
            state.gravity.value(),
            water_cloud_availability(&state.partials, state.gravity),
            state.equilibrium.value(),
            state.target,
            constants.optical_depth(&state.partials, state.equilibrium, state.gravity),
            terms.map(|term| (term.absorber(), term.depth())),
        );
    }
    eprintln!(
        "the measured Earth's forcing of doubled CO2 {:.4} W/m2",
        doubled_forcing(&states[2], constants)
    );
}

/// Kopparapu et al.'s (2013, ApJ 765, 131) Fig. 5a: the outgoing infrared flux, W m⁻², of a 273 K
/// surface under 1 bar of nitrogen, its water saturated, at each carbon dioxide pressure, bar, as
/// the lane traced it (decision-bump-plan-after-22, "T53.a's CO₂ form", C1: "about 112 W m⁻² at 1
/// bar, 67 at 8 bar, flat near 66 beyond 10 bar").
const KOPPARAPU_OUTGOING_INFRARED: [(f64, f64); 9] = [
    (1.02, 111.5),
    (1.5, 101.5),
    (2.0, 93.8),
    (3.0, 83.2),
    (4.0, 77.0),
    (5.0, 72.9),
    (6.0, 70.3),
    (8.0, 67.5),
    (10.0, 66.3),
];

/// P14.T53.a: the broadened greenhouse's constants are their fits, in their chain's order
/// (decision-pre-23-review item 20(6); decision-p14-t54a-bands ruling 2;
/// decision-bump-plan-after-22, "T53.a's CO₂ form and H₂ CIA"), each at its body's measured state
/// and measured Bond albedo under the generator's own irradiation, recomputed here under the batch
/// from the code, so that a change to what the fits read fails here with the refit:
///
/// - nitrogen on Titan, unchanged (1.55 at 75.6 K);
/// - carbon dioxide's a Xⁿ + b Y on its three anchors ([`carbon_dioxide_fit`]): n on the measured
///   Earth's forcing of doubled carbon dioxide ([`DOUBLED_CARBON_DIOXIDE_FORCING_W_PER_M2`]), a on
///   the measured Mars ([`measured_mars`], at [`MARS_SURFACE_TEMPERATURE`], at the irradiation for
///   0.250, less its water's and nitrogen's terms) and b on the measured Venus ([`measured_venus`],
///   at [`VENUS_SURFACE_TEMPERATURE`], at the irradiation for 0.76, no water);
/// - water's vapour and cloud on the table's Earth, as P14.T57.a fits it: its carbon dioxide and
///   nitrogen Earth's, its water at the partition's humidity at [`EARTH_SURFACE_TEMPERATURE`], at
///   the irradiation for 0.294, the residual split [`WATER_VAPOUR_SHARE`] to the vapour;
/// - Mars's water terms read water's constants, so carbon dioxide's and water's fits are iterated
///   to their fixed point;
/// - then Φ⊕ and D⊕, at the committed constants, to Earth's 42 K contrast
///   ([`earth_conductance_fit`]): Φ⊕ in North, Cahalan and Coakley's balance, which reads none of
///   the chain and does not move, and D⊕ at the batch's measured `B_eff`⊕.
///
/// a and water's constants are held to half a unit in their second decimal, n to half a unit in
/// its fourth, b in its fifth, and Φ⊕ and D⊕ within 10⁻⁷, their Newton fits against the
/// bisection's within 10⁻⁹. Stated beside them: the measured states' terms, the fit's Mars
/// sensitivity (n at a Mars of 213 and of 215 K), and Kopparapu et al.'s check, which reads the
/// clear sky (the vapour, no cloud).
#[test]
fn under_the_escape_batch_the_greenhouse_constants_are_their_fits() {
    use crate::planetary::derive::photometry::area_mix::RayleighFloorRules;
    with_rules(EscapeRules::Batch, || {
        // Version 24's whole state: the escape's batch and the Rayleigh floor (P14.T24.h.a) both,
        // so that the fits stay the tripwire at 24 (decision-bump-plan-after-22 V3.2).
        assert_eq!(GreenhouseConstants::in_force(), GreenhouseConstants::BATCH);
        assert_eq!(RayleighFloorRules::in_force(), RayleighFloorRules::Floored);
        let states = escape_measured_states();
        let (carbon_dioxide, water) = escape_chain(&states);
        let (band, exponent, collisions) = carbon_dioxide;
        state_the_measured(
            &states,
            &GreenhouseConstants::broadened(carbon_dioxide, water),
        );
        for target in [213.0, 215.0] {
            let [venus, mars, earth] = states;
            let mars = MeasuredState { target, ..mars };
            let ((a, n, b), (k_v, tau_c)) = escape_chain(&[venus, mars, earth]);
            eprintln!(
                "a Mars of {target} K: a {a:.4}, n {n:.4}, b {b:.6}, k_v {k_v:.4}, tau_c {tau_c:.4}"
            );
        }
        state_kopparapu_s_check(&GreenhouseConstants::BATCH);
        let fit = earth_conductance_fit(&GreenhouseConstants::BATCH);
        eprintln!(
            "the fits: a {band:.6}, n {exponent:.6}, b {collisions:.7}, k_v {:.6} per root bar, \
             tau_c {:.6}; Phi {:.10} (the bisection's {:.10}); D {:.10} W/m2/K (the bisection's \
             {:.10}) at the measured Earth's gain {:.6}, B_P g {:.4}, B_eff {:.6} W/m2/K, 6D/B_eff \
             {:.4}",
            water.0,
            water.1,
            fit.albedo,
            fit.albedo_oracle,
            fit.conductance,
            fit.oracle,
            fit.gain,
            fit.planck * fit.gain,
            fit.effective,
            6.0 * fit.conductance / fit.effective,
        );
        assert!(band > 0.0 && collisions > 0.0, "a {band}, b {collisions}");
        assert!(
            (BROADENED_CARBON_DIOXIDE_BAND - band).abs() <= 0.005,
            "refit BROADENED_CARBON_DIOXIDE_BAND on the measured Mars: {band:.4}"
        );
        assert!(
            (BROADENED_CARBON_DIOXIDE_EXPONENT - exponent).abs() <= 0.000_05,
            "refit BROADENED_CARBON_DIOXIDE_EXPONENT on Earth's forcing of doubled carbon \
             dioxide: {exponent:.5}"
        );
        assert!(
            (BROADENED_CARBON_DIOXIDE_COLLISIONS - collisions).abs() <= 0.000_005,
            "refit BROADENED_CARBON_DIOXIDE_COLLISIONS on the measured Venus: {collisions:.6}"
        );
        assert!(
            (BROADENED_GREENHOUSE_WATER_VAPOUR - water.0).abs() <= 0.005,
            "refit BROADENED_GREENHOUSE_WATER_VAPOUR on Earth after carbon dioxide: {:.4}",
            water.0
        );
        assert!(
            (BROADENED_GREENHOUSE_WATER_CLOUD - water.1).abs() <= 0.005,
            "refit BROADENED_GREENHOUSE_WATER_CLOUD on Earth after carbon dioxide: {:.4}",
            water.1
        );
        for (fitted, oracle, committed, what) in [
            (
                fit.albedo,
                fit.albedo_oracle,
                WET_ALBEDO_GRADIENT,
                "WET_ALBEDO_GRADIENT",
            ),
            (
                fit.conductance,
                fit.oracle,
                BROADENED_EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K,
                "BROADENED_EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K",
            ),
        ] {
            assert!((fitted / oracle - 1.0).abs() < 1e-9, "{what}: {fit:?}");
            assert!(
                (fitted / committed - 1.0).abs() < 1e-7,
                "refit {what} to Earth's {EARTH_EQUATOR_POLE_CONTRAST_K} K: {fitted:.10}"
            );
        }
        // The batch's D⊕ is the one the measured Earth's column reads, every scaling 1 on it.
        assert!(
            (fit.read - BROADENED_EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K).abs() < 1e-15,
            "{fit:?}"
        );
    });
}

/// States Kopparapu et al.'s check under `constants` ([`KOPPARAPU_OUTGOING_INFRARED`]): at each of
/// their carbon dioxide pressures over 1 bar of nitrogen and a 273 K surface, its water saturated
/// by weight, at Earth's gravity, the broadened term's τ over the τ their outgoing flux needs less
/// nitrogen's and the vapour's terms, the clear sky's (no cloud).
fn state_kopparapu_s_check(constants: &GreenhouseConstants) {
    let saturation = water_saturation(273.0);
    let ratios: Vec<String> = KOPPARAPU_OUTGOING_INFRARED
        .iter()
        .map(|&(bar, outgoing)| {
            let mean = (bar * Gas::CarbonDioxide.molar_mass_g_per_mol()
                + Gas::Nitrogen.molar_mass_g_per_mol())
                / (bar + 1.0);
            let water = saturation * Gas::Water.molar_mass_g_per_mol() / mean;
            let state = MeasuredState {
                partials: PartialPressures::from_fn(|gas| match gas {
                    Gas::CarbonDioxide => Pascals::new(bar * PASCALS_PER_BAR),
                    Gas::Nitrogen => Pascals::new(PASCALS_PER_BAR),
                    Gas::Water => Pascals::new(water),
                    _ => Pascals::ZERO,
                }),
                equilibrium: Kelvin::new(math::powf(outgoing / STEFAN_BOLTZMANN, 0.25)),
                gravity: STANDARD_GRAVITY,
                target: 273.0,
            };
            let needed =
                state.remainder(constants, &[Absorber::CarbonDioxide, Absorber::WaterCloud]);
            format!(
                "{bar} bar: {:.3}",
                state.depth_of(constants, Absorber::CarbonDioxide) / needed
            )
        })
        .collect();
    eprintln!(
        "Kopparapu's check, model over needed CO2 tau: {}",
        ratios.join(", ")
    );
}

/// P14.T53.a: under the batch the measured Venus ([`measured_venus`], through the passes) is 737 K
/// ± 2%, the measured Mars ([`measured_mars`], by hand) is within 1 K of
/// [`MARS_SURFACE_TEMPERATURE`], the measured Earth through the passes' greenhouse and partition
/// (as `the_measured_earth_holds_288_k_under_the_batch` takes it) is 288 ± 2 K, and the table's
/// median Venus holds 590–700 K, P14.T57.a's 680–710 K restated (decision-bump-plan-after-22,
/// "T53.a's CO₂ form", C3), stated beside Robinson and Catling's (2012, ApJ 757, 104, §4.1)
/// adiabatic 676 K.
///
/// The table's Mars, Venus, Earth and Titan under the batch are stated beside the hold's, not
/// asserted: escape is not wired yet, so the table's Mars is still its 0.11 bar of nitrogen.
#[test]
fn under_the_escape_batch_the_measured_venus_mars_and_earth_hold() {
    let held = table();
    with_rules(EscapeRules::Batch, || {
        let batch = table();
        let venus = measured_venus().atmosphere().surface_temperature().value();
        let mars = measured_mars(found(&batch, "Mars"));
        let depth = GreenhouseConstants::in_force().optical_depth(
            &mars.partials,
            mars.equilibrium,
            mars.gravity,
        );
        let mars_t = grey_surface_temperature(mars.equilibrium, depth).value();
        let earth = found(&batch, "Earth");
        let at = at_measured_albedo(earth, SurfaceState::Temperate);
        let measured = atmosphere_under(
            &table_earth_inputs(earth, at),
            &PassSurface::stand_in(PassLight::HottestHost),
        );
        assert_eq!(measured.state(), SurfaceState::Temperate);
        let earth_t = measured.surface_temperature().value();
        eprintln!(
            "under the escape's batch: the measured Venus {venus:.3} K, the measured Mars \
             {mars_t:.3} K (tau {depth:.6} at T_eq {:.4} K), the measured Earth {earth_t:.3} K \
             (water {:.3} Pa)",
            mars.equilibrium.value(),
            measured.partial_pressures().of(Gas::Water).value(),
        );
        for name in ["Mars", "Venus", "Earth", "Titan"] {
            let (was, now) = (
                found(&held, name).atmosphere(),
                found(&batch, name).atmosphere(),
            );
            let bar = |air: &Atmosphere| {
                air.surface_pressure()
                    .map_or(0.0, |p| p.value() / PASCALS_PER_BAR)
            };
            eprintln!(
                "the table's {name}: {:?} {:.3} K at {:.6} bar under the batch, {:?} {:.3} K at \
                 {:.6} bar under the hold",
                now.state(),
                now.surface_temperature().value(),
                bar(now),
                was.state(),
                was.surface_temperature().value(),
                bar(was),
            );
        }
        assert!(
            ((venus - VENUS_SURFACE_TEMPERATURE) / VENUS_SURFACE_TEMPERATURE).abs() < 0.02,
            "the measured Venus at {venus} K"
        );
        assert!(
            (mars_t - MARS_SURFACE_TEMPERATURE).abs() < 1.0,
            "the measured Mars at {mars_t} K"
        );
        assert!(
            (earth_t - EARTH_SURFACE_TEMPERATURE).abs() < 2.0,
            "the measured Earth at {earth_t} K"
        );
        let median = found(&batch, "Venus").atmosphere();
        let median_t = median.surface_temperature().value();
        eprintln!("the table's median Venus {median_t:.3} K, against the adiabat's 676 K");
        assert_eq!(median.state(), SurfaceState::RunawayGreenhouse);
        assert!((590.0..700.0).contains(&median_t), "{median_t} K");
    });
}

/// P14.T53.a's pin of the dry Earth's cure (decision-p14-t51a-partition §1;
/// decision-bump-plan-after-22, "T53.a's CO₂ form", C3): under the batch the Earth formed inside
/// the snow line at 2 au of a Sun, beyond the maximum greenhouse
/// (`a_dry_earth_beyond_the_maximum_greenhouse_still_runs_away_past_its_cold_trap`'s; Kopparapu et
/// al. 2013, arXiv:1301.6674v2, §3.3: `S_eff` 0.325 at about 8 bar of carbon dioxide, 1.70 au)
/// keeps from running away, its surface at or under 273.15 K. Stated: the passes' state, its carbon
/// dioxide airborne and condensed, by weight, its partial pressure against the saturation pressure
/// at its climate's coldest; then, as the pin's own test scans them, surfaces from 166 to 273 K with
/// their columns held by the partition over their own climates, and which of them the broadened
/// greenhouse, without the ceiling, holds at or below itself.
///
/// The maximum-greenhouse ceiling (decision-bump-plan-after-22, "T53.a's thick-air ceiling", K4) is
/// the cure: the dry Earth is a `Snowball` at its ceiling, `T_s` = max(273.15 K, `T_rest`) = 273.15
/// K (asserted at or under it), with 30.6 bar of its 70.1 bar of carbon dioxide airborne, at the
/// saturation pressure of its climate's coldest zone (267.9 K), and 39.5 bar condensed. Its
/// broadened greenhouse alone still holds no surface from 166 to 273 K at or below itself, each
/// warmed 21 K at least (196 K to 216.8 K) and 79 K at 266 K, so that without the ceiling it ran
/// away to 489.7 K with all its carbon dioxide airborne; K4 expected it under the ceiling, near 210
/// K. The ceiling is a bound, not the expected state: the true state is colder, its carbon dioxide
/// condensing aloft, C1's radiative–convective closure's (recorded, deferred). Like the census's
/// hot Snowballs of V3.9's item, which the ceiling closes (K6), it lies at the ceiling, not above
/// it.
#[test]
fn under_the_escape_batch_the_dry_earth_beyond_the_maximum_greenhouse_is_stated() {
    with_rules(EscapeRules::Batch, || {
        let earth = earth_inputs(165.6, Insolation::BeyondMaximumGreenhouse);
        let climate = |t: Kelvin, p: &PartialPressures| t24a_span(&earth, t, p);
        let air = atmosphere_under(
            &earth,
            &PassSurface {
                climate: PassClimate::Given(&climate),
                ..PassSurface::stand_in(PassLight::HottestHost)
            },
        );
        let t_s = air.surface_temperature();
        let co2 = Gas::CarbonDioxide;
        let (weight, inventory) = (
            air.partial_pressures().of(co2).value(),
            air.inventory_weights().of(co2).value(),
        );
        let span = t24a_span(&earth, t_s, air.partial_pressures());
        let partial = mole_fraction(&air, co2) * air.surface_pressure().map_or(0.0, Pascals::value);
        let trap =
            saturation_pressure(co2.substance(), span.coldest()).map_or(f64::NAN, Pascals::value);
        eprintln!(
            "the dry Earth at 2 au under the batch: {:?} {:.3} K, CO2 {weight:.4e} Pa of \
             {inventory:.4e} Pa by weight, its partial pressure {partial:.4e} Pa against p_sat \
             {trap:.4e} Pa at its coldest, {:.3} K; water {:.4e} Pa, tau {:.4}",
            air.state(),
            t_s.value(),
            span.coldest().value(),
            air.partial_pressures().of(Gas::Water).value(),
            air.optical_depth(),
        );
        let retention = air.retention();
        let held = IceHeld::of(
            &earth,
            retention,
            earth.equilibrium,
            WattsPerSquareMetre::ZERO,
        );
        let mut airborne = Airborne::kept(&earth, retention, held);
        let divided = airborne.divide(&earth, retention, held);
        assert_eq!(divided, SurfaceState::Snowball);
        let (m, r) = (earth.mass.value(), earth.radius.value());
        let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r));
        // The ceiling: max(273.15 K, T_rest), T_rest the grey surface of nitrogen's and hydrogen's
        // terms.
        let constants = GreenhouseConstants::in_force();
        let rest = constants
            .terms(air.partial_pressures(), earth.equilibrium, gravity)
            .iter()
            .filter(|term| matches!(term.absorber(), Absorber::Nitrogen | Absorber::Hydrogen))
            .fold(0.0, |sum, term| sum + term.depth());
        let t_rest = grey_surface_temperature(earth.equilibrium, rest).value();
        let cap = t_rest.max(WATER_MELTING_POINT.value());
        eprintln!(
            "  its ceiling {:?}: T_rest {t_rest:.3} K, the cap {cap:.3} K; CO2 airborne {:.3} bar, \
             condensed {:.3} bar",
            air.greenhouse_ceiling(),
            weight / PASCALS_PER_BAR,
            (inventory - weight) / PASCALS_PER_BAR,
        );
        assert_eq!(air.state(), SurfaceState::Snowball);
        assert_eq!(
            air.greenhouse_ceiling(),
            GreenhouseCeiling::MaximumGreenhouse
        );
        assert!(
            t_s.value() <= cap + 1e-9,
            "{} K over its cap {cap} K",
            t_s.value()
        );
        let column = airborne
            .column(gravity.value() / (4.0 * core::f64::consts::PI * r * r))
            .unwrap();
        let mut weights = AirborneWeights::new();
        for step in 0..=107 {
            let t = Kelvin::new(166.0 + f64::from(step));
            let mut partials = whole_column(&column);
            for _ in 0..8 {
                let span = t24a_span(&earth, t, &partials);
                partials = partitioned_partials(
                    &column,
                    &span,
                    &Ground::without_basins(gravity),
                    &mut weights,
                );
            }
            let warmed = grey_surface_temperature(
                earth.equilibrium,
                constants.optical_depth(&partials, earth.equilibrium, gravity),
            );
            if step % 10 == 0 || warmed <= t {
                eprintln!(
                    "  at {:.0} K: warmed to {:.3} K{}, CO2 {:.4e} Pa, water {:.4e} Pa",
                    t.value(),
                    warmed.value(),
                    if warmed <= t { " (holds itself)" } else { "" },
                    partials.of(co2).value(),
                    partials.of(Gas::Water).value(),
                );
            }
        }
    });
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
        present_insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: Years::new(age),
        hottest_host: Kelvin::new(5_772.0),
        impact_speed: MetresPerSecond::ZERO,
        xuv_saturation_time: XUV_SATURATION_FGK,
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(Kelvin::new(305.0)),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(Kelvin::new(305.0))),
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
        present_insolation: Insolation::BeyondMaximumGreenhouse,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(5_772.0),
        impact_speed: MetresPerSecond::ZERO,
        xuv_saturation_time: XUV_SATURATION_FGK,
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(Kelvin::new(42.0)),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(Kelvin::new(42.0))),
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
        present_insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: Years::new(5e9),
        hottest_host: Kelvin::new(10_000.0),
        impact_speed: MetresPerSecond::ZERO,
        xuv_saturation_time: XUV_SATURATION_FGK,
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(Kelvin::new(370.0)),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(Kelvin::new(370.0))),
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
        water_state: air.water_state(),
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

// P14.T53.a part C: the escape on the snapshot path, under the escape's batch rules.

use crate::planetary::chem::escape::{
    ESCAPE_ELEMENTS, EscapeOutcome, planet_impact_speed, shoreline_distance_dex,
};
use crate::planetary::derive::atmosphere::escaping::melt_oxygen_capacity;
use crate::planetary::hooks::surface::tests::{heat_flow_of, spin_of};
use crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K;

/// The escape the batch recorded on `air`.
fn escape_of(air: &Atmosphere) -> &EscapeOutcome {
    air.escape()
        .expect("under the escape's batch every body records its escape")
}

/// A one-line statement of the body `name`'s air `air`: its state, surface temperature and
/// pressure, its carbon dioxide's and nitrogen's weights, and its escape's shoreline.
fn air_line(name: &str, air: &Atmosphere) -> String {
    let shoreline = air.escape().map(|escape| *escape.shoreline());
    format!(
        "{name}: {:?} {:.3} K at {:.6e} Pa (CO2 {:.4e} Pa, N2 {:.4e} Pa, O2 {:.4e} Pa, H2 {:.4e} \
         Pa); shoreline {shoreline:?}",
        air.state(),
        air.surface_temperature().value(),
        air.surface_pressure().map_or(f64::NAN, Pascals::value),
        air.partial_pressures().of(Gas::CarbonDioxide).value(),
        air.partial_pressures().of(Gas::Nitrogen).value(),
        air.partial_pressures().of(Gas::Oxygen).value(),
        air.partial_pressures().of(Gas::Hydrogen).value(),
    )
}

/// The weight per kilogram of a body of mass `mass` and radius `radius`, Pa kg⁻¹: g ÷ 4πR², as its
/// partial pressures are taken.
fn weight_per_kg(mass: Kilograms, radius: Metres) -> f64 {
    let (m, r) = (mass.value(), radius.value());
    GRAVITATIONAL_CONSTANT * m / (r * r) / (4.0 * core::f64::consts::PI * r * r)
}

/// P14.T53.a: under the batch the table's Venus, Earth and Titan keep their airs, in the state they
/// had under the hold and within 10% of its surface pressure, and the cosmic shoreline keeps all
/// of each (decision-bump-plan-after-22, R3 Q2: a loss is a stop for main). Titan keeps over a bar
/// of its N₂ and its methane reservoir over the Solar System's age at its median carbon, both
/// stated with what escaped through its nitrogen and what its photolysis took.
#[test]
fn under_the_escape_batch_venus_earth_and_titan_keep_their_airs() {
    let held = table();
    let batch = with_rules(EscapeRules::Batch, table);
    for name in ["Venus", "Earth", "Titan"] {
        let (was, now) = (
            found(&held, name).atmosphere(),
            found(&batch, name).atmosphere(),
        );
        eprintln!("under the batch {}", air_line(name, now));
        eprintln!("under the hold  {}", air_line(name, was));
        assert!(now.keeps_atmosphere(), "{name}: {now:?}");
        assert_eq!(now.state(), was.state(), "{name}");
        let (p, q) = (
            now.surface_pressure().unwrap().value(),
            was.surface_pressure().unwrap().value(),
        );
        assert!((p / q - 1.0).abs() < 0.1, "{name}: {p} Pa against {q} Pa");
        assert!(escape_of(now).shoreline().kept() >= 1.0 - 1e-9, "{name}");
    }
    let titan = found(&batch, "Titan");
    let air = titan.atmosphere();
    let methane = escape_of(air).methane();
    let nitrogen = air.partial_pressures().of(Gas::Nitrogen).value();
    let (column, airborne) = (
        air.inventory_weights().of(Gas::Methane).value(),
        air.partial_pressures().of(Gas::Methane).value(),
    );
    let initial = methane.kept().value() + methane.escaped().value() + methane.photolysed().value();
    eprintln!(
        "the table's Titan under the batch: N2 {nitrogen:.4e} Pa; its methane reservoir {initial:.4e} \
         kg, of which {:.4e} kg escaped through its nitrogen, {:.4e} kg photolysed and {:.4e} kg \
         kept ({:.1}%), {column:.4e} Pa of it by weight with {airborne:.4e} Pa airborne",
        methane.escaped().value(),
        methane.photolysed().value(),
        methane.kept().value(),
        100.0 * methane.kept().value() / initial,
    );
    assert!(nitrogen >= 1e5, "{nitrogen} Pa");
    assert!(methane.kept().value() > 0.0, "{methane:?}");
    assert!(
        column > airborne,
        "a reservoir lies beside the air: {column} against {airborne}"
    );
}

/// P14.T53.a: under the batch Mercury and the Moon are airless, the cosmic shoreline keeping
/// none of their air (Mercury by its X-ray and ultraviolet fluence, the Moon, placed about the Sun
/// at Earth's distance, by its impacts at 12.5 times its escape speed), and the table's Ganymede,
/// placed about the Sun at Jupiter's distance, is airless (stated with its shoreline: about the Sun
/// it is struck at 4.9 times its escape speed, where the measured-moon fixture's Ganymede is struck
/// at 7.1, `moons::regular`'s test).
#[test]
fn under_the_escape_batch_mercury_the_moon_and_ganymede_are_airless() {
    let batch = with_rules(EscapeRules::Batch, table);
    for name in ["Mercury", "Moon", "Ganymede"] {
        let air = found(&batch, name).atmosphere();
        eprintln!("{}", air_line(name, air));
        assert_eq!(air.state(), SurfaceState::Airless, "{name}");
        if name != "Ganymede" {
            assert!(escape_of(air).shoreline().kept() < 1e-9, "{name}");
        }
    }
}

/// The table's Mars's inputs under the batch, at its derivation's bulk, inventory, fluence and
/// orbit (1.523 679 au, e 0.0934, about the Sun's mass), at the generator's irradiation for Mars's
/// measured Bond albedo ([`MARS_BOND_ALBEDO`]), as `measured_mars` takes it; its impact speed
/// Zahnle and Catling's eq. 41 at its circular orbital speed and its escape speed.
fn table_mars_inputs(mars: &DerivedBody) -> AtmosphereInputs {
    let (kg, radius) = (Kilograms::from(mars.mass()), Metres::from(mars.radius()));
    let (a, e) = (Metres::new(1.523_679 * METRES_PER_AU), 0.093_4);
    let sun = historic_sun();
    let orbital = (GRAVITATIONAL_CONSTANT * SOLAR_MASS_KG / a.value()).sqrt();
    let escape = (2.0 * GRAVITATIONAL_CONSTANT * kg.value() / radius.value()).sqrt();
    let equilibrium =
        equilibrium_temperature(mars.flux(), BondAlbedo::from_fraction(MARS_BOND_ALBEDO));
    AtmosphereInputs {
        mass: kg,
        radius,
        material: SurfaceMaterial::of(&mars.fractions()),
        envelope_fraction: mars.fractions().envelope(),
        class: mars.class(),
        inventory: *mars.inventory(),
        formed: mars.formed(),
        equilibrium,
        irradiation_temperature: equilibrium_temperature(
            mars.flux(),
            BondAlbedo::from_fraction(0.0),
        ),
        heated: equilibrium,
        xuv_fluence: mars.xuv_fluence(),
        saturated_xuv_flux: sun.xuv().saturated_flux(a, e),
        insolation: Insolation::Habitable,
        present_insolation: Insolation::Habitable,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K),
        impact_speed: planet_impact_speed(
            MetresPerSecond::new(orbital),
            MetresPerSecond::new(escape),
        ),
        xuv_saturation_time: sun.xuv().saturation_time(),
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: mars.flux(),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(mars.flux()),
    }
}

/// The table's Mars under the batch at its own spin: the table's derived Mars (through the batch's
/// passes at their stand-in), its inputs ([`table_mars_inputs`]), its air under P14.T24.a's
/// climate at its own spin (25.19°, 24.62 h; `hooks::surface`'s table) and its generated heat flow
/// at the irradiation for its measured Bond albedo, as a record derives a body under its law, and
/// that climate's body. The climate's body reads the batch's own column of all of Mars's eroded
/// carbon airborne (the stand-in pass's at the same albedo), the column the thermostat takes at
/// Mars, whose share is 1.
fn table_mars_at_its_spin() -> (DerivedBody, AtmosphereInputs, Atmosphere, ClimateBody) {
    let batch = table();
    let mars = *found(&batch, "Mars");
    let inputs = table_mars_inputs(&mars);
    let stand_in = atmosphere(&inputs);
    assert_eq!(
        escape_of(&stand_in).shoreline(),
        escape_of(mars.atmosphere()).shoreline(),
        "the inputs' shoreline is the derivation's"
    );
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    let body = ClimateBody {
        equilibrium_temperature: inputs.equilibrium,
        surface_gravity: MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r)),
        radius: inputs.radius,
        ground: GroundInertia::of_material(inputs.material),
        spin: spin_of("Mars"),
        heat_flow: heat_flow_of(&mars),
        inventory: *stand_in.inventory_weights(),
        ceiling: stand_in.greenhouse_ceiling(),
    };
    let climate = |t: Kelvin, p: &PartialPressures| step_climate(&body, OceanWeight::NONE, t, p);
    let air = atmosphere_under(
        &inputs,
        &PassSurface {
            climate: PassClimate::Given(&climate),
            ..PassSurface::stand_in(PassLight::HottestHost)
        },
    );
    (mars, inputs, air, body)
}

/// P14.T53.a (plan 14: "calibrated so that the table's Mars keeps 3–10 mbar of CO₂ and loses its
/// 0.11 bar of N₂"): under the batch the table's Mars at its own spin
/// ([`table_mars_at_its_spin`]) loses all but under 5% of its nitrogen's weight, the cosmic
/// shoreline keeping 0.0218 of its air (struck at 4.90 times its escape speed), and the weathering
/// thermostat takes all of its eroded carbon airborne, no share of it holding liquid water at its
/// warmest zone (R3 Q1 (3)); its carbon dioxide, its CO₂ ice and its surface temperature stated,
/// with the batch's passes' Mars at their stand-in, a climate with no contrast.
///
/// A finding for main (P14.T53.a's record): the 0.22 bar of carbon dioxide the shoreline leaves
/// does not cold-trap, so the table's Mars does not reach its 3–10 mbar
/// ([`under_the_escape_batch_the_table_s_mars_keeps_3_to_10_mbar_of_carbon_dioxide`]): at 0.22 bar
/// P14.T24.a's climate spans 230–251 K, where carbon dioxide's frost point is about 178 K, so the
/// cold trap that R3 Q1 (3) takes for Mars's end state (Leighton and Murray 1966) holds none, and
/// the greenhouse's steps, which start from the whole column, keep the thick air of the
/// bistability (Wordsworth 2015).
#[test]
fn under_the_escape_batch_the_table_s_mars_loses_its_nitrogen() {
    with_rules(EscapeRules::Batch, || {
        let (mars, inputs, air, body) = table_mars_at_its_spin();
        eprintln!(
            "through the batch's passes at their stand-in {}",
            air_line("Mars", mars.atmosphere())
        );
        let escape = escape_of(&air);
        let shoreline = escape.shoreline();
        let weight = weight_per_kg(inputs.mass, inputs.radius);
        let co2 = air.partial_pressures().of(Gas::CarbonDioxide).value();
        let column = air.inventory_weights().of(Gas::CarbonDioxide).value();
        let partial =
            mole_fraction(&air, Gas::CarbonDioxide) * air.surface_pressure().unwrap().value();
        let nitrogen = air.partial_pressures().of(Gas::Nitrogen).value();
        let nitrogen_inventory = inputs.inventory.nitrogen().value() * weight;
        let span = step_climate(
            &body,
            OceanWeight::NONE,
            air.surface_temperature(),
            air.partial_pressures(),
        );
        let frost = Gas::CarbonDioxide
            .substance()
            .substance()
            .phase_diagram()
            .and_then(|diagram| diagram.saturation_temperature(Pascals::new(partial)));
        let (m, r) = (inputs.mass.value(), inputs.radius.value());
        eprintln!(
            "the table's Mars under the batch at its own spin: {} ; CO2 {co2:.2} Pa by weight \
             ({partial:.2} Pa partial, its frost point {frost:?}), its column {column:.4e} Pa, so \
             {:.4e} Pa ({:.4e} kg) as CO2 ice; N2 {nitrogen:.3} Pa of its inventory's \
             {nitrogen_inventory:.3} Pa ({:.3}%); T_s {:.3} K (coldest {:.2} K, warmest {:.2} K); \
             the shoreline keeps {:.5} (xuv {:.5}, impact {:.5}, chronology {:.5}, ratio {:.3}), \
             the thermostat's share {}",
            air_line("", &air),
            column - co2,
            (column - co2) / weight,
            100.0 * nitrogen / nitrogen_inventory,
            air.surface_temperature().value(),
            span.coldest().value(),
            span.warmest().value(),
            shoreline.kept(),
            shoreline.xuv(),
            shoreline.impact(),
            shoreline.chronology(),
            inputs.impact_speed.value() / (2.0 * GRAVITATIONAL_CONSTANT * m / r).sqrt(),
            escape.airborne_carbon_share(),
        );
        assert!(nitrogen < 0.05 * nitrogen_inventory, "{nitrogen} Pa of N2");
        assert_same_bits(escape.airborne_carbon_share(), 1.0);
    });
}

/// P14.T53.a's Mars test as plan 14 states it, moved to P14.T53.c (main's ruling, 2026-10-22):
/// every 24 hold in force the table's Mars at its own spin ([`table_mars_at_its_spin`]) keeps 3–10
/// mbar of carbon dioxide by weight, lies within T13.c's 8% of 214 K, and holds the rest of its
/// carbon as CO₂ ice. It does not hold as built (the finding
/// [`under_the_escape_batch_the_table_s_mars_loses_its_nitrogen`] states): 0.22 bar at 243.9 K,
/// with no ice, against 3–10 mbar at about 214 K. The cause chain: the shoreline keeps 2.18% of the
/// air at an impact ratio of 4.90, inside Zahnle and Catling's 4–5; the weathering thermostat
/// leaves all the carbon airborne, a frozen surface weathering none; and 2.18% of the median 10
/// bar is 0.22 bar, over which T24.a's climate spans 230–251 K against carbon dioxide's 177.6 K
/// frost point, so no cold trap forms and the greenhouse, starting from the whole column, takes the
/// warm branch.
///
/// No collapse-first start cures it (P14.T53.c's deferred findings): the model's climate holds no
/// collapsed branch at Mars's inventory. At 2, 6 and 10 mbar of CO₂ its coldest zone lies at
/// 136–139, 164 and 174 K, against CO₂'s frost points of 140, 148 and 151 K there, and the
/// shoreline's 234 Pa of N₂ alone holds a start from an empty column above the 100 Pa line, where
/// the steps' climate stops being an airless one's, so the start climbs back to the warm branch;
/// with no N₂, or Mars's real 16 Pa, it lands at 0.18–0.30 Pa, at the airless climate's 107.5 K
/// coldest zone, the ~2 mbar a marginal fixed point of the contrasts that the empty start never
/// reaches. The cure is the CO₂ cap's own energy balance, its polar insolation at a stated cap
/// albedo and emissivity, which moves every Mars-like body (main, 2026-10-26: after 24, P14.T55.e
/// or a new rider on 25's train). Run by name once that lands.
#[test]
#[ignore = "P14.T53.c's recorded miss: the model's climate holds no collapsed branch at the table's Mars's 0.22 bar (its CO2 cap's own energy balance, after 24)"]
fn under_the_escape_batch_the_table_s_mars_keeps_3_to_10_mbar_of_carbon_dioxide() {
    crate::planetary::testing::with_every_24_hold(|| {
        let (_, _, air, _) = table_mars_at_its_spin();
        let co2 = air.partial_pressures().of(Gas::CarbonDioxide).value();
        let column = air.inventory_weights().of(Gas::CarbonDioxide).value();
        let t = air.surface_temperature().value();
        eprintln!(
            "the table's Mars, every 24 hold: {}; CO2 {co2:.4} Pa of its column's {column:.4}",
            air_line("", &air)
        );
        assert!((300.0..=1_000.0).contains(&co2), "Mars's CO2 at {co2} Pa");
        assert!(
            ((t - MARS_SURFACE_TEMPERATURE) / MARS_SURFACE_TEMPERATURE).abs() < 0.08,
            "{t} K"
        );
        assert!(column > co2, "Mars holds CO2 ice");
    });
}

/// A rocky planet of `earths` M⊕ and `radii` R⊕ on a circular orbit of `au` and period `days`
/// about a star of `solar` M☉, `luminosity` L☉ and effective temperature `t_eff` K, taken at its
/// zero-age luminosity, at age `years`, by hand: its median inventory, formed inside the snow line,
/// at its zero-albedo equilibrium temperature, inside its star's runaway limit; its fluence and
/// saturated flux its star's history's ([`XuvHistory`]) on that orbit, and its impact speed Zahnle
/// and Catling's eq. 41 at its orbital speed 2πa ÷ P.
fn m_dwarf_planet(
    (solar, luminosity, t_eff): (f64, f64, f64),
    (earths, radii, au, days): (f64, f64, f64, f64),
    years: f64,
) -> AtmosphereInputs {
    let (mass, radius) = (EarthMasses::new(earths), EarthRadii::new(radii));
    let a = Metres::new(au * METRES_PER_AU);
    let flux = EarthFluxes::new(luminosity / (au * au));
    let fractions = composition(mass, radius, SnowLineSide::Inside, flux)
        .unwrap()
        .fractions();
    let age = Years::new(years);
    let history = XuvHistory::new(SolarMasses::new(solar), SolarLuminosities::new(luminosity));
    let (kg, metres) = (Kilograms::from(mass), Metres::from(radius));
    let escape = (2.0 * GRAVITATIONAL_CONSTANT * kg.value() / metres.value()).sqrt();
    let orbital = 2.0 * core::f64::consts::PI * a.value() / (days * 86_400.0);
    let t_eq = equilibrium_temperature(flux, BondAlbedo::from_fraction(0.0));
    AtmosphereInputs {
        mass: kg,
        radius: metres,
        material: SurfaceMaterial::of(&fractions),
        envelope_fraction: 0.0,
        class: PlanetClass::of(mass, &fractions),
        // Inside the snow line, where the formation temperature moves no inventory.
        inventory: volatile_inventory(
            mass,
            &fractions,
            SnowLineSide::Inside,
            Kelvin::new(400.0),
            &VolatileDraws::MEDIAN,
            age,
        ),
        formed: SnowLineSide::Inside,
        equilibrium: t_eq,
        irradiation_temperature: t_eq,
        heated: t_eq,
        xuv_fluence: history.fluence(age, a, 0.0),
        saturated_xuv_flux: history.saturated_flux(a, 0.0),
        insolation: Insolation::InsideRunaway,
        present_insolation: Insolation::InsideRunaway,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age,
        hottest_host: Kelvin::new(t_eff),
        impact_speed: planet_impact_speed(
            MetresPerSecond::new(orbital),
            MetresPerSecond::new(escape),
        ),
        xuv_saturation_time: history.saturation_time(),
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(t_eq),
        maximum_greenhouse_flux_ratio: irradiation_flux(t_eq).value()
            / crate::planetary::derive::habitable_zone::HabitableLimit::MaximumGreenhouse
                .flux(Kelvin::new(t_eff))
                .value(),
    }
}

/// P14.T53.a: TRAPPIST-1 b and c and LHS 3844 b, by hand about their stars, are airless or under
/// 1 mbar under the batch, as JWST and Spitzer read them (Greene et al. 2023; Zieba et al. 2023;
/// Kreidberg et al. 2019, Nature 573, 87).
///
/// - TRAPPIST-1 (Agol et al. 2021, PSJ 2, 1, Tables 2, 6 and 7, read): 0.0898 M☉, 5.53 × 10⁻⁴ L☉,
///   2,566 K; b 1.374 M⊕ and 1.116 R⊕ at 0.01154 au every 1.510 826 d, c 1.308 M⊕ and 1.097 R⊕ at
///   0.01580 au every 2.421 937 d; 7.6 Gyr old (Burgasser and Mamajek 2017, ApJ 845, 110, from
///   memory).
/// - LHS 3844 (Vanderspek et al. 2019, ApJ Lett. 871, L24, Table 1, read): 0.151 M☉, 0.00272 L☉,
///   3,036 K; b 1.32 R⊕ at 0.00623 au every 0.462 927 92 d, its mass 2.27 M⊕ (Hacker et al. 2026,
///   arXiv:2603.28237, abstract), its age taken as 5 Gyr (part B's shoreline test).
#[test]
fn under_the_escape_batch_trappist_1_b_and_c_and_lhs_3844_b_are_airless_or_under_a_millibar() {
    with_rules(EscapeRules::Batch, || {
        let trappist = (0.0898, 5.53e-4, 2_566.0);
        for (name, inputs) in [
            (
                "TRAPPIST-1 b",
                m_dwarf_planet(trappist, (1.374, 1.116, 0.011_54, 1.510_826), 7.6e9),
            ),
            (
                "TRAPPIST-1 c",
                m_dwarf_planet(trappist, (1.308, 1.097, 0.015_80, 2.421_937), 7.6e9),
            ),
            (
                "LHS 3844 b",
                m_dwarf_planet(
                    (0.151, 0.002_72, 3_036.0),
                    (2.27, 1.32, 0.006_23, 0.462_927_92),
                    5e9,
                ),
            ),
        ] {
            let air = atmosphere(&inputs);
            let escape = escape_of(&air);
            let v =
                (2.0 * GRAVITATIONAL_CONSTANT * inputs.mass.value() / inputs.radius.value()).sqrt();
            eprintln!(
                "{}; {:.4} F⊕ of fluence, {:+.3} dex from the line; impact ratio {:.3}; O2 kept \
                 {:.3e} kg",
                air_line(name, &air),
                inputs.xuv_fluence.value() / earth_xuv_fluence().value(),
                shoreline_distance_dex(
                    inputs.xuv_fluence.value() / earth_xuv_fluence().value(),
                    MetresPerSecond::new(v)
                ),
                inputs.impact_speed.value() / v,
                escape.oxygen().kept().value(),
            );
            let p = air.surface_pressure().unwrap();
            assert!(
                air.state() == SurfaceState::Airless || p.value() < 100.0,
                "{name}: {air:?}"
            );
        }
    });
}

/// P14.T53.a (as P14.T13.b's test): under the batch a 5 M⊕ core with a 2% envelope at 0.05 au of
/// a Sun is stripped by 1 Gyr, and the same body at 0.5 au is not; each one's helium after its
/// escape stated.
#[test]
fn under_the_escape_batch_a_5_earth_mass_core_is_stripped_at_0_05_au_and_not_at_0_5_au() {
    with_rules(EscapeRules::Batch, || {
        let disc = solar_disc();
        let close = derive(&two_per_cent_envelope(&disc, 0.05), &disc, Years::new(1e9));
        let far = derive(&two_per_cent_envelope(&disc, 0.5), &disc, Years::new(1e9));
        for (name, body) in [("0.05 au", &close), ("0.5 au", &far)] {
            let envelope = escape_of(body.atmosphere()).envelope();
            eprintln!(
                "the 5 M⊕ core at {name}: envelope {:.6} of its mass, {:.5} M⊕ lost; {}; its \
                 helium {envelope:?}",
                body.fractions().envelope(),
                body.envelope_lost().value(),
                air_line(name, body.atmosphere()),
            );
        }
        assert_same_bits(close.fractions().envelope(), 0.0);
        assert!(close.envelope_lost().value() > 0.09);
        assert!(far.fractions().envelope() > 0.018, "{far:?}");
        assert_eq!(far.atmosphere().state(), SurfaceState::GasEnvelope);
    });
}

/// A Mars's inputs by hand at age `years` about the present Sun at 1.524 au, dry, so that its
/// carbon is airborne whatever its climate: its fluence the Sun's history's to the age, its impact
/// speed at 24.07 km/s of orbit and 5.03 of escape, its inventory fixed.
fn dry_mars_at(years: f64) -> AtmosphereInputs {
    let (kg, radius) = (6.4169e23, 3.3895e6);
    let a = Metres::new(1.524 * METRES_PER_AU);
    let sun = historic_sun();
    let age = Years::new(years);
    AtmosphereInputs {
        mass: Kilograms::new(kg),
        radius: Metres::new(radius),
        material: SurfaceMaterial::Rock,
        envelope_fraction: 0.0,
        class: PlanetClass::Rocky,
        inventory: VolatileInventory::new(
            Kilograms::ZERO,
            Kilograms::new(EARTH_CARBON_DIOXIDE_PER_MASS * kg),
            Kilograms::new(EARTH_NITROGEN_PER_MASS * kg),
            Kilograms::new(1e16),
        ),
        formed: SnowLineSide::Inside,
        equilibrium: Kelvin::new(210.0),
        irradiation_temperature: Kelvin::new(225.0),
        heated: Kelvin::new(210.0),
        xuv_fluence: sun.xuv().fluence(age, a, 0.0),
        saturated_xuv_flux: sun.xuv().saturated_flux(a, 0.0),
        insolation: Insolation::Habitable,
        present_insolation: Insolation::Habitable,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age,
        hottest_host: Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K),
        impact_speed: planet_impact_speed(
            MetresPerSecond::new(24.07e3),
            MetresPerSecond::new(5.03e3),
        ),
        xuv_saturation_time: sun.xuv().saturation_time(),
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: irradiation_flux(Kelvin::new(225.0)),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(irradiation_flux(Kelvin::new(225.0))),
    }
}

/// P14.T53.a: escape is continuous and monotone in time. A dry Mars by hand ([`dry_mars_at`]),
/// its inventory and climate fixed, over 10 Myr to 10 Gyr at 32 ages to the decade: the share of
/// its air the shoreline keeps and its surface pressure never rise, and no step of 1/32 dex moves
/// either by more than 8% of the air's first figure (both stated, with the largest step).
#[test]
fn under_the_escape_batch_escape_is_continuous_and_monotone_in_time() {
    with_rules(EscapeRules::Batch, || {
        let mut last: Option<(f64, f64)> = None;
        let (mut first, mut steps) = (None, (0.0_f64, 0.0_f64));
        for k in 0..=96 {
            let years = math::exp10(7.0 + f64::from(k) / 32.0);
            let air = atmosphere(&dry_mars_at(years));
            let (kept, p) = (
                escape_of(&air).shoreline().kept(),
                air.surface_pressure().unwrap().value(),
            );
            if k % 8 == 0 {
                eprintln!(
                    "at {years:.4e} yr: kept {kept:.6}, p_s {p:.6e} Pa, {:?}",
                    air.state()
                );
            }
            let (kept_0, p_0) = *first.get_or_insert((kept, p));
            if let Some((was_kept, was_p)) = last {
                assert!(kept <= was_kept, "the kept share rises at {years} yr");
                assert!(
                    p <= was_p,
                    "the pressure rises at {years} yr: {was_p} to {p}"
                );
                steps.0 = steps.0.max((was_kept - kept) / kept_0);
                steps.1 = steps.1.max((was_p - p) / p_0);
            }
            last = Some((kept, p));
        }
        eprintln!(
            "the largest step: {:.4} of the kept share, {:.4} of the pressure",
            steps.0, steps.1
        );
        assert!(steps.0 < 0.08 && steps.1 < 0.08, "{steps:?}");
    });
}

/// P14.T53.a's tripwire (decision-bump-plan-after-22, R3 Q1 (5)): under the batch the table's
/// Earth, whose stored state holds liquid water, is its stored state to the bit, with the
/// weathering thermostat and without it, its carbon's airborne share the stored 6 × 10⁻⁶; and the
/// thermostat's solves over the table are stated.
#[test]
fn under_the_escape_batch_earth_is_its_stored_state_to_the_bit() {
    with_rules(EscapeRules::Batch, || {
        let (on, counts) = with_thermostat(Thermostat::On, table);
        let (off, _) = with_thermostat(Thermostat::Off, table);
        let (with, without) = (found(&on, "Earth"), found(&off, "Earth"));
        assert_eq!(with, without);
        let (a, b) = (with.atmosphere(), without.atmosphere());
        assert_same_bits(
            a.surface_temperature().value(),
            b.surface_temperature().value(),
        );
        for gas in Gas::ALL {
            assert_same_bits(
                a.partial_pressures().of(gas).value(),
                b.partial_pressures().of(gas).value(),
            );
        }
        assert_same_bits(with.albedo().value(), without.albedo().value());
        assert_same_bits(
            escape_of(a).airborne_carbon_share(),
            TEMPERATE_AIRBORNE_CARBON,
        );
        eprintln!(
            "the table under the batch: {counts:?}; Earth {}",
            air_line("", a)
        );
    });
}

/// P14.T53.a's hold: under the escape's hold, to generator version 23, no body records an escape
/// and the batch's steps take no solve, and the table's bodies are the rules in force's, which
/// version 23 holds (the goldens hold every figure).
#[test]
fn under_the_hold_no_body_records_an_escape() {
    let (held, counts) = with_thermostat(Thermostat::On, || with_rules(EscapeRules::Held, table));
    assert_eq!(counts, ThermostatCounts::default());
    for (name, body) in &held {
        assert!(body.atmosphere().escape().is_none(), "{name}");
    }
    assert_eq!(held, table());
}

/// The elements' inventory, kg, in [`ESCAPE_ELEMENTS`]'s order (hydrogen, helium, carbon,
/// nitrogen, oxygen, argon), of the carriers `inventory` and an envelope of `envelope` kg at
/// formation at the protosolar helium: the masses each element's outcome must sum to.
fn element_inventory_of(inventory: &VolatileInventory, envelope: f64) -> [f64; 6] {
    use crate::substance::Element;
    assert_eq!(
        ESCAPE_ELEMENTS,
        [
            Element::H,
            Element::He,
            Element::C,
            Element::N,
            Element::O,
            Element::Ar
        ]
    );
    let share = |gas: Gas, element: Element| {
        let count = gas
            .substance()
            .substance()
            .elements()
            .iter()
            .find(|(e, _)| *e == element)
            .map_or(0, |&(_, n)| n);
        f64::from(count) * element.standard_atomic_weight_g_per_mol() / gas.molar_mass_g_per_mol()
    };
    let helium = crate::planetary::derive::atmosphere::escaping::protosolar_helium_mass_fraction();
    let (water, co2) = (
        inventory.water().value(),
        inventory.carbon_dioxide().value(),
    );
    [
        water * share(Gas::Water, Element::H) + (1.0 - helium) * envelope,
        helium * envelope,
        co2 * share(Gas::CarbonDioxide, Element::C),
        inventory.nitrogen().value() * share(Gas::Nitrogen, Element::N),
        water * share(Gas::Water, Element::O) + co2 * share(Gas::CarbonDioxide, Element::O),
        inventory.argon().value() * share(Gas::Argon, Element::Ar),
    ]
}

/// A thin envelope's inputs by hand: a 5 M⊕ core of 1.6 R⊕ in a temperate orbit of a Sun, at an
/// irradiation of 300 K, at which its exobase retains its hydrogen, that formed with 2% of its mass
/// in hydrogen and helium and has kept 5 × 10⁻⁴ of its mass, its history consistent with what it
/// kept: the envelope at formation 0.02 M₀, M₀ = M (1 − f) ÷ 0.98, and the loss what it lacks.
fn thin_envelope_core() -> AtmosphereInputs {
    let now = 5.0 * EARTH_MASS_KG;
    let fraction = 5e-4;
    let mut inputs = earth_inputs(280.0, Insolation::Habitable);
    let initial = 0.02 * now * (1.0 - fraction) / 0.98;
    inputs.mass = Kilograms::new(now);
    inputs.radius = Metres::new(1.6 * crate::units::consts::EARTH_RADIUS_M);
    inputs.envelope_fraction = fraction;
    inputs.envelope_history = EnvelopeHistory {
        initial: Kilograms::new(initial),
        lost: Kilograms::new(initial - fraction * now),
    };
    inputs.irradiation_temperature = Kelvin::new(300.0);
    inputs
}

/// P14.T53.a: under the batch each element's outcome is mass-conserving, its four parts summing to
/// the element's inventory, its carriers' and its envelope's at formation, within 10⁻⁹ of the
/// largest: over the table, the two 5 M⊕ cores, a thin envelope kept as air (whose H₂ and He then
/// enter its partial pressures), TRAPPIST-1 b, a runaway Earth with a young melt, a Titan, a cold
/// moon, an Earth and the remnant ice bulks. The young melt's oxygen stated against its capacity.
#[test]
fn under_the_escape_batch_each_element_s_outcome_sums_to_its_inventory() {
    with_rules(EscapeRules::Batch, || {
        let check = |name: &str, air: &Atmosphere, inventory: &VolatileInventory, envelope: f64| {
            let expected = element_inventory_of(inventory, envelope);
            let largest = expected.iter().fold(0.0_f64, |a, &b| a.max(b));
            for (outcome, expected) in escape_of(air).elements().iter().zip(expected) {
                let sum = outcome.inventory().value();
                assert!(
                    (sum - expected).abs() <= 1e-9 * largest,
                    "{name}: {:?} sums to {sum} kg against {expected} kg: {outcome:?}",
                    outcome.element()
                );
            }
        };
        // A derived body's envelope at formation: the one it holds now and what escape took.
        let formed = |body: &DerivedBody| {
            body.fractions().envelope() * Kilograms::from(body.mass()).value()
                + Kilograms::from(body.envelope_lost()).value()
        };
        for (name, body) in &table() {
            check(name, body.atmosphere(), body.inventory(), formed(body));
        }
        let disc = solar_disc();
        for au in [0.05, 0.5] {
            let body = derive(&two_per_cent_envelope(&disc, au), &disc, Years::new(1e9));
            let at_formation = body.initial_envelope_fraction()
                * Kilograms::from(body.mass() + body.envelope_lost()).value();
            assert!((formed(&body) / at_formation - 1.0).abs() < 1e-12);
            check(
                "a 5 M⊕ core",
                body.atmosphere(),
                body.inventory(),
                formed(&body),
            );
        }
        let thin = thin_envelope_core();
        let air = atmosphere(&thin);
        eprintln!("the thin envelope: {}", air_line("", &air));
        assert!(
            air.partial_pressures().of(Gas::Hydrogen) > Pascals::ZERO,
            "{air:?}"
        );
        assert!(
            air.partial_pressures().of(Gas::Helium) > Pascals::ZERO,
            "{air:?}"
        );
        check(
            "the thin envelope",
            &air,
            &thin.inventory,
            thin.envelope_history.initial.value(),
        );
        let mut young = earth_inputs(300.0, Insolation::InsideRunaway);
        young.molten_for = Years::new(5e7);
        young.inventory = VolatileInventory::new(
            Kilograms::new(5.0 * OCEAN),
            young.inventory.carbon_dioxide(),
            young.inventory.nitrogen(),
            young.inventory.argon(),
        );
        let trappist = m_dwarf_planet(
            (0.0898, 5.53e-4, 2_566.0),
            (1.374, 1.116, 0.011_54, 1.510_826),
            7.6e9,
        );
        let titan = titan_inputs();
        let earth = earth_inputs(255.0, Insolation::Habitable);
        for (name, inputs) in [
            ("TRAPPIST-1 b", &trappist),
            ("a runaway Earth with a young melt", &young),
            ("a Titan", &titan),
            ("an Earth", &earth),
        ] {
            let air = atmosphere(inputs);
            check(
                name,
                &air,
                &inputs.inventory,
                inputs.envelope_history.initial.value(),
            );
            if name == "a runaway Earth with a young melt" {
                eprintln!(
                    "{}: {:?}, the melt's capacity {:.4e} kg",
                    air_line(name, &air),
                    escape_of(&air).oxygen(),
                    melt_oxygen_capacity(inputs.mass).value()
                );
            }
        }
        for t in [3.2, 40.0, 300.0] {
            let inputs = remnant_ice_bulk(t);
            check(
                "a remnant ice bulk",
                &atmosphere(&inputs),
                &inputs.inventory,
                0.0,
            );
        }
        let moon = cold_moon(1e19, 94.0, SnowLineSide::Beyond, Crust::Solid);
        assert!(moon.escape().is_some());
    });
}

// P14.T53.a's review (spec D) and decision-bump-plan-after-22's "T53.a's thick-air ceiling": the
// weathering thermostat's edge, the maximum-greenhouse ceiling and the batch's determinism.

use crate::planetary::derive::atmosphere::escaping::{
    THERMOSTAT_HALVINGS, THERMOSTAT_SET_POINT, THERMOSTAT_STEPS, WaterPhase, weathering_thermostat,
};

/// A thermostat's trial state in the tests: its share, its warmest zone, K, and its water.
type Trial = (f64, f64, WaterPhase);

/// The weathering thermostat over a climate whose warmest zone at airborne share s is
/// `warmest(s)`, K, liquid from 273.15 K to `boils`, K, and hot above: its share, its state, and
/// the solves it took beyond the stored state's.
fn thermostat_over(warmest: impl Fn(f64) -> f64, boils: f64) -> (f64, Trial, u32) {
    let solves = core::cell::Cell::new(0_u32);
    let (share, state) = weathering_thermostat(
        TEMPERATE_AIRBORNE_CARBON,
        |s: f64, _steps: u32| {
            solves.set(solves.get() + 1);
            let t = warmest(s);
            let phase = if t < WATER_MELTING_POINT.value() {
                WaterPhase::Frozen
            } else if t < boils {
                WaterPhase::Liquid
            } else {
                WaterPhase::Hot
            };
            (s, t, phase)
        },
        |&(_, t, phase): &Trial| (Kelvin::new(t), phase),
    );
    (share, state, solves.get() - 1)
}

/// The weathering thermostat as decision-bump-plan-after-22 R3 Q1 (3) and its "thick-air ceiling"
/// K5 rule it, on hand climates: a frozen stored state whose all-airborne state runs away past the
/// liquid window stops at the cold edge, its warmest zone at the set point within 10⁻³ K, liquid
/// (the census's overshot Temperates took all their carbon airborne there); one that jumps from ice
/// straight to steam keeps a share still frozen below the jump; all of it airborne only where that
/// state is still frozen; a liquid stored state, and a hot one with no liquid above it, keep the
/// stored share; a boiling stored state takes the share whose fuller air holds its water liquid.
/// The edge takes [`THERMOSTAT_STEPS`] trials beyond its two ends and one at the interpolated edge,
/// the boiling state's [`THERMOSTAT_HALVINGS`]; an end taken that was a trial, the jump's cold end
/// and the boiling state's liquid end, is solved again in full.
#[test]
fn the_weathering_thermostat_stops_at_the_cold_edge_of_the_liquid_window() {
    let stored = TEMPERATE_AIRBORNE_CARBON;
    let set = THERMOSTAT_SET_POINT.value();
    let edge_solves = 1 + THERMOSTAT_STEPS + 1;
    // Smooth, running away past the window at all of it airborne: 200 K + 600 K √s, liquid to 400.
    for (label, warmest) in [
        (
            "200 K + 600 K √s",
            (|s: f64| 200.0 + 600.0 * s.sqrt()) as fn(f64) -> f64,
        ),
        ("(200⁴ + c s^⅓)^¼", |s: f64| {
            let floor = math::powi(200.0, 4);
            let c = (math::powi(WATER_MELTING_POINT.value(), 4) - floor) / math::powf(0.01, 0.33);
            math::powf(floor + c * math::powf(s, 0.33), 0.25)
        }),
        ("260 K + 30 K tanh 5(ln s + 3)", |s: f64| {
            260.0 + 30.0 * math::tanh(5.0 * (math::ln(s) + 3.0))
        }),
    ] {
        let (share, state, solves) = thermostat_over(warmest, 400.0);
        eprintln!(
            "{label}: share {share:.6e}, its warmest zone {:.6} K, {solves} solves",
            state.1
        );
        assert_eq!(state.2, WaterPhase::Liquid, "{label}");
        assert!((state.1 - set).abs() < 1e-3, "{label}: {} K", state.1);
        assert_eq!(solves, edge_solves, "{label}");
    }
    let edge = math::powi((set - 200.0) / 600.0, 2);
    let (share, ..) = thermostat_over(|s| 200.0 + 600.0 * s.sqrt(), 400.0);
    assert!((share / edge - 1.0).abs() < 1e-5, "{share} against {edge}");
    // A jump from ice past the window: a share still frozen, below the jump.
    let (share, state, solves) = thermostat_over(|s| if s < 0.03 { 250.0 } else { 900.0 }, 400.0);
    eprintln!("a jump at 0.03: share {share:.6e}, {solves} solves");
    assert_eq!(state.2, WaterPhase::Frozen);
    assert!(share < 0.03 && share > stored, "{share}");
    // The whole state, the trials, and the cold end, a trial, solved again in full.
    assert_eq!(solves, 1 + THERMOSTAT_STEPS + 1);
    // Too cold even with all of it airborne.
    let (share, state, solves) = thermostat_over(|s| 200.0 + 50.0 * s, 400.0);
    assert_same_bits(share, 1.0);
    assert_eq!((state.2, solves), (WaterPhase::Frozen, 1));
    // Liquid as stored; hot as stored with no liquid above.
    let (share, _, solves) = thermostat_over(|_| 280.0, 400.0);
    assert_same_bits(share, stored);
    assert_eq!(solves, 0);
    let (share, state, solves) = thermostat_over(|s| 700.0 + s, 400.0);
    assert_same_bits(share, stored);
    assert_eq!((state.2, solves), (WaterPhase::Hot, 1));
    // Boiling as stored, a fuller air holding its water liquid from s = 0.01: the liquid end.
    let solves = core::cell::Cell::new(0_u32);
    let (share, state) = weathering_thermostat(
        stored,
        |s: f64, _steps: u32| {
            solves.set(solves.get() + 1);
            let phase = if s < 0.01 {
                WaterPhase::Hot
            } else {
                WaterPhase::Liquid
            };
            (s, 300.0, phase)
        },
        |&(_, t, phase): &Trial| (Kelvin::new(t), phase),
    );
    assert_eq!(state.2, WaterPhase::Liquid);
    assert!((0.01..0.01 * (1.0 + 2e-4)).contains(&share), "{share}");
    assert_eq!(solves.get() - 1, 1 + THERMOSTAT_HALVINGS + 1);
}

/// The thermostat's share is continuous inside its bracket and where the all-airborne state just
/// thaws, and steps only by its set point's margin where the stored state just freezes: a climate
/// shifted by a hair moves the share by a hair, whatever sides its trials fall on; one whose
/// all-airborne state just thaws takes all of it; one whose stored state is just frozen takes the
/// share that warms it the set point's 0.01 K above the melting point (1.4% of the stored share on
/// this climate).
#[test]
fn the_weathering_thermostat_is_continuous() {
    let stored = TEMPERATE_AIRBORNE_CARBON;
    let melting = WATER_MELTING_POINT.value();
    let floor = melting - 600.0 * stored.sqrt() - 1e-6;
    let (share, state, _) = thermostat_over(|s| floor + 600.0 * s.sqrt(), 400.0);
    eprintln!("just frozen: {share:e} against the stored {stored:e}");
    assert_eq!(state.2, WaterPhase::Liquid);
    assert!(share >= stored && share <= stored * 1.02, "{share}");
    let (share, state, _) = thermostat_over(|s| melting - 50.0 + 50.0 * s + 1e-6, 400.0);
    eprintln!("just thawed at all of it airborne: {share}");
    assert_ne!(state.2, WaterPhase::Hot);
    assert!(share >= 1.0 - 1e-3, "{share}");
    // Shifted by 10⁻⁹ K at a time: the share's steps stay at the climate's own scale.
    let (mut last, mut largest) = (None::<f64>, 0.0_f64);
    for k in 0..200 {
        let shift = 1e-9 * f64::from(k);
        let (share, ..) = thermostat_over(|s| 200.0 + shift + 600.0 * s.sqrt(), 400.0);
        if let Some(before) = last {
            largest = largest.max((math::ln(share) - math::ln(before)).abs());
        }
        last = Some(share);
    }
    eprintln!("the largest step in ln s over 10⁻⁹ K shifts: {largest:e}");
    assert!(largest < 1e-9, "{largest}");
}

/// The weathering thermostat over a climate whose warmest zone at airborne share s is `trial(s)`,
/// K, in a trial's greenhouse steps and `full(s)` in a full solve's, frozen below 273.15 K or where
/// it is not a number, liquid up to 400 K and hot above: its share, its state, and what it counted.
fn thermostat_with_trials(
    trial: impl Fn(f64) -> f64,
    full: impl Fn(f64) -> f64,
) -> (f64, Trial, ThermostatCounts) {
    let ((share, state), counts) = with_thermostat(Thermostat::On, || {
        weathering_thermostat(
            TEMPERATE_AIRBORNE_CARBON,
            |s: f64, steps: u32| {
                let t = if steps < GREENHOUSE_STEPS {
                    trial(s)
                } else {
                    full(s)
                };
                let phase = if t.is_nan() || t < WATER_MELTING_POINT.value() {
                    WaterPhase::Frozen
                } else if t < 400.0 {
                    WaterPhase::Liquid
                } else {
                    WaterPhase::Hot
                };
                (s, t, phase)
            },
            |&(_, t, phase): &Trial| (Kelvin::new(t), phase),
        )
    });
    (share, state, counts)
}

/// A trial end whose full solve reads hot, warmer than its trial, is not taken: a cold end's gives
/// way to the stored state, a warm end's to the cold end, and a boiling state's liquid end's to the
/// all-airborne state, each counted, so that no world the thermostat moves lies above water's
/// liquid range.
#[test]
fn a_trial_end_whose_full_solve_reads_hot_gives_way_to_a_colder_one() {
    let stored = TEMPERATE_AIRBORNE_CARBON;
    // A jump from ice to steam at s = 0.03 in a trial and at 10⁻⁵ in full: the cold end, a trial
    // below 0.03, boils in full, and the stored state stands.
    let (share, state, counts) = thermostat_with_trials(
        |s| if s < 0.03 { 250.0 } else { 900.0 },
        |s| if s < 1e-5 { 250.0 } else { 900.0 },
    );
    assert_same_bits(share, stored);
    assert_eq!(state.2, WaterPhase::Frozen);
    assert_eq!((counts.cold_end, counts.retaken_hot), (1, 1), "{counts:?}");
    // No warmest zone below 0.01, liquid above it in a trial, but boiling in full up to 0.5: the
    // warm end, a trial, boils in full, and the cold end, frozen, is taken.
    let (share, state, counts) = thermostat_with_trials(
        |s| if s < 0.01 { f64::NAN } else { 300.0 },
        |s| {
            if s < 0.01 {
                f64::NAN
            } else if s < 0.5 {
                900.0
            } else {
                300.0
            }
        },
    );
    assert_eq!(state.2, WaterPhase::Frozen);
    assert!(share > stored && share < 0.01, "{share}");
    assert_eq!((counts.cold_end, counts.retaken_hot), (0, 1), "{counts:?}");
    // Boiling as stored, liquid from 0.01 in a trial but only from 0.5 in full: the liquid end, a
    // trial, boils in full, and the all-airborne state, liquid, is taken.
    let (share, state, counts) = thermostat_with_trials(
        |s| if s < 0.01 { 900.0 } else { 300.0 },
        |s| if s < 0.5 { 900.0 } else { 300.0 },
    );
    assert_same_bits(share, 1.0);
    assert_eq!(state.2, WaterPhase::Liquid);
    assert_eq!((counts.hot, counts.retaken_hot), (1, 1), "{counts:?}");
}

/// The maximum-greenhouse ceiling ("T53.a's thick-air ceiling", K4, test 4): inside the limit the
/// grey surface is [`grey_surface_temperature`]'s to the bit; beyond it `T_s` = min(`T_grey`,
/// max(273.15 K, `T_rest`)), continuous through the switch as carbon dioxide is added; and a thick
/// H₂ air beyond the limit keeps its `T_rest`, the carbon dioxide's warming above it lost.
///
/// The scan's last air, 100 bar of carbon dioxide over 1 bar of nitrogen, lies beyond the
/// thick-air closure's anchor (decision-p14-t53a-thick-co2), which would hold it 1.2 K under its
/// grey; this test is the ceiling's alone, so the closure is lifted ([`without_thick_air`]), and
/// the closure's own tests take it beside the ceiling.
#[test]
fn under_the_escape_batch_the_maximum_greenhouse_ceiling_is_continuous_and_keeps_t_rest() {
    with_rules(EscapeRules::Batch, || {
        without_thick_air(|| {
            let constants = GreenhouseConstants::in_force();
            let (equilibrium, gravity) = (Kelvin::new(150.0), STANDARD_GRAVITY);
            let air = |co2_bar: f64, h2_bar: f64| {
                PartialPressures::from_fn(|gas| match gas {
                    Gas::CarbonDioxide => Pascals::new(co2_bar * PASCALS_PER_BAR),
                    Gas::Nitrogen => Pascals::new(PASCALS_PER_BAR),
                    Gas::Hydrogen => Pascals::new(h2_bar * PASCALS_PER_BAR),
                    _ => Pascals::ZERO,
                })
            };
            let at = |ceiling: GreenhouseCeiling, partials: &PartialPressures| {
                let tau = constants.optical_depth(partials, equilibrium, gravity);
                (
                    ceiling
                        .surface_temperature(&constants, (partials, tau), equilibrium, gravity)
                        .value(),
                    grey_surface_temperature(equilibrium, tau).value(),
                    ceiling.binds(&constants, (partials, tau), equilibrium, gravity),
                )
            };
            let (mut last, mut largest_step, mut bound) = (None::<f64>, 0.0_f64, 0);
            for k in 0..=400 {
                let partials = air(math::exp10(-3.0 + 0.0125 * f64::from(k)), 0.0);
                let (none, grey, never) = at(GreenhouseCeiling::None, &partials);
                assert_same_bits(none, grey);
                assert!(!never);
                let (capped, _, binds) = at(GreenhouseCeiling::MaximumGreenhouse, &partials);
                assert!(capped <= grey, "{k}");
                assert_eq!(binds, grey > capped, "{k}");
                bound += u32::from(binds);
                if let Some(before) = last {
                    largest_step = largest_step.max((capped - before).abs());
                }
                last = Some(capped);
            }
            eprintln!(
                "CO₂ from 1 mbar to 100 bar over 1 bar of N₂ at T_eq 150 K: capped at {bound} of \
                 401, the largest step {largest_step:.4} K"
            );
            assert!(bound > 0 && bound < 401);
            assert!(largest_step < 5.0, "{largest_step}");
            // 10 bar of H₂: its own greenhouse holds the surface above freezing, at T_rest.
            let hydrogen = air(1.0, 10.0);
            let (capped, grey, binds) = at(GreenhouseCeiling::MaximumGreenhouse, &hydrogen);
            let rest = constants
                .terms(&hydrogen, equilibrium, gravity)
                .iter()
                .filter(|term| matches!(term.absorber(), Absorber::Nitrogen | Absorber::Hydrogen))
                .fold(0.0, |sum, term| sum + term.depth());
            let t_rest = grey_surface_temperature(equilibrium, rest).value();
            eprintln!("10 bar of H₂ beyond the limit: T_grey {grey:.3} K, T_rest {t_rest:.3} K");
            assert!(binds && t_rest > WATER_MELTING_POINT.value());
            assert_same_bits(capped, t_rest);
        });
    });
}

/// The ceiling reaches only a solid crust beyond the limit, under the batch ([`GreenhouseCeiling::of`]).
#[test]
fn under_the_escape_batch_the_ceiling_reaches_a_solid_crust_beyond_the_limit_alone() {
    let beyond = AtmosphereInputs {
        maximum_greenhouse_flux_ratio: 0.5,
        ..earth_inputs(150.0, Insolation::BeyondMaximumGreenhouse)
    };
    let inside = AtmosphereInputs {
        maximum_greenhouse_flux_ratio: 1.0,
        ..beyond
    };
    let molten = AtmosphereInputs {
        crust: Crust::Molten,
        ..beyond
    };
    assert_eq!(GreenhouseCeiling::of(&beyond), GreenhouseCeiling::None);
    with_rules(EscapeRules::Batch, || {
        assert_eq!(
            GreenhouseCeiling::of(&beyond),
            GreenhouseCeiling::MaximumGreenhouse
        );
        assert_eq!(GreenhouseCeiling::of(&inside), GreenhouseCeiling::None);
        assert_eq!(GreenhouseCeiling::of(&molten), GreenhouseCeiling::None);
        assert_eq!(
            without_ceiling(|| GreenhouseCeiling::of(&beyond)),
            GreenhouseCeiling::None
        );
    });
}

/// The ceiling leaves the table's Venus (inside the runaway limit), Earth, Mars (0.431 S⊕ against
/// the Sun's 0.3507: inside) and Titan (beyond it, at 92.8 K, under the cap) to the bit under the
/// batch, with both 24 holds (K4, test 1).
#[test]
fn under_the_escape_batch_the_ceiling_leaves_venus_earth_mars_and_titan_to_the_bit() {
    with_rules(EscapeRules::Batch, || {
        let (capped, lifted) = (table(), without_ceiling(table));
        for name in ["Venus", "Earth", "Mars", "Titan"] {
            let (with, without) = (found(&capped, name), found(&lifted, name));
            eprintln!(
                "{name}: {:?} {:.3} K, ceiling {:?}",
                with.atmosphere().state(),
                with.atmosphere().surface_temperature().value(),
                with.atmosphere().greenhouse_ceiling()
            );
            // Every figure but the record of the ceiling itself, which Titan's carries.
            let unrecorded = DerivedBody {
                atmosphere: Atmosphere {
                    ceiling: GreenhouseCeiling::None,
                    ..*with.atmosphere()
                },
                ..*with
            };
            assert_eq!(&unrecorded, without, "{name}");
        }
        assert_eq!(
            found(&capped, "Titan").atmosphere().greenhouse_ceiling(),
            GreenhouseCeiling::MaximumGreenhouse
        );
    });
}

/// The batch's rules are pure as the bump's are (sim-determinism): under the batch, with both 24
/// holds lifted, the table, a methane world and an oxygen world derived twice are the same
/// (`the_speciated_atmosphere_is_the_same_twice`'s, under the batch).
#[test]
fn under_the_escape_batch_the_speciated_atmosphere_is_the_same_twice() {
    with_rules(
        EscapeRules::Batch,
        the_speciated_atmosphere_is_the_same_twice,
    );
}

// P14.T53.b part 1: the state from the present insolation, and the runaway's deck
// (decision-absurd-states-24 R3–R4; decision-pre-23-review item 33).

/// What part 1 (a) leaves of a runaway's inventory: the greenhouse, its partial pressures, its
/// column, its escape, its carbon's carrier and the state its water follows, to the bit.
fn same_inventory(now: &Atmosphere, peak: &Atmosphere, at: &str) {
    assert_same_bits(
        now.surface_temperature().value(),
        peak.surface_temperature().value(),
    );
    assert_same_bits(now.optical_depth(), peak.optical_depth());
    for gas in Gas::ALL {
        assert_same_bits(
            now.partial_pressures().of(gas).value(),
            peak.partial_pressures().of(gas).value(),
        );
        assert_same_bits(
            now.inventory_weights().of(gas).value(),
            peak.inventory_weights().of(gas).value(),
        );
    }
    assert_eq!(now.escape(), peak.escape(), "{at}");
    assert_eq!(now.carbon_carrier(), peak.carbon_carrier(), "{at}");
    assert_eq!(now.water_state(), peak.water_state(), "{at}");
    assert_eq!(now.retention(), peak.retention(), "{at}");
}

/// One of P14.T53.b part 1 (a)'s hand cases ([`past_only_runaways`]), for the tests of the
/// state's readers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PastOnlyRunaway {
    /// The class it publishes, `Temperate` or `Snowball`.
    pub(crate) published: SurfaceState,
    /// Its inputs: [`earth_inputs`] inside the runaway limit at its hosts' largest past luminosity
    /// alone.
    pub(crate) inputs: AtmosphereInputs,
    /// Its atmosphere: published in its present class, its water a runaway's.
    pub(crate) air: Atmosphere,
    /// The control's: the same Earth in its present class at its peak too, its water its state's.
    pub(crate) control: Atmosphere,
    /// The same Earth inside the runaway limit now too, a runaway published.
    pub(crate) runaway: Atmosphere,
}

/// P14.T53.b part 1 (a)'s two hand cases, each derived with every 24 hold in force: an Earth inside
/// the runaway limit at its hosts' largest past luminosity alone, `Temperate` between the limits
/// now (at an equilibrium temperature of 255 K) and `Snowball` beyond the maximum-greenhouse limit
/// now (at 150 K), with its control and its runaway ([`PastOnlyRunaway`]).
pub(crate) fn past_only_runaways() -> [PastOnlyRunaway; 2] {
    use crate::planetary::testing::with_every_24_hold;
    let case = |t_eq: f64, present: Insolation, published: SurfaceState| {
        let runaway_inputs = earth_inputs(t_eq, Insolation::InsideRunaway);
        let inputs = AtmosphereInputs {
            present_insolation: present,
            ..runaway_inputs
        };
        let control_inputs = AtmosphereInputs {
            insolation: present,
            ..inputs
        };
        with_every_24_hold(|| PastOnlyRunaway {
            published,
            inputs,
            air: atmosphere(&inputs),
            control: atmosphere(&control_inputs),
            runaway: atmosphere(&runaway_inputs),
        })
    };
    [
        case(255.0, Insolation::Habitable, SurfaceState::Temperate),
        case(
            150.0,
            Insolation::BeyondMaximumGreenhouse,
            SurfaceState::Snowball,
        ),
    ]
}

/// [`past_only_runaways`]' cases are what they say: each air published in its class, its water a
/// runaway's; its control in the same class, its water its own; its runaway a runaway.
#[test]
fn under_every_24_hold_the_past_only_runaways_hand_cases_are_as_stated() {
    for case in past_only_runaways() {
        let at = format!("{case:?}");
        assert_eq!(case.air.state(), case.published, "{at}");
        assert_eq!(
            case.air.water_state(),
            SurfaceState::RunawayGreenhouse,
            "{at}"
        );
        assert_eq!(case.control.state(), case.published, "{at}");
        assert_eq!(case.control.water_state(), case.published, "{at}");
        assert_eq!(
            case.runaway.state(),
            SurfaceState::RunawayGreenhouse,
            "{at}"
        );
        assert_eq!(
            case.runaway.water_state(),
            SurfaceState::RunawayGreenhouse,
            "{at}"
        );
    }
}

/// P14.T53.b part 1 (a) on hand inputs, with every 24 hold in force: an Earth inside the runaway
/// limit at its hosts' largest past luminosity alone publishes its present class, `Temperate`
/// between the limits now (at an equilibrium temperature of 255 K) and `Snowball` beyond the
/// maximum-greenhouse limit now (at 150 K), and a runaway only while it is inside the limit now.
/// Its water stays lost and its carbon airborne: its greenhouse, partial pressures, column, escape
/// and carrier are P14.T53.a's runaway's to the bit (`with_present_state(PresentState::Peak, ..)`),
/// its water follows the runaway (`water_state`), and it holds no surface water, where the same
/// Earth in its present class at its peak too holds some. A body whose peak class is not a
/// runaway keeps its peak class's state whatever its present class reads, and under the hold the
/// present class moves nothing.
#[test]
fn under_every_24_hold_a_past_only_runaway_publishes_its_present_class() {
    use crate::planetary::hooks::figures::cover;
    use crate::planetary::testing::with_every_24_hold;
    let surface_water = |inputs: &AtmosphereInputs, air: &Atmosphere| {
        cover::surface_water(
            air,
            inputs.inventory.water(),
            inputs.radius,
            MetresPerSecondSquared::new(9.82),
            1.0,
        )
        .value()
    };
    for (t_eq, present, published) in [
        (255.0, Insolation::Habitable, SurfaceState::Temperate),
        (
            150.0,
            Insolation::BeyondMaximumGreenhouse,
            SurfaceState::Snowball,
        ),
        (
            255.0,
            Insolation::InsideRunaway,
            SurfaceState::RunawayGreenhouse,
        ),
    ] {
        let inputs = AtmosphereInputs {
            present_insolation: present,
            ..earth_inputs(t_eq, Insolation::InsideRunaway)
        };
        // The same Earth in its present class at its peak too.
        let always_inputs = AtmosphereInputs {
            insolation: present,
            ..inputs
        };
        let (now, peak, always) = with_every_24_hold(|| {
            (
                atmosphere(&inputs),
                with_present_state(PresentState::Peak, || atmosphere(&inputs)),
                atmosphere(&always_inputs),
            )
        });
        let at = format!("{t_eq} K, {present:?} now: {now:?}");
        eprintln!(
            "a past runaway at T_eq {t_eq} K, {present:?} now: {:?} (was {:?}), T_s {:.3} K, \
             p_s {:.4} bar, A {:.4}, cloud {:.4}; its surface water {:.4e} kg m⁻², the same Earth \
             {present:?} at its peak too {:?} with {:.4e}",
            now.state(),
            peak.state(),
            now.surface_temperature().value(),
            now.surface_pressure()
                .map_or(0.0, |p| p.value() / PASCALS_PER_BAR),
            now.albedo().value(),
            now.cloud_fraction(),
            surface_water(&inputs, &now),
            always.state(),
            surface_water(&always_inputs, &always),
        );
        assert_eq!(peak.state(), SurfaceState::RunawayGreenhouse, "{at}");
        assert_eq!(now.state(), published, "{at}");
        assert_eq!(now.water_state(), SurfaceState::RunawayGreenhouse, "{at}");
        assert_eq!(now.runaway_deck(), RunawayDeck::Kept, "{at}");
        same_inventory(&now, &peak, &at);
        assert_same_bits(surface_water(&inputs, &now), 0.0);
        if published == SurfaceState::RunawayGreenhouse {
            assert_eq!(now, peak, "{at}");
        } else {
            assert_eq!(always.state(), published, "{at}");
            assert!(surface_water(&always_inputs, &always) > 0.0, "{at}");
        }
        // Under the hold the present class moves nothing.
        assert_eq!(
            atmosphere(&inputs),
            atmosphere(&earth_inputs(t_eq, Insolation::InsideRunaway)),
            "{at}"
        );
    }
    // A body outside the runaway limit at its peak keeps its peak class's state.
    for insolation in [Insolation::Habitable, Insolation::BeyondMaximumGreenhouse] {
        let t_eq = match insolation {
            Insolation::BeyondMaximumGreenhouse => 150.0,
            Insolation::InsideRunaway | Insolation::Habitable => 255.0,
        };
        let inputs = earth_inputs(t_eq, insolation);
        let inside_now = AtmosphereInputs {
            present_insolation: Insolation::InsideRunaway,
            ..inputs
        };
        with_every_24_hold(|| assert_eq!(atmosphere(&inside_now), atmosphere(&inputs)));
    }
}

/// P14.T53.b part 1 (b)'s test, on hand-built atmospheres with every 24 hold in force: a published
/// runaway under Venus's deck is solved again without it only below
/// [`ACID_CYCLING_TEMPERATURE`] (430 K), never at or above it, nor from any other state, from a
/// deckless runaway or at a temperature that is not a number, and never under P14.T53.a's batch as
/// built (`with_present_state(PresentState::Peak, ..)`) or under the hold.
#[test]
fn under_every_24_hold_only_a_runaway_under_its_deck_below_430_k_is_solved_again() {
    use crate::planetary::testing::with_every_24_hold;
    assert_same_bits(ACID_CYCLING_TEMPERATURE.value(), 430.0);
    let air = with_every_24_hold(|| atmosphere(&earth_inputs(255.0, Insolation::InsideRunaway)));
    assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
    assert_eq!(air.runaway_deck(), RunawayDeck::Kept);
    let at = |t: f64| Atmosphere {
        surface_temperature: Kelvin::new(t),
        ..air
    };
    with_every_24_hold(|| {
        for t in [45.8, 273.0, 360.2, 429.999, 430.0_f64.next_down()] {
            assert!(solved_without_the_deck(&at(t)), "{t} K");
        }
        for t in [430.0, 430.0_f64.next_up(), 737.0, 1_031.0, f64::NAN] {
            assert!(!solved_without_the_deck(&at(t)), "{t} K");
        }
        let deckless = Atmosphere {
            runaway_deck: RunawayDeck::Deckless,
            ..at(300.0)
        };
        assert!(!solved_without_the_deck(&deckless));
        for state in [
            SurfaceState::GasEnvelope,
            SurfaceState::MagmaOcean,
            SurfaceState::Airless,
            SurfaceState::Temperate,
            SurfaceState::Snowball,
        ] {
            assert!(
                !solved_without_the_deck(&Atmosphere { state, ..at(300.0) }),
                "{state:?}"
            );
        }
        assert!(!with_present_state(PresentState::Peak, || {
            solved_without_the_deck(&at(300.0))
        }));
    });
    assert!(!solved_without_the_deck(&at(300.0)), "under the hold");
}

/// P14.T53.b part 1 (b) through the passes, with every 24 hold in force: a Venus at Venus's orbit
/// about the Sun with its history, its carbon and nitrogen drawn low (ranks 10⁻⁹ to the median),
/// is a runaway inside the present limit. Where its passes under Venus's deck end below 430 K
/// (P14.T53.a's batch as built, `with_present_state(PresentState::Peak, ..)`, whose solve is the
/// first one about a main-sequence host) it is solved once more without the deck and published so:
/// its state a runaway, its deck `Deckless`, its cloud a temperate world's water cloud at its
/// availability, under 0.67 for its dry air, and one re-solve counted with both solves' passes.
/// At or above 430 K it publishes the decked solve to the bit, and counts none.
#[test]
fn under_every_24_hold_a_cold_runaway_is_solved_again_without_its_deck() {
    use crate::planetary::testing::with_every_24_hold;
    let disc = solar_disc();
    let &(_, kg, km, a, e) = table_bodies().find(|(name, ..)| *name == "Venus").unwrap();
    let placed = placed_about_the_sun(&disc, (kg, km, a, e));
    let body = |rank: f64| {
        let rank = UnitUniform::new(rank).unwrap();
        let draws = VolatileDraws {
            carbon: rank,
            nitrogen: rank,
            ..VolatileDraws::MEDIAN
        };
        derived(&placed.with_volatiles(draws), &disc)
    };
    let (mut kept, mut deckless) = (0, 0);
    with_every_24_hold(|| {
        for rank in [1e-9, 1e-6, 1e-4, 1e-3, 1e-2, 0.1, 0.5] {
            let peak = with_present_state(PresentState::Peak, || body(rank));
            let (now, counts) = with_deck_counts(|| body(rank));
            let (was, air) = (peak.atmosphere(), now.atmosphere());
            let bar = |air: &Atmosphere| {
                air.surface_pressure()
                    .map_or(0.0, |p| p.value() / PASCALS_PER_BAR)
            };
            eprintln!(
                "a Venus at carbon rank {rank:e}: under the deck {:?} {:.3} K, {:.4e} bar, A {:.4} \
                 in {} passes; published {:?} {:?} {:.3} K, {:.4e} bar, A {:.4}, cloud {:.4} in {} \
                 passes; {counts:?}",
                was.state(),
                was.surface_temperature().value(),
                bar(was),
                peak.albedo().value(),
                peak.atmosphere_passes(),
                air.state(),
                air.runaway_deck(),
                air.surface_temperature().value(),
                bar(air),
                now.albedo().value(),
                air.cloud_fraction(),
                now.atmosphere_passes(),
            );
            assert_eq!(was.state(), SurfaceState::RunawayGreenhouse, "{rank:e}");
            if was.surface_temperature().value() < ACID_CYCLING_TEMPERATURE.value() {
                assert_eq!(air.state(), SurfaceState::RunawayGreenhouse, "{rank:e}");
                assert_eq!(air.runaway_deck(), RunawayDeck::Deckless, "{rank:e}");
                assert!(air.cloud_fraction() < 0.67, "{rank:e}");
                assert_eq!(
                    counts,
                    DeckCounts {
                        resolves: 1,
                        decked_passes: u64::from(peak.atmosphere_passes()),
                        deckless_passes: u64::from(now.atmosphere_passes()),
                        ..DeckCounts::default()
                    },
                    "{rank:e}"
                );
                deckless += 1;
            } else {
                assert_eq!(now, peak, "{rank:e}");
                assert_eq!(air.runaway_deck(), RunawayDeck::Kept, "{rank:e}");
                assert_eq!(counts, DeckCounts::default(), "{rank:e}");
                kept += 1;
            }
        }
    });
    assert!(kept > 0 && deckless > 0, "kept {kept}, deckless {deckless}");
}

/// P14.T53.b part 1 (c)'s convention for water's saturation temperature (decision-t53b-arm-c §2),
/// on the registry's water row (IAPWS-95, Wagner and Pruss 2002): the liquid–vapour curve from
/// the triple-point pressure, 611.657 Pa, where it is 273.16 K, through IAPWS-95's normal boiling
/// point, 373.124 K under 101,325 Pa, and 290.6 K under 2 kPa; the critical temperature,
/// 647.096 K, at and above the critical pressure, 22.064 MPa, where the curve gives none; and none
/// below the triple-point pressure, where the frost branch is not read, nor for a pressure that is
/// not a number.
#[test]
fn water_s_saturation_temperature_is_its_liquid_vapour_curve_and_its_critical_point() {
    let at = |p: f64| water_saturation_temperature(Pascals::new(p)).map(Kelvin::value);
    let triple = at(611.657).unwrap();
    let boiling = at(101_325.0).unwrap();
    eprintln!(
        "water's saturation temperature: {triple:.6} K at 611.657 Pa, {boiling:.6} K at 1 atm"
    );
    assert!((triple - 273.16).abs() < 0.01, "{triple}");
    assert!((boiling - 373.124).abs() < 0.02, "{boiling}");
    let below = at(22.064e6_f64.next_down()).unwrap();
    assert!(below <= 647.096 && 647.096 - below < 0.01, "{below}");
    for p in [22.064e6, 22.064e6_f64.next_up(), 41.5e6, 1e9] {
        assert_same_bits(at(p).unwrap(), 647.096);
    }
    for p in [611.657_f64.next_down(), 600.0, 1.0, 0.0, -1.0, f64::NAN] {
        assert_eq!(at(p), None, "{p} Pa");
    }
    let two_kilopascals = at(2_000.0).unwrap();
    assert!((two_kilopascals - 290.6).abs() < 0.1, "{two_kilopascals}");
}

/// P14.T53.b part 1 (c)'s test, with every 24 hold in force (decision-t53b-arm-c §4, §5.5's
/// stubs).
///
/// - Through [`atmosphere`]: a wet Earth between the limits is not a dry division and lists liquid
///   water; a past runaway published `Temperate`, and a dry Earth there, are dry divisions.
/// - On hand-built atmospheres: a dry `Temperate` just under water's saturation temperature at its
///   surface pressure stays, one at it moves; a wet one, one that lists liquid water and one under
///   611.657 Pa stay; one above 22.064 MPa is judged at 647.096 K; the census's hothouse's first
///   solve (1,221.6 K under 414.8 bar) moves; a past runaway's dry division moves; no other state
///   moves, nor a temperature that is not a number, nor a body with no surface pressure; and (c) is
///   off under P14.T53.a's batch as built, with (c) alone off, and under the hold.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "(c)'s wiring and each of its stubs, stated"
)]
fn under_every_24_hold_only_a_dry_temperate_at_water_s_saturation_temperature_is_a_hothouse() {
    use crate::planetary::testing::with_every_24_hold;
    let dry_earth = AtmosphereInputs {
        inventory: VolatileInventory::new(
            Kilograms::ZERO,
            Kilograms::new(EARTH_CARBON_DIOXIDE_PER_MASS * EARTH_MASS_KG),
            Kilograms::new(EARTH_NITROGEN_PER_MASS * EARTH_MASS_KG),
            Kilograms::new(6.6e16),
        ),
        ..earth_inputs(255.0, Insolation::Habitable)
    };
    let past_runaway = AtmosphereInputs {
        present_insolation: Insolation::Habitable,
        ..earth_inputs(255.0, Insolation::InsideRunaway)
    };
    let (wet, dry, past) = with_every_24_hold(|| {
        (
            atmosphere(&earth_inputs(255.0, Insolation::Habitable)),
            atmosphere(&dry_earth),
            atmosphere(&past_runaway),
        )
    });
    for (name, air) in [("wet", &wet), ("dry", &dry), ("past runaway", &past)] {
        eprintln!(
            "the {name} Earth: {:?} (water {:?}), T_s {:.3} K, p_s {:.4} bar, dry division {}, \
             liquid water {}",
            air.state(),
            air.water_state(),
            air.surface_temperature().value(),
            air.surface_pressure()
                .map_or(0.0, |p| p.value() / PASCALS_PER_BAR),
            air.dry_division,
            air.liquid_water
        );
        assert_eq!(air.state(), SurfaceState::Temperate, "{name}");
    }
    assert!(!wet.dry_division && wet.liquid_water);
    assert!(dry.dry_division && !dry.liquid_water);
    assert!(past.dry_division && !past.liquid_water);
    assert_eq!(past.water_state(), SurfaceState::RunawayGreenhouse);

    let saturation = |p: f64| {
        water_saturation_temperature(Pascals::new(p))
            .unwrap()
            .value()
    };
    let hand = |t: f64, p: f64| Atmosphere {
        state: SurfaceState::Temperate,
        water_state: SurfaceState::Temperate,
        dry_division: true,
        liquid_water: false,
        surface_temperature: Kelvin::new(t),
        surface_pressure: Some(Pascals::new(p)),
        ..dry
    };
    let bar = 1e5;
    let boiling = saturation(bar);
    with_every_24_hold(|| {
        let judged = |air: &Atmosphere| solved_as_a_hothouse(air);
        assert!(!judged(&hand(boiling.next_down(), bar)), "just under");
        assert!(judged(&hand(boiling, bar)), "at it");
        assert!(judged(&hand(boiling + 500.0, bar)), "above it");
        let wet = Atmosphere {
            dry_division: false,
            ..hand(boiling + 100.0, bar)
        };
        assert!(!judged(&wet), "a wet one");
        let liquid = Atmosphere {
            liquid_water: true,
            ..hand(boiling + 100.0, bar)
        };
        assert!(!judged(&liquid), "one that lists liquid water");
        for p in [611.657_f64.next_down(), 600.0, 1.0] {
            assert!(!judged(&hand(1_000.0, p)), "{p} Pa");
        }
        assert!(judged(&hand(saturation(611.657), 611.657)), "at 611.657 Pa");
        for p in [22.064e6, 41.5e6] {
            assert!(judged(&hand(647.096, p)), "{p} Pa at 647.096 K");
            assert!(
                !judged(&hand(647.096_f64.next_down(), p)),
                "{p} Pa under 647.096 K"
            );
        }
        assert!(
            judged(&hand(1_221.6, 414.8 * bar)),
            "the census's first solve"
        );
        let runaway_division = Atmosphere {
            water_state: SurfaceState::RunawayGreenhouse,
            ..hand(boiling, bar)
        };
        assert!(judged(&runaway_division), "a past runaway's division");
        for state in [
            SurfaceState::GasEnvelope,
            SurfaceState::MagmaOcean,
            SurfaceState::Airless,
            SurfaceState::RunawayGreenhouse,
            SurfaceState::Snowball,
        ] {
            assert!(
                !judged(&Atmosphere {
                    state,
                    ..hand(boiling + 100.0, bar)
                }),
                "{state:?}"
            );
        }
        assert!(!judged(&hand(f64::NAN, bar)), "NaN K");
        assert!(
            !judged(&Atmosphere {
                surface_pressure: None,
                ..hand(boiling + 100.0, bar)
            }),
            "no surface pressure"
        );
        for state in [PresentState::Peak, PresentState::WithoutHothouse] {
            assert!(
                !with_present_state(state, || judged(&hand(boiling + 100.0, bar))),
                "{state:?}"
            );
        }
    });
    assert!(
        !solved_as_a_hothouse(&hand(boiling + 100.0, bar)),
        "under the hold"
    );
}

/// P14.T53.b part 1 (c)'s third solve through the passes, with every 24 hold in force
/// (decision-t53b-arm-c §4.3): a hothouse between the limits whose re-solve as a runaway under
/// Venus's deck ends below the acid's 430 K is solved a third time without the deck, and that
/// solve is published.
///
/// The body is hand-built and driven over a record with no figures (each pass reads C₀, and no
/// crust), its spin locked 1:1 to its host: an Earth mass at the median radius rank, with Earth's
/// inventory per unit mass but its carbon and nitrogen at rank 0.2, on a circular orbit at
/// 1.1 √(L ÷ L☉) au (0.826 S⊕ now) about a 0.3 M☉ host at its zero-age state by Tout et al.'s
/// fits (0.0130 L☉, 0.302 R☉, 3,549 K), whose largest past luminosity is twice that (the test's
/// own figure), so that the body lay inside the runaway limit then and its water is lost.
///
/// - Its first solve, with (c) off (`PresentState::WithoutHothouse`), is that past runaway's dry
///   division published `Temperate` at about 496.0 K, A 0.176 and 19.83 bar, 10.9 K above water's
///   saturation temperature there, 485.1 K, which (c) judges a hothouse. A dry `Temperate` of
///   this body meets that curve between carbon ranks 0.14 and 0.15, at about 15–16 bar.
/// - Its solve as a runaway under the deck, as P14.T53.a's batch solves it
///   (`PresentState::Peak`), ends at about 364.6 K and A 0.76, below 430 K.
/// - So its third solve, without the deck, is published: a `RunawayGreenhouse`, `Deckless`, its
///   water still a runaway's, at the first solve's 496.0 K and A 0.176, a dry world's water cloud
///   being none; at or above 430 K, so a partial deck, not modelled (P14.T53.e).
///
/// One hothouse is counted with its third solve, each solve's passes beside it, and the traced
/// solve lists the third solve's passes alone: as many as it counts, each a deckless runaway's,
/// the published air among them.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one hand-built body's three solves, its counts and its trace, each stated"
)]
fn under_every_24_hold_a_hothouse_below_430_k_under_its_deck_is_solved_a_third_time() {
    use crate::orbit::{Eccentricity, KeplerElements, Orientation};
    use crate::planetary::derive::atmosphere::{NoMaturity, WeatheringCase};
    use crate::planetary::derive::pass_record::{
        PassBulk, PassRecord, PassRecordOf, RecordCrust, RecordSpin,
    };
    use crate::planetary::derive::solar::disc_of;
    use crate::planetary::derive::{PassDriver, derive_body_driven, traced};
    use crate::planetary::hooks::figures::WetEpoch;
    use crate::planetary::hooks::surface::{HourCosines, SeasonalSpin, ZonalInsolation};
    use crate::planetary::testing::with_every_24_hold;
    use crate::stellar::sse::{ZCoeffs, zams};
    use crate::units::GravitationalParameter;
    use crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K;

    /// A record with no figures: the body's spin under its insolation table, the first context,
    /// C₀, for every pass, and no crust.
    struct NoFigures {
        /// The spin.
        spin: SeasonalSpin,
        /// Its obliquity's insolation table.
        table: ZonalInsolation,
    }
    impl PassRecordOf for NoFigures {
        fn spin(&self, bulk: &PassBulk) -> RecordSpin<'_> {
            RecordSpin {
                spin: self.spin,
                table: &self.table,
                surface_gravity: bulk.surface_gravity,
                radius: Metres::from(bulk.radius),
                material: SurfaceMaterial::of(&bulk.fractions),
            }
        }

        fn context(&self, _: &PassBulk, _: &Atmosphere, _: Kelvin) -> PassContext {
            PassContext::NONE
        }

        fn crust(&self, _: &PassBulk) -> Option<RecordCrust> {
            None
        }

        fn weathering(&self, _: &PassBulk) -> Result<WeatheringCase, NoMaturity> {
            Err(NoMaturity::NoHost)
        }

        fn wet_epoch(&self, _: &PassBulk, _: &Atmosphere, _: Kelvin) -> Option<WetEpoch> {
            None
        }

        fn cold_trap_sky(
            &self,
            _: WattsPerSquareMetre,
        ) -> Result<
            crate::planetary::hooks::cold_traps::ColdTrapSky,
            crate::planetary::hooks::cold_traps::NoColdTraps,
        > {
            // A hand-built record with no seasonal orbit for its cold traps.
            Err(crate::planetary::hooks::cold_traps::NoColdTraps::NoSky)
        }
    }

    let host = SolarMasses::new(0.3);
    let coeffs = ZCoeffs::new(Composition::SOLAR.z_fit());
    let (luminosity, radius) = (zams::luminosity(host, &coeffs), zams::radius(host, &coeffs));
    // Stefan–Boltzmann in solar units, as a star's state takes it.
    let effective =
        SOLAR_EFFECTIVE_TEMPERATURE_K * luminosity.value().sqrt().sqrt() / radius.value().sqrt();
    let lights = [HostLight::new(luminosity, Kelvin::new(effective), radius)
        .unwrap()
        .with_history(
            XuvHistory::new(host, luminosity),
            SolarLuminosities::new(2.0 * luminosity.value()),
        )];
    let hosts = BodyHosts::new(
        Kilograms::new(host.value() * SOLAR_MASS_KG),
        Composition::SOLAR,
        &lights,
        &[],
    )
    .unwrap();
    let disc = disc_of(host.value());
    let orbit = KeplerElements::from_semi_major_axis(
        Metres::new(1.1 * luminosity.value().sqrt() * METRES_PER_AU),
        GravitationalParameter::from_solar_masses(host),
        Eccentricity::new(0.0).unwrap(),
        Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO).unwrap(),
        Radians::ZERO,
    )
    .unwrap();
    let rank = UnitUniform::new(0.2).unwrap();
    let placed = PlacedBody::new(
        EarthMasses::new(1.0),
        orbit,
        orbit.semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap()
    .with_volatiles(VolatileDraws {
        carbon: rank,
        nitrogen: rank,
        ..VolatileDraws::MEDIAN
    });
    let spin = SeasonalSpin {
        obliquity: Radians::ZERO,
        rotation_period: orbit.period(),
        solar_day: None,
    };
    let record = NoFigures {
        spin,
        table: ZonalInsolation::of(spin.obliquity, &HourCosines::new()),
    };
    let derive = |driver| {
        derive_body_driven(
            driver,
            &placed,
            &hosts,
            &disc,
            SOLAR_AGE,
            UniverseTime::EPOCH,
            PassRecord::Record(&record),
        )
        .unwrap()
    };
    let bar = |air: &Atmosphere| {
        air.surface_pressure()
            .map_or(f64::NAN, |p| p.value() / PASCALS_PER_BAR)
    };
    let saturation = |air: &Atmosphere| {
        air.surface_pressure()
            .and_then(water_saturation_temperature)
            .map_or(f64::NAN, Kelvin::value)
    };
    with_every_24_hold(|| {
        let first = with_present_state(PresentState::WithoutHothouse, || derive(PassDriver::Solve));
        let decked = with_present_state(PresentState::Peak, || derive(PassDriver::Solve));
        let ((published, counts), trace) =
            traced(|| with_deck_counts(|| derive(PassDriver::Traced)));
        let solves = [
            ("first solve", &first),
            ("decked solve", &decked),
            ("third solve", &published),
        ];
        for (name, body) in solves {
            let air = body.atmosphere();
            eprintln!(
                "the hand-built hothouse's {name}: {:?} {:?} (water {:?}), T_s {:.3} K, A {:.4}, \
                 p_s {:.4} bar (water's T_sat {:.3} K), {} passes",
                air.state(),
                air.runaway_deck(),
                air.water_state(),
                air.surface_temperature().value(),
                body.albedo().value(),
                bar(air),
                saturation(air),
                body.atmosphere_passes()
            );
        }
        eprintln!("its counts {counts:?}; {} passes traced", trace.len());
        let first_air = first.atmosphere();
        assert_eq!(first_air.state(), SurfaceState::Temperate);
        assert_eq!(first_air.water_state(), SurfaceState::RunawayGreenhouse);
        assert!(first_air.dry_division && !first_air.liquid_water);
        assert!(solved_as_a_hothouse(first_air));
        assert!(first_air.surface_temperature().value() >= saturation(first_air));
        assert!((first_air.surface_temperature().value() - 496.0).abs() < 0.5);
        assert!((first.albedo().value() - 0.176).abs() < 0.005);
        assert!(
            (bar(first_air) - 19.83).abs() < 0.05,
            "{} bar",
            bar(first_air)
        );

        let decked_air = decked.atmosphere();
        assert_eq!(decked_air.state(), SurfaceState::RunawayGreenhouse);
        assert_eq!(decked_air.runaway_deck(), RunawayDeck::Kept);
        assert!(decked_air.surface_temperature().value() < ACID_CYCLING_TEMPERATURE.value());
        assert!((decked_air.surface_temperature().value() - 364.6).abs() < 0.5);
        assert!((decked.albedo().value() - 0.76).abs() < 0.005);

        let air = published.atmosphere();
        assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
        assert_eq!(air.runaway_deck(), RunawayDeck::Deckless);
        assert_eq!(air.water_state(), SurfaceState::RunawayGreenhouse);
        assert!(air.surface_temperature().value() >= ACID_CYCLING_TEMPERATURE.value());
        assert!((air.surface_temperature().value() - 496.0).abs() < 0.5);
        assert!((published.albedo().value() - 0.176).abs() < 0.005);
        assert_eq!(
            counts,
            DeckCounts {
                hothouses: 1,
                temperate_passes: u64::from(first.atmosphere_passes()),
                hothouse_passes: u64::from(decked.atmosphere_passes()),
                hothouse_deckless: 1,
                hothouse_deckless_passes: u64::from(published.atmosphere_passes()),
                ..DeckCounts::default()
            }
        );

        assert_eq!(
            trace.len(),
            usize::try_from(published.atmosphere_passes()).unwrap()
        );
        for pass in &trace {
            assert_eq!(
                (pass.air.state(), pass.air.runaway_deck()),
                (SurfaceState::RunawayGreenhouse, RunawayDeck::Deckless),
                "{pass:?}"
            );
        }
        assert!(
            trace.iter().any(|pass| pass.air == *air),
            "the published pass is traced"
        );
    });
}

/// P14.T53.b part 1 leaves the table's Venus (the median Venus), Earth, Mars and Titan and the
/// measured Venus ([`measured_venus`]) to the bit, with every 24 hold in force: each derivation
/// is P14.T53.a's batch's as built (`with_present_state(PresentState::Peak, ..)`), both Venuses
/// runaways inside the present limit under Venus's deck, well above the acid's 430 K.
#[test]
fn under_every_24_hold_part_one_leaves_venus_earth_mars_and_titan_to_the_bit() {
    use crate::planetary::testing::with_every_24_hold;
    with_every_24_hold(|| {
        let (now, peak) = (table(), with_present_state(PresentState::Peak, table));
        for name in ["Venus", "Earth", "Mars", "Titan"] {
            let body = found(&now, name);
            eprintln!(
                "the table's {name}: {:?} {:?} {:.3} K",
                body.atmosphere().state(),
                body.atmosphere().runaway_deck(),
                body.atmosphere().surface_temperature().value()
            );
            assert_eq!(body, found(&peak, name), "{name}");
        }
        let (measured, measured_peak) = (
            measured_venus(),
            with_present_state(PresentState::Peak, measured_venus),
        );
        assert_eq!(measured, measured_peak);
        for venus in [found(&now, "Venus"), &measured] {
            let air = venus.atmosphere();
            assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
            assert_eq!(air.runaway_deck(), RunawayDeck::Kept);
            assert!(air.surface_temperature().value() > ACID_CYCLING_TEMPERATURE.value());
        }
    });
}

/// P14.T53.b part 2 (decision-absurd-states-24 R2.3): with every landed 24 hold in force, no body
/// of the table and not the measured Venus lies under a supercritical envelope, so each with a
/// solid surface keeps its surface section. Venus's carbon dioxide, above its critical
/// temperature, lies far under its critical isochore (about 815 bar at 737 K, against its 92
/// bar; Lebonnois and Schubert 2017, Nat. Geosci. 10, 473, describe its deep atmosphere as a
/// supercritical fluid over the rock), and Titan's nitrogen lies below its critical temperature.
/// Every column at or above its critical temperature is printed with its margin.
#[test]
fn under_every_24_hold_venus_titan_and_the_table_keep_their_surfaces() {
    use crate::planetary::testing::with_every_24_hold;
    with_every_24_hold(|| {
        let table = table();
        let measured = measured_venus();
        for (name, body) in table
            .iter()
            .map(|(name, body)| (*name, body))
            .chain(core::iter::once(("the measured Venus", &measured)))
        {
            let (air, g) = (body.atmosphere(), body.surface_gravity());
            let columns: Vec<String> = air
                .critical_columns(g)
                .map(|c| {
                    format!(
                        "{:?} T/T_c {:.4}, p_i {:.4e} bar, p* {:.4e} bar, share {:.3e}",
                        c.gas(),
                        c.temperature_ratio(),
                        c.partial_pressure().value() / PASCALS_PER_BAR,
                        c.isochore_pressure().value() / PASCALS_PER_BAR,
                        c.share()
                    )
                })
                .collect();
            eprintln!(
                "{name}: {:?} at {:.2} K; [{}]",
                air.state(),
                air.surface_temperature().value(),
                columns.join("; ")
            );
            let reservoir = air.supercritical_reservoir(g);
            assert!(reservoir.is_none(), "{name}: {reservoir:?}");
            assert_eq!(
                has_surface_section(body.class(), air, g),
                body.class().has_solid_surface(),
                "{name}"
            );
        }
        for venus in [found(&table, "Venus"), &measured] {
            let (air, g) = (venus.atmosphere(), venus.surface_gravity());
            let co2 = air
                .critical_columns(g)
                .find(|c| c.gas() == Gas::CarbonDioxide)
                .expect("Venus's carbon dioxide lies above its critical temperature");
            assert!(
                co2.partial_pressure().value() < 0.2 * co2.isochore_pressure().value(),
                "{co2:?}"
            );
            assert_same_bits(co2.share(), 0.0);
        }
        let titan = found(&table, "Titan");
        let nitrogen = titan
            .atmosphere()
            .critical_columns(titan.surface_gravity())
            .find(|c| c.gas() == Gas::Nitrogen);
        assert!(
            nitrogen.is_none(),
            "Titan's nitrogen lies below its critical temperature: {nitrogen:?}"
        );
    });
}

/// Pluto's semi-major axis, au, and eccentricity (NASA's Pluto fact sheet), at which the Kuiper
/// belt's bodies of P14.T53.c's tests orbit the Sun.
const PLUTO_ORBIT: (f64, f64) = (39.48, 0.248_8);

/// A body of `kg` and mean radius `km` on Pluto's orbit ([`PLUTO_ORBIT`]), formed there, at the
/// rank that keeps its radius (held inside the window), derived about the present Sun with its
/// history at the Solar System's age under the rules in force.
pub(crate) fn on_pluto_s_orbit(kg: f64, km: f64) -> DerivedBody {
    let disc = solar_disc();
    let (a, e) = PLUTO_ORBIT;
    let rank = crate::planetary::derive::solar::rank_for(&disc, kg, km, a, e).clamp(0.001, 0.999);
    let placed = crate::planetary::derive::solar::placed(kg, a, e, rank);
    crate::planetary::derive::solar::derive_historic(&placed, &disc, SOLAR_AGE)
}

/// The inputs of `body` on Pluto's orbit ([`PLUTO_ORBIT`]) at its derivation's bulk, inventory and
/// fluence, at the irradiation for the Bond albedo `albedo` given by hand: beyond the
/// maximum-greenhouse limit, formed there, with no heat flow (the passes' stand-in).
pub(crate) fn on_pluto_s_orbit_at(body: &DerivedBody, albedo: f64) -> AtmosphereInputs {
    let (kg, radius) = (Kilograms::from(body.mass()), Metres::from(body.radius()));
    let (a, e) = PLUTO_ORBIT;
    let a = Metres::new(a * METRES_PER_AU);
    let sun = historic_sun();
    let orbital = (GRAVITATIONAL_CONSTANT * SOLAR_MASS_KG / a.value()).sqrt();
    let escape = (2.0 * GRAVITATIONAL_CONSTANT * kg.value() / radius.value()).sqrt();
    let equilibrium = equilibrium_temperature(body.flux(), BondAlbedo::from_fraction(albedo));
    AtmosphereInputs {
        mass: kg,
        radius,
        material: SurfaceMaterial::of(&body.fractions()),
        envelope_fraction: body.fractions().envelope(),
        class: body.class(),
        inventory: *body.inventory(),
        formed: body.formed(),
        equilibrium,
        irradiation_temperature: equilibrium_temperature(
            body.flux(),
            BondAlbedo::from_fraction(0.0),
        ),
        heated: equilibrium,
        xuv_fluence: body.xuv_fluence(),
        saturated_xuv_flux: sun.xuv().saturated_flux(a, e),
        insolation: Insolation::BeyondMaximumGreenhouse,
        present_insolation: Insolation::BeyondMaximumGreenhouse,
        crust: Crust::Solid,
        molten_for: Years::ZERO,
        age: SOLAR_AGE,
        hottest_host: Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K),
        impact_speed: planet_impact_speed(
            MetresPerSecond::new(orbital),
            MetresPerSecond::new(escape),
        ),
        xuv_saturation_time: sun.xuv().saturation_time(),
        envelope_history: EnvelopeHistory::NONE,
        envelope_helium: None,
        flux: body.flux(),
        maximum_greenhouse_flux_ratio: sun_maximum_ratio(body.flux()),
    }
}

/// P14.T53.c's Pluto and Charon (decision-triton-and-e101 §1.8 item 4; Johnson et al. 2015, ApJ
/// 809, 43, as Young, Braga-Ribas and Johnson 2020 report: Pluto keeps its nitrogen and Charon has
/// lost its), every 24 hold in force:
///
/// - **a Pluto at its inventory** (1.303 × 10²² kg, 1,188.3 km, on its orbit), its N₂ held by its
///   ice, is `Tenuous` with N₂ and the CH₄ beside it (Gladstone et al. 2016, Science 351,
///   aad8866). Its N₂ lies within a factor of 10 of 1.1 Pa (11 ± 1 and 10 ± 1 µbar in 2015,
///   Gladstone et al.; 12.2 µbar in 2020, Sicardy et al. 2021, arXiv:2112.07764), wider than
///   Triton's since its measured season is not its orbit mean, at its measured Bond albedo (Buratti
///   et al. 2017, Icarus 287, 207: 0.72 ± 0.07) by hand. Through its own passes its albedo is
///   T51.b's 0.84, above that range (a recorded miss: fresh frost over its whole surface, none of
///   its dark tholin terrain), so its ice sits near 32.4 K and its N₂ near 0.04 Pa: stated, not
///   asserted. Its haze is not modelled at 24 (P14.T55.n; decision-p14-t53c-tenuous-aerosols): the
///   inventory's rules are not applied to a tenuous air, and its CH₄ by T24.c's rule is stated
///   with the aerosols' tests (`under_every_24_hold_pluto_s_haze_is_not_modelled_at_24`).
/// - **a Charon on Pluto's orbit** (1.586 × 10²¹ kg, 606 km) keeps no N₂: its vapour air has no
///   exobase, so its ice holds none, and T13.b's verdict, lost, stands.
#[test]
fn under_every_24_hold_a_pluto_keeps_its_nitrogen_ice_and_a_charon_none() {
    crate::planetary::testing::with_every_24_hold(|| {
        let pluto = on_pluto_s_orbit(1.303e22, 1_188.3);
        let air = pluto.atmosphere();
        let nitrogen = air.partial_pressures().of(Gas::Nitrogen).value();
        eprintln!(
            "Pluto through its passes: {}; its ice {:?}, Bond albedo {:.4}, T_eq {:.3} K",
            air_line("", air),
            air.held_ice_temperature(Gas::Nitrogen),
            pluto.albedo().value(),
            pluto.equilibrium_temperature().value(),
        );
        assert_eq!(air.state(), SurfaceState::Tenuous, "{air:?}");
        assert!(air.ice_held().retains(Gas::Nitrogen), "{air:?}");
        assert!(nitrogen > 0.0, "{air:?}");
        assert!(
            air.partial_pressures().of(Gas::Methane).value() > 0.0,
            "{air:?}"
        );
        let measured = atmosphere(&on_pluto_s_orbit_at(&pluto, 0.72));
        let nitrogen = measured.partial_pressures().of(Gas::Nitrogen).value();
        let methane = measured.partial_pressures().of(Gas::Methane).value();
        eprintln!(
            "Pluto at its measured 0.72: {}; CH4 {methane:.4e} Pa, its ice {:?}",
            air_line("", &measured),
            measured.held_ice_temperature(Gas::Nitrogen),
        );
        assert_eq!(measured.state(), SurfaceState::Tenuous, "{measured:?}");
        assert!(
            (1.1 / 10.0..=1.1 * 10.0).contains(&nitrogen),
            "Pluto's N2 at {nitrogen} Pa"
        );
        assert!(methane > 0.0, "{measured:?}");
        let charon = on_pluto_s_orbit(1.586e21, 606.0);
        let air = charon.atmosphere();
        eprintln!(
            "a Charon on Pluto's orbit: {}; Bond albedo {:.4}",
            air_line("", air),
            charon.albedo().value()
        );
        assert!(!air.retention().retains(Gas::Nitrogen), "{air:?}");
        assert!(!air.ice_held().retains(Gas::Nitrogen), "{air:?}");
        assert_same_bits(air.partial_pressures().of(Gas::Nitrogen).value(), 0.0);
        assert!(charon.inventory().nitrogen().value() > 0.0);
    });
}

/// P14.T53.c's record of a tenuous air, every 24 hold in force: the conditions of Pluto at its
/// measured 0.72 Bond albedo (`Tenuous`, its N₂ and argon held by their ice) publish its air's own
/// surface pressure, to the bit, and list its gases, while their climate is that of the same air
/// published `Airless`, to the bit, since the steps' climate holds no air under 100 Pa; and its ice
/// holds its argon past T53.a's shoreline, which would take 0.69% of an air's (its impact term),
/// so that its argon's escape is its vapour's alone, under a thousandth of that.
#[test]
fn under_every_24_hold_a_tenuous_record_lists_its_air_over_an_airless_climate() {
    use crate::planetary::hooks::surface::{
        OceanWeight, SeasonalSpin, SurfaceConditions, SurfaceInputs,
    };
    crate::planetary::testing::with_every_24_hold(|| {
        let pluto = on_pluto_s_orbit(1.303e22, 1_188.3);
        let inputs = on_pluto_s_orbit_at(&pluto, 0.72);
        let air = atmosphere(&inputs);
        assert_eq!(air.state(), SurfaceState::Tenuous, "{air:?}");
        let conditions = |air: &Atmosphere| {
            SurfaceConditions::derive(&SurfaceInputs {
                atmosphere: air,
                equilibrium_temperature: inputs.equilibrium,
                surface_gravity: pluto.surface_gravity(),
                radius: inputs.radius,
                material: inputs.material,
                lid: None,
                stellar_flux: WattsPerSquareMetre::from(pluto.flux()),
                xuv_flux: pluto.xuv_flux(),
                spin: SeasonalSpin {
                    obliquity: Radians::from(Degrees::new(119.6)),
                    rotation_period: Seconds::new(551_856.7),
                    solar_day: Some(Seconds::new(551_856.7)),
                },
                heat_flow: WattsPerSquareMetre::new(0.003),
                ocean_weight: OceanWeight::NONE,
            })
            .expect("Pluto has a surface")
        };
        let tenuous = conditions(&air);
        let airless = conditions(&Atmosphere {
            state: SurfaceState::Airless,
            ..air
        });
        let p = air.surface_pressure().expect("a surface").value();
        assert_same_bits(tenuous.surface_pressure().value(), p);
        assert!(
            tenuous
                .gases()
                .iter()
                .any(|&(id, _)| id == Gas::Nitrogen.substance())
        );
        assert!(tenuous.mean_molar_mass_g_per_mol().is_some());
        assert_same_bits(airless.surface_pressure().value(), 0.0);
        assert_eq!(airless.gases(), []);
        let (cold, warm) = (tenuous.climate_range(), airless.climate_range());
        assert_same_bits(cold.0.value(), warm.0.value());
        assert_same_bits(cold.1.value(), warm.1.value());
        assert_same_bits(
            tenuous.mean_surface_temperature().value(),
            airless.mean_surface_temperature().value(),
        );
        // The argon its ice holds is not eroded by the shoreline.
        assert!(air.ice_held().retains(Gas::Argon), "{air:?}");
        let argon = escape_of(&air)
            .element(Element::Ar)
            .expect("argon's outcome");
        let total = argon.escaped().value() + argon.airborne().value();
        let shoreline = escape_of(&air).shoreline().kept();
        eprintln!(
            "Pluto at 0.72: argon escaped {:.4e} kg of {total:.4e}; the shoreline keeps \
             {shoreline:.4}",
            argon.escaped().value()
        );
        // The shoreline would take 1 − its share of an air's argon; the ice-held argon loses far
        // less, its vapour's escape alone.
        assert!(shoreline < 1.0, "{shoreline}");
        assert!(
            argon.escaped().value() < 1e-3 * (1.0 - shoreline) * total,
            "{argon:?}"
        );
    });
}

// P14.T53.b part 3: the steam envelope and the partition's cap (decision-p14-t53b-part3).

/// `run` with every landed 25 hold in force, P14.T53.b part 3's, and with every 24 hold alone, as
/// the two results: part 3's movers against 24.
fn with_part_three<R>(run: impl Fn() -> R) -> (R, R) {
    use crate::planetary::testing::{with_every_24_hold, with_every_25_hold};
    (with_every_25_hold(&run), with_every_24_hold(&run))
}

/// An Earth of the Sun inside the runaway limit now ([`earth_inputs`] at `equilibrium`, K) holding
/// `water` kg, under a saturated X-ray and ultraviolet flux of `xuv` W m⁻².
fn wet_runaway_earth(water: f64, xuv: f64, equilibrium: f64) -> AtmosphereInputs {
    let inputs = earth_inputs(equilibrium, Insolation::InsideRunaway);
    AtmosphereInputs {
        inventory: VolatileInventory::new(
            Kilograms::new(water),
            inputs.inventory.carbon_dioxide(),
            inputs.inventory.nitrogen(),
            inputs.inventory.argon(),
        ),
        saturated_xuv_flux: WattsPerSquareMetre::new(xuv),
        ..inputs
    }
}

/// The free water a runaway with the atmosphere `air` and the inputs `inputs` keeps beyond its
/// escape's capacity, kg: its inventory's water after the cosmic shoreline, less the timed route's
/// loss (P14.T53.b part 3; decision-p14-t53b-part3 §4), none below zero.
fn steam_beyond_the_capacity(inputs: &AtmosphereInputs, air: &Atmosphere) -> f64 {
    let escape = air.escape().expect("every body's outcome from 24");
    let free = inputs.inventory.water().value() * escape.shoreline().kept();
    (free - escape.oxygen().water_lost().value()).max(0.0)
}

/// The relative difference of `ours` from `theirs`.
fn relative(ours: f64, theirs: f64) -> f64 {
    (ours / theirs - 1.0).abs()
}

/// The air's water column as a mass over the body of `inputs`, kg.
fn water_column_kg(inputs: &AtmosphereInputs, air: &Atmosphere) -> f64 {
    let (m, r) = (inputs.mass.value(), inputs.radius.value());
    let g = GRAVITATIONAL_CONSTANT * m / (r * r);
    air.inventory_weights().of(Gas::Water).value() * 4.0 * core::f64::consts::PI * r * r / g
}

/// P14.T53.b part 3 is held until the 24 → 25 bump: the rules in force are version 24's and
/// before, with no cap above a critical point, no steam kept and no steam envelope.
#[test]
fn the_steam_envelope_is_held_only_until_the_25_bump() {
    assert!(
        crate::GENERATOR_VERSION < supercritical::STEAM_ENVELOPE_VERSION,
        "the 24 → 25 bump puts P14.T53.b part 3 on the record (decision-p14-t53b-part3 §8): the \
         partition's cap at the critical isochore (`CriticalCap::Isochore`), the section from the \
         partition's reservoir (`Atmosphere::supercritical_reservoir`), the steam a runaway keeps \
         (`Body::divide`, `water_follows`) and the steam envelope's label \
         (`Atmosphere::published_state`). Delete `STEAM_ENVELOPE_VERSION`, `SteamEnvelopeRules` \
         with its `Held` arm, `CriticalCap::Uncapped` with the arms that read it, the tests' \
         `with_steam_envelope`, its switch in `with_every_25_hold` and this test; part 2's \
         air-based predicate (`hiding_reservoir`) then goes too, and the floor test takes \
         820e6ee00000008d.0900's exemption (decision-p14-t53b-part3 §13.1)"
    );
    assert_eq!(SteamEnvelopeRules::in_force(), SteamEnvelopeRules::Held);
    assert!(!steam_envelope_in_force());
    assert!(with_steam_envelope(SteamEnvelopeRules::Steam, || {
        !steam_envelope_in_force()
    }));
    assert!(with_steam_envelope(SteamEnvelopeRules::Steam, || {
        crate::planetary::chem::escape::with_rules(
            crate::planetary::chem::escape::EscapeRules::Batch,
            steam_envelope_in_force,
        )
    }));
}

/// The plan's test: a world of Earth's mass inside the runaway limit with a quarter of its mass in
/// water keeps nearly all of it beyond its escape's capacity, and its water above its critical
/// point holds a liquid-like reservoir of far more than a thousandth of its mass: its published
/// state is a steam envelope, its record's section the envelope's, and every figure its labelled
/// runaway's solve's. Under part 3's hold it is the runaway that lost all its water.
#[test]
fn under_every_25_hold_a_quarter_water_world_inside_the_runaway_limit_is_a_steam_envelope() {
    let inputs = wet_runaway_earth(0.25 * EARTH_MASS_KG, 0.3, 300.0);
    let (on, off) = with_part_three(|| {
        let air = atmosphere(&inputs);
        let g = MetresPerSecondSquared::new(
            GRAVITATIONAL_CONSTANT * inputs.mass.value() / math::powi(inputs.radius.value(), 2),
        );
        (
            air,
            air.supercritical_reservoir(g),
            has_surface_section(PlanetClass::Rocky, &air, g),
        )
    });
    let (air, reservoir, surface) = on;
    let reservoir = reservoir.expect("its steam hides its surface");
    eprintln!(
        "a quarter-water Earth inside the runaway limit: {:?} labelled {:?} at {:.1} K under \
         {:.1} bar, A {:.3}; its water {:.4e} kg kept of {:.4e}; {reservoir:?}",
        air.state(),
        air.published_state(),
        air.surface_temperature().value(),
        air.surface_pressure().unwrap().value() / PASCALS_PER_BAR,
        air.albedo().value(),
        water_column_kg(&inputs, &air),
        inputs.inventory.water().value(),
    );
    assert_eq!(air.published_state(), SurfaceState::SteamEnvelope);
    assert_eq!(air.state(), SurfaceState::RunawayGreenhouse);
    assert_eq!(air.water_state(), SurfaceState::RunawayGreenhouse);
    assert_eq!(reservoir.gas(), Gas::Water);
    assert!(reservoir.share() > 0.1, "{reservoir:?}");
    assert!(reservoir.temperature_ratio() >= 1.0, "{reservoir:?}");
    assert!(!surface);
    assert!(air.surface_temperature().value() >= 647.096);
    let kept = steam_beyond_the_capacity(&inputs, &air);
    assert!(kept > 0.99 * inputs.inventory.water().value(), "{kept}");
    assert!(relative(water_column_kg(&inputs, &air), kept) < 1e-12);
    // Its photometry and every figure are the runaway's solve's, its deck kept: the label alone
    // differs.
    assert_eq!(air.runaway_deck(), RunawayDeck::Kept);
    assert_same_bits(air.cloud_fraction(), 1.0);
    let (air, _, surface) = off;
    assert_eq!(air.published_state(), SurfaceState::RunawayGreenhouse);
    assert!(surface);
    assert_same_bits(air.partial_pressures().of(Gas::Water).value(), 0.0);
}

/// The plan's test: an Earth with Earth's water inside the runaway limit about the Sun loses its
/// oceans as today, its escape's capacity some ten oceans over its history: its atmosphere is the
/// same to the bit with part 3 and without.
#[test]
fn under_every_25_hold_an_earth_water_world_inside_the_runaway_limit_loses_its_oceans() {
    let inputs = earth_inputs(300.0, Insolation::InsideRunaway);
    let (on, off) = with_part_three(|| atmosphere(&inputs));
    let lost = on.escape().unwrap().oxygen().water_lost().value();
    eprintln!(
        "an Earth-water runaway: {:.3e} kg of water, {lost:.3e} kg lost; {:?} at {:.2} K",
        inputs.inventory.water().value(),
        on.state(),
        on.surface_temperature().value()
    );
    assert_same_bits(steam_beyond_the_capacity(&inputs, &on), 0.0);
    // To the bit: the `Debug` text tells −0 from +0, which `PartialEq` does not.
    assert_eq!(format!("{on:?}"), format!("{off:?}"));
    assert_eq!(on.published_state(), SurfaceState::RunawayGreenhouse);
    assert_same_bits(on.partial_pressures().of(Gas::Water).value(), 0.0);
}

/// decision-p14-t53b-part3 §4: a runaway that keeps water beyond its escape's capacity keeps it
/// as steam to 10⁻¹², its oxygen and carbon its division's to the bit. A present runaway's steam
/// lies all in its air over its surface; a past-only runaway's stands as its present class holds it,
/// a `Snowball`'s ice or a `Temperate` world's sea, over the water it kept, not its inventory's;
/// one that keeps none is part 1's to the bit.
#[test]
fn under_every_25_hold_a_runaway_keeps_its_water_beyond_its_escape_s_capacity() {
    use crate::planetary::hooks::figures::cover;
    // Under a saturated flux of 0.01 W m⁻², below its critical flux, an Earth loses some 0.34 of
    // an ocean over its history (9 k Φ, Luger and Barnes 2015's eq. A9). The past-only runaways
    // carry 2 mbar of carbon dioxide, so that their grey greenhouse, from the base, keeps their
    // steam under its saturation and their water condenses: under Earth's carbon airborne, a
    // runaway's, or 1.9 bar of it, the grey water feedback takes the kept 179 bar of steam all
    // airborne, a hothouse at 845 or 688 K (P14.T53.b part 1 (c)'s, in the passes).
    let present = wet_runaway_earth(1.4e21, 0.01, 300.0);
    let past = |t_eq: f64, now: Insolation| {
        let inputs = wet_runaway_earth(1.4e21, 0.01, t_eq);
        AtmosphereInputs {
            present_insolation: now,
            inventory: VolatileInventory::new(
                inputs.inventory.water(),
                Kilograms::new(1e17),
                inputs.inventory.nitrogen(),
                inputs.inventory.argon(),
            ),
            ..inputs
        }
    };
    let cases = [
        (present, SurfaceState::RunawayGreenhouse),
        (past(255.0, Insolation::Habitable), SurfaceState::Temperate),
        (
            past(150.0, Insolation::BeyondMaximumGreenhouse),
            SurfaceState::Snowball,
        ),
    ];
    for (inputs, water_state) in cases {
        let (on, off) = with_part_three(|| atmosphere(&inputs));
        let kept = steam_beyond_the_capacity(&inputs, &on);
        let at = format!(
            "{water_state:?}: {:?} at {:.2} K under {:.2} bar, {kept:.4e} kg kept",
            on.state(),
            on.surface_temperature().value(),
            on.surface_pressure().unwrap().value() / PASCALS_PER_BAR,
        );
        eprintln!("{at}");
        assert!(kept > 0.3 * 1.4e21 && kept < 1.4e21, "{at}");
        assert!(
            relative(water_column_kg(&inputs, &on), kept) < 1e-12,
            "{at}"
        );
        assert_eq!(on.water_state(), water_state, "{at}");
        assert_eq!(off.water_state(), SurfaceState::RunawayGreenhouse, "{at}");
        assert_eq!(on.state(), off.state(), "{at}");
        for gas in [Gas::Oxygen, Gas::CarbonDioxide, Gas::Nitrogen] {
            assert_same_bits(
                on.inventory_weights().of(gas).value(),
                off.inventory_weights().of(gas).value(),
            );
        }
        let (r, g) = (
            inputs.radius,
            MetresPerSecondSquared::new(
                GRAVITATIONAL_CONSTANT * inputs.mass.value() / math::powi(inputs.radius.value(), 2),
            ),
        );
        let surface = cover::surface_water(&on, inputs.inventory.water(), r, g, 1.0).value();
        if water_state == SurfaceState::RunawayGreenhouse {
            assert_same_bits(surface, 0.0);
            assert_same_bits(
                on.partial_pressures().of(Gas::Water).value(),
                on.inventory_weights().of(Gas::Water).value(),
            );
            assert_same_bits(
                on.surface_water_inventory(inputs.inventory.water()).value(),
                1.4e21,
            );
        } else {
            assert!(surface > 0.0, "{at}");
            assert_same_bits(
                on.surface_water_inventory(inputs.inventory.water()).value(),
                kept,
            );
            let whole = 1.4e21 / (4.0 * core::f64::consts::PI * r.value() * r.value());
            assert!(surface < whole, "{at}: {surface} against {whole}");
        }
        assert_same_bits(
            cover::surface_water(&off, inputs.inventory.water(), r, g, 1.0).value(),
            0.0,
        );
    }
    // One that keeps none is part 1's to the bit.
    for case in past_only_runaways() {
        let (on, off) = with_part_three(|| atmosphere(&case.inputs));
        assert_same_bits(steam_beyond_the_capacity(&case.inputs, &on), 0.0);
        assert_eq!(format!("{on:?}"), format!("{off:?}"));
        assert_eq!(format!("{on:?}"), format!("{:?}", case.air));
    }
}

/// The plan's test (decision-p14-t51a-partition §5; science-p14-t24f §5's 1,600–3,100 bar airs): a
/// methane world warmed above methane's 190.564 K critical point by its nitrogen holds about its
/// critical isochore's pressure of methane in its air at its surface temperature, its cap's
/// h^(1 − d) of it, the rest a liquid-like reservoir that hides its surface (part 2's tags); under
/// part 3's hold all of it is airborne.
#[test]
fn under_every_25_hold_a_nitrogen_warmed_methane_world_holds_its_isochore_of_methane() {
    // 2.5 kbar of methane, carried as its carbon, and 50 bar of nitrogen on an Earth at 185 K
    // beyond the maximum greenhouse, formed beyond the snow line 100 Myr ago, before its methane's
    // escape through its nitrogen and its photolysis take it (P14.T53.a's route takes it all over
    // the Sun's age).
    let weight = 9.82 / (4.0 * core::f64::consts::PI * 6.371e6 * 6.371e6);
    let methane = 2.5e8 / weight;
    let inputs = AtmosphereInputs {
        formed: SnowLineSide::Beyond,
        inventory: VolatileInventory::new(
            Kilograms::new(1e18),
            Kilograms::new(
                methane * Gas::CarbonDioxide.molar_mass_g_per_mol()
                    / Gas::Methane.molar_mass_g_per_mol(),
            ),
            Kilograms::new(5e6 / weight),
            Kilograms::new(6.6e16),
        ),
        age: Years::new(1e8),
        ..earth_inputs(185.0, Insolation::BeyondMaximumGreenhouse)
    };
    let g = MetresPerSecondSquared::new(9.82);
    let ((on, reservoir, surface), (off, _, _)) = with_part_three(|| {
        let air = atmosphere(&inputs);
        (
            air,
            air.supercritical_reservoir(g),
            has_surface_section(PlanetClass::Rocky, &air, g),
        )
    });
    let t_s = on.surface_temperature();
    let isochore = Gas::Methane
        .substance()
        .substance()
        .phase_diagram()
        .unwrap()
        .condensation_cap(t_s)
        .value();
    let held = methane_in_the_air(&on);
    eprintln!(
        "a nitrogen-warmed methane world: {:?} at {:.2} K under {:.1} bar; CH4 {:.2} bar in the \
         air against p* {:.2} bar, column {:.1} bar; {:?}; off: {:.2} K under {:.1} bar",
        on.state(),
        t_s.value(),
        on.surface_pressure().unwrap().value() / PASCALS_PER_BAR,
        held / PASCALS_PER_BAR,
        isochore / PASCALS_PER_BAR,
        on.inventory_weights().of(Gas::Methane).value() / PASCALS_PER_BAR,
        (reservoir, on.supercritical_column()),
        off.surface_temperature().value(),
        off.surface_pressure().unwrap().value() / PASCALS_PER_BAR,
    );
    assert!(t_s.value() > 190.564, "{on:?}");
    // Its cap is the line at its humidity's h^(1 − d) (the partition's pull between its branches,
    // h 0.5 for methane, METHANE_RELATIVE_HUMIDITY), so it lies between half the line and the line.
    assert!(
        held <= isochore && held >= METHANE_RELATIVE_HUMIDITY * isochore,
        "{held} against {isochore}"
    );
    let reservoir = reservoir.expect("its methane hides its surface");
    assert_eq!(reservoir.gas(), Gas::Methane);
    assert!(!surface);
    // Its reservoir is a supercritical fluid, above its critical point and under its melting
    // pressure.
    let column = on.supercritical_column().expect("its methane's column");
    assert_eq!(column.substance(), Gas::Methane.substance());
    assert!(
        column.temperature_ratio() > 1.0 && column.fluid().value() > 0.0,
        "{column:?}"
    );
    assert_eq!(on.published_state(), on.state());
    assert_same_bits(
        off.partial_pressures().of(Gas::Methane).value(),
        off.inventory_weights().of(Gas::Methane).value(),
    );
}

/// decision-p14-t53b-part3 §5, B2: with part 3 on, the table's Venus, Earth, Mars and Titan and the
/// measured Venus are the same to the bit as without it, and each keeps its surface; Venus's carbon
/// dioxide lies far under its isochore, all of it airborne.
#[test]
fn under_every_25_hold_part_three_leaves_venus_earth_mars_and_titan_to_the_bit() {
    let ((table_on, venus_on), (table_off, venus_off)) =
        with_part_three(|| (table(), measured_venus()));
    for name in ["Venus", "Earth", "Mars", "Titan"] {
        let (on, off) = (found(&table_on, name), found(&table_off, name));
        eprintln!(
            "the table's {name}: {:?} at {:.3} K",
            on.atmosphere().published_state(),
            on.atmosphere().surface_temperature().value()
        );
        assert_eq!(format!("{on:?}"), format!("{off:?}"), "{name}");
        assert_eq!(on.atmosphere().published_state(), on.atmosphere().state());
    }
    assert_eq!(format!("{venus_on:?}"), format!("{venus_off:?}"));
}

/// decision-p14-t53b-part3 §5: the steam envelope's label is the section's whatever the bulk's
/// gravity, so a moon or member whose bulk's gravity would not hide its reservoir still has no
/// surface section, and the record's surface refuses the state.
#[test]
fn under_every_25_hold_a_steam_envelope_has_no_surface_under_any_gravity() {
    use crate::planetary::record::Surface;
    let inputs = wet_runaway_earth(0.25 * EARTH_MASS_KG, 0.3, 300.0);
    let (air, _) = with_part_three(|| atmosphere(&inputs));
    assert_eq!(air.published_state(), SurfaceState::SteamEnvelope);
    crate::planetary::testing::with_every_25_hold(|| {
        for g in [9.82, 1e3, 1e6] {
            let g = MetresPerSecondSquared::new(g);
            assert!(!has_surface_section(PlanetClass::Rocky, &air, g), "{g:?}");
        }
        // Under 10⁶ m s⁻² its reservoir would not hide its surface: the label alone refuses it.
        let strong = MetresPerSecondSquared::new(1e6);
        assert!(air.supercritical_reservoir(strong).is_none(), "{air:?}");
    });
    assert!(Surface::refuses(SurfaceState::SteamEnvelope));
    assert!(Surface::refuses(SurfaceState::GasEnvelope));
    assert!(!Surface::refuses(SurfaceState::RunawayGreenhouse));
}

// P14.T53.a's thick-air closure (decision-p14-t53a-thick-co2 §3, §7.3): beyond 100 bar the air
// beneath its top 100 bar follows the dry adiabat, `T_s` = min(`T_grey`, `T_ad`).

/// The partial pressures of the gases `bars`, each its column weight in bar, the rest none.
fn air_of(bars: &[(Gas, f64)]) -> PartialPressures {
    PartialPressures::from_fn(|gas| {
        Pascals::new(
            bars.iter()
                .filter(|(g, _)| *g == gas)
                .fold(0.0, |sum, (_, bar)| sum + bar * PASCALS_PER_BAR),
        )
    })
}

/// An air drawn by `lcg` of the surface pressure `bar` by weight: carbon dioxide, nitrogen,
/// water, hydrogen, methane, oxygen and argon in drawn shares, one of them dominant, each share a
/// draw cubed so that minor gases span decades; over a base temperature of 100–800 K and a surface
/// gravity of 2–30 m s⁻².
fn drawn_air(lcg: &mut Lcg, bar: f64) -> (PartialPressures, Kelvin, MetresPerSecondSquared) {
    const GASES: [Gas; 7] = [
        Gas::CarbonDioxide,
        Gas::Nitrogen,
        Gas::Water,
        Gas::Hydrogen,
        Gas::Methane,
        Gas::Oxygen,
        Gas::Argon,
    ];
    let dominant = usize::try_from(lcg.next_below(7)).expect("under 7");
    let mut shares = [0.0; 7];
    for (k, share) in shares.iter_mut().enumerate() {
        let u = lcg.next_f64();
        *share = u * u * u * if k == dominant { 50.0 } else { 1.0 };
    }
    let total = shares.iter().fold(0.0, |sum, s| sum + s);
    let bars: Vec<(Gas, f64)> = GASES
        .iter()
        .zip(shares)
        .map(|(&gas, share)| (gas, bar * share / total))
        .collect();
    let base = Kelvin::new(100.0 + 700.0 * lcg.next_f64());
    let gravity = MetresPerSecondSquared::new(2.0 + 28.0 * lcg.next_f64());
    (air_of(&bars), base, gravity)
}

/// The surface of the air `partials` over the base `base` under the gravity `gravity`, its grey
/// surface, whether the closure or the ceiling `ceiling` binds, and the closure's (`τ_a`, `T_a`,
/// `T_ad`), under the greenhouse constants in force.
fn closure_at(
    ceiling: GreenhouseCeiling,
    partials: &PartialPressures,
    base: Kelvin,
    gravity: MetresPerSecondSquared,
) -> (f64, f64, bool, Option<(f64, Kelvin, Kelvin)>) {
    let constants = GreenhouseConstants::in_force();
    let tau = constants.optical_depth(partials, base, gravity);
    (
        ceiling
            .surface_temperature(&constants, (partials, tau), base, gravity)
            .value(),
        grey_surface_temperature(base, tau).value(),
        ceiling.binds(&constants, (partials, tau), base, gravity),
        constants.thick_air(partials, base, gravity),
    )
}

/// The measured Venus's air by weight (90.42 bar of carbon dioxide and 2.088 of nitrogen) made
/// `bar` thick, at its gravity, 8.870 m s⁻², and the generator's equilibrium temperature for it,
/// 229.06 K (decision-p14-t53a-thick-co2 §5).
fn venus_column(bar: f64) -> (PartialPressures, Kelvin, MetresPerSecondSquared) {
    let co2 = 90.42 / (90.42 + 2.088);
    (
        air_of(&[
            (Gas::CarbonDioxide, bar * co2),
            (Gas::Nitrogen, bar * (1.0 - co2)),
        ]),
        Kelvin::new(229.06),
        MetresPerSecondSquared::new(8.870),
    )
}

/// The closure leaves every air at or under 100 bar to the bit (decision-p14-t53a-thick-co2 §7.3):
/// over 4,000 drawn airs from 1 mbar to just under the anchor and an air at it exactly, under
/// either ceiling, the surface and whether either binds are the lifted closure's, and with no
/// ceiling the surface is the grey's.
#[test]
fn under_the_escape_batch_the_thick_air_closure_leaves_every_air_to_100_bar_to_the_bit() {
    with_rules(EscapeRules::Batch, || {
        let mut lcg = Lcg::new(0x0053_a0c1_0000_0001);
        let mut airs: Vec<_> = (0..4_000)
            .map(|_| {
                let bar = math::exp10(-3.0 + 4.999_99 * lcg.next_f64());
                drawn_air(&mut lcg, bar)
            })
            .collect();
        // At the anchor exactly: carbon dioxide and its nitrogen summing to 10⁷ Pa to the bit.
        let co2 = 9.774e6;
        airs.push((
            PartialPressures::from_fn(|gas| match gas {
                Gas::CarbonDioxide => Pascals::new(co2),
                Gas::Nitrogen => Pascals::new(1e7 - co2),
                _ => Pascals::ZERO,
            }),
            Kelvin::new(229.06),
            MetresPerSecondSquared::new(8.870),
        ));
        assert_same_bits(
            airs.last().expect("the anchor's air").0.total().value(),
            THICK_AIR_ANCHOR_PRESSURE.value(),
        );
        for (partials, base, gravity) in &airs {
            assert!(partials.total() <= THICK_AIR_ANCHOR_PRESSURE);
            for ceiling in [
                GreenhouseCeiling::None,
                GreenhouseCeiling::MaximumGreenhouse,
            ] {
                let (t, grey, binds, closure) = closure_at(ceiling, partials, *base, *gravity);
                let (lifted, _, lifted_binds, _) =
                    without_thick_air(|| closure_at(ceiling, partials, *base, *gravity));
                assert!(closure.is_none(), "{partials:?}");
                assert_same_bits(t, lifted);
                assert_eq!(binds, lifted_binds);
                if ceiling == GreenhouseCeiling::None {
                    assert_same_bits(t, grey);
                }
            }
        }
    });
}

/// The closure is continuous through its anchor (decision-p14-t53a-thick-co2 §3, §7.3): a Venus
/// column at 100 bar is the grey to the bit, and at 100 (1 ± 10⁻⁹) bar its surface moves under
/// 10⁻⁸ of it, the adiabat just beyond it within 10⁻⁸ of its grey top.
#[test]
fn under_the_escape_batch_the_thick_air_closure_is_continuous_through_its_anchor() {
    with_rules(EscapeRules::Batch, || {
        let at = |bar: f64| {
            let (partials, base, gravity) = venus_column(bar);
            closure_at(GreenhouseCeiling::None, &partials, base, gravity)
        };
        // At the anchor the column's carbon dioxide and nitrogen sum to its pressure to the bit.
        let (partials, ..) = venus_column(100.0);
        assert_same_bits(partials.total().value(), THICK_AIR_ANCHOR_PRESSURE.value());
        let (anchor, grey, binds, closure) = at(100.0);
        assert!(closure.is_none() && !binds);
        assert_same_bits(anchor, grey);
        for bar in [100.0 * (1.0 - 1e-9), 100.0 * (1.0 + 1e-9)] {
            let (t, grey, _, closure) = at(bar);
            eprintln!("{bar:.10} bar: T_s {t:.12} K, grey {grey:.12} K, closure {closure:?}");
            assert!(relative(t, anchor) < 1e-8, "{bar}: {t} against {anchor}");
            if let Some((_, top, adiabat)) = closure {
                assert!(relative(adiabat.value(), top.value()) < 1e-8);
                assert!(relative(top.value(), grey) < 1e-8);
            }
        }
        assert!(at(100.0 * (1.0 + 1e-9)).3.is_some());
    });
}

/// The closure never warms (decision-p14-t53a-thick-co2 §3, §7.3): over 4,000 drawn airs from 100
/// bar to 20 kbar the surface is finite, positive and at most the grey; at least the top's grey
/// temperature `T_a`, which the adiabat warms downward from; and `binds` holds exactly where the
/// surface lies under the grey. Under the maximum-greenhouse ceiling too, its surface is the
/// lesser of the closure's and the ceiling, to the bit, and `binds` holds where either binds. The
/// shares that bind are printed.
#[test]
fn under_the_escape_batch_the_thick_air_closure_never_warms() {
    with_rules(EscapeRules::Batch, || {
        let mut lcg = Lcg::new(0x0053_a0c1_0000_0002);
        let (mut bound, mut capped_bound) = (0, 0);
        let n = 4_000;
        for _ in 0..n {
            let bar = math::exp10(2.0 + 2.301 * lcg.next_f64());
            let (partials, base, gravity) = drawn_air(&mut lcg, bar);
            let (t, grey, binds, closure) =
                closure_at(GreenhouseCeiling::None, &partials, base, gravity);
            let (_, top, adiabat) = closure.expect("beyond the anchor");
            assert!(t.is_finite() && t > 0.0, "{partials:?}: {t}");
            assert!(t <= grey, "{partials:?}: {t} above its grey {grey}");
            assert!(
                top.value() <= t,
                "{partials:?}: {t} under its top's {top:?}"
            );
            assert!(adiabat.value().is_finite(), "{partials:?}");
            assert_eq!(binds, t < grey, "{partials:?}");
            bound += u32::from(binds);
            // Beside the maximum-greenhouse ceiling: the lesser of the closure's surface and the
            // ceiling, which binds where either does.
            let (capped, _, capped_binds, _) = closure_at(
                GreenhouseCeiling::MaximumGreenhouse,
                &partials,
                base,
                gravity,
            );
            let ceiling = GreenhouseCeiling::MaximumGreenhouse
                .temperature(&GreenhouseConstants::in_force(), &partials, base, gravity)
                .value();
            assert_same_bits(capped, t.min(ceiling));
            assert_eq!(capped_binds, capped < grey, "{partials:?}");
            capped_bound += u32::from(capped_binds && !binds);
        }
        eprintln!(
            "the closure binds on {bound} of {n} drawn airs at 100 bar–20 kbar; beside the \
             ceiling, the ceiling alone on {capped_bound} more"
        );
        assert!(bound > 0 && bound < n);
    });
}

/// A kilobar of nitrogen is the grey to the bit, its √p greenhouse subadiabatic; a kilobar of
/// hydrogen, whose term grows as p, moves a little, stated (decision-p14-t53a-thick-co2 §7.3).
/// Both over a base of 300 K at standard gravity.
///
/// At this 300 K base it moves 5.87%, from a grey of 2,153.7 K to 2,027.3 K, pinned under 6%
/// (decision-p14-t53a-thick-co2 §11.3, which confirms the pin: the ruling's "under 5%" was set
/// from a 200 K case, and the move grows with the base as hydrogen's `c_p` rises, −3.1% at 200 K
/// and −6.7% at 350 K on NIST's). Its grey profile's deep slope is the
/// linear term's ¼ (2,153.7 K at 1 kbar over its top's 1,212.1 K at 100 bar, a slope of 0.2496),
/// against hydrogen's dry adiabat's α R ÷ `c_p` of 0.223 over 1,200–2,000 K, where its `c_p` ÷ R
/// is about 3.9: slightly superadiabatic, so the closure binds.
#[test]
fn under_the_escape_batch_a_kilobar_of_nitrogen_is_the_grey_and_of_hydrogen_moves_a_little() {
    with_rules(EscapeRules::Batch, || {
        let (base, gravity) = (Kelvin::new(300.0), STANDARD_GRAVITY);
        let nitrogen = air_of(&[(Gas::Nitrogen, 1_000.0)]);
        let (t, grey, binds, closure) =
            closure_at(GreenhouseCeiling::None, &nitrogen, base, gravity);
        eprintln!("1 kbar of N₂: T_s {t:.3} K, grey {grey:.3} K, closure {closure:?}");
        assert!(!binds);
        assert_same_bits(t, grey);
        let hydrogen = air_of(&[(Gas::Hydrogen, 1_000.0)]);
        let (t, grey, binds, closure) =
            closure_at(GreenhouseCeiling::None, &hydrogen, base, gravity);
        eprintln!(
            "1 kbar of H₂ at a {:.0} K base: T_s {t:.3} K, grey {grey:.3} K ({:.4}% under it), \
             closure {closure:?}",
            base.value(),
            100.0 * (1.0 - t / grey)
        );
        assert!(t <= grey && relative(t, grey) < 0.06, "{t} against {grey}");
        assert_eq!(binds, t < grey);
    });
}

/// The measured Venus's air made 150–1,000 bar thick (decision-p14-t53a-thick-co2 §5): the
/// closure's surface within 3% of the ruling's replica (on NIST's `c_p`, against P14.T49.b's
/// here), far under the batch's grey, and 3.1–6.1% above version 23's √p form, each stated. At the
/// measured Venus's own 92.51 bar the closure is absent.
#[test]
fn under_the_escape_batch_a_thicker_venus_takes_the_dry_adiabat_beyond_100_bar() {
    let replica = [
        (150.0, 807.0),
        (200.0, 841.0),
        (300.0, 890.0),
        (500.0, 956.0),
        (1_000.0, 1_051.0),
    ];
    let held: Vec<f64> = replica
        .iter()
        .map(|&(bar, _)| {
            let (partials, base, gravity) = venus_column(bar);
            closure_at(GreenhouseCeiling::None, &partials, base, gravity).0
        })
        .collect();
    with_rules(EscapeRules::Batch, || {
        let (partials, base, gravity) = venus_column(92.51);
        assert!(
            closure_at(GreenhouseCeiling::None, &partials, base, gravity)
                .3
                .is_none()
        );
        for (&(bar, expected), version_23) in replica.iter().zip(held) {
            let (partials, base, gravity) = venus_column(bar);
            let (t, grey, binds, closure) =
                closure_at(GreenhouseCeiling::None, &partials, base, gravity);
            let (tau_a, top, _) = closure.expect("beyond the anchor");
            eprintln!(
                "Venus at {bar} bar: T_s {t:.1} K (the replica's {expected} K), the batch's grey \
                 {grey:.1} K, version 23's {version_23:.1} K ({:+.2}%); τ_a {tau_a:.2}, T_a {:.2} \
                 K",
                100.0 * (t / version_23 - 1.0),
                top.value()
            );
            assert!(binds);
            assert!(relative(t, expected) < 0.03, "{bar} bar: {t} K");
        }
    });
}

/// Where the closure binds, P14.T24.e's published profile, which leaves the ground at (`T_s`,
/// `p_s`) on the same α and `c_p`(T), passes through the anchor at the top's grey temperature,
/// (`p_a`, `T_a`), within its 5 × 10⁻⁴ (decision-p14-t53a-thick-co2 §3, §7.3): Venus's air at
/// 150 bar to 3 kbar and carbon dioxide at 10 kbar.
#[test]
fn under_the_escape_batch_the_published_profile_passes_through_the_closure_s_anchor() {
    use crate::planetary::hooks::structure::{
        RadiativeConvective, StructureInputs, VerticalStructure, effective_temperature,
        skin_temperature, temperature_at,
    };
    with_rules(EscapeRules::Batch, || {
        let mut columns: Vec<_> = [150.0, 300.0, 1_000.0, 3_000.0]
            .into_iter()
            .map(venus_column)
            .collect();
        columns.push((
            air_of(&[(Gas::CarbonDioxide, 10_000.0)]),
            Kelvin::new(132.4),
            MetresPerSecondSquared::new(8.24),
        ));
        for (partials, base, gravity) in &columns {
            let (t, _, binds, closure) =
                closure_at(GreenhouseCeiling::None, partials, *base, *gravity);
            let (_, top, _) = closure.expect("beyond the anchor");
            assert!(binds);
            let (fractions, len) = air_mixture(partials);
            let profile = RadiativeConvective::derive(&StructureInputs {
                surface_temperature: Kelvin::new(t),
                surface_pressure: partials.total(),
                gases: &fractions[..len],
                condensable: None,
                skin_temperature: skin_temperature(effective_temperature(*base, Kelvin::ZERO)),
            })
            .expect("a thick air's profile");
            let at_anchor = temperature_at(
                &VerticalStructure::RadiativeConvective(profile),
                THICK_AIR_ANCHOR_PRESSURE,
            );
            eprintln!(
                "{:.0} bar: T_s {t:.3} K; the profile at 100 bar {:.4} K, T_a {:.4} K",
                partials.total().value() / PASCALS_PER_BAR,
                at_anchor.value(),
                top.value()
            );
            assert!(relative(at_anchor.value(), top.value()) < 5e-4);
        }
    });
}

/// A runaway Earth at `T_eq` 300 K holding thirty times Earth's carbon dioxide per unit mass, all
/// of it airborne: about 2.1 kbar.
fn kilobar_runaway() -> AtmosphereInputs {
    let earth = earth_inputs(300.0, Insolation::InsideRunaway);
    AtmosphereInputs {
        inventory: VolatileInventory::new(
            earth.inventory.water(),
            Kilograms::new(30.0 * earth.inventory.carbon_dioxide().value()),
            earth.inventory.nitrogen(),
            earth.inventory.argon(),
        ),
        ..earth
    }
}

/// The closure's figures under the batch are pinned to the bit, and a body it binds on derives
/// the same twice (the sim-determinism skill: an algebraically equal rewrite moves the bits):
/// `τ_a`, `T_a`, `T_ad` and `T_s` of the Venus column at 150, 300 and 1,000 bar and of 10 kbar of
/// carbon dioxide at the guard's 42002cb200000015.0700's `T_eq` and gravity, each printed in full;
/// and the kilobar runaway ([`kilobar_runaway`]), derived twice.
#[test]
fn under_the_escape_batch_the_thick_air_closure_s_figures_are_pinned_and_the_same_twice() {
    const PINNED: [(f64, f64, f64, f64); 4] = [
        (
            160.915_677_727_055_42,
            760.781_751_462_361_6,
            806.913_823_706_265,
            806.913_823_706_265,
        ),
        (
            160.915_677_727_055_42,
            760.781_751_462_361_6,
            890.575_185_189_469,
            890.575_185_189_469,
        ),
        (
            160.915_677_727_055_45,
            760.781_751_462_361_6,
            1_051.574_189_117_547_1,
            1_051.574_189_117_547_1,
        ),
        (
            177.192_211_659_632_53,
            450.379_270_191_318_37,
            906.484_658_827_244,
            906.484_658_827_244,
        ),
    ];
    with_rules(EscapeRules::Batch, || {
        let mut columns: Vec<_> = [150.0, 300.0, 1_000.0]
            .into_iter()
            .map(venus_column)
            .collect();
        columns.push((
            air_of(&[(Gas::CarbonDioxide, 10_000.0)]),
            Kelvin::new(132.4),
            MetresPerSecondSquared::new(8.24),
        ));
        let figures: Vec<(f64, f64, f64, f64)> = columns
            .iter()
            .map(|(partials, base, gravity)| {
                let (t, _, _, closure) =
                    closure_at(GreenhouseCeiling::None, partials, *base, *gravity);
                let (tau_a, top, adiabat) = closure.expect("beyond the anchor");
                (tau_a, top.value(), adiabat.value(), t)
            })
            .collect();
        for figures in &figures {
            eprintln!("{figures:?},");
        }
        for (figures, pinned) in figures.iter().zip(PINNED) {
            for (actual, expected) in [
                (figures.0, pinned.0),
                (figures.1, pinned.1),
                (figures.2, pinned.2),
                (figures.3, pinned.3),
            ] {
                assert_same_bits(actual, expected);
            }
        }
        let inputs = kilobar_runaway();
        assert_eq!(atmosphere(&inputs), atmosphere(&inputs));
    });
}

/// Under the escape's hold, to version 23, the closure is absent: P14.T57.a's constants carry no
/// anchor, so a kilobar carbon dioxide air's surface is the grey's to the bit, and a kilobar
/// carbon dioxide runaway derives the same with the closure lifted. Under the batch the same body
/// takes it: its surface under the lifted one's, and at most its grey and its host (stated).
#[test]
fn under_the_escape_s_hold_the_thick_air_closure_is_absent() {
    let thick = air_of(&[(Gas::CarbonDioxide, 1_000.0), (Gas::Nitrogen, 3.0)]);
    let (base, gravity) = (Kelvin::new(300.0), STANDARD_GRAVITY);
    assert_eq!(EscapeRules::in_force(), EscapeRules::Held);
    assert!(
        GreenhouseConstants::FITTED
            .thick_air(&thick, base, gravity)
            .is_none()
    );
    let (t, grey, binds, _) = closure_at(GreenhouseCeiling::None, &thick, base, gravity);
    assert!(!binds);
    assert_same_bits(t, grey);
    assert!(
        GreenhouseConstants::BATCH
            .thick_air(&thick, base, gravity)
            .is_some()
    );
    let inputs = kilobar_runaway();
    let held = atmosphere(&inputs);
    assert_eq!(held, without_thick_air(|| atmosphere(&inputs)));
    let (batch, lifted) = with_rules(EscapeRules::Batch, || {
        (
            atmosphere(&inputs),
            without_thick_air(|| atmosphere(&inputs)),
        )
    });
    let line = |air: &Atmosphere| {
        format!(
            "{:?} T_s {:.2} K, p_s {:.2} bar, τ {:.4e}",
            air.state(),
            air.surface_temperature().value(),
            air.surface_pressure()
                .map_or(0.0, |p| p.value() / PASCALS_PER_BAR),
            air.optical_depth()
        )
    };
    eprintln!(
        "a kilobar runaway: held {}\n  batch {}\n  batch, lifted {}",
        line(&held),
        line(&batch),
        line(&lifted)
    );
    assert!(batch.surface_pressure().expect("an air") > THICK_AIR_ANCHOR_PRESSURE);
    assert!(batch.surface_temperature() < lifted.surface_temperature());
    assert!(batch.surface_temperature().value().is_finite());
    assert!(batch.surface_temperature() < inputs.hottest_host);
}
