//! Motion about the central black hole: the Kepler regime of the brainstorm's "Orbits and time"
//! (plan 09, P09.T28).
//!
//! "Within the central black hole's sphere of influence, about 10 ly, where it outweighs the stars
//! inside, a system follows the Kepler orbit about the black hole and the cluster mass inside its
//! own radius." [`KeplerOrbit`] is that orbit: a state vector relative to the black hole in the
//! galactic frame, and a gravitational parameter μ = G × (`M_bh` + M★(< r₀)), which the caller forms
//! from the centre's mass model at the epoch radius r₀ (P09.T28.b). It is propagated to any time by
//! universal variables, so bound, parabolic and unbound orbits share one code path (Design note 12).
//!
//! This is not [`orbit::KeplerElements`](crate::orbit::KeplerElements), which is a bound orbit of a
//! companion or a planet held as elements and phased from integer seconds. An orbit here starts from
//! a drawn position and velocity and may be unbound, so it is held as that state.
//!
//! # Method
//!
//! With `r₀`, `v₀` the state at the epoch, `σ₀ = r₀ · v₀ ÷ √μ` and `α = 2 ÷ r₀ − v₀² ÷ μ` (the
//! reciprocal semi-major axis: positive bound, zero parabolic, negative unbound), the universal
//! anomaly χ after a time τ solves Kepler's equation in universal form,
//!
//! `√μ τ = σ₀ χ² C(z) + (1 − α r₀) χ³ S(z) + r₀ χ`, with `z = α χ²`,
//!
//! where C and S are Stumpff's functions, and the state follows from the Lagrange coefficients f,
//! g, ḟ and ġ (Battin 1999, *An Introduction to the Mathematics and Methods of Astrodynamics*, rev.
//! ed., §4.5; Vallado 2013, *Fundamentals of Astrodynamics and Applications*, 4th ed., §2.3,
//! algorithm 8). The equation is solved by the Laguerre–Conway iteration (Conway 1986, *Celestial
//! Mechanics* 39, 199), which converges from a poor starting value for every conic, run a fixed
//! [`UNIVERSAL_ITERATIONS`] times and never stopped on a tolerance. A bound orbit's time is first
//! reduced to within half a period of the epoch, so the anomaly never winds up over the thousand
//! orbits an inner star completes in the clock window.
//!
//! # Units and precision
//!
//! Positions are [`PointLy`]s relative to the black hole, velocities [`GalacticVelocity`]s, the
//! gravitational parameter a [`GravitationalParameter`] and times [`Seconds`]; the arithmetic is in
//! SI. A position is converted to metres once, on construction, and back on reading: an `f64`
//! light-year near the black hole resolves under a millimetre, far below anything a propagation
//! keeps. A propagation keeps the energy to about 10⁻¹⁴ of its larger term and the angular
//! momentum to about 10⁻¹³ of |r| |v|. A propagation by `dt` and then by `−dt` returns an S2-like
//! orbit to 10⁻¹² relative over the whole clock window; the error grows with the orbits
//! completed and with 1 ÷ (1 − e), and on an unbound orbit with the square of how far out it has
//! gone against its pericentre, which is the conditioning of the problem and not the method's.
//!
//! # Determinism
//!
//! The module is pure: no streams, no tags, no caches. Every operation is an IEEE operation or a
//! [`math`] function, the iteration count is fixed, and the arithmetic's form is fixed, so every
//! platform gives the same bits. [`UNIVERSAL_ITERATIONS`], the Stumpff series' length and the form
//! of every expression here belong to the generator version once P09.T28.b wires the propagator
//! into the drift hook; until then nothing generated reads it.

use std::error::Error;
use std::fmt;

use crate::coords::GalacticVelocity;
use crate::galaxy::PointLy;
use crate::math;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{GravitationalParameter, LightYears, Metres, Seconds};

/// How many Laguerre–Conway iterations solve Kepler's equation in universal form: a fixed count,
/// never a tolerance, so that every platform takes the same steps.
///
/// From the starting values below the iteration reaches the root to 10⁻¹³ relative in at most
/// seven steps over the stress set of this module's tests (bound orbits to e = 0.9999, parabolic,
/// hyperbolic to e = 10, over the whole clock window), and in two to four for most; ten leaves a
/// margin of three. A step at
/// the root changes χ by a rounding error at most, so the extra steps cost only time.
pub const UNIVERSAL_ITERATIONS: u32 = 10;

/// The order of the Laguerre–Conway iteration; Conway (1986) found 5 best, and any n from 4 to
/// about 10 converges alike.
const LAGUERRE_ORDER: f64 = 5.0;

/// Above this eccentricity an unbound orbit's starting value comes from its hyperbolic anomaly,
/// which is ill-conditioned as e approaches 1, where the parabolic cubic serves instead.
const HYPERBOLIC_STARTER_ECCENTRICITY: f64 = 1.01;

/// Below this |z| the Stumpff functions come from their series, where the closed forms lose
/// digits to cancellation.
const STUMPFF_SERIES_LIMIT: f64 = 1.0;

/// Terms of the Stumpff series: the tenth term of C is z⁹ ÷ 20!, under 5 × 10⁻¹⁹ at |z| = 1, so
/// the series is exact to rounding below [`STUMPFF_SERIES_LIMIT`].
const STUMPFF_SERIES_TERMS: u32 = 10;

