//! The potential reduced to tables once per galaxy (plan 02, P02.T6.d and Design notes 7 and 17).

use super::model::MassModel;
use super::nfw::Nfw;
use super::spherical::{BrokenPowerLaw, PointMass, SphericalMass};
use crate::galaxy::PointLy;
use crate::galaxy::consts::{G, LIGHT_YEARS_PER_YEAR_PER_KM_S};
use crate::math;
use crate::units::{KilometresPerSecond, LightYears, Metres, PerYear, SolarMasses};

/// Points per axis of the grids.
const POINTS: usize = 64;

/// `ln` of the grids' first point, 2⁻⁴ ly.
const LN_FIRST: f64 = -4.0 * core::f64::consts::LN_2;

/// `ln` of the grids' last point, 2¹⁸ ly.
const LN_LAST: f64 = 18.0 * core::f64::consts::LN_2;

/// The grids' first and last points, ly.
const FIRST: f64 = 0.0625;
const LAST: f64 = 262_144.0;

/// The step between grid points in `ln R` (and `ln |z|`): 22 ln 2 ÷ 63.
const STEP: f64 = (LN_LAST - LN_FIRST) / 63.0;

/// Where the denominator of the tidal radius is floored, as a fraction of Ω² (plan 02, Design
/// note 17; brainstorm, "Coordinates").
const TIDAL_FLOOR: f64 = 0.05;

/// The grid point `i` of either axis, ly: `2⁻⁴ × 2^(22 i ÷ 63)`, with both ends exact.
fn grid_point(i: usize) -> f64 {
    match i {
        0 => FIRST,
        63 => LAST,
        _ => math::exp(LN_FIRST + STEP * f64::from(u8::try_from(i).expect("below 64"))),
    }
}

/// The cell of the grid holding `ln x` for `x` inside the grid, and the position in it, 0–1.
fn cell(x: f64) -> (usize, f64) {
    let position = ((math::ln(x) - LN_FIRST) / STEP).clamp(0.0, 62.999_999_999);
    let index = position.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number clamped to 0–62"
    )]
    let i = index as usize;
    (i, position - index)
}

/// The cubic Hermite basis at `t`: the weights of the two values and of the two slopes (in units
/// of the cell).
fn hermite(t: f64) -> [f64; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        2.0 * t3 - 3.0 * t2 + 1.0,
        -2.0 * t3 + 3.0 * t2,
        t3 - 2.0 * t2 + t,
        t3 - t2,
    ]
}

/// Cubic Hermite interpolation in `ln x` between grid points `i` and `i + 1`, with values `y`
/// and their derivatives in `ln x`, `dy`.
fn interpolate(y: &[f64; POINTS], dy: &[f64; POINTS], i: usize, t: f64) -> f64 {
    let [h00, h01, h10, h11] = hermite(t);
    h00 * y[i] + h01 * y[i + 1] + STEP * (h10 * dy[i] + h11 * dy[i + 1])
}

/// The Gaussians' in-plane quantities at the radial grid points.
#[derive(Debug, Clone, PartialEq)]
struct InPlaneTable {
    /// `v_c²`, (km/s)².
    v_circ_sq: [f64; POINTS],
    /// `d v_c² ÷ d ln R`, (km/s)².
    slope: [f64; POINTS],
    /// `d² v_c² ÷ d (ln R)²`, (km/s)².
    curvature: [f64; POINTS],
    /// `Φ(R, 0)`, (km/s)².
    potential: [f64; POINTS],
}

/// The Gaussians' part at one radius: `v_c²`, `d v_c² ÷ d ln R` and `Φ(R, 0)`.
#[derive(Debug, Clone, Copy)]
struct Extended {
    v_circ_sq: f64,
    slope: f64,
    potential: f64,
}

impl InPlaneTable {
    fn new(model: &MassModel) -> Self {
        let mut table = Self {
            v_circ_sq: [0.0; POINTS],
            slope: [0.0; POINTS],
            curvature: [0.0; POINTS],
            potential: [0.0; POINTS],
        };
        for i in 0..POINTS {
            let at = model.extended_in_plane(grid_point(i));
            table.v_circ_sq[i] = at.v_circ_sq;
            table.slope[i] = at.slope;
            table.curvature[i] = at.curvature;
            table.potential[i] = at.potential;
        }
        table
    }

