//! Spherical mass components: the black hole in closed form and the nuclear cluster on its shared
//! table (plan 02, P02.T6.c; ruling 144 of 2026-09-22), and the interface they share with the dark
//! halo ([`Nfw`](super::nfw::Nfw)).

use std::sync::OnceLock;

use super::BuildComponentError;
use crate::galaxy::consts::G;
use crate::galaxy::features::centre::{TracerProfile, TracerShape};
use crate::galaxy::params::NuclearClusterParams;
use crate::units::{LightYears, SolarMasses};

/// A spherical mass distribution with its enclosed mass and potential at any radius.
///
/// Radii are in light-years from the centre, potentials in (km/s)², zero at infinity. Every
/// quantity is taken at `r > 0`; at the centre a point mass's potential is infinite.
pub trait SphericalMass {
    /// The mass inside radius `r`.
    fn enclosed_mass(&self, r: LightYears) -> SolarMasses;

    /// The density at radius `r`, M☉ per cubic light-year.
    fn density(&self, r: LightYears) -> f64;

    /// The potential at radius `r`, (km/s)².
    fn potential(&self, r: LightYears) -> f64;

    /// The circular speed squared `G M(<r) ÷ r`, (km/s)².
    fn v_circ_sq(&self, r: LightYears) -> f64 {
        G * self.enclosed_mass(r).value() / r.value()
    }

    /// `d v_c² ÷ d ln r = G (4π r² ρ − M(<r) ÷ r)`, (km/s)².
    fn v_circ_sq_slope(&self, r: LightYears) -> f64 {
        let x = r.value();
        G * (4.0 * core::f64::consts::PI * x * x * self.density(r)
            - self.enclosed_mass(r).value() / x)
    }

    /// `d v_c² ÷ dr`, (km/s)² per light-year.
    fn v_circ_sq_derivative(&self, r: LightYears) -> f64 {
        self.v_circ_sq_slope(r) / r.value()
    }
}

/// A point mass: the central black hole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointMass {
    mass: SolarMasses,
}

impl PointMass {
    /// A point mass of `mass`, which may be zero.
    ///
    /// # Errors
    ///
    /// [`BuildComponentError`] if the mass is negative or not finite.
    pub fn new(mass: SolarMasses) -> Result<Self, BuildComponentError> {
        BuildComponentError::check_non_negative("mass", mass.value())?;
        Ok(Self { mass })
    }

    /// The mass.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }
}

impl SphericalMass for PointMass {
    fn enclosed_mass(&self, _r: LightYears) -> SolarMasses {
        self.mass
    }

    fn density(&self, _r: LightYears) -> f64 {
        0.0
    }

    fn potential(&self, r: LightYears) -> f64 {
        -G * self.mass.value() / r.value()
    }

    fn v_circ_sq_slope(&self, r: LightYears) -> f64 {
        -self.v_circ_sq(r)
    }
}

/// The nuclear star cluster: plan 02's [`NuclearClusterParams`] law, which the centre holds too
/// (plan 09's [`CentreProfile`](crate::galaxy::features::centre::CentreProfile); ruling 144 of
/// 2026-09-22's joint revision).
///
/// The law is the 3D Nuker law with a smooth break of sharpness α = 10 at 10 ly and a taper near
/// the centre's reach ([`NuclearClusterParams`]), whose mass and potential have no closed form:
/// its shape is tabulated once, normalised to one ([`TracerProfile`], its integrals at 32 knots a
/// decade by Gauss–Legendre panels, within 10⁻⁷ of a direct quadrature), and shared by every
/// galaxy, since only the mass differs. The mass given is the mass inside
/// [`NuclearClusterParams::NORMALISATION_RADIUS`]; the whole law holds that over the tabulated
/// share inside it, 1 ÷ 0.968. With `F` the share inside `r` and `W` the outer moment `∫ᵣ^∞ 4πr′
/// n dr′` of the normalised density `n`:
///
/// - `M(<r) = M F(r)`, `ρ = M n(r)`;
/// - `Φ(r) = −G M (F(r) ÷ r + W(r))`.
///
/// Until the joint revision the potential held a sharp broken power law of the whole mass in
/// closed form, which disagreed with the centre's smooth one (ruling 144.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NuclearCluster {
    /// The whole law's mass.
    total: SolarMasses,
    profile: &'static TracerProfile,
}

impl NuclearCluster {
    /// The cluster holding `mass` inside [`NuclearClusterParams::NORMALISATION_RADIUS`], which may
    /// be zero.
    ///
    /// # Errors
    ///
    /// [`BuildComponentError`] if the mass is negative or not finite.
    pub fn new(mass: SolarMasses) -> Result<Self, BuildComponentError> {
        BuildComponentError::check_non_negative("mass", mass.value())?;
        let profile = Self::shape();
        let inside = profile.fraction_within(NuclearClusterParams::NORMALISATION_RADIUS.value());
        Ok(Self {
            total: SolarMasses::new(mass.value() / inside),
            profile,
        })
    }

    /// The nuclear star cluster of `params` (plan 02, Design note 15).
    ///
    /// # Panics
    ///
    /// Never: the parameters' mass is non-negative and finite.
    #[must_use]
    pub fn of(params: &NuclearClusterParams) -> Self {
        Self::new(params.mass()).expect("the nuclear cluster's mass is valid")
    }

