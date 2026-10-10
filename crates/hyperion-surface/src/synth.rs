//! The local synthesis: a body's height between its coarse cells, band-limited level by level
//! (plan R09, Design notes 13 and 14).
//!
//! [`Synthesiser`] is the height function over a coarse field, on the server's whole field and a
//! client's part of one alike, through [`FieldView`]. Today it returns the base elevation, the
//! coarse cells interpolated over the sphere ([`interp`], R09.T4), at every level; the structural
//! octaves ([`relief`], R09.T5), the channels ([`channels`], whose network R09.T6.a builds and
//! whose incision T6.b adds) and the small craters (T7.b) add the bands below the coarse cell, and
//! the assembly (T8) fixes each level's set. The module also holds the two types a coarse field's
//! header carries: [`BandLevel`], the quadtree level a height is asked at, and [`BandSpectrum`],
//! the law of the structural relief finer than the coarse cells, from which the relief derives
//! each level's amplitude and the closed-form variance below a cell ([`local_variance`]), with the
//! crust classes' shapes and anchors that make a body's law ([`SpectrumShape`],
//! [`BandSpectrum::anchored`]); and the per-contribution variance below a wavelength that R10
//! reads ([`unresolved_variance`]).

pub mod channels;
pub mod interp;
pub mod relief;

use hyperion_base::math;
use hyperion_base::rng::DetailSeed;
use hyperion_base::units::{Metres, SquareMetres};

use crate::cube::{MAX_LEVEL, PatchKey};
use crate::field::{FieldView, SynthesisCell};
use crate::height::HeightSample;
use crate::noise::LatticeCache;
use crate::num;
pub use channels::Channels;
use interp::ReadCellError;
pub use relief::{Relief, local_variance};

/// The height function over a coarse field: what the server's collision and the client's patches
/// read (Design note 13).
///
/// It is a pure function of the field the view holds, the body's detail seed, the direction and
/// the level: the same on the server and in the client's WebAssembly workers, bit for bit, and in
/// any order of queries, any cache the caller passes being the caller's. It borrows the view, so
/// it is `Copy` whatever the view is.
#[derive(Debug)]
pub struct Synthesiser<'a, F> {
    field: &'a F,
    seed: DetailSeed,
}

// By hand, since a derive would bound them on `F: Clone` and `F: Copy`, which no view is.
impl<F> Clone for Synthesiser<'_, F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for Synthesiser<'_, F> {}

impl<'a, F: FieldView> Synthesiser<'a, F> {
    /// The synthesis over `field` with the body's detail seed `seed` (plan 14's
    /// `body.surface.detail`, R09.T1.b), from which the bands below the coarse cell draw.
    #[must_use]
    pub const fn new(field: &'a F, seed: DetailSeed) -> Self {
        Self { field, seed }
    }

    /// The field it reads.
    #[must_use]
    pub const fn field(&self) -> &'a F {
        self.field
    }

    /// The body's detail seed.
    #[must_use]
    pub const fn seed(&self) -> DetailSeed {
        self.seed
    }

    /// The height at the unit direction `dir` (body-fixed) at `level`, above the field's spheroid
    /// along its normal, metres, and its gradient in body-fixed space at the spheroid point, metres
    /// per metre.
    ///
    /// Today it is the base elevation ([`interp::base_elevation`]), the same at every level; the
    /// bands below the coarse cell join it in R09.T5–T8. `cache` is the caller's, and no result
    /// depends on what it holds.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the first cell read that the view does not hold.
    ///
    /// # Panics
    ///
    /// If `dir` is zero or has a component that is not finite.
    pub fn height_at(
        &self,
        cache: &mut SynthCache,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<HeightSample, QueryHeightError> {
        // The base elevation reads no cache and is the same at every level; the octaves (T5) and
        // the channels (T6.b) read and fill `cache` per level.
        let _ = (cache, level);
        Ok(interp::base_elevation(self.field, dir)?)
    }

    /// The cell that contains the unit direction `dir`, and its record, from which the categorical
    /// fields (plate, crust, boundary kind, flow direction, surface class) are read
    /// ([`interp::cell_at`]).
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] if the view does not hold the cell.
    ///
    /// # Panics
    ///
    /// If `dir` is zero or has a component that is not finite.
    pub fn cell_at(
        &self,
        dir: [f64; 3],
    ) -> Result<(PatchKey, &'a SynthesisCell), QueryHeightError> {
        Ok(interp::cell_at(self.field, dir)?)
    }
}

