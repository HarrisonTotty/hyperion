//! P14.T24.b's surface water, oceans, ices and cloud, with θ, on the Solar System table and on
//! their laws' cases (science-p14-t24b-laws §4.2–4.5).

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::partition::Basins;
use crate::planetary::chem::solar_magnesium_to_silicon;
use crate::planetary::derive::DerivedBody;
use crate::planetary::derive::MassFractions;
use crate::planetary::derive::atmosphere::spun_read::{self, SpunRead};
use crate::planetary::derive::atmosphere::{NoMaturity, SurfaceMaterial, WeatheringCase};
use crate::planetary::derive::pass_record::{
    ContextTide, PassBulk, PassContext, PassRecord, PassRecordOf, RecordCrust, RecordSpin,
};
use crate::planetary::derive::photometry::area_mix::{Regolith, sun_bands};
use crate::planetary::derive::solar::SOLAR_AGE;
use crate::planetary::derive::solar::{
    derive_historic, historic_sun, placed_about_the_sun, solar_disc, table_bodies,
};
use crate::planetary::derive::{BodyHosts, PassDriver, Under, derive_body_driven};
use crate::planetary::hooks::cold_traps::{ColdTrapSky, NoColdTraps};
use crate::planetary::hooks::crust::CrustOrigin;
use crate::planetary::hooks::figures::relief::{
    EARTH_CONTINENTAL_FRACTION, Population, Relief, ReliefInputs,
};
use crate::planetary::hooks::figures::tests::{
    europa, figures_from, figures_table, inputs_of, measured_europa,
};
use crate::planetary::hooks::figures::{CraterInputs, WetEpoch, pass_figures};
use crate::planetary::hooks::figures::{
    FiguresInputs, GlobalFigures, TectonicRegime, VolatileHistory,
};
use crate::planetary::hooks::surface::tests::{inputs_of as surface_inputs_of, spin_of};
use crate::planetary::hooks::surface::{HourCosines, ZonalInsolation};
use crate::planetary::hooks::surface::{OceanWeight, SeasonalSpin, SurfaceInputs, area_below};
use crate::stellar::Composition;
use crate::time::UniverseTime;
use crate::units::consts::SECONDS_PER_DAY;
use crate::units::consts::SOLAR_MASS_KG;
use crate::units::{Degrees, MetresPerSecondSquared, PerSquareKilometre, Radians, Seconds, Years};
use crate::units::{Dex, Megayears};

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
                    water_state: SurfaceState::Snowball,
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
        lid: None,
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

/// [`surface_water`] as it was built, before P14.T54.a factored its parts out
/// ([`surface_water_of`]): the oracle of `the_surface_water_is_its_parts_to_the_bit`.
fn surface_water_as_built(
    atmosphere: &Atmosphere,
    water: Kilograms,
    radius: Metres,
    surface_gravity: MetresPerSecondSquared,
    outgassing: f64,
) -> KilogramsPerSquareMetre {
    let holds = match atmosphere.state() {
        SurfaceState::Temperate | SurfaceState::Snowball => true,
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::Airless
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Tenuous => false,
    };
    let (kg, r, g) = (water.value(), radius.value(), surface_gravity.value());
    if !(holds && atmosphere.retains_against_escape(Gas::Water) && kg > 0.0 && r > 0.0 && g > 0.0) {
        return KilogramsPerSquareMetre::ZERO;
    }
    let vapour = atmosphere.partial_pressures().of(Gas::Water);
    let condensed = kg / (4.0 * core::f64::consts::PI * r * r) - vapour.value() / g;
    if condensed.is_nan() || condensed <= 0.0 {
        return KilogramsPerSquareMetre::ZERO;
    }
    KilogramsPerSquareMetre::new(condensed * water_surface_share(outgassing))
}

