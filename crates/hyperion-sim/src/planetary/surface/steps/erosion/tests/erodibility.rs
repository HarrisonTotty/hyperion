//! The erodibility's tests: the plan's three calibrated K values (an Earth's rock under water, a
//! saturated Mars's regolith, a Titan's ice under methane), each to 2%; the bed table's rows; and
//! the rock-equivalent of a pair the table does not hold.

use hyperion_surface::field::{Crust, MechanicsFamily};
use hyperion_surface::substance_key::SubstanceKey;
use hyperion_testkit::float::assert_same_bits;

use super::super::erodibility::{
    BED_FACTORS, EARTH_ERODIBILITY_M0_1_PER_YEAR, Erodibility, Fluid, Substrate, Surface,
    bed_factor, crater_saturated, effective_crater_density, erodibility_m0_1_per_year,
};
use crate::planetary::hooks::figures::relief::BASIN_SATURATION_DENSITY;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::reference;
use crate::planetary::surface::steps::relief::convergent::EARTH_GRAVITY;
use crate::planetary::surface::steps::{Pass, Working};
use crate::substance;
use crate::units::MetresPerSecondSquared;

/// The plan's tolerance on the calibrated K values: 2% (R09.T14.b's Tests).
const TOLERANCE: f64 = 0.02;

/// An Earth's rock under water, m^0.1 a⁻¹: 6.3 × 10⁻⁶ (R09.T14.b's Tests; Design note 9).
const EARTH_ROCK_K: f64 = 6.3e-6;

/// A saturated Mars's regolith under water, m^0.1 a⁻¹: 2.4 × 10⁻⁵ (R09.T14.b's Tests; Design
/// note 9).
const MARS_REGOLITH_K: f64 = 2.4e-5;

/// A Titan's water ice under methane, m^0.1 a⁻¹: 3.1 × 10⁻⁶ (R09.T14.b's Tests; Design note 9).
const TITAN_ICE_K: f64 = 3.1e-6;

/// Titan's surface gravity, m s⁻²: GM = 8,978.14 km³ s⁻² over (2,574.73 km)², NASA's Titan fact
/// sheet's figures (from memory), 1.354, the lane design's §3.1 figure.
const TITAN_GRAVITY: MetresPerSecondSquared =
    MetresPerSecondSquared::new(8.978_14e12 / (2_574.73e3 * 2_574.73e3));

/// Liquid methane's density on Titan's surface, kg m⁻³: 447.6, Setzmann and Wagner 1991's at
/// 93.65 K and Huygens's 1,467 hPa, the surface crate's synthetic Titan sea's
/// (`hyperion_surface::testing::LIQUID_METHANE_TITAN_DENSITY_KG_M3`, which this crate's tests reach
/// only with its `testing` feature, and which
/// `the_titan_sea_s_density_is_the_testing_world_s` holds equal to this).
const TITAN_METHANE_DENSITY_KG_M3: f64 = 447.6;

/// Whether `k` is within [`TOLERANCE`] of `expected`, relatively.
fn assert_within(what: &str, k: f64, expected: f64) {
    let error = (k / expected - 1.0).abs();
    eprintln!(
        "{what}: K = {k:.4e} m^0.1 a⁻¹ against {expected:.2e} ({:.2}%)",
        100.0 * error
    );
    assert!(
        error <= TOLERANCE,
        "{what}: K = {k:e} m^0.1 a⁻¹ is not within 2% of {expected:e}"
    );
}

/// The registry key `key`.
fn key(key: &str) -> SubstanceKey {
    SubstanceKey::new(key).unwrap()
}

