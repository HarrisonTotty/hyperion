//! Each gas row's Rayleigh cross section per molecule at 550 nm (plan 14, P14.T24.h.a;
//! decision-p14-t54a-passes §G3.5), which the snapshot path's Rayleigh floor reads
//! (`planetary::derive::photometry::area_mix`).
//!
//! # The values
//!
//! A row's cross section is the client's for the same gas at 550 nm in vacuum, from the same
//! sources: rendering plan R08's `rayleigh.ts` (R08.T3.b), whose `rayleighCrossSectionM2` is
//!
//! σ(λ) = 24π³ ÷ (λ⁴ `N_ref`²) × ((n² − 1) ÷ (n² + 2))² × `F_K`(λ),
//!
//! with n the gas's measured index at its formula's reference state, `N_ref` = `p_ref` ÷
//! (Z `k_B` `T_ref`) that state's real-gas number density and `F_K` its King factor. Each value
//! below is that expression at 550 nm (*reduced*: [`Basis::Reduced`]), and each row names the
//! dispersion formula and the King factor it was reduced from, as `rayleigh.ts` cites them with
//! the corrections recorded in plan R08's Risks. The client's parity check (R08.T19) holds the
//! registry's mirror to its own `rayleighCrossSectionM2` at 550 nm.
//!
//! The sixteen rows are the client's measured gases. A gas row whose dispersion the client only
//! estimates (`ESTIMATED_RAYLEIGH`: SO₂, HCN, O₃ among them) holds none: its molecules scatter
//! nothing here, so a sum over an air's gases is a lower bound on its scattering, which is what
//! the floor needs. Checks: N₂ 4.631 × 10⁻³¹ m² and CH₄ 2.14 times it. Earth's dry-air mix gives
//! σ = 4.507 × 10⁻³¹ m² against Bodhaine et al. 1999's 4.5105 × 10⁻³¹ (J. Atmos. Oceanic Technol.
//! 16, 1854, Table 3, 0.550 µm), so N₂ is 1.027 times air and CH₄ 2.20 times. CH₄ scaled to 532 nm
//! gives 11.37 × 10⁻²⁷ cm², against He et al. 2021's 11.34 (n-based) and 11.3 (measured) (Atmos.
//! Chem. Phys. 21, 14927, Table 2). Earth's τ(550), at its mole fractions and 101,325 Pa under
//! 9.806 65 m s⁻², is 0.0968 against Bodhaine's 0.0971: the 0.3% is mostly their column-weighted g
//! (their eqs. 25–26). CO₂ scaled to 532 nm is 1.4% below He et al.'s 13.32 (recorded for plan R08;
//! low, the floor's safe side).

use crate::planetary::derive::atmosphere::Gas;
use crate::substance::{Basis, SubstanceId, by_key};
use crate::units::SquareMetres;

/// The wavelength at which a row's cross section is given: 550 nm, in vacuum, the photometry's V
/// reference and Bodhaine et al. 1999's tabled τ(550).
pub const RAYLEIGH_REFERENCE_WAVELENGTH_NM: f64 = 550.0;

/// A gas row's Rayleigh cross section per molecule at [`RAYLEIGH_REFERENCE_WAVELENGTH_NM`], with
/// what it was reduced from ([the module](self)'s expression).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayleighCrossSection {
    key: &'static str,
    cross_section: SquareMetres,
    dispersion: &'static str,
    king_factor: &'static str,
}

impl RayleighCrossSection {
    /// The row's key.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        self.key
    }

    /// The cross section of one molecule at 550 nm, m², finite and positive.
    #[must_use]
    pub const fn cross_section(&self) -> SquareMetres {
        self.cross_section
    }

    /// The dispersion formula and its reference state, as `rayleigh.ts` cites them.
    #[must_use]
    pub const fn dispersion(&self) -> &'static str {
        self.dispersion
    }

    /// The King factor, as `rayleigh.ts` cites it.
    #[must_use]
    pub const fn king_factor(&self) -> &'static str {
        self.king_factor
    }

    /// What its source lets the registry hold: reduced from the cited formulas by the module's
    /// expression.
    #[must_use]
    pub const fn basis(&self) -> Basis {
        Basis::Reduced
    }
}

