//! The part of a coarse field a client holds, built from the payload's blocks as they arrive (plan
//! R09, Design notes 15 to 17; [`crate::wire`]).

use super::cells::{ClimateCell, SynthesisCell};
use super::cover::ResolutionCode;
use super::crater::CoarseCrater;
use super::header::{BodyRef, FieldHeader};
use super::{
    BuildFieldError, CoarseLevel, FieldView, cell_index, check_climate_cell, check_substances,
    crater_key_order, reaching_index,
};
use crate::cube::PatchKey;
use crate::wire::{DecodedBlock, same_crater, same_header};

/// The cells of one body's coarse field that a client holds, with their margin, as the payload's
/// blocks deliver them: what the client's synthesis reads through [`FieldView`] (Design note 17).
///
/// It starts from the field's header, which block 0 of every payload carries, and grows by
/// [`insert`](Self::insert): each block adds its cells with their resolution codes, the climate
/// records over them and the craters that reach them, and the per-cell index of those craters is
/// rebuilt from their reaches. A cell arrives whole and exact, and is the server's record to the
/// bit; a margin cell, sent so that the synthesis can read past a survey's edge (Design note 15),
/// is held but not [surveyed](Self::is_surveyed). Insertion merges: a cell, climate record, crater
/// or header that arrives twice must be the same both times, bit for bit, a cell keeps its finest
/// resolution code, and the craters stay in the field's list order, so the blocks of a payload, or
/// of several payloads of the field, give the same `PartialField` in any order. Every answer of its
/// [`FieldView`] is `None`, or empty, outside the cells it holds.
///
/// It keeps a slot for every cell of the field's level whether held or not, so that a read costs
/// one index: about 13 MB at level 8, as much as the whole field's payload.
#[derive(Clone, PartialEq)]
pub struct PartialField {
    header: FieldHeader,
    /// Each cell's record, in [`cell_index`] order, `None` where it is not held.
    cells: Vec<Option<SynthesisCell>>,
    /// Each held cell's resolution code, [`ResolutionCode::NONE`] for a margin cell, and
    /// [`ResolutionCode::NONE`] where the cell is not held.
    codes: Vec<ResolutionCode>,
    /// Each climate cell's record, in the climate layer's index order, `None` where it is not
    /// held.
    climate: Vec<Option<ClimateCell>>,
    /// The craters held, with the index of the cells they reach.
    craters: CraterIndex,
}

/// The craters a [`PartialField`] holds and the per-cell index of their reaches, replaced together.
#[derive(Clone, PartialEq)]
struct CraterIndex {
    /// The craters, in the field's list order: strictly increasing (centre cell, diameter).
    craters: Vec<CoarseCrater>,
    /// The [`cell_index`] of each crater's centre's cell, beside `craters`.
    centres: Vec<u32>,
    /// Where each cell's run of `reaching` begins, as in [`CoarseField`](super::CoarseField).
    reaching_start: Vec<usize>,
    /// Indices into `craters`, cell by cell, each cell's in list order.
    reaching: Vec<u32>,
}

/// Where a crater of a merged list comes from: the field's next, or the block's at a place.
#[derive(Clone, Copy)]
enum Source {
    Held,
    Fresh(usize),
}

/// The field's craters merged with a block's: the merged list's order, and the per-cell index of
/// the merged list's reaches.
struct Merge {
    order: Vec<Source>,
    reaching_start: Vec<usize>,
    reaching: Vec<u32>,
}

/// Why [`PartialField::insert`] refused a block. The field is left as it was.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InsertBlockError {
    /// The block is of another body than the field.
    WrongBody {
        /// The block's body.
        found: BodyRef,
        /// The field's body.
        expected: BodyRef,
    },
    /// The block is at another level than the field.
    WrongLevel {
        /// The block's level.
        found: CoarseLevel,
        /// The field's level.
        expected: CoarseLevel,
    },
    /// The block is a block 0 whose header is not the field's, bit for bit: it is of another
    /// field of the body.
    WrongHeader,
    /// A record of the block breaks a rule under the field's header: a month outside its year, a
    /// substance its palette does not have, or a crater narrower than its boundary diameter.
    Record(BuildFieldError),
    /// The block carries a cell the field holds with another record.
    CellConflict {
        /// The cell's index.
        cell: u32,
    },
    /// The block carries a climate record the field holds with another value.
    ClimateConflict {
        /// The climate cell's index.
        cell: u32,
    },
    /// The block carries a crater of a key the field holds with another record, bit for bit.
    CraterConflict {
        /// The [`cell_index`] of the crater's centre's cell.
        centre_cell: u32,
    },
    /// The craters held would be more than a `u32` counts.
    TooManyCraters,
    /// The craters' reaches would hold more entries than the per-cell index admits: more than
    /// [`MAX_MEAN_REACHES_PER_CELL`](super::MAX_MEAN_REACHES_PER_CELL), 32, a cell of the level
    /// on average, or than memory holds.
    TooManyReaches,
}

