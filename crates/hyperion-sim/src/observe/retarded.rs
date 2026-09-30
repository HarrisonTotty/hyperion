//! Retarded time: where and when the light an observer receives now left its source (plan 12,
//! P12.T1; Design notes 1 and 2).

use std::error::Error;
use std::fmt;

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity, LyCell};
use crate::galaxy::Galaxy;
use crate::galaxy::features::members::{
    FeatureInteriorCache, MemberRecord, NoInteriorCache, resolve_member,
};
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::query::epoch_velocity;
use crate::id::{SystemId, SystemIdKind};
use crate::math;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SPEED_OF_LIGHT};
use crate::units::{LightYears, Metres};

/// An [`Observer`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildObserverError {
    /// The position lies outside the root cube,
    /// `±`[`ROOT_HALF_WIDTH_LY`](crate::coords::ROOT_HALF_WIDTH_LY) on each axis.
    OutsideRootCube,
    /// The time lies outside the [`ClockWindow`], ±1,000 Julian years about the epoch: an observer
    /// is someone in play, and play happens inside the window. Only the light it receives is older.
    TimeOutsideClockWindow(UniverseTime),
}

impl fmt::Display for BuildObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideRootCube => f.write_str("the observer lies outside the root cube"),
            Self::TimeOutsideClockWindow(t) => {
                write!(f, "the observer's time {t} lies outside the clock window")
            }
        }
    }
}

impl Error for BuildObserverError {}

/// A system's motion cannot be traced yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceMotionError {
    /// The system is a member of the galactic centre. Inside the black hole's sphere of influence
    /// it follows its Kepler orbit, and the regime and the orbit are plan 09's P09.T28, which is
    /// not built; a straight line would misplace it by more than a tidal radius a century at 1 ly
    /// (brainstorm, "Orbits and time"), so no line is given for any centre member until then.
    CentreOrbitNotBuilt(SystemId),
}

impl fmt::Display for TraceMotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CentreOrbitNotBuilt(id) => write!(
                f,
                "the centre member {} follows an orbit that is not built yet",
                id.raw()
            ),
        }
    }
}

impl Error for TraceMotionError {}

/// Someone who receives light: a position inside the root cube at a time inside the clock window.
///
/// For now the client sets it (plan 12, Design note 9); once sessions exist the ship is one.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::observe::{BuildObserverError, Observer};
/// use hyperion_sim::time::UniverseTime;
///
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// assert_eq!(observer.time(), UniverseTime::EPOCH);
///
/// // Play happens inside the clock window; only the light an observer receives is older.
/// let far_past = UniverseTime::from_julian_years(-5_000).expect("in range");
/// assert_eq!(
///     Observer::new(sun, far_past),
///     Err(BuildObserverError::TimeOutsideClockWindow(far_past))
/// );
/// # Ok::<(), BuildObserverError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Observer {
    position: GalacticPosition,
    time: UniverseTime,
}

impl Observer {
    /// The observer at `position` at `time`.
    ///
    /// # Errors
    ///
    /// [`BuildObserverError::OutsideRootCube`] for a position outside the root cube, and
    /// [`BuildObserverError::TimeOutsideClockWindow`] for a time outside ±H.
    pub fn new(position: GalacticPosition, time: UniverseTime) -> Result<Self, BuildObserverError> {
        if !position.in_root_cube() {
            return Err(BuildObserverError::OutsideRootCube);
        }
        if !ClockWindow::contains(time) {
            return Err(BuildObserverError::TimeOutsideClockWindow(time));
        }
        Ok(Self { position, time })
    }

    /// Where the observer is, in the galactic frame.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// The observer's present, on the universe clock.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }
}

/// How a source's path is modelled, which decides whether a retarded reading of it neglects any
/// curvature ([`curvature_error`](super::curvature_error)).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Motion {
    /// A straight line at a constant velocity: every system outside the central black hole's
    /// Kepler regime (brainstorm, "Orbits and time"). The curvature of its true galactic orbit
    /// over the light's age is neglected, and stated.
    #[default]
    Drift,
    /// An orbit followed in full, as plan 09's Kepler regime follows the centre's members
    /// (P09.T28): nothing is neglected, so the stated error is zero.
    Followed,
}

/// Anything with a position that is a pure function of time (plan 12's Provides).
///
/// [`retarded`] evaluates it at two times, so an implementation must give the same answer
/// whatever it was asked before, as every generator does. It must be defined back to
/// [`SourceHorizon::START`](crate::time::SourceHorizon::START), −(H + L), the earliest time light
/// received during play can have left.
pub trait Trajectory {
    /// The position at `t`, in the galactic frame.
    fn position_at(&self, t: UniverseTime) -> GalacticPosition;

    /// The velocity at `t`, in the galactic frame.
    fn velocity_at(&self, t: UniverseTime) -> GalacticVelocity;

    /// How the path is modelled; [`Motion::Drift`] unless the implementation follows an orbit.
    fn motion(&self) -> Motion {
        Motion::Drift
    }

