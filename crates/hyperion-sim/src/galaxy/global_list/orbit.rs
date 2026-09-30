//! The orbit integrator of the global list (plan 10, P10.T2): a fixed-step kick–drift–kick
//! leapfrog in the galaxy's tabulated axisymmetric potential.
//!
//! A stream's track is measured by integrating its progenitor and about 2,000 tracers (brainstorm,
//! "Streams and accreted structure": "integrated in the tabulated potential with a fixed-step
//! leapfrog and `libm`"), and every entry of the list takes its bounding shell from one integrated
//! orbit (P10.T4). Everything here is a pure function of the tables, the start and the step.
//!
//! # The force
//!
//! Inside the grid the force is plan 02's [`PotentialTables::force`], the exact gradient of the
//! bicubic Hermite interpolant that [`PotentialTables::potential`] reads, with the dark halo, the
//! nuclear cluster and the black hole in closed form (plan 08's addition, ruling 101.4 of
//! 2026-09-22). A field that is the gradient of one continuously differentiable potential is
//! conservative, so the leapfrog holds the energy [`Leapfrog::energy_j_kg`] to its own accuracy;
//! bilinear interpolation of tabulated forces, which plan 10's Design note 3 named when plan 02 had
//! no gradient, is not a gradient and drifts. Plan 15's displaced-form integrator steps in the same
//! force.
//!
//! The bar is left out (Design note 2): no progenitor with a tube comes inside corotation.
//!
//! # The far field
//!
//! Beyond the grid's edge at 2¹⁸ ly (80 kpc), where dwarfs and some orphans have their
//! apocentres, the tables continue the Gaussian components by scaling the edge's value as `1 ÷ h`
//! from the clamped point, and their force there is not the gradient of their potential: it
//! departs from the potential's central differences by up to 3 × 10⁻⁴ of itself outside the grid's
//! box, against 10⁻¹⁰ inside (measured on the Milky Way fixture, 2026-09-29). So the integrator
//! continues the galaxy on its own, as Design note 3 asks: the spherical components in closed
//! form (the NFW halo among them) and the Gaussian components, the stars and gas, as a point mass
//! plus the axisymmetric quadrupole of the discs, `A ÷ h + B P₂(z ÷ h) ÷ h³`, with `A` and `B`
//! matched to the tables' Gaussian part in the plane and on the axis at 2¹⁸ ly. Between spherical
//! radii of 2¹⁷·⁵ and 2¹⁸ ly the two potentials are blended by a smooth step in radius, and the
//! force is the blend's exact gradient, so the whole field is the gradient of one continuously
//! differentiable potential. Inside 100 ly no tube progenitor comes, and the tables serve there
//! as everywhere inside the blend.
//!
//! # The step
//!
//! Kick–drift–kick in Cartesian galactic coordinates about the galactic centre, in SI: metres,
//! metres per second, seconds. Each step is a half kick by the acceleration held in the state, a
//! drift, one force evaluation at the new position and the second half kick, so a step costs one
//! force and `n` calls of [`Leapfrog::step`] are bit for bit one call of
//! [`Leapfrog::integrate`] over `n` steps. The step is fixed per orbit, 1 ⁄ 64 of the pericentre
//! crossing time `τ_p = r_p ÷ v_p` and at most 2 Myr, a whole number of steps over the span
//! ([`FixedStep`]; ruling 146 of 2026-09-22, replacing Design note 4's 1 ⁄ 256 of the radial
//! period). A leapfrog's energy error goes as `(h ÷ τ_p)²`: what it must resolve is the pericentre
//! passage, where the field changes fastest, not the period, and a step set from the period left
//! the passage of an eccentric orbit only 10–20 steps. Adaptive or time-transformed steps are
//! rejected: every body of a stream shares the progenitor's time grid, for releases on step
//! boundaries and for landing exactly on the present. A negative step integrates backwards; the
//! scheme is time-symmetric, so reversing the velocity and stepping again retraces the orbit to
//! rounding.
//!
//! # Determinism
//!
//! The integrator uses `+ − × ÷`, `sqrt` and [`math`] only, and the force path the same (P10.T2.c:
//! a test reads this module's source and the force's and fails on any platform float method,
//! `libm` called directly or `mul_add`, so that no local `#[expect]` can hide one). Its bits are
//! pinned by a golden file, which every architecture the sim's tests run on must reproduce.

use std::error::Error;
use std::fmt;

use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::potential::{GRID_EDGE_LY, PotentialTables};
use crate::math;
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR, SECONDS_PER_MEGAYEAR};
use crate::units::{LightYears, Metres, MetresPerSecond, PerYear, Seconds};

