//! A coarse field's per-cell records and the codes they carry: the synthesis record of each cell
//! and the climate record of each cell of the level above (plan R09, Design note 17).
//!
//! Each record is plain data with public fields, the integer codes the payload carries; every
//! code's documentation gives its step and its SI meaning, and the records' methods read the
//! codes back in SI units. The rules that tie one field to another (a water surface at or above
//! the ground, a boundary distance only where there is a boundary) are checked where records enter
//! a field, by [`CoarseField::new`](super::CoarseField::new).

use hyperion_base::math;
use hyperion_base::units::{Metres, MetresPerSecond, Radians, SquareMetres, consts};

use crate::cube::Edge;

/// Why a byte is not the code of a field enum: no variant has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecodeFieldCodeError {
    /// The enum's name.
    pub kind: &'static str,
    /// The byte.
    pub code: u8,
}

impl std::fmt::Display for DecodeFieldCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is not a {} code", self.code, self.kind)
    }
}

impl std::error::Error for DecodeFieldCodeError {}

/// Why a value has no code: it is not finite, or it lies beyond what its code can hold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuantiseValueError {
    /// The value is a NaN or infinite.
    NotFinite(f64),
    /// The value is outside the code's range.
    OutOfRange(f64),
}

impl std::fmt::Display for QuantiseValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFinite(v) => write!(f, "{v} is not finite"),
            Self::OutOfRange(v) => write!(f, "{v} is outside its code's range"),
        }
    }
}

impl std::error::Error for QuantiseValueError {}

/// The code of `x` on a logarithmic scale of `per_octave` codes an octave, code `offset` standing
/// for 1: 0 for zero and for values that round below code 1, at most `max`.
fn log_code(x: f64, per_octave: f64, offset: f64, max: u32) -> Result<u32, QuantiseValueError> {
    if !x.is_finite() {
        return Err(QuantiseValueError::NotFinite(x));
    }
    if x < 0.0 {
        return Err(QuantiseValueError::OutOfRange(x));
    }
    // Zero, of either sign, has no logarithm and is code 0.
    if x <= 0.0 {
        return Ok(0);
    }
    let code = (per_octave * math::log2(x)).round() + offset;
    if code < 1.0 {
        return Ok(0);
    }
    if code > f64::from(max) {
        return Err(QuantiseValueError::OutOfRange(x));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "code is an integer from 1 to max, checked above"
    )]
    Ok(code as u32)
}

/// [`log_code`] for a one-byte code, at most 255.
fn log_code_u8(x: f64, per_octave: f64, offset: f64) -> Result<u8, QuantiseValueError> {
    let code = log_code(x, per_octave, offset, u32::from(u8::MAX))?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "log_code keeps the code to 255"
    )]
    Ok(code as u8)
}

/// [`log_code`] for a two-byte code, at most 65,535.
fn log_code_u16(x: f64, per_octave: f64, offset: f64) -> Result<u16, QuantiseValueError> {
    let code = log_code(x, per_octave, offset, u32::from(u16::MAX))?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "log_code keeps the code to 65,535"
    )]
    Ok(code as u16)
}

/// The value of `code` on [`log_code`]'s scale: 0 for code 0, else 2^((code − offset) ÷
/// `per_octave`).
#[must_use]
fn log_value(code: u32, per_octave: f64, offset: f64) -> f64 {
    if code == 0 {
        0.0
    } else {
        math::exp2((f64::from(code) - offset) / per_octave)
    }
}

coded_enum! {
    /// The crust under a cell, one byte (Design notes 6 and 7).
    ///
    /// A mobile-lid world's crust is two populations about sea level, as Earth2014 shows (its
    /// TBI2014 layer, area-weighted and split at −2 km: continental mean +406 m, s.d. 1.08 km;
    /// oceanic −4,279 m, s.d. 0.93 km; Hirt and Rexer 2015, Int. J. Appl. Earth Obs. Geoinf. 39,
    /// 103; Design note 7); a stagnant lid is one crust, with the volcanic provinces its volcanism
    /// built. The variants are structural, where and how a crust formed; what each is made of, its
    /// lithology, is the header's palette entry for it
    /// ([`FieldHeaderParts::crust_palette`](super::FieldHeaderParts::crust_palette);
    /// decision-composition §1.7), so a new rock is a palette entry, not a new code.
    Crust {
        /// Continental crust of a mobile-lid world.
        Continental = 0,
        /// Oceanic crust of a mobile-lid world.
        Oceanic = 1,
        /// A stagnant lid's crust, rock or ice by the body's material.
        Lid = 2,
        /// Volcanic plains and edifices a stagnant lid's volcanism built, such as the lunar maria
        /// or Tharsis (Design note 6's provinces).
        Province = 3,
    }
}

coded_enum! {
    /// The kind of the plate boundary nearest a cell, one byte, from the two plates' relative
    /// motion across it (Design note 6).
    BoundaryKind {
        /// No boundary: the body is a stagnant lid, which has none.
        Absent = 0,
        /// A convergent boundary where one plate subducts beneath the other.
        Subduction = 1,
        /// A convergent boundary where two continents collide.
        Collision = 2,
        /// A divergent boundary, a spreading ridge or a rift.
        Divergent = 3,
        /// A transform boundary, where the plates slide past each other.
        Transform = 4,
    }
}

coded_enum! {
    /// Where a cell's water goes, one byte: one of its four edge neighbours, or nowhere (Design
    /// note 9).
    ///
    /// The edges are the cell's own ([`Edge`]), on its own face's axes; the receiver is
    /// [`PatchKey::edge_neighbour`](crate::cube::PatchKey::edge_neighbour) of that edge, which may
    /// lie on the next face.
    FlowDirection {
        /// No receiver: the sea, a terminal outlet, or a world with no fluvial step.
        Terminal = 0,
        /// Into the neighbour towards i − 1.
        UMin = 1,
        /// Into the neighbour towards i + 1.
        UMax = 2,
        /// Into the neighbour towards j − 1.
        VMin = 3,
        /// Into the neighbour towards j + 1.
        VMax = 4,
    }
}

impl FlowDirection {
    /// The edge the water leaves by, or `None` for [`FlowDirection::Terminal`].
    #[must_use]
    pub const fn edge(self) -> Option<Edge> {
        match self {
            Self::Terminal => None,
            Self::UMin => Some(Edge::UMin),
            Self::UMax => Some(Edge::UMax),
            Self::VMin => Some(Edge::VMin),
            Self::VMax => Some(Edge::VMax),
        }
    }
}

impl From<Edge> for FlowDirection {
    fn from(edge: Edge) -> Self {
        match edge {
            Edge::UMin => Self::UMin,
            Edge::UMax => Self::UMax,
            Edge::VMin => Self::VMin,
            Edge::VMax => Self::VMax,
        }
    }
}

