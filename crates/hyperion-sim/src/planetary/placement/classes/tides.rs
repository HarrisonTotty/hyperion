//! Tidal circularisation of a placed planet's orbit (plan 14, P14.T8.e; design note 9).
//!
//! Tides raised on a planet by its host damp its eccentricity at constant orbital angular
//! momentum. In the standard constant-Q form the damping time is
//!
//! τ = (4 ÷ 63) Q′ (m ÷ M★) (a ÷ R)⁵ ÷ n,
//!
//! n being the mean motion, R the planet's radius and Q′ = 3Q ÷ (2k₂) its modified tidal quality
//! factor (Goldreich and Soter 1966, Icarus 5, 375; in this form Rasio et al. 1996, ApJ 470,
//! 1187, eq. 9, and Jackson, Greenberg and Barnes 2008, ApJ 678, 1396, eq. 1, the planet's term).
//! Evaluated at the primordial orbit and held there, it gives the closed form
//!
//! e(t) = e₀ exp(−t ÷ τ), a(t) = a₀ (1 − e₀²) ÷ (1 − e(t)²),
//!
//! with t the time since the planet formed, the host's age plus the clock time. The damping
//! stops at the forced eccentricity its neighbours hold (ruling 133.1, amending 112.8;
//! [`secular`](super::secular)): e(t) = `e_f`(t) + (e₀ − `e_f`(0)) exp(−t ÷ τ), `e_f` the larger
//! of the surviving secular mode's, which decays with the mode, and a near-resonant pair's, which
//! does not ([`circularise_to`]). TRAPPIST-1's planets keep e cos ω and e sin ω of 0.002–0.008
//! (Agol et al. 2021, Table 2), where the pure exponential gave eccentricities of 10⁻²²⁶. The orbit's
//! semi-latus rectum a (1 − e²) is kept, so a(t) falls from a₀ towards a₀ (1 − e₀²), which lies
//! between the primordial periapsis and semi-major axis: the pericentre rises and the apocentre
//! falls, and a planet never leaves the interval its spacing was checked on (design note 9). The
//! fate transform (P14.T28) evaluates it; the planet's radius is the derivation's (P14.T11).
//!
//! # Q′, re-checked
//!
//! [`GIANT_TIDAL_Q_PRIME`] is 10⁶: Ogilvie (2014, ARA&A 52, 171, §5.4) recomputes Jupiter's
//! Q′ from Io's orbit as 1.2 × 10⁶ (down to 2.7 × 10⁵ if the Laplace resonance formed early),
//! and Jackson, Greenberg and Barnes (2008, abstract) fit the eccentricities of hot Jupiters best
//! with Q′ = 10^6.5. [`ROCKY_TIDAL_Q_PRIME`] is plan 14's 10², the order of the terrestrial
//! planets' Q′ (Earth's Q near 12 with k₂ ≈ 0.3, Q′ ≈ 60). Goldreich and Soter's own paper was
//! not reachable, so its figures were checked through these.

use crate::math;
use crate::orbit::{BuildOrbitError, Eccentricity, KeplerElements};
use crate::units::consts::{EARTH_MASS_KG, SECONDS_PER_JULIAN_YEAR, SOLAR_MASS_KG};
use crate::units::{EarthMasses, Metres, Seconds, SolarMasses, Years};

/// A giant planet's modified tidal quality factor Q′: 10⁶ (plan 14, P14.T8.e).
pub const GIANT_TIDAL_Q_PRIME: f64 = 1e6;

/// A rocky planet's modified tidal quality factor Q′: 10² (plan 14, P14.T8.e).
pub const ROCKY_TIDAL_Q_PRIME: f64 = 1e2;

