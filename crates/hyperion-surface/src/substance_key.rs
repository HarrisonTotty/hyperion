//! The key that names a substance on every wire and payload: plan 14's substance registry's key
//! (decision-composition §1.1; P14.T49.a), defined here because R09's field header, in this
//! crate, carries keys in its palette ([`crate::field::MaterialPalette`]).
//!
//! A substance is its string key wherever it leaves a Rust process (the body records' JSON, the
//! field's binary header, the fixtures and the TypeScript client), so that an older reader shown
//! a newer key can still name it; inside the sim a row is a `SubstanceId(u16)`, which never leaves
//! the process. This crate holds only the key: the registry and its columns are the sim's
//! (`hyperion_sim::substance`), and the field carries each body's values resolved.
//!
//! # The grammar
//!
//! A key is 1 to 16 ASCII bytes ([`SubstanceKey::MAX_BYTES`]), of one of three forms
//! ([`SubstanceKeyForm`]):
//!
//! - **A formula**, for a definite species or compound, written as chemists write it (not in Hill
//!   order) and in chemical case: a sequence of element symbols, each an uppercase letter and an
//!   optional lowercase letter, each with an optional count, then an optional final `+` or `-`
//!   (`H2O`, `CO2`, `NH4SH`, `H2SO4`, `Mg2SiO4`, `KCl`, `Fe`, `S8`, `H-`). A count is 2 or more
//!   with no leading zero, so that a species has one spelling: a count of 1 is never written.
//!   Case keeps what chemistry needs (`CO` and `Co`, `CS` and `Cs`, `HF` and `Hf`).
//! - **The electron**, `e-`.
//! - **A name**, `[a-z][a-z0-9_]*`, for a material with no single formula: a lithology (`basalt`,
//!   `anorthosite`), a solution (`brine_nacl`), an organic (`tholin`) or an assemblage
//!   (`mars_dust`), and polymorphs of one formula whose properties differ (`graphite`).
//!
//! The grammar does not check that a formula's symbols are elements or its counts its
//! stoichiometry: the registry's test does, for its rows (P14.T49.a). A key is never renamed or
//! removed, and one key serves every phase of its substance (`H2O` is the vapour, the ice and the
//! liquid), so a phase is never part of a key.

/// A substance's key: 1 to 16 ASCII bytes in the grammar of the [module documentation](self),
/// held NUL-padded to 16 bytes.
///
/// It is ordered as its string is, byte by byte, since NUL sorts before every byte a key holds; a
/// [`MaterialPalette`](crate::field::MaterialPalette) is sorted by it. On the payload it is its 16
/// padded bytes ([`as_padded`](Self::as_padded), [`from_padded`](Self::from_padded)).
///
/// # Examples
///
/// ```
/// use hyperion_surface::substance_key::{ParseSubstanceKeyError, SubstanceKey, SubstanceKeyForm};
///
/// // The registry's constants are checked when the crate compiles.
/// const WATER: SubstanceKey = SubstanceKey::new_const("H2O");
/// // A key from a wire or a file is checked when it is read.
/// let basalt = SubstanceKey::new("basalt")?;
/// assert_eq!(WATER.as_str(), "H2O");
/// assert_eq!(basalt.form(), SubstanceKeyForm::Name);
/// // A lowercase formula is a name, and a name has no `!`.
/// assert_eq!(
///     SubstanceKey::new("h2o!"),
///     Err(ParseSubstanceKeyError::Malformed { at: 3 })
/// );
/// # Ok::<(), ParseSubstanceKeyError>(())
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubstanceKey([u8; SubstanceKey::MAX_BYTES]);

/// Which of the grammar's three forms a key takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SubstanceKeyForm {
    /// A formula in chemical case, such as `H2O` or `NH4+`.
    Formula,
    /// The electron, `e-`.
    Electron,
    /// A lowercase name, such as `basalt` or `mars_dust`.
    Name,
}

/// Why a string or a padded key is not a [`SubstanceKey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseSubstanceKeyError {
    /// The key is empty.
    Empty,
    /// The key is longer than [`SubstanceKey::MAX_BYTES`].
    TooLong {
        /// Its length in bytes.
        length: usize,
    },
    /// The key breaks the grammar at byte `at`: a byte no form admits there, a count of 0 or 1 or
    /// with a leading zero, or a sign that is not last.
    Malformed {
        /// The offset of the first byte that breaks it.
        at: usize,
    },
    /// A padded key's bytes after its first NUL are not all NUL, so that a key has one padded
    /// form.
    Padding,
}

