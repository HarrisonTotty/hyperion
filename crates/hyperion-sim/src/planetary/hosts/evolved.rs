//! Evolved hosts: what an orbit becomes as its host loses mass, grows and dies (plan 14, design
//! notes 9 and 11, P14.T28.b and c).
//!
//! The closed forms the fate transform ([`fate`](crate::planetary::fate)) strings together:
//!
//! - **Circularisation** (P14.T8.e, applied first): tides raised on the planet by its host damp
//!   its eccentricity as e(t) = e₀ exp(−(age + t) ÷ `τ_c`) at constant orbital angular momentum, so
//!   a (1 − e²) is kept and a(t) = a₀ (1 − e₀²) ÷ (1 − e(t)²), which stays between the primordial
//!   pericentre and semi-major axis (design note 9). Circularisation has one definition, T8.e's
//!   (ruling 62.5): the time `τ_c` is its constant-Q form
//!   ([`circularisation_time`](crate::planetary::placement::classes::tides::circularisation_time),
//!   Goldreich and Soter 1966), which the generator passes as a [`Circularisation`], and the
//!   damping is its [`circularise`](crate::planetary::placement::classes::tides::circularise).
//! - **Expansion** (design note 11): winds are slow against any planetary period inside about
//!   1,000 au, so an orbit expands adiabatically, a(t) = a₀ M₀ ÷ M(t) with the eccentricity kept,
//!   M being the mass the body orbits: its star's, or a circumbinary body's pair's
//!   ([`KeplerElements::scaled`]; Veras et al. 2011).
//! - **Engulfment** (design note 11, ruling 62): a body is destroyed at the first time its
//!   semi-major axis is inside f times the largest radius its host has had, f = (1 + `M_p` ÷
//!   3.1 M⊕)^⅛, 1.04 for the Earth and 1.79 for Jupiter ([`engulfment_reach`], fitted to Mustill
//!   and Villaver 2012). The time is found by a scan and a bisection of fixed points
//!   (`first_engulfment`).
//! - **Supernovae** (design note 11): a sudden death is instantaneous against the orbit, so the
//!   body keeps its position and velocity while its host's mass drops to the remnant's and the
//!   host's velocity changes by the natal kick; the new orbit is [`elements_from_state`]'s, bound
//!   or not, and a bound one whose pericentre is inside the remnant's fluid Roche limit ends in
//!   the body's disruption (`supernova`).

use std::error::Error;
use std::f64::consts::{PI, TAU};
use std::fmt;

use crate::coords::{SystemVector, SystemVelocity};
use crate::math;
use crate::orbit::{Eccentricity, InvertStateError, KeplerElements, Orbit, elements_from_state};
use crate::planetary::derive::roche_limit_fluid;
use crate::planetary::params::ENGULFMENT_TIDAL_MASS;
use crate::planetary::placement::classes::tides;
use crate::time::{NANOS_PER_SECOND, Span, UniverseTime};
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{
    EarthMasses, GravitationalParameter, KilogramsPerCubicMetre, Metres, SolarMasses, Years,
};

/// A [`Circularisation`] could not be built from the timescale given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildCircularisationError {
    /// The timescale was not positive, or was NaN.
    TimescaleNotPositive(Years),
}

impl fmt::Display for BuildCircularisationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimescaleNotPositive(timescale) => write!(
                f,
                "circularisation timescale {} yr is not positive",
                timescale.value()
            ),
        }
    }
}

impl Error for BuildCircularisationError {}

/// How a body's eccentricity decays under the tides its host raises on it (P14.T8.e): not at all,
/// or with an e-folding time `τ_c`.
///
/// T8.e defines `τ_c` from the constant-Q form of Goldreich and Soter (1966), with Q′ = 10⁶ for
/// giants and 10² for rocky bodies; the fate transform takes the time as a plain value and
/// applies the decay first.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Circularisation(Option<Years>);

impl Circularisation {
    /// No circularisation: the eccentricity keeps its primordial value.
    pub const NONE: Self = Self(None);

    /// Circularisation with e-folding time `timescale`, which may be infinite (none, in effect).
    ///
    /// # Errors
    ///
    /// [`BuildCircularisationError::TimescaleNotPositive`] unless `timescale` is positive.
    pub fn new(timescale: Years) -> Result<Self, BuildCircularisationError> {
        if timescale.value() > 0.0 {
            Ok(Self(Some(timescale)))
        } else {
            Err(BuildCircularisationError::TimescaleNotPositive(timescale))
        }
    }

