//! The climate models' grid (R09.T13.a–b, Design note 8): latitude bands of 2.5° and longitude
//! columns about the model's pole, each band's geometry for the diffusion, the tridiagonal solve
//! of the implicit step in latitude and the periodic one in longitude, the bicubic interpolation
//! from the points' centres to a direction, and the time means a model leaves on it.
//!
//! A grid point is (band, column), stored band-major at `band * columns + column`. The zonal
//! model has one column ([`ModelGrid::zonal`]); the geographic model 72 of 5° about the spin axis
//! ([`ModelGrid::geographic`]); the locked model 144 of 2.5° in tidally locked coordinates, whose
//! pole is the substellar point ([`ModelGrid::locked`], [`Coordinates`](super::coordinates)). The
//! grid itself knows only its own coordinates: band k spans the latitudes −90° + 2.5° k to
//! −90° + 2.5° (k + 1), and column j the longitudes −180° + Δλ j to −180° + Δλ (j + 1), about the
//! model frame's third axis from its first towards its second.
//!
//! The diffusion is in flux form (finite volumes). In latitude, band k spans φₖ to φₖ₊₁, holds the
//! share wₖ = sin φₖ₊₁ − sin φₖ of 2πR², and exchanges with its neighbour across the edge φₖ₊₁
//! through cos φₖ₊₁ ÷ Δφ, Δφ the spacing of the centres. The band average of
//! (1 ÷ cos φ) ∂/∂φ (cos φ ∂h/∂φ) is then [cos φ ∂h/∂φ] over the band's edges ÷ wₖ, so the
//! weighted sum over the bands telescopes to the poles' terms, which vanish with cos ±90°: the
//! scheme conserves energy exactly but for rounding, and the poles are no-flux. In longitude, the
//! sphere's isotropic Laplacian adds (1 ÷ cos² φ) ∂²h/∂λ²: a point of band k exchanges with each
//! neighbour in its band across their shared meridian, of length Δφ, over the distance cos φ Δλ
//! between their centres, so through Δφ ÷ (cos φ Δλ), over its own share wₖ Δλ
//! ([`ModelGrid::longitude_coupling`], which is 1 ÷ (cos² φ Δλ²) to 1 part in 10⁴). The sum over a
//! band telescopes round it, so the longitude term conserves energy too.

use core::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::math;

/// The model grids' latitude bands: 72, of 2.5° each, Design note 8's grid.
///
/// Spiegel, Menou and Scharf (2008, ApJ 681, 1609, Appendix I, read) found the global mean of a
/// seasonal Earth-like EBM on a 2.5° grid within about 0.5 K of its 0.5° solution, on a 1.25°
/// grid within 0.2 K, and on a 10° grid off by more than 4 K.
pub(super) const LATITUDE_BANDS: usize = 72;

/// The zonal model's longitude columns: one.
pub(super) const ZONAL_COLUMNS: usize = 1;

/// The geographic model's longitude columns: 72, of 5° each (Design note 8's 72 × 72 cells).
pub(super) const GEOGRAPHIC_COLUMNS: usize = 72;

/// The locked model's longitude columns about the substellar axis: 144, of 2.5° each (Design note
/// 8's 2.5° × 2.5° in a locked world's tidally locked coordinates).
pub(super) const LOCKED_COLUMNS: usize = 144;

/// The bands' angular width, π ÷ 72 rad (2.5°), which is also the spacing of their centres.
const BAND_WIDTH_RAD: f64 = PI / 72.0;

/// The geometry of a model grid of [`LATITUDE_BANDS`] bands, each of `columns` columns.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ModelGrid {
    columns: usize,
    centre_sine: Vec<f64>,
    centre_cosine: Vec<f64>,
    weight: Vec<f64>,
    edge_conductance: Vec<f64>,
    column_width_rad: f64,
    column_sine: Vec<f64>,
    column_cosine: Vec<f64>,
    longitude_coupling: Vec<f64>,
}

impl ModelGrid {
    /// The zonal model's grid: [`LATITUDE_BANDS`] bands of one column.
    #[must_use]
    pub(super) fn zonal() -> Self {
        Self::with_columns(ZONAL_COLUMNS)
    }

    /// The geographic model's grid: [`LATITUDE_BANDS`] bands of [`GEOGRAPHIC_COLUMNS`] columns.
    #[must_use]
    pub(super) fn geographic() -> Self {
        Self::with_columns(GEOGRAPHIC_COLUMNS)
    }

    /// The locked model's grid: [`LATITUDE_BANDS`] bands of [`LOCKED_COLUMNS`] columns, in
    /// tidally locked coordinates.
    #[must_use]
    pub(super) fn locked() -> Self {
        Self::with_columns(LOCKED_COLUMNS)
    }

