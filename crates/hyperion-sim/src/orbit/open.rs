//! Open orbits: hyperbolic, parabolic and nearly parabolic (plan 14, P14.T2.b).
//!
//! Comets and bodies a supernova has unbound need eccentricities of 1 and above, which
//! [`Eccentricity`](super::Eccentricity) refuses (plan 14, D2). An [`OpenOrbit`] is described by
//! its pericentre and the time of pericentre passage rather than a semi-major axis, which is
//! infinite at e = 1 and ill-conditioned near it, and so it also carries the bound orbits that are
//! nearly parabolic, from [`OpenOrbit::MIN_ECCENTRICITY`] up.
//!
//! Three regimes, each the most accurate form there:
//!
//! - **|e − 1| < [`NEAR_PARABOLIC_BAND`]**: Kepler's equation in the universal variable χ, with
//!   the Stumpff functions by their series, which is one continuous function of e through 1. A
//!   bound orbit's interval is reduced modulo its period first.
//! - **e above the band**: the hyperbolic equation e sinh H − H = M by the same fixed scheme as the
//!   bound one ([`solve_kepler_hyperbolic`]).
//! - **e below the band**: the ellipse, by [`solve_kepler`](super::solve_kepler)'s method, the
//!   phase reduced exactly modulo the period from the pericentre time.

use super::kepler::{checked_mu, eccentric_anomaly, elliptic_state, reduce_to_half_turn};
use super::orientation::Orientation;
use super::{BuildOrbitError, is_positive, phase, sinh_excess, stumpff};
use crate::coords::{SystemVector, SystemVelocity};
use crate::math;
use crate::time::UniverseTime;
use core::f64::consts::TAU;

use crate::units::{GravitationalParameter, Metres};

/// The half-width of the band around e = 1 inside which an [`OpenOrbit`] is propagated by the
/// near-parabolic series rather than as an ellipse or a hyperbola (plan 14, P14.T2.b).
///
/// The elliptic and hyperbolic forms divide by 1 − e, which vanishes at the parabola; the
/// universal variable does not, so the band makes e = 1 and its neighbourhood one continuous
/// function. Positions from the two sides of each edge agree to well under a metre at 1 au.
pub const NEAR_PARABOLIC_BAND: f64 = 1e-6;

/// Halley iterations for the hyperbolic equation from Mikkola's starter, as for the bound one.
///
/// Measured over e from 1 + 10⁻⁷ to 1 + 10² and |M| from 10⁻¹⁶ to 10¹², the relative residual
/// |e sinh H − H − M| ÷ max(1, |M|) is at most 2.0 × 10⁻¹⁵ after two iterations, against
/// 1.1 × 10⁻⁸ after one.
const HYPERBOLIC_HALLEY_ITERATIONS: u32 = 2;

/// Halley iterations of the universal Kepler equation inside the near-parabolic band, from the
/// conic's own solution (see `near_parabolic_state`).
const UNIVERSAL_HALLEY_ITERATIONS: u32 = 2;

/// An open or nearly parabolic relative orbit: pericentre distance, eccentricity of at least
/// [`OpenOrbit::MIN_ECCENTRICITY`], [`Orientation`], time of pericentre passage and gravitational
/// parameter.
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::{OpenOrbit, Orientation};
/// use hyperion_sim::time::{Span, UniverseTime};
/// use hyperion_sim::units::{AstronomicalUnits, GravitationalParameter, Metres, Radians, SolarMasses};
///
/// // An interstellar visitor like ʻOumuamua: e = 1.2, pericentre 0.26 au, at pericentre at the epoch.
/// let visitor = OpenOrbit::new(
///     Metres::from(AstronomicalUnits::new(0.26)),
///     1.2,
///     Orientation::new(Radians::new(2.1), Radians::new(0.4), Radians::new(4.2))?,
///     UniverseTime::EPOCH,
///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
/// )?;
/// // A year later it is several astronomical units out and still leaving at over 26 km/s.
/// let (r, v) = visitor.relative_state_at(UniverseTime::from_julian_years(1).expect("in range"));
/// assert!(AstronomicalUnits::from(r.length()).value() > 5.0);
/// assert!(v.speed().value() > 26_000.0);
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenOrbit {
    pericentre: Metres,
    eccentricity: f64,
    orientation: Orientation,
    time_of_pericentre: UniverseTime,
    mu: GravitationalParameter,
}

