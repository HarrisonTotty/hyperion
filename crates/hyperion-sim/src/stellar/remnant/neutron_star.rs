//! Neutron stars (plan 06, P06.T21): birth, spin-down and emission.
//!
//! Until P06.T21 builds the pulsar model, this holds the neutron-star luminosity of Hurley, Pols
//! and Tout (2000, MNRAS 315, 543, "HPT", section 6.2.2, equation 93), which the track's remnant
//! stage needs (ruling 33 of 2026-09-22).

use crate::math;
use crate::units::{SolarLuminosities, SolarMasses, Years};

/// The luminosity of a neutron star of `mass`, `age` after its birth, by HPT's photon-cooling law
/// (equation 93): L = 0.02 M^(2/3) ÷ max(t, 0.1)² L☉ with t in Myr, constant for the first 10⁵
/// years, which HPT calibrate to an effective temperature of about 2 × 10⁶ K for the Crab pulsar.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive or `age` is negative.
#[must_use]
pub(crate) fn hpt_luminosity(mass: SolarMasses, age: Years) -> SolarLuminosities {
    debug_assert!(
        mass.value() > 0.0 && age.value() >= 0.0,
        "a neutron star of {mass:?} at {age:?}"
    );
    let t_myr = (age.value() * 1e-6).max(0.1);
    SolarLuminosities::new(0.02 * math::powf(mass.value(), 2.0 / 3.0) / (t_myr * t_myr))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 0.02 × 1.4^⅔ ÷ 0.1² = 2.50 L☉ for the first 10⁵ years, then falling as t⁻².
    #[test]
    fn a_neutron_star_cools_as_the_inverse_square_of_its_age() {
        let m = SolarMasses::new(1.4);
        let young = hpt_luminosity(m, Years::new(5e4)).value();
        assert!(
            (young - 2.0 * math::powf(1.4, 2.0 / 3.0)).abs() < 1e-12,
            "{young}"
        );
        let same = hpt_luminosity(m, Years::new(1e5)).value();
        assert!((same / young - 1.0).abs() < 1e-15);
        let older = hpt_luminosity(m, Years::new(1e6)).value();
        assert!((older * 100.0 / young - 1.0).abs() < 1e-12, "{older}");
    }
}
