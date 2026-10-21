//! P14.T24.b's surface water, oceans, ices and cloud, with θ, on the Solar System table and on
//! their laws' cases (science-p14-t24b-laws §4.2–4.5).

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::MassFractions;
use crate::planetary::derive::atmosphere::SurfaceMaterial;
use crate::planetary::derive::solar::SOLAR_AGE;
use crate::planetary::hooks::figures::relief::{
    EARTH_CONTINENTAL_FRACTION, Population, Relief, ReliefInputs,
};
use crate::planetary::hooks::figures::tests::{
    europa, figures_from, figures_table, inputs_of, measured_europa,
};
use crate::planetary::hooks::figures::{
    FiguresInputs, GlobalFigures, TectonicRegime, VolatileHistory,
};
use crate::planetary::hooks::surface::tests::{inputs_of as surface_inputs_of, spin_of};
use crate::planetary::hooks::surface::{OceanWeight, SeasonalSpin, SurfaceInputs, area_below};
use crate::units::consts::SECONDS_PER_DAY;
use crate::units::{Degrees, MetresPerSecondSquared, PerSquareKilometre, Radians, Seconds, Years};

/// The conditions of the table's body `name`, derived as `body`, at the figures `figures`' heat
/// flow and ocean weight under the spin `spin`, as `hooks_at` takes them.
fn conditions_under(
    body: &DerivedBody,
    figures: &GlobalFigures,
    spin: SeasonalSpin,
) -> SurfaceConditions {
    SurfaceConditions::derive(&SurfaceInputs {
        heat_flow: figures.heat_flow(),
        ocean_weight: figures.ocean_weight(),
        ..surface_inputs_of(body, spin)
    })
    .expect("the table's bodies have surfaces")
}

/// The table's bodies with their figures at both stages and their conditions, each under its own
/// spin, with no volatile history (P14.T24.b's second stage).
pub(in crate::planetary::hooks::figures) fn table_surfaces()
-> Vec<(&'static str, DerivedBody, GlobalFigures, SurfaceConditions)> {
    figures_table()
        .into_iter()
        .map(|(name, body, figures)| {
            let conditions = conditions_under(&body, &figures, spin_of(name));
            let figures = figures.with_surface(&conditions, VolatileHistory::NONE);
            (name, body, figures, conditions)
        })
        .collect()
}

fn found<'a>(
    table: &'a [(&'static str, DerivedBody, GlobalFigures, SurfaceConditions)],
    name: &str,
) -> &'a (&'static str, DerivedBody, GlobalFigures, SurfaceConditions) {
    table.iter().find(|(n, ..)| *n == name).unwrap()
}

/// Earth2014's mobile lid at `f_c` = 0.405 with no basin or constructional share.
fn earth_hypsometry() -> Hypsometry {
    *Relief::of(&ReliefInputs {
        regime: TectonicRegime::MobileLid,
        continental_fraction: EARTH_CONTINENTAL_FRACTION,
        crust: Crust::Silicate,
        surface_gravity: MetresPerSecondSquared::new(9.82),
        radius: Metres::new(6.371e6),
        crater_density: PerSquareKilometre::ZERO,
        volcanism: 0.0,
        lithosphere: Metres::ZERO,
    })
    .hypsometry()
}

/// The ruling's checks of the filling (`ocean.py`): Earth's 1.335 × 10⁹ km³ over 5.101 × 10⁸ km²,
/// 2,617 m, fills 0.731 of its two populations (0.7095 measured), where one Gaussian of their
/// `σ_h` would fill 0.831 ± 0.005 (science-p14-t24b-laws §4.2).
#[test]
fn earth_s_ocean_fills_its_two_populations() {
    let depth = Metres::new(1.335e18 / 5.101e14);
    let two = earth_hypsometry();
    let (level, fraction) = two.fill(depth);
    eprintln!(
        "Earth: {fraction} of the surface, its level {} m",
        level.value()
    );
    assert!((fraction - 0.731).abs() < 0.001, "{fraction}");
    let sigma = (0.405 * 0.595 * math::powi(4_690.0, 2)
        + 0.405 * math::powi(1_080.0, 2)
        + 0.595 * math::powi(930.0, 2))
    .sqrt();
    let one = Hypsometry::One(Population {
        weight: 1.0,
        mean: Metres::ZERO,
        width: Metres::new(sigma),
    });
    let (_, single) = one.fill(depth);
    assert!((single - 0.831).abs() < 0.005, "{single}");
}

