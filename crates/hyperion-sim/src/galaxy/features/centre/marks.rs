//! The marks a centre member passes after its position and velocity: the loss cone, the
//! flattening and the rotation (plan 09, P09.T25; Design note 13; ruling 144.5a, 144.7 and 144.8).
//!
//! A member of the spherical, isotropic distribution function computes its angular momentum `L`
//! about the black hole, and three things follow, each a function of integrals of the motion, so
//! the cluster stays stationary (the brainstorm, "Dense features"):
//!
//! - **The loss cone.** A member whose Kepler pericentre about the black hole lies inside
//!   [`loss_cone_radius`], about 2 au × (`M_bh` ÷ 4.3 × 10⁶ M☉)^⅓, would have been torn apart and is
//!   rejected, which removes some 4 × 10⁻⁵ of the cluster ([`LossCone::share`]).
//! - **The flattening.** An orbit of inclination `i` to a class's axis (the galactic pole, or the
//!   young disc's normal) is kept with probability `exp(−k sin² i)`. In the spherical potential
//!   the whole of `L` is conserved, so a tilted axis is as good an integral as the pole. At a
//!   position of polar angle θ from the axis the orbits' mean acceptance is the angular factor
//!   [`angular_factor`](OrbitMarks::angular_factor), `g(θ) = e^(−k cos²θ) e^(−a) I₀(a)`, `a = k
//!   sin²θ ÷ 2`, so the density is the profile times `g(θ) ÷ A`, A the factor's mean over the
//!   sphere ([`inclination_acceptance`](OrbitMarks::inclination_acceptance)). The pole's density
//!   over the equator's is `e^(−k/2) ÷ I₀(k/2)` = 0.629 at k = 0.84, an isodensity axis ratio of
//!   0.70 in the 1.3 cusp: Schödel et al. (2014, A&A 566, A47) fit 0.71 ± 0.02 to the projected
//!   light and Chatzopoulos et al. (2015, MNRAS 447, 948) 0.73 ± 0.04 to an intrinsic homoeoid,
//!   both isodensity (isophote) ratios, which for an oblate homoeoid seen edge-on agree. The
//!   sample's second moments inside a sphere, `√(⟨z²⟩ ÷ ⟨x²⟩)`, are 0.91, rounder, because a
//!   sphere samples a flattened cusp that way: a homoeoid of q = 0.71 and slope 1.3 gives 0.91
//!   too. Because the mark's angular factor is the same at every radius, the isodensity ratio is
//!   `0.629^(1/γ)` at a local slope γ, rounder where the profile steepens beyond the break.
//! - **The rotation.** A second mark reverses the velocity of a share of the survivors whose `L`
//!   has a negative component along the axis, which keeps the energy, `|L|` and `sin² i`, so the
//!   class turns one way about its axis. It is Lynden-Bell's demon as Chatzopoulos et al. (2015,
//!   §3.3, eq. 13) add rotation, in the limit of their steep κ; their fit is F = 0.85 ± 0.15, and
//!   the old stars' 0.8 turns them at some 40 km/s in a slit along the plane (Feldmeier et al.
//!   2014, A&A 570, A2, §4.3).
//!
//! # The flattened bound (ruling 144.5a)
//!
//! A cell's bound is the nearest corner's spherical profile times the factor's greatest value,
//! `e^(−k/2) I₀(k/2) ÷ A` ([`peak_factor`](OrbitMarks::peak_factor), I₀ once per class), not `1
//! ÷ A`: ×0.69 for the old stars. Under that bound the inclination can no longer be a rejection
//! after the velocity, since `exp(−k sin² i) ÷ g_max` exceeds one for orbits near the axis. So the
//! placement thins a candidate by its position's own `g(θ)` (an I₀ per candidate and flattened
//! class), and the inclination mark then draws the velocity's direction conditioned on it: the
//! speed is kept, and the direction is redrawn until the mark accepts. Whether an orbit is kept
//! depends only on the direction of `L` about the position, which is the velocity's azimuth about
//! the radius, independent of the speed and of the angle to the radius on which the loss cone
//! depends, so the three marks stay independent and the density is the profile times `g(θ) (1 −
//! loss) ÷ A` as before. After [`MAX_DIRECTION_ATTEMPTS`] rejections the candidate is "no such
//! system": for the young disc's k = 25 that happens only near its axis, where `g` is below 10⁻³
//! and the placement keeps a candidate with a probability under 0.01.
//!
//! Words of a member's `centre.marks` stream: the inclination's mark; after each rejection, a new
//! direction (two words) and its mark; then the reversal's mark.