/// One row of [`CROSS_SECTIONS`].
const fn row(
    key: &'static str,
    square_metres: f64,
    dispersion: &'static str,
    king_factor: &'static str,
) -> RayleighCrossSection {
    RayleighCrossSection {
        key,
        cross_section: SquareMetres::new(square_metres),
        dispersion,
        king_factor,
    }
}

/// Isotropic: an atom, or a spherical top (Sneep and Ubachs 2005, §5.2), `F_K` = 1.
const ISOTROPIC: &str = "isotropic, F_K = 1";

/// The noble gases' dispersion (Börzsönyi et al. 2008, their Table 2, Sellmeier forms at 0 °C and
/// 1,000 mbar).
const BORZSONYI_2008: &str = "Börzsönyi, Heiner, Kalashnikov, Kovács and Osvay, Appl. Opt. 47 \
                              (2008) 4856, a Sellmeier form at 0 °C and 1,000 mbar";

/// The sixteen measured gases' cross sections at 550 nm, in registry order.
const CROSS_SECTIONS: [RayleighCrossSection; 16] = [
    row(
        "H2",
        1.003_435_898_307_510_5e-31,
        "Peck and Huang, J. Opt. Soc. Am. 67 (1977) 1550, at 0 °C and 760 torr, Z = 1.000 624",
        "1.0312 + 3.09 × 10⁻⁴ ÷ λ² (λ in µm), 1 + (2/9)(γ/ᾱ)² from Raj, Hamaguchi and Witek, J. \
         Chem. Phys. 148 (2018) 104308",
    ),
    row(
        "He",
        6.120_922_674_357_475e-33,
        "Mansfield and Peck, J. Opt. Soc. Am. 59 (1969) 199, at 0 °C and 760 torr, Z = 1.000 532",
        ISOTROPIC,
    ),
    row(
        "H2O",
        3.209_598_433_539_573e-31,
        "Ciddor, Appl. Opt. 35 (1996) 1566, eq. 3, pure vapour at 20 °C and 1,333 Pa, Z = 0.999 237",
        "1.001, from Murphy, J. Chem. Phys. 67 (1977) 5877, ρ = (3.0 ± 1.4) × 10⁻⁴",
    ),
    row(
        "CH4",
        9.919_293_831_212_23e-31,
        "He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 10, at \
         288.15 K and 1,013.25 hPa, Z = 1 (derived through the ideal N)",
        ISOTROPIC,
    ),
    row(
        "NH3",
        7.163_015_088_962_506e-31,
        "C. and M. Cuthbertson, Phil. Trans. R. Soc. Lond. A 213 (1914) 1, at 0 °C and 760 mm, \
         Z = 0.984 798 × 0.7708 ÷ 0.7605",
        "(3 + 6ρₚ) ÷ (3 − 4ρₚ) at ρₚ = 0.000 91 (R. I. Keir, PhD thesis, University of New England \
         1995)",
    ),
    row(
        "N2",
        4.630_518_767_775_845_5e-31,
        "Peck and Khanna, J. Opt. Soc. Am. 56 (1966) 1059, the 15 °C form, at 101,325 Pa, \
         Z = 0.999 715",
        "1.034 + 3.17 × 10⁻⁴ ÷ λ² (λ in µm), Bates 1984 (Bodhaine et al. 1999, eq. 5)",
    ),
    row(
        "O2",
        4.056_450_535_757_989e-31,
        "Křen, Appl. Opt. 50 (2011) 6484, refitting Zhang, Lu and Wang, Appl. Opt. 47 (2008) 3143, \
         at 20 °C and 101,325 Pa, Z = 0.999 282",
        "1.096 + 1.385 × 10⁻³ ÷ λ² + 1.448 × 10⁻⁴ ÷ λ⁴ (λ in µm), Bates 1984 (Bodhaine et al. \
         1999, eq. 6)",
    ),
    row(
        "CO2",
        1.145_860_661_487_764_6e-30,
        "Bideau-Mehu, Guern, Abjean and Johannin-Gilles, Opt. Commun. 9 (1973) 432, as Sneep and \
         Ubachs 2005, eq. 13, correct it, at 0 °C and 760 torr, Z = 0.993 265",
        "1.1364 + 25.3 × 10⁻¹² ν² (ν in cm⁻¹), Sneep and Ubachs 2005, eq. 14 (Alms et al. 1975)",
    ),
    row(
        "Ar",
        3.984_965_183_460_277e-31,
        "Peck and Fisher, J. Opt. Soc. Am. 54 (1964) 1362, the 15 °C form, at 760 torr, \
         Z = 0.999 260",
        ISOTROPIC,
    ),
    row("Ne", 2.265_158_521_173_269_4e-32, BORZSONYI_2008, ISOTROPIC),
    row("Kr", 9.230_384_861_448_749e-31, BORZSONYI_2008, ISOTROPIC),
    row("Xe", 2.466_688_063_729_219_7e-30, BORZSONYI_2008, ISOTROPIC),
    row(
        "N2O",
        1.570_048_974_556_929_5e-30,
        "He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927, eq. 9, at \
         288.15 K and 1,013.25 hPa, Z = 1",
        "(3 + 6ρₚ) ÷ (3 − 4ρₚ) at ρₚ = 0.0577 + 11.8 × 10⁻¹² ν² (ν in cm⁻¹), Sneep and Ubachs 2005, \
         eq. 19 (Alms et al. 1975)",
    ),
    row(
        "CO",
        5.751_768_990_292_543e-31,
        "C. and M. Cuthbertson, Proc. R. Soc. Lond. A 97 (1920) 152, Table II, at 0 °C and 760 \
         mm, Z = 0.999 54",
        "1.017 (Bogaard, Buckingham, Pierens and White 1978; Couling and Graham 1994)",
    ),
    row(
        "H2S",
        2.076_726_934_862_633_2e-30,
        "C. and M. Cuthbertson, Proc. R. Soc. Lond. A 83 (1910) 171, at 0 °C and 760 mm, \
         Z = 0.991 571 ÷ 0.990 92",
        "1 + (2/9)(0.6735 ÷ 25.5)² (Bogaard, Buckingham and Ritchie 1982; Monan, Bribes and \
         Gaufrès 1982)",
    ),
    row(
        "C2H6",
        2.866_757_628_646_656_2e-30,
        "Loria, Ann. Phys. 334 (1909) 605, Table IX, with refractiveindex.info's refit of a, at \
         0 °C and 760 mm, Z = 0.998 97",
        "(3 + 6ρₚ) ÷ (3 − 4ρₚ) at ρₚ = 0.001 59 (R. I. Keir, PhD thesis, University of New England \
         1995, Table 6.3)",
    ),
];