    /// The position at `t` with what its rounding to the representation dropped, as a
    /// displacement of under a metre: zero unless the implementation keeps it.
    ///
    /// [`extrapolate_to_present`] adds it back, so that a straight line carried forward from its
    /// apparent position lands on itself to one rounding, not two.
    fn position_with_residual_at(
        &self,
        t: UniverseTime,
    ) -> (GalacticPosition, GalacticDisplacement) {
        (self.position_at(t), GalacticDisplacement::default())
    }
}

/// A straight line from a position at the epoch at a constant velocity: how every system outside
/// the central black hole's Kepler regime moves (brainstorm, "Orbits and time"; plan 08's drift).
///
/// Its position at `t` is the epoch position plus the velocity times the span since the epoch,
/// computed so that the error does not grow with the span: the product is carried as an exact
/// pair and the whole light-years are split off exactly before the offset is rounded once, so a
/// position far back in the source horizon is as good as the representation, half a unit in the
/// last place of the offset, under a metre (plan 12, P12.T1 as built). Plan 03's
/// [`position_at`](crate::galaxy::query::position_at) rounds the product and then the offset,
/// which within the clock window agrees with this to a few metres and beyond it would lose tens
/// of metres at light times of tens of thousands of years.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::observe::{Drift, Trajectory};
/// use hyperion_sim::time::UniverseTime;
///
/// let start = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
/// // 299.792458 km/s is a thousandth of c: a light-year every thousand years.
/// let drift = Drift::new(start, GalacticVelocity::new([299_792.458, 0.0, 0.0]));
/// let later = drift.position_at(UniverseTime::from_julian_years(1_000).expect("in range"));
/// let moved = start.distance_to(&later).value();
/// assert!((moved - hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR).abs() < 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drift {
    epoch_position: GalacticPosition,
    velocity: GalacticVelocity,
}

impl Drift {
    /// The line through `epoch_position` at the epoch with `velocity`.
    #[must_use]
    pub const fn new(epoch_position: GalacticPosition, velocity: GalacticVelocity) -> Self {
        Self {
            epoch_position,
            velocity,
        }
    }

    /// The line a system follows: its epoch position and its velocity.
    ///
    /// A grid record moves at plan 08's [`epoch_velocity`] (zero in a galaxy built without its
    /// kinematic tables); a feature member at its own, the cluster's bulk motion plus its internal
    /// one (plan 09, P09.T10), which only its [`MemberRecord`] carries, so the member is resolved
    /// again here. A caller that has the member at hand passes it to [`Drift::of_member`] instead.
    ///
    /// # Errors
    ///
    /// [`TraceMotionError::CentreOrbitNotBuilt`] for a member of the galactic centre, whose Kepler
    /// regime and orbit wait for plan 09's P09.T28 (P12.T2 as built, provisional).
    ///
    /// # Panics
    ///
    /// If `record` is a feature member that does not resolve in `galaxy`, which only a record of
    /// another galaxy can be.
    pub fn of_record(galaxy: &Galaxy, record: &SystemRecord) -> Result<Self, TraceMotionError> {
        Self::of_record_in(galaxy, &NoInteriorCache, record)
    }

    /// [`of_record`](Self::of_record) with a feature member's interior from `interiors`, so that a
    /// caller tracing many members of one feature builds its interior once (P12.T3 as built). The
    /// line is the same whatever the cache holds.
    ///
    /// # Errors
    ///
    /// As [`of_record`](Self::of_record).
    ///
    /// # Panics
    ///
    /// As [`of_record`](Self::of_record).
    pub(crate) fn of_record_in(
        galaxy: &Galaxy,
        interiors: &dyn FeatureInteriorCache,
        record: &SystemRecord,
    ) -> Result<Self, TraceMotionError> {
        let velocity = match record.origin() {
            SystemOrigin::Grid(_) => epoch_velocity(galaxy, record),
            SystemOrigin::FeatureMember { .. } => {
                let SystemIdKind::FeatureMember(id) = record.id().kind() else {
                    unreachable!("a feature member's record is built with a member ID")
                };
                resolve_member(galaxy, interiors, id)
                    .expect("a feature member's record resolves in the galaxy that placed it")
                    .velocity()
            }
            SystemOrigin::CentreMember { .. } => {
                return Err(TraceMotionError::CentreOrbitNotBuilt(record.id()));
            }
        };
        Ok(Self::new(*record.epoch_position(), velocity))
    }

    /// The line through `position` at `at` with `velocity`: what a source that reports only
    /// positions at given times is read as, from two of them.
    ///
    /// # Panics
    ///
    /// If the line leaves the addressable range at the epoch, which nothing slower than light can
    /// from a position in the cube within the universe clock's range near the source horizon.
    #[must_use]
    pub fn through(
        position: &GalacticPosition,
        at: UniverseTime,
        velocity: GalacticVelocity,
    ) -> Self {
        let (epoch_position, _) = drift(position, [0.0; 3], &(-velocity), at.since_epoch())
            .expect("a line slower than light through the cube is addressable at the epoch");
        Self::new(epoch_position, velocity)
    }

    /// The line a feature member follows, from the member itself.
    #[must_use]
    pub fn of_member(member: &MemberRecord) -> Self {
        Self::new(*member.record().epoch_position(), member.velocity())
    }

