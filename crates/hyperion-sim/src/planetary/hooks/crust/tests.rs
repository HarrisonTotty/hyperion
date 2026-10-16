use std::path::{Path, PathBuf};

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

use super::history::{
    CHONDRITIC_SOURCES, DRY_ROCK_SPECIFIC_HEAT_J_PER_KG_K, SERIES_TERMS,
    adiabatic_central_temperature, aluminium_26_decay_rate_per_s, aluminium_26_production_w_per_kg,
    central_rise_with_terms, central_temperature, ice_rock_specific_heat_j_per_kg_k,
};
use super::*;
use crate::GENERATOR_VERSION;
use crate::id::SystemId;
use crate::planetary::chem::redox::{OXIDISED_FROM_BELOW_FMQ, REDUCED_BELOW, fmq_above_iw};
use crate::planetary::derive::solar::{
    SOLAR_AGE, derive_historic, placed_about_the_sun, solar_disc,
};
use crate::planetary::hooks::surface::tests::{conditions_of, spin_of, table};
use crate::planetary::hooks::surface::{EYEBALL_SLOPE, SeasonalSpin};
use crate::units::consts::{EARTH_MASS_KG, SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR};
use crate::units::{Degrees, Radians, Seconds};

/// The surface conditions of the Solar System table's body `name`, as
/// [`SurfaceConditions::derive`] gives them from its derivation about the present Sun with its own
/// spin (the surface module's table).
fn table_conditions(name: &str) -> SurfaceConditions {
    table()
        .into_iter()
        .find(|(n, ..)| *n == name)
        .map(|(_, _, conditions)| conditions)
        .unwrap()
}

/// The equilibrium temperature of the Solar System table's body `name`.
fn table_equilibrium_temperature(name: &str) -> Kelvin {
    table()
        .into_iter()
        .find(|(n, ..)| *n == name)
        .map(|(_, body, _)| body.equilibrium_temperature())
        .unwrap()
}

/// The conditions of a body of `kg` and `km` about the present Sun at `au`, formed there, under
/// `spin`.
fn conditions_at(kg: f64, km: f64, au: f64, spin: SeasonalSpin) -> SurfaceConditions {
    let disc = solar_disc();
    let placed = placed_about_the_sun(&disc, (kg, km, au, 0.0));
    let body = derive_historic(&placed, &disc, SOLAR_AGE);
    conditions_of(&body, spin).unwrap()
}

/// Pluto's spin: obliquity 119.51° to its orbit, sidereal period 6.387 d and year 90,560 d
/// (NASA's Pluto fact sheet), retrograde, so its solar day is 2π ÷ (ω + n).
fn pluto_spin() -> SeasonalSpin {
    let tau = 2.0 * core::f64::consts::PI;
    let (rotation, year) = (6.387 * SECONDS_PER_DAY, 90_560.0 * SECONDS_PER_DAY);
    SeasonalSpin {
        obliquity: Radians::from(Degrees::new(119.51)),
        rotation_period: Seconds::new(rotation),
        solar_day: Some(Seconds::new(tau / (tau / rotation + tau / year))),
    }
}

/// A Kuiper-belt surface: Pluto's mass (1.303 × 10²² kg) and radius (1,188 km) at its mean
/// distance, 39.48 au (NASA's Pluto fact sheet), under its spin. Its warmest lies below water's
/// 110 K.
fn kuiper_belt_conditions() -> SurfaceConditions {
    conditions_at(1.303e22, 1_188.0, 39.48, pluto_spin())
}

/// The median magma ocean's end, P14.T28.a's: 10^1.5 Myr, the geometric mean of 10 and 100 Myr.
fn median_magma_ocean_end() -> Years {
    Years::new(math::exp10(1.5) * 1e6)
}

/// The mass of `kg` kilograms in Earth masses.
fn earths(kg: f64) -> EarthMasses {
    EarthMasses::new(kg / EARTH_MASS_KG)
}

/// A planet's inputs, which each test body overrides: a solid body inside the snow line about a
/// host of solar Mg/Si, [α/Fe] 0 and [Fe/H] 0, at the Sun's age, with P14.T28.a's median magma
/// ocean and its formation at a 3 Myr disc's end (a small planet's).
fn planet(surface: &SurfaceConditions) -> CrustCompositionInputs<'_> {
    CrustCompositionInputs {
        origin: CrustOrigin::Planet,
        formation: Megayears::new(3.0),
        age: SOLAR_AGE,
        molten_until: Some(median_magma_ocean_end()),
        radius: Metres::new(6.371e6),
        mass: EarthMasses::new(1.0),
        fractions: MassFractions::solid(0.325, 0.675, 0.0),
        snow_line_side: SnowLineSide::Inside,
        ammonia: AmmoniaIce::Absent,
        delta_iw: Dex::new(0.0),
        host_magnesium_to_silicon: solar_magnesium_to_silicon(),
        alpha_fe: Dex::new(0.0),
        fe_h: Dex::new(0.0),
        surface_age: SOLAR_AGE,
        volcanism: 0.0,
        continental_fraction: 0.0,
        tidal: TidalVolcanism::Quiet,
        state: SurfaceState::Airless,
        formation_temperature: Kelvin::new(250.0),
        surface,
    }
}

/// The key of `id`'s row, a lithology of the closed set; "none" for none, and "unnamed" for a row
/// outside the set.
fn key(id: Option<SubstanceId>) -> &'static str {
    id.map_or("none", |id| {
        CrustLithology::ALL
            .iter()
            .find(|l| l.substance() == id)
            .map_or("unnamed", |l| l.key())
    })
}

/// The crust's members by key, for the assertions: primary, secondary, tertiary, bulk, provinces.
fn keys(crust: &CrustComposition) -> [&'static str; 5] {
    [
        key(crust.primary()),
        key(crust.secondary()),
        key(crust.tertiary()),
        key(crust.bulk()),
        key(Some(crust.provinces())),
    ]
}

/// Mercury (NASA's fact sheet: 2,439.7 km, 0.330 10 × 10²⁴ kg), its core 0.67 ± 0.06 of its mass
/// (Sossi et al. 2025, Space Sci. Rev. 221, 118, Table 1, after Hauck et al. 2013), its mantle at
/// T50.a's table ΔIW, IW−5.04, under its measured surface age, 4.0 Gyr: its oldest surfaces
/// formed "about 4.0–4.1 Gyr ago", and "Resurfacing was global and was due to volcanism" (Marchi
/// et al. 2013, Nature 499, 59, abstract). T24.b's surface age for the table's Mercury is its age
/// since its magma ocean, its present heat flow resurfacing nothing (plan 14's record of
/// P14.T51.c).
fn mercury(surface: &SurfaceConditions) -> CrustCompositionInputs<'_> {
    CrustCompositionInputs {
        radius: Metres::new(2.439_7e6),
        mass: earths(0.330_10e24),
        fractions: MassFractions::solid(0.67, 0.33, 0.0),
        delta_iw: Dex::new(-5.044),
        surface_age: Years::new(4.0e9),
        ..planet(surface)
    }
}

/// The Moon (NASA's fact sheet: 1,737.4 km, 7.342 × 10²² kg), a rocky parent's giant-impact moon,
/// its core 0.012 ± 0.003 of its mass (Sossi et al. 2025, Table 1), at T50.a's table ΔIW, IW−1.55:
/// nothing has resurfaced its highlands since its magma ocean, so its surface age is its age
/// (T24.b's for the table's Moon), and its maria make its volcanism level positive (P14.T48.a's
/// for the table's Moon, 0.0009).
fn moon(surface: &SurfaceConditions) -> CrustCompositionInputs<'_> {
    CrustCompositionInputs {
        origin: CrustOrigin::ImpactMoon(ParentSurface::Rocky),
        molten_until: None,
        radius: Metres::new(1.737_4e6),
        mass: earths(7.342e22),
        fractions: MassFractions::solid(0.012, 0.988, 0.0),
        delta_iw: Dex::new(-1.547),
        surface_age: SOLAR_AGE,
        volcanism: 0.000_9,
        ..planet(surface)
    }
}

