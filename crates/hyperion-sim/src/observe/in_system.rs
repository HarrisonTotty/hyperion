//! What an observer inside a system sees of the system's bodies and stars: their apparent
//! positions, light time and aberration together (rendering plan R03, Design note 7).
//!
//! The galactic functions beside this module ([`retarded`](super::retarded)) take one light-time
//! step for a star thousands of light-years off. Inside a system the light time is seconds to
//! hours, the sources move fast against it, and a ship's own velocity bends what it sees, so the
//! evaluation here differs in three ways:
//!
//! - the light time is iterated to a fixed point, τ = |`x_B`(t − τ) − `x_o`(t)| ÷ c, until a
//!   correction changes it by at most [`IN_SYSTEM_LIGHT_TIME_TOLERANCE`], or, from the second
//!   correction on, by no less than the one before (rounding noise far out), and refused past
//!   [`IN_SYSTEM_MAX_CORRECTIONS`];
//! - the observer enters only at reception: its position and velocity at the observed time t, as
//!   SPICE's converged "CN+S" correction and NOVAS take them;
//! - the apparent point is the emission event Lorentz-boosted into the observer's rest frame, the
//!   exact special-relativistic aberration, which costs one square root.
//!
//! Positions are [`SystemPosition`]s, metres from the barycentre along the galactic axes, and
//! velocities [`SystemVelocity`]s relative to the barycentre. The light-time solve stays Newtonian
//! in the system frame, as SPICE's and NOVAS's do; gravitational deflection is omitted, as SPICE
//! omits it.
//!
//! Sources:
//!
//! - NAIF, SPICE Toolkit: `spkezr_c` and `spkaps_c` (the observer's state at the observation epoch
//!   for the light time and the stellar aberration, and the converged "CN" iteration), `stelab_c`
//!   (stellar aberration), and the Aberration Corrections Required Reading, eq. 1.
//! - Bangert, J., Puatua, W., Kaplan, G., Bartlett, J., Harris, W., Fredericks, A. and Monet, A.
//!   2011, _User's Guide to NOVAS Version C3.1_, U.S. Naval Observatory, pp. C-16–C-17.
//! - Jackson, J. D. 1999, _Classical Electrodynamics_, 3rd ed., Wiley, §11.3, eq. 11.19: the Lorentz
//!   transformation for a boost in an arbitrary direction, passive form.
//! - Urban, S. E. and Seidelmann, P. K. (eds.) 2013, _Explanatory Supplement to the Astronomical
//!   Almanac_, 3rd ed., University Science Books, §7.2.3 "Aberration", pp. 263–269 (from the
//!   printed contents).
//!
//! SPICE's `stelab_c` is classical (sin φ = v sin w ÷ c, relativistic effects excluded), so it
//! agrees with the exact boost here only to first order in β and is no bit-level oracle.
//!
//! Nothing here changes generated output: observation only reads.

use std::error::Error;
use std::fmt;

use crate::coords::{SystemPosition, SystemVector, SystemVelocity};
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::SPEED_OF_LIGHT;

use super::retarded::{before, light_time};

mod tracks;

pub use tracks::{BodyTrack, StarTrack};

/// The change in the light time at which the iteration stops, inclusive: one nanosecond.
///
/// The light time is rounded to the nanosecond ([`light_time`]), so a strict test could cycle
/// between two adjacent nanoseconds; an inclusive one stops there.
pub const IN_SYSTEM_LIGHT_TIME_TOLERANCE: Span = match Span::new(0, 1) {
    Ok(span) => span,
    Err(_) => panic!("one nanosecond is a span"),
};

/// The most corrections [`retarded_in_system`] makes before it refuses: 10.
///
/// For uniform radial motion the k-th correction changes the light time by τ₀ `β_r`^k, `β_r` the
/// source's radial speed over c, so two to four corrections are typical within a system's
/// planets, five for a hot Jupiter seen from 100 au (250 km/s) and seven, or eight, at a system's
/// reach (2.7 × 10⁵ au, 800 km/s); SPICE's three iterations are for Solar System speeds (R03, Design
/// note 7).
pub const IN_SYSTEM_MAX_CORRECTIONS: u8 = 10;

/// A [`SystemObserver`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSystemObserverError {
    /// A component of the position or the velocity is not finite.
    NotFinite,
    /// The speed is not below that of light, for which the aberration has no Lorentz factor.
    NotSlowerThanLight,
    /// The time lies outside the [`ClockWindow`], ±1,000 Julian years about the epoch.
    TimeOutsideClockWindow(UniverseTime),
}

impl fmt::Display for BuildSystemObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite => f.write_str("the observer's position or velocity is not finite"),
            Self::NotSlowerThanLight => {
                f.write_str("the observer's speed is not below that of light")
            }
            Self::TimeOutsideClockWindow(t) => {
                write!(f, "the observer's time {t} lies outside the clock window")
            }
        }
    }
}

impl Error for BuildSystemObserverError {}