    /// The position at the epoch.
    #[must_use]
    pub const fn epoch_position(&self) -> &GalacticPosition {
        &self.epoch_position
    }

    /// The constant velocity.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }
}

impl Trajectory for Drift {
    /// # Panics
    ///
    /// If the drift leaves the addressable range of ±2³¹ ly, which nothing slower than light can
    /// do within ±2 × 10⁹ years of the epoch.
    fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        self.position_with_residual_at(t).0
    }

    /// # Panics
    ///
    /// As [`position_at`](Self::position_at).
    fn position_with_residual_at(
        &self,
        t: UniverseTime,
    ) -> (GalacticPosition, GalacticDisplacement) {
        drift(
            &self.epoch_position,
            [0.0; 3],
            &self.velocity,
            t.since_epoch(),
        )
        .map(|(p, r)| (p, GalacticDisplacement::new(r)))
        .expect(
            "a drift slower than light stays addressable over any span the universe clock holds \
             near the source horizon",
        )
    }

    fn velocity_at(&self, _t: UniverseTime) -> GalacticVelocity {
        self.velocity
    }
}

/// The light-travel time over `distance`, rounded to the nanosecond (Design note 2).
///
/// At 50,000 ly an `f64` of seconds resolves 0.2 ms, far below anything observable, so the
/// rounding is exact to the clock's nanosecond wherever the `f64` is.
///
/// # Panics
///
/// If `distance` is negative or not finite, which no distance between two positions is.
///
/// # Examples
///
/// ```
/// use hyperion_sim::observe::light_time;
/// use hyperion_sim::units::{LightYears, Metres};
///
/// // A light-year is the distance light travels in a Julian year, by definition.
/// let year = light_time(Metres::from(LightYears::new(1.0)));
/// assert_eq!(year.seconds(), 31_557_600);
/// assert!(year.subsec_nanos() <= 1);
/// ```
#[must_use]
pub fn light_time(distance: Metres) -> Span {
    let seconds = distance.value() / SPEED_OF_LIGHT;
    assert!(
        seconds.is_finite() && seconds >= 0.0,
        "a distance must be finite and not negative, got {distance:?}"
    );
    // Half a nanosecond before the floor rounds to the nearest nanosecond; where an `f64` of
    // seconds is coarser than that, the addition does nothing and the floor drops less than a
    // nanosecond.
    Span::from_seconds_f64(seconds + 5e-10)
        .expect("a light time across the addressable range fits in i64 seconds")
}

/// Where and when the light an observer receives left its source (plan 12's Provides).
///
/// The emitted time is `observed − light_age`, the apparent position is the source's position then
/// and `velocity_then` its velocity then; `distance_now` is the distance at the observer's present,
/// which the search used. A drifting source's apparent position and velocity, carried forward over
/// the light's age ([`extrapolate_to_present`]), land on the star.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retardation {
    observed: UniverseTime,
    emitted: UniverseTime,
    light_age: Span,
    distance_now: LightYears,
    apparent_position: GalacticPosition,
    /// What the apparent position's rounding dropped.
    apparent_residual: GalacticDisplacement,
    velocity_then: GalacticVelocity,
    motion: Motion,
}

impl Retardation {
    /// The observer's time: when the light arrives.
    #[must_use]
    pub const fn observed(&self) -> UniverseTime {
        self.observed
    }

    /// When the light left the source, `observed − light_age`: at or after
    /// [`SourceHorizon::START`](crate::time::SourceHorizon::START) for any observer and source
    /// inside the cube.
    #[must_use]
    pub const fn emitted(&self) -> UniverseTime {
        self.emitted
    }

    /// The age of the light, to the nanosecond: its travel time from the apparent position.
    #[must_use]
    pub const fn light_age(&self) -> Span {
        self.light_age
    }

    /// The distance to the source at the observer's present.
    #[must_use]
    pub const fn distance_now(&self) -> LightYears {
        self.distance_now
    }

    /// Where the source was when the light left it: where the observer sees it.
    #[must_use]
    pub const fn apparent_position(&self) -> &GalacticPosition {
        &self.apparent_position
    }

    /// The source's velocity when the light left it.
    #[must_use]
    pub const fn velocity_then(&self) -> GalacticVelocity {
        self.velocity_then
    }

    /// How the source's path is modelled.
    #[must_use]
    pub const fn motion(&self) -> Motion {
        self.motion
    }

    /// Where the line this reading shows passes at the epoch: the apparent position carried to
    /// the epoch at the velocity then. For a drifting source that is its epoch position, to about a
    /// metre, the point at which plan 08 draws its velocity and where the line and the true orbit
    /// agree (ruling 143.2).
    ///
    /// # Panics
    ///
    /// If the line leaves the addressable range at the epoch, which nothing slower than light can
    /// from a position in the cube within the universe clock's range near the source horizon.
    #[must_use]
    pub(crate) fn line_at_epoch(&self) -> GalacticPosition {
        let span = UniverseTime::EPOCH
            .checked_since(self.emitted)
            .expect("two times on the clock differ by a span the clock holds");
        drift(
            &self.apparent_position,
            self.apparent_residual.metres(),
            &self.velocity_then,
            span,
        )
        .expect("a line slower than light through the cube is addressable at the epoch")
        .0
    }
}

