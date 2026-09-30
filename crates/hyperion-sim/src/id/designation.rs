//! Designations: human-readable catalogue names derived from an ID alone.
//!
//! A designation is bijective with its ID, in the manner of Elite's `Sector AB-C d12-3`, so every
//! system has a unique catalogue name before any language generation exists. Proper names from
//! generated languages are a later overlay. The format is **not** part of the generator version:
//! saves hold IDs, so it may change without moving a star.
//!
//! Digits are Crockford's base 32, `0123456789ABCDEFGHJKMNPQRSTVWXYZ`. A **sector** is a 4,096 ly
//! cube, 32 per axis, named by three digits for x, y and z. Layer and band letters are `A`–`E`,
//! `F` for brown dwarfs and `G` for rogue planets. Indices and numbers are decimal without
//! padding.
//!
//! | Kind                  | Format                                                      | Example                   |
//! | --------------------- | ----------------------------------------------------------- | ------------------------- |
//! | Grid                  | `<sector> <xx><yy><zz> <layer>-<index>`                     | `H7K 4C0RFZ A-7`          |
//! | Feature member        | `<sector> F<feature index> <band><level>-<x><y><z>-<index>` | `H7K F212 C3-8F2-40`      |
//! | Feature-level member  | `<sector> F<feature index> M<index>`                        | `H7K F212 M0`             |
//! | Centre                | `CENTRE <band><level>-<x><y><z>-<index>`                    | `CENTRE A9-2HJ-12`        |
//! | Centre, feature level | `CENTRE M<index>`                                           | `CENTRE M0`               |
//! | Stream                | `STREAM <number> <band>-<along>-<a>.<b>-<index>`            | `STREAM 87 A-5121-3.23-9` |
//! | Dwarf core            | `DWARF <number> …` as a feature member                      | `DWARF 1 B2-47C-3`        |
//! | Pinned                | `PIN <number>`                                              | `PIN 42`                  |
//! | Catalogue system      | `<sector> K<class>-<x><y><z>-<index>`, `/<member>` if ≠ 0   | `H7K K3-052-118/1`        |
//!
//! In a grid designation the sector is the top five bits of each stored cell coordinate, and
//! `<xx>` is the remaining `9 − k` bits (10 for rogue planets) of that axis as two digits. In a
//! catalogue system's, the sector is the top five bits of each 8-bit coordinate and `<x>` the low
//! three as one digit. A feature member's sector is its feature cell. Levels and nested cells are
//! one digit each. A body appends ` /<body index>`.

use std::error::Error;
use std::fmt::{self, Write as _};
use std::str::FromStr;

use super::body::BodyId;
use super::catalogue::{CatalogueSystemId, PinnedId};
use super::global::{CentreMemberId, DwarfCoreMemberId, StreamMemberId};
use super::grid::GridId;
use super::layer::Layer;
use super::nested::MemberSlot;
use super::reserved::{FeatureCell, FeatureMemberId, FeatureRef};
use super::system::{BuildSystemIdError, DecodeSystemIdError, SystemId, SystemIdKind};
use crate::coords::GenCell;

/// Crockford's base-32 alphabet: the digits and the letters without I, L, O and U.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The base-32 digit of a value below 32.
fn digit(value: u8) -> char {
    char::from(ALPHABET[usize::from(value & 31)])
}

/// The value of a canonical base-32 digit.
fn digit_value(byte: u8) -> Option<u8> {
    ALPHABET
        .iter()
        .position(|&d| d == byte)
        .map(|v| u8::try_from(v).expect("the alphabet has 32 digits"))
}

/// A system's or body's designation: the catalogue name derived from its ID.
///
/// It holds the ID and formats on [`Display`](fmt::Display); [`FromStr`] parses the text back to
/// the same ID, validated by [`SystemId::from_raw`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{BodyId, Designation, SystemId};
///
/// let id = SystemId::from_raw(0x0228_C386_27FF_0007)?;
/// assert_eq!(id.designation().to_string(), "H7K 4C0RFZ A-7");
/// let parsed: Designation = "H7K 4C0RFZ A-7".parse()?;
/// assert_eq!(SystemId::try_from(&parsed)?, id);
///
/// let planet = BodyId::new(id, 0x0100);
/// assert_eq!(planet.designation().to_string(), "H7K 4C0RFZ A-7 /256");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Designation {
    system: SystemId,
    body_index: Option<u16>,
}

