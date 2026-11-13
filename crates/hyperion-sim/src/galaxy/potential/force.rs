//! The potential's gradient off the plane: the force an orbit integrator steps with (plan 02's
//! potential, extended by plan 08 for plan 15's P15.T6.a; ruling 101.4 of 2026-09-22).
//!
//! Two sources, for two uses:
//!
//! - [`MassModel::force`] sums every component directly, the Gaussians by their one-dimensional
//!   quadratures and the spherical components in closed form. It is the reference, and costs what a
//!   mass-model point costs, about a millisecond.
//! - [`PotentialTables::force`] differentiates the (R, |z|) grid's bicubic Hermite interpolant of
//!   the Gaussians' potential analytically and adds the spherical components in closed form, so it
//!   is the exact gradient of what [`PotentialTables::potential`] returns. A field that is the
//!   gradient of one continuously differentiable potential is conservative, so a symplectic
//!   integrator holds its energy, [`PotentialTables::potential`] plus the kinetic energy, to the
//!   step's own accuracy: bilinear interpolation of tabulated forces would not be a gradient, and
//!   drifts (ruling 101.4). It costs about as much as one interpolation, a few hundred nanoseconds.
//!   That holds inside the grid's box. Beyond it, where R or |z| exceeds 2¹⁸ ly, the force is the
//!   exact gradient of the potential's continuation from the box's edge (plan 02, P02.T6.f). That
//!   continuation is continuous but has kinks, at the box's faces and 2⁻⁴ ly from the plane; they
//!   conserve energy, and a leapfrog pays a first-order error at each crossing.
//!
//! Both give `∂Φ ÷ ∂R` and `∂Φ ÷ ∂z` in (km/s)² per light-year, the acceleration's negative: at a
//! point above the plane and off the axis, both are positive, pulling inward and down. One exception
//! is the tables' continuation beyond 1.45 × 10⁶ ly in R, within 2⁻⁴ ly of the plane, which pushes
//! away from the plane, faintly (P02.T6.f).

use super::{MassModel, PotentialTables};
use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::math;
use crate::units::LightYears;

/// The potential's gradient at a point, in cylindrical components: `∂Φ ÷ ∂R` and `∂Φ ÷ ∂z`,
/// (km/s)² per light-year. The acceleration is its negative.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::potential::CylindricalForce;
///
/// let f = CylindricalForce { radial: 2.0, vertical: -1.0 };
/// // In light-years per year squared, the acceleration points inward and up.
/// let [ar, az] = f.acceleration_ly_per_yr2();
/// assert!(ar < 0.0 && az > 0.0);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CylindricalForce {
    /// `∂Φ ÷ ∂R`, (km/s)² per light-year: positive where gravity pulls towards the axis.
    pub radial: f64,
    /// `∂Φ ÷ ∂z`, (km/s)² per light-year: positive above the plane, odd in z.
    pub vertical: f64,
}

impl CylindricalForce {
    /// The acceleration `−∇Φ` in light-years per year squared, `[a_R, a_z]`: (km/s)² per light-year
    /// times the square of light-years per year per km/s.
    #[must_use]
    pub fn acceleration_ly_per_yr2(&self) -> [f64; 2] {
        let k = LIGHT_YEARS_PER_YEAR_PER_KM_S * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        [-self.radial * k, -self.vertical * k]
    }
}

