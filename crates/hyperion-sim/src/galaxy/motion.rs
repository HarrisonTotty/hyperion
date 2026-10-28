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
//! # The regime (P09.T28.b.a)
//!
//! "The Kepler regime is a property of the system" (plan 09, Design note 12): a system is in it if
//! its epoch distance from the black hole is inside the influence radius, where the stars inside
//! weigh the black hole's mass, whatever the query's time, so no system changes regime during play
//! ([`Regime`], [`regime_of`]). [`position_velocity_at`] is a system's state at any time under its
//! regime's law: outside, plan 08's straight line, as plan 03's
//! [`position_at`](crate::galaxy::query::position_at) draws it, bit for bit; inside, the
//! [`KeplerOrbit`] from its epoch state about μ = G (`M_bh` + M★(< r₀)). A grid system takes plan
//! 08's epoch velocity as that state's velocity, and one whose orbit passes inside the black hole's
//! loss cone is "no such system", as a centre member's would have been at its placement
//! ([`loss_cone_verdict`]). The galaxy holds what these read, its [`CentreMotion`], and builds it
//! on first read (Design note 15).
//!
//! **Nothing on the record calls these yet.** Plan 03's `position_at`, `hit_at` and `resolve` are
//! unchanged, so no generated output moves; plan 09's P09.T28.b.b swaps them in behind its galaxy
//! hold for version 25 (decision-galaxy-25-train §1 and §2), and P09.T29's centre source reads them.
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
//! Mechanics* 39, 199), which converges from a poor starting value for nearly every state, run a
//! fixed [`UNIVERSAL_ITERATIONS`] times and never stopped early. Its result is then checked
//! against the equation, and a χ it leaves unsolved, to `UNSOLVED_RESIDUAL` of the equation's
//! largest term, is found again by a fixed `BISECTION_STEPS` bisections, which cannot fail: an
//! inbound flyby that passes its pericentre within the span can need them (P09.T28.a's fix, found
//! by P09.T28.b.a). A bound orbit's time is first reduced to within half a period of the epoch, so
//! the anomaly never winds up over the thousand orbits an inner star completes in the clock
//! window.
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
//! platform gives the same bits; the residual's test compares deterministic values, so a state
//! takes the bisection on every platform or on none. [`UNIVERSAL_ITERATIONS`],
//! `UNSOLVED_RESIDUAL`, `BISECTION_STEPS`, the Stumpff series' length, the bisection of the
//! influence radius and the form of every expression here belong to the generator version once
//! P09.T28.b.b wires the propagator
//! into the drift hook; until then nothing generated reads it. A grid system's velocity is plan
//! 08's draw on its own stream ([`epoch_velocity`]), which [`EpochState::of_grid_record`] reads and
//! nothing here adds a word to.

use std::error::Error;
use std::fmt;

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::galaxy::features::centre::members::CentreMemberRecord;
use crate::galaxy::features::centre::{BuildCentreError, CentreProfile, LossCone};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::potential::MassModel;
use crate::galaxy::query::{drifted, epoch_velocity};
use crate::galaxy::{Galaxy, PointLy};
use crate::math;
use crate::time::UniverseTime;
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SPEED_OF_LIGHT};
use crate::units::{GravitationalParameter, LightYears, Metres, Seconds, SolarMasses};

/// How many Laguerre–Conway iterations solve Kepler's equation in universal form: a fixed count,
/// never stopped early, so that every platform takes the same steps; a result they leave unsolved
/// is bisected (`UNSOLVED_RESIDUAL`).
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

/// The share of the largest of its terms within which Kepler's universal equation counts as solved
/// ([`KeplerOrbit::universal_anomaly`]): far above the 10⁻¹³ to which a solved χ leaves it, even
/// far out on a hyperbola, where its terms are a million times χ's own, and far below what an
/// unsolved one leaves, a large share of its terms.
const UNSOLVED_RESIDUAL: f64 = 1e-8;