    /// The e-folding time of the eccentricity, if there is one.
    #[must_use]
    pub const fn timescale(&self) -> Option<Years> {
        self.0
    }
}

/// The engulfment reach f of a planet of `mass`, in host radii (design note 11, ruling 62):
/// f = (1 + `M_p` ÷ `M_c`)^⅛ with `M_c` = [`ENGULFMENT_TIDAL_MASS`], 3.1 M⊕.
///
/// A planet is destroyed once its semi-major axis is inside f times the largest radius its host
/// has had. The law is Zahn's (1977) equilibrium tide, whose drag on a planet grows as its mass
/// times (R★ ÷ a)⁸, fitted to Mustill and Villaver's (2012) critical axes
/// ([`ENGULFMENT_TIDAL_MASS`] has the fit): a planet of negligible mass meets only the
/// photosphere, and a Jupiter is drawn in from 1.79 host radii. A mass that is not a positive
/// number takes the reach of none, 1.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::hosts::evolved::engulfment_reach;
/// use hyperion_sim::units::EarthMasses;
///
/// let reach = |m: f64| engulfment_reach(EarthMasses::new(m));
/// assert!((reach(1.0) - 1.036).abs() < 1e-3); // the Earth
/// assert!((reach(17.1) - 1.264).abs() < 1e-3); // Neptune
/// assert!((reach(317.8) - 1.786).abs() < 1e-3); // Jupiter
/// ```
#[must_use]
pub fn engulfment_reach(mass: EarthMasses) -> f64 {
    let m = mass.value();
    if m.is_nan() || m <= 0.0 {
        return 1.0;
    }
    // The eighth root by three square roots, each exactly rounded.
    (1.0 + m / ENGULFMENT_TIDAL_MASS.value())
        .sqrt()
        .sqrt()
        .sqrt()
}

/// `orbit` circularised to the host age `host_age` (P14.T8.e): e = e₀ exp(−age ÷ `τ_c`) and
/// a = a₀ (1 − e₀²) ÷ (1 − e²), with the orientation, the mean anomaly at the epoch and the
/// gravitational parameter kept, and the period from the new axis.
///
/// `orbit` is returned unchanged, bit for bit, when it is circular or does not circularise. The
/// age is held at zero and above, since tides act only on a body that exists.
#[must_use]
pub(crate) fn circularised(
    orbit: &KeplerElements,
    circularisation: Circularisation,
    host_age: Years,
) -> KeplerElements {
    let Some((e, a)) = damped(orbit, circularisation, host_age) else {
        return *orbit;
    };
    KeplerElements::from_semi_major_axis(
        a,
        orbit.gravitational_parameter(),
        Eccentricity::new(e).expect("a damped eccentricity lies in [0, e₀]"),
        *orbit.orientation(),
        orbit.mean_anomaly_at_epoch(),
    )
    .expect("an axis between a circular orbit's and its pericentre's stays representable")
}

/// The semi-major axis of [`circularised`], m, bit for bit, without building the orbit: the
/// engulfment search's, which evaluates it a hundred times a body.
#[must_use]
pub(crate) fn circularised_axis(
    orbit: &KeplerElements,
    circularisation: Circularisation,
    host_age: Years,
) -> Metres {
    damped(orbit, circularisation, host_age).map_or(orbit.semi_major_axis(), |(_, a)| a)
}

/// The eccentricity and axis `orbit` is damped to at `host_age`, or `None` if it is circular or
/// does not circularise.
///
/// The damping is P14.T8.e's own, [`tides::circularise`], which defines it (ruling 62.5): the
/// transform keeps no copy of the law.
fn damped(
    orbit: &KeplerElements,
    circularisation: Circularisation,
    host_age: Years,
) -> Option<(f64, Metres)> {
    let e0 = orbit.eccentricity().value();
    let timescale = circularisation.timescale()?;
    if e0 <= 0.0 {
        return None;
    }
    let elapsed = Years::new(host_age.value().max(0.0));
    let (a, e) = tides::circularise(orbit.semi_major_axis(), e0, timescale, elapsed);
    Some((e, a))
}