/// Steps per pericentre crossing time `τ_p = r_p ÷ v_p` (ruling 146 of 2026-09-22), a parameter of
/// the generator version.
///
/// At 64 the energy of every pinned orbit on the Milky Way fixture, e 0.08–0.9, holds to 6 × 10⁻⁵
/// over ten radial periods, against a bound of 2 × 10⁻⁴ (`tests/global_list_orbit.rs`); a toy Milky
/// Way gives 4 × 10⁻⁵ to 2.6 × 10⁻⁴ over pericentres of 5–20 kpc and e 0.2–0.95 (research
/// `r-int10`). That is 340 steps a radial period for the least eccentric pinned orbit and more for
/// the rest, so Design note 4's 1 ⁄ 256 of the period would not bind and is dropped.
pub const STEPS_PER_PERICENTRE_CROSSING: u32 = 64;

/// The longest step, 2 Myr in seconds (Design note 4), a parameter of the generator version.
pub const MAX_STEP: Seconds = Seconds::new(2.0 * SECONDS_PER_MEGAYEAR);

/// (km/s)² in (m/s)².
const SQUARED_KM_S_IN_SI: f64 = 1e6;

/// A gradient in (km/s)² per light-year in metres per second squared.
const FORCE_IN_SI: f64 = SQUARED_KM_S_IN_SI / METRES_PER_LIGHT_YEAR;

/// Where the blend from the tables to the far field begins, 2¹⁷·⁵ ly (module documentation, "The
/// far field"); it ends at the grid's edge, 2¹⁸ ly.
const BLEND_START_LY: f64 = GRID_EDGE_LY * core::f64::consts::FRAC_1_SQRT_2;

/// A [`Leapfrog`] or a [`FixedStep`] could not be made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildLeapfrogError {
    /// The tables are in the plane only: an orbit needs the (R, |z|) grid of
    /// [`Galaxy::with_full_potential`](crate::galaxy::Galaxy::with_full_potential).
    NoVerticalGrid,
    /// The step, or a pericentre crossing time or span a step was asked for, is zero, negative
    /// where it must be positive, not finite, or needs more than `u32::MAX` steps.
    InvalidStep(Seconds),
}

impl fmt::Display for BuildLeapfrogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoVerticalGrid => f.write_str("the potential tables have no (R, z) grid"),
            Self::InvalidStep(step) => {
                write!(f, "no fixed step can be made from {} s", step.value())
            }
        }
    }
}

impl Error for BuildLeapfrogError {}

/// A fixed step over a span: a whole number of steps, each at most 1 ⁄ 64 of the pericentre
/// crossing time and at most 2 Myr (module documentation, "The step").
///
/// The crossing time `τ_p = r_p ÷ v_p` comes from the progenitor's spec, its pericentre and its
/// speed there, so the step depends on the spec alone.
///
/// # Examples
///
/// A progenitor like Pal 5's, with its pericentre at 7.5 kpc passed at 300 km/s, stripped over the
/// last 3 Gyr, is integrated backwards in 7,855 steps of 0.38 Myr:
///
/// ```
/// use hyperion_sim::galaxy::global_list::orbit::FixedStep;
/// use hyperion_sim::units::{
///     Gigayears, KilometresPerSecond, Kiloparsecs, Megayears, Metres, MetresPerSecond, Seconds,
/// };
///
/// let pericentre = Metres::from(Kiloparsecs::new(7.5));
/// let speed = MetresPerSecond::from(KilometresPerSecond::new(300.0));
/// let span = -Seconds::from(Gigayears::new(3.0));
/// let fixed = FixedStep::new(pericentre, speed, span)?;
/// assert_eq!(fixed.count(), 7_855);
/// assert!(fixed.step().value() < 0.0);
/// let crossing = Megayears::from(Seconds::new(pericentre.value() / speed.value())).value();
/// assert!(Megayears::from(fixed.step().abs()).value() <= crossing / 64.0);
/// # Ok::<(), hyperion_sim::galaxy::global_list::orbit::BuildLeapfrogError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedStep {
    step: Seconds,
    count: u32,
}

impl FixedStep {
    /// The step over `span` (negative to integrate backwards) for an orbit whose pericentre
    /// `pericentre` is passed at speed `pericentre_speed`: the longest step, `min(τ_p ÷ 64, 2
    /// Myr)` with `τ_p = r_p ÷ v_p`, then shortened so that a whole number of steps spans `span`
    /// exactly. A zero span takes no step, at the longest step.
    ///
    /// # Errors
    ///
    /// [`BuildLeapfrogError::InvalidStep`] for a crossing time that is not finite and positive
    /// (carrying it), a span that is not finite, or a span that needs more than `u32::MAX`
    /// steps.
    pub fn new(
        pericentre: Metres,
        pericentre_speed: MetresPerSecond,
        span: Seconds,
    ) -> Result<Self, BuildLeapfrogError> {
        let crossing = pericentre.value() / pericentre_speed.value();
        if !(crossing.is_finite() && crossing > 0.0) {
            return Err(BuildLeapfrogError::InvalidStep(Seconds::new(crossing)));
        }
        if !span.value().is_finite() {
            return Err(BuildLeapfrogError::InvalidStep(span));
        }
        let longest = (crossing / f64::from(STEPS_PER_PERICENTRE_CROSSING)).min(MAX_STEP.value());
        let steps = (span.value().abs() / longest).ceil();
        // Also refuses a NaN, from a subnormal crossing time that makes `longest` zero.
        if steps.is_nan() || steps > f64::from(u32::MAX) {
            return Err(BuildLeapfrogError::InvalidStep(span));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number of 0 to u32::MAX, NaN excluded, checked above"
        )]
        let count = steps as u32;
        let step = if count == 0 {
            longest
        } else {
            span.value() / steps
        };
        Ok(Self {
            step: Seconds::new(step),
            count,
        })
    }

    /// The step, negative for a backward span.
    #[must_use]
    pub fn step(&self) -> Seconds {
        self.step
    }

    /// The number of steps over the span.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }
}