/// An Earth's rock under water has K⊕: the reference Earth (wet now) takes its sea's water, its
/// palette entry's density (997.047 kg m⁻³), its gravity factor is 1 to the bit, its surface is
/// bedrock (its craters older than the epoch's end far below saturation), and both its crusts are
/// silicate rock at B = 1, so K is 6.3 × 10⁻⁶ m^0.1 a⁻¹ to 2% (6.28 × 10⁻⁶).
#[test]
fn an_earth_s_rock_under_water_has_the_reference_erodibility() {
    let inputs = reference::earth_like();
    assert_same_bits(inputs.gravity().value(), EARTH_GRAVITY.value());
    let erodibility = Erodibility::of(&inputs).unwrap();
    let fluid = erodibility.fluid();
    eprintln!(
        "the reference Earth's fluid: {} at {} kg m⁻³; N_e = {:.3e} km⁻²",
        fluid.key.as_str(),
        fluid.density_kg_m3,
        effective_crater_density(&inputs).value()
    );
    assert_eq!(fluid.key.as_str(), "H2O");
    let main = inputs.main_liquid().unwrap();
    let entry = inputs.palette().get(main).unwrap();
    assert_same_bits(
        fluid.density_kg_m3,
        entry.liquid.as_ref().unwrap().density_kg_m3,
    );
    assert!(!crater_saturated(&inputs));
    assert_eq!(erodibility.surface(), Surface::Bedrock);
    for crust in [Crust::Continental, Crust::Oceanic] {
        let k = erodibility.of_crust(crust);
        assert_within(
            &format!("the reference Earth's {crust:?} crust"),
            k,
            EARTH_ROCK_K,
        );
        assert_same_bits(
            k,
            erodibility_m0_1_per_year(inputs.gravity(), fluid.density_kg_m3, 1.0),
        );
    }
}

/// A saturated Mars's regolith has ten times its rock's K: the reference Mars (dry now, with no
/// liquid on its ground) takes its wet epoch's water at the registry's triple-point density
/// (999.793 kg m⁻³), its craters older than the epoch's end (about 0.050 km⁻²) saturate it at
/// 0.018 km⁻², and its basaltic lid is silicate regolith at B = 10, so K is 2.4 × 10⁻⁵ m^0.1 a⁻¹
/// to 2% (6.3 × 10⁻⁶ × 0.380 × 0.9998 × 10 = 2.39 × 10⁻⁵), which every cell of its lid takes.
#[test]
fn a_saturated_mars_s_regolith_has_ten_times_the_rock_s_erodibility() {
    let inputs = reference::mars_like();
    assert_eq!(inputs.main_liquid(), None);
    let erodibility = Erodibility::of(&inputs).unwrap();
    let fluid = erodibility.fluid();
    let water = substance::by_key("H2O").unwrap().substance();
    let n_e = effective_crater_density(&inputs).value();
    eprintln!(
        "the reference Mars's fluid: {} at {} kg m⁻³; N_e = {n_e:.4} km⁻² ({:.2} × saturation)",
        fluid.key.as_str(),
        fluid.density_kg_m3,
        n_e / BASIN_SATURATION_DENSITY.value()
    );
    assert_eq!(fluid.key.as_str(), "H2O");
    assert_same_bits(
        fluid.density_kg_m3,
        water.phase().unwrap().triple_liquid_density().value(),
    );
    assert!(crater_saturated(&inputs));
    assert_eq!(erodibility.surface(), Surface::SaturatedRegolith);
    let k = erodibility.of_crust(Crust::Lid);
    assert_within("the reference Mars's regolith", k, MARS_REGOLITH_K);
    let rock = erodibility_m0_1_per_year(inputs.gravity(), fluid.density_kg_m3, 1.0);
    assert!((k / rock - 10.0).abs() < 1e-12, "B = {}", k / rock);
    let (grid, climate) = (CellGraph::new(5).unwrap(), CellGraph::new(4).unwrap());
    let pass = Pass::new(reference::SEED, &inputs, &grid, &climate);
    let working = Working::initial(&pass);
    let per_cell = erodibility.per_cell(&working.cells);
    assert_eq!(per_cell.len(), working.cells.len());
    for (cell, &each) in working.cells.iter().zip(&per_cell) {
        assert_eq!(cell.crust, Crust::Lid);
        assert_same_bits(each, k);
    }
}

/// A Titan's water ice under methane has about half an Earth's K: at Titan's gravity (1.354 m
/// s⁻²) under its liquid methane (447.6 kg m⁻³), water-ice bedrock takes the table's B = 8, so K
/// is 3.1 × 10⁻⁶ m^0.1 a⁻¹ to 2% (3.11 × 10⁻⁶), about half of K⊕.
#[test]
fn a_titan_s_ice_under_methane_has_about_half_an_earth_s_erodibility() {
    let methane = Fluid {
        key: key("CH4"),
        density_kg_m3: TITAN_METHANE_DENSITY_KG_M3,
    };
    let bed = bed_factor(
        Substrate {
            mechanics: MechanicsFamily::WaterIce,
            surface: Surface::Bedrock,
        },
        methane.key,
    );
    assert_same_bits(bed, 8.0);
    let k = erodibility_m0_1_per_year(TITAN_GRAVITY, methane.density_kg_m3, bed);
    assert_within("a Titan's ice under methane", k, TITAN_ICE_K);
    let share = k / EARTH_ERODIBILITY_M0_1_PER_YEAR;
    assert!((0.4..0.6).contains(&share), "{share} of K⊕");
}

