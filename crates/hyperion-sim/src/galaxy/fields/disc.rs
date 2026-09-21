//! The double-exponential discs: the young disc, the old thin disc's sub-discs, the thick disc
//! and the nuclear disc (brainstorm, "Populations"; plan 02, P02.T7.a).

use super::arms::{Arm, ArmPoint};
use super::{BuildFieldError, Site};
use crate::galaxy::PointLy;
use crate::math;
use crate::units::LightYears;

/// A double-exponential disc, `n0 exp(−R ÷ length) exp(−|z| ÷ height)` systems per cubic
/// light-year, optionally times an arm factor.
///
/// It is normalised analytically over all space, `n0 = count ÷ (4π length² height)`: the tails
/// beyond the root cube are lost, as the brainstorm accepts (plan 02, Design note 11). The arm
/// factor averages 1 around every circle, so it moves no systems. The envelope, the density
/// without the arm factor, never rises with |x|, |y| or |z|.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::fields::disc::DoubleExponential;
/// use hyperion_sim::units::LightYears;
///
/// let disc = DoubleExponential::new(1e10, LightYears::new(8_000.0), LightYears::new(300.0), None)?;
/// // The central density: count ÷ (4π L² h).
/// let centre = 1e10 / (4.0 * std::f64::consts::PI * 8_000.0 * 8_000.0 * 300.0);
/// assert!((disc.density(&PointLy::new(0.0, 0.0, 0.0)) / centre - 1.0).abs() < 1e-15);
/// // One scale length out and one scale height up it has fallen by e².
/// let there = disc.density(&PointLy::new(0.0, 8_000.0, -300.0));
/// assert!((there / centre * hyperion_sim::math::exp(2.0) - 1.0).abs() < 1e-14);
/// # Ok::<(), hyperion_sim::galaxy::fields::BuildFieldError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoubleExponential {
    n0: f64,
    length: LightYears,
    height: LightYears,
    /// `1 ÷ length`, ly⁻¹.
    inv_length: f64,
    /// `1 ÷ height`, ly⁻¹.
    inv_height: f64,
    arm: Option<Arm>,
}

impl DoubleExponential {
    /// A disc of `count` systems over all space with this scale length and height, modulated by
    /// `arm`.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the count is negative or either length is not positive, or any is
    /// not finite.
    pub fn new(
        count: f64,
        length: LightYears,
        height: LightYears,
        arm: Option<Arm>,
    ) -> Result<Self, BuildFieldError> {
        BuildFieldError::check_non_negative("count", count)?;
        BuildFieldError::check_positive("scale length", length.value())?;
        BuildFieldError::check_positive("scale height", height.value())?;
        let (l, h) = (length.value(), height.value());
        Ok(Self {
            n0: count / (4.0 * core::f64::consts::PI * l * l * h),
            length,
            height,
            inv_length: 1.0 / l,
            inv_height: 1.0 / h,
            arm,
        })
    }

    /// The central density `n0`, systems per cubic light-year.
    #[must_use]
    pub fn n0(&self) -> f64 {
        self.n0
    }

    /// The radial scale length.
    #[must_use]
    pub fn length(&self) -> LightYears {
        self.length
    }

    /// The vertical scale height.
    #[must_use]
    pub fn height(&self) -> LightYears {
        self.height
    }

    /// The arm factor, if the disc has arms.
    #[must_use]
    pub fn arm(&self) -> Option<&Arm> {
        self.arm.as_ref()
    }

    /// The number of systems over all space, `4π length² height n0`.
    #[must_use]
    pub fn count(&self) -> f64 {
        let (l, h) = (self.length.value(), self.height.value());
        4.0 * core::f64::consts::PI * l * l * h * self.n0
    }

    /// The envelope `n0 exp(−(R ÷ length + |z| ÷ height))` at cylindrical radius `r_cyl` and
    /// height `|z|` (ly): the density without the arm factor, systems per cubic light-year.
    #[must_use]
    pub fn envelope(&self, r_cyl: f64, abs_z: f64) -> f64 {
        self.n0 * math::exp(-(r_cyl * self.inv_length + abs_z * self.inv_height))
    }

    /// The density at `p`, arm factor included, systems per cubic light-year.
    #[must_use]
    pub fn density(&self, p: &PointLy) -> f64 {
        let site = Site::new(p);
        let arm = match &self.arm {
            Some(arm) => arm.geometry().point_with(site.x, site.y, site.r_sq, site.r),
            None => ArmPoint::CENTRE,
        };
        self.density_at(&site, &arm)
    }

    /// The density at `site`, whose arm quantities are `arm` (read only if the disc has arms).
    pub(crate) fn density_at(&self, site: &Site, arm: &ArmPoint) -> f64 {
        let envelope = self.envelope(site.r, site.abs_z);
        match &self.arm {
            Some(modulation) => envelope * modulation.factor(arm),
            None => envelope,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::fields::arms::{ArmGeometry, GentleArm};
    use crate::galaxy::params::ArmCount;
    use crate::units::Radians;

    #[test]
    fn the_count_round_trips_and_the_envelope_never_rises() {
        let disc =
            DoubleExponential::new(3e10, LightYears::new(9_000.0), LightYears::new(700.0), None)
                .unwrap();
        assert!((disc.count() / 3e10 - 1.0).abs() < 1e-15);
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x7a);
        for _ in 0..10_000 {
            let r = 80_000.0 * lcg.next_f64();
            let z = 10_000.0 * lcg.next_f64();
            let dr = 500.0 * lcg.next_f64();
            let dz = 100.0 * lcg.next_f64();
            assert!(disc.envelope(r + dr, z) <= disc.envelope(r, z));
            assert!(disc.envelope(r, z + dz) <= disc.envelope(r, z));
        }
    }

    /// With an arm, the density is the envelope times the arm's factor, bit for bit.
    #[test]
    fn the_density_is_the_envelope_times_the_arm() {
        let geometry =
            ArmGeometry::new(ArmCount::Four, Radians::new(0.2), LightYears::new(15_000.0)).unwrap();
        let arm = GentleArm::new(geometry, 0.2).unwrap();
        let disc = DoubleExponential::new(
            1e10,
            LightYears::new(8_000.0),
            LightYears::new(300.0),
            Some(Arm::Gentle(arm)),
        )
        .unwrap();
        for (x, y, z) in [(20_000.0, 3_000.0, 40.0), (-5.0, 17_000.0, -900.0)] {
            let p = PointLy::new(x, y, z);
            let r = (x * x + y * y).sqrt();
            let expected = disc.envelope(r, z.abs()) * arm.factor(&geometry.point(x, y));
            hyperion_testkit::float::assert_same_bits(disc.density(&p), expected);
        }
        assert!(matches!(disc.arm(), Some(Arm::Gentle(_))));
    }

    #[test]
    fn invalid_discs_are_rejected() {
        let l = LightYears::new(1.0);
        assert_eq!(
            DoubleExponential::new(-1.0, l, l, None)
                .unwrap_err()
                .quantity(),
            "count"
        );
        assert_eq!(
            DoubleExponential::new(1.0, l, LightYears::new(0.0), None)
                .unwrap_err()
                .quantity(),
            "scale height"
        );
    }
}