    /// The Gaussians' part at radius `r`: interpolated inside the grid, solid-body below it and
    /// Keplerian above it.
    fn at(&self, r: f64) -> Extended {
        if r < FIRST {
            // Solid body: v_c² ∝ R², so d v_c² ÷ d ln R = 2 v_c², and Φ rises as ½ Ω² R².
            let omega_sq = self.v_circ_sq[0] / (FIRST * FIRST);
            return Extended {
                v_circ_sq: omega_sq * r * r,
                slope: 2.0 * omega_sq * r * r,
                potential: self.potential[0] + 0.5 * omega_sq * (r * r - FIRST * FIRST),
            };
        }
        if r > LAST {
            // A point mass: v_c² and Φ fall as 1 ÷ R from their values at the edge.
            let scale = LAST / r;
            let v_circ_sq = self.v_circ_sq[POINTS - 1] * scale;
            return Extended {
                v_circ_sq,
                slope: -v_circ_sq,
                potential: self.potential[POINTS - 1] * scale,
            };
        }
        let (i, t) = cell(r);
        Extended {
            v_circ_sq: interpolate(&self.v_circ_sq, &self.slope, i, t),
            slope: interpolate(&self.slope, &self.curvature, i, t),
            // dΦ ÷ d ln R in the plane is v_c².
            potential: interpolate(&self.potential, &self.v_circ_sq, i, t),
        }
    }
}

/// The Gaussians' potential on the (R, |z|) grid, with its derivatives in `ln R` and `ln |z|`
/// for bicubic Hermite interpolation.
#[derive(Debug, Clone, PartialEq)]
struct Grid {
    /// `[Φ, R ∂Φ ÷ ∂R, z ∂Φ ÷ ∂z, R z ∂²Φ ÷ ∂R ∂z]` at `(R_i, z_j)`, row `i`, column `j`.
    points: Vec<[f64; 4]>,
}

impl Grid {
    fn new(model: &MassModel) -> Self {
        let mut points = Vec::with_capacity(POINTS * POINTS);
        for i in 0..POINTS {
            for j in 0..POINTS {
                points.push(model.extended_grid_point(grid_point(i), grid_point(j)));
            }
        }
        Self { points }
    }

    /// The Gaussians' potential at a point inside the grid's range, bicubic in the logarithms.
    fn at(&self, r_cyl: f64, height: f64) -> f64 {
        let (row, across) = cell(r_cyl);
        let (column, up) = cell(height);
        let [r0, r1, dr0, dr1] = hermite(across);
        let [z0, z1, dz0, dz1] = hermite(up);
        let (value_r, slope_r) = ([r0, r1], [dr0, dr1]);
        let (value_z, slope_z) = ([z0, z1], [dz0, dz1]);
        let mut sum = 0.0;
        for di in 0..2 {
            for dj in 0..2 {
                let [phi, by_r, by_z, by_both] = self.points[(row + di) * POINTS + column + dj];
                sum += value_r[di] * value_z[dj] * phi
                    + STEP * (slope_r[di] * value_z[dj] * by_r + value_r[di] * slope_z[dj] * by_z)
                    + STEP * STEP * slope_r[di] * slope_z[dj] * by_both;
            }
        }
        sum
    }

