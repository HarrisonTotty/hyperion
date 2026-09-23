//! The orbit of a relative state: the inverse of the propagation (plan 14, P14.T2.c).
//!
//! Plan 14's T28.c needs it when a supernova changes a star's mass and velocity at a known time:
//! each planet's state then is known in closed form, and its new orbit follows from the two-body
//! energy and angular momentum (D11). The result may be bound or not.

use std::error::Error;
use std::fmt;

use super::kepler::KeplerElements;
use super::open::{NEAR_PARABOLIC_BAND, OpenOrbit};
use super::orientation::Orientation;
use super::{BuildOrbitError, Eccentricity, cross, dot, sin_deficit, sinh_excess, stumpff};
use crate::coords::{SystemVector, SystemVelocity};
use crate::math;
use crate::time::{Span, UniverseTime};
use crate::units::{GravitationalParameter, Metres, Radians};

/// A relative orbit of either kind: what [`elements_from_state`] returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orbit {
    /// A bound orbit with eccentricity below [`OpenOrbit::MIN_ECCENTRICITY`].
    Bound(KeplerElements),
    /// An unbound orbit, or a bound one so nearly parabolic that it is described by its pericentre.
    Open(OpenOrbit),
}

impl Orbit {
    /// The position and velocity of the secondary relative to the primary at `t`, by whichever
    /// kind this is.
    #[must_use]
    pub fn relative_state_at(&self, t: UniverseTime) -> (SystemVector, SystemVelocity) {
        match self {
            Self::Bound(elements) => elements.relative_state_at(t),
            Self::Open(open) => open.relative_state_at(t),
        }
    }
}

/// A relative state has no orbit that [`elements_from_state`] can describe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InvertStateError {
    /// A component of the position or velocity was not finite.
    NotFinite,
    /// The gravitational parameter was not finite and positive.
    GravitationalParameterNotPositive {
        /// The offending value, m³ s⁻².
        value: f64,
    },
    /// The angular momentum was zero: a position or velocity of zero, or motion along the line of
    /// centres, a rectilinear orbit that no element set describes.
    Radial,
    /// The time of pericentre of an open orbit falls outside the universe clock.
    PericentreTimeOutOfRange,
    /// The state is valid but its orbit is not representable: its period, mean motion or
    /// pericentre overflows or underflows, as for a gravitational parameter of 10⁻³⁰⁰ m³ s⁻².
    Unrepresentable(BuildOrbitError),
}

impl fmt::Display for InvertStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite => f.write_str("a component of the state is not finite"),
            Self::GravitationalParameterNotPositive { value } => write!(
                f,
                "gravitational parameter {value} m³ s⁻² is not finite and positive"
            ),
            Self::Radial => f.write_str("the state has no angular momentum"),
            Self::PericentreTimeOutOfRange => {
                f.write_str("the time of pericentre is outside the universe clock")
            }
            Self::Unrepresentable(_) => f.write_str("the state's orbit is not representable"),
        }
    }
}

impl Error for InvertStateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unrepresentable(error) => Some(error),
            Self::NotFinite
            | Self::GravitationalParameterNotPositive { .. }
            | Self::Radial
            | Self::PericentreTimeOutOfRange => None,
        }
    }
}

