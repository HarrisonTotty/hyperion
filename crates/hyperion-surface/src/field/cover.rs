//! Covers: sets of a field's cells as sorted runs of cell indices, each run with the resolution it
//! was surveyed at (plan R09, Design notes 10 and 16).

use hyperion_base::math;
use hyperion_base::units::Metres;

use super::cells::QuantiseValueError;

/// The resolution a cell was surveyed at, one byte (Design note 16).
///
/// Code 0 is [`ResolutionCode::NONE`], not surveyed, and code c ≥ 1 is a resolution no coarser
/// than 1 cm × 2^((c − 1) ÷ 8): a resolution r codes max(1, ⌈8 log₂(r ÷ 1 cm)⌉ + 1), so a pass at
/// 1 cm or finer codes 1, and the codes span 1 cm to about 36,000 km in steps of 9%. A smaller
/// code (other than 0) is finer. Coverage keeps a cell's finest; the codes gate what a readout
/// may quote (R10, through the synthesis's unresolved variance), never what the client holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ResolutionCode(u8);

/// [`ResolutionCode`]'s codes an octave.
const RESOLUTION_PER_OCTAVE: f64 = 8.0;

/// Centimetres a metre.
const CM_PER_M: f64 = 100.0;

impl ResolutionCode {
    /// Not surveyed. A cover that says nothing of resolution, such as a crater's reach, carries it
    /// on every range; in a payload's cover it marks a margin cell, held but not surveyed.
    pub const NONE: Self = Self(0);

    /// The finest code, a survey at 1 cm or finer.
    pub const FINEST: Self = Self(1);

    /// The resolution of code `code`.
    #[must_use]
    pub const fn new(code: u8) -> Self {
        Self(code)
    }

    /// The code.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// Whether the code is a survey's, not [`ResolutionCode::NONE`].
    #[must_use]
    pub const fn is_surveyed(self) -> bool {
        self.0 != 0
    }