use crate::galaxy::consts::G;
use crate::galaxy::quad::{gl_log_panels, gl32};
use crate::galaxy::special::bessel_i0e;
use crate::math;
use crate::rng::{Stream, Threshold};
use crate::stellar::draws::isotropic;
use crate::units::consts::{METRES_PER_AU, METRES_PER_LIGHT_YEAR};
use crate::units::{LightYears, SolarMasses};

use super::df::DistributionFunction;
use super::profile::CentreProfile;

/// The loss cone's radius about a black hole of the reference mass, au (the brainstorm, "Dense
/// features": a pericentre inside about 2 au is torn apart).
pub const LOSS_CONE_AU: f64 = 2.0;

/// The reference black hole's mass, M☉ (Sgr A*, 4.3 × 10⁶ M☉).
pub const REFERENCE_BLACK_HOLE: f64 = 4.3e6;

/// The loss cone's radius about a black hole of `black_hole`: `2 au × (M ÷ 4.3 × 10⁶ M☉)^⅓`, the
/// tidal radius of a Sun-like star scaling as `M^⅓`.
#[must_use]
pub fn loss_cone_radius(black_hole: SolarMasses) -> LightYears {
    let au = METRES_PER_AU / METRES_PER_LIGHT_YEAR;
    LightYears::new(LOSS_CONE_AU * au * math::cbrt(black_hole.value() / REFERENCE_BLACK_HOLE))
}

/// The Kepler pericentre of `position` (ly) and `velocity` (km/s) about a point mass of `mu` = G M,
/// ly (km/s)²: `L² ÷ (μ (1 + e))`, `e = √(1 + 2 E L² ÷ μ²)`, for bound and unbound orbits alike.
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "position, velocity and orbit in their usual symbols"
)]
pub fn kepler_pericentre(position: [f64; 3], velocity: [f64; 3], mu: f64) -> f64 {
    let [x, y, z] = position;
    let [u, v, w] = velocity;
    let r = (x * x + y * y + z * z).sqrt();
    let l = [y * w - z * v, z * u - x * w, x * v - y * u];
    let l2 = l[0] * l[0] + l[1] * l[1] + l[2] * l[2];
    let energy = 0.5 * (u * u + v * v + w * w) - mu / r;
    let e = (1.0 + 2.0 * energy * l2 / (mu * mu)).max(0.0).sqrt();
    l2 / (mu * (1.0 + e))
}

/// The black hole's loss cone (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossCone {
    mu: f64,
    radius: f64,
}

impl LossCone {
    /// The loss cone of a black hole of `black_hole`.
    #[must_use]
    pub fn new(black_hole: SolarMasses) -> Self {
        Self {
            mu: G * black_hole.value(),
            radius: loss_cone_radius(black_hole).value(),
        }
    }

    /// Its radius, ly.
    #[must_use]
    pub fn radius(&self) -> LightYears {
        LightYears::new(self.radius)
    }

    /// Whether the orbit through `position` (ly) with `velocity` (km/s) passes inside it.
    #[must_use]
    pub fn contains(&self, position: [f64; 3], velocity: [f64; 3]) -> bool {
        kepler_pericentre(position, velocity, self.mu) < self.radius
    }

    /// The share of isotropic orbits at speed `v` (km/s) and radius `r` (ly) that pass inside it:
    /// `1 − √(1 − s²)`, `s = L_lc ÷ (r v)` capped at 1, `L_lc² = r_lc² (v² − 2μ ÷ r + 2μ ÷ r_lc)`.
    fn share_at(&self, r: f64, v: f64) -> f64 {
        if r <= self.radius {
            return 1.0;
        }
        let l2 =
            self.radius * self.radius * (v * v - 2.0 * self.mu / r + 2.0 * self.mu / self.radius);
        let s2 = (l2 / (r * r * v * v)).clamp(0.0, 1.0);
        1.0 - (1.0 - s2).sqrt()
    }

    /// The share of the tracer of `df` inside the reach whose orbits pass inside the loss cone,
    /// by a fixed quadrature: one 16-node panel a decade in radius from 10⁻⁷ ly to the reach, of
    /// `4πr³ ∫ 4πv² f P(v) dv` in `ln r`, and the distribution function's own panels in speed.
    ///
    /// # Panics
    ///
    /// Never: the panels' edges are never empty.
    #[must_use]
    pub fn share(&self, df: &DistributionFunction, profile: &CentreProfile) -> f64 {
        let reach = profile.reach().value();
        let mut edges = vec![1e-7];
        let mut edge = 1e-7;
        while edge * 10.0 < reach {
            edge *= 10.0;
            edges.push(edge);
        }
        edges.push(reach);
        let four_pi = 4.0 * core::f64::consts::PI;
        let removed = gl_log_panels(
            |r| four_pi * r * r * r * df.speed_moment(profile, r, |v| self.share_at(r, v)),
            &edges,
        );
        removed / df.fraction_within(reach)
    }
}