/// How many bisections solve Kepler's universal equation when the iteration has not: 128 halve the
/// bracket `√μ τ ÷ q`, under 10²⁶ √m over ±10⁴ yr for any pericentre over a metre about any
/// black hole under 10¹⁰ M☉, by 3 × 10³⁸, to below the rounding of any root it holds. Steps
/// past the rounding leave the bracket where it is.
const BISECTION_STEPS: u32 = 128;

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

    /// The position relative to the black hole as a displacement in metres, galactic axes: the
    /// orbit's own state, without [`position`](Self::position)'s rounding to light-years.
    #[must_use]
    pub(crate) fn displacement(&self) -> GalacticDisplacement {
        GalacticDisplacement::new(self.position)
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
        let chi = self.universal_anomaly(tau * sqrt_mu, sigma, iterations);
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

    /// The universal anomaly χ, √m, at which Kepler's universal equation gives `sqrt_mu_tau`: by
    /// `iterations` Laguerre–Conway steps ([`KeplerOrbit::solve`]), or, if they leave the equation
    /// unsolved, by [`KeplerOrbit::bisect_anomaly`].
    ///
    /// From its starting value the iteration reaches the root in a few steps for every state of
    /// P09.T28.a's stress set, but not for every state the centre holds. An inbound flyby that
    /// passes its pericentre within the span starts near the pericentre, where `F′ = r` is least,
    /// and its first step can overshoot into the hyperbolic branch, where each step gains only
    /// about `1 ÷ √−α`: a rogue planet at 1.5 ly falling in at 958 km/s on an orbit of e = 1.45
    /// came out 26,480 ly away after −1,000 yr (plan 09's Risks, "P09.T28.a's propagator: an
    /// unsolved universal anomaly is bisected"). Such a χ is found by its
    /// residual, which is then a large share of the equation's terms, and solved again. A state
    /// the iteration solves keeps its bits, so P09.T28.a's goldens and tests are unchanged.
    #[must_use]
    fn universal_anomaly(&self, sqrt_mu_tau: f64, sigma: f64, iterations: u32) -> f64 {
        let chi = self.solve(sqrt_mu_tau, sigma, iterations);
        if self.solves(chi, sqrt_mu_tau, sigma) {
            chi
        } else {
            self.bisect_anomaly(sqrt_mu_tau, sigma)
        }
    }

    /// Whether `chi` solves Kepler's universal equation for `sqrt_mu_tau` to
    /// [`UNSOLVED_RESIDUAL`] of the largest of its terms.
    #[must_use]
    fn solves(&self, chi: f64, sqrt_mu_tau: f64, sigma: f64) -> bool {
        let (r0, alpha) = (self.radius, self.alpha);
        let z = alpha * chi * chi;
        let (c, s) = stumpff(z);
        let chi2 = chi * chi;
        let terms = [
            sigma * chi2 * c,
            (1.0 - alpha * r0) * chi2 * chi * s,
            r0 * chi,
            -sqrt_mu_tau,
        ];
        let residual = terms[0] + terms[1] + terms[2] + terms[3];
        let scale = terms.iter().fold(0.0_f64, |m, t| m.max(t.abs()));
        // An overflowed χ is never solved, whatever its terms' signs.
        residual.is_finite() && residual.abs() <= UNSOLVED_RESIDUAL * scale
    }

    /// χ by [`BISECTION_STEPS`] bisections of Kepler's universal equation `F(χ) = √μ τ`, with
    /// `F(χ) = σ₀ χ² C(z) + (1 − α r₀) χ³ S(z) + r₀ χ`. Slow, and certain, because the bracket
    /// always holds the one root:
    ///
    /// - `F` is strictly increasing: `dF ÷ dχ = r(χ)`, the distance at χ (Battin 1999, §4.5), which
    ///   is positive on every orbit that has a pericentre, as every one built does (not `Radial`,
    ///   not `AtCentre`);
    /// - `F(0) = 0`, and since `r ≥ q`, the pericentre distance, `F(√μ τ ÷ q) = ∫ r dχ` over
    ///   `[0, √μ τ ÷ q]` is at least `q × √μ τ ÷ q = √μ τ` for `τ > 0`, and at most it for `τ < 0`
    ///   over `[√μ τ ÷ q, 0]`, so the root lies between 0 and `√μ τ ÷ q`.
    ///
    /// Far out on a hyperbola the bracket's end can lie where the Stumpff terms overflow, and
    /// their sum is then not a number. Such a χ lies beyond the root: the terms grow with |χ|, and
    /// every root the clock window reaches is finite ([`KeplerOrbit::propagate`]'s domain), so the
    /// bracket's far end moves in to it.
    #[must_use]
    fn bisect_anomaly(&self, sqrt_mu_tau: f64, sigma: f64) -> f64 {
        let (r0, alpha) = (self.radius, self.alpha);
        let b = 1.0 - alpha * r0;
        let far = sqrt_mu_tau / self.pericentre_m();
        let (mut lo, mut hi) = if far >= 0.0 { (0.0, far) } else { (far, 0.0) };
        for _ in 0..BISECTION_STEPS {
            let chi = f64::midpoint(lo, hi);
            let z = alpha * chi * chi;
            let (c, s) = stumpff(z);
            let chi2 = chi * chi;
            let value = sigma * chi2 * c + b * chi2 * chi * s + r0 * chi;
            let below = if value.is_finite() {
                value < sqrt_mu_tau
            } else {
                far < 0.0
            };
            if below {
                lo = chi;
            } else {
                hi = chi;
            }
        }
        f64::midpoint(lo, hi)
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

/// The law a system moves by (plan 09, Design note 12): a property of the system, fixed by its
/// epoch distance from the black hole, so no system changes regime during play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Regime {
    /// At or outside the influence radius: a straight line from the epoch position at the epoch
    /// velocity, plan 08's drift.
    Straight,
    /// Inside the influence radius: the Kepler orbit about the black hole and the stars inside the
    /// epoch radius (brainstorm, "Orbits and time").
    Kepler,
}

impl Regime {
    /// A short snake-case name, `straight` or `kepler`, for logs and golden files.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::Kepler => "kepler",
        }
    }
}

