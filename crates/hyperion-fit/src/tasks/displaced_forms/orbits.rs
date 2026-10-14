//! The orbit integrator of the displaced form table (plan 15, P15.T6.a, with P15.T6.g's
//! time-transformed leapfrog): orbits in the sim's own tabulated potential, with an optional
//! rotating quadrupole bar, each step a fixed fraction of the local circular period.
//!
//! # The potential
//!
//! The axisymmetric part is plan 02's [`PotentialTables`] with its (R, |z|) grid, whose
//! [`force`](PotentialTables::force) is the exact gradient of the bicubic Hermite interpolant of
//! [`potential`](PotentialTables::potential) (ruling 101.4 of 2026-09-22), so that the field is
//! conservative and the leapfrog holds the energy. Bilinear interpolation of tabulated forces,
//! which the plan first named, is not a gradient, and drifts. Nothing is softened: the central
//! black hole is the tables' point mass and the nuclear cluster's cusp is the potential's own.
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
//! # The step
//!
//! In the frame rotating with the bar the Hamiltonian is `H = ½|p|² + Φ(x) − Ω_p (x p_y − y
//! p_x)`, with `p` the inertial velocity along the rotating axes, and the Jacobi integral `H` is
//! conserved. The kinetic term and the rotation term commute (the kinetic energy does not change
//! under a rotation), so their joint flow is exact: a drift of the position by `p`, then a
//! rotation of position and momentum together by `−Ω_p t` about z. Without a bar the rotation is
//! the identity. At the epoch the rotating axes are the galaxy's, so a state read then is in the
//! galactic frame.
//!
//! The steps are those of Mikkola and Aarseth's (2002, Celest. Mech. Dyn. Astron. 84, 343)
//! time-transformed leapfrog (P15.T6.g), with the time transformation `dt = ds ÷ Ω(r)`: `Ω` is the
//! circular frequency `v_c(r) ÷ r` of the plane's tables at the spherical radius `r = |x|`
//! ([`CircularFrequency`]), so that every step is `fraction` of the local circular period. The
//! step therefore scales as `r^1.5` where the black hole dominates, which gives a fixed number of
//! steps per pericentre passage at any depth, as `r^0.65` where a γ = 1.3 cusp's own mass
//! dominates and as `r` on the flat rotation curve. In the Milky Way fixture the black hole
//! outweighs the cusp's enclosed mass out to about 7 ly, inside the cluster's 10 ly break, so
//! there the exponent runs from 1.5 down to about 1. A step of fictitious length `h = 2π × fraction` is `K(h ÷ 2) D(h)
//! K(h ÷ 2)`: each kick adds `−∇Φ δt` to the momentum, with `δt = (h ÷ 2) ÷ Ω(r)` at the current
//! position, and adds `δt Ω′(r) (x · (p + p′) ÷ 2) ÷ r` to the auxiliary `W`, `p` and `p′` being
//! the momentum before and after the kick; the drift is the exact drift-and-rotation above over
//! the physical time `h ÷ W`. `W`, which carries `Ω` along the orbit so that the kicks and the
//! drift see consistent steps, starts at `Ω(r₀)`; its rate `Ω′ ṙ` needs only `x · p`, because the
//! rotation term is perpendicular to `x`. The step that would pass the orbit's duration is
//! replaced by one plain kick-drift-kick step of the time left, so that the orbit ends at its
//! duration exactly. A step costs one force, one table read and one rotation.
//!
//! The method is explicit and time-symmetric, not symplectic, and needs no step floor. It
//! replaces T6.a's fixed step, set from the orbit's pericentre, which cannot resolve an orbit the
//! bar throws onto a plunging, chaotic path into the black hole's radius of influence and the
//! cusp (Gerhard and Binney 1985, MNRAS 216, 467): from version 16's potential on, two of
//! T6.a's 1,000 barred test orbits never converged under the halving below
//! (decision-p15-t6g-orbits, 2026-10-12, which also measured why the logarithmic-Hamiltonian
//! leapfrog, Bulirsch–Stoer and a fixed step per orbit were not chosen).
//!
//! # The check, capture and unresolved orbits
//!
//! The run integrates each orbit through
//! [`integrate_checked`](OrbitPotential::integrate_checked): an orbit whose Jacobi integral has
//! drifted by more than the manifest's `drift_tolerance` is integrated again from its start at
//! half the step fraction, up to `max_halvings` times (ruling 140.7, the check, no longer the
//! method: 10⁻³ and three in the production manifest; T6.a's tests hold 10⁻⁴ with six). The pass
//! it keeps is a pure function of the orbit; one still over the tolerance after the last halving
//! is unresolved, which the run records and does not classify.
//!
//! Every drift is measured on the scale `max(|J₀|, |Φ(x₀)|)` ([`OrbitPotential::drift`];
//! decision-p15-t6g-orbits §2), `Φ(x₀)` the full potential, bar included, at the start. An energy
//! error moves a bound orbit by `δ ln r ≈ δJ ÷ v_c²` and a free one by `δr ÷ r ≈ δJ ÷ 2T`, neither
//! of which involves `J` itself, so dividing by `|J₀|` alone measured a cancellation for the
//! marginally bound and unbound. For a bound orbit the scale is `|Φ₀|`; in the fixture's
//! `galaxy_potential` golden at version 21, `|Φ₀| ÷ v_c²` is about 3 at the Sun's 26,000 ly, 9 at
//! 3,260 ly, 14 at 1,000 ly and 30 at 100 ly, so 10⁻³ bounds `δ ln r` by 0.003–0.03, under a
//! seventh of a 0.20-wide cell (the decision's "3–9, about 0.009" leaves out the inner galaxy). For
//! a fast kick the scale is `|J₀| ≈ T₀`; in the bar's frame the prograde term can make `|J₀|` the
//! larger, and the scale keeps it.
//!
//! Within [`CAPTURE_RADIUS_LY`] of the centre, where the black hole holds about 96% of the
//! enclosed mass, an orbit whose specific angular momentum `|x × p|` is below `4GM• ÷ c` falls into
//! the black hole at its next pericentre: the marginally bound orbit, at `4GM ÷ c²`, is the least
//! pericentre of every parabolic orbit about a Schwarzschild hole, and it has `L = 4GM ÷ c`
//! (Bardeen, Press and Teukolsky 1972, ApJ 178, 347, eqs. 2.13 and 2.19; Shapiro and Teukolsky
//! 1983, *Black Holes, White Dwarfs and Neutron Stars*, §12.4, the capture cross-section). The
//! rule is checked at the start and after every step but the plain one of the time left, and a
//! captured orbit's pass stops there. No other relativistic term enters: the first post-Newtonian
//! corrections are of order `GM• ÷ (c² r)`, under 10⁻³ outside 10⁻³ ly, and the histograms begin
//! at 16 ly. There is no tidal disruption: the table's objects are compact
//! remnants, and even a white dwarf's tidal radius about Sgr A* lies inside the horizon.
//!
//! # Determinism
//!
//! Every operation is IEEE-754 arithmetic, `sqrt` or [`hyperion_sim::math`] (`ln`, `exp`,
//! `sin_cos`, `hypot`), never `mul_add` or a platform transcendental, and the steps depend only on
//! the orbit's state, never on a clock or a thread, so an orbit is a pure function of its start,
//! its duration and its step fraction, the same on every platform.
//!
//! Units: light-years, years, light-years per year; potentials in (km/s)².

