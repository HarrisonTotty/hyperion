//! The discs: the young disc, the old thin disc's sub-discs, the thick disc and the nuclear disc,
//! each exponential in radius and cored in height (brainstorm, "Fields" and "Populations"; plan
//! 02, P02.T7.a and Design note 9).
//!
//! # The thin disc's central hole
//!
//! The young and old thin discs have a central hole: their surface density is `Σ ∝ exp(−R_h ÷ R −
//! R ÷ R_d)`, the form Dehnen and Binney (1998, MNRAS 294, 429, eq. 1) give their interstellar disc
//! and López-Corredoira et al. (2004, A&A 421, 953, eq. 19) fit to the stellar disc's near-plane
//! counts inside 8 kpc, and which plan 07's gas already takes (plan 02, ruling 32 of 2026-09-22 and
//! P02.T12.b). Without it the fixture held 2.48 × 10¹⁰ M☉ in Portail et al.'s (2017, MNRAS 465,
//! 1621) bulge box against their dynamical 1.85 ± 0.05 × 10¹⁰, and turned at 227 km/s at 2 kpc
//! against the published 180–200: the bulge's share of the stars in the box already counts the
//! inner disc's (Bland-Hawthorn and Gerhard 2016, ARA&A 54, 529, §4.2.4), and a hole-free disc adds
//! them again. The hole's scale is a fixed [`THIN_DISC_HOLE_LENGTHS`] of the thin disc's scale
//! length, so that one Gaussian expansion serves every galaxy's potential
//! ([`potential::mge`](crate::galaxy::potential::mge)). The thick and nuclear discs have none.
//!
//! The hole's factor `exp(−R_h ÷ R)` only rises with R, the exponential only falls, so a cell's
//! bound takes the first at the cell's largest radius and the second at its smallest, as the arm
//! bounds do with their factors ([`bounds`](crate::galaxy::bounds)).

use super::arms::{Arm, ArmPoint};
use super::vertical::{Height, VerticalProfile, locate};
use super::{BuildFieldError, Site};
use crate::galaxy::PointLy;
use crate::galaxy::quad::gl_log_panels;
use crate::math;
use crate::units::LightYears;

/// The thin discs' central hole `R_h` in thin-disc scale lengths: 1.18 kpc on the Milky Way
/// fixture's 2.15 kpc, near the Besançon model's measured hole scale of 1.32 ± 0.14 kpc (Robin et
/// al. 2003, A&A 409, 523, Table 3, in its own Einasto-type form; module documentation).
///
/// Ruling 32 of 2026-09-22 asked for 1.5–2 kpc, from Dehnen and Binney's (1998) form, but their
/// stellar discs have no hole (`R_m` = 0; only their interstellar disc's is 4 kpc). With the drawn
/// ranges held, a hole of 1.5 kpc or more cannot keep the fixture's inner rotation curve, its mass
/// inside 1 kpc, the local density and the youngest sub-disc inside their brackets together unless
/// the bulge and bar carry 38% of the stars, over the drawn 20–35% (plan 02, P02.T12.d and R25;
/// ruling 76.1). Other fits in other forms find larger holes: Freudenreich's (1998, ApJ 492, 495,
/// Table 4) 2.97 kpc, and López-Corredoira et al.'s (2004, A&A 421, 953, eq. 19) 3.74 ± 0.13 kpc in
/// this form on a 1.97 kpc disc.
pub const THIN_DISC_HOLE_LENGTHS: f64 = 0.55;

/// How a disc's surface density falls with radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadialProfile {
    /// `exp(−R ÷ R_d)`.
    Exponential,
    /// `exp(−R_h ÷ R − R ÷ R_d)`, with the hole's scale `R_h` in light-years (module
    /// documentation, "The thin disc's central hole").
    Holed {
        /// `R_h`, positive and finite.
        hole: LightYears,
    },
}