impl std::fmt::Display for InsertBlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongBody { found, expected } => write!(
                f,
                "a block of body {} of system {:#x} is not of the field's body {} of system {:#x}",
                found.body_index(),
                found.raw_system_id(),
                expected.body_index(),
                expected.raw_system_id()
            ),
            Self::WrongLevel { found, expected } => write!(
                f,
                "a block at level {} is not at the field's level {}",
                found.get(),
                expected.get()
            ),
            Self::WrongHeader => write!(f, "a block 0's header is not the field's"),
            Self::Record(e) => write!(f, "a block's record is refused: {e}"),
            Self::CellConflict { cell } => {
                write!(f, "a block's cell {cell} differs from the one held")
            }
            Self::ClimateConflict { cell } => {
                write!(f, "a block's climate cell {cell} differs from the one held")
            }
            Self::CraterConflict { centre_cell } => write!(
                f,
                "a block's crater centred in cell {centre_cell} differs from the one held"
            ),
            Self::TooManyCraters => write!(f, "the craters held are more than a field lists"),
            Self::TooManyReaches => write!(f, "the craters' reaches are too many to index"),
        }
    }
}

impl std::error::Error for InsertBlockError {}

/// A slot of a per-cell vector.
#[must_use]
fn slot(index: u32) -> usize {
    usize::try_from(index).expect("a cell index fits a usize")
}

impl PartialField {
    /// The field of `header`, holding no cell yet: the header of block 0, which every payload
    /// carries ([`DecodedBlock::header`]).
    ///
    /// # Panics
    ///
    /// Never: a coarse level's cells, at most 393,216, fit a `usize`, and an index of no crater
    /// has no entry to overflow.
    #[must_use]
    pub fn new(header: FieldHeader) -> Self {
        let level = header.level();
        let cells = slot(level.cell_count());
        let (reaching_start, reaching) = reaching_index(level.cell_count(), std::iter::empty())
            .expect("an index of no crater has no entry to overflow");
        Self {
            header,
            cells: vec![None; cells],
            codes: vec![ResolutionCode::NONE; cells],
            climate: vec![None; slot(level.climate_cell_count())],
            craters: CraterIndex {
                craters: Vec::new(),
                centres: Vec::new(),
                reaching_start,
                reaching,
            },
        }
    }

    /// Adds `block`'s cells with their codes, its climate records and its craters, merged with
    /// what the field holds, and rebuilds the per-cell crater index if a crater is new.
    ///
    /// A cell held already keeps the finer of its two codes, so that a later survey's finer
    /// resolution replaces an earlier one and a margin cell that is surveyed becomes surveyed.
    /// Insertion is all or nothing: every check, and the new crater index, come before the field
    /// changes.
    ///
    /// # Errors
    ///
    /// [`InsertBlockError`] if the block is of another body or level, a block 0 carries another
    /// header, a record breaks a rule under the header, a cell, climate record or crater differs
    /// from the one held, or the craters would be too many to index. The field is then unchanged.
    pub fn insert(&mut self, block: &DecodedBlock) -> Result<(), InsertBlockError> {
        self.check(block)?;
        let merged = self.merge_order(block)?;
        for range in block.cover().ranges() {
            for cell in range.start()..range.end() {
                let n = slot(cell);
                self.codes[n] = if self.cells[n].is_some() {
                    self.codes[n].finer(range.code())
                } else {
                    range.code()
                };
            }
        }
        for (cell, record) in block.cover().cells().zip(block.cells()) {
            self.cells[slot(cell)] = Some(*record);
        }
        for (&cell, record) in block.climate_indices().iter().zip(block.climate()) {
            self.climate[slot(cell)] = Some(*record);
        }
        if let Some(Merge {
            order,
            reaching_start,
            reaching,
        }) = merged
        {
            let mut held = std::mem::take(&mut self.craters.craters).into_iter();
            let mut held_centres = std::mem::take(&mut self.craters.centres).into_iter();
            let mut craters = Vec::with_capacity(order.len());
            let mut centres = Vec::with_capacity(order.len());
            for source in order {
                let (crater, centre) = match source {
                    Source::Held => (held.next(), held_centres.next()),
                    Source::Fresh(k) => (
                        block.craters().get(k).cloned(),
                        block.crater_centres().get(k).copied(),
                    ),
                };
                craters.extend(crater);
                centres.extend(centre);
            }
            self.craters = CraterIndex {
                craters,
                centres,
                reaching_start,
                reaching,
            };
        }
        Ok(())
    }

