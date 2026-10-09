//! The local synthesis: a body's height between its coarse cells, band-limited level by level
//! (plan R09, Design notes 13 and 14).
//!
//! Today it holds the two types a coarse field's header carries, to which R09's later tasks give
//! behaviour: [`BandLevel`], the quadtree level a height is asked at, and [`BandSpectrum`], the law
//! of the relief finer than the coarse cells, from which R09.T5 derives each level's structural
//! amplitudes and the closed-form variance below a cell. The interpolant (R09.T4), the octaves
//! (T5), the channels (T6), the small craters (T7.b) and the assembly (T8) follow.

use hyperion_base::units::SquareMetres;

use crate::cube::MAX_LEVEL;

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
