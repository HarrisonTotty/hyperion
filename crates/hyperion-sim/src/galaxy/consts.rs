//! The galaxy model's working units and the constants between them and SI.
//!
//! Generation code computes in light-years, solar masses, Julian years and km/s, as the
//! brainstorm's "Coordinates" allows where the source formulae use them (plan 02, Design note 1).
//! Every constant here is derived from the SI constants of [`units::consts`](crate::units::consts)
//! at compile time, never typed as a literal, so the working units and SI cannot drift apart.

use crate::units::consts::{
    GM_SUN, METRES_PER_KILOPARSEC, METRES_PER_LIGHT_YEAR, METRES_PER_PARSEC,
    METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND, SECONDS_PER_JULIAN_YEAR,
};

/// The gravitational constant in working units, ly (km/s)² M☉⁻¹: GM☉ ÷ (1 ly × (1 km/s)²).
///
/// About 1.402 77 × 10⁻², which is the familiar 4.300 91 × 10⁻³ pc (km/s)² M☉⁻¹ in light-years.
/// It rests on the nominal GM☉ (IAU 2015 Resolution B3), which is what defines the solar mass in
/// [`units`](crate::units), so `G × 1 M☉` is exactly GM☉ in these units. With it,
/// `v² = G M ÷ r` takes a mass in M☉ and a radius in light-years and gives (km/s)².
pub const G: f64 = GM_SUN
    / (METRES_PER_LIGHT_YEAR
        * METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND
        * METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND);

/// Light-years travelled in one Julian year at 1 km/s: 1 ÷ c in km/s, about 3.3356 × 10⁻⁶.
///
/// It turns a speed in km/s over a length in light-years into an angular frequency in radians per
/// year: Ω = v ÷ R × this.
pub const LIGHT_YEARS_PER_YEAR_PER_KM_S: f64 =
    METRES_PER_SECOND_PER_KILOMETRE_PER_SECOND * SECONDS_PER_JULIAN_YEAR / METRES_PER_LIGHT_YEAR;

/// Light-years in a parsec, about 3.261 56 (IAU 2015 Resolution B2 through
/// [`units::consts::METRES_PER_PARSEC`](crate::units::consts::METRES_PER_PARSEC)).
pub const LIGHT_YEARS_PER_PARSEC: f64 = METRES_PER_PARSEC / METRES_PER_LIGHT_YEAR;

/// Light-years in a kiloparsec, about 3,261.56.
pub const LIGHT_YEARS_PER_KILOPARSEC: f64 = METRES_PER_KILOPARSEC / METRES_PER_LIGHT_YEAR;

/// Julian years in a megayear.
pub const YEARS_PER_MEGAYEAR: f64 = 1e6;

/// Julian years in a gigayear.
pub const YEARS_PER_GIGAYEAR: f64 = 1e9;

/// Light-years in a megaparsec, for the Hubble constant's unit.
pub const LIGHT_YEARS_PER_MEGAPARSEC: f64 = LIGHT_YEARS_PER_KILOPARSEC * 1e3;

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

    /// The textbook value, 4.300 91 × 10⁻³ pc M☉⁻¹ (km/s)², converted to light-years. It is
    /// quoted to six figures, so it is checked to the sixth.
    #[test]
    fn g_in_working_units_matches_the_textbook_value() {
        let textbook_pc = 4.300_91e-3;
        assert_relative(G, textbook_pc * LIGHT_YEARS_PER_PARSEC, 2e-6);
        assert_relative(G / LIGHT_YEARS_PER_PARSEC, textbook_pc, 2e-6);
    }

    /// The Sun's circular speed around the Milky Way: about 230 km/s at 26,700 ly gives an orbital
    /// period near 220 Myr.
    #[test]
    fn frequencies_come_out_in_radians_per_year() {
        let omega = 230.0 / 26_700.0 * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        let period_myr = 2.0 * std::f64::consts::PI / omega / YEARS_PER_MEGAYEAR;
        assert!((215.0..225.0).contains(&period_myr), "{period_myr} Myr");
        assert_relative(LIGHT_YEARS_PER_YEAR_PER_KM_S, 1.0 / 299_792.458, 1e-15);
    }

    #[test]
    fn distance_units_agree() {
        assert_relative(LIGHT_YEARS_PER_PARSEC, 3.261_563_777, 1e-9);
        assert_relative(LIGHT_YEARS_PER_KILOPARSEC, 3_261.563_777, 1e-9);
        assert_relative(LIGHT_YEARS_PER_MEGAPARSEC, 3.261_563_777e6, 1e-9);
    }
}