/// What `observer` sees of `source`: one fixed-point step on the light cone (Design note 1).
///
/// With Δ(τ) = `x_s`(τ) − `x_o`: s₀ = |Δ(t)| ÷ c from the present position, then s₁ = |Δ(t − s₀)| ÷
/// c, and the emitted time is t − s₁. The step corrects by about d `v_r` ÷ c², 37 years at 50,000
/// ly for a star receding at 220 km/s, and what it leaves is at most (v ÷ c)² of the light time: at
/// 50,000 ly under seven months at plan 03's padding speed of 1,000 km/s and about ten days for a
/// disc star at 220 km/s, and 2.5 years at 1,000 km/s across the whole cube. For a bound orbit the
/// residual is bounded by the orbit's light-crossing time times v ÷ c. One path serves drift and
/// followed orbits alike; [`retarded_exact_linear`] is the drift's closed form and the test oracle.
///
/// Light times are taken from the displacement in metres, integer cells first, and rounded to the
/// nanosecond (Design note 2).
///
/// # Panics
///
/// As `source` does, for a time before the source horizon's start; and if the light time does not
/// fit the clock, which no source inside the addressable range can reach.
///
/// # Examples
///
/// A star 10,000 ly away is seen as it was 10,000 years ago.
///
/// ```
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::observe::{Drift, Observer, retarded};
/// use hyperion_sim::time::UniverseTime;
///
/// let here = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in range");
/// let there = GalacticPosition::from_light_years([0.0, 16_000.0, 0.0]).expect("in range");
/// let star = Drift::new(there, GalacticVelocity::default());
/// let seen = retarded(&Observer::new(here, UniverseTime::EPOCH)?, &star);
/// assert!((seen.light_age().as_julian_years_f64() - 10_000.0).abs() < 1e-6);
/// assert!(seen.emitted() < UniverseTime::EPOCH);
/// # Ok::<(), hyperion_sim::observe::BuildObserverError>(())
/// ```
#[must_use]
pub fn retarded(observer: &Observer, source: &impl Trajectory) -> Retardation {
    let present = source.position_at(observer.time);
    retarded_from(observer, source, &present)
}

/// [`retarded`] from the source's present position, which a caller that searched on present
/// positions already has: it saves one of the step's three evaluations of the trajectory (ruling
/// 143.4).
///
/// `present` must be `source.position_at(observer.time())`; given that, the result is bit for bit
/// [`retarded`]'s. Given anything else, the first light time is taken from it, which moves the
/// step's starting guess and so, by up to the step's residual, the answer.
///
/// # Panics
///
/// As [`retarded`].
///
/// # Examples
///
/// A range query has found a star on its present position; its light is read from there.
///
/// ```
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::observe::{Drift, Observer, Trajectory, retarded, retarded_from};
/// use hyperion_sim::time::UniverseTime;
///
/// let here = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let star = Drift::new(
///     GalacticPosition::from_light_years([40.0, 25_990.0, 3.0]).ok_or("in range")?,
///     GalacticVelocity::new([12e3, -30e3, 4e3]),
/// );
/// let observer = Observer::new(here, UniverseTime::from_julian_years(300).ok_or("in range")?)?;
/// let present = star.position_at(observer.time());
/// assert_eq!(retarded_from(&observer, &star, &present), retarded(&observer, &star));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn retarded_from(
    observer: &Observer,
    source: &impl Trajectory,
    present: &GalacticPosition,
) -> Retardation {
    let t = observer.time;
    let distance_now = observer.position.distance_to(present);
    let first = light_time(distance_now);
    let guess = before(t, first);
    let light_age = light_time(observer.position.distance_to(&source.position_at(guess)));
    let emitted = before(t, light_age);
    let (apparent_position, apparent_residual) = source.position_with_residual_at(emitted);
    Retardation {
        observed: t,
        emitted,
        light_age,
        distance_now: LightYears::from(distance_now),
        apparent_position,
        apparent_residual,
        velocity_then: source.velocity_at(emitted),
        motion: source.motion(),
    }
}

/// `t − span`.
///
/// # Panics
///
/// If the difference leaves the clock, which a light time across the addressable range cannot
/// take a time inside the clock window to.
fn before(t: UniverseTime, span: Span) -> UniverseTime {
    t.checked_sub(span)
        .expect("a light time across the addressable range keeps a time on the clock")
}

