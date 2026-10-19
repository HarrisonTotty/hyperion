//! The lapse rate (R09.T13.d; decision-r09-t13a-relief §2.1): plan 14's ground gradient of its
//! air's profile, the one rate per body by which the climate's temperatures are reduced to sea
//! level, which the header carries and the client re-applies at each point's height.
//!
//! Plan 14's profile (`hooks::structure`, P14.T24.e) leaves the ground on dln T ÷ dln p =
//! α R ÷ `c_p`(T), Robinson and Catling's (2012, ApJ 757, 104, Eq. 10) scaled adiabat with `c_p`(T)
//! the registry's mixture at T. With hydrostatic balance, dln p ÷ dz = −μ g ÷ (R T), so its
//! gradient at the ground is
//!
//! ```text
//! Γ = −dT/dz = α μ g ÷ c_p,molar = α g ÷ c_p,
//! ```
//!
//! `c_p` per kilogram at the mean surface temperature `T_s` (research-d §4). α is plan 14's
//! [`lapse_factor`]: water's 0.6 and methane's 0.77 where their latent heat can drive a moist
//! lapse rate, 0.88 otherwise (Robinson and Catling 2012, 2014). Not the dry adiabat, which
//! overstates a moist air's, and not Earth's environmental 6.5 K km⁻¹ (U.S. Standard Atmosphere
//! 1976), which is no law for another world: the reference Earth's is about 5.8 K km⁻¹, inside
//! the near-surface gradients measured over terrain, annual means of 3.9–5.2 K km⁻¹ (Minder, Mote
//! and Lundquist 2010, JGR Atmospheres 115, doi:10.1029/2009JD013493) and 5.4–5.8 K km⁻¹
//! (Rolland 2003, J. Climate 16, 1032), abstracts read.
//!
//! The energy-balance and isothermal models take it; the radiative model takes none, airless by
//! the header's contract and under a thin air because its ground is in radiative balance with
//! the sun, not on the air's adiabat (decision-r09-t13d-normalisation §2.4). R09.T13.g's
//! energy-balance model reads the same function.

use super::ebm::Air;
use crate::planetary::hooks::structure::lapse_factor;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::substance::{SubstanceId, by_key};

/// The lapse rate of the air of `inputs`, K m⁻¹, positive where the temperature falls with height:
/// Γ = α g ÷ `c_p`(`T_s`) (the [module documentation](self)), α from plan 14's [`lapse_factor`] over
/// the air's gases and climate condensable at the mean surface temperature, `c_p` the air's own
/// mixture there ([`Air::of`], which takes Earth's dry air where the registry cannot mix the
/// gases, and then the dry factor). Zero on an airless body: no surface pressure, or no gases.
#[must_use]
pub(super) fn lapse_rate_k_per_m(inputs: &CoarseInputs) -> f64 {
    let pressure = inputs.surface_pressure().value();
    // A pressure that is not a number reads as airless.
    if pressure.is_nan() || pressure <= 0.0 || inputs.gases().is_empty() {
        return 0.0;
    }
    let mixture: Option<Vec<(SubstanceId, f64)>> = inputs
        .gases()
        .iter()
        .map(|gas| by_key(gas.substance.as_str()).map(|id| (id, gas.mole_fraction)))
        .collect();
    let condensable = inputs
        .climate()
        .condensable
        .and_then(|key| by_key(key.as_str()));
    let alpha = lapse_factor(
        mixture.as_deref().unwrap_or(&[]),
        inputs.mean_surface_temperature(),
        condensable,
    );
    alpha * inputs.gravity().value() / Air::of(inputs).specific_heat_j_per_kg_k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::hooks::structure::{DRY_LAPSE_FACTOR, WATER_LAPSE_FACTOR};
    use crate::planetary::surface::reference;

    /// The reference Earth's air, with its water vapour, takes water's α, 0.6, and its moist
    /// `c_p`: about 5.8 K km⁻¹ (decision-r09-t13a-relief §2.1), where its dry air alone would take
    /// the dry 0.88, about 8.6. Mars's takes the dry factor, about 4.4 K km⁻¹ on the registry's
    /// `c_p` (research-d §4's 4.0 on the class values), and the airless Moon none.
    #[test]
    fn the_reference_earth_s_lapse_rate_is_its_moist_air_s() {
        let earth = reference::earth_like();
        let gamma = lapse_rate_k_per_m(&earth);
        let air = Air::of(&earth);
        eprintln!(
            "Earth: Γ {:.3} K km⁻¹, c_p {:.1} J kg⁻¹ K⁻¹, μ {:.3} g mol⁻¹",
            gamma * 1e3,
            air.specific_heat_j_per_kg_k,
            air.molar_mass_g_per_mol
        );
        assert!((gamma * 1e3 - 5.8).abs() < 0.1, "{gamma}");
        let expected = WATER_LAPSE_FACTOR * earth.gravity().value() / air.specific_heat_j_per_kg_k;
        assert!((gamma - expected).abs() < 1e-15);
        let mars = reference::mars_like();
        let mars_gamma = lapse_rate_k_per_m(&mars);
        let mars_air = Air::of(&mars);
        assert!(
            (mars_gamma
                - DRY_LAPSE_FACTOR * mars.gravity().value() / mars_air.specific_heat_j_per_kg_k)
                .abs()
                < 1e-15
        );
        // Research-d §4's 4.0 K km⁻¹ took the class heat capacity; the statistical sum's carbon
        // dioxide at 214 K is cooler-blooded, about 750 J kg⁻¹ K⁻¹.
        assert!((4.0..4.6).contains(&(mars_gamma * 1e3)), "{mars_gamma}");
        hyperion_testkit::float::assert_same_bits(lapse_rate_k_per_m(&reference::moon_like()), 0.0);
    }
}