/// A body's state on its orbit: position about the galactic centre and velocity in the galactic
/// frame, in SI, with the acceleration at the position.
///
/// The acceleration is the field's at the position, computed once when the state is made or
/// stepped, so that each step costs one force evaluation. A state belongs to the [`Leapfrog`] (the
/// potential tables) that made it; its step may change freely, since the acceleration depends on
/// the position alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitState {
    position: [f64; 3],
    velocity: [f64; 3],
    acceleration: [f64; 3],
}

impl OrbitState {
    /// The position about the galactic centre, m.
    #[must_use]
    pub fn position(&self) -> GalacticDisplacement {
        GalacticDisplacement::new(self.position)
    }

    /// The velocity in the galactic frame, m/s.
    #[must_use]
    pub fn velocity(&self) -> GalacticVelocity {
        GalacticVelocity::new(self.velocity)
    }

    /// The acceleration at the position, m/s², along the galactic axes.
    #[must_use]
    pub fn acceleration_m_s2(&self) -> [f64; 3] {
        self.acceleration
    }

    /// The distance from the galactic centre.
    #[must_use]
    pub fn radius(&self) -> Metres {
        Metres::new(dot(self.position, self.position).sqrt())
    }

    /// The angular momentum about the galaxy's z axis per unit mass, `x v_y − y v_x`, m² s⁻¹:
    /// conserved in the axisymmetric potential.
    #[must_use]
    pub fn angular_momentum_z_m2_s(&self) -> f64 {
        self.position[0] * self.velocity[1] - self.position[1] * self.velocity[0]
    }

    /// The same state with its velocity reversed: stepping it retraces the orbit backwards.
    #[must_use]
    pub fn reversed(&self) -> Self {
        Self {
            velocity: self.velocity.map(|v| -v),
            ..*self
        }
    }
}

/// What an integrated orbit did, from its radial turning points (plan 10, P10.T2.b).
///
/// Turning points are where the distance from the centre stops falling (a pericentre) or rising
/// (an apocentre) between steps, each refined by the parabola through the three samples about
/// it. A near-circular orbit's turning points are its epicycles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitSummary {
    pericentre: Option<Metres>,
    apocentre: Option<Metres>,
    radial_period: Option<Seconds>,
    mean_angular_frequency: PerYear,
    bounding_radii: [Metres; 2],
    turning_points: u32,
}

impl OrbitSummary {
    /// The smallest distance at a pericentre, if the orbit passed one.
    #[must_use]
    pub fn pericentre(&self) -> Option<Metres> {
        self.pericentre
    }

    /// The largest distance at an apocentre, if the orbit passed one.
    #[must_use]
    pub fn apocentre(&self) -> Option<Metres> {
        self.apocentre
    }

    /// The radial period, the time from pericentre to pericentre, as the mean interval between the
    /// turning points of the spherical radius: `2 (t_last − t_first) ÷ (n − 1)` for `n ≥ 2`
    /// turning points. In a flattened potential r(t) also carries the vertical oscillation, so this
    /// is the mean period of r(t)'s turns rather than strictly the radial period; if anything it
    /// reads short, which only shortens a step set from it. `None` for fewer than two.
    #[must_use]
    pub fn radial_period(&self) -> Option<Seconds> {
        self.radial_period
    }

    /// The mean angular frequency Ω̄: the angle swept about the centre, step by step in the
    /// orbit's instantaneous plane, over the time taken. Zero for an orbit of no steps.
    #[must_use]
    pub fn mean_angular_frequency(&self) -> PerYear {
        self.mean_angular_frequency
    }

    /// The least and greatest distance from the centre over the whole integration, its ends and
    /// its refined turning points included: the radii between which the body stayed.
    #[must_use]
    pub fn bounding_radii(&self) -> [Metres; 2] {
        self.bounding_radii
    }

    /// The number of radial turning points passed.
    #[must_use]
    pub fn turning_points(&self) -> u32 {
        self.turning_points
    }
}