/// Earth (NASA's fact sheet: 6,371 km, 5.9722 × 10²⁴ kg), its core 0.325 of its mass (Wang,
/// Lineweaver and Ireland 2018, Icarus 299, 460, Table 3), its upper mantle at T50.a's table ΔIW,
/// FMQ − 0.04, under its mean ocean floor's 64.2 Myr (Seton et al. 2020), its volcanism level
/// P14.T48.a's for the table's Earth, 0.96, and its measured continental fraction, 0.405
/// (Earth2014).
fn earth(surface: &SurfaceConditions) -> CrustCompositionInputs<'_> {
    CrustCompositionInputs {
        delta_iw: Dex::new(3.553),
        surface_age: Years::new(64.2e6),
        volcanism: 0.96,
        continental_fraction: 0.405,
        state: SurfaceState::Temperate,
        ..planet(surface)
    }
}

/// Venus (NASA's fact sheet: 6,051.8 km, 4.8673 × 10²⁴ kg), its core 0.30 of its mass, between
/// Sossi et al. 2025's Table 1, 0.27 ± 0.05, and Earth's 0.325, which its moment of inertia
/// cannot be told from (Margot et al. 2021, as Sossi et al. 2025, §2, read it), at T50.a's table
/// ΔIW, IW+1.89, its surface 0.39 Gyr old (P14.T24.b's, inside the published 0.3–0.8 Gyr), its
/// volcanism level about P14.T48.a's for the table's Venus, 0.90.
///
/// Its basalts are terrestrial in `FeO`: 9.3 ± 2.2 wt% at Venera 13, 8.8 ± 1.8 at Venera 14 and
/// 7.7 ± 1.1 at Vega 2 (Surkov et al. 1984 and 1986, as Shellnutt 2018, PLOS ONE 13, e0194155,
/// Table 1, gives them). The mass balance puts the iron-rich edge at a core of 0.292.
fn venus(surface: &SurfaceConditions) -> CrustCompositionInputs<'_> {
    CrustCompositionInputs {
        radius: Metres::new(6.051_8e6),
        mass: earths(4.867_3e24),
        fractions: MassFractions::solid(0.30, 0.70, 0.0),
        delta_iw: Dex::new(1.893),
        surface_age: Years::new(3.9e8),
        volcanism: 0.9,
        state: SurfaceState::RunawayGreenhouse,
        ..planet(surface)
    }
}

/// Mars (NASA's fact sheet: 3,389.5 km, 0.641 69 × 10²⁴ kg), its core 0.204 ± 0.015 of its mass
/// (Khan et al. 2023, as Sossi et al. 2025, Table 1, gives it; Stähler et al.'s 2021 core of
/// 1,830 km made it about 0.24), at T50.a's table ΔIW, IW+0.01, its Noachian surface 3.9 Gyr old,
/// its volcanism level P14.T48.a's unit, 1.
fn mars(surface: &SurfaceConditions) -> CrustCompositionInputs<'_> {
    CrustCompositionInputs {
        radius: Metres::new(3.389_5e6),
        mass: earths(0.641_69e24),
        fractions: MassFractions::solid(0.204, 0.796, 0.0),
        delta_iw: Dex::new(0.015),
        surface_age: Years::new(3.9e9),
        volcanism: 1.0,
        state: SurfaceState::Temperate,
        ..planet(surface)
    }
}

/// A small body's inputs: a belt member of `km` and `water` of its mass formed at `formed` Myr on
/// `side`, its thermal history's surface at `t_s` K, at ΔIW `delta_iw`.
fn small_body(
    surface: &SurfaceConditions,
    km: f64,
    water: f64,
    formed: f64,
    side: SnowLineSide,
    t_s: f64,
) -> CrustCompositionInputs<'_> {
    let density = if water > 0.0 { 2_000.0 } else { 3_300.0 };
    let r = km * 1e3;
    let kg = 4.0 / 3.0 * core::f64::consts::PI * r * r * r * density;
    CrustCompositionInputs {
        origin: CrustOrigin::BeltMember,
        formation: Megayears::new(formed),
        molten_until: None,
        radius: Metres::new(r),
        mass: earths(kg),
        fractions: MassFractions::solid(0.0, 1.0 - water, water),
        snow_line_side: side,
        delta_iw: Dex::new(-1.0),
        formation_temperature: Kelvin::new(t_s),
        ..planet(surface)
    }
}

#[test]
fn the_moon_has_a_feldspathic_primary_lid_with_iron_rich_maria() {
    let surface = table_conditions("Moon");
    let crust = CrustComposition::derive(&moon(&surface)).unwrap();
    // Its highlands' 4.7–5.4 wt% `FeO` and its maria's 16–22 wt%.
    assert_eq!(
        keys(&crust),
        ["feldspathic", "basalt_fe", "none", "none", "basalt_fe"]
    );
    assert_eq!(crust.lid(), LidCrust::Primary);
    assert_eq!(crust.redox(), RedoxClass::Intermediate);
    assert_eq!(
        crust.thermal_history(),
        ThermalHistory::RockMelted(MeltCause::ImpactDebris)
    );
    assert!(crust.melt_area_fraction() < 1e-12);
}

#[test]
fn mercury_has_a_reduced_basalt_lid_over_a_graphite_primary_crust() {
    let surface = table_conditions("Mercury");
    let crust = CrustComposition::derive(&mercury(&surface)).unwrap();
    // Nittler et al. 2011; McCubbin et al. 2012 (IW−2.6 to −6.3); Peplowski et al. 2016.
    assert_eq!(
        keys(&crust),
        [
            "graphite",
            "basalt_reduced",
            "none",
            "none",
            "basalt_reduced"
        ]
    );
    assert_eq!(crust.lid(), LidCrust::Secondary);
    assert_eq!(crust.redox(), RedoxClass::Reduced);
    // Its low `FeO`, about 1.3–2.6 wt% at its surface (Nittler et al. 2018's Fe of 1–2 wt%): its
    // core leaves its mantle none by mass balance.
    let feo = mantle_iron_oxide_mass_fraction(0.67, solar_magnesium_to_silicon(), Dex::new(0.0));
    assert!(feo < 0.02, "{feo}");
}

#[test]
fn earth_has_granite_continents_on_a_basalt_ocean_floor() {
    let surface = table_conditions("Earth");
    let crust = CrustComposition::derive(&earth(&surface)).unwrap();
    assert_eq!(
        keys(&crust),
        ["ultramafic", "basalt", "granite", "none", "basalt"]
    );
    assert_eq!(crust.lid(), LidCrust::Secondary);
    assert_eq!(crust.redox(), RedoxClass::Oxidised);
    assert!(crust.melt_area_fraction() < 1e-12);
}

#[test]
fn venus_has_a_basalt_stagnant_lid() {
    let surface = table_conditions("Venus");
    let crust = CrustComposition::derive(&venus(&surface)).unwrap();
    assert_eq!(
        keys(&crust),
        ["ultramafic", "basalt", "none", "none", "basalt"]
    );
    assert_eq!(crust.lid(), LidCrust::Secondary);
    assert!(crust.melt_area_fraction() < 1e-12);
}

#[test]
fn mars_has_an_iron_rich_basalt_lid_over_a_buried_ultramafic_crust() {
    let surface = table_conditions("Mars");
    let crust = CrustComposition::derive(&mars(&surface)).unwrap();
    // Its ferric surface is P14.T51.d's, at 24.
    assert_eq!(
        keys(&crust),
        ["ultramafic", "basalt_fe", "none", "none", "basalt_fe"]
    );
    assert_eq!(crust.lid(), LidCrust::Secondary);
    assert_eq!(crust.redox(), RedoxClass::Intermediate);
}

/// An Earth's and a Mars's mass balance put their mantles either side of the iron-rich edge: an
/// Earth's 7.3 wt% beside the bulk silicate Earth's 8.05, and a Mars's 22.8 wt% beside Dreibus
/// and Wänke's 17.9 and InSight's 13.8 (plan 14's record of P14.T51.c).
#[test]
fn an_earth_s_mass_balance_gives_basalt_and_a_mars_s_iron_rich_basalt() {
    let solar = solar_magnesium_to_silicon();
    let earth = mantle_iron_oxide_mass_fraction(0.325, solar, Dex::new(0.0));
    let mars = mantle_iron_oxide_mass_fraction(0.204, solar, Dex::new(0.0));
    assert!((0.06..0.09).contains(&earth), "{earth}");
    assert!((0.17..0.24).contains(&mars), "{mars}");
    assert!(earth < IRON_RICH_MANTLE_FEO && mars > IRON_RICH_MANTLE_FEO);
    // The edge is the two measured mantles' geometric mean.
    assert!((IRON_RICH_MANTLE_FEO - (0.080_5_f64 * 0.179).sqrt()).abs() < 1e-4);
    // The bulk iron at the Sun's ratios is the bulk Earth's 32 wt%, within a few per cent.
    let bulk = bulk_iron_mass_fraction(solar, Dex::new(0.0));
    assert!((0.30..0.33).contains(&bulk), "{bulk}");
}