/// The orbit on which the secondary is at relative position `r` with relative velocity `v` at
/// time `t`, under gravitational parameter `mu`: the inverse of
/// [`KeplerElements::relative_state_at`] and [`OpenOrbit::relative_state_at`] (plan 14,
/// P14.T2.c).
///
/// An eccentricity below [`OpenOrbit::MIN_ECCENTRICITY`] gives [`Orbit::Bound`], anything else
/// [`Orbit::Open`]. The angles follow the module's conventions. Where one is undefined it is
/// chosen: at inclination 0 or π the ascending node is 0 and the argument of periapsis carries the
/// longitude of periapsis; on a circular orbit the periapsis is put at the ascending node. The
/// anomaly is measured from the periapsis so chosen, so the state is reproduced whatever the
/// choice.
///
/// The formulation (the angular momentum h = r × v, the eccentricity vector (v × h) ÷ μ − r̂, the
/// orientation from h and the eccentricity vector, then the anomaly from the position's
/// components in the orbit's plane) is the standard one; see Vallado (2013, *Fundamentals of
/// Astrodynamics and Applications*, algorithm 9). A bound orbit's semi-major axis comes from the
/// energy, and an open one's pericentre from h² ÷ μ (1 + e), which has no cancellation at e = 1.
///
/// # Errors
///
/// - [`InvertStateError::NotFinite`] if a component of `r` or `v` is not finite, or so large
///   that r × v overflows.
/// - [`InvertStateError::GravitationalParameterNotPositive`] unless `mu` is finite and
///   positive.
/// - [`InvertStateError::Radial`] if r × v is zero.
/// - [`InvertStateError::PericentreTimeOutOfRange`] if an open orbit's pericentre time would
///   leave the universe clock, some 2.9 × 10¹¹ years away.
/// - [`InvertStateError::Unrepresentable`] if the orbit's period, mean motion or pericentre
///   overflows or underflows, which only extreme scales cause.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{SystemVector, SystemVelocity};
/// use hyperion_sim::orbit::{Orbit, elements_from_state};
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{GravitationalParameter, SolarMasses};
///
/// let sun = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
/// let r = SystemVector::new([1.496e11, 0.0, 0.0]);
/// // At 29.8 km/s a body 1 au out is on a nearly circular orbit.
/// let orbit = elements_from_state(r, SystemVelocity::new([0.0, 29_800.0, 0.0]), sun, UniverseTime::EPOCH)?;
/// assert!(matches!(orbit, Orbit::Bound(k) if k.eccentricity().value() < 0.01));
/// // At 45 km/s it leaves the Solar System.
/// let orbit = elements_from_state(r, SystemVelocity::new([0.0, 45_000.0, 0.0]), sun, UniverseTime::EPOCH)?;
/// assert!(matches!(orbit, Orbit::Open(o) if o.eccentricity() > 1.0));
/// # Ok::<(), hyperion_sim::orbit::InvertStateError>(())
/// ```
pub fn elements_from_state(
    r: SystemVector,
    v: SystemVelocity,
    mu: GravitationalParameter,
    t: UniverseTime,
) -> Result<Orbit, InvertStateError> {
    let (position, velocity, mu) = (r.metres(), v.metres_per_second(), mu.value());
    if !position.iter().chain(&velocity).all(|c| c.is_finite()) {
        return Err(InvertStateError::NotFinite);
    }
    if !(mu.is_finite() && mu > 0.0) {
        return Err(InvertStateError::GravitationalParameterNotPositive { value: mu });
    }
    let radius = dot(position, position).sqrt();
    let momentum = cross(position, velocity);
    let momentum_length = dot(momentum, momentum).sqrt();
    let v_cross_h = cross(velocity, momentum);
    let eccentricity_vector = [
        v_cross_h[0] / mu - position[0] / radius,
        v_cross_h[1] / mu - position[1] / radius,
        v_cross_h[2] / mu - position[2] / radius,
    ];
    let eccentricity = dot(eccentricity_vector, eccentricity_vector).sqrt();
    if !(momentum_length.is_finite() && eccentricity.is_finite()) {
        // Finite components whose products overflow, or a zero radius with motion.
        return Err(if momentum_length > 0.0 && radius > 0.0 {
            InvertStateError::NotFinite
        } else {
            InvertStateError::Radial
        });
    }
    if !(momentum_length > 0.0 && radius > 0.0) {
        return Err(InvertStateError::Radial);
    }
    let orientation = orientation_of(momentum, momentum_length, eccentricity_vector, eccentricity);
    let in_plane = orientation.system_to_plane(position);

    let axis = mu * radius / (2.0 * mu - radius * dot(velocity, velocity));
    if eccentricity < OpenOrbit::MIN_ECCENTRICITY && axis.is_finite() && axis > 0.0 {
        return bound_orbit(axis, eccentricity, mu, &orientation, in_plane, t)
            .map(Orbit::Bound)
            .map_err(InvertStateError::Unrepresentable);
    }
    let pericentre = momentum_length * momentum_length / (mu * (1.0 + eccentricity));
    if !(pericentre.is_finite() && pericentre > 0.0) {
        return Err(InvertStateError::Unrepresentable(
            BuildOrbitError::PericentreNotPositive { metres: pericentre },
        ));
    }
    // An eccentricity just under the open floor can only arrive here with a non-positive
    // energy-based axis, a rounding artefact; its pericentre describes it better.
    let eccentricity = eccentricity.max(OpenOrbit::MIN_ECCENTRICITY);
    let since_pericentre = time_since_pericentre(pericentre, eccentricity, mu, in_plane);
    let pericentre_time = Span::from_seconds_f64(since_pericentre)
        .and_then(|span| t.checked_sub(span))
        .ok_or(InvertStateError::PericentreTimeOutOfRange)?;
    OpenOrbit::new(
        Metres::new(pericentre),
        eccentricity,
        orientation,
        pericentre_time,
        GravitationalParameter::new(mu),
    )
    .map(Orbit::Open)
    .map_err(InvertStateError::Unrepresentable)
}