/// A state could not be made into a Kepler orbit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildKeplerOrbitError {
    /// A component of the position was not finite.
    PositionNotFinite {
        /// The offending position, ly.
        position: PointLy,
    },
    /// The position was the black hole's own: no orbit starts there.
    AtCentre,
    /// A component of the velocity was not finite.
    VelocityNotFinite {
        /// The offending velocity.
        velocity: GalacticVelocity,
    },
    /// The gravitational parameter was not finite and positive.
    GravitationalParameterNotPositive {
        /// The offending value.
        value: GravitationalParameter,
    },
    /// The velocity was parallel to the position, so the orbit is radial and falls into the black
    /// hole: it has no pericentre to pass and no propagation through the collision.
    Radial,
}

impl fmt::Display for BuildKeplerOrbitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PositionNotFinite { position } => write!(
                f,
                "position ({}, {}, {}) ly is not finite",
                position.x, position.y, position.z
            ),
            Self::AtCentre => write!(f, "position is at the black hole"),
            Self::VelocityNotFinite { velocity } => {
                let [x, y, z] = velocity.metres_per_second();
                write!(f, "velocity ({x}, {y}, {z}) m/s is not finite")
            }
            Self::GravitationalParameterNotPositive { value } => write!(
                f,
                "gravitational parameter {} m³ s⁻² is not finite and positive",
                value.value()
            ),
            Self::Radial => write!(f, "orbit is radial and has no angular momentum"),
        }
    }
}

impl Error for BuildKeplerOrbitError {}

/// A Kepler orbit about the central black hole, held as its state at one time.
///
/// Built by [`KeplerOrbit::from_state`] from a position relative to the black hole, a velocity and
/// the gravitational parameter of the mass it orbits; [`KeplerOrbit::propagate`] gives the orbit's
/// state a time `dt` later, as another `KeplerOrbit`. The state is validated once, on
/// construction: it is finite, off the black hole and not radial, and μ is positive, so a
/// propagation never meets a value it cannot use.
///
/// # Examples
///
/// A star on S2's orbit, started at apocentre, reaches pericentre half a period later:
///
/// ```
/// use hyperion_sim::coords::GalacticVelocity;
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::motion::KeplerOrbit;
/// use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
/// use hyperion_sim::units::{AstronomicalUnits, GravitationalParameter, Seconds, SolarMasses};
///
/// let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(4.297e6));
/// // S2's semi-major axis in light-years and eccentricity (GRAVITY Collaboration 2020).
/// let (a, e) = (0.0163, 0.8846);
/// let apocentre = a * (1.0 + e);
/// let speed = (mu.value() * (1.0 - e) / (apocentre * METRES_PER_LIGHT_YEAR)).sqrt();
/// let orbit = KeplerOrbit::from_state(
///     PointLy::new(apocentre, 0.0, 0.0),
///     GalacticVelocity::new([0.0, speed, 0.0]),
///     mu,
/// )?;
/// let period = orbit.period().expect("the orbit is bound");
/// let pericentre = orbit.propagate(Seconds::new(0.5 * period.value()));
/// let distance = AstronomicalUnits::from(pericentre.distance()).value();
/// assert!((distance - 119.0).abs() < 1.0, "{distance} au");
/// # Ok::<(), hyperion_sim::galaxy::motion::BuildKeplerOrbitError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeplerOrbit {
    /// The position relative to the black hole, m.
    position: [f64; 3],
    /// The velocity, m/s.
    velocity: [f64; 3],
    /// μ, m³ s⁻².
    mu: f64,
    /// |position|, m.
    radius: f64,
    /// α = 2 ÷ r − v² ÷ μ, m⁻¹.
    alpha: f64,
}

impl KeplerOrbit {
    /// The orbit through `position` (relative to the black hole, galactic axes, ly) at `velocity`
    /// about a mass of gravitational parameter `mu`.
    ///
    /// For a system in the Kepler regime μ is G × (`M_bh` + M★(< r₀)), the black hole and the
    /// stars inside the epoch radius r₀, held fixed along the orbit (brainstorm, "Orbits and
    /// time").
    ///
    /// # Errors
    ///
    /// [`BuildKeplerOrbitError::PositionNotFinite`] or
    /// [`VelocityNotFinite`](BuildKeplerOrbitError::VelocityNotFinite) if a component is not
    /// finite; [`AtCentre`](BuildKeplerOrbitError::AtCentre) if the position is the origin;
    /// [`GravitationalParameterNotPositive`](BuildKeplerOrbitError::GravitationalParameterNotPositive)
    /// if μ is not finite and positive; [`Radial`](BuildKeplerOrbitError::Radial) if the angular
    /// momentum is exactly zero, which includes a system at rest.
    pub fn from_state(
        position: PointLy,
        velocity: GalacticVelocity,
        mu: GravitationalParameter,
    ) -> Result<Self, BuildKeplerOrbitError> {
        let ly = [position.x, position.y, position.z];
        if !ly.iter().all(|c| c.is_finite()) {
            return Err(BuildKeplerOrbitError::PositionNotFinite { position });
        }
        let v = velocity.metres_per_second();
        if !v.iter().all(|c| c.is_finite()) {
            return Err(BuildKeplerOrbitError::VelocityNotFinite { velocity });
        }
        if !(mu.value().is_finite() && mu.value() > 0.0) {
            return Err(BuildKeplerOrbitError::GravitationalParameterNotPositive { value: mu });
        }
        let r = ly.map(|c| c * METRES_PER_LIGHT_YEAR);
        // Exactly zero is meant in both checks: any other state, however close to radial, has a
        // pericentre and propagates, if at great cost in precision through the passage.
        if length(r) <= 0.0 {
            return Err(BuildKeplerOrbitError::AtCentre);
        }
        if length(cross(r, v)) <= 0.0 {
            return Err(BuildKeplerOrbitError::Radial);
        }
        Ok(Self::from_si(r, v, mu.value()))
    }

