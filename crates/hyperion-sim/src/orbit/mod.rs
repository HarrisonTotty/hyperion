//! Orbits on rails: Keplerian elements and the relative state they give at any time.
//!
//! The brainstorm's "Orbits and time": "Every orbit is a set of Keplerian elements, and a body's
//! position is a pure function of time. No N-body integration." A companion star about its
//! primary (plan 11) and a planet or moon about its host (plan 14) are propagated by the same code
//! here (plan 14, D2). Plan 09's `galaxy::motion::KeplerOrbit`, a state vector about the central
//! black hole in the galactic frame, is a different thing and shares only [`math`].
//!
//! # What is here
//!
//! - [`KeplerElements`]: a bound relative orbit, [`Eccentricity`] in `[0, 1)`, its
//!   [`Orientation`] in the system frame, and [`KeplerElements::relative_state_at`], which gives
//!   the position and velocity of the secondary relative to the primary at any
//!   [`UniverseTime`](crate::time::UniverseTime), before or after the epoch.
//! - [`solve_kepler`]: Kepler's equation, from a fixed starter by a fixed number of iterations.
//! - [`roche_lobe_radius`] (Eggleton 1983), [`peters_merger_time`] and [`peters_separation_for`]
//!   (Peters 1964), which plan 11's binary stage and plan 09's Type Ia entries read.
//! - [`elements_from_state`], the inverse of the propagation, whose result is an [`Orbit`]: bound,
//!   or an [`OpenOrbit`] for an unbound or near-parabolic state (plan 14, P14.T2.c and T2.b).
//!
//! # Frame and conventions
//!
//! Angles are in the system frame of [`coords`](crate::coords), whose axes are the galactic axes:
//! the inclination is measured from +z (galactic north), the ascending node from +x in the x–y
//! plane, counter-clockwise seen from the north, and the argument of periapsis from the ascending
//! node in the direction of motion. An orbit's angular momentum points along +z at inclination 0
//! and along −z at π. Planetary and binary planes are tilted at random to this frame (plan 14,
//! D21), so every orientation is equally valid.
//!
//! The elements hold at the epoch, [`UniverseTime::EPOCH`](crate::time::UniverseTime::EPOCH): the
//! mean anomaly at time t is the mean anomaly at the epoch plus 2π times the fraction of a period
//! elapsed since it.
//!
//! # Precision
//!
//! Plan 14 needs a one-day orbit to keep its phase a thousand years out (P14.T2.a), which an `f64`
//! of seconds times the mean motion cannot do: at 3 × 10¹⁰ s the product's last bit is worth
//! 5 × 10⁻¹⁰ rad. So the elapsed time is reduced modulo the period from the clock's integer
//! seconds, exactly, before anything is rounded: the whole seconds, exactly representable below
//! 2⁵³ s (285 million years; beyond, in two exact halves), are reduced by the exact truncated
//! remainder, [`math::fmod`], and the nanoseconds are added to the remainder. The fraction of a
//! period is centred on zero, so that it keeps its relative precision just before a whole period
//! as well as just after. The phase is then good to a few units in the last place whatever the
//! time, and a state at t and at t plus a whole number of periods agree to the resolution of an
//! `f64` (see [`KeplerElements::mean_anomaly_at`]). Near periapsis at high eccentricity, where
//! Kepler's equation is a small difference of large terms, it is evaluated in a form that keeps
//! its relative precision (see [`solve_kepler`]).
//!
//! # Determinism
//!
//! The module is pure: no streams, no tags, no caches. Every operation is an IEEE operation or a
//! [`math`] function, iterations run a fixed number of times and never stop on a
//! tolerance, and quadratures have fixed nodes and panels, so every platform gives the same bits.

mod kepler;
mod open;
mod orientation;
mod peters;
mod phase;
mod roche;
mod state;

use std::error::Error;
use std::fmt;

use crate::math;

pub use kepler::{Eccentricity, KEPLER_HALLEY_ITERATIONS, KeplerElements, solve_kepler};
pub use open::{NEAR_PARABOLIC_BAND, OpenOrbit, solve_barker, solve_kepler_hyperbolic};
pub use orientation::Orientation;
pub use peters::{peters_merger_time, peters_separation_for};
pub use roche::roche_lobe_radius;
pub use state::{InvertStateError, Orbit, elements_from_state};

