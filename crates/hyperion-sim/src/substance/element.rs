//! The chemical elements a substance's stoichiometry is written in (decision-composition §1.1).
//!
//! The 25 elements of plan 14's Provides, first listed in order of atomic number. The list is
//! append-only by the registry's rule: an element is never renamed, removed or reordered, and a
//! later element is appended after these whatever its atomic number, so that a loop over
//! [`Element::ALL`] keeps its order as elements are added. The registry's golden pins the order.
//! P14.T49.a gave each its symbol, the grammar a formula key is checked against. P14.T50.a adds
//! each one's data, a row of three cited columns ([`Element::standard_atomic_weight_g_per_mol`],
//! [`Element::protosolar_log_abundance`] and [`Element::condensation_temperature`]):
//!
//! - **The standard atomic weight**, g mol⁻¹: CIAAW's abridged value to five significant figures
//!   (Prohaska et al. 2022, "Standard atomic weights of the elements 2021", Pure Appl. Chem. 94,
//!   573, Table 2; read in CIAAW's abridged table of 2024, which changed none of these 25), the
//!   values the substance registry's molar masses are summed from.
//! - **The protosolar abundance**, A(X)₀ = log₁₀(`N_X` ÷ `N_H`) + 12: Lodders (2010, "Solar System
//!   abundances of the elements", in Principles and Perspectives in Cosmochemistry, Astrophys.
//!   Space Sci. Proc., 379–417, arXiv:1010.2746), Table 6, "Solar system abundances 4.56 Ga ago",
//!   its log N(H) = 12 column (read). Her protosolar values are her recommended present-day ones
//!   (Table 3) plus 0.053 dex for the elements heavier than He and 0.061 for He, the photosphere's
//!   loss to settling since the Sun formed. This is the series of Lodders (2003), whose Table 11
//!   this plan's disc already reads ([`disc`](crate::planetary::disc)). Its solar C/O is 10^−0.34
//!   = 0.457 (her §3, "C/O = 0.457"); Asplund, Amarsi and Grevesse's (2021, A&A 653, A141, §2.3)
//!   photospheric 0.59 ± 0.08 is the recorded alternative, an uncertainty in every C/O that reads
//!   it (plan 14's Risks). The table's N(Si) = 10⁶ column gives 0.458, the same figures rounded
//!   otherwise.
//! - **The 50% condensation temperature**, K, at 10⁻⁴ bar total pressure in a gas of solar
//!   composition: Lodders (2003, ApJ 591, 1220), Table 8. Her paper was not read (its publisher
//!   refuses automated access); each value is checked against a reading that quotes it, which
//!   [`Element::condensation_temperature`] names element by element.

use crate::units::{Dex, Kelvin};

/// One element's row of data: its standard atomic weight, g mol⁻¹, its protosolar A(X)₀ and its
/// 50% condensation temperature, K, with the sources the module's documentation names.
struct ElementData {
    weight_g_per_mol: f64,
    protosolar: f64,
    condensation_k: f64,
}

/// A chemical element, by its symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Element {
    /// Hydrogen, Z = 1.
    H,
    /// Helium, Z = 2.
    He,
    /// Carbon, Z = 6.
    C,
    /// Nitrogen, Z = 7.
    N,
    /// Oxygen, Z = 8.
    O,
    /// Neon, Z = 10.
    Ne,
    /// Sodium, Z = 11.
    Na,
    /// Magnesium, Z = 12.
    Mg,
    /// Aluminium, Z = 13.
    Al,
    /// Silicon, Z = 14.
    Si,
    /// Phosphorus, Z = 15.
    P,
    /// Sulphur, Z = 16.
    S,
    /// Chlorine, Z = 17.
    Cl,
    /// Argon, Z = 18.
    Ar,
    /// Potassium, Z = 19.
    K,
    /// Calcium, Z = 20.
    Ca,
    /// Titanium, Z = 22.
    Ti,
    /// Vanadium, Z = 23.
    V,
    /// Chromium, Z = 24.
    Cr,
    /// Manganese, Z = 25.
    Mn,
    /// Iron, Z = 26.
    Fe,
    /// Nickel, Z = 28.
    Ni,
    /// Zinc, Z = 30.
    Zn,
    /// Krypton, Z = 36.
    Kr,
    /// Xenon, Z = 54.
    Xe,
}