/// `orbit`, whose elements are about the mass `reference`, expanded adiabatically to the mass
/// `now` (design note 11): the axis times `reference` ÷ `now` and the gravitational parameter
/// times `now` ÷ `reference`, so that the specific angular momentum √(μ a (1 − e²)) is kept.
///
/// `orbit` is returned unchanged, bit for bit, when the two masses are the same.
///
/// # Panics
///
/// If `now` is not positive: a body orbits a positive mass while it is bound, which the fate
/// transform keeps.
#[must_use]
pub(crate) fn expanded(
    orbit: &KeplerElements,
    reference: SolarMasses,
    now: SolarMasses,
) -> KeplerElements {
    if now.total_cmp(&reference).is_eq() {
        return *orbit;
    }
    let factor = reference.value() / now.value();
    let mu = orbit.gravitational_parameter() * (now.value() / reference.value());
    orbit
        .scaled(factor, mu)
        .expect("a bound body orbits a positive mass, and winds scale an orbit by under 10⁴")
}

/// The semi-major axis of [`expanded`], m, bit for bit, from the axis `axis` about `reference`,
/// without building the orbit ([`KeplerElements::scaled`] multiplies the axis by the same factor).
#[must_use]
pub(crate) fn expanded_axis(axis: Metres, reference: SolarMasses, now: SolarMasses) -> Metres {
    if now.total_cmp(&reference).is_eq() {
        axis
    } else {
        axis * (reference.value() / now.value())
    }
}

/// The points of the engulfment scan between a segment's start and end, besides the two ends.
pub(crate) const ENGULFMENT_SCAN_POINTS: u32 = 64;

/// The ratio of the time left to a segment's end at one point of the engulfment scan to the time
/// left at the one before: each step of the scan covers a quarter of the time then left.
pub(crate) const ENGULFMENT_SCAN_RATIO: f64 = 0.75;

/// The most halvings of the engulfment bisection: enough to take any bracket on the universe
/// clock to under a second (2⁶⁴ ns is 585 years, and 2⁶⁴ s is far beyond the clock).
pub(crate) const ENGULFMENT_BISECTION_STEPS: u32 = 64;

/// The first time in `start`–`end` at which `clearance` is negative, or `None` if there is none:
/// the time of engulfment, with `clearance` the body's semi-major axis less the engulfment reach
/// times its host's largest radius (P14.T28.b).
///
/// Design note 11 calls the clearance monotone, and so it is in each piece of the host's life, but
/// not across them: winds widen the orbit after the red-giant tip while the host's largest radius
/// stays at the tip's, so a planet engulfed there can see the clearance positive again by the end
/// of the asymptotic giant branch (along a 1 M☉ track, any rocky planet born at 1.06–1.31 au).
/// So the first crossing is found by a scan and then a bisection, both of fixed points, and a
/// clearance negative anywhere in the scan's bracket counts from its first crossing:
///
/// - the clearance at `start`, then at `end` − (`end` − `start`) × 0.75ᵏ for k = 1 to 64
///   ([`ENGULFMENT_SCAN_POINTS`], [`ENGULFMENT_SCAN_RATIO`]), then at `end`, which puts the points
///   ever closer as the end approaches, where a host's giant phases are;
/// - at the first point with a negative clearance, a bisection of the bracket from the point before
///   it, on the clock's nanoseconds, to within a second, in at most 64 halvings
///   ([`ENGULFMENT_BISECTION_STEPS`]); the time returned is the bracket's later end, at which the
///   clearance is negative.
///
/// A dip that begins and ends between two points of the scan, shorter than a quarter of the time
/// left to `end`, is missed. Along the tracks the engulfment of a red-giant tip lasts through core
/// helium burning, about a tenth of the time then left, so none is. The points depend on `start`
/// and `end` alone, never on a query time, so every query of a body sees the same engulfment.
pub(crate) fn first_engulfment(
    start: UniverseTime,
    end: UniverseTime,
    mut clearance: impl FnMut(UniverseTime) -> f64,
) -> Option<UniverseTime> {
    if clearance(start) < 0.0 {
        return Some(start);
    }
    if end <= start {
        return None;
    }
    let span = end
        .checked_since(start)
        .expect("end is after start on the clock")
        .as_seconds_f64();
    let mut previous = start;
    let mut left = 1.0;
    for k in 1..=ENGULFMENT_SCAN_POINTS + 1 {
        let point = if k > ENGULFMENT_SCAN_POINTS {
            end
        } else {
            left *= ENGULFMENT_SCAN_RATIO;
            Span::from_seconds_f64(span * left)
                .and_then(|back| end.checked_sub(back))
                .expect("a point between start and end is on the clock")
        };
        if point <= previous {
            continue;
        }
        if clearance(point) < 0.0 {
            return Some(bisect(previous, point, &mut clearance));
        }
        previous = point;
    }
    None
}