/// The filling inverts to 10⁻⁹, is monotone in the inventory, and gives exactly 1 from a layer at
/// the upper population's top, `μ_c` + 3 `σ_c′`, and none from no water (science-p14-t24b-laws
/// §4.5).
#[test]
fn the_filling_inverts_and_is_monotone() {
    for hypsometry in [
        earth_hypsometry(),
        Hypsometry::One(Population {
            weight: 1.0,
            mean: Metres::ZERO,
            width: Metres::new(900.0),
        }),
    ] {
        let mut last = 0.0;
        for step in 1..=400 {
            let depth = Metres::new(25.0 * f64::from(step));
            let (level, fraction) = hypsometry.fill(depth);
            let volume = hypsometry.volume_below(level);
            assert!(
                (volume - depth).value().abs() < 1e-9 * depth.value().max(1.0),
                "{} m: {} m",
                depth.value(),
                volume.value()
            );
            assert!(
                fraction >= last && fraction <= 1.0,
                "{} m: {fraction}",
                depth.value()
            );
            last = fraction;
        }
        let top = hypsometry
            .populations()
            .iter()
            .fold(f64::NEG_INFINITY, |top, p| {
                top.max((p.mean + p.width * 3.0).value())
            });
        assert_same_bits(hypsometry.fill(Metres::new(top)).1, 1.0);
        assert_same_bits(hypsometry.fill(Metres::new(top * 2.0)).1, 1.0);
        assert_same_bits(hypsometry.fill(Metres::ZERO).1, 0.0);
    }
    // An Earth's ocean world comes at about 6 km of layer, and a 0.9 km stagnant lid's at 2.7 km.
    let (_, at_five) = earth_hypsometry().fill(Metres::new(5_000.0));
    assert!(at_five < 1.0);
    let stagnant = Hypsometry::One(Population {
        weight: 1.0,
        mean: Metres::ZERO,
        width: Metres::new(900.0),
    });
    assert_same_bits(stagnant.fill(Metres::new(2_700.0)).1, 1.0);
}

/// The surface water: T13.a's water over 4πR² while O is at least the volcanic greenhouse's edge,
/// 2% of it after, and none where the surface holds none (science-p14-t24b-laws §4.2).
#[test]
fn the_surface_water_follows_the_outgassing() {
    let table = figures_table();
    for (name, body, figures) in &table {
        let kg = body.inventory().water().value();
        let r = Metres::from(body.radius()).value();
        let column = kg / (4.0 * core::f64::consts::PI * r * r);
        eprintln!(
            "{name}: {:?}, O {:.3}, water {column:.4e} kg m⁻², σ_s {:.4e}, f_pot {:.4}, θ {:.4}",
            body.atmosphere().state(),
            figures.outgassing(),
            figures.surface_water().value(),
            figures.potential_ocean(),
            figures.ocean_weight().get(),
        );
    }
    let of = |name: &str| table.iter().find(|(n, ..)| *n == name).unwrap();
    // The condensed water: T13.a's less the vapour T13.c holds in the air, p_H₂O ÷ g.
    let condensed = |body: &DerivedBody| {
        let r = Metres::from(body.radius()).value();
        body.inventory().water().value() / (4.0 * core::f64::consts::PI * r * r)
            - body.atmosphere().partial_pressures().of(Gas::Water).value()
                / body.surface_gravity().value()
    };
    let (_, earth, figures) = of("Earth");
    assert!((figures.surface_water().value() / condensed(earth) - 1.0).abs() < 1e-12);
    let r = Metres::from(earth.radius()).value();
    let column = earth.inventory().water().value() / (4.0 * core::f64::consts::PI * r * r);
    assert!((figures.surface_water().value() / column - 1.0).abs() < 1e-3);
    let (_, mars, figures) = of("Mars");
    assert!(figures.outgassing() < VOLCANIC_GREENHOUSE_OUTGASSING);
    assert!(
        (figures.surface_water().value() / condensed(mars) - DRY_SURFACE_WATER_SHARE).abs() < 1e-12
    );
    // The runaway Venus has lost its oceans, and the airless bodies keep none at their surface.
    for name in ["Venus", "Mercury", "Moon", "Ganymede", "Ceres"] {
        assert_same_bits(of(name).2.surface_water().value(), 0.0);
    }
}

