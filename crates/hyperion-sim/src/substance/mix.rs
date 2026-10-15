//! The gas columns, and a mixture's mean molar mass and held heat capacity (decision-composition
//! §1.1; P14.T49.a, with P14.T49.b's molecular constants).
//!
//! A gas row's columns are its [`MolecularConstants`], over which [`thermo`](super::thermo) sums
//! its `c_p`(T), H̃ and S̃ (P14.T49.b), and, for rows 0–8 until the 22 → 23 batch's bump, the
//! `c_p` ÷ R class 21 → 22 publishes ([`HeatCapacityClass`]).
//!
//! The class is kinetic theory's for fixed degrees of freedom, as Robinson and Catling (2012, ApJ
//! 757, 104, §2.3, eqs. 7–9) take it: 1 + N ÷ 2 for N degrees, so that the dry adiabat's exponent
//! R ÷ `c_p` is 2 ÷ (N + 2). P14.T24.a's published mixture reads it through [`heat_capacity_over_r`]
//! at generator version 22. It is held, not in force: science-r08-gases-2.md §1 rules that no
//! fixed `c_p` serves the galaxy's 30–1,500 K, and the classes are only the statistical sum's
//! low-temperature limit (CO₂'s 13/3 is not even that: its limit is 7/2). At the batch's bump
//! (P14.T54.a) the published `c_p` ÷ R becomes the mixture's
//! [`mixture_heat_capacity_over_r_at`](super::mixture_heat_capacity_over_r_at) at the mean surface
//! temperature, and the class, this function and their tests retire.
//!
//! A mixture is a list of `(SubstanceId, x)` mole fractions, summed in the order given: P14.T24.a
//! sorts it, largest first with ties by id, so that its sums are output it fixes. Molar heat
//! capacities add over an ideal mixture's moles, so `c_p` = Σ xᵢ `c_p,ᵢ`; averaging γ instead is
//! wrong (equal parts of helium and carbon dioxide give R ÷ `c_p` = 0.2927 by `c_p` and 0.3258 by
//! γ).

use super::thermo::MolecularConstants;
use super::{Source, SubstanceId};

/// A gas's `c_p` ÷ R class: kinetic theory's degrees of freedom, or a stated value (Robinson and
/// Catling 2012, eq. 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HeatCapacityClass {
    /// An atom: three translational degrees, `c_p` ÷ R = 5 ÷ 2, exact for a monatomic ideal gas.
    Atom,
    /// A linear molecule with two rotational degrees and its vibrations frozen, N = 5: `c_p` ÷ R =
    /// 7 ÷ 2 (H₂, N₂, O₂).
    Linear,
    /// A non-linear molecule with three rotational degrees, N = 6: `c_p` ÷ R = 4 (H₂O, CH₄, NH₃).
    NonLinear,
    /// A linear triatomic with about two vibrational degrees excited: γ = 1.3, so `c_p` ÷ R =
    /// γ ÷ (γ − 1) = 13 ÷ 3, Robinson and Catling's for Venus and Mars (§2.3; N ≈ 7 after Bent
    /// 1965, rounded), the γ their Venus α = 0.8 was fitted against (§4.1) (CO₂).
    LinearTriatomic,
}

impl HeatCapacityClass {
    /// The class's `c_p` ÷ R.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::substance::HeatCapacityClass;
    ///
    /// // Air's dry adiabat: R ÷ c_p = 2 ÷ 7.
    /// assert!((1.0 / HeatCapacityClass::Linear.over_r() - 2.0 / 7.0).abs() < 1e-15);
    /// ```
    #[must_use]
    pub const fn over_r(self) -> f64 {
        match self {
            Self::Atom => 1.0 + 3.0 / 2.0,
            Self::Linear => 1.0 + 5.0 / 2.0,
            Self::NonLinear => 1.0 + 6.0 / 2.0,
            Self::LinearTriatomic => 13.0 / 3.0,
        }
    }
}

/// A gas row's columns: its molecular constants and, for rows 0–8 until the 22 → 23 bump, its held
/// heat-capacity class, each value with its sources.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasColumns {
    pub(super) molecular: MolecularConstants,
    pub(super) held_class: Option<HeatCapacityClass>,
    pub(super) sources: &'static [Source],
}

impl GasColumns {
    /// The gas's molecular constants, over which its `c_p`(T), H̃ and S̃ are summed
    /// ([`thermo`](super::thermo)).
    #[must_use]
    pub const fn molecular(&self) -> &MolecularConstants {
        &self.molecular
    }

    /// The `c_p` ÷ R class generator version 22 publishes, for rows 0–8: held until the 22 → 23
    /// bump, and `None` for every later row, which never had one.
    #[must_use]
    pub const fn heat_capacity(&self) -> Option<HeatCapacityClass> {
        self.held_class
    }