/// What a galaxy's Kepler regime reads of its centre: the stars about the black hole, the
/// influence radius where they weigh it and the black hole's loss cone, held by the galaxy and
/// built on first read (plan 09, Design note 15; P09.T28.b.a), through [`Galaxy::centre_motion`].
///
/// - **The mass inside** a radius, M(< r), is the nuclear cluster's ([`CentreProfile`]) and the
///   rest of the galaxy's discs, bar and bulge, as the centre's spherical average of the mass
///   model holds them
///   ([`SphericalAverage::extended_mass`](crate::galaxy::potential::SphericalAverage::extended_mass)),
///   which includes the gas disc and not the dark halo ([`stars_within`](Self::stars_within)).
///   The gas is 2 × 10⁻⁶ of it at the Milky Way fixture's edge and 10⁻⁴ at seed 1's, and an NFW
///   halo of Milky Way normalisation holds some 0.1% of the black hole's mass inside the
///   fixture's edge: "gas and dark matter probably do not contribute significantly" there
///   (Genzel, Eisenhauer and Gillessen 2010, Rev. Mod. Phys. 82, 3121, §5). The brainstorm's
///   regime is where the black hole "outweighs the stars inside" ("Orbits and time"), all of
///   them: a fifth of drawn galaxies have a black hole heavier than their whole cluster (60 of
///   seeds 1–300), which the cluster alone never weighs, so its edge would be no radius at all
///   (main's ruling of 2026-10-27 on P09.T28.b.a; plan 09's Risks, "P09.T28.b.a as built").
/// - **The influence radius** `r_h` is where the mass inside equals the black hole's,
///   M(< `r_h`) = `M_bh` ([`influence_radius`](Self::influence_radius)): the enclosed-mass
///   convention of Baumgardt, Amaro-Seoane and Schödel (2018, A&A 609, A28, §3, "the radius
///   where the cumulated mass in stars becomes equal to the central black hole mass"). It is
///   neither Merritt's `r_m`, where M★(< `r_m`) = 2 `M_bh`, nor G `M_bh` ÷ σ² (Merritt 2004,
///   arXiv `astro-ph/0301257`, eqs. 1–2; Merritt 2013, CQG 30, 244005, eqs. 5–6), which agree with each
///   other in a singular isothermal sphere, where this radius is half of them. At Milky Way
///   values it is 7.19 ly, 2.20 pc, against the 2.18 pc that Schödel et al.'s (2018, A&A 609,
///   A27, §4.5) masses inside 1 and 3 pc give; the brainstorm's "about 10 ly" is the
///   literature's ~3 pc, which is `r_m`, some 11 ly here. Over drawn galaxies it is 1.9–177 ly,
///   median 11.8 ly (seeds 1–300; P09.T28.b.a as built).
/// - **μ** of an orbit from r₀ is G (`M_bh` + M(< r₀)) over the same mass
///   ([`enclosed_mass`](Self::enclosed_mass)), held fixed along the orbit (brainstorm, "Orbits
///   and time"): the spherical average's exact pull at r₀, which errs as M(< r) changes along
///   the orbit, by about 1% of `M_bh` at 0.5 ly, where orbits close within the clock window, and
///   over under a seventh of a period each way beyond 1 ly. The enclosed stars' precession is
///   P09.T28.d's.
/// - **The loss cone** is the one the centre's members are thinned by ([`LossCone`]).
///
/// The profile is the centre's model's, bit for bit
/// ([`CentreModel`](crate::galaxy::features::centre::CentreModel)'s: the same parameters and the
/// same mass model), and so is the loss cone; a test holds them equal. The model's own
/// [`influence_radius`](crate::galaxy::features::centre::CentreModel::influence_radius) counts the
/// cluster alone and is unchanged. The galaxy holds this part and not the whole model, whose
/// four inversions take 91 ms (P09.T27 as built), and builds it when first read: about 64 ms,
/// nearly all the galaxy's spherical average (P09.T28.b.a as built). It is a pure function of the
/// parameters and the mass model.
#[derive(Debug, Clone, PartialEq)]
pub struct CentreMotion {
    profile: CentreProfile,
    influence_radius: LightYears,
    loss_cone: LossCone,
}

impl CentreMotion {
    /// The centre's motion of a galaxy of `params` whose mass model is `model`.
    ///
    /// # Errors
    ///
    /// [`BuildCentreError::Mass`] if the nuclear cluster's or the black hole's mass is negative
    /// or not finite, which no parameters the builder accepts have;
    /// [`BuildCentreError::NoInfluenceRadius`] if the mass inside 10⁻³–10⁴ ly does not bracket the
    /// black hole's, which no galaxy of the sweep (seeds 1–300) and no model of its own
    /// parameters does.
    pub fn new(params: &GalaxyParams, model: &MassModel) -> Result<Self, BuildCentreError> {
        let black_hole = params.black_hole().mass();
        let profile = CentreProfile::new(params.nuclear_cluster(), black_hole)?.in_galaxy(model);
        let influence_radius = influence_radius_of(&profile)?;
        Ok(Self {
            profile,
            influence_radius,
            loss_cone: LossCone::new(black_hole),
        })
    }

    /// The centre's profile: the nuclear cluster about the black hole, in the galaxy's monopole.
    #[must_use]
    pub fn profile(&self) -> &CentreProfile {
        &self.profile
    }

    /// The influence radius, where the stars inside weigh the black hole's mass: the edge of the
    /// Kepler regime.
    #[must_use]
    pub fn influence_radius(&self) -> LightYears {
        self.influence_radius
    }

    /// The black hole's loss cone.
    #[must_use]
    pub fn loss_cone(&self) -> &LossCone {
        &self.loss_cone
    }

