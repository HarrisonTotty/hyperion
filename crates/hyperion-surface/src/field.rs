//! The coarse field: a body's surface at one cube-sphere level, quantised once, and the only input
//! of the local synthesis on the server and the client alike (plan R09, Design notes 4, 10 and 17).
//!
//! The coarse pass, which only the server runs (the sim's `planetary::surface`, R09.T10–T16),
//! works in `f64` and ends by quantising its result into a [`CoarseField`]. The server's own
//! synthesis and collision read that quantised field, never the pass's working state, and the
//! client receives the same cells exactly and whole (R09.T3's payload), so the two sides' inputs
//! are identical by construction. A client holds only the cells its ship has surveyed, with their
//! margin; [`FieldView`] is what the synthesis reads, so that it runs on the server's whole field
//! and on a client's part of one alike.
//!
//! # Cells and their order
//!
//! A cell is R05's [`PatchKey`] at the field's [`CoarseLevel`], the shallowest whose mean cell is
//! no wider than 40 km, floored at 5 and capped at 8 ([`coarse_level`]). Every per-cell array is
//! in [`cell_index`] order: face-major, then Morton (Z) order within the face, so that a cell's
//! four children at the next level are four consecutive indices and a quadtree subtree is one run.
//! The climate layer is one level coarser, L − 1, one record per four cells (Design note 17).
//!
//! # The datum and the units
//!
//! Heights, elevations and the sea level are measured in metres along the normal of the body's
//! rotational spheroid, the header's figure (R07's Design note 19; Design note 17), positive
//! outwards. Each record stores its quantities as the integer codes the payload carries, and each
//! code's doc comment states its step and its SI meaning; the methods that read them back return
//! the SI units of `hyperion_base::units`.

/// Defines a field enum carried as one byte: the enum, `ALL` in code order, and its conversions to
/// and from the byte, the unknown codes refused with [`DecodeFieldCodeError`].
macro_rules! coded_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident = $code:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)+
        }

        impl $name {
            /// Every variant, in the order of their codes.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];
        }

        impl From<$name> for u8 {
            fn from(value: $name) -> Self {
                match value {
                    $($name::$variant => $code,)+
                }
            }
        }

        impl TryFrom<u8> for $name {
            type Error = $crate::field::DecodeFieldCodeError;

            fn try_from(code: u8) -> Result<Self, Self::Error> {
                match code {
                    $($code => Ok(Self::$variant),)+
                    _ => Err($crate::field::DecodeFieldCodeError {
                        kind: stringify!($name),
                        code,
                    }),
                }
            }
        }
    };
}

mod cells;
mod cover;
mod crater;
mod header;

pub use cells::{
    BoundaryKind, ClimateCell, Crust, DecodeFieldCodeError, FlowDirection, LogArea,
    LogPrecipitation, LogSteepness, QuantiseValueError, SurfaceClass, SynthesisCell, Wind,
};
pub use cover::{BuildCoverError, Cover, CoverRange, ResolutionCode};
pub use crater::{CoarseCrater, Morphology};
pub use header::{
    BodyRef, BuildFieldHeaderError, ClimateModelKind, FieldHeader, FieldHeaderParts,
    PrecipitationSource,
};

use hyperion_base::units::Metres;

use crate::cube::{Face, PatchKey};

/// The level of a body's coarse field: 5 to 8 (plan R09, Design note 4).
///
/// The level fixes the field's cell count, 6 · 4ᴸ, from 6,144 at level 5 to 393,216 at level 8,
/// and its climate layer's, one level coarser. It is one of the values the generator version
/// reserves: the rule of [`coarse_level`] and its bounds are part of the generated output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoarseLevel(u8);

impl CoarseLevel {
    /// The shallowest level, so that a small body still has a grid: 6,144 cells, about 21 km on
    /// Ceres.
    pub const MIN: Self = Self(5);

    /// The deepest level, which bounds the transfer: 393,216 cells, about 36 km on an Earth and
    /// 72 km on a world of 2 R⊕, where the cap binds.
    pub const MAX: Self = Self(8);

    /// The level `level`, or `None` outside 5 to 8.
    #[must_use]
    pub const fn new(level: u8) -> Option<Self> {
        if level >= Self::MIN.0 && level <= Self::MAX.0 {
            Some(Self(level))
        } else {
            None
        }
    }

    /// The quadtree level, 5 to 8.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The field's cell count, 6 · 4ᴸ.
    #[must_use]
    pub const fn cell_count(self) -> u32 {
        cell_count(self.0)
    }

    /// The level of the climate layer, L − 1 (Design note 17): 4 to 7.
    #[must_use]
    pub const fn climate_level(self) -> u8 {
        self.0 - 1
    }

    /// The climate layer's cell count, 6 · 4ᴸ⁻¹, a quarter of the field's.
    #[must_use]
    pub const fn climate_cell_count(self) -> u32 {
        cell_count(self.0 - 1)
    }
}

/// The widest mean cell a coarse field may have, metres: the brainstorm's "no wider than about
/// 40 km" (Design note 4), part of the generator version.
const MAX_MEAN_CELL_M: f64 = 40_000.0;