/// The published ocean (science-p14-t24b-laws §4.5): Earth's 0.70–0.76, and none on Mars, Venus
/// and the Moon; `surface_liquids` holds `H2O` alone and sums to it to 10⁻¹².
#[test]
fn the_table_s_oceans_are_published_where_water_is_liquid() {
    let table = table_surfaces();
    for (name, _, figures, conditions) in &table {
        eprintln!(
            "{name}: liquid {}, f_o {:.4}, f_ice {:.4}, clouds {}",
            conditions.liquid_water(),
            figures.ocean_fraction(),
            figures.ice_fraction(),
            figures.cloud_fraction()
        );
        let liquids = figures.surface_liquids();
        assert_eq!(liquids.len(), 1, "{name}");
        assert_eq!(liquids[0].0, Gas::Water.substance(), "{name}");
        let sum = liquids.iter().fold(0.0, |sum, &(_, f)| sum + f);
        assert!((sum - figures.ocean_fraction()).abs() < 1e-12, "{name}");
        assert!((0.0..=1.0).contains(&figures.ocean_fraction()), "{name}");
    }
    let ocean = |name: &str| found(&table, name).2.ocean_fraction();
    assert!((0.70..0.76).contains(&ocean("Earth")), "{}", ocean("Earth"));
    for name in ["Mars", "Venus", "Moon"] {
        assert_same_bits(ocean(name), 0.0);
    }
}