/// The bed table holds the three calibrated cases (decision-composition) and nothing twice: rock
/// under water 1, a crater-saturated silicate regolith under water 10, and water-ice bedrock under
/// methane, ethane and Titan's methane–ethane–nitrogen sea 8; each row's fluid is a registry key,
/// each has a source, and [`bed_factor`] reads each row's own factor.
#[test]
fn the_bed_table_holds_the_three_calibrated_cases() {
    let rock = Substrate {
        mechanics: MechanicsFamily::Silicate,
        surface: Surface::Bedrock,
    };
    let regolith = Substrate {
        mechanics: MechanicsFamily::Silicate,
        surface: Surface::SaturatedRegolith,
    };
    let ice = Substrate {
        mechanics: MechanicsFamily::WaterIce,
        surface: Surface::Bedrock,
    };
    let expected = [
        (rock, "H2O", 1.0),
        (regolith, "H2O", 10.0),
        (ice, "CH4", 8.0),
        (ice, "C2H6", 8.0),
        (ice, "ch4_c2h6_n2", 8.0),
    ];
    assert_eq!(BED_FACTORS.len(), expected.len());
    for (row, (substrate, fluid, factor)) in BED_FACTORS.iter().zip(expected) {
        assert_eq!(row.substrate, substrate, "{}", row.fluid);
        assert_eq!(row.fluid, fluid);
        assert_same_bits(row.factor, factor);
        assert!(!row.source.is_empty(), "{fluid}'s row has a source");
        assert!(
            substance::by_key(fluid).is_some(),
            "{fluid} is a registry key"
        );
        assert_same_bits(bed_factor(substrate, key(fluid)), factor);
    }
    for (i, a) in BED_FACTORS.iter().enumerate() {
        for b in &BED_FACTORS[i + 1..] {
            assert!(
                !(a.substrate == b.substrate && a.fluid == b.fluid),
                "{:?} under {} twice",
                a.substrate,
                a.fluid
            );
        }
    }
}

/// A pair the table does not hold is the rock-equivalent, B = 1, not modelled: a volatile ice
/// under nitrogen, rock under methane, water ice under water, a regolith of water ice under
/// methane and an organic bed under ethane.
#[test]
fn an_unknown_pair_is_the_rock_equivalent() {
    let pairs = [
        (MechanicsFamily::VolatileIce, Surface::Bedrock, "N2"),
        (MechanicsFamily::Silicate, Surface::Bedrock, "CH4"),
        (MechanicsFamily::WaterIce, Surface::Bedrock, "H2O"),
        (MechanicsFamily::WaterIce, Surface::SaturatedRegolith, "CH4"),
        (MechanicsFamily::Organic, Surface::Bedrock, "C2H6"),
    ];
    for (mechanics, surface, fluid) in pairs {
        let b = bed_factor(Substrate { mechanics, surface }, key(fluid));
        assert_same_bits(b, 1.0);
    }
}

/// A world never wet has no erodibility: the reference Moon.
#[test]
fn a_world_never_wet_has_no_erodibility() {
    let inputs = reference::moon_like();
    assert!(inputs.wet_epoch().is_none());
    assert_eq!(Fluid::of(&inputs), None);
    assert_eq!(Erodibility::of(&inputs), None);
}

/// The Titan sea's density here is the surface crate's synthetic record's, to the bit.
#[cfg(feature = "testing")]
#[test]
fn the_titan_sea_s_density_is_the_testing_world_s() {
    use hyperion_surface::testing::{LIQUID_METHANE_TITAN_DENSITY_KG_M3, liquid_methane_titan};
    assert_same_bits(
        TITAN_METHANE_DENSITY_KG_M3,
        LIQUID_METHANE_TITAN_DENSITY_KG_M3,
    );
    assert_same_bits(
        TITAN_METHANE_DENSITY_KG_M3,
        liquid_methane_titan().density_kg_m3,
    );
}