    /// The orbit from a checked state in SI units.
    fn from_si(position: [f64; 3], velocity: [f64; 3], mu: f64) -> Self {
        let radius = length(position);
        let alpha = 2.0 / radius - dot(velocity, velocity) / mu;
        Self {
            position,
            velocity,
            mu,
            radius,
            alpha,
        }
    }

    /// The position relative to the black hole, galactic axes, ly.
    #[must_use]
    pub fn position(&self) -> PointLy {
        let [x, y, z] = self.position.map(|c| c / METRES_PER_LIGHT_YEAR);
        PointLy::new(x, y, z)
    }

    /// The velocity, galactic axes.
    #[must_use]
    pub fn velocity(&self) -> GalacticVelocity {
        GalacticVelocity::new(self.velocity)
    }

    /// The distance from the black hole.
    #[must_use]
    pub fn distance(&self) -> Metres {
        Metres::new(self.radius)
    }

    /// The gravitational parameter of the mass orbited.
    #[must_use]
    pub fn gravitational_parameter(&self) -> GravitationalParameter {
        GravitationalParameter::new(self.mu)
    }

    /// The specific orbital energy v² ÷ 2 − μ ÷ r, J kg⁻¹ (m² s⁻²): negative bound, positive
    /// unbound.
    #[must_use]
    pub fn specific_energy_j_per_kg(&self) -> f64 {
        0.5 * dot(self.velocity, self.velocity) - self.mu / self.radius
    }

    /// The specific angular momentum r × v, galactic axes, m² s⁻¹.
    #[must_use]
    pub fn angular_momentum_m2_per_s(&self) -> [f64; 3] {
        cross(self.position, self.velocity)
    }

    /// The eccentricity, `[0, ∞)`: below 1 bound, 1 parabolic, above 1 unbound.
    ///
    /// The length of the eccentricity vector ((v² − μ ÷ r) r − (r · v) v) ÷ μ, which keeps its
    /// precision for nearly circular orbits, where √(1 + 2 E h² ÷ μ²) does not.
    #[must_use]
    pub fn eccentricity(&self) -> f64 {
        length(self.eccentricity_vector())
    }

    /// The eccentricity vector, pointing at pericentre, dimensionless.
    fn eccentricity_vector(&self) -> [f64; 3] {
        let radial = dot(self.velocity, self.velocity) - self.mu / self.radius;
        let rv = dot(self.position, self.velocity);
        [0, 1, 2].map(|i| (radial * self.position[i] - rv * self.velocity[i]) / self.mu)
    }

    /// The pericentre distance h² ÷ (μ (1 + e)), for every conic.
    #[must_use]
    pub fn pericentre(&self) -> LightYears {
        LightYears::from(Metres::new(self.pericentre_m()))
    }

    /// The pericentre distance, m.
    fn pericentre_m(&self) -> f64 {
        let h = self.angular_momentum_m2_per_s();
        dot(h, h) / (self.mu * (1.0 + self.eccentricity()))
    }

    /// The semi-major axis 1 ÷ α of a bound orbit, or `None` for a parabolic or unbound one.
    #[must_use]
    pub fn semi_major_axis(&self) -> Option<LightYears> {
        (self.alpha > 0.0).then(|| LightYears::from(Metres::new(1.0 / self.alpha)))
    }

    /// The period 2π √(a³ ÷ μ) of a bound orbit, or `None` for a parabolic or unbound one.
    #[must_use]
    pub fn period(&self) -> Option<Seconds> {
        (self.alpha > 0.0).then(|| Seconds::new(self.period_si()))
    }

    /// The period in seconds; meaningful only when α > 0.
    fn period_si(&self) -> f64 {
        let a = 1.0 / self.alpha;
        std::f64::consts::TAU * a * (a / self.mu).sqrt()
    }

    /// The orbit's state `dt` later (earlier for a negative `dt`), on the same conic about the
    /// same μ.
    ///
    /// A bound orbit's `dt` is first reduced by whole periods to within half a period, so the
    /// result is as precise a thousand orbits out as one. The result is an orbit in its own right,
    /// and propagating it by `−dt` returns this one to rounding.
    ///
    /// # Panics
    ///
    /// Never for a `dt` within the clock window. An unbound orbit's state overflows only when its
    /// hyperbolic anomaly passes about 700, which needs a flyby at under an astronomical unit to
    /// run for some 10²⁸⁷ periods of its pericentre passage; the result would then not be finite,
    /// and a debug assertion says so.
    #[must_use]
    pub fn propagate(&self, dt: Seconds) -> Self {
        self.propagate_with(dt, UNIVERSAL_ITERATIONS)
    }