    /// The held class's `c_p` ÷ R ([`HeatCapacityClass::over_r`]), for rows 0–8.
    #[must_use]
    pub const fn heat_capacity_over_r(&self) -> Option<f64> {
        match self.held_class {
            Some(class) => Some(class.over_r()),
            None => None,
        }
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

/// The mixture's held `c_p` ÷ R, Σ xᵢ (`c_p,ᵢ` ÷ R) over each row's class, mixed by molar heat
/// capacity and summed in its order; its reciprocal is the dry adiabat's exponent R ÷ `c_p`.
///
/// It is generator version 22's published value, held until the 22 → 23 bump (the
/// [module documentation](self)); [`mixture_heat_capacity_over_r_at`](super::mixture_heat_capacity_over_r_at)
/// is in force. The fractions are the mixture's mole fractions, summing to 1, and are not
/// renormalised. `None` for an empty mixture, or one with a row that has no held class: every row
/// after row 8.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::heat_capacity_over_r;
///
/// # fn main() {
/// #     assert!(example().is_some(), "every value the example reads is there");
/// # }
/// # fn example() -> Option<()> {
/// // Venus's carbon dioxide: R ÷ c_p = 3 ÷ 13.
/// let venus = [(Gas::CarbonDioxide.substance(), 1.0)];
/// assert!((1.0 / heat_capacity_over_r(&venus)? - 3.0 / 13.0).abs() < 1e-15);
/// # Some(())
/// # }
/// ```
#[must_use]
pub fn heat_capacity_over_r(mixture: &[(SubstanceId, f64)]) -> Option<f64> {
    mixed(mixture, |id| {
        id.substance()
            .gas()
            .and_then(GasColumns::heat_capacity_over_r)
    })
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

    /// The R ÷ `c_p` of plan 14's T24.e: 2/7 for H₂, N₂ and O₂, 0.400 for He and Ar, 3/13 for CO₂
    /// and 0.25 for H₂O, CH₄ and NH₃.
    #[test]
    fn substance_mixture_r_over_cp_is_each_pure_gas_s() {
        for (gas, expected) in [
            (Gas::Hydrogen, 2.0 / 7.0),
            (Gas::Helium, 0.4),
            (Gas::Water, 0.25),
            (Gas::Methane, 0.25),
            (Gas::Ammonia, 0.25),
            (Gas::Nitrogen, 2.0 / 7.0),
            (Gas::Oxygen, 2.0 / 7.0),
            (Gas::CarbonDioxide, 3.0 / 13.0),
            (Gas::Argon, 0.4),
        ] {
            let r_over_cp = 1.0 / heat_capacity_over_r(&pure(gas)).unwrap();
            assert!((r_over_cp - expected).abs() < 1e-15, "{gas:?}: {r_over_cp}");
        }
    }

    /// Each class is 1 + N ÷ 2 for its N degrees and CO₂'s 13 ÷ 3, the expressions the client's
    /// `ofDegrees(N)` and `LINEAR_TRIATOMIC_CP_OVER_R` evaluate, so the two sides can hold the same
    /// bits (R08.T19 compares them at 10⁻¹²).
    #[test]
    fn each_class_is_one_plus_half_its_degrees() {
        for (class, degrees) in [
            (HeatCapacityClass::Atom, 3.0),
            (HeatCapacityClass::Linear, 5.0),
            (HeatCapacityClass::NonLinear, 6.0),
        ] {
            assert_same_bits(class.over_r(), 1.0 + degrees / 2.0);
        }
        assert_same_bits(HeatCapacityClass::LinearTriatomic.over_r(), 13.0 / 3.0);
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
    /// 28.966 g mol⁻¹ (the U.S. Standard Atmosphere 1976's 28.9644 for its composition) and
    /// R ÷ `c_p` 0.26% above 2 ÷ 7, by argon's lower `c_p`; equal parts of helium and carbon
    /// dioxide give 0.2927 (`c_p` is mixed, not γ).
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
        let r_over_cp = 1.0 / heat_capacity_over_r(&air).unwrap();
        assert!((r_over_cp - 0.286_46).abs() < 1e-5, "{r_over_cp}");
        let even = [
            (Gas::Helium.substance(), 0.5),
            (Gas::CarbonDioxide.substance(), 0.5),
        ];
        let r_over_cp = 1.0 / heat_capacity_over_r(&even).unwrap();
        assert!((r_over_cp - 0.2927).abs() < 1e-4, "{r_over_cp}");
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
        assert_eq!(heat_capacity_over_r(&[]), None);
    }
}