/// The secondary crust shifts with Mg/Si at fixed everything else (the Mg-rich side): an Earth's
/// `basalt` turns `komatiite` where its mantle becomes periclase-normative, and stays so above.
#[test]
fn the_secondary_crust_shifts_towards_mg_rich_with_magnesium_to_silicon() {
    let surface = table_conditions("Earth");
    let at = |ratio: f64| {
        let inputs = CrustCompositionInputs {
            host_magnesium_to_silicon: ratio,
            ..earth(&surface)
        };
        key(CrustComposition::derive(&inputs).unwrap().secondary())
    };
    assert_eq!(at(solar_magnesium_to_silicon()), "basalt");
    assert_eq!(at(1.8), "komatiite");
    let mut shifted = false;
    for step in 0..=40 {
        let ratio = 1.0 + 0.02 * f64::from(step);
        let secondary = at(ratio);
        if secondary == "komatiite" {
            shifted = true;
        } else {
            assert!(!shifted, "back to {secondary} at Mg/Si {ratio}");
        }
    }
    // A quartz-normative mantle keeps its unshifted row: a silica-rich crust is not modelled.
    assert_eq!(at(0.5), "basalt");
    let feo = mantle_iron_oxide_mass_fraction(0.325, 0.5, Dex::new(0.0));
    assert_eq!(MantleNorm::of(0.5, feo), MantleNorm::QuartzNormative);
}

#[test]
fn a_hot_young_mantle_is_komatiitic() {
    let surface = table_conditions("Earth");
    let young = CrustCompositionInputs {
        age: Years::new(1.5e9),
        surface_age: Years::new(64.2e6),
        ..earth(&surface)
    };
    let crust = CrustComposition::derive(&young).unwrap();
    assert_eq!(key(crust.secondary()), "komatiite");
    // The edge lies at 2.6 Ga, near the Archean's end: an Earth of 2.0 Gyr is just past it.
    assert!(hot_mantle(
        Dex::new(0.0),
        Years::new(1.9e9),
        TidalVolcanism::Quiet
    ));
    assert!(!hot_mantle(
        Dex::new(0.0),
        Years::new(2.1e9),
        TidalVolcanism::Quiet
    ));
    assert!(!hot_mantle(Dex::new(0.0), SOLAR_AGE, TidalVolcanism::Quiet));
    assert!(hot_mantle(
        Dex::new(0.0),
        SOLAR_AGE,
        TidalVolcanism::Volcanic
    ));
}

/// Io (NASA's fact sheet: 1,821.6 km, 8.932 × 10²² kg; its core about a fifth of its mass, from
/// memory, which its tides keep from entering), tidally volcanic, a regular moon formed inside its
/// parent's ice line, its mantle at IW+0.95 (Pele's magma "3.3 log fO2 units below the Ni‐NiO
/// buffer" at 1,440 K, Zolotov and Fegley 2000, GRL 27, 2789, abstract, moved to IW by NNO =
/// FMQ + 0.7, Schaefer and Elkins-Tanton 2018's Fig. 1 caption, and by the crate's FMQ − IW of
/// 3.55 at 1,440 K and 1 bar): a crust, with a `komatiite` secondary lid (Pillan's lava "must have
/// exceeded 1700 kelvin", and the "most likely explanation is that these lavas are ultramafic",
/// McEwen et al. 1998, Science 281, 87, abstract). Its surface is the table's Ganymede's, an
/// airless one at Jupiter's distance. Its melt area is none: its hot spots are internal heat, which
/// the closed form does not read, against an active share of order 10⁻⁴ (plan 14's record).
#[test]
fn io_has_a_komatiite_secondary_lid_and_no_melt_area() {
    let surface = table_conditions("Ganymede");
    let io = CrustCompositionInputs {
        origin: CrustOrigin::RegularMoon,
        formation: Megayears::new(2.0),
        molten_until: None,
        radius: Metres::new(1.821_6e6),
        mass: earths(8.932e22),
        fractions: MassFractions::solid(0.2, 0.8, 0.0),
        delta_iw: Dex::new(0.95),
        surface_age: Years::new(1e4),
        tidal: TidalVolcanism::Volcanic,
        formation_temperature: table_equilibrium_temperature("Ganymede"),
        ..planet(&surface)
    };
    let crust = CrustComposition::derive(&io).unwrap();
    assert_eq!(
        keys(&crust),
        ["none", "komatiite", "none", "none", "komatiite"]
    );
    assert_eq!(crust.lid(), LidCrust::Secondary);
    assert_eq!(
        crust.thermal_history(),
        ThermalHistory::RockMelted(MeltCause::Tides)
    );
    assert_eq!(crust.peak_central_temperature(), None);
    assert!(crust.melt_area_fraction() < 1e-12);
}

/// A Ceres-like body (470 km, a quarter water, formed at 3 Myr): its ice melted, its rock did not,
/// and its frozen ocean is a primary `phyllosilicate` crust with no secondary, at its own surface
/// (the table's Ceres) and its present equilibrium temperature.
#[test]
fn a_ceres_like_body_has_a_primary_phyllosilicate_crust_and_no_secondary() {
    let surface = table_conditions("Ceres");
    let t_s = table_equilibrium_temperature("Ceres").value();
    let ceres = CrustCompositionInputs {
        delta_iw: Dex::new(1.45),
        ..small_body(&surface, 470.0, 0.25, 3.0, SnowLineSide::Beyond, t_s)
    };
    let crust = CrustComposition::derive(&ceres).unwrap();
    assert_eq!(
        crust.thermal_history(),
        ThermalHistory::IceMelted(IceMeltCause::Radiogenic),
        "{:?}",
        crust.peak_central_temperature()
    );
    assert_eq!(
        keys(&crust),
        ["phyllosilicate", "none", "none", "none", "phyllosilicate"]
    );
    assert_eq!(crust.lid(), LidCrust::Primary);
}

/// An icy body whose ice melted and whose ocean froze, of 40% ice: an `H2O` primary crust.
#[test]
fn an_icy_body_whose_ice_melted_has_a_water_ice_primary_crust() {
    let surface = table_conditions("Ceres");
    let t_s = table_equilibrium_temperature("Ceres").value();
    let icy = small_body(&surface, 400.0, 0.4, 2.5, SnowLineSide::Beyond, t_s);
    let crust = CrustComposition::derive(&icy).unwrap();
    assert!(
        matches!(crust.thermal_history(), ThermalHistory::IceMelted(_)),
        "{:?}",
        crust.thermal_history()
    );
    assert_eq!(key(crust.primary()), "H2O");
    assert_eq!(crust.lid(), LidCrust::Primary);
}

/// Henke et al. 2012's 100 km H-chondrite body, which "accreted at t = 2.3 Ma after CAI
/// formation" and stayed below partial melting, at their surface's 300 K: an ordinary-chondrite
/// `bulk`.
#[test]
fn a_dry_100_km_body_formed_at_2_3_myr_keeps_an_ordinary_chondrite_bulk() {
    let surface = table_conditions("Ceres");
    let henke = small_body(&surface, 100.0, 0.0, 2.3, SnowLineSide::Inside, 300.0);
    let crust = CrustComposition::derive(&henke).unwrap();
    assert_eq!(crust.thermal_history(), ThermalHistory::Unmelted);
    assert!(
        ["chondrite_h", "chondrite_l", "chondrite_ll"].contains(&key(crust.bulk())),
        "{}",
        key(crust.bulk())
    );
    assert_eq!(crust.lid(), LidCrust::Bulk);
}

