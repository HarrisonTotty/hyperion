//! The test planet's octave table (plan R05, Design note 12): lattice spacings, target RMS
//! heights, offsets and rotations.
//!
//! Octave k has lattice spacing `λ_k` = 10,000 km ÷ 2^k and a target RMS height `σ_k` that follows
//! Earth's topographic spectrum:
//!
//! - for k ≤ 12, degree variances near ℓ⁻² (Balmino 1993, "The spectra of the topography of the
//!   Earth, Venus and Mars", Geophysical Research Letters 20, 1063; Rexer and Hirt 2015,
//!   "Ultra-high-degree surface spherical harmonic analysis using the Gauss–Legendre and the
//!   Driscoll/Healy quadrature theorem and application to planetary topography models of Earth,
//!   Mars and Moon", Surveys in Geophysics 36, 803), which is Hurst exponent ½ and a per-octave
//!   gain of 2^−½: `σ_k` = 1,732 m × 2^(−k ÷ 2);
//! - above the ridge–valley scale, where landscape spectra steepen (Perron, Kirchner and Dietrich
//!   2008, "Spectral signatures of characteristic spatial scales and nonfractal structure in
//!   landscapes", Journal of Geophysical Research 113, F04003, doi:10.1029/2007JF000866), a gain of ½: `σ_k` = 27.1 m ×
//!   2^−(k − 12) for k from 13 to [`FINEST_OCTAVE`], 21, whose λ is 4.77 m, above the 2 m band
//!   limit.
//!
//! 1,732 m is chosen so that the total, √`Σσ_k²`, is `σ_h` ≈ 2.45 km, Earth's hypsometric standard
//! deviation, and 27.1 m continues the first law at k = 12 (27.06 m).
//!
//! Each octave has an offset drawn from the planet's seed and a fixed rotation with rational
//! entries, the quaternion rotation of an integer quaternion (a, b, c, d) divided by
//! a² + b² + c² + d², so that octaves share no lattice points and no transcendental is needed.

use hyperion_base::Seed;
use hyperion_base::math;
use hyperion_base::rng::{ObjectKey, Stream};

use crate::noise::Octave;
use crate::tags::TEST_PLANET;

/// The finest octave's index: λ = 10,000 km ÷ 2²¹ = 4.77 m.
pub const FINEST_OCTAVE: u8 = 21;

/// The number of octaves, 0 to [`FINEST_OCTAVE`].
pub const OCTAVES: usize = 22;

/// The coarsest octave's lattice spacing, metres (λ₀ = 10,000 km).
const COARSEST_SPACING_M: f64 = 1.0e7;

/// The RMS height of octave 0 under the ℓ⁻² law, metres.
const SIGMA_0_M: f64 = 1_732.0;

/// The last octave of the ℓ⁻² law; finer octaves halve their RMS per octave.
const ROLL_OVER_OCTAVE: u8 = 12;

/// The RMS height of the roll-over octave as the steeper law starts from it, metres.
const SIGMA_ROLL_OVER_M: f64 = 27.1;

/// The first word of the offsets' draws, far above every lattice corner's draw number (which are
/// below 2³⁷; see [`crate::noise`]).
const OFFSET_DRAW_BASE: u64 = 1 << 40;

/// The lattice spacing of octave `k`, metres: 10,000 km ÷ 2^k, exact.
///
/// # Panics
///
/// If `k` is above [`FINEST_OCTAVE`].
#[must_use]
pub fn spacing_m(k: u8) -> f64 {
    assert!(
        k <= FINEST_OCTAVE,
        "octave {k} is beyond the finest, {FINEST_OCTAVE}"
    );
    COARSEST_SPACING_M / f64::from(1_u32 << k)
}

/// The nominal RMS height of octave `k`, metres, before the coarse octaves' rescale.
///
/// # Panics
///
/// If `k` is above [`FINEST_OCTAVE`].
#[must_use]
pub fn nominal_sigma_m(k: u8) -> f64 {
    assert!(
        k <= FINEST_OCTAVE,
        "octave {k} is beyond the finest, {FINEST_OCTAVE}"
    );
    if k <= ROLL_OVER_OCTAVE {
        SIGMA_0_M * math::exp2(-f64::from(k) / 2.0)
    } else {
        SIGMA_ROLL_OVER_M / f64::from(1_u32 << (k - ROLL_OVER_OCTAVE))
    }
}

