//! The marks a centre member passes after its position and velocity: the loss cone, the
//! flattening and the rotation (plan 09, P09.T25; Design note 13).
//!
//! A candidate drawn from the spherical, isotropic distribution function computes its angular
//! momentum about the black hole, and three things follow, each a function of integrals of the
//! motion, so the cluster stays stationary (the brainstorm, "Dense features"):
//!
//! - **The loss cone.** A member whose Kepler pericentre about the black hole lies inside
//!   [`loss_cone_radius`], about 2 au × (`M_bh` ÷ 4.3 × 10⁶ M☉)^⅓, would have been torn apart and is
//!   rejected, which removes some 4 × 10⁻⁵ of the cluster ([`LossCone::share`]).
//! - **The flattening.** One mark accepts the orbit with probability `exp(−k sin² i)`, `i` its
//!   inclination to the galactic plane. The density becomes the profile times
//!   `e^−k e^a I₀(a)` with `a = k sin²θ ÷ 2` at polar angle θ, so the pole's density over the
//!   equator's at one radius is `e^(−k/2) ÷ I₀(k/2)`; in a cusp of slope 1.3 that is an isodensity
//!   axis ratio of 0.70 at k = 0.84 (the brainstorm's figure; Schödel et al. 2014 measure 0.71 ±
//!   0.02). Its second moments are rounder, `√(⟨z²⟩ ÷ ⟨x²⟩)` = 0.91 (a finding against the plan's
//!   test, in the plan's Risks).
//! - **The rotation.** A second mark reverses the velocity of a share of the retrograde survivors
//!   (`L_z < 0`), which keeps the energy, `|L|` and `sin² i`, so the cluster turns the galaxy's
//!   way (counter-clockwise seen from the north).
//!
//! The placement bound is the spherical profile's, divided by the marks' mean acceptance
//! ([`mean_acceptance`]), so that the accepted members hold the cluster's mass; this wastes some
//! two fifths of the candidates at k = 0.84 and needs no Bessel function in the bound. Words of a
//! member's `centre.marks` stream: the inclination's mark, then the reversal's, both always drawn.

use crate::galaxy::consts::G;
use crate::galaxy::quad::{gl_log_panels, gl32};
use crate::math;
use crate::rng::{Stream, Threshold};
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

/// A class's flattening and rotation marks (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitMarks {
    flattening: f64,
    reversal: f64,
}

impl OrbitMarks {
    /// The old cluster's: k = 0.84 for the brainstorm's flattening of 0.7, and a reversal share
    /// of 0.8 (provisional, ours; the slit's rotation is linear in it, 24 km/s at 0.5), which
    /// gives a rotation of some 40 km/s in a slit along the plane (Feldmeier et al. 2014, A&A 570, A2, §4.3: "a rotation curve with an amplitude of
    /// ∼40 km/s", its faint stars reaching about 50).
    pub const OLD_STARS: Self = Self {
        flattening: 0.84,
        reversal: 0.8,
    };

    /// The young inner disc's: k = 16, an rms inclination near 10°, and every retrograde orbit
    /// reversed, so the disc turns one way (provisional, ours; the Milky Way's clockwise disc is
    /// tilted to the plane and some 10° thick, Yelda et al. 2014).
    pub const YOUNG_DISC: Self = Self {
        flattening: 16.0,
        reversal: 1.0,
    };

    /// The flattening constant k.
    #[must_use]
    pub fn flattening(&self) -> f64 {
        self.flattening
    }

    /// The share of retrograde orbits reversed.
    #[must_use]
    pub fn reversal_share(&self) -> f64 {
        self.reversal
    }

    /// The inclination mark's mean acceptance over isotropic orbits, `∫₀¹ exp(−k (1 − μ²)) dμ`, by
    /// 32-node Gauss–Legendre: 0.590 at k = 0.84.
    #[must_use]
    pub fn inclination_acceptance(&self) -> f64 {
        gl32(|mu| math::exp(-self.flattening * (1.0 - mu * mu)), 0.0, 1.0)
    }