impl Designation {
    /// The designation of a system.
    #[must_use]
    pub const fn of_system(system: SystemId) -> Self {
        Self {
            system,
            body_index: None,
        }
    }

    /// The designation of a body.
    #[must_use]
    pub const fn of_body(body: BodyId) -> Self {
        Self {
            system: body.system(),
            body_index: Some(body.body_index()),
        }
    }

    /// The system designated, or the system of the body designated.
    #[must_use]
    pub const fn system(self) -> SystemId {
        self.system
    }

    /// The body designated, or `None` for a system's designation.
    #[must_use]
    pub fn body(self) -> Option<BodyId> {
        self.body_index.map(|index| BodyId::new(self.system, index))
    }
}

impl From<SystemId> for Designation {
    fn from(system: SystemId) -> Self {
        Self::of_system(system)
    }
}

impl From<BodyId> for Designation {
    fn from(body: BodyId) -> Self {
        Self::of_body(body)
    }
}

/// A [`Designation`] names the other kind of object than the one asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConvertDesignationError {
    /// A system was asked for, and the designation names a body.
    NamesBody,
    /// A body was asked for, and the designation names a system.
    NamesSystem,
}

impl fmt::Display for ConvertDesignationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NamesBody => "the designation names a body, not a system",
            Self::NamesSystem => "the designation names a system, not a body",
        })
    }
}

impl Error for ConvertDesignationError {}

impl TryFrom<&Designation> for SystemId {
    type Error = ConvertDesignationError;

    fn try_from(designation: &Designation) -> Result<Self, Self::Error> {
        match designation.body_index {
            None => Ok(designation.system),
            Some(_) => Err(ConvertDesignationError::NamesBody),
        }
    }
}

impl TryFrom<&Designation> for BodyId {
    type Error = ConvertDesignationError;

    fn try_from(designation: &Designation) -> Result<Self, Self::Error> {
        designation
            .body()
            .ok_or(ConvertDesignationError::NamesSystem)
    }
}

impl fmt::Display for Designation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.system.kind() {
            SystemIdKind::Grid(id) => write_grid(f, id)?,
            SystemIdKind::FeatureMember(id) => {
                let feature = id.feature();
                write_digits(f, feature.cell().stored())?;
                write!(f, " F{} ", feature.index())?;
                write_slot(f, id.slot())?;
            }
            SystemIdKind::Centre(id) => {
                f.write_str("CENTRE ")?;
                write_slot(f, id.slot())?;
            }
            SystemIdKind::Stream(id) => {
                let [a, b] = id.across();
                write!(
                    f,
                    "STREAM {} {}-{}-{a}.{b}-{}",
                    id.number(),
                    id.band().letter(),
                    id.along(),
                    id.index()
                )?;
            }
            SystemIdKind::DwarfCore(id) => {
                write!(f, "DWARF {} ", id.number())?;
                write_slot(f, id.slot())?;
            }
            SystemIdKind::Pinned(id) => write!(f, "PIN {}", id.number())?,
            SystemIdKind::Catalogue(id) => {
                let stored = id.stored_cell();
                write_digits(f, stored.map(|s| s >> 3))?;
                write!(f, " K{}-", id.class())?;
                write_digits(f, stored.map(|s| s & 7))?;
                write!(f, "-{}", id.index())?;
                if id.member() != 0 {
                    write!(f, "/{}", id.member())?;
                }
            }
        }
        if let Some(index) = self.body_index {
            write!(f, " /{index}")?;
        }
        Ok(())
    }
}

/// Writes one base-32 digit per value.
fn write_digits(f: &mut fmt::Formatter<'_>, values: [u8; 3]) -> fmt::Result {
    values.into_iter().try_for_each(|v| f.write_char(digit(v)))
}

/// Writes `<sector> <xx><yy><zz> <layer>-<index>`.
fn write_grid(f: &mut fmt::Formatter<'_>, id: GridId) -> fmt::Result {
    let layer = id.layer();
    let rest_bits = layer.cell_bits_per_axis() - 5;
    let stored = id.stored_cell();
    for s in stored {
        f.write_char(digit(low_u8(s >> rest_bits)))?;
    }
    f.write_char(' ')?;
    for s in stored {
        let rest = s & ((1 << rest_bits) - 1);
        f.write_char(digit(low_u8(rest >> 5)))?;
        f.write_char(digit(low_u8(rest)))?;
    }
    write!(f, " {}-{}", layer.letter(), id.index())
}