/// The surface water is its parts' to the bit (P14.T54.a; decision-p14-t54a-passes §3.1): over the
/// table's atmospheres, airless, temperate, a runaway's and the rest, at waters from none to an
/// ocean world's and a NaN, at each body's radius and gravity, a small moon's and none, and on
/// both sides of the outgassing's edge, [`surface_water`], [`surface_water_of`] of the air's parts,
/// is the expression as built.
#[test]
fn the_surface_water_is_its_parts_to_the_bit() {
    let (mut compared, mut wet) = (0, 0);
    for (_, body, _) in figures_table() {
        let air = body.atmosphere();
        let own = (Metres::from(body.radius()), body.surface_gravity());
        let small = (Metres::new(1.0e6), MetresPerSecondSquared::new(1.3));
        let none = (Metres::new(0.0), MetresPerSecondSquared::new(0.0));
        for kg in [0.0, 1e15, 1e18, 1.4e21, 1e23, 1e25, f64::NAN] {
            for outgassing in [0.0, 0.3, 1.0, f64::NAN] {
                for (r, g) in [own, small, none] {
                    let water = Kilograms::new(kg);
                    let now = surface_water(air, water, r, g, outgassing);
                    let built = surface_water_as_built(air, water, r, g, outgassing);
                    assert_same_bits(now.value(), built.value());
                    compared += 1;
                    if now.value() > 0.0 {
                        wet += 1;
                    }
                }
            }
        }
    }
    assert!(wet > 10, "{wet} of {compared}");
}

/// The passes' basins and the record's hypsometry fill one layer, [`water_layer`]'s `σ_s` ÷ `ρ_l`
/// (P14.T54.a; decision-p14-t54a-passes §3.1, "one function"): over the table's reliefs and surface
/// waters from none to an ocean world's, the share the passes' basins fill,
/// `Basins::of(h).area_filled(water_layer(σ))`, is [`potential_ocean`]'s to the bit, and at each
/// body's own surface water its θ is its figures'.
#[test]
fn the_passes_and_the_record_fill_one_layer() {
    for (name, _, figures) in figures_table() {
        let hypsometry = figures.relief().hypsometry();
        let basins = Basins::of(hypsometry);
        for sigma in [0.0, 1e2, 2e4, 1e5, 1e6, 2.7e6, 1e7, 1e8] {
            let water = KilogramsPerSquareMetre::new(sigma);
            assert_same_bits(
                basins.area_filled(water_layer(water)),
                potential_ocean(hypsometry, water),
            );
        }
        let own = basins.area_filled(water_layer(figures.surface_water()));
        assert_same_bits(own, figures.potential_ocean());
        assert_same_bits(
            OceanWeight::of_potential_ocean(own).get(),
            figures.ocean_weight().get(),
        );
        assert!(own.is_finite(), "{name}");
    }
}

// P14.T54.a, Batch E's D1: the table's Earth and Mars over a record (decision-pre-23-review items
// 2, 13, 27 and 31).

/// The Solar System table's body `name` over a record, with no system about it (P14.T54.a;
/// decision-pre-23-review items 2, 13, 27 and 31): its passes take its own climate under its
/// measured spin ([`spin_of`]) and its obliquity's insolation table; each reads the context of the
/// pass before, K(P), the figures' first stage of that pass's own air at the table's figure
/// inputs ([`inputs_of`]: its sidereal day, its craters about the Sun, Earth's measured continents,
/// no tide) over its own bulk; and its crust's basis is a planet's about a host of solar Mg/Si at
/// \[α/Fe\] and \[Fe/H\] 0, formed at a 3 Myr disc's end, as `hooks/crust/tests.rs`'s planets are.
/// So its basins, cold profile, lid and water share are its own, the generator's real inputs, as a
/// system's record gives them (`PrimaryPasses`), but for the continents, given rather than grown,
/// and the formation time, the crust tests' rather than P14.T28.a's draw.
struct TableRecord<'a> {
    /// The table's name.
    name: &'static str,
    /// Its derivation over the passes' stand-in, whose figure inputs the table's are.
    stand_in: &'a DerivedBody,
    /// Its measured spin.
    spin: SeasonalSpin,
    /// Its obliquity's insolation table.
    table: ZonalInsolation,
    /// The sky its passes' cold traps read, or why they read none (P14.T54.b.f).
    cold_traps: Result<ColdTrapSky, NoColdTraps>,
}

