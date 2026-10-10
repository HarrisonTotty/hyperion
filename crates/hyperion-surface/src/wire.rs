//! The coarse field's bulk payload: a sequence of self-contained little-endian blocks, each at most
//! [`MAX_BLOCK_BYTES`], carrying a survey's cells with their margin, the climate over them and the
//! craters that reach them (plan R09, Design notes 15 to 17).
//!
//! The server encodes the cells a client may hold with [`encode_payload`], whole or as the delta
//! since a cover the client already holds; R03's transport cuts the bytes into its binary frames.
//! The client splits them at block boundaries, decodes each block alone ([`decode_block`], or
//! [`decode_payload`] for the whole), and inserts the blocks into its
//! [`PartialField`](crate::field::PartialField), in any order: block 0 of every payload, delta or
//! whole, carries the whole [`FieldHeader`], so the first block a worker is given builds its field
//! (Design note 17). Every cell travels exact and whole, as the 21 bytes the field holds it in,
//! so the client's synthesis reads what the server's does to the bit.
//!
//! # A block
//!
//! Every integer is little-endian and every `f64` its IEEE 754 bits, little-endian. The first ten
//! bytes are the frame every format keeps, so that a block of any format can be split from its
//! neighbours by its length; the rest is format 1's ([`SURFACE_PAYLOAD_FORMAT`]).
//!
//! | Offset | Bytes | Contents                                                           |
//! | ------ | ----- | ------------------------------------------------------------------ |
//! | 0      | 4     | the magic, `HYSF` ([`BLOCK_MAGIC`])                                |
//! | 4      | 2     | the format, `u16`                                                  |
//! | 6      | 4     | the block's length in bytes, this header included, `u32`           |
//! | 10     | 4     | the generator version the field was computed under, `u32`          |
//! | 14     | 8     | the body's system, its raw 64-bit ID                               |
//! | 22     | 2     | the body's index in its system, `u16`                              |
//! | 24     | 1     | the field's level, 5 to 8                                          |
//! | 25     | 4     | the block's index in the payload, from 0, `u32`                    |
//! | 29     | 4     | the payload's block count, `u32`                                   |
//!
//! Then, in order:
//!
//! 1. In block 0 alone, the field header: its length in bytes, `u16`, then the parts of
//!    [`FieldHeaderParts`] in their declared order: the body as in the block's header (and the
//!    same), each `f64` and unit as 8 bytes, each one-byte code as a byte, the figure as a then c,
//!    the spectrum as β then V₁, the crater contract as N(>1 km), the screening (a tag, 0 for
//!    none, 1 for an atmosphere followed by its column mass and projectile density, 2 for a cutoff
//!    followed by its diameter), the gravity, the target factor and the impact velocity, and each
//!    `Option` (the impact velocity, the albedo scale) as a presence byte, 0 or 1, followed by
//!    its value when 1.
//! 2. The block's cover: its range count, `u32`, then each range as its first cell index and the
//!    index past its last, two `u32`, and its [`ResolutionCode`], a byte: the block's surveyed
//!    cells with their codes and its margin cells at [`ResolutionCode::NONE`], sorted, disjoint and
//!    canonical (no two adjacent ranges of one code), in [`cell_index`] order.
//! 3. A synthesis record for every cell of the cover, in the cover's order, 21 bytes each, the
//!    fields of [`SynthesisCell`] in their declared order: `elevation_mm` (`i32`),
//!    `boundary_distance_km` (`i16`), `plate`, `crust`, `boundary`, `boundary_obliquity`, `flow`
//!    (a byte each), `drainage` (`u16`), `steepness` (a byte), `water_surface_mm` (`i32`), and
//!    `ice`, `class` and `crater_state` (a byte each).
//! 4. A climate record for every climate cell over the cover's cells (each cell's parent, one level
//!    up, once, in index order), 50 bytes each, the fields of [`ClimateCell`] in their declared
//!    order: `sea_level_temperature` (`i16`), the twelve `month_anomaly` bytes, the twelve
//!    `month_precipitation` codes and the twelve winds, each an azimuth then a speed code.
//! 5. The craters that reach any of the block's cells, in the field's list order: their count,
//!    `u32`, then each crater's centre (three `f64`), diameter in metres (`f64`), morphology (a
//!    byte), age in thousands of millions of years (`f64`), degradation (a byte), and reach: its
//!    range count, `u32`, then each range's first index and the index past its last, two `u32`
//!    (a reach's code is always [`ResolutionCode::NONE`]).
//!
//! The block ends where its length says, with nothing after its craters. A crater that reaches
//! the cells of several blocks travels in each, whole, so that every block is self-contained; a
//! [`PartialField`](crate::field::PartialField) keeps one copy of it.
//!
//! Each fixed record's layout (the synthesis and climate records, the header's parts) is one
//! table in the private `form` module, from which its writer, its reader and its length are all
//! generated, so that a field added to a record, or a part to the header, is one line there; the
//! strides above are the tables' sums. A crater, whose reach has no fixed length, is written and
//! read by hand here (`put_crater`, `read_crater`, `CRATER_FIXED_BYTES`), and a field added to
//! [`CoarseCrater`] touches all three; the encoder's check of each block's length against its plan
//! catches a writer that disagrees with the sizes.
//!
//! [`cell_index`]: crate::field::cell_index

use hyperion_base::units::{Gigayears, Metres};