    /// The bytes it owns on the heap, about 20 KiB: the cluster's tables and the galaxy's
    /// monopole.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.profile.heap_bytes()
    }

    /// The stars inside `r` of the black hole: the nuclear cluster's and the rest of the galaxy's
    /// discs, bar and bulge (their gas disc included), in that order.
    #[must_use]
    pub fn stars_within(&self, r: LightYears) -> SolarMasses {
        stars_within(&self.profile, r.value())
    }

    /// The mass a Kepler orbit from `r` out of the black hole moves about: the black hole's and
    /// the stars' inside `r` (brainstorm, "Orbits and time").
    #[must_use]
    pub fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        self.profile.black_hole() + self.stars_within(r)
    }

    /// The regime of a system whose epoch position is `position`: [`Regime::Kepler`] strictly
    /// inside the influence radius, [`Regime::Straight`] at it and beyond.
    #[must_use]
    pub fn regime_at(&self, position: &GalacticPosition) -> Regime {
        if distance_ly(&PointLy::from(position)) < self.influence_radius.value() {
            Regime::Kepler
        } else {
            Regime::Straight
        }
    }

    /// The Kepler orbit `start` follows if it is in the regime, about μ = G (`M_bh` + M(< r₀)) at
    /// its epoch distance r₀ ([`enclosed_mass`](Self::enclosed_mass)), the orbit
    /// [`position_velocity_at`] propagates, bit for bit; `None` for a state that has none, at the
    /// black hole itself or with no angular momentum, which includes every grid system of a
    /// galaxy built without its kinematic tables (P09.T28.a as built). It does not ask the
    /// regime: for P09.T28.c's pericentre and P09.T29's cache of orbits.
    ///
    /// # Panics
    ///
    /// Never: an [`EpochState`]'s velocity is finite, a record's position is, and the mass inside
    /// is finite and positive.
    #[must_use]
    pub fn kepler_orbit(&self, start: &EpochState) -> Option<KeplerOrbit> {
        let position = PointLy::from(start.record.epoch_position());
        let mu = GravitationalParameter::from_solar_masses(
            self.enclosed_mass(LightYears::new(distance_ly(&position))),
        );
        match KeplerOrbit::from_state(position, start.velocity, mu) {
            Ok(orbit) => Some(orbit),
            Err(BuildKeplerOrbitError::AtCentre | BuildKeplerOrbitError::Radial) => None,
            Err(
                error @ (BuildKeplerOrbitError::PositionNotFinite { .. }
                | BuildKeplerOrbitError::VelocityNotFinite { .. }
                | BuildKeplerOrbitError::GravitationalParameterNotPositive { .. }),
            ) => panic!(
                "an epoch state is finite by its constructors, and the mass inside it is finite \
                 and positive: {error}"
            ),
        }
    }
}

/// The distance of `p` from the black hole, at the origin, ly.
#[must_use]
fn distance_ly(p: &PointLy) -> f64 {
    length([p.x, p.y, p.z])
}

/// [`CentreMotion::stars_within`] of `profile` at `r` ly.
#[must_use]
fn stars_within(profile: &CentreProfile, r: f64) -> SolarMasses {
    let rest = profile
        .galaxy()
        .map_or(SolarMasses::ZERO, |g| g.extended_mass(LightYears::new(r)));
    profile.stellar_mass_within(r) + rest
}

/// The radius where the stars inside `profile`'s black hole weigh its mass, by a fixed bisection
/// in `ln r` over 10⁻³–10⁴ ly, as [`CentreProfile::influence_radius`] takes the cluster's.
///
/// The bracket holds the root at every galaxy of the sweep (seeds 1–300, roots at 1.9–177 ly;
/// P09.T28.b.a as built): inside 10⁻³ ly the cluster holds about one solar mass, and 10⁴ ly takes
/// in the bulge and the bar.
///
/// # Errors
///
/// [`BuildCentreError::NoInfluenceRadius`] if the bracket holds no root.
fn influence_radius_of(profile: &CentreProfile) -> Result<LightYears, BuildCentreError> {
    let black_hole = profile.black_hole().value();
    let excess = |ln_r: f64| stars_within(profile, math::exp(ln_r)).value() - black_hole;
    let (lo, hi) = (math::ln(1e-3), math::ln(1e4));
    if !(excess(lo) < 0.0 && excess(hi) > 0.0) {
        return Err(BuildCentreError::NoInfluenceRadius);
    }
    Ok(LightYears::new(math::exp(crate::galaxy::quad::bisect(
        excess, lo, hi, 64,
    ))))
}

/// The regime of `record` in `galaxy` (plan 09, Design note 12): [`Regime::Kepler`] if its epoch
/// distance from the black hole is inside the galaxy's influence radius
/// ([`CentreMotion::influence_radius`]), whatever the time it is asked about, and
/// [`Regime::Straight`] otherwise.
///
/// It reads the epoch position alone, so it is the same for a grid system, a feature member or a
/// centre member, and costs a distance.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::motion::{Regime, regime_of};
/// use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
/// use hyperion_sim::galaxy::{Galaxy, Population};
/// use hyperion_sim::id::{CentreMemberId, Layer, MemberSlot, SystemId};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let galaxy = Galaxy::new(Seed::new(7));
/// let edge = galaxy.centre_motion().influence_radius().value();
/// let slot = MemberSlot::InCell { band: Layer::A, level: 4, cell: [0, 0, 0], index: 0 };
/// let id = SystemId::from(CentreMemberId::new(slot)?);
/// let at = |x: f64| {
///     SystemRecord::from_parts(
///         id,
///         GalacticPosition::from_light_years([x, 0.0, 0.0]).expect("in range"),
///         SystemOrigin::CentreMember { attempt: 0 },
///         Population::NuclearDisc,
///         SolarMasses::new(1.0),
///         Years::new(1e9),
///     )
/// };
/// // The edge sits some light-years out, where the stars inside weigh the black hole.
/// assert!((3.0..30.0).contains(&edge), "{edge} ly");
/// assert_eq!(regime_of(&galaxy, &at(0.9 * edge)), Regime::Kepler);
/// assert_eq!(regime_of(&galaxy, &at(1.1 * edge)), Regime::Straight);
/// # Ok::<(), hyperion_sim::id::BuildSystemIdError>(())
/// ```
#[must_use]
pub fn regime_of(galaxy: &Galaxy, record: &SystemRecord) -> Regime {
    galaxy.centre_motion().regime_at(record.epoch_position())
}