/// The orientation of the plane normal to `momentum` with periapsis along
/// `eccentricity_vector`, or at the ascending node when the orbit is circular.
fn orientation_of(
    momentum: [f64; 3],
    momentum_length: f64,
    eccentricity_vector: [f64; 3],
    eccentricity: f64,
) -> Orientation {
    let normal = momentum.map(|component| component / momentum_length);
    let sin_inclination = math::hypot(normal[0], normal[1]);
    let inclination = math::atan2(sin_inclination, normal[2]);
    let node = if sin_inclination > 0.0 {
        math::atan2(normal[0], -normal[1])
    } else {
        0.0
    };
    let (sin_node, cos_node) = math::sin_cos(node);
    let towards_node = [cos_node, sin_node, 0.0];
    let ahead_of_node = cross(normal, towards_node);
    let periapsis = if eccentricity > 0.0 {
        eccentricity_vector
    } else {
        towards_node
    };
    let argument = math::atan2(dot(periapsis, ahead_of_node), dot(periapsis, towards_node));
    Orientation::new(
        Radians::new(inclination),
        Radians::new(node),
        Radians::new(argument),
    )
    .expect("atan2 of finite values with a non-negative sine gives angles in range")
}

/// The bound orbit of semi-major axis `axis` and eccentricity `eccentricity` whose position in
/// the plane of `orientation` is `in_plane` (along P, along Q) at `t`.
fn bound_orbit(
    axis: f64,
    eccentricity: f64,
    mu: f64,
    orientation: &Orientation,
    in_plane: (f64, f64),
    t: UniverseTime,
) -> Result<KeplerElements, BuildOrbitError> {
    let (along_p, along_q) = in_plane;
    let e = eccentricity;
    let axis_ratio = ((1.0 - e) * (1.0 + e)).sqrt();
    let anomaly = math::atan2(along_q / axis_ratio, along_p + axis * e);
    let mean_anomaly = (1.0 - e) * anomaly + e * sin_deficit(anomaly, math::sin(anomaly));
    let eccentricity = Eccentricity::new(e)?;
    let build = |mean_anomaly_at_epoch: f64| {
        KeplerElements::from_semi_major_axis(
            Metres::new(axis),
            GravitationalParameter::new(mu),
            eccentricity,
            *orientation,
            Radians::new(mean_anomaly_at_epoch),
        )
    };
    // The phase the orbit gains between the epoch and t, by the very reduction propagation uses,
    // so that the elements reproduce the state at t.
    let gained = build(0.0)?.mean_anomaly_at(t).value();
    build(mean_anomaly - gained)
}

