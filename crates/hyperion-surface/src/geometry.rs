//! The terrain's sampling scale: the band limit, the finest vertex spacing and the finest level of
//! a body.
//!
//! The brainstorm's terrain is band-limited at 2 m and sampled at 0.5 m at its finest level, so
//! that the triangle mesh's piecewise-linear interpolant keeps the band limit "to within about a
//! third of its amplitude" (the rendering brainstorm, "The line between truth and decoration").
//! On the quadratic warp a level's vertex spacing runs from 0.65 to 1.17 times its mean, and on a
//! triangle mesh a plane wave's worst pointwise error reaches a third of its amplitude only at a
//! spacing of about 0.19 of its wavelength; so the finest level of a body is the shallowest whose
//! **largest** vertex spacing is at most [`MAX_FINEST_SPACING_M`], 0.375 m, which keeps the
//! "within a third" true in two dimensions and satisfies the 0.5 m as a bound (plan R05, Design
//! note 3). For an Earth that is level 19: spacing 0.18–0.32 m, mean 0.277 m, patches of 17.7 m.
//!
//! These constants and the rule of [`finest_level`] become part of the generator version when
//! R09's real height function reads them; until then they move only [`crate::TEST_PLANET_VERSION`].
//!
//! # Spacing
//!
//! A vertex's spacing is the arc between it and its neighbour along a grid line. On the unit
//! sphere, along a u-line of face 0 at (u, v) the arc grows at
//! dσ/ds = (du/ds) · √(1 + v²) ÷ (1 + u² + v²), with du/ds = (8 ÷ 3) · max(s, 1 − s), and every
//! face and both grid directions are alike by symmetry. A level's spacing is that rate times the
//! step in s, 1 ÷ 2^(level + 6), times the radius: the limit of the edges' arcs, exact to a
//! relative (step)² for the mean and the largest, under 10⁻⁴ even at level 0. The smallest sits
//! on the rate's kink at s = ½, where a finite edge differs to first order in the step: about
//! 1.6% at level 0 and under 3 × 10⁻⁸ at level 19. The rate's extremes have closed forms:
//!
//! - **largest**, on the face's axes (v = 0) at u* = (√31 − 2) ÷ 9, where
//!   d/du [√(1 + 3u) ÷ (1 + u²)] = 0, i.e. 9u² + 4u − 3 = 0: (4 ÷ 3) · √(1 + 3u*) ÷ (1 + u*²)
//!   = 1.704 897;
//! - **smallest**, at an edge's midpoint (u = 0, v = ±1): 2√2 ÷ 3 = 0.942 809;
//! - **mean** over the face, the mean of the u-lines' whole arcs, ∫₀¹ 2 atan(1 ÷ √(1 + v(t)²)) dt
//!   = 1.459 214, pinned and recomputed by a test.
//!
//! Their ratios to the mean, 1.168 and 0.646, are Design note 3's "0.65 to 1.17".

/// The band limit of the terrain's geometry, metres: no relief finer than this is geometry, and
/// what is finer is decoration (the rendering brainstorm's 2 m; plan R05, Design note 5).
pub const BAND_LIMIT_M: f64 = 2.0;

/// The brainstorm's finest sampling, metres: a quarter of [`BAND_LIMIT_M`] (plan R05, Design note
/// 3, where it is read as a mean).
pub const FINEST_SPACING_M: f64 = 0.5;

/// The largest vertex spacing a body's finest level may have, metres: three quarters of
/// [`FINEST_SPACING_M`], 0.1875 of [`BAND_LIMIT_M`], where a plane wave's worst error on the
/// triangle mesh is a third of its amplitude (plan R05, Design note 3).
pub const MAX_FINEST_SPACING_M: f64 = 0.75 * FINEST_SPACING_M;

/// The mean arc rate over a face, per unit of s on the unit sphere (see the module documentation).
const MEAN_RATE: f64 = 1.459_213_746_386_106;

/// The smallest vertex spacing, the mean and the largest at one level of one body, metres.
///
/// Plain data with public fields: [`vertex_spacing`] fills it, and nothing reads it back as a
/// checked value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingRange {
    /// The smallest, at the midpoints of a face's edges, metres.
    pub min_m: f64,
    /// The mean over a face, metres.
    pub mean_m: f64,
    /// The largest, on a face's axes, metres.
    pub max_m: f64,
}

