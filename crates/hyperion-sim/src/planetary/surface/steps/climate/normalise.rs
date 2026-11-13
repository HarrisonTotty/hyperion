//! The normalisation (R09.T13.d; decision-r09-t13d-normalisation §2): plan 14's mean and signed
//! contrasts imposed on a model's field as plan 14 defines them, by corrections of fixed shape.
//!
//! # The figures and their kinds
//!
//! - **The equator–pole contrast** on the annual field's equator less its poles' mean,
//!   T(0°) − ½[T(90° N) + T(90° S)] ([`CoarseInputs::equator_pole_contrast_k`]), T(0°) the mean
//!   along the geographic equator.
//! - **A world locked 1:1's day–night contrast** ([`Forcing::Locked`] only) on its μ-weighted day
//!   and night about the substellar axis, D − N with D = Σ_{μ>0} Ω μ X ÷ Σ_{μ>0} Ω μ and N its
//!   |μ|-weighted twin over the night, μ = cos γ: Cowan and Agol's (2011, ApJ 729, 54, eqs. 4–5)
//!   "equator-weighted" hemispheres, as plan 14's `cold_area` documents them.
//! - **The mean** ([`CoarseInputs::mean_surface_temperature`]).
//!
//! Each is read in the model's kind of mean (§2.2): **arithmetic** on the energy-balance and
//! isothermal models, whose figures plan 14 calibrates on measured arithmetic means; on the
//! **radiative** model the brightness of the emitted flux F = εσ⟨T⁴⟩, (max(0, F − `E_nf`) ÷ σ)^¼
//! for the mean and the ends, `E_nf` the night floor's flux (plan 14 states both for absorbed
//! starlight alone), and (F ÷ σ)^¼ whole for the day and the night (plan 14's night is the floor).
//!
//! # The correction
//!
//! s = c + a P₂(sin φ) + b P₁(cos γ), b on a world locked 1:1 only, added to every sample and every
//! month of a grid point, so that each record moves by s and the months' anomalies stay (§2.4).
//! Its shape is fixed, never the field's own scaled, which is undefined on an isothermal field and
//! unbounded where a contrast changes sign (§2.5).
//!
//! - **Arithmetic** ([`arithmetic`]): a and b by the exact linear solve of the two conditions on
//!   the annual sea-level field, E(T̄ + s) = ΔT and DN(T̄ + s) = `ΔT_dn`, with E and DN applied to the
//!   shapes as the interpolant reads them, so that the conditions hold to rounding. c is then the
//!   input mean less the cells' area-weighted surface mean after the lapse term, added last
//!   (the step's pipeline, Design note 8).
//! - **Radiative** ([`radiative`]): no lapse term, so c, a and b are solved together on the
//!   converged orbit's moments, ⟨(T + s)⁴⟩ = Σₖ C(4, k)(R + s)⁴⁻ᵏ Mₖ, by Gauss–Seidel sweeps
//!   c → a → b, each the bracketed Newton of decision-p14-t51a-part2 §1.3 (the analytic slope, a
//!   midpoint where a step leaves the bracket, a fixed cap), until every residual is under
//!   [`RESIDUAL_TOLERANCE_K`].
//!
//! # The guard
//!
//! After the correction every grid point's records are held at least the model's lower bound (the
//! night floor, or the frost point the model holds the surface at), and the step clamps every
//! interpolated value to its stencil's range ([`floor_records`]; §2.4).

use super::airless::{Moments, RadiativeSolution};
use super::coordinates::{Coordinates, substellar_cosine};
use super::grid::{GridClimate, ModelGrid, index};
use crate::math;
use crate::planetary::surface::inputs::{CoarseInputs, Forcing};
use crate::units::consts::STEFAN_BOLTZMANN;

/// The residual under which the radiative solve stops, K: 10⁻⁹ (decision-r09-t13d-normalisation
/// §2.4).
pub(super) const RESIDUAL_TOLERANCE_K: f64 = 1e-9;

/// The most Gauss–Seidel sweeps of the radiative solve: 100 (`Adopted`), against the handful the
/// weak coupling of its three conditions takes (the reference Moon's and Mars's take under ten as
/// built).
const MAX_SWEEPS: u32 = 100;