    /// The grid of [`LATITUDE_BANDS`] bands of `columns` columns each.
    ///
    /// # Panics
    ///
    /// If `columns` is zero, or neither one nor even: a column's continuation over a pole is the
    /// column half a turn round, which an odd count lacks.
    #[must_use]
    pub(super) fn with_columns(columns: usize) -> Self {
        assert!(
            columns == 1 || (columns > 0 && columns.is_multiple_of(2)),
            "a grid has one column or an even number of them"
        );
        let bands = 0..LATITUDE_BANDS;
        let edge = |k: usize| -FRAC_PI_2 + index(k) * BAND_WIDTH_RAD;
        let edge_sine: Vec<f64> = (0..=LATITUDE_BANDS)
            .map(|k| match k {
                0 => -1.0,
                LATITUDE_BANDS => 1.0,
                _ => math::sin(edge(k)),
            })
            .collect();
        let (centre_sine, centre_cosine): (Vec<f64>, Vec<f64>) = bands
            .clone()
            .map(|k| math::sin_cos(edge(k) + 0.5 * BAND_WIDTH_RAD))
            .unzip();
        // The poles' edges carry no flux: cos ±90° is exactly zero there, not the 6 × 10⁻¹⁷ the
        // cosine of the nearest `f64` to π ÷ 2 gives.
        let edge_conductance = (0..=LATITUDE_BANDS)
            .map(|k| match k {
                0 | LATITUDE_BANDS => 0.0,
                _ => math::cos(edge(k)) / BAND_WIDTH_RAD,
            })
            .collect();
        let weight: Vec<f64> = bands
            .clone()
            .map(|k| edge_sine[k + 1] - edge_sine[k])
            .collect();
        let column_width_rad = TAU / index(columns);
        let (column_sine, column_cosine) = (0..columns)
            .map(|j| math::sin_cos(-PI + (index(j) + 0.5) * column_width_rad))
            .unzip();
        let longitude_coupling = bands
            .map(|k| {
                if columns == 1 {
                    0.0
                } else {
                    BAND_WIDTH_RAD
                        / (centre_cosine[k] * column_width_rad)
                        / (weight[k] * column_width_rad)
                }
            })
            .collect();
        Self {
            columns,
            centre_sine,
            centre_cosine,
            weight,
            edge_conductance,
            column_width_rad,
            column_sine,
            column_cosine,
            longitude_coupling,
        }
    }

    /// The grid's columns.
    #[must_use]
    pub(super) fn columns(&self) -> usize {
        self.columns
    }

    /// The grid's points, bands × columns.
    #[must_use]
    pub(super) fn points(&self) -> usize {
        LATITUDE_BANDS * self.columns
    }

    /// The point of band `band` and column `column`: `band * columns + column`.
    #[must_use]
    pub(super) fn point(&self, band: usize, column: usize) -> usize {
        band * self.columns + column
    }

    /// The sine of band `band`'s centre latitude.
    #[must_use]
    pub(super) fn centre_sine(&self, band: usize) -> f64 {
        self.centre_sine[band]
    }

    /// The cosine of band `band`'s centre latitude, positive.
    #[must_use]
    pub(super) fn centre_cosine(&self, band: usize) -> f64 {
        self.centre_cosine[band]
    }

    /// Band `band`'s share of 2πR², sin φₖ₊₁ − sin φₖ: the bands' shares sum to 2.
    #[must_use]
    pub(super) fn weight(&self, band: usize) -> f64 {
        self.weight[band]
    }

    /// The conductance of edge `edge` (from 0, the south pole, to [`LATITUDE_BANDS`], the north
    /// pole), cos φ ÷ Δφ per radian: zero at the poles.
    #[must_use]
    pub(super) fn edge_conductance(&self, edge: usize) -> f64 {
        self.edge_conductance[edge]
    }

    /// The longitude coupling of band `band`, Δφ ÷ (cos φ Δλ) ÷ (wₖ Δλ) per unit of Λ: the
    /// weight of each neighbour's h in a point's longitude convergence, (1 ÷ cos² φ) ∂²h/∂λ² in
    /// flux form ([module documentation](self)); zero on a grid of one column.
    #[must_use]
    pub(super) fn longitude_coupling(&self, band: usize) -> f64 {
        self.longitude_coupling[band]
    }

    /// The unit vector, in the model frame, of point `point`'s centre: (cos φ cos λ, cos φ sin λ,
    /// sin φ).
    #[must_use]
    pub(super) fn centre_direction(&self, point: usize) -> [f64; 3] {
        let (band, column) = (point / self.columns, point % self.columns);
        self.centre_direction_of(band, column)
    }

    /// The unit vector, in the model frame, of the centre of the point of band `band` and column
    /// `column` ([`centre_direction`](Self::centre_direction), with no division).
    #[must_use]
    pub(super) fn centre_direction_of(&self, band: usize, column: usize) -> [f64; 3] {
        let (s, c) = (self.centre_sine[band], self.centre_cosine[band]);
        [
            c * self.column_cosine[column],
            c * self.column_sine[column],
            s,
        ]
    }

    /// The band holding the point at sine latitude `sine_latitude`, by its latitude: band k holds
    /// −90° + 2.5° k up to, not including, −90° + 2.5° (k + 1), and the last band the north pole.
    #[must_use]
    pub(super) fn band_of(sine_latitude: f64) -> usize {
        let latitude = math::asin(sine_latitude.clamp(-1.0, 1.0));
        floor_index((latitude + FRAC_PI_2) / BAND_WIDTH_RAD, LATITUDE_BANDS)
    }

    /// The point holding the direction `direction` (a unit vector in the model frame): its band by
    /// its latitude ([`band_of`](Self::band_of)) and its column by its longitude, column j holding
    /// −180° + Δλ j up to, not including, −180° + Δλ (j + 1), and the last column 180°.
    #[must_use]
    pub(super) fn point_of(&self, direction: [f64; 3]) -> usize {
        let band = Self::band_of(direction[2]);
        let longitude = math::atan2(direction[1], direction[0]);
        let column = floor_index((longitude + PI) / self.column_width_rad, self.columns);
        self.point(band, column)
    }

