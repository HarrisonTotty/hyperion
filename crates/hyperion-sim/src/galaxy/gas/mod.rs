//! The gas and dust field: density, pressure, phase and dust anywhere in the root cube (plan 07).
//!
//! The field is a pure function of the seed and a position, built from plan 02's galaxy: it shares
//! the gas disc's mass, scale length and scale height ([`params::GasParams`]), the arm geometry its
//! lanes follow and the metallicity its dust-to-gas ratio follows. Nothing stellar reads it; the
//! consumers are the extinction integral along a line of sight, a supernova shell's site, and the
//! galaxy map's dust layer.
//!
//! Units here are plan 07's Design note 2, which are the units every measurement of the
//! interstellar medium is quoted in, not SI: a density is hydrogen nuclei per cubic centimetre
//! ([`HydrogenPerCm3`](crate::units::HydrogenPerCm3)), a pressure is P ÷ k in K cm⁻³
//! ([`KelvinPerCm3`](crate::units::KelvinPerCm3)), and a length at this interface is light-years,
//! as in the rest of [`galaxy`](crate::galaxy). [`units::consts`](crate::units::consts) stays SI,
//! so the non-SI factors between the two live here and are derived from it at compile time.
//!
//! Hot paths take a cylindrical radius and a height as bare `f64` light-years and return a bare
//! `f64` whose unit their documentation states, as plan 02's fields do; the facade that assembles
//! them wraps both in newtypes.

pub mod ccm;
pub mod extinction;
pub mod field;
pub mod lanes;
pub mod map;
pub mod modifiers;
pub mod noise;
pub mod params;
pub mod phase;
pub mod pressure;
pub mod smooth;

use crate::units::consts::{HYDROGEN_MASS_KG, METRES_PER_LIGHT_YEAR, SOLAR_MASS_KG};

/// Centimetres in a light-year, 9.460 730 472 580 8 × 10¹⁷.
///
/// Derived from [`METRES_PER_LIGHT_YEAR`], which is exact, so that the non-SI unit the interstellar
/// medium is measured in cannot drift from the SI one (Design note 2).
pub const CENTIMETRES_PER_LIGHT_YEAR: f64 = METRES_PER_LIGHT_YEAR * 100.0;

/// Cubic centimetres in a cubic light-year, about 8.468 × 10⁵³.
pub const CUBIC_CENTIMETRES_PER_CUBIC_LIGHT_YEAR: f64 =
    CENTIMETRES_PER_LIGHT_YEAR * CENTIMETRES_PER_LIGHT_YEAR * CENTIMETRES_PER_LIGHT_YEAR;

/// The mass the gas carries per hydrogen nucleus, in units of the hydrogen atom's own mass: 1.4,
/// the helium that accompanies the hydrogen (Design note 2).
///
/// A helium mass fraction of 0.28 of the hydrogen-and-helium mass gives 1 ÷ (1 − 0.28) = 1.39, and
/// the ratio is quoted as 1.4 throughout the literature on the interstellar medium.
pub const MASS_PER_HYDROGEN_FACTOR: f64 = 1.4;

/// Solar masses in a cubic light-year of gas holding one hydrogen nucleus per cubic centimetre,
/// about 9.978 × 10⁻⁴ M☉.
///
/// This is what turns a density in cm⁻³ into the mass budget's units, and back: a component's
/// central density is its mass ÷ (this × its volume integral in ly³) (Design note 4).
pub const SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY: f64 =
    MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG * CUBIC_CENTIMETRES_PER_CUBIC_LIGHT_YEAR
        / SOLAR_MASS_KG;

/// Particles per hydrogen nucleus in ionised gas, 2.3: the free electrons and the helium as well
/// as the protons (Design note 12).
///
/// It turns a pressure P ÷ k and a density n into the equilibrium temperature T = (P ÷ k) ÷ (x n),
/// which is what labels a point's phase. Neutral gas takes
/// [`NEUTRAL_PARTICLES_PER_HYDROGEN`] instead.
pub const IONISED_PARTICLES_PER_HYDROGEN: f64 = 2.3;

/// Particles per hydrogen nucleus in neutral gas, 1.1: the atoms and the helium's tenth (Design
/// note 12).
///
/// The warm and cold phases' thresholds are taken with it: gas is warm down to 5,000 K, which is
/// `n < P ÷ (1.1 × 5,000 k)`.
pub const NEUTRAL_PARTICLES_PER_HYDROGEN: f64 = 1.1;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_relative(actual: f64, expected: f64, tolerance: f64) {
        let error = ((actual - expected) / expected).abs();
        assert!(
            error <= tolerance,
            "{actual} is {error:e} from {expected}, over {tolerance:e}"
        );
    }

    /// The light-year is exactly 9,460,730,472,580,800 m, so a centimetre count is that times 100
    /// and the cube of it is about 8.468 × 10⁵³.
    #[test]
    fn the_non_si_factors_follow_from_the_light_year() {
        assert_relative(CENTIMETRES_PER_LIGHT_YEAR, 9.460_730_472_580_8e17, 1e-15);
        assert_relative(CUBIC_CENTIMETRES_PER_CUBIC_LIGHT_YEAR, 8.467_867e53, 1e-6);
    }

    /// One hydrogen nucleus per cubic centimetre over a cubic light-year is about a thousandth of
    /// a solar mass, which is the figure the mass budget of Design note 4 rests on.
    #[test]
    fn a_cubic_light_year_of_unit_density_gas_weighs_a_millisolar_mass() {
        assert_relative(SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY, 9.978e-4, 1e-3);
    }
}