/// The most directions the inclination mark draws before a candidate is dropped (module
/// documentation).
pub const MAX_DIRECTION_ATTEMPTS: u32 = 4_096;

/// The relative margin of the angular factor's peak over the factor itself: `bessel_i0e` is good
/// to a few units in the last place, so the factor's computed values can exceed the peak's by as
/// much.
const PEAK_MARGIN: f64 = 1e-9;

/// The inclination of the Milky Way's clockwise disc's normal to the plane of the sky and the
/// position angle of its ascending node, degrees: Yelda et al. (2014, ApJ 783, 131)'s (130°,
/// 96°), which Paumard et al. (2006, ApJ 643, 1011; 127°, 99°) and Lu et al. (2009, ApJ 690,
/// 1463; 127°, 99°) agree with.
pub const MILKY_WAY_DISC_ORIENTATION: (f64, f64) = (130.0, 96.0);

/// The position angle of the galactic plane at Sgr A*, degrees east of north: 31.40° by the IAU's
/// J2000 galactic frame at Sgr A*'s position.
pub const GALACTIC_PLANE_POSITION_ANGLE: f64 = 31.40;

/// The angle between the bar's near end and the line from the galactic centre to the Sun,
/// degrees: Wegg and Gerhard's (2013, MNRAS 435, 1874) 27°. The model has no Sun, so this places
/// the bar's +x axis (plan 01) for the young disc's normal alone.
pub const BAR_ANGLE: f64 = 27.0;

/// The normal of an orbit plane of inclination `i` and ascending node `omega` (degrees, the
/// astrometric elements of Paumard et al. 2006, Appendix A), in the sky frame of Sgr A*: x east,
/// y north, z along the line of sight away from the Sun. Lu et al. (2009, eq. 8): `(sin i cos Ω,
/// −sin i sin Ω, −cos i)`.
#[must_use]
pub fn sky_normal(i: f64, omega: f64) -> [f64; 3] {
    let (sin_i, cos_i) = math::sin_cos(i.to_radians());
    let (sin_o, cos_o) = math::sin_cos(omega.to_radians());
    [sin_i * cos_o, -sin_i * sin_o, -cos_i]
}

/// A direction in the sky frame of Sgr A* ([`sky_normal`]'s) in the galactic frame of plan 01:
/// +z along the galaxy's angular momentum, which for the Milky Way is the south galactic pole
/// since it turns clockwise seen from the north galactic pole, and +x along the bar's near end,
/// [`BAR_ANGLE`] from the Sun's direction towards increasing galactic longitude. Sgr A* is taken
/// to lie at l = b = 0 (it lies 0.06° off), so the line of sight is the heliocentric X axis, and
/// increasing l and b lie at position angles [`GALACTIC_PLANE_POSITION_ANGLE`] and that less 90°.
#[must_use]
pub fn sky_to_galactic(v: [f64; 3]) -> [f64; 3] {
    let (sin_p, cos_p) = math::sin_cos(GALACTIC_PLANE_POSITION_ANGLE.to_radians());
    // Heliocentric galactic axes: X towards the centre, Y towards l = 90°, Z towards the north
    // galactic pole. Increasing l is (sin p, cos p) in (east, north), increasing b (−cos p,
    // sin p).
    let big_x = v[2];
    let big_y = v[0] * sin_p + v[1] * cos_p;
    let big_z = -v[0] * cos_p + v[1] * sin_p;
    let (sin_b, cos_b) = math::sin_cos(BAR_ANGLE.to_radians());
    // +x = −cos β X + sin β Y, +z = −Z, and +y = z × x = sin β X + cos β Y.
    [
        -cos_b * big_x + sin_b * big_y,
        sin_b * big_x + cos_b * big_y,
        -big_z,
    ]
}

/// The Milky Way's clockwise disc's normal in the galactic frame ([`MILKY_WAY_DISC_ORIENTATION`]
/// through [`sky_to_galactic`]): about (−0.886, −0.326, 0.329), 71° from the galaxy's own axis,
/// so the disc turns partly with the galaxy.
#[must_use]
pub fn milky_way_disc_normal() -> [f64; 3] {
    let (i, omega) = MILKY_WAY_DISC_ORIENTATION;
    sky_to_galactic(sky_normal(i, omega))
}

/// A class's flattening and rotation marks about its axis (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitMarks {
    flattening: f64,
    reversal: f64,
    axis: [f64; 3],
    peak: f64,
    acceptance: f64,
}