impl OpenOrbit {
    /// The lowest eccentricity of an open orbit, 1 − 10⁻⁴ (plan 14's sketch of `OpenOrbit`).
    ///
    /// Below it a bound orbit is a [`KeplerElements`](super::KeplerElements).
    pub const MIN_ECCENTRICITY: f64 = 0.9999;

    /// An open orbit from its pericentre distance, eccentricity, orientation, time of pericentre
    /// passage and gravitational parameter (the sum of both bodies' GM).
    ///
    /// # Errors
    ///
    /// - [`BuildOrbitError::PericentreNotPositive`] unless the pericentre is finite and positive.
    /// - [`BuildOrbitError::OpenEccentricityOutOfRange`] for an eccentricity below
    ///   [`OpenOrbit::MIN_ECCENTRICITY`] or not finite.
    /// - [`BuildOrbitError::GravitationalParameterNotPositive`] unless `mu` is finite and
    ///   positive.
    /// - [`BuildOrbitError::MeanMotionNotPositive`] if, for any e but exactly 1, the mean motion
    ///   √(μ ÷ |a|³) with |a| = q ÷ |1 − e| overflows or underflows: an eccentricity of 10³⁰⁰, say.
    pub fn new(
        pericentre: Metres,
        eccentricity: f64,
        orientation: Orientation,
        time_of_pericentre: UniverseTime,
        mu: GravitationalParameter,
    ) -> Result<Self, BuildOrbitError> {
        if !is_positive(pericentre.value()) {
            return Err(BuildOrbitError::PericentreNotPositive {
                metres: pericentre.value(),
            });
        }
        if !(eccentricity.is_finite() && eccentricity >= Self::MIN_ECCENTRICITY) {
            return Err(BuildOrbitError::OpenEccentricityOutOfRange {
                value: eccentricity,
            });
        }
        let mu_value = checked_mu(mu)?;
        // Propagation uses the mean motion on |a| = q ÷ |1 − e| everywhere but on the parabola,
        // for the elliptic or hyperbolic equation or for the universal variable's starting value.
        if (eccentricity - 1.0).abs() > 0.0 {
            let semi_axis = pericentre.value() / (eccentricity - 1.0).abs();
            let mean_motion = (mu_value / semi_axis).sqrt() / semi_axis;
            if !is_positive(mean_motion) {
                return Err(BuildOrbitError::MeanMotionNotPositive {
                    radians_per_second: mean_motion,
                });
            }
        }
        Ok(Self {
            pericentre,
            eccentricity,
            orientation,
            time_of_pericentre,
            mu,
        })
    }

    /// The pericentre distance, m.
    #[must_use]
    pub const fn pericentre(&self) -> Metres {
        self.pericentre
    }

    /// The eccentricity, at least [`OpenOrbit::MIN_ECCENTRICITY`]: below 1 for a bound orbit near
    /// the parabola, 1 for a parabola, above 1 for a hyperbola.
    #[must_use]
    pub const fn eccentricity(&self) -> f64 {
        self.eccentricity
    }

    /// The orientation of the plane and the pericentre.
    #[must_use]
    pub const fn orientation(&self) -> &Orientation {
        &self.orientation
    }

    /// The time of pericentre passage: for a bound orbit, the one it was built with; every other
    /// passage is a whole number of periods from it.
    #[must_use]
    pub const fn time_of_pericentre(&self) -> UniverseTime {
        self.time_of_pericentre
    }