/// The most iterations of one bracketed Newton solve: 200 (`Adopted`), past the 64 bisections
/// that halve any bracket of the field's to the last bit.
const MAX_NEWTON_STEPS: u32 = 200;

/// The points round the geographic equator at which a field on the tidally locked grid is read
/// for its zonal mean there: 144, every 2.5° from the substellar point, the locked grid's own
/// spacing and the radiative model's end ring's ([`EQUATOR_RING_COLUMNS`](super::airless::EQUATOR_RING_COLUMNS)).
const EQUATOR_RING: usize = 144;

/// P₂(x) = (3x² − 1) ÷ 2, the second Legendre polynomial.
#[must_use]
pub(super) fn legendre_2(x: f64) -> f64 {
    0.5 * (3.0 * x * x - 1.0)
}

/// The correction a model's field took, K: s = c + a P₂(sin φ) + b P₁(cos γ) (the
/// [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[expect(
    clippy::struct_field_names,
    reason = "each is a temperature in kelvin, its unit in its name as the crate's rules ask"
)]
pub(super) struct Correction {
    /// c, the constant.
    pub(super) mean_k: f64,
    /// a, the coefficient of P₂(sin φ).
    pub(super) equator_pole_k: f64,
    /// b, the coefficient of P₁(cos γ), zero off a world locked 1:1.
    pub(super) day_night_k: f64,
}

impl Correction {
    /// The shape terms a P₂(sin φ) + b cos γ at the body-frame unit vector `direction`, K: the
    /// correction without its constant.
    #[must_use]
    pub(super) fn shape_at(&self, direction: [f64; 3]) -> f64 {
        self.equator_pole_k * legendre_2(direction[2])
            + self.day_night_k * substellar_cosine(direction)
    }

    /// The whole correction s at the body-frame unit vector `direction`, K.
    #[must_use]
    pub(super) fn at(&self, direction: [f64; 3]) -> f64 {
        self.mean_k + self.shape_at(direction)
    }
}

/// The body-frame unit vector of each point of `grid` in `coordinates`.
#[must_use]
fn body_directions(grid: &ModelGrid, coordinates: Coordinates) -> Vec<[f64; 3]> {
    (0..grid.points())
        .map(|p| coordinates.to_body(grid.centre_direction(p)))
        .collect()
}

/// The measures of §2.3 on a field of a grid, read as the cells' interpolant reads it.
#[derive(Debug, Clone, PartialEq)]
struct Measures<'a> {
    grid: &'a ModelGrid,
    coordinates: Coordinates,
    directions: Vec<[f64; 3]>,
}

impl<'a> Measures<'a> {
    /// The measures of a field on `grid` in `coordinates`.
    #[must_use]
    fn new(grid: &'a ModelGrid, coordinates: Coordinates) -> Self {
        Self {
            grid,
            coordinates,
            directions: body_directions(grid, coordinates),
        }
    }

    /// E(X), the field `field`'s equator less its poles' mean, K: on a grid in geographic
    /// coordinates the latitude cubic of its band means, the zonal mean of the bicubic
    /// interpolant exactly, since Catmull and Rom's kernel integrates to one round a band; in
    /// tidally locked coordinates, whose bands are not the geographic latitudes, the interpolant's
    /// mean over [`EQUATOR_RING`] points round the geographic equator, and its value at each pole.
    #[must_use]
    fn ends(&self, field: &[f64]) -> f64 {
        let (equator, north, south) = match self.coordinates {
            Coordinates::Geographic => {
                let zonal = ModelGrid::zonal();
                let bands = self.grid.band_means(field);
                let at = |z: f64| {
                    let c = (1.0 - z * z).max(0.0).sqrt();
                    zonal.stencil_at([c, 0.0, z]).apply(&bands)
                };
                (at(0.0), at(1.0), at(-1.0))
            }
            Coordinates::TidallyLocked => {
                let at = |d: [f64; 3]| {
                    self.grid
                        .stencil_at(self.coordinates.to_model(d))
                        .apply(field)
                };
                let ring = (0..EQUATOR_RING).fold(0.0, |sum, m| {
                    let (s, c) =
                        math::sin_cos(core::f64::consts::TAU * index(m) / index(EQUATOR_RING));
                    sum + at([c, s, 0.0])
                }) / index(EQUATOR_RING);
                (ring, at([0.0, 0.0, 1.0]), at([0.0, 0.0, -1.0]))
            }
        };
        equator - f64::midpoint(north, south)
    }