/// A fixed-step kick–drift–kick leapfrog in one galaxy's potential tables (module
/// documentation).
///
/// # Examples
///
/// An orbit from the Sun's neighbourhood, integrated for half a gigayear (not run: the (R, z)
/// grid takes seconds to build):
///
/// ```no_run
/// use hyperion_sim::coords::{GalacticDisplacement, GalacticVelocity};
/// use hyperion_sim::galaxy::global_list::orbit::Leapfrog;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
/// use hyperion_sim::units::{LightYears, Megayears, Metres, Seconds};
///
/// let tables = PotentialTables::full(&MassModel::new(&GalaxyParams::milky_way_like()));
/// let leapfrog = Leapfrog::new(&tables, Seconds::from(Megayears::new(0.5)))?;
/// let x = Metres::from(LightYears::new(26_000.0)).value();
/// let mut state = leapfrog.state(
///     GalacticDisplacement::new([x, 0.0, 0.0]),
///     GalacticVelocity::new([0.0, 200e3, 30e3]),
/// );
/// let summary = leapfrog.integrate(&mut state, 1_000);
/// let [inner, outer] = summary.bounding_radii().map(|r| LightYears::from(r).value());
/// assert!(inner < 26_000.0 && 26_000.0 < outer);
/// assert!(summary.radial_period().is_some());
/// # Ok::<(), hyperion_sim::galaxy::global_list::orbit::BuildLeapfrogError>(())
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Leapfrog<'a> {
    tables: &'a PotentialTables,
    far: FarField,
    step: Seconds,
}

impl<'a> Leapfrog<'a> {
    /// A leapfrog of fixed step `step` (negative to integrate backwards) in `tables`.
    ///
    /// # Errors
    ///
    /// [`BuildLeapfrogError::NoVerticalGrid`] for tables without the (R, |z|) grid;
    /// [`BuildLeapfrogError::InvalidStep`] for a step that is zero or not finite.
    pub fn new(tables: &'a PotentialTables, step: Seconds) -> Result<Self, BuildLeapfrogError> {
        if !tables.has_grid() {
            return Err(BuildLeapfrogError::NoVerticalGrid);
        }
        if !(step.value().is_finite() && step.value().abs() > 0.0) {
            return Err(BuildLeapfrogError::InvalidStep(step));
        }
        Ok(Self {
            tables,
            far: FarField::new(tables),
            step,
        })
    }

    /// The step.
    #[must_use]
    pub fn time_step(&self) -> Seconds {
        self.step
    }

    /// This leapfrog with another step, as [`new`](Self::new) checks it.
    ///
    /// # Errors
    ///
    /// [`BuildLeapfrogError::InvalidStep`] for a step that is zero or not finite.
    pub fn with_step(&self, step: Seconds) -> Result<Self, BuildLeapfrogError> {
        if !(step.value().is_finite() && step.value().abs() > 0.0) {
            return Err(BuildLeapfrogError::InvalidStep(step));
        }
        Ok(Self { step, ..*self })
    }

    /// The state at `position` (about the galactic centre) moving at `velocity`, with the
    /// acceleration there.
    #[must_use]
    pub fn state(&self, position: GalacticDisplacement, velocity: GalacticVelocity) -> OrbitState {
        let position = position.metres();
        OrbitState {
            position,
            velocity: velocity.metres_per_second(),
            acceleration: self.acceleration(position),
        }
    }

    /// The acceleration `−∇Φ` at `position` (about the galactic centre), m/s² along the galactic
    /// axes: the gradient of [`potential_j_kg`](Self::potential_j_kg) (module documentation, "The
    /// far field"). On the z axis the horizontal part is zero.
    #[must_use]
    pub fn acceleration_m_s2(&self, position: &GalacticDisplacement) -> [f64; 3] {
        self.acceleration(position.metres())
    }

    /// The potential at `position` (about the galactic centre), J/kg, zero at infinity: the
    /// tables' inside 2¹⁷·⁵ ly, the far field's beyond 2¹⁸ ly and a smooth blend of the two between
    /// (module documentation, "The far field").
    #[must_use]
    pub fn potential_j_kg(&self, position: &GalacticDisplacement) -> f64 {
        self.potential(position.metres())
    }

    /// The energy per unit mass of `state`, `½ |v|² + Φ`, J/kg: conserved to the step's accuracy.
    #[must_use]
    pub fn energy_j_kg(&self, state: &OrbitState) -> f64 {
        0.5 * dot(state.velocity, state.velocity) + self.potential(state.position)
    }

    /// [`acceleration_m_s2`](Self::acceleration_m_s2) at `position`, m.
    fn acceleration(&self, position: [f64; 3]) -> [f64; 3] {
        let [x, y, _] = position;
        let gradient = self.gradient(position.map(|c| c / METRES_PER_LIGHT_YEAR));
        // One factor for both horizontal axes keeps the torque about z zero to rounding.
        let horizontal = -gradient.horizontal * FORCE_IN_SI / METRES_PER_LIGHT_YEAR;
        [
            horizontal * x,
            horizontal * y,
            -gradient.vertical * FORCE_IN_SI,
        ]
    }

