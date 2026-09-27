//! The orbit integrator of the displaced form table (plan 15, P15.T6.a): a fixed-step
//! kick-drift-kick leapfrog in the sim's own tabulated potential, with an optional rotating
//! quadrupole bar in the standard rotating-frame splitting.
//!
//! # The potential
//!
//! The axisymmetric part is plan 02's [`PotentialTables`] with its (R, |z|) grid, whose
//! [`force`](PotentialTables::force) is the exact gradient of the bicubic Hermite interpolant of
//! [`potential`](PotentialTables::potential) (ruling 101.4 of 2026-09-22), so that the field is
//! conservative and the leapfrog holds the energy. Bilinear interpolation of tabulated forces,
//! which the plan first named, is not a gradient, and drifts.
//!
//! The bar exists only here: the sim's tables are axisymmetric (plan 15, Risks). It is Dehnen's
//! (2000, AJ 119, 800, eq. 1) quadrupole in the three-dimensional form of Monari et al. (2016,
//! MNRAS 461, 3835, eq. 2), `Φ_b = A U(r ÷ R_b) (x′² − y′²) ÷ r²` in the bar's frame, the bar
//! along x′, with `U(s) = −s⁻³` outside the bar's radius `R_b`. Inside, Dehnen's `s³ − 2` leaves
//! `Φ_b` direction-dependent at the centre, a force that grows as `1 ÷ r` there and throws the
//! orbits that pass through it; HYPERION takes `U(s) = 5s³ − 6s²`, the cubic that meets the
//! outer law with its value and slope at `s = 1` and falls as `s²` at the centre, where `Φ_b`
//! becomes the smooth `−6A (x′² − y′²) ÷ R_b²` of a finite bar. `A` is the manifest's `bar_strength` times the
//! circular speed squared at `R_b`; the bar rotates at the pattern speed `Ω_p` of the sim's
//! corotation ratio.
//!
//! # The splitting
//!
//! In the frame rotating with the bar the Hamiltonian is `H = ½|p|² + Φ(x) − Ω_p (x p_y − y
//! p_x)`, with `p` the inertial velocity along the rotating axes, and the Jacobi integral `H` is
//! conserved. The kinetic term and the rotation term commute (the kinetic energy does not change
//! under a rotation), so their joint flow is exact: a drift `D` of the position by `p`, then a
//! rotation `R` of position and momentum together by `−Ω_p t` about z. Each step is the symmetric
//! composition `K(h ÷ 2) R(h) D(h) K(h ÷ 2)`, with `K` the kick of the momentum by `−∇Φ`: a
//! Strang splitting, symplectic and second order, with one force per step, which carries the
//! Coriolis and centrifugal terms exactly without solving for a velocity-dependent force. Without a bar `R` is the identity and the step is the plain
//! kick-drift-kick leapfrog. At the epoch the rotating axes are the galaxy's, so a state read then
//! is in the galactic frame.
//!
//! # The step
//!
//! Each orbit takes a fixed step, `step_fraction` (1 ÷ 200) of the circular period at its
//! pericentre, estimated from its energy and angular momentum in the plane's potential, and never
//! below `min_step` years: a step set from the pericentre keeps the leapfrog symplectic, where one
//! that followed the orbit's radius would not. With the bar the angular momentum is not
//! conserved, and an inner orbit the bar torques onto a plunging one can pass the black hole far
//! inside the pericentre its step was set for (one of 1,000 test orbits at 200–4,000 ly lost 79%
//! of its Jacobi integral). So the run integrates each orbit through
//! [`integrate_checked`](OrbitPotential::integrate_checked): an orbit whose Jacobi integral has
//! drifted by more than the manifest's `drift_tolerance` is integrated again from its start at
//! half the step, up to `max_halvings` times (10⁻³ and three in the production manifest; T6.a's
//! tests hold 10⁻⁴ with six). The pass it keeps is a pure function of the orbit.
//!
//! Units: light-years, years, light-years per year; potentials in (km/s)².