    /// DN(X), the field `field`'s μ-weighted day less its |μ|-weighted night over the grid's
    /// points, each of its solid angle, μ the cosine from the substellar point, K.
    #[must_use]
    fn day_night(&self, field: &[f64]) -> f64 {
        let (day, night) = hemispheres(self.grid, &self.directions, field);
        day - night
    }
}

/// The μ-weighted means of `field` over the day (μ > 0) and the night (μ < 0) of `grid`'s points
/// at the body-frame `directions`, each weighted by its solid angle and |μ|.
#[must_use]
fn hemispheres(grid: &ModelGrid, directions: &[[f64; 3]], field: &[f64]) -> (f64, f64) {
    let columns = grid.columns();
    let (mut day, mut day_weight, mut night, mut night_weight) = (0.0, 0.0, 0.0, 0.0);
    for (p, (direction, &x)) in directions.iter().zip(field).enumerate() {
        let mu = substellar_cosine(*direction);
        let w = grid.weight(p / columns) * mu.abs();
        if mu > 0.0 {
            day += w * x;
            day_weight += w;
        } else if mu < 0.0 {
            night += w * x;
            night_weight += w;
        }
    }
    let mean = |sum: f64, weight: f64| if weight > 0.0 { sum / weight } else { 0.0 };
    (mean(day, day_weight), mean(night, night_weight))
}

/// The arithmetic models' corrections on the annual sea-level field `annual_k` of `grid` in
/// `coordinates`, for the body of `inputs` (the [module documentation](self)): a, and b on a world
/// locked 1:1, from the exact linear solve of
///
/// ```text
/// [ΔT_ep; ΔT_dn] = [E(T̄); DN(T̄)] + [[E(P₂), E(cos γ)]; [DN(P₂), DN(cos γ)]] [a; b]
/// ```
///
/// (decision-r09-t13d-normalisation §2.4), the single condition in a off a locked world.
/// Analytically E(P₂) = −3/2, E(cos γ) = 0, DN(P₂) = 0 and DN(cos γ) = 4/3, so the system takes up
/// only the grid's leakage. c is zero here: the step adds it after the lapse term.
#[must_use]
pub(super) fn arithmetic(
    inputs: &CoarseInputs,
    grid: &ModelGrid,
    coordinates: Coordinates,
    annual_k: &[f64],
) -> Correction {
    let measures = Measures::new(grid, coordinates);
    let p2: Vec<f64> = measures
        .directions
        .iter()
        .map(|d| legendre_2(d[2]))
        .collect();
    let want_ends = inputs.equator_pole_contrast_k() - measures.ends(annual_k);
    let ends_p2 = measures.ends(&p2);
    if inputs.climate().forcing != Forcing::Locked {
        return Correction {
            equator_pole_k: want_ends / ends_p2,
            ..Correction::default()
        };
    }
    let mu: Vec<f64> = measures
        .directions
        .iter()
        .map(|d| substellar_cosine(*d))
        .collect();
    let want_day_night = inputs.day_night_contrast_k() - measures.day_night(annual_k);
    let (ends_mu, day_night_p2, day_night_mu) = (
        measures.ends(&mu),
        measures.day_night(&p2),
        measures.day_night(&mu),
    );
    let determinant = ends_p2 * day_night_mu - ends_mu * day_night_p2;
    Correction {
        mean_k: 0.0,
        equator_pole_k: (want_ends * day_night_mu - ends_mu * want_day_night) / determinant,
        day_night_k: (ends_p2 * want_day_night - day_night_p2 * want_ends) / determinant,
    }
}

