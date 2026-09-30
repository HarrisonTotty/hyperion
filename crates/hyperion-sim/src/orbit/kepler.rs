//! Bound orbits: the eccentricity, the elements, Kepler's equation and the state at a time.

use std::f64::consts::{PI, TAU};

use super::orientation::{Orientation, reduce_to_turn};
use super::{BuildOrbitError, is_positive, one_minus_cos, phase, sin_deficit};
use crate::coords::{SystemVector, SystemVelocity};
use crate::math;
use crate::time::UniverseTime;
use crate::units::{GravitationalParameter, Metres, Radians, Seconds};

/// The number of Halley iterations [`solve_kepler`] runs from Mikkola's starter: always this
/// many, never fewer on a tolerance, so that every platform takes the same steps.
///
/// Two is the fewest that reaches the rounding floor everywhere. Measured over a grid of 10⁵
/// values of the eccentricity and mean anomaly and 2 × 10⁷ random ones, with M from 10⁻¹⁵ to π,
/// the residual |E − e sin E − M| after two iterations is at most 8.9 × 10⁻¹⁶ rad (two units in
/// the last place of π) for e up to 0.999 and 1.3 × 10⁻¹⁵ rad up to 0.999 999, against
/// 8.7 × 10⁻⁹ after one; and E is within 4.4 × 10⁻¹⁶ of itself (two units in its last place) of
/// the root that four further Newton steps converge to. Plan 14's requirement (P14.T2.a) is
/// 10⁻¹³ up to e = 0.999. Three Newton iterations reach the same floor at the cost of a third sine
/// and cosine.
pub const KEPLER_HALLEY_ITERATIONS: u32 = 2;

/// The eccentricity of a bound orbit, in `[0, 1)`.
///
/// Open orbits, and bound ones so nearly parabolic that pericentre and pericentre time describe
/// them better than a semi-major axis, are [`OpenOrbit`](super::OpenOrbit)s instead (plan 14, D2).
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Eccentricity(f64);

impl Eccentricity {
    /// A circular orbit. Also the [`Default`].
    pub const CIRCULAR: Self = Self(0.0);

    /// An eccentricity from its value; `−0` is taken as `0`.
    ///
    /// # Errors
    ///
    /// [`BuildOrbitError::EccentricityOutOfRange`] for a value outside `[0, 1)` or NaN.
    pub fn new(value: f64) -> Result<Self, BuildOrbitError> {
        if (0.0..1.0).contains(&value) {
            Ok(Self(value + 0.0))
        } else {
            Err(BuildOrbitError::EccentricityOutOfRange { value })
        }
    }

