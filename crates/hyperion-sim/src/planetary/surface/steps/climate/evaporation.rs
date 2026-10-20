//! The precipitation heuristic's evaporation over open liquid (R09.T13.e; Design note 8,
//! decision-r09-t13ge §2.6, e6): Priestley and Taylor's equilibrium evaporation over the climate
//! condensable's own unfrozen liquid, its energy the sea's share of its absorbed sunlight less what
//! the sea stores.
//!
//! The source is Priestley, C. H. B., and Taylor, R. J. 1972, "On the Assessment of Surface Heat
//! Flux and Evaporation Using Large-Scale Parameters", Mon. Wea. Rev. 100, 81–92, read: the
//! evaporation of a horizontally uniform saturated surface in energy units is α s ÷ (s + γ)
//! (R − G), α = 1.26 (their eq. 14), s the slope of the saturation specific humidity with the
//! temperature and γ = `c_p` ÷ L (their eq. 5), R the net radiation and G the heat into the
//! ground, here the sea's storage. Over a condensable's sea, in kg m⁻² s⁻¹:
//!
//! E = min(1, α s ÷ (s + γ)) × max(0, β `S_abs` − `C_sea` dT/dt) ÷ L,
//!
//! - β = 0.47, the ocean's net surface radiation over its absorbed sunlight
//!   ([`OCEAN_NET_RADIATION_SHARE`]): the model has no surface energy budget of its own, so R is
//!   that share of the sunlight the sea absorbs, `S_abs`, the month's insolation × (1 − albedo) at
//!   the water's surface from the model's own records (decision-r09-t13ge §2.11);
//! - `C_sea` dT/dt the storage, the sea's heat capacity times the month's centred change of its
//!   temperature, none in a one-month year; and none where it would exceed the energy;
//! - the clamp at 1: a wet surface in equilibrium cannot spend more than its available energy on
//!   evaporation; it binds above about 304 K for water and at Titan's 93.7 K for methane
//!   (decision-r09-t13ge §2.6);
//! - s from the saturation specific humidity `q_s` = ε `e_s` ÷ (p − (1 − ε) `e_s`) at the surface
//!   pressure, `e_s` the registry's saturation pressure of the condensable, by a centred difference
//!   of 0.01 K as the model takes h's slope, over the liquid's branch: within that step of the
//!   registry's triple point, where the difference would read the solid's branch below it, at a
//!   step above the triple point; where `e_s` reaches the surface pressure on either side the
//!   liquid boils, and the clamp holds.
//!
//! None evaporates from frozen liquid (below the model's freezing point) or from any liquid but the
//! condensable's own: R09.T13.d's other liquids (a Titan's ethane) give the air no vapour of the
//! condensable.

use super::ebm::{ENERGY_SLOPE_STEP_K, Moisture};
use crate::substance::saturation_pressure;
use crate::units::{Kelvin, Pascals};

/// Priestley and Taylor's α, the equilibrium evaporation's multiplier over a saturated surface:
/// 1.26, "the overall mean (land and water)" of their sites (1972, §6 and eq. 14, read; 1.28 without
/// the fluxatron). Medium confidence for Earth's oceans; the same for every condensable, a figure
/// measured on water.
pub(super) const PRIESTLEY_TAYLOR_COEFFICIENT: f64 = 1.26;

/// β, the ocean's net surface radiation over the sunlight it absorbs, the share of the absorbed
/// sunlight the heuristic gives its evaporation: 0.47, Wild et al. 2015's (Clim. Dyn. 44, 3393,
/// abstract read) ocean net surface radiation, 117 W m⁻², over the ocean's absorbed sunlight at
/// the top of the air, about 245–250 W m⁻² (its surface 170 plus the air's ~77, from memory;
/// decision-r09-t13ge §2.6, e6, which amends the lane's global 0.44). It is the Earth's ocean's,
/// named apart from the orographic model's re-evaporation β ([`lfpm`](super::lfpm)); a Titan's haze
/// would want its own (decision-r09-t13ge §3, E2). Medium confidence on the Earth.
pub(super) const OCEAN_NET_RADIATION_SHARE: f64 = 0.47;