/// Adds `correction` to every record of `climate` on `grid` in `coordinates`, at each point's
/// centre, the annual mean and every month alike.
pub(super) fn apply(
    climate: &mut GridClimate,
    grid: &ModelGrid,
    coordinates: Coordinates,
    correction: &Correction,
) {
    let shifts: Vec<f64> = body_directions(grid, coordinates)
        .iter()
        .map(|d| correction.at(*d))
        .collect();
    for field in core::iter::once(&mut climate.annual_k).chain(climate.months_k.iter_mut()) {
        for (value, shift) in field.iter_mut().zip(&shifts) {
            *value += shift;
        }
    }
}

/// Holds every record of `climate` at least `lower_bound_k`, the model's lower bound (the guard of
/// decision-r09-t13d-normalisation §2.4): the number of records it raised, a diagnostic that is no
/// output.
pub(super) fn floor_records(climate: &mut GridClimate, lower_bound_k: f64) -> u32 {
    let mut raised = 0_u32;
    for value in climate
        .annual_k
        .iter_mut()
        .chain(climate.months_k.iter_mut().flatten())
    {
        if *value < lower_bound_k {
            *value = lower_bound_k;
            raised = raised.saturating_add(1);
        }
    }
    raised
}

/// One column the radiative solve reads: its shapes' values, its weight in the grid's means (zero
/// for an end column) and its moments.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Site {
    p2: f64,
    mu: f64,
    weight: f64,
    moments: Moments,
}

impl Site {
    /// The site's correction at (c, a, b), K.
    #[must_use]
    fn shift(&self, [c, a, b]: [f64; 3]) -> f64 {
        c + a * self.p2 + b * self.mu
    }

    /// The emitted flux F = εσ⟨(T + s)⁴⟩ at the shift `s`, W m⁻², and its slope in s, εσ 4⟨(T + s)³⟩.
    #[must_use]
    fn flux(&self, emissivity: f64, s: f64) -> (f64, f64) {
        let scale = emissivity * STEFAN_BOLTZMANN;
        (
            scale * self.moments.fourth_power_mean(s),
            4.0 * scale * self.moments.third_power_mean(s),
        )
    }
}

/// The radiative solve's columns and constants.
#[derive(Debug, Clone, PartialEq)]
struct Radiative {
    points: Vec<Site>,
    equator: Vec<Site>,
    north: Site,
    south: Site,
    emissivity: f64,
    floor_flux_w_m2: f64,
    /// The input's mean, equator–pole and day–night contrasts, K.
    targets_k: [f64; 3],
    locked: bool,
}

/// The brightness temperature of the flux `flux_w_m2` less the night floor's `floor_w_m2`,
/// (max(0, F − `E_nf`) ÷ σ)^¼, K, and its slope in F, K W⁻¹ m²: zero where the flux is at or under
/// the floor's.
#[must_use]
fn brightness(flux_w_m2: f64, floor_w_m2: f64) -> (f64, f64) {
    let excess = flux_w_m2 - floor_w_m2;
    if excess > 0.0 {
        let b = (excess / STEFAN_BOLTZMANN).sqrt().sqrt();
        (b, 0.25 * b / excess)
    } else {
        (0.0, 0.0)
    }
}

impl Radiative {
    /// The radiative solve's sites for the body of `inputs` from its model's answer `solution`.
    #[must_use]
    fn of(inputs: &CoarseInputs, solution: &RadiativeSolution) -> Self {
        let grid = &solution.grid;
        let site = |direction: [f64; 3], weight: f64, moments: Moments| Site {
            p2: legendre_2(direction[2]),
            mu: substellar_cosine(direction),
            weight,
            moments,
        };
        let points = body_directions(grid, solution.coordinates)
            .into_iter()
            .zip(&solution.moments)
            .enumerate()
            .map(|(p, (d, m))| site(d, grid.weight(p / grid.columns()), *m))
            .collect();
        let mut equator = Vec::new();
        let (mut north, mut south) = (None, None);
        for end in &solution.ends {
            let s = site(end.direction, 0.0, end.moments);
            match end.end {
                super::airless::End::Equator => equator.push(s),
                super::airless::End::NorthPole => north = Some(s),
                super::airless::End::SouthPole => south = Some(s),
            }
        }
        Self {
            points,
            equator,
            north: north.expect("the radiative model carries an end column at each pole"),
            south: south.expect("the radiative model carries an end column at each pole"),
            emissivity: solution.emissivity,
            floor_flux_w_m2: solution.floor_flux_w_m2,
            targets_k: [
                inputs.mean_surface_temperature().value(),
                inputs.equator_pole_contrast_k(),
                inputs.day_night_contrast_k(),
            ],
            locked: inputs.climate().forcing == Forcing::Locked,
        }
    }