    /// The value, in `[0, 1)`.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// The eccentric anomaly E of a mean anomaly M: the solution of Kepler's equation
/// E − e sin E = M.
///
/// M may be any angle; it is reduced into `[−π, π]` and E is returned in the same interval, with
/// the sign of the reduced M, so that E(−M) = −E(M) exactly. A non-finite M gives NaN.
///
/// The method is fixed so that every platform agrees bit for bit (plan 11, P11.T3.a; plan 14,
/// P14.T2.a): Mikkola's (1987, Celestial Mechanics 40, 329) cubic starter, then exactly
/// [`KEPLER_HALLEY_ITERATIONS`] Halley iterations, with no exit on a tolerance. The starter's
/// error is at most 3.6 × 10⁻³ rad, largest near e = 1 and M ≈ 1.7; each Halley iteration
/// cubes it. Near periapsis at high eccentricity E and e sin E are nearly equal, so the function
/// is taken as (1 − e) E + e (E − sin E), with E − sin E from its series, and its derivative
/// 1 − e cos E as (1 − e) + e (1 − cos E): both keep their relative precision there, which is
/// what lets a nearly parabolic orbit be propagated far from periapsis. The residual is at most
/// 8.9 × 10⁻¹⁶ rad for e up to 0.999 (see [`KEPLER_HALLEY_ITERATIONS`]).
///
/// # Examples
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::orbit::{Eccentricity, solve_kepler};
/// use hyperion_sim::units::Radians;
///
/// // A comet-like orbit a little after periapsis, where the equation is hardest.
/// let e = Eccentricity::new(0.999)?;
/// let m = 1e-4;
/// let big_e = solve_kepler(Radians::new(m), e).value();
/// assert!((big_e - 0.999 * math::sin(big_e) - m).abs() < 1e-15);
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
#[must_use]
pub fn solve_kepler(mean_anomaly: Radians, e: Eccentricity) -> Radians {
    Radians::new(eccentric_anomaly(
        reduce_to_half_turn(mean_anomaly.value()),
        e.value(),
    ))
}

/// E for M in `[−π, π]` and e in `[0, 1)`: solved for |M|, the sign restored.
pub(super) fn eccentric_anomaly(m: f64, e: f64) -> f64 {
    let magnitude = m.abs();
    let mut anomaly = mikkola_starter(magnitude, e);
    for _ in 0..KEPLER_HALLEY_ITERATIONS {
        anomaly = halley_step(anomaly, magnitude, e);
    }
    anomaly.copysign(m)
}

/// Mikkola's (1987) starting value of E for M in `[0, π]`.
///
/// Kepler's equation with sin E replaced by a cubic in s = sin(E ÷ 3), solved in closed form,
/// then corrected for the cubic's leading error. `z − α ÷ z`, Mikkola's form of s, cancels for
/// small M, so s is taken as the equal `2β ÷ (z² + α + α² ÷ z²)`. α is positive for e < 1, so z
/// is too.
fn mikkola_starter(m: f64, e: f64) -> f64 {
    let denominator = 4.0 * e + 0.5;
    let alpha = (1.0 - e) / denominator;
    let beta = 0.5 * m / denominator;
    let z = math::cbrt(beta + (beta * beta + alpha * alpha * alpha).sqrt());
    let z2 = z * z;
    let mut s = 2.0 * beta / (z2 + alpha + alpha * alpha / z2);
    let s2 = s * s;
    s -= 0.078 * (s2 * s2 * s) / (1.0 + e);
    m + e * s * (3.0 - 4.0 * s * s)
}

/// One Halley iteration on f(E) = E − e sin E − M.
fn halley_step(anomaly: f64, m: f64, e: f64) -> f64 {
    let (sin, cos) = math::sin_cos(anomaly);
    // E − e sin E as (1 − e) E + e (E − sin E), which keeps its precision near periapsis at high
    // eccentricity, where E and e sin E are nearly equal.
    let f = (1.0 - e) * anomaly + e * sin_deficit(anomaly, sin) - m;
    let f1 = (1.0 - e) + e * one_minus_cos(sin, cos);
    let f2 = e * sin;
    anomaly - 2.0 * f * f1 / (2.0 * f1 * f1 - f * f2)
}

/// An angle reduced into `[−π, π]`. A value already there is kept exactly, so that small angles
/// of either sign keep their relative precision. Otherwise the exact remainder modulo 2π
/// ([`math::fmod`]) is taken, and at most one exact addition or subtraction of 2π (Sterbenz)
/// brings it into range, so every angle below 3π in magnitude is reduced without rounding.
pub(super) fn reduce_to_half_turn(radians: f64) -> f64 {
    if radians.abs() <= PI {
        return radians;
    }
    let turn = math::fmod(radians, TAU);
    if turn > PI {
        turn - TAU
    } else if turn < -PI {
        turn + TAU
    } else {
        turn
    }
}

/// The relative position and velocity on an ellipse at `mean_anomaly` (rad, any angle).
///
/// `semi_major_axis` a in metres, `eccentricity` e in `[0, 1)`, `mean_motion` 2π ÷ period in
/// rad s⁻¹. In the orbit's plane, x = a (cos E − e) and y = a √(1 − e²) sin E, with cos E − e
/// taken as (1 − e) − (1 − cos E) and 1 − e cos E as (1 − e) + e (1 − cos E), so that both keep
/// their precision near periapsis at high eccentricity, where each is a small difference of
/// numbers near 1.
pub(super) fn elliptic_state(
    semi_major_axis: f64,
    eccentricity: f64,
    mean_motion: f64,
    mean_anomaly: f64,
    orientation: &Orientation,
) -> (SystemVector, SystemVelocity) {
    let (a, e) = (semi_major_axis, eccentricity);
    let anomaly = eccentric_anomaly(reduce_to_half_turn(mean_anomaly), e);
    let (sin, cos) = math::sin_cos(anomaly);
    let one_minus_cos = one_minus_cos(sin, cos);
    let one_minus_e = 1.0 - e;
    let axis_ratio = (one_minus_e * (1.0 + e)).sqrt();
    let along_periapsis = a * (one_minus_e - one_minus_cos);
    let across = a * axis_ratio * sin;
    let radius_over_a = one_minus_e + e * one_minus_cos;
    let speed = mean_motion * a / radius_over_a;
    (
        SystemVector::new(orientation.plane_to_system(along_periapsis, across)),
        SystemVelocity::new(orientation.plane_to_system(-speed * sin, speed * axis_ratio * cos)),
    )
}

/// The Keplerian elements of a bound relative orbit at the epoch, in the system frame.
///
/// Semi-major axis, [`Eccentricity`], [`Orientation`], mean anomaly at the epoch, period and
/// gravitational parameter: the whole set the wire carries (ruling 33 of 2026-09-22). The period
/// and semi-major axis agree with the gravitational parameter by Kepler's third law, one of the
/// two being derived from the other by each constructor. The orbit is that of the secondary
/// relative to the primary, or of a body relative to its host; placing both about their
/// barycentre is the caller's (plan 11, P11.T3.b).
///
/// # Examples
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::time::{Span, UniverseTime};
/// use hyperion_sim::units::{AstronomicalUnits, GravitationalParameter, Metres, Radians, SolarMasses};
///
/// // The Earth about the Sun, at periapsis at the epoch.
/// let earth = KeplerElements::from_semi_major_axis(
///     Metres::from(AstronomicalUnits::new(1.0)),
///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///     Eccentricity::new(0.0167)?,
///     Orientation::new(Radians::ZERO, Radians::ZERO, Radians::ZERO)?,
///     Radians::ZERO,
/// )?;
/// assert!((earth.period().value() / 86_400.0 - 365.256_9).abs() < 1e-4);
///
/// // Half a year on it is at the far side, near apoapsis, moving the other way.
/// let half = UniverseTime::EPOCH.checked_add(Span::from_seconds(15_778_800)).expect("in range");
/// let (r, v) = earth.relative_state_at(half);
/// assert!(r.metres()[0] < -1.52e11 && v.metres_per_second()[1] < -29_000.0);
/// # Ok::<(), hyperion_sim::orbit::BuildOrbitError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeplerElements {
    semi_major_axis: Metres,
    eccentricity: Eccentricity,
    orientation: Orientation,
    mean_anomaly_at_epoch: Radians,
    period: Seconds,
    mu: GravitationalParameter,
}