/// The level of the coarse field of a body of volumetric mean radius `radius`: the shallowest
/// cube-sphere level whose mean cell, √(4πR² ÷ (6 · 4ᴸ)), is no wider than 40 km, floored at 5
/// and capped at 8 (Design note 4).
///
/// Level 8 for an Earth (cells of about 36 km) and for a world of 2 R⊕ (about 72 km, where the cap
/// binds), 7 for a Mars, 6 for the Moon and 5 for Ceres. The mean cell is computed as
/// (R × √(4π ÷ 6)) ÷ 2ᴸ, in that order, which is part of the generator version: the radii at which
/// the level switches are pinned to the bit by a test.
///
/// # Panics
///
/// If `radius` is not finite and positive.
#[must_use]
pub fn coarse_level(radius: Metres) -> CoarseLevel {
    let r = radius.value();
    assert!(
        r.is_finite() && r > 0.0,
        "a body's radius must be finite and positive, got {r} m"
    );
    let whole_face = r * (4.0 * core::f64::consts::PI / 6.0).sqrt();
    (CoarseLevel::MIN.0..CoarseLevel::MAX.0)
        .find(|&level| whole_face / f64::from(1_u32 << level) <= MAX_MEAN_CELL_M)
        .map_or(CoarseLevel::MAX, CoarseLevel)
}

/// The boundary diameter `D_b` of a field at `level` on a body of volumetric mean radius `radius`:
/// twice the level's largest cell edge (Design note 4).
///
/// The largest edge is S2's `kMaxEdge` for the quadratic warp, a derivative of 1.704 897 radians
/// per unit of s at 2⁻ᴸ of s a cell, times the radius (`s2metrics.cc`; this crate's
/// `geometry` derives it in closed form). `D_b` is 84.9 km on an Earth at level 8, 90.3 km on a Mars
/// at 7, 92.6 km on the Moon at 6 and 50.0 km on Ceres at 5, not the brainstorm's "about 100 km".
/// Craters of `D_b` and wider are the coarse pass's, listed in the field; every narrower one is the
/// synthesis's (Design note 10).
///
/// # Panics
///
/// If `radius` is not finite and positive.
#[must_use]
pub fn boundary_diameter(level: CoarseLevel, radius: Metres) -> Metres {
    let r = radius.value();
    assert!(
        r.is_finite() && r > 0.0,
        "a body's radius must be finite and positive, got {r} m"
    );
    Metres::new(2.0 * crate::geometry::max_rate() * r / f64::from(1_u32 << level.0))
}

/// The deepest level whose cells [`cell_index`] numbers: 6 · 4¹⁴ cells, about 1.6 × 10⁹, is the
/// last count to fit a `u32`.
const MAX_INDEXED_LEVEL: u8 = 14;

/// The cell count of `level`, 6 · 4^`level`, for a level up to 14.
#[must_use]
const fn cell_count(level: u8) -> u32 {
    6 << (2 * level)
}

/// The index of `cell` among the cells of its level: face-major, then Morton order within the face
/// (Design note 16's `cell_index`).
///
/// The index is face × 4ᴸ plus the cell's Morton code, whose bit 2k is bit k of i and whose bit
/// 2k + 1 is bit k of j, so that a cell's children, in [`PatchKey::children`]'s order, have the
/// four consecutive indices 4n to 4n + 3, and every quadtree subtree is one run of indices. The
/// indices of a level are 0 to 6 · 4ᴸ − 1, each once ([`cell_at_index`] is the inverse); the
/// field's arrays, its covers and the payload's ranges are in this order.
///
/// # Panics
///
/// If `cell` is deeper than level 14, whose index would not fit a `u32`; a coarse field's cells
/// are at level 8 at most.
#[must_use]
pub fn cell_index(cell: PatchKey) -> u32 {
    let level = cell.level();
    assert!(
        level <= MAX_INDEXED_LEVEL,
        "cell indices number levels up to {MAX_INDEXED_LEVEL}, not level {level}"
    );
    (u32::from(cell.face().index()) << (2 * level)) | spread(cell.i()) | (spread(cell.j()) << 1)
}

/// The cell of `level` whose [`cell_index`] is `index`, or `None` if `level` is deeper than 14 or
/// `index` is not below 6 · 4^`level`.
///
/// # Panics
///
/// Never: an index below 6 · 4ᴸ has a face of 0 to 5 above its 2L Morton bits, which give i and
/// j below 2ᴸ.
#[must_use]
pub fn cell_at_index(level: u8, index: u32) -> Option<PatchKey> {
    if level > MAX_INDEXED_LEVEL || index >= cell_count(level) {
        return None;
    }
    let face = u8::try_from(index >> (2 * level))
        .ok()
        .and_then(Face::from_index)
        .expect("an index below 6 · 4^level has a face of 0 to 5");
    let morton = index & ((1_u32 << (2 * level)) - 1);
    let cell = PatchKey::new(face, level, compact(morton), compact(morton >> 1))
        .expect("a level's Morton bits give i and j below 2^level");
    Some(cell)
}