/// The largest arc rate over a face, per unit of s on the unit sphere, (4 ÷ 3) · √(1 + 3u*) ÷
/// (1 + u*²) at u* = (√31 − 2) ÷ 9.
fn max_rate() -> f64 {
    let u = (31.0_f64.sqrt() - 2.0) / 9.0;
    4.0 / 3.0 * (1.0 + 3.0 * u).sqrt() / (1.0 + u * u)
}

/// The smallest arc rate over a face, 2√2 ÷ 3, at an edge's midpoint.
fn min_rate() -> f64 {
    2.0 * 2.0_f64.sqrt() / 3.0
}

/// The step in s of `level`'s vertex lattice, 2^−(level + 6), exact.
pub(crate) fn lattice_step(level: u8) -> f64 {
    let quads = u64::from(crate::cube::PATCH_QUADS) << level;
    #[expect(
        clippy::cast_precision_loss,
        reason = "a power of two up to 2^30 is exact in f64"
    )]
    let quads = quads as f64;
    1.0 / quads
}

/// The vertex spacing of `level` on a body of radius `radius_m`, metres.
///
/// # Panics
///
/// If `level` is above [`crate::cube::MAX_LEVEL`], or `radius_m` is not finite and positive.
#[must_use]
pub fn vertex_spacing(radius_m: f64, level: u8) -> SpacingRange {
    assert!(
        level <= crate::cube::MAX_LEVEL,
        "level {level} is above the maximum"
    );
    assert!(
        radius_m.is_finite() && radius_m > 0.0,
        "a body's radius must be finite and positive, got {radius_m}"
    );
    let scale = radius_m * lattice_step(level);
    SpacingRange {
        min_m: min_rate() * scale,
        mean_m: MEAN_RATE * scale,
        max_m: max_rate() * scale,
    }
}