    /// The area-weighted mean over the grid of `values`, one per point: Σ wₖ x ÷ (2 × columns),
    /// summed band by band from the south and column by column within each.
    #[must_use]
    pub(super) fn mean(&self, values: &[f64]) -> f64 {
        let total = (0..LATITUDE_BANDS).fold(0.0, |sum, band| {
            let row = &values[band * self.columns..(band + 1) * self.columns];
            sum + self.weight[band] * row.iter().fold(0.0, |s, v| s + v)
        });
        total / (2.0 * index(self.columns))
    }

    /// Each band's mean over its columns of `values`, one per point, summed column by column; on a
    /// grid of one column, `values` itself.
    #[must_use]
    pub(super) fn band_means(&self, values: &[f64]) -> Vec<f64> {
        if self.columns == 1 {
            return values.to_vec();
        }
        let n = index(self.columns);
        values
            .chunks_exact(self.columns)
            .map(|row| row.iter().fold(0.0, |s, v| s + v) / n)
            .collect()
    }

    /// The field of this grid whose every column is `bands`, one value per band.
    #[must_use]
    pub(super) fn broadcast(&self, bands: &[f64]) -> Vec<f64> {
        bands
            .iter()
            .flat_map(|&v| core::iter::repeat_n(v, self.columns))
            .collect()
    }

    /// The stencil that interpolates a field of one column to the point at sine latitude
    /// `sine_latitude`, in latitude by the cubic through the four nearest band centres: the zonal
    /// model's, which [`stencil_at`](Self::stencil_at) is on a grid of one column.
    ///
    /// The cubic is Catmull and Rom's (Keys 1981, IEEE Trans. ASSP 29, 1153, the cubic convolution
    /// with a = −½): it passes through every centre with a continuous slope. Across a pole the
    /// stencil reads the bands mirrored about it, as a zonal field continues over the pole, so the
    /// slope there is zero.
    #[cfg(test)]
    #[must_use]
    pub(super) fn stencil(sine_latitude: f64) -> Stencil {
        let (nodes, weights) = latitude_nodes(sine_latitude);
        Stencil {
            bands: nodes.map(|node| mirrored(node).0),
            weights,
            longitude: None,
        }
    }

    /// The stencil that interpolates a field of this grid to the direction `direction` (a unit
    /// vector in the model frame): bicubic, Catmull and Rom's cubic (Keys 1981, IEEE Trans. ASSP
    /// 29, 1153, the cubic convolution with a = −½, which passes through every centre with a
    /// continuous slope) through the four nearest band centres in latitude and, at each, through
    /// the four nearest column centres in longitude, periodic round the band.
    ///
    /// Across a pole the meridian continues as the one half a turn round, so a band mirrored there
    /// is read [`columns`](Self::columns) ÷ 2 columns on: a field smooth on the sphere is then
    /// interpolated through the pole with every term up to the cubic's along each great circle
    /// through it, so that its value there is the same, to the fourth order of the spacing, from
    /// every meridian. On a grid of one column it is the cubic in latitude alone, the zonal
    /// model's, whose bands mirrored across a pole make its slope there zero.
    #[must_use]
    pub(super) fn stencil_at(&self, direction: [f64; 3]) -> Stencil {
        let (nodes, weights) = latitude_nodes(direction[2]);
        let mirrors = nodes.map(mirrored);
        let bands = mirrors.map(|(band, _)| band);
        if self.columns == 1 {
            return Stencil {
                bands,
                weights,
                longitude: None,
            };
        }
        let longitude = math::atan2(direction[1], direction[0]);
        // The position in column centres, from −½ at −180° to `columns` − ½ at 180°.
        let position = (longitude + PI) / self.column_width_rad - 0.5;
        let below = position.floor();
        let f = position - below;
        let columns = i64::try_from(self.columns).expect("a column count fits an i64");
        let first = finite_integer(below) - 1;
        let half = self.columns / 2;
        let points = mirrors.map(|(band, crossed)| {
            [0, 1, 2, 3].map(|offset| {
                let column = usize::try_from((first + offset).rem_euclid(columns))
                    .expect("a column reduced by the column count is non-negative");
                let column = if crossed {
                    (column + half) % self.columns
                } else {
                    column
                };
                self.point(band, column)
            })
        });
        Stencil {
            bands,
            weights,
            longitude: Some(LongitudeStencil {
                points,
                weights: catmull_rom(f),
            }),
        }
    }
}

/// The interpolation of a field of a [`ModelGrid`] to a point ([`ModelGrid::stencil_at`]): four
/// bands and their weights, which sum to one, and on a grid of more than one column four columns
/// at each band and their weights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Stencil {
    bands: [usize; 4],
    weights: [f64; 4],
    longitude: Option<LongitudeStencil>,
}

/// The longitude part of a [`Stencil`]: each latitude node's four points and the columns'
/// weights.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LongitudeStencil {
    points: [[usize; 4]; 4],
    weights: [f64; 4],
}