impl KeplerElements {
    /// The orbit of semi-major axis `a` under gravitational parameter `mu` (the sum of both
    /// bodies' GM for a relative orbit), with period 2π √(a³ ÷ μ) (plan 14, P14.T2.a).
    ///
    /// The mean anomaly at the epoch may be any finite angle and is reduced into `[0, 2π)`.
    ///
    /// # Errors
    ///
    /// - [`BuildOrbitError::SemiMajorAxisNotPositive`] unless `a` is finite and positive.
    /// - [`BuildOrbitError::GravitationalParameterNotPositive`] unless `mu` is finite and
    ///   positive.
    /// - [`BuildOrbitError::AngleNotFinite`] for a mean anomaly that is not finite.
    /// - [`BuildOrbitError::PeriodNotPositive`] if the period overflows or underflows.
    pub fn from_semi_major_axis(
        a: Metres,
        mu: GravitationalParameter,
        e: Eccentricity,
        orientation: Orientation,
        mean_anomaly_at_epoch: Radians,
    ) -> Result<Self, BuildOrbitError> {
        let (a, mu) = (a.value(), checked_mu(mu)?);
        if !is_positive(a) {
            return Err(BuildOrbitError::SemiMajorAxisNotPositive { metres: a });
        }
        let period = TAU * (a * (a / mu).sqrt());
        if !is_positive(period) {
            return Err(BuildOrbitError::PeriodNotPositive { seconds: period });
        }
        Ok(Self {
            semi_major_axis: Metres::new(a),
            eccentricity: e,
            orientation,
            mean_anomaly_at_epoch: Radians::new(reduce_to_turn(mean_anomaly_at_epoch.value())?),
            period: Seconds::new(period),
            mu: GravitationalParameter::new(mu),
        })
    }

    /// The orbit of period `period` under gravitational parameter `mu`, with semi-major axis
    /// ∛(μ P² ÷ 4π²): the form for an orbit drawn by its period, as plan 11's companions are.
    ///
    /// The period is kept exactly as given. The mean anomaly at the epoch is reduced into
    /// `[0, 2π)`.
    ///
    /// # Errors
    ///
    /// - [`BuildOrbitError::PeriodNotPositive`] unless the period is finite and positive.
    /// - [`BuildOrbitError::GravitationalParameterNotPositive`] unless `mu` is finite and
    ///   positive.
    /// - [`BuildOrbitError::AngleNotFinite`] for a mean anomaly that is not finite.
    /// - [`BuildOrbitError::SemiMajorAxisNotPositive`] if the axis overflows or underflows.
    pub fn from_period(
        period: Seconds,
        mu: GravitationalParameter,
        e: Eccentricity,
        orientation: Orientation,
        mean_anomaly_at_epoch: Radians,
    ) -> Result<Self, BuildOrbitError> {
        let (period, mu) = (period.value(), checked_mu(mu)?);
        if !is_positive(period) {
            return Err(BuildOrbitError::PeriodNotPositive { seconds: period });
        }
        let per_radian = period / TAU;
        let a = math::cbrt(mu * per_radian * per_radian);
        if !is_positive(a) {
            return Err(BuildOrbitError::SemiMajorAxisNotPositive { metres: a });
        }
        Ok(Self {
            semi_major_axis: Metres::new(a),
            eccentricity: e,
            orientation,
            mean_anomaly_at_epoch: Radians::new(reduce_to_turn(mean_anomaly_at_epoch.value())?),
            period: Seconds::new(period),
            mu: GravitationalParameter::new(mu),
        })
    }