/// Hunt et al. 2018's IAB parent, "accreted at around 1.4 ± 0.1 Ma after CAIs with a radius of
/// greater than 60 km", and Neumann et al. 2023's Erg Chech 002 parent, "a radius of 20–30 km
/// that formed at 0.1 Ma", each at their models' 290 K: a crust.
#[test]
fn early_dry_bodies_melt_and_have_a_crust() {
    let surface = table_conditions("Ceres");
    for (km, formed) in [(60.0, 1.4), (100.0, 1.4), (25.0, 0.1)] {
        let body = small_body(&surface, km, 0.0, formed, SnowLineSide::Inside, 290.0);
        let crust = CrustComposition::derive(&body).unwrap();
        assert_eq!(
            crust.thermal_history(),
            ThermalHistory::RockMelted(MeltCause::Radiogenic),
            "{km} km at {formed} Myr"
        );
        assert!(crust.bulk().is_none() && crust.secondary().is_some());
        assert_eq!(crust.primary(), None, "no magma ocean, no flotation crust");
    }
}

/// Bland and Travis 2017's icy 100 km body melts its ice when it accretes at 3 Myr, and "Objects
/// accreting after 4 My do not melt", at a water-to-rock ratio of 0.6 and Ceres's present
/// equilibrium temperature, near the asteroid belt's.
#[test]
fn an_icy_100_km_body_melts_its_ice_at_3_myr_and_not_after_4() {
    let surface = table_conditions("Ceres");
    let t_s = table_equilibrium_temperature("Ceres").value();
    let ice = 0.6 / 1.6;
    let at = |formed: f64| {
        let body = small_body(&surface, 100.0, ice, formed, SnowLineSide::Beyond, t_s);
        CrustComposition::derive(&body).unwrap()
    };
    let early = at(3.0);
    assert_eq!(
        early.thermal_history(),
        ThermalHistory::IceMelted(IceMeltCause::Radiogenic),
        "{:?}",
        early.peak_central_temperature()
    );
    // Altered in place: its surface, warmer than 110 K, lost its ice.
    assert_eq!(key(early.bulk()), "chondrite_ci");
    for formed in [4.0, 4.5, 6.0] {
        let late = at(formed);
        assert_eq!(late.thermal_history(), ThermalHistory::Unmelted, "{formed}");
        assert_eq!(key(late.bulk()), "cometary");
    }
}

/// A Pluto (1,190 km, a third ice, formed at 5 Myr) at the Kuiper belt's 40 K: a primary `H2O`
/// crust at its surface.
#[test]
fn a_pluto_has_a_water_ice_primary_crust() {
    let surface = kuiper_belt_conditions();
    let pluto = small_body(
        &surface,
        1_190.0,
        1.0 / 3.0,
        5.0,
        SnowLineSide::Beyond,
        40.0,
    );
    let crust = CrustComposition::derive(&pluto).unwrap();
    assert_eq!(key(crust.primary()), "H2O", "{:?}", crust.thermal_history());
    assert_eq!(crust.lid(), LidCrust::Primary);
    assert_eq!(key(Some(crust.provinces())), "H2O");
}

/// A 10 km icy body formed at 1 Myr or later, and a 200 km Kuiper-belt body formed at 5 Myr, at
/// the Kuiper belt's 40 K and surface: `cometary`.
#[test]
fn small_and_distant_icy_bodies_are_cometary() {
    let surface = kuiper_belt_conditions();
    for (km, formed) in [(10.0, 1.0), (10.0, 2.0), (10.0, 4.0), (200.0, 5.0)] {
        let body = small_body(&surface, km, 0.5, formed, SnowLineSide::Beyond, 40.0);
        let crust = CrustComposition::derive(&body).unwrap();
        assert_eq!(
            key(crust.bulk()),
            "cometary",
            "{km} km at {formed} Myr: {:?}",
            crust.thermal_history()
        );
    }
}

#[test]
fn a_reduced_dry_body_that_never_melted_is_enstatite_chondrite() {
    let surface = table_conditions("Ceres");
    let body = CrustCompositionInputs {
        delta_iw: Dex::new(-4.0),
        ..small_body(&surface, 50.0, 0.0, 5.0, SnowLineSide::Inside, 200.0)
    };
    let crust = CrustComposition::derive(&body).unwrap();
    assert_eq!(crust.thermal_history(), ThermalHistory::Unmelted);
    assert_eq!(crust.redox(), RedoxClass::Reduced);
    assert_eq!(key(crust.bulk()), "chondrite_e");
}

/// The crust's class is T50.a's banding at its two edges, and the H, L and LL edges each hold.
#[test]
fn the_redox_bands_are_t50a_s_and_the_ordinary_chondrite_edges_hold() {
    let surface = table_conditions("Ceres");
    let class = |delta_iw: f64| {
        let body = CrustCompositionInputs {
            delta_iw: Dex::new(delta_iw),
            ..small_body(&surface, 50.0, 0.0, 5.0, SnowLineSide::Inside, 200.0)
        };
        CrustComposition::derive(&body).unwrap().redox()
    };
    let reduced_below = REDUCED_BELOW.value();
    let oxidised_from = (fmq_above_iw() - OXIDISED_FROM_BELOW_FMQ).value();
    assert_eq!(class(reduced_below - 1e-9), RedoxClass::Reduced);
    assert_eq!(class(reduced_below), RedoxClass::Intermediate);
    assert_eq!(class(oxidised_from - 1e-9), RedoxClass::Intermediate);
    assert_eq!(class(oxidised_from), RedoxClass::Oxidised);
    let (h_below, ll_from) = ordinary_chondrite_edges();
    assert!(reduced_below < h_below.value() && ll_from.value() <= 0.0);
    assert_eq!(
        ordinary_chondrite(h_below - Dex::new(1e-9)),
        CrustLithology::ChondriteH
    );
    assert_eq!(ordinary_chondrite(h_below), CrustLithology::ChondriteL);
    assert_eq!(
        ordinary_chondrite(ll_from - Dex::new(1e-9)),
        CrustLithology::ChondriteL
    );
    assert_eq!(ordinary_chondrite(ll_from), CrustLithology::ChondriteLl);
    // Each group at its cited centre, Righter and Neff's ΔFMQ on the crate's scale: H −3.05, L
    // −2.43 and LL −1.86, so the edges −2.74 and −2.14.
    let groups = [
        CrustLithology::ChondriteH,
        CrustLithology::ChondriteL,
        CrustLithology::ChondriteLl,
    ];
    for (delta_fmq, group) in ORDINARY_CHONDRITE_DELTA_FMQ.into_iter().zip(groups) {
        assert_eq!(ordinary_chondrite(delta_fmq + fmq_above_iw()), group);
    }
    assert!((h_below.value() + 2.74).abs() < 0.005, "{h_below:?}");
    assert!((ll_from.value() + 2.14).abs() < 0.005, "{ll_from:?}");
}

/// The conditions of a Mercury placed at `au` about the present Sun and locked 1:1 to it.
fn locked_mercury_at(au: f64) -> SurfaceConditions {
    conditions_at(
        0.330_10e24,
        2_439.7,
        au,
        SeasonalSpin {
            solar_day: None,
            ..spin_of("Mercury")
        },
    )
}

/// A locked lava world's melt area is its eyeball's dayside pool: in [0, 0.5], rising with its
/// substellar temperature, and ½[1 − (`T_sol`⁴ − `T_n`⁴) ÷ (`T_ss`⁴ − `T_n`⁴)] within 10⁻⁹, at a
/// `basalt` lid's 1,253.15 K (Léger et al. 2011; Kite et al. 2016).
#[test]
fn a_locked_lava_world_s_melt_area_is_its_dayside_pool() {
    let solidus = CrustLithology::Basalt.transition().value();
    let mut last = -1.0;
    for au in [0.09, 0.08, 0.07, 0.06, 0.055, 0.05, 0.045] {
        let surface = locked_mercury_at(au);
        let inputs = CrustCompositionInputs {
            surface_age: Years::new(1e8),
            ..mercury(&surface)
        };
        let lava = CrustCompositionInputs {
            delta_iw: Dex::new(0.0),
            ..inputs
        };
        let crust = CrustComposition::derive(&lava).unwrap();
        assert_eq!(key(Some(crust.provinces())), "basalt");
        let melt = crust.melt_area_fraction();
        let (day, night) = surface.day_and_night();
        let (e_d, e_n) = (math::powi(day.value(), 4), math::powi(night.value(), 4));
        let e_ss = e_n + EYEBALL_SLOPE * (e_d - e_n);
        let e_sol = math::powi(solidus, 4);
        let pool = if e_sol >= e_ss {
            0.0
        } else {
            0.5 * (1.0 - (e_sol - e_n) / (e_ss - e_n))
        };
        eprintln!(
            "{au} au: T_ss {:.1} K, T_n {:.1} K, melt {melt:.6}",
            math::powf(e_ss, 0.25),
            night.value()
        );
        assert!((0.0..=0.5).contains(&melt), "{melt}");
        assert!((melt - pool).abs() < 1e-9, "{melt} against {pool}");
        assert!(melt >= last, "{melt} after {last}");
        last = melt;
    }
    assert!(last > 0.2, "the hottest pool covers {last}");
}

