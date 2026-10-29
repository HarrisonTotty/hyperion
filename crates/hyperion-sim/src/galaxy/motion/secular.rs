//! The secular rates of an orbit about the black hole: the relativistic advance of its pericentre,
//! the retrograde precession the enclosed stars drive, and the change of its mean motion that goes
//! with it (plan 09, P09.T28.d.a; decision-galaxy-25-train §2.2).
//!
//! "A system follows the Kepler orbit about the black hole and the cluster mass inside its own
//! radius, with the relativistic advance of pericentre, and the precession from the enclosed stars,
//! added as secular rates" (brainstorm, "Orbits and time"). [`KeplerOrbit::propagate`] keeps the
//! two-body law; [`KeplerOrbit::propagate_secular`] is the secular step, the two-body orbit
//! propagated at the corrected mean motion and turned in its plane by the rates × dt, from the
//! [`SecularRates`] of the orbit ([`KeplerOrbit::secular_rates`]). Nothing reads them at this
//! version: the term in [`position_velocity_at`](super::position_velocity_at) is P09.T28.d.b's,
//! behind `KEPLER_SECULAR_VERSION` = 25.
//!
//! # The relativistic advance
//!
//! The Schwarzschild advance of a bound orbit is `Δω = 6π G M ÷ (c² a (1 − e²))` a radial period
//! about a black hole of mass M (Einstein 1915; Weinberg 1972, *Gravitation and Cosmology*, §8.6),
//! so `ω̇ = 3 G M n ÷ (c² a (1 − e²))` with the orbit's mean motion n
//! ([`KeplerOrbit::schwarzschild_rate`]). M is the black hole's alone: the stars' own term is of
//! order `G M★ ÷ (c² r)`, under 10⁻¹⁰ of the orbit's. S2's orbit (a = 0.0163 ly, e = 0.8846;
//! GRAVITY Collaboration 2020, A&A 636, L5) advances 12.3′ an orbit about the Milky Way fixture's
//! 4.297 × 10⁶ M☉, where GRAVITY measured 12.1′ about their 4.261 × 10⁶ M☉. The change it makes to
//! the radial period, of order `G M ÷ (c² a)`, depends on the coordinates and is not modelled.
//!
//! # The stars' precession
//!
//! An orbit at `r₀` follows the Kepler orbit about μ = G M(<r₀), the black hole and the stars
//! inside its epoch radius (P09.T28.b.a), while the true potential, Φ with `Φ′ = G M(<r) ÷ r²`,
//! holds the stars spread over the orbit. The difference, `ΔΦ = Φ + μ ÷ r`, perturbs it. Averaged
//! over the orbit it depends on a and e alone, so it moves neither, and turns the pericentre and
//! shifts the mean motion at rates given by Lagrange's equations for the disturbing function
//! `R = −⟨ΔΦ⟩` (Murray and Dermott 1999, *Solar System Dynamics*, §6.8, eqs. 6.145–6.150; Merritt
//! 2013, *Dynamics and Evolution of Galactic Nuclei*, §4.4.1, for the stars about a black hole):
//!
//! `ω̇ = √(1 − e²) ÷ (n a² e) ∂R ÷ ∂e`,
//! `Ṁ − n = −2 ÷ (n a) ∂R ÷ ∂a − (1 − e²) ÷ (n a² e) ∂R ÷ ∂e`.
//!
//! With the time average `⟨f⟩ = (1 ÷ π) ∫₀^π f(r) (1 − e cos E) dE`, `r = a (1 − e cos E)`, and
//! the stars' pull `g★ = G M★(<r) ÷ r²` (M★ the stars of [`CentreMotion::stars_within`]), the two
//! partial derivatives come to the orbit averages of the pull, so no potential is needed for them
//! (the second by parts in E):
//!
//! - `∂⟨ΔΦ⟩ ÷ ∂a = (G M_bh − μ) ÷ a² + A★`, `A★ = (1 ÷ π) ∫₀^π g★ (1 − e cos E)² dE`;
//! - `∂⟨ΔΦ⟩ ÷ ∂e = a B★`, `B★ = (1 ÷ π) ∫₀^π g★ (e − cos E) dE`, to which the black hole's and μ's
//!   point masses add nothing.
//!
//! So `ω̇★ = −n √(1 − e²) β★ ÷ μ` ([`KeplerOrbit::mass_precession_rate`]) and
//! `Ṁ − n = n (2 (G M_bh − μ) + 2 α★ + (1 − e²) β★) ÷ μ`, with `α★ = a² A★` and
//! `β★ = a² B★ ÷ e`, both positive for a density that falls outward. ω̇★ is negative, retrograde.
//! For a circular orbit `α★ = G M★(<r₀)`, so the mean motion's shift is `−ω̇★` and the orbit keeps
//! its two-body motion, the circular orbit at r₀ being exact about μ; at small e, `ω̇★` is
//! `Ω − κ` to first order in the stars' mass, the apsidal rate of a nearly circular orbit (Binney
//! and Tremaine 2008, *Galactic Dynamics*, 2nd ed., §3.2.3).
//!
//! **The mean motion is the mean orbit's.** The epoch's osculating a is not the mean one: the
//! Kepler energy of the state, `v² ÷ 2 − μ ÷ r`, is the true energy less `ΔΦ(r)`, so over the
//! orbit it averages to the true energy less `⟨ΔΦ⟩`, and the mean a is
//! `1 ÷ ā = 1 ÷ a − 2 (ΔΦ(r₀) − ⟨ΔΦ⟩) ÷ μ`, with
//! `ΔΦ(r₀) − ⟨ΔΦ⟩ = (μ − G M_bh) (1 ÷ r₀ − 1 ÷ a) + Γ★(r₀) − ⟨Γ★⟩`, Γ★ the stars' potential
//! (`Γ★′ = g★`). The rate of the mean anomaly is then `√(μ ÷ ā³) + (Ṁ − n)`, and its difference
//! from the epoch orbit's n is the step's
//! [`mean_motion_correction`](SecularRates::mean_motion_correction).
//! Without it an eccentric orbit's mean motion would be off by as much as the precession it gains.
//!
//! This is first-order secular theory: the rates are the orbit averages, and the short-period
//! terms are not modelled, so the step tracks the true orbit over many radial periods and not
//! within one. Against a direct integration in the black hole's and the stars' potential over
//! ±1,000 yr (P09.T28.d.a as built, in plan 09's Risks), it keeps a circular orbit exact, and on
//! eccentric ones cuts the two-body law's error some thirtyfold at 0.1 ly, where the span holds
//! one to eight radial periods; at 1 and 5 ly, where it holds a fraction of one, the two-body law
//! is the closer, and neither meets a tenth of the orbit's tidal radius on an eccentric orbit.
//!
//! # The eccentricity table
//!
//! The centre's profile is a broken power law (`CentreProfile`, an inner slope of 1.3 with breaks
//! at 10 and 100 ly), with the rest of the galaxy's monopole on top, so no one slope serves every
//! orbit. [`SecularTable`] holds the orbit averages of the profile itself on (a, e), built once
//! from a galaxy's [`CentreMotion`] (lean, 2026-10-29; plan 09's Risks): `α★`, `β★` and
//! `⟨Γ★⟩ − Γ★(a)`, each divided by its scale `G M★(<a)` (or `G M★(<a) ÷ a`) so that what is
//! interpolated varies slowly, at 16 knots a decade in a from 10⁻⁴ to 10⁴ ly and 32 eccentricities,
//! read by cubic Lagrange interpolation in `ln a` and in e. The averages are 80-node quadratures in
//! the eccentric anomaly, graded towards pericentre, of `M★(<r)` and Γ★ tabulated at 32 knots a
//! decade from 10⁻⁷ to 10⁶ ly. Outside those ranges a and r are clamped to the tables' ends; an
//! eccentricity above the last node's, 63 ÷ 64, takes its averages there.
//!
//! # Determinism
//!
//! Pure: fixed nodes, fixed knots, fixed operations in a fixed order, and [`math`] functions, so
//! the table and the rates are the same bits on every platform. The table, its knots and nodes,
//! and the form of every expression here belong to the generator version from the version whose
//! term reads them (P09.T28.d.b, 25).