/// The caller's cache for a [`Synthesiser`]'s queries, one per worker or per bake.
///
/// It holds R05's lattice cache, generalised to the body's detail seed (R09.T5), for the
/// structural octaves ([`relief`]): their octave table, built once per seed, and the lattice
/// corners a bake covers. T6.b adds the channel network's cells, keyed by `u64`. No height depends
/// on what it holds, only the time a query takes.
#[derive(Debug, Clone, Default)]
pub struct SynthCache {
    lattice: LatticeCache,
}

impl SynthCache {
    /// An empty cache.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lattice: LatticeCache::new(),
        }
    }

    /// The structural octaves' lattice cache.
    #[must_use]
    pub(crate) fn lattice_mut(&mut self) -> &mut LatticeCache {
        &mut self.lattice
    }
}

/// The variance of a cell's ground in wavelengths shorter than a given one, per contribution:
/// what R10's material weights and slope readouts read (Design note 18).
///
/// Each is a closed form of the cell's own fields. The structural octaves' is built (R09.T5); the
/// channels (T6.b) and the small craters (T7.b) add theirs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UnresolvedVariance {
    relief: SquareMetres,
}

impl UnresolvedVariance {
    /// The structural octaves' share, square metres: their variance in the band asked for, the
    /// cell's amplitude squared times the spectrum's ([`relief`]).
    #[must_use]
    pub const fn relief(&self) -> SquareMetres {
        self.relief
    }

    /// The whole, square metres: the contributions are independent, so their variances add.
    #[must_use]
    pub const fn total(&self) -> SquareMetres {
        self.relief
    }
}

/// The variance of `cell`'s ground in wavelengths shorter than `finer_than`, per contribution, or
/// `None` where `field` does not hold the cell (Design note 18).
///
/// # Panics
///
/// If `finer_than` is not finite and positive, or `cell` is not of the field's level.
#[must_use]
pub fn unresolved_variance(
    field: &impl FieldView,
    cell: PatchKey,
    finer_than: Metres,
) -> Option<UnresolvedVariance> {
    Some(UnresolvedVariance {
        relief: relief::unresolved_relief_variance(field, cell, finer_than)?,
    })
}

/// Why a [`Synthesiser`] could not answer a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryHeightError {
    /// The query reads a cell the view does not hold: neither surveyed nor in a survey's margin
    /// ([`crate::field::SYNTHESIS_MARGIN_CELLS`]), so the height there is not known to this side.
    NotSurveyed(PatchKey),
}

impl std::fmt::Display for QueryHeightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSurveyed(c) => write!(
                f,
                "cell ({}, {}) of face {} at level {} is not surveyed or in a survey's margin",
                c.i(),
                c.j(),
                c.face().index(),
                c.level()
            ),
        }
    }
}

impl std::error::Error for QueryHeightError {}

impl From<ReadCellError> for QueryHeightError {
    fn from(e: ReadCellError) -> Self {
        Self::NotSurveyed(e.cell)
    }
}

/// A quadtree level at which the synthesis is evaluated, 0 to [`MAX_LEVEL`]: a patch's level, whose
/// band is the relief its vertices resolve (Design note 13). R10 makes one from a patch's `u8`
/// level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BandLevel(u8);

/// Why a level is not a [`BandLevel`]: it is deeper than [`MAX_LEVEL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NewBandLevelError(pub u8);

impl std::fmt::Display for NewBandLevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "band level {} is above the maximum {MAX_LEVEL}", self.0)
    }
}

impl std::error::Error for NewBandLevelError {}

impl BandLevel {
    /// The band of quadtree level `level`.
    ///
    /// # Errors
    ///
    /// [`NewBandLevelError`] above [`MAX_LEVEL`].
    pub const fn new(level: u8) -> Result<Self, NewBandLevelError> {
        if level > MAX_LEVEL {
            Err(NewBandLevelError(level))
        } else {
            Ok(Self(level))
        }
    }

    /// The quadtree level.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for BandLevel {
    type Error = NewBandLevelError;

    fn try_from(level: u8) -> Result<Self, Self::Error> {
        Self::new(level)
    }
}