    /// Every site, the grid's points and then the end columns.
    fn sites(&self) -> impl Iterator<Item = &Site> {
        self.points
            .iter()
            .chain(&self.equator)
            .chain([&self.north, &self.south])
    }

    /// The mean's residual at `x` = (c, a, b), the brightness mean less the input's, K, and its
    /// slope in c.
    #[must_use]
    fn mean(&self, x: [f64; 3]) -> (f64, f64) {
        let (mut sum, mut slope, mut weight) = (0.0, 0.0, 0.0);
        for site in &self.points {
            let (f, df) = site.flux(self.emissivity, site.shift(x));
            sum += site.weight * (f - self.floor_flux_w_m2);
            slope += site.weight * df;
            weight += site.weight;
        }
        let (b, db) = brightness(sum / weight, 0.0);
        (b - self.targets_k[0], db * slope / weight)
    }

    /// The ends' residual at `x`, the equator's brightness less the poles' mean, less the input's
    /// contrast, K, and its slope in a; the equator's flux averaged along it before the root.
    #[must_use]
    fn ends(&self, x: [f64; 3]) -> (f64, f64) {
        let n = index(self.equator.len());
        let (flux, slope) = self.equator.iter().fold((0.0, 0.0), |(f, d), site| {
            let (sf, sd) = site.flux(self.emissivity, site.shift(x));
            (f + sf / n, d + sd * site.p2 / n)
        });
        let (equator, equator_slope) = brightness(flux, self.floor_flux_w_m2);
        let pole = |site: &Site| {
            let (f, df) = site.flux(self.emissivity, site.shift(x));
            let (b, db) = brightness(f, self.floor_flux_w_m2);
            (b, db * df * site.p2)
        };
        let ((north, north_slope), (south, south_slope)) = (pole(&self.north), pole(&self.south));
        (
            equator - f64::midpoint(north, south) - self.targets_k[1],
            equator_slope * slope - f64::midpoint(north_slope, south_slope),
        )
    }

    /// The day–night's residual at `x`, the μ-weighted day's brightness of its whole flux less the
    /// night's, less the input's contrast, K, and its slope in b.
    #[must_use]
    fn day_night(&self, x: [f64; 3]) -> (f64, f64) {
        let mut sums = [0.0_f64; 6];
        for site in &self.points {
            let (f, df) = site.flux(self.emissivity, site.shift(x));
            let w = site.weight * site.mu.abs();
            let base = if site.mu > 0.0 {
                0
            } else if site.mu < 0.0 {
                3
            } else {
                continue;
            };
            sums[base] += w * f;
            sums[base + 1] += w * df * site.mu;
            sums[base + 2] += w;
        }
        let side = |base: usize| {
            if sums[base + 2] > 0.0 {
                let (b, db) = brightness(sums[base] / sums[base + 2], 0.0);
                (b, db * sums[base + 1] / sums[base + 2])
            } else {
                (0.0, 0.0)
            }
        };
        let ((day, day_slope), (night, night_slope)) = (side(0), side(3));
        (day - night - self.targets_k[2], day_slope - night_slope)
    }

    /// The physical range of unknown `k` (0 c, 1 a, 2 b) with the others at `x`: where every
    /// sample of every site stays at or above 0 K, on which each condition is monotone.
    #[must_use]
    fn range(&self, x: [f64; 3], k: usize) -> (f64, f64) {
        let (mut low, mut high) = (f64::NEG_INFINITY, f64::INFINITY);
        for site in self.sites() {
            let coefficient = [1.0, site.p2, site.mu][k];
            let mut others = x;
            others[k] = 0.0;
            let room = -site.moments.coldest_k - site.shift(others);
            if coefficient > 0.0 {
                low = low.max(room / coefficient);
            } else if coefficient < 0.0 {
                high = high.min(room / coefficient);
            }
        }
        (low, high)
    }