    /// The Gaussians' `∂Φ ÷ ∂ln R` and `∂Φ ÷ ∂ln |z|` at a point inside the grid's range: the
    /// derivatives of the bicubic interpolant [`at`](Self::at) reads, which equal the stored exact
    /// derivatives at the grid points.
    fn log_derivatives(&self, r_cyl: f64, height: f64) -> [f64; 2] {
        let (row, across) = cell(r_cyl);
        let (column, up) = cell(height);
        let radial = hermite(across);
        let vertical = hermite(up);
        let radial_rate = hermite_slope(across);
        let vertical_rate = hermite_slope(up);
        // Basis weights of the value and of the slope, along each axis: `[value, slope]` pairs.
        let weights = |basis: [f64; 4], k: usize| [basis[k], basis[k + 2]];
        let (mut by_ln_r, mut by_ln_z) = (0.0, 0.0);
        for di in 0..2 {
            for dj in 0..2 {
                let node = self.points[(row + di) * POINTS + column + dj];
                let term = |[r_value, r_slope]: [f64; 2], [z_value, z_slope]: [f64; 2]| {
                    let [phi, by_r, by_z, by_both] = node;
                    r_value * z_value * phi
                        + STEP * (r_slope * z_value * by_r + r_value * z_slope * by_z)
                        + STEP * STEP * r_slope * z_slope * by_both
                };
                by_ln_r += term(weights(radial_rate, di), weights(vertical, dj));
                by_ln_z += term(weights(radial, di), weights(vertical_rate, dj));
            }
        }
        // The basis's rates are per cell; one cell is STEP in the logarithm.
        [by_ln_r / STEP, by_ln_z / STEP]
    }
}

/// The derivative in `t` of the cubic Hermite basis of [`hermite`].
fn hermite_slope(t: f64) -> [f64; 4] {
    let t2 = t * t;
    [
        6.0 * t2 - 6.0 * t,
        -6.0 * t2 + 6.0 * t,
        3.0 * t2 - 4.0 * t + 1.0,
        3.0 * t2 - 2.0 * t,
    ]
}

/// The galaxy's potential as tables: circular speed, its radial derivative and the potential on
/// a radial grid in the plane, and on request the potential on an (R, |z|) grid (plan 02,
/// P02.T6.d; Design note 7).
///
/// Both grids have 64 points per axis, log-spaced from 2⁻⁴ to 2¹⁸ ly, and hold the Gaussian
/// components alone, interpolated by cubic Hermite in `ln R` (bicubic in `ln R` and `ln |z|` on
/// the (R, z) grid) from exact derivatives. The dark halo, the nuclear cluster and the black hole
/// are added in closed form at lookup. Below 2⁻⁴ ly the Gaussians' part is taken as solid-body,
/// where the black hole dominates anyway; beyond 2¹⁸ ly as a point mass; on the (R, z) grid,
/// below 2⁻⁴ ly in R it is taken at 2⁻⁴ ly, and below 2⁻⁴ ly in |z| it is blended quadratically
/// into the plane's value.
///
/// Ω and κ are in radians per year, speeds in km/s, potentials in (km/s)² with zero at infinity.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
/// use hyperion_sim::units::LightYears;
///
/// let model = MassModel::new(&GalaxyParams::milky_way_like());
/// let tables = PotentialTables::in_plane(&model);
/// let sun = LightYears::new(26_100.0);
/// // The escape speed at the Sun's radius exceeds the circular speed by about √2 or more.
/// let (v, escape) = (tables.v_circ(sun).value(), tables.escape_speed_in_plane(sun).value());
/// assert!(escape > 1.414 * v);
/// // The off-plane potential needs the (R, z) grid.
/// assert!(tables.potential(sun, LightYears::new(1_000.0)).is_none());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PotentialTables {
    in_plane: InPlaneTable,
    grid: Option<Grid>,
    dark_halo: Nfw,
    nuclear_cluster: BrokenPowerLaw,
    black_hole: PointMass,
    bar_corotation: LightYears,
}

impl PotentialTables {
    /// The in-plane tables of `model`: 64 radii of one quadrature per Gaussian.
    #[must_use]
    pub fn in_plane(model: &MassModel) -> Self {
        Self {
            in_plane: InPlaneTable::new(model),
            grid: None,
            dark_halo: *model.dark_halo(),
            nuclear_cluster: *model.nuclear_cluster(),
            black_hole: *model.black_hole(),
            bar_corotation: model.bar_corotation(),
        }
    }

    /// The in-plane tables and the 64 × 64 (R, |z|) grid of `model`.
    #[must_use]
    pub fn full(model: &MassModel) -> Self {
        Self::in_plane(model).with_grid(model)
    }

    /// These tables with the (R, |z|) grid of `model` added, which must be the model the tables
    /// were built from.
    #[must_use]
    pub fn with_grid(self, model: &MassModel) -> Self {
        Self {
            grid: Some(Grid::new(model)),
            ..self
        }
    }