use hyperion_sim::galaxy::consts::{G, LIGHT_YEARS_PER_YEAR_PER_KM_S};
use hyperion_sim::galaxy::potential::PotentialTables;
use hyperion_sim::math;
use hyperion_sim::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR, SPEED_OF_LIGHT};
use hyperion_sim::units::{LightYears, SolarMasses};

/// Light-years per year per km/s, squared: (ly/yr)² per (km/s)².
const C2: f64 = LIGHT_YEARS_PER_YEAR_PER_KM_S * LIGHT_YEARS_PER_YEAR_PER_KM_S;

/// The speed of light in light-years per Julian year: 1, since the IAU defines the light-year as
/// the distance light travels in a Julian year, computed from the SI constants so that the units
/// cannot drift apart.
const LIGHT_SPEED: f64 = SPEED_OF_LIGHT * SECONDS_PER_JULIAN_YEAR / METRES_PER_LIGHT_YEAR;

/// The radius within which an orbit may be captured by the black hole, ly (P15.T6.g): inside it
/// the black hole holds about 96% of the enclosed mass (the fixture's cusp puts about 1.5 × 10⁵ M☉
/// inside 1 ly against the black hole's 4.3 × 10⁶ M☉), so the osculating angular momentum decides
/// correctly, and an inbound orbit is checked dozens of times on its way in.
pub const CAPTURE_RADIUS_LY: f64 = 1.0;

/// ln r (r in ly) at the circular-frequency table's first knot: ln 10⁻⁶.
const FREQUENCY_LN_FIRST: f64 = -13.815_510_557_964_274;

/// ln r at its last knot: ln (4 × 10⁶).
const FREQUENCY_LN_LAST: f64 = 15.201_804_919_084_164;

/// Its knots: 511 intervals over 12.6 decades, 40.5 a decade.
const FREQUENCY_KNOTS: u32 = 512;

/// The logarithmic slope of Ω inside the first knot: Kepler's `r^−3/2` about the black hole.
const INNER_SLOPE: f64 = -1.5;

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

/// The circular frequency `Ω(r) = v_c(r) ÷ r` of the plane's tables at a spherical radius, and its
/// derivative, which set the time-transformed leapfrog's steps (module documentation, "The
/// step").
///
/// They are read from a table of ln Ω against ln r, built once per potential from
/// [`PotentialTables::v_circ_sq`] and [`PotentialTables::v_circ_sq_derivative`]: a cubic Hermite
/// interpolant with the exact slopes at 512 knots evenly spaced in ln r from 10⁻⁶ ly to
/// 4 × 10⁶ ly (40.5 a decade), continued as Kepler's `r^−3/2` inside the first knot and with the
/// last knot's slope outside the last. `Ω′` is the interpolant's own derivative, so `W` follows the
/// interpolated `Ω` exactly along the orbit.
#[derive(Debug, Clone, PartialEq)]
pub struct CircularFrequency {
    /// The knots' spacing in ln r.
    spacing: f64,
    /// ln Ω (Ω in radians per year) and d ln Ω ÷ d ln r at each knot.
    knots: Vec<[f64; 2]>,
}