    /// The same orbit with its semi-major axis multiplied by `factor` about a new gravitational
    /// parameter: eccentricity, orientation and mean anomaly at the epoch unchanged, the period
    /// from Kepler's third law.
    ///
    /// This is plan 14's adiabatic expansion under mass loss (D11): a(t) = a₀ μ₀ ÷ μ(t) with the
    /// eccentricity unchanged, where μ is the pair's total G(M₁ + M₂) (Veras et al. 2011, MNRAS
    /// 417, 2104, eqs. 2 and 18), so `factor` is μ₀ ÷ μ(t), which is M₀ ÷ M(t) for a planet of
    /// negligible mass, and `mu` the new μ. The specific angular momentum √(μ a (1 − e²)), the
    /// adiabatic invariant, is then conserved. It holds while the mass is lost slowly against the
    /// period, which is true of winds for orbits inside about 1,000 au.
    ///
    /// # Errors
    ///
    /// - [`BuildOrbitError::ScaleFactorNotPositive`] unless `factor` is finite and positive.
    /// - As [`KeplerElements::from_semi_major_axis`] for the new axis and `mu`.
    pub fn scaled(&self, factor: f64, mu: GravitationalParameter) -> Result<Self, BuildOrbitError> {
        if !is_positive(factor) {
            return Err(BuildOrbitError::ScaleFactorNotPositive { factor });
        }
        Self::from_semi_major_axis(
            self.semi_major_axis * factor,
            mu,
            self.eccentricity,
            self.orientation,
            self.mean_anomaly_at_epoch,
        )
    }

    /// The semi-major axis, m.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Metres {
        self.semi_major_axis
    }

    /// The eccentricity.
    #[must_use]
    pub const fn eccentricity(&self) -> Eccentricity {
        self.eccentricity
    }

    /// The orientation of the plane and the periapsis.
    #[must_use]
    pub const fn orientation(&self) -> &Orientation {
        &self.orientation
    }

    /// The inclination, rad, in `[0, π]`.
    #[must_use]
    pub const fn inclination(&self) -> Radians {
        self.orientation.inclination()
    }

    /// The longitude of the ascending node, rad, in `[0, 2π)`.
    #[must_use]
    pub const fn ascending_node(&self) -> Radians {
        self.orientation.ascending_node()
    }

    /// The argument of periapsis, rad, in `[0, 2π)`.
    #[must_use]
    pub const fn argument_of_periapsis(&self) -> Radians {
        self.orientation.argument_of_periapsis()
    }

    /// The mean anomaly at the epoch, rad, in `[0, 2π)`.
    #[must_use]
    pub const fn mean_anomaly_at_epoch(&self) -> Radians {
        self.mean_anomaly_at_epoch
    }

    /// The period, s.
    #[must_use]
    pub const fn period(&self) -> Seconds {
        self.period
    }

    /// The gravitational parameter the orbit was built with, m³ s⁻²: 4π²a³ ÷ P² to rounding.
    #[must_use]
    pub const fn gravitational_parameter(&self) -> GravitationalParameter {
        self.mu
    }

    /// The pericentre distance a (1 − e), m.
    #[must_use]
    pub fn periapsis(&self) -> Metres {
        self.semi_major_axis * (1.0 - self.eccentricity.value())
    }

    /// The apocentre distance a (1 + e), m.
    #[must_use]
    pub fn apoapsis(&self) -> Metres {
        self.semi_major_axis * (1.0 + self.eccentricity.value())
    }

    /// The mean anomaly at `t`, rad, in `[0, 2π)`: the mean anomaly at the epoch plus 2π times
    /// the fraction of a period elapsed since the epoch.
    ///
    /// The fraction is reduced from the clock's integer seconds modulo the period before anything
    /// rounds (module docs, "Precision"), so it is good to a few units in the last place at any
    /// time, before or after the epoch, and a one-day orbit keeps its phase a thousand years out
    /// (plan 14, P14.T2.a).
    #[must_use]
    pub fn mean_anomaly_at(&self, t: UniverseTime) -> Radians {
        let m = self.unreduced_mean_anomaly_at(t);
        Radians::new(if m < 0.0 {
            // Below zero only by less than π, where the lift's rounding is at the scale of 2π.
            let lifted = m + TAU;
            if lifted < TAU { lifted } else { 0.0 }
        } else if m >= TAU {
            // Below 3π, so the subtraction is exact (Sterbenz).
            m - TAU
        } else {
            m
        })
    }

    /// The position and velocity of the secondary relative to the primary at `t`, in the system
    /// frame's axes, m and m s⁻¹.
    ///
    /// A pure function of the elements and the time, symmetric in time: the epoch is not a
    /// boundary and any time before or after it is valid.
    #[must_use]
    pub fn relative_state_at(&self, t: UniverseTime) -> (SystemVector, SystemVelocity) {
        elliptic_state(
            self.semi_major_axis.value(),
            self.eccentricity.value(),
            self.mean_motion(),
            self.unreduced_mean_anomaly_at(t),
            &self.orientation,
        )
    }

    /// The mean anomaly at the epoch plus 2π times the centred fraction of a period elapsed, rad,
    /// in `[−π, 3π)`: what propagation reduces itself, which keeps a small anomaly's precision.
    fn unreduced_mean_anomaly_at(&self, t: UniverseTime) -> f64 {
        let fraction = phase::fraction_of_period(
            i128::from(t.seconds()),
            t.subsec_nanos(),
            self.period.value(),
        );
        self.mean_anomaly_at_epoch.value() + TAU * fraction
    }

    /// 2π ÷ the period, rad s⁻¹.
    fn mean_motion(&self) -> f64 {
        TAU / self.period.value()
    }
}