    /// [`potential_j_kg`](Self::potential_j_kg) at `position`, m.
    fn potential(&self, position: [f64; 3]) -> f64 {
        let p = position.map(|c| c / METRES_PER_LIGHT_YEAR);
        let h = dot(p, p).sqrt();
        let potential = if h <= BLEND_START_LY {
            self.table_potential(p)
        } else if h >= GRID_EDGE_LY {
            self.far.potential(self.tables, p, h)
        } else {
            let (w, _) = blend(h);
            let (table, far) = (
                self.table_potential(p),
                self.far.potential(self.tables, p, h),
            );
            w * table + (1.0 - w) * far
        };
        potential * SQUARED_KM_S_IN_SI
    }

    /// The tables' potential at `p` (ly), (km/s)².
    fn table_potential(&self, p: [f64; 3]) -> f64 {
        self.tables
            .potential(
                LightYears::new((p[0] * p[0] + p[1] * p[1]).sqrt()),
                LightYears::new(p[2]),
            )
            .expect("a leapfrog is only built from tables with the (R, z) grid")
    }

    /// The tables' gradient at `p` (ly).
    fn table_gradient(&self, p: [f64; 3]) -> Gradient {
        let r_cyl = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let force = self
            .tables
            .force(LightYears::new(r_cyl), LightYears::new(p[2]))
            .expect("a leapfrog is only built from tables with the (R, z) grid");
        Gradient {
            horizontal: if r_cyl > 0.0 {
                force.radial / r_cyl
            } else {
                0.0
            },
            vertical: force.vertical,
        }
    }

    /// The gradient of the potential at `p` (ly), blended as the potential is.
    fn gradient(&self, p: [f64; 3]) -> Gradient {
        let h = dot(p, p).sqrt();
        if h <= BLEND_START_LY {
            return self.table_gradient(p);
        }
        if h >= GRID_EDGE_LY {
            return self.far.gradient(self.tables, p, h);
        }
        let (w, slope) = blend(h);
        let (table, far) = (self.table_gradient(p), self.far.gradient(self.tables, p, h));
        // ∇(w Φ_t + (1 − w) Φ_f) = w ∇Φ_t + (1 − w) ∇Φ_f + (Φ_t − Φ_f) w′(h) p ÷ h.
        let difference = self.table_potential(p) - self.far.potential(self.tables, p, h);
        let radial = difference * slope / h;
        Gradient {
            horizontal: w * table.horizontal + (1.0 - w) * far.horizontal + radial,
            vertical: w * table.vertical + (1.0 - w) * far.vertical + radial * p[2],
        }
    }

    /// One kick–drift–kick step of `state`.
    pub fn step(&self, state: &mut OrbitState) {
        let h = self.step.value();
        let half = 0.5 * h;
        for (v, a) in state.velocity.iter_mut().zip(state.acceleration) {
            *v += a * half;
        }
        for (x, v) in state.position.iter_mut().zip(state.velocity) {
            *x += v * h;
        }
        state.acceleration = self.acceleration(state.position);
        for (v, a) in state.velocity.iter_mut().zip(state.acceleration) {
            *v += a * half;
        }
    }

    /// `steps` steps of `state`, which is left at the end, and a summary of the orbit from its
    /// radial turning points ([`OrbitSummary`]).
    pub fn integrate(&self, state: &mut OrbitState, steps: u32) -> OrbitSummary {
        let mut turns = Turns::new(dot(state.position, state.position));
        let mut angle = 0.0;
        for _ in 0..steps {
            let before = state.position;
            self.step(state);
            let swept = cross(before, state.position);
            angle += math::atan2(dot(swept, swept).sqrt(), dot(before, state.position));
            turns.push(dot(state.position, state.position));
        }
        let duration = f64::from(steps) * self.step.value().abs();
        let frequency = if steps == 0 {
            0.0
        } else {
            angle / duration * SECONDS_PER_JULIAN_YEAR
        };
        turns.summary(self.step.value().abs(), PerYear::new(frequency))
    }
}

/// A potential's gradient at a point `p` (ly) of an axisymmetric field: `∂Φ ÷ ∂x = horizontal ×
/// x`, `∂Φ ÷ ∂y = horizontal × y` and `∂Φ ÷ ∂z = vertical`, with `horizontal = (∂Φ ÷ ∂R) ÷ R` in
/// (km/s)² per ly² and `vertical` in (km/s)² per ly.
#[derive(Debug, Clone, Copy)]
struct Gradient {
    horizontal: f64,
    vertical: f64,
}