/// The bracket's later end after halving `lo`–`hi` (clearance at `lo` not negative, at `hi`
/// negative) to within a second.
fn bisect(
    mut lo: UniverseTime,
    mut hi: UniverseTime,
    clearance: &mut impl FnMut(UniverseTime) -> f64,
) -> UniverseTime {
    let second = i128::from(NANOS_PER_SECOND);
    for _ in 0..ENGULFMENT_BISECTION_STEPS {
        let (low, high) = (nanos(lo), nanos(hi));
        if high - low <= second {
            break;
        }
        let mid = from_nanos(low + (high - low) / 2);
        if clearance(mid) < 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// `t` in nanoseconds since the epoch.
fn nanos(t: UniverseTime) -> i128 {
    i128::from(t.seconds()) * i128::from(NANOS_PER_SECOND) + i128::from(t.subsec_nanos())
}

/// The clock time `n` nanoseconds after the epoch, for an `n` between two clock times.
fn from_nanos(n: i128) -> UniverseTime {
    let per_second = i128::from(NANOS_PER_SECOND);
    let seconds = i64::try_from(n.div_euclid(per_second)).expect("between two clock times");
    let rest = u32::try_from(n.rem_euclid(per_second)).expect("a remainder below 10⁹");
    UniverseTime::new(seconds, rest).expect("between two clock times")
}

/// What a supernova leaves of a body's orbit (P14.T28.c).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Aftermath {
    /// The body is bound on these elements, about the mass left.
    Bound(KeplerElements),
    /// The body's new orbit carries it inside the remnant's Roche limit, where it is disrupted at
    /// `at`: at its next pericentre passage on `orbit`, or at once where there is none to follow.
    Disrupted {
        /// The orbit it follows until then.
        orbit: Option<KeplerElements>,
        /// When it is disrupted.
        at: UniverseTime,
    },
    /// The body is no longer bound.
    Unbound,
}

/// What a supernova at `at` leaves of a body on `before`, the orbit it was on just before, when
/// the mass it orbits becomes `mu_after` ÷ G and its host's velocity changes by `kick` (design
/// note 11, P14.T28.c).
///
/// The body keeps its position and velocity relative to where the host was, the host moves off at
/// `kick`, so the relative velocity loses `kick`, and the new orbit is [`elements_from_state`]'s
/// about `mu_after`. A bound orbit whose pericentre is inside the fluid Roche limit of the mass
/// left for the body's `density` ends in its disruption at its next pericentre, or at once if it
/// is inside that limit already. The rest are [`Aftermath::Unbound`]: an open orbit, no mass left
/// (a pair-instability or thermonuclear death), and the states that have no orbit:
///
/// - an open orbit, which [`elements_from_state`] gives from e = 0.9999 whether or not the body
///   is bound, since a bound one's apocentre is then over 2 × 10⁴ times its pericentre, beyond
///   where a system keeps anything; but if its pericentre is inside the Roche limit and the body
///   is falling towards it, it is disrupted, dated to the supernova since the fall that follows
///   takes a fraction of the old period;
/// - a radial state (no angular momentum, which only a kick along the line of centres gives) is
///   disrupted at once if it is bound, since it falls straight in, and unbound otherwise;
/// - a bound state whose period or pericentre overflows is bound so weakly that it lies far
///   beyond any system's tidal radius;
/// - a non-finite state, which a representable orbit never gives.
#[must_use]
pub(crate) fn supernova(
    before: &KeplerElements,
    at: UniverseTime,
    mu_after: GravitationalParameter,
    kick: SystemVelocity,
    density: KilogramsPerCubicMetre,
) -> Aftermath {
    if mu_after.value().is_nan() || mu_after.value() <= 0.0 {
        return Aftermath::Unbound;
    }
    let (r, v) = before.relative_state_at(at);
    let v = {
        let (v, k) = (v.metres_per_second(), kick.metres_per_second());
        SystemVelocity::new([v[0] - k[0], v[1] - k[1], v[2] - k[2]])
    };
    let limit = remnant_roche_limit(mu_after, density);
    let distance = length(r.metres());
    match elements_from_state(r, v, mu_after, at) {
        Ok(Orbit::Bound(orbit)) => {
            if orbit.periapsis() >= limit {
                Aftermath::Bound(orbit)
            } else if distance <= limit.value() {
                Aftermath::Disrupted { orbit: None, at }
            } else {
                Aftermath::Disrupted {
                    orbit: Some(orbit),
                    at: next_pericentre(&orbit, at),
                }
            }
        }
        Ok(Orbit::Open(open)) if open.pericentre() < limit && radial_speed(&r, &v) <= 0.0 => {
            Aftermath::Disrupted { orbit: None, at }
        }
        Err(InvertStateError::Radial) if specific_energy(&r, &v, mu_after) < 0.0 => {
            Aftermath::Disrupted { orbit: None, at }
        }
        Ok(Orbit::Open(_))
        | Err(
            InvertStateError::Radial
            | InvertStateError::PericentreTimeOutOfRange
            | InvertStateError::Unrepresentable(_)
            | InvertStateError::NotFinite
            | InvertStateError::GravitationalParameterNotPositive { .. },
        ) => Aftermath::Unbound,
    }
}

/// The fluid Roche limit about the mass μ ÷ G for a body of `density`: 2.456 (3M ÷ 4πρ)^⅓
/// ([`roche_limit_fluid`], P14.T15), taken as that of a primary of the body's own density, whose
/// radius (3M ÷ 4πρ)^⅓ makes R³ρ the primary's mass, so that the remnant's own radius, which the
/// limit does not depend on, is not needed.
///
/// A planet is held together by its gravity, not its strength, so the fluid limit, not the rigid
/// one, is where tides pull it apart.
pub(crate) fn remnant_roche_limit(
    mu: GravitationalParameter,
    density: KilogramsPerCubicMetre,
) -> Metres {
    let mass = mu.value() / GRAVITATIONAL_CONSTANT;
    let radius = math::cbrt(3.0 * mass / (4.0 * PI * density.value()));
    roche_limit_fluid(Metres::new(radius), density, density)
}

/// The first pericentre passage of `orbit` at or after `at`.
fn next_pericentre(orbit: &KeplerElements, at: UniverseTime) -> UniverseTime {
    let anomaly = orbit.mean_anomaly_at(at).value();
    if anomaly <= 0.0 {
        return at;
    }
    let wait = (TAU - anomaly) / TAU * orbit.period().value();
    Span::from_seconds_f64(wait)
        .and_then(|span| at.checked_add(span))
        .expect("a planetary period after a clock time is on the clock")
}

/// The rate at which the body recedes from the host, r · v ÷ |r|, m s⁻¹: negative while it falls
/// towards it.
fn radial_speed(r: &SystemVector, v: &SystemVelocity) -> f64 {
    let (r, v) = (r.metres(), v.metres_per_second());
    (r[0] * v[0] + r[1] * v[1] + r[2] * v[2]) / length(r)
}

/// |x|, summed in the fixed order x, y, z.
fn length(x: [f64; 3]) -> f64 {
    (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt()
}

/// The specific orbital energy v² ÷ 2 − μ ÷ r, J kg⁻¹.
fn specific_energy(r: &SystemVector, v: &SystemVelocity, mu: GravitationalParameter) -> f64 {
    let speed = length(v.metres_per_second());
    0.5 * speed * speed - mu.value() / length(r.metres())
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::orbit::Orientation;
    use crate::units::consts::{GM_SUN, METRES_PER_AU, SECONDS_PER_JULIAN_YEAR};
    use crate::units::{AstronomicalUnits, JupiterMasses, Radians};

    fn orientation() -> Orientation {
        Orientation::new(Radians::new(0.3), Radians::new(1.1), Radians::new(2.0))
            .expect("valid angles")
    }

    fn orbit(a_au: f64, e: f64, solar_masses: f64) -> KeplerElements {
        KeplerElements::from_semi_major_axis(
            Metres::from(AstronomicalUnits::new(a_au)),
            GravitationalParameter::from_solar_masses(SolarMasses::new(solar_masses)),
            Eccentricity::new(e).expect("valid"),
            orientation(),
            Radians::new(0.7),
        )
        .expect("valid orbit")
    }

    fn years(y: f64) -> UniverseTime {
        UniverseTime::EPOCH
            .checked_add(Span::from_seconds_f64(y * SECONDS_PER_JULIAN_YEAR).expect("finite"))
            .expect("on the clock")
    }

    #[test]
    fn the_reach_is_the_eighth_root_of_one_plus_the_mass_over_the_tidal_mass() {
        assert_same_bits(engulfment_reach(EarthMasses::new(0.0)), 1.0);
        assert_same_bits(engulfment_reach(EarthMasses::new(f64::NAN)), 1.0);
        assert_same_bits(
            engulfment_reach(EarthMasses::new(3.1)),
            2.0_f64.sqrt().sqrt().sqrt(),
        );
        let jupiter = EarthMasses::from(JupiterMasses::new(1.0));
        let f = engulfment_reach(jupiter);
        let eighth = f * f * f * f * f * f * f * f;
        assert!((eighth - (1.0 + jupiter.value() / 3.1)).abs() < 1e-12 * eighth);
        let mut last = 1.0;
        for i in 0..100 {
            let f = engulfment_reach(EarthMasses::new(math::exp10(f64::from(i) * 0.04 - 1.0)));
            assert!(f >= last && f < 2.5, "{f}");
            last = f;
        }
    }

    /// The fit against Mustill and Villaver's (2012) Figure 7, as read from its vector paths: for
    /// each star, its largest AGB radius and the critical initial axes of the circular Terrestrial,
    /// Neptunian and Jovian planets, au. With each star's share of its mass left at its largest
    /// radius the best for it, f reproduces every critical axis within 4%.
    #[test]
    fn the_reach_reproduces_mustill_and_villavers_critical_axes() {
        let figure: [(f64, [f64; 4]); 6] = [
            (1.0, [1.580, 1.480, 1.870, 2.690]),
            (1.5, [2.410, 1.870, 2.240, 3.190]),
            (2.0, [3.000, 2.140, 2.490, 3.530]),
            (2.5, [3.300, 2.300, 2.680, 3.800]),
            (3.5, [4.150, 2.380, 3.000, 4.270]),
            (5.0, [5.180, 2.830, 3.490, 5.000]),
        ];
        let reach = [1.0, 17.1, 318.0].map(|m| engulfment_reach(EarthMasses::new(m)));
        let mut squares = 0.0;
        for (star, [radius, axes @ ..]) in figure {
            // The share left at the largest radius that fits this star best, in the logarithm.
            let share = math::exp(
                (0..3)
                    .map(|p| math::ln(axes[p] / (reach[p] * radius)))
                    .sum::<f64>()
                    / 3.0,
            );
            for p in 0..3 {
                let predicted = reach[p] * share * radius;
                let off = math::ln(predicted / axes[p]);
                assert!(
                    off.abs() < 0.04,
                    "{star} M_sun, planet {p}: {predicted} au against {}",
                    axes[p]
                );
                squares += off * off;
            }
        }
        assert!((squares / 18.0).sqrt() < 0.02);
    }

    #[test]
    fn circularisation_keeps_the_angular_momentum_and_stays_between_pericentre_and_axis() {
        let hot = orbit(0.05, 0.3, 1.0);
        let tidal = Circularisation::new(Years::new(1e8)).expect("positive");
        let h = |k: &KeplerElements| {
            let e = k.eccentricity().value();
            (k.gravitational_parameter().value() * k.semi_major_axis().value() * (1.0 - e * e))
                .sqrt()
        };
        let mut last_e = 0.3;
        for age in [0.0, 1e6, 1e8, 5e8, 5e9] {
            let c = circularised(&hot, tidal, Years::new(age));
            let e = c.eccentricity().value();
            assert!((e - 0.3 * math::exp(-age / 1e8)).abs() < 1e-15);
            assert!(e <= last_e);
            last_e = e;
            assert!(
                (h(&c) / h(&hot) - 1.0).abs() < 1e-14,
                "angular momentum at {age}"
            );
            assert!(c.semi_major_axis() <= hot.semi_major_axis());
            assert!(c.semi_major_axis() >= hot.periapsis());
            assert_same_bits(c.inclination().value(), hot.inclination().value());
            assert_same_bits(
                c.mean_anomaly_at_epoch().value(),
                hot.mean_anomaly_at_epoch().value(),
            );
        }
        // After 50 e-foldings it ends at a (1 − e²).
        let end = circularised(&hot, tidal, Years::new(5e9));
        let expected = hot.semi_major_axis().value() * (1.0 - 0.09);
        assert!((end.semi_major_axis().value() / expected - 1.0).abs() < 1e-15);
        // No circularisation, or a circular orbit, leave the orbit as it is.
        assert_eq!(
            circularised(&hot, Circularisation::NONE, Years::new(5e9)),
            hot
        );
        let round = orbit(0.05, 0.0, 1.0);
        assert_eq!(circularised(&round, tidal, Years::new(5e9)), round);
        assert_eq!(
            Circularisation::new(Years::new(0.0)),
            Err(BuildCircularisationError::TimescaleNotPositive(Years::new(
                0.0
            )))
        );
        assert_eq!(
            BuildCircularisationError::TimescaleNotPositive(Years::new(-1.0)).to_string(),
            "circularisation timescale -1 yr is not positive"
        );
    }

    #[test]
    fn expansion_scales_the_axis_by_the_mass_lost_and_keeps_the_rest() {
        let earth = orbit(1.0, 0.0167, 1.0);
        let wide = expanded(&earth, SolarMasses::new(1.0), SolarMasses::new(0.52));
        assert_same_bits(
            wide.semi_major_axis().value(),
            earth.semi_major_axis().value() * (1.0 / 0.52),
        );
        assert_same_bits(wide.eccentricity().value(), earth.eccentricity().value());
        assert!((wide.gravitational_parameter().value() / (GM_SUN * 0.52) - 1.0).abs() < 1e-15);
        assert_eq!(
            expanded(&earth, SolarMasses::new(1.0), SolarMasses::new(1.0)),
            earth
        );
    }

    #[test]
    fn the_scan_finds_the_first_crossing_even_when_the_clearance_recovers() {
        // A clearance that dips below zero between 700 and 800 years and is positive again at the
        // end, as a planet engulfed at the red-giant tip and widened by the winds after it.
        let start = years(-1000.0);
        let end = years(1000.0);
        let dip = |t: UniverseTime| {
            let y = t.since_epoch().as_julian_years_f64();
            if (700.0..800.0).contains(&y) {
                -1.0
            } else {
                1.0
            }
        };
        let found = first_engulfment(start, end, dip).expect("the dip is found");
        let at = found.since_epoch().as_julian_years_f64();
        assert!(
            (at - 700.0).abs() < 1.0 / SECONDS_PER_JULIAN_YEAR * 1.01,
            "at {at}"
        );
        // A clearance negative at the start is engulfed at the start.
        assert_eq!(first_engulfment(start, end, |_| -1.0), Some(start));
        assert_eq!(first_engulfment(start, end, |_| 1.0), None);
        // A crossing at a known time, found to within a second, on its negative side.
        let crossing = years(123.456);
        let linear = |t: UniverseTime| {
            -(t.since_epoch().as_seconds_f64() - crossing.since_epoch().as_seconds_f64())
        };
        let found = first_engulfment(start, end, linear).expect("found");
        let off = found.since_epoch().as_seconds_f64() - crossing.since_epoch().as_seconds_f64();
        assert!((0.0..=1.0).contains(&off), "{off} s");
        // The points are those of the bracket alone.
        let mut seen = Vec::new();
        let _ = first_engulfment(start, end, |t| {
            seen.push(t);
            1.0
        });
        assert_eq!(seen.len(), 66);
        assert!(seen.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(seen[0], start);
        assert_eq!(*seen.last().expect("points"), end);
    }

    /// Test (c): half the mass lost at once with no kick unbinds a circular orbit.
    #[test]
    fn losing_half_the_mass_at_once_unbinds_a_circular_orbit() {
        let density = KilogramsPerCubicMetre::new(5_500.0);
        let before = orbit(5.0, 0.0, 10.0);
        let at = years(0.0);
        let mu = |m: f64| GravitationalParameter::from_solar_masses(SolarMasses::new(m));
        let kick = SystemVelocity::ZERO;
        for kept in [0.1, 0.3, 0.49, 0.499_999] {
            assert_eq!(
                supernova(&before, at, mu(10.0 * kept), kick, density),
                Aftermath::Unbound,
                "keeping {kept} of the mass"
            );
        }
        assert_eq!(
            supernova(&before, at, mu(0.0), kick, density),
            Aftermath::Unbound
        );
        // Keeping more than half leaves it bound, at its old radius, now the pericentre: e =
        // ΔM ÷ M_after and a = r M_after ÷ (2 M_after − M_before). From e = 0.9999, within 5 × 10⁻⁵
        // of half, its apocentre is beyond 2 × 10⁴ times its pericentre, and it counts as unbound.
        assert_eq!(
            supernova(&before, at, mu(5.000_1), kick, density),
            Aftermath::Unbound
        );
        for kept in [0.500_1, 0.6, 0.9, 0.999_999_99] {
            let Aftermath::Bound(after) = supernova(&before, at, mu(10.0 * kept), kick, density)
            else {
                panic!("keeping {kept} of the mass leaves the orbit bound");
            };
            let e = (1.0 - kept) / kept;
            let a = 5.0 * METRES_PER_AU * kept / (2.0 * kept - 1.0);
            assert!(
                (after.eccentricity().value() - e).abs() < 1e-9,
                "e at {kept}"
            );
            assert!(
                (after.semi_major_axis().value() / a - 1.0).abs() < 1e-9,
                "a at {kept}"
            );
            assert!((after.periapsis().value() / (5.0 * METRES_PER_AU) - 1.0).abs() < 1e-9);
        }
    }

    /// Test (c): kicked ones do not keep their bodies.
    #[test]
    fn a_kick_faster_than_escape_unbinds_what_the_mass_loss_would_not() {
        let density = KilogramsPerCubicMetre::new(1_330.0);
        let before = orbit(30.0, 0.0, 8.3);
        let at = years(0.0);
        let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(8.3));
        assert!(matches!(
            supernova(&before, at, mu, SystemVelocity::ZERO, density),
            Aftermath::Bound(_)
        ));
        // A kick against the orbital motion at the orbital speed, 15.7 km/s at 30 au of 8.3 M☉,
        // leaves the planet moving at twice it relative to the remnant, above escape.
        let (_, v) = before.relative_state_at(at);
        let speed = length(v.metres_per_second());
        assert!((speed / 15_700.0 - 1.0).abs() < 0.01, "{speed} m/s");
        let v = v.metres_per_second();
        let kick = SystemVelocity::new([-v[0], -v[1], -v[2]]);
        assert_eq!(
            supernova(&before, at, mu, kick, density),
            Aftermath::Unbound
        );
    }

    #[test]
    fn a_plunging_orbit_is_disrupted_at_its_next_pericentre() {
        let density = KilogramsPerCubicMetre::new(1_330.0);
        let before = orbit(0.5, 0.0, 1.0);
        let at = years(0.0);
        let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(1.0));
        // Nine tenths of the velocity taken away: e = 0.99, and a pericentre of 0.0025 au, inside
        // a Jupiter's Roche limit of 0.012 au about the Sun.
        let (r, v) = before.relative_state_at(at);
        let v = v.metres_per_second();
        let kick = SystemVelocity::new([v[0] * 0.9, v[1] * 0.9, v[2] * 0.9]);
        let Aftermath::Disrupted {
            orbit: Some(plunge),
            at: when,
        } = supernova(&before, at, mu, kick, density)
        else {
            panic!("a plunge inside the Roche limit is disrupted");
        };
        assert!(plunge.periapsis() < remnant_roche_limit(mu, density));
        // It falls from apocentre, half a period.
        let wait = when.checked_since(at).expect("later").as_seconds_f64();
        assert!(
            (wait / (0.5 * plunge.period().value()) - 1.0).abs() < 1e-6,
            "{wait}"
        );
        assert!(length(r.metres()) > remnant_roche_limit(mu, density).value());
        // Nearly all of it taken away, an orbit so eccentric that it is given as open, and all of
        // it, a radial fall: disrupted at once.
        let kick = SystemVelocity::new([v[0] * 0.999_9, v[1] * 0.999_9, v[2] * 0.999_9]);
        assert_eq!(
            supernova(&before, at, mu, kick, density),
            Aftermath::Disrupted { orbit: None, at }
        );
        let kick = SystemVelocity::new(v);
        assert_eq!(
            supernova(&before, at, mu, kick, density),
            Aftermath::Disrupted { orbit: None, at }
        );
    }

    #[test]
    fn a_remnants_roche_limit_follows_from_its_mass_and_the_bodys_density() {
        // The Sun and Jupiter: 2.456 R☉ (1,408 ÷ 1,326)^⅓ = 2.506 R☉.
        let limit = remnant_roche_limit(
            GravitationalParameter::new(GM_SUN),
            KilogramsPerCubicMetre::new(1_326.0),
        );
        let sun_radius: f64 = 6.957e8;
        let sun_volume = 4.0 * PI * sun_radius * sun_radius * sun_radius / 3.0;
        let sun_density = GM_SUN / GRAVITATIONAL_CONSTANT / sun_volume;
        let direct = 2.456 * sun_radius * math::cbrt(sun_density / 1_326.0);
        assert!((limit.value() / direct - 1.0).abs() < 1e-12);
    }
}