/// The low five bits of a value, as a digit value.
fn low_u8(value: u32) -> u8 {
    u8::try_from(value & 31).expect("five bits fit in u8")
}

/// Writes `<band><level>-<x><y><z>-<index>` or `M<index>`.
fn write_slot(f: &mut fmt::Formatter<'_>, slot: MemberSlot) -> fmt::Result {
    match slot {
        MemberSlot::InCell {
            band,
            level,
            cell,
            index,
        } => {
            f.write_char(band.letter())?;
            f.write_char(digit(level))?;
            f.write_char('-')?;
            write_digits(f, cell)?;
            write!(f, "-{index}")
        }
        MemberSlot::FeatureLevel { index } => write!(f, "M{index}"),
    }
}

/// A [`Designation`] did not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseDesignationError {
    /// The text does not have the shape of any designation.
    UnknownForm,
    /// A character is not a digit of the field's alphabet (Crockford base 32, or decimal).
    InvalidDigit,
    /// A layer or band letter is not `A`–`G`.
    UnknownLayer,
    /// A base-32 digit is beyond its field: a cell digit beyond the layer's bits, a level or a
    /// nested cell beyond the grid's.
    DigitOutOfRange,
    /// A decimal number is beyond its field.
    NumberOutOfRange,
    /// The text is not the one string of its ID: a decimal with a leading zero, or an explicit
    /// catalogue member `/0`.
    NotCanonical,
    /// The fields break a rule of the ID's layout.
    Decode(DecodeSystemIdError),
}

impl fmt::Display for ParseDesignationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownForm => f.write_str("not the form of any designation"),
            Self::InvalidDigit => f.write_str("a character is not a digit of its field"),
            Self::UnknownLayer => f.write_str("a layer letter is not A to G"),
            Self::DigitOutOfRange => f.write_str("a digit is beyond its field"),
            Self::NumberOutOfRange => f.write_str("a number is beyond its field"),
            Self::NotCanonical => f.write_str("not the canonical text of its id"),
            Self::Decode(e) => write!(f, "the designation names a malformed id: {e}"),
        }
    }
}

impl Error for ParseDesignationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::UnknownForm
            | Self::InvalidDigit
            | Self::UnknownLayer
            | Self::DigitOutOfRange
            | Self::NumberOutOfRange
            | Self::NotCanonical => None,
        }
    }
}

impl FromStr for Designation {
    type Err = ParseDesignationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (text, body_index) = match s.rsplit_once(" /") {
            Some((system, index)) => {
                let index = u16::try_from(decimal(index, 1 << 16)?)
                    .expect("the bound keeps the index in u16");
                (system, Some(index))
            }
            None => (s, None),
        };
        let kind = parse_system(text)?;
        let system = SystemId::from_raw(SystemId::from(kind).raw())
            .map_err(ParseDesignationError::Decode)?;
        Ok(Self { system, body_index })
    }
}

/// Maps a constructor's error after the parser has range-checked every field: only the layout's
/// rules can still fail.
fn from_build(error: BuildSystemIdError) -> ParseDesignationError {
    match error {
        BuildSystemIdError::NotCanonical(e) => ParseDesignationError::Decode(e),
        BuildSystemIdError::CellOutsideRootCube
        | BuildSystemIdError::CellSizeMismatch
        | BuildSystemIdError::IndexTooLarge
        | BuildSystemIdError::FieldOutOfRange { .. } => ParseDesignationError::NumberOutOfRange,
    }
}