/// A system's state at the epoch, where its motion starts: its record and its epoch velocity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EpochState {
    record: SystemRecord,
    velocity: GalacticVelocity,
}

impl EpochState {
    /// The state of `record` moving at `velocity` at the epoch, for a source whose velocity is its
    /// own, and for tests.
    ///
    /// # Errors
    ///
    /// [`BuildEpochStateError::VelocityNotFinite`] if a component is not finite, and
    /// [`BuildEpochStateError::FasterThanLight`] if the speed is not below light's, so that no
    /// law here can take the state out of the addressable range over the clock window.
    pub fn new(
        record: SystemRecord,
        velocity: GalacticVelocity,
    ) -> Result<Self, BuildEpochStateError> {
        if !velocity.metres_per_second().iter().all(|c| c.is_finite()) {
            return Err(BuildEpochStateError::VelocityNotFinite { velocity });
        }
        if velocity.speed().value() >= SPEED_OF_LIGHT {
            return Err(BuildEpochStateError::FasterThanLight { velocity });
        }
        Ok(Self { record, velocity })
    }

    /// A grid system's state: its record and plan 08's epoch velocity ([`epoch_velocity`]), zero in
    /// a galaxy built without its kinematic tables (plan 09, Design note 12: "a grid system there
    /// takes plan 08's velocity as the initial condition").
    ///
    /// # Panics
    ///
    /// As [`epoch_velocity`]: in a galaxy with kinematic tables, if `record` is not a grid record.
    #[must_use]
    pub fn of_grid_record(galaxy: &Galaxy, record: &SystemRecord) -> Self {
        // Plan 08's draw is finite and below its layer's pad speed, at most 3,000 km/s.
        Self {
            record: *record,
            velocity: epoch_velocity(galaxy, record),
        }
    }

    /// A centre member's state: its record and its own velocity, drawn from the centre's
    /// distribution function at its placement (P09.T27).
    #[must_use]
    pub fn of_centre_member(member: &CentreMemberRecord) -> Self {
        // The distribution function's draw is finite and below the escape speed, under light's.
        Self {
            record: *member.record(),
            velocity: member.velocity(),
        }
    }

    /// The system's record.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The velocity at the epoch, galactic axes.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }
}

/// An [`EpochState`] could not be built: its velocity is not one any system can have.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildEpochStateError {
    /// A component of the velocity was not finite.
    VelocityNotFinite {
        /// The offending velocity.
        velocity: GalacticVelocity,
    },
    /// The speed was not below light's.
    FasterThanLight {
        /// The offending velocity.
        velocity: GalacticVelocity,
    },
}

impl fmt::Display for BuildEpochStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VelocityNotFinite { velocity } => {
                let [x, y, z] = velocity.metres_per_second();
                write!(f, "velocity ({x}, {y}, {z}) m/s is not finite")
            }
            Self::FasterThanLight { velocity } => write!(
                f,
                "speed {} m/s is not below light's",
                velocity.speed().value()
            ),
        }
    }
}

impl Error for BuildEpochStateError {}

/// Where a system is at a time and how it moves there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionState {
    position: GalacticPosition,
    velocity: GalacticVelocity,
}

impl MotionState {
    /// The position, galactic.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// The velocity, galactic axes.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }
}

/// A system's state at a time under its regime's law, or the loss cone's verdict that there is no
/// such system ([`position_velocity_at`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StateAt {
    /// The system is there, in this state.
    Exists(MotionState),
    /// Its orbit passes inside the black hole's loss cone, so there is no such system at any time
    /// (plan 09, Design note 12; [`LossConeVerdict::Inside`]).
    NoSuchSystem,
}

/// Whether a system's orbit passes inside the black hole's loss cone (plan 09, Design note 12):
/// "one whose pericentre falls inside the loss cone resolves to 'no such system', as a member
/// would".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LossConeVerdict {
    /// The system exists: its orbit stays outside the loss cone, or the verdict does not apply to
    /// it ([`loss_cone_verdict`]).
    Clear,
    /// Its Kepler pericentre about the black hole lies inside the loss cone: there is no such
    /// system.
    Inside,
}

/// The loss cone's verdict on a system starting from `start` in `galaxy` (plan 09, Design note
/// 12), the same test the centre's members are thinned by
/// ([`LossCone::contains`](crate::galaxy::features::centre::LossCone::contains): the Kepler
/// pericentre about the black hole alone, inside about 2 au).
///
/// It applies to a system in the Kepler regime, which would follow that orbit. It is
/// [`LossConeVerdict::Clear`] for:
///
/// - a system in the straight regime, which follows no orbit about the black hole;
/// - a centre member, whose placement has passed the same test already (P09.T25): a member that
///   resolves is outside the loss cone, so the verdict is not taken again on its rounded record;
/// - a state with no orbit, at the black hole itself or with no angular momentum, which includes
///   every grid system of a galaxy built without its kinematic tables.
#[must_use]
pub fn loss_cone_verdict(galaxy: &Galaxy, start: &EpochState) -> LossConeVerdict {
    let motion = galaxy.centre_motion();
    if motion.regime_at(start.record.epoch_position()) == Regime::Straight
        || motion.kepler_orbit(start).is_none()
    {
        return LossConeVerdict::Clear;
    }
    verdict_on(motion, start)
}