    /// The gravitational parameter, m³ s⁻².
    #[must_use]
    pub const fn gravitational_parameter(&self) -> GravitationalParameter {
        self.mu
    }

    /// The position and velocity of the secondary relative to the primary at `t`, in the system
    /// frame's axes, m and m s⁻¹. A pure function of the orbit and the time, before or after the
    /// pericentre.
    #[must_use]
    pub fn relative_state_at(&self, t: UniverseTime) -> (SystemVector, SystemVelocity) {
        let (seconds, nanos) = phase::interval(self.time_of_pericentre, t);
        let (q, e, mu) = (self.pericentre.value(), self.eccentricity, self.mu.value());
        if e < 1.0 {
            // Bound, in the band or below it: the interval is reduced exactly modulo the period
            // first, to the nearest pericentre passage, as for `KeplerElements`.
            let a = q / (1.0 - e);
            let period = TAU * (a * (a / mu).sqrt());
            let fraction = phase::fraction_of_period(seconds, nanos, period);
            if 1.0 - e < NEAR_PARABOLIC_BAND {
                near_parabolic_state(q, e, mu, fraction * period, &self.orientation)
            } else {
                elliptic_state(a, e, TAU / period, TAU * fraction, &self.orientation)
            }
        } else if e - 1.0 < NEAR_PARABOLIC_BAND {
            near_parabolic_state(q, e, mu, as_seconds(seconds, nanos), &self.orientation)
        } else {
            hyperbolic_state(q, e, mu, as_seconds(seconds, nanos), &self.orientation)
        }
    }
}

/// An exact interval as an `f64` of seconds, good to a unit in the last place: an open orbit's
/// anomaly grows without bound, so there is no period to reduce by.
fn as_seconds(seconds: i128, nanos: u32) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "rounded to the nearest f64, a relative 1e-16, which is all a non-periodic anomaly needs"
    )]
    let whole = seconds as f64;
    whole + f64::from(nanos) / 1e9
}

/// The hyperbolic anomaly H of a hyperbolic mean anomaly M: the solution of e sinh H − H = M, for
/// an eccentricity above 1 (plan 14, P14.T2.b).
///
/// M is n (t − τ), with n = √(μ ÷ |a|³), |a| = q ÷ (e − 1), and τ the time of pericentre. The
/// method is the bound equation's: Mikkola's (1987, Celestial Mechanics 40, 329) cubic starter for
/// the hyperbola, s = sinh(H ÷ 3), then exactly two Halley iterations, never stopping on a
/// tolerance. The result is odd in M. For e ≤ 1, or an input that is not finite, it is NaN.
///
/// # Examples
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::orbit::solve_kepler_hyperbolic;
///
/// let (m, e) = (250.0, 1.5);
/// let h = solve_kepler_hyperbolic(m, e);
/// assert!((e * math::sinh(h) - h - m).abs() < 1e-12);
/// ```
#[must_use]
pub fn solve_kepler_hyperbolic(mean_anomaly: f64, e: f64) -> f64 {
    if !(e > 1.0 && e.is_finite() && mean_anomaly.is_finite()) {
        return f64::NAN;
    }
    let magnitude = mean_anomaly.abs();
    let denominator = 4.0 * e + 0.5;
    let alpha = (e - 1.0) / denominator;
    let beta = 0.5 * magnitude / denominator;
    let z = math::cbrt(beta + (beta * beta + alpha * alpha * alpha).sqrt());
    let z2 = z * z;
    let mut s = 2.0 * beta / (z2 + alpha + alpha * alpha / z2);
    let s2 = s * s;
    s += 0.071 * (s2 * s2 * s) / ((1.0 + 0.45 * s2) * (1.0 + 4.0 * s2) * e);
    let mut anomaly = 3.0 * math::asinh(s);
    for _ in 0..HYPERBOLIC_HALLEY_ITERATIONS {
        let (sinh, cosh) = (math::sinh(anomaly), math::cosh(anomaly));
        // e sinh H − H as (e − 1) H + e (sinh H − H), which keeps its precision near e = 1.
        let f = (e - 1.0) * anomaly + e * sinh_excess(anomaly, sinh) - magnitude;
        // e cosh H − 1 = (e − 1) + e (cosh H − 1), and cosh H − 1 = sinh² H ÷ (cosh H + 1).
        let f1 = (e - 1.0) + e * (sinh * sinh / (cosh + 1.0));
        let f2 = e * sinh;
        anomaly -= 2.0 * f * f1 / (2.0 * f1 * f1 - f * f2);
    }
    anomaly.copysign(mean_anomaly)
}