/// An orbit could not be built from the values given.
///
/// Every value is checked once, when the orbit is built, so that propagation never meets one it
/// cannot use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildOrbitError {
    /// A bound orbit's eccentricity was not in `[0, 1)`.
    EccentricityOutOfRange {
        /// The offending value.
        value: f64,
    },
    /// An open orbit's eccentricity was below [`OpenOrbit::MIN_ECCENTRICITY`] or not finite.
    OpenEccentricityOutOfRange {
        /// The offending value.
        value: f64,
    },
    /// The inclination was not in `[0, π]`.
    InclinationOutOfRange {
        /// The offending value, rad.
        radians: f64,
    },
    /// The ascending node, the argument of periapsis or the mean anomaly at the epoch was not
    /// finite.
    AngleNotFinite {
        /// The offending value, rad.
        radians: f64,
    },
    /// The semi-major axis was not finite and positive.
    SemiMajorAxisNotPositive {
        /// The offending value, m.
        metres: f64,
    },
    /// The pericentre distance was not finite and positive.
    PericentreNotPositive {
        /// The offending value, m.
        metres: f64,
    },
    /// The period was not finite and positive, as given or as it follows from the other values.
    PeriodNotPositive {
        /// The offending value, s.
        seconds: f64,
    },
    /// The gravitational parameter was not finite and positive.
    GravitationalParameterNotPositive {
        /// The offending value, m³ s⁻².
        value: f64,
    },
    /// A scale factor was not finite and positive.
    ScaleFactorNotPositive {
        /// The offending value.
        factor: f64,
    },
    /// An open orbit's mean motion, which follows from its pericentre, eccentricity and
    /// gravitational parameter, overflows or underflows.
    MeanMotionNotPositive {
        /// The offending value, rad s⁻¹.
        radians_per_second: f64,
    },
}

impl fmt::Display for BuildOrbitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EccentricityOutOfRange { value } => {
                write!(f, "eccentricity {value} is outside [0, 1)")
            }
            Self::OpenEccentricityOutOfRange { value } => write!(
                f,
                "open-orbit eccentricity {value} is below {} or not finite",
                OpenOrbit::MIN_ECCENTRICITY
            ),
            Self::InclinationOutOfRange { radians } => {
                write!(f, "inclination {radians} rad is outside [0, π]")
            }
            Self::AngleNotFinite { radians } => write!(f, "angle {radians} rad is not finite"),
            Self::SemiMajorAxisNotPositive { metres } => {
                write!(f, "semi-major axis {metres} m is not finite and positive")
            }
            Self::PericentreNotPositive { metres } => {
                write!(f, "pericentre {metres} m is not finite and positive")
            }
            Self::PeriodNotPositive { seconds } => {
                write!(f, "period {seconds} s is not finite and positive")
            }
            Self::GravitationalParameterNotPositive { value } => write!(
                f,
                "gravitational parameter {value} m³ s⁻² is not finite and positive"
            ),
            Self::ScaleFactorNotPositive { factor } => {
                write!(f, "scale factor {factor} is not finite and positive")
            }
            Self::MeanMotionNotPositive { radians_per_second } => write!(
                f,
                "mean motion {radians_per_second} rad s⁻¹ is not finite and positive"
            ),
        }
    }
}

impl Error for BuildOrbitError {}

/// Whether `value` is finite and above zero.
fn is_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// `a × b`, component-wise sums in the fixed order x, y, z.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `a · b`, summed in the fixed order x, y, z.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `1 − cos x` without the cancellation of the direct form near `x = 0`, from `sin x` and
/// `cos x`: `sin² x ÷ (1 + cos x)` while `cos x > 0`, the direct form otherwise.
fn one_minus_cos(sin: f64, cos: f64) -> f64 {
    if cos > 0.0 {
        sin * sin / (1.0 + cos)
    } else {
        1.0 - cos
    }
}

/// Terms of the Stumpff series, used while |z| ≤ 1: the first term left out is below 1 ÷ 24! ≈
/// 1.6 × 10⁻²⁴ of the sum.
const STUMPFF_TERMS: u32 = 11;

/// The Stumpff functions (C(z), S(z)): C = (1 − cos √z) ÷ z and S = (√z − sin √z) ÷ z^(3/2),
/// continued through z < 0 by cosh and sinh (Battin 1999, §4.5).
///
/// While |z| ≤ 1, where the closed forms cancel, both come from their series
/// C = Σ (−z)ᵏ ÷ (2k + 2)! and S = Σ (−z)ᵏ ÷ (2k + 3)!, in nested form from the smallest term.
fn stumpff(z: f64) -> (f64, f64) {
    if z > 1.0 {
        let root = z.sqrt();
        let (sin, cos) = math::sin_cos(root);
        ((1.0 - cos) / z, (root - sin) / (z * root))
    } else if z < -1.0 {
        let root = (-z).sqrt();
        (
            (math::cosh(root) - 1.0) / -z,
            (math::sinh(root) - root) / (-z * root),
        )
    } else {
        (stumpff_c_series(z), stumpff_s_series(z))
    }
}

/// C(z) by its series, for |z| ≤ 1: ½ (1 − z ÷ (3·4) (1 − z ÷ (5·6) (1 − …))).
fn stumpff_c_series(z: f64) -> f64 {
    let mut c = 1.0;
    for k in (1..STUMPFF_TERMS).rev() {
        let k = f64::from(k);
        c = 1.0 - z * c / ((2.0 * k + 1.0) * (2.0 * k + 2.0));
    }
    0.5 * c
}