/// [`loss_cone_verdict`] for a Kepler-regime `start` that has an orbit.
#[must_use]
fn verdict_on(motion: &CentreMotion, start: &EpochState) -> LossConeVerdict {
    match start.record.origin() {
        SystemOrigin::CentreMember { .. } => return LossConeVerdict::Clear,
        SystemOrigin::Grid(_) | SystemOrigin::FeatureMember { .. } => {}
    }
    let p = PointLy::from(start.record.epoch_position());
    let velocity_km_s = start.velocity.metres_per_second().map(|c| c / 1e3);
    if motion.loss_cone.contains([p.x, p.y, p.z], velocity_km_s) {
        LossConeVerdict::Inside
    } else {
        LossConeVerdict::Clear
    }
}

/// Where a system starting from `start` is at `t` in `galaxy`, and how it moves there, under its
/// regime's law (plan 09, Design note 12; P09.T28.b.a).
///
/// - **[`Regime::Straight`]:** plan 08's straight line from the epoch position at the epoch
///   velocity, the same bits as plan 03's [`position_at`](crate::galaxy::query::position_at)
///   gives a grid system.
/// - **[`Regime::Kepler`]:** the [`KeplerOrbit`] from the epoch state about μ = G (`M_bh` +
///   M★(< r₀)), r₀ the epoch distance from the black hole ([`CentreMotion::enclosed_mass`]),
///   propagated by the span from the epoch to `t`. An orbit that passes inside the loss cone is
///   [`StateAt::NoSuchSystem`] at every time ([`loss_cone_verdict`]). A state with no orbit, the
///   black hole itself or a state with no angular momentum, keeps its epoch position, which is how
///   a grid system of a galaxy built without its kinematic tables stays put, as plan 03's systems
///   do there.
///
/// At the epoch either law gives the epoch state to the bit. The two laws meet at the influence
/// radius: a thousand years on they differ there by G `M_bh` t² ÷ r², the pull of the black hole
/// and the mass inside over the span to leading order in t, 0.0130 ly at the Milky Way fixture's
/// 7.19 ly (P09.T28.b.a as built).
///
/// Nothing on the record calls this yet: plan 03's `position_at` is unchanged until P09.T28.b.b.
///
/// # Panics
///
/// Never: an [`EpochState`] is finite and slower than light, and no law moves anything slower
/// than light by 1,000 ly over the clock window, from inside the cube or an influence radius
/// (under 200 ly at every drawn seed of P09.T28.b.a's sweep), against the range's 2³¹ ly.
///
/// # Examples
///
/// A star on a circle at 0.1 ly from the black hole stays at 0.1 ly, where a straight line would
/// have left it:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::galaxy::motion::{EpochState, StateAt, position_velocity_at};
/// use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
/// use hyperion_sim::galaxy::{Galaxy, Population};
/// use hyperion_sim::id::{CentreMemberId, Layer, MemberSlot, SystemId};
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
/// use hyperion_sim::units::{GravitationalParameter, LightYears, SolarMasses, Years};
///
/// let galaxy = Galaxy::new(Seed::new(7));
/// let slot = MemberSlot::InCell { band: Layer::A, level: 4, cell: [0, 0, 0], index: 0 };
/// let record = SystemRecord::from_parts(
///     SystemId::from(CentreMemberId::new(slot)?),
///     GalacticPosition::from_light_years([0.1, 0.0, 0.0]).expect("in range"),
///     SystemOrigin::CentreMember { attempt: 0 },
///     Population::NuclearDisc,
///     SolarMasses::new(1.0),
///     Years::new(1e9),
/// );
/// // The circular speed about the black hole and the stars inside 0.1 ly.
/// let inside = galaxy.centre_motion().enclosed_mass(LightYears::new(0.1));
/// let mu = GravitationalParameter::from_solar_masses(inside).value();
/// let speed = (mu / (0.1 * METRES_PER_LIGHT_YEAR)).sqrt();
/// let start = EpochState::new(record, GalacticVelocity::new([0.0, speed, 0.0]))?;
/// let t = UniverseTime::from_julian_years(100).expect("in the clock window");
/// let StateAt::Exists(later) = position_velocity_at(&galaxy, &start, t) else {
///     panic!("a circular orbit at 0.1 ly misses the loss cone");
/// };
/// let r = GalacticPosition::ORIGIN.distance_to(later.position()).value() / METRES_PER_LIGHT_YEAR;
/// assert!((r - 0.1).abs() < 1e-9, "{r} ly");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn position_velocity_at(galaxy: &Galaxy, start: &EpochState, t: UniverseTime) -> StateAt {
    let motion = galaxy.centre_motion();
    let epoch = MotionState {
        position: *start.record.epoch_position(),
        velocity: start.velocity,
    };
    match motion.regime_at(start.record.epoch_position()) {
        Regime::Straight => {
            if t == UniverseTime::EPOCH {
                return StateAt::Exists(epoch);
            }
            StateAt::Exists(MotionState {
                position: drifted(&start.record, start.velocity, t),
                velocity: start.velocity,
            })
        }
        Regime::Kepler => {
            let Some(orbit) = motion.kepler_orbit(start) else {
                return StateAt::Exists(epoch);
            };
            match verdict_on(motion, start) {
                LossConeVerdict::Inside => return StateAt::NoSuchSystem,
                LossConeVerdict::Clear => {}
            }
            if t == UniverseTime::EPOCH {
                return StateAt::Exists(epoch);
            }
            let later = orbit.propagate(Seconds::new(t.since_epoch().as_seconds_f64()));
            let position = GalacticPosition::ORIGIN
                .translated(later.displacement())
                .expect(
                    "an orbit slower than light moves under 1,000 ly over the clock window, from \
                     inside the influence radius, and stays in the addressable range",
                );
            StateAt::Exists(MotionState {
                position,
                velocity: later.velocity(),
            })
        }
    }
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
                // The stress set never takes the fallback, so its states keep the iterated χ.
                assert!(
                    orbit.solves(fixed, tau * sqrt_mu, sigma),
                    "the stress set took the fallback after {years} yr: {orbit:?}"
                );
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

    /// The position `dt` seconds on of a hyperbolic orbit by its hyperbolic anomaly, a reference
    /// independent of the universal variables: `M = e sinh H − H` advanced by the mean motion
    /// `√(μ ÷ |a|³)` and solved for H by bisection (it is increasing in H), and the perifocal
    /// position `x = |a| (e − cosh H)`, `y = |a| √(e² − 1) sinh H` (Vallado 2013, §2.2), with
    /// `sinh H₀ = r · v ÷ (e √(μ |a|))` at the start.
    #[expect(
        clippy::many_single_char_names,
        reason = "the literature's symbols: r, v, e, h, x, y"
    )]
    fn hyperbolic_reference(orbit: &KeplerOrbit, dt: f64) -> [f64; 3] {
        let (r, v, mu) = (orbit.position, orbit.velocity, orbit.mu);
        let semi = -1.0 / orbit.alpha;
        let ev = orbit.eccentricity_vector();
        let e = length(ev);
        let p_hat = ev.map(|c| c / e);
        let h = cross(r, v);
        let q_hat = cross(h, p_hat).map(|c| c / length(h));
        let h0 = math::asinh(dot(r, v) / (e * (mu * semi).sqrt()));
        let mean = e * math::sinh(h0) - h0 + (mu / (semi * semi * semi)).sqrt() * dt;
        let reach = math::asinh(mean.abs() / (e - 1.0)) + 1.0;
        let (mut lo, mut hi) = (-reach, reach);
        for _ in 0..200 {
            let mid = f64::midpoint(lo, hi);
            if e * math::sinh(mid) - mid < mean {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let anomaly = f64::midpoint(lo, hi);
        let x = semi * (e - math::cosh(anomaly));
        let y = semi * (e * e - 1.0).sqrt() * math::sinh(anomaly);
        [0, 1, 2].map(|i| x * p_hat[i] + y * q_hat[i])
    }

    /// The position `dt` seconds on of a parabolic orbit by Barker's equation, a reference
    /// independent of the universal variables: `D + D³ ÷ 3 = W`, D = tan(ν ÷ 2), with W advanced
    /// by `dt ÷ √(2q³ ÷ μ)` and solved by bisection, and the perifocal position `x = q (1 − D²)`,
    /// `y = 2qD` (Vallado 2013, §2.2), with `D₀ = r · v ÷ √(2μq)` at the start.
    #[expect(
        clippy::many_single_char_names,
        reason = "the literature's symbols: r, v, h, q, w, d, x, y"
    )]
    fn parabolic_reference(orbit: &KeplerOrbit, dt: f64) -> [f64; 3] {
        let (r, v, mu) = (orbit.position, orbit.velocity, orbit.mu);
        let h = cross(r, v);
        let q = dot(h, h) / (2.0 * mu);
        let ev = orbit.eccentricity_vector();
        let p_hat = ev.map(|c| c / length(ev));
        let q_hat = cross(h, p_hat).map(|c| c / length(h));
        let d0 = dot(r, v) / (2.0 * mu * q).sqrt();
        let w = d0 + d0 * d0 * d0 / 3.0 + dt / (2.0 * q * q * q / mu).sqrt();
        let reach = math::cbrt(3.0 * w.abs()) + 1.0;
        let (mut lo, mut hi) = (-reach, reach);
        for _ in 0..200 {
            let mid = f64::midpoint(lo, hi);
            if mid + mid * mid * mid / 3.0 < w {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let d = f64::midpoint(lo, hi);
        let (x, y) = (q * (1.0 - d * d), 2.0 * q * d);
        [0, 1, 2].map(|i| x * p_hat[i] + y * q_hat[i])
    }

    /// A rogue planet of seed 0x5eed0000c0ffee00's centre (`0xcfffdfffc0005f87`, P09.T28.b.a's
    /// pre-pin): 1.5 ly out on a nearly radial flyby, e = 1.45, q = 0.0875 ly. A thousand years
    /// back it passes its pericentre on the way in, and ten Laguerre–Conway steps from the
    /// starting value leave Kepler's equation unsolved: the state came out 26,480 ly away, where
    /// the orbit is some 2 ly out. The bisection solves it, to the hyperbolic anomaly's answer.
    #[test]
    fn an_inbound_flyby_through_its_pericentre_is_solved_pinned() {
        let orbit = KeplerOrbit::from_state(
            PointLy::new(
                -0.924_728_158_321_281,
                -0.523_016_646_507_413_1,
                1.062_825_901_658_277_5,
            ),
            GalacticVelocity::new([
                -521_820.097_021_385_2,
                -277_315.864_063_661_37,
                753_724.179_127_419,
            ]),
            GravitationalParameter::new(1.344_056_460_257_621_7e27),
        )
        .unwrap();
        assert!((orbit.eccentricity() - 1.4486).abs() < 1e-4);
        let dt = -1_000.0 * YEAR;
        let sqrt_mu = orbit.mu.sqrt();
        let sigma = dot(orbit.position, orbit.velocity) / sqrt_mu;
        let target = orbit.reduced_time(dt) * sqrt_mu;
        let iterated = orbit.solve(target, sigma, UNIVERSAL_ITERATIONS);
        assert!(
            !orbit.solves(iterated, target, sigma),
            "the iteration solved it"
        );
        let chi = orbit.universal_anomaly(target, sigma, UNIVERSAL_ITERATIONS);
        assert!(orbit.solves(chi, target, sigma));
        let later = orbit.propagate(Seconds::new(dt));
        let expected = hyperbolic_reference(&orbit, dt);
        let off = distance(later.position, expected) / length(expected);
        assert!(off < 1e-9, "{off:e} from the hyperbolic anomaly's position");
        let r = later.distance().value() / METRES_PER_LIGHT_YEAR;
        assert!((1.5..2.5).contains(&r), "{r} ly");
        let de = relative(
            later.specific_energy_j_per_kg(),
            orbit.specific_energy_j_per_kg(),
        );
        assert!(de < 1e-12, "{de:e}");
    }

    /// Nearly radial and parabolic flybys, inbound and outbound, at e from 1 to 2, pericentres
    /// of 10⁻⁴–1 ly and starts 1–300 pericentres out, over up to ±10⁴ yr, each against a
    /// reference independent of the universal variables: Barker's equation for the parabolas and
    /// the hyperbolic anomaly for the rest, both solved by bisection. The sweep reaches the two
    /// regions where ten steps fall short (a solve towards the pericentre from 20 or more
    /// pericentres out, and a nearly parabolic flyby over 10⁶ of its pericentre's time scale
    /// √(q³ ÷ μ)), so the fallback serves some of it; every propagation solves Kepler's
    /// equation, and the positions agree to 10⁻⁹ of the farther radius.
    #[test]
    fn nearly_radial_flybys_match_independent_references_pinned() {
        let mut lcg = Lcg::new(0x0928_a000_0000_0f1b);
        let mu = sgr_a().value();
        let (mut worst, mut fallbacks, mut count) = (0.0_f64, 0_u32, 0_u32);
        for e in [1.0, 1.0 + 1e-6, 1.0 + 1e-4, 1.01, 1.05, 1.2, 1.45, 2.0] {
            for n in 0..24 {
                let q = 1e-4 * math::exp10(4.0 * lcg.next_f64());
                let out = math::exp10(math::log10(300.0) * lcg.next_f64());
                if q * out > 10.0 {
                    continue;
                }
                let q_m = q * METRES_PER_LIGHT_YEAR;
                let p = q_m * (1.0 + e);
                // The true anomaly `out` pericentres out, falling in for even n and leaving for odd.
                let cos = ((1.0 + e) / out - 1.0) / e;
                let sin = (1.0 - cos * cos).max(0.0).sqrt() * if n % 2 == 0 { -1.0 } else { 1.0 };
                let r = p / (1.0 + e * cos);
                let vr = (mu / p).sqrt() * e * sin;
                let vt = (mu / p).sqrt() * (1.0 + e * cos);
                let angles = [0, 1, 2].map(|_| std::f64::consts::TAU * lcg.next_f64());
                let position =
                    rotate([r * cos, r * sin, 0.0], angles).map(|c| c / METRES_PER_LIGHT_YEAR);
                let orbit = KeplerOrbit::from_state(
                    PointLy::new(position[0], position[1], position[2]),
                    GalacticVelocity::new(rotate(
                        [vr * cos - vt * sin, vr * sin + vt * cos, 0.0],
                        angles,
                    )),
                    GravitationalParameter::new(mu),
                )
                .unwrap();
                let sqrt_mu = orbit.mu.sqrt();
                let sigma = dot(orbit.position, orbit.velocity) / sqrt_mu;
                for years in [
                    -1e4, -3e3, -1e3, -300.0, -100.0, -1.0, 1.0, 100.0, 300.0, 1e3, 3e3, 1e4,
                ] {
                    let dt = years * YEAR;
                    let target = orbit.reduced_time(dt) * sqrt_mu;
                    let iterated = orbit.solve(target, sigma, UNIVERSAL_ITERATIONS);
                    if !orbit.solves(iterated, target, sigma) {
                        fallbacks += 1;
                    }
                    let chi = orbit.universal_anomaly(target, sigma, UNIVERSAL_ITERATIONS);
                    assert!(
                        orbit.solves(chi, target, sigma),
                        "unsolved after {years} yr: {orbit:?}"
                    );
                    let later = orbit.propagate(Seconds::new(dt));
                    let expected = if orbit.alpha < 0.0 && e > 1.0 {
                        hyperbolic_reference(&orbit, dt)
                    } else {
                        parabolic_reference(&orbit, dt)
                    };
                    let scale = length(expected).max(orbit.radius);
                    let off = distance(later.position, expected) / scale;
                    worst = worst.max(off);
                    count += 1;
                    assert!(
                        off < 1e-9,
                        "{off:e} off after {years} yr at e = {e}, q = {q} ly, {out} q out: {orbit:?}"
                    );
                }
            }
        }
        println!(
            "{count} propagations, {fallbacks} by the fallback, worst {worst:e} of the radius"
        );
        assert!(fallbacks > 0, "the sweep never reached the fallback");
        assert!(count > 1_000, "{count} propagations");
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