/// Someone inside a system who receives light: a position and a velocity in the system's frame at
/// a time inside the clock window.
///
/// Until sessions exist the scene's ship stand-in is one (R03, Design note 2); the observer is
/// always the ship, never a camera.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{SystemPosition, SystemVelocity};
/// use hyperion_sim::observe::{BuildSystemObserverError, SystemObserver};
/// use hyperion_sim::time::UniverseTime;
///
/// // A ship 1 au from the barycentre, moving at 30 km/s.
/// let ship = SystemObserver::new(
///     SystemPosition::new([1.496e11, 0.0, 0.0]),
///     SystemVelocity::new([0.0, 3.0e4, 0.0]),
///     UniverseTime::EPOCH,
/// )?;
/// assert_eq!(ship.time(), UniverseTime::EPOCH);
///
/// // Nothing outruns its own light.
/// assert_eq!(
///     SystemObserver::new(
///         SystemPosition::ORIGIN,
///         SystemVelocity::new([3.0e8, 0.0, 0.0]),
///         UniverseTime::EPOCH,
///     ),
///     Err(BuildSystemObserverError::NotSlowerThanLight)
/// );
/// # Ok::<(), BuildSystemObserverError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemObserver {
    position: SystemPosition,
    velocity: SystemVelocity,
    time: UniverseTime,
}

impl SystemObserver {
    /// The observer at `position`, moving at `velocity` relative to the barycentre, at `time`.
    ///
    /// # Errors
    ///
    /// [`BuildSystemObserverError::NotFinite`] for a component that is not finite,
    /// [`BuildSystemObserverError::NotSlowerThanLight`] for a speed of c or more, and
    /// [`BuildSystemObserverError::TimeOutsideClockWindow`] for a time outside ±H.
    pub fn new(
        position: SystemPosition,
        velocity: SystemVelocity,
        time: UniverseTime,
    ) -> Result<Self, BuildSystemObserverError> {
        let finite = position
            .metres()
            .iter()
            .chain(velocity.metres_per_second().iter())
            .all(|component| component.is_finite());
        if !finite {
            return Err(BuildSystemObserverError::NotFinite);
        }
        if velocity.dot(&velocity) >= SPEED_OF_LIGHT * SPEED_OF_LIGHT {
            return Err(BuildSystemObserverError::NotSlowerThanLight);
        }
        if !ClockWindow::contains(time) {
            return Err(BuildSystemObserverError::TimeOutsideClockWindow(time));
        }
        Ok(Self {
            position,
            velocity,
            time,
        })
    }

    /// Where the observer is at its present, in the system's frame.
    #[must_use]
    pub const fn position(&self) -> &SystemPosition {
        &self.position
    }

    /// The observer's velocity at its present, relative to the barycentre, m/s.
    #[must_use]
    pub const fn velocity(&self) -> SystemVelocity {
        self.velocity
    }

    /// The observer's present, on the universe clock.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }
}

/// A position in a system's frame that is a pure function of time (the rendering brainstorm's
/// `SystemTrajectory`, "The floating origin is already in the simulation").
///
/// A body's or a star's track through its system, which plan 14's orbits and plan 11's hierarchy
/// give.
pub trait SystemTrajectory {
    /// Where the source is at `t`, in metres from the barycentre along the galactic axes, or
    /// `None` if it is not present then (not yet formed, or destroyed). Every component is
    /// finite: [`retarded_in_system`] panics on a position that is not.
    fn position_at(&self, t: UniverseTime) -> Option<SystemPosition>;

    /// The source's velocity at `t` relative to the barycentre, m/s, or `None` if it is not
    /// present then.
    fn velocity_at(&self, t: UniverseTime) -> Option<SystemVelocity>;
}

/// What a [`SystemObserver`] sees of a source: when its light left, where the source was then and
/// where it appears.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InSystemRetardation {
    observed: UniverseTime,
    emitted: UniverseTime,
    light_time: Span,
    corrections: u8,
    geometric_then: SystemPosition,
    apparent: SystemPosition,
    residual: Span,
}

impl InSystemRetardation {
    /// The observer's time: when the light arrives.
    #[must_use]
    pub const fn observed(&self) -> UniverseTime {
        self.observed
    }

    /// When the light left the source, `observed − light_time`.
    #[must_use]
    pub const fn emitted(&self) -> UniverseTime {
        self.emitted
    }

    /// The light time τ, to the nanosecond: |`x_B`(emitted) − `x_o`(observed)| ÷ c, rounded.
    #[must_use]
    pub const fn light_time(&self) -> Span {
        self.light_time
    }

    /// How many corrections the iteration made after its starting guess from the present
    /// distance, the confirming one counted: at least 1 and at most
    /// [`IN_SYSTEM_MAX_CORRECTIONS`].
    #[must_use]
    pub const fn corrections(&self) -> u8 {
        self.corrections
    }

    /// Where the source was at the emitted time, in the system's frame: the light-time point,
    /// before aberration.
    #[must_use]
    pub const fn geometric_then(&self) -> &SystemPosition {
        &self.geometric_then
    }

    /// Where the observer sees the source, in the system's frame: the observer's present position
    /// plus the emission event's separation in the observer's rest frame (light time and
    /// aberration together). A view differences this against its own camera.
    #[must_use]
    pub const fn apparent(&self) -> &SystemPosition {
        &self.apparent
    }