/// The equilibrium evaporation's coefficient min(1, α s ÷ (s + γ)) of `moisture` in an air of
/// specific heat `specific_heat_j_per_kg_k`, J kg⁻¹ K⁻¹, at the liquid's temperature `t_k`, K (the
/// [module documentation](self)): 1 where the liquid boils within the difference's step.
#[must_use]
pub(super) fn coefficient(moisture: &Moisture, specific_heat_j_per_kg_k: f64, t_k: f64) -> f64 {
    let p = moisture.surface_pressure_pa;
    let epsilon = moisture.mass_ratio;
    let saturated = |t: f64| -> Option<f64> {
        let e =
            saturation_pressure(moisture.condensable, Kelvin::new(t)).map_or(0.0, Pascals::value);
        // By comparison, so that an infinite pressure (above the critical point) boils.
        (e < p).then(|| epsilon * e / (p - (1.0 - epsilon) * e))
    };
    // The liquid's slope: within a step of the triple point the difference would read the solid's
    // branch below it, so it is taken a step above.
    let triple_k = moisture
        .condensable
        .substance()
        .phase()
        .map_or(0.0, |phase| phase.triple_temperature().value());
    let at_k = if t_k < triple_k + ENERGY_SLOPE_STEP_K {
        triple_k + ENERGY_SLOPE_STEP_K
    } else {
        t_k
    };
    let (Some(above), Some(below)) = (
        saturated(at_k + ENERGY_SLOPE_STEP_K),
        saturated(at_k - ENERGY_SLOPE_STEP_K),
    ) else {
        return 1.0;
    };
    let slope = (above - below) / (2.0 * ENERGY_SLOPE_STEP_K);
    let gamma = specific_heat_j_per_kg_k / moisture.latent_heat_j_per_kg;
    let equilibrium = PRIESTLEY_TAYLOR_COEFFICIENT * slope / (slope + gamma);
    if equilibrium < 1.0 { equilibrium } else { 1.0 }
}

/// The evaporation rate, kg m⁻² s⁻¹, of an open liquid of coefficient `coefficient`
/// ([`coefficient`]) that absorbs `absorbed_w_m2` of sunlight and stores `storage_w_m2`, under the
/// latent heat `latent_heat_j_per_kg`: `coefficient` × max(0, β `S_abs` − G) ÷ L.
#[must_use]
pub(super) fn rate_kg_m2_s(
    coefficient: f64,
    absorbed_w_m2: f64,
    storage_w_m2: f64,
    latent_heat_j_per_kg: f64,
) -> f64 {
    let energy = OCEAN_NET_RADIATION_SHARE * absorbed_w_m2 - storage_w_m2;
    if energy > 0.0 {
        coefficient * energy / latent_heat_j_per_kg
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::hooks::surface::EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K;
    use crate::planetary::surface::reference;
    use crate::planetary::surface::steps::climate::ebm::EnergyBalance;

    /// Over the reference Earth's water, α s ÷ (s + γ) is the lane's exact form's (scratch
    /// `e/pt-ratios.txt`, Murray's saturation curve at 1,013 hPa, `c_p` 1,005 and L 2.465 MJ kg⁻¹):
    /// 0.511 at 273.15 K, 0.837 at 291.15 K and 0.994 at 303.15 K, to within 0.015, the difference
    /// of the registry's constant-enthalpy curve, its L of 2.415 MJ kg⁻¹, the moist air's `c_p` and
    /// the reference Earth's 985.5 hPa (0.498, 0.825 and 0.987 as built; at the triple point the
    /// liquid's slope is taken); the clamp binds above about 304 K; the rate is the coefficient
    /// times the share of the energy less the storage, and nothing where the storage takes it all.
    #[test]
    fn the_coefficient_is_priestley_and_taylor_s_and_clamped_at_one() {
        let model = EnergyBalance::for_inputs(&reference::earth_like());
        let moisture = model.moisture.expect("the reference Earth is wet");
        let cp = model.specific_heat_j_per_kg_k;
        assert!(
            (cp - EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K).abs() < 60.0,
            "{cp}"
        );
        eprintln!(
            "c_p {cp:.2} J kg⁻¹ K⁻¹, L {:.4e} J kg⁻¹, ε {:.4}, p {} Pa, γ {:.4e} K⁻¹",
            moisture.latent_heat_j_per_kg,
            moisture.mass_ratio,
            moisture.surface_pressure_pa,
            cp / moisture.latent_heat_j_per_kg
        );
        for (t, expected) in [(273.15, 0.511), (291.15, 0.837), (303.15, 0.994)] {
            let c = coefficient(&moisture, cp, t);
            let e = saturation_pressure(moisture.condensable, Kelvin::new(t))
                .unwrap()
                .value();
            eprintln!("T {t} K: e_s {e:.2} Pa, α s ÷ (s + γ) {c:.4}, the lane's {expected}");
            assert!((c - expected).abs() < 0.015, "{t}: {c}");
        }
        hyperion_testkit::float::assert_same_bits(coefficient(&moisture, cp, 310.0), 1.0);
        hyperion_testkit::float::assert_same_bits(coefficient(&moisture, cp, 400.0), 1.0);
        let c = coefficient(&moisture, cp, 291.15);
        let rate = rate_kg_m2_s(c, 200.0, 10.0, moisture.latent_heat_j_per_kg);
        let expected = c * (0.47 * 200.0 - 10.0) / moisture.latent_heat_j_per_kg;
        assert!((rate - expected).abs() <= 1e-15 * expected);
        hyperion_testkit::float::assert_same_bits(
            rate_kg_m2_s(c, 200.0, 95.0, moisture.latent_heat_j_per_kg),
            0.0,
        );
    }
}
