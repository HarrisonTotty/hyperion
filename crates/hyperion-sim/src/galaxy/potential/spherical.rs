//! Spherical mass components in closed form: the black hole and the nuclear cluster (plan 02,
//! P02.T6.c), and the interface they share with the dark halo ([`Nfw`](super::nfw::Nfw)).

use super::BuildComponentError;
use crate::galaxy::consts::G;
use crate::galaxy::params::NuclearClusterParams;
use crate::math;
use crate::units::{LightYears, SolarMasses};

/// A spherical mass distribution with its enclosed mass and potential in closed form.
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

/// A broken power law with a sharp break: `ρ = ρ_b (r ÷ r_b)^−γ₁` inside the break radius `r_b`
/// and `ρ_b (r ÷ r_b)^−γ₂` outside, with `γ₁ < 2` so that the potential is finite at the centre
/// and `γ₂ > 3` so that the mass is.
///
/// It stands for the nuclear star cluster: inner slope 1.3 (Gallego-Cano et al. 2018, A&A 609,
/// A26), break 10 ly and outer slope 3.5 (brainstorm, "Dense features"). The brainstorm gives
/// the two slopes and the break and nothing of how sharp the break is; a sharp one needs no
/// further parameter and has the enclosed mass and the potential in closed form. With `x = r ÷
/// r_b` and `A = 4π ρ_b r_b³`:
///
/// - `M(<r) = A x^(3−γ₁) ÷ (3 − γ₁)` inside, `A [1 ÷ (3 − γ₁) + (1 − x^(3−γ₂)) ÷ (γ₂ − 3)]`
///   outside, and in total `A [1 ÷ (3 − γ₁) + 1 ÷ (γ₂ − 3)]`;
/// - `Φ(r) = −G M(<r) ÷ r − 4πG ∫_r^∞ ρ r′ dr′`, where the integral is `ρ_b r_b² [(1 −
///   x^(2−γ₁)) ÷ (2 − γ₁) + 1 ÷ (γ₂ − 2)]` inside and `ρ_b r_b² x^(2−γ₂) ÷ (γ₂ − 2)` outside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrokenPowerLaw {
    mass: SolarMasses,
    break_radius: LightYears,
    inner_slope: f64,
    outer_slope: f64,
    /// The density at the break, M☉ ly⁻³.
    rho_break: f64,
}

impl BrokenPowerLaw {
    /// A broken power law of total `mass`, which may be zero, with the given break radius and
    /// slopes.
    ///
    /// # Errors
    ///
    /// [`BuildComponentError`] if the mass is negative, the break radius not positive, the inner
    /// slope outside `[0, 2)` or the outer slope not above 3.
    pub fn new(
        mass: SolarMasses,
        break_radius: LightYears,
        inner_slope: f64,
        outer_slope: f64,
    ) -> Result<Self, BuildComponentError> {
        BuildComponentError::check_non_negative("mass", mass.value())?;
        BuildComponentError::check_positive("break radius", break_radius.value())?;
        BuildComponentError::check_non_negative("inner slope", inner_slope)?;
        BuildComponentError::check_positive("2 − inner slope", 2.0 - inner_slope)?;
        BuildComponentError::check_positive("outer slope − 3", outer_slope - 3.0)?;
        let rb = break_radius.value();
        let shape = 1.0 / (3.0 - inner_slope) + 1.0 / (outer_slope - 3.0);
        Ok(Self {
            mass,
            break_radius,
            inner_slope,
            outer_slope,
            rho_break: mass.value() / (4.0 * core::f64::consts::PI * rb * rb * rb * shape),
        })
    }

    /// The nuclear star cluster of `params` (plan 02, Design note 15).
    ///
    /// # Panics
    ///
    /// Never: the parameters' mass is non-negative and their slopes are fixed constants inside
    /// the ranges [`new`](Self::new) accepts.
    #[must_use]
    pub fn nuclear_cluster(params: &NuclearClusterParams) -> Self {
        Self::new(
            params.mass(),
            params.break_radius(),
            params.inner_slope(),
            params.outer_slope(),
        )
        .expect("the nuclear cluster's parameters are valid")
    }

    /// The total mass.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The break radius.
    #[must_use]
    pub fn break_radius(&self) -> LightYears {
        self.break_radius
    }

    /// `A = 4π ρ_b r_b³`, M☉.
    fn scale_mass(&self) -> f64 {
        let rb = self.break_radius.value();
        4.0 * core::f64::consts::PI * self.rho_break * rb * rb * rb
    }
}