/// The spectrum of a body's structural relief finer than its coarse cells: a degree variance that
/// breaks once, V(l) = V₁ l^−β₁ below the break degree `l_b` and V₁ `l_b`^(β₂−β₁) l^−β₂ from it,
/// continuous at the break (Design note 7; `decision-r09-t5.md` items 2–4).
///
/// V(l) is the variance of height contributed by the 2l + 1 spherical harmonics of degree l, so
/// that the height variance about the mean is the sum of V(l) over every degree from 1, and the
/// synthesis's expected variance below a coarse cell is the sum from the cell's Nyquist degree up,
/// in closed form ([`local_variance`]). It is the law of the structural share alone, the relief
/// that is neither cratered nor incised (tectonic and volcanic relief, and the hillslopes between
/// channels): the craters (R09.T7) and the channels (R09.T6) add their own variance on top, and
/// the coarse pass books each share in `σ_h`'s budget rather than in this law (item 4). It is the
/// law of the reference style, amplitude 1 ([`relief::ReliefStyle`]): the styles are absolute
/// ratios to it, and nothing divides it by their mean square (item 1).
///
/// A body's law is [`BandSpectrum::anchored`] of its crust's [`SpectrumShape`]: one break, from
/// β₁ at the coarse cells' degrees to a steeper β₂ below a wavelength of a kilometre or two, which
/// every dataset the ruling measured shows. The single law V₁ l^−1.9 that R09.T2 began with gave
/// slopes of 56° along a profile at 1 m on the Earth-like world (R09.T5's science check). The
/// relief ([`relief`]) derives each level's amplitude from the law
/// ([`BandSpectrum::level_variance`]), so the header carries the law alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandSpectrum {
    exponent: f64,
    unit_degree_variance: SquareMetres,
    break_degree: f64,
    small_scale_exponent: f64,
}

/// The parts of a [`BandSpectrum`], which [`BandSpectrum::new`] checks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandSpectrumParts {
    /// The exponent β₁ of V(l) ∝ l^−β₁ below the break degree: finite and above 1.
    pub exponent: f64,
    /// The degree-1 variance V₁, square metres, the law's scale: finite and non-negative.
    pub unit_degree_variance: SquareMetres,
    /// The break degree `l_b`, from which the exponent is β₂: finite and at least 1, and need not
    /// be an integer.
    pub break_degree: f64,
    /// The exponent β₂ of V(l) ∝ l^−β₂ from the break degree up: finite and above 1.
    pub small_scale_exponent: f64,
}

/// Why [`BandSpectrum::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildBandSpectrumError {
    /// The exponent β₁ is not finite and above 1, so the variance would not sum.
    Exponent(f64),
    /// The degree-1 variance V₁ is not finite and non-negative.
    Variance(f64),
    /// The break degree is not finite and at least 1.
    BreakDegree(f64),
    /// The small-scale exponent β₂ is not finite and above 1, so the variance would not sum.
    SmallScaleExponent(f64),
    /// The law's whole variance, Σ V(l) from degree 1, is not finite: parts each in range whose
    /// closed forms overflow (a break degree far beyond every field's degrees with a much steeper
    /// β₂, or a V₁ near the largest `f64`), so that every variance the relief reads stays finite.
    Total(f64),
}

impl std::fmt::Display for BuildBandSpectrumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exponent(b) => write!(f, "spectral exponent {b} is not finite and above 1"),
            Self::Variance(v) => {
                write!(f, "degree variance {v} m² is not finite and non-negative")
            }
            Self::BreakDegree(l) => write!(f, "break degree {l} is not finite and at least 1"),
            Self::SmallScaleExponent(b) => write!(
                f,
                "small-scale spectral exponent {b} is not finite and above 1"
            ),
            Self::Total(v) => write!(f, "the spectrum's whole variance {v} m² is not finite"),
        }
    }
}

impl std::error::Error for BuildBandSpectrumError {}

/// Why [`SpectrumShape::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildSpectrumShapeError {
    /// The exponent β₁ is not finite and above 1.
    Exponent(f64),
    /// The break wavelength is not finite and positive, metres.
    BreakWavelength(f64),
    /// The small-scale exponent β₂ is not finite and above 1.
    SmallScaleExponent(f64),
}

impl std::fmt::Display for BuildSpectrumShapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exponent(b) => write!(f, "spectral exponent {b} is not finite and above 1"),
            Self::BreakWavelength(w) => {
                write!(f, "break wavelength {w} m is not finite and positive")
            }
            Self::SmallScaleExponent(b) => write!(
                f,
                "small-scale spectral exponent {b} is not finite and above 1"
            ),
        }
    }
}

impl std::error::Error for BuildSpectrumShapeError {}