/// S(z) by its series, for |z| ≤ 1: ⅙ (1 − z ÷ (4·5) (1 − z ÷ (6·7) (1 − …))).
fn stumpff_s_series(z: f64) -> f64 {
    let mut s = 1.0;
    for k in (1..STUMPFF_TERMS).rev() {
        let k = f64::from(k);
        s = 1.0 - z * s / ((2.0 * k + 2.0) * (2.0 * k + 3.0));
    }
    s / 6.0
}

/// x − sin x, given sin x, without the cancellation of the direct form for small x: x³ S(x²)
/// by the series while x² ≤ 1.
fn sin_deficit(x: f64, sin: f64) -> f64 {
    let x2 = x * x;
    if x2 <= 1.0 {
        x2 * x * stumpff_s_series(x2)
    } else {
        x - sin
    }
}

/// sinh x − x, given sinh x, without the cancellation of the direct form for small x:
/// x³ S(−x²) by the series while x² ≤ 1.
fn sinh_excess(x: f64, sinh: f64) -> f64 {
    let x2 = x * x;
    if x2 <= 1.0 {
        x2 * x * stumpff_s_series(-x2)
    } else {
        sinh - x
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    #[test]
    fn stumpff_series_and_closed_forms_agree_where_they_meet() {
        for z in [1.0, -1.0] {
            let (c_series, s_series) = stumpff(z);
            let (c_closed, s_closed) = stumpff(z * (1.0 + 1e-15));
            assert!((c_series - c_closed).abs() < 1e-15, "C at {z}");
            assert!((s_series - s_closed).abs() < 1e-15, "S at {z}");
        }
        let (c, s) = stumpff(0.0);
        assert!((c - 0.5).abs() < 1e-17 && (s - 1.0 / 6.0).abs() < 1e-17);
        let (c, s) = stumpff(PI * PI);
        assert!((c - 2.0 / (PI * PI)).abs() < 1e-15 && (s - 1.0 / (PI * PI)).abs() < 1e-15);
        // 1 − cos 1 and 1 − sin 1, from the series at z = 1.
        let (c, s) = stumpff(1.0);
        assert!((c - (1.0 - crate::math::cos(1.0))).abs() < 1e-16);
        assert!((s - (1.0 - crate::math::sin(1.0))).abs() < 1e-16);
    }

    #[test]
    fn the_deficits_keep_their_precision_for_small_arguments() {
        // x − sin x ≈ x³/6 − x⁵/120 and sinh x − x ≈ x³/6 + x⁵/120.
        for x in [1e-8, 1e-4, 0.01, 0.3] {
            let x3 = x * x * x;
            let x5 = x3 * x * x;
            let deficit = sin_deficit(x, crate::math::sin(x));
            let excess = sinh_excess(x, crate::math::sinh(x));
            let tolerance = 1e-15 * x3 + x5 * x * x / 5000.0;
            assert!(
                (deficit - (x3 / 6.0 - x5 / 120.0)).abs() <= tolerance,
                "at {x}"
            );
            assert!(
                (excess - (x3 / 6.0 + x5 / 120.0)).abs() <= tolerance,
                "at {x}"
            );
            assert!((sin_deficit(-x, crate::math::sin(-x)) + deficit).abs() <= 1e-30);
        }
        for x in [1.5, -2.0, 7.0] {
            let (sin, sinh) = (crate::math::sin(x), crate::math::sinh(x));
            assert!((sin_deficit(x, sin) - (x - sin)).abs() < 1e-300);
            assert!((sinh_excess(x, sinh) - (sinh - x)).abs() < 1e-300);
        }
    }

    #[test]
    fn errors_name_the_offending_value() {
        assert_eq!(
            BuildOrbitError::EccentricityOutOfRange { value: 1.0 }.to_string(),
            "eccentricity 1 is outside [0, 1)"
        );
        assert_eq!(
            BuildOrbitError::OpenEccentricityOutOfRange { value: 0.5 }.to_string(),
            "open-orbit eccentricity 0.5 is below 0.9999 or not finite"
        );
        assert_eq!(
            BuildOrbitError::GravitationalParameterNotPositive { value: -1.0 }.to_string(),
            "gravitational parameter -1 m³ s⁻² is not finite and positive"
        );
    }

    #[test]
    fn one_minus_cos_matches_the_direct_form_away_from_zero() {
        for x in [0.3_f64, 1.0, 1.5, 2.0, 3.0, -2.5] {
            let (s, c) = crate::math::sin_cos(x);
            assert!((one_minus_cos(s, c) - (1.0 - c)).abs() < 1e-15, "at {x}");
        }
        let (s, c) = crate::math::sin_cos(1e-9);
        assert!((one_minus_cos(s, c) - 5e-19).abs() < 1e-33);
    }
}