use std::f64::consts::{PI, TAU};

use super::{CentreMotion, KeplerOrbit, cross, dot, length};
use crate::math;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR, SPEED_OF_LIGHT};
use crate::units::{GravitationalParameter, LightYears, PerYear, Seconds};

/// The first knot of the stars' mass and potential, ly: inside it the stars weigh some 10⁻⁷ M☉,
/// and their potential's change is under 10⁻⁶ of what an orbit at 10⁻⁴ ly feels.
const RADIUS_LO_LY: f64 = 1e-7;

/// Decades of the stars' tables, 10⁻⁷ to 10⁶ ly, as the profile's own.
const RADIUS_DECADES: u32 = 13;

/// Knots a decade of the stars' tables, as the profile's own.
const RADIUS_KNOTS_PER_DECADE: u32 = 32;

/// The least semi-major axis tabulated, ly: about 6 au, inside the loss cone's 2 au pericentre
/// for every eccentricity above 2/3.
const AXIS_LO_LY: f64 = 1e-4;

/// Decades of semi-major axis tabulated, 10⁻⁴ to 10⁴ ly: beyond the largest influence radius of
/// the sweep, 177 ly, by more than an order of magnitude.
const AXIS_DECADES: u32 = 8;

/// Knots a decade of semi-major axis: at 8 the averages of orbits about the profile's break at 10
/// ly, whose sharpness is 10, were read 4 × 10⁻³ off; at 16, under 3 × 10⁻⁴.
const AXIS_KNOTS_PER_DECADE: u32 = 16;

/// The eccentricities tabulated, `(j + ½) ÷ 32` for j = 0..31: half-steps, so that the even
/// averages are interpolated across e = 0 by their mirror images.
const ECCENTRICITY_NODES: u32 = 32;

/// The eccentric anomaly's panels, each a 16-node Gauss–Legendre rule, graded towards pericentre
/// (E = 0), where an orbit near e = 1 passes in a small range of E.
const ANOMALY_PANELS: [f64; 6] = [0.0, PI / 16.0, PI / 8.0, PI / 4.0, PI / 2.0, PI];

/// Knots `lo + k × step` of a uniform axis, k = 0..count.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Axis {
    lo: f64,
    step: f64,
    count: u32,
}

impl Axis {
    /// The axis of `ln (r ÷ 1 ly)` from `lo` ly over `decades` decades at `per_decade` knots each.
    fn decades(lo: f64, decades: u32, per_decade: u32) -> Self {
        Self {
            lo: math::ln(lo),
            step: core::f64::consts::LN_10 / f64::from(per_decade),
            count: decades * per_decade + 1,
        }
    }

    /// Knot `k`.
    fn knot(&self, k: u32) -> f64 {
        self.lo + f64::from(k) * self.step
    }

    /// The first of the four knots that interpolate at `x`, clamped to the axis, and the cubic
    /// Lagrange weights of the four.
    fn window(&self, x: f64) -> (u32, [f64; 4]) {
        let last = f64::from(self.count - 1);
        let u = ((x - self.lo) / self.step).clamp(0.0, last);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "u is clamped to [0, count − 1] and floored: a small non-negative integer"
        )]
        let i = (u.floor() as u32).clamp(1, self.count - 3);
        (i - 1, lagrange(u - f64::from(i)))
    }
}

/// The cubic Lagrange weights at offset `t` from the second of four uniform knots.
fn lagrange(t: f64) -> [f64; 4] {
    let (tm, t1, t2) = (t + 1.0, t - 1.0, t - 2.0);
    [
        -t * t1 * t2 / 6.0,
        tm * t1 * t2 / 2.0,
        -tm * t * t2 / 2.0,
        tm * t * t1 / 6.0,
    ]
}

/// The eccentricity node `j`, `(j + ½) ÷ 32`.
fn eccentricity_node(j: u32) -> f64 {
    (f64::from(j) + 0.5) / f64::from(ECCENTRICITY_NODES)
}