    /// The last correction's change in the light time, |τₖ − τₖ₋₁|: at most
    /// [`IN_SYSTEM_LIGHT_TIME_TOLERANCE`] where the iteration converged, and above it only where it
    /// stopped on rounding noise (a change that no longer shrank).
    #[must_use]
    pub const fn residual(&self) -> Span {
        self.residual
    }
}

/// [`retarded_in_system`] could not trace a source's light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceInSystemError {
    /// The source is not present at `emitted`: not yet formed, or destroyed within the light time.
    /// For a source absent at the observer's present, `emitted` is the observed time, from which
    /// the iteration starts.
    NotPresentThen {
        /// The time at which the source was asked for and was absent.
        emitted: UniverseTime,
    },
    /// The light time still shrank after [`IN_SYSTEM_MAX_CORRECTIONS`] corrections: a source
    /// approaching or receding at nearly the speed of light.
    NotConverged {
        /// The corrections made.
        corrections: u8,
        /// The last correction's change in the light time.
        last_change: Span,
    },
}

impl fmt::Display for TraceInSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPresentThen { emitted } => {
                write!(f, "the source is not present at {emitted}")
            }
            Self::NotConverged {
                corrections,
                last_change,
            } => write!(
                f,
                "the light time did not converge after {corrections} corrections, the last \
                 changing it by {} s",
                last_change.as_seconds_f64()
            ),
        }
    }
}

impl Error for TraceInSystemError {}

/// What `observer` sees of `source`: the light time by a fixed point, and the apparent point by
/// the exact special-relativistic aberration (R03, Design note 7).
///
/// The light time starts from the present distance, τ₀ = |`x_B`(t) − `x_o`(t)| ÷ c, and correction
/// k ≥ 1 evaluates the source again, τₖ = |`x_B`(t − τₖ₋₁) − `x_o`(t)| ÷ c, each rounded to the
/// nanosecond by [`light_time`]. It stops at the first k whose change δₖ = |τₖ − τₖ₋₁| is at
/// most [`IN_SYSTEM_LIGHT_TIME_TOLERANCE`], or, from k = 2, at the first δₖ ≥ δₖ₋₁: a true
/// contraction strictly shrinks (its Lipschitz constant is |`v_B` · r̂| ÷ c), so a change that stops
/// shrinking is the rounding of an `f64` τ, whose step passes 1 ns near 10⁴ au and reaches 15–30 ns
/// at 1.5–2.7 × 10⁵ au. A fixed
/// point is kept rather than Newton's method, which fails when τ spans many orbits of a close
/// pair.
///
/// With r = `x_B`(t − τ) − `x_o`(t), β = `v_o` ÷ c and γ = 1 ÷ √(1 − β²), the apparent separation is
/// the emission event (r, −τ) boosted into the observer's rest frame,
///
/// r′ = r + γ `v_o` τ + [γ² ÷ (c² (γ + 1))] (r · `v_o`) `v_o`,
///
/// in which γ² ÷ (γ + 1) ÷ c² stands for (γ − 1) ÷ v², so nothing divides by |`v_o`| (Jackson
/// 1999, eq. 11.19; the Explanatory Supplement, §7.2.3). It points along the relativistically aberrated direction, cos θ′ = (cos θ +
/// β) ÷ (1 + β cos θ), and equals the first-order r + `v_o` τ to O(β²). For a source and an observer
/// moving together at u the light time's τ terms cancel and r′ is the rest-frame separation r₀ +
/// (γ − 1)(r₀ · û) û, within β² ÷ 4 of the present direction to leading order.
///
/// # Errors
///
/// [`TraceInSystemError::NotPresentThen`] if the source is absent at the observer's present or at
/// a time the iteration asks for; [`TraceInSystemError::NotConverged`] if the light time still
/// shrinks after [`IN_SYSTEM_MAX_CORRECTIONS`] corrections.
///
/// # Panics
///
/// If the source's position is not finite, or so far off that its light time leaves the clock,
/// which no position in a system can be.
///
/// # Examples
///
/// A planet at rest 1 au off, seen by a ship at rest: it appears where it is, 499 s ago.
///
/// ```
/// use hyperion_sim::coords::{SystemPosition, SystemVelocity};
/// use hyperion_sim::observe::{SystemObserver, SystemTrajectory, retarded_in_system};
/// use hyperion_sim::time::UniverseTime;
///
/// struct AtRest(SystemPosition);
/// impl SystemTrajectory for AtRest {
///     fn position_at(&self, _: UniverseTime) -> Option<SystemPosition> {
///         Some(self.0)
///     }
///     fn velocity_at(&self, _: UniverseTime) -> Option<SystemVelocity> {
///         Some(SystemVelocity::ZERO)
///     }
/// }
///
/// let planet = AtRest(SystemPosition::new([1.495_978_707e11, 0.0, 0.0]));
/// let ship = SystemObserver::new(SystemPosition::ORIGIN, SystemVelocity::ZERO, UniverseTime::EPOCH)?;
/// let seen = retarded_in_system(&ship, &planet)?;
/// assert_eq!(seen.light_time().seconds(), 499);
/// assert_eq!(seen.apparent(), &planet.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn retarded_in_system(
    observer: &SystemObserver,
    source: &impl SystemTrajectory,
) -> Result<InSystemRetardation, TraceInSystemError> {
    retarded_in_system_traced(observer, source, |_| {})
}