impl std::fmt::Display for ParseSubstanceKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "a substance key is empty"),
            Self::TooLong { length } => write!(
                f,
                "a substance key of {length} bytes is longer than {}",
                SubstanceKey::MAX_BYTES
            ),
            Self::Malformed { at } => write!(
                f,
                "a substance key breaks its grammar at byte {at}: it is neither a formula in \
                 chemical case, e- nor a lowercase name"
            ),
            Self::Padding => write!(f, "a padded substance key has bytes after its padding"),
        }
    }
}

impl std::error::Error for ParseSubstanceKeyError {}

/// The form of `key`, or why it breaks the grammar.
const fn check(key: &[u8]) -> Result<SubstanceKeyForm, ParseSubstanceKeyError> {
    if key.is_empty() {
        return Err(ParseSubstanceKeyError::Empty);
    }
    if key.len() > SubstanceKey::MAX_BYTES {
        return Err(ParseSubstanceKeyError::TooLong { length: key.len() });
    }
    let first = key[0];
    if first.is_ascii_lowercase() {
        if key.len() == 2 && first == b'e' && key[1] == b'-' {
            return Ok(SubstanceKeyForm::Electron);
        }
        let mut i = 1;
        while i < key.len() {
            let b = key[i];
            if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
                return Err(ParseSubstanceKeyError::Malformed { at: i });
            }
            i += 1;
        }
        return Ok(SubstanceKeyForm::Name);
    }
    if !first.is_ascii_uppercase() {
        return Err(ParseSubstanceKeyError::Malformed { at: 0 });
    }
    // A formula: symbols with their counts, then at most one sign, last.
    let mut i = 0;
    while i < key.len() {
        let b = key[i];
        if b == b'+' || b == b'-' {
            // A sign follows a symbol or a count (`i > 0`, the first byte being a letter) and ends
            // the key.
            if i + 1 != key.len() {
                return Err(ParseSubstanceKeyError::Malformed { at: i });
            }
            return Ok(SubstanceKeyForm::Formula);
        }
        if !b.is_ascii_uppercase() {
            return Err(ParseSubstanceKeyError::Malformed { at: i });
        }
        i += 1;
        if i < key.len() && key[i].is_ascii_lowercase() {
            i += 1;
        }
        if i < key.len() && key[i].is_ascii_digit() {
            let start = i;
            while i < key.len() && key[i].is_ascii_digit() {
                i += 1;
            }
            // No leading zero, and no count of 1, which chemists never write.
            if key[start] == b'0' || (i == start + 1 && key[start] == b'1') {
                return Err(ParseSubstanceKeyError::Malformed { at: start });
            }
        }
    }
    Ok(SubstanceKeyForm::Formula)
}

impl SubstanceKey {
    /// The most bytes a key holds: 16 (decision-composition §1.1).
    pub const MAX_BYTES: usize = 16;

    /// The key `key`, checked against the grammar.
    ///
    /// # Errors
    ///
    /// [`ParseSubstanceKeyError::Empty`], [`ParseSubstanceKeyError::TooLong`] past 16 bytes, or
    /// [`ParseSubstanceKeyError::Malformed`] at the first byte that breaks the grammar.
    pub const fn new(key: &str) -> Result<Self, ParseSubstanceKeyError> {
        let bytes = key.as_bytes();
        match check(bytes) {
            Ok(_) => {
                let mut padded = [0; Self::MAX_BYTES];
                let mut i = 0;
                while i < bytes.len() {
                    padded[i] = bytes[i];
                    i += 1;
                }
                Ok(Self(padded))
            }
            Err(e) => Err(e),
        }
    }

    /// The key `key`, for a constant: in a `const` item, as the registry's rows are, a key that
    /// breaks the grammar fails to compile.
    ///
    /// # Panics
    ///
    /// If `key` breaks the grammar, which in a `const` item is a compile error, so call it in one.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_surface::substance_key::SubstanceKey;
    ///
    /// const SULPHURIC_ACID: SubstanceKey = SubstanceKey::new_const("H2SO4");
    /// assert_eq!(SULPHURIC_ACID.as_str(), "H2SO4");
    /// ```
    ///
    /// ```compile_fail,E0080
    /// // Lowercase formulas are names, and a name has no `!`.
    /// const _: hyperion_surface::substance_key::SubstanceKey =
    ///     hyperion_surface::substance_key::SubstanceKey::new_const("h2o!");
    /// ```
    ///
    /// ```compile_fail,E0080
    /// // A count of 1 is never written.
    /// const _: hyperion_surface::substance_key::SubstanceKey =
    ///     hyperion_surface::substance_key::SubstanceKey::new_const("C1O2");
    /// ```
    ///
    /// ```compile_fail,E0080
    /// // Seventeen bytes.
    /// const _: hyperion_surface::substance_key::SubstanceKey =
    ///     hyperion_surface::substance_key::SubstanceKey::new_const("ice_giant_hazes_x");
    /// ```
    #[must_use]
    pub const fn new_const(key: &str) -> Self {
        match Self::new(key) {
            Ok(k) => k,
            Err(_) => panic!(
                "a substance key is 1 to 16 ASCII bytes: a formula in chemical case, e- or a \
                 lowercase name"
            ),
        }
    }