/// The four eccentricity nodes that interpolate at `e`, as indices into the table's row (a node
/// below e = 0 mirrored to its image above), and their weights.
fn eccentricity_window(e: f64) -> ([u32; 4], [f64; 4]) {
    let n = f64::from(ECCENTRICITY_NODES);
    let u = (e * n - 0.5).min(n - 1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "u is at most 31 and floored; a negative u (e under the first node) gives 0"
    )]
    let i = (u.max(0.0).floor() as u32).min(ECCENTRICITY_NODES - 3);
    // Node i − 1 is node −1 for i = 0, whose mirror image is node 0.
    let first = i.saturating_sub(1);
    ([first, i, i + 1, i + 2], lagrange(u - f64::from(i)))
}

/// The orbit averages of the stars' pull on a Kepler orbit about the black hole, tabulated once
/// from a galaxy's centre (module documentation): the eccentricity table of P09.T28.d, from which
/// [`KeplerOrbit::secular_rates`] reads the stars' precession and the shift of the mean motion.
///
/// It is a pure function of the [`CentreMotion`]: the black hole's mass and the stars inside each
/// radius ([`CentreMotion::stars_within`]). Building it reads the stars' mass at some 7,000 radii
/// and its own tables of it some 660,000 times, about 50 ms in a release build; it holds about
/// 106 kB.
///
/// # Examples
///
/// A star 0.1 ly from the Milky Way's black hole, on an orbit of e ≈ 0.5, precesses backwards by
/// some arcminutes an orbit, and S2's relativistic advance is the larger:
///
/// ```
/// use hyperion_sim::coords::GalacticVelocity;
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::motion::secular::SecularTable;
/// use hyperion_sim::galaxy::motion::{CentreMotion, KeplerOrbit};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
/// use hyperion_sim::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR};
/// use hyperion_sim::units::{GravitationalParameter, LightYears};
///
/// let params = GalaxyParams::milky_way_like();
/// let motion = CentreMotion::new(&params, &MassModel::new(&params))?;
/// let table = SecularTable::new(&motion);
/// let r = 0.1;
/// let mu = GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r)));
/// let speed = 0.7 * (mu.value() / (r * METRES_PER_LIGHT_YEAR)).sqrt();
/// let orbit = KeplerOrbit::from_state(
///     PointLy::new(r, 0.0, 0.0),
///     GalacticVelocity::new([0.0, speed, 0.0]),
///     mu,
/// )?;
/// let rates = orbit.secular_rates(&table).expect("the orbit is bound");
/// let period_yr = orbit.period().expect("bound").value() / SECONDS_PER_JULIAN_YEAR;
/// let arcmin = |per_year: f64| per_year * period_yr * 180.0 / std::f64::consts::PI * 60.0;
/// let stars = arcmin(rates.mass_precession().value());
/// let einstein = arcmin(rates.schwarzschild().value());
/// assert!(stars < 0.0 && einstein > 0.0, "{stars}′ and {einstein}′ an orbit");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SecularTable {
    /// G `M_bh`, m³ s⁻².
    black_hole: f64,
    /// `ln (r ÷ 1 ly)` of the stars' tables.
    radii: Axis,
    /// G M★(<r) at each radius knot, m³ s⁻².
    stars: Vec<f64>,
    /// The stars' potential Γ★ at each radius knot, zero at the first, m² s⁻².
    potential: Vec<f64>,
    /// `ln (a ÷ 1 ly)` of the orbit averages.
    axes: Axis,
    /// `[α★, β★] ÷ G M★(<a)` and `(⟨Γ★⟩ − Γ★(a)) a ÷ G M★(<a)` at each (a, e) knot, a-major.
    averages: Vec<[f64; 3]>,
}

impl SecularTable {
    /// The table of the centre whose motion is `motion` ([`Galaxy::centre_motion`]).
    ///
    /// [`Galaxy::centre_motion`]: crate::galaxy::Galaxy::centre_motion
    #[must_use]
    pub fn new(motion: &CentreMotion) -> Self {
        let black_hole = GravitationalParameter::from_solar_masses(motion.profile().black_hole());
        let radii = Axis::decades(RADIUS_LO_LY, RADIUS_DECADES, RADIUS_KNOTS_PER_DECADE);
        let stars_at = |x: f64| {
            GravitationalParameter::from_solar_masses(
                motion.stars_within(LightYears::new(math::exp(x))),
            )
            .value()
        };
        let stars: Vec<f64> = (0..radii.count).map(|k| stars_at(radii.knot(k))).collect();
        // Γ★ = ∫ g★ dr = ∫ G M★(<r) ÷ r d ln r, panel by panel from the first knot.
        let mut potential = Vec::with_capacity(stars.len());
        let mut sum = 0.0;
        potential.push(sum);
        for k in 1..radii.count {
            sum += crate::galaxy::quad::gl16(
                |x| stars_at(x) / (math::exp(x) * METRES_PER_LIGHT_YEAR),
                radii.knot(k - 1),
                radii.knot(k),
            );
            potential.push(sum);
        }
        let mut table = Self {
            black_hole: black_hole.value(),
            radii,
            stars,
            potential,
            axes: Axis::decades(AXIS_LO_LY, AXIS_DECADES, AXIS_KNOTS_PER_DECADE),
            averages: Vec::new(),
        };
        let averages = (0..table.axes.count)
            .flat_map(|i| {
                let a = math::exp(table.axes.knot(i)) * METRES_PER_LIGHT_YEAR;
                (0..ECCENTRICITY_NODES).map(move |j| (a, eccentricity_node(j)))
            })
            .map(|(a, e)| table.normalised_averages(a, e))
            .collect();
        table.averages = averages;
        table
    }

    /// G M★(<r) at `r` m, m³ s⁻², from the stars' table.
    fn stars_at(&self, r: f64) -> f64 {
        self.read_radius(&self.stars, r)
    }

    /// The stars' potential Γ★ at `r` m, m² s⁻², from its table.
    fn potential_at(&self, r: f64) -> f64 {
        self.read_radius(&self.potential, r)
    }