impl PassRecordOf for TableRecord<'_> {
    fn cold_trap_sky(&self, _: WattsPerSquareMetre) -> Result<ColdTrapSky, NoColdTraps> {
        self.cold_traps
    }

    fn spin(&self, bulk: &PassBulk) -> RecordSpin<'_> {
        RecordSpin {
            spin: self.spin,
            table: &self.table,
            surface_gravity: bulk.surface_gravity,
            radius: Metres::from(bulk.radius),
            material: SurfaceMaterial::of(&bulk.fractions),
        }
    }

    fn context(&self, bulk: &PassBulk, air: &Atmosphere, _: Kelvin) -> PassContext {
        let table = inputs_of(self.stand_in, self.name);
        let inputs = FiguresInputs {
            atmosphere: air,
            mass: bulk.mass,
            radius: Metres::from(bulk.radius),
            surface_gravity: bulk.surface_gravity,
            class: bulk.class,
            fractions: bulk.fractions,
            inventory_water: bulk.inventory_water,
            craters: CraterInputs {
                density: bulk.density,
                ..table.craters
            },
            ..table
        };
        // A refusal is the crater contract's screening of a dense air: no figures in K, as the
        // record's `PrimaryPasses` reads it.
        let first = pass_figures(&inputs).ok().flatten();
        PassContext::of(first.as_ref(), ContextTide::Untided)
    }

    fn crust(&self, bulk: &PassBulk) -> Option<RecordCrust> {
        Some(RecordCrust {
            origin: CrustOrigin::Planet,
            formation: Megayears::new(3.0),
            snow_line_side: None,
            host_magnesium_to_silicon: solar_magnesium_to_silicon(),
            alpha_fe: Dex::new(0.0),
            fe_h: Dex::new(0.0),
            radius: Metres::from(bulk.radius),
            mass: bulk.mass,
            fractions: bulk.fractions,
        })
    }

    fn weathering(&self, _: &PassBulk) -> Result<WeatheringCase, NoMaturity> {
        // The table's bodies take no maturity: their regolith is the stand-in's.
        Err(NoMaturity::NoHost)
    }

    fn wet_epoch(&self, _: &PassBulk, _: &Atmosphere, _: Kelvin) -> Option<WetEpoch> {
        None
    }
}

/// The table's body `name` about the present Sun with its history at its age, derived over the
/// passes' stand-in and over its record ([`TableRecord`]), with its published pass's publication
/// run again at its published albedo and context (rule (f)): what its area mix read.
fn over_its_record(name: &'static str) -> (DerivedBody, DerivedBody, SpunRead) {
    over_its_record_under(name, Err(NoColdTraps::Held))
}

/// [`over_its_record`] with its passes' cold traps reading the sky `cold_traps`, or none
/// (P14.T54.b.f): the table's airless bodies through the real passes.
fn over_its_record_under(
    name: &'static str,
    cold_traps: Result<ColdTrapSky, NoColdTraps>,
) -> (DerivedBody, DerivedBody, SpunRead) {
    let disc = solar_disc();
    let &(_, kg, km, a, e) = table_bodies().find(|(n, ..)| *n == name).unwrap();
    let placed = placed_about_the_sun(&disc, (kg, km, a, e));
    let stand_in = derive_historic(&placed, &disc, SOLAR_AGE);
    let spin = spin_of(name);
    let record = TableRecord {
        name,
        stand_in: &stand_in,
        spin,
        table: ZonalInsolation::of(spin.obliquity, &HourCosines::new()),
        cold_traps,
    };
    let lights = [historic_sun()];
    let hosts = BodyHosts::new(
        Kilograms::new(SOLAR_MASS_KG),
        Composition::SOLAR,
        &lights,
        &[],
    )
    .unwrap();
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
    let derived = derive(PassDriver::Solve);
    let (again, read) = spun_read::capture(|| derive(PassDriver::Again(Under::Read)));
    assert_eq!(
        format!("{:?}", again.atmosphere()),
        format!("{:?}", derived.atmosphere()),
        "{name}"
    );
    (
        stand_in,
        derived,
        read.expect("a solid surface's pass publishes its partition"),
    )
}