    /// The key whose NUL-padded bytes are `padded`, as the payload carries it: the key's bytes,
    /// then NUL to 16.
    ///
    /// # Errors
    ///
    /// [`ParseSubstanceKeyError::Padding`] if a byte after the first NUL is not NUL, and
    /// otherwise [`new`](Self::new)'s errors for the bytes before it ([`ParseSubstanceKeyError::Empty`]
    /// for sixteen NULs).
    pub const fn from_padded(
        padded: [u8; Self::MAX_BYTES],
    ) -> Result<Self, ParseSubstanceKeyError> {
        let length = padded_length(&padded);
        let mut i = length;
        while i < Self::MAX_BYTES {
            if padded[i] != 0 {
                return Err(ParseSubstanceKeyError::Padding);
            }
            i += 1;
        }
        let (key, _) = padded.split_at(length);
        match check(key) {
            Ok(_) => Ok(Self(padded)),
            Err(e) => Err(e),
        }
    }

    /// The key's bytes, NUL-padded to 16: its wire form.
    #[must_use]
    pub const fn as_padded(&self) -> &[u8; Self::MAX_BYTES] {
        &self.0
    }

    /// The key, such as `"H2O"`.
    ///
    /// # Panics
    ///
    /// Never: a key's bytes before its padding are ASCII, checked when it was made.
    #[must_use]
    pub fn as_str(&self) -> &str {
        let (key, _) = self.0.split_at(padded_length(&self.0));
        core::str::from_utf8(key).expect("a substance key is ASCII, checked when it was made")
    }

    /// The key's form: a formula, the electron or a name.
    ///
    /// # Panics
    ///
    /// Never: a key was checked against the grammar when it was made.
    #[must_use]
    pub fn form(&self) -> SubstanceKeyForm {
        let (key, _) = self.0.split_at(padded_length(&self.0));
        check(key).expect("a substance key was checked when it was made")
    }
}

/// The bytes of `padded` before its first NUL.
#[must_use]
const fn padded_length(padded: &[u8; SubstanceKey::MAX_BYTES]) -> usize {
    let mut length = 0;
    while length < SubstanceKey::MAX_BYTES && padded[length] != 0 {
        length += 1;
    }
    length
}

impl std::str::FromStr for SubstanceKey {
    type Err = ParseSubstanceKeyError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        Self::new(key)
    }
}

impl std::fmt::Debug for SubstanceKey {
    /// `SubstanceKey("H2O")`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SubstanceKey").field(&self.as_str()).finish()
    }
}

impl std::fmt::Display for SubstanceKey {
    /// The key itself.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The registry's formulas (decision-composition §1.1's rows) are formulas, each its own
    /// string back.
    #[test]
    fn substance_key_formulas_in_chemical_case_are_keys() {
        for key in [
            "H2", "He", "H2O", "CH4", "NH3", "N2", "O2", "CO2", "Ar", "N2O", "CO", "SO2", "H2S",
            "O3", "HCN", "C2H2", "C2H6", "PH3", "CH3OH", "H", "H-", "Na", "Fe", "SiO", "TiO", "VO",
            "FeH", "NH4SH", "H2SO4", "S8", "KCl", "Mg2SiO4", "CaTiO3", "Al2O3", "SiC", "MgSO4",
            "Na2CO3", "CaCO3", "NH4+", "HeH+", "C10H16",
        ] {
            let parsed = SubstanceKey::new(key).unwrap_or_else(|e| panic!("{key}: {e}"));
            assert_eq!(parsed.as_str(), key);
            assert_eq!(parsed.form(), SubstanceKeyForm::Formula, "{key}");
            assert_eq!(parsed.to_string(), key);
            assert_eq!(key.parse::<SubstanceKey>(), Ok(parsed));
        }
        // Case is chemistry: carbon monoxide and cobalt are two keys.
        assert_ne!(SubstanceKey::new("CO"), SubstanceKey::new("Co"));
    }