/// The finest level of a body of radius `radius_m`: the shallowest whose largest vertex spacing is
/// at most [`MAX_FINEST_SPACING_M`], or [`crate::cube::MAX_LEVEL`] if none is (plan R05, Design
/// note 3). Level 19 for an Earth.
///
/// For a spheroid pass the equatorial radius, where the spacing is largest.
///
/// # Panics
///
/// If `radius_m` is not finite and positive.
#[must_use]
pub fn finest_level(radius_m: f64) -> u8 {
    (0..=crate::cube::MAX_LEVEL)
        .find(|&level| vertex_spacing(radius_m, level).max_m <= MAX_FINEST_SPACING_M)
        .unwrap_or(crate::cube::MAX_LEVEL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube::{Face, FaceUv, MAX_LEVEL, face_uv_to_xyz, st_to_uv, unit_dir};
    use hyperion_base::math;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    const EARTH_VOLUMETRIC_M: f64 = 6.371e6;
    const WGS84_EQUATORIAL_M: f64 = 6.378_137e6;

    #[test]
    fn an_earth_is_level_19() {
        assert_eq!(finest_level(EARTH_VOLUMETRIC_M), 19);
        assert_eq!(finest_level(WGS84_EQUATORIAL_M), 19);
        let spacing = vertex_spacing(EARTH_VOLUMETRIC_M, 19);
        assert!(
            (spacing.mean_m / 0.277 - 1.0).abs() < 0.01,
            "mean {}",
            spacing.mean_m
        );
        assert!(spacing.max_m <= 0.375, "max {}", spacing.max_m);
        assert!(spacing.min_m > 0.17 && spacing.min_m < 0.19);
        // Level 18 fails the claim, at 0.65 m (Design note 3).
        assert!((vertex_spacing(EARTH_VOLUMETRIC_M, 18).max_m - 0.647).abs() < 0.001);
    }

    #[test]
    fn the_moon_and_ceres_are_pinned() {
        // The Moon's mean radius, 1,737.4 km (Archinal et al. 2018, the IAU WGCCRE report,
        // Celestial Mechanics and Dynamical Astronomy 130, 22), and Ceres's, 469.7 km (469.73 km,
        // the equivalent spherical radius of Raymond and Roatsch 2018, "Ceres Coordinate System
        // Description", PDS DAWN-A-FC2-5-CERESSHAPESPC-V1.0, Table 1, "Dawn Final", from the
        // stereophotoclinometry shape model of Park et al. 2019, Icarus 319, 812), as plan R05,
        // T1.b names them. Ceres's level 15 misses the bound by 1.8%, at 0.382 m.
        assert_eq!(finest_level(1.7374e6), 17);
        assert_eq!(finest_level(4.697e5), 16);
    }

    #[test]
    fn the_finest_level_is_the_shallowest_within_the_bound() {
        // 100 radii from 100 km to 70,000 km, evenly in the logarithm.
        for k in 0..100 {
            let radius = 1.0e5 * math::powf(700.0, f64::from(k) / 99.0);
            let level = finest_level(radius);
            assert!(level < MAX_LEVEL);
            assert!(vertex_spacing(radius, level).max_m <= MAX_FINEST_SPACING_M);
            assert!(vertex_spacing(radius, level - 1).max_m > MAX_FINEST_SPACING_M);
        }
    }

    fn arc(a: [f64; 3], b: [f64; 3]) -> f64 {
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let sin = math::hypot(math::hypot(cross[0], cross[1]), cross[2]);
        math::atan2(sin, a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
    }

    fn point(s: f64, t: f64) -> [f64; 3] {
        unit_dir(face_uv_to_xyz(FaceUv {
            face: Face::PosX,
            u: st_to_uv(s),
            v: st_to_uv(t),
        }))
    }

    #[test]
    fn the_rates_are_the_extremes_of_a_fine_grid() {
        // Every u-line edge of a 512 × 512 grid on one face, scaled to unit step: the extremes and
        // the mean agree with the closed forms to the grid's resolution.
        let n = 512_u32;
        let step = 1.0 / f64::from(n);
        let (mut lo, mut hi, mut sum) = (f64::INFINITY, 0.0_f64, 0.0);
        for j in 0..=n {
            let t = f64::from(j) * step;
            for i in 0..n {
                let s = f64::from(i) * step;
                let rate = arc(point(s, t), point(s + step, t)) / step;
                lo = lo.min(rate);
                hi = hi.max(rate);
                if j < n {
                    sum += arc(point(s, t + step / 2.0), point(s + step, t + step / 2.0));
                }
            }
        }
        let mean = sum / f64::from(n);
        assert!(
            (hi / max_rate() - 1.0).abs() < 1e-4,
            "{hi} vs {}",
            max_rate()
        );
        assert!(hi <= max_rate() * (1.0 + 1e-9));
        // The rate has a kink at the minimum (du/ds is smallest at s = ½), so a finite edge there
        // overshoots it to first order in the step.
        assert!(
            (lo / min_rate() - 1.0).abs() < 3e-3,
            "{lo} vs {}",
            min_rate()
        );
        assert!(lo >= min_rate() * (1.0 - 1e-9));
        assert!(
            (mean / MEAN_RATE - 1.0).abs() < 1e-5,
            "{mean} vs {MEAN_RATE}"
        );
    }

    #[test]
    fn the_mean_rate_is_its_integral() {
        // Simpson's rule on ∫ 2 atan(1 ÷ √(1 + v(t)²)) dt over [½, 1], doubled by symmetry.
        let f = |t: f64| {
            let v = st_to_uv(t);
            2.0 * math::atan(1.0 / (1.0 + v * v).sqrt())
        };
        let n = 2_000_u32;
        let h = 0.5 / f64::from(n);
        let mut sum = f(0.5) + f(1.0);
        for k in 1..n {
            let weight = if k % 2 == 1 { 4.0 } else { 2.0 };
            sum += weight * f(0.5 + f64::from(k) * h);
        }
        let mean = 2.0 * sum * h / 3.0;
        assert!((mean - MEAN_RATE).abs() < 1e-12, "{mean} vs {MEAN_RATE}");
    }

    #[test]
    fn the_rates_ratios_are_design_note_3s() {
        assert!((max_rate() / MEAN_RATE - 1.17).abs() < 0.005);
        assert!((min_rate() / MEAN_RATE - 0.65).abs() < 0.005);
    }
}