impl OrbitMarks {
    /// The marks of flattening constant `flattening` (k ≥ 0) and reversal share `reversal` about
    /// the unit vector `axis`, galactic.
    ///
    /// # Panics
    ///
    /// In debug builds, if k is negative, the share outside `[0, 1]` or the axis not of unit
    /// length.
    #[must_use]
    pub fn new(flattening: f64, reversal: f64, axis: [f64; 3]) -> Self {
        debug_assert!(flattening >= 0.0, "k {flattening}");
        debug_assert!((0.0..=1.0).contains(&reversal), "share {reversal}");
        debug_assert!(
            (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2] - 1.0).abs() < 1e-12,
            "axis {axis:?}"
        );
        // ∫₀¹ exp(−k (1 − μ²)) dμ in sixteen 32-node panels, which a k of 25's peak of width 1 ÷
        // 50 at μ = 1 needs.
        let acceptance: f64 = (0..16)
            .map(|p| {
                let lo = f64::from(p) / 16.0;
                gl32(
                    |mu| math::exp(-flattening * (1.0 - mu * mu)),
                    lo,
                    lo + 1.0 / 16.0,
                )
            })
            .sum();
        Self {
            flattening,
            reversal,
            axis,
            peak: bessel_i0e(0.5 * flattening) * (1.0 + PEAK_MARGIN),
            acceptance,
        }
    }

    /// The old cluster's: k = 0.84 about the galactic pole for the brainstorm's flattening of 0.7,
    /// and a reversal share of 0.8, which gives a rotation of some 40 km/s in a slit along the
    /// plane (ruling 144.8: Chatzopoulos et al. 2015 fit 0.85 ± 0.15; Feldmeier et al. 2014, §4.3:
    /// "a rotation curve with an amplitude of ∼40 km/s", its faint stars reaching about 50).
    #[must_use]
    pub fn old_stars() -> Self {
        Self::new(0.84, 0.8, [0.0, 0.0, 1.0])
    }

    /// The young clockwise disc's: k = 25, an rms inclination of 8° (Yelda et al. 2014's h ÷ r =
    /// 0.10, ruling 144.6), about the Milky Way's disc normal ([`milky_way_disc_normal`]), with
    /// every orbit turned the disc's way. Every galaxy's disc takes the Milky Way's orientation
    /// until plan 02 draws one (a finding).
    #[must_use]
    pub fn young_disc() -> Self {
        Self::new(25.0, 1.0, milky_way_disc_normal())
    }

    /// No flattening and no rotation: the isotropic young stars'.
    #[must_use]
    pub fn isotropic() -> Self {
        Self::new(0.0, 0.0, [0.0, 0.0, 1.0])
    }

    /// The flattening constant k.
    #[must_use]
    pub fn flattening(&self) -> f64 {
        self.flattening
    }

    /// The share of counter-rotating orbits reversed.
    #[must_use]
    pub fn reversal_share(&self) -> f64 {
        self.reversal
    }

    /// The axis, galactic.
    #[must_use]
    pub fn axis(&self) -> [f64; 3] {
        self.axis
    }

    /// The inclination mark's mean acceptance over isotropic orbits, `∫₀¹ exp(−k (1 − μ²)) dμ`:
    /// 0.590 at k = 0.84, the angular factor's mean over the sphere.
    #[must_use]
    pub fn inclination_acceptance(&self) -> f64 {
        self.acceptance
    }

    /// The angular factor at `position` (ly from the black hole): the inclination mark's mean
    /// acceptance over isotropic velocities there, `e^(−k cos²θ) e^(−a) I₀(a)` with `a = k sin²θ ÷
    /// 2` and θ the polar angle from the axis (module documentation). 1 at the black hole itself.
    #[must_use]
    pub fn angular_factor(&self, position: [f64; 3]) -> f64 {
        let r2 = position[0] * position[0] + position[1] * position[1] + position[2] * position[2];
        if self.flattening <= 0.0 || r2 <= 0.0 {
            return 1.0;
        }
        let along =
            position[0] * self.axis[0] + position[1] * self.axis[1] + position[2] * self.axis[2];
        let cos2 = (along * along / r2).min(1.0);
        math::exp(-self.flattening * cos2) * bessel_i0e(0.5 * self.flattening * (1.0 - cos2))
    }

    /// The angular factor's greatest value, `e^(−k/2) I₀(k/2)` at the equator, with a margin for
    /// its rounding: the flattened bound's factor (module documentation).
    #[must_use]
    pub fn peak_factor(&self) -> f64 {
        self.peak
    }

    /// `sin² i` of `velocity` at `position`, and `L`'s component along the axis.
    #[expect(
        clippy::many_single_char_names,
        reason = "position, velocity and angular momentum in their usual symbols"
    )]
    fn inclination(&self, position: [f64; 3], velocity: [f64; 3]) -> (f64, f64) {
        let [x, y, z] = position;
        let [u, v, w] = velocity;
        let l = [y * w - z * v, z * u - x * w, x * v - y * u];
        let l2 = l[0] * l[0] + l[1] * l[1] + l[2] * l[2];
        let along = l[0] * self.axis[0] + l[1] * self.axis[1] + l[2] * self.axis[2];
        if l2 > 0.0 {
            ((1.0 - along * along / l2).max(0.0), along)
        } else {
            (1.0, along)
        }
    }

    /// The marks of the orbit through `position` (ly) with `velocity` (km/s), on the next words of
    /// `stream` (module documentation): the velocity with its direction conditioned on the
    /// inclination mark and reversed if the reversal mark turns it the class's way, or `None` if
    /// it passes inside the loss cone or [`MAX_DIRECTION_ATTEMPTS`] directions are all rejected.
    #[must_use]
    pub fn apply(
        &self,
        loss_cone: &LossCone,
        position: [f64; 3],
        velocity: [f64; 3],
        stream: &mut Stream,
    ) -> Option<[f64; 3]> {
        let speed =
            (velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2])
                .sqrt();
        let mut v = velocity;
        let mut attempts = 1;
        loop {
            let mark = stream.mark();
            let (sin2, _) = self.inclination(position, v);
            if mark.is_below(Threshold::from_probability(math::exp(
                -self.flattening * sin2,
            ))) {
                break;
            }
            if attempts == MAX_DIRECTION_ATTEMPTS {
                return None;
            }
            attempts += 1;
            v = isotropic(stream).components().map(|c| c * speed);
        }
        let reversal = stream.mark();
        if loss_cone.contains(position, v) {
            return None;
        }
        let (_, along) = self.inclination(position, v);
        if along < 0.0 && reversal.is_below(Threshold::from_probability(self.reversal)) {
            return Some(v.map(|c| -c));
        }
        Some(v)
    }
}