/// The ice (science-p14-t24b-laws §4.5): Earth 0.01–0.07, band 5 as decision-p14-t54a-bands §4.3
/// asserts it on the batch's table Earth, Mars 0.005–0.02, Europa and a `Snowball` 1;
/// `surface_ices` holds `H2O` alone and sums to it to 10⁻¹².
#[test]
fn the_table_s_ices_are_the_ruling_s() {
    let table = table_surfaces();
    for (name, _, figures, _) in &table {
        let ices = figures.surface_ices();
        assert_eq!(ices.len(), 1, "{name}");
        assert_eq!(ices[0].0, Gas::Water.substance(), "{name}");
        let sum = ices.iter().fold(0.0, |sum, &(_, f)| sum + f);
        assert!((sum - figures.ice_fraction()).abs() < 1e-12, "{name}");
        assert!((0.0..=1.0).contains(&figures.ice_fraction()), "{name}");
    }
    let ice = |name: &str| found(&table, name).2.ice_fraction();
    assert!((0.005..0.02).contains(&ice("Mars")), "{}", ice("Mars"));
    // Titan is a `Snowball` whose whole profile lies below the melting point: wholly iced
    // (science-p14-t24b-laws §4.x.2).
    let (_, titan, figures, titan_conditions) = found(&table, "Titan");
    assert_eq!(titan.atmosphere().state(), SurfaceState::Snowball);
    assert!(titan_conditions.climate_range().1 < melting_point());
    assert_same_bits(figures.ice_fraction(), 1.0);
    assert_same_bits(melting_point().value(), 273.16);
    // A `Snowball` takes its profile's cold area below the melting point, uncapped by its surface
    // water: on Earth's profile, its high latitudes; on Venus's, none; and on Titan's, all.
    for (name, between) in [("Earth", true), ("Venus", false), ("Titan", false)] {
        let (_, _, _, conditions) = found(&table, name);
        let expected = conditions.cold_area(melting_point());
        for water in [0.0, 2.0e4, 2.0e6] {
            for crust in [Crust::Silicate, Crust::IceRich] {
                let ice = ice_fraction(&IceInputs {
                    conditions,
                    state: SurfaceState::Snowball,
                    crust,
                    surface_water: KilogramsPerSquareMetre::new(water),
                });
                assert_same_bits(ice, expected);
            }
        }
        assert_eq!(
            expected > 0.0 && expected < 1.0,
            between,
            "{name}: {expected}"
        );
    }
    // Ceres, ice-rich at McCord and Sotin's 17% water, keeps its ice at high latitudes.
    let (_, ceres_body, _, ceres_conditions) = found(&table, "Ceres");
    let wet_ceres = figures_from(&FiguresInputs {
        inventory_water: Kilograms::new(0.17 * Kilograms::from(ceres_body.mass()).value()),
        ..inputs_of(ceres_body, "Ceres")
    })
    .unwrap()
    .with_surface(ceres_conditions, VolatileHistory::NONE);
    assert_eq!(wet_ceres.impactors().crust(), Crust::IceRich);
    let ceres_ice = wet_ceres.ice_fraction();
    eprintln!("the wet Ceres: ice {ceres_ice}");
    assert!(ceres_ice > 0.0 && ceres_ice < 1.0, "{ceres_ice}");
    // Europa at its measured 102 K and 3.13° (Jupiter's obliquity), airless, lies under 110 K at
    // every latitude: its radiative profile peaks at 108.3 K.
    let obliquity = Radians::from(Degrees::new(3.13));
    let warmest = 102.0
        * math::powf(
            crate::planetary::hooks::surface::annual_insolation_at(obliquity, 0.0),
            0.25,
        );
    assert!(warmest < 110.0, "{warmest}");
    // The generated Europa, its measured bulk under its own air, is ice-rich and airless, but its
    // mean surface lies at 118.6 K, so its ice keeps to its high latitudes (plan 14's Risks,
    // deferred).
    let (figures, ..) = measured_europa();
    let derived = europa();
    let spin = SeasonalSpin {
        obliquity: Radians::from(Degrees::new(3.13)),
        rotation_period: Seconds::new(3.551 * SECONDS_PER_DAY),
        solar_day: Some(Seconds::new(3.554 * SECONDS_PER_DAY)),
    };
    let conditions = SurfaceConditions::derive(&SurfaceInputs {
        atmosphere: derived.atmosphere(),
        equilibrium_temperature: derived.equilibrium_temperature(),
        surface_gravity: MetresPerSecondSquared::new(1.315),
        radius: Metres::new(1.560_8e6),
        material: SurfaceMaterial::of(&MassFractions::solid(0.1, 0.82, 0.08)),
        stellar_flux: WattsPerSquareMetre::from(derived.flux()),
        xuv_flux: derived.xuv_flux(),
        spin,
        heat_flow: figures.heat_flow(),
        ocean_weight: figures.ocean_weight(),
    })
    .unwrap();
    let europa = figures.with_surface(&conditions, VolatileHistory::NONE);
    eprintln!(
        "Europa: {:?} at {} K, crust {:?}, warmest point {} K, ice {}",
        derived.atmosphere().state(),
        conditions.mean_surface_temperature().value(),
        europa.impactors().crust(),
        conditions.zonal_temperature(0.0).value(),
        europa.ice_fraction()
    );
    assert_eq!(europa.impactors().crust(), Crust::IceRich);
    assert_same_bits(
        europa.ice_fraction(),
        conditions.cold_area(AIRLESS_ICE_TEMPERATURE),
    );
    assert!((0.1..0.3).contains(&europa.ice_fraction()));
    // Band 5 (decision-p14-t54a-bands §4.3): the generated Earth's ice within 0.01–0.07, its
    // figure printed, the cover's equality held above, on the batch's table Earth at the passes'
    // stand-ins, its zonal pole near the 263.15 K edge, where the cap moves about 0.013 for each
    // kelvin the pole warms. Earth's measured perennial ice is 0.045–0.051 of the globe and its
    // annual-mean ice 0.068–0.078 (Fretwell et al. 2013, The Cryosphere 7, 375, Table 7; IPCC AR5
    // WGI Ch. 4, Tables 4.1–4.2; the NSIDC Sea Ice Index, v4, 1981–2010), and the cover gives the
    // measured Earth's profile 0.035 (`surface::tests`'s fit test).
    let earth = ice("Earth");
    eprintln!("the generated Earth's ice: {earth}");
    assert!((0.01..=0.07).contains(&earth), "Earth's ice {earth}");
}

