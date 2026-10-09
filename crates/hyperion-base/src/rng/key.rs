//! What a stream is keyed by: the universe seed and the object it draws for.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use super::TagScope;
use crate::hex::{HexFault, parse_lower_hex};

/// A universe's seed: with [`GENERATOR_VERSION`](crate::GENERATOR_VERSION), what identifies a
/// universe.
///
/// It is the first key word of every stream. Its text form is exactly 16 lower-case hexadecimal
/// digits, as a `SystemId`'s is, so that one seed has one string and a
/// JSON reader cannot round it.
///
/// # Examples
///
/// ```
/// use hyperion_base::Seed;
///
/// let seed: Seed = "00000000deadbeef".parse()?;
/// assert_eq!(seed.get(), 0xdead_beef);
/// assert_eq!(seed.to_string(), "00000000deadbeef");
/// # Ok::<(), hyperion_base::rng::ParseSeedError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seed(u64);

impl Seed {
    /// The seed with this value. Every `u64` is a seed.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The seed's value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Seed {
    /// Exactly 16 lower-case hexadecimal digits.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl fmt::LowerHex for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl FromStr for Seed {
    type Err = ParseSeedError;

    /// Parses exactly 16 lower-case hexadecimal digits.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_lower_hex(s, 16)
            .map(Self)
            .map_err(ParseSeedError::from_fault)
    }
}

/// A [`Seed`] did not parse from its text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseSeedError {
    /// The text contains whitespace.
    Whitespace,
    /// The text starts with `0x`.
    HexPrefix,
    /// The text is not 16 bytes long.
    WrongLength {
        /// The length found, in bytes.
        length: usize,
    },
    /// A digit is upper case.
    UpperCase,
    /// A character is not a hexadecimal digit.
    InvalidDigit,
}

impl ParseSeedError {
    fn from_fault(fault: HexFault) -> Self {
        match fault {
            HexFault::Whitespace => Self::Whitespace,
            HexFault::HexPrefix => Self::HexPrefix,
            HexFault::WrongLength(length) => Self::WrongLength { length },
            HexFault::UpperCase => Self::UpperCase,
            HexFault::InvalidDigit => Self::InvalidDigit,
        }
    }
}

impl fmt::Display for ParseSeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Whitespace => f.write_str("a seed contains no whitespace"),
            Self::HexPrefix => f.write_str("a seed has no 0x prefix"),
            Self::WrongLength { length } => {
                write!(f, "a seed is 16 hexadecimal digits, not {length} bytes")
            }
            Self::UpperCase => f.write_str("a seed is lower case"),
            Self::InvalidDigit => f.write_str("a seed is hexadecimal digits only"),
        }
    }
}

impl Error for ParseSeedError {}

/// The faces of a cube sphere, which a surface cell key's 3-bit face field numbers 0 to 5.
const SURFACE_FACES: u8 = 6;

/// The deepest level a surface cell key holds: its `i` and `j` fields have 28 bits each.
const SURFACE_CELL_MAX_LEVEL: u8 = 28;

/// Bit position of a surface cell key's face field, its top 3 bits.
const SURFACE_FACE_SHIFT: u32 = 61;

/// Bit position of a surface cell key's level field, the 5 bits beneath the face.
const SURFACE_LEVEL_SHIFT: u32 = 56;

/// Bit position of a surface cell key's `i` field, the 28 bits above `j`.
const SURFACE_I_SHIFT: u32 = 28;

/// [`ObjectKey::surface_cell`] was given a cell that no cube-sphere level up to 28 holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceCellKeyError {
    /// The face is not one of the cube's six, 0 to 5.
    Face {
        /// The face given.
        face: u8,
    },
    /// The level is above 28, the deepest a 28-bit coordinate holds.
    Level {
        /// The level given.
        level: u8,
    },
    /// A coordinate is 2^`level` or more.
    Coordinate {
        /// The level given, at most 28.
        level: u8,
        /// The `i` given.
        i: u32,
        /// The `j` given.
        j: u32,
    },
}