    /// The law's shape normalised to one, tabulated on first use and shared by every cluster.
    #[must_use]
    pub fn shape() -> &'static TracerProfile {
        static SHAPE: OnceLock<TracerProfile> = OnceLock::new();
        SHAPE.get_or_init(|| TracerProfile::new(TracerShape::nuclear_cluster()))
    }

    /// The whole law's mass, inside the reach and beyond it.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.total
    }

    /// The mass inside [`NuclearClusterParams::NORMALISATION_RADIUS`]: the parameter's.
    #[must_use]
    pub fn mass_within_reach(&self) -> SolarMasses {
        self.enclosed_mass(NuclearClusterParams::NORMALISATION_RADIUS)
    }
}

impl SphericalMass for NuclearCluster {
    fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        SolarMasses::new(self.total.value() * self.profile.fraction_within(r.value()))
    }

    fn density(&self, r: LightYears) -> f64 {
        self.total.value() * self.profile.density(r.value())
    }

    fn potential(&self, r: LightYears) -> f64 {
        let x = r.value();
        let inner = if x > 0.0 {
            self.profile.fraction_within(x) / x
        } else {
            0.0
        };
        let outer = self.profile.outer_moment(x.max(f64::MIN_POSITIVE));
        -G * self.total.value() * (inner + outer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::quad::gl_log_panels;
    use crate::math;

    fn cluster() -> NuclearCluster {
        NuclearCluster::new(SolarMasses::new(2.5e7)).unwrap()
    }

    /// The parameter is the mass inside the reach, the whole law's is 3.3% more, and the mass
    /// rises to it: the taper's r^−5.5 leaves 6 × 10⁻³ beyond 256 ly (ruling 144.3).
    #[test]
    fn the_cluster_holds_its_parameter_inside_the_reach() {
        let c = cluster();
        assert!((c.mass_within_reach().value() / 2.5e7 - 1.0).abs() < 1e-12);
        let total = c.mass().value() / 2.5e7;
        assert!((1.030..1.036).contains(&total), "{total}");
        let at = |r: f64| c.enclosed_mass(LightYears::new(r)).value() / c.mass().value();
        assert!((at(1e9) - 1.0).abs() < 1e-9);
        assert!((0.990..0.996).contains(&at(256.0)), "{}", at(256.0));
        let mut previous = 0.0;
        for i in -40..=60 {
            let m = at(10.0 * math::exp(f64::from(i) * 0.1));
            assert!(m > previous);
            previous = m;
        }
    }

    /// The tabulated mass and potential agree with quadratures of the density.
    #[test]
    fn the_cluster_mass_and_potential_integrate_the_density() {
        let c = cluster();
        for r in [0.1_f64, 3.0, 10.0, 30.0, 128.0, 500.0] {
            let edges: Vec<f64> = [1e-9, 1e-6, 1e-3, 0.01, 0.1, 1.0, 10.0, 100.0]
                .into_iter()
                .filter(|&e| e < r)
                .chain([r])
                .collect();
            let shell =
                |x: f64| 4.0 * core::f64::consts::PI * x * x * c.density(LightYears::new(x));
            let mass = gl_log_panels(shell, &edges);
            let closed = c.enclosed_mass(LightYears::new(r)).value();
            assert!(
                (mass / closed - 1.0).abs() < 1e-6,
                "M({r}) {mass} against {closed}"
            );
            // Φ(r) = −∫_r^∞ G M(<x) ÷ x² dx.
            let mut outer: Vec<f64> = [r, 10.0 * r, 1e3 * r, 1e6 * r, 1e12 * r]
                .into_iter()
                .chain([10.0, 100.0].into_iter().filter(|&b| b > r))
                .collect();
            outer.sort_by(f64::total_cmp);
            let force = |x: f64| G * c.enclosed_mass(LightYears::new(x)).value() / (x * x);
            let phi = -gl_log_panels(force, &outer);
            let closed = c.potential(LightYears::new(r));
            assert!(
                (phi / closed - 1.0).abs() < 1e-6,
                "Φ({r}) {phi} against {closed}"
            );
        }
        assert!(c.potential(LightYears::ZERO).is_finite());
        assert!(
            (c.potential(LightYears::new(1e-12)) / c.potential(LightYears::ZERO) - 1.0).abs()
                < 1e-9
        );
    }

    #[test]
    fn a_point_mass_is_keplerian() {
        let bh = PointMass::new(SolarMasses::new(4.3e6)).unwrap();
        let r = LightYears::new(3.0);
        assert!((bh.v_circ_sq(r) + bh.potential(r)).abs() < 1e-9);
        assert!((bh.v_circ_sq_slope(r) + bh.v_circ_sq(r)).abs() < 1e-9);
        assert_eq!(
            PointMass::new(SolarMasses::new(-1.0)),
            Err(BuildComponentError {
                quantity: "mass",
                value: -1.0
            })
        );
    }

    #[test]
    fn a_negative_cluster_mass_is_rejected() {
        assert_eq!(
            NuclearCluster::new(SolarMasses::new(-1.0)),
            Err(BuildComponentError {
                quantity: "mass",
                value: -1.0
            })
        );
        let empty = NuclearCluster::new(SolarMasses::ZERO).unwrap();
        assert!(empty.potential(LightYears::new(3.0)).abs() < 1e-300);
    }
}