/// The blend's weight on the tables at spherical radius `h` (ly) inside the blend shell, and its
/// derivative in `h`: `w = 1 − (3t² − 2t³)` with `t` running from 0 at 2¹⁷·⁵ ly to 1 at 2¹⁸ ly, so
/// that the potential and the force are continuous at both ends.
fn blend(h: f64) -> (f64, f64) {
    let width = GRID_EDGE_LY - BLEND_START_LY;
    let t = (h - BLEND_START_LY) / width;
    (1.0 - t * t * (3.0 - 2.0 * t), -6.0 * t * (1.0 - t) / width)
}

/// The galaxy beyond the tables' grid (module documentation, "The far field"): the spherical
/// components in closed form, and the Gaussian components as a monopole and an axisymmetric
/// quadrupole, `A ÷ h + B P₂(z ÷ h) ÷ h³`, matched to the tables' Gaussian part in the plane and
/// on the axis at 2¹⁸ ly.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FarField {
    /// `A`, (km/s)² ly.
    monopole: f64,
    /// `B`, (km/s)² ly³.
    quadrupole: f64,
}

impl FarField {
    fn new(tables: &PotentialTables) -> Self {
        let edge = GRID_EDGE_LY;
        let [spherical, _] = tables.spherical_at(edge);
        let at = |r_cyl: f64, z: f64| {
            tables
                .potential(LightYears::new(r_cyl), LightYears::new(z))
                .expect("a leapfrog is only built from tables with the (R, z) grid")
                - spherical
        };
        // In the plane P₂ = −½, on the axis 1.
        let (plane, axis) = (at(edge, 0.0), at(0.0, edge));
        Self {
            monopole: (axis + 2.0 * plane) / 3.0 * edge,
            quadrupole: (axis - plane) * 2.0 / 3.0 * edge * edge * edge,
        }
    }

    /// The potential at `p` (ly), `h = |p|`, (km/s)².
    fn potential(&self, tables: &PotentialTables, p: [f64; 3], h: f64) -> f64 {
        let [spherical, _] = tables.spherical_at(h);
        let h2 = h * h;
        let shape = 3.0 * p[2] * p[2] - h2;
        spherical + self.monopole / h + 0.5 * self.quadrupole * shape / (h2 * h2 * h)
    }

    /// The gradient at `p` (ly), `h = |p|`.
    fn gradient(&self, tables: &PotentialTables, p: [f64; 3], h: f64) -> Gradient {
        let [_, v_circ_sq] = tables.spherical_at(h);
        let h2 = h * h;
        let h5 = h2 * h2 * h;
        let shape = 3.0 * p[2] * p[2] - h2;
        // ∂ ÷ ∂xᵢ of B (3z² − h²) ÷ (2h⁵) is B ÷ 2 × [(6z δᵢz − 2xᵢ) ÷ h⁵ − 5 (3z² − h²) xᵢ ÷ h⁷].
        let radial = v_circ_sq / h2 - self.monopole / (h2 * h)
            + 0.5 * self.quadrupole * (-2.0 / h5 - 5.0 * shape / (h5 * h2));
        Gradient {
            horizontal: radial,
            vertical: radial * p[2] + 3.0 * self.quadrupole * p[2] / h5,
        }
    }
}

/// The radial turning points of an orbit, from the squared distance at each step.
#[derive(Debug, Clone, Copy)]
struct Turns {
    /// The last three squared distances, oldest first; `samples` of them are filled.
    last: [f64; 3],
    samples: u64,
    least: f64,
    greatest: f64,
    pericentre: Option<f64>,
    apocentre: Option<f64>,
    /// The first and last turning point's time, in steps from the start.
    first: f64,
    latest: f64,
    count: u32,
}

impl Turns {
    fn new(start: f64) -> Self {
        Self {
            last: [0.0, 0.0, start],
            samples: 1,
            least: start,
            greatest: start,
            pericentre: None,
            apocentre: None,
            first: 0.0,
            latest: 0.0,
            count: 0,
        }
    }

