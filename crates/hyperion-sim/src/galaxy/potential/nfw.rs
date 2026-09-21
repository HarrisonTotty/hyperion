//! The dark halo: a Navarro, Frenk and White (1997, ApJ 490, 493) profile in closed form (plan
//! 02, P02.T6.c).

use super::BuildComponentError;
use super::spherical::SphericalMass;
use crate::galaxy::consts::G;
use crate::galaxy::params::DarkHaloParams;
use crate::math;
use crate::units::{LightYears, SolarMasses};

/// Below this `x = r ÷ r_s`, [`mu`] sums its series instead of the closed form, which cancels.
const SERIES_BELOW: f64 = 0.01;

/// `μ(x) = ln(1 + x) − x ÷ (1 + x)`, the NFW mass inside `x` scale radii in units of `4π ρ_s
/// r_s³`.
///
/// Below `x = 0.01` it is the series `Σₙ₌₂ (−1)ⁿ (n − 1) xⁿ ÷ n`, whose terms fall by a factor of
/// at least a hundred, so nine of them reach 10⁻¹⁶ of the first.
fn mu(x: f64) -> f64 {
    if x < SERIES_BELOW {
        let mut power = x * x;
        let mut sum = 0.0;
        let mut n = 2.0;
        for _ in 0..9 {
            sum += power * (n - 1.0) / n;
            power *= -x;
            n += 1.0;
        }
        sum
    } else {
        math::ln_1p(x) - x / (1.0 + x)
    }
}

/// A Navarro–Frenk–White halo, `ρ = ρ_s ÷ ((r ÷ r_s)(1 + r ÷ r_s)²)`, described by its mass
/// `M₂₀₀` inside `r₂₀₀` and its concentration `c = r₂₀₀ ÷ r_s`.
///
/// `M(<r) = M₂₀₀ μ(x) ÷ μ(c)` and `Φ(r) = −G M₂₀₀ ÷ μ(c) × ln(1 + x) ÷ r` with `x = r ÷ r_s`: the
/// mass grows without limit, but the potential is finite everywhere and tends to 0 at infinity,
/// so the escape speed is finite, which a logarithmic halo's is not (brainstorm, "Galaxy
/// parameters").
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::potential::nfw::Nfw;
/// use hyperion_sim::galaxy::potential::spherical::SphericalMass;
/// use hyperion_sim::units::{LightYears, SolarMasses};
///
/// # fn main() -> Result<(), hyperion_sim::galaxy::potential::BuildComponentError> {
/// let halo = Nfw::new(SolarMasses::new(1.2e12), 8.2, LightYears::new(734_000.0))?;
/// let inside = halo.enclosed_mass(LightYears::new(734_000.0)).value();
/// assert!((inside / 1.2e12 - 1.0).abs() < 1e-12);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nfw {
    m200: SolarMasses,
    concentration: f64,
    r200: LightYears,
    /// `M₂₀₀ ÷ μ(c)`, M☉.
    scale_mass: f64,
}

impl Nfw {
    /// A halo of mass `m200` inside `r200` with concentration `concentration`.
    ///
    /// # Errors
    ///
    /// [`BuildComponentError`] if the mass is negative, or the concentration or `r200` is not
    /// positive, or any is not finite.
    pub fn new(
        m200: SolarMasses,
        concentration: f64,
        r200: LightYears,
    ) -> Result<Self, BuildComponentError> {
        BuildComponentError::check_non_negative("M200", m200.value())?;
        BuildComponentError::check_positive("concentration", concentration)?;
        BuildComponentError::check_positive("r200", r200.value())?;
        Ok(Self {
            m200,
            concentration,
            r200,
            scale_mass: m200.value() / mu(concentration),
        })
    }

    /// The dark halo of `params`.
    ///
    /// # Panics
    ///
    /// Never: a built [`DarkHaloParams`] has a positive mass, concentration and radius.
    #[must_use]
    pub fn from_params(params: &DarkHaloParams) -> Self {
        Self::new(params.m200(), params.concentration(), params.r200())
            .expect("the dark halo's parameters are valid")
    }

    /// The mass inside `r₂₀₀`.
    #[must_use]
    pub fn m200(&self) -> SolarMasses {
        self.m200
    }

    /// The concentration `r₂₀₀ ÷ r_s`.
    #[must_use]
    pub fn concentration(&self) -> f64 {
        self.concentration
    }

    /// `r₂₀₀`.
    #[must_use]
    pub fn r200(&self) -> LightYears {
        self.r200
    }