/// The Rayleigh cross section of the row `id` at 550 nm, `None` for a row the client does not
/// measure (an estimated or unscattering gas, or no gas at all).
///
/// # Examples
///
/// A methane molecule scatters a little more than twice what a nitrogen molecule does, which is
/// why a deep methane air is bright under its own Rayleigh layer:
///
/// ```
/// use hyperion_sim::substance::by_key;
/// use hyperion_sim::substance::rayleigh::rayleigh_cross_section;
///
/// let of = |key: &str| {
///     by_key(key)
///         .and_then(rayleigh_cross_section)
///         .map(|row| row.cross_section().value())
/// };
/// let ratio = of("CH4").zip(of("N2")).map(|(ch4, n2)| ch4 / n2);
/// assert!(ratio.is_some_and(|r| (2.1..2.2).contains(&r)), "{ratio:?}");
/// assert_eq!(of("basalt"), None);
/// ```
#[must_use]
pub fn rayleigh_cross_section(id: SubstanceId) -> Option<RayleighCrossSection> {
    let key = id.substance().key();
    CROSS_SECTIONS
        .iter()
        .find(|row| row.key == key.as_str())
        .copied()
}

/// The cross section at 550 nm of one of the passes' nine gases, which the snapshot path's floor
/// reads per pass: registry rows 0–8 are [`Gas::ALL`] in its order, and lead `CROSS_SECTIONS` in
/// the same order (`the_passes_gases_lead_the_table`), so no key is searched.
#[must_use]
pub fn gas_cross_section(gas: Gas) -> SquareMetres {
    CROSS_SECTIONS[usize::from(gas.substance().index())].cross_section
}