/// The eccentricity damping time τ of a planet of mass `planet` and radius `radius` on an orbit
/// of semi-major axis `a` and mean motion from `orbit_period`, about a host of mass `host`, with
/// modified tidal quality `q_prime` (P14.T8.e): (4 ÷ 63) Q′ (m ÷ M★) (a ÷ R)⁵ ÷ n.
///
/// # Panics
///
/// In debug builds, unless every argument is positive and finite.
///
/// # Examples
///
/// A Jupiter at five days from a Sun circularises in a few thousand million years, and one at
/// three days in a few hundred million:
///
/// ```
/// use hyperion_sim::planetary::placement::classes::tides::{
///     GIANT_TIDAL_Q_PRIME, circularisation_time,
/// };
/// use hyperion_sim::units::{EarthMasses, JupiterRadii, Metres, Seconds, SolarMasses};
/// use hyperion_sim::units::consts::{GM_SUN, SECONDS_PER_DAY};
///
/// let jupiter = (EarthMasses::new(317.8), Metres::from(JupiterRadii::new(1.0)));
/// let at = |days: f64| {
///     let period = Seconds::new(days * SECONDS_PER_DAY);
///     let n = core::f64::consts::TAU / period.value();
///     let a = Metres::new(hyperion_sim::math::cbrt(GM_SUN / (n * n)));
///     circularisation_time(jupiter.0, jupiter.1, SolarMasses::new(1.0), a, period, GIANT_TIDAL_Q_PRIME)
/// };
/// assert!((2.0e9..5.0e9).contains(&at(5.0).value()));
/// assert!((1.0e8..9.0e8).contains(&at(3.0).value()));
/// ```
#[must_use]
pub fn circularisation_time(
    planet: EarthMasses,
    radius: Metres,
    host: SolarMasses,
    a: Metres,
    orbit_period: Seconds,
    q_prime: f64,
) -> Years {
    let positive = |x: f64| x.is_finite() && x > 0.0;
    debug_assert!(
        positive(planet.value())
            && positive(radius.value())
            && positive(host.value())
            && positive(a.value())
            && positive(orbit_period.value())
            && positive(q_prime),
        "circularisation needs positive values"
    );
    let mass_ratio = planet.value() * EARTH_MASS_KG / (host.value() * SOLAR_MASS_KG);
    let size_ratio = a.value() / radius.value();
    let mean_motion = core::f64::consts::TAU / orbit_period.value();
    let seconds = 4.0 / 63.0 * q_prime * mass_ratio * math::powi(size_ratio, 5) / mean_motion;
    Years::new(seconds / SECONDS_PER_JULIAN_YEAR)
}

/// The semi-major axis and eccentricity of a primordial orbit of `a0` and `e0` after `elapsed`
/// of damping with time `tau` (P14.T8.e): e₀ exp(−t ÷ τ) and a₀ (1 − e₀²) ÷ (1 − e²). A time
/// before formation, or an infinite τ, leaves the orbit as it was. It is [`circularise_to`] with
/// no floor and no drain.
///
/// # Panics
///
/// In debug builds, unless `a0` is positive and `e0` lies in `[0, 1)`.
#[must_use]
pub fn circularise(a0: Metres, e0: f64, tau: Years, elapsed: Years) -> (Metres, f64) {
    debug_assert!(a0.value() > 0.0 && (0.0..1.0).contains(&e0));
    if elapsed.value() <= 0.0 || tau.value().is_nan() || tau.value() <= 0.0 {
        return (a0, e0);
    }
    let e = e0 * math::exp(-elapsed.value() / tau.value());
    let a = a0 * ((1.0 - e0 * e0) / (1.0 - e * e));
    (a, e)
}