use crate::craters::BuildCraterParamsError;
use crate::field::{
    BodyRef, BuildCoverError, BuildFieldError, BuildFieldHeaderError, ClimateCell, CoarseCrater,
    CoarseField, CoarseLevel, Cover, CoverRange, DecodeFieldCodeError, FieldHeader,
    FieldHeaderParts, FieldView, Morphology, ResolutionCode, SYNTHESIS_MARGIN_CELLS, SynthesisCell,
    check_crater, check_synthesis_cell, crater_key_follows,
};
use crate::synth::BuildBandSpectrumError;
use form::{FixedWire, Reader, Wire};

mod form;

/// The version of the payload's layout, in every block's header: 1.
///
/// A decoder reads the formats it knows and refuses any other ([`DecodeBlockError::Format`]).
/// Every format keeps a block's first ten bytes (the magic, the format and the block's length), so
/// that blocks of any format can be split apart; the rest of the layout changes only with a new
/// format. The format belongs to `GENERATOR_VERSION` (Design note 17, open question 4), so a new
/// one is a generator-version change too, and a block also carries the generator version it was
/// computed under, which a decoder refuses unless it is its own
/// ([`DecodeBlockError::GeneratorVersion`]).
///
/// # Codes grow by appending
///
/// The one-byte codes of the closed sets are registries that grow without a new format: the
/// field's enums ([`Crust`], [`BoundaryKind`], [`FlowDirection`], [`Morphology`],
/// [`ClimateModelKind`], [`PrecipitationSource`]) and this layout's own tags (the screening's 0,
/// 1 and 2, and the presence bytes). A new variant takes the next unused code, and no code is ever
/// renumbered, reused or removed, so a code means the same in every build; the layout is
/// unchanged, so the format stays, while the generated output is new, so `GENERATOR_VERSION` is
/// bumped, and a payload of the older version is then refused by its generator version and
/// fetched again, never misread. A decoder that meets a code it does not know refuses the block
/// ([`DecodeBlockError::Code`] or [`DecodeBlockError::Tag`]) rather than misread it. The open
/// codes ([`SurfaceClass`], the crater state) are bytes whose meanings their owners assign, by the
/// same rule. The scaled codes ([`LogArea`], [`LogSteepness`], [`LogPrecipitation`], [`Wind`],
/// [`ResolutionCode`] and the linear steps) keep their scales: a new scale is a new format. The
/// golden file `tests/golden/wire/codes.golden` pins every code's decoded value, so a renumbered
/// code fails it and an appended one only extends it.
///
/// [`Crust`]: crate::field::Crust
/// [`BoundaryKind`]: crate::field::BoundaryKind
/// [`FlowDirection`]: crate::field::FlowDirection
/// [`ClimateModelKind`]: crate::field::ClimateModelKind
/// [`PrecipitationSource`]: crate::field::PrecipitationSource
/// [`SurfaceClass`]: crate::field::SurfaceClass
/// [`LogArea`]: crate::field::LogArea
/// [`LogSteepness`]: crate::field::LogSteepness
/// [`LogPrecipitation`]: crate::field::LogPrecipitation
/// [`Wind`]: crate::field::Wind
pub const SURFACE_PAYLOAD_FORMAT: u16 = 1;

/// The largest block, bytes, its header included: 1 MiB, the brainstorm's "surveyed-region chunks
/// of at most 1 MB" (Design note 17), so that a worker can be posted one block at a time.
pub const MAX_BLOCK_BYTES: usize = 1 << 20;

/// The first four bytes of every block, `HYSF`: a HYPERION surface field.
pub const BLOCK_MAGIC: [u8; 4] = *b"HYSF";

/// The bytes of a block's fixed header (the table of the module documentation).
pub const BLOCK_HEADER_BYTES: usize = 33;

/// The bytes of the frame every format keeps: the magic, the format and the length.
const FRAME_BYTES: usize = 10;

/// The bytes of one cover range: two `u32` and a code.
const COVER_RANGE_BYTES: usize = 9;

/// The bytes of a crater with an empty reach: its centre, diameter, morphology, age, degradation
/// and reach count, as [`put_crater`] writes them.
const CRATER_FIXED_BYTES: usize = <[f64; 3]>::BYTES
    + Metres::BYTES
    + Morphology::BYTES
    + Gigayears::BYTES
    + u8::BYTES
    + u32::BYTES;

/// The bytes of one range of a crater's reach: two `u32`.
const REACH_RANGE_BYTES: usize = 8;

/// One block of a payload, decoded and checked: its place in the payload, the body and level it is
/// of, the field header if it is block 0, and the cells, climate records and craters it carries.
///
/// A decoded block holds only what its bytes say, each record checked against the rules that need
/// no header (codes, water and ground, boundaries, the craters' centres, ages, reaches and order);
/// [`PartialField::insert`](crate::field::PartialField::insert) checks the rest against the
/// field's header as it merges the block.
#[derive(Clone, PartialEq)]
pub struct DecodedBlock {
    index: u32,
    count: u32,
    body: BodyRef,
    level: CoarseLevel,
    header: Option<FieldHeader>,
    cover: Cover,
    cells: Vec<SynthesisCell>,
    climate_indices: Vec<u32>,
    climate: Vec<ClimateCell>,
    craters: Vec<CoarseCrater>,
    crater_centres: Vec<u32>,
}

impl DecodedBlock {
    /// The block's index in its payload, from 0.
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// The number of blocks in its payload.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// The body the field is of.
    #[must_use]
    pub fn body(&self) -> BodyRef {
        self.body
    }

    /// The field's level.
    #[must_use]
    pub fn level(&self) -> CoarseLevel {
        self.level
    }