/// The shape of a crust class's structural spectrum: β₁ from the coarse cell to the break
/// wavelength `λ_b`, and β₂ below it (`decision-r09-t5.md` item 2), which
/// [`BandSpectrum::anchored`] scales to a body.
///
/// The break is a wavelength, not a degree, so that equal crusts on bodies of different radius
/// break at the same scale. It has no gravity term: breaks between about 1 and 3 km are found on
/// bodies whose gravity spans a factor of 7 (Earth, the Moon's breakover, Europa), and Landais,
/// Schmidt and Lovejoy (2019, MNRAS 484, 787, doi:10.1093/mnras/sty3253; their 2018 preprint,
/// arXiv:1805.11249) find a common transition near 10 km on Earth, Mars, the Moon and Mercury,
/// which they attribute to the elastic thickness; the ruling infers from it that the break carries
/// no gravity term. Age acts through the craters, not through the law. The two constants are
/// the set the coarse pass chooses from (R09.T12.e), a closed one for the composition audit (the
/// plan's Risks); [`new`](Self::new) makes another crust class's shape from its measurements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectrumShape {
    exponent: f64,
    break_wavelength: Metres,
    small_scale_exponent: f64,
}

impl SpectrumShape {
    /// The shape of exponent β₁ `exponent` above the break wavelength `break_wavelength` and
    /// `small_scale_exponent` β₂ below it.
    ///
    /// # Errors
    ///
    /// [`BuildSpectrumShapeError::Exponent`] unless β₁ is finite and above 1,
    /// [`BuildSpectrumShapeError::BreakWavelength`] unless the break wavelength is finite and
    /// positive, and [`BuildSpectrumShapeError::SmallScaleExponent`] unless β₂ is finite and
    /// above 1, checked in that order.
    pub fn new(
        exponent: f64,
        break_wavelength: Metres,
        small_scale_exponent: f64,
    ) -> Result<Self, BuildSpectrumShapeError> {
        if !(exponent.is_finite() && exponent > 1.0) {
            return Err(BuildSpectrumShapeError::Exponent(exponent));
        }
        let w = break_wavelength.value();
        if !(w.is_finite() && w > 0.0) {
            return Err(BuildSpectrumShapeError::BreakWavelength(w));
        }
        if !(small_scale_exponent.is_finite() && small_scale_exponent > 1.0) {
            return Err(BuildSpectrumShapeError::SmallScaleExponent(
                small_scale_exponent,
            ));
        }
        Ok(Self {
            exponent,
            break_wavelength,
            small_scale_exponent,
        })
    }

    /// The exponent β₁ above the break wavelength.
    #[must_use]
    pub const fn exponent(&self) -> f64 {
        self.exponent
    }

    /// The break wavelength `λ_b`.
    #[must_use]
    pub const fn break_wavelength(&self) -> Metres {
        self.break_wavelength
    }

    /// The exponent β₂ below the break wavelength.
    #[must_use]
    pub const fn small_scale_exponent(&self) -> f64 {
        self.small_scale_exponent
    }

    /// A silicate crust's shape, a rock surface in any regime but an ice shell: β₁ 2.0 to a break
    /// at 2 km, then β₂ 3.0 (`decision-r09-t5.md` item 2).
    ///
    /// Above the break, Earth's land measures β 2.04 at 26–2.8 km in 60 random tiles of SRTM's 1″
    /// grid (Farr et al. 2007, Rev. Geophys. 45, RG2004), 2.0 at 10–40 km on Earth2014's land (the
    /// grid of Hirt and Rexer 2015, Int. J. Appl. Earth Obs. Geoinf. 39, 103) and 2.04–2.17 in
    /// Gagnon, Lovejoy and Schertzer 2006 (Nonlin. Processes Geophys. 13, 541, Table 5), and the
    /// sea floor 2.0–2.3 above about 2 km (slope ratios on GMRT, the grid of Ryan et al. 2009, G³
    /// 10, Q03014). The land steepens through 2–3 km, mountains first, to 2.79 at 2.8 km–350 m and
    /// 3.14 at 350–87 m (the SRTM sample; Perron, Kirchner and Dietrich 2008, JGR 113, F04003, ¶36,
    /// their 2D exponents less 1), and the sea floor to 2.4–2.7 at 0.25–1 km (GMRT) and about 2.8
    /// (von Kármán ν ≈ 0.9, the ruling's reading of Goff 2020, GRL 47, e2020GL088162; Goff 2010,
    /// JGR 115, B12104, ¶33, takes a fractal dimension of 2.2 as typical, ν ≈ 0.8). β₂ = 3.0
    /// matches the land's slope statistics from 1 km down to the 2 m band limit. The soil-mantled
    /// rollover below about 200 m on wet worlds (β 3.5–4.2, Perron et al. 2008) is not a second
    /// break, since bedrock and mountains, which carry most of the slope variance, stay near β
    /// 3.1–3.4 there; and the cratered bodies' own breaks near 10 km are their craters', which
    /// R09.T7 adds. These measurements are the ruling's (its scratch spectra). Medium confidence.
    pub const SILICATE: Self = Self {
        exponent: 2.0,
        break_wavelength: Metres::new(2_000.0),
        small_scale_exponent: 3.0,
    };