/// [`circularise`] of a planet whose eccentricity is held at a floor (ruling 133.1): e(t) =
/// `e_f`(t) + (e₀ − `e_f`(0)) exp(−(1/τ + D) t), with `e_f`(t) = min(`floor(t)`, e₀), and the axis
/// from the tides' part alone, a = a₀ (1 − e₀²) ÷ (1 − `e_tide`²), `e_tide`(t) = `e_f`(t) +
/// (e₀ − `e_f`(0)) exp(−t ÷ τ). `floor` gives the forced eccentricity at a time since formation,
/// non-increasing; `drain` D, per year, is the rate at which the planets it anchors draw its
/// eccentricity out through their own tides, which moves angular momentum and not energy, so it
/// keeps the axis. An orbit at or below its floor keeps its eccentricity until the floor falls
/// below it. A time before formation, or a τ that is not positive, leaves the orbit as it was.
///
/// # Panics
///
/// In debug builds, unless `a0` is positive and `e0` lies in `[0, 1)`.
///
/// # Examples
///
/// A hot planet damped for a hundred e-folding times keeps its forced eccentricity:
///
/// ```
/// use hyperion_sim::planetary::placement::classes::tides::circularise_to;
/// use hyperion_sim::units::{Metres, Years};
///
/// let floor = |_: f64| 0.004;
/// let (_, e) = circularise_to(Metres::new(3e9), 0.1, floor, 0.0, Years::new(1e7), Years::new(1e9));
/// assert!((e - 0.004).abs() < 1e-15);
/// ```
#[must_use]
pub fn circularise_to(
    a0: Metres,
    e0: f64,
    floor: impl Fn(f64) -> f64,
    drain: f64,
    tau: Years,
    elapsed: Years,
) -> (Metres, f64) {
    debug_assert!(a0.value() > 0.0 && (0.0..1.0).contains(&e0));
    let t = elapsed.value();
    if t <= 0.0 || tau.value().is_nan() || tau.value() <= 0.0 {
        return (a0, e0);
    }
    let held = |time: f64| floor(time).clamp(0.0, e0);
    let (start, now) = (held(0.0), held(t));
    let free = e0 - start;
    let tides = math::exp(-t / tau.value());
    let e = now + free * math::exp(-t / tau.value() - drain.max(0.0) * t);
    let e_tide = now + free * tides;
    let a = a0 * ((1.0 - e0 * e0) / (1.0 - e_tide * e_tide));
    (a, e)
}