/// Every row's cross section, in registry order.
///
/// # Panics
///
/// Never: every cross section's key is a registry row (its test).
pub fn cross_sections() -> impl ExactSizeIterator<Item = (SubstanceId, RayleighCrossSection)> {
    CROSS_SECTIONS.iter().map(|row| {
        (
            by_key(row.key).expect("every cross section's key is a registry row (its test)"),
            *row,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::substance::ids;

    #[test]
    fn every_row_is_a_rayleigh_gas_row_in_registry_order() {
        let mut last = None;
        for row in &CROSS_SECTIONS {
            let id = by_key(row.key).unwrap_or_else(|| panic!("{} is a row", row.key));
            assert!(id.substance().optics().rayleigh(), "{}", row.key);
            assert!(
                id.substance().gas().is_some(),
                "{} has gas columns",
                row.key
            );
            let sigma = row.cross_section.value();
            assert!(sigma.is_finite() && sigma > 0.0, "{}: {sigma}", row.key);
            assert!(last < Some(id), "{} is out of registry order", row.key);
            last = Some(id);
            assert!(
                !row.dispersion.is_empty() && !row.king_factor.is_empty(),
                "{} names its sources",
                row.key
            );
        }
        assert_eq!(cross_sections().len(), CROSS_SECTIONS.len());
    }

    #[test]
    fn the_passes_gases_lead_the_table() {
        for gas in Gas::ALL {
            let row = CROSS_SECTIONS[usize::from(gas.substance().index())];
            assert_eq!(
                row.key,
                gas.substance().substance().key().as_str(),
                "{gas:?}"
            );
            assert_eq!(gas_cross_section(gas), row.cross_section, "{gas:?}");
        }
    }

    #[test]
    fn every_gas_of_the_passes_has_one() {
        for gas in Gas::ALL {
            assert!(
                rayleigh_cross_section(gas.substance()).is_some(),
                "{gas:?} scatters"
            );
        }
        let none = ids()
            .filter(|&id| rayleigh_cross_section(id).is_none())
            .count();
        assert_eq!(none, ids().len() - CROSS_SECTIONS.len());
    }

    /// Bodhaine et al. 1999's dry air at 550 nm (their Table 3, 360 ppm of CO₂, at 1,013.25 hPa and
    /// 45° N's 9.806 m s⁻²): τ = 0.0971. The column of each gas is its mole fraction times p ÷ (g
    /// m̄), at the registry's molar masses.
    #[test]
    fn earth_s_dry_air_is_bodhaine_s() {
        use crate::planetary::derive::atmosphere::ATOMIC_MASS_CONSTANT_KG;
        let air = [
            ("N2", 0.780_84),
            ("O2", 0.209_46),
            ("Ar", 0.009_34),
            ("CO2", 0.000_36),
        ];
        let total: f64 = air.iter().map(|&(_, x)| x).sum();
        let mean_mass: f64 = air
            .iter()
            .map(|&(key, x)| {
                x * by_key(key)
                    .and_then(|id| id.substance().molar_mass_g_per_mol())
                    .unwrap()
            })
            .sum::<f64>()
            / total
            * ATOMIC_MASS_CONSTANT_KG;
        let sigma: f64 = air
            .iter()
            .map(|&(key, x)| {
                x * by_key(key)
                    .and_then(rayleigh_cross_section)
                    .unwrap()
                    .cross_section()
                    .value()
            })
            .sum::<f64>()
            / total;
        let tau = 101_325.0 / (9.806_65 * mean_mass) * sigma;
        assert!((tau - 0.0971).abs() < 0.002, "{tau}");
    }
}
