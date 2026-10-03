//! A body's rotational spheroid, the one datum heights are measured from (plan 14, Phase J,
//! P14.T46.e; plan R05, Design note 5).
//!
//! A body's figure is the spheroid of revolution about its pole with equatorial radius a and
//! polar radius c; height is measured along the spheroid's normal above it, and a sphere is the
//! case a = c. This module was written by R05's lane before P14.T46.e's own landed, to the API the
//! orchestrator described for it (`sphere`, `from_volumetric`, `flattening`,
//! `volumetric_radius_m`); R05's geometry of the datum is the second `impl` block.

use hyperion_base::math;

/// A rotational spheroid: equatorial radius a and polar radius c, metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spheroid {
    /// The equatorial radius a, metres.
    pub equatorial_radius_m: f64,
    /// The polar radius c, metres.
    pub polar_radius_m: f64,
}

/// Why a spheroid could not be built from a volumetric radius and a flattening.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildSpheroidError {
    /// The radius is not finite and positive.
    Radius(f64),
    /// The flattening is not finite and in [0, 1).
    Flattening(f64),
}

impl std::fmt::Display for BuildSpheroidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Radius(r) => write!(f, "volumetric radius {r} m is not finite and positive"),
            Self::Flattening(x) => write!(f, "flattening {x} is not finite and in [0, 1)"),
        }
    }
}

impl std::error::Error for BuildSpheroidError {}

impl Spheroid {
    /// The sphere of radius `radius_m` metres.
    #[must_use]
    pub const fn sphere(radius_m: f64) -> Self {
        Self {
            equatorial_radius_m: radius_m,
            polar_radius_m: radius_m,
        }
    }

    /// The spheroid of volumetric radius `radius_m` (the sphere of equal volume, a² c = R³) and
    /// flattening `f` = (a − c) ÷ a: a = R ÷ ∛(1 − f), c = a (1 − f).
    ///
    /// # Errors
    ///
    /// [`BuildSpheroidError`] if the radius is not finite and positive or the flattening is not
    /// finite and in [0, 1).
    pub fn from_volumetric(radius_m: f64, f: f64) -> Result<Self, BuildSpheroidError> {
        if !(radius_m.is_finite() && radius_m > 0.0) {
            return Err(BuildSpheroidError::Radius(radius_m));
        }
        if !(f.is_finite() && (0.0..1.0).contains(&f)) {
            return Err(BuildSpheroidError::Flattening(f));
        }
        let a = radius_m / math::cbrt(1.0 - f);
        Ok(Self {
            equatorial_radius_m: a,
            polar_radius_m: a * (1.0 - f),
        })
    }

    /// The flattening (a − c) ÷ a.
    #[must_use]
    pub fn flattening(&self) -> f64 {
        (self.equatorial_radius_m - self.polar_radius_m) / self.equatorial_radius_m
    }

    /// The volumetric radius ∛(a² c), metres.
    #[must_use]
    pub fn volumetric_radius_m(&self) -> f64 {
        let a = self.equatorial_radius_m;
        math::cbrt(a * a * self.polar_radius_m)
    }
}

/// R05's geometry of the datum (plan R05, Design note 5): in the body-fixed frame with z along the
/// pole, M = diag(a, a, c) maps a unit direction d to the spheroid point M·d, whose outward normal
/// is ν = M⁻¹d ÷ |M⁻¹d|, and a point at height h above the datum along its normal is
/// P = M·d + h·ν. A sphere is the case a = c.
impl Spheroid {
    /// WGS 84's ellipsoid: a = 6,378,137 m and f = 1 ÷ 298.257223563 (NIMA TR8350.2, 3rd edition,
    /// 2000, Table 3.1), so c = a (1 − f) = 6,356,752.314 245 m, the semi-minor axis (b in
    /// TR8350.2's Table 3.3, which lists 6,356,752.3142 m and uses c for another quantity). The
    /// figure of R05's test planet.
    pub const WGS84: Self = Self {
        equatorial_radius_m: 6_378_137.0,
        polar_radius_m: 6_378_137.0 * (1.0 - 1.0 / 298.257_223_563),
    };

    /// The spheroid point M·d for the unit direction `dir`, metres.
    #[must_use]
    pub fn point(&self, dir: [f64; 3]) -> [f64; 3] {
        [
            self.equatorial_radius_m * dir[0],
            self.equatorial_radius_m * dir[1],
            self.polar_radius_m * dir[2],
        ]
    }

    /// The outward unit normal ν = M⁻¹d ÷ |M⁻¹d| at the spheroid point of the unit direction
    /// `dir`.
    #[must_use]
    pub fn normal(&self, dir: [f64; 3]) -> [f64; 3] {
        let m = [
            dir[0] / self.equatorial_radius_m,
            dir[1] / self.equatorial_radius_m,
            dir[2] / self.polar_radius_m,
        ];
        let len = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
        [m[0] / len, m[1] / len, m[2] / len]
    }