    /// The flux every sample must reach for the brightness mean to reach the input's, as a
    /// temperature: ((σT*⁴ + `E_nf`) ÷ εσ)^¼, K.
    #[must_use]
    fn sufficient_k(&self) -> f64 {
        let t = self.targets_k[0];
        ((STEFAN_BOLTZMANN * t * t * t * t + self.floor_flux_w_m2)
            / (self.emissivity * STEFAN_BOLTZMANN))
            .sqrt()
            .sqrt()
    }
}

/// Which way a residual runs with its unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Slope {
    /// It rises.
    Rising,
    /// It falls.
    Falling,
}

/// The root of a residual monotone on the bracket (`low`, `high`), running as `slope` says, its
/// value and slope from `residual`, by the bracketed Newton of decision-p14-t51a-part2 §1.3 from
/// `start`: a step that leaves the bracket takes its midpoint, a slope that is not positive
/// likewise, at most [`MAX_NEWTON_STEPS`], stopping once the residual is under
/// [`RESIDUAL_TOLERANCE_K`]. Where the residual does not change sign over the bracket the nearer
/// end is the answer.
#[must_use]
fn bracketed_newton(
    residual: impl Fn(f64) -> (f64, f64),
    (mut low, mut high): (f64, f64),
    start: f64,
    slope: Slope,
) -> f64 {
    let sign = match slope {
        Slope::Rising => 1.0,
        Slope::Falling => -1.0,
    };
    let rising = |at: f64| {
        let (value, gradient) = residual(at);
        (sign * value, sign * gradient)
    };
    if rising(low).0 >= 0.0 {
        return low;
    }
    if rising(high).0 <= 0.0 {
        return high;
    }
    let mut at = if start > low && start < high {
        start
    } else {
        f64::midpoint(low, high)
    };
    for _ in 0..MAX_NEWTON_STEPS {
        let (value, gradient) = rising(at);
        if value.abs() < RESIDUAL_TOLERANCE_K {
            return at;
        }
        if value < 0.0 {
            low = at;
        } else {
            high = at;
        }
        let next = at - value / gradient;
        // By comparison, so that a step that is not a number takes the midpoint.
        at = if gradient > 0.0 && next > low && next < high {
            next
        } else {
            f64::midpoint(low, high)
        };
        let width = high - low;
        if width.is_nan() || width <= 0.0 {
            break;
        }
    }
    at
}

/// The radiative model's correction (c, a, b) for the body of `inputs` from its answer
/// `solution` (the [module documentation](self)), with the conditions' residuals left, K (the
/// mean's, the ends', the day–night's, zero off a locked world).
///
/// A one-dimensional bracket that does not hold its root, an input no field in the physical
/// range meets, leaves its residual: the guard then holds the records.
#[must_use]
pub(super) fn radiative(
    inputs: &CoarseInputs,
    solution: &RadiativeSolution,
) -> (Correction, [f64; 3]) {
    let model = Radiative::of(inputs, solution);
    let mut x = [0.0_f64; 3];
    let unknowns = if model.locked { 3 } else { 2 };
    let mut residuals = [0.0; 3];
    for _ in 0..MAX_SWEEPS {
        for k in 0..unknowns {
            let (low, high) = model.range(x, k);
            let span = model.targets_k[0] + 1_000.0;
            let low = if low.is_finite() { low } else { x[k] - span };
            let high = if high.is_finite() {
                high
            } else if k == 0 {
                low + model.sufficient_k()
            } else {
                x[k] + span
            };
            let at = |v: f64| {
                let mut y = x;
                y[k] = v;
                y
            };
            x[k] = match k {
                0 => bracketed_newton(|v| model.mean(at(v)), (low, high), x[0], Slope::Rising),
                1 => bracketed_newton(|v| model.ends(at(v)), (low, high), x[1], Slope::Falling),
                _ => bracketed_newton(|v| model.day_night(at(v)), (low, high), x[2], Slope::Rising),
            };
        }
        residuals = [
            model.mean(x).0,
            model.ends(x).0,
            if model.locked {
                model.day_night(x).0
            } else {
                0.0
            },
        ];
        if residuals.iter().all(|r| r.abs() < RESIDUAL_TOLERANCE_K) {
            break;
        }
    }
    (
        Correction {
            mean_k: x[0],
            equator_pole_k: x[1],
            day_night_k: x[2],
        },
        residuals,
    )
}