    /// The field's header, which block 0 of every payload carries, and `None` in every other.
    #[must_use]
    pub fn header(&self) -> Option<&FieldHeader> {
        self.header.as_ref()
    }

    /// The block's cells as a cover: each surveyed cell with its resolution code and each margin
    /// cell at [`ResolutionCode::NONE`].
    #[must_use]
    pub fn cover(&self) -> &Cover {
        &self.cover
    }

    /// The synthesis record of each cell of the [`cover`](Self::cover), in its order.
    #[must_use]
    pub fn cells(&self) -> &[SynthesisCell] {
        &self.cells
    }

    /// The climate-layer index of each climate record ([`climate`](Self::climate)): the parents of
    /// the cover's cells, each once, in index order.
    #[must_use]
    pub fn climate_indices(&self) -> &[u32] {
        &self.climate_indices
    }

    /// The climate records over the block's cells, beside
    /// [`climate_indices`](Self::climate_indices).
    #[must_use]
    pub fn climate(&self) -> &[ClimateCell] {
        &self.climate
    }

    /// The craters that reach any of the block's cells, in the field's list order.
    #[must_use]
    pub fn craters(&self) -> &[CoarseCrater] {
        &self.craters
    }

    /// The [`cell_index`](crate::field::cell_index) of each crater's centre's cell, beside
    /// [`craters`](Self::craters).
    #[must_use]
    pub(crate) fn crater_centres(&self) -> &[u32] {
        &self.crater_centres
    }
}

impl std::fmt::Debug for DecodedBlock {
    /// The block's header and its counts: a block holds up to tens of thousands of records.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodedBlock")
            .field("index", &self.index)
            .field("count", &self.count)
            .field("body", &self.body)
            .field("level", &self.level)
            .field("header", &self.header)
            .field("cells", &self.cells.len())
            .field("climate", &self.climate.len())
            .field("craters", &self.craters.len())
            .finish_non_exhaustive()
    }
}

/// Why a block was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeBlockError {
    /// The bytes end before the block, or a part of it, does.
    Truncated {
        /// The bytes needed.
        needed: usize,
        /// The bytes there are.
        available: usize,
    },
    /// The block's first four bytes are not [`BLOCK_MAGIC`].
    Magic([u8; 4]),
    /// The block is of a format this build does not read.
    Format(u16),
    /// The block's length is beyond [`MAX_BLOCK_BYTES`].
    Oversize {
        /// The length the block declares.
        length: u32,
    },
    /// The block's length is shorter than its fixed header.
    Undersize {
        /// The length the block declares.
        length: u32,
    },
    /// The bytes go on past the one block they were to hold, or the block's parts end before its
    /// length does.
    TrailingBytes {
        /// The block's length.
        length: usize,
        /// The bytes its parts used, or the bytes given.
        used: usize,
    },
    /// The field was computed under another generator version than this build's.
    GeneratorVersion {
        /// The block's.
        found: u32,
        /// This build's.
        expected: u32,
    },
    /// The level is not a coarse field's, 5 to 8.
    Level(u8),
    /// The block's index is not below its payload's block count.
    Index {
        /// The block's index.
        index: u32,
        /// The block count.
        count: u32,
    },
    /// The field header's parts do not fill its declared length exactly: they end before it, or
    /// run past it.
    HeaderLength {
        /// The declared length.
        declared: u16,
    },
    /// The field header's parts are refused.
    Header(BuildFieldHeaderError),
    /// The field header's spectrum is refused.
    Spectrum(BuildBandSpectrumError),
    /// The field header's crater contract is refused.
    CraterParams(BuildCraterParamsError),
    /// The field header is of another body than the block.
    HeaderBody {
        /// The header's body.
        header: BodyRef,
        /// The block's.
        block: BodyRef,
    },
    /// The field header is of another level than the block.
    HeaderLevel {
        /// The header's level, from its radius.
        header: CoarseLevel,
        /// The block's.
        block: CoarseLevel,
    },
    /// A tag of this layout (a screening's, or a presence byte) is not one of its codes.
    Tag {
        /// What the tag selects.
        part: &'static str,
        /// The byte.
        code: u8,
    },
    /// A field enum's byte is not one of its codes.
    Code(DecodeFieldCodeError),
    /// A cover range is empty, or begins before the previous one ends.
    Cover(BuildCoverError),
    /// A cover is not canonical: two adjacent ranges share a code.
    CoverNotCanonical,
    /// A cover holds an index past the level's cells.
    CoverOutsideField {
        /// The index past the cover's last cell.
        end: u32,
    },
    /// A record or a crater breaks a rule of its type.
    Record(BuildFieldError),
    /// A crater reaches none of the block's cells, which a block carries only the craters of.
    CraterMissesBlock {
        /// The crater's place in the block's list.
        crater: u32,
    },
}