/// `mu`'s value, if it is finite and positive.
pub(super) fn checked_mu(mu: GravitationalParameter) -> Result<f64, BuildOrbitError> {
    let value = mu.value();
    if is_positive(value) {
        Ok(value)
    } else {
        Err(BuildOrbitError::GravitationalParameterNotPositive { value })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::super::{cross, dot};
    use super::*;
    use crate::time::Span;
    use crate::units::consts::{GM_SUN, METRES_PER_AU};
    use crate::units::{AstronomicalUnits, SolarMasses};

    fn orientation(i: f64, node: f64, argument: f64) -> Orientation {
        Orientation::new(Radians::new(i), Radians::new(node), Radians::new(argument)).unwrap()
    }

    fn sun() -> GravitationalParameter {
        GravitationalParameter::new(GM_SUN)
    }

    fn orbit_about_the_sun(a_au: f64, e: f64, o: Orientation, m0: f64) -> KeplerElements {
        KeplerElements::from_semi_major_axis(
            Metres::from(AstronomicalUnits::new(a_au)),
            sun(),
            Eccentricity::new(e).unwrap(),
            o,
            Radians::new(m0),
        )
        .unwrap()
    }

    fn at(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    fn after(t: UniverseTime, seconds: f64) -> UniverseTime {
        t.checked_add(Span::from_seconds_f64(seconds).unwrap())
            .unwrap()
    }

    fn residual(m: f64, e: f64) -> f64 {
        let big_e = solve_kepler(Radians::new(m), Eccentricity::new(e).unwrap()).value();
        (big_e - e * math::sin(big_e) - reduce_to_half_turn(m)).abs()
    }

    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        dot(d, d).sqrt()
    }

    /// The eccentricities of the residual tests: a grid to 0.999, then towards 1.
    fn eccentricities() -> Vec<f64> {
        let mut es: Vec<f64> = (0..=999).map(|k| f64::from(k) / 1000.0).collect();
        es.extend((1..=99).map(|k| 0.99 + 0.009 * f64::from(k) / 100.0));
        es.extend([0.9995, 0.9999, 0.999_99, 0.999_999]);
        es
    }

    /// Mean anomalies from 10⁻¹⁵ to π, dense near 0 and π where the equation is hardest.
    fn mean_anomalies() -> Vec<f64> {
        let mut ms = vec![0.0, PI];
        for k in 0..=90 {
            let small = math::exp10(-15.0 + f64::from(k) / 6.0);
            ms.extend([small, PI - small]);
        }
        ms.extend((1..200).map(|k| PI * f64::from(k) / 200.0));
        ms.retain(|m| (0.0..=PI).contains(m));
        ms
    }

    #[test]
    fn kepler_residual_is_under_a_tenth_of_a_picoradian_to_e_0_999() {
        // P14.T2.a: under 1e-13 up to e = 0.999, which covers P11.T3.a's 1e-12 up to 0.99.
        let mut worst: f64 = 0.0;
        let mut worst_beyond: f64 = 0.0;
        for &e in &eccentricities() {
            for &m in &mean_anomalies() {
                let r = residual(m, e).max(residual(-m, e));
                if e <= 0.999 {
                    worst = worst.max(r);
                } else {
                    worst_beyond = worst_beyond.max(r);
                }
            }
        }
        let mut lcg = Lcg::new(0x0e1e_0002);
        for _ in 0..200_000 {
            let e = 0.999 * lcg.next_f64();
            let m = TAU * lcg.next_f64() - PI;
            worst = worst.max(residual(m, e));
        }
        assert!(worst < 1e-13, "worst residual {worst:e} for e ≤ 0.999");
        assert!(
            worst < 1e-15,
            "worst residual {worst:e}: above the floor measured when chosen"
        );
        assert!(
            worst_beyond < 2e-15,
            "worst residual {worst_beyond:e} for e ≤ 0.999999"
        );
    }

    #[test]
    fn kepler_is_odd_periodic_and_exact_for_a_circle() {
        let e = Eccentricity::new(0.7).unwrap();
        for m in [1e-9, 0.3, 1.5, 3.0, PI] {
            let plus = solve_kepler(Radians::new(m), e).value();
            let minus = solve_kepler(Radians::new(-m), e).value();
            assert_same_bits(minus, -plus);
            let later = solve_kepler(Radians::new(m + 4.0 * TAU), e).value();
            assert!((later - plus).abs() < 1e-14, "{later} vs {plus} at {m}");
        }
        // Just past π the anomaly wraps to just past −π.
        assert!(solve_kepler(Radians::new(PI + 0.1), e).value() < -3.0);
        for m in [0.0, 0.25, 2.0, -3.0] {
            assert_same_bits(
                solve_kepler(Radians::new(m), Eccentricity::CIRCULAR).value(),
                m,
            );
        }
        assert!(solve_kepler(Radians::new(f64::NAN), e).value().is_nan());
    }

    #[test]
    fn a_state_closes_after_one_period() {
        // P11.T3.a: the state at t and t + P agree to 1e-9 relative.
        let mut lcg = Lcg::new(0x0e1e_0003);
        for _ in 0..2_000 {
            let o = orientation(
                PI * lcg.next_f64(),
                TAU * lcg.next_f64(),
                TAU * lcg.next_f64(),
            );
            let a_au = math::exp10(-3.0 + 7.0 * lcg.next_f64());
            let orbit = orbit_about_the_sun(a_au, 0.99 * lcg.next_f64(), o, TAU * lcg.next_f64());
            let t = after(UniverseTime::EPOCH, (lcg.next_f64() - 0.5) * 2e12);
            let later = after(t, orbit.period().value());
            let (r0, v0) = orbit.relative_state_at(t);
            let (r1, v1) = orbit.relative_state_at(later);
            let (r0, v0, r1, v1) = (
                r0.metres(),
                v0.metres_per_second(),
                r1.metres(),
                v1.metres_per_second(),
            );
            assert!(
                distance(r0, r1) <= 1e-9 * dot(r0, r0).sqrt(),
                "{r0:?} vs {r1:?}"
            );
            assert!(
                distance(v0, v1) <= 1e-9 * dot(v0, v0).sqrt(),
                "{v0:?} vs {v1:?}"
            );
        }
    }

    #[test]
    fn a_state_is_symmetric_in_time() {
        // P11.T3.a: at periapsis at the epoch, the state at −t is the state at t mirrored in the
        // line of apsides, with the velocity reversed; and before the epoch is as good as after.
        let frame = orientation(1.1, 0.4, 2.3);
        let orbit = orbit_about_the_sun(0.8, 0.6, frame, 0.0);
        let (apsides, across) = (frame.periapsis_direction(), frame.plane_to_system(0.0, 1.0));
        for seconds in [1.0, 3.3e5, 1.0e7, 7.77e9, 3.1e11] {
            let (r_plus, v_plus) = orbit.relative_state_at(after(UniverseTime::EPOCH, seconds));
            let (r_minus, v_minus) = orbit.relative_state_at(after(UniverseTime::EPOCH, -seconds));
            let (r_plus, r_minus) = (r_plus.metres(), r_minus.metres());
            let (v_plus, v_minus) = (v_plus.metres_per_second(), v_minus.metres_per_second());
            let scale = dot(r_plus, r_plus).sqrt();
            let speed = dot(v_plus, v_plus).sqrt();
            assert!((dot(r_plus, apsides) - dot(r_minus, apsides)).abs() < 1e-12 * scale);
            assert!((dot(r_plus, across) + dot(r_minus, across)).abs() < 1e-12 * scale);
            assert!((dot(v_plus, apsides) + dot(v_minus, apsides)).abs() < 1e-12 * speed);
            assert!((dot(v_plus, across) - dot(v_minus, across)).abs() < 1e-12 * speed);
        }
        // Moving the epoch: elements whose epoch phase is the phase at t₁ give at t − t₁ what the
        // original gives at t.
        let t1 = at(-4_321_987_654, 250_000_000);
        let shifted = KeplerElements::from_semi_major_axis(
            orbit.semi_major_axis(),
            orbit.gravitational_parameter(),
            orbit.eccentricity(),
            frame,
            orbit.mean_anomaly_at(t1),
        )
        .unwrap();
        for seconds in [-9.0e9, -1.0, 0.0, 2.5e8] {
            let (original, _) = orbit.relative_state_at(after(t1, seconds));
            let (moved, _) = shifted.relative_state_at(after(UniverseTime::EPOCH, seconds));
            let (original, moved) = (original.metres(), moved.metres());
            assert!(distance(original, moved) < 1e-11 * dot(original, original).sqrt());
        }
    }

    #[test]
    fn a_hot_jupiter_orbit_keeps_its_place_for_a_hundred_thousand_periods() {
        // P14.T2.a: a one-day orbit at 0.02 au, at t and at t + 10⁵ P, agree to 1 m.
        let o = orientation(0.3, 1.9, 4.4);
        let orbit = orbit_about_the_sun(0.02, 0.05, o, 1.234);
        assert!((orbit.period().value() / 86_400.0 - 1.0).abs() < 0.05);
        for start in [
            UniverseTime::EPOCH,
            at(123_456_789, 987_654_321),
            at(-5_000_000_000, 5),
        ] {
            let later = after(start, 1e5 * orbit.period().value());
            let (r0, _) = orbit.relative_state_at(start);
            let (r1, _) = orbit.relative_state_at(later);
            let gap = distance(r0.metres(), r1.metres());
            assert!(gap < 1.0, "{gap} m apart from {start}");
        }
    }

    #[test]
    fn a_wide_orbit_closes_to_a_millimetre() {
        // P14.T2.a: at 50 au, the position after one period agrees to 1 mm.
        let o = orientation(2.0, 5.5, 0.7);
        let orbit = orbit_about_the_sun(50.0, 0.3, o, 3.0);
        for start in [
            UniverseTime::EPOCH,
            at(31_557_600_000, 0),
            at(-12_345_678_901, 123_456_789),
            at(8_000_000_000_000, 999_999_999),
        ] {
            let later = after(start, orbit.period().value());
            let (r0, _) = orbit.relative_state_at(start);
            let (r1, _) = orbit.relative_state_at(later);
            let gap = distance(r0.metres(), r1.metres());
            assert!(gap <= 1e-3, "{gap} m apart from {start}");
        }
    }

    #[test]
    fn a_one_day_orbit_keeps_its_phase_a_thousand_years_out() {
        // A period of exactly one day: a thousand Julian years is 365,250 periods, so the state
        // there is the state at the epoch, bit for bit, and so on around the orbit.
        let orbit = KeplerElements::from_period(
            Seconds::new(86_400.0),
            sun(),
            Eccentricity::new(0.2).unwrap(),
            orientation(0.5, 1.0, 1.5),
            Radians::new(0.75),
        )
        .unwrap();
        let thousand_years = UniverseTime::from_julian_years(1_000).unwrap();
        let day = Span::from_seconds(86_400);
        for (seconds, nanos) in [
            (0, 0),
            (21_600, 500_000_000),
            (43_200, 0),
            (86_399, 999_999_999),
        ] {
            let offset = Span::new(seconds, nanos).unwrap();
            let before = offset.checked_neg().unwrap();
            let (r0, v0) =
                orbit.relative_state_at(UniverseTime::EPOCH.checked_add(offset).unwrap());
            let (r1, v1) = orbit.relative_state_at(thousand_years.checked_add(offset).unwrap());
            let (r2, _) = orbit.relative_state_at(
                thousand_years
                    .checked_add(before)
                    .unwrap()
                    .checked_sub(day)
                    .unwrap(),
            );
            let (r3, _) = orbit.relative_state_at(UniverseTime::EPOCH.checked_add(before).unwrap());
            for axis in 0..3 {
                assert_same_bits(r1.metres()[axis], r0.metres()[axis]);
                assert_same_bits(v1.metres_per_second()[axis], v0.metres_per_second()[axis]);
                assert_same_bits(r2.metres()[axis], r3.metres()[axis]);
            }
        }
        let phase = orbit
            .mean_anomaly_at(after(thousand_years, 21_600.0))
            .value();
        assert!((phase - (0.75 + TAU / 4.0)).abs() < 1e-15);
    }

    #[test]
    fn energy_and_angular_momentum_match_the_elements() {
        // P14.T2.a: ε = v²/2 − μ/r = −μ/2a and h = r × v = √(μ a (1 − e²)) along the normal.
        let mut lcg = Lcg::new(0x0e1e_0004);
        for _ in 0..5_000 {
            let frame = orientation(
                PI * lcg.next_f64(),
                TAU * lcg.next_f64(),
                TAU * lcg.next_f64(),
            );
            let eccentricity = 0.999 * lcg.next_f64();
            let orbit = KeplerElements::from_semi_major_axis(
                Metres::new(math::exp10(6.0 + 8.0 * lcg.next_f64())),
                GravitationalParameter::new(math::exp10(12.0 + 9.0 * lcg.next_f64())),
                Eccentricity::new(eccentricity).unwrap(),
                frame,
                Radians::new(TAU * lcg.next_f64()),
            )
            .unwrap();
            let time = after(UniverseTime::EPOCH, (lcg.next_f64() - 0.5) * 1e13);
            let (position, velocity) = orbit.relative_state_at(time);
            let (position, velocity) = (position.metres(), velocity.metres_per_second());
            let mu = orbit.gravitational_parameter().value();
            let axis = orbit.semi_major_axis().value();
            let energy = 0.5 * dot(velocity, velocity) - mu / dot(position, position).sqrt();
            let expected = -mu / (2.0 * axis);
            assert!(
                ((energy - expected) / expected).abs() < 1e-12,
                "energy {energy:e} vs {expected:e} at e = {eccentricity}"
            );
            let momentum = cross(position, velocity);
            let expected = (mu * axis * (1.0 - eccentricity) * (1.0 + eccentricity)).sqrt();
            for (component, normal) in momentum.iter().zip(frame.normal()) {
                assert!(
                    (component - expected * normal).abs() < 1e-12 * expected,
                    "h {momentum:?} vs {expected:e} × {:?}",
                    frame.normal()
                );
            }
        }
    }

    #[test]
    fn a_period_and_an_axis_follow_keplers_third_law() {
        let earth = orbit_about_the_sun(1.0, 0.0, orientation(0.0, 0.0, 0.0), 0.0);
        // The Gaussian year: 365.256 898 3 days for a massless body at 1 au.
        assert!((earth.period().value() / 86_400.0 - 365.256_898_3).abs() < 2e-7);
        assert_same_bits(earth.gravitational_parameter().value(), GM_SUN);
        let implied = TAU * TAU * METRES_PER_AU * METRES_PER_AU * METRES_PER_AU
            / (earth.period().value() * earth.period().value());
        assert!((implied / GM_SUN - 1.0).abs() < 1e-15);
        let back = KeplerElements::from_period(
            earth.period(),
            sun(),
            Eccentricity::CIRCULAR,
            *earth.orientation(),
            Radians::ZERO,
        )
        .unwrap();
        assert!((back.semi_major_axis().value() / METRES_PER_AU - 1.0).abs() < 1e-15);
        assert_same_bits(back.period().value(), earth.period().value());
        let orbit = orbit_about_the_sun(2.0, 0.25, orientation(1.0, 2.0, 3.0), -1.0);
        assert!((orbit.periapsis().value() / METRES_PER_AU - 1.5).abs() < 1e-15);
        assert!((orbit.apoapsis().value() / METRES_PER_AU - 2.5).abs() < 1e-15);
        assert!((orbit.mean_anomaly_at_epoch().value() - (TAU - 1.0)).abs() < 1e-15);
        assert_same_bits(
            orbit.mean_anomaly_at(UniverseTime::EPOCH).value(),
            TAU - 1.0,
        );
        assert_same_bits(orbit.inclination().value(), 1.0);
        assert_same_bits(orbit.ascending_node().value(), 2.0);
        assert_same_bits(orbit.argument_of_periapsis().value(), 3.0);
    }

    #[test]
    fn scaling_expands_an_orbit_and_keeps_its_angular_momentum() {
        let orbit = orbit_about_the_sun(3.0, 0.4, orientation(0.7, 0.2, 4.0), 2.2);
        // The star loses 40% of its mass: a grows by 1 ÷ 0.6 and μ falls to 0.6 μ.
        let expanded = orbit
            .scaled(
                1.0 / 0.6,
                GravitationalParameter::from_solar_masses(SolarMasses::new(0.6)),
            )
            .unwrap();
        assert!((expanded.semi_major_axis() / orbit.semi_major_axis() - 1.0 / 0.6).abs() < 1e-15);
        assert_eq!(expanded.eccentricity(), orbit.eccentricity());
        assert_eq!(expanded.orientation(), orbit.orientation());
        assert_eq!(
            expanded.mean_anomaly_at_epoch(),
            orbit.mean_anomaly_at_epoch()
        );
        let ratio = expanded.period() / orbit.period();
        let growth = 1.0 / 0.6;
        let expected = (growth * growth * growth / 0.6_f64).sqrt();
        assert!(
            (ratio / expected - 1.0).abs() < 1e-14,
            "{ratio} vs {expected}"
        );
        let h = |k: &KeplerElements| {
            let e = k.eccentricity().value();
            (k.gravitational_parameter().value()
                * k.semi_major_axis().value()
                * (1.0 - e)
                * (1.0 + e))
                .sqrt()
        };
        assert!((h(&expanded) / h(&orbit) - 1.0).abs() < 1e-14);
        assert_eq!(
            orbit.scaled(0.0, sun()),
            Err(BuildOrbitError::ScaleFactorNotPositive { factor: 0.0 })
        );
    }

    #[test]
    fn invalid_elements_are_refused() {
        let o = orientation(0.0, 0.0, 0.0);
        let e = Eccentricity::CIRCULAR;
        assert_eq!(
            Eccentricity::new(1.0),
            Err(BuildOrbitError::EccentricityOutOfRange { value: 1.0 })
        );
        assert_eq!(
            Eccentricity::new(-0.1),
            Err(BuildOrbitError::EccentricityOutOfRange { value: -0.1 })
        );
        assert!(matches!(
            Eccentricity::new(f64::NAN),
            Err(BuildOrbitError::EccentricityOutOfRange { .. })
        ));
        assert_same_bits(Eccentricity::new(-0.0).unwrap().value(), 0.0);
        assert_eq!(
            KeplerElements::from_semi_major_axis(Metres::new(0.0), sun(), e, o, Radians::ZERO),
            Err(BuildOrbitError::SemiMajorAxisNotPositive { metres: 0.0 })
        );
        assert_eq!(
            KeplerElements::from_semi_major_axis(
                Metres::new(1.0),
                GravitationalParameter::new(-1.0),
                e,
                o,
                Radians::ZERO
            ),
            Err(BuildOrbitError::GravitationalParameterNotPositive { value: -1.0 })
        );
        assert!(matches!(
            KeplerElements::from_semi_major_axis(
                Metres::new(1.0),
                sun(),
                e,
                o,
                Radians::new(f64::INFINITY)
            ),
            Err(BuildOrbitError::AngleNotFinite { .. })
        ));
        assert_eq!(
            KeplerElements::from_semi_major_axis(Metres::new(1e300), sun(), e, o, Radians::ZERO),
            Err(BuildOrbitError::PeriodNotPositive {
                seconds: f64::INFINITY
            })
        );
        assert_eq!(
            KeplerElements::from_period(Seconds::new(-5.0), sun(), e, o, Radians::ZERO),
            Err(BuildOrbitError::PeriodNotPositive { seconds: -5.0 })
        );
    }

    #[test]
    fn the_same_elements_and_time_give_the_same_bits() {
        let build = || orbit_about_the_sun(5.2, 0.048, orientation(0.02, 1.75, 4.78), 0.35);
        let t = at(-31_557_600 * 37, 42);
        let (r1, v1) = build().relative_state_at(t);
        let (r2, v2) = build().relative_state_at(t);
        for axis in 0..3 {
            assert_same_bits(r1.metres()[axis], r2.metres()[axis]);
            assert_same_bits(v1.metres_per_second()[axis], v2.metres_per_second()[axis]);
        }
    }
}