impl Stencil {
    /// The value at the stencil's point of `values`, one per grid point, summed in the stencil's
    /// order from the south and, in each band, from the west.
    #[must_use]
    pub(super) fn apply(&self, values: &[f64]) -> f64 {
        match &self.longitude {
            None => self
                .bands
                .iter()
                .zip(self.weights)
                .fold(0.0, |sum, (&band, w)| sum + w * values[band]),
            Some(longitude) => {
                longitude
                    .points
                    .iter()
                    .zip(self.weights)
                    .fold(0.0, |sum, (row, w)| {
                        let along = row
                            .iter()
                            .zip(longitude.weights)
                            .fold(0.0, |s, (&point, u)| s + u * values[point]);
                        sum + w * along
                    })
            }
        }
    }

    /// The value at the stencil's point of `values` ([`apply`](Self::apply)), held to the range of
    /// the values it reads (R09.T13.c): the guard of decision-r09-t13d-normalisation §2.4 against
    /// the cubic's overshoot where a pointwise model's field breaks, as a locked world's does at
    /// its terminator, falling by hundreds of kelvin within one band, where Catmull and Rom's
    /// cubic overshoots by about a tenth of the jump.
    ///
    /// # Panics
    ///
    /// If a value it reads is not finite, which the models rule out.
    #[cfg(test)]
    #[must_use]
    pub(super) fn apply_clamped(&self, values: &[f64]) -> f64 {
        self.clamped(values).0
    }

    /// [`apply_clamped`](Self::apply_clamped)'s value, and whether the range held it, the guard's
    /// diagnostic (R09.T13.d).
    ///
    /// # Panics
    ///
    /// If a value it reads is not finite, which the models rule out.
    #[must_use]
    pub(super) fn clamped(&self, values: &[f64]) -> (f64, bool) {
        let value = self.apply(values);
        let (low, high) = self.range(values);
        if value < low {
            (low, true)
        } else if value > high {
            (high, true)
        } else {
            (value, false)
        }
    }

    /// The least and the greatest of the values of `values` the stencil reads.
    #[must_use]
    fn range(&self, values: &[f64]) -> (f64, f64) {
        let read = |range: (f64, f64), &point: &usize| {
            let v = super::finite_k(values[point]);
            (
                if v < range.0 { v } else { range.0 },
                if v > range.1 { v } else { range.1 },
            )
        };
        let start = (f64::INFINITY, f64::NEG_INFINITY);
        match &self.longitude {
            None => self.bands.iter().fold(start, read),
            Some(longitude) => longitude.points.iter().flatten().fold(start, read),
        }
    }
}

/// The time means a climate model leaves on its grid: the annual mean and each month's at every
/// point, kelvin, with the orbits the model ran and whether it converged.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct GridClimate {
    /// The annual mean surface temperature at sea level at each point, kelvin.
    pub(super) annual_k: Vec<f64>,
    /// Each month's mean at each point, kelvin: twelve months, or one in a one-month year, whose
    /// month is the annual mean.
    pub(super) months_k: Vec<Vec<f64>>,
    /// The whole seasonal orbits the model ran after its spin-up: zero for a steady state.
    pub(super) orbits: u32,
    /// Whether the run converged: the annual mean moved under the model's tolerance between its
    /// last two orbits, or the steady state's iteration met its tolerance.
    pub(super) converged: bool,
}

/// Solves the tridiagonal system with sub-diagonal `lower` (its first entry unused), diagonal
/// `diagonal` and super-diagonal `upper` (its last entry unused) for the right-hand side `rhs`,
/// into `solution`, by the Thomas algorithm, with `scratch` the length of the system.
///
/// The model's systems are strictly diagonally dominant, for which the algorithm needs no
/// pivoting and is stable (Golub and Van Loan 2013, Matrix Computations, 4th ed., §4.3.6, from
/// memory).
///
/// # Panics
///
/// If the system is empty, or a slice is shorter than `diagonal`.
pub(super) fn solve_tridiagonal(
    lower: &[f64],
    diagonal: &[f64],
    upper: &[f64],
    rhs: &[f64],
    solution: &mut [f64],
    scratch: &mut [f64],
) {
    let n = diagonal.len();
    assert!(n > 0, "a tridiagonal system has at least one unknown");
    scratch[0] = upper[0] / diagonal[0];
    solution[0] = rhs[0] / diagonal[0];
    for i in 1..n {
        let pivot = diagonal[i] - lower[i] * scratch[i - 1];
        scratch[i] = if i + 1 < n { upper[i] / pivot } else { 0.0 };
        solution[i] = (rhs[i] - lower[i] * solution[i - 1]) / pivot;
    }
    for i in (0..n - 1).rev() {
        solution[i] -= scratch[i] * solution[i + 1];
    }
}

/// The working arrays of [`solve_cyclic_tridiagonal`], for a system of the length it records.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CyclicWork {
    unknowns: usize,
    diagonal: Vec<f64>,
    correction_rhs: Vec<f64>,
    correction: Vec<f64>,
    scratch: Vec<f64>,
}

impl CyclicWork {
    /// The working arrays for a system of `n` unknowns.
    #[must_use]
    pub(super) fn new(n: usize) -> Self {
        let zeros = vec![0.0; n];
        Self {
            unknowns: n,
            diagonal: zeros.clone(),
            correction_rhs: zeros.clone(),
            correction: zeros.clone(),
            scratch: zeros,
        }
    }
}