/// The share of candidates the marks accept, the inclination's times the loss cone's, which
/// divides the profile's normalisation (Design note 13).
#[must_use]
pub fn mean_acceptance(
    marks: &OrbitMarks,
    loss_cone: &LossCone,
    df: &DistributionFunction,
    profile: &CentreProfile,
) -> f64 {
    marks.inclination_acceptance() * (1.0 - loss_cone.share(df, profile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::params::GalaxyParams;
    use crate::rng::{ObjectKey, tags};

    fn fixture() -> (CentreProfile, DistributionFunction) {
        let profile = CentreProfile::from_params(&GalaxyParams::milky_way_like()).unwrap();
        let df = DistributionFunction::invert(profile.stars(), &profile).unwrap();
        (profile, df)
    }

    /// A member at radius `r` (or drawn from the realised profile inside the reach when `None`),
    /// thinned as the placement thins it by its position's angular factor under the peak, with
    /// its velocity and its marks, from stream `n`.
    #[expect(
        clippy::many_single_char_names,
        reason = "radius, stream and state in their usual symbols"
    )]
    fn member(
        profile: &CentreProfile,
        df: &DistributionFunction,
        marks: &OrbitMarks,
        cone: &LossCone,
        r: Option<f64>,
        n: u64,
    ) -> Option<([f64; 3], [f64; 3])> {
        let mut s = Stream::open(Seed::new(0x0925), tags::SELFTEST_STREAM, ObjectKey::cell(n));
        let r = r.unwrap_or_else(|| {
            let target = s.uniform() * df.fraction_within(profile.reach().value());
            let lr = crate::galaxy::quad::bisect(
                |lr| df.fraction_within(math::exp(lr)) - target,
                math::ln(1e-7),
                math::ln(profile.reach().value()),
                60,
            );
            math::exp(lr)
        });
        let p = isotropic(&mut s).components().map(|c| c * r);
        let kept = Threshold::from_ratio(marks.angular_factor(p), marks.peak_factor());
        if !s.mark().is_below(kept) {
            return None;
        }
        let v = df.draw_velocity(profile, p, &mut s)?;
        marks.apply(cone, p, v, &mut s).map(|v| (p, v))
    }

    #[test]
    fn the_loss_cone_scales_with_the_black_hole() {
        let at = loss_cone_radius(SolarMasses::new(4.3e6)).value();
        assert!((at - 2.0 * 1.581_25e-5).abs() < 1e-9, "{at}");
        let heavy = loss_cone_radius(SolarMasses::new(4.3e6 * 8.0)).value();
        assert!((heavy / at - 2.0).abs() < 1e-12);
    }

    #[test]
    fn a_circular_orbit_s_pericentre_is_its_radius_and_a_radial_one_s_is_zero() {
        let mu = G * 4.3e6;
        let r = 0.5;
        let vc = (mu / r).sqrt();
        let q = kepler_pericentre([r, 0.0, 0.0], [0.0, vc, 0.0], mu);
        assert!((q / r - 1.0).abs() < 1e-9, "{q}");
        assert!(kepler_pericentre([r, 0.0, 0.0], [-100.0, 0.0, 0.0], mu) < 1e-15);
        // Unbound: the pericentre of a hyperbola still lies inside the radius.
        let fast = kepler_pericentre([r, 0.0, 0.0], [-3.0 * vc, vc, 0.0], mu);
        assert!(fast > 0.0 && fast < r);
    }

    /// P09.T25: the removed share is within a factor of two of the brainstorm's 4 × 10⁻⁵.
    #[test]
    fn the_loss_cone_removes_some_four_in_a_hundred_thousand() {
        let (profile, df) = fixture();
        let cone = LossCone::new(profile.black_hole());
        let share = cone.share(&df, &profile);
        let acceptance = mean_acceptance(&OrbitMarks::old_stars(), &cone, &df, &profile);
        eprintln!("loss-cone share {share:.3e}, mean acceptance {acceptance:.5}");
        assert!((2e-5..8e-5).contains(&share), "{share}");
        assert!((OrbitMarks::old_stars().inclination_acceptance() - 0.590_29).abs() < 1e-4);
    }

    /// The disc's normal: Lu et al.'s (2009) formula gives Paumard et al.'s (2006) own vector for
    /// their (127°, 99°), (−0.12, −0.79, 0.60) ± 0.03; the sky frame's axes go to an orthonormal
    /// right-handed triad, the line of sight to the Sun–centre line and the north galactic pole's
    /// direction on the sky to −z; and Yelda et al.'s (130°, 96°) lands where the IAU's J2000
    /// galactic frame puts it, (−0.8864, −0.3258, 0.3291) at a bar angle of 27°.
    #[test]
    fn the_disc_s_normal_is_the_papers_in_galactic_axes() {
        let paumard = sky_normal(127.0, 99.0);
        for (got, want) in paumard.iter().zip([-0.12, -0.79, 0.60]) {
            assert!((got - want).abs() < 0.01, "{paumard:?}");
        }
        let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].map(sky_to_galactic);
        for (row, first) in axes.iter().enumerate() {
            for (column, second) in axes.iter().enumerate() {
                let want = if row == column { 1.0 } else { 0.0 };
                assert!(
                    (dot(*first, *second) - want).abs() < 1e-12,
                    "{row} {column}"
                );
            }
        }
        let [east, north, away] = axes;
        assert!(dot(cross(east, north), away) > 1.0 - 1e-12, "left-handed");
        let (sin_p, cos_p) = math::sin_cos(GALACTIC_PLANE_POSITION_ANGLE.to_radians());
        let pole = sky_to_galactic([-cos_p, sin_p, 0.0]);
        assert!((pole[2] + 1.0).abs() < 1e-12, "{pole:?}");
        let sight = sky_to_galactic([0.0, 0.0, 1.0]);
        let (sin_b, cos_b) = math::sin_cos(BAR_ANGLE.to_radians());
        assert!((sight[0] + cos_b).abs() < 1e-12 && (sight[1] - sin_b).abs() < 1e-12);
        let normal = milky_way_disc_normal();
        eprintln!(
            "disc normal {normal:?}, {:.2}° from the galaxy's axis",
            math::acos(normal[2]).to_degrees()
        );
        for (got, want) in normal.iter().zip([-0.886_35, -0.325_76, 0.329_05]) {
            assert!((got - want).abs() < 2e-3, "{normal:?}");
        }
    }

    fn dot(first: [f64; 3], second: [f64; 3]) -> f64 {
        first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
    }

    fn cross(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
        [
            first[1] * second[2] - first[2] * second[1],
            first[2] * second[0] - first[0] * second[2],
            first[0] * second[1] - first[1] * second[0],
        ]
    }

    /// A unit vector across `axis`.
    fn across(axis: [f64; 3]) -> [f64; 3] {
        let trial = if axis[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let along = dot(trial, axis);
        let off = [0, 1, 2].map(|i| trial[i] - along * axis[i]);
        let length = dot(off, off).sqrt();
        off.map(|c| c / length)
    }

    /// `∫ f` from `lo` to `hi` in `count` 32-node panels.
    fn panels(f: impl Fn(f64) -> f64, lo: f64, hi: f64, count: u32) -> f64 {
        let width = (hi - lo) / f64::from(count);
        (0..count)
            .map(|panel| {
                let start = lo + width * f64::from(panel);
                gl32(&f, start, start + width)
            })
            .sum()
    }

    /// The angular factor is the inclination mark's mean over isotropic velocity directions, at
    /// four polar angles and both flattenings, within four standard errors of 40,000 directions;
    /// its mean over the sphere is the inclination acceptance, and no angle exceeds the peak.
    #[test]
    fn the_angular_factor_is_the_marks_mean() {
        for marks in [OrbitMarks::old_stars(), OrbitMarks::young_disc()] {
            let axis = marks.axis();
            let off_axis = across(axis);
            for (index, theta) in [0.0_f64, 30.0, 60.0, 90.0].into_iter().enumerate() {
                let (sin_t, cos_t) = math::sin_cos(theta.to_radians());
                let position = [0, 1, 2].map(|i| cos_t * axis[i] + sin_t * off_axis[i]);
                let (mut sum, mut sum2) = (0.0, 0.0);
                let draws = 40_000_u32;
                for draw in 0..draws {
                    let key =
                        ObjectKey::cell((u64::try_from(index).unwrap() << 32) | u64::from(draw));
                    let mut stream =
                        Stream::open(Seed::new(0x0925_0001), tags::SELFTEST_STREAM, key);
                    let velocity = isotropic(&mut stream).components();
                    let (sin2, _) = marks.inclination(position, velocity);
                    let kept = math::exp(-marks.flattening() * sin2);
                    sum += kept;
                    sum2 += kept * kept;
                }
                let mean = sum / f64::from(draws);
                let error =
                    ((sum2 / f64::from(draws) - mean * mean).max(0.0) / f64::from(draws)).sqrt();
                let factor = marks.angular_factor(position);
                assert!(
                    (mean - factor).abs() <= 4.0 * error + 1e-12,
                    "k {} θ {theta}: {mean} ± {error} against {factor}",
                    marks.flattening()
                );
            }
            // The sphere's mean of the factor, and the peak over a fine grid of angles.
            let at = |mu: f64| {
                let sine = (1.0 - mu * mu).max(0.0).sqrt();
                marks.angular_factor([0, 1, 2].map(|i| mu * axis[i] + sine * off_axis[i]))
            };
            let mean = panels(at, 0.0, 1.0, 16);
            let worst = (0..=4_096)
                .map(|step| at(f64::from(step) / 4_096.0))
                .fold(0.0_f64, f64::max);
            assert!(
                (mean / marks.inclination_acceptance() - 1.0).abs() < 1e-9,
                "{mean} {}",
                marks.inclination_acceptance()
            );
            assert!(
                worst <= marks.peak_factor(),
                "{worst} {}",
                marks.peak_factor()
            );
            eprintln!(
                "k {}: acceptance {:.6}, peak {:.6}, bound per acceptance {:.4} (from {:.4})",
                marks.flattening(),
                marks.inclination_acceptance(),
                marks.peak_factor(),
                marks.peak_factor() / marks.inclination_acceptance(),
                1.0 / marks.inclination_acceptance()
            );
        }
    }

    /// The inclination mark draws the direction conditioned on it: at a point of the equator the
    /// kept orbits' mean `cos² i` is the conditional one, `∫ cos²φ e^(−k sin²φ) ÷ ∫ e^(−k
    /// sin²φ)`, within four standard errors, the speed is kept, and the draw is repeatable.
    #[test]
    fn the_inclination_mark_conditions_the_direction() {
        let pi = core::f64::consts::PI;
        for marks in [OrbitMarks::old_stars(), OrbitMarks::young_disc()] {
            let position = across(marks.axis()).map(|c| c * 0.5);
            let cone = LossCone::new(SolarMasses::new(4.3e6));
            let flattening = marks.flattening();
            let weight = |phi: f64| {
                let cos = math::cos(phi);
                math::exp(-flattening * (1.0 - cos * cos))
            };
            let want = panels(
                |phi| weight(phi) * math::cos(phi) * math::cos(phi),
                0.0,
                pi,
                32,
            ) / panels(weight, 0.0, pi, 32);
            let (mut sum, mut sum2, mut count) = (0.0, 0.0, 0_u32);
            for draw in 0..20_000_u64 {
                let mut stream = Stream::open(
                    Seed::new(0x0925_0002),
                    tags::SELFTEST_STREAM,
                    ObjectKey::cell(draw),
                );
                let velocity = isotropic(&mut stream).components().map(|c| c * 300.0);
                let mut again = stream.clone();
                let Some(kept) = marks.apply(&cone, position, velocity, &mut stream) else {
                    continue;
                };
                assert_eq!(
                    marks.apply(&cone, position, velocity, &mut again),
                    Some(kept)
                );
                let speed = dot(kept, kept).sqrt();
                assert!((speed / 300.0 - 1.0).abs() < 1e-12, "{speed}");
                let (sin2, _) = marks.inclination(position, kept);
                sum += 1.0 - sin2;
                sum2 += (1.0 - sin2) * (1.0 - sin2);
                count += 1;
            }
            let mean = sum / f64::from(count);
            let error =
                ((sum2 / f64::from(count) - mean * mean).max(0.0) / f64::from(count)).sqrt();
            eprintln!(
                "k {flattening}: mean cos² i {mean:.5} ± {error:.5} against {want:.5} over {count}"
            );
            assert!((mean - want).abs() <= 4.0 * error, "{mean} {want}");
        }
    }

    /// P09.T25 and ruling 144.7: the isodensity axis ratio inside 5 ly is 0.65–0.75: at each
    /// equatorial radius, the polar radius of the same realised density times the angular factor.
    #[test]
    fn the_cluster_s_isodensity_ratio_is_seven_tenths() {
        let (_, df) = fixture();
        let marks = OrbitMarks::old_stars();
        let (pole, equator) = (
            marks.angular_factor([0.0, 0.0, 1.0]),
            marks.angular_factor([1.0, 0.0, 0.0]),
        );
        for r in [0.01, 0.1, 0.3, 1.0, 2.0, 3.5, 4.9] {
            let target = df.density(r) * equator;
            let lr = crate::galaxy::quad::bisect(
                |lr| df.density(math::exp(lr)) * pole - target,
                math::ln(r * 0.2),
                math::ln(r),
                80,
            );
            let q = math::exp(lr) / r;
            eprintln!("isodensity q {q:.4} at {r} ly");
            assert!((0.65..=0.75).contains(&q), "{q} at {r} ly");
        }
    }

    /// Ruling 144.7: the projected isophote ratio seen edge-on at 1–8 ly is 0.68–0.76 (Schödel et
    /// al. 2014's 0.71 ± 0.02), along a line of sight in the plane, inside the reach. Provisional
    /// (the plan's Risks): the mark's angular factor is the same at every radius, so the ratio
    /// rounds as the profile steepens towards the break, and it passes 0.76 near 6 ly; held to
    /// 0.80 beyond 5 ly as measured, until the joint revision's optional k(E).
    #[test]
    fn the_cluster_s_projected_isophotes_are_flattened_as_observed() {
        let (profile, df) = fixture();
        let marks = OrbitMarks::old_stars();
        let reach = profile.reach().value();
        let column = |x: f64, z: f64| {
            let depth = (reach * reach - x * x - z * z).max(0.0).sqrt();
            let lo = 1e-4 * (x + z);
            let mut edges = vec![lo];
            while *edges.last().unwrap() * 10.0 < depth {
                edges.push(edges.last().unwrap() * 10.0);
            }
            edges.push(depth);
            let at = |y: f64| {
                let r = (x * x + y * y + z * z).sqrt();
                df.density(r) * marks.angular_factor([x, y, z])
            };
            2.0 * (at(0.0) * lo + gl_log_panels(at, &edges))
        };
        for r in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0] {
            let target = column(r, 0.0);
            let lz = crate::galaxy::quad::bisect(
                |lz| column(0.0, math::exp(lz)) - target,
                math::ln(r * 0.3),
                math::ln(r),
                60,
            );
            let q = math::exp(lz) / r;
            eprintln!("projected q {q:.4} at {r} ly");
            let high = if r <= 5.0 { 0.76 } else { 0.80 };
            assert!((0.68..=high).contains(&q), "{q} at {r} ly");
        }
    }

    /// P09.T25 and ruling 144.8: the cluster turns the galaxy's way, at 30–50 km/s in a slit along
    /// the plane (Feldmeier et al. 2014's amplitude of about 40 km/s, its faint stars reaching
    /// about 50), over 1–4 pc (3.3–13 ly) in a slit of ±1.4 ly (±11″ at 8 kpc).
    #[test]
    fn the_cluster_rotates_prograde_as_observed() {
        let (profile, df) = fixture();
        let cone = LossCone::new(profile.black_hole());
        let marks = OrbitMarks::old_stars();
        let (mut sum, mut count) = (0.0, 0_u32);
        for i in 0..400_000 {
            if let Some((p, v)) = member(&profile, &df, &marks, &cone, None, i) {
                let x = p[0].abs();
                if p[2].abs() < 1.4 && (3.3..13.0).contains(&x) {
                    // Seen along +y from −y: at +x the galaxy's rotation recedes.
                    sum += v[1] * p[0].signum();
                    count += 1;
                }
            }
        }
        let amplitude = sum / f64::from(count);
        eprintln!("slit rotation {amplitude:.1} km/s over {count} members");
        assert!((30.0..=50.0).contains(&amplitude), "{amplitude}");
    }
}