    /// `values`, tabulated at the radius knots, at `r` m.
    fn read_radius(&self, values: &[f64], r: f64) -> f64 {
        let (first, w) = self.radii.window(math::ln(r / METRES_PER_LIGHT_YEAR));
        let at = |k: u32| values[k as usize];
        w[0] * at(first) + w[1] * at(first + 1) + w[2] * at(first + 2) + w[3] * at(first + 3)
    }

    /// `[α★, β★] ÷ G M★(<a)` and `(⟨Γ★⟩ − Γ★(a)) a ÷ G M★(<a)` of the orbit of semi-major axis `a`
    /// m and eccentricity `e` in `(0, 1)`, by quadrature in the eccentric anomaly; zero where the
    /// stars inside a weigh nothing.
    fn normalised_averages(&self, a: f64, e: f64) -> [f64; 3] {
        let scale = self.stars_at(a);
        if scale <= 0.0 {
            return [0.0; 3];
        }
        let at_a = self.potential_at(a);
        let mut sums = [0.0; 3];
        for edges in ANOMALY_PANELS.windows(2) {
            let half = 0.5 * (edges[1] - edges[0]);
            let mid = edges[0] + half;
            for (&x, &w) in GL16_NODES.iter().zip(&GL16_WEIGHTS) {
                let cos = math::cos(mid + half * x);
                let s = 1.0 - e * cos;
                let r = a * s;
                let pull = self.stars_at(r) / (r * r);
                let weight = w * half;
                sums[0] += weight * pull * s * s;
                sums[1] += weight * pull * (e - cos);
                sums[2] += weight * (self.potential_at(r) - at_a) * s;
            }
        }
        let [alpha, beta, gamma] = sums.map(|sum| sum / PI);
        [
            a * a * alpha / scale,
            a * a * beta / (e * scale),
            gamma * a / scale,
        ]
    }

    /// The normalised averages at semi-major axis `a` m and eccentricity `e`, interpolated.
    fn averages_at(&self, a: f64, e: f64) -> [f64; 3] {
        let (first, wa) = self.axes.window(math::ln(a / METRES_PER_LIGHT_YEAR));
        let (nodes, we) = eccentricity_window(e);
        let mut out = [0.0; 3];
        for (di, &w_a) in (0..4).zip(&wa) {
            let row = (first + di) * ECCENTRICITY_NODES;
            for (&j, &w_e) in nodes.iter().zip(&we) {
                let value = self.averages[(row + j) as usize];
                for (o, v) in out.iter_mut().zip(value) {
                    *o += w_a * w_e * v;
                }
            }
        }
        out
    }
}

/// The secular rates of one bound Kepler orbit about the black hole (module documentation), from
/// [`KeplerOrbit::secular_rates`], for [`KeplerOrbit::propagate_secular`] on the same orbit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecularRates {
    /// The Schwarzschild advance of the pericentre, rad s⁻¹.
    schwarzschild: f64,
    /// The stars' precession of the pericentre, rad s⁻¹.
    mass_precession: f64,
    /// The mean anomaly's rate less the orbit's two-body mean motion, rad s⁻¹.
    mean_motion: f64,
}

impl SecularRates {
    /// The Schwarzschild advance of the pericentre, radians per Julian year: positive, prograde.
    #[must_use]
    pub fn schwarzschild(&self) -> PerYear {
        PerYear::new(self.schwarzschild * SECONDS_PER_JULIAN_YEAR)
    }

    /// The precession of the pericentre that the stars about the black hole drive, radians per
    /// Julian year: negative, retrograde.
    #[must_use]
    pub fn mass_precession(&self) -> PerYear {
        PerYear::new(self.mass_precession * SECONDS_PER_JULIAN_YEAR)
    }

    /// The rate at which the orbit turns in its plane, the sum of the two, radians per Julian year.
    #[must_use]
    pub fn apsidal(&self) -> PerYear {
        PerYear::new((self.schwarzschild + self.mass_precession) * SECONDS_PER_JULIAN_YEAR)
    }

    /// The mean anomaly's rate less the two-body mean motion of the orbit from the epoch state,
    /// radians per Julian year: the mean orbit's mean motion and the stars' shift of it, less
    /// `2π ÷ P` (module documentation).
    #[must_use]
    pub fn mean_motion_correction(&self) -> PerYear {
        PerYear::new(self.mean_motion * SECONDS_PER_JULIAN_YEAR)
    }
}

impl KeplerOrbit {
    /// The Schwarzschild advance of the pericentre about a black hole of gravitational parameter
    /// `black_hole`, radians per Julian year: `3 G M n ÷ (c² a (1 − e²))`, which is
    /// `6π G M ÷ (c² a (1 − e²))` a radial period (module documentation); `None` for a parabolic or
    /// unbound orbit, which takes no secular rate.
    #[must_use]
    pub fn schwarzschild_rate(&self, black_hole: GravitationalParameter) -> Option<PerYear> {
        (self.alpha > 0.0).then(|| {
            PerYear::new(self.schwarzschild_si(black_hole.value()) * SECONDS_PER_JULIAN_YEAR)
        })
    }

    /// The precession of the pericentre that the stars about the black hole drive, radians per
    /// Julian year, from `table`: `−n √(1 − e²) β★ ÷ μ`, negative (module documentation); `None`
    /// for a parabolic or unbound orbit.
    #[must_use]
    pub fn mass_precession_rate(&self, table: &SecularTable) -> Option<PerYear> {
        self.secular_rates(table)
            .map(|rates| rates.mass_precession())
    }