/// The drift's retardation in closed form, the test oracle of [`retarded`] (Design note 1).
///
/// For `x_s`(τ) = `x_e` + v τ the light cone |`x_s`(t − s) − `x_o`| = c s is the quadratic (c² −
/// v²) s² + 2 (D · v) s − |D|² = 0 with D = `x_s`(t) − `x_o`, whose positive root is taken in the
/// form s = |D|² ÷ (D · v + √((D · v)² + (c² − v²) |D|²)), which does not cancel. It is kept as the
/// oracle and not used for the implementation, because one path for every kind of motion is easier
/// to keep reproducible.
///
/// # Panics
///
/// If the speed is not below that of light, for which the light cone has no single root.
#[must_use]
pub fn retarded_exact_linear(
    observer: &Observer,
    epoch_position: &GalacticPosition,
    velocity: &GalacticVelocity,
) -> Retardation {
    let line = Drift::new(*epoch_position, *velocity);
    let t = observer.time;
    let present = line.position_at(t);
    let d = observer.position.displacement_to(&present);
    let v = velocity.metres_per_second();
    let speed_sq = velocity.speed().value() * velocity.speed().value();
    let c_sq = SPEED_OF_LIGHT * SPEED_OF_LIGHT;
    assert!(speed_sq < c_sq, "a source must move slower than light");
    let dv = d.dot(&GalacticDisplacement::new(v));
    let d_sq = d.dot(&d);
    let seconds = if d_sq > 0.0 {
        d_sq / (dv + (dv * dv + (c_sq - speed_sq) * d_sq).sqrt())
    } else {
        0.0
    };
    let light_age = light_time(Metres::new(seconds * SPEED_OF_LIGHT));
    let emitted = before(t, light_age);
    let (apparent_position, apparent_residual) = line.position_with_residual_at(emitted);
    Retardation {
        observed: t,
        emitted,
        light_age,
        distance_now: LightYears::from(d.length()),
        apparent_position,
        apparent_residual,
        velocity_then: *velocity,
        motion: Motion::Drift,
    }
}

/// The observed position carried forward to `now` at the observed velocity: where a navigation
/// computer puts the source.
///
/// For a drifting source this is the source's position at `now`, to about a metre, whatever the
/// light's age: the model is linear, so an observed position and velocity at any emitted time
/// extrapolate to the same place (brainstorm, "What a sensor sees is the past"). The error of
/// treating the true galactic orbit as straight is what
/// [`curvature_error`](super::curvature_error) states.
///
/// For a [`Motion::Followed`] source this is still the straight line; following the orbit needs
/// its elements, which arrive with plan 09's Kepler regime (P09.T28) and are recorded as a finding
/// of P12.T2.
///
/// # Panics
///
/// If the extrapolation leaves the addressable range, which nothing slower than light can reach
/// from the cube within the universe clock's range near the source horizon.
///
/// # Examples
///
/// A navigation computer sees a star as it was 20,000 years ago, carries its observed position
/// forward, and the jump lands on the star.
///
/// ```
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::observe::{Drift, Observer, Trajectory, extrapolate_to_present, retarded};
/// use hyperion_sim::time::UniverseTime;
///
/// let here = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let star = Drift::new(
///     GalacticPosition::from_light_years([0.0, 6_000.0, 0.0]).ok_or("in range")?,
///     GalacticVelocity::new([230e3, -15e3, 8e3]),
/// );
/// let now = UniverseTime::from_julian_years(500).ok_or("in range")?;
/// let seen = retarded(&Observer::new(here, now)?, &star);
/// // The light shows it about 15 light-years from where it is now.
/// assert!(seen.apparent_position().distance_to(&star.position_at(now)).value() > 1e17);
/// let target = extrapolate_to_present(&seen, now);
/// assert!(target.distance_to(&star.position_at(now)).value() < 1.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn extrapolate_to_present(r: &Retardation, now: UniverseTime) -> GalacticPosition {
    let span = now
        .checked_since(r.emitted)
        .expect("two times on the clock differ by a span the clock holds");
    drift(
        &r.apparent_position,
        r.apparent_residual.metres(),
        &r.velocity_then,
        span,
    )
    .expect("an extrapolation slower than light from the cube stays addressable")
    .0
}