    /// [`KeplerOrbit::propagate`] with the iteration count given, for the convergence tests.
    #[expect(
        clippy::many_single_char_names,
        reason = "the names are the literature's (Battin 1999, §4.5): z, C, S, f, g, b, n"
    )]
    fn propagate_with(&self, dt: Seconds, iterations: u32) -> Self {
        let sqrt_mu = self.mu.sqrt();
        let tau = self.reduced_time(dt.value());
        let sigma = dot(self.position, self.velocity) / sqrt_mu;
        let chi = self.solve(tau * sqrt_mu, sigma, iterations);
        let (r0, alpha) = (self.radius, self.alpha);
        let z = alpha * chi * chi;
        let (c, s) = stumpff(z);
        let chi2c = chi * chi * c;
        let one_minus_zs = 1.0 - z * s;
        let f = 1.0 - chi2c / r0;
        // g = τ − χ³ S ÷ √μ, rewritten through Kepler's equation so that it is the solved χ's own
        // and not the difference of two large numbers.
        let g = (sigma * chi2c + r0 * chi * one_minus_zs) / sqrt_mu;
        let position = [0, 1, 2].map(|i| f * self.position[i] + g * self.velocity[i]);
        let r = length(position);
        let f_dot = -sqrt_mu * chi * one_minus_zs / (r * r0);
        let g_dot = 1.0 - chi2c / r;
        let velocity = [0, 1, 2].map(|i| f_dot * self.position[i] + g_dot * self.velocity[i]);
        debug_assert!(
            position.iter().chain(&velocity).all(|c| c.is_finite()),
            "a propagation by {} s left the finite range",
            dt.value()
        );
        Self::from_si(position, velocity, self.mu)
    }

    /// `dt` in seconds, reduced by whole periods to within half a period of zero for a bound
    /// orbit, unchanged otherwise.
    fn reduced_time(&self, dt: f64) -> f64 {
        if self.alpha > 0.0 {
            let period = self.period_si();
            dt - (dt / period).round() * period
        } else {
            dt
        }
    }

    /// The universal anomaly χ, √m, at which Kepler's universal equation gives `sqrt_mu_tau`,
    /// from the starting value of [`KeplerOrbit::starter`] by `iterations` Laguerre–Conway steps.
    #[expect(
        clippy::many_single_char_names,
        reason = "the names are the literature's (Battin 1999, §4.5): z, C, S, f, g, b, n"
    )]
    fn solve(&self, sqrt_mu_tau: f64, sigma: f64, iterations: u32) -> f64 {
        let (r0, alpha) = (self.radius, self.alpha);
        let b = 1.0 - alpha * r0;
        let n = LAGUERRE_ORDER;
        let mut chi = self.starter(sqrt_mu_tau, sigma);
        for _ in 0..iterations {
            let z = alpha * chi * chi;
            let (c, s) = stumpff(z);
            let chi2 = chi * chi;
            let value = sigma * chi2 * c + b * chi2 * chi * s + r0 * chi - sqrt_mu_tau;
            // F′ is the distance at χ, never negative; F″ is its derivative.
            let slope = sigma * chi * (1.0 - z * s) + b * chi2 * c + r0;
            let curvature = sigma * (1.0 - z * c) + b * chi * (1.0 - z * s);
            let discriminant =
                ((n - 1.0) * (n - 1.0) * slope * slope - n * (n - 1.0) * value * curvature).abs();
            let denominator = slope + discriminant.sqrt();
            if denominator > 0.0 {
                chi -= n * value / denominator;
            }
        }
        chi
    }

    /// The starting value of χ.
    ///
    /// A bound orbit, whose time is already within half a period, starts from the mean motion's
    /// χ = √μ τ α (Vallado 2013, algorithm 8). An unbound or parabolic one must not start far
    /// beyond the root: where the hyperbolic terms dominate, F grows as e^(√−α χ) and every step,
    /// Laguerre's as Newton's, gains only about 1 ÷ √−α. So it starts from the least of three
    /// values: the parabolic cubic (C and S at z = 0, which is exact for a parabola and close for
    /// a nearly parabolic orbit), for a clearly hyperbolic orbit the value its hyperbolic anomaly
    /// gives (between the epoch and the root), and √μ τ ÷ q, beyond the root for every conic
    /// because F′ = r never falls below the pericentre distance q. A negative τ is solved as the
    /// time-reversed orbit's positive one.
    fn starter(&self, sqrt_mu_tau: f64, sigma: f64) -> f64 {
        let (r0, alpha) = (self.radius, self.alpha);
        if alpha > 0.0 {
            return sqrt_mu_tau * alpha;
        }
        if sqrt_mu_tau.abs() <= 0.0 {
            return 0.0;
        }
        // Time reversal: χ(−τ; σ) = −χ(τ; −σ).
        let sign = sqrt_mu_tau.signum();
        let (t, sigma) = (sqrt_mu_tau.abs(), sign * sigma);
        let b = 1.0 - alpha * r0;
        let mut chi = t / self.pericentre_m();
        let cubic = parabolic_root(b, sigma, r0, t);
        if cubic.is_finite() && cubic > 0.0 {
            chi = chi.min(cubic);
        }
        let e = self.eccentricity();
        if alpha < 0.0 && e > HYPERBOLIC_STARTER_ECCENTRICITY {
            // Through the hyperbolic anomaly, since χ = √−a (H − H₀) exactly: H₀ from
            // σ = e √−a sinh H₀, the mean anomaly M = e sinh H₀ − H₀ + √μ τ (−α)^(3⁄2), and H from
            // asinh(M ÷ e), which lies between H₀ and the root, as sinh H = (M + H) ÷ e.
            let root_minus_a = (-1.0 / alpha).sqrt();
            let h0 = math::asinh(sigma / (e * root_minus_a));
            let mean = sigma / root_minus_a - h0 + t / (root_minus_a * root_minus_a * root_minus_a);
            let hyperbolic = root_minus_a * (math::asinh(mean / e) - h0);
            if hyperbolic.is_finite() && hyperbolic > 0.0 {
                chi = chi.min(hyperbolic);
            }
        }
        sign * chi
    }
}