/// A cell's drainage area on a logarithmic scale, two bytes: the area upstream of the cell's
/// outflow, the cell's own included (Design note 9).
///
/// Code 0 is no drainage (a world with no fluvial step, or under 1 m²), and code c ≥ 1 is
/// 2^((c − 1) ÷ 1,024) m², 1,024 codes an octave, a step of 0.068%, from 1 m² to about 1.8 ×
/// 10¹⁹ m², far beyond any body's area. The coarse pass replaces the area by Hergarten's
/// runoff-weighted equivalent area where it has a climate (Design note 9), which is why the scale
/// reaches far below one cell's area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LogArea(u16);

/// [`LogArea`]'s codes an octave.
const AREA_PER_OCTAVE: f64 = 1_024.0;

impl LogArea {
    /// No drainage.
    pub const ZERO: Self = Self(0);

    /// The area of code `code`.
    #[must_use]
    pub const fn new(code: u16) -> Self {
        Self(code)
    }

    /// The code.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// The code of `area`, rounded to the nearest step; 0 for an area under 1 m².
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if `area` is not finite, is negative or is beyond the last code.
    pub fn from_area(area: SquareMetres) -> Result<Self, QuantiseValueError> {
        Ok(Self(log_code_u16(area.value(), AREA_PER_OCTAVE, 1.0)?))
    }

    /// The area, square metres.
    #[must_use]
    pub fn area(self) -> SquareMetres {
        SquareMetres::new(log_value(u32::from(self.0), AREA_PER_OCTAVE, 1.0))
    }
}

/// A cell's channel steepness index on a logarithmic scale, one byte: `k_s` = S A^θ, with S the
/// slope to the receiver, A the drainage area in square metres and θ = 0.45, so `k_s` is in m^0.9
/// (Design note 9, after Kirby and Whipple 2012).
///
/// Code 0 is no channel (a terminal cell, or under about 0.004 m^0.9), and code c ≥ 1 is
/// 2^((c − 64) ÷ 8) m^0.9, 8 codes an octave, a step of 9%, from 0.0043 to 1.5 × 10⁷ m^0.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LogSteepness(u8);

/// [`LogSteepness`]'s codes an octave.
const STEEPNESS_PER_OCTAVE: f64 = 8.0;

/// The [`LogSteepness`] code of 1 m^0.9.
const STEEPNESS_UNIT_CODE: f64 = 64.0;

impl LogSteepness {
    /// No channel.
    pub const ZERO: Self = Self(0);

    /// The concavity θ of `k_s` = S A^θ, with m = θ = 0.45 in the stream-power law: the
    /// conventional reference concavity (Design note 9, after Kirby and Whipple 2012, J. Struct.
    /// Geol. 44, 54; from memory there, and not yet checked against the paper).
    pub const THETA: f64 = 0.45;

    /// The steepness of code `code`.
    #[must_use]
    pub const fn new(code: u8) -> Self {
        Self(code)
    }

    /// The code.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The code of a steepness index of `k_s` m^0.9, rounded to the nearest step; 0 below the
    /// first code.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if `k_s` is not finite, is negative or is beyond the last code.
    pub fn from_index_m0_9(k_s: f64) -> Result<Self, QuantiseValueError> {
        Ok(Self(log_code_u8(
            k_s,
            STEEPNESS_PER_OCTAVE,
            STEEPNESS_UNIT_CODE,
        )?))
    }

    /// The steepness index, m^0.9.
    #[must_use]
    pub fn index_m0_9(self) -> f64 {
        log_value(u32::from(self.0), STEEPNESS_PER_OCTAVE, STEEPNESS_UNIT_CODE)
    }
}

/// A cell's climate or surface-state class, one byte (Design note 11), its codes split in three
/// (decision-composition §1.7):
///
/// - 0, [`SurfaceClass::UNCLASSIFIED`];
/// - 1 to 63, Köppen–Geiger's classes for the seasonal water-cycle regimes, the 30 of Peel,
///   Finlayson and McMahon's Table 1 (Hydrol. Earth Syst. Sci. 11, 1633, 2007) with room;
/// - 64 to 255, the surface-state forms and named zones of the other regimes, each a form (a
///   frozen sea, a glacier, a melt), whose substance is the cell's palette entry: its ice or
///   liquid entry ([`SynthesisCell::substances`]) or its crust's.
///
/// R09.T15, which classifies, assigns the codes in each range; until then every field's cells are
/// [`SurfaceClass::UNCLASSIFIED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SurfaceClass(u8);

/// Which of [`SurfaceClass`]'s three ranges a code is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurfaceClassRange {
    /// Code 0: not classified.
    Unclassified,
    /// Codes 1 to 63: a Köppen–Geiger class.
    KoppenGeiger,
    /// Codes 64 to 255: a surface-state form, of the cell's palette entry.
    SurfaceState,
}

impl SurfaceClass {
    /// No class: the pass has not classified the cell.
    pub const UNCLASSIFIED: Self = Self(0);

    /// The first Köppen–Geiger code, 1.
    pub const FIRST_KOPPEN_GEIGER: u8 = 1;

    /// The first surface-state code, 64; the Köppen–Geiger codes end below it.
    pub const FIRST_SURFACE_STATE: u8 = 64;

    /// The class of code `code`.
    #[must_use]
    pub const fn new(code: u8) -> Self {
        Self(code)
    }

    /// The code.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The range the code is in.
    #[must_use]
    pub const fn range(self) -> SurfaceClassRange {
        if self.0 >= Self::FIRST_SURFACE_STATE {
            SurfaceClassRange::SurfaceState
        } else if self.0 >= Self::FIRST_KOPPEN_GEIGER {
            SurfaceClassRange::KoppenGeiger
        } else {
            SurfaceClassRange::Unclassified
        }
    }
}