impl std::fmt::Display for DecodeBlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated { needed, available } => {
                write!(
                    f,
                    "the block needs {needed} bytes but {available} are given"
                )
            }
            Self::Magic(m) => write!(f, "{m:02x?} is not a surface payload block's magic"),
            Self::Format(v) => write!(
                f,
                "payload format {v} is not this build's {SURFACE_PAYLOAD_FORMAT}"
            ),
            Self::Oversize { length } => {
                write!(f, "a block of {length} bytes is beyond {MAX_BLOCK_BYTES}")
            }
            Self::Undersize { length } => write!(
                f,
                "a block of {length} bytes is shorter than its {BLOCK_HEADER_BYTES}-byte header"
            ),
            Self::TrailingBytes { length, used } => {
                write!(
                    f,
                    "a block of {length} bytes is not the {used} given or used"
                )
            }
            Self::GeneratorVersion { found, expected } => write!(
                f,
                "a field of generator version {found} is not this build's {expected}"
            ),
            Self::Level(level) => write!(f, "level {level} is not a coarse field's"),
            Self::Index { index, count } => {
                write!(
                    f,
                    "block index {index} is not below the block count {count}"
                )
            }
            Self::HeaderLength { declared } => {
                write!(
                    f,
                    "the field header's parts do not fill its {declared} bytes"
                )
            }
            Self::Header(e) => write!(f, "the field header is refused: {e}"),
            Self::Spectrum(e) => write!(f, "the field header's spectrum is refused: {e}"),
            Self::CraterParams(e) => {
                write!(f, "the field header's crater contract is refused: {e}")
            }
            Self::HeaderBody { header, block } => write!(
                f,
                "a header of body {} of system {:#x} is in a block of body {} of system {:#x}",
                header.body_index(),
                header.raw_system_id(),
                block.body_index(),
                block.raw_system_id()
            ),
            Self::HeaderLevel { header, block } => write!(
                f,
                "a field header of level {} is in a block of level {}",
                header.get(),
                block.get()
            ),
            Self::Tag { part, code } => write!(f, "{code} is not a {part} tag"),
            Self::Code(e) => write!(f, "{e}"),
            Self::Cover(e) => write!(f, "a cover is refused: {e}"),
            Self::CoverNotCanonical => write!(f, "a cover has two adjacent ranges of one code"),
            Self::CoverOutsideField { end } => {
                write!(f, "a cover reaches index {end}, past the field's cells")
            }
            Self::Record(e) => write!(f, "a record is refused: {e}"),
            Self::CraterMissesBlock { crater } => {
                write!(f, "crater {crater} reaches none of the block's cells")
            }
        }
    }
}

impl std::error::Error for DecodeBlockError {}

/// Why a payload was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodePayloadError {
    /// The payload is empty: every payload has at least block 0.
    Empty,
    /// The block at `block`, its place in the payload from 0, is refused.
    Block {
        /// The block's place.
        block: usize,
        /// Why.
        error: DecodeBlockError,
    },
    /// The block at `block` is not the payload's next: its index is not its place, or its count
    /// is not block 0's.
    Sequence {
        /// The block's place.
        block: usize,
        /// Its index.
        index: u32,
        /// Its count.
        count: u32,
    },
    /// The block at `block` is of another body than block 0.
    WrongBody {
        /// The block's place.
        block: usize,
        /// Its body.
        found: BodyRef,
        /// Block 0's.
        expected: BodyRef,
    },
    /// The block at `block` is at another level than block 0.
    WrongLevel {
        /// The block's place.
        block: usize,
        /// Its level.
        found: CoarseLevel,
        /// Block 0's.
        expected: CoarseLevel,
    },
    /// The payload ends at a block boundary before its last block.
    MissingBlocks {
        /// The block count the blocks declare.
        count: u32,
        /// The blocks there are.
        found: usize,
    },
}

impl std::fmt::Display for DecodePayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "the payload is empty"),
            Self::Block { block, error } => write!(f, "block {block} is refused: {error}"),
            Self::Sequence {
                block,
                index,
                count,
            } => write!(
                f,
                "block {block} is block {index} of {count}, out of the payload's sequence"
            ),
            Self::WrongBody {
                block,
                found,
                expected,
            } => write!(
                f,
                "block {block} is of body {} of system {:#x}, not block 0's {} of {:#x}",
                found.body_index(),
                found.raw_system_id(),
                expected.body_index(),
                expected.raw_system_id()
            ),
            Self::WrongLevel {
                block,
                found,
                expected,
            } => write!(
                f,
                "block {block} is at level {}, not block 0's {}",
                found.get(),
                expected.get()
            ),
            Self::MissingBlocks { count, found } => {
                write!(f, "the payload ends after {found} of its {count} blocks")
            }
        }
    }
}

impl std::error::Error for DecodePayloadError {}

/// The payload of `field`'s cells that a client with `cover` may hold: the cells of `cover`, with
/// their codes, and their margin of [`SYNTHESIS_MARGIN_CELLS`] king moves at
/// [`ResolutionCode::NONE`] ([`Cover::with_margin`]), with the climate records over them and every
/// crater that reaches them, in blocks of at most [`MAX_BLOCK_BYTES`] split in
/// [`cell_index`](crate::field::cell_index) order.
///
/// `cover` is the surveyed cells with their codes, at the field's level (the server converts its
/// coverage to it), without their margin, which is added here. With `since`, the survey cover the
/// client's earlier payload was encoded from (its codes, again without its margin), the payload is
/// a delta: the cells of `cover` and its margin that `since` and its margin did not hold, or held
/// with another code, so that a cell surveyed again more finely, or a margin cell since surveyed,
/// is sent again with its new code. Block 0 of every payload, delta or whole, carries the field's
/// header; a payload with no cell to send is block 0 alone. The same arguments give the same
/// bytes.
///
/// # Panics
///
/// If `cover` or `since` holds an index of no cell of the field's level, or if one cell's records
/// and the craters that reach it would not fit an empty block, which would take thousands of
/// basins overlapping one cell.
#[must_use]
pub fn encode_payload(field: &CoarseField, cover: &Cover, since: Option<&Cover>) -> Vec<u8> {
    encode_within(field, cover, since, SYNTHESIS_MARGIN_CELLS, MAX_BLOCK_BYTES)
}