/// The terms of a table body's Bond albedo over its record, as its published pass's area mix read
/// them: its state and cloud, its lid's row with that row's Bond albedo about the Sun under its
/// state, and each condensate entry's substance, phase, area and frost or sea albedo.
fn terms_of(derived: &DerivedBody, read: &SpunRead) -> String {
    let air = derived.atmosphere();
    let material = SurfaceMaterial::of(&derived.fractions());
    let lid = derived.crust_lid().map(|lid| {
        let row = Regolith::with_lid(lid.row(), material).bond(air.state(), &sun_bands());
        format!("{} at {:.4}", lid.lithology().key(), row.bond)
    });
    let entries: Vec<String> = read
        .condensates
        .entries()
        .iter()
        .map(|e| {
            format!(
                "{} {:?} over {:.5}",
                e.substance().substance().key(),
                e.phase(),
                e.area_fraction()
            )
        })
        .collect();
    format!(
        "{:?}, Bond albedo {:.5} under {:.5} of cloud, T_eq {:.3} K, T_s {:.3} K at {:.1} Pa, lid \
         {lid:?}, {} passes ({:?}); entries {entries:?}",
        air.state(),
        derived.albedo().value(),
        air.cloud_fraction(),
        derived.equilibrium_temperature().value(),
        air.surface_temperature().value(),
        air.surface_pressure()
            .map_or(0.0, crate::units::Pascals::value),
        derived.atmosphere_passes(),
        derived.convergence(),
    )
}

/// Decision-pre-23-review items 2 and 27: the table's Earth over its record, its passes reading its
/// basins, its cold profile, its lid and its water share, holds 288 ± 2 K, with nothing refitted
/// (decision-p14-t57a-fits §1.5); its Bond albedo is stated against NASA's 0.294 with its terms;
/// and its condensates hold `H2O` (P14.T54.a's test list): a sea with a reservoir, a layer and an
/// area. Its passes settle. A miss of 288 ± 2 K goes to main with its terms before the bless.
#[test]
fn the_table_s_earth_over_its_record_holds_288_k() {
    use crate::planetary::chem::partition::CondensatePhase;
    use crate::planetary::derive::passes::Convergence;
    let (stand_in, earth, read) = over_its_record("Earth");
    let air = earth.atmosphere();
    let t_s = air.surface_temperature().value();
    eprintln!(
        "the table's Earth over its record: {}; against 0.294 and 288 K; over the stand-in: Bond \
         albedo {:.5}, T_eq {:.3} K, T_s {:.3} K at {:.1} Pa",
        terms_of(&earth, &read),
        stand_in.albedo().value(),
        stand_in.equilibrium_temperature().value(),
        stand_in.surface_temperature().value(),
        stand_in
            .atmosphere()
            .surface_pressure()
            .map_or(0.0, crate::units::Pascals::value),
    );
    assert_eq!(air.state(), SurfaceState::Temperate);
    assert_eq!(earth.convergence(), Convergence::Converged);
    let sea = read
        .condensates
        .entry(Gas::Water.substance(), CondensatePhase::Liquid)
        .expect("an Earth's condensates hold its sea");
    assert!(
        sea.reservoir().value() > 0.0
            && sea.reservoir_gel().is_some_and(|gel| gel.value() > 0.0)
            && sea.area_fraction() > 0.0,
        "{sea:?}"
    );
    assert!((t_s - 288.0).abs() <= 2.0, "the table's Earth at {t_s} K");
}