/// [`retarded_in_system`], calling `on_change` with each correction's change δₖ in order, so that
/// the tests can check the iteration's contraction.
fn retarded_in_system_traced(
    observer: &SystemObserver,
    source: &impl SystemTrajectory,
    mut on_change: impl FnMut(Span),
) -> Result<InSystemRetardation, TraceInSystemError> {
    let t = observer.time;
    let x_o = observer.position;
    let seen_from = |emitted: UniverseTime| {
        source
            .position_at(emitted)
            .ok_or(TraceInSystemError::NotPresentThen { emitted })
    };
    let light_time_to = |p: &SystemPosition| light_time(x_o.displacement_to(p).length());

    let mut tau = light_time_to(&seen_from(t)?);
    let mut previous_change: Option<Span> = None;
    for k in 1..=IN_SYSTEM_MAX_CORRECTIONS {
        let next = light_time_to(&seen_from(before(t, tau))?);
        let change = next
            .checked_sub(tau)
            .and_then(Span::checked_abs)
            .expect("two light times on the clock differ by a span");
        on_change(change);
        tau = next;
        let converged = change <= IN_SYSTEM_LIGHT_TIME_TOLERANCE;
        let noise = previous_change.is_some_and(|previous| change >= previous);
        if converged || noise {
            let emitted = before(t, tau);
            let geometric_then = seen_from(emitted)?;
            let r = x_o.displacement_to(&geometric_then);
            return Ok(InSystemRetardation {
                observed: t,
                emitted,
                light_time: tau,
                corrections: k,
                geometric_then,
                apparent: x_o.translated(aberrated(r, observer.velocity, tau)),
                residual: change,
            });
        }
        previous_change = Some(change);
    }
    Err(TraceInSystemError::NotConverged {
        corrections: IN_SYSTEM_MAX_CORRECTIONS,
        last_change: previous_change.unwrap_or(Span::ZERO),
    })
}