    /// The electron is `e-`, and the registry's names are names, up to 16 bytes.
    #[test]
    fn substance_key_the_electron_and_lowercase_names_are_keys() {
        let electron = SubstanceKey::new("e-").unwrap();
        assert_eq!(electron.form(), SubstanceKeyForm::Electron);
        assert_eq!(electron.as_str(), "e-");
        for key in [
            "basalt",
            "anorthosite",
            "granite",
            "phyllosilicate",
            "mars_dust",
            "brine_nacl",
            "ch4_c2h6_n2",
            "basalt_reduced",
            "ice_giant_haze",
            "tholin",
            "graphite",
            "c_type",
            "kbo_red",
            "e",
        ] {
            let parsed = SubstanceKey::new(key).unwrap_or_else(|e| panic!("{key}: {e}"));
            assert_eq!(parsed.form(), SubstanceKeyForm::Name, "{key}");
            assert_eq!(parsed.as_str(), key);
        }
        let longest = "abcdefghijklmnop";
        assert_eq!(SubstanceKey::new(longest).unwrap().as_str(), longest);
        assert_eq!(
            SubstanceKey::new("abcdefghijklmnopq"),
            Err(ParseSubstanceKeyError::TooLong { length: 17 })
        );
    }

    /// What breaks the grammar is refused, at the first byte that breaks it.
    #[test]
    fn substance_key_rejects_what_breaks_the_grammar() {
        let at = |at| Err(ParseSubstanceKeyError::Malformed { at });
        assert_eq!(SubstanceKey::new(""), Err(ParseSubstanceKeyError::Empty));
        for (key, expected) in [
            // A formula's symbols: an uppercase letter and at most one lowercase letter.
            ("H2o", at(2)),
            ("Basalt", at(2)),
            ("2H", at(0)),
            // Counts of 0 or 1, and leading zeros.
            ("H0", at(1)),
            ("H1", at(1)),
            ("C1O2", at(1)),
            ("H02", at(1)),
            // A sign is single and last.
            ("H--", at(1)),
            ("H-2", at(1)),
            ("+", at(0)),
            ("-", at(0)),
            // Names: lowercase letters, digits and underscores after a lowercase letter.
            ("mars-dust", at(4)),
            ("mars dust", at(4)),
            ("_basalt", at(0)),
            ("e+", at(1)),
            ("Fe O", at(2)),
            ("H\0O", at(1)),
            ("H₂O", at(1)),
        ] {
            assert_eq!(SubstanceKey::new(key), expected, "{key:?}");
        }
    }

    /// A key's padded form is its bytes and NUL to 16, and back; a padded form with bytes after its
    /// padding, or of no key, is refused.
    #[test]
    fn substance_key_round_trips_through_its_padded_form() {
        const WATER: SubstanceKey = SubstanceKey::new_const("H2O");
        let mut padded = [0; 16];
        padded[..3].copy_from_slice(b"H2O");
        assert_eq!(WATER.as_padded(), &padded);
        assert_eq!(SubstanceKey::from_padded(padded), Ok(WATER));
        let mut trailing = padded;
        trailing[9] = b'x';
        assert_eq!(
            SubstanceKey::from_padded(trailing),
            Err(ParseSubstanceKeyError::Padding)
        );
        assert_eq!(
            SubstanceKey::from_padded([0; 16]),
            Err(ParseSubstanceKeyError::Empty)
        );
        let full = *b"abcdefghijklmnop";
        assert_eq!(
            SubstanceKey::from_padded(full).unwrap().as_str(),
            "abcdefghijklmnop"
        );
        let mut bad = padded;
        bad[1] = b'1';
        assert_eq!(
            SubstanceKey::from_padded(bad),
            Err(ParseSubstanceKeyError::Malformed { at: 1 })
        );
        assert_eq!(format!("{WATER:?}"), "SubstanceKey(\"H2O\")");
    }

    /// Keys order as their strings do, so a palette sorted by key is sorted by string.
    #[test]
    fn substance_keys_order_as_their_strings() {
        let mut keys = ["basalt", "H2O", "CO2", "H2", "e-", "mars_dust", "Co", "CO"]
            .map(|k| SubstanceKey::new(k).unwrap());
        keys.sort();
        let mut strings = keys.map(|k| k.as_str().to_owned());
        let sorted = strings.clone();
        strings.sort();
        assert_eq!(sorted, strings);
    }
}