impl fmt::Display for SurfaceCellKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Face { face } => write!(f, "a cube face is 0 to 5, not {face}"),
            Self::Level { level } => {
                write!(f, "a surface cell's level is at most 28, not {level}")
            }
            Self::Coordinate { level, i, j } => write!(
                f,
                "cell ({i}, {j}) is past level {level}, whose coordinates are below 2^{level}"
            ),
        }
    }
}

impl Error for SurfaceCellKeyError {}

/// The object a stream draws for: its counter word 0, its sub-object number and its scope.
///
/// | Scope                      | Word                                          | Sub              |
/// | -------------------------- | --------------------------------------------- | ---------------- |
/// | [`TagScope::Galaxy`]       | 0 for the galaxy, or an item number of a list | 0                |
/// | [`TagScope::Cell`]         | the cell word: a candidate ID, index zeroed   | 0                |
/// | [`TagScope::Feature`]      | the feature's object word                     | 0                |
/// | [`TagScope::System`]       | the `SystemId`'s raw value | 0            |
/// | [`TagScope::Body`]         | the body's system's raw value                 | the body index   |
/// | [`TagScope::SurfaceCoarse`], a surface cell | face (3 bits), level (5), `i` (28), `j` (28) | the instance |
/// | [`TagScope::SurfaceCoarse`], a surface item | the item number, such as a plate's | 0         |
///
/// The word becomes counter word 0 and `sub` the top 16 bits of counter word 1. Keys are built
/// from integers only: float bits are never hashed. A key of one scope may share its word with a
/// key of another (a cell's word is its candidate 0's ID, and body 0 shares its system's word
/// and `sub`), which is safe because a domain tag names one scope and [`Stream::open`] checks it.
///
/// The two surface forms are the exception (plan R09, Design note 2). Their streams are opened by
/// a body's surface or detail seed, which takes the universe seed's place and so carries the body,
/// and both forms carry [`TagScope::SurfaceCoarse`], which [`SurfaceSeed::stream`] and
/// [`DetailSeed::stream`] do not read: they check the tag's scope against their seed's, and
/// [`Stream::open`] refuses both surface scopes, so the key's own scope only keeps a surface key
/// from every tag but a self-test one. A surface item's word can equal a low surface cell's, so
/// each surface tag is used with one form and never both, as a tag is used with
/// [`galaxy`](Self::galaxy) or [`galaxy_item`](Self::galaxy_item) and never both; the surface
/// crate's registry documents each tag's form, and its tests hold every call site to it.
///
/// [`Stream::open`]: super::Stream::open
/// [`SurfaceSeed::stream`]: super::SurfaceSeed::stream
/// [`DetailSeed::stream`]: super::DetailSeed::stream
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey {
    word: u64,
    sub: u16,
    scope: TagScope,
}

impl ObjectKey {
    /// A key from its parts. The public constructors and the conversions from IDs call this.
    #[must_use]
    pub(crate) const fn new(word: u64, sub: u16, scope: TagScope) -> Self {
        Self { word, sub, scope }
    }

    /// The galaxy as a whole: word 0.
    ///
    /// This is the same key as [`galaxy_item(0)`](Self::galaxy_item). A tag is used with one
    /// form or the other, never both.
    #[must_use]
    pub const fn galaxy() -> Self {
        Self::new(0, 0, TagScope::Galaxy)
    }

    /// Item `n` of a galaxy-wide list, such as a progenitor or a halo component.
    #[must_use]
    pub const fn galaxy_item(n: u64) -> Self {
        Self::new(n, 0, TagScope::Galaxy)
    }

    /// A generation cell, by its word: the ID of the cell's candidate 0 with the index zeroed,
    /// `SystemId::cell_word`.
    #[must_use]
    pub const fn cell(word: u64) -> Self {
        Self::new(word, 0, TagScope::Cell)
    }