impl CircularFrequency {
    /// The table of `tables`.
    #[must_use]
    pub fn new(tables: &PotentialTables) -> Self {
        let spacing = (FREQUENCY_LN_LAST - FREQUENCY_LN_FIRST) / f64::from(FREQUENCY_KNOTS - 1);
        let knots = (0..FREQUENCY_KNOTS)
            .map(|k| {
                let r = math::exp(FREQUENCY_LN_FIRST + spacing * f64::from(k));
                let (omega, derivative) = Self::direct(tables, r);
                [math::ln(omega), r * derivative / omega]
            })
            .collect();
        Self { spacing, knots }
    }

    /// `Ω` and `dΩ ÷ dr` at `r_ly > 0` ly straight from `tables`, radians per year and radians
    /// per year per ly.
    #[must_use]
    pub fn direct(tables: &PotentialTables, r_ly: f64) -> (f64, f64) {
        let radius = LightYears::new(r_ly);
        let v2 = tables.v_circ_sq(radius);
        let dv2 = tables.v_circ_sq_derivative(radius);
        let v = v2.sqrt();
        (
            (v / r_ly) * LIGHT_YEARS_PER_YEAR_PER_KM_S,
            ((dv2 / (2.0 * v)) / r_ly - v / (r_ly * r_ly)) * LIGHT_YEARS_PER_YEAR_PER_KM_S,
        )
    }

    /// `Ω` and `dΩ ÷ dr` at `r_ly > 0` ly from the table, radians per year and radians per year
    /// per ly.
    #[must_use]
    pub fn at(&self, r_ly: f64) -> (f64, f64) {
        let ln_r = math::ln(r_ly);
        let last = self.knots.len() - 1;
        let (ln_omega, slope) = if ln_r <= FREQUENCY_LN_FIRST {
            let [y0, _] = self.knots[0];
            (y0 + INNER_SLOPE * (ln_r - FREQUENCY_LN_FIRST), INNER_SLOPE)
        } else if ln_r >= FREQUENCY_LN_LAST {
            let [y0, d0] = self.knots[last];
            (y0 + d0 * (ln_r - FREQUENCY_LN_LAST), d0)
        } else {
            let place = (ln_r - FREQUENCY_LN_FIRST) / self.spacing;
            let cell = place.floor().min(f64::from(FREQUENCY_KNOTS - 2));
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole number from 0 to the knot count less two"
            )]
            let knot = cell as usize;
            let t = place - cell;
            let [y0, d0] = self.knots[knot];
            let [y1, d1] = self.knots[knot + 1];
            let spacing = self.spacing;
            let (t2, t3) = (t * t, t * t * t);
            let value = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
                + (t3 - 2.0 * t2 + t) * spacing * d0
                + (-2.0 * t3 + 3.0 * t2) * y1
                + (t3 - t2) * spacing * d1;
            let derivative = ((6.0 * t2 - 6.0 * t) * y0 + (-6.0 * t2 + 6.0 * t) * y1) / spacing
                + (3.0 * t2 - 4.0 * t + 1.0) * d0
                + (3.0 * t2 - 2.0 * t) * d1;
            (value, derivative)
        };
        let omega = math::exp(ln_omega);
        (omega, omega * slope / r_ly)
    }
}

/// The potential an orbit moves in: the sim's axisymmetric tables, and a rotating bar or none.
#[derive(Debug, Clone, PartialEq)]
pub struct OrbitPotential {
    tables: PotentialTables,
    bar: Option<Bar>,
    frequency: CircularFrequency,
    /// `4GM• ÷ c`, ly²/yr: the angular momentum below which an orbit inside
    /// [`CAPTURE_RADIUS_LY`] is captured.
    capture_momentum_ly2_per_year: f64,
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

/// How a pass of [`OrbitPotential::integrate`] ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ending {
    /// At its duration.
    Aged,
    /// Captured by the black hole (module documentation, "The check, capture and unresolved
    /// orbits"), `after_years` from the start.
    Captured {
        /// The time from the start, years.
        after_years: f64,
    },
    /// At a step that was not positive (its fictitious time over a `W` that is not positive, or
    /// not a number), or at once for a duration that is not finite: never seen in a run, and
    /// guarded so that a pass always ends. Its drift counts as infinite.
    Broken,
}

/// One pass of [`OrbitPotential::integrate`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pass {
    /// The state where the pass ended.
    pub end: OrbitState,
    /// The steps taken.
    pub steps: u64,
    /// How it ended.
    pub ending: Ending,
    /// The closest approach to the centre at the start and after each step, ly.
    pub closest_ly: f64,
}

/// An orbit integrated by [`OrbitPotential::integrate_checked`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Checked {
    /// The state at the end of the pass kept.
    pub end: OrbitState,
    /// The steps of every pass.
    pub steps: u64,
    /// How many times the step fraction was halved.
    pub halvings: u32,
    /// The pass kept's drift of the Jacobi integral on its scale ([`OrbitPotential::drift`]);
    /// infinite if it broke off.
    pub drift: f64,
    /// The first pass's, likewise.
    pub first_drift: f64,
    /// How the pass kept ended.
    pub ending: Ending,
    /// The pass kept's closest approach to the centre, ly.
    pub closest_ly: f64,
}