/// An airless body's ice lasts below 110 K (Schorghofer and Williams 2020), and its cold area is
/// where its radiative profile, `T_s` q(x)^¼, lies below it: the generated Moon's and Mercury's
/// at their edges, and none of the Moon's ice, whose crust is silicate and holds no surface water.
///
/// The table's Ganymede is airless, and its ice is its cold area under 110 K. Its band about its
/// pole, which the edge gave at T13.c's 0.35, is stated, not asserted: under the frost's albedo
/// (`T_eq` 78 K against 109.6 K at its measured 0.35, decision-p14-t54a-ganymede §1.5) its
/// warmest zone lies under the edge, so the whole of it is cold. Its band returns with its icy
/// regolith's darkening, P14.T49.m's.
#[test]
fn an_airless_body_s_edge_is_110_k() {
    assert_same_bits(ice_temperature(SurfaceState::Airless).value(), 110.0);
    assert_same_bits(ice_temperature(SurfaceState::Temperate).value(), 263.15);
    let table = table_surfaces();
    for (name, threshold) in [("Moon", 200.0), ("Mercury", 300.0)] {
        let (_, body, _, conditions) = found(&table, name);
        assert_eq!(body.atmosphere().state(), SurfaceState::Airless, "{name}");
        let t_s = conditions.mean_surface_temperature().value();
        let obliquity = spin_of(name).obliquity;
        // The crossing by a fine bisection of the radiative profile.
        let radiative = |x: f64| {
            t_s * math::powf(
                crate::planetary::hooks::surface::annual_insolation_at(obliquity, x),
                0.25,
            )
        };
        let (mut low, mut high) = (0.0, 1.0);
        for _ in 0..60 {
            let middle = f64::midpoint(low, high);
            if radiative(middle) < threshold {
                high = middle;
            } else {
                low = middle;
            }
        }
        let area = conditions.cold_area(Kelvin::new(threshold));
        eprintln!("{name}: {area} below {threshold} K, the edge at x = {low}");
        assert!(area > 0.0 && area < 1.0, "{name}: {area}");
        // Linear between the points: within a quarter of the last half-step, even where the
        // insolation falls to nothing inside it, as Mercury's does at 0.034°.
        assert!(
            (area - (1.0 - low)).abs() < 2e-3,
            "{name}: {area} against {}",
            1.0 - low
        );
    }
    let (_, body, ganymede, conditions) = found(&table, "Ganymede");
    assert_eq!(body.atmosphere().state(), SurfaceState::Airless);
    let cold = conditions.cold_area(AIRLESS_ICE_TEMPERATURE);
    assert!((ganymede.ice_fraction() - cold).abs() < 1e-15);
    eprintln!(
        "Ganymede: its equator's zone at {:.2} K, {cold} of it below 110 K (P14.T49.m)",
        conditions.zonal_temperature(0.0).value()
    );
    assert_same_bits(found(&table, "Moon").2.ice_fraction(), 0.0);
}

/// A world locked 1:1 takes the eyeball's cap from its day and night means: none where its night
/// is at or above the threshold, and (1 + `μ_c`) ÷ 2 below it, the whole sphere once its
/// substellar point, `E_n` + (3/2)(`E_d` − `E_n`), is cold (science-p14-t24b-laws §4.3, with Cowan
/// and Agol's equator-weighted means).
#[test]
fn a_locked_world_takes_the_eyeball_s_cap() {
    let table = table_surfaces();
    for name in ["Earth", "Venus"] {
        let (_, body, figures, _) = found(&table, name);
        let locked = conditions_under(
            body,
            figures,
            SeasonalSpin {
                solar_day: None,
                ..spin_of(name)
            },
        );
        let (day, night) = locked.day_and_night();
        let (d, n) = (day.value(), night.value());
        eprintln!(
            "{name} locked: day {d:.1} K, night {n:.1} K, below 263.15 K {:.4}",
            locked.cold_area(PERENNIAL_ICE_TEMPERATURE)
        );
        assert_same_bits(locked.cold_area(night), 0.0);
        assert!((locked.cold_area(Kelvin::new(n * (1.0 + 1e-12))) - 0.5).abs() < 1e-9);
        let substellar = math::powf(
            math::powi(n, 4) + 1.5 * (math::powi(d, 4) - math::powi(n, 4)),
            0.25,
        );
        assert_same_bits(
            locked.cold_area(Kelvin::new(substellar * (1.0 + 1e-9))),
            1.0,
        );
        // At the dayside's own mean, `μ_c` is two thirds: five sixths of the sphere.
        assert!((locked.cold_area(day) - 5.0 / 6.0).abs() < 1e-12, "{name}");
    }
    // Venus's night lies far above 263.15 K: none.
    let (_, venus, figures, _) = found(&table, "Venus");
    let locked = conditions_under(
        venus,
        figures,
        SeasonalSpin {
            solar_day: None,
            ..spin_of("Venus")
        },
    );
    assert_same_bits(locked.cold_area(PERENNIAL_ICE_TEMPERATURE), 0.0);
}