/// Decision-pre-23-review items 13 and 31 (decision-p14-t51-batch E3): the table's Mars over its
/// record. Its water ice in its passes, its published pass's `H2O` solid entry, which its area mix
/// read, is the record's, T24.b's ice over the ground and the conditions rule (f) builds the
/// record on (its published pass's context: its basins, water share, heat flow, θ and lid), to the
/// bit, and lies in T24.b's 0.005–0.02. Its Bond albedo is stated against NASA's 0.250 with its
/// terms, item 13's pre-answer: its lid `basalt_fe` near 0.34, its water ice, its cloud under
/// 0.01 (Mars blesses at 0.33–0.36 with those terms; any other goes to main). Its passes settle.
#[test]
fn the_table_s_mars_over_its_record_takes_the_record_s_ice_in_its_passes() {
    use crate::planetary::chem::partition::{CondensatePhase, CrustWater};
    use crate::planetary::derive::pass_record::WaterGate;
    use crate::planetary::derive::passes::Convergence;
    let (stand_in, mars, read) = over_its_record("Mars");
    let air = mars.atmosphere();
    eprintln!(
        "the table's Mars over its record: {}; against 0.250; over the stand-in: Bond albedo \
         {:.5}, T_eq {:.3} K, T_s {:.3} K at {:.1} Pa",
        terms_of(&mars, &read),
        stand_in.albedo().value(),
        stand_in.equilibrium_temperature().value(),
        stand_in.surface_temperature().value(),
        stand_in
            .atmosphere()
            .surface_pressure()
            .map_or(0.0, crate::units::Pascals::value),
    );
    assert_eq!(mars.convergence(), Convergence::Converged);
    let spun = mars.spun().expect("derived over its record");
    let context = spun.context();
    // The record's basis by rule (f): its published pass's context's ground, heat flow, θ and lid.
    let material = SurfaceMaterial::of(&mars.fractions());
    let radius = Metres::from(mars.radius());
    let water = WaterGate::of_air(
        air,
        mars.inventory().water(),
        (radius, mars.surface_gravity()),
    );
    let lid = mars
        .crust_lid()
        .map(crate::planetary::hooks::crust::CrustLid::lithology);
    let ground =
        context.partition_ground(air, &water, material, CrustWater::of_record(material, lid));
    let conditions = SurfaceConditions::derive(&SurfaceInputs {
        atmosphere: air,
        equilibrium_temperature: mars.equilibrium_temperature(),
        surface_gravity: mars.surface_gravity(),
        radius,
        material,
        lid: context.lid().map(|lid| lid.lithology),
        stellar_flux: WattsPerSquareMetre::from(mars.flux()),
        xuv_flux: mars.xuv_flux(),
        spin: spin_of("Mars"),
        heat_flow: context.heat_flow(),
        ocean_weight: spun.ocean_weight(),
    })
    .expect("Mars has a surface");
    let pass = read
        .conditions
        .as_ref()
        .expect("Mars's pass reads its climate");
    assert_same_bits(
        conditions.mean_surface_temperature().value(),
        pass.mean_surface_temperature().value(),
    );
    let record = ice_fraction(&IceInputs {
        conditions: &conditions,
        state: air.state(),
        water_state: air.water_state(),
        crust: ground.crust,
        surface_water: ground.surface_water,
    });
    let ice = read
        .condensates
        .entry(Gas::Water.substance(), CondensatePhase::Solid)
        .map_or(
            0.0,
            crate::planetary::chem::partition::CondensateEntry::area_fraction,
        );
    eprintln!("Mars's water ice: {ice:.6} in its passes, {record:.6} the record's");
    assert_same_bits(ice, record);
    assert!((0.005..=0.02).contains(&ice), "{ice}");
}