/// The rotation of octave `k`: that of the integer quaternion
/// (k + 2, 1 + k mod 3, 2 + k mod 5, 1 + k mod 7), each entry an integer over the quaternion's
/// squared norm.
///
/// # Panics
///
/// If `k` is above [`FINEST_OCTAVE`].
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "a quaternion (a, b, c, d) and its squared norm n, as the rotation formula names them"
)]
pub fn rotation(k: u8) -> [[f64; 3]; 3] {
    assert!(
        k <= FINEST_OCTAVE,
        "octave {k} is beyond the finest, {FINEST_OCTAVE}"
    );
    let index = i64::from(k);
    let (a, b, c, d) = (index + 2, 1 + index % 3, 2 + index % 5, 1 + index % 7);
    let n = a * a + b * b + c * c + d * d;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the quaternion's entries are below 30, so every product is exact in f64"
    )]
    let over = |m: i64| m as f64 / n as f64;
    [
        [
            over(a * a + b * b - c * c - d * d),
            over(2 * (b * c - a * d)),
            over(2 * (b * d + a * c)),
        ],
        [
            over(2 * (b * c + a * d)),
            over(a * a - b * b + c * c - d * d),
            over(2 * (c * d - a * b)),
        ],
        [
            over(2 * (b * d - a * c)),
            over(2 * (c * d + a * b)),
            over(a * a - b * b - c * c + d * d),
        ],
    ]
}

/// The lattice offset of octave `k` of the planet of `seed`, each axis uniform in [0, 1): the top
/// 53 bits of words 2⁴⁰ + 4k + axis of the test planet's stream for object 0.
///
/// # Panics
///
/// If `k` is above [`FINEST_OCTAVE`].
#[must_use]
pub fn offset(seed: Seed, k: u8) -> [f64; 3] {
    assert!(
        k <= FINEST_OCTAVE,
        "octave {k} is beyond the finest, {FINEST_OCTAVE}"
    );
    let stream = Stream::open(seed, TEST_PLANET, ObjectKey::galaxy_item(0));
    [0_u64, 1, 2].map(|axis| {
        let word = stream.word_at(OFFSET_DRAW_BASE + 4 * u64::from(k) + axis);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit integer is exact in f64"
        )]
        let top = (word >> 11) as f64;
        top / 9_007_199_254_740_992.0
    })
}

/// Octave `k` of the planet of `seed`.
///
/// # Panics
///
/// If `k` is above [`FINEST_OCTAVE`].
#[must_use]
pub fn octave(seed: Seed, k: u8) -> Octave {
    assert!(
        k <= FINEST_OCTAVE,
        "octave {k} is beyond the finest, {FINEST_OCTAVE}"
    );
    Octave::new(k, spacing_m(k), rotation(k), offset(seed, k), seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn the_finest_octave_is_above_the_band_limit() {
        assert!((spacing_m(FINEST_OCTAVE) - 4.768).abs() < 1e-3);
        assert!(spacing_m(FINEST_OCTAVE) > crate::geometry::BAND_LIMIT_M);
        assert_eq!(usize::from(FINEST_OCTAVE) + 1, OCTAVES);
    }

    #[test]
    fn the_spectrum_totals_earths_hypsometric_deviation() {
        let total: f64 = (0..=FINEST_OCTAVE)
            .map(|k| {
                let s = nominal_sigma_m(k);
                s * s
            })
            .sum();
        assert!((total.sqrt() - 2_449.0).abs() < 2.0, "{}", total.sqrt());
        // Below 35 km, 0.19% of the variance and about 106 m RMS (Design note 12).
        let fine: f64 = (0..=FINEST_OCTAVE)
            .filter(|&k| spacing_m(k) < 35_000.0)
            .map(|k| {
                let s = nominal_sigma_m(k);
                s * s
            })
            .sum();
        assert!((fine / total - 0.0019).abs() < 0.0001, "{}", fine / total);
        assert!((fine.sqrt() - 106.0).abs() < 1.0, "{}", fine.sqrt());
        // The two laws meet at the roll-over.
        assert!((nominal_sigma_m(12) - 27.06).abs() < 0.01);
    }

    #[test]
    fn rotations_are_orthonormal_and_distinct() {
        for k in 0..=FINEST_OCTAVE {
            let r = rotation(k);
            for a in 0..3 {
                for b in 0..3 {
                    let dot = r[a][0] * r[b][0] + r[a][1] * r[b][1] + r[a][2] * r[b][2];
                    let expected = if a == b { 1.0 } else { 0.0 };
                    assert!((dot - expected).abs() < 1e-15, "octave {k}");
                }
            }
            // A proper rotation: determinant +1.
            let det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
                - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
                + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
            assert!((det - 1.0).abs() < 1e-15);
        }
        assert_ne!(rotation(0), rotation(1));
    }

    #[test]
    fn offsets_are_in_the_unit_cell_and_seeded() {
        let seed = Seed::new(5);
        for k in 0..=FINEST_OCTAVE {
            assert!(offset(seed, k).iter().all(|o| (0.0..1.0).contains(o)));
        }
        let differs = |a: [f64; 3], b: [f64; 3]| a.iter().zip(&b).any(|(x, y)| (x - y).abs() > 0.0);
        assert!(differs(offset(seed, 3), offset(seed, 4)));
        assert!(differs(offset(seed, 3), offset(Seed::new(6), 3)));
    }
}