/// A magma ocean's surface holds melt everywhere, whatever its lid.
#[test]
fn a_magma_ocean_s_melt_area_is_whole() {
    let surface = table_conditions("Earth");
    let young = CrustCompositionInputs {
        state: SurfaceState::MagmaOcean,
        ..earth(&surface)
    };
    let crust = CrustComposition::derive(&young).unwrap();
    assert!((crust.melt_area_fraction() - 1.0).abs() < 1e-15);
}

/// The chondritic sources of a body formed at `formed` Myr of rock `rock`, all rock.
fn sources(formed: f64, rock: ChondriticRock) -> [HeatSource; CHONDRITIC_SOURCES] {
    chondritic_sources(Megayears::new(formed), rock, 1.0, Dex::new(0.0))
}

/// A dry sphere of `km` at the module's constants and 200 K.
fn dry_sphere(km: f64) -> ConductiveSphere {
    ConductiveSphere {
        radius: Metres::new(km * 1e3),
        diffusivity_m2_per_s: ROCK_THERMAL_DIFFUSIVITY_M2_PER_S,
        specific_heat_j_per_kg_k: DRY_ROCK_SPECIFIC_HEAT_J_PER_KG_K,
        surface_temperature: Kelvin::new(200.0),
    }
}

/// The series of 24 tapered terms lies within 1% of 400 over the anchors' radii and times, the
/// slow early times of a large body included, where few modes have damped; and at 400 terms a
/// large body's early centre is its adiabatic limit.
#[test]
fn the_series_lies_within_one_percent_of_400_terms() {
    let mut worst: f64 = 0.0;
    for km in [10.0, 25.0, 60.0, 100.0, 200.0, 470.0, 1_190.0, 2_630.0] {
        for formed in [0.1, 2.3, 5.0] {
            let sphere = dry_sphere(km);
            let sources = sources(formed, ChondriticRock::Ordinary);
            for years in [1e2, 1e3, 1e4, 1e5, 1e6, 3e6, 1e7, 1e8, 1e9, 4.5e9] {
                let t = years * SECONDS_PER_JULIAN_YEAR;
                let reference = central_rise_with_terms(&sphere, &sources, t, 400);
                let ours = central_rise_with_terms(&sphere, &sources, t, SERIES_TERMS);
                let error = (ours - reference).abs() / reference;
                worst = worst.max(error);
                assert!(error < 0.01, "{km} km, {formed} Myr, {years} yr: {error}");
            }
        }
    }
    eprintln!("the series' worst error against 400 terms: {worst:.2e}");
    let sphere = dry_sphere(1_190.0);
    let sources = sources(0.0, ChondriticRock::Ordinary);
    let years = Years::new(100.0);
    let t = years.value() * SECONDS_PER_JULIAN_YEAR;
    let adiabatic = adiabatic_central_temperature(&sphere, &sources, years).value() - 200.0;
    let at_400 = central_rise_with_terms(&sphere, &sources, t, 400);
    assert!(
        (at_400 / adiabatic - 1.0).abs() < 1e-6,
        "{at_400} against {adiabatic}"
    );
}

/// `T_c,max` never lies above the adiabatic bound, at any time or as a peak.
#[test]
fn the_peak_never_lies_above_the_adiabatic_bound() {
    for km in [1.0, 10.0, 100.0, 1_000.0, 4_000.0] {
        for formed in [0.0, 1.0, 5.0] {
            let sphere = dry_sphere(km);
            let sources = sources(formed, ChondriticRock::Ordinary);
            for years in [1e2, 1e4, 1e6, 1e8, 4.5e9] {
                let since = Years::new(years);
                let series = central_temperature(&sphere, &sources, since).value();
                let bound = adiabatic_central_temperature(&sphere, &sources, since).value();
                assert!(series <= bound * (1.0 + 1e-12), "{km} km at {years} yr");
            }
            let peak = peak_central_temperature(&sphere, &sources, SOLAR_AGE).value();
            let bound = adiabatic_central_temperature(&sphere, &sources, SOLAR_AGE).value();
            assert!(
                peak <= bound * (1.0 + 1e-12),
                "{km} km: {peak} over {bound}"
            );
        }
    }
}

/// The unheld tapered series exceeds the adiabatic limit by under 10⁻⁴ of it, where few modes have
/// damped by its last term, as `central_rise_with_terms`' doc states (6 × 10⁻⁵ at most over radii
/// of 100 m to 8,000 km and times of 1 yr to 10 Gyr); the hold takes the limit there.
#[test]
fn the_unheld_series_exceeds_the_adiabatic_limit_by_under_1e_4() {
    let mut worst: f64 = 0.0;
    for km in [
        0.1, 0.3, 1.0, 3.0, 10.0, 30.0, 100.0, 300.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0,
    ] {
        for formed in [0.0, 1.0, 5.0] {
            let sphere = dry_sphere(km);
            let sources = sources(formed, ChondriticRock::Ordinary);
            for years in [
                1.0, 3.0, 10.0, 30.0, 1e2, 3e2, 1e3, 3e3, 1e4, 3e4, 1e5, 3e5, 1e6, 3e6, 1e7, 3e7,
                1e8, 3e8, 1e9, 3e9, 1e10,
            ] {
                let t = years * SECONDS_PER_JULIAN_YEAR;
                let (series, bound) =
                    super::history::series_and_bound(&sphere, &sources, t, SERIES_TERMS);
                let excess = (series - bound) / bound;
                worst = worst.max(excess);
                assert!(excess < 1e-4, "{km} km at {years} yr: {excess}");
            }
        }
    }
    eprintln!("the unheld series' largest excess over the adiabatic limit: {worst:.2e}");
}

/// Under a constant source the centre's peak, at long times, is the steady state H R² ÷ (6 κ c)
/// within 10⁻⁶.
#[test]
fn a_constant_source_reaches_its_steady_state() {
    for km in [10.0, 100.0, 1_000.0] {
        let sphere = dry_sphere(km);
        let constant = [HeatSource {
            production_w_per_kg: 1e-11,
            decay_rate_per_s: 0.0,
        }];
        let r = km * 1e3;
        let steady = 1e-11 * r * r
            / (6.0 * ROCK_THERMAL_DIFFUSIVITY_M2_PER_S * DRY_ROCK_SPECIFIC_HEAT_J_PER_KG_K);
        let first_mode =
            ROCK_THERMAL_DIFFUSIVITY_M2_PER_S * math::powi(core::f64::consts::PI, 2) / (r * r);
        let long = Years::new(40.0 / first_mode / SECONDS_PER_JULIAN_YEAR);
        let peak = peak_central_temperature(&sphere, &constant, long).value() - 200.0;
        assert!(
            (peak / steady - 1.0).abs() < 1e-6,
            "{km} km: {peak} against {steady}"
        );
    }
}