/// One cell's synthesis record: what the local synthesis reads of the coarse field there, 23 bytes
/// in the payload (Design note 17's 21, the substance byte of decision-composition §1.7 and the
/// sea floor's byte of `decision-r09-t5.md` item 5).
///
/// Plain data with public fields, each an integer code whose step and meaning its documentation
/// gives; the methods read them in SI units. Heights are along the normal of the header's
/// spheroid, positive outwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SynthesisCell {
    /// The ground's height above the datum, millimetres: the coarse pass's final elevation,
    /// reconstructed `σ_h` constrained to plan 14's (Design note 7).
    pub elevation_mm: i32,
    /// The great-circle distance to the nearest plate boundary, whole kilometres, saturating at
    /// ±32,767 km (Design note 17); [`SynthesisCell::NO_BOUNDARY_KM`] on a body with no
    /// boundaries. Zero on a boundary cell; negative on the plate that subducts beneath a
    /// subduction boundary, positive on every other side.
    pub boundary_distance_km: i16,
    /// The cell's plate, by the plates step's index; 0 on a stagnant lid.
    pub plate: u8,
    /// The crust under the cell.
    pub crust: Crust,
    /// The kind of the nearest boundary, [`BoundaryKind::Absent`] on a stagnant lid.
    pub boundary: BoundaryKind,
    /// The obliquity of the plates' relative motion at the nearest boundary: the acute angle
    /// between it and the boundary's normal, in steps of 90° ÷ 255, 0 for head-on convergence or
    /// divergence and 255 for pure strike-slip.
    pub boundary_obliquity: u8,
    /// Where the cell's water goes.
    pub flow: FlowDirection,
    /// The drainage area through the cell.
    pub drainage: LogArea,
    /// The channel steepness index `k_s`.
    pub steepness: LogSteepness,
    /// The height of the water surface over the cell above the datum, millimetres: the sea's or a
    /// lake's level where the cell is under water, and the ground's, `elevation_mm`, where it is
    /// dry. Never below the ground.
    pub water_surface_mm: i32,
    /// The share of the cell under ice, in steps of 1 ÷ 255: 0 is free of ice and 255 covered
    /// (Design note 8: ice is placed coldest first until its area matches plan 14's ice fraction).
    /// Its substance is the cell's ice entry.
    pub ice: u8,
    /// The cell's substances, indices into the header's palette: its ice entry in the high nibble
    /// and its liquid entry in the low, each [`NO_ENTRY`](super::NO_ENTRY) (0xF) for none
    /// ([`ice_entry`](Self::ice_entry), [`liquid_entry`](Self::liquid_entry),
    /// [`pack_substances`](Self::pack_substances)). A cell with an ice share names an `Ice`
    /// entry, and a cell under water a `Liquid` entry; a cell may name an entry it holds none of
    /// (a seasonal frost's), but never hold a share it does not name.
    pub substances: u8,
    /// The cell's climate or surface-state class.
    pub class: SurfaceClass,
    /// The cell's crater state, whose codes R09.T15 defines with the classes (Design note 11); 0
    /// until then.
    pub crater_state: u8,
    /// The sea floor's roughness inputs, which the coarse pass derives from the cell's own ridge
    /// (R09.T11 and T12.a; `decision-r09-t5.md` item 5): the draped abyssal-hill relief H in the
    /// high nibble and the ponded (turbidite) sediment's thickness `S_t` in the low
    /// ([`hill_relief`](Self::hill_relief), [`ponded_sediment`](Self::ponded_sediment),
    /// [`pack_seafloor`](Self::pack_seafloor)).
    ///
    /// The hill nibble is code 0 for none and c in 1 to 15 for 20 m × 2^((c − 1) ÷ 3), 20 to
    /// 508 m, and is non-zero exactly on [`Crust::Oceanic`]. The sediment nibble is code 0 for
    /// under 21 m and c in 1 to 15 for 25 m × 2^((c − 1) ÷ 2), 25 m to 3.2 km, and is zero off
    /// oceanic crust: reserved there for the sedimented basins of land (Mars's northern plains
    /// among them), which a later task may fill under the layout rule. The age and the spreading
    /// rate that make them do not travel: the synthesis needs only what they make.
    pub seafloor: u8,
}

/// Millimetres a metre.
const MM_PER_M: f64 = 1_000.0;

/// The largest code of a nibble.
const NIBBLE_MAX: u8 = 0x0F;

/// The hill relief of the sea floor byte's code 1, metres: code c ≥ 1 is 20 m × 2^((c − 1) ÷ 3), a
/// step of 26%, from 20 m to 508 m at code 15, which spans the draped abyssal hills from a fast
/// ridge's floor under its full drape (H₀ ÷ 2, 28 m) to an ultraslow ridge's 286 m and beyond
/// (`decision-r09-t5.md` item 5).
const HILL_RELIEF_UNIT_M: f64 = 20.0;

/// The hill relief's codes an octave, 3 ([`HILL_RELIEF_UNIT_M`]).
const HILL_RELIEF_PER_OCTAVE: f64 = 3.0;

/// The ponded sediment of the sea floor byte's code 1, metres: code c ≥ 1 is 25 m × 2^((c − 1) ÷
/// 2), a step of 41%, from 25 m to 3.2 km at code 15, which spans the turbidite wedge from a thin
/// cover over the smoothest hills to the continental rise's 2.4 km (`decision-r09-t5.md` item 5).
const PONDED_SEDIMENT_UNIT_M: f64 = 25.0;

/// The ponded sediment's codes an octave, 2 ([`PONDED_SEDIMENT_UNIT_M`]).
const PONDED_SEDIMENT_PER_OCTAVE: f64 = 2.0;

/// The nibble code of a length `x` metres on a scale of `per_octave` codes an octave from `unit`
/// at code 1, nearest in log and saturating at 15: `None` for zero and for a length that rounds
/// below code 1, which the caller codes as it must.
fn nibble_code(x: f64, unit: f64, per_octave: f64) -> Result<Option<u8>, QuantiseValueError> {
    if !x.is_finite() {
        return Err(QuantiseValueError::NotFinite(x));
    }
    if x < 0.0 {
        return Err(QuantiseValueError::OutOfRange(x));
    }
    // Zero, of either sign, has no logarithm.
    if x <= 0.0 {
        return Ok(None);
    }
    let code = (per_octave * math::log2(x / unit)).round() + 1.0;
    if code < 1.0 {
        return Ok(None);
    }
    if code >= f64::from(NIBBLE_MAX) {
        return Ok(Some(NIBBLE_MAX));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "code is an integer from 1 to 14, checked above"
    )]
    Ok(Some(code as u8))
}

/// The draped abyssal-hill relief of a sea floor byte `seafloor`, or `None` where its hill nibble
/// is 0 ([`SynthesisCell::hill_relief`]).
#[must_use]
pub(crate) fn hill_relief_of(seafloor: u8) -> Option<Metres> {
    let code = seafloor >> 4;
    (code != 0).then(|| {
        Metres::new(HILL_RELIEF_UNIT_M * log_value(u32::from(code), HILL_RELIEF_PER_OCTAVE, 1.0))
    })
}

/// The ponded sediment's thickness of a sea floor byte `seafloor`, zero where its sediment nibble
/// is 0 ([`SynthesisCell::ponded_sediment`]).
#[must_use]
pub(crate) fn ponded_sediment_of(seafloor: u8) -> Metres {
    let code = seafloor & NIBBLE_MAX;
    Metres::new(
        PONDED_SEDIMENT_UNIT_M * log_value(u32::from(code), PONDED_SEDIMENT_PER_OCTAVE, 1.0),
    )
}