/// The positive root of the parabolic Kepler equation `b χ³ ÷ 6 + σ χ² ÷ 2 + r₀ χ = t`, by
/// Cardano's formula on the depressed cubic; NaN or a non-positive value if it has none.
///
/// It is only a starting value, so its cancellations are harmless.
#[expect(
    clippy::many_single_char_names,
    reason = "the names are those of Cardano's formula"
)]
fn parabolic_root(b: f64, sigma: f64, r0: f64, t: f64) -> f64 {
    // χ³ + 3p χ² + k χ − m = 0 with p = σ ÷ b, k = 6 r₀ ÷ b, m = 6 t ÷ b; χ = y − p.
    let p = sigma / b;
    let k = 6.0 * r0 / b;
    let m = 6.0 * t / b;
    let depressed_p = (k - 3.0 * p * p).max(0.0);
    let depressed_q = 2.0 * p * p * p - k * p - m;
    let root =
        (depressed_q * depressed_q / 4.0 + depressed_p * depressed_p * depressed_p / 27.0).sqrt();
    let y = math::cbrt(-depressed_q / 2.0 + root) + math::cbrt(-depressed_q / 2.0 - root);
    y - p
}

/// Stumpff's functions `(C(z), S(z))`: `C = (1 − cos √z) ÷ z` and `S = (√z − sin √z) ÷ √z³` for
/// z > 0, their hyperbolic forms for z < 0, and `(1 ÷ 2, 1 ÷ 6)` at 0 (Battin 1999, §4.5).
fn stumpff(z: f64) -> (f64, f64) {
    if z.abs() < STUMPFF_SERIES_LIMIT {
        // C = Σ (−z)^k ÷ (2k + 2)!, S = Σ (−z)^k ÷ (2k + 3)!, nested from the last term.
        let mut c = 1.0;
        let mut s = 1.0;
        for k in (0..STUMPFF_SERIES_TERMS - 1).rev() {
            let k = f64::from(k);
            c = 1.0 - z / ((2.0 * k + 3.0) * (2.0 * k + 4.0)) * c;
            s = 1.0 - z / ((2.0 * k + 4.0) * (2.0 * k + 5.0)) * s;
        }
        (c / 2.0, s / 6.0)
    } else if z > 0.0 {
        let x = z.sqrt();
        let (sin, cos) = math::sin_cos(x);
        // 1 − cos x without its cancellation while cos x > 0.
        let one_minus_cos = if cos > 0.0 {
            sin * sin / (1.0 + cos)
        } else {
            1.0 - cos
        };
        (one_minus_cos / z, (x - sin) / (z * x))
    } else {
        let x = (-z).sqrt();
        ((math::cosh(x) - 1.0) / -z, (math::sinh(x) - x) / (-z * x))
    }
}

/// `a × b`, each component in the fixed order of its two products.
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