/// The solution D = tan(ν ÷ 2) of Barker's equation D + D³ ÷ 3 = M for a parabola, in closed
/// form (plan 14, P14.T2.b).
///
/// M is √(μ ÷ 2q³) (t − τ), with q the pericentre distance and τ the time of pericentre; ν is
/// the true anomaly. By Cardano, D = A − 1 ÷ A with A³ = B + √(1 + B²) and B = 3M ÷ 2, taken as
/// the equal 2B ÷ (A² + 1 + A⁻²), which does not cancel for small M. The result is odd in M.
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::solve_barker;
///
/// // A quarter of the way round from pericentre, ν = 90°, is D = 1 at M = 4/3.
/// assert!((solve_barker(4.0 / 3.0) - 1.0).abs() < 1e-15);
/// ```
#[must_use]
pub fn solve_barker(mean_anomaly: f64) -> f64 {
    let b = 1.5 * mean_anomaly.abs();
    let a = math::cbrt(b + math::hypot(1.0, b));
    let d = 2.0 * b / (a * a + 1.0 + 1.0 / (a * a));
    d.copysign(mean_anomaly)
}

/// The state on a hyperbola `dt` seconds after pericentre, for pericentre distance `pericentre`
/// q, `eccentricity` e above 1 and gravitational parameter `mu`.
///
/// In the orbit's plane x = |a| (e − cosh H) = q − |a| (cosh H − 1) and y = |a| √(e² − 1) sinh H,
/// with cosh H − 1 taken as sinh² H ÷ (cosh H + 1) so that neither cancels near pericentre.
fn hyperbolic_state(
    pericentre: f64,
    eccentricity: f64,
    mu: f64,
    dt: f64,
    orientation: &Orientation,
) -> (SystemVector, SystemVelocity) {
    let (q, e) = (pericentre, eccentricity);
    let semi_axis = q / (e - 1.0);
    let mean_motion = (mu / semi_axis).sqrt() / semi_axis;
    let anomaly = solve_kepler_hyperbolic(mean_motion * dt, e);
    let (sinh, cosh) = (math::sinh(anomaly), math::cosh(anomaly));
    let cosh_minus_one = sinh * sinh / (cosh + 1.0);
    let axis_ratio = ((e - 1.0) * (e + 1.0)).sqrt();
    let radius = q + e * semi_axis * cosh_minus_one;
    let rate = mean_motion * semi_axis / radius;
    (
        SystemVector::new(orientation.plane_to_system(
            q - semi_axis * cosh_minus_one,
            semi_axis * axis_ratio * sinh,
        )),
        SystemVelocity::new(orientation.plane_to_system(
            -semi_axis * sinh * rate,
            semi_axis * axis_ratio * cosh * rate,
        )),
    )
}