/// The ruling's check (science-composition-crusts §1.4), ²⁶Al alone at its own constants (Al
/// 1.13 wt%, c 850, κ 10⁻⁶, `T_s` 200 K), within 0.5% of its table.
#[test]
fn the_ruling_s_check_is_reproduced() {
    let p0 = aluminium_26_production_w_per_kg(
        super::history::ORDINARY_CHONDRITE_ALUMINIUM_MASS_FRACTION,
    );
    assert!((p0 / 2.02e-7 - 1.0).abs() < 0.005, "{p0}");
    let rise = p0 / (aluminium_26_decay_rate_per_s() * 850.0);
    assert!((rise / 7_757.0 - 1.0).abs() < 0.005, "{rise}");
    let table = [
        (0.0, [2_506.0, 5_122.0, 7_683.0, 7_953.0]),
        (1.0, [1_077.0, 2_072.0, 3_046.0, 3_149.0]),
        (1.5, [741.0, 1_355.0, 1_955.0, 2_018.0]),
        (2.0, [534.0, 912.0, 1_282.0, 1_321.0]),
        (2.3, [450.0, 733.0, 1_010.0, 1_039.0]),
        (3.0, [327.0, 471.0, 612.0, 627.0]),
    ];
    for (formed, row) in table {
        for (km, expected) in [10.0, 20.0, 50.0, 100.0].into_iter().zip(row) {
            let sphere = ConductiveSphere {
                specific_heat_j_per_kg_k: 850.0,
                ..dry_sphere(km)
            };
            let al = [sources(formed, ChondriticRock::Ordinary)[0]];
            let peak = peak_central_temperature(&sphere, &al, Years::new(1e8)).value();
            assert!(
                (peak / expected - 1.0).abs() < 0.005,
                "{km} km at {formed} Myr: {peak} against {expected}"
            );
        }
    }
}

/// The ice's latent heat is `H2O`'s diagram's, within 0.1% of IAPWS R10-06's 333.4 kJ kg⁻¹, and
/// the solidus and melting points are the registry's rows', not restated.
#[test]
fn the_latent_heat_and_the_first_melts_are_the_registry_s() {
    let latent = ice_latent_heat_j_per_kg();
    assert!((latent / 333.4e3 - 1.0).abs() < 1e-3, "{latent}");
    let first_melt = |key: &str| {
        crate::substance::by_key(key)
            .unwrap()
            .substance()
            .condensed()
            .unwrap()
            .transition()
            .temperature()
    };
    assert_eq!(
        CrustLithology::ChondriteH.transition(),
        first_melt("chondrite_h")
    );
    assert_eq!(ice_melting_point(AmmoniaIce::Absent), first_melt("H2O"));
    assert_eq!(
        ice_melting_point(AmmoniaIce::Carried),
        first_melt("nh3_h2o")
    );
    assert!((CrustLithology::ChondriteH.transition().value() - 1_423.15).abs() < 1e-9);
    assert!((ice_melting_point(AmmoniaIce::Absent).value() - 273.16).abs() < 1e-9);
    assert!((ice_melting_point(AmmoniaIce::Carried).value() - 176.15).abs() < 1e-9);
}

#[test]
fn every_lithology_named_is_a_registry_row_with_condensed_columns() {
    for lithology in CrustLithology::ALL {
        let id = crate::substance::by_key(lithology.key()).unwrap();
        assert_eq!(lithology.substance(), id, "{}", lithology.key());
        assert!(id.substance().condensed().is_some(), "{}", lithology.key());
    }
    let ammonia_water = crate::substance::by_key("nh3_h2o").unwrap();
    assert_eq!(ammonia_water.index(), AMMONIA_WATER_INDEX);
    assert!(ammonia_water.substance().condensed().is_some());
    // In registry order.
    for pair in CrustLithology::ALL.windows(2) {
        assert!(pair[0].substance() < pair[1].substance());
    }
}