    /// A feature, by its object word, such as
    /// `FeatureRef::object_word`.
    #[must_use]
    pub const fn feature(word: u64) -> Self {
        Self::new(word, 0, TagScope::Feature)
    }

    /// A system, by its raw 64-bit ID: that word, `sub` 0, scope [`TagScope::System`].
    ///
    /// Callers with a `SystemId` convert it (`ObjectKey::from(id)`), which calls this; the raw
    /// form exists so that the conversion can live beside the ID type, above this crate. A key is
    /// only ever built from integers: never from float bits, pointers or a `usize`.
    #[must_use]
    pub const fn system(raw_system_id: u64) -> Self {
        Self::new(raw_system_id, 0, TagScope::System)
    }

    /// A body, by its system's raw 64-bit ID and its body index: that word, `sub` the index, which
    /// shares counter word 1 with the draw number, scope [`TagScope::Body`].
    ///
    /// Callers with a `BodyId` convert it (`ObjectKey::from(id)`), which calls this.
    #[must_use]
    pub const fn body(raw_system_id: u64, body_index: u16) -> Self {
        Self::new(raw_system_id, body_index, TagScope::Body)
    }

    /// A cell of a body's cube sphere: `face` 0–5, `level` 0–28, and the cell's integer
    /// coordinates `i` and `j` on the face, each below 2^`level`; `instance` numbers the draws of
    /// one cell, such as a channel level's slot or a crater's index (plan R09, Design note 2).
    ///
    /// The word packs face (3 bits), level (5 bits), `i` and `j` (28 bits each), from the top
    /// bit down, so two cells never share a word; `instance` is `sub`. The body is not in the
    /// key: it is in the surface or detail seed that opens the stream. 28 bits reach level 28,
    /// beyond the deepest level the terrain uses.
    ///
    /// # Errors
    ///
    /// [`SurfaceCellKeyError::Face`] for a face of 6 or more, [`SurfaceCellKeyError::Level`] for a
    /// level above 28, and [`SurfaceCellKeyError::Coordinate`] for an `i` or `j` of 2^`level` or
    /// more.
    pub const fn surface_cell(
        face: u8,
        level: u8,
        i: u32,
        j: u32,
        instance: u16,
    ) -> Result<Self, SurfaceCellKeyError> {
        if face >= SURFACE_FACES {
            return Err(SurfaceCellKeyError::Face { face });
        }
        if level > SURFACE_CELL_MAX_LEVEL {
            return Err(SurfaceCellKeyError::Level { level });
        }
        // `level` is at most 28 here, so the side fits a `u32`.
        let side = 1_u32 << level;
        if i >= side || j >= side {
            return Err(SurfaceCellKeyError::Coordinate { level, i, j });
        }
        // `u64::from` is not `const`; each part widens to `u64` losslessly, and each is below its
        // field's width, so the fields do not overlap.
        let word = ((face as u64) << SURFACE_FACE_SHIFT)
            | ((level as u64) << SURFACE_LEVEL_SHIFT)
            | ((i as u64) << SURFACE_I_SHIFT)
            | (j as u64);
        Ok(Self::new(word, instance, TagScope::SurfaceCoarse))
    }

    /// Item `n` of a body's surface list, such as plate `n`, crater slot `n`, or a lattice
    /// corner's word: word `n`, `sub` 0 (plan R09, Design note 2).
    ///
    /// The body is not in the key: it is in the surface or detail seed that opens the stream.
    #[must_use]
    pub const fn surface_item(n: u64) -> Self {
        Self::new(n, 0, TagScope::SurfaceCoarse)
    }

    /// Counter word 0 of every stream opened for this object.
    #[must_use]
    pub const fn word(self) -> u64 {
        self.word
    }

    /// The sub-object number: a body's index, a surface cell's instance, otherwise 0.
    #[must_use]
    pub const fn sub(self) -> u16 {
        self.sub
    }