/// Decodes a payload, block after block: every block checked, in sequence from block 0, of one
/// body and level, and the payload whole.
///
/// # Errors
///
/// [`DecodePayloadError::Empty`] for no bytes; [`DecodePayloadError::Block`] for a block that
/// [`decode_block`] would refuse, a truncated last block among them;
/// [`DecodePayloadError::Sequence`], [`DecodePayloadError::WrongBody`] or
/// [`DecodePayloadError::WrongLevel`] for a block that does not follow block 0; and
/// [`DecodePayloadError::MissingBlocks`] for a payload that ends before its last block. It never
/// panics, whatever the bytes.
pub fn decode_payload(bytes: &[u8]) -> Result<Vec<DecodedBlock>, DecodePayloadError> {
    if bytes.is_empty() {
        return Err(DecodePayloadError::Empty);
    }
    let mut blocks: Vec<DecodedBlock> = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let place = blocks.len();
        let refused = |error| DecodePayloadError::Block {
            block: place,
            error,
        };
        let length = block_length(rest).map_err(refused)?;
        let (this, after) = rest.split_at(length);
        let block = parse_block(this).map_err(refused)?;
        let first = blocks.first().unwrap_or(&block);
        if u32::try_from(place).ok() != Some(block.index) || block.count != first.count {
            return Err(DecodePayloadError::Sequence {
                block: place,
                index: block.index,
                count: block.count,
            });
        }
        if block.body != first.body {
            return Err(DecodePayloadError::WrongBody {
                block: place,
                found: block.body,
                expected: first.body,
            });
        }
        if block.level != first.level {
            return Err(DecodePayloadError::WrongLevel {
                block: place,
                found: block.level,
                expected: first.level,
            });
        }
        blocks.push(block);
        rest = after;
    }
    let count = blocks.first().map_or(0, |b| b.count);
    if u32::try_from(blocks.len()).ok() != Some(count) {
        return Err(DecodePayloadError::MissingBlocks {
            count,
            found: blocks.len(),
        });
    }
    Ok(blocks)
}

/// Decodes one block, whose bytes are exactly `bytes`: what a worker is posted, one block at a
/// time.
///
/// # Errors
///
/// - [`DecodeBlockError::Truncated`] if the bytes end before the block or a part of it does, and
///   [`DecodeBlockError::TrailingBytes`] if they go on past the block, or its parts end before its
///   length does;
/// - [`DecodeBlockError::Magic`], [`DecodeBlockError::Format`] or
///   [`DecodeBlockError::GeneratorVersion`] for a block this build does not read;
/// - [`DecodeBlockError::Oversize`] or [`DecodeBlockError::Undersize`] for a length beyond
///   [`MAX_BLOCK_BYTES`] or short of the fixed header;
/// - [`DecodeBlockError::Level`] or [`DecodeBlockError::Index`] for a level that is not a coarse
///   field's or an index not below the count;
/// - in block 0, [`DecodeBlockError::HeaderLength`] for header parts that do not fill their
///   length, [`DecodeBlockError::Tag`] for an unknown screening or presence byte,
///   [`DecodeBlockError::Header`], [`DecodeBlockError::Spectrum`] or
///   [`DecodeBlockError::CraterParams`] for parts out of their ranges, and
///   [`DecodeBlockError::HeaderBody`] or [`DecodeBlockError::HeaderLevel`] for a header of another
///   body or level than the block;
/// - [`DecodeBlockError::Cover`], [`DecodeBlockError::CoverNotCanonical`] or
///   [`DecodeBlockError::CoverOutsideField`] for a cover or reach that is not sorted, disjoint and
///   canonical within the level's cells;
/// - [`DecodeBlockError::Code`] for an unknown enum code, [`DecodeBlockError::Record`] for a record
///   or crater that breaks a rule of its type or a crater out of the list's order, and
///   [`DecodeBlockError::CraterMissesBlock`] for a crater that reaches none of the block's cells.
///
/// It never panics, whatever the bytes.
pub fn decode_block(bytes: &[u8]) -> Result<DecodedBlock, DecodeBlockError> {
    let length = block_length(bytes)?;
    if length != bytes.len() {
        return Err(DecodeBlockError::TrailingBytes {
            length,
            used: bytes.len(),
        });
    }
    parse_block(bytes)
}

// ---------------------------------------------------------------------------------------------
// Encoding

/// The cells, codes and craters planned for one block.
struct BlockPlan {
    /// The block's bytes so far.
    bytes: usize,
    /// Its cover: (first index, index past the last, code), sorted, disjoint and canonical.
    ranges: Vec<(u32, u32, ResolutionCode)>,
    /// The climate cell of its last cell, whose record it carries already.
    last_parent: Option<u32>,
    /// Its craters, by their place in the field's list.
    craters: Vec<u32>,
}

impl BlockPlan {
    /// An empty block whose fixed parts take `bytes`.
    #[must_use]
    fn new(bytes: usize) -> Self {
        Self {
            bytes,
            ranges: Vec::new(),
            last_parent: None,
            craters: Vec::new(),
        }
    }

    /// The bytes that adding `cell`, of code `code`, reached by the craters `reaching`, would add:
    /// a cover range unless it extends the last, its record, a climate record unless its parent's
    /// is in already, and each crater not yet in block `id`.
    #[must_use]
    fn cost(
        &self,
        cell: u32,
        code: ResolutionCode,
        reaching: &[u32],
        id: u32,
        in_block: &[u32],
        crater_bytes: &[usize],
    ) -> usize {
        let range = match self.ranges.last() {
            Some(&(_, end, c)) if end == cell && c == code => 0,
            _ => COVER_RANGE_BYTES,
        };
        let climate = if self.last_parent == Some(cell >> 2) {
            0
        } else {
            ClimateCell::BYTES
        };
        let craters: usize = reaching
            .iter()
            .filter(|&&k| in_block[slot(k)] != id)
            .map(|&k| crater_bytes[slot(k)])
            .sum();
        range + SynthesisCell::BYTES + climate + craters
    }