/// The bodies of the invariants' sweep: every origin, side, size, formation time, ice share,
/// ΔIW and tide.
fn sweep(surface: &SurfaceConditions) -> Vec<CrustCompositionInputs<'_>> {
    let origins = [
        CrustOrigin::Planet,
        CrustOrigin::RegularMoon,
        CrustOrigin::ImpactMoon(ParentSurface::Rocky),
        CrustOrigin::ImpactMoon(ParentSurface::Icy),
        CrustOrigin::CapturedMoon,
        CrustOrigin::BeltMember,
        CrustOrigin::Comet,
    ];
    let mut bodies = Vec::new();
    for origin in origins {
        for side in [SnowLineSide::Inside, SnowLineSide::Beyond] {
            for km in [5.0, 60.0, 300.0, 3_000.0] {
                for formed in [0.2, 2.0, 6.0] {
                    for water in [0.0, 0.2, 0.5] {
                        for delta_iw in [-4.0, -1.2, 3.0] {
                            for tidal in [TidalVolcanism::Quiet, TidalVolcanism::Volcanic] {
                                bodies.push(CrustCompositionInputs {
                                    origin,
                                    delta_iw: Dex::new(delta_iw),
                                    tidal,
                                    continental_fraction: if km > 1_000.0 { 0.3 } else { 0.0 },
                                    volcanism: 0.5,
                                    ..small_body(surface, km, water, formed, side, 150.0)
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    bodies
}

/// `bulk` excludes every crust, a crust has a primary or a secondary, no tertiary stands without a
/// secondary, the lid names a present member, the melt area lies in [0, 1], and every member is a
/// lithology of the closed set.
#[test]
fn the_crust_s_invariants_hold_over_a_sweep() {
    let surface = table_conditions("Ceres");
    let named: Vec<SubstanceId> = CrustLithology::ALL.iter().map(|l| l.substance()).collect();
    for inputs in sweep(&surface) {
        let crust = CrustComposition::derive(&inputs).unwrap();
        let members = [
            crust.primary(),
            crust.secondary(),
            crust.tertiary(),
            crust.bulk(),
        ];
        if crust.bulk().is_some() {
            assert!(crust.primary().is_none() && crust.secondary().is_none());
            assert!(crust.tertiary().is_none());
        } else {
            assert!(crust.primary().is_some() || crust.secondary().is_some());
        }
        assert!(crust.tertiary().is_none() || crust.secondary().is_some());
        let lid = match crust.lid() {
            LidCrust::Primary => crust.primary(),
            LidCrust::Secondary => crust.secondary(),
            LidCrust::Bulk => crust.bulk(),
        };
        assert!(lid.is_some(), "{inputs:?}");
        assert!((0.0..=1.0).contains(&crust.melt_area_fraction()));
        for id in members.into_iter().flatten().chain([crust.provinces()]) {
            assert!(named.contains(&id), "{}", id.substance().key().as_str());
        }
    }
}

#[test]
fn each_input_out_of_range_is_refused_by_its_variant() {
    let surface = table_conditions("Earth");
    let base = earth(&surface);
    let refused =
        |inputs: CrustCompositionInputs<'_>| CrustComposition::derive(&inputs).unwrap_err();
    assert_eq!(
        refused(CrustCompositionInputs {
            radius: Metres::new(0.0),
            ..base
        }),
        DeriveCrustError::RadiusNotPositive(Metres::new(0.0))
    );
    assert_eq!(
        refused(CrustCompositionInputs {
            mass: EarthMasses::new(-1.0),
            ..base
        }),
        DeriveCrustError::MassNotPositive(EarthMasses::new(-1.0))
    );
    assert_eq!(
        refused(CrustCompositionInputs {
            age: Years::new(0.0),
            ..base
        }),
        DeriveCrustError::AgeNotPositive(Years::new(0.0))
    );
    assert_eq!(
        refused(CrustCompositionInputs {
            formation_temperature: Kelvin::new(0.0),
            ..base
        }),
        DeriveCrustError::TemperatureNotPositive(Kelvin::new(0.0))
    );
    assert_eq!(
        refused(CrustCompositionInputs {
            host_magnesium_to_silicon: 0.0,
            ..base
        }),
        DeriveCrustError::MagnesiumToSiliconNotPositive(0.0)
    );
    assert!(matches!(
        refused(CrustCompositionInputs {
            fractions: MassFractions::solid(0.3, 0.6, 0.0),
            ..base
        }),
        DeriveCrustError::FractionsNotNormalised(_)
    ));
    assert!(matches!(
        refused(CrustCompositionInputs {
            formation: Megayears::new(5_000.0),
            ..base
        }),
        DeriveCrustError::FormationOutsideAge { .. }
    ));
    assert!(matches!(
        refused(CrustCompositionInputs {
            formation: Megayears::new(-1.0),
            ..base
        }),
        DeriveCrustError::FormationOutsideAge { .. }
    ));
    assert_eq!(
        refused(CrustCompositionInputs {
            state: SurfaceState::GasEnvelope,
            ..base
        }),
        DeriveCrustError::NoSolidSurface
    );
    for (inputs, error) in [
        (
            CrustCompositionInputs {
                surface_age: Years::new(5e9),
                ..base
            },
            DeriveCrustError::SurfaceAgeOutsideAge {
                surface_age: Years::new(5e9),
                age: SOLAR_AGE,
            },
        ),
        (
            CrustCompositionInputs {
                volcanism: -0.1,
                ..base
            },
            DeriveCrustError::VolcanismNegative(-0.1),
        ),
        (
            CrustCompositionInputs {
                continental_fraction: 1.5,
                ..base
            },
            DeriveCrustError::ContinentalFractionOutsideUnit(1.5),
        ),
    ] {
        assert_eq!(refused(inputs), error);
    }
}

/// Every error's message is lowercase with no trailing punctuation
/// (`.claude/rules/rust-dev.md`, Errors and panics).
#[test]
fn every_error_message_is_lowercase_with_no_trailing_punctuation() {
    let messages = [
        DeriveCrustError::NoSolidSurface.to_string(),
        DeriveCrustError::RadiusNotPositive(Metres::new(0.0)).to_string(),
        DeriveCrustError::MassNotPositive(EarthMasses::new(0.0)).to_string(),
        DeriveCrustError::AgeNotPositive(Years::new(0.0)).to_string(),
        DeriveCrustError::TemperatureNotPositive(Kelvin::new(0.0)).to_string(),
        DeriveCrustError::MagnesiumToSiliconNotPositive(0.0).to_string(),
        DeriveCrustError::FractionsNotNormalised(0.9).to_string(),
        DeriveCrustError::FormationOutsideAge {
            formation: Megayears::new(-1.0),
            age: SOLAR_AGE,
        }
        .to_string(),
        DeriveCrustError::NotFinite(CrustInput::ContinentalFraction).to_string(),
        DeriveCrustError::SurfaceAgeOutsideAge {
            surface_age: Years::new(-1.0),
            age: SOLAR_AGE,
        }
        .to_string(),
        DeriveCrustError::VolcanismNegative(-1.0).to_string(),
        DeriveCrustError::ContinentalFractionOutsideUnit(2.0).to_string(),
        BuildFormationWindowError::NotFiniteOrNegative(Megayears::new(-1.0)).to_string(),
        BuildFormationWindowError::Reversed {
            earliest: Megayears::new(2.0),
            latest: Megayears::new(1.0),
        }
        .to_string(),
    ];
    for message in messages {
        assert!(
            message.starts_with(char::is_lowercase)
                && !message.ends_with(['.', '!', '?', ':', ';', ',']),
            "{message}"
        );
    }
    assert_eq!(
        DeriveCrustError::NotFinite(CrustInput::DeltaIw).to_string(),
        "delta_iw is not finite"
    );
}

#[test]
fn each_input_not_finite_is_refused_by_name() {
    let surface = table_conditions("Earth");
    let base = earth(&surface);
    let nan = f64::NAN;
    for (inputs, input) in [
        (
            CrustCompositionInputs {
                delta_iw: Dex::new(nan),
                ..base
            },
            CrustInput::DeltaIw,
        ),
        (
            CrustCompositionInputs {
                alpha_fe: Dex::new(nan),
                ..base
            },
            CrustInput::AlphaFe,
        ),
        (
            CrustCompositionInputs {
                fe_h: Dex::new(nan),
                ..base
            },
            CrustInput::FeH,
        ),
        (
            CrustCompositionInputs {
                surface_age: Years::new(nan),
                ..base
            },
            CrustInput::SurfaceAge,
        ),
        (
            CrustCompositionInputs {
                molten_until: Some(Years::new(nan)),
                ..base
            },
            CrustInput::MoltenUntil,
        ),
        (
            CrustCompositionInputs {
                volcanism: nan,
                ..base
            },
            CrustInput::Volcanism,
        ),
        (
            CrustCompositionInputs {
                continental_fraction: nan,
                ..base
            },
            CrustInput::ContinentalFraction,
        ),
    ] {
        assert_eq!(
            CrustComposition::derive(&inputs).unwrap_err(),
            DeriveCrustError::NotFinite(input)
        );
    }
}

/// The `//!` lines of the module's section `heading`, up to the next heading.
fn module_section(heading: &str) -> String {
    let text = include_str!("../crust.rs");
    let mut inside = false;
    let mut lines = Vec::new();
    for line in text.lines().take_while(|l| l.starts_with("//!")) {
        let body = line.trim_start_matches("//!").trim();
        if body.starts_with("# ") {
            inside = body == heading;
            continue;
        }
        if inside {
            lines.push(body.to_owned());
        }
    }
    lines.join(" ")
}

/// The closed sets, the lithologies the crust can name and those not modelled, match the module
/// documentation's lists.
#[test]
fn the_closed_sets_match_the_module_doc() {
    let named = module_section("# The lithologies it names");
    let listed: Vec<&str> = named
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|token| !token.contains(':') && !token.contains("ALL"))
        .collect();
    let keys: Vec<&str> = CrustLithology::ALL.iter().map(|l| l.key()).collect();
    assert_eq!(listed, keys);
    let not_modelled = module_section("# Not modelled");
    let bold: Vec<&str> = not_modelled.split("**").skip(1).step_by(2).collect();
    assert_eq!(bold, NOT_MODELLED);
}

/// The formation time is deterministic, lies in its window, and reads only word 0 of its own tag.
#[test]
fn the_formation_time_is_deterministic_in_its_window_and_reads_word_0() {
    let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let window = FormationWindow::new(Megayears::new(0.8), Megayears::new(3.0)).unwrap();
    for raw_seed in [0, 7, u64::MAX] {
        for index in [0x0100_u16, 0x0101, 0x0300, 0xE001] {
            let (seed, body) = (Seed::new(raw_seed), BodyId::new(system, index));
            let t = formation_time(seed, body, window);
            assert_eq!(t, formation_time(seed, body, window));
            assert!((0.8..=3.0).contains(&t.value()), "{t:?}");
            let mut stream = Stream::open(seed, tags::BODY_FORMATION, ObjectKey::from(body));
            let u = stream.uniform_open();
            assert!((t.value() - (0.8 + u * 2.2)).abs() < 1e-15);
        }
    }
    assert_eq!(
        FormationWindow::new(Megayears::new(2.0), Megayears::new(1.0)),
        Err(BuildFormationWindowError::Reversed {
            earliest: Megayears::new(2.0),
            latest: Megayears::new(1.0)
        })
    );
    assert!(matches!(
        FormationWindow::disc(Megayears::new(f64::NAN)),
        Err(BuildFormationWindowError::NotFiniteOrNegative(_))
    ));
    assert_eq!(
        FormationWindow::new(Megayears::new(-1.0), Megayears::new(1.0)),
        Err(BuildFormationWindowError::NotFiniteOrNegative(
            Megayears::new(-1.0)
        ))
    );
    // A body's time does not depend on which bodies were drawn before it.
    let keys: Vec<(u64, u16)> = [0, 7, u64::MAX]
        .into_iter()
        .flat_map(|seed| [0x0100_u16, 0x0101, 0x0300, 0xE001].map(|index| (seed, index)))
        .collect();
    hyperion_testkit::order::assert_order_independent(&keys, |&(seed, index)| {
        formation_time(Seed::new(seed), BodyId::new(system, index), window)
    });
}

/// The source files under the crate's `src/`, in sorted order.
fn source_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            source_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// The code of a source file as the tripwire scans it: its lines up to its inline test module,
/// the first `#[cfg(test)]` whose next line opens `mod tests {`, its comment lines left out (a doc
/// may name the module).
fn scanned_code(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let end = lines
        .windows(2)
        .position(|pair| {
            pair[0].trim() == "#[cfg(test)]" && pair[1].trim_start().starts_with("mod tests {")
        })
        .unwrap_or(lines.len());
    lines[..end]
        .iter()
        .filter(|line| !line.trim_start().starts_with("//"))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Nothing on the record calls the crust's derivation, its draw or its thermal history at
/// generator version 22 (P14.T49.c's tripwire, as its ruling narrows it). P14.T54.a, the 22 → 23
/// bump, wires the module onto the record and deletes this test.
///
/// It walks the crate's `src/` outside `planetary/hooks/crust*`, in sorted order, and fails on an
/// unambiguous call form or path in each file's code: the file less its comment lines and less
/// its inline test module, from the first `#[cfg(test)]` that opens a `mod tests {` (a
/// `#[cfg(test)]` on a single item, or on a `mod tests;` declaration near the top, ends nothing,
/// so that the scan reaches the code after it). `hooks/mod.rs`'s `pub mod crust;` and its one
/// `pub use crust::{…};` are allowed. Bare names such as `CrustComposition` are not matched:
/// `record.rs` has its own uninhabited type of that name.
#[test]
fn crust_nothing_on_the_record_reads_the_crust_yet() {
    const PATTERNS: [&str; 6] = [
        "CrustComposition::derive(",
        "formation_time(",
        "peak_central_temperature(",
        "chondritic_sources(",
        "hooks::crust::",
        "crust::{",
    ];
    // (file, hold): files allowed to match while a named hold stands; none yet.
    const HELD: [(&str, &str); 0] = [];
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&src, &mut files);
    let mut readers = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&src)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with("planetary/hooks/crust") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let code = scanned_code(&text);
        if relative == "planetary/system.rs" {
            // The record's builders are scanned, not cut off by an earlier test-only item.
            assert!(code.contains("fn primary_record") && code.contains("fn satellite_record"));
        }
        for pattern in PATTERNS {
            let allowed = relative == "planetary/hooks/mod.rs" && pattern == "crust::{";
            let held = HELD.iter().any(|(file, _)| *file == relative);
            if code.contains(pattern) && !allowed && !held {
                readers.push(format!("{relative}: {pattern}"));
            }
        }
    }
    assert!(
        readers.is_empty(),
        "the crust is read on the record: {readers:?}"
    );
    // The one allowed re-export is there, once.
    let hooks = std::fs::read_to_string(src.join("planetary/hooks/mod.rs")).unwrap();
    assert_eq!(hooks.matches("pub use crust::{").count(), 1);
    assert_eq!(hooks.matches("pub mod crust;").count(), 1);
}

/// The golden's anchor bodies, by name, on the table's surfaces of the Moon, Mercury, Earth,
/// Venus, Mars and Ceres (`surfaces`, in that order), the Kuiper belt's and Ceres's equilibrium
/// temperature.
fn anchor_bodies<'a>(
    surfaces: &'a [SurfaceConditions; 6],
    kuiper_s: &'a SurfaceConditions,
    ceres_t: f64,
) -> Vec<(&'static str, CrustCompositionInputs<'a>)> {
    let [moon_s, mercury_s, earth_s, venus_s, mars_s, ceres_s] = surfaces;
    vec![
        ("Moon", moon(moon_s)),
        ("Mercury", mercury(mercury_s)),
        ("Earth", earth(earth_s)),
        ("Venus", venus(venus_s)),
        ("Mars", mars(mars_s)),
        (
            "Ceres",
            CrustCompositionInputs {
                delta_iw: Dex::new(1.45),
                ..small_body(ceres_s, 470.0, 0.25, 3.0, SnowLineSide::Beyond, ceres_t)
            },
        ),
        (
            "Henke 100 km at 2.3 Myr",
            small_body(ceres_s, 100.0, 0.0, 2.3, SnowLineSide::Inside, 300.0),
        ),
        (
            "Hunt 60 km at 1.4 Myr",
            small_body(ceres_s, 60.0, 0.0, 1.4, SnowLineSide::Inside, 290.0),
        ),
        (
            "Neumann 25 km at 0.1 Myr",
            small_body(ceres_s, 25.0, 0.0, 0.1, SnowLineSide::Inside, 290.0),
        ),
        (
            "Bland-Travis 100 km at 3 Myr",
            small_body(ceres_s, 100.0, 0.375, 3.0, SnowLineSide::Beyond, ceres_t),
        ),
        (
            "Bland-Travis 100 km at 4 Myr",
            small_body(ceres_s, 100.0, 0.375, 4.0, SnowLineSide::Beyond, ceres_t),
        ),
        (
            "Pluto",
            small_body(
                kuiper_s,
                1_190.0,
                1.0 / 3.0,
                5.0,
                SnowLineSide::Beyond,
                40.0,
            ),
        ),
        (
            "Kuiper belt 200 km at 5 Myr",
            small_body(kuiper_s, 200.0, 0.5, 5.0, SnowLineSide::Beyond, 40.0),
        ),
        (
            "comet 10 km at 1 Myr",
            small_body(kuiper_s, 10.0, 0.5, 1.0, SnowLineSide::Beyond, 40.0),
        ),
    ]
}

/// The derived values, pinned by their bits: `T_c,max` over a grid of radii and formation times,
/// rock and icy; each anchor body's crust; the melt area over a sweep of a locked world's
/// substellar temperature; and the formation time's ranks. Nothing on the record reads them until
/// P14.T54.a, so until then a change re-blesses this golden at the version in force, citing that.
#[test]
fn the_crust_values_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for km in [10.0, 25.0, 50.0, 100.0, 200.0, 470.0, 1_190.0] {
        for formed in [0.0, 0.5, 1.0, 2.0, 3.0, 5.0] {
            let rock = dry_sphere(km);
            let peak = peak_central_temperature(
                &rock,
                &sources(formed, ChondriticRock::Ordinary),
                SOLAR_AGE,
            );
            w.f64(&format!("peak rock {km} km at {formed} Myr"), peak.value());
            let ice = 0.375;
            let icy = ConductiveSphere {
                specific_heat_j_per_kg_k: ice_rock_specific_heat_j_per_kg_k(ice),
                surface_temperature: Kelvin::new(160.0),
                ..rock
            };
            let icy_sources = chondritic_sources(
                Megayears::new(formed),
                ChondriticRock::Carbonaceous,
                1.0 - ice,
                Dex::new(0.0),
            );
            let peak = peak_central_temperature(&icy, &icy_sources, SOLAR_AGE);
            w.f64(&format!("peak icy {km} km at {formed} Myr"), peak.value());
        }
    }
    let surfaces = ["Moon", "Mercury", "Earth", "Venus", "Mars", "Ceres"].map(table_conditions);
    let kuiper = kuiper_belt_conditions();
    let ceres_t = table_equilibrium_temperature("Ceres").value();
    for (name, inputs) in anchor_bodies(&surfaces, &kuiper, ceres_t) {
        let crust = CrustComposition::derive(&inputs).unwrap();
        let [primary, secondary, tertiary, bulk, provinces] = keys(&crust);
        w.line(&format!(
            "{name}: primary {primary}, secondary {secondary}, tertiary {tertiary}, bulk {bulk}, \
             lid {:?}, provinces {provinces}, redox {:?}, {:?}",
            crust.lid(),
            crust.redox(),
            crust.thermal_history()
        ));
        w.f64(
            &format!("{name} melt_area_fraction"),
            crust.melt_area_fraction(),
        );
        if let Some(peak) = crust.peak_central_temperature() {
            w.f64(&format!("{name} peak_central_temperature"), peak.value());
        }
    }
    for au in [0.09, 0.07, 0.055, 0.045] {
        let surface = locked_mercury_at(au);
        let lava = CrustCompositionInputs {
            delta_iw: Dex::new(0.0),
            surface_age: Years::new(1e8),
            ..mercury(&surface)
        };
        let crust = CrustComposition::derive(&lava).unwrap();
        let (day, night) = surface.day_and_night();
        let (e_d, e_n) = (math::powi(day.value(), 4), math::powi(night.value(), 4));
        let substellar = math::powf(e_n + EYEBALL_SLOPE * (e_d - e_n), 0.25);
        w.f64(&format!("locked Mercury at {au} au T_ss"), substellar);
        w.f64(
            &format!("locked Mercury at {au} au melt_area_fraction"),
            crust.melt_area_fraction(),
        );
    }
    let system = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
    let window = FormationWindow::disc(Megayears::new(3.0)).unwrap();
    for raw_seed in [0, 7, 0x0014_0051, u64::MAX] {
        for index in [0x0100_u16, 0x0101, 0x0300, 0xE001] {
            let (seed, body) = (Seed::new(raw_seed), BodyId::new(system, index));
            let rank =
                Stream::open(seed, tags::BODY_FORMATION, ObjectKey::from(body)).uniform_open();
            let label = format!("seed {raw_seed:016x} body {body}");
            w.f64(&format!("{label} rank"), rank);
            let t = formation_time(seed, body, window);
            w.f64(&format!("{label} formation_time in 0-3 Myr"), t.value());
        }
    }
    golden!("planetary/crust", w.as_str());
}