/// The low 16 bits of `x` spread to the even bits of the result.
#[must_use]
const fn spread(x: u32) -> u32 {
    let mut x = x & 0x0000_ffff;
    x = (x | (x << 8)) & 0x00ff_00ff;
    x = (x | (x << 4)) & 0x0f0f_0f0f;
    x = (x | (x << 2)) & 0x3333_3333;
    (x | (x << 1)) & 0x5555_5555
}

/// The even bits of `x` gathered into the low 16 bits of the result, the inverse of [`spread`].
#[must_use]
const fn compact(x: u32) -> u32 {
    let mut x = x & 0x5555_5555;
    x = (x | (x >> 1)) & 0x3333_3333;
    x = (x | (x >> 2)) & 0x0f0f_0f0f;
    x = (x | (x >> 4)) & 0x00ff_00ff;
    (x | (x >> 8)) & 0x0000_ffff
}

/// What the synthesis reads of a coarse field: the server's whole [`CoarseField`], or the part of
/// one a client holds (R09.T3's `PartialField`).
///
/// Every answer is `None`, or empty, for a cell the view does not hold, and for a key that is not
/// at the field's level, so that a reader that strays beyond what it was given finds out rather
/// than reading another cell.
pub trait FieldView {
    /// The field's header.
    fn header(&self) -> &FieldHeader;

    /// The synthesis record of `cell`, a cell at the field's level.
    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell>;

    /// The climate record over `cell`, a cell at the field's level: the record of its parent at
    /// the climate layer's level, L − 1, which it shares with its three siblings.
    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell>;

    /// The coarse craters whose [`CoarseCrater::reach`] holds `cell`, a cell at the field's level,
    /// in the field's list order.
    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_;
}

/// A body's whole coarse field, as the server holds it: the header, a synthesis record per cell, a
/// climate record per cell of the level above, and the list of craters of `D_b` and wider, with the
/// index of the craters reaching each cell (Design notes 10 and 17).
///
/// [`CoarseField::new`] validates what it is given, so a field always holds one record per cell,
/// water surfaces at or above the ground, months only within the header's year, and its craters
/// in strictly increasing (centre cell, diameter), a key no two craters share, each its reach
/// complete with its centre's cell. The per-cell crater index is built from the reaches there and
/// is not on the wire: a client rebuilds it as blocks arrive (R09.T3), merging the craters of
/// several blocks by that key.
#[derive(Clone, PartialEq)]
pub struct CoarseField {
    header: FieldHeader,
    synthesis: Vec<SynthesisCell>,
    climate: Vec<ClimateCell>,
    craters: Vec<CoarseCrater>,
    /// Where each cell's run of [`reaching`](Self::reaching) begins, in cell-index order, with one
    /// more entry at the end: cell n's craters are `reaching[reaching_start[n]..reaching_start[n +
    /// 1]]`.
    reaching_start: Vec<usize>,
    /// Indices into `craters`, cell by cell, each cell's in list order.
    reaching: Vec<u32>,
}

/// Why [`CoarseField::new`] refused its parts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BuildFieldError {
    /// The synthesis records are not one per cell of the field's level.
    SynthesisCount {
        /// The field's cell count.
        expected: u32,
        /// The records given.
        found: usize,
    },
    /// The climate records are not one per cell of the climate layer's level, L − 1.
    ClimateCount {
        /// The climate layer's cell count.
        expected: u32,
        /// The records given.
        found: usize,
    },
    /// A cell's water surface is below its ground.
    WaterBelowGround {
        /// The cell's index.
        cell: u32,
    },
    /// A cell names a boundary kind but no distance to it, or a distance but no boundary.
    BoundaryWithoutDistance {
        /// The cell's index.
        cell: u32,
    },
    /// A climate record carries a month past the header's year, or, in a one-month year, an
    /// anomaly or a second wind.
    MonthOutsideYear {
        /// The climate cell's index.
        cell: u32,
    },
    /// There are more craters than a `u32` can count.
    TooManyCraters(usize),
    /// The craters' reaches, cell by cell, hold more entries than the per-cell index can count.
    TooManyReaches,
    /// A crater's centre is not a finite unit vector.
    CraterCentre {
        /// The crater's place in the list.
        crater: u32,
    },
    /// A crater's diameter is not finite or is below the field's boundary diameter.
    CraterDiameter {
        /// The crater's place in the list.
        crater: u32,
    },
    /// A crater's age is not finite and non-negative.
    CraterAge {
        /// The crater's place in the list.
        crater: u32,
    },
    /// A crater's reach is empty, names a cell the field does not have, or misses its centre's
    /// cell.
    CraterReach {
        /// The crater's place in the list.
        crater: u32,
    },
    /// A crater is listed after one of an equal or higher (centre cell, diameter).
    CratersUnsorted {
        /// The place of the first crater out of order.
        crater: u32,
    },
}