    /// An ice-rich crust's shape, an ice surface or an ice shell: β₁ 1.8 to a break at 1 km, then
    /// β₂ 2.6 (`decision-r09-t5.md` item 2).
    ///
    /// Europa's limb profiles give β about 2 at 10–60 km and its stereo models 1.4–1.8 at 1–5 km,
    /// with "a break in slope at about 1 km" and a Hurst exponent of 0.7–0.8 below it, β 2.4–2.6
    /// (Nimmo and Schenk 2008, LPSC XXXIX, abstract 1464; Schenk and Nimmo 2017, Europa Deep Dive
    /// I, LPI Contrib. 2048, abstract 7015; Steinbrügge et al. 2020, Icarus 343, 113669).
    /// Medium-low confidence: one body's measurements.
    pub const ICE_RICH: Self = Self {
        exponent: 1.8,
        break_wavelength: Metres::new(1_000.0),
        small_scale_exponent: 2.6,
    };
}

/// The wavelength below which a crust class's structural RMS is anchored, 72 km: the Earth's coarse
/// cell's Nyquist wavelength, 2πR⊕ ÷ 556, held in physical wavelength so that equal crusts on
/// bodies of different radius are equally rough (`decision-r09-t5.md` item 3).
pub const ANCHOR_WAVELENGTH: Metres = Metres::new(72_000.0);

/// A mobile lid's structural RMS in wavelengths shorter than [`ANCHOR_WAVELENGTH`] at the Earth's
/// gravity, 200 m (`decision-r09-t5.md` item 3). On another body it is this times g⊕ ÷ g, the
/// strength-limited factor the coarse pass gives belts and trenches (R09.T12.b, 1 on the reference
/// Earth), so that a belt's coarse and fine relief scale together.
///
/// Earth2014's BED layer carries 184 m above degree 556 (Hirt and Rexer 2015's fitted degree
/// variance, Int. J. Appl. Earth Obs. Geoinf. 39, 103, recomputed by the ruling); its bathymetry,
/// about 90% altimetric at 4–5′, lacks the abyssal hills, which GMRT's multibeam shows carrying
/// 50–230 m RMS at 2–10 km over 71% of the surface, which brings it to about 200–210 m, Design
/// note 7's "about 200 m". Medium confidence.
pub const STRUCTURAL_RMS_MOBILE_LID: Metres = Metres::new(200.0);

/// A rocky stagnant lid's structural RMS in wavelengths shorter than [`ANCHOR_WAVELENGTH`], 60 m,
/// with no gravity term; a heat-pipe or episodic lid's too (`decision-r09-t5.md` item 3).
///
/// Crater-poor plains in 35 km windows measure 45 m (Vastitas Borealis), 55 m (Amazonis) and 43 m
/// (Daedalia) on MOLA's 32 ppd grid (Smith et al. 2001, JGR 106, 23689), and the lunar maria 89 m
/// with their small craters and wrinkle ridges on LOLA's 16 ppd grid (Smith et al. 2010, GRL 37,
/// L18204), times 1.15, the Earth's ratio of that window RMS to the RMS above 72 km (the ruling's
/// measurements). The maria and Mars's plains differ by under 2× at 10 km though their gravities
/// differ by 2.3×, both being set by emplacement and craters rather than by strength. Medium-low
/// confidence.
pub const STRUCTURAL_RMS_STAGNANT_LID: Metres = Metres::new(60.0);