/// The zonal profile at each latitude: its ends at x = 0 and 1 bit for bit, symmetric about the
/// equator, and its cold area continuous in the threshold.
#[test]
fn the_zonal_profile_joins_its_ends() {
    let table = table_surfaces();
    for (name, _, _, conditions) in &table {
        let (equator, pole) = (
            conditions.zonal_temperature(0.0).value(),
            conditions.zonal_temperature(1.0).value(),
        );
        assert_same_bits(equator - pole, conditions.equator_pole_contrast_k());
        for x in [0.1, 0.5, 0.9] {
            assert_same_bits(
                conditions.zonal_temperature(x).value(),
                conditions.zonal_temperature(-x).value(),
            );
        }
        // Next to each end the profile meets it.
        let near = |x: f64| conditions.zonal_temperature(x).value();
        assert!((near(1e-6) - equator).abs() < 1e-3 * equator, "{name}");
        assert!(
            (near(1.0 - 1e-9) - pole).abs() < 2e-3 * pole,
            "{name}: {} against {pole}",
            near(1.0 - 1e-9)
        );
        // The cold area is the profile's points' piecewise-linear area: monotone and continuous
        // in the threshold, from none to the whole sphere.
        let points = conditions.zonal_points();
        assert_eq!(points.len(), 66);
        assert_same_bits(points[0].1, equator);
        assert_same_bits(points[65].1, pole);
        assert_same_bits(
            points[32].1,
            conditions.zonal_temperature(points[32].0).value(),
        );
        for threshold in [100.0, 200.0, 263.15, 300.0] {
            let at = Kelvin::new(threshold);
            assert_same_bits(conditions.cold_area(at), area_below(&points, at));
        }
        let mut last = area_below(&points, Kelvin::new(0.0));
        assert_same_bits(last, 0.0);
        let mut threshold = 50.0;
        while threshold < 800.0 {
            threshold += 0.1;
            let area = area_below(&points, Kelvin::new(threshold));
            assert!(area >= last, "{name} at {threshold} K: {last} to {area}");
            let nudged = area_below(&points, Kelvin::new(threshold + 1e-9));
            assert!(
                (nudged - area).abs() < 1e-5,
                "{name} at {threshold} K: {area} to {nudged}"
            );
            last = area;
        }
        assert_same_bits(last, 1.0);
    }
}

/// The ice and the ocean are continuous in time: the table's Earth and Mars a thousand years
/// later move each by under 10⁻⁶.
#[test]
fn the_cover_is_continuous_in_time() {
    let table = figures_table();
    for name in ["Earth", "Mars"] {
        let (_, body, _) = table.iter().find(|(n, ..)| *n == name).unwrap();
        let surface = |age: f64| {
            let inputs = FiguresInputs {
                age: Years::new(age),
                ..inputs_of(body, name)
            };
            let figures = figures_from(&inputs).unwrap();
            let conditions = conditions_under(body, &figures, spin_of(name));
            figures.with_surface(&conditions, VolatileHistory::NONE)
        };
        let age = SOLAR_AGE.value();
        let (now, later) = (surface(age), surface(age + 1e3));
        assert!(
            (now.ice_fraction() - later.ice_fraction()).abs() < 1e-6,
            "{name}"
        );
        assert!(
            (now.ocean_fraction() - later.ocean_fraction()).abs() < 1e-6,
            "{name}"
        );
        assert!(
            (now.potential_ocean() - later.potential_ocean()).abs() < 1e-6,
            "{name}"
        );
    }
}