    /// The code of a survey at `resolution`: max(1, ⌈8 log₂(r ÷ 1 cm)⌉ + 1).
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError::NotFinite`] for a resolution that is not finite, and
    /// [`QuantiseValueError::OutOfRange`] for one that is not positive or is coarser than the last
    /// code's, about 36,000 km.
    pub fn from_resolution(resolution: Metres) -> Result<Self, QuantiseValueError> {
        let r = resolution.value();
        if !r.is_finite() {
            return Err(QuantiseValueError::NotFinite(r));
        }
        if r <= 0.0 {
            return Err(QuantiseValueError::OutOfRange(r));
        }
        let code = ((RESOLUTION_PER_OCTAVE * math::log2(r * CM_PER_M)).ceil() + 1.0).max(1.0);
        if code > 255.0 {
            return Err(QuantiseValueError::OutOfRange(r));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "code is an integer from 1 to 255, checked above"
        )]
        Ok(Self(code as u8))
    }

    /// The coarsest resolution the code admits, 1 cm × 2^((c − 1) ÷ 8), or `None` for
    /// [`ResolutionCode::NONE`]: a cell of this code was surveyed at least this finely.
    #[must_use]
    pub fn resolution(self) -> Option<Metres> {
        self.is_surveyed().then(|| {
            Metres::new(math::exp2((f64::from(self.0) - 1.0) / RESOLUTION_PER_OCTAVE) / CM_PER_M)
        })
    }

    /// The finer of two codes, a survey beating [`ResolutionCode::NONE`].
    #[must_use]
    pub const fn finer(self, other: Self) -> Self {
        match (self.0, other.0) {
            (0, _) => other,
            (_, 0) => self,
            (a, b) if a <= b => self,
            _ => other,
        }
    }
}

/// A run of consecutive cell indices, half-open, `start` to `end`, all with one resolution code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoverRange {
    start: u32,
    end: u32,
    code: ResolutionCode,
}

/// Why a range or a cover was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildCoverError {
    /// A range is empty: its end is not above its start.
    EmptyRange {
        /// The range's first index.
        start: u32,
        /// The index past its last.
        end: u32,
    },
    /// A range begins before the previous one ends.
    Unsorted {
        /// The place of the range out of order.
        range: usize,
    },
}

impl std::fmt::Display for BuildCoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRange { start, end } => write!(f, "cell range {start}..{end} is empty"),
            Self::Unsorted { range } => {
                write!(f, "cell range {range} begins before the previous one ends")
            }
        }
    }
}

impl std::error::Error for BuildCoverError {}

impl CoverRange {
    /// The cells `start` to `end` (half-open), each of resolution `code`.
    ///
    /// # Errors
    ///
    /// [`BuildCoverError::EmptyRange`] if `end` is not above `start`.
    pub const fn new(start: u32, end: u32, code: ResolutionCode) -> Result<Self, BuildCoverError> {
        if end <= start {
            return Err(BuildCoverError::EmptyRange { start, end });
        }
        Ok(Self { start, end, code })
    }

    /// The first cell index.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// The index past the last.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }

    /// The cells' resolution code.
    #[must_use]
    pub const fn code(self) -> ResolutionCode {
        self.code
    }

    /// The number of cells, at least 1.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    /// Never: a range holds at least one cell.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        false
    }
}

/// A set of cells of one field level, each with a resolution code, as sorted, disjoint runs of
/// [`cell_index`](super::cell_index) values (Design notes 10 and 16).
///
/// A cover is canonical: its ranges are sorted and disjoint, and two adjacent ranges of one code
/// are always merged, so two covers of the same cells and codes are equal. It does not know its
/// level; the field it is used with does. A crater's [`reach`](super::CoarseCrater::reach) is a
/// cover of [`ResolutionCode::NONE`]; a survey's carries each cell's code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Cover {
    ranges: Vec<CoverRange>,
}

impl Cover {
    /// The empty cover.
    #[must_use]
    pub const fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// The cover of `ranges`, which must be sorted and disjoint; adjacent ranges of one code are
    /// merged.
    ///
    /// # Errors
    ///
    /// [`BuildCoverError::Unsorted`] if a range begins before the previous one ends.
    pub fn from_ranges(
        ranges: impl IntoIterator<Item = CoverRange>,
    ) -> Result<Self, BuildCoverError> {
        let mut merged: Vec<CoverRange> = Vec::new();
        for (place, range) in ranges.into_iter().enumerate() {
            match merged.last_mut() {
                Some(last) if range.start < last.end => {
                    return Err(BuildCoverError::Unsorted { range: place });
                }
                Some(last) if range.start == last.end && range.code == last.code => {
                    last.end = range.end;
                }
                _ => merged.push(range),
            }
        }
        Ok(Self { ranges: merged })
    }

    /// The cover of `cells`, in any order and with repeats, each of [`ResolutionCode::NONE`].
    ///
    /// # Panics
    ///
    /// If a cell is `u32::MAX`, past which a half-open range cannot end; no level's cells reach
    /// it, the last index of level 14 being 6 · 4¹⁴ − 1.
    #[must_use]
    pub fn from_cells(cells: impl IntoIterator<Item = u32>) -> Self {
        let mut cells: Vec<u32> = cells.into_iter().collect();
        cells.sort_unstable();
        cells.dedup();
        let mut ranges: Vec<CoverRange> = Vec::new();
        for cell in cells {
            assert!(
                cell < u32::MAX,
                "a cover holds cells below u32::MAX, past which no range can end"
            );
            let end = cell + 1;
            match ranges.last_mut() {
                Some(last) if last.end == cell => last.end = end,
                _ => ranges.push(CoverRange {
                    start: cell,
                    end,
                    code: ResolutionCode::NONE,
                }),
            }
        }
        Self { ranges }
    }

    /// The ranges, sorted and disjoint.
    #[must_use]
    pub fn ranges(&self) -> &[CoverRange] {
        &self.ranges
    }

    /// Whether the cover holds no cell.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// The number of cells the cover holds.
    #[must_use]
    pub fn cell_count(&self) -> u64 {
        self.ranges.iter().map(|r| u64::from(r.len())).sum()
    }

    /// The code of cell `index`, or `None` if the cover does not hold it.
    #[must_use]
    pub fn code(&self, index: u32) -> Option<ResolutionCode> {
        let at = self.ranges.partition_point(|r| r.end <= index);
        self.ranges
            .get(at)
            .filter(|r| r.start <= index)
            .map(|r| r.code)
    }

    /// Whether the cover holds cell `index`.
    #[must_use]
    pub fn contains(&self, index: u32) -> bool {
        self.code(index).is_some()
    }

    /// The cells the cover holds, in index order.
    pub fn cells(&self) -> impl Iterator<Item = u32> + '_ {
        self.ranges.iter().flat_map(|r| r.start..r.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// Design note 16's codes: 1 cm or finer codes 1, 2 cm codes 9, 36,000 km 255, and each code's
    /// resolution bounds the surveys that code to it.
    #[test]
    fn resolution_codes_are_design_note_sixteens() {
        let code = |r: f64| ResolutionCode::from_resolution(Metres::new(r));
        assert_eq!(code(0.01), Ok(ResolutionCode::FINEST));
        assert_eq!(code(1e-4), Ok(ResolutionCode::FINEST));
        assert_eq!(code(0.02), Ok(ResolutionCode::new(9)));
        assert_eq!(code(3.6e7), Ok(ResolutionCode::new(255)));
        assert_eq!(code(3.7e7), Err(QuantiseValueError::OutOfRange(3.7e7)));
        assert_eq!(code(0.0), Err(QuantiseValueError::OutOfRange(0.0)));
        assert!(matches!(
            code(f64::NAN),
            Err(QuantiseValueError::NotFinite(v)) if v.is_nan()
        ));
        for r in [0.013, 0.5, 2.0, 36_000.0, 1.2e6] {
            let c = code(r).unwrap();
            let bound = c.resolution().unwrap().value();
            assert!(
                r <= bound * (1.0 + 1e-12),
                "{r} m codes {c:?}, bound {bound}"
            );
            let finer = ResolutionCode::new(c.get() - 1)
                .resolution()
                .unwrap()
                .value();
            assert!(
                r > finer,
                "{r} m codes {c:?}, but the finer bound {finer} holds it"
            );
        }
        assert_eq!(ResolutionCode::NONE.resolution(), None);
        let (a, b) = (ResolutionCode::new(9), ResolutionCode::new(40));
        assert_eq!(a.finer(b), a);
        assert_eq!(b.finer(a), a);
        assert_eq!(ResolutionCode::NONE.finer(b), b);
        assert_eq!(b.finer(ResolutionCode::NONE), b);
    }

    /// A cover merges adjacent ranges of one code, keeps ranges of two codes apart, refuses
    /// overlaps and empty ranges, and answers each cell's code.
    #[test]
    fn a_field_cover_is_canonical_and_answers_each_cell() {
        let range = |s, e, c| CoverRange::new(s, e, ResolutionCode::new(c)).unwrap();
        let cover = Cover::from_ranges([range(0, 4, 9), range(4, 8, 9), range(8, 10, 12)]).unwrap();
        assert_eq!(cover.ranges(), &[range(0, 8, 9), range(8, 10, 12)]);
        assert_eq!(cover.cell_count(), 10);
        assert_eq!(cover.code(7), Some(ResolutionCode::new(9)));
        assert_eq!(cover.code(8), Some(ResolutionCode::new(12)));
        assert_eq!(cover.code(10), None);
        assert!(!cover.contains(11));
        assert_eq!(
            Cover::from_ranges([range(0, 4, 9), range(3, 8, 9)]),
            Err(BuildCoverError::Unsorted { range: 1 })
        );
        assert_eq!(
            CoverRange::new(5, 5, ResolutionCode::NONE),
            Err(BuildCoverError::EmptyRange { start: 5, end: 5 })
        );
        let cells = Cover::from_cells([7, 3, 4, 5, 3, 20]);
        assert_eq!(
            cells.ranges(),
            &[range(3, 6, 0), range(7, 8, 0), range(20, 21, 0)]
        );
        assert_eq!(cells.cells().collect::<Vec<_>>(), vec![3, 4, 5, 7, 20]);
        assert!(Cover::new().is_empty());
        assert_eq!(Cover::from_cells([]), Cover::new());
    }
}