use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use hyperion_sim::galaxy::potential::PotentialTables;
use hyperion_sim::math;
use hyperion_sim::units::LightYears;

/// Light-years per year per km/s, squared: (ly/yr)² per (km/s)².
const C2: f64 = LIGHT_YEARS_PER_YEAR_PER_KM_S * LIGHT_YEARS_PER_YEAR_PER_KM_S;

/// Bisection steps of the pericentre's estimate.
const PERICENTRE_STEPS: u32 = 60;

/// A rotating quadrupole bar (module documentation, "The potential").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    /// `A`, (km/s)².
    amplitude: f64,
    /// `R_b`, ly.
    radius: f64,
    /// `Ω_p`, radians per year.
    pattern_speed: f64,
}

impl Bar {
    /// The bar of radius `radius` (ly) whose amplitude is `strength` times the circular speed
    /// squared at that radius in `tables`, rotating at `pattern_speed` (radians per year).
    #[must_use]
    pub fn new(tables: &PotentialTables, radius: f64, strength: f64, pattern_speed: f64) -> Self {
        Self {
            amplitude: strength * tables.v_circ_sq(LightYears::new(radius)),
            radius,
            pattern_speed,
        }
    }

    /// The pattern speed, radians per year.
    #[must_use]
    pub fn pattern_speed(&self) -> f64 {
        self.pattern_speed
    }

    /// `U(s)` and `U′(s)` (module documentation).
    fn radial(s: f64) -> (f64, f64) {
        if s < 1.0 {
            (s * s * (5.0 * s - 6.0), s * (15.0 * s - 12.0))
        } else {
            let inv = 1.0 / s;
            let inv3 = inv * inv * inv;
            (-inv3, 3.0 * inv3 * inv)
        }
    }

    /// `Φ_b` at `x` in the bar's frame, (km/s)².
    #[must_use]
    pub fn potential(&self, x: [f64; 3]) -> f64 {
        let r2 = x[0] * x[0] + x[1] * x[1] + x[2] * x[2];
        if r2 <= 0.0 {
            return 0.0;
        }
        let (u, _) = Self::radial(r2.sqrt() / self.radius);
        self.amplitude * u * (x[0] * x[0] - x[1] * x[1]) / r2
    }

    /// `∇Φ_b` at `x` in the bar's frame, (km/s)² per ly.
    #[must_use]
    pub fn gradient(&self, x: [f64; 3]) -> [f64; 3] {
        let r2 = x[0] * x[0] + x[1] * x[1] + x[2] * x[2];
        if r2 <= 0.0 {
            return [0.0; 3];
        }
        let r = r2.sqrt();
        let (u, du) = Self::radial(r / self.radius);
        let q = x[0] * x[0] - x[1] * x[1];
        // ∂/∂xᵢ of U(r ÷ R_b) q ÷ r²: U′ xᵢ q ÷ (R_b r³) + U (∂q ÷ ∂xᵢ ÷ r² − 2 q xᵢ ÷ r⁴).
        let radial = du * q / (self.radius * r * r2) - 2.0 * u * q / (r2 * r2);
        let dq = [2.0 * x[0], -2.0 * x[1], 0.0];
        core::array::from_fn(|i| self.amplitude * (radial * x[i] + u * dq[i] / r2))
    }
}

/// The potential an orbit moves in: the sim's axisymmetric tables, and a rotating bar or none.
#[derive(Debug, Clone, PartialEq)]
pub struct OrbitPotential {
    tables: PotentialTables,
    bar: Option<Bar>,
}

/// The state of an orbit in the frame of its potential (the bar's, which at the epoch is the
/// galaxy's): position, ly, and inertial velocity along the frame's axes, ly/yr.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitState {
    /// The position, ly.
    pub x: [f64; 3],
    /// The inertial velocity, ly/yr.
    pub v: [f64; 3],
}