/// Parses a system's designation into its kind.
fn parse_system(text: &str) -> Result<SystemIdKind, ParseDesignationError> {
    let mut tokens = text.split(' ');
    let first = tokens.next().unwrap_or_default();
    let (second, third) = (tokens.next(), tokens.next());
    if tokens.next().is_some() {
        return Err(ParseDesignationError::UnknownForm);
    }
    let kind = match (first, second, third) {
        ("CENTRE", Some(slot), None) => {
            let slot = parse_slot(slot, 12, 32)?;
            SystemIdKind::Centre(CentreMemberId::new(slot).map_err(from_build)?)
        }
        ("STREAM", Some(number), Some(rest)) => SystemIdKind::Stream(parse_stream(number, rest)?),
        ("DWARF", Some(number), Some(slot)) => {
            let number = decimal(number, u64::from(DwarfCoreMemberId::NUMBER_LIMIT))?;
            let number = u8::try_from(number).expect("the bound keeps the number in u8");
            let slot = parse_slot(slot, 8, 16)?;
            SystemIdKind::DwarfCore(DwarfCoreMemberId::new(number, slot).map_err(from_build)?)
        }
        ("PIN", Some(number), None) => {
            let number = decimal(number, PinnedId::NUMBER_LIMIT)?;
            SystemIdKind::Pinned(PinnedId::new(number).map_err(from_build)?)
        }
        (sector, Some(second), None) => {
            let entry = second
                .strip_prefix('K')
                .ok_or(ParseDesignationError::UnknownForm)?;
            SystemIdKind::Catalogue(parse_catalogue(parse_sector(sector)?, entry)?)
        }
        (sector, Some(second), Some(third)) => {
            let sector = parse_sector(sector)?;
            if third.as_bytes().get(1) == Some(&b'-') {
                SystemIdKind::Grid(parse_grid(sector, second, third)?)
            } else {
                SystemIdKind::FeatureMember(parse_feature_member(sector, second, third)?)
            }
        }
        _ => return Err(ParseDesignationError::UnknownForm),
    };
    Ok(kind)
}

/// Parses a decimal number below `limit`, without sign or leading zeros.
fn decimal(text: &str, limit: u64) -> Result<u64, ParseDesignationError> {
    if text.is_empty() {
        return Err(ParseDesignationError::UnknownForm);
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseDesignationError::InvalidDigit);
    }
    if text.len() > 1 && text.starts_with('0') {
        return Err(ParseDesignationError::NotCanonical);
    }
    text.bytes()
        .try_fold(0_u64, |acc, b| {
            acc.checked_mul(10)?.checked_add(u64::from(b - b'0'))
        })
        .filter(|&value| value < limit)
        .ok_or(ParseDesignationError::NumberOutOfRange)
}

/// Parses exactly `N` base-32 digits.
fn base32<const N: usize>(text: &str) -> Result<[u8; N], ParseDesignationError> {
    if text.len() != N {
        return Err(ParseDesignationError::UnknownForm);
    }
    let bytes = text.as_bytes();
    let mut digits = [0; N];
    for (d, &b) in digits.iter_mut().zip(bytes) {
        *d = digit_value(b).ok_or(ParseDesignationError::InvalidDigit)?;
    }
    Ok(digits)
}

/// Parses three base-32 digits, each below `limit`.
fn digits_below(text: &str, limit: u8) -> Result<[u8; 3], ParseDesignationError> {
    let digits = base32::<3>(text)?;
    if digits.iter().any(|&d| d >= limit) {
        return Err(ParseDesignationError::DigitOutOfRange);
    }
    Ok(digits)
}

/// Parses a sector: three base-32 digits, each 0–31.
fn parse_sector(text: &str) -> Result<[u8; 3], ParseDesignationError> {
    base32::<3>(text)
}

/// Parses a one-letter layer or band.
fn parse_layer(text: &str) -> Result<Layer, ParseDesignationError> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), None) => {
            Layer::from_letter(letter).ok_or(ParseDesignationError::UnknownLayer)
        }
        _ => Err(ParseDesignationError::UnknownForm),
    }
}