/// Solves the periodic (cyclic) tridiagonal system whose row i is `lower[i]` x₍ᵢ₋₁₎ +
/// `diagonal[i]` xᵢ + `upper[i]` x₍ᵢ₊₁₎ = `rhs[i]`, the indices taken round the system (row 0's
/// `lower` multiplies the last unknown and the last row's `upper` the first), into `solution`,
/// with `work` for the system's length ([`CyclicWork::new`]), which must be at least 3.
///
/// By the Sherman–Morrison formula about the tridiagonal part with γ = −b₀, two Thomas solves and
/// a rank-one correction (Press, Teukolsky, Vetterling and Flannery 2007, Numerical Recipes, 3rd
/// ed., §2.7.2, `cyclic`, from memory; Sherman and Morrison 1950, Ann. Math. Stat. 21, 124, not
/// read). The model's longitude systems are strictly diagonally dominant, where it is stable.
///
/// # Panics
///
/// If the system has fewer than 3 unknowns, `work` is for a system of another length, or a slice
/// is shorter than `diagonal`.
pub(super) fn solve_cyclic_tridiagonal(
    lower: &[f64],
    diagonal: &[f64],
    upper: &[f64],
    rhs: &[f64],
    solution: &mut [f64],
    work: &mut CyclicWork,
) {
    let n = diagonal.len();
    assert!(n >= 3, "a cyclic system has at least three unknowns");
    assert_eq!(
        work.unknowns, n,
        "a cyclic system's working arrays are for its own length"
    );
    let last = n - 1;
    // The corners: row `last`'s coefficient of x₀, and row 0's of x_last.
    let (alpha, beta) = (upper[last], lower[0]);
    let gamma = -diagonal[0];
    work.diagonal.copy_from_slice(diagonal);
    work.diagonal[0] = diagonal[0] - gamma;
    work.diagonal[last] = diagonal[last] - alpha * beta / gamma;
    solve_tridiagonal(
        lower,
        &work.diagonal,
        upper,
        rhs,
        solution,
        &mut work.scratch,
    );
    work.correction_rhs.fill(0.0);
    work.correction_rhs[0] = gamma;
    work.correction_rhs[last] = alpha;
    solve_tridiagonal(
        lower,
        &work.diagonal,
        upper,
        &work.correction_rhs,
        &mut work.correction,
        &mut work.scratch,
    );
    let factor = (solution[0] + beta * solution[last] / gamma)
        / (1.0 + work.correction[0] + beta * work.correction[last] / gamma);
    for (x, z) in solution.iter_mut().zip(&work.correction) {
        *x -= factor * z;
    }
}

/// The latitude nodes of the cubic through the band centres at sine latitude `sine_latitude`:
/// the four bands from the stencil's first (from −2, the south pole's mirror, to 70), not yet
/// mirrored, and their Catmull–Rom weights.
#[must_use]
fn latitude_nodes(sine_latitude: f64) -> ([i64; 4], [f64; 4]) {
    let latitude = math::asin(sine_latitude.clamp(-1.0, 1.0));
    // The position in band centres, from −½ at the south pole to 71½ at the north.
    let position = (latitude + FRAC_PI_2) / BAND_WIDTH_RAD - 0.5;
    let below = position.floor();
    let f = position - below;
    let first = i64::try_from(floor_index(below + 1.0, LATITUDE_BANDS + 1))
        .expect("a band index fits an i64")
        - 2;
    ([0, 1, 2, 3].map(|offset| first + offset), catmull_rom(f))
}

/// Catmull and Rom's weights at fraction `f` from the second node to the third (Keys 1981's cubic
/// convolution at a = −½): they sum to one, and are (0, 1, 0, 0) at f = 0.
#[must_use]
fn catmull_rom(f: f64) -> [f64; 4] {
    let (f2, f3) = (f * f, f * f * f);
    [
        0.5 * f * (2.0 * f - 1.0 - f2),
        1.0 - 2.5 * f2 + 1.5 * f3,
        0.5 * f * (1.0 + 4.0 * f - 3.0 * f2),
        0.5 * f2 * (f - 1.0),
    ]
}

/// Band `band`, or the band it mirrors across a pole, and whether it crossed one: −1 is band 0
/// and −2 band 1, and 72 is band 71 and 73 band 70.
#[must_use]
fn mirrored(band: i64) -> (usize, bool) {
    let bands = i64::try_from(LATITUDE_BANDS).expect("the band count fits an i64");
    let (inside, crossed) = if band < 0 {
        (-1 - band, true)
    } else if band >= bands {
        (2 * bands - 1 - band, true)
    } else {
        (band, false)
    };
    (
        usize::try_from(inside).expect("a stencil reaches at most two bands past a pole"),
        crossed,
    )
}

/// `k` as an `f64`, exactly: every index of the grid is far below 2⁵³.
#[must_use]
pub(super) fn index(k: usize) -> f64 {
    f64::from(u32::try_from(k).expect("a grid index fits a u32"))
}

/// ⌊x⌋ as an index from 0 to `len` − 1, x clamped into that range first; 0 for a NaN.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "x is clamped to [0, len − 1] with len a grid size, so its floor is a small \
              non-negative integer that the cast holds exactly"
)]
fn floor_index(x: f64, len: usize) -> usize {
    let top = index(len - 1);
    let clamped = if x > 0.0 { x.min(top) } else { 0.0 };
    clamped.floor() as usize
}