/// The state on a nearly parabolic orbit `dt` seconds after pericentre, by the universal
/// variable, for pericentre distance `pericentre` q and `eccentricity` e within
/// [`NEAR_PARABOLIC_BAND`] of 1. For a bound orbit `dt` is already reduced to within half a
/// period of the pericentre.
///
/// With α = (1 − e) ÷ q, the reciprocal semi-major axis, and z = α χ², Kepler's equation from
/// pericentre is √μ Δt = q χ + e χ³ S(z), whose derivative in χ is the distance
/// r = q + e χ² C(z) (Battin 1999, §4.5; the pericentre has no radial velocity). It is solved by
/// [`UNIVERSAL_HALLEY_ITERATIONS`] Halley iterations from the conic's own solution, χ = E ÷ √α on
/// an ellipse and H ÷ √−α on a hyperbola, which the fixed schemes of [`solve_kepler`] and
/// [`solve_kepler_hyperbolic`] give accurately here because they evaluate their equations in a
/// form that keeps its precision near e = 1, and from Barker's parabola, χ = √(2q) D, at e = 1
/// exactly. The iterations make the result one continuous function of e across 1. The Lagrange
/// coefficients then give x = q − χ² C, y = χ (1 − z S) √(q (1 + e)), and the velocity, with ġ
/// taken as q (1 − z C) ÷ r so that it does not cancel far out.
///
/// [`solve_kepler`]: super::solve_kepler
fn near_parabolic_state(
    pericentre: f64,
    eccentricity: f64,
    mu: f64,
    dt: f64,
    orientation: &Orientation,
) -> (SystemVector, SystemVelocity) {
    let (q, e) = (pericentre, eccentricity);
    let alpha = (1.0 - e) / q;
    let sqrt_mu = mu.sqrt();
    let target = sqrt_mu * dt;
    let mut chi = if alpha > 0.0 {
        let root = alpha.sqrt();
        let mean_anomaly = sqrt_mu * root * alpha * dt;
        eccentric_anomaly(reduce_to_half_turn(mean_anomaly), e) / root
    } else if alpha < 0.0 {
        let root = (-alpha).sqrt();
        solve_kepler_hyperbolic(sqrt_mu * root * -alpha * dt, e) / root
    } else {
        (2.0 * q).sqrt() * solve_barker((mu / (2.0 * q)).sqrt() / q * dt)
    };
    for _ in 0..UNIVERSAL_HALLEY_ITERATIONS {
        let z = alpha * chi * chi;
        let (stumpff_c, stumpff_s) = stumpff(z);
        let residual = q * chi + e * chi * chi * chi * stumpff_s - target;
        let first = q + e * chi * chi * stumpff_c;
        let second = e * chi * (1.0 - z * stumpff_s);
        chi -= 2.0 * residual * first / (2.0 * first * first - residual * second);
    }
    let z = alpha * chi * chi;
    let (stumpff_c, stumpff_s) = stumpff(z);
    let chi2_c = chi * chi * stumpff_c;
    let radius = q + e * chi2_c;
    let one_minus_zs = 1.0 - z * stumpff_s;
    (
        SystemVector::new(
            orientation.plane_to_system(q - chi2_c, chi * one_minus_zs * (q * (1.0 + e)).sqrt()),
        ),
        SystemVelocity::new(orientation.plane_to_system(
            -sqrt_mu * chi * one_minus_zs / radius,
            (mu * (1.0 + e) / q).sqrt() * q * (1.0 - z * stumpff_c) / radius,
        )),
    )
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{PI, TAU};

    use hyperion_testkit::lcg::Lcg;

    use super::super::{cross, dot};
    use super::*;
    use crate::orbit::{Eccentricity, KeplerElements};
    use crate::time::Span;
    use crate::units::Radians;
    use crate::units::consts::{GM_SUN, METRES_PER_AU};

    fn orientation(i: f64, node: f64, argument: f64) -> Orientation {
        Orientation::new(Radians::new(i), Radians::new(node), Radians::new(argument)).unwrap()
    }

    fn sun() -> GravitationalParameter {
        GravitationalParameter::new(GM_SUN)
    }

    fn after(t: UniverseTime, seconds: f64) -> UniverseTime {
        t.checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        dot(d, d).sqrt()
    }

    #[test]
    fn the_hyperbolic_residual_is_at_the_rounding_floor() {
        let mut worst: f64 = 0.0;
        for i in 0..=70 {
            let e = 1.0 + math::exp10(-7.0 + f64::from(i) / 7.0);
            for k in 0..=280 {
                let m = math::exp10(-16.0 + f64::from(k) / 10.0);
                for m in [m, -m] {
                    let h = solve_kepler_hyperbolic(m, e);
                    let residual = (e * math::sinh(h) - h - m).abs() / m.abs().max(1.0);
                    worst = worst.max(residual);
                }
            }
        }
        assert!(worst < 1e-13, "worst relative residual {worst:e}");
        assert!(
            worst < 3e-15,
            "worst relative residual {worst:e}: above the measured floor"
        );
        assert!(solve_kepler_hyperbolic(1.0, 1.0).is_nan());
        assert!(solve_kepler_hyperbolic(1.0, 0.5).is_nan());
        assert!(solve_kepler_hyperbolic(f64::INFINITY, 2.0).is_nan());
        assert!(solve_kepler_hyperbolic(0.0, 2.0).abs() < 1e-300);
    }

    #[test]
    fn barkers_equation_is_solved_in_closed_form() {
        for k in -60..=60 {
            let m = math::exp10(f64::from(k) / 5.0) * if k % 2 == 0 { 1.0 } else { -1.0 };
            let d = solve_barker(m);
            let residual = (d + d * d * d / 3.0 - m).abs() / m.abs().max(1.0);
            assert!(residual < 4e-15, "M = {m}: D = {d}, residual {residual:e}");
        }
        assert!(solve_barker(0.0).abs() < 1e-300);
        assert!((solve_barker(-4.0 / 3.0) + 1.0).abs() < 1e-15);
    }

    #[test]
    fn a_state_is_continuous_across_the_near_parabolic_band() {
        // P14.T2.b: at 1 au, the positions either side of each switch agree to 1 m, over a
        // century either side of pericentre.
        let q = METRES_PER_AU;
        let o = orientation(0.8, 2.0, 1.0);
        for edge in [1.0 - NEAR_PARABOLIC_BAND, 1.0 + NEAR_PARABOLIC_BAND] {
            for k in -40..=40 {
                let dt = f64::from(k) * 0.025 * 3.155_76e9;
                let (series, _) = near_parabolic_state(q, edge, GM_SUN, dt, &o);
                let (conic, _) = if edge < 1.0 {
                    let a = q / (1.0 - edge);
                    let period = TAU * (a * (a / GM_SUN).sqrt());
                    elliptic_state(a, edge, TAU / period, TAU * dt / period, &o)
                } else {
                    hyperbolic_state(q, edge, GM_SUN, dt, &o)
                };
                let gap = distance(series.metres(), conic.metres());
                assert!(gap < 1.0, "e = {edge}, dt = {dt} s: {gap} m apart");
            }
        }
    }

    #[test]
    fn open_orbits_conserve_energy_and_angular_momentum() {
        let mut lcg = Lcg::new(0x0e1e_0005);
        for n in 0..3_000 {
            let frame = orientation(
                PI * lcg.next_f64(),
                TAU * lcg.next_f64(),
                TAU * lcg.next_f64(),
            );
            let eccentricity = match n % 4 {
                0 => 1.0 + 2e-6 * (lcg.next_f64() - 0.5),
                1 => 1.0 + math::exp10(-6.0 + 8.0 * lcg.next_f64()),
                2 => 1.0 - math::exp10(-6.0 - 2.0 * lcg.next_f64() + 2.0),
                _ => 1.0 + 1e-6 * (1.0 + lcg.next_f64()),
            }
            .max(OpenOrbit::MIN_ECCENTRICITY);
            let pericentre = math::exp10(8.0 + 5.0 * lcg.next_f64());
            let orbit = OpenOrbit::new(
                Metres::new(pericentre),
                eccentricity,
                frame,
                UniverseTime::new(-1_000_000, 5).unwrap(),
                sun(),
            )
            .unwrap();
            let time = after(UniverseTime::EPOCH, (lcg.next_f64() - 0.5) * 1e10);
            let (position, velocity) = orbit.relative_state_at(time);
            let (position, velocity) = (position.metres(), velocity.metres_per_second());
            let radius = dot(position, position).sqrt();
            let speed2 = dot(velocity, velocity);
            // ε = v²/2 − μ/r = μ (e − 1) ÷ 2q, compared on the scale of its two terms.
            let energy = 0.5 * speed2 - GM_SUN / radius;
            let expected = GM_SUN * (eccentricity - 1.0) / (2.0 * pericentre);
            assert!(
                (energy - expected).abs() < 1e-12 * (0.5 * speed2 + GM_SUN / radius),
                "e = {eccentricity}: {energy:e} vs {expected:e}"
            );
            // r × v cancels where r and v are nearly parallel, far out on a hyperbola, so the
            // check allows the product's own rounding, a few 10⁻¹⁶ of |r| |v|.
            let momentum = cross(position, velocity);
            let expected = (GM_SUN * pericentre * (1.0 + eccentricity)).sqrt();
            let tolerance = 1e-12 * expected + 1e-15 * radius * speed2.sqrt();
            for (component, normal) in momentum.iter().zip(frame.normal()) {
                assert!(
                    (component - expected * normal).abs() < tolerance,
                    "e = {eccentricity}: h {momentum:?} vs {expected:e} × {:?}",
                    frame.normal()
                );
            }
        }
    }

    #[test]
    fn an_open_orbit_is_at_pericentre_at_its_pericentre_time() {
        let frame = orientation(1.0, 1.0, 1.0);
        let pericentre_time = UniverseTime::new(123_456, 789).unwrap();
        for eccentricity in [0.9999, 0.999_999_5, 1.0, 1.000_000_5, 1.3, 25.0] {
            let orbit = OpenOrbit::new(
                Metres::new(5e10),
                eccentricity,
                frame,
                pericentre_time,
                sun(),
            )
            .unwrap();
            let (position, velocity) = orbit.relative_state_at(pericentre_time);
            for (component, direction) in position.metres().iter().zip(frame.periapsis_direction())
            {
                assert!(
                    (component - 5e10 * direction).abs() < 1e-5,
                    "e = {eccentricity}"
                );
            }
            assert!(dot(position.metres(), velocity.metres_per_second()).abs() < 1e-5 * 5e10);
        }
    }

    #[test]
    fn a_bound_open_orbit_matches_the_same_ellipse_as_elements() {
        let o = orientation(0.4, 3.0, 5.0);
        let (q, e) = (2e11, 0.99995);
        let orbit = OpenOrbit::new(Metres::new(q), e, o, UniverseTime::EPOCH, sun()).unwrap();
        let elements = KeplerElements::from_semi_major_axis(
            Metres::new(q / (1.0 - e)),
            sun(),
            Eccentricity::new(e).unwrap(),
            o,
            Radians::ZERO,
        )
        .unwrap();
        for seconds in [-3e10, -1e6, 0.0, 4.4e8, 1e11] {
            let t = after(UniverseTime::EPOCH, seconds);
            let (r_open, _) = orbit.relative_state_at(t);
            let (r_bound, _) = elements.relative_state_at(t);
            assert!(distance(r_open.metres(), r_bound.metres()) <= 1e-12 * r_open.length().value());
        }
    }

    #[test]
    fn a_bound_orbit_in_the_band_is_periodic_and_is_the_ellipse() {
        // e within 10⁻⁶ of 1 and bound: the interval is reduced by the period, so the state is
        // right however many periods out, beyond r = a, and matches the ellipse's own solution.
        let frame = orientation(0.6, 1.7, 2.8);
        let (q, e) = (1e9, 1.0 - 5e-7);
        let orbit = OpenOrbit::new(Metres::new(q), e, frame, UniverseTime::EPOCH, sun()).unwrap();
        let axis = q / (1.0 - e);
        let period = TAU * (axis * (axis / GM_SUN).sqrt());
        for periods in [0.1, 0.3, 0.49, 0.9, 3.0, 10.25, -2.6] {
            let t = after(UniverseTime::EPOCH, periods * period);
            let (position, velocity) = orbit.relative_state_at(t);
            let (position, velocity) = (position.metres(), velocity.metres_per_second());
            // The ellipse at the phase of the time actually used: `periods × period` as a float
            // of seconds is good only to about 10⁻² s, which is kilometres at pericentre.
            let fraction =
                phase::fraction_of_period(i128::from(t.seconds()), t.subsec_nanos(), period);
            let (ellipse, _) = elliptic_state(axis, e, TAU / period, TAU * fraction, &frame);
            let radius = dot(position, position).sqrt();
            assert!(
                distance(position, ellipse.metres()) < 1e-9 * radius,
                "{periods} periods: {} m from the ellipse",
                distance(position, ellipse.metres())
            );
            let energy = 0.5 * dot(velocity, velocity) - GM_SUN / radius;
            let expected = -GM_SUN / (2.0 * axis);
            assert!(
                (energy - expected).abs() < 1e-9 * GM_SUN / radius,
                "{periods} periods: energy {energy:e} vs {expected:e}"
            );
            let (later, _) = orbit.relative_state_at(after(t, period));
            assert!(distance(later.metres(), position) < 1e-9 * radius);
        }
    }

    #[test]
    fn invalid_open_orbits_are_refused() {
        let o = orientation(0.0, 0.0, 0.0);
        let t = UniverseTime::EPOCH;
        assert_eq!(
            OpenOrbit::new(Metres::new(0.0), 1.5, o, t, sun()),
            Err(BuildOrbitError::PericentreNotPositive { metres: 0.0 })
        );
        assert_eq!(
            OpenOrbit::new(Metres::new(1.0), 0.9, o, t, sun()),
            Err(BuildOrbitError::OpenEccentricityOutOfRange { value: 0.9 })
        );
        assert!(matches!(
            OpenOrbit::new(Metres::new(1.0), f64::INFINITY, o, t, sun()),
            Err(BuildOrbitError::OpenEccentricityOutOfRange { .. })
        ));
        assert_eq!(
            OpenOrbit::new(Metres::new(1.0), 2.0, o, t, GravitationalParameter::ZERO),
            Err(BuildOrbitError::GravitationalParameterNotPositive { value: 0.0 })
        );
        assert!(matches!(
            OpenOrbit::new(Metres::new(1.0), 1e300, o, t, sun()),
            Err(BuildOrbitError::MeanMotionNotPositive { .. })
        ));
        assert!(matches!(
            OpenOrbit::new(Metres::new(1e300), 0.99995, o, t, sun()),
            Err(BuildOrbitError::MeanMotionNotPositive { .. })
        ));
        // A parabola divides by nothing, so a huge pericentre is still usable there.
        assert!(OpenOrbit::new(Metres::new(1e300), 1.0, o, t, sun()).is_ok());
        let orbit = OpenOrbit::new(Metres::new(1.0), 2.0, o, t, sun()).unwrap();
        assert!((orbit.pericentre().value() - 1.0).abs() < 1e-300);
        assert!((orbit.eccentricity() - 2.0).abs() < 1e-300);
        assert_eq!(orbit.time_of_pericentre(), t);
        assert_eq!(orbit.gravitational_parameter(), sun());
        assert_eq!(orbit.orientation(), &o);
    }
}