    /// Adds `cell`, of code `code` and reached by `reaching`, to block `id`, at `cost` bytes.
    fn add(
        &mut self,
        cell: u32,
        code: ResolutionCode,
        reaching: &[u32],
        id: u32,
        in_block: &mut [u32],
        cost: usize,
    ) {
        match self.ranges.last_mut() {
            Some((_, end, c)) if *end == cell && *c == code => *end = cell + 1,
            _ => self.ranges.push((cell, cell + 1, code)),
        }
        self.last_parent = Some(cell >> 2);
        for &k in reaching {
            if in_block[slot(k)] != id {
                in_block[slot(k)] = id;
                self.craters.push(k);
            }
        }
        self.bytes += cost;
    }
}

/// A `u32` index as a slot of a vector.
#[must_use]
fn slot(index: u32) -> usize {
    usize::try_from(index).expect("a cell or crater index fits a usize")
}

/// The code each cell had in a cover, read in increasing cell order.
struct CodeCursor<'a> {
    ranges: &'a [CoverRange],
    at: usize,
}

impl CodeCursor<'_> {
    /// The code of `cell`, which is not below any cell asked before, or `None` if the cover does
    /// not hold it.
    fn code(&mut self, cell: u32) -> Option<ResolutionCode> {
        while self.ranges.get(self.at).is_some_and(|r| r.end() <= cell) {
            self.at += 1;
        }
        self.ranges
            .get(self.at)
            .filter(|r| r.start() <= cell)
            .map(|r| r.code())
    }
}

/// [`encode_payload`] with a margin of `margin` king moves and blocks of at most `max_block` bytes.
pub(crate) fn encode_within(
    field: &CoarseField,
    cover: &Cover,
    since: Option<&Cover>,
    margin: u8,
    max_block: usize,
) -> Vec<u8> {
    let header = field.header();
    let level = header.level();
    let held = cover.with_margin(level, margin);
    let had = since.map(|s| s.with_margin(level, margin));
    let mut header_section = Vec::new();
    header.parts().put(&mut header_section);
    let crater_bytes: Vec<usize> = field
        .craters()
        .iter()
        .map(|c| CRATER_FIXED_BYTES + REACH_RANGE_BYTES * c.reach.ranges().len())
        .collect();
    let fixed = |index: usize| {
        let header = if index == 0 {
            2 + header_section.len()
        } else {
            0
        };
        BLOCK_HEADER_BYTES + header + 4 + 4
    };
    let mut in_block = vec![u32::MAX; field.craters().len()];
    let mut plans: Vec<BlockPlan> = Vec::new();
    let mut plan = BlockPlan::new(fixed(0));
    let mut had = had.as_ref().map(|h| CodeCursor {
        ranges: h.ranges(),
        at: 0,
    });
    for range in held.ranges() {
        let code = range.code();
        for cell in range.start()..range.end() {
            if had.as_mut().is_some_and(|h| h.code(cell) == Some(code)) {
                continue;
            }
            let reaching = field.reaching_indices(cell);
            loop {
                let id = u32::try_from(plans.len()).expect("a field's blocks are fewer than 2³²");
                let cost = plan.cost(cell, code, reaching, id, &in_block, &crater_bytes);
                if plan.bytes + cost <= max_block {
                    plan.add(cell, code, reaching, id, &mut in_block, cost);
                    break;
                }
                assert!(
                    !plan.ranges.is_empty(),
                    "cell {cell}'s records and the craters reaching it exceed a block of \
                     {max_block} bytes"
                );
                let next = BlockPlan::new(fixed(plans.len() + 1));
                plans.push(std::mem::replace(&mut plan, next));
            }
        }
    }
    plans.push(plan);
    let count = u32::try_from(plans.len()).expect("a field's blocks are fewer than 2³²");
    let mut out = Vec::with_capacity(plans.iter().map(|p| p.bytes).sum());
    for (index, plan) in (0_u32..).zip(&mut plans) {
        let start = out.len();
        let section = (index == 0).then_some(&header_section[..]);
        write_block(&mut out, field, plan, index, count, section);
        let length = out.len() - start;
        assert!(
            length == plan.bytes && length <= max_block,
            "block {index} is {length} bytes, planned {} of at most {max_block}",
            plan.bytes
        );
        let length = u32::try_from(length).expect("a block is at most 1 MiB");
        out[start + 6..start + FRAME_BYTES].copy_from_slice(&length.to_le_bytes());
    }
    out
}