impl std::fmt::Display for BuildFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SynthesisCount { expected, found } => {
                write!(f, "{found} synthesis records for {expected} cells")
            }
            Self::ClimateCount { expected, found } => {
                write!(f, "{found} climate records for {expected} climate cells")
            }
            Self::WaterBelowGround { cell } => {
                write!(f, "cell {cell}'s water surface is below its ground")
            }
            Self::BoundaryWithoutDistance { cell } => write!(
                f,
                "cell {cell} names a boundary without a distance, or a distance without one"
            ),
            Self::MonthOutsideYear { cell } => {
                write!(f, "climate cell {cell} carries a month outside the year")
            }
            Self::TooManyCraters(count) => write!(f, "{count} craters are more than a field lists"),
            Self::TooManyReaches => write!(f, "the craters' reaches are too many to index"),
            Self::CraterCentre { crater } => {
                write!(f, "crater {crater}'s centre is not a finite unit vector")
            }
            Self::CraterDiameter { crater } => write!(
                f,
                "crater {crater}'s diameter is not finite or is below the boundary diameter"
            ),
            Self::CraterAge { crater } => {
                write!(f, "crater {crater}'s age is not finite and non-negative")
            }
            Self::CraterReach { crater } => write!(
                f,
                "crater {crater}'s reach is empty, outside the field or misses its centre"
            ),
            Self::CratersUnsorted { crater } => write!(
                f,
                "crater {crater} is not after its predecessor in (centre cell, diameter)"
            ),
        }
    }
}

impl std::error::Error for BuildFieldError {}

/// How far a crater's centre may be from unit length, relative: a few rounding errors of a
/// normalisation, far below any centre that was not one.
const UNIT_TOLERANCE: f64 = 1e-12;

impl CoarseField {
    /// The field of `header`, its synthesis records in [`cell_index`] order, its climate records in
    /// the order of the climate layer's cell indices, and its craters in strictly increasing (the
    /// [`cell_index`] of the centre's cell, diameter), so that no two share a key.
    ///
    /// The per-cell index of the craters reaching each cell is built here from the craters'
    /// reaches, in list order.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the records are not one per cell, a record breaks a rule of its type
    /// (water below the ground, a boundary without a distance, a month outside the year, or a
    /// one-month year with an anomaly or two winds), a crater is not a valid coarse crater of this
    /// field or is not after its predecessor, or the reaches are too many to index.
    pub fn new(
        header: FieldHeader,
        synthesis: Vec<SynthesisCell>,
        climate: Vec<ClimateCell>,
        craters: Vec<CoarseCrater>,
    ) -> Result<Self, BuildFieldError> {
        let level = header.level();
        let cell_total = level.cell_count();
        if usize::try_from(cell_total).ok() != Some(synthesis.len()) {
            return Err(BuildFieldError::SynthesisCount {
                expected: cell_total,
                found: synthesis.len(),
            });
        }
        let climate_total = level.climate_cell_count();
        if usize::try_from(climate_total).ok() != Some(climate.len()) {
            return Err(BuildFieldError::ClimateCount {
                expected: climate_total,
                found: climate.len(),
            });
        }
        for (cell, record) in (0_u32..).zip(&synthesis) {
            if record.water_surface_mm < record.elevation_mm {
                return Err(BuildFieldError::WaterBelowGround { cell });
            }
            let absent = record.boundary == BoundaryKind::Absent;
            if absent != (record.boundary_distance_km == SynthesisCell::NO_BOUNDARY_KM) {
                return Err(BuildFieldError::BoundaryWithoutDistance { cell });
            }
        }
        let months = usize::from(header.months());
        for (cell, record) in (0_u32..).zip(&climate) {
            let seasons_in_one_month = months == 1
                && (record.month_anomaly[0] != 0
                    || record.wind[1..].iter().any(|&w| w != record.wind[0]));
            let outside = record.month_anomaly[months..].iter().any(|&a| a != 0)
                || record.month_precipitation[months..]
                    .iter()
                    .any(|&p| p != LogPrecipitation::NONE)
                || seasons_in_one_month;
            if outside {
                return Err(BuildFieldError::MonthOutsideYear { cell });
            }
        }
        let crater_count = u32::try_from(craters.len())
            .map_err(|_| BuildFieldError::TooManyCraters(craters.len()))?;
        let mut previous: Option<(u32, Metres)> = None;
        for (crater, c) in (0_u32..).zip(&craters) {
            let centre_cell = validate_crater(&header, crater, c)?;
            let key = (centre_cell, c.diameter);
            if let Some((cell, diameter)) = previous {
                let order = cell.cmp(&key.0).then_with(|| diameter.total_cmp(&key.1));
                if order != std::cmp::Ordering::Less {
                    return Err(BuildFieldError::CratersUnsorted { crater });
                }
            }
            previous = Some(key);
        }
        let (reaching_start, reaching) = reaching_index(cell_total, crater_count, &craters)
            .ok_or(BuildFieldError::TooManyReaches)?;
        Ok(Self {
            header,
            synthesis,
            climate,
            craters,
            reaching_start,
            reaching,
        })
    }

    /// The synthesis records, one per cell, in [`cell_index`] order.
    #[must_use]
    pub fn synthesis(&self) -> &[SynthesisCell] {
        &self.synthesis
    }

    /// The climate records, one per cell of the climate layer's level (L − 1), in the order of
    /// that level's cell indices.
    #[must_use]
    pub fn climate_layer(&self) -> &[ClimateCell] {
        &self.climate
    }

    /// The craters of `D_b` and wider, in strictly increasing (the [`cell_index`] of the centre's
    /// cell, diameter).
    #[must_use]
    pub fn craters(&self) -> &[CoarseCrater] {
        &self.craters
    }

