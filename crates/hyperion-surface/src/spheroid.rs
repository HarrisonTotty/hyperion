//! The reference spheroid: a body's figure, and the one datum every height is measured from.
//!
//! A body's figure is the rotational spheroid (a, a, c) about its pole: `a` the equatorial radius,
//! `c` the polar. Every height a generator states or derives from it (plan 14's relief and an ocean
//! world's sea level, the rendering plans' coarse elevations and terrain) is a geodetic height
//! along this spheroid's normal, not a radius from the centre. On an Earth a sphere of the mean
//! radius would put sea level 7 km high at the equator and 14 km low at the poles (WGS 84:
//! a = 6,378,137 m, 1 ÷ f = 298.257223563, c = 6,356,752.314 m; NIMA TR8350.2, 3rd ed., 2000,
//! Table 3.1 and Table 3.3). Decided for the rendering plans by the coordinator on 2026-09-29
//! (R07 Design note 19, R05 Design note 5); plan 14's P14.T46.e gives every generated body one.
//!
//! The sim depends on this crate, never the reverse, so the type lives here and the sim's
//! `BodyFigure` holds it. Plan 14 adds only the constructor from the volumetric radius and the
//! accessors here; R05 adds the point and normal functions beside them.
//!
//! The volumetric (mean) radius is the radius of the sphere of equal volume, R = (a² c)^⅓, which
//! is what a planet record's `radius_m` holds, so density and gravity read the same radius with or
//! without a figure. Both cube roots go through [`hyperion_base::math::cbrt`], since the client runs
//! this crate as WebAssembly and must agree with the server bit for bit.

use std::error::Error;
use std::fmt;

use hyperion_base::math;

/// A rotational spheroid (a, a, c) about a body's pole, in metres.
///
/// An oblate body has `polar_radius_m` below `equatorial_radius_m`; a sphere has the two equal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spheroid {
    /// The equatorial radius a, metres.
    pub equatorial_radius_m: f64,
    /// The polar radius c, metres.
    pub polar_radius_m: f64,
}

impl Spheroid {
    /// The spheroid of volumetric radius `radius_m` and flattening `flattening` = (a − c) ÷ a,
    /// keeping the volume: a = R (1 − f)^(−⅓) and c = a (1 − f).
    ///
    /// # Errors
    ///
    /// [`BuildSpheroidError::RadiusNotPositive`] if `radius_m` is not positive and finite, and
    /// [`BuildSpheroidError::FlatteningOutOfRange`] if `flattening` is outside [0, 1) (NaN
    /// included).
    pub fn from_volumetric(radius_m: f64, flattening: f64) -> Result<Self, BuildSpheroidError> {
        if !(radius_m > 0.0 && radius_m.is_finite()) {
            return Err(BuildSpheroidError::RadiusNotPositive);
        }
        if !(0.0..1.0).contains(&flattening) {
            return Err(BuildSpheroidError::FlatteningOutOfRange);
        }
        let axis_ratio = 1.0 - flattening;
        let equatorial_radius_m = radius_m / math::cbrt(axis_ratio);
        Ok(Self {
            equatorial_radius_m,
            polar_radius_m: equatorial_radius_m * axis_ratio,
        })
    }

    /// The sphere of radius `radius_m`, a spheroid of flattening 0.
    #[must_use]
    pub const fn sphere(radius_m: f64) -> Self {
        Self {
            equatorial_radius_m: radius_m,
            polar_radius_m: radius_m,
        }
    }

    /// The flattening (a − c) ÷ a, 0 for a sphere.
    #[must_use]
    pub fn flattening(&self) -> f64 {
        (self.equatorial_radius_m - self.polar_radius_m) / self.equatorial_radius_m
    }

    /// The volumetric radius (a² c)^⅓, metres: the radius of the sphere of equal volume.
    #[must_use]
    pub fn volumetric_radius_m(&self) -> f64 {
        math::cbrt(self.equatorial_radius_m * self.equatorial_radius_m * self.polar_radius_m)
    }
}

/// A [`Spheroid`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSpheroidError {
    /// The volumetric radius was not positive and finite.
    RadiusNotPositive,
    /// The flattening was outside [0, 1).
    FlatteningOutOfRange,
}

impl fmt::Display for BuildSpheroidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RadiusNotPositive => "a spheroid's volumetric radius must be positive and finite",
            Self::FlatteningOutOfRange => "a spheroid's flattening must lie in [0, 1)",
        })
    }
}

impl Error for BuildSpheroidError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// WGS 84's semi-major axis, metres (NIMA TR8350.2, Table 3.1).
    const WGS84_A: f64 = 6_378_137.0;
    /// WGS 84's inverse flattening (NIMA TR8350.2, Table 3.1).
    const WGS84_INVERSE_F: f64 = 298.257_223_563;
    /// WGS 84's semi-minor axis, metres (NIMA TR8350.2, Table 3.3).
    const WGS84_C: f64 = 6_356_752.314_2;
    /// The radius of the sphere of equal volume, metres (NIMA TR8350.2, Table 3.3: 6,371,000.7900).
    const WGS84_VOLUMETRIC: f64 = 6_371_000.79;

    /// The volumetric radius and the flattening rebuild WGS 84's axes to a millimetre.
    #[test]
    fn wgs84_from_volumetric() {
        let s = Spheroid::from_volumetric(WGS84_VOLUMETRIC, 1.0 / WGS84_INVERSE_F).unwrap();
        assert!(
            (s.equatorial_radius_m - WGS84_A).abs() < 1e-3,
            "a = {}",
            s.equatorial_radius_m
        );
        assert!(
            (s.polar_radius_m - WGS84_C).abs() < 1e-3,
            "c = {}",
            s.polar_radius_m
        );
        assert!((s.flattening() * WGS84_INVERSE_F - 1.0).abs() < 1e-12);
    }

    /// The constructor keeps the volume: the volumetric radius returns to 10⁻¹².
    #[test]
    fn volume_is_kept() {
        for &(r, f) in &[
            (6.371e6, 0.003_352_8),
            (6.9911e7, 0.064_87),
            (5.8232e7, 0.097_96),
            (1.0e3, 0.0),
            (2.0e5, 0.2),
            (1.0, 0.999),
        ] {
            let s = Spheroid::from_volumetric(r, f).unwrap();
            assert!(
                (s.volumetric_radius_m() / r - 1.0).abs() < 1e-12,
                "R {r}, f {f}"
            );
            assert!((s.flattening() - f).abs() < 1e-12, "R {r}, f {f}");
        }
    }

    /// A sphere has flattening 0 and its radius as both axes and as its volumetric radius.
    #[test]
    fn sphere_is_flattening_zero() {
        let s = Spheroid::sphere(1_737_400.0);
        assert!(s.flattening().abs() < f64::MIN_POSITIVE);
        assert_eq!(s, Spheroid::from_volumetric(1_737_400.0, 0.0).unwrap());
        assert!((s.volumetric_radius_m() - 1_737_400.0).abs() < 1e-6);
    }

    /// A flattening outside [0, 1) or a radius that is not positive and finite is refused.
    #[test]
    fn bad_inputs_are_refused() {
        for f in [-1e-9, 1.0, 1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Spheroid::from_volumetric(1.0e6, f),
                Err(BuildSpheroidError::FlatteningOutOfRange),
                "f {f}"
            );
        }
        for r in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Spheroid::from_volumetric(r, 0.1),
                Err(BuildSpheroidError::RadiusNotPositive),
                "R {r}"
            );
        }
    }
}