/// The (R, |z|) grid is missing from the tables an orbit is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an orbit's potential needs the tables' (R, z) grid (`PotentialTables::full`)")]
pub struct MissingGridError;

/// `a · b`.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `|x|`.
#[must_use]
fn norm(x: [f64; 3]) -> f64 {
    math::hypot(math::hypot(x[0], x[1]), x[2])
}

/// `|x × v|`.
#[must_use]
fn angular_momentum(x: [f64; 3], v: [f64; 3]) -> f64 {
    norm([
        x[1] * v[2] - x[2] * v[1],
        x[2] * v[0] - x[0] * v[2],
        x[0] * v[1] - x[1] * v[0],
    ])
}

impl OrbitPotential {
    /// The potential of `tables`, which must hold the (R, |z|) grid, with `bar`; `black_hole` is
    /// the tables' central black hole's mass (the model's
    /// [`MassModel::black_hole`](hyperion_sim::galaxy::potential::MassModel::black_hole)), which
    /// sets the capture rule.
    ///
    /// # Errors
    ///
    /// [`MissingGridError`] if the tables were built without the grid.
    pub fn new(
        tables: PotentialTables,
        black_hole: SolarMasses,
        bar: Option<Bar>,
    ) -> Result<Self, MissingGridError> {
        if !tables.has_grid() {
            return Err(MissingGridError);
        }
        let gm = G * black_hole.value() * C2;
        Ok(Self {
            frequency: CircularFrequency::new(&tables),
            tables,
            bar,
            capture_momentum_ly2_per_year: 4.0 * gm / LIGHT_SPEED,
        })
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

    /// `4GM• ÷ c`, ly²/yr: the specific angular momentum below which an orbit inside
    /// [`CAPTURE_RADIUS_LY`] is captured.
    #[must_use]
    pub fn capture_momentum_ly2_per_year(&self) -> f64 {
        self.capture_momentum_ly2_per_year
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

    /// Whether an orbit at `x` (ly, `r_ly = |x|`, which the caller holds) with velocity `v` (ly/yr)
    /// is captured: inside [`CAPTURE_RADIUS_LY`] with `|x × v|` below
    /// [`capture_momentum_ly2_per_year`](Self::capture_momentum_ly2_per_year).
    #[must_use]
    fn is_captured(&self, x: [f64; 3], v: [f64; 3], r_ly: f64) -> bool {
        r_ly < CAPTURE_RADIUS_LY && angular_momentum(x, v) < self.capture_momentum_ly2_per_year
    }

    /// The frame's rotation over `dt` years applied to the position `x` and the momentum `v`.
    fn rotate(&self, dt: f64, x: &mut [f64; 3], v: &mut [f64; 3]) {
        if let Some(bar) = &self.bar {
            let (sin, cos) = math::sin_cos(-bar.pattern_speed * dt);
            let [x0, y0, _] = *x;
            x[0] = cos * x0 - sin * y0;
            x[1] = sin * x0 + cos * y0;
            let [vx, vy, _] = *v;
            v[0] = cos * vx - sin * vy;
            v[1] = sin * vx + cos * vy;
        }
    }

    /// `state` after `duration_years` in steps of `fraction` of the local circular period, or
    /// until it is captured (module documentation, "The step"). A duration that is not finite
    /// ends the pass `Broken` at once, as a step that is not positive does later.
    #[must_use]
    pub fn integrate(&self, state: OrbitState, duration_years: f64, fraction: f64) -> Pass {
        // The fictitious step, `h` in the module documentation, and the auxiliary `W`.
        let ds = core::f64::consts::TAU * fraction;
        let (mut position, mut momentum) = (state.x, state.v);
        let mut r = norm(position);
        let mut closest_ly = r;
        let mut steps = 0_u64;
        let pass = |position, momentum, steps, ending, closest_ly| Pass {
            end: OrbitState {
                x: position,
                v: momentum,
            },
            steps,
            ending,
            closest_ly,
        };
        if !duration_years.is_finite() {
            return pass(position, momentum, steps, Ending::Broken, r);
        }
        if self.is_captured(position, momentum, r) {
            let ending = Ending::Captured { after_years: 0.0 };
            return pass(position, momentum, steps, ending, r);
        }
        let (mut omega, mut slope) = self.frequency.at(r);
        let mut aux = omega;
        let mut accel = self.acceleration(position);
        let mut t = 0.0;
        loop {
            let half = 0.5 * ds / omega;
            let kicked: [f64; 3] = core::array::from_fn(|i| momentum[i] + half * accel[i]);
            let aux_half =
                aux + half * slope * 0.5 * (dot(position, momentum) + dot(position, kicked)) / r;
            let dt = ds / aux_half;
            // A step that is not positive would never end the pass. One shorter than half the
            // clock's last digit (under 5 × 10⁻⁷ yr late in a 10 Gyr run, at a pericentre near the
            // capture rule's 5 × 10⁻⁶ ly) loses its time to rounding, which is harmless, and the
            // orbit leaves such a pericentre within a few steps.
            if dt.is_nan() || dt <= 0.0 {
                return pass(position, momentum, steps, Ending::Broken, closest_ly);
            }
            if t + dt >= duration_years {
                // The time left as one plain kick-drift-kick step, so the orbit ends at its age.
                let left = duration_years - t;
                if left > 0.0 {
                    for (m, g) in momentum.iter_mut().zip(accel) {
                        *m += 0.5 * left * g;
                    }
                    for (q, m) in position.iter_mut().zip(momentum) {
                        *q += left * m;
                    }
                    self.rotate(left, &mut position, &mut momentum);
                    accel = self.acceleration(position);
                    for (m, g) in momentum.iter_mut().zip(accel) {
                        *m += 0.5 * left * g;
                    }
                    steps += 1;
                    closest_ly = closest_ly.min(norm(position));
                }
                return pass(position, momentum, steps, Ending::Aged, closest_ly);
            }
            let mut drifted = kicked;
            for (q, m) in position.iter_mut().zip(drifted) {
                *q += dt * m;
            }
            self.rotate(dt, &mut position, &mut drifted);
            t += dt;
            r = norm(position);
            (omega, slope) = self.frequency.at(r);
            accel = self.acceleration(position);
            let half = 0.5 * ds / omega;
            let next: [f64; 3] = core::array::from_fn(|i| drifted[i] + half * accel[i]);
            aux =
                aux_half + half * slope * 0.5 * (dot(position, drifted) + dot(position, next)) / r;
            momentum = next;
            steps += 1;
            closest_ly = closest_ly.min(r);
            if self.is_captured(position, momentum, r) {
                let ending = Ending::Captured { after_years: t };
                return pass(position, momentum, steps, ending, closest_ly);
            }
        }
    }

    /// The scale an orbit's drift from `start` is measured on, `max(|J₀|, |Φ(x₀)|)`, (km/s)²
    /// (module documentation, "The check, capture and unresolved orbits").
    #[must_use]
    pub fn drift_scale(&self, start: &OrbitState) -> f64 {
        self.jacobi(start).abs().max(self.potential(start.x).abs())
    }

    /// The drift of the Jacobi integral from `start` to `end` on its scale, `|J − J₀| ÷
    /// max(|J₀|, |Φ(x₀)|)`: infinite rather than not a number.
    #[must_use]
    pub fn drift(&self, start: &OrbitState, end: &OrbitState) -> f64 {
        Self::drift_on(
            self.jacobi(start),
            self.drift_scale(start),
            self.jacobi(end),
        )
    }

    /// `|jacobi − start| ÷ scale`, infinite rather than not a number.
    fn drift_on(start: f64, scale: f64, jacobi: f64) -> f64 {
        let drift = ((jacobi - start) / scale).abs();
        if drift.is_nan() { f64::INFINITY } else { drift }
    }

    /// `state` after `duration_years`, integrated as [`integrate`](Self::integrate) at `fraction`
    /// and, while the Jacobi integral has drifted by more than `tolerance` on its scale
    /// ([`drift`](Self::drift)), again from the start at half the fraction, at most `refinements`
    /// times (module documentation, "The check, capture and unresolved orbits"). The drift of a
    /// captured pass is its drift at the capture, so a capture is kept only from a resolved
    /// integration.
    #[must_use]
    pub fn integrate_checked(
        &self,
        state: OrbitState,
        duration_years: f64,
        fraction: f64,
        tolerance: f64,
        refinements: u32,
    ) -> Checked {
        let start = self.jacobi(&state);
        let scale = self.drift_scale(&state);
        let mut fraction = fraction;
        let mut steps = 0_u64;
        let mut halvings = 0;
        let mut first_drift = None;
        loop {
            let pass = self.integrate(state, duration_years, fraction);
            steps += pass.steps;
            let drift = match pass.ending {
                Ending::Broken => f64::INFINITY,
                Ending::Aged | Ending::Captured { .. } => {
                    Self::drift_on(start, scale, self.jacobi(&pass.end))
                }
            };
            let first_drift = *first_drift.get_or_insert(drift);
            if drift <= tolerance || halvings == refinements {
                return Checked {
                    end: pass.end,
                    steps,
                    halvings,
                    drift,
                    first_drift,
                    ending: pass.ending,
                    closest_ly: pass.closest_ly,
                };
            }
            fraction *= 0.5;
            halvings += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use hyperion_sim::galaxy::params::GalaxyParams;
    use hyperion_sim::galaxy::potential::MassModel;

    use super::*;
    use crate::parallel::map_reduce_chunks;

    /// The test potentials' step fraction, as the production manifest's: 1 ÷ 200.
    const FRACTION: f64 = 1.0 / 200.0;

    fn model() -> MassModel {
        MassModel::new(&GalaxyParams::milky_way_like())
    }

    fn plain() -> OrbitPotential {
        let model = model();
        OrbitPotential::new(
            PotentialTables::full(&model),
            model.black_hole().mass(),
            None,
        )
        .unwrap()
    }

    fn barred() -> OrbitPotential {
        let model = model();
        let tables = PotentialTables::full(&model);
        let omega = tables.bar_pattern_speed().value();
        let radius = tables.bar_corotation().value() / 1.2;
        let bar = Bar::new(&tables, radius, 0.05, omega);
        OrbitPotential::new(tables, model.black_hole().mass(), Some(bar)).unwrap()
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

    /// P15.T6.g: the circular-frequency table holds the tables' Ω between its knots, its `Ω′` is
    /// its own `Ω`'s derivative (so `W` follows the interpolant), it continues as Kepler's
    /// `r^−3/2` inside its first knot, and its end knots are ln 10⁻⁶ and ln (4 × 10⁶).
    #[test]
    fn the_circular_frequency_follows_the_tables() {
        assert!((FREQUENCY_LN_FIRST - math::ln(1e-6)).abs() < 1e-14);
        assert!((FREQUENCY_LN_LAST - math::ln(4e6)).abs() < 1e-14);
        let model = model();
        let tables = PotentialTables::in_plane(&model);
        let frequency = CircularFrequency::new(&tables);
        let mut r = 1.3e-6;
        while r < 3e6 {
            let (table, table_slope) = frequency.at(r);
            let (direct, _) = CircularFrequency::direct(&tables, r);
            assert!(
                (table / direct - 1.0).abs() < 1e-4,
                "{r} ly: {table} against {direct}"
            );
            let d = 1e-6 * r;
            let fd = (frequency.at(r + d).0 - frequency.at(r - d).0) / (2.0 * d);
            assert!(
                (table_slope / fd - 1.0).abs() < 1e-5,
                "{r} ly: Ω′ {table_slope} against {fd}"
            );
            r *= 1.37;
        }
        let (inner, inner_slope) = frequency.at(1e-8);
        let (first, _) = frequency.at(1e-6);
        assert!((inner / (first * 1e3) - 1.0).abs() < 1e-12, "{inner}");
        assert!((inner_slope * 1e-8 / inner + 1.5).abs() < 1e-12);
    }

    /// Decision-p15-t6g-orbits §2: a drift is measured on `max(|J₀|, |Φ(x₀)|)`, so an orbit at its
    /// escape speed, whose `J₀` is about zero, is measured on the potential it moves in, a bound
    /// orbit on `|Φ₀|` and a fast kick on `|J₀|`.
    #[test]
    fn the_drift_is_measured_on_the_larger_of_the_integral_and_the_potential() {
        let pot = plain();
        let r = 26_000.0;
        let phi = pot.potential([r, 0.0, 0.0]);
        let at = |speed: f64| OrbitState {
            x: [r, 0.0, 0.0],
            v: [0.0, 0.0, speed],
        };
        let escape = (-2.0 * phi).sqrt() * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        let marginal = at(escape);
        assert!(pot.jacobi(&marginal).abs() < 1e-6 * phi.abs());
        assert!((pot.drift_scale(&marginal) / phi.abs() - 1.0).abs() < 1e-12);
        let bound = at(0.5 * escape);
        assert!((pot.drift_scale(&bound) / phi.abs() - 1.0).abs() < 1e-12);
        let fast = at(3.0 * escape);
        let j = pot.jacobi(&fast);
        assert!(j > phi.abs());
        assert!((pot.drift_scale(&fast) / j - 1.0).abs() < 1e-12);
        // An error of 10⁻³ of the potential reads as 10⁻³, whatever J₀.
        let mut moved = marginal;
        let speed = escape / LIGHT_YEARS_PER_YEAR_PER_KM_S;
        moved.v[2] = (speed * speed + 2e-3 * phi.abs()).sqrt() * LIGHT_YEARS_PER_YEAR_PER_KM_S;
        let d = pot.drift(&marginal, &moved);
        assert!((d - 1e-3).abs() < 1e-8, "{d}");
        assert!(pot.drift(&bound, &bound).abs() < 1e-15);
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

    /// The largest drift of the Jacobi integral on its scale on the first pass at `fraction` over
    /// `n` test orbits of `years` each.
    fn worst_drift(pot: &OrbitPotential, n: u64, years: f64, fraction: f64) -> f64 {
        let mut worst: f64 = 0.0;
        for i in 0..n {
            let start = test_orbit(pot, i);
            let pass = pot.integrate(start, years, fraction);
            assert_eq!(pass.ending, Ending::Aged, "orbit {i}");
            worst = worst.max(pot.drift(&start, &pass.end));
        }
        worst
    }

    /// P15.T6.a (fast sample): the energy holds to 10⁻⁴ of its scale (`max(|J₀|, |Φ(x₀)|)`,
    /// decision-p15-t6g-orbits §2) over 10 Gyr for 20 test orbits
    /// on the first pass, and the Jacobi integral with the bar does on the checked integration the
    /// run uses, `integrate_checked(…, 1e-4, 6)` (ruling 140.7; the first pass's worst of the 1,000
    /// is just over 10⁻⁴, so 20 of them are held as the run holds them). An orbit that keeps more
    /// than 10⁻⁴ after six halvings is reported with its drift at its first pass.
    #[test]
    fn energy_and_jacobi_hold_over_ten_gyr() {
        let plain = plain();
        let drift = worst_drift(&plain, 20, 1e10, FRACTION);
        assert!(drift < 1e-4, "energy drift {drift}");
        let barred = barred();
        for i in 0..20 {
            let start = test_orbit(&barred, i);
            let checked = barred.integrate_checked(start, 1e10, FRACTION, 1e-4, 6);
            assert!(
                checked.drift < 1e-4,
                "orbit {i}: Jacobi drift {:e} kept after {} halvings; {:e} at the first pass",
                checked.drift,
                checked.halvings,
                checked.first_drift,
            );
        }
    }

    /// **P15.T6.a's acceptance on P15.T6.g's method** (slow): 1,000 test orbits without the bar
    /// on the first pass, and 1,000 with it on the checked integration (1 ÷ 200, 10⁻⁴, six
    /// halvings) with at most 20 refined, the first pass's worst reported; orbits 368 and 740,
    /// which the fixed step never resolved from version 16's potential on (1.41 and 0.143 after six
    /// halvings), asserted by number. The decision measured 6.5 × 10⁻⁵ without the bar, 1.31 ×
    /// 10⁻⁴ at the first pass and 8.6 × 10⁻⁵ kept with it, 3 refined, and orbits 368 and 740 at
    /// 3.5 and 5.5 × 10⁻⁵ on their first pass.
    #[test]
    #[ignore = "slow: 2,000 orbits of 10 Gyr"]
    fn energy_and_jacobi_hold_over_ten_gyr_for_a_thousand_orbits() {
        let plain = plain();
        let drift = worst_drift(&plain, 1_000, 1e10, FRACTION);
        eprintln!("worst energy drift {drift:e}");
        assert!(drift < 1e-4, "energy drift {drift}");
        let barred = barred();
        let (mut first, mut kept, mut refined): (f64, f64, u64) = (0.0, 0.0, 0);
        let (mut steps, mut closest) = (0_u64, f64::INFINITY);
        for i in 0..1_000 {
            let start = test_orbit(&barred, i);
            let checked = barred.integrate_checked(start, 1e10, FRACTION, 1e-4, 6);
            assert_eq!(checked.ending, Ending::Aged, "orbit {i}");
            first = first.max(checked.first_drift);
            kept = kept.max(checked.drift);
            refined += u64::from(checked.halvings > 0);
            steps += checked.steps;
            closest = closest.min(checked.closest_ly);
            if i == 368 || i == 740 {
                eprintln!(
                    "orbit {i}: Jacobi drift {:e} at the first pass, {:e} kept after {} \
                     halvings; closest approach {:.3} ly",
                    checked.first_drift, checked.drift, checked.halvings, checked.closest_ly
                );
                assert!(checked.first_drift < 1e-4, "orbit {i}: {checked:?}");
            }
        }
        eprintln!(
            "worst Jacobi drift {first:e} at the first pass, {kept:e} kept; {refined} refined; \
             {steps} steps; closest approach {closest:.3} ly"
        );
        assert!(kept < 1e-4, "Jacobi drift {kept}");
        assert!(
            refined <= 20,
            "{refined} of 1,000 orbits needed a smaller step"
        );
    }

    /// P15.T6.g: the time-transformed leapfrog converges at second order. Over the 1,000 barred test
    /// orbits, the median first-pass drift (on the drift's scale) falls 3–5 fold per halving of the
    /// step fraction from 1 ÷ 50 to 1 ÷ 400, against a leapfrog's predicted 4
    /// (decision-p15-t6g-orbits §3: 4.11, 3.81 and 4.11, each at least 3.4 bootstrap standard
    /// deviations inside). The worst orbit's falls are reported, not asserted: a different orbit is
    /// worst at each step.
    #[test]
    #[ignore = "slow: 4,000 barred orbits of 10 Gyr"]
    fn the_median_drift_falls_fourfold_per_halving() {
        let barred = barred();
        let fractions = [1.0 / 50.0, 1.0 / 100.0, 1.0 / 200.0, 1.0 / 400.0];
        let (mut medians, mut worst) = (Vec::new(), Vec::new());
        for fraction in fractions {
            let mut drifts: Vec<f64> = (0..1_000)
                .map(|i| {
                    let start = test_orbit(&barred, i);
                    let end = barred.integrate(start, 1e10, fraction).end;
                    barred.drift(&start, &end)
                })
                .collect();
            drifts.sort_by(f64::total_cmp);
            medians.push(f64::midpoint(drifts[499], drifts[500]));
            worst.push(drifts[999]);
        }
        let falls: Vec<f64> = medians.windows(2).map(|m| m[0] / m[1]).collect();
        let worst_falls: Vec<f64> = worst.windows(2).map(|w| w[0] / w[1]).collect();
        eprintln!("median drifts {medians:?}, falls {falls:?}; the worst's falls {worst_falls:?}");
        assert!(
            falls.iter().all(|fall| (3.0..=5.0).contains(fall)),
            "median falls {falls:?} outside 3–5"
        );
    }

    /// P15.T6.g (slow): 16 in-plane orbits released at zero radial speed from 300 and 3,000 ly
    /// with a tangential speed of 10⁻², 10⁻³, 10⁻⁴ and 10⁻⁵ of `v_c`, in the plain and the barred
    /// potential, plunge through the cusp and towards the black hole on every passage, and each
    /// holds 10⁻⁴ over 10 Gyr on the checked integration or is captured (a capture is kept only
    /// from a pass within the tolerance).
    #[test]
    #[ignore = "slow: 16 plunging orbits of 10 Gyr"]
    fn low_angular_momentum_orbits_hold_or_are_captured() {
        for (name, pot) in [("plain", plain()), ("barred", barred())] {
            for r0 in [300.0, 3_000.0] {
                for f in [1e-2, 1e-3, 1e-4, 1e-5] {
                    let v_c = pot.tables().v_circ(LightYears::new(r0)).value()
                        * LIGHT_YEARS_PER_YEAR_PER_KM_S;
                    let start = OrbitState {
                        x: [r0, 0.0, 0.0],
                        v: [0.0, f * v_c, 0.0],
                    };
                    let checked = pot.integrate_checked(start, 1e10, FRACTION, 1e-4, 6);
                    eprintln!(
                        "{name} from {r0} ly at {f:e} v_c: drift {:e} after {} halvings \
                         ({:e} at the first), {:?}, closest approach {:e} ly, {} steps",
                        checked.drift,
                        checked.halvings,
                        checked.first_drift,
                        checked.ending,
                        checked.closest_ly,
                        checked.steps
                    );
                    assert!(
                        checked.drift <= 1e-4,
                        "{name} from {r0} ly at {f:e} v_c: {checked:?}"
                    );
                }
            }
        }
    }

    /// P15.T6.g: an orbit released at 0.5 ly with `|x × p|` 0.9 of `4GM• ÷ c` is captured, one
    /// with 1.1 of it is not, and one with 0.9 of it released from 1.5 ly, outside the capture
    /// radius, is captured on its way in.
    #[test]
    fn the_capture_rule_takes_only_orbits_below_four_gm_over_c() {
        let pot = plain();
        let threshold = pot.capture_momentum_ly2_per_year();
        // 4GM• ÷ c for the fixture's 4.3 × 10⁶ M☉: about 2.68 × 10⁻⁶ ly²/yr.
        assert!((2.6e-6..2.8e-6).contains(&threshold), "{threshold}");
        let released = |r0: f64, share: f64| OrbitState {
            x: [r0, 0.0, 0.0],
            v: [0.0, share * threshold / r0, 0.0],
        };
        let inside = pot.integrate(released(0.5, 0.9), 1e6, FRACTION);
        assert_eq!(inside.ending, Ending::Captured { after_years: 0.0 });
        let start = released(0.5, 1.1);
        let missed = pot.integrate(start, 1e6, FRACTION);
        assert_eq!(missed.ending, Ending::Aged);
        let passes = pot.drift(&start, &missed.end);
        eprintln!(
            "1.1 of 4GM• ÷ c from 0.5 ly: {} steps, closest approach {:e} ly, drift {passes:e}",
            missed.steps, missed.closest_ly
        );
        // Its pericentre is about L² ÷ 2GM•, 6.5 × 10⁻⁶ ly, five Schwarzschild radii, a thousand
        // times over in 10⁶ years, and the time-transformed steps hold its energy there.
        assert!(missed.closest_ly < 1e-5, "{missed:?}");
        assert!(passes < 1e-4, "{missed:?}: drift {passes:e}");
        let falling = pot.integrate(released(1.5, 0.9), 1e6, FRACTION);
        let Ending::Captured { after_years } = falling.ending else {
            panic!("not captured: {falling:?}");
        };
        assert!(after_years > 0.0 && falling.steps > 0, "{falling:?}");
        assert!(norm(falling.end.x) < CAPTURE_RADIUS_LY, "{falling:?}");
    }

    /// A pass that cannot proceed ends `Broken` rather than looping: a duration that is not finite,
    /// or a step fraction of zero (no step is positive); the check counts its drift as infinite.
    #[test]
    fn a_pass_that_cannot_step_ends_broken() {
        let pot = plain();
        let start = test_orbit(&pot, 1);
        for duration in [f64::NAN, f64::INFINITY] {
            let pass = pot.integrate(start, duration, FRACTION);
            assert_eq!((pass.ending, pass.steps), (Ending::Broken, 0), "{duration}");
        }
        assert_eq!(pot.integrate(start, 1e6, 0.0).ending, Ending::Broken);
        let checked = pot.integrate_checked(start, 1e6, 0.0, 1e-4, 0);
        assert!(checked.drift.is_infinite() && checked.first_drift.is_infinite());
        assert_eq!(checked.ending, Ending::Broken);
    }

    /// P15.T6.a: a circular orbit in the plane stays circular to 10⁻³ in radius over 10 Gyr, at
    /// radii across the disc.
    #[test]
    fn a_circular_orbit_stays_circular() {
        let pot = plain();
        for r in [3_000.0, 10_000.0, 26_000.0, 50_000.0] {
            let v = pot.tables().v_circ(LightYears::new(r)).value() * LIGHT_YEARS_PER_YEAR_PER_KM_S;
            let mut s = OrbitState {
                x: [r, 0.0, 0.0],
                v: [0.0, v, 0.0],
            };
            let mut worst: f64 = 0.0;
            for _ in 0..100 {
                s = pot.integrate(s, 1e8, FRACTION).end;
                worst = worst.max((math::hypot(s.x[0], s.x[1]) / r - 1.0).abs());
            }
            assert!(worst < 1e-3, "{r} ly: {worst}");
        }
    }

    /// P15.T6.a: the same orbits on one and on four threads give identical bytes.
    #[test]
    fn one_and_four_threads_give_the_same_bytes() {
        let pot = barred();
        let run = |threads: usize| {
            let mut values = Vec::new();
            map_reduce_chunks(
                16,
                3,
                NonZeroUsize::new(threads).unwrap(),
                |range| {
                    range
                        .map(|i| pot.integrate(test_orbit(&pot, i), 1e9, FRACTION).end)
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