    /// The field with its header's albedo scale set to `albedo_scale`, which R10 computes on the
    /// server once the field is built (Design note 18), its records kept as they are.
    ///
    /// # Errors
    ///
    /// [`BuildFieldHeaderError::NotPositive`] or [`BuildFieldHeaderError::NotFinite`] for a scale
    /// that is not finite and positive.
    pub fn with_albedo_scale(
        mut self,
        albedo_scale: Option<f64>,
    ) -> Result<Self, BuildFieldHeaderError> {
        self.header = self.header.with_albedo_scale(albedo_scale)?;
        Ok(self)
    }

    /// The index of `cell` in the synthesis records, if it is a cell of the field's level.
    #[must_use]
    fn index_of(&self, cell: PatchKey) -> Option<usize> {
        if cell.level() == self.header.level().get() {
            usize::try_from(cell_index(cell)).ok()
        } else {
            None
        }
    }
}

/// Checks crater `crater` of a field with `header`, and returns the index of its centre's cell.
fn validate_crater(
    header: &FieldHeader,
    crater: u32,
    c: &CoarseCrater,
) -> Result<u32, BuildFieldError> {
    let [x, y, z] = c.centre;
    let norm2 = x * x + y * y + z * z;
    if !(c.centre.iter().all(|v| v.is_finite()) && (norm2 - 1.0).abs() <= UNIT_TOLERANCE) {
        return Err(BuildFieldError::CraterCentre { crater });
    }
    let diameter = c.diameter.value();
    if !(diameter.is_finite() && diameter >= header.boundary_diameter().value()) {
        return Err(BuildFieldError::CraterDiameter { crater });
    }
    let age = c.age.value();
    if !(age.is_finite() && age >= 0.0) {
        return Err(BuildFieldError::CraterAge { crater });
    }
    let level = header.level();
    let centre_cell = cell_index(
        PatchKey::containing(level.get(), c.centre)
            .expect("a coarse level is below the cube's deepest"),
    );
    let reach_ok = c
        .reach
        .ranges()
        .last()
        .is_some_and(|last| last.end() <= level.cell_count())
        && c.reach.contains(centre_cell);
    if !reach_ok {
        return Err(BuildFieldError::CraterReach { crater });
    }
    Ok(centre_cell)
}

/// The per-cell index of the craters reaching each cell, in compressed rows: each cell's run
/// begins at its entry of the first vector, and lists crater indices in list order; `None` if the
/// entries are more than a `usize` counts (a `u32` on WebAssembly). The reaches must lie within the
/// field's cells.
#[must_use]
fn reaching_index(
    cell_total: u32,
    crater_count: u32,
    craters: &[CoarseCrater],
) -> Option<(Vec<usize>, Vec<u32>)> {
    let cells = usize::try_from(cell_total).expect("a coarse field's cell count fits a usize");
    let mut counts = vec![0_usize; cells];
    for crater in craters {
        for cell in crater.reach.cells() {
            counts[usize::try_from(cell).expect("a cell index fits a usize")] += 1;
        }
    }
    let mut start = Vec::with_capacity(cells + 1);
    let mut total = 0_usize;
    start.push(0);
    for count in &counts {
        total = total.checked_add(*count)?;
        start.push(total);
    }
    let mut next = start[..cells].to_vec();
    let mut reaching = vec![0_u32; total];
    for (k, crater) in (0..crater_count).zip(craters) {
        for cell in crater.reach.cells() {
            let slot = &mut next[usize::try_from(cell).expect("a cell index fits a usize")];
            reaching[*slot] = k;
            *slot += 1;
        }
    }
    Some((start, reaching))
}

impl FieldView for CoarseField {
    fn header(&self) -> &FieldHeader {
        &self.header
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.synthesis.get(self.index_of(cell)?)
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.index_of(cell)?;
        let parent = cell.parent()?;
        self.climate
            .get(usize::try_from(cell_index(parent)).expect("a cell index fits a usize"))
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        let run = self.index_of(cell).map_or(&[][..], |n| {
            &self.reaching[self.reaching_start[n]..self.reaching_start[n + 1]]
        });
        run.iter()
            .map(|&k| &self.craters[usize::try_from(k).expect("a crater index fits a usize")])
    }
}