/// Parses `<xx><yy><zz>` and `<layer>-<index>` in `sector`.
fn parse_grid(sector: [u8; 3], cell: &str, tail: &str) -> Result<GridId, ParseDesignationError> {
    let (letter, index) = tail
        .split_once('-')
        .ok_or(ParseDesignationError::UnknownForm)?;
    let layer = parse_layer(letter)?;
    let rest_bits = layer.cell_bits_per_axis() - 5;
    let offset = 1_i32 << (layer.cell_bits_per_axis() - 1);
    let digits = base32::<6>(cell)?;
    let mut coordinates = [0; 3];
    for (axis, c) in coordinates.iter_mut().enumerate() {
        let rest = u32::from(digits[2 * axis]) * 32 + u32::from(digits[2 * axis + 1]);
        if rest >> rest_bits != 0 {
            return Err(ParseDesignationError::DigitOutOfRange);
        }
        let stored = (u32::from(sector[axis]) << rest_bits) | rest;
        *c = stored.cast_signed() - offset;
    }
    let index = decimal(index, 1 << layer.index_bits())?;
    let cell = GenCell::new(layer.cell_size(), coordinates).map_err(|_| {
        // Unreachable: every stored coordinate names a cell inside the root cube.
        ParseDesignationError::DigitOutOfRange
    })?;
    GridId::new(
        layer,
        cell,
        u32::try_from(index).expect("the bound keeps the index in u32"),
    )
    .map_err(from_build)
}

/// Parses `<band><level>-<x><y><z>-<index>` or `M<index>`, with levels below `levels` and cell
/// coordinates below `cells`.
fn parse_slot(text: &str, levels: u8, cells: u8) -> Result<MemberSlot, ParseDesignationError> {
    let index_of = |text: &str| {
        decimal(text, u64::from(MemberSlot::INDEX_LIMIT))
            .map(|i| u16::try_from(i).expect("the bound keeps the index in u16"))
    };
    if let Some(index) = text.strip_prefix('M') {
        return Ok(MemberSlot::FeatureLevel {
            index: index_of(index)?,
        });
    }
    let mut parts = text.split('-');
    let (Some(head), Some(cell), Some(index), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ParseDesignationError::UnknownForm);
    };
    let (band, level) = match head.char_indices().nth(1) {
        Some((split, _)) => head.split_at(split),
        None => return Err(ParseDesignationError::UnknownForm),
    };
    let band = parse_layer(band)?;
    let [level] = base32::<1>(level)?;
    if level >= levels {
        return Err(ParseDesignationError::DigitOutOfRange);
    }
    Ok(MemberSlot::InCell {
        band,
        level,
        cell: digits_below(cell, cells)?,
        index: index_of(index)?,
    })
}

/// Parses `F<feature index>` and the slot in `sector`.
fn parse_feature_member(
    sector: [u8; 3],
    feature: &str,
    slot: &str,
) -> Result<FeatureMemberId, ParseDesignationError> {
    let index = feature
        .strip_prefix('F')
        .ok_or(ParseDesignationError::UnknownForm)?;
    let index = decimal(index, u64::from(FeatureRef::INDEX_LIMIT))?;
    let cell = FeatureCell::new(sector.map(|s| i32::from(s) - 16)).map_err(from_build)?;
    let feature = FeatureRef::new(
        cell,
        u16::try_from(index).expect("the bound keeps the index in u16"),
    )
    .map_err(from_build)?;
    FeatureMemberId::new(feature, parse_slot(slot, 8, 16)?).map_err(from_build)
}

/// Parses `<number>` and `<band>-<along>-<a>.<b>-<index>`.
fn parse_stream(number: &str, rest: &str) -> Result<StreamMemberId, ParseDesignationError> {
    let number = decimal(number, u64::from(StreamMemberId::NUMBER_LIMIT))?;
    let mut parts = rest.split('-');
    let (Some(band), Some(along), Some(across), Some(index), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return Err(ParseDesignationError::UnknownForm);
    };
    let (a, b) = across
        .split_once('.')
        .ok_or(ParseDesignationError::UnknownForm)?;
    let across_limit = u64::from(StreamMemberId::ACROSS_LIMIT);
    let narrow = |v: u64| u16::try_from(v).expect("every stream field bound fits in u16");
    StreamMemberId::new(
        narrow(number),
        parse_layer(band)?,
        narrow(decimal(along, u64::from(StreamMemberId::ALONG_LIMIT))?),
        [decimal(a, across_limit)?, decimal(b, across_limit)?]
            .map(|v| u8::try_from(v).expect("the bound keeps a cell across in u8")),
        narrow(decimal(index, u64::from(MemberSlot::INDEX_LIMIT))?),
    )
    .map_err(from_build)
}