impl SphericalMass for BrokenPowerLaw {
    fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        let x = r.value() / self.break_radius.value();
        let (g1, g2) = (self.inner_slope, self.outer_slope);
        let fraction = if x <= 1.0 {
            math::powf(x, 3.0 - g1) / (3.0 - g1)
        } else {
            1.0 / (3.0 - g1) + (1.0 - math::powf(x, 3.0 - g2)) / (g2 - 3.0)
        };
        SolarMasses::new(self.scale_mass() * fraction)
    }

    fn density(&self, r: LightYears) -> f64 {
        let x = r.value() / self.break_radius.value();
        let slope = if x <= 1.0 {
            self.inner_slope
        } else {
            self.outer_slope
        };
        self.rho_break * math::powf(x, -slope)
    }

    fn potential(&self, r: LightYears) -> f64 {
        let rb = self.break_radius.value();
        let x = r.value() / rb;
        let (g1, g2) = (self.inner_slope, self.outer_slope);
        let outside = if x <= 1.0 {
            (1.0 - math::powf(x, 2.0 - g1)) / (2.0 - g1) + 1.0 / (g2 - 2.0)
        } else {
            math::powf(x, 2.0 - g2) / (g2 - 2.0)
        };
        let shell = 4.0 * core::f64::consts::PI * G * self.rho_break * rb * rb * outside;
        if r.value() > 0.0 {
            -G * self.enclosed_mass(r).value() / r.value() - shell
        } else {
            -shell
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::quad::gl_log_panels;

    fn cluster() -> BrokenPowerLaw {
        BrokenPowerLaw::new(SolarMasses::new(2.5e7), LightYears::new(10.0), 1.3, 3.5).unwrap()
    }

    /// The nuclear cluster's mass converges to its parameter, slowly, as the outer slope 3.5
    /// gives: the mass outside r falls as r^−½.
    #[test]
    fn the_cluster_mass_converges_to_its_parameter() {
        let c = cluster();
        let at = |r: f64| c.enclosed_mass(LightYears::new(r)).value() / 2.5e7;
        assert!(at(1e12) > 1.0 - 1e-5 && at(1e12) <= 1.0 + 1e-12);
        // At 1,000 break radii the mass outside is 2 × 1000^−½ of A, out of A (1 ÷ 1.7 + 2).
        let outside = 2.0 / 1_000_f64.sqrt() / (1.0 / 1.7 + 2.0);
        assert!((at(1e4) - (1.0 - outside)).abs() < 1e-12);
        let mut previous = 0.0;
        for i in -40..=60 {
            let m = at(10.0 * math::exp(f64::from(i) * 0.1));
            assert!(m > previous);
            previous = m;
        }
    }

    /// The closed forms agree with quadratures of the density.
    #[test]
    fn the_cluster_mass_and_potential_integrate_the_density() {
        let c = cluster();
        for r in [0.1, 3.0, 10.0, 30.0, 500.0] {
            let edges = [1e-9, 0.01, 1.0, 10.0, 100.0, r.max(100.0)];
            let edges: Vec<f64> = edges.into_iter().filter(|&e| e <= r).chain([r]).collect();
            let shell = |x: f64| 4.0 * core::f64::consts::PI * x * x * c.density(LightYears::new(x));
            let mass = gl_log_panels(shell, &edges);
            let closed = c.enclosed_mass(LightYears::new(r)).value();
            assert!((mass / closed - 1.0).abs() < 1e-8, "M({r}) {mass} against {closed}");
            // Φ(r) = −∫_r^∞ G M(<x) ÷ x² dx.
            let outer = [r, 10.0 * r, 1e3 * r, 1e6 * r, 1e12 * r];
            let force = |x: f64| G * c.enclosed_mass(LightYears::new(x)).value() / (x * x);
            let phi = -gl_log_panels(force, &outer);
            let closed = c.potential(LightYears::new(r));
            assert!((phi / closed - 1.0).abs() < 1e-6, "Φ({r}) {phi} against {closed}");
        }
        assert!(c.potential(LightYears::ZERO).is_finite());
        assert!((c.potential(LightYears::new(1e-12)) / c.potential(LightYears::ZERO) - 1.0).abs() < 1e-9);
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
    fn slopes_outside_their_ranges_are_rejected() {
        let make = |g1, g2| BrokenPowerLaw::new(SolarMasses::new(1.0), LightYears::new(1.0), g1, g2);
        assert!(make(2.0, 3.5).is_err());
        assert!(make(1.3, 3.0).is_err());
        assert!(make(-0.1, 3.5).is_err());
        assert!(make(0.0, 4.0).is_ok());
    }
}