    /// The marks of the orbit through `position` (ly) with `velocity` (km/s), on the next two
    /// words of `stream`: `None` if the loss cone or the inclination rejects it, else its
    /// velocity, reversed if the reversal mark turns it prograde.
    #[expect(
        clippy::many_single_char_names,
        reason = "position, velocity and angular momentum in their usual symbols"
    )]
    pub fn apply(
        &self,
        loss_cone: &LossCone,
        position: [f64; 3],
        velocity: [f64; 3],
        stream: &mut Stream,
    ) -> Option<[f64; 3]> {
        let inclination = stream.mark();
        let reversal = stream.mark();
        if loss_cone.contains(position, velocity) {
            return None;
        }
        let [x, y, z] = position;
        let [u, v, w] = velocity;
        let l = [y * w - z * v, z * u - x * w, x * v - y * u];
        let l2 = l[0] * l[0] + l[1] * l[1] + l[2] * l[2];
        let sin2 = if l2 > 0.0 {
            (1.0 - l[2] * l[2] / l2).max(0.0)
        } else {
            1.0
        };
        if !inclination.is_below(Threshold::from_probability(math::exp(
            -self.flattening * sin2,
        ))) {
            return None;
        }
        if l[2] < 0.0 && reversal.is_below(Threshold::from_probability(self.reversal)) {
            return Some(velocity.map(|c| -c));
        }
        Some(velocity)
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
    use crate::stellar::draws::isotropic;

    fn fixture() -> (CentreProfile, DistributionFunction) {
        let profile = CentreProfile::from_params(&GalaxyParams::milky_way_like()).unwrap();
        let df = DistributionFunction::invert(profile.stars(), &profile).unwrap();
        (profile, df)
    }

    /// A member at radius `r` (or drawn from the realised profile inside the reach when `None`),
    /// isotropic in direction, with its velocity and its marks, from stream `n`.
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
        let acceptance = mean_acceptance(&OrbitMarks::OLD_STARS, &cone, &df, &profile);
        eprintln!("loss-cone share {share:.3e}, mean acceptance {acceptance:.5}");
        assert!((2e-5..8e-5).contains(&share), "{share}");
        assert!((OrbitMarks::OLD_STARS.inclination_acceptance() - 0.590_29).abs() < 1e-4);
    }

    /// P09.T25: the axis ratio is 0.65–0.75, read as the brainstorm's: the isodensity ratio of
    /// the cusp, `(n_pole ÷ n_equator)^(1/γ)` at one radius. The second moments' ratio is printed
    /// (a finding).
    #[test]
    fn the_cluster_is_flattened_to_seven_tenths() {
        let (profile, df) = fixture();
        let cone = LossCone::new(profile.black_hole());
        let r = 1.0;
        let gamma = -math::ln(df.density(r * 1.01) / df.density(r / 1.01)) / math::ln(1.0201);
        let (mut pole, mut equator, mut z2, mut x2, mut kept) = (0_u32, 0_u32, 0.0, 0.0, 0_u32);
        let n = 200_000;
        for i in 0..n {
            if let Some((p, _)) = member(&profile, &df, &OrbitMarks::OLD_STARS, &cone, Some(r), i) {
                kept += 1;
                let mu = (p[2] / r).abs();
                if mu >= 0.95 {
                    pole += 1;
                }
                if mu < 0.05 {
                    equator += 1;
                }
                z2 += p[2] * p[2];
                x2 += f64::midpoint(p[0] * p[0], p[1] * p[1]);
            }
        }
        let q = math::powf(f64::from(pole) / f64::from(equator), 1.0 / gamma);
        let moments = (z2 / x2).sqrt();
        let acceptance = f64::from(kept) / 200_000.0;
        eprintln!(
            "γ {gamma:.4}, pole {pole}, equator {equator}: isodensity q {q:.4}; second moments \
             {moments:.4}; accepted {acceptance:.4}"
        );
        assert!((0.65..=0.75).contains(&q), "{q}");
        assert!((acceptance - 0.5903).abs() < 0.005, "{acceptance}");
    }

    /// P09.T25: the cluster turns the galaxy's way, at a speed in a slit along the plane within
    /// Feldmeier et al.'s (2014) range: an amplitude of about 40 km/s, the faint stars' reaching
    /// about 50; held to 25–55 km/s over 1–4 pc (3.3–13 ly) in a slit of ±1.4 ly (±11″ at 8 kpc).
    #[test]
    fn the_cluster_rotates_prograde_as_observed() {
        let (profile, df) = fixture();
        let cone = LossCone::new(profile.black_hole());
        let (mut sum, mut count) = (0.0, 0_u32);
        for i in 0..400_000 {
            if let Some((p, v)) = member(&profile, &df, &OrbitMarks::OLD_STARS, &cone, None, i) {
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
        assert!((25.0..=55.0).contains(&amplitude), "{amplitude}");
    }
}