    /// The scale radius `r_s = r₂₀₀ ÷ c`.
    #[must_use]
    pub fn scale_radius(&self) -> LightYears {
        self.r200 / self.concentration
    }
}

impl SphericalMass for Nfw {
    fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        SolarMasses::new(self.scale_mass * mu(r / self.scale_radius()))
    }

    fn density(&self, r: LightYears) -> f64 {
        let rs = self.scale_radius().value();
        let x = r.value() / rs;
        let one_x = 1.0 + x;
        self.scale_mass / (4.0 * core::f64::consts::PI * rs * rs * rs * x * one_x * one_x)
    }

    fn potential(&self, r: LightYears) -> f64 {
        let rs = self.scale_radius().value();
        let x = r.value() / rs;
        // ln(1 + x) ÷ x tends to 1 at the centre.
        let shape = if x > 0.0 { math::ln_1p(x) / x } else { 1.0 };
        -G * self.scale_mass * shape / rs
    }

    fn v_circ_sq_slope(&self, r: LightYears) -> f64 {
        // G M₂₀₀ ÷ μ(c) × (x² ÷ (1 + x)² − μ(x)) ÷ r, which avoids 0 × ∞ in 4π r² ρ at small r.
        let x = r / self.scale_radius();
        let one_x = 1.0 + x;
        G * self.scale_mass * (x * x / (one_x * one_x) - mu(x)) / r.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::quad::gl_log_panels;

    fn halo() -> Nfw {
        Nfw::new(SolarMasses::new(1.19e12), 8.2, LightYears::new(734_229.0)).unwrap()
    }

    #[test]
    fn the_mass_inside_r200_is_m200() {
        let h = halo();
        assert!((h.enclosed_mass(h.r200()).value() / 1.19e12 - 1.0).abs() < 1e-14);
    }

    /// The series and the closed form meet at the switch, and the series is right far below it.
    #[test]
    fn mu_is_continuous_across_its_switch() {
        let below = mu(SERIES_BELOW.next_down());
        let above = mu(SERIES_BELOW);
        assert!(
            ((below - above) / above).abs() < 1e-13,
            "{below} against {above}"
        );
        let x: f64 = 1e-6;
        assert!((mu(x) / (0.5 * x * x) - 1.0).abs() < 2e-6);
    }

    /// Φ is finite at the centre and tends to 0 at infinity, and it is minus the integral of the
    /// force.
    #[test]
    fn the_potential_is_finite_and_vanishes_at_infinity() {
        let h = halo();
        let centre = h.potential(LightYears::ZERO);
        assert!(centre.is_finite() && centre < 0.0);
        assert!((h.potential(LightYears::new(1e-9)) / centre - 1.0).abs() < 1e-12);
        assert!(h.potential(LightYears::new(1e15)).abs() < 1e-4 * centre.abs());
        for r in [100.0, 26_000.0, 300_000.0] {
            let edges = [r, 1e2 * r, 1e4 * r, 1e8 * r, 1e16 * r];
            let force = |x: f64| h.v_circ_sq(LightYears::new(x)) / x;
            let phi = -gl_log_panels(force, &edges);
            let closed = h.potential(LightYears::new(r));
            // The tail beyond 10¹⁶ r, G M₂₀₀ ÷ μ(c) × ln(10¹⁶ x) ÷ 10¹⁶ r, is left out.
            assert!(
                (phi / closed - 1.0).abs() < 1e-11,
                "{r}: {phi} against {closed}"
            );
        }
    }

    /// The escape speed from the halo alone is finite: about 600 km/s at the centre of a
    /// Milky Way halo.
    #[test]
    fn the_escape_speed_is_finite() {
        let escape = (-2.0 * halo().potential(LightYears::ZERO)).sqrt();
        assert!((400.0..900.0).contains(&escape), "{escape} km/s");
    }

    #[test]
    fn the_slope_matches_a_finite_difference() {
        let h = halo();
        for r in [1.0, 1e3, 3e4, 1e6] {
            let step = 1e-4;
            let up = h.v_circ_sq(LightYears::new(r * math::exp(step)));
            let down = h.v_circ_sq(LightYears::new(r * math::exp(-step)));
            let numeric = (up - down) / (2.0 * step);
            let slope = h.v_circ_sq_slope(LightYears::new(r));
            assert!((numeric - slope).abs() < 1e-6 * slope.abs().max(1.0), "{r}");
        }
    }
}
