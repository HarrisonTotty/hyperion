//! The discs: the young disc, the old thin disc's sub-discs, the thick disc and the nuclear disc,
//! each exponential in radius and cored in height (brainstorm, "Fields" and "Populations"; plan
//! 02, P02.T7.a and Design note 9).

use super::arms::{Arm, ArmPoint};
use super::vertical::{Height, VerticalProfile, locate};
use super::{BuildFieldError, Site};
use crate::galaxy::PointLy;
use crate::math;
use crate::units::LightYears;

/// A disc exponential in radius and cored in height, `n0 exp(−R ÷ length) f(|z|)` systems per
/// cubic light-year, optionally times an arm factor, where `f` is the disc's
/// [`VerticalProfile`]: the vertical Jeans equation's solution, 1 in the plane.
///
/// It is normalised analytically over all space, `n0 = count ÷ (4π length² h)` with `h` the
/// profile's effective height `Σ ÷ 2ρ₀`: the tails beyond the root cube are lost, as the
/// brainstorm accepts (plan 02, Design note 11). The arm factor averages 1 around every circle, so
/// it moves no systems. The envelope, the density without the arm factor, never rises with |x|,
/// |y| or |z|, in floating point too ([`vertical`](super::vertical), "Floating point").
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::fields::{Fields, Shape};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// let fields = Fields::new(&params, &MassModel::new(&params));
/// let thick = &fields.components()[6];
/// let Shape::Disc(disc) = thick.shape() else { unreachable!() };
/// // The central density: count ÷ (4π L² h), with h the effective height.
/// let (l, h) = (disc.length().value(), disc.height().value());
/// let centre = thick.count() / (4.0 * std::f64::consts::PI * l * l * h);
/// assert!((disc.density(&PointLy::new(0.0, 0.0, 0.0)) / centre - 1.0).abs() < 1e-15);
/// // One scale length out in the plane it has fallen by e; above the plane, by the profile.
/// let there = disc.density(&PointLy::new(0.0, l, 0.0));
/// assert!((there / centre * hyperion_sim::math::exp(1.0) - 1.0).abs() < 1e-14);
/// let above = disc.density(&PointLy::new(0.0, l, -h));
/// assert!((above / there - disc.profile().value(h)).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExponentialDisc {
    n0: f64,
    length: LightYears,
    /// `1 ÷ length`, ly⁻¹.
    inv_length: f64,
    profile: VerticalProfile,
    arm: Option<Arm>,
}

impl ExponentialDisc {
    /// A disc of `count` systems over all space with this scale length and vertical profile,
    /// modulated by `arm`.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the count is negative or the scale length is not positive, or
    /// either is not finite.
    pub fn new(
        count: f64,
        length: LightYears,
        profile: VerticalProfile,
        arm: Option<Arm>,
    ) -> Result<Self, BuildFieldError> {
        BuildFieldError::check_non_negative("count", count)?;
        BuildFieldError::check_positive("scale length", length.value())?;
        let h = profile.effective_height().value();
        BuildFieldError::check_positive("effective height", h)?;
        let l = length.value();
        Ok(Self {
            n0: count / (4.0 * core::f64::consts::PI * l * l * h),
            length,
            inv_length: 1.0 / l,
            profile,
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

    /// The effective height `Σ ÷ 2ρ₀` of the vertical profile: the disc's height.
    #[must_use]
    pub fn height(&self) -> LightYears {
        self.profile.effective_height()
    }

    /// The vertical profile.
    #[must_use]
    pub fn profile(&self) -> &VerticalProfile {
        &self.profile
    }

    /// The arm factor, if the disc has arms.
    #[must_use]
    pub fn arm(&self) -> Option<&Arm> {
        self.arm.as_ref()
    }

    /// The number of systems over all space, `4π length² h n0`.
    #[must_use]
    pub fn count(&self) -> f64 {
        let (l, h) = (self.length.value(), self.height().value());
        4.0 * core::f64::consts::PI * l * l * h * self.n0
    }

    /// The envelope `n0 exp(−(R ÷ length + E(|z|)))` at cylindrical radius `r_cyl` and height
    /// `|z|` (ly): the density without the arm factor, systems per cubic light-year.
    #[must_use]
    pub fn envelope(&self, r_cyl: f64, abs_z: f64) -> f64 {
        self.envelope_at(r_cyl, locate(abs_z))
    }

    /// The envelope at cylindrical radius `r_cyl` (ly) and the located height `height`.
    pub(crate) fn envelope_at(&self, r_cyl: f64, height: Height) -> f64 {
        self.n0 * math::exp(-(r_cyl * self.inv_length + self.profile.exponent_at(height)))
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
        let envelope = self.envelope_at(site.r, site.height);
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
    use crate::galaxy::fields::vertical::{JeansIntegral, VerticalForce};
    use crate::galaxy::params::ArmCount;
    use crate::units::{KilometresPerSecond, Radians};

    /// A profile in a constant vertical force: exponential far from the plane.
    fn profile(sigma: f64) -> VerticalProfile {
        let force = VerticalForce::new(|z| 0.01 * math::tanh(z / 100.0));
        JeansIntegral::new(&force, 0.0, 0.0, LightYears::new(1.0))
            .profile(KilometresPerSecond::new(sigma))
    }

    #[test]
    fn the_count_round_trips_and_the_envelope_never_rises() {
        let disc =
            ExponentialDisc::new(3e10, LightYears::new(9_000.0), profile(20.0), None).unwrap();
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
        let disc = ExponentialDisc::new(
            1e10,
            LightYears::new(8_000.0),
            profile(10.0),
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
            ExponentialDisc::new(-1.0, l, profile(10.0), None)
                .unwrap_err()
                .quantity(),
            "count"
        );
        assert_eq!(
            ExponentialDisc::new(1.0, LightYears::new(0.0), profile(10.0), None)
                .unwrap_err()
                .quantity(),
            "scale length"
        );
    }
}