/// `base`, plus `residual` metres per axis, moved by `velocity` over `span`: the position and what
/// its rounding dropped, or `None` if it would leave the addressable range or an input is not
/// finite.
///
/// The error does not grow with the span. Per axis the product v × Δt is carried as an exact pair
/// (the rounded product and its error, by [`math::two_product`], which is bit for bit the fused
/// multiply-add's error), the whole light-years k are split
/// off with k × 1 ly also an exact pair, and every sum is a two-sum whose error is kept, so the
/// offset is rounded once, at the end, and what that rounding dropped is returned. Its arithmetic
/// is fixed, so the result is the same on every platform.
fn drift(
    base: &GalacticPosition,
    residual: [f64; 3],
    velocity: &GalacticVelocity,
    span: Span,
) -> Option<(GalacticPosition, [f64; 3])> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "whole seconds below 2^53 (285 million years) are exact in an f64, and beyond \
                  that the rounding is below a part in 10^15 of the span"
    )]
    let whole_seconds = span.seconds() as f64;
    let fraction = f64::from(span.subsec_nanos()) * 1e-9;
    let v = velocity.metres_per_second();
    let cells = base.cell().to_array();
    let offsets = base.offset_metres();
    let mut cell = [0_i32; 3];
    let mut offset = [0.0; 3];
    let mut dropped = [0.0; 3];
    for axis in 0..3 {
        if !(v[axis].is_finite() && residual[axis].is_finite()) {
            return None;
        }
        // Dekker's product, bit for bit the fused multiply-add's error at a tenth of the cost of
        // `libm`'s software `fma` (ruling 143.4: 7 against 34 ns an operation, six an evaluation).
        let (product, product_error) = math::two_product(v[axis], whole_seconds);
        let mut whole = (product / METRES_PER_LIGHT_YEAR).floor();
        let (whole_high, whole_low) = math::two_product(whole, METRES_PER_LIGHT_YEAR);
        let (rest, rest_error) = two_sum(product, -whole_high);
        let (mut high, sum_error) = two_sum(offsets[axis], rest);
        let mut low = (((rest_error + sum_error) - whole_low) + product_error)
            + (v[axis] * fraction + residual[axis]);
        // The pair lies within a few light-years of [0, 1 ly): a few carries normalise it, and the
        // rounded sum is tested so that the final rounding cannot leave the range.
        for _ in 0..6 {
            let rounded = high + low;
            if rounded < 0.0 {
                let (moved, error) = two_sum(high, METRES_PER_LIGHT_YEAR);
                high = moved;
                low += error;
                whole -= 1.0;
            } else if rounded >= METRES_PER_LIGHT_YEAR {
                let (moved, error) = two_sum(high, -METRES_PER_LIGHT_YEAR);
                high = moved;
                low += error;
                whole += 1.0;
            } else {
                break;
            }
        }
        let rounded = high + low;
        let (kept, error) = if (0.0..METRES_PER_LIGHT_YEAR).contains(&rounded) {
            (rounded, two_sum(high, low).1)
        } else if rounded >= METRES_PER_LIGHT_YEAR {
            // Within half a unit in the last place below a whole light-year the sum rounds up to it
            // and, carried, down below zero: it is the next cell's start, less a residual.
            let (moved, carried) = two_sum(high, -METRES_PER_LIGHT_YEAR);
            whole += 1.0;
            (0.0, moved + (low + carried))
        } else if rounded > -2.0 {
            // The same boundary met from below zero: this cell's start, less a residual.
            (0.0, rounded)
        } else {
            return None;
        };
        if error.abs() >= 2.0 {
            return None;
        }
        cell[axis] = cells[axis].checked_add(whole_to_i32(whole)?)?;
        offset[axis] = kept;
        dropped[axis] = error;
    }
    let position = GalacticPosition::new(LyCell::new(cell), offset).ok()?;
    Some((position, dropped))
}

/// Knuth's two-sum: `a + b` rounded, and the exact error of that rounding.
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let b_virtual = sum - a;
    let a_virtual = sum - b_virtual;
    (sum, (a - a_virtual) + (b - b_virtual))
}