    /// The orbit's secular rates about the centre whose table is `table`: the Schwarzschild advance
    /// about its black hole, the stars' precession and the correction to the mean motion (module
    /// documentation); `None` for a parabolic or unbound orbit, which takes neither rate.
    #[must_use]
    pub fn secular_rates(&self, table: &SecularTable) -> Option<SecularRates> {
        if self.alpha <= 0.0 {
            return None;
        }
        let (mu, a, r0) = (self.mu, 1.0 / self.alpha, self.radius);
        let e = self.eccentricity();
        let n = self.mean_motion_si();
        let one_minus_e_sq = (1.0 - e) * (1.0 + e);
        let black_hole = table.black_hole;
        let scale = table.stars_at(a);
        let [alpha_hat, beta_hat, gamma_hat] = table.averages_at(a, e);
        let (alpha, beta, gamma) = (alpha_hat * scale, beta_hat * scale, gamma_hat * scale / a);
        let mass_precession = -n * one_minus_e_sq.sqrt() * beta / mu;
        let shift = n * (2.0 * (black_hole - mu) + 2.0 * alpha + one_minus_e_sq * beta) / mu;
        // ΔΦ(r₀) − ⟨ΔΦ⟩, and the mean orbit's 1 ÷ a.
        let offset = (mu - black_hole) * (1.0 / r0 - 1.0 / a) + table.potential_at(r0)
            - table.potential_at(a)
            - gamma;
        let inverse_mean_a = (1.0 / a - 2.0 * offset / mu).max(0.0);
        let mean_n = mu.sqrt() * inverse_mean_a * inverse_mean_a.sqrt();
        Some(SecularRates {
            schwarzschild: self.schwarzschild_si(black_hole),
            mass_precession,
            mean_motion: mean_n + shift - n,
        })
    }

    /// The orbit's state `dt` later under its secular rates (the secular step, module
    /// documentation): the two-body orbit propagated at its mean motion plus
    /// [`SecularRates::mean_motion_correction`], then turned in its plane about its angular
    /// momentum by [`SecularRates::apsidal`] × dt. `rates` are this orbit's
    /// ([`secular_rates`](Self::secular_rates)), taken at the epoch state that `dt` counts from. A
    /// parabolic or unbound orbit takes neither rate, and is [`propagate`](Self::propagate)d.
    ///
    /// At `dt` = 0 it is the orbit itself. It keeps the orbit's energy and angular momentum about
    /// μ, which the turn does not change and the propagation keeps.
    ///
    /// # Panics
    ///
    /// As [`propagate`](Self::propagate).
    #[must_use]
    pub fn propagate_secular(&self, dt: Seconds, rates: &SecularRates) -> Self {
        if self.alpha <= 0.0 {
            return self.propagate(dt);
        }
        let t = dt.value();
        let n = self.mean_motion_si();
        let later = self.propagate(Seconds::new(t + t * (rates.mean_motion / n)));
        let angle = (rates.schwarzschild + rates.mass_precession) * t;
        let h = self.angular_momentum_m2_per_s();
        let norm = length(h);
        let axis = h.map(|c| c / norm);
        let (sin, cos) = math::sin_cos(angle);
        // Rodrigues' rotation about the unit axis k:
        // x cos θ + (k × x) sin θ + k (k · x)(1 − cos θ).
        let turn = |x: [f64; 3]| {
            let across = cross(axis, x);
            let along = dot(axis, x) * (1.0 - cos);
            [0, 1, 2].map(|i| x[i] * cos + across[i] * sin + axis[i] * along)
        };
        Self::from_si(turn(later.position), turn(later.velocity), self.mu)
    }

    /// The mean motion `√(μ α³)` of a bound orbit, rad s⁻¹; meaningful only when α > 0.
    fn mean_motion_si(&self) -> f64 {
        TAU / self.period_si()
    }