/// The cloud is the atmosphere's to the bit, the area mix's (P14.T51.b; science-p14-t24b-laws
/// §4.4), and θ reads the potential ocean: moving the liquid-water flag, by the second stage of
/// another body's conditions, moves the published ocean and not θ.
#[test]
fn the_cloud_is_the_atmosphere_s_and_theta_reads_the_potential_ocean() {
    let table = table_surfaces();
    for (name, body, figures, _) in &table {
        assert_same_bits(figures.cloud_fraction(), body.atmosphere().cloud_fraction());
        assert_eq!(
            figures.ocean_weight(),
            OceanWeight::of_potential_ocean(figures.potential_ocean()),
            "{name}"
        );
    }
    let (_, _, earth, wet) = found(&table, "Earth");
    let (_, _, _, dry) = found(&table, "Mars");
    assert!(wet.liquid_water() && !dry.liquid_water());
    let first = figures_table()
        .into_iter()
        .find(|(n, ..)| *n == "Earth")
        .unwrap()
        .2;
    let drier = first.with_surface(dry, VolatileHistory::NONE);
    assert_eq!(drier.ocean_weight(), earth.ocean_weight());
    assert_same_bits(drier.ocean_fraction(), 0.0);
    assert!(earth.ocean_fraction() > 0.7);
    assert_eq!(earth.ocean_weight(), OceanWeight::FULL);
}

/// A `Snowball` holds an ocean under its ice where its water outruns the ice that conducts its
/// heat: 651 ln(273.15 K ÷ `T_s`) ÷ F; no other state does.
#[test]
fn a_snowball_s_ocean_lies_under_its_ice() {
    let f = WattsPerSquareMetre::new(0.05);
    let t_s = Kelvin::new(200.0);
    let shell = 651.0 * math::ln(273.15 / 200.0) / 0.05;
    let water = |depth: f64| KilogramsPerSquareMetre::new(depth * 1_000.0);
    assert!(under_ice_ocean(
        SurfaceState::Snowball,
        water(shell * 1.01),
        t_s,
        f
    ));
    assert!(!under_ice_ocean(
        SurfaceState::Snowball,
        water(shell * 0.99),
        t_s,
        f
    ));
    assert!(!under_ice_ocean(
        SurfaceState::Temperate,
        water(shell * 2.0),
        t_s,
        f
    ));
    assert!(!under_ice_ocean(
        SurfaceState::Snowball,
        water(shell * 2.0),
        t_s,
        WattsPerSquareMetre::new(0.0)
    ));
}

/// Decision-p14-t54a-states §6.2, a regression test: a `Snowball` at or above the melting point at
/// its ice's base holds no ocean under ice, however much surface water and heat it has, since no
/// shell conducts there; above 273.15 K the shell term, 651 ln(273.15 K ÷ `T_s`) ÷ F, is negative,
/// and the comparison alone once called every such world's water an ocean under its ice.
#[test]
fn a_snowball_above_melting_holds_no_ocean_under_ice() {
    let f = WattsPerSquareMetre::new(0.05);
    let water = KilogramsPerSquareMetre::new(2.7e6);
    for t in [300.0, UNDER_ICE_MELTING_POINT.value()] {
        assert!(
            !under_ice_ocean(SurfaceState::Snowball, water, Kelvin::new(t), f),
            "{t} K"
        );
    }
    let just_below = Kelvin::new(UNDER_ICE_MELTING_POINT.value() - 0.01);
    assert!(under_ice_ocean(
        SurfaceState::Snowball,
        water,
        just_below,
        f
    ));
}

/// A steam atmosphere holds no surface water: an Earth-like world whose air takes all its water
/// as vapour, under its saturation pressure, has none, and the condensed water falls continuously
/// to nothing at the edge (science-p14-t24b-laws §4.2, the lane's reading of T13.a's water).
#[test]
fn a_steam_atmosphere_holds_no_surface_water() {
    let table = figures_table();
    let (_, earth, _) = table.iter().find(|(n, ..)| *n == "Earth").unwrap();
    let air = earth.atmosphere();
    let (r, g) = (Metres::from(earth.radius()), earth.surface_gravity());
    let vapour = air.partial_pressures().of(Gas::Water).value();
    // The water whose weight is just the vapour's: all of it airborne, at the edge.
    let edge = vapour / g.value() * 4.0 * core::f64::consts::PI * r.value() * r.value();
    let at = |kg: f64| surface_water(air, Kilograms::new(kg), r, g, 1.0).value();
    assert!(at(edge * (1.0 + 1e-6)) < 2e-6 * at(edge * 2.0));
    assert!(at(edge * 2.0) > 0.0);
    assert_same_bits(at(edge * 0.5), 0.0);
}