/// An ice-rich crust's structural RMS in wavelengths shorter than [`ANCHOR_WAVELENGTH`], 50 m, with
/// no gravity term (`decision-r09-t5.md` item 3).
///
/// Europa's ridged plains have an RMS deviation of 35–50 m over a 1 km baseline (Nimmo and Schenk
/// 2008, LPSC XXXIX, abstract 1464, Fig. 5a; its Table 1 gives 5.6–8.5 m at 100 m) and a Hurst
/// exponent of about 0.3 out to 70 km (Schenk and Nimmo 2017, LPI Contrib. 2048, abstract 7015,
/// Fig. 1). With [`SpectrumShape::ICE_RICH`] it gives adirectional RMS slopes (√2 times a
/// profile's) of 2.0°, 5.7° and 10.3° over 1 km, 100 m and 10 m, against Europa's 2.8–4.1°,
/// 4.5–6.9° and 8.5–13° (the last extrapolated). Medium-low confidence.
pub const STRUCTURAL_RMS_ICE_RICH: Metres = Metres::new(50.0);

impl BandSpectrum {
    /// The default exponent β₁, per-degree variance ∝ l^−2 above the break: a silicate crust's
    /// ([`SpectrumShape::SILICATE`]).
    pub const DEFAULT_EXPONENT: f64 = 2.0;

    /// The spectrum of `parts`.
    ///
    /// # Errors
    ///
    /// [`BuildBandSpectrumError::Exponent`] unless β₁ is finite and above 1, where the sum over
    /// degrees converges; [`BuildBandSpectrumError::Variance`] unless V₁ is finite and
    /// non-negative; [`BuildBandSpectrumError::BreakDegree`] unless the break degree is finite and
    /// at least 1; [`BuildBandSpectrumError::SmallScaleExponent`] unless β₂ is finite and above
    /// 1; and [`BuildBandSpectrumError::Total`] unless the whole variance from degree 1 is finite;
    /// checked in that order.
    pub fn new(parts: BandSpectrumParts) -> Result<Self, BuildBandSpectrumError> {
        let BandSpectrumParts {
            exponent,
            unit_degree_variance,
            break_degree,
            small_scale_exponent,
        } = parts;
        if !(exponent.is_finite() && exponent > 1.0) {
            return Err(BuildBandSpectrumError::Exponent(exponent));
        }
        let v = unit_degree_variance.value();
        if !(v.is_finite() && v >= 0.0) {
            return Err(BuildBandSpectrumError::Variance(v));
        }
        if !(break_degree.is_finite() && break_degree >= 1.0) {
            return Err(BuildBandSpectrumError::BreakDegree(break_degree));
        }
        if !(small_scale_exponent.is_finite() && small_scale_exponent > 1.0) {
            return Err(BuildBandSpectrumError::SmallScaleExponent(
                small_scale_exponent,
            ));
        }
        let spectrum = Self {
            exponent,
            unit_degree_variance,
            break_degree,
            small_scale_exponent,
        };
        // Every band the relief reads, and the break's factor within it, is bounded by the whole;
        // an overflowing factor makes the whole infinite, or NaN where V₁ is 0.
        let total = spectrum.variance_from_degree(1).value();
        if !total.is_finite() {
            return Err(BuildBandSpectrumError::Total(total));
        }
        Ok(spectrum)
    }