/// decision-p14-t54b-f-coldtraps §9 test 8: the delivered traps survive L2. The table's Moon,
/// Mercury and Ceres, each in its own sky (the Moon's T51.f's; Mercury and Ceres as
/// `time_the_cold_traps` takes them), taken through the real passes over their records, every 24
/// hold in force: each published pass reads a field, and its water and carbon dioxide traps are
/// T51.f's and T51.i's figures at its own albedo, heat flow and craters, `f_ct` × `f_x` to the bit,
/// on the pass and so on the record; the Moon's water `f_ct` lies in plan 14's 0.024–0.2% (half of
/// Williams et al. 2019's Diviner-mapped 0.048% to twice Hayne et al. 2021's point-source 0.10%).
/// Mercury's and Ceres's figures are stated.
#[test]
fn the_table_s_airless_bodies_keep_their_delivered_traps_through_the_passes() {
    use crate::planetary::chem::partition::{CondensateEntry, CondensatePhase};
    use crate::planetary::derive::atmosphere::Gas;
    use crate::planetary::hooks::cold_traps::{Sunlit, lunar_sky, terrain_of, trap_threshold};
    use crate::planetary::testing::with_every_24_hold;
    use crate::units::consts::METRES_PER_AU;
    use crate::units::{Metres, SolarRadii, WattsPerSquareMetre};

    // Mercury and Ceres in their own skies (NASA's fact sheets, *from memory*: Mercury 0.387 au,
    // e 0.2056, 0.034°; Ceres 2.767 au, e 0.0785, 4°), as `time_the_cold_traps` takes them.
    let own = |degrees: f64, (a_au, e): (f64, f64)| {
        ColdTrapSky::at_periapsis(
            Radians::from(Degrees::new(degrees)),
            WattsPerSquareMetre::new(1361.0 / (a_au * a_au * (1.0 - e * e).sqrt())),
            (Metres::new(a_au * METRES_PER_AU), e),
            Some(SolarRadii::new(1.0)),
            0.035,
        )
    };
    with_every_24_hold(|| {
        for (name, sky) in [
            ("Moon", lunar_sky()),
            ("Mercury", own(0.034, (0.387_098, 0.205_63))),
            ("Ceres", own(4.0, (2.767, 0.0785))),
        ] {
            let (_, derived, read) = over_its_record_under(name, Ok(sky));
            let traps = derived
                .cold_traps()
                .as_ref()
                .unwrap_or_else(|why| panic!("{name}: no field, {why:?}"));
            let inputs = traps.inputs();
            let context = derived.spun().expect("over its record").context();
            let first = context.figures().expect("its context holds figures");
            let sunlit = Sunlit::new(sky, inputs.regolith_albedo(), context.heat_flow());
            let shadows = sunlit.shadows().expect("a lit body's shadows");
            let terrain = terrain_of(first.craters());
            let column = derived.atmosphere().column_inventory().condenses();
            for gas in [Gas::Water, Gas::CarbonDioxide] {
                let substance = gas.substance();
                let f_ct =
                    sunlit.trap_fraction(&shadows, &terrain, trap_threshold(substance).unwrap());
                let entry = read.condensates.entry(substance, CondensatePhase::Solid);
                let trapped = entry.map_or(0.0, CondensateEntry::trapped_area_fraction);
                eprintln!(
                    "{name} through its passes ({:?}, its column condensing {column}): {gas:?} \
                     f_ct {f_ct:.4e}, A_s {trapped:.4e}, reservoir {:?}, at the regolith's \
                     {:.4} and {:.4e} W m⁻²",
                    derived.convergence(),
                    entry.map(|e| e.reservoir().value()),
                    inputs.regolith_albedo(),
                    context.heat_flow().value()
                );
                assert_same_bits(
                    traps.field().trap(substance).map_or(f64::NAN, |t| t.1),
                    f_ct,
                );
                // A delivered species (none in its column) lies in its traps alone, its whole
                // trapped area `f_ct` × `f_x`; one its column holds takes what is left of its
                // condensed reservoir (T51.f), stated.
                let delivered = derived.atmosphere().partial_pressures().of(gas).value() <= 0.0;
                if f_ct > 0.0 && delivered {
                    assert_same_bits(trapped, f_ct * sky.exposed_fraction);
                }
                if name == "Moon" && gas == Gas::Water {
                    assert!((0.000_24..=0.002).contains(&f_ct), "{f_ct}");
                }
            }
        }
    });
}