    /// Checks `block` against the field: its body, level and header, the rules its records must
    /// keep under the header, and that what it carries of the field's cells is the same.
    fn check(&self, block: &DecodedBlock) -> Result<(), InsertBlockError> {
        let header = &self.header;
        if block.body() != header.body() {
            return Err(InsertBlockError::WrongBody {
                found: block.body(),
                expected: header.body(),
            });
        }
        if block.level() != header.level() {
            return Err(InsertBlockError::WrongLevel {
                found: block.level(),
                expected: header.level(),
            });
        }
        if block.header().is_some_and(|h| !same_header(h, header)) {
            return Err(InsertBlockError::WrongHeader);
        }
        let boundary = header.boundary_diameter();
        let narrow = (0_u32..)
            .zip(block.craters())
            .find_map(|(k, c)| (c.diameter < boundary).then_some(k));
        if let Some(crater) = narrow {
            return Err(InsertBlockError::Record(BuildFieldError::CraterDiameter {
                crater,
            }));
        }
        for (&cell, record) in block.climate_indices().iter().zip(block.climate()) {
            check_climate_cell(cell, record, header.months()).map_err(InsertBlockError::Record)?;
            if self.climate[slot(cell)].is_some_and(|held| held != *record) {
                return Err(InsertBlockError::ClimateConflict { cell });
            }
        }
        for (cell, record) in block.cover().cells().zip(block.cells()) {
            check_substances(cell, record, header.palette()).map_err(InsertBlockError::Record)?;
            if self.cells[slot(cell)].is_some_and(|held| held != *record) {
                return Err(InsertBlockError::CellConflict { cell });
            }
        }
        Ok(())
    }

    /// The order of the field's craters merged with `block`'s, and the per-cell index of the
    /// merged list, or `None` if the block brings no crater the field does not hold. Nothing is
    /// cloned or moved: the caller builds the list from the order once insertion cannot fail.
    fn merge_order(&self, block: &DecodedBlock) -> Result<Option<Merge>, InsertBlockError> {
        let held = &self.craters;
        let fresh = block.crater_centres().iter().copied().zip(block.craters());
        let (mut a, mut order) = (0, Vec::new());
        for (k, (centre_cell, crater)) in fresh.enumerate() {
            let key = (centre_cell, crater.diameter);
            while a < held.craters.len()
                && crater_key_order((held.centres[a], held.craters[a].diameter), key).is_lt()
            {
                order.push(Source::Held);
                a += 1;
            }
            let same_key = a < held.craters.len()
                && crater_key_order((held.centres[a], held.craters[a].diameter), key).is_eq();
            if same_key {
                if !same_crater(&held.craters[a], crater) {
                    return Err(InsertBlockError::CraterConflict { centre_cell });
                }
                order.push(Source::Held);
                a += 1;
            } else {
                order.push(Source::Fresh(k));
            }
        }
        if !order.iter().any(|s| matches!(s, Source::Fresh(_))) {
            return Ok(None);
        }
        order.extend(std::iter::repeat_n(Source::Held, held.craters.len() - a));
        u32::try_from(order.len()).map_err(|_| InsertBlockError::TooManyCraters)?;
        let mut next_held = held.craters.iter();
        let reaches: Vec<&super::Cover> = order
            .iter()
            .filter_map(|source| match *source {
                Source::Held => next_held.next(),
                Source::Fresh(k) => block.craters().get(k),
            })
            .map(|c| &c.reach)
            .collect();
        let (reaching_start, reaching) =
            reaching_index(self.header.level().cell_count(), reaches.iter().copied())
                .ok_or(InsertBlockError::TooManyReaches)?;
        Ok(Some(Merge {
            order,
            reaching_start,
            reaching,
        }))
    }

    /// Whether the field holds `cell`, a cell at its level, as a surveyed cell rather than a
    /// margin cell; `false` for a cell it does not hold.
    #[must_use]
    pub fn is_surveyed(&self, cell: PatchKey) -> bool {
        self.resolution(cell)
            .is_some_and(ResolutionCode::is_surveyed)
    }

    /// The resolution code of `cell`, a cell at the field's level: a survey's code for a surveyed
    /// cell, [`ResolutionCode::NONE`] for a margin cell, and `None` for a cell the field does not
    /// hold.
    #[must_use]
    pub fn resolution(&self, cell: PatchKey) -> Option<ResolutionCode> {
        let n = self.held(cell)?;
        Some(self.codes[n])
    }

    /// The number of cells held, surveyed and margin.
    #[must_use]
    fn held_count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// The slot of `cell` if it is a cell of the field's level that the field holds.
    #[must_use]
    fn held(&self, cell: PatchKey) -> Option<usize> {
        if cell.level() != self.header.level().get() {
            return None;
        }
        let n = slot(cell_index(cell));
        self.cells[n].is_some().then_some(n)
    }
}

impl FieldView for PartialField {
    fn header(&self) -> &FieldHeader {
        &self.header
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.cells[self.held(cell)?].as_ref()
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.held(cell)?;
        let parent = cell.parent()?;
        self.climate[slot(cell_index(parent))].as_ref()
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        let index = &self.craters;
        let run = self.held(cell).map_or(&[][..], |n| {
            &index.reaching[index.reaching_start[n]..index.reaching_start[n + 1]]
        });
        run.iter()
            .map(|&k| &index.craters[usize::try_from(k).expect("a crater index fits a usize")])
    }
}

impl std::fmt::Debug for PartialField {
    /// The header and the counts: the records themselves are up to hundreds of thousands long.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartialField")
            .field("header", &self.header)
            .field("held_cells", &self.held_count())
            .field("craters", &self.craters.craters.len())
            .finish_non_exhaustive()
    }
}