/// `|a|`.
fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::units::SolarMasses;
    use crate::units::consts::{METRES_PER_AU, SECONDS_PER_JULIAN_YEAR};

    /// The Milky Way fixture's black hole, 4.297 × 10⁶ M☉ (GRAVITY Collaboration 2022, A&A 657,
    /// L12), as the parameters draw it.
    fn sgr_a() -> GravitationalParameter {
        GravitationalParameter::from_solar_masses(SolarMasses::new(4.297e6))
    }

    const YEAR: f64 = SECONDS_PER_JULIAN_YEAR;

    /// The orbit of semi-major axis `a` (ly) and eccentricity `e` about `mu`, started at
    /// apocentre on +x moving along +y.
    fn from_apocentre(a: f64, e: f64, mu: GravitationalParameter) -> KeplerOrbit {
        let apocentre = a * (1.0 + e);
        let speed = (mu.value() * (1.0 - e) / (apocentre * METRES_PER_LIGHT_YEAR)).sqrt();
        KeplerOrbit::from_state(
            PointLy::new(apocentre, 0.0, 0.0),
            GalacticVelocity::new([0.0, speed, 0.0]),
            mu,
        )
        .unwrap()
    }

    fn relative(a: f64, b: f64) -> f64 {
        ((a - b) / b).abs()
    }

    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        length([a[0] - b[0], a[1] - b[1], a[2] - b[2]])
    }

    /// The orbit's energy scale: the larger of its kinetic and potential terms, which a parabolic
    /// orbit's near-zero energy cannot serve as.
    fn energy_scale(orbit: &KeplerOrbit) -> f64 {
        (0.5 * dot(orbit.velocity, orbit.velocity)).max(orbit.mu / orbit.radius)
    }

    /// A stress set: bound orbits to e = 0.9999, nearly parabolic both sides, parabolic to
    /// rounding and hyperbolic to e = 10, at 10⁻³–10 ly with random orientations and phases.
    fn stress_set() -> Vec<KeplerOrbit> {
        let mut lcg = Lcg::new(0x0928_a000_0000_0001);
        let eccentricities = [
            0.0,
            1e-6,
            0.3,
            0.88,
            0.99,
            0.9999,
            1.0 - 1e-9,
            1.0,
            1.0 + 1e-9,
            1.0001,
            1.5,
            3.0,
            10.0,
        ];
        let mut orbits = Vec::new();
        for &e in &eccentricities {
            for _ in 0..24 {
                // Pericentre from 10⁻³ to 10 ly, log-uniform.
                let q = 1e-3 * math::exp10(4.0 * lcg.next_f64());
                let mu = sgr_a().value();
                let q_m = q * METRES_PER_LIGHT_YEAR;
                // A true anomaly within the orbit's range, then the state there, in the plane.
                let limit = if e < 1.0 {
                    std::f64::consts::PI
                } else {
                    0.95 * math::acos(-1.0 / e)
                };
                let nu = limit * (2.0 * lcg.next_f64() - 1.0);
                let (sin, cos) = math::sin_cos(nu);
                let p = q_m * (1.0 + e);
                let r = p / (1.0 + e * cos);
                let vr = (mu / p).sqrt() * e * sin;
                let vt = (mu / p).sqrt() * (1.0 + e * cos);
                let planar_r = [r * cos, r * sin, 0.0];
                let planar_v = [vr * cos - vt * sin, vr * sin + vt * cos, 0.0];
                // A random rotation by three angles.
                let angles = [0, 1, 2].map(|_| std::f64::consts::TAU * lcg.next_f64());
                let rotate = |v: [f64; 3]| rotate(v, angles);
                let position = rotate(planar_r).map(|c| c / METRES_PER_LIGHT_YEAR);
                orbits.push(
                    KeplerOrbit::from_state(
                        PointLy::new(position[0], position[1], position[2]),
                        GalacticVelocity::new(rotate(planar_v)),
                        GravitationalParameter::new(mu),
                    )
                    .unwrap(),
                );
            }
        }
        orbits
    }

    #[expect(clippy::many_single_char_names, reason = "three angles and a vector")]
    fn rotate(v: [f64; 3], [a, b, c]: [f64; 3]) -> [f64; 3] {
        let turn = |v: [f64; 3], angle: f64, (i, j): (usize, usize)| {
            let (s, co) = math::sin_cos(angle);
            let mut out = v;
            out[i] = co * v[i] - s * v[j];
            out[j] = s * v[i] + co * v[j];
            out
        };
        turn(turn(turn(v, a, (0, 1)), b, (1, 2)), c, (0, 1))
    }

    /// Times from a second to the clock window, both signs.
    const TIMES_YEARS: [f64; 10] = [
        -1_000.0, -333.3, -16.0, -1.0, -1e-7, 1e-7, 1.0, 16.0, 333.3, 1_000.0,
    ];

    /// S2's orbit about the Milky Way's black hole: a 16-year period, 120 au at pericentre and
    /// 7,800 km/s there (brainstorm, "Orbits and time"). The elements are S2's measured ones,
    /// a = 125.058 mas at R₀ = 8,246.7 pc, 0.0163 ly, and e = 0.8846 (GRAVITY Collaboration 2020,
    /// A&A 636, L5, table E.1), which give 15.97 yr, 119.0 au and 7,771 km/s at this black hole's
    /// mass. The plan's a = 0.0158 ly is 1,000 au, a rounding of S2's 1,031 au, and gives 15.24 yr.
    #[test]
    fn an_s2_like_orbit_has_s2_s_period_pericentre_and_speed() {
        let orbit = from_apocentre(0.0163, 0.8846, sgr_a());
        let period = orbit.period().unwrap().value() / YEAR;
        assert!((period - 16.0).abs() < 0.1, "{period} yr");
        let at_pericentre = orbit.propagate(Seconds::new(0.5 * period * YEAR));
        let q = at_pericentre.distance().value() / METRES_PER_AU;
        assert!((q - 120.0).abs() < 2.0, "{q} au");
        let elements_q = orbit.pericentre().value() * METRES_PER_LIGHT_YEAR / METRES_PER_AU;
        assert!(relative(elements_q, q) < 1e-12, "{elements_q} au");
        let speed = at_pericentre.velocity().speed().value() / 1e3;
        assert!(relative(speed, 7_800.0) < 0.01, "{speed} km/s");

        let rounded = from_apocentre(0.0158, 0.88, sgr_a());
        let period = rounded.period().unwrap().value() / YEAR;
        assert!(relative(period, 15.24) < 1e-3, "{period} yr");
    }

    /// The plan's figure, 10⁻¹² relative, for the S2-like orbit; across the stress set the
    /// energy to 10⁻¹² of the larger of its kinetic and potential terms at either end, and the
    /// angular momentum to 10⁻¹² of |r| |v| at the larger end, since r × v is the difference of
    /// products that size, which far out on a hyperbola are a million times |h|.
    #[test]
    fn energy_and_angular_momentum_are_conserved_over_the_clock_window() {
        let s2 = from_apocentre(0.0163, 0.8846, sgr_a());
        for years in TIMES_YEARS {
            let later = s2.propagate(Seconds::new(years * YEAR));
            let de = relative(
                later.specific_energy_j_per_kg(),
                s2.specific_energy_j_per_kg(),
            );
            let h = s2.angular_momentum_m2_per_s();
            let dh = distance(later.angular_momentum_m2_per_s(), h) / length(h);
            assert!(de < 1e-12 && dh < 1e-12, "{de:e}, {dh:e} after {years} yr");
        }
        for orbit in stress_set() {
            let h = orbit.angular_momentum_m2_per_s();
            for years in TIMES_YEARS {
                let later = orbit.propagate(Seconds::new(years * YEAR));
                let scale = energy_scale(&orbit).max(energy_scale(&later));
                let de = (later.specific_energy_j_per_kg() - orbit.specific_energy_j_per_kg())
                    .abs()
                    / scale;
                assert!(
                    de < 1e-12,
                    "energy off by {de:e} after {years} yr: {orbit:?}"
                );
                let products = (orbit.radius * length(orbit.velocity))
                    .max(later.radius * length(later.velocity));
                let dh = distance(later.angular_momentum_m2_per_s(), h) / products;
                assert!(
                    dh < 1e-12,
                    "angular momentum off by {dh:e} after {years} yr: {orbit:?}"
                );
            }
        }
    }

    /// The plan's figure, 10⁻¹² relative, for the S2-like orbit. Across the stress set's bound
    /// orbits the error grows with the orbits completed, since the state `dt` on carries its own
    /// rounding of the period, and with 1 ÷ (1 − e), the sensitivity of a pericentre passage; the
    /// bound is 10⁻¹² times both, where the worst measured is some forty times inside it. An
    /// unbound orbit's round trip is limited by the same sensitivity at (r ÷ q)², a million
    /// million for a flyby a thousand years out, and is not held to a figure; nor are the nearly
    /// parabolic bound orbits, for which 1 ÷ (1 − e) is a billion.
    #[test]
    fn propagating_forward_then_back_returns_the_start() {
        let s2 = from_apocentre(0.0163, 0.8846, sgr_a());
        let quarter = s2.propagate(Seconds::new(4.0 * YEAR));
        for start in [s2, quarter] {
            for years in TIMES_YEARS {
                let dt = years * YEAR;
                let back = start
                    .propagate(Seconds::new(dt))
                    .propagate(Seconds::new(-dt));
                let dr = distance(back.position, start.position) / start.radius;
                let dv = distance(back.velocity, start.velocity) / length(start.velocity);
                assert!(dr < 1e-12 && dv < 1e-12, "{dr:e}, {dv:e} after ±{years} yr");
            }
        }
        // The nearly parabolic bound orbits, whose periods pass 10¹⁰ yr, go with the unbound.
        for orbit in stress_set()
            .into_iter()
            .filter(|o| o.eccentricity() < 0.999_99)
        {
            let e = orbit.eccentricity();
            for years in TIMES_YEARS {
                let dt = years * YEAR;
                let back = orbit
                    .propagate(Seconds::new(dt))
                    .propagate(Seconds::new(-dt));
                let dr = distance(back.position, orbit.position) / orbit.radius;
                let orbits = dt.abs() / orbit.period_si();
                let bound = 1e-12 * (1.0 + orbits) / (1.0 - e);
                let dv = distance(back.velocity, orbit.velocity) / length(orbit.velocity);
                assert!(
                    dr < bound && dv < bound,
                    "off by {dr:e}, {dv:e} after ±{years} yr: {orbit:?}"
                );
            }
        }
        let mut worst = 0.0_f64;
        for orbit in stress_set()
            .into_iter()
            .filter(|o| o.eccentricity() >= 0.999_99)
        {
            let q = orbit.pericentre_m();
            for years in TIMES_YEARS {
                let dt = years * YEAR;
                let later = orbit.propagate(Seconds::new(dt));
                let back = later.propagate(Seconds::new(-dt));
                let dr = distance(back.position, orbit.position) / orbit.radius;
                let dv = distance(back.velocity, orbit.velocity) / length(orbit.velocity);
                let reach = later.radius.max(orbit.radius) / q;
                let bound = 1e-12 * reach * reach;
                worst = worst.max(dr.max(dv) / (reach * reach));
                assert!(
                    dr < bound && dv < bound,
                    "off by {dr:e}, {dv:e} after ±{years} yr: {orbit:?}"
                );
            }
        }
        println!("worst unbound round trip ÷ (r ÷ q)²: {worst:e}");
    }

    /// The same orbit propagated twice gives the same bits.
    #[test]
    fn propagation_is_repeatable() {
        for orbit in stress_set() {
            let dt = Seconds::new(333.3 * YEAR);
            let (first, second) = (orbit.propagate(dt), orbit.propagate(dt));
            assert_eq!(first, second);
        }
    }

    /// The fixed count is enough: a further twenty steps move χ by rounding only, and the state
    /// solves Kepler's equation to the last few bits.
    #[test]
    fn the_fixed_iteration_count_converges() {
        let mut worst = 0_u32;
        // The stress set's own states. The states it reaches far out on a hyperbola, which a
        // propagation back starts from, have a root defined only to about 10⁻¹³, the rounding of
        // terms far larger than χ; the round-trip test covers them.
        let starts = stress_set();
        for orbit in starts {
            for years in TIMES_YEARS {
                let dt = years * YEAR;
                let sqrt_mu = orbit.mu.sqrt();
                let tau = orbit.reduced_time(dt);
                let sigma = dot(orbit.position, orbit.velocity) / sqrt_mu;
                let reference = orbit.solve(tau * sqrt_mu, sigma, 80);
                // A few units in the last place of χ: the root's own rounding noise.
                let tolerance = 1e-13 * reference.abs().max(1e-300);
                let needed = (0..=80)
                    .find(|&n| {
                        (orbit.solve(tau * sqrt_mu, sigma, n) - reference).abs() <= tolerance
                    })
                    .unwrap();
                worst = worst.max(needed);
                let fixed = orbit.solve(tau * sqrt_mu, sigma, UNIVERSAL_ITERATIONS);
                assert!(
                    (fixed - reference).abs() <= tolerance,
                    "χ {fixed} against {reference} after {years} yr: {orbit:?}"
                );
            }
        }
        println!("the fixed iteration count needed at most {worst} steps");
        assert!(worst + 3 <= UNIVERSAL_ITERATIONS, "needed {worst} steps");
    }

    #[test]
    fn a_whole_period_returns_a_bound_orbit_to_its_start() {
        let orbit = from_apocentre(0.3, 0.6, sgr_a());
        let period = orbit.period().unwrap();
        for n in [1.0, 7.0, -3.0] {
            let again = orbit.propagate(period * n);
            let dr = distance(again.position, orbit.position) / orbit.radius;
            assert!(dr < 1e-13, "off by {dr:e} after {n} periods");
        }
    }

    #[test]
    fn the_elements_describe_each_conic() {
        let circular = from_apocentre(1.0, 0.0, sgr_a());
        assert!(circular.eccentricity() < 1e-15);
        assert!(relative(circular.pericentre().value(), 1.0) < 1e-14);
        assert!(relative(circular.semi_major_axis().unwrap().value(), 1.0) < 1e-14);

        let r = METRES_PER_LIGHT_YEAR;
        let escape = (2.0 * sgr_a().value() / r).sqrt();
        let unbound = KeplerOrbit::from_state(
            PointLy::new(1.0, 0.0, 0.0),
            GalacticVelocity::new([0.0, 2.0 * escape, 0.0]),
            sgr_a(),
        )
        .unwrap();
        // Launched perpendicular at twice the escape speed: e = 2 v² r ÷ μ − 1 = 7, from pericentre.
        assert!(relative(unbound.eccentricity(), 7.0) < 1e-14);
        assert!(relative(unbound.pericentre().value(), 1.0) < 1e-14);
        assert_eq!(unbound.semi_major_axis(), None);
        assert_eq!(unbound.period(), None);
        assert!(unbound.specific_energy_j_per_kg() > 0.0);
        // It leaves: a thousand years on it is far out and still moving at above the escape speed
        // there.
        let later = unbound.propagate(Seconds::new(1_000.0 * YEAR));
        assert!(later.distance().value() > 2.0 * r);
    }

    /// Propagation matches a direct integration of the two-body problem where the orbit bends
    /// most, at a pericentre passage of e = 0.99: a check of the method that shares none of its
    /// algebra.
    #[test]
    fn propagation_agrees_with_a_direct_integration() {
        let orbit = from_apocentre(0.01, 0.99, sgr_a());
        let period = orbit.period().unwrap().value();
        // Start a tenth of a period before pericentre and integrate across it.
        let start = orbit.propagate(Seconds::new(0.4 * period));
        let span = 0.2 * period;
        let steps = 200_000_u32;
        let h = span / f64::from(steps);
        let mu = start.mu;
        let accel = |r: [f64; 3]| {
            let d = length(r);
            r.map(|c| -mu * c / (d * d * d))
        };
        // Fourth-order Runge–Kutta.
        let (mut r, mut v) = (start.position, start.velocity);
        for _ in 0..steps {
            let add = |a: [f64; 3], b: [f64; 3], k: f64| [0, 1, 2].map(|i| a[i] + k * b[i]);
            let (k1r, k1v) = (v, accel(r));
            let (k2r, k2v) = (add(v, k1v, h / 2.0), accel(add(r, k1r, h / 2.0)));
            let (k3r, k3v) = (add(v, k2v, h / 2.0), accel(add(r, k2r, h / 2.0)));
            let (k4r, k4v) = (add(v, k3v, h), accel(add(r, k3r, h)));
            r = [0, 1, 2].map(|i| r[i] + h / 6.0 * (k1r[i] + 2.0 * k2r[i] + 2.0 * k3r[i] + k4r[i]));
            v = [0, 1, 2].map(|i| v[i] + h / 6.0 * (k1v[i] + 2.0 * k2v[i] + 2.0 * k3v[i] + k4v[i]));
        }
        let kepler = start.propagate(Seconds::new(span));
        let dr = distance(kepler.position, r) / kepler.radius;
        assert!(dr < 1e-8, "off by {dr:e}");
    }

    #[test]
    fn stumpff_functions_join_their_series_smoothly() {
        for z in [-1.0, 1.0] {
            let (below_c, below_s) = stumpff(z * (1.0 - 1e-15));
            let (above_c, above_s) = stumpff(z * (1.0 + 1e-15));
            assert!(relative(below_c, above_c) < 1e-14, "C at {z}");
            assert!(relative(below_s, above_s) < 1e-14, "S at {z}");
        }
        let (c, s) = stumpff(0.0);
        assert!(relative(c, 0.5) < 1e-16 && relative(s, 1.0 / 6.0) < 1e-16);
        // At z = (2π)², a full turn: C = 0 and S = 1 ÷ (2π)².
        let turn = std::f64::consts::TAU * std::f64::consts::TAU;
        let (c, s) = stumpff(turn);
        assert!(c.abs() < 1e-16);
        assert!(relative(s, 1.0 / turn) < 1e-14);
    }

    #[test]
    fn invalid_states_are_refused() {
        let v = GalacticVelocity::new([0.0, 1e5, 0.0]);
        let r = PointLy::new(1.0, 0.0, 0.0);
        assert!(matches!(
            KeplerOrbit::from_state(PointLy::new(f64::NAN, 0.0, 0.0), v, sgr_a()),
            Err(BuildKeplerOrbitError::PositionNotFinite { .. })
        ));
        assert!(matches!(
            KeplerOrbit::from_state(r, GalacticVelocity::new([f64::INFINITY, 0.0, 0.0]), sgr_a()),
            Err(BuildKeplerOrbitError::VelocityNotFinite { .. })
        ));
        assert_eq!(
            KeplerOrbit::from_state(r, v, GravitationalParameter::new(0.0)),
            Err(BuildKeplerOrbitError::GravitationalParameterNotPositive {
                value: GravitationalParameter::new(0.0)
            })
        );
        assert_eq!(
            KeplerOrbit::from_state(PointLy::default(), v, sgr_a()),
            Err(BuildKeplerOrbitError::AtCentre)
        );
        assert_eq!(
            KeplerOrbit::from_state(r, GalacticVelocity::new([-1e5, 0.0, 0.0]), sgr_a()),
            Err(BuildKeplerOrbitError::Radial)
        );
        assert_eq!(
            KeplerOrbit::from_state(r, GalacticVelocity::default(), sgr_a()),
            Err(BuildKeplerOrbitError::Radial)
        );
    }
}