impl SynthesisCell {
    /// The [`boundary_distance_km`](Self::boundary_distance_km) of a cell on a body with no plate
    /// boundaries, outside the ±32,767 km a distance saturates at.
    pub const NO_BOUNDARY_KM: i16 = i16::MIN;

    /// The [`substances`](Self::substances) of a cell that names no ice and no liquid: 0xFF.
    pub const NO_SUBSTANCES: u8 = 0xFF;

    /// The substance byte of an ice entry `ice` and a liquid entry `liquid`, palette indices, or
    /// `None` if either is 15 or more, which a nibble cannot name beside its "none".
    #[must_use]
    pub const fn pack_substances(ice: Option<u8>, liquid: Option<u8>) -> Option<u8> {
        let ice = match ice {
            None => super::NO_ENTRY,
            Some(i) if i < super::NO_ENTRY => i,
            Some(_) => return None,
        };
        let liquid = match liquid {
            None => super::NO_ENTRY,
            Some(l) if l < super::NO_ENTRY => l,
            Some(_) => return None,
        };
        Some((ice << 4) | liquid)
    }

    /// The palette index of the cell's ice entry, or `None`.
    #[must_use]
    pub const fn ice_entry(&self) -> Option<u8> {
        let nibble = self.substances >> 4;
        if nibble == super::NO_ENTRY {
            None
        } else {
            Some(nibble)
        }
    }

    /// The palette index of the cell's liquid entry, or `None`.
    #[must_use]
    pub const fn liquid_entry(&self) -> Option<u8> {
        let nibble = self.substances & 0x0F;
        if nibble == super::NO_ENTRY {
            None
        } else {
            Some(nibble)
        }
    }

    /// The [`seafloor`](Self::seafloor) of a cell that is not oceanic crust: no hill relief and
    /// no ponded sediment, 0.
    pub const NO_SEAFLOOR: u8 = 0;

    /// The sea floor byte of a hill-relief code `hill` ([`quantise_hill_relief`]) and a
    /// ponded-sediment code `ponded` ([`quantise_ponded_sediment`]), or `None` if either is above
    /// 15, which a nibble cannot hold.
    ///
    /// [`quantise_hill_relief`]: Self::quantise_hill_relief
    /// [`quantise_ponded_sediment`]: Self::quantise_ponded_sediment
    #[must_use]
    pub const fn pack_seafloor(hill: u8, ponded: u8) -> Option<u8> {
        if hill > NIBBLE_MAX || ponded > NIBBLE_MAX {
            None
        } else {
            Some((hill << 4) | ponded)
        }
    }

    /// The draped abyssal-hill relief H of the cell's sea floor, metres: the RMS height of its
    /// basement hills less what its pelagic drape smooths (`decision-r09-t5.md` item 5), or
    /// `None` where the hill nibble is 0, which a valid record has exactly off oceanic crust.
    #[must_use]
    pub fn hill_relief(&self) -> Option<Metres> {
        hill_relief_of(self.seafloor)
    }

    /// The thickness of the ponded (turbidite) sediment over the cell's sea floor, metres: zero
    /// under 21 m and off oceanic crust (`decision-r09-t5.md` item 5).
    #[must_use]
    pub fn ponded_sediment(&self) -> Metres {
        ponded_sediment_of(self.seafloor)
    }

    /// The hill-relief code of a draped abyssal-hill relief `relief`: 0 for none (zero), else the
    /// code from 1 to 15 nearest in log, saturating at both ends, so that every relief of an
    /// oceanic cell has a code.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if `relief` is not finite or is negative.
    pub fn quantise_hill_relief(relief: Metres) -> Result<u8, QuantiseValueError> {
        let h = relief.value();
        Ok(
            match nibble_code(h, HILL_RELIEF_UNIT_M, HILL_RELIEF_PER_OCTAVE)? {
                Some(code) => code,
                None if h > 0.0 => 1,
                None => 0,
            },
        )
    }

    /// The ponded-sediment code of a thickness `thickness`: 0 under 21 m (half a step below code
    /// 1's 25 m), else the code from 1 to 15 nearest in log, saturating at 15.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if `thickness` is not finite or is negative.
    pub fn quantise_ponded_sediment(thickness: Metres) -> Result<u8, QuantiseValueError> {
        Ok(nibble_code(
            thickness.value(),
            PONDED_SEDIMENT_UNIT_M,
            PONDED_SEDIMENT_PER_OCTAVE,
        )?
        .unwrap_or(0))
    }

    /// The ground's height above the datum, metres.
    #[must_use]
    pub fn elevation(&self) -> Metres {
        Metres::new(f64::from(self.elevation_mm) / MM_PER_M)
    }

    /// The water surface's height above the datum, metres.
    #[must_use]
    pub fn water_surface(&self) -> Metres {
        Metres::new(f64::from(self.water_surface_mm) / MM_PER_M)
    }

    /// Whether the cell is under water: its water surface is above its ground.
    #[must_use]
    pub const fn is_under_water(&self) -> bool {
        self.water_surface_mm > self.elevation_mm
    }

    /// The signed distance to the nearest plate boundary, metres, or `None` on a body with none.
    #[must_use]
    pub fn boundary_distance(&self) -> Option<Metres> {
        (self.boundary_distance_km != Self::NO_BOUNDARY_KM)
            .then(|| Metres::new(f64::from(self.boundary_distance_km) * 1e3))
    }

    /// The obliquity of the relative motion at the nearest boundary, radians, 0 to π ÷ 2.
    #[must_use]
    pub fn boundary_obliquity_angle(&self) -> Radians {
        Radians::new(f64::from(self.boundary_obliquity) * core::f64::consts::FRAC_PI_2 / 255.0)
    }

    /// The share of the cell under ice, 0 to 1.
    #[must_use]
    pub fn ice_fraction(&self) -> f64 {
        f64::from(self.ice) / 255.0
    }