impl Element {
    /// Every element, in the registry's order: as declared, which for these first 25 is by atomic
    /// number.
    pub const ALL: [Self; 25] = [
        Self::H,
        Self::He,
        Self::C,
        Self::N,
        Self::O,
        Self::Ne,
        Self::Na,
        Self::Mg,
        Self::Al,
        Self::Si,
        Self::P,
        Self::S,
        Self::Cl,
        Self::Ar,
        Self::K,
        Self::Ca,
        Self::Ti,
        Self::V,
        Self::Cr,
        Self::Mn,
        Self::Fe,
        Self::Ni,
        Self::Zn,
        Self::Kr,
        Self::Xe,
    ];

    /// The element's symbol, as a formula key writes it: an uppercase letter and at most one
    /// lowercase letter.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::H => "H",
            Self::He => "He",
            Self::C => "C",
            Self::N => "N",
            Self::O => "O",
            Self::Ne => "Ne",
            Self::Na => "Na",
            Self::Mg => "Mg",
            Self::Al => "Al",
            Self::Si => "Si",
            Self::P => "P",
            Self::S => "S",
            Self::Cl => "Cl",
            Self::Ar => "Ar",
            Self::K => "K",
            Self::Ca => "Ca",
            Self::Ti => "Ti",
            Self::V => "V",
            Self::Cr => "Cr",
            Self::Mn => "Mn",
            Self::Fe => "Fe",
            Self::Ni => "Ni",
            Self::Zn => "Zn",
            Self::Kr => "Kr",
            Self::Xe => "Xe",
        }
    }

    /// The element whose symbol is `symbol`, or `None` for a symbol of none of [`Element::ALL`].
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::substance::Element;
    ///
    /// assert_eq!(Element::from_symbol("Fe"), Some(Element::Fe));
    /// // Case is chemistry: "Co" is cobalt, not carbon monoxide, and is not yet an element here.
    /// assert_eq!(Element::from_symbol("Co"), None);
    /// ```
    #[must_use]
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.symbol() == symbol)
    }

    /// The element's standard atomic weight, g mol⁻¹: CIAAW's abridged value (Prohaska et al. 2022).
    ///
    /// It is their Table 2's, the mean over terrestrial isotopic compositions. An element whose
    /// weight varies in nature has an interval there (H, C, N, O, Mg, Si, S, Cl, Ar), and the
    /// abridged value is its conventional one: argon's 39.95 is ±0.16 because air's argon is
    /// radiogenic ⁴⁰Ar.
    #[must_use]
    pub const fn standard_atomic_weight_g_per_mol(self) -> f64 {
        self.data().weight_g_per_mol
    }

    /// The element's protosolar abundance, A(X)₀ = log₁₀(`N_X` ÷ `N_H`) + 12 (Lodders 2010).
    ///
    /// It is her Table 6's (read): the solar system's 4.56 Ga ago, which a host of \[Fe/H\] = 0 is
    /// taken to share.
    ///
    /// # Examples
    ///
    /// The protosolar carbon-to-oxygen ratio by number, the one this plan's chemistry starts
    /// from:
    ///
    /// ```
    /// use hyperion_sim::math;
    /// use hyperion_sim::substance::Element;
    ///
    /// let c = Element::C.protosolar_log_abundance().value();
    /// let o = Element::O.protosolar_log_abundance().value();
    /// assert!((math::exp10(c - o) - 0.457).abs() < 5e-4);
    /// ```
    #[must_use]
    pub const fn protosolar_log_abundance(self) -> Dex {
        Dex::new(self.data().protosolar)
    }

    /// The element's protosolar number abundance relative to hydrogen, `N_X` ÷ `N_H` =
    /// 10^(A(X)₀ − 12) ([`protosolar_log_abundance`](Self::protosolar_log_abundance)).
    #[must_use]
    pub fn protosolar_abundance_per_hydrogen(self) -> f64 {
        crate::math::exp10(self.data().protosolar - 12.0)
    }

    /// The temperature at which half of the element has condensed from a cooling gas of solar
    /// composition at 10⁻⁴ bar total pressure (Lodders 2003, ApJ 591, 1220, Table 8).
    ///
    /// It measures volatility, from the refractory Al (1,653 K) to the ultra-volatile He (below
    /// 3 K).
    ///
    /// Lodders (2003) was not read. Each value is checked against a reading that quotes her
    /// Table 8:
    ///
    /// - **Lodders, Palme and Gail** (2009, Landolt-Börnstein VI/4B, 560, arXiv:0901.1149,
    ///   Table 1, "taken from [03L]"; read) agree for H 182, He (below 3; 3 is taken), C 40,
    ///   N 123, O 180, Ne 9.1, Na 958, Al 1,653, S 664, Ti 1,582, Fe 1,334, Ni 1,353, Zn 726,
    ///   Kr 52 and Xe 68. Lodders (2010, read) gives S's 664 too.
    /// - **Si**, 1,310: Wang, Lineweaver and Ireland (2019, Icarus 328, 287, arXiv:1810.12741,
    ///   §4.1, read), quoting Lodders (2003). LPG09's table gives 1,354.
    /// - **Mg**, 1,336, and **Ca**, 1,517, and **V**, 1,429: from memory of Lodders (2003). Wang et
    ///   al. give Mg 1,330; LPG09's table gives Mg 1,354, Ca 1,659 and V 1,452, but its block from
    ///   Ca to V is out of the table's descending order, its values one row below the order of
    ///   Lodders (2003): a transcription error, as the lane reads it (plan 14's Risks).
    /// - **P** 1,229, **Cl** 948, **Ar** 47, **K** 1,006, **Cr** 1,296 and **Mn** 1,158: from
    ///   memory of Lodders (2003), not in LPG09's table and not checked (plan 14's Risks).
    #[must_use]
    pub const fn condensation_temperature(self) -> Kelvin {
        Kelvin::new(self.data().condensation_k)
    }

    /// The element's row, in the module documentation's three columns.
    #[must_use]
    #[expect(
        clippy::approx_constant,
        reason = "nickel's protosolar 6.28 is Lodders's figure, not τ"
    )]
    const fn data(self) -> ElementData {
        let (weight_g_per_mol, protosolar, condensation_k) = match self {
            Self::H => (1.0080, 12.00, 182.0),
            Self::He => (4.0026, 10.986, 3.0),
            Self::C => (12.011, 8.44, 40.0),
            Self::N => (14.007, 7.91, 123.0),
            Self::O => (15.999, 8.78, 180.0),
            Self::Ne => (20.180, 8.10, 9.1),
            Self::Na => (22.990, 6.35, 958.0),
            Self::Mg => (24.305, 7.60, 1_336.0),
            Self::Al => (26.982, 6.51, 1_653.0),
            Self::Si => (28.085, 7.59, 1_310.0),
            Self::P => (30.974, 5.51, 1_229.0),
            Self::S => (32.06, 7.21, 664.0),
            Self::Cl => (35.45, 5.30, 948.0),
            Self::Ar => (39.95, 6.55, 47.0),
            Self::K => (39.098, 5.16, 1_006.0),
            Self::Ca => (40.078, 6.37, 1_517.0),
            Self::Ti => (47.867, 4.98, 1_582.0),
            Self::V => (50.942, 4.04, 1_429.0),
            Self::Cr => (51.996, 5.70, 1_296.0),
            Self::Mn => (54.938, 5.55, 1_158.0),
            Self::Fe => (55.845, 7.51, 1_334.0),
            Self::Ni => (58.693, 6.28, 1_353.0),
            Self::Zn => (65.38, 4.70, 726.0),
            Self::Kr => (83.798, 3.33, 52.0),
            Self::Xe => (131.29, 2.32, 68.0),
        };
        ElementData {
            weight_g_per_mol,
            protosolar,
            condensation_k,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_symbol_names_its_own_element_once() {
        for (i, e) in Element::ALL.into_iter().enumerate() {
            assert_eq!(Element::from_symbol(e.symbol()), Some(e));
            let bytes = e.symbol().as_bytes();
            assert!(
                (1..=2).contains(&bytes.len())
                    && bytes[0].is_ascii_uppercase()
                    && bytes[1..].iter().all(u8::is_ascii_lowercase),
                "{e:?}'s symbol is an uppercase letter and at most one lowercase letter"
            );
            assert!(
                Element::ALL[..i]
                    .iter()
                    .all(|other| other.symbol() != e.symbol()),
                "{e:?}'s symbol is unique"
            );
        }
    }

    #[test]
    fn the_elements_are_in_their_declared_order() {
        let mut sorted = Element::ALL;
        sorted.sort();
        assert_eq!(sorted, Element::ALL);
    }

    /// P14.T50.a: the nine gas rows' molar masses are their elements' weights summed, to the
    /// rounding of their stated figures (helium's row rounds 4.0026 to 4.003), so that an element
    /// inventory's shares of a carrier sum back to it.
    #[test]
    fn the_gas_rows_molar_masses_are_their_elements_weights_summed() {
        for gas in crate::planetary::derive::atmosphere::Gas::ALL {
            let row = gas.substance().substance();
            let summed = row.elements().iter().fold(0.0, |sum, &(e, n)| {
                sum + f64::from(n) * e.standard_atomic_weight_g_per_mol()
            });
            let molar = gas.molar_mass_g_per_mol();
            assert!(
                (molar - summed).abs() < 5e-4,
                "{gas:?}: {molar} against {summed}"
            );
        }
    }

    /// P14.T50.a: Lodders's (2010) protosolar figures as built: hydrogen's 12, her C/O of 0.457
    /// (§3) and Mg/Si of 1.02, and helium's 10.986.
    #[test]
    fn the_protosolar_abundances_are_lodders_s() {
        hyperion_testkit::float::assert_same_bits(
            Element::H.protosolar_log_abundance().value(),
            12.0,
        );
        assert!((Element::H.protosolar_abundance_per_hydrogen() - 1.0).abs() < 1e-15);
        let ratio = |a: Element, b: Element| {
            Element::protosolar_abundance_per_hydrogen(a)
                / Element::protosolar_abundance_per_hydrogen(b)
        };
        assert!((ratio(Element::C, Element::O) - 0.457).abs() < 5e-4);
        assert!((ratio(Element::Mg, Element::Si) - 1.023).abs() < 1e-3);
        // Table 7's protosolar He/H, 10^(10.986 − 12) = 0.0968.
        assert!((Element::He.protosolar_abundance_per_hydrogen() - 0.0968).abs() < 1e-4);
        // Every element is rarer than hydrogen, and helium is the most common of the rest.
        for e in &Element::ALL[1..] {
            let a = e.protosolar_log_abundance().value();
            assert!(a < 12.0, "{e:?}");
            assert!(a <= Element::He.protosolar_log_abundance().value(), "{e:?}");
        }
    }

    /// P14.T50.a: the 50% condensation temperatures span the refractory to the ultra-volatile,
    /// in the order of Lodders's (2003) classes: aluminium the highest of the 25, the major
    /// silicate and metal elements between 1,300 and 1,400 K, sulphur's troilite at 664 K, water's
    /// oxygen at 180 K and the noble gases' clathrates and ices below it.
    #[test]
    fn the_condensation_temperatures_follow_volatility() {
        let t = |e: Element| e.condensation_temperature().value();
        for e in Element::ALL {
            assert!(t(e) > 0.0 && t(e) <= t(Element::Al), "{e:?}");
        }
        for e in [Element::Mg, Element::Si, Element::Fe, Element::Ni] {
            assert!((1_300.0..1_400.0).contains(&t(e)), "{e:?}");
        }
        assert!(t(Element::S) < t(Element::Fe) && t(Element::O) < t(Element::S));
        for e in [
            Element::Xe,
            Element::Kr,
            Element::Ar,
            Element::Ne,
            Element::He,
        ] {
            assert!(t(e) < t(Element::O), "{e:?}");
        }
        assert!(t(Element::He) < t(Element::Ne));
    }
}