/// `orbit` circularised for `elapsed` with damping time `tau`: [`circularise`] of its semi-major
/// axis and eccentricity, its orientation and mean anomaly at the epoch kept and its period from
/// Kepler's third law about the same gravitational parameter.
///
/// # Errors
///
/// As [`KeplerElements::from_semi_major_axis`], which cannot fail for an orbit that was valid.
pub fn circularised_orbit(
    orbit: &KeplerElements,
    tau: Years,
    elapsed: Years,
) -> Result<KeplerElements, BuildOrbitError> {
    let (a, e) = circularise(
        orbit.semi_major_axis(),
        orbit.eccentricity().value(),
        tau,
        elapsed,
    );
    KeplerElements::from_semi_major_axis(
        a,
        orbit.gravitational_parameter(),
        Eccentricity::new(e)?,
        *orbit.orientation(),
        orbit.mean_anomaly_at_epoch(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::consts::{GM_SUN, METRES_PER_AU};

    #[test]
    fn circularisation_keeps_the_semi_latus_rectum_and_stays_inside_the_old_orbit() {
        let a0 = Metres::new(0.05 * METRES_PER_AU);
        for e0 in [0.0, 0.01, 0.2, 0.6, 0.95] {
            let mut previous = (a0, e0);
            for t in [0.0, 1e6, 1e8, 1e9, 5e9, 1e11] {
                let (a, e) = circularise(a0, e0, Years::new(1e9), Years::new(t));
                let p0 = a0.value() * (1.0 - e0 * e0);
                assert!((a.value() * (1.0 - e * e) - p0).abs() < 1e-6 * p0);
                assert!(a <= a0 && a.value() >= a0.value() * (1.0 - e0) * (1.0 - 1e-15));
                assert!(e <= previous.1 && a <= previous.0, "monotone");
                previous = (a, e);
            }
        }
        // Before formation, nothing has happened.
        let (a, e) = circularise(a0, 0.3, Years::new(1e9), Years::new(-5.0));
        assert_eq!((a, e), (a0, 0.3));
    }

    /// Ruling 133.1: damping stops at the floor, follows a floor that decays, keeps the axis the
    /// tides alone give when a drain is added, and never raises an eccentricity below the floor.
    #[test]
    fn damping_stops_at_the_floor() {
        let a0 = Metres::new(0.02 * METRES_PER_AU);
        let tau = Years::new(1e6);
        let fixed = |_: f64| 0.004;
        let mut previous = 0.2;
        for t in [0.0, 1e5, 1e6, 1e7, 1e9, 1e12] {
            let (a, e) = circularise_to(a0, 0.2, fixed, 0.0, tau, Years::new(t));
            assert!(e >= 0.004 && e <= previous, "{t}: {e}");
            let p0 = a0.value() * (1.0 - 0.04);
            assert!((a.value() * (1.0 - e * e) - p0).abs() < 1e-9 * p0);
            previous = e;
        }
        let (_, e) = circularise_to(a0, 0.2, fixed, 0.0, tau, Years::new(1e12));
        assert!((e - 0.004).abs() < 1e-15, "{e}");
        assert_eq!(
            circularise_to(a0, 0.001, fixed, 0.0, tau, Years::new(1e9)),
            (a0, 0.001)
        );
        // A floor that decays takes the eccentricity with it.
        let decaying = |t: f64| 0.004 * math::exp(-t / 1e8);
        let (_, late) = circularise_to(a0, 0.2, decaying, 0.0, tau, Years::new(1e9));
        assert!(late < 1e-6, "{late}");
        // A drain lowers e but keeps the axis the tides give.
        let (a_plain, e_plain) = circularise_to(a0, 0.2, |_| 0.0, 0.0, tau, Years::new(5e5));
        let (a_drained, e_drained) = circularise_to(a0, 0.2, |_| 0.0, 1e-6, tau, Years::new(5e5));
        assert_eq!(a_plain, a_drained);
        assert!(e_drained < e_plain);
        // With no floor and no drain the damping is P14.T8.e's.
        for t in [1e5, 1e8, 5e9] {
            let (a, e) = circularise_to(a0, 0.3, |_| 0.0, 0.0, tau, Years::new(t));
            let (a_old, e_old) = circularise(a0, 0.3, tau, Years::new(t));
            assert!(
                (e - e_old).abs() <= 1e-15 * e_old.max(1e-300)
                    && (a - a_old).abs() < Metres::new(1e-3)
            );
        }
    }

    #[test]
    fn the_damping_time_follows_the_constant_q_form() {
        // (4/63) Q′ (m/M) (a/R)⁵ / n by hand, for Earth at 1 au with Q′ = 100.
        let (m, r) = (EarthMasses::new(1.0), Metres::new(6.371e6));
        let a = Metres::new(METRES_PER_AU);
        let n = (GM_SUN / (a.value() * a.value() * a.value())).sqrt();
        let period = Seconds::new(core::f64::consts::TAU / n);
        let tau = circularisation_time(m, r, SolarMasses::new(1.0), a, period, ROCKY_TIDAL_Q_PRIME);
        let ratio = EARTH_MASS_KG / SOLAR_MASS_KG;
        let expected = 4.0 / 63.0 * 100.0 * ratio * math::powi(a.value() / r.value(), 5) / n;
        assert!((tau.value() * SECONDS_PER_JULIAN_YEAR / expected - 1.0).abs() < 1e-12);
        // τ scales as a^6.5 at fixed host: twice as far is 2^6.5 times longer.
        let a2 = a * 2.0;
        let period2 = period * math::powf(2.0, 1.5);
        let tau2 = circularisation_time(
            m,
            r,
            SolarMasses::new(1.0),
            a2,
            period2,
            ROCKY_TIDAL_Q_PRIME,
        );
        assert!((tau2 / tau - math::powf(2.0, 6.5)).abs() < 1e-9);
    }
}