/// `∫₀^∞ s exp(−x ÷ s − s) ds`, the mass of a holed disc of hole `x` scale lengths over that of the
/// exponential disc with the same `exp(−R ÷ R_d)` factor, by `gl32` in `ln s` on fixed panels.
///
/// It is `2x K₂(2√x)` in the modified Bessel function of the second kind, which nothing else in the
/// sim needs; the panels hold the quadrature to 10⁻¹² of it for `x` from 0.05 to 5 (unit test),
/// the range it is valid for. It is what a holed disc's amplitude is divided by, so that the disc
/// holds its count.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::disc::{THIN_DISC_HOLE_LENGTHS, hole_mass_fraction};
///
/// // The fixture's thin discs hold two thirds of the systems an exponential disc of the same
/// // amplitude would: the hole moves them outwards, and the amplitude rises by the inverse.
/// let m = hole_mass_fraction(THIN_DISC_HOLE_LENGTHS);
/// assert!((0.6..0.7).contains(&m));
/// assert!(hole_mass_fraction(0.1) > m && m > hole_mass_fraction(2.0));
/// ```
#[must_use]
pub fn hole_mass_fraction(x: f64) -> f64 {
    const EDGES: [f64; 11] = [1e-4, 1e-3, 0.01, 0.03, 0.1, 0.3, 1.0, 3.0, 10.0, 30.0, 80.0];
    gl_log_panels(|s| s * math::exp(-(x / s + s)), &EDGES)
}

/// A disc's radial exponent at cylindrical radius `r` (ly): `r ÷ R_d`, plus `R_h ÷ r` for a holed
/// disc, with `inv_length` = `1 ÷ R_d` (ly⁻¹) and `hole_ly` = `R_h` (ly, 0 for none). At `r = 0` a
/// holed disc's is `+∞`, so its density is exactly 0 there.
#[must_use]
pub(crate) fn radial_exponent(r: f64, inv_length: f64, hole_ly: f64) -> f64 {
    // `R_h ÷ −0` would be −∞; every radius is a square root, never −0.
    debug_assert!(r.is_sign_positive(), "a radius of {r} ly");
    if hole_ly > 0.0 {
        r * inv_length + hole_ly / r
    } else {
        r * inv_length
    }
}

/// A disc exponential in radius and cored in height, `n0 exp(−R ÷ length) f(|z|)` systems per
/// cubic light-year, optionally with a central hole `exp(−R_h ÷ R)` and times an arm factor,
/// where `f` is the disc's [`VerticalProfile`]: the vertical Jeans equation's solution, 1 in the
/// plane.
///
/// It is normalised analytically over all space, `n0 = count ÷ (4π length² h m)` with `h` the
/// profile's effective height `Σ ÷ 2ρ₀` and `m` 1, or for a holed disc
/// [`hole_mass_fraction`]`(R_h ÷ length)`: the tails beyond the root cube are lost, as the
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
    /// `R_h`, ly; 0 for a disc without a hole (kept as a bare `f64` for the density's hot path).
    hole_ly: f64,
    /// The disc's mass over that of the hole-free disc with the same `n0`: 1 without a hole.
    mass_fraction: f64,
    profile: VerticalProfile,
    arm: Option<Arm>,
}

impl ExponentialDisc {
    /// A disc of `count` systems over all space with this scale length, radial profile and
    /// vertical profile, modulated by `arm`. A profile's effective height is positive and finite
    /// by construction.
    ///
    /// # Errors
    ///
    /// [`BuildFieldError`] if the count is negative, or the scale length or a hole's scale is not
    /// positive, or any of them is not finite.
    pub(crate) fn new(
        count: f64,
        length: LightYears,
        radial: RadialProfile,
        profile: VerticalProfile,
        arm: Option<Arm>,
    ) -> Result<Self, BuildFieldError> {
        BuildFieldError::check_non_negative("count", count)?;
        BuildFieldError::check_positive("scale length", length.value())?;
        let h = profile.effective_height().value();
        let l = length.value();
        let (hole_ly, mass_fraction) = match radial {
            RadialProfile::Exponential => (0.0, 1.0),
            RadialProfile::Holed { hole } => {
                BuildFieldError::check_positive("hole scale", hole.value())?;
                (hole.value(), hole_mass_fraction(hole.value() / l))
            }
        };
        Ok(Self {
            n0: count / (4.0 * core::f64::consts::PI * l * l * h * mass_fraction),
            length,
            inv_length: 1.0 / l,
            hole_ly,
            mass_fraction,
            profile,
            arm,
        })
    }