    /// The point at height `h_m` metres above the spheroid along its normal, over the unit
    /// direction `dir`: P = M·d + h·ν, metres.
    #[must_use]
    pub fn surface_point(&self, dir: [f64; 3], h_m: f64) -> [f64; 3] {
        let p = self.point(dir);
        let nu = self.normal(dir);
        [p[0] + h_m * nu[0], p[1] + h_m * nu[1], p[2] + h_m * nu[2]]
    }

    /// The meridional and prime-vertical radii of curvature, M and N, metres, at the spheroid point
    /// of the unit direction `dir`: with e² = 1 − c²/a² and the geodetic latitude's sine sin φ = `ν_z`,
    /// M = a (1 − e²) ÷ (1 − e² sin²φ)^{3/2} and N = a ÷ (1 − e² sin²φ)^{1/2} (NIMA TR8350.2,
    /// eq. 4-15 for N, and §7.4, p. 7-4, for M, which it calls `R_M`).
    #[must_use]
    pub fn curvature_radii_m(&self, dir: [f64; 3]) -> (f64, f64) {
        let a = self.equatorial_radius_m;
        let c = self.polar_radius_m;
        let e2 = 1.0 - (c * c) / (a * a);
        let sin_phi = self.normal(dir)[2];
        let w2 = 1.0 - e2 * sin_phi * sin_phi;
        let w = w2.sqrt();
        (a * (1.0 - e2) / (w2 * w), a / w)
    }

    /// The radius of curvature, metres, of the normal section in the unit tangent direction
    /// `tangent` at the spheroid point of `dir`: Euler's 1/ρ = cos²α ÷ M + sin²α ÷ N, with α the
    /// direction's azimuth from the meridian.
    #[must_use]
    pub fn section_radius_m(&self, dir: [f64; 3], tangent: [f64; 3]) -> f64 {
        let (m, n) = self.curvature_radii_m(dir);
        let nu = self.normal(dir);
        // East is ẑ × ν, undefined at a pole, where M = N and the azimuth does not matter.
        let east = [-nu[1], nu[0], 0.0];
        let east_len = (east[0] * east[0] + east[1] * east[1]).sqrt();
        if east_len <= 0.0 {
            return m;
        }
        let sin_alpha = (tangent[0] * east[0] + tangent[1] * east[1]) / east_len;
        let sin2 = crate::num::min(sin_alpha * sin_alpha, 1.0);
        let cos2 = 1.0 - sin2;
        1.0 / (cos2 / m + sin2 / n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn wgs84s_volumetric_radius_and_flattening() {
        let w = Spheroid::WGS84;
        assert!((w.flattening() - 1.0 / 298.257_223_563).abs() < 1e-15);
        // The volumetric radius of WGS 84, 6,371,000.79 m (NIMA TR8350.2, Table 3.3).
        assert!((w.volumetric_radius_m() - 6_371_000.79).abs() < 0.01);
        let back = Spheroid::from_volumetric(w.volumetric_radius_m(), w.flattening()).unwrap();
        assert!((back.equatorial_radius_m - w.equatorial_radius_m).abs() < 1e-6);
        assert!((back.polar_radius_m - w.polar_radius_m).abs() < 1e-6);
    }

    #[test]
    fn bad_figures_are_refused() {
        assert_eq!(
            Spheroid::from_volumetric(-1.0, 0.0),
            Err(BuildSpheroidError::Radius(-1.0))
        );
        assert_eq!(
            Spheroid::from_volumetric(1.0, 1.0),
            Err(BuildSpheroidError::Flattening(1.0))
        );
    }

    #[test]
    #[expect(
        clippy::many_single_char_names,
        reason = "the textbook names of the figure, its point, gradient and radii"
    )]
    fn the_normal_is_the_gradient_and_the_radii_are_wgs84s() {
        let w = Spheroid::WGS84;
        let d = crate::cube::unit_dir([0.3, -0.5, 0.8]);
        let p = w.point(d);
        let a2 = w.equatorial_radius_m * w.equatorial_radius_m;
        let c2 = w.polar_radius_m * w.polar_radius_m;
        let g = [p[0] / a2, p[1] / a2, p[2] / c2];
        let len = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
        let nu = w.normal(d);
        for k in 0..3 {
            assert!((nu[k] - g[k] / len).abs() < 1e-15);
        }
        // At the equator M = a (1 − e²) and N = a; at the pole both are a² ÷ c.
        let (m, n) = w.curvature_radii_m([1.0, 0.0, 0.0]);
        let e2 = 1.0 - c2 / a2;
        assert!((m - w.equatorial_radius_m * (1.0 - e2)).abs() < 1e-6);
        assert!((n - w.equatorial_radius_m).abs() < 1e-6);
        let (m, n) = w.curvature_radii_m([0.0, 0.0, 1.0]);
        assert!((m - a2 / w.polar_radius_m).abs() < 1e-6 && (n - m).abs() < 1e-6);
        // A sphere's every section has its radius.
        let s = Spheroid::sphere(1_000.0);
        assert!(
            (s.section_radius_m(d, crate::cube::unit_dir([0.8, 0.0, -0.3])) - 1_000.0).abs() < 1e-9
        );
    }
}