impl MassModel {
    /// The potential's gradient at `(R, z)`, summed directly over every component (module
    /// documentation): the Gaussians' `R ∂Φ ÷ ∂R` from their quadratures divided by R, and their
    /// `K_z` as [`MassModel::vertical_force`] gives it; the spherical components' `G M(<r) ÷ r²`
    /// resolved along R and z. On the axis the radial part is zero; at the centre itself both are.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::galaxy::potential::MassModel;
    /// use hyperion_sim::units::LightYears;
    ///
    /// let model = MassModel::new(&GalaxyParams::milky_way_like());
    /// let (r, z) = (LightYears::new(26_000.0), LightYears::new(0.0));
    /// // In the plane, R ∂Φ ÷ ∂R is the circular speed squared.
    /// let f = model.force(r, z);
    /// assert!((f.radial * r.value() / model.v_circ_sq(r) - 1.0).abs() < 1e-12);
    /// assert!(f.vertical.abs() < f64::EPSILON);
    /// ```
    #[must_use]
    pub fn force(&self, r_cyl: LightYears, z: LightYears) -> CylindricalForce {
        let (r, height) = (r_cyl.value().abs(), z.value());
        let radial = if r > 0.0 {
            self.extended_grid_point(r, height)[1] / r
        } else {
            0.0
        };
        let vertical = self.vertical_force(LightYears::new(r), z);
        let h = math::hypot(r, height);
        let spherical = if h > 0.0 {
            let radius = LightYears::new(h);
            self.spherical()
                .iter()
                .fold(0.0, |sum, c| sum + c.v_circ_sq(radius))
                * r
                / (h * h)
        } else {
            0.0
        };
        // `vertical_force` already holds the spherical components' `K_z`.
        CylindricalForce {
            radial: radial + spherical,
            vertical,
        }
    }
}

impl PotentialTables {
    /// The potential's gradient at `(R, z)` from the tables, the exact derivative of
    /// [`potential`](Self::potential) (module documentation); `None` without the (R, z) grid
    /// ([`full`](Self::full)). Below 2⁻⁴ ly in R the Gaussians' radial part is continued as
    /// solid-body, as in the plane, so it falls to zero on the axis. Beyond the grid's box, where R
    /// or |z| exceeds 2¹⁸ ly, it is the exact gradient of the potential's continuation there too
    /// (plan 02, P02.T6.f); it jumps where a path crosses one of the box's faces, at a kink of the
    /// continuous potential.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
    /// use hyperion_sim::units::LightYears;
    ///
    /// let model = MassModel::new(&GalaxyParams::milky_way_like());
    /// let tables = PotentialTables::full(&model);
    /// let (r, z) = (LightYears::new(26_000.0), LightYears::new(1_000.0));
    /// let (fast, exact) = (tables.force(r, z).expect("the full tables"), model.force(r, z));
    /// assert!((fast.radial / exact.radial - 1.0).abs() < 5e-3);
    /// assert!((fast.vertical / exact.vertical - 1.0).abs() < 5e-3);
    /// assert!(PotentialTables::in_plane(&model).force(r, z).is_none());
    /// ```
    #[must_use]
    pub fn force(&self, r_cyl: LightYears, z: LightYears) -> Option<CylindricalForce> {
        let r = r_cyl.value().abs();
        let [by_ln_r, vertical] = self.forces(r, z.value())?;
        Some(CylindricalForce {
            radial: if r > 0.0 { by_ln_r / r } else { 0.0 },
            vertical,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::params::GalaxyParams;

    // The tables' force against their own potential's gradient is asserted in `tables`' tests,
    // on the one build of the full tables that the grid's other test reads.

    /// The direct force is the gradient of the direct potential, by central differences, across
    /// the disc, the bulge, the halo and far above the plane.
    #[test]
    fn the_models_force_is_the_gradient_of_its_potential() {
        let model = MassModel::new(&GalaxyParams::milky_way_like());
        for (r, z) in [
            (26_000.0, 300.0),
            (26_000.0, -1_500.0),
            (3_000.0, 400.0),
            (500.0, 60.0),
            (40_000.0, 20_000.0),
            (8_000.0, 30_000.0),
        ] {
            let f = model.force(LightYears::new(r), LightYears::new(z));
            let phi = |a: f64, b: f64| model.potential(LightYears::new(a), LightYears::new(b));
            let d = 1e-3 * r.min(z.abs());
            let by_r = (phi(r + d, z) - phi(r - d, z)) / (2.0 * d);
            let by_z = (phi(r, z + d) - phi(r, z - d)) / (2.0 * d);
            assert!(
                (f.radial - by_r).abs() < 1e-4 * by_r.abs(),
                "∂Φ/∂R at ({r}, {z}): {} against {by_r}",
                f.radial
            );
            assert!(
                (f.vertical - by_z).abs() < 1e-4 * by_z.abs(),
                "∂Φ/∂z at ({r}, {z}): {} against {by_z}",
                f.vertical
            );
        }
    }
}