    /// The code of a height `height` above the datum: whole millimetres, rounded half away from
    /// zero.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if `height` is not finite or beyond ±2,147 km, an `i32` of
    /// millimetres.
    pub fn quantise_height(height: Metres) -> Result<i32, QuantiseValueError> {
        let h = height.value();
        if !h.is_finite() {
            return Err(QuantiseValueError::NotFinite(h));
        }
        let mm = (h * MM_PER_M).round();
        if mm < f64::from(i32::MIN) || mm > f64::from(i32::MAX) {
            return Err(QuantiseValueError::OutOfRange(h));
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "mm is an integer within the i32 range, checked above"
        )]
        Ok(mm as i32)
    }

    /// The code of a signed boundary distance, whole kilometres rounded half away from zero and
    /// saturating at ±32,767 km, or [`NO_BOUNDARY_KM`](Self::NO_BOUNDARY_KM) for `None`.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError::NotFinite`] if the distance is a NaN.
    pub fn quantise_boundary_distance(distance: Option<Metres>) -> Result<i16, QuantiseValueError> {
        let Some(d) = distance else {
            return Ok(Self::NO_BOUNDARY_KM);
        };
        let d = d.value();
        if d.is_nan() {
            return Err(QuantiseValueError::NotFinite(d));
        }
        let km = (d / 1e3)
            .round()
            .clamp(-f64::from(i16::MAX), f64::from(i16::MAX));
        #[expect(
            clippy::cast_possible_truncation,
            reason = "km is an integer clamped to ±32,767"
        )]
        Ok(km as i16)
    }

    /// The code of a boundary obliquity `angle`, 0 to π ÷ 2, in steps of 90° ÷ 255.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if `angle` is not finite or outside 0 to π ÷ 2.
    pub fn quantise_obliquity(angle: Radians) -> Result<u8, QuantiseValueError> {
        let a = angle.value();
        unit_code(a / core::f64::consts::FRAC_PI_2).map_err(|_| {
            if a.is_finite() {
                QuantiseValueError::OutOfRange(a)
            } else {
                QuantiseValueError::NotFinite(a)
            }
        })
    }

    /// The code of an ice-covered share `fraction`, 0 to 1, in steps of 1 ÷ 255.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if `fraction` is not finite or outside 0 to 1.
    pub fn quantise_ice(fraction: f64) -> Result<u8, QuantiseValueError> {
        unit_code(fraction)
    }
}

/// The code of `x` in 0 to 1 in steps of 1 ÷ 255, rounded.
fn unit_code(x: f64) -> Result<u8, QuantiseValueError> {
    if !x.is_finite() {
        return Err(QuantiseValueError::NotFinite(x));
    }
    if !(0.0..=1.0).contains(&x) {
        return Err(QuantiseValueError::OutOfRange(x));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "x × 255 rounded is an integer from 0 to 255"
    )]
    Ok((x * 255.0).round() as u8)
}

/// A month's mean precipitation rate on a logarithmic scale, one byte (Design note 8).
///
/// The rate is the condensable's mass flux onto the surface, kg m⁻² s⁻¹ (for water, 1 kg m⁻² is a
/// millimetre). Code 0 is none (under about 0.1 mm a year), and code c ≥ 1 is 0.1 mm per Julian
/// year × 2^((c − 1) ÷ 12), 12 codes an octave, a step of 5.9%, to about 2.4 × 10⁵ mm a year
/// (240 m), so that a hyper-arid desert and a monsoon both have codes. Köppen–Geiger's
/// thresholds read rates, per 30.44 d and per 365.25 d (Design note 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LogPrecipitation(u8);

/// [`LogPrecipitation`]'s codes an octave.
const PRECIPITATION_PER_OCTAVE: f64 = 12.0;

/// The rate of [`LogPrecipitation`]'s code 1, kg m⁻² s⁻¹: 0.1 mm of water a Julian year.
const PRECIPITATION_UNIT_KG_M2_S: f64 = 0.1 / consts::SECONDS_PER_JULIAN_YEAR;

impl LogPrecipitation {
    /// No precipitation.
    pub const NONE: Self = Self(0);

    /// The rate of code `code`.
    #[must_use]
    pub const fn new(code: u8) -> Self {
        Self(code)
    }

    /// The code.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The code of a rate of `kg_per_m2_s` kg m⁻² s⁻¹, rounded to the nearest step; 0 below the
    /// first code.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if the rate is not finite, is negative or is beyond the last code.
    pub fn from_rate(kg_per_m2_s: f64) -> Result<Self, QuantiseValueError> {
        let code = log_code_u8(
            kg_per_m2_s / PRECIPITATION_UNIT_KG_M2_S,
            PRECIPITATION_PER_OCTAVE,
            1.0,
        )
        .map_err(|e| match e {
            QuantiseValueError::NotFinite(_) => QuantiseValueError::NotFinite(kg_per_m2_s),
            QuantiseValueError::OutOfRange(_) => QuantiseValueError::OutOfRange(kg_per_m2_s),
        })?;
        Ok(Self(code))
    }

    /// The rate, kg m⁻² s⁻¹.
    #[must_use]
    pub fn rate_kg_per_m2_s(self) -> f64 {
        PRECIPITATION_UNIT_KG_M2_S * log_value(u32::from(self.0), PRECIPITATION_PER_OCTAVE, 1.0)
    }
}

/// A month's prevailing wind 10 m above the ground, two bytes: where the air moves and how fast
/// (Design note 8).
///
/// `azimuth` is the direction the air moves towards, clockwise from local north through east, in
/// 256 steps of 1.406 25°: north is towards the body-fixed +z pole along the surface, east is
/// +z × up; at either pole, where north has no direction, the azimuth is from the body-fixed +x
/// meridian's direction. `speed` is code 0 for calm (under about 0.01 m/s) and code c ≥ 1 for
/// 0.01 m/s × 2^((c − 1) ÷ 16), 16 codes an octave, a step of 4.4%, to about 600 m/s: the
/// relative precision a sea state wants from a Titan breeze of 0.3 m/s to a gale, since R11's
/// Cox–Munk mean-square slope is linear in the speed (Cox and Munk 1954, J. Opt. Soc. Am. 44,
/// 838–850).
///
/// In a climate record ([`ClimateCell::wind`]) the height is 10 m, the WMO's standard exposure
/// for surface wind (WMO-No. 8, the CIMO Guide, 2008 edition, Part I, chapter 5, §5.9.2), which
/// R11's sea state converts to its own height by a log profile. The azimuth is the direction of
/// the month's resultant (vector-mean) wind, which cloud advection and ripples follow, and the
/// speed is the month's mean scalar speed, which the linear Cox–Munk law wants
/// (`decision-r09-t2.md` item 1). Design note 8's three-cell heuristic has no transients, so the
/// two speeds coincide today; the definition keeps a record right under a model that has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Wind {
    /// The direction the air moves towards, in 256ths of a turn clockwise from local north.
    pub azimuth: u8,
    /// The speed on the logarithmic scale above.
    pub speed: u8,
}

/// [`Wind`]'s speed codes an octave.
const SPEED_PER_OCTAVE: f64 = 16.0;

/// The speed of [`Wind`]'s speed code 1, m/s.
const SPEED_UNIT_M_S: f64 = 0.01;

impl Wind {
    /// Still air.
    pub const CALM: Self = Self {
        azimuth: 0,
        speed: 0,
    };