/// Writes block `index` of `count` of `field`'s payload as `plan` lays it out, its length left 0
/// for the caller to fill, and, in block 0, the field header's `section`.
fn write_block(
    out: &mut Vec<u8>,
    field: &CoarseField,
    plan: &mut BlockPlan,
    index: u32,
    count: u32,
    section: Option<&[u8]>,
) {
    let header = field.header();
    out.extend_from_slice(&BLOCK_MAGIC);
    SURFACE_PAYLOAD_FORMAT.put(out);
    0_u32.put(out);
    crate::generator_version().put(out);
    header.body().put(out);
    header.level().get().put(out);
    index.put(out);
    count.put(out);
    if let Some(section) = section {
        u16::try_from(section.len())
            .expect("a field header is a few hundred bytes")
            .put(out);
        out.extend_from_slice(section);
    }
    put_count(out, plan.ranges.len());
    for &(first, end, code) in &plan.ranges {
        first.put(out);
        end.put(out);
        code.get().put(out);
    }
    let cells = || plan.ranges.iter().flat_map(|&(first, end, _)| first..end);
    for cell in cells() {
        field.synthesis()[slot(cell)].put(out);
    }
    let mut parent = None;
    for cell in cells() {
        if parent != Some(cell >> 2) {
            parent = Some(cell >> 2);
            field.climate_layer()[slot(cell >> 2)].put(out);
        }
    }
    plan.craters.sort_unstable();
    put_count(out, plan.craters.len());
    for &k in &plan.craters {
        put_crater(out, &field.craters()[slot(k)]);
    }
}

/// Writes a count of a block's parts as a `u32`.
fn put_count(out: &mut Vec<u8>, count: usize) {
    u32::try_from(count)
        .expect("a block of at most 1 MiB holds fewer than 2³² parts")
        .put(out);
}

/// Writes a crater: its centre, diameter, morphology, age and degradation, then its reach as
/// ranges without codes.
fn put_crater(out: &mut Vec<u8>, crater: &CoarseCrater) {
    let CoarseCrater {
        centre,
        diameter,
        morphology,
        age,
        degradation,
        reach,
    } = crater;
    centre.put(out);
    diameter.put(out);
    morphology.put(out);
    age.put(out);
    degradation.put(out);
    put_count(out, reach.ranges().len());
    for range in reach.ranges() {
        range.start().put(out);
        range.end().put(out);
    }
}

// ---------------------------------------------------------------------------------------------
// Decoding

/// The length of the block at the start of `bytes`, from its frame, checked: its magic and
/// format are this build's, it is no longer than [`MAX_BLOCK_BYTES`] nor shorter than its fixed
/// header, and `bytes` hold it whole.
fn block_length(bytes: &[u8]) -> Result<usize, DecodeBlockError> {
    let mut frame = Reader::new(bytes);
    let magic = frame.array::<4>()?;
    if magic != BLOCK_MAGIC {
        return Err(DecodeBlockError::Magic(magic));
    }
    let format = u16::read(&mut frame)?;
    if format != SURFACE_PAYLOAD_FORMAT {
        return Err(DecodeBlockError::Format(format));
    }
    let declared = u32::read(&mut frame)?;
    let length = usize::try_from(declared)
        .ok()
        .filter(|&l| l <= MAX_BLOCK_BYTES)
        .ok_or(DecodeBlockError::Oversize { length: declared })?;
    if length < BLOCK_HEADER_BYTES {
        return Err(DecodeBlockError::Undersize { length: declared });
    }
    if length > bytes.len() {
        return Err(DecodeBlockError::Truncated {
            needed: length,
            available: bytes.len(),
        });
    }
    Ok(length)
}

/// Decodes the block that is exactly `bytes`, whose frame [`block_length`] has checked.
fn parse_block(bytes: &[u8]) -> Result<DecodedBlock, DecodeBlockError> {
    let mut r = Reader::new(bytes);
    r.take(FRAME_BYTES)?;
    let generator = u32::read(&mut r)?;
    if generator != crate::generator_version() {
        return Err(DecodeBlockError::GeneratorVersion {
            found: generator,
            expected: crate::generator_version(),
        });
    }
    let body = BodyRef::read(&mut r)?;
    let level_code = u8::read(&mut r)?;
    let level = CoarseLevel::new(level_code).ok_or(DecodeBlockError::Level(level_code))?;
    let index = u32::read(&mut r)?;
    let count = u32::read(&mut r)?;
    if index >= count {
        return Err(DecodeBlockError::Index { index, count });
    }
    let header = if index == 0 {
        Some(read_header(&mut r, body, level)?)
    } else {
        None
    };
    let cover = read_cover(&mut r, level)?;
    r.need(cover.cell_count(), SynthesisCell::BYTES)?;
    let mut cells = Vec::with_capacity(checked_capacity(cover.cell_count()));
    for cell in cover.cells() {
        let record = SynthesisCell::read(&mut r)?;
        check_synthesis_cell(cell, &record).map_err(DecodeBlockError::Record)?;
        cells.push(record);
    }
    let mut climate_indices: Vec<u32> = cover.cells().map(|cell| cell >> 2).collect();
    climate_indices.dedup();
    r.need(
        u64::try_from(climate_indices.len()).unwrap_or(u64::MAX),
        ClimateCell::BYTES,
    )?;
    let mut climate = Vec::with_capacity(climate_indices.len());
    for _ in &climate_indices {
        climate.push(ClimateCell::read(&mut r)?);
    }
    let crater_count = u32::read(&mut r)?;
    r.need(u64::from(crater_count), CRATER_FIXED_BYTES)?;
    let mut craters: Vec<CoarseCrater> = Vec::with_capacity(checked_capacity(crater_count.into()));
    let mut crater_centres: Vec<u32> = Vec::with_capacity(craters.capacity());
    for k in 0..crater_count {
        let crater = read_crater(&mut r)?;
        let centre =
            check_crater(level, Metres::ZERO, k, &crater).map_err(DecodeBlockError::Record)?;
        if !covers_meet(&crater.reach, &cover) {
            return Err(DecodeBlockError::CraterMissesBlock { crater: k });
        }
        let key = (centre, crater.diameter);
        let previous = crater_centres.last().zip(craters.last());
        if previous.is_some_and(|(&cell, before)| !crater_key_follows((cell, before.diameter), key))
        {
            return Err(DecodeBlockError::Record(BuildFieldError::CratersUnsorted {
                crater: k,
            }));
        }
        craters.push(crater);
        crater_centres.push(centre);
    }
    if !r.is_empty() {
        return Err(DecodeBlockError::TrailingBytes {
            length: bytes.len(),
            used: r.position(),
        });
    }
    Ok(DecodedBlock {
        index,
        count,
        body,
        level,
        header,
        cover,
        cells,
        climate_indices,
        climate,
        craters,
        crater_centres,
    })
}