/// Seconds from the pericentre passage nearest in phase to the state at `in_plane` (along P,
/// along Q) on the open orbit of pericentre `pericentre` and eccentricity `eccentricity`:
/// negative before it.
fn time_since_pericentre(pericentre: f64, eccentricity: f64, mu: f64, in_plane: (f64, f64)) -> f64 {
    let (along_p, along_q) = in_plane;
    let (q, e) = (pericentre, eccentricity);
    if (e - 1.0).abs() < NEAR_PARABOLIC_BAND {
        // The universal variable in closed form: with y′ = along Q ÷ √(q (1 + e)), √α χ is the
        // eccentric anomaly E = atan2(√α y′, 1 − α (q − along P)) on an ellipse and the
        // hyperbolic anomaly H = asinh(√−α y′) on a hyperbola, and χ = y′ on the parabola. The
        // arctangent sees both components, so a state beyond r = a keeps its branch.
        let alpha = (1.0 - e) / q;
        let target = along_q / (q * (1.0 + e)).sqrt();
        let chi = if alpha > 0.0 {
            let root = alpha.sqrt();
            math::atan2(root * target, 1.0 - alpha * (q - along_p)) / root
        } else if alpha < 0.0 {
            let root = (-alpha).sqrt();
            math::asinh(root * target) / root
        } else {
            target
        };
        let (_, stumpff_s) = stumpff(alpha * chi * chi);
        (q * chi + e * chi * chi * chi * stumpff_s) / mu.sqrt()
    } else if e < 1.0 {
        let axis = q / (1.0 - e);
        let axis_ratio = ((1.0 - e) * (1.0 + e)).sqrt();
        let anomaly = math::atan2(along_q / axis_ratio, along_p + axis * e);
        let mean_anomaly = (1.0 - e) * anomaly + e * sin_deficit(anomaly, math::sin(anomaly));
        mean_anomaly * (axis * (axis / mu).sqrt())
    } else {
        let semi_axis = q / (e - 1.0);
        let axis_ratio = ((e - 1.0) * (e + 1.0)).sqrt();
        let sinh = along_q / (semi_axis * axis_ratio);
        let anomaly = math::asinh(sinh);
        let mean_anomaly = (e - 1.0) * anomaly + e * sinh_excess(anomaly, sinh);
        mean_anomaly * (semi_axis * (semi_axis / mu).sqrt())
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{PI, TAU};

    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::units::consts::{GM_SUN, METRES_PER_AU};

    fn orientation(i: f64, node: f64, argument: f64) -> Orientation {
        Orientation::new(Radians::new(i), Radians::new(node), Radians::new(argument)).unwrap()
    }

    fn at(seconds: i64, nanos: u32) -> UniverseTime {
        UniverseTime::new(seconds, nanos).unwrap()
    }

    fn relative_gap(a: [f64; 3], b: [f64; 3]) -> f64 {
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        (dot(d, d) / dot(a, a)).sqrt()
    }

    fn assert_reproduces(
        orbit: &Orbit,
        r: SystemVector,
        v: SystemVelocity,
        t: UniverseTime,
        tolerance: f64,
    ) {
        let (r2, v2) = orbit.relative_state_at(t);
        let (dr, dv) = (
            relative_gap(r.metres(), r2.metres()),
            relative_gap(v.metres_per_second(), v2.metres_per_second()),
        );
        assert!(
            dr < tolerance && dv < tolerance,
            "position {dr:e}, velocity {dv:e} relative, {orbit:?}"
        );
    }

    #[test]
    fn ten_thousand_bound_states_round_trip_to_a_picometre_per_metre() {
        // P14.T2.c: state → elements → state at the same time, to 1e-12 relative.
        let mut lcg = Lcg::new(0x0e1e_0006);
        for n in 0..10_000 {
            let inclination = match n % 7 {
                0 => 0.0,
                1 => PI,
                _ => PI * lcg.next_f64(),
            };
            let eccentricity = match n % 5 {
                0 => 0.0,
                1 => 1e-9 * lcg.next_f64(),
                _ => 0.99 * lcg.next_f64(),
            };
            let original = KeplerElements::from_semi_major_axis(
                Metres::new(math::exp10(6.0 + 8.0 * lcg.next_f64())),
                GravitationalParameter::new(math::exp10(12.0 + 9.0 * lcg.next_f64())),
                Eccentricity::new(eccentricity).unwrap(),
                orientation(inclination, TAU * lcg.next_f64(), TAU * lcg.next_f64()),
                Radians::new(TAU * lcg.next_f64()),
            )
            .unwrap();
            let time = UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64((lcg.next_f64() - 0.5) * 1e12).unwrap())
                .unwrap();
            let (position, velocity) = original.relative_state_at(time);
            let mu = original.gravitational_parameter();
            let orbit = elements_from_state(position, velocity, mu, time).unwrap();
            let Orbit::Bound(recovered) = orbit else {
                panic!("a bound state of e = {eccentricity} gave {orbit:?}");
            };
            assert_reproduces(&orbit, position, velocity, time, 1e-12);
            let axis_ratio = recovered.semi_major_axis() / original.semi_major_axis();
            assert!((axis_ratio - 1.0).abs() < 1e-12, "a off by {axis_ratio}");
            assert!((recovered.eccentricity().value() - eccentricity).abs() < 1e-12);
            assert!((recovered.inclination().value() - inclination).abs() < 1e-7);
        }
    }

    #[test]
    fn recovered_elements_propagate_like_the_originals() {
        let original = KeplerElements::from_semi_major_axis(
            Metres::new(2.2 * METRES_PER_AU),
            GravitationalParameter::new(GM_SUN),
            Eccentricity::new(0.37).unwrap(),
            orientation(0.9, 4.1, 2.6),
            Radians::new(5.0),
        )
        .unwrap();
        let t = at(-987_654_321, 123);
        let (r, v) = original.relative_state_at(t);
        let orbit = elements_from_state(r, v, original.gravitational_parameter(), t).unwrap();
        let Orbit::Bound(recovered) = orbit else {
            panic!("bound state gave {orbit:?}");
        };
        assert!((recovered.ascending_node().value() - 4.1).abs() < 1e-12);
        assert!((recovered.argument_of_periapsis().value() - 2.6).abs() < 1e-11);
        assert!((recovered.mean_anomaly_at_epoch().value() - 5.0).abs() < 1e-11);
        for seconds in [-3e9, 0.0, 1e7, 2.5e10] {
            let later = UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64(seconds).unwrap())
                .unwrap();
            let (r1, _) = original.relative_state_at(later);
            let (r2, _) = recovered.relative_state_at(later);
            assert!(relative_gap(r1.metres(), r2.metres()) < 1e-10, "at {later}");
        }
    }

    #[test]
    fn an_unbound_state_returns_an_open_orbit() {
        // P14.T2.c: hyperbolic, parabolic and nearly parabolic states give an `OpenOrbit` that
        // reproduces them.
        let mut lcg = Lcg::new(0x0e1e_0007);
        let mu = GravitationalParameter::new(GM_SUN);
        for n in 0..3_000 {
            let e = match n % 4 {
                0 => 1.0 + math::exp10(-5.5 + 7.5 * lcg.next_f64()),
                1 => 1.0 + 1.8e-6 * (lcg.next_f64() - 0.5),
                2 => 1.0 - math::exp10(-4.0 - 1.9 * lcg.next_f64()),
                _ => 1.0 + 1e-6 * (1.0 + 0.5 * lcg.next_f64()),
            };
            let original = OpenOrbit::new(
                Metres::new(math::exp10(9.0 + 3.0 * lcg.next_f64())),
                e,
                orientation(
                    PI * lcg.next_f64(),
                    TAU * lcg.next_f64(),
                    TAU * lcg.next_f64(),
                ),
                at(2_000_000, 0),
                mu,
            )
            .unwrap();
            let t = UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64((lcg.next_f64() - 0.5) * 2e9).unwrap())
                .unwrap();
            let (r, v) = original.relative_state_at(t);
            let orbit = elements_from_state(r, v, mu, t).unwrap();
            let Orbit::Open(recovered) = orbit else {
                panic!("an open state of e = {e} gave {orbit:?}");
            };
            // Far out on a hyperbola r and v are nearly parallel and r × v loses about
            // 10⁻¹⁶ ÷ sin(r, v) of itself: a state's own conditioning, not the inversion's.
            assert!(
                (recovered.eccentricity() / e - 1.0).abs() < 1e-9,
                "e {e} came back as {}",
                recovered.eccentricity()
            );
            assert!((recovered.pericentre() / original.pericentre() - 1.0).abs() < 1e-9);
            assert_reproduces(&orbit, r, v, t, 1e-9);
        }
    }

    #[test]
    fn a_bound_state_in_the_band_keeps_its_branch_beyond_the_semi_major_axis() {
        // e within 10⁻⁶ of 1 and bound, at phases where r exceeds a (|E| > π/2), before and
        // after pericentre.
        let mu = GravitationalParameter::new(GM_SUN);
        let (pericentre, eccentricity) = (1e9, 1.0 - 5e-7);
        let original = OpenOrbit::new(
            Metres::new(pericentre),
            eccentricity,
            orientation(2.2, 0.3, 4.4),
            at(-5_000, 0),
            mu,
        )
        .unwrap();
        let axis = pericentre / (1.0 - eccentricity);
        let period = TAU * (axis * (axis / GM_SUN).sqrt());
        for periods in [0.01, 0.3, 0.45, -0.4, 1.35] {
            let time = UniverseTime::EPOCH
                .checked_add(Span::from_seconds_f64(periods * period).unwrap())
                .unwrap();
            let (position, velocity) = original.relative_state_at(time);
            let orbit = elements_from_state(position, velocity, mu, time).unwrap();
            let Orbit::Open(recovered) = orbit else {
                panic!("a band state gave {orbit:?}");
            };
            assert!((recovered.eccentricity() - eccentricity).abs() < 1e-12);
            assert_reproduces(&orbit, position, velocity, time, 1e-9);
        }
    }

    #[test]
    fn a_state_with_no_angular_momentum_has_no_orbit() {
        let mu = GravitationalParameter::new(GM_SUN);
        let t = UniverseTime::EPOCH;
        let r = SystemVector::new([1e11, 0.0, 0.0]);
        assert_eq!(
            elements_from_state(r, SystemVelocity::new([-3e4, 0.0, 0.0]), mu, t),
            Err(InvertStateError::Radial)
        );
        assert_eq!(
            elements_from_state(r, SystemVelocity::ZERO, mu, t),
            Err(InvertStateError::Radial)
        );
        assert_eq!(
            elements_from_state(
                SystemVector::ZERO,
                SystemVelocity::new([0.0, 1.0, 0.0]),
                mu,
                t
            ),
            Err(InvertStateError::Radial)
        );
        assert_eq!(
            elements_from_state(r, SystemVelocity::new([0.0, f64::NAN, 0.0]), mu, t),
            Err(InvertStateError::NotFinite)
        );
        assert_eq!(
            elements_from_state(
                r,
                SystemVelocity::new([0.0, 1.0, 0.0]),
                GravitationalParameter::ZERO,
                t
            ),
            Err(InvertStateError::GravitationalParameterNotPositive { value: 0.0 })
        );
        assert_eq!(
            InvertStateError::Radial.to_string(),
            "the state has no angular momentum"
        );
        // A valid state whose period overflows: a tiny gravitational parameter.
        let error = elements_from_state(
            SystemVector::new([1e10, 0.0, 0.0]),
            SystemVelocity::new([0.0, 1e-155, 0.0]),
            GravitationalParameter::new(1e-300),
            t,
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                InvertStateError::Unrepresentable(BuildOrbitError::PeriodNotPositive { .. })
            ),
            "{error:?}"
        );
        assert!(Error::source(&error).is_some());
    }

    #[test]
    fn a_circular_equatorial_orbit_puts_its_periapsis_on_the_x_axis() {
        let mu = GravitationalParameter::new(GM_SUN);
        let r = SystemVector::new([0.0, METRES_PER_AU, 0.0]);
        let speed = (GM_SUN / METRES_PER_AU).sqrt();
        let orbit = elements_from_state(
            r,
            SystemVelocity::new([-speed, 0.0, 0.0]),
            mu,
            UniverseTime::EPOCH,
        )
        .unwrap();
        let Orbit::Bound(k) = orbit else {
            panic!("{orbit:?}");
        };
        assert!(k.eccentricity().value() < 1e-15);
        assert!(k.inclination().value().abs() < 1e-300);
        assert!(k.ascending_node().value().abs() < 1e-300);
        // Whatever the periapsis, the body is a quarter-turn round from it.
        let longitude = k.argument_of_periapsis().value() + k.mean_anomaly_at_epoch().value();
        assert!((longitude.rem_euclid(TAU) - PI / 2.0).abs() < 1e-7);
    }
}