    /// The wind of `speed` towards `azimuth` (clockwise from local north, any finite angle),
    /// each rounded to the nearest step; calm below the first speed code.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError`] if either is not finite, or the speed is negative or beyond the
    /// last code.
    pub fn from_velocity(
        speed: MetresPerSecond,
        azimuth: Radians,
    ) -> Result<Self, QuantiseValueError> {
        let v = speed.value();
        let speed =
            log_code_u8(v / SPEED_UNIT_M_S, SPEED_PER_OCTAVE, 1.0).map_err(|e| match e {
                QuantiseValueError::NotFinite(_) => QuantiseValueError::NotFinite(v),
                QuantiseValueError::OutOfRange(_) => QuantiseValueError::OutOfRange(v),
            })?;
        let a = azimuth.value();
        if !a.is_finite() {
            return Err(QuantiseValueError::NotFinite(a));
        }
        let steps = (a / core::f64::consts::TAU * 256.0).round();
        if steps.abs() > 1e15 {
            return Err(QuantiseValueError::OutOfRange(a));
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "steps is an integer below 10^15 in magnitude, checked above"
        )]
        let turn = (steps as i64).rem_euclid(256);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a Euclidean remainder of 256 is 0 to 255"
        )]
        let azimuth = turn as u8;
        Ok(Self { azimuth, speed })
    }

    /// The direction the air moves towards, radians clockwise from local north, 0 to 2π.
    #[must_use]
    pub fn azimuth_angle(self) -> Radians {
        Radians::new(f64::from(self.azimuth) * core::f64::consts::TAU / 256.0)
    }

    /// The speed.
    #[must_use]
    pub fn speed(self) -> MetresPerSecond {
        MetresPerSecond::new(
            SPEED_UNIT_M_S * log_value(u32::from(self.speed), SPEED_PER_OCTAVE, 1.0),
        )
    }
}

