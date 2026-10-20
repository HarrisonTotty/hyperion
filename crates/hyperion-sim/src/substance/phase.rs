//! The phase vocabulary the registry's phase diagrams share (decision-composition §1.1;
//! P14.T49.a): the molar gas constant, the [`Phase`] a substance lies in at a temperature and
//! pressure, and ice Ih's density.
//!
//! Where a substance condenses, and in which phase it lies, is its phase diagram's
//! ([`diagram`](super::diagram), P14.T49.b's fitted curves: each row's reference equations of
//! state, evaluated verbatim). Until the 22 → 23 batch's wiring (P14.T54.a) this module also held
//! P14.T49.a's phase columns, five constant-enthalpy Clausius–Clapeyron sets for water, methane,
//! nitrogen, carbon dioxide and argon (P14.T13.c's effective enthalpies, with methane's for
//! P14.T24.f); every caller now reads the diagrams, and the columns are retired.

use crate::units::{Kelvin, KilogramsPerCubicMetre};

/// The molar gas constant, J mol⁻¹ K⁻¹ (exact in the 2019 SI, the Avogadro constant times
/// Boltzmann's: 8.314 462 618 153 24).
pub const MOLAR_GAS_CONSTANT: f64 = 8.314_462_618_153_24;

/// The phase a substance lies in at a temperature and pressure
/// ([`phase_at`](super::diagram::phase_at)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    /// The solid: an ice, a frost or a crystal.
    Solid,
    /// The liquid.
    Liquid,
    /// The vapour, below the saturation pressure or above the critical temperature and below the
    /// critical pressure: the gas.
    Vapour,
    /// The supercritical fluid, at or above both the critical temperature and the critical
    /// pressure, which has no interface (Venus's carbon dioxide).
    Supercritical,
}

/// The melting point of ice Ih at 1 atm, kelvin: 273.152 519 K (IAPWS R10-06), to 0.01 K, where
/// [`ice_ih_density`] stops.
const ICE_IH_MELTING_K: f64 = 273.15;

/// Ice Ih's density at `temperature` and 1 atm: the Gibbs function of IAPWS R10-06(2009)
/// (Feistel and Wagner 2006, J. Phys. Chem. Ref. Data 35, 1021), as the polynomial
/// 933.790 351 + 9.911 602 x³ − 109.185 868 x⁴ + 127.136 897 x⁵ − 44.949 496 x⁶ in
/// x = T ÷ 273.15 K, fitted to it over 0.5–273.15 K within 0.02 kg m⁻³, below the release's own
/// uncertainty, 0.1 per cent below 268 K and 0.02 per cent above (its Table 5;
/// science-composition-crusts.md §3); clamped to 0 K and the melting point.
///
/// Moved here verbatim from the reference worlds' palettes by P14.T48.f, whose ice-rich crater
/// targets take it at the body's mean surface temperature (science-r09-craters.md F5.3);
/// P14.T49.b makes it water's `ρ_s`(T).
#[must_use]
pub fn ice_ih_density(temperature: Kelvin) -> KilogramsPerCubicMetre {
    let x = temperature.value().clamp(0.0, ICE_IH_MELTING_K) / 273.15;
    let x3 = x * x * x;
    KilogramsPerCubicMetre::new(
        933.790_351 + x3 * (9.911_602 + x * (-109.185_868 + x * (127.136_897 - 44.949_496 * x))),
    )
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// Ice Ih's density follows IAPWS R10-06(2009) to 0.03 kg m⁻³ (science-composition-crusts.md
    /// §3), clamped to its melting point and to 0 K.
    #[test]
    fn ice_ih_s_density_follows_iapws() {
        let at = |k: f64| ice_ih_density(Kelvin::new(k)).value();
        for (k, rho) in [(273.15, 916.72), (100.0, 933.05), (160.0, 929.87)] {
            assert!((at(k) - rho).abs() < 0.03, "{} kg m⁻³ at {k} K", at(k));
        }
        assert_same_bits(at(300.0), at(ICE_IH_MELTING_K));
        assert_same_bits(at(-1.0), at(0.0));
    }
}