impl std::fmt::Debug for CoarseField {
    /// The header and the counts: the records themselves are hundreds of thousands long.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoarseField")
            .field("header", &self.header)
            .field("synthesis_cells", &self.synthesis.len())
            .field("climate_cells", &self.climate.len())
            .field("craters", &self.craters.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube::{FaceUv, face_uv_to_xyz, st_to_uv, unit_dir};
    use crate::testing::{FieldBuilder, SyntheticWorld, synthetic_field};
    use hyperion_base::math;
    use hyperion_base::units::Gigayears;
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// Volumetric mean radii, m: Earth 6,371.0 km, Mars 3,389.5 km and the Moon 1,737.4 km (NASA's
    /// planetary fact sheets, Williams), and Ceres 469.7 km (Ermakov et al. 2017, JGR Planets 122,
    /// 2267, from Dawn's shape model).
    const EARTH_M: f64 = 6.371e6;
    const MARS_M: f64 = 3.3895e6;
    const MOON_M: f64 = 1.7374e6;
    const CERES_M: f64 = 4.697e5;

    fn mean_cell_km(radius_m: f64, level: CoarseLevel) -> f64 {
        let area = 4.0 * core::f64::consts::PI * radius_m * radius_m;
        (area / f64::from(level.cell_count())).sqrt() / 1e3
    }

    /// Design note 4's table: the level of each body and the brainstorm's mean cell sizes, about
    /// 36 km on an Earth, 72 km on a world of 2 R⊕, 38 km on a Mars, 39 km on the Moon and 21 km
    /// on Ceres, each within 5%.
    #[test]
    fn the_level_table_is_design_note_fours() {
        for (name, radius, level, cells, cell_km) in [
            ("Earth", EARTH_M, 8, 393_216, 36.0),
            ("2 R⊕", 2.0 * EARTH_M, 8, 393_216, 72.0),
            ("Mars", MARS_M, 7, 98_304, 38.0),
            ("Moon", MOON_M, 6, 24_576, 39.0),
            ("Ceres", CERES_M, 5, 6_144, 21.0),
        ] {
            let got = coarse_level(Metres::new(radius));
            assert_eq!(got.get(), level, "{name}");
            assert_eq!(got.cell_count(), cells, "{name}");
            assert_eq!(got.climate_cell_count(), cells / 4, "{name}");
            let size = mean_cell_km(radius, got);
            assert!(
                (size / cell_km - 1.0).abs() <= 0.05,
                "{name}: mean cell {size} km against {cell_km}"
            );
        }
        // The floor and the cap: a 10 km moonlet keeps level 5, a 10 R⊕ world level 8.
        assert_eq!(coarse_level(Metres::new(1e4)), CoarseLevel::MIN);
        assert_eq!(coarse_level(Metres::new(10.0 * EARTH_M)), CoarseLevel::MAX);
        assert_eq!(CoarseLevel::new(4), None);
        assert_eq!(CoarseLevel::new(9), None);
        assert_eq!(CoarseLevel::new(6).map(CoarseLevel::climate_level), Some(5));
    }

    /// The arc of the cube-sphere edge between two lattice points of face 0, on the unit sphere.
    fn arc(a: [f64; 3], b: [f64; 3]) -> f64 {
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let sine = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        math::atan2(sine, a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
    }

    /// `D_b` is twice the closed-form largest edge, (4 ÷ 3) √(1 + 3u*) ÷ (1 + u*²) at u* =
    /// (√31 − 2) ÷ 9 per unit of s, recomputed here, bit for bit; it is Design note 4's 84.9 km on
    /// an Earth, 90.3 on a Mars, 92.6 on the Moon and 50.0 on Ceres, each to 0.5 km; and the
    /// closed form is the realised largest edge of level 8, every edge of a face measured, to
    /// 10⁻⁴.
    #[test]
    fn the_boundary_diameter_is_twice_the_largest_edge() {
        let u = (31.0_f64.sqrt() - 2.0) / 9.0;
        let rate = 4.0 / 3.0 * (1.0 + 3.0 * u).sqrt() / (1.0 + u * u);
        assert!((rate - 1.704_897).abs() < 1e-6, "rate {rate}");
        for (radius, km) in [
            (EARTH_M, 84.9),
            (MARS_M, 90.3),
            (MOON_M, 92.6),
            (CERES_M, 50.0),
        ] {
            let level = coarse_level(Metres::new(radius));
            let d_b = boundary_diameter(level, Metres::new(radius)).value();
            let largest_edge = rate * radius / f64::from(1_u32 << level.get());
            assert_same_bits(d_b, 2.0 * largest_edge);
            assert!((d_b / 1e3 - km).abs() <= 0.5, "D_b {d_b} m against {km} km");
        }
        let cells = 256_u32;
        let corner = |i: u32, j: u32| {
            unit_dir(face_uv_to_xyz(FaceUv {
                face: Face::PosX,
                u: st_to_uv(f64::from(i) / f64::from(cells)),
                v: st_to_uv(f64::from(j) / f64::from(cells)),
            }))
        };
        let mut largest = 0.0_f64;
        for i in 0..cells {
            for j in 0..=cells {
                largest = largest.max(arc(corner(i, j), corner(i + 1, j)));
                largest = largest.max(arc(corner(j, i), corner(j, i + 1)));
            }
        }
        let realised = largest * f64::from(cells);
        assert!(
            (realised / rate - 1.0).abs() <= 1e-4,
            "realised largest edge {realised} × 2⁻⁸ against {rate}"
        );
    }

    /// Every index of a level is one cell's and back, every cell has an index below 6 · 4ᴸ, no two
    /// cells share one, and a cell's four children are four consecutive indices.
    #[test]
    fn cell_index_is_a_bijection_onto_the_cells() {
        for level in 0..=8_u8 {
            let count = cell_count(level);
            let mut seen = vec![false; usize::try_from(count).unwrap()];
            for face in Face::ALL {
                for i in 0..(1_u32 << level) {
                    for j in 0..(1_u32 << level) {
                        let cell = PatchKey::new(face, level, i, j).unwrap();
                        let index = cell_index(cell);
                        assert!(index < count, "{cell:?} → {index}");
                        let slot = &mut seen[usize::try_from(index).unwrap()];
                        assert!(!*slot, "{cell:?} shares index {index}");
                        *slot = true;
                        assert_eq!(cell_at_index(level, index), Some(cell));
                        if level < 8 {
                            let children = cell.children().map(cell_index);
                            assert_eq!(children, [0, 1, 2, 3].map(|k| 4 * index + k));
                        }
                    }
                }
            }
            assert!(seen.iter().all(|&s| s), "level {level} misses an index");
            assert_eq!(cell_at_index(level, count), None);
        }
        let deepest = PatchKey::new(Face::NegZ, 14, (1 << 14) - 1, (1 << 14) - 1).unwrap();
        assert_eq!(cell_index(deepest), cell_count(14) - 1);
        assert_eq!(cell_at_index(14, cell_count(14) - 1), Some(deepest));
        assert_eq!(cell_at_index(15, 0), None);
    }

    /// For every cell of the Moon-like world (dozens of craters, a basin reaching most of the
    /// body) and the one-crater world, `craters_reaching` yields exactly the craters whose reach
    /// holds the cell, in list order; and nothing for a key at another level.
    #[test]
    fn craters_reaching_yields_the_craters_whose_reach_holds_the_field_cell() {
        for world in [SyntheticWorld::MoonLike, SyntheticWorld::OneCrater] {
            let field = synthetic_field(world);
            let level = field.header().level();
            let mut reaches = 0_usize;
            for index in 0..level.cell_count() {
                let cell = cell_at_index(level.get(), index).unwrap();
                let got: Vec<*const CoarseCrater> = field
                    .craters_reaching(cell)
                    .map(std::ptr::from_ref)
                    .collect();
                let expected: Vec<*const CoarseCrater> = field
                    .craters()
                    .iter()
                    .filter(|c| c.reach.contains(index))
                    .map(std::ptr::from_ref)
                    .collect();
                assert_eq!(got, expected, "{world:?}, cell {index}");
                reaches += got.len();
            }
            assert!(reaches > 0, "{world:?} has no reach");
            let coarser = cell_at_index(level.get() - 1, 0).unwrap();
            assert_eq!(field.craters_reaching(coarser).count(), 0);
            assert!(field.cell(coarser).is_none());
            assert!(field.climate(coarser).is_none());
        }
    }

    /// A cell's climate is its parent's record, which its siblings share.
    #[test]
    fn a_field_cell_reads_its_parents_climate() {
        let field = synthetic_field(SyntheticWorld::CeresLike);
        let level = field.header().level().get();
        let cell = cell_at_index(level, 4_321).unwrap();
        let parent = cell.parent().unwrap();
        let record = &field.climate_layer()[usize::try_from(cell_index(parent)).unwrap()];
        for sibling in parent.children() {
            assert_eq!(field.climate(sibling), Some(record));
        }
        assert_eq!(
            field.cell(cell),
            Some(&field.synthesis()[usize::try_from(cell_index(cell)).unwrap()])
        );
    }

    fn flat_parts() -> (FieldHeader, Vec<SynthesisCell>, Vec<ClimateCell>) {
        let field = synthetic_field(SyntheticWorld::Flat);
        (
            field.header().clone(),
            field.synthesis().to_vec(),
            field.climate_layer().to_vec(),
        )
    }

    /// Each rule of a field's records is enforced, with its own error.
    #[test]
    fn a_field_refuses_records_that_break_their_rules() {
        let (header, synthesis, climate) = flat_parts();
        let short = synthesis[1..].to_vec();
        assert_eq!(
            CoarseField::new(header.clone(), short, climate.clone(), vec![]),
            Err(BuildFieldError::SynthesisCount {
                expected: 24_576,
                found: 24_575
            })
        );
        assert_eq!(
            CoarseField::new(header.clone(), synthesis.clone(), vec![], vec![]),
            Err(BuildFieldError::ClimateCount {
                expected: 6_144,
                found: 0
            })
        );
        let mut dry = synthesis.clone();
        dry[7].water_surface_mm = dry[7].elevation_mm - 1;
        assert_eq!(
            CoarseField::new(header.clone(), dry, climate.clone(), vec![]),
            Err(BuildFieldError::WaterBelowGround { cell: 7 })
        );
        let mut boundary = synthesis.clone();
        boundary[9].boundary = BoundaryKind::Transform;
        assert_eq!(
            CoarseField::new(header.clone(), boundary, climate.clone(), vec![]),
            Err(BuildFieldError::BoundaryWithoutDistance { cell: 9 })
        );
        // The flat world has a one-month year: a second month is outside it.
        let mut seasons = climate.clone();
        seasons[3].month_anomaly[1] = 2;
        assert_eq!(
            CoarseField::new(header, synthesis, seasons, vec![]),
            Err(BuildFieldError::MonthOutsideYear { cell: 3 })
        );
    }

    /// A crater below `D_b`, off unit length, of negative age, out of order, or whose reach misses
    /// its centre is refused.
    #[test]
    fn a_field_refuses_craters_that_are_not_its_own() {
        let one = synthetic_field(SyntheticWorld::OneCrater);
        let header = one.header().clone();
        let synthesis = one.synthesis().to_vec();
        let climate = one.climate_layer().to_vec();
        let crater = one.craters()[0].clone();
        let build = |craters: Vec<CoarseCrater>| {
            CoarseField::new(header.clone(), synthesis.clone(), climate.clone(), craters)
        };
        assert!(build(vec![crater.clone()]).is_ok());
        let mut small = crater.clone();
        small.diameter = header.boundary_diameter() * 0.99;
        assert_eq!(
            build(vec![small]),
            Err(BuildFieldError::CraterDiameter { crater: 0 })
        );
        let mut long = crater.clone();
        long.centre = long.centre.map(|c| c * 1.001);
        assert_eq!(
            build(vec![long]),
            Err(BuildFieldError::CraterCentre { crater: 0 })
        );
        let mut future = crater.clone();
        future.age = Gigayears::new(-0.1);
        assert_eq!(
            build(vec![future]),
            Err(BuildFieldError::CraterAge { crater: 0 })
        );
        let mut elsewhere = crater.clone();
        elsewhere.reach = Cover::from_cells([0]);
        assert_eq!(
            build(vec![elsewhere]),
            Err(BuildFieldError::CraterReach { crater: 0 })
        );
        let mut wider = crater.clone();
        wider.diameter = crater.diameter * 2.0;
        assert_eq!(
            build(vec![wider, crater]),
            Err(BuildFieldError::CratersUnsorted { crater: 1 })
        );
    }

    /// A built field's flow leads each routed cell to a lower edge neighbour, whose drainage is at
    /// least its own.
    #[test]
    fn a_built_field_routes_each_cell_to_a_lower_neighbour() {
        let field = synthetic_field(SyntheticWorld::MarsLike);
        let level = field.header().level().get();
        let mut routed = 0;
        for (index, record) in (0_u32..).zip(field.synthesis()) {
            let Some(edge) = record.flow.edge() else {
                continue;
            };
            let cell = cell_at_index(level, index).unwrap();
            let next = field.cell(cell.edge_neighbour(edge)).unwrap();
            assert!(next.elevation_mm < record.elevation_mm, "cell {index}");
            // The receiver drains its own cell as well, but a code is 0.07% wide, so a wide basin's
            // receiver may share its donor's code.
            assert!(next.drainage >= record.drainage, "cell {index}");
            routed += 1;
        }
        assert!(routed > 1_000, "only {routed} cells drain");
    }

    /// The radii at which the level rule switches, pinned to the bit: the largest radius of each
    /// level from 5 to 7 keeps it, and the next `f64` up takes the next level. A rewrite of the
    /// rule's arithmetic into another form moves them, which is a generator-version change.
    #[test]
    fn the_level_rule_switches_at_pinned_radii() {
        for (level, largest) in [
            (5, 884_465.022_646_618_9),
            (6, 1_768_930.045_293_237_8),
            (7, 3_537_860.090_586_475_6),
        ] {
            assert_eq!(coarse_level(Metres::new(largest)).get(), level);
            assert_eq!(
                coarse_level(Metres::new(largest.next_up())).get(),
                level + 1
            );
        }
    }

    /// Two craters of one centre cell and diameter share a key, so the second is refused; a
    /// one-month year's four winds must be one; and the albedo scale is set alone, validated.
    #[test]
    fn a_field_refuses_shared_crater_keys_and_seasonal_winds() {
        let one = synthetic_field(SyntheticWorld::OneCrater);
        let crater = one.craters()[0].clone();
        assert_eq!(
            CoarseField::new(
                one.header().clone(),
                one.synthesis().to_vec(),
                one.climate_layer().to_vec(),
                vec![crater.clone(), crater],
            ),
            Err(BuildFieldError::CratersUnsorted { crater: 1 })
        );
        let (header, synthesis, climate) = flat_parts();
        let mut windy = climate;
        windy[5].wind[2] = Wind {
            azimuth: 64,
            speed: 100,
        };
        assert_eq!(
            CoarseField::new(header, synthesis, windy, vec![]),
            Err(BuildFieldError::MonthOutsideYear { cell: 5 })
        );
        let lit = one.clone().with_albedo_scale(Some(0.9)).unwrap();
        assert_eq!(lit.header().albedo_scale(), Some(0.9));
        assert_eq!(lit.synthesis(), one.synthesis());
        assert_eq!(
            one.clone().with_albedo_scale(Some(0.0)),
            Err(BuildFieldHeaderError::NotPositive {
                part: "albedo scale"
            })
        );
        assert_eq!(
            one.with_albedo_scale(Some(f64::NAN)),
            Err(BuildFieldHeaderError::NotFinite {
                part: "albedo scale"
            })
        );
    }

    #[test]
    fn a_field_builder_without_craters_lists_none() {
        let field = FieldBuilder::new(Metres::new(CERES_M)).build();
        assert!(field.craters().is_empty());
        assert_eq!(field.header().level(), CoarseLevel::MIN);
        let level = field.header().level().get();
        let any = cell_at_index(level, 17).unwrap();
        assert_eq!(field.craters_reaching(any).count(), 0);
    }
}