/// Whether two covers hold a cell in common.
#[must_use]
fn covers_meet(a: &Cover, b: &Cover) -> bool {
    let (a, b) = (a.ranges(), b.ranges());
    let (mut i, mut j) = (0, 0);
    while let (Some(x), Some(y)) = (a.get(i), b.get(j)) {
        if x.end() <= y.start() {
            i += 1;
        } else if y.end() <= x.start() {
            j += 1;
        } else {
            return true;
        }
    }
    false
}

/// Whether two headers have the same wire form, bit for bit, where `==` would take a −0.0 for a
/// 0.0: a [`PartialField`](crate::field::PartialField) keeps the first of two that are the same.
#[must_use]
pub(crate) fn same_header(a: &FieldHeader, b: &FieldHeader) -> bool {
    let (mut x, mut y) = (Vec::new(), Vec::new());
    a.parts().put(&mut x);
    b.parts().put(&mut y);
    x == y
}

/// Whether two craters have the same wire form, bit for bit, as [`same_header`].
#[must_use]
pub(crate) fn same_crater(a: &CoarseCrater, b: &CoarseCrater) -> bool {
    let (mut x, mut y) = (Vec::new(), Vec::new());
    put_crater(&mut x, a);
    put_crater(&mut y, b);
    x == y
}

/// A count already checked against a block's bytes, as a capacity.
#[must_use]
fn checked_capacity(count: u64) -> usize {
    usize::try_from(count).expect("a count checked against a block's bytes fits a usize")
}

/// Reads the field header's section of a block of `body` at `level`: its length, then its parts'
/// table, validated, of that body and level.
fn read_header(
    r: &mut Reader<'_>,
    body: BodyRef,
    level: CoarseLevel,
) -> Result<FieldHeader, DecodeBlockError> {
    let declared = u16::read(r)?;
    let mut section = Reader::new(r.take(usize::from(declared))?);
    let parts = FieldHeaderParts::read(&mut section).map_err(|e| match e {
        DecodeBlockError::Truncated { .. } => DecodeBlockError::HeaderLength { declared },
        other => other,
    })?;
    if !section.is_empty() {
        return Err(DecodeBlockError::HeaderLength { declared });
    }
    if parts.body != body {
        return Err(DecodeBlockError::HeaderBody {
            header: parts.body,
            block: body,
        });
    }
    let header = FieldHeader::new(parts).map_err(DecodeBlockError::Header)?;
    if header.level() != level {
        return Err(DecodeBlockError::HeaderLevel {
            header: header.level(),
            block: level,
        });
    }
    Ok(header)
}

/// Reads ranges into a cover, which must be canonical: its ranges as given, none merged.
fn read_ranges(
    r: &mut Reader<'_>,
    each: usize,
    mut range: impl FnMut(&mut Reader<'_>) -> Result<CoverRange, DecodeBlockError>,
) -> Result<Cover, DecodeBlockError> {
    let count = u32::read(r)?;
    r.need(u64::from(count), each)?;
    let mut ranges = Vec::with_capacity(checked_capacity(count.into()));
    for _ in 0..count {
        ranges.push(range(r)?);
    }
    let cover = Cover::from_ranges(ranges).map_err(DecodeBlockError::Cover)?;
    if u32::try_from(cover.ranges().len()).ok() != Some(count) {
        return Err(DecodeBlockError::CoverNotCanonical);
    }
    Ok(cover)
}

/// Reads a block's cover, which must lie within the level's cells.
fn read_cover(r: &mut Reader<'_>, level: CoarseLevel) -> Result<Cover, DecodeBlockError> {
    let cover = read_ranges(r, COVER_RANGE_BYTES, |r| {
        let (start, end) = (u32::read(r)?, u32::read(r)?);
        CoverRange::new(start, end, ResolutionCode::new(u8::read(r)?))
            .map_err(DecodeBlockError::Cover)
    })?;
    if let Some(last) = cover.ranges().last()
        && last.end() > level.cell_count()
    {
        return Err(DecodeBlockError::CoverOutsideField { end: last.end() });
    }
    Ok(cover)
}

/// Reads a crater, its reach a cover of [`ResolutionCode::NONE`].
fn read_crater(r: &mut Reader<'_>) -> Result<CoarseCrater, DecodeBlockError> {
    let centre = <[f64; 3]>::read(r)?;
    let diameter = Metres::read(r)?;
    let morphology = Morphology::read(r)?;
    let age = Gigayears::read(r)?;
    let degradation = u8::read(r)?;
    let reach = read_ranges(r, REACH_RANGE_BYTES, |r| {
        let (start, end) = (u32::read(r)?, u32::read(r)?);
        CoverRange::new(start, end, ResolutionCode::NONE).map_err(DecodeBlockError::Cover)
    })?;
    Ok(CoarseCrater {
        centre,
        diameter,
        morphology,
        age,
        degradation,
        reach,
    })
}

#[cfg(test)]
mod tests;