    fn push(&mut self, r2: f64) {
        self.last = [self.last[1], self.last[2], r2];
        self.samples += 1;
        self.least = self.least.min(r2);
        self.greatest = self.greatest.max(r2);
        if self.samples < 3 {
            return;
        }
        let [s0, s1, s2] = self.last;
        let (falling, rising) = (s1 - s0, s2 - s1);
        let is_pericentre = falling < 0.0 && rising >= 0.0;
        let is_apocentre = falling > 0.0 && rising <= 0.0;
        if !(is_pericentre || is_apocentre) {
            return;
        }
        // The parabola through the three samples: its vertex lies `offset` steps from the middle
        // one, which is sample `samples − 2` counted from 0 at the start. The curvature is
        // `rising − falling`, which is not zero at a strict turn.
        let curvature = rising - falling;
        let offset = (s0 - s2) / (2.0 * curvature);
        let vertex = s1 - (s0 - s2) * (s0 - s2) / (8.0 * curvature);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a step count of at most u32::MAX + 1, exact in f64"
        )]
        let at = (self.samples - 2) as f64 + offset;
        if self.count == 0 {
            self.first = at;
        }
        self.latest = at;
        self.count += 1;
        if is_pericentre {
            self.least = self.least.min(vertex);
            self.pericentre = Some(self.pericentre.map_or(vertex, |p| p.min(vertex)));
        } else {
            self.greatest = self.greatest.max(vertex);
            self.apocentre = Some(self.apocentre.map_or(vertex, |a| a.max(vertex)));
        }
    }

    fn summary(&self, step: f64, mean_angular_frequency: PerYear) -> OrbitSummary {
        let radius = |r2: f64| Metres::new(r2.max(0.0).sqrt());
        let radial_period = (self.count >= 2).then(|| {
            Seconds::new(2.0 * (self.latest - self.first) * step / f64::from(self.count - 1))
        });
        OrbitSummary {
            pericentre: self.pericentre.map(radius),
            apocentre: self.apocentre.map(radius),
            radial_period,
            mean_angular_frequency,
            bounding_radii: [radius(self.least), radius(self.greatest)],
            turning_points: self.count,
        }
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::potential::MassModel;
    use crate::units::{Gigayears, Megayears};

    /// P10.T2.c: the integrator's source and the force path's use `+ − × ÷`, `sqrt` and `math`
    /// only. Clippy bans the platform's float maths, but a local `#[expect]` would hide a call from
    /// it; this reads the sources themselves. Each file is read up to its test module.
    #[test]
    fn the_integrator_and_its_force_call_no_platform_float_maths() {
        // Clippy's disallowed float methods (`clippy.toml`), without the byte conversions, which
        // are about hashing, not arithmetic.
        let banned = [
            "sin", "cos", "tan", "sin_cos", "asin", "acos", "atan", "atan2", "sinh", "cosh",
            "tanh", "asinh", "acosh", "atanh", "exp", "exp2", "exp_m1", "ln", "log", "log2",
            "log10", "ln_1p", "powf", "powi", "cbrt", "hypot", "mul_add", "to_bits",
        ];
        let sources = [
            ("global_list/orbit.rs", include_str!("orbit.rs")),
            ("potential/force.rs", include_str!("../potential/force.rs")),
            (
                "potential/tables.rs",
                include_str!("../potential/tables.rs"),
            ),
            ("potential/nfw.rs", include_str!("../potential/nfw.rs")),
            (
                "potential/spherical.rs",
                include_str!("../potential/spherical.rs"),
            ),
        ];
        for (name, source) in sources {
            let code = source.split_once("#[cfg(test)]\nmod tests").map_or_else(
                || panic!("{name} has no test module to stop at"),
                |(code, _)| code,
            );
            assert!(code.len() > 1_000, "{name} was not read");
            for method in banned {
                for call in [
                    format!(".{method}("),
                    format!("f64::{method}("),
                    format!("f32::{method}("),
                ] {
                    assert!(!code.contains(&call), "{name} calls `{call}`");
                }
            }
            assert!(!code.contains("libm::"), "{name} calls `libm` directly");
            // `mul_add` in any form, `math::mul_add` included: it is exact only where the
            // hardware fuses, and the integrator has no use for it.
            assert!(
                !code.contains("mul_add("),
                "{name} calls a fused multiply-add"
            );
        }
    }

    #[test]
    fn a_leapfrog_needs_the_vertical_grid() {
        let model = MassModel::new(&GalaxyParams::milky_way_like());
        let in_plane = PotentialTables::in_plane(&model);
        let step = Seconds::from(Megayears::new(1.0));
        assert_eq!(
            Leapfrog::new(&in_plane, step).map(|_| ()),
            Err(BuildLeapfrogError::NoVerticalGrid)
        );
    }

    #[test]
    fn the_fixed_step_is_a_whole_number_of_steps_over_the_span() {
        // A pericentre of 24,000 ly passed at 300 km/s: τ_p = 23.98 Myr, at most 0.375 Myr a step.
        let pericentre = Metres::from(LightYears::new(24_000.0));
        let speed = MetresPerSecond::new(3e5);
        let crossing = pericentre.value() / speed.value();
        let span = -Seconds::from(Gigayears::new(2.0));
        let fixed = FixedStep::new(pericentre, speed, span).unwrap();
        let longest = crossing / 64.0;
        assert_eq!(fixed.count(), 5_338);
        assert!(fixed.step().value() < 0.0 && fixed.step().value().abs() <= longest);
        assert!(fixed.step().value().abs() > 0.999 * longest);
        let covered = f64::from(fixed.count()) * fixed.step().value();
        assert!((covered / span.value() - 1.0).abs() < 1e-15);
        // A slow crossing is held to 2 Myr: 200,000 ly at 100 km/s is 600 Myr.
        let far = Metres::from(LightYears::new(200_000.0));
        let slow = FixedStep::new(far, MetresPerSecond::new(1e5), -span).unwrap();
        assert_eq!(slow.count(), 1_000);
        assert!((slow.step().value() / MAX_STEP.value() - 1.0).abs() < 1e-15);
        // No span, no steps.
        let none = FixedStep::new(pericentre, speed, Seconds::ZERO).unwrap();
        assert_eq!(none.count(), 0);
        assert!((none.step().value() / longest - 1.0).abs() < 1e-15);
        // A crossing time that is zero, negative or not finite is refused, carrying it.
        for (r, v, bad) in [
            (0.0, 3e5, 0.0),
            (-1.0, 1.0, -1.0),
            (1.0, 0.0, f64::INFINITY),
            (f64::INFINITY, 3e5, f64::INFINITY),
        ] {
            assert_eq!(
                FixedStep::new(Metres::new(r), MetresPerSecond::new(v), span),
                Err(BuildLeapfrogError::InvalidStep(Seconds::new(bad)))
            );
        }
        for (r, v) in [(f64::NAN, 3e5), (0.0, 0.0), (1.0, f64::NAN)] {
            assert!(matches!(
                FixedStep::new(Metres::new(r), MetresPerSecond::new(v), span),
                Err(BuildLeapfrogError::InvalidStep(_))
            ));
        }
        assert!(matches!(
            FixedStep::new(pericentre, speed, Seconds::new(f64::NAN)),
            Err(BuildLeapfrogError::InvalidStep(_))
        ));
        // A subnormal crossing time rounds the longest step to zero: refused, not a zero step.
        assert!(matches!(
            FixedStep::new(
                Metres::new(1e-322),
                MetresPerSecond::new(1.0),
                Seconds::ZERO
            ),
            Err(BuildLeapfrogError::InvalidStep(_))
        ));
        assert_eq!(
            FixedStep::new(
                Metres::new(64.0),
                MetresPerSecond::new(1.0),
                Seconds::new(1e300)
            ),
            Err(BuildLeapfrogError::InvalidStep(Seconds::new(1e300)))
        );
    }

    /// The far field's fitted monopole, `−A ÷ G`, is the mass of the Gaussian components that
    /// `GalaxyParams` gives, the stars of the discs, bar and bulge and the gas disc, to 1 %
    /// (ruling 146.4 of 2026-09-22). The fit reads the tables' edge, not the parameters (module
    /// documentation, "The far field"), so this restores Design note 3's point mass as a check,
    /// and a bad table edge would fail it.
    #[test]
    #[ignore = "slow: builds the (R, z) grid"]
    fn the_far_fields_monopole_is_the_gaussian_components_mass() {
        use crate::galaxy::Population;
        use crate::galaxy::consts::G;

        let params = GalaxyParams::milky_way_like();
        let model = MassModel::new(&params);
        let tables = PotentialTables::full(&model);
        let fitted = -FarField::new(&tables).monopole / G;
        let stars = [
            Population::YoungThinDisc,
            Population::OldThinDisc,
            Population::ThickDisc,
            Population::Bulge,
            Population::LongBar,
            Population::NuclearDisc,
        ]
        .into_iter()
        .fold(0.0, |sum, p| sum + params.population_mass(p).value());
        let expected = stars + params.gas_disc().mass().value();
        let expanded = model.extended_mass();
        eprintln!(
            "monopole {fitted:.5e} M☉, parameters {expected:.5e} M☉, Gaussians {expanded:.5e} M☉"
        );
        assert!(
            (fitted / expected - 1.0).abs() < 0.01,
            "{fitted:e} against {expected:e} M☉"
        );
    }

    #[test]
    fn turning_points_are_refined_by_the_parabola() {
        // r² = 1 + cos(θ) sampled every 0.1 rad: pericentres (r² = 0) at θ = π, 3π, apocentres
        // (r² = 2) at 2π; the parabola puts them within the sampling's error.
        let mut turns = Turns::new(2.0);
        for i in 1..=100_u32 {
            turns.push(1.0 + math::cos(0.1 * f64::from(i)));
        }
        let summary = turns.summary(0.1, PerYear::ZERO);
        assert_eq!(summary.turning_points(), 3);
        let period = summary.radial_period().unwrap().value();
        assert!(
            (period / (2.0 * core::f64::consts::PI) - 1.0).abs() < 1e-3,
            "{period}"
        );
        let apocentre = summary.apocentre().unwrap().value();
        assert!((apocentre / 2.0_f64.sqrt() - 1.0).abs() < 1e-3);
        let [inner, outer] = summary.bounding_radii().map(Metres::value);
        assert!(inner < 0.05 && (outer / 2.0_f64.sqrt() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        for message in [
            BuildLeapfrogError::NoVerticalGrid.to_string(),
            BuildLeapfrogError::InvalidStep(Seconds::ZERO).to_string(),
        ] {
            assert!(!message.starts_with(char::is_uppercase), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
    }
}
