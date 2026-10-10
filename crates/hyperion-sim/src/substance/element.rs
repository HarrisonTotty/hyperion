//! The chemical elements a substance's stoichiometry is written in (decision-composition §1.1).
//!
//! The 25 elements of plan 14's Provides, first listed in order of atomic number. The list is
//! append-only by the registry's rule: an element is never renamed, removed or reordered, and a
//! later element is appended after these whatever its atomic number, so that a loop over
//! [`Element::ALL`] keeps its order as elements are added. The registry's golden pins the order.
//! This subtask (P14.T49.a) gives each its symbol, the grammar a formula key is checked against;
//! P14.T50.a adds each one's standard atomic weight, protosolar abundance and 50% condensation
//! temperature.

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
}