/// An orbit integrated by [`OrbitPotential::integrate_checked`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Checked {
    /// The state at the end.
    pub end: OrbitState,
    /// The leapfrog steps of every pass.
    pub steps: f64,
    /// How many times the step was halved.
    pub halvings: u32,
    /// The last pass's relative drift of the Jacobi integral.
    pub drift: f64,
}

/// The (R, |z|) grid is missing from the tables an orbit is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an orbit's potential needs the tables' (R, z) grid (`PotentialTables::full`)")]
pub struct MissingGridError;

impl OrbitPotential {
    /// The potential of `tables`, which must hold the (R, |z|) grid, with `bar`.
    ///
    /// # Errors
    ///
    /// [`MissingGridError`] if the tables were built without the grid.
    pub fn new(tables: PotentialTables, bar: Option<Bar>) -> Result<Self, MissingGridError> {
        if tables.has_grid() {
            Ok(Self { tables, bar })
        } else {
            Err(MissingGridError)
        }
    }

    /// The axisymmetric tables.
    #[must_use]
    pub fn tables(&self) -> &PotentialTables {
        &self.tables
    }

    /// The bar, if any.
    #[must_use]
    pub fn bar(&self) -> Option<&Bar> {
        self.bar.as_ref()
    }

    /// The frame's rotation, radians per year: the bar's pattern speed, or 0.
    #[must_use]
    pub fn frame_rotation(&self) -> f64 {
        self.bar.map_or(0.0, |b| b.pattern_speed)
    }

    /// The potential at `x`, (km/s)².
    ///
    /// # Panics
    ///
    /// Never: the tables' grid is checked by [`OrbitPotential::new`].
    #[must_use]
    pub fn potential(&self, x: [f64; 3]) -> f64 {
        let r = math::hypot(x[0], x[1]);
        let axisymmetric = self
            .tables
            .potential(LightYears::new(r), LightYears::new(x[2]))
            .expect("the grid is present by construction");
        axisymmetric + self.bar.map_or(0.0, |b| b.potential(x))
    }

    /// The acceleration `−∇Φ` at `at`, ly/yr².
    ///
    /// # Panics
    ///
    /// Never: the tables' grid is checked by [`OrbitPotential::new`].
    #[must_use]
    pub fn acceleration(&self, at: [f64; 3]) -> [f64; 3] {
        let radius = math::hypot(at[0], at[1]);
        let force = self
            .tables
            .force(LightYears::new(radius), LightYears::new(at[2]))
            .expect("the grid is present by construction");
        let (cos, sin) = if radius > 0.0 {
            (at[0] / radius, at[1] / radius)
        } else {
            (0.0, 0.0)
        };
        let mut gradient = [force.radial * cos, force.radial * sin, force.vertical];
        if let Some(bar) = &self.bar {
            for (g, b) in gradient.iter_mut().zip(bar.gradient(at)) {
                *g += b;
            }
        }
        gradient.map(|g| -g * C2)
    }

    /// The inertial energy `½|v|² + Φ` of `state`, (km/s)².
    #[must_use]
    pub fn energy(&self, state: &OrbitState) -> f64 {
        let v2 = state.v.iter().map(|v| v * v).sum::<f64>();
        0.5 * v2 / C2 + self.potential(state.x)
    }

    /// The Jacobi integral `½|p|² + Φ − Ω_p (x p_y − y p_x)` of `state`, (km/s)²: the energy
    /// without a bar.
    #[must_use]
    pub fn jacobi(&self, state: &OrbitState) -> f64 {
        let lz = state.x[0] * state.v[1] - state.x[1] * state.v[0];
        self.energy(state) - self.frame_rotation() * lz / C2
    }