/// Each point of the radiative model's grid's corrected ice key, ⟨(T + s)⁴⟩^¼ over the converged
/// orbit at its correction s, K: the annual radiative mean (decision-r09-t13d-normalisation
/// §1.2), its emissivity left out, since it does not change the order.
#[must_use]
pub(super) fn radiative_keys(solution: &RadiativeSolution, correction: &Correction) -> Vec<f64> {
    body_directions(&solution.grid, solution.coordinates)
        .iter()
        .zip(&solution.moments)
        .map(|(d, m)| {
            let fourth = m.fourth_power_mean(correction.at(*d));
            // By comparison, so that a NaN reaches the keys' finiteness check.
            let fourth = if fourth < 0.0 { 0.0 } else { fourth };
            fourth.sqrt().sqrt()
        })
        .collect()
}

#[cfg(test)]
pub(super) mod measure {
    //! The measures as the tests read them.
    use super::*;

    /// E of `field` on `grid` in `coordinates`.
    #[must_use]
    pub(in super::super) fn ends(grid: &ModelGrid, coordinates: Coordinates, field: &[f64]) -> f64 {
        Measures::new(grid, coordinates).ends(field)
    }

    /// DN of `field` on `grid` in `coordinates`.
    #[must_use]
    pub(in super::super) fn day_night(
        grid: &ModelGrid,
        coordinates: Coordinates,
        field: &[f64],
    ) -> f64 {
        Measures::new(grid, coordinates).day_night(field)
    }

    /// The radiative brightness mean, ends and day–night of `solution` at `correction`, K, from
    /// its moments.
    #[must_use]
    pub(in super::super) fn radiative(
        inputs: &CoarseInputs,
        solution: &RadiativeSolution,
        correction: &Correction,
    ) -> [f64; 3] {
        let model = Radiative::of(inputs, solution);
        let x = [
            correction.mean_k,
            correction.equator_pole_k,
            correction.day_night_k,
        ];
        [
            model.mean(x).0 + model.targets_k[0],
            model.ends(x).0 + model.targets_k[1],
            model.day_night(x).0 + model.targets_k[2],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bracketed Newton finds a root of a rising and a falling residual, one with an infinite
    /// slope at the bracket's end (a dark pole's brightness), and answers the nearer end where the
    /// bracket holds no root.
    #[test]
    fn the_bracketed_newton_finds_its_root_or_the_nearer_end() {
        let root = bracketed_newton(
            |x| (x * x * x - 8.0, 3.0 * x * x),
            (0.0, 10.0),
            1.0,
            Slope::Rising,
        );
        assert!((root - 2.0).abs() < 1e-9, "{root}");
        let falling = bracketed_newton(|x| (5.0 - x, -1.0), (-100.0, 100.0), 0.0, Slope::Falling);
        assert!((falling - 5.0).abs() < 1e-9, "{falling}");
        let dark = bracketed_newton(
            |x: f64| {
                let (b, db) = brightness(x, 0.0);
                (b - 3.0, db)
            },
            (0.0, 1e4),
            0.0,
            Slope::Rising,
        );
        let expected = STEFAN_BOLTZMANN * 81.0;
        assert!((dark / expected - 1.0).abs() < 1e-9, "{dark}");
        let below = bracketed_newton(|x| (x + 5.0, 1.0), (0.0, 1.0), 0.5, Slope::Rising);
        let above = bracketed_newton(|x| (x - 5.0, 1.0), (0.0, 1.0), 0.5, Slope::Rising);
        assert!(below.abs() < 1e-15 && (above - 1.0).abs() < 1e-15);
    }
}
