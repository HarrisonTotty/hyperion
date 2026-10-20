//! The gas columns, and a mixture's mean molar mass (decision-composition §1.1; P14.T49.a, with
//! P14.T49.b's molecular constants).
//!
//! A gas row's columns are its [`MolecularConstants`], over which [`thermo`](super::thermo) sums
//! its `c_p`(T), H̃ and S̃ (P14.T49.b). No row holds a fixed `c_p`: science-r08-gases-2.md §1 rules
//! that none serves the galaxy's 30–1,500 K. The kinetic-theory classes P14.T49.a's rows 0–8 held
//! (Robinson and Catling 2012, ApJ 757, 104, eq. 9) retired at P14.T54.a's switch, which publishes
//! [`mixture_heat_capacity_over_r_at`](super::mixture_heat_capacity_over_r_at) at the mean surface
//! temperature in their place.
//!
//! A mixture is a list of `(SubstanceId, x)` mole fractions, summed in the order given: P14.T24.a
//! sorts it, largest first with ties by id, so that its sums are output it fixes. Molar heat
//! capacities add over an ideal mixture's moles, so `c_p` = Σ xᵢ `c_p,ᵢ`, as
//! [`thermo`](super::thermo) sums them; averaging γ instead is wrong.

use super::thermo::MolecularConstants;
use super::{Source, SubstanceId};

/// A gas row's columns: its molecular constants, with their sources.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasColumns {
    pub(super) molecular: MolecularConstants,
    pub(super) sources: &'static [Source],
}

impl GasColumns {
    /// The gas's molecular constants, over which its `c_p`(T), H̃ and S̃ are summed
    /// ([`thermo`](super::thermo)).
    #[must_use]
    pub const fn molecular(&self) -> &MolecularConstants {
        &self.molecular
    }

    /// Where each of these values comes from.
    #[must_use]
    pub const fn sources(&self) -> &'static [Source] {
        self.sources
    }
}

/// The mean molar mass μ = Σ xᵢ Mᵢ of the mixture `mixture`, g mol⁻¹, summed in its order.
///
/// The fractions are the mixture's mole fractions, summing to 1, and are not renormalised. `None`
/// for an empty mixture, or one with a row that has no molar mass.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::mean_molar_mass_g_per_mol;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// // Titan's lower atmosphere, 94.35% nitrogen and 5.65% methane: about 27.3 g mol⁻¹.
/// let titan = [(Gas::Nitrogen.substance(), 0.9435), (Gas::Methane.substance(), 0.0565)];
/// assert!((mean_molar_mass_g_per_mol(&titan)? - 27.34).abs() < 0.01);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn mean_molar_mass_g_per_mol(mixture: &[(SubstanceId, f64)]) -> Option<f64> {
    mixed(mixture, |id| id.substance().molar_mass_g_per_mol())
}

/// Σ xᵢ vᵢ over `mixture` in its order, with vᵢ = `value(idᵢ)`; `None` if the mixture is empty or a
/// value is missing.
pub(super) fn mixed(
    mixture: &[(SubstanceId, f64)],
    value: impl Fn(SubstanceId) -> Option<f64>,
) -> Option<f64> {
    if mixture.is_empty() {
        return None;
    }
    mixture
        .iter()
        .try_fold(0.0, |sum, &(id, x)| Some(sum + x * value(id)?))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::derive::atmosphere::Gas;

    fn pure(gas: Gas) -> [(SubstanceId, f64); 1] {
        [(gas.substance(), 1.0)]
    }

    #[test]
    fn a_pure_gas_s_mean_molar_mass_is_its_own() {
        for gas in Gas::ALL {
            assert_same_bits(
                mean_molar_mass_g_per_mol(&pure(gas)).unwrap(),
                gas.molar_mass_g_per_mol(),
            );
        }
    }

    /// Dry air, Earth's 78.084% N₂, 20.946% O₂, 0.934% Ar and 0.036% CO₂ by volume, gives
    /// 28.966 g mol⁻¹ (the U.S. Standard Atmosphere 1976's 28.9644 for its composition).
    #[test]
    fn mixtures_sum_their_parts_by_mole_fraction() {
        let air = [
            (Gas::Nitrogen.substance(), 0.780_84),
            (Gas::Oxygen.substance(), 0.209_46),
            (Gas::Argon.substance(), 0.009_34),
            (Gas::CarbonDioxide.substance(), 0.000_36),
        ];
        let mu = mean_molar_mass_g_per_mol(&air).unwrap();
        assert!((mu - 28.9644).abs() < 0.005, "{mu}");
    }

    /// The sum runs in the order given, so a reordered mixture is a different sum, which is why
    /// P14.T24.a fixes the order; here the two agree to rounding.
    #[test]
    fn the_sum_runs_in_the_order_given() {
        let a = (Gas::Nitrogen.substance(), 0.1);
        let b = (Gas::Hydrogen.substance(), 0.2);
        let c = (Gas::CarbonDioxide.substance(), 0.7);
        let by_hand = 0.0 + 0.1 * 28.014 + 0.2 * 2.016 + 0.7 * 44.009;
        assert_same_bits(mean_molar_mass_g_per_mol(&[a, b, c]).unwrap(), by_hand);
        let other = mean_molar_mass_g_per_mol(&[c, b, a]).unwrap();
        assert!((other - by_hand).abs() < 1e-12);
    }

    #[test]
    fn an_empty_mixture_has_no_mean() {
        assert_eq!(mean_molar_mass_g_per_mol(&[]), None);
    }
}