/// `x`, an integer-valued `f64` of magnitude at most a few grid sizes, as an `i64`; 0 for a NaN.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "x is the floor of a longitude's position in columns, clamped to ±2²⁰ first, so the \
              cast holds it exactly"
)]
fn finite_integer(x: f64) -> i64 {
    let bound = 1_048_576.0;
    let clamped = if x > -bound {
        x.min(bound)
    } else if x.is_nan() {
        0.0
    } else {
        -bound
    };
    clamped as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::float::assert_same_bits;

    #[test]
    fn the_bands_share_the_sphere_and_their_poles_carry_no_flux() {
        let grid = ModelGrid::zonal();
        let total: f64 = (0..LATITUDE_BANDS).map(|k| grid.weight(k)).sum();
        assert!((total - 2.0).abs() < 1e-14, "{total}");
        assert_same_bits(grid.edge_conductance(0), 0.0);
        assert_same_bits(grid.edge_conductance(LATITUDE_BANDS), 0.0);
        // Symmetric about the equator, and the equatorial edge's cos 0 ÷ Δφ.
        for k in 0..LATITUDE_BANDS {
            let mirror = LATITUDE_BANDS - 1 - k;
            assert!((grid.weight(k) - grid.weight(mirror)).abs() < 1e-15);
            assert!((grid.centre_sine(k) + grid.centre_sine(mirror)).abs() < 1e-15);
        }
        let equator = grid.edge_conductance(LATITUDE_BANDS / 2);
        assert!((equator - 1.0 / BAND_WIDTH_RAD).abs() < 1e-12);
        assert!((grid.mean(&vec![3.5; grid.points()]) - 3.5).abs() < 1e-14);
    }

    #[test]
    fn each_point_falls_in_the_band_of_its_latitude() {
        let grid = ModelGrid::zonal();
        for k in 0..LATITUDE_BANDS {
            assert_eq!(ModelGrid::band_of(grid.centre_sine(k)), k);
        }
        assert_eq!(ModelGrid::band_of(-1.0), 0);
        assert_eq!(ModelGrid::band_of(1.0), LATITUDE_BANDS - 1);
        assert_eq!(ModelGrid::band_of(0.0), LATITUDE_BANDS / 2);
        assert_eq!(ModelGrid::band_of(f64::NAN), 0);
    }

    /// A known system's solution, and the residual of a diagonally dominant one.
    #[test]
    fn the_tridiagonal_solve_inverts_its_system() {
        let n = 9;
        let lower: Vec<f64> = (0..n).map(|i| -1.0 - 0.1 * index(i)).collect();
        let upper: Vec<f64> = (0..n).map(|i| -0.5 + 0.05 * index(i)).collect();
        let diagonal: Vec<f64> = (0..n).map(|i| 4.0 + 0.3 * index(i)).collect();
        let expected: Vec<f64> = (0..n).map(|i| math::sin(index(i))).collect();
        let rhs: Vec<f64> = (0..n)
            .map(|i| {
                let mut r = diagonal[i] * expected[i];
                if i > 0 {
                    r += lower[i] * expected[i - 1];
                }
                if i + 1 < n {
                    r += upper[i] * expected[i + 1];
                }
                r
            })
            .collect();
        let (mut solution, mut scratch) = (vec![0.0; n], vec![0.0; n]);
        solve_tridiagonal(&lower, &diagonal, &upper, &rhs, &mut solution, &mut scratch);
        for (x, e) in solution.iter().zip(&expected) {
            assert!((x - e).abs() < 1e-14, "{x} against {e}");
        }
    }

    /// The cubic passes through every band's centre, its weights sum to one, it reproduces a
    /// line in latitude away from the poles, and it is flat at each pole.
    #[test]
    fn the_stencil_interpolates_through_the_centres_and_is_flat_at_the_poles() {
        let grid = ModelGrid::zonal();
        let field: Vec<f64> = (0..LATITUDE_BANDS)
            .map(|k| 250.0 + 40.0 * grid.centre_cosine(k) + 3.0 * grid.centre_sine(k))
            .collect();
        for k in 0..LATITUDE_BANDS {
            let at = ModelGrid::stencil(grid.centre_sine(k)).apply(&field);
            assert!(
                (at - field[k]).abs() < 1e-11,
                "band {k}: {at} against {}",
                field[k]
            );
        }
        for z in [-1.0, -0.999_9, -0.3, 0.0, 0.123, 0.999_9, 1.0] {
            let stencil = ModelGrid::stencil(z);
            let total: f64 = stencil.weights.iter().sum();
            assert!((total - 1.0).abs() < 1e-14, "{z}: {total}");
        }
        let linear: Vec<f64> = (0..LATITUDE_BANDS)
            .map(|k| index(k) * BAND_WIDTH_RAD)
            .collect();
        let latitude = 0.4;
        let at = ModelGrid::stencil(math::sin(latitude)).apply(&linear);
        assert!(
            (at - (latitude + FRAC_PI_2 - 0.5 * BAND_WIDTH_RAD)).abs() < 1e-12,
            "{at}"
        );
        for pole in [-1.0_f64, 1.0] {
            let at_pole = ModelGrid::stencil(pole).apply(&field);
            let near = ModelGrid::stencil(pole * math::cos(1e-4)).apply(&field);
            assert!(
                (at_pole - near).abs() < 1e-5,
                "{pole}: {at_pole} against {near}"
            );
        }
    }

    /// Across a jump, a locked terminator's 400 K to 30 K between two bands, the cubic overshoots
    /// on both sides and the clamped stencil does not: it stays within the values it reads, and
    /// where the cubic stays within them it is the cubic, to the bit.
    #[test]
    fn the_clamped_stencil_stays_within_the_values_it_reads() {
        for grid in [ModelGrid::zonal(), ModelGrid::locked()] {
            let field: Vec<f64> = (0..grid.points())
                .map(|p| {
                    if p / grid.columns() >= 36 {
                        400.0
                    } else {
                        30.0
                    }
                })
                .collect();
            let (mut over, mut under) = (false, false);
            for k in 0..200 {
                let latitude = -0.3 + 0.003 * f64::from(k);
                let d = [math::cos(latitude), 0.0, math::sin(latitude)];
                let stencil = grid.stencil_at(d);
                let (plain, clamped) = (stencil.apply(&field), stencil.apply_clamped(&field));
                over |= plain > 400.0 + 1e-9;
                under |= plain < 30.0 - 1e-9;
                let (low, high) = stencil.range(&field);
                assert!(
                    (low..=high).contains(&clamped) && (30.0..=400.0).contains(&clamped),
                    "{latitude}: {clamped}"
                );
                if (low..=high).contains(&plain) {
                    assert_same_bits(plain, clamped);
                }
            }
            assert!(over && under, "the cubic overshoots a jump");
        }
    }

    /// A dense solve by Gaussian elimination with partial pivoting, the cyclic solve's check.
    fn dense_solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
        let n = b.len();
        for col in 0..n {
            let pivot = (col..n)
                .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
                .unwrap();
            a.swap(col, pivot);
            b.swap(col, pivot);
            for row in col + 1..n {
                let factor = a[row][col] / a[col][col];
                let pivot_row = a[col].clone();
                for (entry, &above) in a[row].iter_mut().zip(&pivot_row).skip(col) {
                    *entry -= factor * above;
                }
                b[row] -= factor * b[col];
            }
        }
        let mut x = vec![0.0; n];
        for row in (0..n).rev() {
            let tail: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
            x[row] = (b[row] - tail) / a[row][row];
        }
        x
    }

    /// The cyclic solve gives a dense solve's answer for diagonally dominant periodic systems of
    /// three unknowns, nine and a grid's 144, with corners of either sign, and the model's own
    /// form: equal couplings round a band over a varying diagonal.
    #[test]
    fn the_cyclic_solve_matches_a_dense_solve() {
        for n in [3, 9, LOCKED_COLUMNS] {
            for model_form in [false, true] {
                let at = |i: usize, scale: f64| math::sin(scale * index(i) + 0.3);
                let (lower, upper, diagonal): (Vec<f64>, Vec<f64>, Vec<f64>) = if model_form {
                    let off = -1.7e3;
                    (
                        vec![off; n],
                        vec![off; n],
                        (0..n)
                            .map(|i| 2.0 * 1.7e3 + 300.0 + 200.0 * at(i, 0.7))
                            .collect(),
                    )
                } else {
                    (
                        (0..n).map(|i| -1.0 + 0.4 * at(i, 1.3)).collect(),
                        (0..n).map(|i| 0.7 * at(i, 2.1)).collect(),
                        (0..n).map(|i| 4.0 + at(i, 0.5)).collect(),
                    )
                };
                let rhs: Vec<f64> = (0..n).map(|i| 10.0 * at(i, 0.9)).collect();
                let mut dense = vec![vec![0.0; n]; n];
                for i in 0..n {
                    dense[i][i] += diagonal[i];
                    dense[i][(i + n - 1) % n] += lower[i];
                    dense[i][(i + 1) % n] += upper[i];
                }
                let expected = dense_solve(dense, rhs.clone());
                let mut solution = vec![0.0; n];
                let mut work = CyclicWork::new(n);
                solve_cyclic_tridiagonal(&lower, &diagonal, &upper, &rhs, &mut solution, &mut work);
                for (i, (x, e)) in solution.iter().zip(&expected).enumerate() {
                    assert!(
                        (x - e).abs() < 1e-12 * (1.0 + e.abs()),
                        "n {n}, model form {model_form}, row {i}: {x} against {e}"
                    );
                }
            }
        }
    }

    /// The longitude coupling is the isotropic Laplacian's 1 ÷ (cos² φ Δλ²) to 1 part in 10⁴, the
    /// same in both hemispheres, and none on one column; each point's centre falls in it, and the
    /// band means and broadcast are each other's inverse on a field constant in longitude.
    #[test]
    fn the_columns_tile_the_bands_and_couple_as_the_laplacian_does() {
        assert_same_bits(ModelGrid::zonal().longitude_coupling(10), 0.0);
        for (grid, columns) in [
            (ModelGrid::geographic(), GEOGRAPHIC_COLUMNS),
            (ModelGrid::locked(), LOCKED_COLUMNS),
        ] {
            assert_eq!(grid.columns(), columns);
            assert_eq!(grid.points(), LATITUDE_BANDS * columns);
            let width = TAU / index(columns);
            for k in 0..LATITUDE_BANDS {
                let c = grid.centre_cosine(k);
                let laplacian = 1.0 / (c * c * width * width);
                let ratio = grid.longitude_coupling(k) / laplacian;
                assert!((ratio - 1.0).abs() < 1e-4, "band {k}: {ratio}");
                let mirror = LATITUDE_BANDS - 1 - k;
                assert!(
                    (grid.longitude_coupling(k) / grid.longitude_coupling(mirror) - 1.0).abs()
                        < 1e-12
                );
            }
            for point in 0..grid.points() {
                assert_eq!(grid.point_of(grid.centre_direction(point)), point);
            }
            // The seam: 180° is the last column, −180° the first.
            let equator = LATITUDE_BANDS / 2;
            assert_eq!(
                grid.point_of([-1.0, 0.0, 0.0]),
                grid.point(equator, columns - 1)
            );
            assert_eq!(grid.point_of([-1.0, -1e-15, 0.0]), grid.point(equator, 0));
            let bands: Vec<f64> = (0..LATITUDE_BANDS).map(|k| 200.0 + index(k)).collect();
            let means = grid.band_means(&grid.broadcast(&bands));
            assert_eq!(means.len(), bands.len());
            for (&mean, &band) in means.iter().zip(&bands) {
                assert_same_bits(mean, band);
            }
            assert!(
                (grid.mean(&grid.broadcast(&bands)) - ModelGrid::zonal().mean(&bands)).abs()
                    < 1e-12
            );
        }
    }

    /// A field smooth on the sphere, sampled at a grid's centres.
    fn smooth(direction: [f64; 3]) -> f64 {
        let [x, y, z] = direction;
        250.0 + 30.0 * x - 20.0 * y + 10.0 * z + 8.0 * x * y - 6.0 * z * z + 4.0 * x * x * z
    }

    /// The unit vector at latitude `latitude` and longitude `longitude`, radians.
    fn direction(latitude: f64, longitude: f64) -> [f64; 3] {
        let (s, c) = math::sin_cos(latitude);
        let (sl, cl) = math::sin_cos(longitude);
        [c * cl, c * sl, s]
    }

    /// The bicubic interpolation passes through every centre, reproduces a field constant in
    /// longitude as the zonal stencil does, and is continuous across the longitude seam and
    /// through both poles, where every meridian gives the same value, close to the field's own.
    #[test]
    fn the_bicubic_interpolation_is_continuous_across_the_seam_and_the_poles() {
        for grid in [ModelGrid::geographic(), ModelGrid::locked()] {
            let field: Vec<f64> = (0..grid.points())
                .map(|p| smooth(grid.centre_direction(p)))
                .collect();
            for point in (0..grid.points()).step_by(7) {
                let at = grid.stencil_at(grid.centre_direction(point)).apply(&field);
                assert!((at - field[point]).abs() < 1e-9, "point {point}: {at}");
            }
            // Away from the nodes, within the cubic's error of the field.
            for (latitude, longitude) in [(0.3, 1.1), (-1.2, -2.9), (1.5, 0.2), (-0.01, 3.1)] {
                let d = direction(latitude, longitude);
                let at = grid.stencil_at(d).apply(&field);
                assert!(
                    (at - smooth(d)).abs() < 2e-3,
                    "{latitude}, {longitude}: {at}"
                );
            }
            // The seam, from either side.
            for latitude in [-1.4, -0.5, 0.0, 0.7, 1.45] {
                let west = grid
                    .stencil_at(direction(latitude, -PI + 1e-9))
                    .apply(&field);
                let east = grid
                    .stencil_at(direction(latitude, PI - 1e-9))
                    .apply(&field);
                assert!(
                    (west - east).abs() < 1e-6,
                    "{latitude}: {west} against {east}"
                );
            }
            // The poles, from twelve meridians, and through each pole along a great circle.
            for pole in [-1.0_f64, 1.0] {
                let near = FRAC_PI_2 - 1e-7;
                let values: Vec<f64> = (0..12)
                    .map(|m| {
                        let longitude = -PI + index(m) * PI / 6.0 + 0.01;
                        grid.stencil_at(direction(pole * near, longitude))
                            .apply(&field)
                    })
                    .collect();
                let truth = smooth([0.0, 0.0, pole]);
                for v in &values {
                    assert!((v - values[0]).abs() < 1e-3, "{pole}: {values:?}");
                    assert!((v - truth).abs() < 1e-2, "{pole}: {v} against {truth}");
                }
            }
            // A field constant in longitude is the zonal one's.
            let bands: Vec<f64> = (0..LATITUDE_BANDS)
                .map(|k| 250.0 + 40.0 * grid.centre_cosine(k))
                .collect();
            let wide = grid.broadcast(&bands);
            for (latitude, longitude) in [(0.3, 1.1), (-1.56, 2.0), (1.0, -3.0)] {
                let d = direction(latitude, longitude);
                let zonal = ModelGrid::stencil(d[2]).apply(&bands);
                assert!((grid.stencil_at(d).apply(&wide) - zonal).abs() < 1e-11);
            }
        }
        // On one column the stencil is the zonal one, to the bit.
        let zonal = ModelGrid::zonal();
        let bands: Vec<f64> = (0..LATITUDE_BANDS).map(|k| 250.0 + index(k)).collect();
        for z in [-1.0_f64, -0.4, 0.2, 0.99] {
            assert_same_bits(
                zonal
                    .stencil_at([0.0, (1.0 - z * z).sqrt(), z])
                    .apply(&bands),
                ModelGrid::stencil(z).apply(&bands),
            );
        }
    }
}