/// One climate record, for a cell of the climate layer's level, L − 1, which its four children at
/// the field's level share: 50 bytes in the payload (Design note 17).
///
/// Plain data with public fields; the temperatures' codes are in the header's per-body steps, and
/// [`FieldHeader::sea_level_temperature`](super::FieldHeader::sea_level_temperature) and
/// [`FieldHeader::month_temperature`](super::FieldHeader::month_temperature) read them. The
/// layer is interpolated from the energy-balance grid and carries nothing finer but the lapse
/// term, which the synthesis re-applies from the header's rate and a point's elevation. Its three
/// monthly fields are on the header's months ([`month_edges`](super::month_edges)), which a
/// reader blends with [`month_blend`](super::month_blend).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClimateCell {
    /// The annual mean surface temperature, reduced to sea level by the header's lapse rate, as
    /// its departure from the header's reference temperature in the header's temperature steps
    /// (0.01 K × 2ⁿ).
    pub sea_level_temperature: i16,
    /// Each month's mean temperature less the annual mean, in the header's anomaly steps (0.25 K ×
    /// 2ⁿ), months from periapsis; 0 past the header's months, and 0 in a one-month year.
    pub month_anomaly: [i8; 12],
    /// Each month's mean precipitation rate, the labelled heuristic of the header's precipitation
    /// source; [`LogPrecipitation::NONE`] past the header's months.
    pub month_precipitation: [LogPrecipitation; 12],
    /// Each month's 10 m wind ([`Wind`]: the direction of the month's resultant and its mean
    /// speed), the wind its orographic step carried vapour along, so that the rain shadows and the
    /// winds agree month by month, for R11's clouds and sea state; [`Wind::CALM`] past the
    /// header's months, so a one-month year's eleven later winds are calm.
    pub wind: [Wind; 12],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{
        ClimateModelKind, MechanicsFamily, Morphology, PaletteRole, PrecipitationSource,
    };
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn round_trips<T>(all: &[T])
    where
        T: Copy + PartialEq + std::fmt::Debug + TryFrom<u8, Error = DecodeFieldCodeError>,
        u8: From<T>,
    {
        let mut known = 0;
        for code in 0..=u8::MAX {
            match T::try_from(code) {
                Ok(value) => {
                    assert_eq!(u8::from(value), code);
                    known += 1;
                }
                Err(e) => assert_eq!(e.code, code),
            }
        }
        assert_eq!(known, all.len());
        for (code, &value) in (0_u8..).zip(all) {
            assert_eq!(u8::from(value), code, "{value:?}");
        }
    }

    /// Every field enum's codes are 0, 1, … in its order, each one byte and back, and every other
    /// byte is refused.
    #[test]
    fn field_enums_round_trip_through_their_codes() {
        round_trips(Crust::ALL);
        round_trips(BoundaryKind::ALL);
        round_trips(FlowDirection::ALL);
        round_trips(Morphology::ALL);
        round_trips(ClimateModelKind::ALL);
        round_trips(PrecipitationSource::ALL);
        round_trips(PaletteRole::ALL);
        round_trips(MechanicsFamily::ALL);
        assert_eq!(
            Crust::try_from(4),
            Err(DecodeFieldCodeError {
                kind: "Crust",
                code: 4
            })
        );
        for edge in Edge::ALL {
            assert_eq!(FlowDirection::from(edge).edge(), Some(edge));
        }
        assert_eq!(FlowDirection::Terminal.edge(), None);
    }

    fn relative(a: f64, b: f64) -> f64 {
        (a / b - 1.0).abs()
    }

    /// Each logarithmic code returns its value to within half a step, zero is code 0, and values
    /// beyond the last code are refused.
    #[test]
    fn field_log_codes_round_trip_to_half_a_step() {
        for area in [1.0, 2.5e3, 1.3e9, 5.1e14, 2.0e15] {
            let back = LogArea::from_area(SquareMetres::new(area)).unwrap().area();
            assert!(relative(back.value(), area) <= 3.4e-4, "{area} m²");
        }
        assert_eq!(LogArea::from_area(SquareMetres::ZERO), Ok(LogArea::ZERO));
        assert_eq!(
            LogArea::from_area(SquareMetres::new(0.5)),
            Ok(LogArea::ZERO)
        );
        assert_eq!(LogArea::new(1).area(), SquareMetres::new(1.0));
        assert_eq!(
            LogArea::from_area(SquareMetres::new(-1.0)),
            Err(QuantiseValueError::OutOfRange(-1.0))
        );
        for k_s in [0.01, 1.0, 47.0, 1e6] {
            let back = LogSteepness::from_index_m0_9(k_s).unwrap().index_m0_9();
            assert!(relative(back, k_s) <= 0.045, "k_s {k_s}");
        }
        assert_same_bits(LogSteepness::new(64).index_m0_9(), 1.0);
        assert_eq!(LogSteepness::from_index_m0_9(0.0), Ok(LogSteepness::ZERO));
        let mm_per_year = 1.0 / consts::SECONDS_PER_JULIAN_YEAR;
        for mm in [0.1, 1.0, 250.0, 2_000.0, 1e5] {
            let rate = mm * mm_per_year;
            let back = LogPrecipitation::from_rate(rate)
                .unwrap()
                .rate_kg_per_m2_s();
            assert!(relative(back, rate) <= 0.03, "{mm} mm a year");
        }
        assert_eq!(LogPrecipitation::from_rate(0.0), Ok(LogPrecipitation::NONE));
        assert_eq!(
            LogPrecipitation::from_rate(1e7 * mm_per_year),
            Err(QuantiseValueError::OutOfRange(1e7 * mm_per_year))
        );
        for v in [0.3, 7.0, 45.0] {
            let wind = Wind::from_velocity(MetresPerSecond::new(v), Radians::new(1.0)).unwrap();
            assert!(relative(wind.speed().value(), v) <= 0.022, "{v} m/s");
            let step = core::f64::consts::TAU / 256.0;
            assert!((wind.azimuth_angle().value() - 1.0).abs() <= step / 2.0);
        }
        let west = Wind::from_velocity(
            MetresPerSecond::new(5.0),
            Radians::new(-core::f64::consts::FRAC_PI_2),
        )
        .unwrap();
        assert_eq!(west.azimuth, 192);
        assert_eq!(
            Wind::from_velocity(MetresPerSecond::ZERO, Radians::ZERO),
            Ok(Wind::CALM)
        );
    }

    /// Whether `result` refuses a NaN as not finite.
    fn refuses_nan<T>(result: &Result<T, QuantiseValueError>) -> bool {
        matches!(result, Err(QuantiseValueError::NotFinite(v)) if v.is_nan())
    }

    /// Every quantiser refuses a NaN as not finite, and a value past its code's range as out of
    /// range.
    #[test]
    fn field_quantisers_refuse_what_no_code_holds() {
        let nan = f64::NAN;
        let speed = |v: f64, a: f64| Wind::from_velocity(MetresPerSecond::new(v), Radians::new(a));
        assert!(refuses_nan(&SynthesisCell::quantise_height(Metres::new(
            nan
        ))));
        assert!(refuses_nan(&SynthesisCell::quantise_boundary_distance(
            Some(Metres::new(nan))
        )));
        assert!(refuses_nan(&SynthesisCell::quantise_obliquity(
            Radians::new(nan)
        )));
        assert!(refuses_nan(&SynthesisCell::quantise_ice(nan)));
        assert!(refuses_nan(&LogArea::from_area(SquareMetres::new(nan))));
        assert!(refuses_nan(&LogSteepness::from_index_m0_9(nan)));
        assert!(refuses_nan(&LogPrecipitation::from_rate(nan)));
        assert!(refuses_nan(&speed(nan, 0.0)));
        assert!(refuses_nan(&speed(1.0, nan)));
        assert_eq!(
            SynthesisCell::quantise_ice(1.5),
            Err(QuantiseValueError::OutOfRange(1.5))
        );
        assert_eq!(
            SynthesisCell::quantise_obliquity(Radians::new(2.0)),
            Err(QuantiseValueError::OutOfRange(2.0))
        );
        assert_eq!(
            LogSteepness::from_index_m0_9(1e9),
            Err(QuantiseValueError::OutOfRange(1e9))
        );
        // 10 km/s is 16 log₂(10⁶) + 1 = 320 codes, past the last.
        assert_eq!(speed(1e4, 0.0), Err(QuantiseValueError::OutOfRange(1e4)));
        assert_eq!(speed(-1.0, 0.0), Err(QuantiseValueError::OutOfRange(-1.0)));
    }

    /// Heights are whole millimetres, distances whole kilometres saturating at ±32,767 km, and the
    /// two shares are 255ths, each read back in SI units.
    #[test]
    fn field_cell_codes_read_back_in_si_units() {
        assert_eq!(
            SynthesisCell::quantise_height(Metres::new(-4_279.000_6)),
            Ok(-4_279_001)
        );
        assert_eq!(
            SynthesisCell::quantise_height(Metres::new(3e6)),
            Err(QuantiseValueError::OutOfRange(3e6))
        );
        assert_eq!(
            SynthesisCell::quantise_boundary_distance(Some(Metres::new(-1_499.0))),
            Ok(-1)
        );
        assert_eq!(
            SynthesisCell::quantise_boundary_distance(Some(Metres::new(1e12))),
            Ok(i16::MAX)
        );
        assert_eq!(
            SynthesisCell::quantise_boundary_distance(None),
            Ok(SynthesisCell::NO_BOUNDARY_KM)
        );
        assert_eq!(
            SynthesisCell::quantise_obliquity(Radians::new(core::f64::consts::FRAC_PI_2)),
            Ok(255)
        );
        assert_eq!(
            SynthesisCell::quantise_obliquity(Radians::new(-0.1)),
            Err(QuantiseValueError::OutOfRange(-0.1))
        );
        assert_eq!(SynthesisCell::quantise_ice(0.5), Ok(128));
        let cell = SynthesisCell {
            elevation_mm: -2_500,
            boundary_distance_km: -120,
            plate: 3,
            crust: Crust::Oceanic,
            boundary: BoundaryKind::Subduction,
            boundary_obliquity: 0,
            flow: FlowDirection::Terminal,
            drainage: LogArea::ZERO,
            steepness: LogSteepness::ZERO,
            water_surface_mm: 0,
            ice: 255,
            substances: 0x23,
            class: SurfaceClass::UNCLASSIFIED,
            crater_state: 0,
            seafloor: 0x93,
        };
        assert_eq!(cell.elevation(), Metres::new(-2.5));
        assert_eq!(cell.ice_entry(), Some(2));
        assert_eq!(cell.liquid_entry(), Some(3));
        assert_eq!(cell.water_surface(), Metres::ZERO);
        assert!(cell.is_under_water());
        assert_eq!(cell.boundary_distance(), Some(Metres::new(-120_000.0)));
        assert_same_bits(cell.ice_fraction(), 1.0);
        assert_eq!(cell.boundary_obliquity_angle(), Radians::ZERO);
        // Hill code 9 is 20 m × 2^(8 ÷ 3), and sediment code 3 is 25 m × 2.
        let hills = cell.hill_relief().unwrap().value();
        assert!(
            (hills - 20.0 * math::exp2(8.0 / 3.0)).abs() < 1e-12,
            "{hills}"
        );
        assert_same_bits(cell.ponded_sediment().value(), 50.0);
    }

    /// The sea floor byte packs a hill-relief code in the high nibble and a ponded-sediment code in
    /// the low; each scale returns its value to within half a step in log, saturates at its ends,
    /// and refuses what no code holds.
    #[test]
    fn field_cell_seafloor_packs_two_log_nibbles() {
        let hill = |m: f64| SynthesisCell::quantise_hill_relief(Metres::new(m));
        let ponded = |m: f64| SynthesisCell::quantise_ponded_sediment(Metres::new(m));
        let cell = |byte| SynthesisCell {
            seafloor: byte,
            ..field_test_cell()
        };
        // Every code reads back its own value, and its value quantises to it.
        for code in 0..=NIBBLE_MAX {
            let byte = SynthesisCell::pack_seafloor(code, code).unwrap();
            assert_eq!(byte, code * 0x11);
            let at = cell(byte);
            match at.hill_relief() {
                None => assert_eq!(code, 0),
                Some(h) => {
                    let expected = 20.0 * math::exp2(f64::from(code - 1) / 3.0);
                    assert!((h.value() / expected - 1.0).abs() < 1e-15, "{code}");
                    assert_eq!(hill(h.value()), Ok(code));
                }
            }
            let s = at.ponded_sediment().value();
            if code == 0 {
                assert_same_bits(s, 0.0);
            } else {
                let expected = 25.0 * math::exp2(f64::from(code - 1) / 2.0);
                assert!((s / expected - 1.0).abs() < 1e-15, "{code}");
                assert_eq!(ponded(s), Ok(code));
            }
        }
        assert_same_bits(
            cell(0xF0).hill_relief().unwrap().value(),
            20.0 * math::exp2(14.0 / 3.0),
        );
        assert_same_bits(cell(0x0F).ponded_sediment().value(), 3_200.0);
        // Within half a step in log of the nearest code: 2^(1 ÷ 6) and 2^(1 ÷ 4).
        for m in [27.5, 55.0, 116.0, 232.0, 286.0, 400.0] {
            let back = cell(SynthesisCell::pack_seafloor(hill(m).unwrap(), 0).unwrap())
                .hill_relief()
                .unwrap()
                .value();
            assert!(math::ln(back / m).abs() <= core::f64::consts::LN_2 / 6.0 + 1e-12);
        }
        for m in [30.0, 100.0, 404.0, 2_400.0] {
            let back = cell(SynthesisCell::pack_seafloor(0, ponded(m).unwrap()).unwrap())
                .ponded_sediment()
                .value();
            assert!(math::ln(back / m).abs() <= core::f64::consts::LN_2 / 4.0 + 1e-12);
        }
        // The hill scale saturates at both ends for any relief; zero is none.
        assert_eq!(hill(0.0), Ok(0));
        assert_eq!(hill(-0.0), Ok(0));
        assert_eq!(hill(1e-300), Ok(1));
        assert_eq!(hill(5.0), Ok(1));
        assert_eq!(hill(1e6), Ok(15));
        // The sediment scale's code 0 is everything under 25 m × 2^(−1 ÷ 4) = 21.02 m.
        assert_eq!(ponded(0.0), Ok(0));
        assert_eq!(ponded(21.0), Ok(0));
        assert_eq!(ponded(21.1), Ok(1));
        assert_eq!(ponded(1e9), Ok(15));
        // Nothing negative or not finite has a code, and a nibble holds no more than 15.
        for quantise in [hill, ponded] {
            assert_eq!(quantise(-1.0), Err(QuantiseValueError::OutOfRange(-1.0)));
            assert!(refuses_nan(&quantise(f64::NAN)));
            assert_eq!(
                quantise(f64::INFINITY),
                Err(QuantiseValueError::NotFinite(f64::INFINITY))
            );
        }
        assert_eq!(SynthesisCell::pack_seafloor(16, 0), None);
        assert_eq!(SynthesisCell::pack_seafloor(0, 16), None);
        assert_eq!(SynthesisCell::pack_seafloor(15, 15), Some(0xFF));
        assert_eq!(cell(SynthesisCell::NO_SEAFLOOR).hill_relief(), None);
        assert_eq!(
            cell(SynthesisCell::NO_SEAFLOOR).ponded_sediment(),
            Metres::ZERO
        );
    }

    /// A dry record of a stagnant lid's crust with no boundary, for tests that vary one field.
    const fn field_test_cell() -> SynthesisCell {
        SynthesisCell {
            elevation_mm: 0,
            boundary_distance_km: SynthesisCell::NO_BOUNDARY_KM,
            plate: 0,
            crust: Crust::Lid,
            boundary: BoundaryKind::Absent,
            boundary_obliquity: 0,
            flow: FlowDirection::Terminal,
            drainage: LogArea::ZERO,
            steepness: LogSteepness::ZERO,
            water_surface_mm: 0,
            ice: 0,
            substances: SynthesisCell::NO_SUBSTANCES,
            class: SurfaceClass::UNCLASSIFIED,
            crater_state: 0,
            seafloor: SynthesisCell::NO_SEAFLOOR,
        }
    }

    /// The substance byte packs an ice entry in the high nibble and a liquid entry in the low, 0xF
    /// for none, and refuses an index a nibble cannot name.
    #[test]
    fn field_cell_substances_pack_two_nibbles() {
        assert_eq!(
            SynthesisCell::pack_substances(None, None),
            Some(SynthesisCell::NO_SUBSTANCES)
        );
        assert_eq!(SynthesisCell::pack_substances(Some(2), Some(3)), Some(0x23));
        assert_eq!(SynthesisCell::pack_substances(Some(14), None), Some(0xEF));
        assert_eq!(SynthesisCell::pack_substances(None, Some(0)), Some(0xF0));
        assert_eq!(SynthesisCell::pack_substances(Some(15), None), None);
        assert_eq!(SynthesisCell::pack_substances(None, Some(200)), None);
        for byte in 0..=u8::MAX {
            let cell = SynthesisCell {
                elevation_mm: 0,
                boundary_distance_km: SynthesisCell::NO_BOUNDARY_KM,
                plate: 0,
                crust: Crust::Lid,
                boundary: BoundaryKind::Absent,
                boundary_obliquity: 0,
                flow: FlowDirection::Terminal,
                drainage: LogArea::ZERO,
                steepness: LogSteepness::ZERO,
                water_surface_mm: 0,
                ice: 0,
                substances: byte,
                class: SurfaceClass::UNCLASSIFIED,
                crater_state: 0,
                seafloor: SynthesisCell::NO_SEAFLOOR,
            };
            assert_eq!(
                SynthesisCell::pack_substances(cell.ice_entry(), cell.liquid_entry()),
                Some(byte)
            );
        }
    }

    /// The class codes split at 1 and 64: unclassified, Köppen–Geiger, surface state.
    #[test]
    fn field_surface_class_codes_split_in_three_ranges() {
        let range = |code| SurfaceClass::new(code).range();
        assert_eq!(
            SurfaceClass::UNCLASSIFIED.range(),
            SurfaceClassRange::Unclassified
        );
        assert_eq!(range(1), SurfaceClassRange::KoppenGeiger);
        assert_eq!(range(63), SurfaceClassRange::KoppenGeiger);
        assert_eq!(range(64), SurfaceClassRange::SurfaceState);
        assert_eq!(range(255), SurfaceClassRange::SurfaceState);
    }
}