    /// The Schwarzschild advance about `black_hole` (G M, m³ s⁻²), rad s⁻¹; meaningful only when
    /// α > 0.
    fn schwarzschild_si(&self, black_hole: f64) -> f64 {
        let a = 1.0 / self.alpha;
        let e = self.eccentricity();
        3.0 * black_hole * self.mean_motion_si()
            / (SPEED_OF_LIGHT * SPEED_OF_LIGHT * a * (1.0 - e) * (1.0 + e))
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::coords::GalacticVelocity;
    use crate::galaxy::PointLy;
    use crate::galaxy::features::centre::{CentreProfile, LossCone, SlopeBreak, TracerShape};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::potential::MassModel;
    use crate::units::SolarMasses;
    use crate::units::consts::RADIANS_PER_DEGREE;

    const YEAR: f64 = SECONDS_PER_JULIAN_YEAR;

    /// Arcminutes in one radian.
    const ARCMIN_PER_RADIAN: f64 = 60.0 / RADIANS_PER_DEGREE;

    /// The Milky Way fixture's centre motion, built from its parameters without a galaxy.
    fn milky_way() -> CentreMotion {
        let params = GalaxyParams::milky_way_like();
        CentreMotion::new(&params, &MassModel::new(&params)).expect("the fixture's centre")
    }

    /// μ = G M(<r) at `r` ly.
    fn mu_at(motion: &CentreMotion, r: f64) -> f64 {
        GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r))).value()
    }

    /// The orbit from `r` ly on +x at `factor` times the circular speed there, `tilt` radians
    /// outward from the tangent and inclined by 0.3 rad out of the x-y plane.
    fn orbit_at(motion: &CentreMotion, r: f64, factor: f64, tilt: f64) -> KeplerOrbit {
        let mu = mu_at(motion, r);
        let speed = factor * (mu / (r * METRES_PER_LIGHT_YEAR)).sqrt();
        let (sin, cos) = math::sin_cos(tilt);
        let (sin_i, cos_i) = math::sin_cos(0.3);
        KeplerOrbit::from_state(
            PointLy::new(r, 0.0, 0.0),
            GalacticVelocity::new([speed * sin, speed * cos * cos_i, speed * cos * sin_i]),
            GravitationalParameter::new(mu),
        )
        .expect("a valid state")
    }

    /// The black hole of `black_hole` M☉ with a cluster of uniform density holding `mass` M☉
    /// inside the reach: a harmonic core about a point mass, whose averages have closed forms.
    fn harmonic(black_hole: f64, mass: f64) -> CentreMotion {
        let shape = TracerShape::new(
            0.0,
            [SlopeBreak {
                radius: 1e5,
                sharpness: 2.0,
                rise: 4.0,
            }],
        )
        .expect("a valid shape");
        let profile =
            CentreProfile::from_shape(shape, SolarMasses::new(mass), SolarMasses::new(black_hole))
                .expect("a valid profile");
        CentreMotion {
            profile,
            influence_radius: LightYears::new(10.0),
            loss_cone: LossCone::new(SolarMasses::new(black_hole)),
        }
    }

    /// The plan's test: the Schwarzschild advance of an S2-like orbit is 12′ an orbit, at GRAVITY
    /// Collaboration 2020's elements (a = 0.0163 ly, e = 0.8846; their 12.1′ is about 4.261 × 10⁶
    /// M☉) and at the plan's own (a = 0.0158 ly, e = 0.88), about the fixture's 4.297 × 10⁶ M☉;
    /// and it is the closed form `6π G M ÷ (c² a (1 − e²))` a period.
    #[test]
    fn an_s2_like_orbit_advances_12_arcminutes_an_orbit() {
        let black_hole = GravitationalParameter::from_solar_masses(SolarMasses::new(4.297e6));
        for (a, e) in [(0.0163, 0.8846), (0.0158, 0.88)] {
            let apocentre = a * (1.0 + e);
            let speed =
                (black_hole.value() * (1.0 - e) / (apocentre * METRES_PER_LIGHT_YEAR)).sqrt();
            let orbit = KeplerOrbit::from_state(
                PointLy::new(apocentre, 0.0, 0.0),
                GalacticVelocity::new([0.0, speed, 0.0]),
                black_hole,
            )
            .expect("a valid state");
            let period = orbit.period().expect("bound").value() / YEAR;
            let rate = orbit.schwarzschild_rate(black_hole).expect("bound").value();
            let per_orbit = rate * period;
            let closed = 6.0 * PI * black_hole.value()
                / (SPEED_OF_LIGHT * SPEED_OF_LIGHT * a * METRES_PER_LIGHT_YEAR * (1.0 - e * e));
            println!(
                "a {a} ly, e {e}: {period:.3} yr, {:.3}′ an orbit",
                per_orbit * ARCMIN_PER_RADIAN
            );
            assert!(
                (11.8..12.6).contains(&(per_orbit * ARCMIN_PER_RADIAN)),
                "{}′",
                per_orbit * ARCMIN_PER_RADIAN
            );
            assert!(
                (per_orbit / closed - 1.0).abs() < 1e-9,
                "{per_orbit} against {closed}"
            );
        }
    }

    /// In a harmonic core, M★ ∝ r³ and g★ = k r, the averages are `α★ = G M★(<a) (1 + 3e² ÷ 2)`,
    /// `β★ = 3 G M★(<a) ÷ 2` and `⟨Γ★⟩ − Γ★(a) = (3e² ÷ 4) G M★(<a) ÷ a`, from `⟨r²⟩ = a² (1 +
    /// 3e² ÷ 2)` (Murray and Dermott 1999, table 2.1); the table holds them between its knots,
    /// and the stars' precession is the harmonic potential's `−(3 ÷ 2) n √(1 − e²) G M★(<a) ÷ μ`.
    #[test]
    fn the_table_holds_a_harmonic_core_s_closed_forms() {
        let motion = harmonic(4.297e6, 1e7);
        let table = SecularTable::new(&motion);
        let mut lcg = Lcg::new(0x0928_d000_0000_0001);
        let mut worst = [0.0_f64; 3];
        for _ in 0..200 {
            let a = 1e-3 * math::exp10(4.0 * lcg.next_f64()) * METRES_PER_LIGHT_YEAR;
            let e = 0.95 * lcg.next_f64();
            let [alpha, beta, gamma] = table.averages_at(a, e);
            let expected = [1.0 + 1.5 * e * e, 1.5, 0.75 * e * e];
            for k in 0..3 {
                let error = (([alpha, beta, gamma][k] - expected[k]) / expected[k].max(0.01)).abs();
                worst[k] = worst[k].max(error);
            }
        }
        println!("harmonic core, worst relative errors (α★, β★, γ★): {worst:?}");
        assert!(
            worst[0] < 1e-4 && worst[1] < 1e-4 && worst[2] < 1e-3,
            "{worst:?}"
        );
        // The rate, at a knot-free point.
        let orbit = orbit_at(&motion, 0.37, 0.8, 0.2);
        let mu = orbit.gravitational_parameter().value();
        let a = 1.0 / orbit.alpha;
        let e = orbit.eccentricity();
        let n = orbit.mean_motion_si();
        let stars = GravitationalParameter::from_solar_masses(
            motion.stars_within(LightYears::new(a / METRES_PER_LIGHT_YEAR)),
        )
        .value();
        let expected = -1.5 * n * (1.0 - e * e).sqrt() * stars / mu * YEAR;
        let rate = orbit.mass_precession_rate(&table).expect("bound").value();
        assert!(
            (rate / expected - 1.0).abs() < 1e-4,
            "{rate} against {expected}"
        );
    }

    /// The table holds the Milky Way fixture's orbit averages, the broken power law and the
    /// galaxy's monopole, between its knots: against a quadrature of 4 × 16 panels of 32 nodes on
    /// the exact mass inside, at 300 orbits from 10⁻³ to 300 ly and e to 0.98: to 5 × 10⁻⁴ for
    /// a under 100 ly, and to 3 × 10⁻³ beyond, where an orbit of e above 0.93 runs from inside the
    /// break at 10 ly to beyond the taper at 100 ly.
    #[test]
    fn the_table_holds_the_profile_s_orbit_averages() {
        let motion = milky_way();
        let table = SecularTable::new(&motion);
        let pull = |r: f64| {
            GravitationalParameter::from_solar_masses(
                motion.stars_within(LightYears::new(r / METRES_PER_LIGHT_YEAR)),
            )
            .value()
                / (r * r)
        };
        let mut lcg = Lcg::new(0x0928_d000_0000_0002);
        let mut worst = [0.0_f64; 2];
        let mut inner = [0.0_f64; 2];
        for _ in 0..300 {
            let a = 1e-3 * math::exp10(5.5 * lcg.next_f64()) * METRES_PER_LIGHT_YEAR;
            let e = 0.98 * lcg.next_f64().sqrt();
            let scale = table.stars_at(a);
            let edges: Vec<f64> = (0..=64).map(|k| PI * f64::from(k) / 64.0).collect();
            let mut sums = [0.0; 2];
            for panel in edges.windows(2) {
                sums[0] += crate::galaxy::quad::gl32(
                    |x| {
                        let s = 1.0 - e * math::cos(x);
                        pull(a * s) * s * s
                    },
                    panel[0],
                    panel[1],
                );
                sums[1] += crate::galaxy::quad::gl32(
                    |x| {
                        let cos = math::cos(x);
                        pull(a * (1.0 - e * cos)) * (e - cos)
                    },
                    panel[0],
                    panel[1],
                );
            }
            let direct = [
                a * a * sums[0] / PI / scale,
                a * a * sums[1] / PI / (e * scale),
            ];
            let [alpha, beta, _] = table.averages_at(a, e);
            for (k, (t, d)) in [alpha, beta].into_iter().zip(direct).enumerate() {
                let error = (t / d - 1.0).abs();
                worst[k] = worst[k].max(error);
                if a < 100.0 * METRES_PER_LIGHT_YEAR {
                    inner[k] = inner[k].max(error);
                }
            }
        }
        println!(
            "Milky Way centre, worst relative errors (α★, β★): {worst:?}, inside 100 ly {inner:?}"
        );
        assert!(inner.iter().all(|&w| w < 5e-4), "{inner:?}");
        assert!(worst.iter().all(|&w| w < 3e-3), "{worst:?}");
    }

    /// A circular orbit at r₀ is exact about μ = G M(<r₀): its mean motion's correction cancels
    /// the stars' precession, so the secular step keeps its two-body motion but for the
    /// relativistic advance.
    #[test]
    fn a_circular_orbit_keeps_its_two_body_motion() {
        let motion = milky_way();
        let table = SecularTable::new(&motion);
        for r in [0.1, 1.0, 5.0] {
            let orbit = orbit_at(&motion, r, 1.0, 0.0);
            let rates = orbit.secular_rates(&table).expect("bound");
            let sum = rates.mass_precession().value() + rates.mean_motion_correction().value();
            let scale = rates.mass_precession().value().abs();
            assert!(sum.abs() < 1e-4 * scale, "{r} ly: {rates:?}");
            let newtonian = SecularRates {
                schwarzschild: 0.0,
                ..rates
            };
            let dt = Seconds::new(1_000.0 * YEAR);
            let two_body = orbit.propagate(dt).position;
            let secular = orbit.propagate_secular(dt, &newtonian).position;
            let gap = length([0, 1, 2].map(|i| secular[i] - two_body[i]));
            assert!(
                gap < 1e-6 * r * METRES_PER_LIGHT_YEAR,
                "{r} ly: {} ly apart",
                gap / METRES_PER_LIGHT_YEAR
            );
        }
    }

    /// Parabolic and unbound orbits take neither rate, and their secular step is the two-body
    /// propagation; at dt = 0 the step gives the orbit itself; and the same inputs give the same
    /// bits, table and all.
    #[test]
    fn unbound_orbits_take_neither_rate_and_the_step_repeats_to_the_bit() {
        let motion = milky_way();
        let table = SecularTable::new(&motion);
        assert_eq!(table, SecularTable::new(&motion));
        let bound = orbit_at(&motion, 0.5, 0.9, 0.4);
        let rates = bound.secular_rates(&table).expect("bound");
        for factor in [1.5, 2.0, 4.0] {
            let open = orbit_at(&motion, 0.5, factor, 0.4);
            assert_eq!(open.secular_rates(&table), None);
            assert_eq!(open.mass_precession_rate(&table), None);
            assert_eq!(
                open.schwarzschild_rate(GravitationalParameter::new(table.black_hole)),
                None
            );
            let dt = Seconds::new(300.0 * YEAR);
            assert_eq!(open.propagate_secular(dt, &rates), open.propagate(dt));
        }
        assert_eq!(bound.propagate_secular(Seconds::new(0.0), &rates), bound);
        for years in [-1_000.0, 10.0, 1_000.0] {
            let dt = Seconds::new(years * YEAR);
            let once = bound.propagate_secular(dt, &rates);
            let twice = bound.propagate_secular(dt, &bound.secular_rates(&table).expect("bound"));
            for i in 0..3 {
                assert_same_bits(once.position[i], twice.position[i]);
                assert_same_bits(once.velocity[i], twice.velocity[i]);
            }
            // The turn keeps the energy and the angular momentum's length.
            let later = bound.propagate(dt);
            let energy = |o: &KeplerOrbit| o.specific_energy_j_per_kg();
            assert!((energy(&once) / energy(&later) - 1.0).abs() < 1e-12);
        }
    }

    /// The acceleration `−G M(<r) r̂ ÷ r²` of the black hole and the stars inside r, m s⁻².
    fn acceleration(motion: &CentreMotion, x: [f64; 3]) -> [f64; 3] {
        let r = length(x);
        let gm = mu_at(motion, r / METRES_PER_LIGHT_YEAR);
        x.map(|c| -gm * c / (r * r * r))
    }

    /// The position `span` s after the state `(start, speed)` (m, m s⁻¹) in the smooth potential
    /// of the black hole and the stars (`motion`'s mass inside each radius), by the classical
    /// fourth-order Runge–Kutta method with steps of `eta` local dynamical times
    /// `√(r³ ÷ G M(<r))`.
    fn integrate(
        motion: &CentreMotion,
        start: [f64; 3],
        speed: [f64; 3],
        span: f64,
        eta: f64,
    ) -> [f64; 3] {
        let (mut position, mut velocity, mut done) = (start, speed, 0.0);
        let add = |a: [f64; 3], b: [f64; 3], step: f64| [0, 1, 2].map(|i| a[i] + step * b[i]);
        while (span - done).abs() > 0.0 {
            let radius = length(position);
            let dynamical =
                (radius * radius * radius / mu_at(motion, radius / METRES_PER_LIGHT_YEAR)).sqrt();
            let step = (eta * dynamical).min((span - done).abs()) * span.signum();
            let k1 = (velocity, acceleration(motion, position));
            let k2 = (
                add(velocity, k1.1, step / 2.0),
                acceleration(motion, add(position, k1.0, step / 2.0)),
            );
            let k3 = (
                add(velocity, k2.1, step / 2.0),
                acceleration(motion, add(position, k2.0, step / 2.0)),
            );
            let k4 = (
                add(velocity, k3.1, step),
                acceleration(motion, add(position, k3.0, step)),
            );
            position = [0, 1, 2].map(|i| {
                position[i] + step / 6.0 * (k1.0[i] + 2.0 * k2.0[i] + 2.0 * k3.0[i] + k4.0[i])
            });
            velocity = [0, 1, 2].map(|i| {
                velocity[i] + step / 6.0 * (k1.1[i] + 2.0 * k2.1[i] + 2.0 * k3.1[i] + k4.1[i])
            });
            done = if (span - done).abs() <= step.abs() {
                span
            } else {
                done + step
            };
        }
        position
    }

    /// The secular step's variants, for the plan's test: the stars' rates as built (the mean
    /// orbit's mean motion and the stars' shift of it), without the mean orbit's correction, the
    /// precession alone (the plan's "two rates × dt"), and none (the two-body law), each Newtonian.
    fn variants(orbit: &KeplerOrbit, table: &SecularTable) -> [SecularRates; 4] {
        let rates = orbit.secular_rates(table).expect("bound");
        let (mu, a) = (orbit.mu, 1.0 / orbit.alpha);
        let e = orbit.eccentricity();
        let n = orbit.mean_motion_si();
        let scale = table.stars_at(a);
        let [alpha, beta, _] = table.averages_at(a, e).map(|v| v * scale);
        let shift = n * (2.0 * (table.black_hole - mu) + 2.0 * alpha + (1.0 - e * e) * beta) / mu;
        let with = |mass_precession: f64, mean_motion: f64| SecularRates {
            schwarzschild: 0.0,
            mass_precession,
            mean_motion,
        };
        [
            with(rates.mass_precession, rates.mean_motion),
            with(rates.mass_precession, shift),
            with(rates.mass_precession, 0.0),
            with(0.0, 0.0),
        ]
    }

    /// The plan's test of the pair: against a direct integration in the smooth potential of the
    /// black hole and the stars (Newtonian, so the step takes the stars' rates alone), the
    /// position after ±1,000 yr from 0.1, 1 and 5 ly, on a circular orbit and four eccentric ones,
    /// each against a tenth of the orbit's tidal radius at pericentre for a solar mass
    /// (P09.T28.c).
    ///
    /// The bound is met only by the circular orbits, by the step as by the two-body law; the
    /// eccentric ones miss it (plan 09's Risks, "P09.T28.d.a as built"), and the misses are
    /// pinned as they stand. At 0.1 ly, where the span holds one to eight radial periods, the step
    /// beats the two-body law on every orbit; at 1 and 5 ly, where it holds a fraction of one, the
    /// two-body law beats the step on every eccentric orbit, since the rates are orbit averages and
    /// the arc is short.
    #[test]
    fn the_secular_step_against_a_direct_integration() {
        let motion = milky_way();
        let table = SecularTable::new(&motion);
        let cases = [(1.0, 0.0), (0.7, 0.0), (1.25, 0.0), (0.9, 0.9), (1.1, -1.0)];
        let names = ["as built", "no mean orbit", "precession alone", "two-body"];
        // The worst of each radius, in tenths of the tidal radius, as measured (2026-10-29, on
        // x86-64), pinned to 10%: the step as built, and the two-body law.
        let pinned = [
            (0.1, 65.37, 1804.6),
            (1.0, 3180.8, 379.56),
            (5.0, 1953.3, 1.092),
        ];
        for (r, pinned_built, pinned_two_body) in pinned {
            let mut worst = [0.0_f64; 4];
            for (factor, tilt) in cases {
                let orbit = orbit_at(&motion, r, factor, tilt);
                let bound = 0.1
                    * motion
                        .tidal_radius_at_pericentre(&orbit, SolarMasses::new(1.0))
                        .value();
                let steps = variants(&orbit, &table);
                for years in [-1_000.0, 1_000.0] {
                    let t = years * YEAR;
                    let truth = integrate(&motion, orbit.position, orbit.velocity, t, 2e-3);
                    let finer = integrate(&motion, orbit.position, orbit.velocity, t, 1e-3);
                    let off = |x: [f64; 3]| length([0, 1, 2].map(|i| x[i] - truth[i]));
                    assert!(off(finer) < 1e-3 * bound, "the integration converges");
                    let errors = steps.map(|rates| {
                        off(orbit.propagate_secular(Seconds::new(t), &rates).position) / bound
                    });
                    println!(
                        "r {r} ly, ×{factor} at {tilt} rad (e {:.3}, P {:.0} yr), {years} yr, \
                         bound {:.3e} ly: {}",
                        orbit.eccentricity(),
                        orbit.period().expect("bound").value() / YEAR,
                        bound / METRES_PER_LIGHT_YEAR,
                        names
                            .iter()
                            .zip(errors)
                            .map(|(name, x)| format!("{name} {x:.3}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    for (w, x) in worst.iter_mut().zip(errors) {
                        *w = w.max(x);
                    }
                    let [built, .., two_body] = errors;
                    if (factor - 1.0_f64).abs() < f64::EPSILON {
                        // A circular orbit is exact about μ, and the step keeps it so.
                        assert!(
                            built < 0.01 && two_body < 0.01,
                            "circular at {r} ly: {errors:?}"
                        );
                    } else if r < 0.5 {
                        assert!(built < two_body, "{r} ly, ×{factor}: {errors:?}");
                    } else {
                        assert!(two_body < built, "{r} ly, ×{factor}: {errors:?}");
                    }
                }
            }
            println!(
                "r {r} ly, worst over the orbits, in tenths of the tidal radius: {}",
                names
                    .iter()
                    .zip(worst)
                    .map(|(name, x)| format!("{name} {x:.3}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let [built, .., two_body] = worst;
            assert!((built / pinned_built - 1.0).abs() < 0.1, "{r} ly: {built}");
            assert!(
                (two_body / pinned_two_body - 1.0).abs() < 0.1,
                "{r} ly: {two_body}"
            );
        }
    }
}