/// A float that holds a whole number, as an `i32`, or `None` if out of range or not a number.
fn whole_to_i32(whole: f64) -> Option<i32> {
    if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&whole) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "whole is an integer inside the i32 range"
    )]
    Some(whole as i32)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::coords::ROOT_HALF_WIDTH_LY;
    use crate::time::{LIGHT_CROSSING_L, SourceHorizon};
    use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

    fn at_ly(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).unwrap()
    }

    fn years(y: i64) -> UniverseTime {
        UniverseTime::from_julian_years(y).unwrap()
    }

    /// A uniform point of the root cube, from three draws.
    fn in_cube(lcg: &mut Lcg) -> GalacticPosition {
        let half = f64::from(ROOT_HALF_WIDTH_LY);
        let mut ly = [0.0; 3];
        for c in &mut ly {
            *c = (2.0 * lcg.next_f64() - 1.0) * half * 0.999_999;
        }
        at_ly(ly)
    }

    /// A velocity of uniform direction and a speed uniform up to `max_km_s`.
    fn velocity(lcg: &mut Lcg, max_km_s: f64) -> GalacticVelocity {
        loop {
            let u = [0; 3].map(|_| 2.0 * lcg.next_f64() - 1.0);
            let n = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
            if n > 1e-3 && n <= 1.0 {
                let speed = lcg.next_f64() * max_km_s * 1e3;
                return GalacticVelocity::new(u.map(|c| c / n * speed));
            }
        }
    }

    fn seconds(span: Span) -> f64 {
        span.as_seconds_f64()
    }

    /// Plan 12, P12.T1: the step agrees with the closed form to (v ÷ c)² of the light time for 10⁴
    /// random sources in the cube at up to 1,000 km/s, seen from random observers at random times
    /// of the clock window. The bound is exact: the map s ↦ |Δ(t − s)| ÷ c has slope at most v ÷ c,
    /// so one step from s₀ leaves at most (v ÷ c)² s.
    #[test]
    fn retarded_step_agrees_with_the_closed_form_to_second_order() {
        let mut lcg = Lcg::new(0x1201_0001);
        let mut worst = 0.0_f64;
        let mut worst_within_50_kly = 0.0_f64;
        for _ in 0..10_000 {
            let observer_at = in_cube(&mut lcg);
            let t = UniverseTime::from_julian_years(
                i64::try_from(lcg.next_u64() % 2_001).unwrap() - 1_000,
            )
            .unwrap();
            let observer = Observer::new(observer_at, t).unwrap();
            let start = in_cube(&mut lcg);
            let v = velocity(&mut lcg, 1_000.0);
            let step = retarded(&observer, &Drift::new(start, v));
            let exact = retarded_exact_linear(&observer, &start, &v);
            let beta = v.speed().value() / SPEED_OF_LIGHT;
            let s = seconds(exact.light_age());
            // The positions are good to about a metre, 3 ns of light, and a light time near L is an
            // f64 good to a millisecond.
            let bound = beta * beta * s + 1e-2;
            let gap = (seconds(step.light_age()) - s).abs();
            assert!(
                gap <= bound,
                "the step is {gap} s from the closed form, over the bound of {bound} s"
            );
            worst = worst.max(gap / bound);
            if exact.distance_now().value() <= 50_000.0 {
                worst_within_50_kly = worst_within_50_kly.max(gap / SECONDS_PER_JULIAN_YEAR);
            }
        }
        assert!(worst <= 1.0, "the worst residual is {worst} of its bound");
        // Under seven months at the padding speed out to 50,000 ly (Design note 1).
        assert!(
            worst_within_50_kly < 7.0 / 12.0,
            "the worst residual within 50,000 ly is {worst_within_50_kly} yr"
        );
    }

    /// Design note 1: the step's correction is d `v_r` ÷ c², 37 ± 1 years for a star receding at
    /// 220 km/s at 50,000 ly.
    #[test]
    fn retarded_correction_is_thirty_seven_years_at_fifty_thousand_light_years() {
        let observer = Observer::new(at_ly([-25_000.0, 0.0, 0.0]), UniverseTime::EPOCH).unwrap();
        let star = Drift::new(
            at_ly([25_000.0, 0.0, 0.0]),
            GalacticVelocity::new([220e3, 0.0, 0.0]),
        );
        let seen = retarded(&observer, &star);
        let first = seen.distance_now().value();
        let correction = first - seen.light_age().as_julian_years_f64();
        assert!(
            (correction - 37.0).abs() <= 1.0,
            "{correction} years of correction"
        );
        // It is d v_r ÷ c² to first order: 36.69 years.
        let expected = 50_000.0 * 220e3 / SPEED_OF_LIGHT;
        assert!(
            (correction - expected).abs() < 0.05,
            "{correction} against {expected}"
        );
        // What the step leaves is (v_r ÷ c)² of the light time, ten days, which the closed form
        // shows.
        let exact = retarded_exact_linear(&observer, star.epoch_position(), &star.velocity());
        let gap = (seconds(seen.light_age()) - seconds(exact.light_age())).abs();
        let beta = 220e3 / SPEED_OF_LIGHT;
        let residual = beta * beta * seconds(exact.light_age());
        assert!(
            gap <= residual + 1e-2,
            "{gap} s from the closed form, over {residual} s"
        );
        assert!(
            gap > 0.5 * residual,
            "{gap} s: the radial residual is the bound itself"
        );
        assert!((residual / 86_400.0 - 10.0).abs() < 0.5, "{residual} s");
    }

    /// No light received inside the cube is older than L: every pair of points, the corners
    /// included, and sources moving at up to 3,000 km/s.
    #[test]
    fn retarded_light_is_never_older_than_the_light_crossing_bound() {
        let mut lcg = Lcg::new(0x1201_0002);
        let half = f64::from(ROOT_HALF_WIDTH_LY) - 1e-6;
        let corner = |sx: f64, sy: f64, sz: f64| at_ly([sx * half, sy * half, sz * half]);
        let mut pairs = vec![
            (corner(1.0, 1.0, 1.0), corner(-1.0, -1.0, -1.0)),
            (corner(-1.0, 1.0, -1.0), corner(1.0, -1.0, 1.0)),
        ];
        for _ in 0..2_000 {
            pairs.push((in_cube(&mut lcg), in_cube(&mut lcg)));
        }
        for (i, (from, to)) in pairs.into_iter().enumerate() {
            let t = if i % 2 == 0 {
                ClockWindow::START
            } else {
                ClockWindow::END
            };
            let observer = Observer::new(from, t).unwrap();
            let v = velocity(&mut lcg, 3_000.0);
            let seen = retarded(&observer, &Drift::new(to, v));
            assert!(
                seen.light_age() <= LIGHT_CROSSING_L,
                "{:?}",
                seen.light_age()
            );
            assert!(SourceHorizon::contains(seen.emitted()));
            assert_eq!(seen.observed(), t);
        }
    }

    /// Extrapolating a drifting source's observed position and velocity lands on its present
    /// position, whatever the light's age: from the closed form's emitted time and from the step's.
    #[test]
    fn retarded_extrapolation_of_a_drift_lands_on_the_line() {
        let mut lcg = Lcg::new(0x1201_0003);
        let mut worst = 0.0_f64;
        for _ in 0..2_000 {
            let observer = Observer::new(in_cube(&mut lcg), years(750)).unwrap();
            let line = Drift::new(in_cube(&mut lcg), velocity(&mut lcg, 1_000.0));
            let present = line.position_at(observer.time());
            for seen in [
                retarded(&observer, &line),
                retarded_exact_linear(&observer, line.epoch_position(), &line.velocity()),
            ] {
                let landed = extrapolate_to_present(&seen, observer.time());
                let miss = landed.distance_to(&present).value();
                worst = worst.max(miss);
            }
        }
        assert!(worst < 1.0, "an extrapolation missed the line by {worst} m");
    }

    /// The drift is as good as the representation however far back it goes, and agrees with plan
    /// 03's single-rounding drift inside the clock window.
    #[test]
    fn retarded_drift_does_not_lose_precision_with_the_span() {
        let start = at_ly([12_345.678, -26_000.25, 310.5]);
        let v = GalacticVelocity::new([-231_456.789, 12_345.6, -7_890.123]);
        let line = Drift::new(start, v);
        let far = SourceHorizon::START;
        let there = line.position_at(far);
        // Back from the far end by the same span, at the negated velocity.
        let back = Drift::new(there, -v).position_at(far);
        let miss = back.distance_to(&start).value();
        assert!(miss < 2.0, "{miss} m after two drifts of 263,144 years");
        // The drift itself is the velocity times the span, to a part in 10¹³ of the span.
        let moved = start.displacement_to(&there).metres();
        let span = far.since_epoch().as_seconds_f64();
        for (moved, v) in moved.into_iter().zip(v.metres_per_second()) {
            let expected = v * span;
            assert!(
                (moved - expected).abs() <= 1e-13 * expected.abs() + 2.0,
                "{moved} against {expected}"
            );
        }
        // Inside the window it agrees with GalacticPosition::translated's single rounding.
        let elapsed = crate::units::Seconds::new(years(1_000).since_epoch().as_seconds_f64());
        let single = start.translated(v.displacement_over(elapsed)).unwrap();
        let double = line.position_at(years(1_000));
        assert!(single.distance_to(&double).value() < 2.0);
        // At the epoch the line is the epoch position itself, bit for bit.
        assert_eq!(line.position_at(UniverseTime::EPOCH), start);
    }

    /// A drift that ends within half a unit in the last place below a whole light-year, which
    /// rounds up to it, lands at the next cell's start (the `Drift` example's case: a thousandth
    /// of c for a thousand years from a whole light-year).
    #[test]
    fn retarded_drift_to_just_below_a_whole_light_year_takes_the_next_cell() {
        let start = at_ly([0.0, 26_000.0, 0.0]);
        let line = Drift::new(
            start,
            GalacticVelocity::new([SPEED_OF_LIGHT / 1_000.0, 0.0, 0.0]),
        );
        let later = line.position_at(years(1_000));
        let moved = start.distance_to(&later).value();
        assert!((moved - METRES_PER_LIGHT_YEAR).abs() < 1.0, "{moved} m");
        for k in 0..2_000_i64 {
            let v = SPEED_OF_LIGHT / 1_000.0 * (1.0 + f64::from(i32::try_from(k).unwrap()) * 1e-16);
            let p =
                Drift::new(start, GalacticVelocity::new([v, 0.0, 0.0])).position_at(years(1_000));
            assert!(p.offset_metres()[0] < METRES_PER_LIGHT_YEAR);
        }
    }

    #[test]
    fn retarded_observer_rejects_positions_outside_the_cube_and_times_outside_the_window() {
        let outside = at_ly([70_000.0, 0.0, 0.0]);
        assert_eq!(
            Observer::new(outside, UniverseTime::EPOCH),
            Err(BuildObserverError::OutsideRootCube)
        );
        let inside = at_ly([0.0, 26_000.0, 0.0]);
        let late = years(1_001);
        assert_eq!(
            Observer::new(inside, late),
            Err(BuildObserverError::TimeOutsideClockWindow(late))
        );
        assert!(Observer::new(inside, ClockWindow::START).is_ok());
        assert!(Observer::new(inside, ClockWindow::END).is_ok());
        for error in [
            BuildObserverError::OutsideRootCube,
            BuildObserverError::TimeOutsideClockWindow(late),
        ] {
            let message = error.to_string();
            assert!(!message.chars().next().unwrap().is_uppercase(), "{message}");
            assert!(!message.ends_with('.'), "{message}");
        }
    }

    #[test]
    fn retarded_light_time_rounds_to_the_nanosecond() {
        assert_eq!(light_time(Metres::ZERO), Span::ZERO);
        // 1 m is 3.3356 ns of light: rounded to 3 ns, not floored.
        assert_eq!(light_time(Metres::new(1.0)), Span::new(0, 3).unwrap());
        // 1.1 m is 3.669 ns, which rounds up.
        assert_eq!(light_time(Metres::new(1.1)), Span::new(0, 4).unwrap());
        let age = light_time(Metres::from(LightYears::new(227_023.0)));
        assert!((age.as_julian_years_f64() - 227_023.0).abs() < 1e-9);
    }

    #[test]
    fn retarded_is_the_same_twice_and_static_sources_are_seen_where_they_are() {
        let observer = Observer::new(at_ly([0.0, 26_000.0, 0.0]), years(-300)).unwrap();
        let star = Drift::new(at_ly([100.0, 26_050.0, -30.0]), GalacticVelocity::default());
        let a = retarded(&observer, &star);
        let b = retarded(&observer, &star);
        assert_eq!(a, b);
        assert_eq!(a.apparent_position(), star.epoch_position());
        assert_eq!(a.motion(), Motion::Drift);
        assert_eq!(
            a.light_age(),
            light_time(observer.position().distance_to(star.epoch_position()))
        );
    }
}