    /// The step of an orbit from `state`: `fraction` of the circular period at its pericentre
    /// (module documentation, "The step"), at least `min_step`, years.
    #[must_use]
    pub fn step_for(&self, state: &OrbitState, fraction: f64, min_step: f64) -> f64 {
        let r0 = math::hypot(math::hypot(state.x[0], state.x[1]), state.x[2]).max(1.0);
        let l = {
            let [x, y, z] = state.x;
            let [vx, vy, vz] = state.v;
            let c = [y * vz - z * vy, z * vx - x * vz, x * vy - y * vx];
            math::hypot(math::hypot(c[0], c[1]), c[2])
        };
        let plane = |r: f64| self.tables.potential_in_plane(LightYears::new(r));
        let v2 = state.v.iter().map(|v| v * v).sum::<f64>() / C2;
        let energy = 0.5 * v2 + plane(r0);
        // The inner root of Φ(r) + L² ÷ 2r² = E, where the orbit turns: bisection from r₀ inward.
        let effective = |r: f64| plane(r) + 0.5 * l * l / (C2 * r * r) - energy;
        let floor = 1.0;
        let pericentre = if effective(floor) <= 0.0 {
            floor
        } else {
            let (mut lo, mut hi) = (floor, r0);
            for _ in 0..PERICENTRE_STEPS {
                let mid = f64::midpoint(lo, hi);
                if effective(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            hi
        };
        let at = pericentre.min(r0);
        let v_c = self.tables.v_circ(LightYears::new(at)).value() * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        let period = 2.0 * core::f64::consts::PI * at / v_c;
        (fraction * period).max(min_step)
    }

    /// `state` after `duration` years, integrated as [`integrate`](Self::integrate) at `step`
    /// and, while the Jacobi integral has drifted by more than `tolerance` relative, again from
    /// the start at half the step, at most `refinements` times (module documentation, "The
    /// step"). Returns the end state, the steps taken over every pass, and the passes' halvings.
    #[must_use]
    pub fn integrate_checked(
        &self,
        state: OrbitState,
        duration: f64,
        step: f64,
        tolerance: f64,
        refinements: u32,
    ) -> Checked {
        let start = self.jacobi(&state);
        let mut step = step;
        let mut steps = 0.0;
        let mut halvings = 0;
        loop {
            let end = self.integrate(state, duration, step);
            steps += (duration / step).ceil().max(1.0);
            let drift = ((self.jacobi(&end) - start) / start).abs();
            if drift <= tolerance || halvings == refinements {
                return Checked {
                    end,
                    steps,
                    halvings,
                    drift,
                };
            }
            step *= 0.5;
            halvings += 1;
        }
    }

    /// `state` after `duration` years in steps of about `step` years: the duration cut into the
    /// least whole number of equal steps no longer than `step` (module documentation, "The
    /// splitting").
    #[must_use]
    pub fn integrate(&self, state: OrbitState, duration: f64, step: f64) -> OrbitState {
        let steps = (duration / step).ceil().max(1.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole, positive number of steps below 2⁵³"
        )]
        let count = steps as u64;
        let step = duration / steps;
        let (sin, cos) = math::sin_cos(-self.frame_rotation() * step);
        let barred = self.bar.is_some();
        let mut state = state;
        let mut accel = self.acceleration(state.x);
        for _ in 0..count {
            for (v, a) in state.v.iter_mut().zip(accel) {
                *v += 0.5 * step * a;
            }
            for (x, v) in state.x.iter_mut().zip(state.v) {
                *x += step * v;
            }
            if barred {
                let [x, y, _] = state.x;
                state.x[0] = cos * x - sin * y;
                state.x[1] = sin * x + cos * y;
                let [vx, vy, _] = state.v;
                state.v[0] = cos * vx - sin * vy;
                state.v[1] = sin * vx + cos * vy;
            }
            accel = self.acceleration(state.x);
            for (v, a) in state.v.iter_mut().zip(accel) {
                *v += 0.5 * step * a;
            }
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use hyperion_sim::galaxy::params::GalaxyParams;
    use hyperion_sim::galaxy::potential::MassModel;

    use super::*;
    use crate::parallel::map_reduce_chunks;

    fn tables() -> PotentialTables {
        PotentialTables::full(&MassModel::new(&GalaxyParams::milky_way_like()))
    }

    /// The bar's gradient is its potential's, by central differences, inside and outside `R_b`.
    #[test]
    fn the_bars_gradient_is_its_potentials() {
        let bar = Bar {
            amplitude: 1_000.0,
            radius: 15_000.0,
            pattern_speed: 0.0,
        };
        for x in [
            [3_000.0, 1_000.0, 400.0],
            [20_000.0, -9_000.0, 2_000.0],
            [-4_000.0, 14_000.0, -800.0],
        ] {
            let g = bar.gradient(x);
            for i in 0..3 {
                let d = 0.5;
                let (mut hi, mut lo) = (x, x);
                hi[i] += d;
                lo[i] -= d;
                let fd = (bar.potential(hi) - bar.potential(lo)) / (2.0 * d);
                assert!(
                    (g[i] - fd).abs() < 1e-7 * g.iter().map(|v| v.abs()).fold(0.0, f64::max),
                    "{x:?} axis {i}: {} against {fd}",
                    g[i]
                );
            }
        }
    }

    /// A test orbit of index `i`, some eccentric, some tilted: every fourth in the nuclear disc,
    /// the bulge and the bar (200–4,000 ly), where the bar's core and most of the production run's
    /// steps are, the rest in the disc and the inner halo (4,000–40,000 ly).
    fn test_orbit(pot: &OrbitPotential, i: u64) -> OrbitState {
        let mut draws = super::super::births::Draws::new(0x6a, 0, i);
        let radius = if i.is_multiple_of(4) {
            200.0 * math::exp(draws.uniform() * math::ln(20.0))
        } else {
            4_000.0 + 36_000.0 * draws.uniform()
        };
        let azimuth = 2.0 * core::f64::consts::PI * draws.uniform();
        let height = 0.1 * radius.min(20_000.0) * (draws.uniform() - 0.5);
        let v_c =
            pot.tables().v_circ(LightYears::new(radius)).value() * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        let (sin, cos) = math::sin_cos(azimuth);
        let rotation = 0.6 + 0.5 * draws.uniform();
        let radial = 0.3 * (draws.uniform() - 0.5) * v_c;
        let vertical = 0.3 * (draws.uniform() - 0.5) * v_c;
        OrbitState {
            x: [radius * cos, radius * sin, height],
            v: [
                radial * cos - rotation * v_c * sin,
                radial * sin + rotation * v_c * cos,
                vertical,
            ],
        }
    }

    /// The largest relative change of the Jacobi integral over `n` test orbits of `years` each.
    fn worst_drift(pot: &OrbitPotential, n: u64, years: f64) -> f64 {
        let mut worst: f64 = 0.0;
        for i in 0..n {
            let start = test_orbit(pot, i);
            let step = pot.step_for(&start, 1.0 / 200.0, 1_000.0);
            let e0 = pot.jacobi(&start);
            let end = pot.integrate(start, years, step);
            worst = worst.max(((pot.jacobi(&end) - e0) / e0).abs());
        }
        worst
    }

    /// The largest drift over `n` test orbits integrated as the run does, and how many needed a
    /// smaller step.
    fn worst_checked_drift(pot: &OrbitPotential, n: u64, years: f64) -> (f64, u64) {
        let (mut worst, mut refined): (f64, u64) = (0.0, 0);
        for i in 0..n {
            let start = test_orbit(pot, i);
            let step = pot.step_for(&start, 1.0 / 200.0, 1_000.0);
            let checked = pot.integrate_checked(start, years, step, 1e-4, 6);
            worst = worst.max(checked.drift);
            refined += u64::from(checked.halvings > 0);
        }
        (worst, refined)
    }

    fn barred(tables: PotentialTables) -> OrbitPotential {
        let omega = tables.bar_pattern_speed().value();
        let radius = tables.bar_corotation().value() / 1.2;
        let bar = Bar::new(&tables, radius, 0.05, omega);
        OrbitPotential::new(tables, Some(bar)).unwrap()
    }

    /// P15.T6.a (fast sample): the energy, and the Jacobi integral with the bar, hold to 10⁻⁴
    /// relative over 10 Gyr for 20 test orbits.
    #[test]
    fn energy_and_jacobi_hold_over_ten_gyr() {
        let plain = OrbitPotential::new(tables(), None).unwrap();
        let drift = worst_drift(&plain, 20, 1e10);
        assert!(drift < 1e-4, "energy drift {drift}");
        let drift = worst_drift(&barred(tables()), 20, 1e10);
        assert!(drift < 1e-4, "Jacobi drift {drift}");
    }

    /// **P15.T6.a's acceptance** (slow): 1,000 test orbits, with and without the bar.
    #[test]
    #[ignore = "slow: 2,000 orbits of 10 Gyr"]
    fn energy_and_jacobi_hold_over_ten_gyr_for_a_thousand_orbits() {
        let plain = OrbitPotential::new(tables(), None).unwrap();
        let drift = worst_drift(&plain, 1_000, 1e10);
        eprintln!("worst energy drift {drift:e}");
        assert!(drift < 1e-4, "energy drift {drift}");
        let barred = barred(tables());
        let first = worst_drift(&barred, 1_000, 1e10);
        let (drift, refined) = worst_checked_drift(&barred, 1_000, 1e10);
        eprintln!(
            "worst Jacobi drift {first:e} at the first pass, {drift:e} kept; {refined} refined"
        );
        assert!(drift < 1e-4, "Jacobi drift {drift}");
        assert!(
            refined <= 20,
            "{refined} of 1,000 orbits needed a smaller step"
        );
    }

    /// P15.T6.a: a circular orbit in the plane stays circular to 10⁻³ in radius over 10 Gyr, at
    /// radii across the disc.
    #[test]
    fn a_circular_orbit_stays_circular() {
        let pot = OrbitPotential::new(tables(), None).unwrap();
        for r in [3_000.0, 10_000.0, 26_000.0, 50_000.0] {
            let v = pot.tables().v_circ(LightYears::new(r)).value() * LIGHT_YEARS_PER_YEAR_PER_KM_S;
            let mut s = OrbitState {
                x: [r, 0.0, 0.0],
                v: [0.0, v, 0.0],
            };
            let step = pot.step_for(&s, 1.0 / 200.0, 1_000.0);
            let mut worst: f64 = 0.0;
            for _ in 0..100 {
                s = pot.integrate(s, 1e8, step);
                worst = worst.max((math::hypot(s.x[0], s.x[1]) / r - 1.0).abs());
            }
            assert!(worst < 1e-3, "{r} ly: {worst}");
        }
    }

    /// P15.T6.a: the same orbits on one and on four threads give identical bytes.
    #[test]
    fn one_and_four_threads_give_the_same_bytes() {
        let pot = OrbitPotential::new(tables(), None).unwrap();
        let run = |threads: usize| {
            let mut values = Vec::new();
            map_reduce_chunks(
                16,
                3,
                NonZeroUsize::new(threads).unwrap(),
                |range| {
                    range
                        .map(|i| {
                            let start = test_orbit(&pot, i);
                            let step = pot.step_for(&start, 1.0 / 200.0, 1_000.0);
                            pot.integrate(start, 1e9, step)
                        })
                        .collect::<Vec<_>>()
                },
                |states| {
                    for s in states {
                        values.extend(s.x.iter().chain(&s.v).copied());
                    }
                },
            )
            .unwrap();
            values
        };
        let (one, four) = (run(1), run(4));
        assert_eq!(one.len(), four.len());
        assert!(one.iter().zip(&four).all(|(a, b)| a.total_cmp(b).is_eq()));
    }
}