    /// Whether the (R, |z|) grid is present.
    #[must_use]
    pub fn has_grid(&self) -> bool {
        self.grid.is_some()
    }

    /// The bytes the tables own on the heap: the (R, |z|) grid, if present. The in-plane tables
    /// are held inline.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.grid
            .as_ref()
            .map_or(0, |grid| grid.points.capacity() * size_of::<[f64; 4]>())
    }

    /// The spherical components in the model's order.
    fn spherical(&self) -> [&dyn SphericalMass; 3] {
        [&self.dark_halo, &self.nuclear_cluster, &self.black_hole]
    }

    /// `v_c²`, `d v_c² ÷ d ln R` and `Φ(R, 0)` at `r > 0` (ly), everything included.
    fn totals(&self, r: f64) -> Extended {
        let radius = LightYears::new(r);
        self.spherical()
            .iter()
            .fold(self.in_plane.at(r), |sum, c| Extended {
                v_circ_sq: sum.v_circ_sq + c.v_circ_sq(radius),
                slope: sum.slope + c.v_circ_sq_slope(radius),
                potential: sum.potential + c.potential(radius),
            })
    }

    /// The circular speed squared in the plane at radius `r > 0`, (km/s)².
    #[must_use]
    pub fn v_circ_sq(&self, r: LightYears) -> f64 {
        self.totals(r.value()).v_circ_sq
    }

    /// The circular speed in the plane at radius `r > 0`.
    #[must_use]
    pub fn v_circ(&self, r: LightYears) -> KilometresPerSecond {
        KilometresPerSecond::new(self.v_circ_sq(r).sqrt())
    }

    /// `d v_c² ÷ dR` in the plane at radius `r > 0`, (km/s)² per light-year.
    #[must_use]
    pub fn v_circ_sq_derivative(&self, r: LightYears) -> f64 {
        self.totals(r.value()).slope / r.value()
    }

    /// Ω² and κ² at `r > 0` (ly), in (km/s ÷ ly)²: `Ω² = v_c² ÷ R²` and
    /// `κ² = (1 ÷ R) dv_c² ÷ dR + 2 v_c² ÷ R²`.
    fn frequencies_sq(&self, r: f64) -> (f64, f64) {
        let at = self.totals(r);
        let r2 = r * r;
        (at.v_circ_sq / r2, (at.slope + 2.0 * at.v_circ_sq) / r2)
    }

    /// The circular frequency `Ω = v_c ÷ R` at radius `r > 0`.
    #[must_use]
    pub fn omega(&self, r: LightYears) -> PerYear {
        PerYear::new(self.frequencies_sq(r.value()).0.sqrt() * LIGHT_YEARS_PER_YEAR_PER_KM_S)
    }

    /// The epicyclic frequency `κ = √((1 ÷ R) dv_c² ÷ dR + 2 v_c² ÷ R²)` at radius `r > 0`.
    #[must_use]
    pub fn kappa(&self, r: LightYears) -> PerYear {
        PerYear::new(self.frequencies_sq(r.value()).1.sqrt() * LIGHT_YEARS_PER_YEAR_PER_KM_S)
    }

    /// The potential in the plane at radius `r > 0`, (km/s)², zero at infinity.
    #[must_use]
    pub fn potential_in_plane(&self, r: LightYears) -> f64 {
        self.totals(r.value()).potential
    }

    /// The potential at `(R, z)`, (km/s)², zero at infinity; `None` without the (R, z) grid
    /// ([`full`](Self::full)). Off the centre only.
    #[must_use]
    pub fn potential(&self, r_cyl: LightYears, z: LightYears) -> Option<f64> {
        let grid = self.grid.as_ref()?;
        let (r, z) = (r_cyl.value().max(FIRST), z.value().abs());
        let extended = if r > LAST || z > LAST {
            // A point mass beyond the grid, scaled from the grid's edge.
            let (rc, zc) = (r.min(LAST), z.min(LAST));
            grid.at(rc, zc) * math::hypot(rc, zc) / math::hypot(r, z)
        } else if z < FIRST {
            let plane = self.in_plane.at(r).potential;
            let edge = grid.at(r, FIRST);
            plane + (edge - plane) * (z / FIRST) * (z / FIRST)
        } else {
            grid.at(r, z)
        };
        let radius = LightYears::new(math::hypot(r_cyl.value(), z));
        Some(
            self.spherical()
                .iter()
                .fold(extended, |sum, c| sum + c.potential(radius)),
        )
    }

    /// The escape speed `√(−2Φ)` in the plane at radius `r > 0`.
    #[must_use]
    pub fn escape_speed_in_plane(&self, r: LightYears) -> KilometresPerSecond {
        KilometresPerSecond::new((-2.0 * self.potential_in_plane(r)).sqrt())
    }

    /// How far a star must go to have escaped the galaxy: twice the dark halo's `r₂₀₀`, where
    /// Deason et al. (2019, MNRAS 485, 3514) measure the Milky Way's escape speed to (plan 07,
    /// ruling 91 of 2026-09-22).
    #[must_use]
    pub fn escape_boundary(&self) -> LightYears {
        self.dark_halo.r200() * 2.0
    }

    /// The escape speed from the galaxy in the plane at radius `r > 0`: the speed that reaches
    /// [`escape_boundary`](Self::escape_boundary), `√(2 [Φ(2 r₂₀₀) − Φ(R)])`, with Φ in the plane
    /// out to the boundary (Deason et al. 2019, MNRAS 485, 3514: 528 +24 −25 km/s at the Sun). It is
    /// what the Milky Way's measured 500–580 km/s is, where [`escape_speed_in_plane`]'s `√(−2Φ)` is
    /// the speed to infinity through an untruncated halo: 512.0 against 558.1 km/s for the Milky
    /// Way fixture at 26,000 ly. Piffl et al. (2014, A&A 562, A91) and Monari et al. (2018, A&A
    /// 616, L9) measure to three virial radii `r₃₄₀`, about 2.4 `r₂₀₀`, which Koppelman and Helmi
    /// (2021, A&A 649, A55) find differs by 5 km/s. Zero at or beyond the boundary.
    ///
    /// [`escape_speed_in_plane`]: Self::escape_speed_in_plane
    #[must_use]
    pub fn galactic_escape_speed_in_plane(&self, r: LightYears) -> KilometresPerSecond {
        let depth = self.potential_in_plane(self.escape_boundary()) - self.potential_in_plane(r);
        KilometresPerSecond::new(if depth > 0.0 {
            (2.0 * depth).sqrt()
        } else {
            0.0
        })
    }

    /// The escape speed `√(−2Φ)` at `(R, z)`; `None` without the (R, z) grid.
    #[must_use]
    pub fn escape_speed(&self, r_cyl: LightYears, z: LightYears) -> Option<KilometresPerSecond> {
        self.potential(r_cyl, z)
            .map(|phi| KilometresPerSecond::new((-2.0 * phi).sqrt()))
    }

    /// The tidal (Jacobi) radius of a system of mass `m` at `p`: `(G m ÷ (4Ω² − κ²))^⅓`
    /// (brainstorm, "Coordinates"), in metres (plan 02, Design note 17).
    ///
    /// Ω and κ are read at the spherical radius `|p|` from the in-plane tables. The denominator is
    /// floored at `0.05 Ω²`, where the rotation curve is nearly solid-body. Around a lone point
    /// mass M at distance R it is `R (m ÷ 3M)^⅓`. At the centre itself it is zero.
    #[must_use]
    pub fn tidal_radius(&self, m: SolarMasses, p: &PointLy) -> Metres {
        let r = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
        if r == 0.0 {
            return Metres::ZERO;
        }
        let (omega_sq, kappa_sq) = self.frequencies_sq(r);
        let denominator = (4.0 * omega_sq - kappa_sq).max(TIDAL_FLOOR * omega_sq);
        Metres::from(LightYears::new(math::cbrt(G * m.value() / denominator)))
    }

    /// The bar's corotation radius: its corotation ratio times its half-length.
    #[must_use]
    pub fn bar_corotation(&self) -> LightYears {
        self.bar_corotation
    }

    /// The bar's pattern speed: the circular frequency at its corotation radius.
    #[must_use]
    pub fn bar_pattern_speed(&self) -> PerYear {
        self.omega(self.bar_corotation)
    }

    /// `R ∂Φ ÷ ∂R`, (km/s)², and `K_z = ∂Φ ÷ ∂z`, (km/s)² per light-year, at `(R, z)` (ly),
    /// everything included; `None` without the (R, z) grid (plan 08, the kinematics' force source).
    ///
    /// They are the derivatives of what [`potential`](Self::potential) interpolates, so they agree
    /// with it to the interpolation's accuracy and are exact at the grid points. `K_z` is odd in z.
    /// Below 2⁻⁴ ly in R the Gaussians' radial term is continued as solid-body, as in the plane.
    pub(crate) fn forces(&self, r_cyl: f64, z: f64) -> Option<[f64; 2]> {
        let grid = self.grid.as_ref()?;
        let height = z.abs();
        let r = r_cyl.max(FIRST);
        let [mut radial, mut vertical] = if r > LAST || height > LAST {
            // A point mass beyond the grid: Φ = Φ_edge h_edge ÷ h.
            let (rc, zc) = (r.min(LAST), height.min(LAST));
            let h = math::hypot(r, height);
            let phi = grid.at(rc, zc) * math::hypot(rc, zc) / h;
            [-phi * r * r / (h * h), -phi * height / (h * h)]
        } else if height < FIRST {
            let plane = self.in_plane.at(r);
            let edge = grid.at(r, FIRST);
            let [edge_by_ln_r, _] = grid.log_derivatives(r, FIRST);
            let blend = (height / FIRST) * (height / FIRST);
            [
                plane.v_circ_sq + (edge_by_ln_r - plane.v_circ_sq) * blend,
                2.0 * (edge - plane.potential) * height / (FIRST * FIRST),
            ]
        } else {
            let [by_ln_r, by_ln_z] = grid.log_derivatives(r, height);
            [by_ln_r, by_ln_z / height]
        };
        if r_cyl < FIRST {
            radial *= (r_cyl / FIRST) * (r_cyl / FIRST);
        }
        let h = math::hypot(r_cyl, height);
        if h > 0.0 {
            let radius = LightYears::new(h);
            for c in self.spherical() {
                let v2 = c.v_circ_sq(radius);
                radial += v2 * (r_cyl / h) * (r_cyl / h);
                vertical += v2 * height / (h * h);
            }
        }
        Some([radial, vertical.copysign(z)])
    }

    /// The Gaussians' total mass, for tests of the far field.
    #[cfg(test)]
    fn extended_edge(&self) -> f64 {
        self.in_plane.v_circ_sq[POINTS - 1] * LAST / G
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::params::GalaxyParams;

    /// Ruling 91: the escape speed from the Milky Way fixture at the Sun's radius, measured to
    /// twice `r₂₀₀` as Deason et al. (2019) measure it, lies in the Milky Way's 500–580 km/s
    /// (Deason et al. 528 +24 −25; Piffl et al. 2014, 533 +54 −41; Monari et al. 2018, 580 ± 63),
    /// at 512.0 km/s, below the untruncated `√(−2Φ)` of 558.1; and it falls outward to nothing at
    /// the boundary. Ruling 91's 528.6 was the research's own model of an earlier fixture, whose
    /// `√(−2Φ)` it put at 574.
    #[test]
    fn the_fixtures_escape_speed_to_twice_r200_is_the_milky_ways() {
        let tables = PotentialTables::in_plane(&MassModel::new(&GalaxyParams::milky_way_like()));
        let sun = LightYears::new(26_000.0);
        let escape = tables.galactic_escape_speed_in_plane(sun).value();
        assert!((500.0..=580.0).contains(&escape), "{escape} km/s");
        assert!((escape - 512.0).abs() < 1.0, "{escape} km/s");
        let infinity = tables.escape_speed_in_plane(sun).value();
        assert!(escape < infinity, "{escape} against {infinity} km/s");
        let boundary = tables.escape_boundary();
        assert!((boundary.value() / (2.0 * tables.dark_halo.r200().value()) - 1.0).abs() < 1e-15);
        let farther = tables.galactic_escape_speed_in_plane(LightYears::new(60_000.0));
        assert!(farther.value() < escape);
        assert!(
            tables
                .galactic_escape_speed_in_plane(boundary)
                .value()
                .abs()
                < f64::EPSILON
        );
    }

    /// The tables' forces are the mass model's, to the interpolation's accuracy, across the disc,
    /// the bulge and the halo, above and below the plane, and beyond the grid (plan 08's force
    /// source).
    #[test]
    fn the_grid_forces_match_the_mass_model() {
        let params = crate::galaxy::params::GalaxyParams::milky_way_like();
        let model = MassModel::new(&params);
        let tables = PotentialTables::full(&model);
        assert!(PotentialTables::in_plane(&model).forces(1.0, 1.0).is_none());
        for (r, z) in [
            (26_000.0, 300.0),
            (26_000.0, -1_500.0),
            (3_000.0, 400.0),
            (500.0, 60.0),
            (12_000.0, 0.03),
            (40_000.0, 20_000.0),
            (0.02, 800.0),
            (300_000.0, 10.0),
        ] {
            let [radial, vertical] = tables.forces(r, z).unwrap();
            let k_z = model.vertical_force(LightYears::new(r), LightYears::new(z));
            // R ∂Φ ÷ ∂R by a central difference of the model's potential in ln R.
            let d = 1e-4;
            let (lo, hi) = (r * math::exp(-d), r * math::exp(d));
            let by_ln_r = (model.potential(LightYears::new(hi), LightYears::new(z))
                - model.potential(LightYears::new(lo), LightYears::new(z)))
                / (2.0 * d);
            assert!(
                (vertical - k_z).abs() <= 5e-3 * k_z.abs() + 1e-9 * (by_ln_r.abs() / r),
                "K_z at ({r}, {z}): {vertical} against {k_z}"
            );
            assert!(
                (radial - by_ln_r).abs() <= 5e-3 * by_ln_r.abs() + 1e-3,
                "R ∂Φ/∂R at ({r}, {z}): {radial} against {by_ln_r}"
            );
        }
    }

    #[test]
    fn the_grid_spans_two_to_the_minus_four_to_two_to_the_eighteen() {
        assert!(grid_point(0).total_cmp(&0.0625).is_eq());
        assert!(grid_point(63).total_cmp(&262_144.0).is_eq());
        for i in 1..63 {
            let ratio = grid_point(i) / grid_point(i - 1);
            assert!((math::ln(ratio) - STEP).abs() < 1e-12);
        }
        for i in 0..63 {
            let (cell_i, t) = cell(grid_point(i) * math::exp(0.5 * STEP));
            assert_eq!(cell_i, i);
            assert!((t - 0.5).abs() < 1e-9);
        }
        assert_eq!(cell(LAST).0, 62);
        assert_eq!(cell(FIRST).0, 0);
    }

    #[test]
    fn hermite_reproduces_a_cubic_in_the_logarithm() {
        let cubic = |u: f64| 0.3 * u * u * u - u + 2.0;
        let slope = |u: f64| 0.9 * u * u - 1.0;
        let mut values = [0.0; POINTS];
        let mut slopes = [0.0; POINTS];
        for (i, (value, derivative)) in values.iter_mut().zip(&mut slopes).enumerate() {
            let u = math::ln(grid_point(i));
            *value = cubic(u);
            *derivative = slope(u);
        }
        for x in [0.1, 3.7, 1_234.5, 200_000.0] {
            let (index, position) = cell(x);
            let exact = cubic(math::ln(x));
            let interpolated = interpolate(&values, &slopes, index, position);
            assert!((interpolated - exact).abs() < 1e-9 * exact.abs().max(1.0));
        }
    }

    /// A synthetic solid-body rotation curve, `v_c = Ω₀ R`, with nothing else: κ = 2Ω, so
    /// `4Ω² − κ²` vanishes and the tidal radius takes the floor `(G m ÷ 0.05 Ω²)^⅓`.
    #[test]
    fn the_tidal_floor_engages_for_a_solid_body_curve() {
        let omega_sq = 1e-4;
        let mut table = InPlaneTable {
            v_circ_sq: [0.0; POINTS],
            slope: [0.0; POINTS],
            curvature: [0.0; POINTS],
            potential: [0.0; POINTS],
        };
        for i in 0..POINTS {
            let r = grid_point(i);
            table.v_circ_sq[i] = omega_sq * r * r;
            table.slope[i] = 2.0 * omega_sq * r * r;
            table.curvature[i] = 4.0 * omega_sq * r * r;
            table.potential[i] = 0.5 * omega_sq * r * r;
        }
        let zero = SolarMasses::ZERO;
        let tables = PotentialTables {
            in_plane: table,
            grid: None,
            dark_halo: Nfw::new(zero, 10.0, LightYears::new(1e5)).unwrap(),
            nuclear_cluster: BrokenPowerLaw::new(zero, LightYears::new(10.0), 1.3, 3.5).unwrap(),
            black_hole: PointMass::new(zero).unwrap(),
            bar_corotation: LightYears::new(10_000.0),
        };
        for r in [0.01, 3.0, 5_000.0, 100_000.0] {
            let ratio =
                tables.kappa(LightYears::new(r)).value() / tables.omega(LightYears::new(r)).value();
            assert!((ratio - 2.0).abs() < 1e-9, "κ ÷ Ω = {ratio} at {r}");
            let p = PointLy::new(0.0, 0.0, r);
            let rt = LightYears::from(tables.tidal_radius(SolarMasses::new(1.0), &p)).value();
            let floor = math::cbrt(G / (TIDAL_FLOOR * omega_sq));
            // Ω² itself is interpolated, and e^(2 ln R) is not a cubic in ln R: the Hermite
            // error is up to h⁴ ÷ 384 × 16 ≈ 1.4 × 10⁻⁴ in Ω², a third of that in the radius.
            assert!(
                (rt / floor - 1.0).abs() < 1e-4,
                "{rt} against {floor} at {r}"
            );
        }
    }

    /// Around a lone point mass M, κ = Ω and the tidal radius is `R (m ÷ 3M)^⅓` at any radius,
    /// inside the grid, below it and beyond it.
    #[test]
    fn the_tidal_radius_around_a_lone_point_mass() {
        let zero = SolarMasses::ZERO;
        let mass = 4.3e6;
        let tables = PotentialTables {
            in_plane: InPlaneTable {
                v_circ_sq: [0.0; POINTS],
                slope: [0.0; POINTS],
                curvature: [0.0; POINTS],
                potential: [0.0; POINTS],
            },
            grid: None,
            dark_halo: Nfw::new(zero, 10.0, LightYears::new(1e5)).unwrap(),
            nuclear_cluster: BrokenPowerLaw::new(zero, LightYears::new(10.0), 1.3, 3.5).unwrap(),
            black_hole: PointMass::new(SolarMasses::new(mass)).unwrap(),
            bar_corotation: LightYears::new(10_000.0),
        };
        for r in [0.01, 3.0, 5_000.0, 1e6] {
            let p = PointLy::new(0.36 * r, 0.48 * r, 0.8 * r);
            let rt = LightYears::from(tables.tidal_radius(SolarMasses::new(2.0), &p)).value();
            let expected = r * math::cbrt(2.0 / (3.0 * mass));
            assert!(
                (rt / expected - 1.0).abs() < 1e-12,
                "{rt} against {expected}"
            );
            let (omega, kappa) = (
                tables.omega(LightYears::new(r)),
                tables.kappa(LightYears::new(r)),
            );
            assert!((kappa.value() / omega.value() - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn the_far_field_mass_is_the_gaussians_mass() {
        let params = crate::galaxy::params::GalaxyParams::milky_way_like();
        let model = MassModel::new(&params);
        let tables = PotentialTables::in_plane(&model);
        // At 2¹⁸ ly the Gaussians are within a per cent of a point mass.
        let edge = tables.extended_edge();
        assert!((edge / model.extended_mass() - 1.0).abs() < 0.01, "{edge}");
    }
}