    /// The density's amplitude `n0`, systems per cubic light-year: the central density of a disc
    /// without a hole, and the density a holed disc's factor `exp(−R_h ÷ R)` multiplies.
    #[must_use]
    pub fn n0(&self) -> f64 {
        self.n0
    }

    /// The radial scale length.
    #[must_use]
    pub fn length(&self) -> LightYears {
        self.length
    }

    /// How the surface density falls with radius.
    #[must_use]
    pub fn radial(&self) -> RadialProfile {
        if self.hole_ly > 0.0 {
            RadialProfile::Holed {
                hole: LightYears::new(self.hole_ly),
            }
        } else {
            RadialProfile::Exponential
        }
    }

    /// `R_h` in light-years, 0 for a disc without a hole: what [`radial_exponent`] takes.
    #[must_use]
    pub(crate) fn hole_ly(&self) -> f64 {
        self.hole_ly
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

    /// The number of systems over all space, `4π length² h m n0`, with `m` the hole's
    /// [`hole_mass_fraction`] (1 without one).
    #[must_use]
    pub fn count(&self) -> f64 {
        let (l, h) = (self.length.value(), self.height().value());
        4.0 * core::f64::consts::PI * l * l * h * self.mass_fraction * self.n0
    }

    /// The envelope `n0 exp(−(R ÷ length [+ R_h ÷ R] + E(|z|)))` at cylindrical radius `r_cyl`
    /// and height `z` (ly, either side of the plane): the density without the arm factor, systems
    /// per cubic light-year.
    #[must_use]
    pub fn envelope(&self, r_cyl: f64, z: f64) -> f64 {
        self.envelope_at(r_cyl, locate(z))
    }

    /// The envelope at cylindrical radius `r_cyl` (ly) and the located height `height`.
    #[must_use]
    pub(crate) fn envelope_at(&self, r_cyl: f64, height: Height) -> f64 {
        let radial = radial_exponent(r_cyl, self.inv_length, self.hole_ly);
        self.n0 * math::exp(-(radial + self.profile.exponent_at(height)))
    }

    /// An upper bound on the envelope over radii `r_min` to `r_max` (ly) at the located height
    /// `height`: the exponential at `r_min` and the hole's factor at `r_max`, each of which only
    /// falls, or only rises, with radius, in floating point too (a correctly rounded division and
    /// sum are monotone in each operand). Without a hole it is [`envelope_at`](Self::envelope_at)
    /// at `r_min`, bit for bit.
    #[must_use]
    pub(crate) fn envelope_sup(&self, r_min: f64, r_max: f64, height: Height) -> f64 {
        if self.hole_ly > 0.0 {
            let radial = r_min * self.inv_length + self.hole_ly / r_max;
            self.n0 * math::exp(-(radial + self.profile.exponent_at(height)))
        } else {
            self.envelope_at(r_min, height)
        }
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
        JeansIntegral::new(&force, 0.0, LightYears::new(0.0), LightYears::new(1.0))
            .profile(KilometresPerSecond::new(sigma))
    }

    #[test]
    fn the_count_round_trips_and_the_envelope_never_rises() {
        let disc = ExponentialDisc::new(
            3e10,
            LightYears::new(9_000.0),
            RadialProfile::Exponential,
            profile(20.0),
            None,
        )
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
        let disc = ExponentialDisc::new(
            1e10,
            LightYears::new(8_000.0),
            RadialProfile::Exponential,
            profile(10.0),
            Some(Arm::Gentle(arm)),
        )
        .unwrap();
        for (x, y, z) in [(20_000.0, 3_000.0, 40.0), (-5.0, 17_000.0, -900.0)] {
            let p = PointLy::new(x, y, z);
            let r = (x * x + y * y).sqrt();
            let expected = disc.envelope(r, z) * arm.factor(&geometry.point(x, y));
            hyperion_testkit::float::assert_same_bits(disc.density(&p), expected);
        }
        assert!(matches!(disc.arm(), Some(Arm::Gentle(_))));
    }

    #[test]
    fn invalid_discs_are_rejected() {
        let l = LightYears::new(1.0);
        assert_eq!(
            ExponentialDisc::new(-1.0, l, RadialProfile::Exponential, profile(10.0), None)
                .unwrap_err()
                .quantity(),
            "count"
        );
        assert_eq!(
            ExponentialDisc::new(
                1.0,
                LightYears::new(0.0),
                RadialProfile::Exponential,
                profile(10.0),
                None
            )
            .unwrap_err()
            .quantity(),
            "scale length"
        );
        let no_hole = RadialProfile::Holed {
            hole: LightYears::new(0.0),
        };
        assert_eq!(
            ExponentialDisc::new(1.0, l, no_hole, profile(10.0), None)
                .unwrap_err()
                .quantity(),
            "hole scale"
        );
    }

    /// `K₂(z) = ∫₀^∞ exp(−z cosh t) cosh 2t dt`, by Simpson's rule on 20,000 steps to t = 12,
    /// knowing nothing of the scheme under test.
    fn bessel_k2(z: f64) -> f64 {
        let steps = 20_000_u32;
        let dt = 12.0 / f64::from(steps);
        let f = |t: f64| math::exp(-z * math::cosh(t)) * math::cosh(2.0 * t);
        let mut sum = f(0.0) + f(12.0);
        for i in 1..steps {
            sum += f(f64::from(i) * dt) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        sum * dt / 3.0
    }

    /// The hole's mass fraction is `2x K₂(2√x)`, and falls from 1 as the hole grows.
    #[test]
    fn the_hole_mass_fraction_is_the_bessel_closed_form() {
        for x in [0.05, 0.3, THIN_DISC_HOLE_LENGTHS, 2.0, 5.0] {
            let closed = 2.0 * x * bessel_k2(2.0 * x.sqrt());
            let ours = hole_mass_fraction(x);
            assert!(
                (ours / closed - 1.0).abs() < 1e-12,
                "x {x}: {ours} against {closed}"
            );
        }
        let m = hole_mass_fraction(THIN_DISC_HOLE_LENGTHS);
        assert!((0.6..0.7).contains(&m), "{m}");
        assert!(hole_mass_fraction(0.3) > m && m > hole_mass_fraction(2.0));
    }

    /// A holed disc keeps its count, is 0 at the centre, peaks at `√(R_h R_d)` in the plane, and
    /// its envelope's bound over a range of radii is never below the envelope inside it.
    #[test]
    fn a_holed_disc_counts_its_systems_and_bounds_its_envelope() {
        let (l, hole) = (7_000.0, THIN_DISC_HOLE_LENGTHS * 7_000.0);
        let disc = ExponentialDisc::new(
            3e10,
            LightYears::new(l),
            RadialProfile::Holed {
                hole: LightYears::new(hole),
            },
            profile(20.0),
            None,
        )
        .unwrap();
        assert!((disc.count() / 3e10 - 1.0).abs() < 1e-15);
        assert!(disc.envelope(0.0, 0.0).abs() < f64::MIN_POSITIVE);
        let peak = (hole * l).sqrt();
        assert!(disc.envelope(peak, 0.0) > disc.envelope(0.99 * peak, 0.0));
        assert!(disc.envelope(peak, 0.0) > disc.envelope(1.01 * peak, 0.0));
        assert_eq!(
            disc.radial(),
            RadialProfile::Holed {
                hole: LightYears::new(hole)
            }
        );
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x401e);
        for _ in 0..10_000 {
            let r_min = 60_000.0 * lcg.next_f64();
            let r_max = r_min + 2_000.0 * lcg.next_f64();
            let r = r_min + (r_max - r_min) * lcg.next_f64();
            let z = locate(3_000.0 * lcg.next_f64());
            assert!(disc.envelope_at(r, z) <= disc.envelope_sup(r_min, r_max, z));
        }
        // The edges: a range at the axis, a range of one radius, and the axis alone.
        let z = locate(40.0);
        assert!(disc.envelope_sup(0.0, 0.0, z).abs() < f64::MIN_POSITIVE);
        assert!(disc.envelope_sup(0.0, 3_000.0, z) >= disc.envelope_at(3_000.0, z));
        assert!(disc.envelope_sup(0.0, 3_000.0, z) >= disc.envelope_at(1.0, z));
        for r in [1.0, 4_000.0, 20_000.0] {
            assert!(disc.envelope_sup(r, r, z) >= disc.envelope_at(r, z));
            assert!(disc.envelope_sup(r, r, z) <= disc.envelope_at(r, z) * (1.0 + 1e-15));
        }
    }
}