/// Parses `<class>-<x><y><z>-<index>` and an optional `/<member>` in `sector`.
fn parse_catalogue(
    sector: [u8; 3],
    entry: &str,
) -> Result<CatalogueSystemId, ParseDesignationError> {
    let mut parts = entry.split('-');
    let (Some(class), Some(cell), Some(tail), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ParseDesignationError::UnknownForm);
    };
    let class = decimal(class, u64::from(CatalogueSystemId::CLASS_LIMIT))?;
    let low = digits_below(cell, 8)?;
    let (index, member) = match tail.split_once('/') {
        Some((index, member)) => {
            let member = decimal(member, u64::from(CatalogueSystemId::MEMBER_LIMIT))?;
            if member == 0 {
                return Err(ParseDesignationError::NotCanonical);
            }
            (index, member)
        }
        None => (tail, 0),
    };
    let index = decimal(index, u64::from(CatalogueSystemId::INDEX_LIMIT))?;
    let mut coordinates = [0; 3];
    for ((c, s), l) in coordinates.iter_mut().zip(sector).zip(low) {
        *c = ((i32::from(s) << 3) | i32::from(l)) - 128;
    }
    CatalogueSystemId::new(
        u8::try_from(class).expect("the bound keeps the class in u8"),
        coordinates,
        u32::try_from(index).expect("the bound keeps the index in u32"),
        u8::try_from(member).expect("the bound keeps the member in u8"),
    )
    .map_err(from_build)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::coords::CellSize;
    use crate::id::testing::random_valid_id;

    fn id(raw: u64) -> SystemId {
        SystemId::from_raw(raw).unwrap()
    }

    fn parse(text: &str) -> Result<Designation, ParseDesignationError> {
        text.parse()
    }

    #[test]
    fn the_table_examples_recomputed_by_hand() {
        // Each raw value was assembled field by field from the designation:
        // - H7K 4C0RFZ A-7: sectors H = 17, 7, K = 19; cell digits 4C = 140, 0R = 24, FZ = 511;
        //   stored x = 17 × 512 + 140 = 8,844, y = 3,608, z = 10,239, index 7.
        // - H7K F212 …: feature cell stored (17, 7, 19), feature 212; band C = 2, level 3, cell
        //   (8, 15, 2), index 40; or band 7, member 0.
        // - CENTRE A9-2HJ-12: band 0, level 9, cell (2, 17, 18), index 12.
        // - STREAM 87 A-5121-3.23-9: number 87, band 0, along 5,121, across 3 and 23, index 9.
        // - DWARF 1 B2-47C-3: number 1, band 1, level 2, cell (4, 7, 12), index 3.
        // - H7K K3-052-118/1: stored cell (17 × 8 + 0, 7 × 8 + 5, 19 × 8 + 2) = (136, 61, 154).
        let examples = [
            ("H7K 4C0RFZ A-7", 0x0228_C386_27FF_0007),
            ("H7K F212 C3-8F2-40", 0xE89E_606A_271E_4028),
            ("H7K F212 M0", 0xE89E_606A_7000_0000),
            ("CENTRE A9-2HJ-12", 0xF000_0000_9146_400C),
            ("CENTRE M0", 0xF000_0007_0000_0000),
            ("STREAM 87 A-5121-3.23-9", 0xF201_5C28_021A_E009),
            ("DWARF 1 B2-47C-3", 0xF400_0000_948F_8003),
            ("PIN 42", 0xF800_0000_0000_002A),
            ("H7K K3-052-118/1", 0xFC38_83D9_A000_0761),
        ];
        for (text, raw) in examples {
            assert_eq!(id(raw).designation().to_string(), text);
            assert_eq!(parse(text), Ok(Designation::of_system(id(raw))), "{text}");
        }
        let grid = SystemId::from_parts(
            Layer::A,
            GenCell::new(CellSize::Ly8, [652, -4_584, 2_047]).unwrap(),
            7,
        )
        .unwrap();
        assert_eq!(grid.raw(), 0x0228_C386_27FF_0007);
    }

    #[test]
    fn every_kind_round_trips_over_field_extremes() {
        let mut ids = Vec::new();
        for layer in Layer::ALL {
            let half = 65_536 / i32::try_from(layer.cell_size_ly()).unwrap();
            for c in [[-half; 3], [half - 1; 3], [0, -1, half - 1]] {
                let cell = GenCell::new(layer.cell_size(), c).unwrap();
                for index in [0, (1 << layer.index_bits()) - 1] {
                    ids.push(SystemId::from_parts(layer, cell, index).unwrap());
                }
            }
        }
        let slots = [
            MemberSlot::FeatureLevel { index: 0 },
            MemberSlot::FeatureLevel { index: 8_191 },
            MemberSlot::InCell {
                band: Layer::RoguePlanet,
                level: 7,
                cell: [15, 0, 15],
                index: 8_191,
            },
            MemberSlot::InCell {
                band: Layer::A,
                level: 0,
                cell: [0, 0, 0],
                index: 0,
            },
        ];
        for c in [[-16; 3], [15; 3]] {
            let feature = FeatureRef::new(FeatureCell::new(c).unwrap(), 16_383).unwrap();
            for slot in slots {
                ids.push(FeatureMemberId::new(feature, slot).unwrap().into());
            }
        }
        for slot in slots {
            ids.push(DwarfCoreMemberId::new(3, slot).unwrap().into());
        }
        ids.push(CentreMemberId::CENTRAL_BLACK_HOLE.into());
        ids.push(
            CentreMemberId::new(MemberSlot::InCell {
                band: Layer::E,
                level: 11,
                cell: [31, 31, 31],
                index: 8_191,
            })
            .unwrap()
            .into(),
        );
        ids.push(
            StreamMemberId::new(0, Layer::A, 0, [0, 0], 0)
                .unwrap()
                .into(),
        );
        ids.push(
            StreamMemberId::new(4_095, Layer::RoguePlanet, 16_383, [63, 63], 8_191)
                .unwrap()
                .into(),
        );
        ids.push(PinnedId::new(0).unwrap().into());
        ids.push(PinnedId::new(PinnedId::NUMBER_LIMIT - 1).unwrap().into());
        ids.push(CatalogueSystemId::new(0, [-128; 3], 0, 0).unwrap().into());
        ids.push(
            CatalogueSystemId::new(63, [127; 3], (1 << 24) - 1, 15)
                .unwrap()
                .into(),
        );
        for system in ids {
            let text = system.designation().to_string();
            assert_eq!(parse(&text), Ok(Designation::of_system(system)), "{text}");
            for index in [0, u16::MAX] {
                let body = BodyId::new(system, index);
                let text = body.designation().to_string();
                assert_eq!(
                    parse(&text).map(Designation::body),
                    Ok(Some(body)),
                    "{text}"
                );
            }
        }
    }

    #[test]
    fn random_ids_round_trip_and_designations_are_unique() {
        let mut lcg = Lcg::new(0xde51);
        let mut seen: BTreeMap<String, SystemId> = BTreeMap::new();
        let mut distinct = BTreeSet::new();
        for _ in 0..10_000 {
            let system = random_valid_id(&mut lcg);
            distinct.insert(system);
            let text = system.designation().to_string();
            assert_eq!(parse(&text), Ok(Designation::of_system(system)), "{text}");
            if let Some(other) = seen.insert(text.clone(), system) {
                assert_eq!(other, system, "{text} names two IDs");
            }
            let body = BodyId::new(system, u16::try_from(lcg.next_below(1 << 16)).unwrap());
            assert_eq!(
                parse(&body.designation().to_string()),
                Ok(body.designation())
            );
        }
        assert_eq!(seen.len(), distinct.len());
    }

    #[test]
    fn out_of_range_digits_are_rejected() {
        // Layer E keeps 5 bits of each axis below the sector: the first digit of a pair is 0.
        assert!(parse("GGG 0Z0000 E-0").is_ok());
        assert_eq!(
            parse("GGG 100000 E-0"),
            Err(ParseDesignationError::DigitOutOfRange)
        );
        // Layer A keeps 9 bits: the first digit is at most F (15).
        assert!(parse("GGG FZ0000 A-0").is_ok());
        assert_eq!(
            parse("GGG G00000 A-0"),
            Err(ParseDesignationError::DigitOutOfRange)
        );
        // Rogue planets keep 10 bits: every pair is valid.
        assert!(parse("GGG ZZZZZZ G-0").is_ok());
        // Feature cells are 0–15, levels 0–7; the centre's cells 0–31, levels 0–11.
        assert_eq!(
            parse("GGG F0 A0-G00-0"),
            Err(ParseDesignationError::DigitOutOfRange)
        );
        assert_eq!(
            parse("GGG F0 A8-000-0"),
            Err(ParseDesignationError::DigitOutOfRange)
        );
        assert!(parse("CENTRE AB-Z00-0").is_ok());
        assert_eq!(
            parse("CENTRE AC-000-0"),
            Err(ParseDesignationError::DigitOutOfRange)
        );
        // Catalogue cell digits are the low three bits.
        assert!(parse("GGG K0-777-0").is_ok());
        assert_eq!(
            parse("GGG K0-080-0"),
            Err(ParseDesignationError::DigitOutOfRange)
        );
    }

    #[test]
    fn other_rejections_have_their_own_variants() {
        let cases = [
            ("", ParseDesignationError::UnknownForm),
            ("GGG", ParseDesignationError::UnknownForm),
            ("GGG  000000 A-0", ParseDesignationError::UnknownForm),
            ("GGG 000000 A-0 x", ParseDesignationError::UnknownForm),
            ("GG 000000 A-0", ParseDesignationError::UnknownForm),
            ("GGG 00000 A-0", ParseDesignationError::UnknownForm),
            ("GGI 000000 A-0", ParseDesignationError::InvalidDigit),
            ("ggg 000000 A-0", ParseDesignationError::InvalidDigit),
            ("GGG 000000 H-0", ParseDesignationError::UnknownLayer),
            ("GGG 000000 A-01", ParseDesignationError::NotCanonical),
            ("GGG 000000 A-+1", ParseDesignationError::InvalidDigit),
            (
                "GGG 000000 A-65536",
                ParseDesignationError::NumberOutOfRange,
            ),
            (
                "GGG 000000 A-99999999999999999999999",
                ParseDesignationError::NumberOutOfRange,
            ),
            ("GGG K1-000-0/0", ParseDesignationError::NotCanonical),
            ("GGG K64-000-0", ParseDesignationError::NumberOutOfRange),
            ("GGG K1-000-0/16", ParseDesignationError::NumberOutOfRange),
            ("GGG F16384 M0", ParseDesignationError::NumberOutOfRange),
            ("GGG F1 M8192", ParseDesignationError::NumberOutOfRange),
            (
                "STREAM 4096 A-0-0.0-0",
                ParseDesignationError::NumberOutOfRange,
            ),
            (
                "STREAM 1 A-0-64.0-0",
                ParseDesignationError::NumberOutOfRange,
            ),
            ("STREAM 1 A-0-0-0", ParseDesignationError::UnknownForm),
            ("DWARF 4 M0", ParseDesignationError::NumberOutOfRange),
            (
                "PIN 288230376151711744",
                ParseDesignationError::NumberOutOfRange,
            ),
            ("PIN 042", ParseDesignationError::NotCanonical),
            (
                "GGG 000000 A-0 /65536",
                ParseDesignationError::NumberOutOfRange,
            ),
            ("GGG 000000 A-0 /", ParseDesignationError::UnknownForm),
            ("CENTRE M0 /01", ParseDesignationError::NotCanonical),
            (
                "GGG F1 A1-777-0",
                ParseDesignationError::Decode(DecodeSystemIdError::InnerCellOwnedByLowerLevel),
            ),
            (
                "CENTRE AB-GGG-0",
                ParseDesignationError::Decode(DecodeSystemIdError::InnerCellOwnedByLowerLevel),
            ),
        ];
        for (text, error) in cases {
            assert_eq!(parse(text), Err(error), "{text:?}");
        }
    }

    #[test]
    fn conversions_distinguish_systems_from_bodies() {
        let system = id(0xF800_0000_0000_002A);
        let body = BodyId::new(system, 9);
        let of_system = Designation::from(system);
        let of_body = Designation::from(body);
        assert_eq!(SystemId::try_from(&of_system), Ok(system));
        assert_eq!(
            SystemId::try_from(&of_body),
            Err(ConvertDesignationError::NamesBody)
        );
        assert_eq!(BodyId::try_from(&of_body), Ok(body));
        assert_eq!(
            BodyId::try_from(&of_system),
            Err(ConvertDesignationError::NamesSystem)
        );
        assert_eq!(of_body.system(), system);
        assert_eq!(of_body.to_string(), "PIN 42 /9");
    }
}
