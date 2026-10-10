//! The local synthesis: a body's height between its coarse cells, band-limited level by level
//! (plan R09, Design notes 13 and 14).
//!
//! [`Synthesiser`] is the height function over a coarse field, on the server's whole field and a
//! client's part of one alike, through [`FieldView`]. Today it returns the base elevation, the
//! coarse cells interpolated over the sphere ([`interp`], R09.T4), at every level; the structural
//! octaves (T5), the channels (T6) and the small craters (T7.b) add the bands below the coarse
//! cell, and the assembly (T8) fixes each level's set. The module also holds the two types a coarse
//! field's header carries: [`BandLevel`], the quadtree level a height is asked at, and
//! [`BandSpectrum`], the law of the relief finer than the coarse cells, from which R09.T5 derives
//! each level's structural amplitudes and the closed-form variance below a cell.

pub mod interp;

use hyperion_base::rng::DetailSeed;
use hyperion_base::units::SquareMetres;

use crate::cube::{MAX_LEVEL, PatchKey};
use crate::field::{FieldView, SynthesisCell};
use crate::height::HeightSample;
use interp::ReadCellError;

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
/// It holds nothing yet: the base elevation needs no cache. R09.T5 puts R05's lattice cache in it
/// for the structural octaves, and T6.b the channel network's cells, keyed by `u64`.
#[derive(Debug, Clone, Default)]
pub struct SynthCache {
    _reserved: (),
}

impl SynthCache {
    /// An empty cache.
    #[must_use]
    pub const fn new() -> Self {
        Self { _reserved: () }
    }
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

/// The spectrum of a body's relief finer than its coarse cells: a degree variance V(l) = V₁ l^−β
/// (Design note 7).
///
/// V(l) is the variance of height contributed by the 2l + 1 spherical harmonics of degree l, so
/// that the height variance about the mean is the sum of V(l) over every degree from 1, and the
/// synthesis's expected variance below a coarse cell is the sum from the cell's Nyquist degree up,
/// in closed form (`local_variance`, R09.T5). Per-degree variance falls as l^−1.9 by default, within
/// the −1.7 to −2.05 the plan's own research measured (Design note 7, researched 2026-09-29, on
/// the shape models Design note 3 names, which R09.T5 re-checks). R09.T5 derives each level's
/// per-contribution amplitudes from the law, so the header carries the law alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandSpectrum {
    exponent: f64,
    unit_degree_variance: SquareMetres,
}

/// Why [`BandSpectrum::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildBandSpectrumError {
    /// The exponent β is not finite and above 1, so the variance would not sum.
    Exponent(f64),
    /// The degree-1 variance V₁ is not finite and non-negative.
    Variance(f64),
}

impl std::fmt::Display for BuildBandSpectrumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exponent(b) => write!(f, "spectral exponent {b} is not finite and above 1"),
            Self::Variance(v) => {
                write!(f, "degree variance {v} m² is not finite and non-negative")
            }
        }
    }
}

impl std::error::Error for BuildBandSpectrumError {}

impl BandSpectrum {
    /// The default exponent β, per-degree variance ∝ l^−1.9 (Design note 7).
    pub const DEFAULT_EXPONENT: f64 = 1.9;

    /// The spectrum V(l) = `unit_degree_variance` × l^−`exponent`.
    ///
    /// # Errors
    ///
    /// [`BuildBandSpectrumError::Exponent`] unless β is finite and above 1, where the sum over
    /// degrees converges, and [`BuildBandSpectrumError::Variance`] unless V₁ is finite and
    /// non-negative.
    pub fn new(
        exponent: f64,
        unit_degree_variance: SquareMetres,
    ) -> Result<Self, BuildBandSpectrumError> {
        if !(exponent.is_finite() && exponent > 1.0) {
            return Err(BuildBandSpectrumError::Exponent(exponent));
        }
        let v = unit_degree_variance.value();
        if !(v.is_finite() && v >= 0.0) {
            return Err(BuildBandSpectrumError::Variance(v));
        }
        Ok(Self {
            exponent,
            unit_degree_variance,
        })
    }

    /// The exponent β of V(l) ∝ l^−β.
    #[must_use]
    pub const fn exponent(&self) -> f64 {
        self.exponent
    }

    /// The degree-1 variance V₁, square metres: the law's scale.
    #[must_use]
    pub const fn unit_degree_variance(&self) -> SquareMetres {
        self.unit_degree_variance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn a_band_spectrum_needs_a_summable_law() {
        let v = SquareMetres::new(3.6e6);
        let spectrum = BandSpectrum::new(BandSpectrum::DEFAULT_EXPONENT, v).unwrap();
        assert_eq!(spectrum.unit_degree_variance(), v);
        assert_eq!(
            BandSpectrum::new(1.0, v),
            Err(BuildBandSpectrumError::Exponent(1.0))
        );
        assert_eq!(
            BandSpectrum::new(1.9, SquareMetres::new(-1.0)),
            Err(BuildBandSpectrumError::Variance(-1.0))
        );
    }
}