/// The separation `r` of an emission event a light time `tau` before reception, boosted into the
/// rest frame of an observer moving at `velocity` (Jackson 1999, eq. 11.19):
/// r + γ v τ + [γ² ÷ (c² (γ + 1))] (r · v) v.
///
/// The grouping is output: the client mirrors it operation for operation (R03.T13), so `c²` is
/// rounded once, γ = 1 ÷ √(1 − β²), κ = γ γ ÷ (c² (γ + 1)), and the sum is
/// (r + v (γ τ)) + v (κ (r · v)), left to right. A rewrite changes the golden vectors.
fn aberrated(r: SystemVector, velocity: SystemVelocity, tau: Span) -> SystemVector {
    let v = velocity.metres_per_second();
    let v_vec = SystemVector::new(v);
    let c2 = SPEED_OF_LIGHT * SPEED_OF_LIGHT;
    let beta2 = velocity.dot(&velocity) / c2;
    let gamma = 1.0 / (1.0 - beta2).sqrt();
    let kappa = gamma * gamma / (c2 * (gamma + 1.0));
    let tau_s = tau.as_seconds_f64();
    r + v_vec * (gamma * tau_s) + v_vec * (kappa * r.dot(&v_vec))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::math;
    use crate::units::{Metres, Seconds};

    /// The astronomical unit, m (IAU 2012 Resolution B2).
    const AU: f64 = 1.495_978_707e11;

    /// The instant every test observes at.
    fn now() -> UniverseTime {
        UniverseTime::from_julian_years(10).expect("ten years is on the clock")
    }

    /// Seconds from [`now`] to `t`, negative before it.
    fn seconds_from_now(t: UniverseTime) -> f64 {
        t.checked_since(now())
            .expect("two times on the clock differ by a span")
            .as_seconds_f64()
    }

    /// A source moving in a straight line: at `at_now` at [`now`], at `velocity` throughout, and
    /// present only from `from` on, if given.
    #[derive(Debug, Clone, Copy)]
    struct Uniform {
        at_now: [f64; 3],
        velocity: [f64; 3],
        from: Option<UniverseTime>,
    }

    impl Uniform {
        fn new(at_now: [f64; 3], velocity: [f64; 3]) -> Self {
            Self {
                at_now,
                velocity,
                from: None,
            }
        }

        fn present(&self, t: UniverseTime) -> bool {
            self.from.is_none_or(|from| t >= from)
        }
    }

    impl SystemTrajectory for Uniform {
        fn position_at(&self, t: UniverseTime) -> Option<SystemPosition> {
            self.present(t).then(|| {
                SystemPosition::new(self.at_now).translated(
                    SystemVelocity::new(self.velocity)
                        .displacement_over(Seconds::new(seconds_from_now(t))),
                )
            })
        }

        fn velocity_at(&self, t: UniverseTime) -> Option<SystemVelocity> {
            self.present(t).then(|| SystemVelocity::new(self.velocity))
        }
    }

    fn observer(position: [f64; 3], velocity: [f64; 3]) -> SystemObserver {
        SystemObserver::new(
            SystemPosition::new(position),
            SystemVelocity::new(velocity),
            now(),
        )
        .expect("a finite observer slower than light, now")
    }

    fn at_rest_observer(position: [f64; 3]) -> SystemObserver {
        observer(position, [0.0; 3])
    }

    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn norm(a: [f64; 3]) -> f64 {
        dot(a, a).sqrt()
    }

    fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
        [a[0] * k, a[1] * k, a[2] * k]
    }

    /// The angle between two vectors, rad, by atan2 of the cross and dot products, which keeps its
    /// precision near zero.
    fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        math::atan2(norm(cross), dot(a, b))
    }

    /// The apparent separation, `apparent − x_o(t)`.
    fn apparent_vector(seen: &InSystemRetardation, observer: &SystemObserver) -> [f64; 3] {
        sub(seen.apparent().metres(), observer.position().metres())
    }

    /// Asserts that `position` is `metres`, bit for bit.
    fn assert_same_position(position: &SystemPosition, metres: [f64; 3]) {
        for (got, expected) in position.metres().into_iter().zip(metres) {
            assert_same_bits(got, expected);
        }
    }

    fn nanos(span: Span) -> i128 {
        i128::from(span.seconds()) * 1_000_000_000 + i128::from(span.subsec_nanos())
    }

    /// The corrections' changes, in order, and the outcome.
    fn traced(
        observer: &SystemObserver,
        source: &impl SystemTrajectory,
    ) -> (Vec<Span>, Result<InSystemRetardation, TraceInSystemError>) {
        let mut changes = Vec::new();
        let result = retarded_in_system_traced(observer, source, |change| changes.push(change));
        (changes, result)
    }

    #[test]
    fn a_source_at_rest_is_seen_where_it_is_a_light_time_ago() {
        let place = [3.0 * AU, AU, -0.5 * AU];
        let source = Uniform::new(place, [0.0; 3]);
        let ship = at_rest_observer([0.0; 3]);
        let seen = retarded_in_system(&ship, &source).unwrap();
        assert_eq!(seen.light_time(), light_time(Metres::new(norm(place))));
        assert_eq!(
            seen.corrections(),
            1,
            "the first correction confirms the guess"
        );
        assert_eq!(seen.residual(), Span::ZERO);
        assert_eq!(seen.observed(), now());
        assert_eq!(
            seen.emitted(),
            now().checked_sub(seen.light_time()).unwrap()
        );
        assert_same_position(seen.geometric_then(), place);
        assert_same_position(seen.apparent(), place);
    }

    #[test]
    fn a_source_in_uniform_motion_is_on_the_light_cone_to_a_nanosecond() {
        let at_now = [2.0 * AU, -AU, 0.3 * AU];
        let velocity = [20e3, -35e3, 5e3];
        let x_o = [0.2 * AU, 0.1 * AU, 0.0];
        let seen =
            retarded_in_system(&at_rest_observer(x_o), &Uniform::new(at_now, velocity)).unwrap();
        // |D − v s| = c s: (c² − v²) s² + 2 (D · v) s − |D|² = 0, its positive root in the form
        // that does not cancel.
        let d = sub(at_now, x_o);
        let c2 = SPEED_OF_LIGHT * SPEED_OF_LIGHT;
        let dv = dot(d, velocity);
        let s = dot(d, d) / (dv + (dv * dv + (c2 - dot(velocity, velocity)) * dot(d, d)).sqrt());
        let tau = seen.light_time().as_seconds_f64();
        assert!((tau - s).abs() <= 1e-9, "τ {tau} s against the root {s} s");
    }

    #[test]
    fn a_source_receding_at_100_km_s_from_100_au_stops_after_four_corrections() {
        let beta = 1e5 / SPEED_OF_LIGHT;
        let source = Uniform::new([100.0 * AU, 0.0, 0.0], [1e5, 0.0, 0.0]);
        let (changes, seen) = traced(&at_rest_observer([0.0; 3]), &source);
        let seen = seen.unwrap();
        assert_eq!(seen.corrections(), 4, "{changes:?}");
        assert_eq!(changes.len(), 4);
        let tau0 = 100.0 * AU / SPEED_OF_LIGHT;
        assert!((tau0 - 49_900.478).abs() < 1e-3, "τ₀ {tau0}");
        assert!((beta - 3.335_64e-4).abs() < 1e-9, "β {beta}");
        let seconds: Vec<f64> = changes.iter().map(|c| c.as_seconds_f64()).collect();
        // For uniform radial motion the map is linear, δₖ = τ₀ β^k: 16.645 s, 5.552 ms,
        // 1,852 ns and 0.62 ns, each light time rounded to the nanosecond.
        assert!(
            (seconds[0] / tau0 / beta - 1.0).abs() < 1e-6,
            "δ₁ {}",
            seconds[0]
        );
        assert!((seconds[0] - 16.645).abs() < 1e-3, "δ₁ {}", seconds[0]);
        assert!(
            (seconds[1] / seconds[0] / beta - 1.0).abs() < 1e-6,
            "δ₂ {}",
            seconds[1]
        );
        assert!((seconds[1] - 5.552e-3).abs() < 1e-6, "δ₂ {}", seconds[1]);
        // For the linear map δ₃ = β δ₂ + (e₃ − e₂), each e one rounding of a light time to the
        // nanosecond, at most 0.5 ns: δ₃ ÷ δ₂ is β only to 1 ns in 1,852 (5.4 × 10⁻⁴), so δ₃ is
        // held to that rather than to 10⁻⁶.
        assert!(
            (nanos(changes[2]) - 1_852).abs() <= 1,
            "δ₃ {} ns",
            nanos(changes[2])
        );
        assert!(
            (seconds[2] - seconds[1] * beta).abs() <= 1e-9,
            "δ₃ {} s against β δ₂",
            seconds[2]
        );
        assert!(
            changes[3] <= IN_SYSTEM_LIGHT_TIME_TOLERANCE,
            "δ₄ {:?}",
            changes[3]
        );
        assert_eq!(seen.residual(), changes[3]);
    }

    #[test]
    fn a_source_receding_at_5_km_s_from_100_au_stops_after_three_corrections() {
        let source = Uniform::new([100.0 * AU, 0.0, 0.0], [5e3, 0.0, 0.0]);
        let (changes, seen) = traced(&at_rest_observer([0.0; 3]), &source);
        assert_eq!(seen.unwrap().corrections(), 3, "{changes:?}");
    }

    #[test]
    fn a_source_approaching_at_800_km_s_at_a_systems_reach_stops_within_the_cap() {
        let source = Uniform::new([2.7e5 * AU, 0.0, 0.0], [-8e5, 0.0, 0.0]);
        let (changes, seen) = traced(&at_rest_observer([0.0; 3]), &source);
        let corrections = seen.unwrap().corrections();
        assert!(
            (7..=8).contains(&corrections),
            "{corrections} corrections: {changes:?}"
        );
    }

    /// A source whose rounded light-time map has no fixed point: in light that left it
    /// [`Self::FAR_S`] or more ago it stood 5 ns of light nearer, so the map alternates between
    /// 1,000 s and 5 ns less.
    struct Alternating;

    impl Alternating {
        const FAR_S: i64 = 1_000;
    }

    impl SystemTrajectory for Alternating {
        fn position_at(&self, t: UniverseTime) -> Option<SystemPosition> {
            let far_ago = now()
                .checked_sub(Span::from_seconds(Self::FAR_S))
                .expect("on the clock");
            let light_s = if t <= far_ago {
                1_000.0 - 5e-9
            } else {
                1_000.0
            };
            Some(SystemPosition::new([light_s * SPEED_OF_LIGHT, 0.0, 0.0]))
        }

        fn velocity_at(&self, _: UniverseTime) -> Option<SystemVelocity> {
            Some(SystemVelocity::ZERO)
        }
    }

    #[test]
    fn a_map_with_no_fixed_point_stops_on_a_change_that_does_not_shrink() {
        let (changes, seen) = traced(&at_rest_observer([0.0; 3]), &Alternating);
        let seen = seen.unwrap();
        assert_eq!(
            changes.iter().map(|c| nanos(*c)).collect::<Vec<_>>(),
            [5, 5]
        );
        assert_eq!(seen.corrections(), 2);
        assert_eq!(seen.residual(), Span::new(0, 5).unwrap());
        assert_eq!(seen.light_time(), Span::from_seconds(Alternating::FAR_S));
    }

    #[test]
    fn a_source_near_the_speed_of_light_is_not_converged() {
        let tau0 = 1_000.0;
        let source = Uniform::new(
            [tau0 * SPEED_OF_LIGHT, 0.0, 0.0],
            [0.99 * SPEED_OF_LIGHT, 0.0, 0.0],
        );
        let (changes, seen) = traced(&at_rest_observer([0.0; 3]), &source);
        assert_eq!(changes.len(), usize::from(IN_SYSTEM_MAX_CORRECTIONS));
        let last = changes[changes.len() - 1];
        assert_eq!(
            seen,
            Err(TraceInSystemError::NotConverged {
                corrections: IN_SYSTEM_MAX_CORRECTIONS,
                last_change: last,
            })
        );
        // δ₁₀ = τ₀ β¹⁰ for the linear map.
        let expected = tau0 * math::powi(0.99, 10);
        assert!(
            (last.as_seconds_f64() - expected).abs() < 1e-3,
            "{last:?} against {expected} s"
        );
    }

    #[test]
    fn an_observer_at_a_hundredth_of_c_sees_the_relativistic_aberration() {
        let beta = 0.01;
        let ship = observer([0.0; 3], [beta * SPEED_OF_LIGHT, 0.0, 0.0]);
        for theta in [0.1, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0] {
            let (sin, cos) = math::sin_cos(theta);
            let source = Uniform::new([AU * cos, AU * sin, 0.0], [0.0; 3]);
            let seen = retarded_in_system(&ship, &source).unwrap();
            let r = apparent_vector(&seen, &ship);
            let cos_seen = r[0] / norm(r);
            let expected = (cos + beta) / (1.0 + beta * cos);
            assert!(
                (cos_seen - expected).abs() <= 1e-12,
                "θ {theta}: cos θ′ {cos_seen} against {expected}"
            );
        }
    }

    #[test]
    fn at_30_km_s_the_aberration_is_the_first_order_one_within_beta_squared_over_four() {
        let velocity = [3e4 * 0.6, 3e4 * 0.8, 0.0];
        let beta = 3e4 / SPEED_OF_LIGHT;
        let ship = observer([AU, 0.0, 0.0], velocity);
        for place in [[4.0 * AU, AU, 0.2 * AU], [-2.0 * AU, 3.0 * AU, -AU]] {
            let seen = retarded_in_system(&ship, &Uniform::new(place, [0.0; 3])).unwrap();
            let r = sub(seen.geometric_then().metres(), ship.position().metres());
            let first_order = add(r, scale(velocity, seen.light_time().as_seconds_f64()));
            let off = angle(apparent_vector(&seen, &ship), first_order);
            assert!(off <= beta * beta / 4.0, "{off} rad");
        }
    }

    #[test]
    fn an_accelerated_observer_differs_from_the_retarded_difference_by_a_tau_over_2c() {
        // A ship in low orbit, 6,778 km from its planet's centre with a centripetal acceleration
        // of 8.7 m/s², seeing a body at rest 2 au off, perpendicular to its orbit's plane.
        let radius: f64 = 6.778e6;
        let a: f64 = 8.7;
        let speed = (a * radius).sqrt();
        let omega = speed / radius;
        let ship = observer([radius, 0.0, 0.0], [0.0, speed, 0.0]);
        let body = [0.0, 0.0, 2.0 * AU];
        let seen = retarded_in_system(&ship, &Uniform::new(body, [0.0; 3])).unwrap();
        let tau = seen.light_time().as_seconds_f64();
        let (sin, cos) = math::sin_cos(-omega * tau);
        let ship_then = [radius * cos, radius * sin, 0.0];
        // The brainstorm's x_B(t − τ) − x_o(t − τ).
        let brainstorm = sub(seen.geometric_then().metres(), ship_then);
        let off = angle(apparent_vector(&seen, &ship), brainstorm);
        // a τ ÷ 2c is the limit for ω τ ≪ 1; here ω τ ≈ 1.13 rad, a fifth of an orbit in the light
        // time, and the exact offset R |(cos ω τ − 1, ω τ − sin ω τ)| is 0.965 of ½ a τ², inside
        // the plan's 10%, and held to 1% of the exact form besides.
        let expected = a * tau / (2.0 * SPEED_OF_LIGHT);
        assert!(
            (off / expected - 1.0).abs() <= 0.1,
            "{off} rad against a τ ÷ 2c = {expected} rad"
        );
        let (sin_wt, cos_wt) = math::sin_cos(omega * tau);
        let exact =
            radius * norm([cos_wt - 1.0, omega * tau - sin_wt, 0.0]) / (SPEED_OF_LIGHT * tau);
        assert!(
            (off / exact - 1.0).abs() <= 0.01,
            "{off} rad against the exact {exact} rad"
        );
    }

    #[test]
    fn a_body_400_km_off_is_displaced_10_27_m_by_light_time_and_aberration_together() {
        let body = [4e5, 0.0, 0.0];
        let ship = observer([0.0; 3], [0.0, 7.7e3, 0.0]);
        let seen = retarded_in_system(&ship, &Uniform::new(body, [0.0; 3])).unwrap();
        // Light time alone moves nothing.
        assert_same_position(seen.geometric_then(), body);
        let displaced = sub(seen.apparent().metres(), body);
        assert!((displaced[1] - 10.27).abs() <= 0.01, "{displaced:?}");
        assert!(
            displaced[0].abs() < 1e-6 && displaced[2].abs() < 1e-6,
            "{displaced:?}"
        );

        // The same 30 km/s added to both: the apparent displacement from the body's present
        // position is unchanged, while the light-time point moves about 40 m.
        let u = 3e4;
        let moving_ship = observer([0.0; 3], [0.0, 7.7e3 + u, 0.0]);
        let moving_body = Uniform::new(body, [0.0, u, 0.0]);
        let moving = retarded_in_system(&moving_ship, &moving_body).unwrap();
        let moving_displaced = sub(moving.apparent().metres(), body);
        assert!(
            norm(sub(moving_displaced, displaced)) <= 1e-3,
            "{moving_displaced:?} against {displaced:?}"
        );
        let light_time_point = norm(sub(moving.geometric_then().metres(), body));
        assert!(
            (light_time_point - 40.0).abs() < 0.1,
            "{light_time_point} m"
        );
    }

    #[test]
    fn comoving_observer_and_source_see_the_rest_frame_separation() {
        let direction = [1.0, 2.0, -0.5];
        let u_hat = scale(direction, 1.0 / norm(direction));
        for speed in [3e4, 3e6] {
            let u = scale(u_hat, speed);
            let x_o = [AU, -0.4 * AU, 0.1 * AU];
            let x_b = [-0.3 * AU, 0.8 * AU, 0.25 * AU];
            let ship = observer(x_o, u);
            let seen = retarded_in_system(&ship, &Uniform::new(x_b, u)).unwrap();
            let r0 = sub(x_b, x_o);
            let beta2 = speed * speed / (SPEED_OF_LIGHT * SPEED_OF_LIGHT);
            let gamma = 1.0 / (1.0 - beta2).sqrt();
            let rest = add(r0, scale(u_hat, (gamma - 1.0) * dot(r0, u_hat)));
            let apparent = apparent_vector(&seen, &ship);
            let tau = seen.light_time().as_seconds_f64();
            let bound = 1e-3 + 1e-14 * (norm(x_b) + norm(x_o) + speed * tau);
            let miss = norm(sub(apparent, rest));
            assert!(
                miss <= bound,
                "at {speed} m/s: {miss} m off the rest-frame separation, bound {bound} m"
            );
            let off = angle(apparent, r0);
            assert!(off <= beta2 / 4.0, "at {speed} m/s: {off} rad");
        }
    }

    #[test]
    fn a_relative_velocity_turns_the_apparent_vector_by_at_most_its_transverse_part_over_c() {
        let direction = [1.0, 2.0, -0.5];
        let u = scale(direction, 3e4 / norm(direction));
        let dv = [1e4, -2e4, 5e3];
        let x_o = [AU, -0.4 * AU, 0.1 * AU];
        let x_b = [-0.3 * AU, 0.8 * AU, 0.25 * AU];
        let ship = observer(x_o, u);
        let seen = retarded_in_system(&ship, &Uniform::new(x_b, add(u, dv))).unwrap();
        let r0 = sub(x_b, x_o);
        let r_hat = scale(r0, 1.0 / norm(r0));
        let dv_perp = norm(sub(dv, scale(r_hat, dot(dv, r_hat))));
        let beta_o2 = dot(u, u) / (SPEED_OF_LIGHT * SPEED_OF_LIGHT);
        let tau = seen.light_time().as_seconds_f64();
        let bound =
            dv_perp / SPEED_OF_LIGHT + beta_o2 / 4.0 + norm(dv) * 0.5e-9 / (SPEED_OF_LIGHT * tau);
        let off = angle(apparent_vector(&seen, &ship), r0);
        assert!(off <= bound, "{off} rad against {bound} rad");
    }

    #[test]
    fn a_source_absent_when_its_light_left_is_not_present_then() {
        let mut source = Uniform::new([AU, 0.0, 0.0], [0.0; 3]);
        source.from = Some(now().checked_sub(Span::from_seconds(100)).unwrap());
        let ship = at_rest_observer([0.0; 3]);
        let tau = light_time(Metres::new(AU));
        assert_eq!(
            retarded_in_system(&ship, &source),
            Err(TraceInSystemError::NotPresentThen {
                emitted: now().checked_sub(tau).unwrap()
            })
        );
        source.from = Some(now().checked_add(Span::from_seconds(1)).unwrap());
        assert_eq!(
            retarded_in_system(&ship, &source),
            Err(TraceInSystemError::NotPresentThen { emitted: now() })
        );
    }

    #[test]
    fn the_same_observer_and_source_give_the_same_reading_twice() {
        let source = Uniform::new([2.0 * AU, -AU, 0.3 * AU], [20e3, -35e3, 5e3]);
        let ship = observer([0.2 * AU, 0.1 * AU, 0.0], [1e3, 29e3, -2e3]);
        assert_eq!(
            retarded_in_system(&ship, &source),
            retarded_in_system(&ship, &source)
        );
    }

    #[test]
    fn an_observer_must_be_finite_slower_than_light_and_in_the_window() {
        let t = now();
        assert_eq!(
            SystemObserver::new(
                SystemPosition::new([f64::NAN, 0.0, 0.0]),
                SystemVelocity::ZERO,
                t
            ),
            Err(BuildSystemObserverError::NotFinite)
        );
        assert_eq!(
            SystemObserver::new(
                SystemPosition::ORIGIN,
                SystemVelocity::new([0.0, f64::INFINITY, 0.0]),
                t
            ),
            Err(BuildSystemObserverError::NotFinite)
        );
        assert_eq!(
            SystemObserver::new(
                SystemPosition::ORIGIN,
                SystemVelocity::new([0.0, 0.0, SPEED_OF_LIGHT]),
                t
            ),
            Err(BuildSystemObserverError::NotSlowerThanLight)
        );
        let late = ClockWindow::END.checked_add(Span::from_seconds(1)).unwrap();
        assert_eq!(
            SystemObserver::new(SystemPosition::ORIGIN, SystemVelocity::ZERO, late),
            Err(BuildSystemObserverError::TimeOutsideClockWindow(late))
        );
    }
}