    /// The structural spectrum of a crust of shape `shape` on a body of mean radius `radius`,
    /// whose RMS in wavelengths shorter than [`ANCHOR_WAVELENGTH`] is `rms_shorter_than`
    /// (`decision-r09-t5.md` item 3).
    ///
    /// The break degree is 2πR ÷ `λ_b`, or 1 on a body so small that its whole spectrum lies below
    /// the break wavelength. V₁ is the one for which the variance from degree
    /// `l_72` = max(2, round(2πR ÷ 72 km)) up
    /// ([`variance_from_degree`](Self::variance_from_degree)) is `rms_shorter_than`²: anchored at
    /// the coarse cell, never from degree 1, which the Earth's continent–ocean dichotomy dominates
    /// and no power law describes. The coarse pass (R09.T12.e) and the synthetic worlds call it
    /// alike, with the crust class's anchor:
    /// [`STRUCTURAL_RMS_MOBILE_LID`] times g⊕ ÷ g, [`STRUCTURAL_RMS_STAGNANT_LID`] or
    /// [`STRUCTURAL_RMS_ICE_RICH`]. An anchor of zero gives V₁ = 0, a body with no relief finer
    /// than its cells.
    ///
    /// # Panics
    ///
    /// If `radius` is not finite and positive or is over about 10²⁰ m (its anchor degree past
    /// 2⁵³), or `rms_shorter_than` is not finite and non-negative; or if [`new`](Self::new)
    /// refuses the law on this body, from a shape outside every crust's: a break wavelength so
    /// short against the radius that the break degree is not finite, a unit law whose variance
    /// from `l_72` underflows to 0, or a whole variance that is not finite.
    #[must_use]
    pub fn anchored(shape: SpectrumShape, radius: Metres, rms_shorter_than: Metres) -> Self {
        let r = radius.value();
        assert!(
            r.is_finite() && r > 0.0,
            "a body's radius must be finite and positive, got {r} m"
        );
        let sigma = rms_shorter_than.value();
        assert!(
            sigma.is_finite() && sigma >= 0.0,
            "an anchor RMS must be finite and non-negative, got {sigma} m"
        );
        let circumference = 2.0 * core::f64::consts::PI * r;
        let unit = BandSpectrumParts {
            exponent: shape.exponent,
            unit_degree_variance: SquareMetres::new(1.0),
            break_degree: num::max(circumference / shape.break_wavelength.value(), 1.0),
            small_scale_exponent: shape.small_scale_exponent,
        };
        let refused = |e: BuildBandSpectrumError| -> Self {
            panic!("a spectrum shape must give a valid law on this body: {e}")
        };
        let per_unit = Self::new(unit)
            .unwrap_or_else(refused)
            .variance_from_degree(anchor_degree(circumference))
            .value();
        Self::new(BandSpectrumParts {
            unit_degree_variance: SquareMetres::new(sigma * sigma / per_unit),
            ..unit
        })
        .unwrap_or_else(refused)
    }

    /// The exponent β₁ of V(l) ∝ l^−β₁ below the break degree.
    #[must_use]
    pub const fn exponent(&self) -> f64 {
        self.exponent
    }

    /// The degree-1 variance V₁, square metres: the law's scale.
    #[must_use]
    pub const fn unit_degree_variance(&self) -> SquareMetres {
        self.unit_degree_variance
    }

    /// The break degree `l_b`, at least 1, from which the exponent is β₂.
    #[must_use]
    pub const fn break_degree(&self) -> f64 {
        self.break_degree
    }

    /// The exponent β₂ of V(l) ∝ l^−β₂ from the break degree up.
    #[must_use]
    pub const fn small_scale_exponent(&self) -> f64 {
        self.small_scale_exponent
    }

    /// The degree variance at degree `l`, square metres: V₁ l^−β₁ below the break degree and
    /// V₁ `l_b`^(β₂−β₁) l^−β₂ from it. `l` need not be an integer, so that a structure function
    /// may integrate it (R09.T8, R10).
    ///
    /// # Panics
    ///
    /// If `l` is not finite and at least 1.
    #[must_use]
    pub fn degree_variance(&self, l: f64) -> SquareMetres {
        assert!(
            l.is_finite() && l >= 1.0,
            "a degree must be finite and at least 1, got {l}"
        );
        let v1 = self.unit_degree_variance.value();
        SquareMetres::new(if l < self.break_degree {
            v1 * math::powf(l, -self.exponent)
        } else {
            v1 * self.break_factor() * math::powf(l, -self.small_scale_exponent)
        })
    }

    /// `l_b`^(β₂−β₁), the factor that makes the law continuous at the break.
    #[must_use]
    pub(crate) fn break_factor(&self) -> f64 {
        math::powf(self.break_degree, self.small_scale_exponent - self.exponent)
    }
}