    /// The kind of object this key names, which must be its tag's scope.
    #[must_use]
    pub const fn scope(self) -> TagScope {
        self.scope
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn seed_text_is_sixteen_lower_case_digits() {
        for value in [0, 1, 0xdead_beef, u64::MAX, 1 << 63] {
            let seed = Seed::new(value);
            let text = seed.to_string();
            assert_eq!(text.len(), 16);
            assert_eq!(text.parse::<Seed>(), Ok(seed));
        }
        assert_eq!(Seed::new(0xabc).to_string(), "0000000000000abc");
        assert_eq!(format!("{:x}", Seed::new(0xabc)), "abc");
    }

    #[test]
    fn seed_parser_rejects_each_malformation_with_its_own_variant() {
        let cases = [
            ("00000000DEADBEEF", ParseSeedError::UpperCase),
            ("0x000000deadbeef", ParseSeedError::HexPrefix),
            (" 0000000deadbeef", ParseSeedError::Whitespace),
            ("deadbeef", ParseSeedError::WrongLength { length: 8 }),
            ("000000000000000g", ParseSeedError::InvalidDigit),
        ];
        for (text, error) in cases {
            assert_eq!(text.parse::<Seed>(), Err(error), "{text:?}");
        }
        assert_eq!(
            ParseSeedError::WrongLength { length: 8 }.to_string(),
            "a seed is 16 hexadecimal digits, not 8 bytes"
        );
    }

    #[test]
    fn a_seed_above_two_to_the_fifty_three_survives_its_text_form() {
        let seed = Seed::new((1 << 53) + 1);
        assert_eq!(seed.to_string().parse::<Seed>(), Ok(seed));
    }

    #[test]
    fn constructors_fix_word_sub_and_scope() {
        assert_eq!(ObjectKey::galaxy(), ObjectKey::galaxy_item(0));
        let item = ObjectKey::galaxy_item(7);
        assert_eq!(
            (item.word(), item.sub(), item.scope()),
            (7, 0, TagScope::Galaxy)
        );
        let cell = ObjectKey::cell(0x0200_0800_2000_0000);
        assert_eq!(
            (cell.word(), cell.sub(), cell.scope()),
            (0x0200_0800_2000_0000, 0, TagScope::Cell)
        );
        let feature = ObjectKey::feature(42);
        assert_eq!(
            (feature.word(), feature.sub(), feature.scope()),
            (42, 0, TagScope::Feature)
        );
        let system = ObjectKey::system(0x0200_0800_2000_0007);
        assert_eq!(
            (system.word(), system.sub(), system.scope()),
            (0x0200_0800_2000_0007, 0, TagScope::System)
        );
        let body = ObjectKey::body(0x0200_0800_2000_0007, 3);
        assert_eq!(
            (body.word(), body.sub(), body.scope()),
            (0x0200_0800_2000_0007, 3, TagScope::Body)
        );
        let item = ObjectKey::surface_item(48);
        assert_eq!(
            (item.word(), item.sub(), item.scope()),
            (48, 0, TagScope::SurfaceCoarse)
        );
    }

    /// The face, level, `i` and `j` a surface cell key's word holds, read back by its layout.
    fn unpack(key: ObjectKey) -> (u8, u8, u32, u32, u16) {
        let word = key.word();
        let field = |shift: u32, bits: u32| (word >> shift) & ((1 << bits) - 1);
        (
            u8::try_from(field(SURFACE_FACE_SHIFT, 3)).unwrap(),
            u8::try_from(field(SURFACE_LEVEL_SHIFT, 5)).unwrap(),
            u32::try_from(field(SURFACE_I_SHIFT, 28)).unwrap(),
            u32::try_from(field(0, 28)).unwrap(),
            key.sub(),
        )
    }

    #[test]
    fn a_surface_cell_key_round_trips_its_fields() {
        let cases = [
            (0, 0, 0, 0, 0),
            (5, 0, 0, 0, u16::MAX),
            (2, 8, 255, 17, 3),
            (3, 19, (1 << 19) - 1, 0, 1),
            (4, 24, 0x00ab_cdef, 0x0012_3456, 7),
            (5, 28, (1 << 28) - 1, (1 << 28) - 1, u16::MAX),
        ];
        for (face, level, i, j, instance) in cases {
            let key = ObjectKey::surface_cell(face, level, i, j, instance).unwrap();
            assert_eq!(unpack(key), (face, level, i, j, instance));
            assert_eq!(key.scope(), TagScope::SurfaceCoarse);
        }
        let top = ObjectKey::surface_cell(5, 28, (1 << 28) - 1, (1 << 28) - 1, 0).unwrap();
        assert_eq!(
            top.word(),
            0xbcff_ffff_ffff_ffff,
            "face 5, level 28, all ones"
        );
    }

    #[test]
    fn a_surface_cell_key_refuses_what_no_level_holds() {
        assert_eq!(
            ObjectKey::surface_cell(6, 0, 0, 0, 0),
            Err(SurfaceCellKeyError::Face { face: 6 })
        );
        assert_eq!(
            ObjectKey::surface_cell(7, 3, 0, 0, 0),
            Err(SurfaceCellKeyError::Face { face: 7 })
        );
        for level in 29..=u8::MAX {
            assert_eq!(
                ObjectKey::surface_cell(0, level, 0, 0, 0),
                Err(SurfaceCellKeyError::Level { level })
            );
        }
        for level in 0..=SURFACE_CELL_MAX_LEVEL {
            let side = 1_u32 << level;
            let last = ObjectKey::surface_cell(1, level, side - 1, side - 1, 0).unwrap();
            assert_eq!(unpack(last), (1, level, side - 1, side - 1, 0));
            for (i, j) in [(side, 0), (0, side), (side, side), (u32::MAX, 0)] {
                assert_eq!(
                    ObjectKey::surface_cell(1, level, i, j, 0),
                    Err(SurfaceCellKeyError::Coordinate { level, i, j }),
                    "level {level}, ({i}, {j})"
                );
            }
        }
        assert_eq!(
            SurfaceCellKeyError::Coordinate {
                level: 3,
                i: 8,
                j: 0
            }
            .to_string(),
            "cell (8, 0) is past level 3, whose coordinates are below 2^3"
        );
        assert_eq!(
            SurfaceCellKeyError::Level { level: 29 }.to_string(),
            "a surface cell's level is at most 28, not 29"
        );
        assert_eq!(
            SurfaceCellKeyError::Face { face: 6 }.to_string(),
            "a cube face is 0 to 5, not 6"
        );
    }

    /// Every cell of every face at levels 0–5, and the corners and centres of every level to 28,
    /// has a word of its own: a cell at one level never shares a key with a cell at another, as
    /// a quadtree's parent and child share their `i` and `j` bits.
    #[test]
    fn two_surface_cells_never_share_a_key() {
        let mut words = std::collections::BTreeSet::new();
        let mut count = 0_u64;
        for face in 0..SURFACE_FACES {
            for level in 0..=5 {
                let side = 1_u32 << level;
                for i in 0..side {
                    for j in 0..side {
                        words.insert(
                            ObjectKey::surface_cell(face, level, i, j, 0)
                                .unwrap()
                                .word(),
                        );
                        count += 1;
                    }
                }
            }
            for level in 6..=SURFACE_CELL_MAX_LEVEL {
                let last = (1_u32 << level) - 1;
                let mid = 1_u32 << (level - 1);
                for (i, j) in [
                    (0, 0),
                    (0, last),
                    (last, 0),
                    (last, last),
                    (mid, mid),
                    (1, 0),
                ] {
                    words.insert(
                        ObjectKey::surface_cell(face, level, i, j, 0)
                            .unwrap()
                            .word(),
                    );
                    count += 1;
                }
            }
        }
        assert_eq!(u64::try_from(words.len()).unwrap(), count);
        // 6 faces × (1 + 4 + 16 + 64 + 256 + 1,024 cells) + 6 × 23 levels × 6 cells.
        assert_eq!(count, 6 * 1_365 + 6 * 23 * 6);
    }
}