/// The first degree of [`BandSpectrum::anchored`]'s anchor on a body of circumference
/// `circumference` metres: max(2, round(2πR ÷ [`ANCHOR_WAVELENGTH`])).
///
/// # Panics
///
/// If the degree is 2⁵³ or more, a body some 10¹⁰ AU round.
#[must_use]
fn anchor_degree(circumference: f64) -> u64 {
    let degree = (circumference / ANCHOR_WAVELENGTH.value()).round();
    if degree <= 2.0 {
        return 2;
    }
    assert!(
        degree < 9.007_199_254_740_992e15,
        "a body of circumference {circumference} m is beyond every degree"
    );
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive integer below 2^53, checked above"
    )]
    let degree = degree as u64;
    degree
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn a_band_level_is_a_quadtree_level() {
        assert_eq!(BandLevel::new(19).map(BandLevel::get), Ok(19));
        assert_eq!(
            BandLevel::try_from(MAX_LEVEL + 1),
            Err(NewBandLevelError(25))
        );
    }

    /// Parts of a silicate crust's law at an Earth's break, with V₁ `v1` m².
    fn parts(v1: f64) -> BandSpectrumParts {
        BandSpectrumParts {
            exponent: BandSpectrum::DEFAULT_EXPONENT,
            unit_degree_variance: SquareMetres::new(v1),
            break_degree: 20_015.1,
            small_scale_exponent: 3.0,
        }
    }

    #[test]
    fn a_band_spectrum_needs_a_summable_law_and_a_break() {
        let v = SquareMetres::new(2.253e7);
        let spectrum = BandSpectrum::new(parts(v.value())).unwrap();
        assert_eq!(spectrum.unit_degree_variance(), v);
        assert_same_bits(spectrum.break_degree(), 20_015.1);
        assert_same_bits(spectrum.small_scale_exponent(), 3.0);
        let refused = |p: BandSpectrumParts| BandSpectrum::new(p).unwrap_err();
        assert_eq!(
            refused(BandSpectrumParts {
                exponent: 1.0,
                ..parts(1.0)
            }),
            BuildBandSpectrumError::Exponent(1.0)
        );
        assert_eq!(refused(parts(-1.0)), BuildBandSpectrumError::Variance(-1.0));
        for l in [0.5, 0.0, -3.0, f64::INFINITY] {
            assert_eq!(
                refused(BandSpectrumParts {
                    break_degree: l,
                    ..parts(1.0)
                }),
                BuildBandSpectrumError::BreakDegree(l)
            );
        }
        assert!(matches!(
            refused(BandSpectrumParts {
                break_degree: f64::NAN,
                ..parts(1.0)
            }),
            BuildBandSpectrumError::BreakDegree(l) if l.is_nan()
        ));
        for b in [1.0, 0.7, f64::INFINITY] {
            assert_eq!(
                refused(BandSpectrumParts {
                    small_scale_exponent: b,
                    ..parts(1.0)
                }),
                BuildBandSpectrumError::SmallScaleExponent(b)
            );
        }
        // A break at degree 1 is a law of β₂ alone, and is accepted.
        assert!(
            BandSpectrum::new(BandSpectrumParts {
                break_degree: 1.0,
                ..parts(1.0)
            })
            .is_ok()
        );
        // Parts each in range whose closed forms overflow are refused, so that no variance the
        // relief reads is infinite or NaN: a break far beyond every degree under a far steeper
        // β₂ (its factor l_b^(β₂−β₁) overflows), with V₁ above zero and at zero, and a V₁ near
        // the largest f64.
        let overflowing = BandSpectrumParts {
            break_degree: 1e200,
            small_scale_exponent: 4.0,
            ..parts(1.0)
        };
        assert!(matches!(
            refused(overflowing),
            BuildBandSpectrumError::Total(v) if !v.is_finite()
        ));
        assert!(matches!(
            refused(BandSpectrumParts {
                unit_degree_variance: SquareMetres::ZERO,
                ..overflowing
            }),
            BuildBandSpectrumError::Total(v) if v.is_nan()
        ));
        assert!(matches!(
            refused(parts(1.5e308)),
            BuildBandSpectrumError::Total(v) if v.is_infinite() && v > 0.0
        ));
    }

    #[test]
    fn a_spectrum_shape_needs_summable_exponents_and_a_break_wavelength() {
        let shape = SpectrumShape::new(1.8, Metres::new(1_000.0), 2.6).unwrap();
        assert_eq!(shape, SpectrumShape::ICE_RICH);
        assert_same_bits(SpectrumShape::SILICATE.exponent(), 2.0);
        assert_eq!(
            SpectrumShape::SILICATE.break_wavelength(),
            Metres::new(2_000.0)
        );
        assert_same_bits(SpectrumShape::SILICATE.small_scale_exponent(), 3.0);
        let new = |b: f64, w: f64, s: f64| SpectrumShape::new(b, Metres::new(w), s);
        assert_eq!(
            new(1.0, 1e3, 2.6),
            Err(BuildSpectrumShapeError::Exponent(1.0))
        );
        for w in [0.0, -1.0, f64::INFINITY] {
            assert_eq!(
                new(2.0, w, 3.0),
                Err(BuildSpectrumShapeError::BreakWavelength(w))
            );
        }
        assert_eq!(
            new(2.0, 2e3, 0.5),
            Err(BuildSpectrumShapeError::SmallScaleExponent(0.5))
        );
    }
}
