//! The base elevation: a body's coarse cells interpolated over the sphere, smooth and identical on
//! both sides of every face edge (plan R09, T4; Design note 13).
//!
//! # The interpolant
//!
//! Each face carries a uniform cubic B-spline in its cell coordinates X = s · 2ᴸ and Y = t · 2ᴸ,
//! its control points the cells' values at their centres, X = i + ½. Near an edge the spline needs
//! control points beyond the face, and the blend below evaluates it up to one cell outside, so
//! each face is extended by **ghost cells** three deep: a ghost is a cell of the face's own grid
//! continued past its edge, and its value is bilinear in the cells of the face that owns its
//! centre's direction (R05's canonical-face rule, ties to the lowest face index), its stencil
//! clamped to that face's cells.
//!
//! The faces are blended by a partition of unity. Face F's weight is
//! `w_F` = β(s)β(1 − s)β(t)β(1 − t), each factor a quintic smoothstep in cells, 1 on the face and
//! falling to 0 one cell beyond its edge: β(s) = S(X + 1) with S(x) = 6x⁵ − 15x⁴ + 10x³ on [0, 1].
//! The height is Σ `w_F` `f_F` ÷ Σ `w_F` over the faces whose weight is not zero, summed in face
//! order, every face's (s, t) taken from the same direction, the query's: its gnomonic coordinates
//! on that face's plane, unwarped. The direction's own face always has weight 1, so the sum is
//! never below 1. Within a cell of an edge the two faces' splines are blended, half and half on the
//! edge itself; within a cell of a cube corner, three.
//!
//! # Its properties
//!
//! - **The same bits from either side.** The result is a pure function of the direction, and a
//!   vertex on a face edge or a cube corner has one direction whichever patch asks (R05's
//!   `PatchKey::vertex_dir`), so both sides evaluate the same faces, weights and order and agree
//!   in value and gradient bit for bit.
//! - **C¹, not C².** Each spline and each weight is C² in its face's (s, t), and (s, t) is smooth
//!   in the direction except on the three great circles x = 0, y = 0 and z = 0, which carry the
//!   faces' twelve centre lines, where S2's quadratic warp s(u) is C¹ only (its second derivative
//!   jumps from 9 ÷ 8 to −9 ÷ 8 at u = 0). So the field is C¹ everywhere and C² away from those
//!   circles, across the face edges included.
//! - **No overshoot.** The B-spline is approximating: its basis is non-negative and sums to one,
//!   the ghosts are convex combinations of cells, and so is the blend, so no value leaves the
//!   range of the cells it read. It is a low-pass of the cells, not an interpolant through them:
//!   the centre of a cell more than one cell from a face edge takes 4/9 of the cell's value, 1/9
//!   of each edge neighbour's and 1/36 of each corner neighbour's (the basis at a knot is 1/6, 2/3,
//!   1/6 along each axis), and nearer an edge the blend mixes in the neighbouring face's spline,
//!   so the reconstructed field's variance is below the cells' (R09.T12.e measures `σ_h` on it).
//! - **A constant field interpolates to itself exactly.** Every control value is taken relative to
//!   the value of the query's own cell, so equal cells give zero terms and the reference back, bit
//!   for bit, with a gradient of +0 in every component (a field of −0 everywhere gives +0, since
//!   −0 + +0 is +0; a coarse field's elevations, whole millimetres, are never −0).
//! - **Its reads stay within the margin.** A query reads its own cell, up to 4 × 4 control points
//!   on each face it blends, and four cells for each ghost among them. The farthest are
//!   [`SYNTHESIS_MARGIN_CELLS`], 5 king moves from the query's cell, near the ends of the face
//!   edges, where a ghost three cells past an edge lies up to three cells along it from the query's
//!   row; in the middle of an edge they are 4 (T4's measurement at levels 6 to 8; at level 5, 4
//!   throughout).
//!
//! The gradient is analytic: the spline's and the weights' derivatives in (X, Y), through
//! ds/du = ¾ ÷ √(1 + 3|u|) (S2's inverse warp) and the gnomonic ∇u = (`e_u` − u n) ÷ (d · n) of
//! each face's axes, as a vector tangent to the unit sphere in the cells' unit per radian.
//! [`base_elevation`] turns it into R05's [`HeightSample`]: metres, and a body-fixed gradient per
//! metre at the spheroid point, the gradient of H(P) = h(M⁻¹P ÷ |M⁻¹P|) with M = diag(a, a, c),
//! which is the elevation itself on the spheroid, so that its tangential part is the surface
//! gradient R05's bake turns into normals. Only that part is: on an oblate body the gradient also
//! has a part along the spheroid's normal, up to f ÷ (1 − f) of the slope (0.3% on an Earth, 8% on
//! a Ceres), so a reader that wants the slope projects it into the tangent plane first, as R05's
//! `patch::normals::surface_normal` does.
//!
//! # Categorical fields
//!
//! Plate, crust, boundary kind, flow direction and the other categories are not interpolated: they
//! come from the cell that contains the direction ([`cell_at`]), R05's `PatchKey::containing` at
//! the field's level, which is the brainstorm's "nearest cell".
//!
//! [`SYNTHESIS_MARGIN_CELLS`]: crate::field::SYNTHESIS_MARGIN_CELLS

use crate::cube::{
    Face, FaceUv, MAX_LEVEL, PatchKey, face_of, face_uv_to_xyz, st_to_uv, uv_on_face, uv_to_st,
};
use crate::field::{FieldHeader, FieldView, SynthesisCell};
use crate::height::HeightSample;
use crate::num;

/// What the interpolant reads: one value for each cell of one level.
///
/// A coarse field's elevations are one ([`base_elevation`] reads them through a [`FieldView`]);
/// the coarse pass's working arrays at the levels of its multigrid are others (R09.T14.b).
pub trait CellValues {
    /// The level of the cells, 1 to [`MAX_LEVEL`].
    fn level(&self) -> u8;

    /// The value at `cell`, a cell of [`level`](Self::level), or `None` where it is not held.
    fn value(&self, cell: PatchKey) -> Option<f64>;
}

/// The interpolant's value at a direction and its gradient.
///
/// Plain data with public fields, as R05's [`HeightSample`] is: [`interpolate`] fills it, and
/// nothing reads it back as a checked value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interpolated {
    /// The value, in the cells' unit.
    pub value: f64,
    /// The value's gradient as a function of direction on the unit sphere, body-fixed: the cells'
    /// unit per radian, tangent to the sphere at the direction.
    pub gradient: [f64; 3],
}

/// Why the interpolant could not be evaluated: a cell it reads is not held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReadCellError {
    /// The first cell, in the interpolant's reading order, that the values do not hold.
    pub cell: PatchKey,
}

impl std::fmt::Display for ReadCellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = self.cell;
        write!(
            f,
            "cell ({}, {}) of face {} at level {} is not held",
            c.i(),
            c.j(),
            c.face().index(),
            c.level()
        )
    }
}

impl std::error::Error for ReadCellError {}

/// The interpolated value of `values` at the direction `dir`, and its gradient (see the module
/// documentation).
///
/// `dir` is a direction in the body-fixed frame, normally of unit length: the gnomonic coordinates
/// do not depend on its length, but the gradient is the unit sphere's at `dir` ÷ |`dir`| only when
/// it is a unit vector.
///
/// # Errors
///
/// [`ReadCellError`] naming the first cell, in a fixed reading order, that `values` does not hold.
///
/// # Panics
///
/// If `values`' level is 0 or above [`MAX_LEVEL`], or if `dir` is zero or has a component that is
/// not finite.
pub fn interpolate<V: CellValues + ?Sized>(
    values: &V,
    dir: [f64; 3],
) -> Result<Interpolated, ReadCellError> {
    let [one] = interpolate_each(&Single(values), dir, &mut Fresh)?;
    Ok(one)
}

/// What the fused pass reads: `N` values for each cell of one level, all from one read of the
/// cell (R09.T8's follow-up): the base elevation with the relief's three style values, or the three
/// alone.
pub(crate) trait CellRecords<const N: usize> {
    /// The level of the cells, 1 to [`MAX_LEVEL`].
    fn level(&self) -> u8;

    /// The values at `cell`, a cell of [`level`](Self::level), or `None` where it is not held.
    fn values(&self, cell: PatchKey) -> Option<[f64; N]>;
}

/// One [`CellValues`] as a fused pass's single value.
struct Single<'a, V: ?Sized>(&'a V);

impl<V: CellValues + ?Sized> CellRecords<1> for Single<'_, V> {
    fn level(&self) -> u8 {
        self.0.level()
    }

    fn values(&self, cell: PatchKey) -> Option<[f64; 1]> {
        self.0.value(cell).map(|v| [v])
    }
}

/// [`interpolate`] of `N` values at once, each bit for bit what [`interpolate`] gives of that
/// value alone (R09.T8's follow-up, the fused pass).
///
/// The values share everything but their own arithmetic: the query's cell, each face's weight and
/// spline basis, the stencil's walk and each cell's read, one read for all `N` values. Each value
/// takes the same operations in the same order as [`interpolate`] takes for it, so its value and
/// gradient are the same bits, and the error is the same: the first cell, in the one reading
/// order, that `values` does not hold. `controls` hands out the control points' values, read
/// afresh ([`Fresh`]) or from a patch bake's memo ([`ControlMemo`]), which changes no bit.
///
/// # Errors
///
/// [`ReadCellError`] naming the first cell, in a fixed reading order, that `values` does not hold.
///
/// # Panics
///
/// If `values`' level is 0 or above [`MAX_LEVEL`], or if `dir` is zero or has a component that is
/// not finite.
pub(crate) fn interpolate_each<const N: usize, V, C>(
    values: &V,
    dir: [f64; 3],
    controls: &mut C,
) -> Result<[Interpolated; N], ReadCellError>
where
    V: CellRecords<N> + ?Sized,
    C: Controls<N> + ?Sized,
{
    let level = values.level();
    assert!(
        (1..=MAX_LEVEL).contains(&level),
        "the interpolant reads levels 1 to {MAX_LEVEL}, not level {level}"
    );
    let home = PatchKey::containing(level, dir).expect("the level is at most MAX_LEVEL");
    let reference = controls.cell(values, home)?;
    let grid = Grid::new(level);
    let mut terms = [None; 6];
    for (slot, face) in terms.iter_mut().zip(Face::ALL) {
        *slot = face_term(values, controls, &grid, face, dir, &reference)?;
    }
    let mut weight = 0.0;
    let mut weighted = [0.0; N];
    for term in terms.iter().flatten() {
        weight += term.weight;
        for (w, value) in weighted.iter_mut().zip(term.value) {
            *w += term.weight * value;
        }
    }
    let relative = weighted.map(|w| w / weight);
    let mut gradient = [[0.0; 3]; N];
    for term in terms.iter().flatten() {
        for ((gradient, (value, relative)), value_gradient) in gradient
            .iter_mut()
            .zip(term.value.iter().zip(&relative))
            .zip(&term.value_gradient)
        {
            let lift = value - relative;
            for (g, (dw, df)) in gradient
                .iter_mut()
                .zip(term.weight_gradient.iter().zip(value_gradient))
            {
                *g += dw * lift + term.weight * df;
            }
        }
    }
    Ok(std::array::from_fn(|n| Interpolated {
        value: reference[n] + relative[n],
        gradient: gradient[n].map(|g| g / weight),
    }))
}

/// A coarse field's base elevation at the unit direction `dir`: its cells' elevations interpolated
/// (see the module documentation), in metres above the spheroid, with the gradient in body-fixed
/// space at the spheroid point, metres per metre, as R05's [`HeightSample`] carries it.
///
/// The same at every level of detail: the base elevation is the coarse field's, and the finer
/// bands are other contributions' (R09.T5–T8).
///
/// # Errors
///
/// [`ReadCellError`] naming the first cell read that `field` does not hold.
///
/// # Panics
///
/// If `dir` is zero or has a component that is not finite.
pub fn base_elevation<F: FieldView>(
    field: &F,
    dir: [f64; 3],
) -> Result<HeightSample, ReadCellError> {
    let mm = interpolate(&Elevations(field), dir)?;
    Ok(elevation_sample(field.header(), mm))
}

/// The base elevation of a field of header `header` from its interpolated elevations `mm`, in
/// millimetres and millimetres per radian: metres above the spheroid, with the gradient in
/// body-fixed space at the spheroid point, metres per metre ([`base_elevation`]; the fused pass
/// makes its millimetres with the relief's styles).
///
/// # Panics
///
/// If the height or a component of its gradient is not finite (a bug).
#[must_use]
pub(crate) fn elevation_sample(header: &FieldHeader, mm: Interpolated) -> HeightSample {
    let figure = header.figure();
    let radii = [
        figure.equatorial_radius_m,
        figure.equatorial_radius_m,
        figure.polar_radius_m,
    ];
    let mut gradient = [0.0; 3];
    for ((g, mm_per_rad), radius) in gradient.iter_mut().zip(mm.gradient).zip(radii) {
        *g = num::assert_finite(mm_per_rad / MM_PER_M / radius);
    }
    HeightSample {
        height_m: num::assert_finite(mm.value / MM_PER_M),
        gradient,
    }
}

/// The cell of `field` that contains the direction `dir`, and its record: where the categorical
/// fields hold (see the module documentation).
///
/// # Errors
///
/// [`ReadCellError`] if `field` does not hold the cell.
///
/// # Panics
///
/// If `dir` is zero or has a component that is not finite.
pub fn cell_at<F: FieldView>(
    field: &F,
    dir: [f64; 3],
) -> Result<(PatchKey, &SynthesisCell), ReadCellError> {
    let cell = PatchKey::containing(field.header().level().get(), dir)
        .expect("a coarse level is below the cube's deepest");
    field
        .cell(cell)
        .map(|record| (cell, record))
        .ok_or(ReadCellError { cell })
}

/// Millimetres a metre: elevations are carried in whole millimetres.
const MM_PER_M: f64 = 1_000.0;

/// A coarse field's elevations as the interpolant's values, in millimetres, so that every value
/// and every difference between two is an exact `f64`.
struct Elevations<'a, F>(&'a F);

impl<F: FieldView> CellValues for Elevations<'_, F> {
    fn level(&self) -> u8 {
        self.0.header().level().get()
    }

    fn value(&self, cell: PatchKey) -> Option<f64> {
        self.0.cell(cell).map(|c| f64::from(c.elevation_mm))
    }
}

/// A level's grid: its level, cells a face side as an integer and as an `f64`, and the |u| beyond
/// which a face's weight is zero.
pub(crate) struct Grid {
    level: u8,
    cells: i64,
    side: f64,
    /// The u of s = 1 + 2 ÷ 2ᴸ ([`st_to_uv`]): a point whose |u| or |v| on a face is at least this
    /// lies more than one cell beyond the face, where its weight is exactly zero, with a cell's
    /// margin over any rounding, so skipping it changes no bit.
    reach_uv: f64,
}

impl Grid {
    #[must_use]
    fn new(level: u8) -> Self {
        let cells = 1_i64 << level;
        let side = f64::from(1_u32 << level);
        Self {
            level,
            cells,
            side,
            reach_uv: st_to_uv(1.0 + 2.0 / side),
        }
    }

    /// The cell (`i`, `j`) of `face`, which must be on it.
    #[must_use]
    fn cell(&self, face: Face, i: i64, j: i64) -> PatchKey {
        let index = |n: i64| u32::try_from(n).expect("a cell index on its face is below 2^24");
        PatchKey::new(face, self.level, index(i), index(j)).expect("the cell is on its face")
    }

    #[must_use]
    fn on_face(&self, i: i64, j: i64) -> bool {
        (0..self.cells).contains(&i) && (0..self.cells).contains(&j)
    }
}

/// One face's term of the blend: its weight and its spline's `N` values relative to the query's
/// cell, each with its gradient on the unit sphere.
#[derive(Debug, Clone, Copy)]
struct FaceTerm<const N: usize> {
    weight: f64,
    weight_gradient: [f64; 3],
    value: [f64; N],
    value_gradient: [[f64; 3]; N],
}

/// A face's normal and its u and v axes in body-fixed space, by S2's axes ([`face_uv_to_xyz`]:
/// the point (u, v) of a face is n + u `e_u` + v `e_v`).
#[must_use]
pub(crate) const fn axes(face: Face) -> ([f64; 3], [f64; 3], [f64; 3]) {
    match face {
        Face::PosX => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        Face::PosY => ([0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        Face::PosZ => ([0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
        Face::NegX => ([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]),
        Face::NegY => ([0.0, -1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]),
        Face::NegZ => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    }
}

/// The dot product a · b, summed in index order.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `face`'s term at the direction `dir`, or `None` where its weight is zero.
fn face_term<const N: usize, V, C>(
    values: &V,
    controls: &mut C,
    grid: &Grid,
    face: Face,
    dir: [f64; 3],
    reference: &[f64; N],
) -> Result<Option<FaceTerm<N>>, ReadCellError>
where
    V: CellRecords<N> + ?Sized,
    C: Controls<N> + ?Sized,
{
    let (normal, e_u, e_v) = axes(face);
    let along = dot(dir, normal);
    if along <= 0.0 {
        return Ok(None);
    }
    let (u, v) = uv_on_face(face, dir);
    // Written so that a NaN, which no finite direction gives, is skipped rather than read.
    if !(u.abs() < grid.reach_uv && v.abs() < grid.reach_uv) {
        return Ok(None);
    }
    let x = uv_to_st(u) * grid.side;
    let y = uv_to_st(v) * grid.side;
    let (wx, dwx) = band(x, grid.side);
    let (wy, dwy) = band(y, grid.side);
    let weight = wx * wy;
    if weight <= 0.0 {
        return Ok(None);
    }
    // ∇X = 2ᴸ (ds/du) ∇u, with ∇u = (e_u − u n) ÷ (d · n), and likewise Y.
    let dx_du = grid.side * warp_slope(u);
    let dy_dv = grid.side * warp_slope(v);
    let grad_x = [0, 1, 2].map(|k| dx_du * ((e_u[k] - u * normal[k]) / along));
    let grad_y = [0, 1, 2].map(|k| dy_dv * ((e_v[k] - v * normal[k]) / along));
    let SplineSums { value, d_x, d_y } = spline(values, controls, grid, face, x, y, reference)?;
    let weight_slope = [dwx * wy, wx * dwy];
    Ok(Some(FaceTerm {
        weight,
        weight_gradient: [0, 1, 2]
            .map(|k| weight_slope[0] * grad_x[k] + weight_slope[1] * grad_y[k]),
        value,
        value_gradient: std::array::from_fn(|n| {
            [0, 1, 2].map(|k| d_x[n] * grad_x[k] + d_y[n] * grad_y[k])
        }),
    }))
}

/// ds/du of S2's inverse warp ([`uv_to_st`]): ¾ ÷ √(1 + 3u) for u ≥ 0 and ¾ ÷ √(1 − 3u) below,
/// one expression in |u|.
#[must_use]
pub(crate) fn warp_slope(u: f64) -> f64 {
    0.75 / (1.0 + 3.0 * u.abs()).sqrt()
}

/// The quintic smoothstep S(x) = 6x⁵ − 15x⁴ + 10x³ on [0, 1], 0 below and 1 above, and its
/// derivative 30x²(1 − x)², both C² at the ends.
#[must_use]
fn smoothstep(x: f64) -> (f64, f64) {
    if x <= 0.0 {
        (0.0, 0.0)
    } else if x >= 1.0 {
        (1.0, 0.0)
    } else {
        let x2 = x * x;
        let rest = 1.0 - x;
        (
            x2 * x * (x * (6.0 * x - 15.0) + 10.0),
            30.0 * x2 * rest * rest,
        )
    }
}

/// A face's weight along one axis at the cell coordinate `x` (s × 2ᴸ) of a grid of `side` cells a
/// side, and its derivative in `x`: β(s)β(1 − s), 1 on the face and 0 from one cell beyond either
/// edge.
#[must_use]
fn band(x: f64, side: f64) -> (f64, f64) {
    let (low, d_low) = smoothstep(x + 1.0);
    let (high, d_high) = smoothstep(side - x + 1.0);
    (low * high, d_low * high - low * d_high)
}

/// The uniform cubic B-spline's four basis weights at the fraction `f` of a knot span, and their
/// derivatives in `f`: (1 − f)³ ÷ 6, (3f³ − 6f² + 4) ÷ 6, (−3f³ + 3f² + 3f + 1) ÷ 6 and f³ ÷ 6,
/// each non-negative on [0, 1] and summing to one.
#[must_use]
fn basis(f: f64) -> ([f64; 4], [f64; 4]) {
    let g = 1.0 - f;
    let f2 = f * f;
    let f3 = f2 * f;
    (
        [
            g * g * g / 6.0,
            (3.0 * f3 - 6.0 * f2 + 4.0) / 6.0,
            (-3.0 * f3 + 3.0 * f2 + 3.0 * f + 1.0) / 6.0,
            f3 / 6.0,
        ],
        [
            -0.5 * g * g,
            1.5 * f2 - 2.0 * f,
            -1.5 * f2 + f + 0.5,
            0.5 * f2,
        ],
    )
}

/// The first of the four control points about the cell coordinate `x` and the fraction of its
/// knot span: the knots are the cell centres, at i + ½.
#[must_use]
fn knot_span(x: f64) -> (i64, f64) {
    let knot = x - 0.5;
    let first = knot.floor();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the floor is an integer within two cells of the face, far inside an i64"
    )]
    let index = first as i64;
    (index - 1, knot - first)
}

/// A face's spline at a point, each of its `N` values relative to its reference: the value and
/// its derivatives in the cell coordinates x and y.
struct SplineSums<const N: usize> {
    value: [f64; N],
    d_x: [f64; N],
    d_y: [f64; N],
}

/// Face `face`'s spline at the cell coordinates (`x`, `y`), each value relative to its
/// `reference`, and their derivatives in x and y; the 4 × 4 control points summed row by row, each
/// row in order of i.
fn spline<const N: usize, V, C>(
    values: &V,
    controls: &mut C,
    grid: &Grid,
    face: Face,
    x: f64,
    y: f64,
    reference: &[f64; N],
) -> Result<SplineSums<N>, ReadCellError>
where
    V: CellRecords<N> + ?Sized,
    C: Controls<N> + ?Sized,
{
    let (i0, fx) = knot_span(x);
    let (j0, fy) = knot_span(y);
    let (wx, dx) = basis(fx);
    let (wy, dy) = basis(fy);
    let mut value = [0.0; N];
    let mut d_x = [0.0; N];
    let mut d_y = [0.0; N];
    for (j, (wj, dj)) in (j0..).zip(wy.iter().zip(&dy)) {
        let mut row = [0.0; N];
        let mut row_dx = [0.0; N];
        for (i, (wi, di)) in (i0..).zip(wx.iter().zip(&dx)) {
            let c = controls.relative(values, grid, face, i, j, reference)?;
            for ((row, row_dx), c) in row.iter_mut().zip(&mut row_dx).zip(c) {
                *row += wi * c;
                *row_dx += di * c;
            }
        }
        for (((value, d_x), d_y), (row, row_dx)) in value
            .iter_mut()
            .zip(&mut d_x)
            .zip(&mut d_y)
            .zip(row.iter().zip(&row_dx))
        {
            *value += wj * row;
            *d_x += wj * row_dx;
            *d_y += dj * row;
        }
    }
    Ok(SplineSums { value, d_x, d_y })
}

/// What a control point of a face's spline reads, before it is taken relative to the query's cell:
/// a cell's `N` values on the face, or a ghost's four cells and its bilinear fractions beyond it.
///
/// It is a pure function of the cells' values, the level, the face and the control point's
/// indices, so a patch bake can keep it for its vertices' queries ([`ControlMemo`]).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Raw<const N: usize> {
    /// A cell of the face: its values.
    Cell([f64; N]),
    /// A ghost cell beyond the face: the values of the four cells about its centre's direction on
    /// the face that owns it, (i0, j0), (i0 + 1, j0), (i0, j0 + 1) and (i0 + 1, j0 + 1), with the
    /// bilinear fractions between them.
    Ghost {
        corners: [[f64; N]; 4],
        fx: f64,
        fy: f64,
    },
}

impl<const N: usize> Raw<N> {
    /// Control point (`i`, `j`) of `face`'s spline on `grid`, read from `values`: the cell's
    /// values on the face, or a ghost's four cells beyond it, read in [`ghost_stencil`]'s order.
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] naming the first cell `values` does not hold.
    fn read<V: CellRecords<N> + ?Sized>(
        values: &V,
        grid: &Grid,
        face: Face,
        i: i64,
        j: i64,
    ) -> Result<Self, ReadCellError> {
        if grid.on_face(i, j) {
            return read(values, grid.cell(face, i, j)).map(Self::Cell);
        }
        let ([k00, k10, k01, k11], fx, fy) = ghost_stencil(grid, face, i, j);
        Ok(Self::Ghost {
            corners: [
                read(values, k00)?,
                read(values, k10)?,
                read(values, k01)?,
                read(values, k11)?,
            ],
            fx,
            fy,
        })
    }

    /// Each value relative to its `reference`: the cell's value less it, or the ghost's bilinear
    /// blend of its four cells' values less it, in the one-value interpolant's order.
    #[must_use]
    fn relative(&self, reference: &[f64; N]) -> [f64; N] {
        match *self {
            Self::Cell(v) => less(v, reference),
            Self::Ghost {
                corners: [c00, c10, c01, c11],
                fx,
                fy,
            } => bilinear(
                [
                    less(c00, reference),
                    less(c10, reference),
                    less(c01, reference),
                    less(c11, reference),
                ],
                fx,
                fy,
            ),
        }
    }
}

/// Each of `values` less its `reference`.
#[must_use]
fn less<const N: usize>(values: [f64; N], reference: &[f64; N]) -> [f64; N] {
    std::array::from_fn(|n| values[n] - reference[n])
}

/// Each value's bilinear blend of a ghost's four cells' (relative) values `c00`, `c10`, `c01`,
/// `c11` at the fractions `fx` and `fy`.
#[must_use]
fn bilinear<const N: usize>([c00, c10, c01, c11]: [[f64; N]; 4], fx: f64, fy: f64) -> [f64; N] {
    std::array::from_fn(|n| {
        let low = c00[n] + fx * (c10[n] - c00[n]);
        let high = c01[n] + fx * (c11[n] - c01[n]);
        low + fy * (high - low)
    })
}

/// Where the fused pass takes its control points' values: read afresh each time ([`Fresh`]), or
/// kept by a patch bake ([`ControlMemo`]).
///
/// Neither changes a bit, since a control point's values are a pure function of the cells it
/// reads, and each takes the same operations on them: [`less`], then [`bilinear`] for a ghost.
pub(crate) trait Controls<const N: usize> {
    /// The values of `cell`, the query's own cell, of `values`' level.
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] if `values` does not hold `cell`.
    fn cell<V: CellRecords<N> + ?Sized>(
        &mut self,
        values: &V,
        cell: PatchKey,
    ) -> Result<[f64; N], ReadCellError>;

    /// Control point (`i`, `j`) of `face`'s spline on `grid`, each value relative to its
    /// `reference`: the cell's values on the face, or a ghost's beyond it.
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] naming the first cell, in the reading order, that `values` does not hold.
    fn relative<V: CellRecords<N> + ?Sized>(
        &mut self,
        values: &V,
        grid: &Grid,
        face: Face,
        i: i64,
        j: i64,
        reference: &[f64; N],
    ) -> Result<[f64; N], ReadCellError>;
}

/// The control points read afresh at every query: the point query's way.
///
/// It reads each cell and takes it relative at once, as the one-value interpolant did, and a
/// ghost's cells as its stencil names them ([`fresh_ghost`]), rather than through a [`Raw`] as the
/// memo does (the same operations on the same values). Measured on the Earth-like world's patch at
/// L + 4, which reads ghosts, the one-value base elevation's sweep (2026-10-14 and 15):
/// - read through a `Raw`, natively 13.37 ms against 11.56 ms before (loads 3.4 and 1.0);
/// - with the ghost's four cells keyed first ([`ghost_stencil`]), on Electron's V8 14.08 ms
///   against 8.67 ms before (loads 10 and 14);
/// - as it is, on V8 8.65 ms (load 7) and 8.84 ms (load 14) against 8.61 ms before (load 9), and
///   natively 11.79 ms against 11.54 ms before (loads 11 and 1.2): the same, within the loads'
///   noise.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Fresh;

impl<const N: usize> Controls<N> for Fresh {
    fn cell<V: CellRecords<N> + ?Sized>(
        &mut self,
        values: &V,
        cell: PatchKey,
    ) -> Result<[f64; N], ReadCellError> {
        read(values, cell)
    }

    fn relative<V: CellRecords<N> + ?Sized>(
        &mut self,
        values: &V,
        grid: &Grid,
        face: Face,
        i: i64,
        j: i64,
        reference: &[f64; N],
    ) -> Result<[f64; N], ReadCellError> {
        if grid.on_face(i, j) {
            return Ok(less(read(values, grid.cell(face, i, j))?, reference));
        }
        fresh_ghost(values, grid, face, i, j, reference)
    }
}

/// Ghost cell (`i`, `j`) of `face` read afresh, each value relative to its `reference`: the
/// one-value interpolant's ghost, each cell read and taken relative as its stencil names it.
fn fresh_ghost<const N: usize, V: CellRecords<N> + ?Sized>(
    values: &V,
    grid: &Grid,
    face: Face,
    i: i64,
    j: i64,
    reference: &[f64; N],
) -> Result<[f64; N], ReadCellError> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a ghost's index is within three cells of a face of at most 2^24, exact in f64"
    )]
    let centre = |n: i64| (2 * n + 1) as f64 / (2.0 * grid.side);
    let point = face_uv_to_xyz(FaceUv {
        face,
        u: st_to_uv(centre(i)),
        v: st_to_uv(centre(j)),
    });
    let owner = face_of(point);
    let (u, v) = uv_on_face(owner, point);
    let (i0, fx) = stencil(uv_to_st(u), grid);
    let (j0, fy) = stencil(uv_to_st(v), grid);
    let c00 = less(read(values, grid.cell(owner, i0, j0))?, reference);
    let c10 = less(read(values, grid.cell(owner, i0 + 1, j0))?, reference);
    let c01 = less(read(values, grid.cell(owner, i0, j0 + 1))?, reference);
    let c11 = less(read(values, grid.cell(owner, i0 + 1, j0 + 1))?, reference);
    Ok(bilinear([c00, c10, c01, c11], fx, fy))
}

/// The slots of a [`ControlMemo`]: a power of two, about ten times the control points a patch's
/// queries read when it lies within one coarse cell (25 a face, at most three faces), so that few
/// of them share a slot. At the fused pass's four values a slot is about 176 bytes, the memo about
/// 176 KiB.
const MEMO_SLOTS: usize = 1 << 10;

/// The control points a patch bake's queries have read, kept by face and indices so that its
/// vertices share them (R09.T8's follow-up): its 5,314 queries read about the same 25 a face.
///
/// It is direct-mapped and bounded, [`MEMO_SLOTS`] slots, a control point replacing the one in
/// its slot, so a bake whose vertices lie in many coarse cells reads afresh what it no longer
/// holds. It holds only control points every cell of which was read: a query that reads a cell
/// not held reads it afresh, and is refused as before. What it holds is for one view of one field
/// at one level, whose held cells only grow ([`crate::synth::SynthCache`]'s rule), and it serves
/// that alone.
///
/// Measured (2026-10-14, natively, one thread, load 1–4; plan R09's Risks, the fused pass's
/// record): the fused pass with this memo takes R09's bake of the Earth-like world's patch at
/// L + 4 from 103.8 ms to 34.2 ms, since each cell's read and style (a sea floor's takes powers
/// and logarithms) and each ghost's geometry are made once a patch rather than once a vertex.
#[derive(Debug, Clone, Default)]
pub(crate) struct ControlMemo<const N: usize> {
    slots: Vec<Option<MemoSlot<N>>>,
}

/// A [`ControlMemo`]'s slot: a control point's face, indices and values.
#[derive(Debug, Clone, Copy)]
struct MemoSlot<const N: usize> {
    face: Face,
    i: i64,
    j: i64,
    raw: Raw<N>,
}

impl<const N: usize> ControlMemo<N> {
    /// An empty memo, which allocates its slots at its first read.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Forgets every control point, keeping the slots' allocation.
    pub(crate) fn clear(&mut self) {
        self.slots.fill(None);
    }
}

/// The slot of control point (`i`, `j`) of `face`: a multiplicative hash of the three.
#[must_use]
fn memo_slot(face: Face, i: i64, j: i64) -> usize {
    let mixed = (i.cast_unsigned().wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ j.cast_unsigned().wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ u64::from(face.index()).wrapping_mul(0x1656_67b1_9e37_79f9))
    .rotate_left(32);
    usize::try_from(mixed % u64::try_from(MEMO_SLOTS).expect("the slots fit a u64"))
        .expect("a slot below MEMO_SLOTS fits a usize")
}

impl<const N: usize> ControlMemo<N> {
    /// The slot of control point (`i`, `j`) of `face`, the slots allocated at the first.
    fn slot(&mut self, face: Face, i: i64, j: i64) -> &mut Option<MemoSlot<N>> {
        if self.slots.is_empty() {
            self.slots.resize(MEMO_SLOTS, None);
        }
        &mut self.slots[memo_slot(face, i, j)]
    }
}

impl<const N: usize> Controls<N> for ControlMemo<N> {
    fn cell<V: CellRecords<N> + ?Sized>(
        &mut self,
        values: &V,
        cell: PatchKey,
    ) -> Result<[f64; N], ReadCellError> {
        let (face, i, j) = (cell.face(), i64::from(cell.i()), i64::from(cell.j()));
        let slot = self.slot(face, i, j);
        if let Some(MemoSlot {
            face: kept_face,
            i: kept_i,
            j: kept_j,
            raw: Raw::Cell(v),
        }) = *slot
            && (kept_face, kept_i, kept_j) == (face, i, j)
        {
            return Ok(v);
        }
        let v = read(values, cell)?;
        *slot = Some(MemoSlot {
            face,
            i,
            j,
            raw: Raw::Cell(v),
        });
        Ok(v)
    }

    fn relative<V: CellRecords<N> + ?Sized>(
        &mut self,
        values: &V,
        grid: &Grid,
        face: Face,
        i: i64,
        j: i64,
        reference: &[f64; N],
    ) -> Result<[f64; N], ReadCellError> {
        let slot = self.slot(face, i, j);
        if let Some(kept) = slot
            && (kept.face, kept.i, kept.j) == (face, i, j)
        {
            return Ok(kept.raw.relative(reference));
        }
        let raw = Raw::read(values, grid, face, i, j)?;
        *slot = Some(MemoSlot { face, i, j, raw });
        Ok(raw.relative(reference))
    }
}

/// `cell`'s values.
fn read<const N: usize, V: CellRecords<N> + ?Sized>(
    values: &V,
    cell: PatchKey,
) -> Result<[f64; N], ReadCellError> {
    values.values(cell).ok_or(ReadCellError { cell })
}

/// Ghost cell (`i`, `j`) of `face`, a cell of its grid beyond its edge: the four cells about its
/// centre's direction on the face that owns it, in their reading order (i0, j0), (i0 + 1, j0),
/// (i0, j0 + 1), (i0 + 1, j0 + 1), and its bilinear fractions there, the stencil clamped to that
/// face's cells.
#[must_use]
fn ghost_stencil(grid: &Grid, face: Face, i: i64, j: i64) -> ([PatchKey; 4], f64, f64) {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a ghost's index is within three cells of a face of at most 2^24, exact in f64"
    )]
    let centre = |n: i64| (2 * n + 1) as f64 / (2.0 * grid.side);
    let point = face_uv_to_xyz(FaceUv {
        face,
        u: st_to_uv(centre(i)),
        v: st_to_uv(centre(j)),
    });
    let owner = face_of(point);
    let (u, v) = uv_on_face(owner, point);
    let (i0, fx) = stencil(uv_to_st(u), grid);
    let (j0, fy) = stencil(uv_to_st(v), grid);
    (
        [
            grid.cell(owner, i0, j0),
            grid.cell(owner, i0 + 1, j0),
            grid.cell(owner, i0, j0 + 1),
            grid.cell(owner, i0 + 1, j0 + 1),
        ],
        fx,
        fy,
    )
}

/// The bilinear stencil's first cell and fraction at the face coordinate `s`: the cell centres'
/// coordinate s × 2ᴸ − ½, clamped to the face's centres, so that a point within half a cell of an
/// edge takes the edge cell's value.
#[must_use]
fn stencil(s: f64, grid: &Grid) -> (i64, f64) {
    let last = grid.side - 1.0;
    let x = num::min(num::max(s * grid.side - 0.5, 0.0), last);
    let first = num::min(x.floor(), last - 1.0);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the floor is an integer from 0 to 2^level − 2, exact in an i64"
    )]
    let index = first as i64;
    (index, x - first)
}

// The tests of `cell_at`, `QueryHeightError` and the base elevation's part of
// `Synthesiser::height_at` live here, since they are the interpolant's. The whole height function's
// own (its sum, its order and cache independence and its read set) are R09.T8's, in `synth::tests`;
// `a_cell_not_held_is_not_surveyed` still holds for the whole function, because its base elevation
// reads first.
#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    use super::*;
    use crate::cube::{Edge, PATCH_QUADS, unit_dir};
    use crate::field::{
        ClimateCell, CoarseCrater, CoarseField, Cover, FieldHeader, SYNTHESIS_MARGIN_CELLS,
        cell_index,
    };
    use crate::synth::{BandLevel, QueryHeightError, SynthCache, Synthesiser};
    use crate::testing::{CellSite, FieldBuilder, SyntheticWorld, synthetic_field};
    use hyperion_base::rng::DetailSeed;
    use hyperion_base::units::Metres;
    use hyperion_testkit::float::{assert_same_bits, bits};
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// Volumetric mean radii, m (NASA's planetary fact sheets; Ceres from Dawn): their fields are
    /// at levels 8, 7, 6 and 5.
    const EARTH_M: f64 = 6.371e6;
    const MARS_M: f64 = 3.3895e6;
    const MOON_M: f64 = 1.7374e6;
    const CERES_M: f64 = 4.697e5;

    fn earth() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| synthetic_field(SyntheticWorld::EarthLike))
    }

    fn ceres() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| synthetic_field(SyntheticWorld::CeresLike))
    }

    /// A 64-bit mix of `x` (`SplitMix64`'s finaliser), for values that vary from cell to cell.
    fn mix(mut x: u64) -> u64 {
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^ (x >> 31)
    }

    /// A Moon-sized field (level 6) whose cells are independent, uniform in ±8 km: the roughest a
    /// field can be, the hardest case for overshoot and the gradient.
    fn rough() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| {
            FieldBuilder::new(Metres::new(MOON_M))
                .elevation(|site: &CellSite| {
                    let cell = PatchKey::containing(6, site.dir).unwrap();
                    let word = mix(u64::from(cell_index(cell)) + 0x5eed);
                    #[expect(clippy::cast_precision_loss, reason = "a 53-bit fraction")]
                    let unit = (word >> 11) as f64 / (1_u64 << 53) as f64;
                    Metres::new(16_000.0 * unit - 8_000.0)
                })
                .build()
        })
    }

    /// A Moon-sized field (level 6) tilted along a direction, smooth everywhere: its splines have a
    /// slope across every face's centre lines.
    fn tilted() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| {
            FieldBuilder::new(Metres::new(MOON_M))
                .elevation(|site: &CellSite| {
                    let [x, y, z] = site.dir;
                    Metres::new(3_000.0 * (x + 2.0 * y + 3.0 * z))
                })
                .build()
        })
    }

    fn synthesiser(field: &CoarseField) -> Synthesiser<'_, CoarseField> {
        Synthesiser::new(field, DetailSeed::new(0x7465_7374))
    }

    fn height(field: &CoarseField, dir: [f64; 3]) -> HeightSample {
        base_elevation(field, dir).unwrap()
    }

    /// The direction of face coordinates (`s`, `t`) of `face`, which may lie beyond the face.
    fn st_dir(face: Face, s: f64, t: f64) -> [f64; 3] {
        unit_dir(face_uv_to_xyz(FaceUv {
            face,
            u: st_to_uv(s),
            v: st_to_uv(t),
        }))
    }

    /// Directions within a cell and a half of every face edge and cube corner at `level`: on every
    /// face, five places along each edge at nine offsets across it, and a 7 × 7 grid about each
    /// corner, in cells of the level.
    fn band_points(level: u8) -> Vec<[f64; 3]> {
        let cell = 1.0 / f64::from(1_u32 << level);
        let across = [-1.5, -1.0, -0.5, -0.1, 0.0, 0.1, 0.5, 1.0, 1.5];
        let along = [0.03, 0.21, 0.38, 0.62, 0.93];
        let near = [-1.5, -1.0, -0.4, 0.0, 0.4, 1.0, 1.5];
        let mut points = Vec::new();
        for face in Face::ALL {
            for &a in &along {
                for &c in &across {
                    let off = c * cell;
                    points.push(st_dir(face, off, a));
                    points.push(st_dir(face, 1.0 + off, a));
                    points.push(st_dir(face, a, off));
                    points.push(st_dir(face, a, 1.0 + off));
                }
            }
            for (cs, ct) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                for &ds in &near {
                    for &dt in &near {
                        points.push(st_dir(face, cs + ds * cell, ct + dt * cell));
                    }
                }
            }
        }
        points
    }

    /// `count` directions spread at random over the sphere.
    fn random_points(seed: u64, count: usize) -> Vec<[f64; 3]> {
        let mut rng = Lcg::new(seed);
        (0..count)
            .map(|_| {
                unit_dir([
                    rng.next_f64() - 0.5,
                    rng.next_f64() - 0.5,
                    rng.next_f64() - 0.5,
                ])
            })
            .collect()
    }

    /// A view of `inner` that records every cell asked of it.
    struct Recorder<'a> {
        inner: &'a CoarseField,
        reads: RefCell<Vec<PatchKey>>,
    }

    impl<'a> Recorder<'a> {
        fn new(inner: &'a CoarseField) -> Self {
            Self {
                inner,
                reads: RefCell::new(Vec::new()),
            }
        }

        /// The cells read since the last call, sorted and once each.
        fn take(&self) -> Vec<PatchKey> {
            let mut reads = self.reads.take();
            reads.sort_unstable();
            reads.dedup();
            reads
        }
    }

    impl FieldView for Recorder<'_> {
        fn header(&self) -> &FieldHeader {
            self.inner.header()
        }

        fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
            self.reads.borrow_mut().push(cell);
            self.inner.cell(cell)
        }

        fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
            self.reads.borrow_mut().push(cell);
            self.inner.climate(cell)
        }

        fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
            self.reads.borrow_mut().push(cell);
            self.inner.craters_reaching(cell)
        }
    }

    /// A view of `inner` that holds only the cells of `held`.
    struct Held<'a> {
        inner: &'a CoarseField,
        held: BTreeMap<PatchKey, u8>,
    }

    impl FieldView for Held<'_> {
        fn header(&self) -> &FieldHeader {
            self.inner.header()
        }

        fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
            self.held
                .contains_key(&cell)
                .then(|| self.inner.cell(cell))
                .flatten()
        }

        fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
            self.held
                .contains_key(&cell)
                .then(|| self.inner.climate(cell))
                .flatten()
        }

        fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
            let held = self.held.contains_key(&cell);
            self.inner.craters_reaching(cell).filter(move |_| held)
        }
    }

    /// The cells within `radius` king moves of `cell` (edge and corner neighbours, across face
    /// edges by the cube's neighbour rule), each with its distance: how
    /// [`SYNTHESIS_MARGIN_CELLS`] counts.
    fn king_distances(cell: PatchKey, radius: u8) -> BTreeMap<PatchKey, u8> {
        let mut distances = BTreeMap::from([(cell, 0)]);
        let mut ring = vec![cell];
        for step in 1..=radius {
            let mut next = Vec::new();
            for key in ring {
                let around = Edge::ALL
                    .into_iter()
                    .map(|e| key.edge_neighbour(e))
                    .chain(key.corner_neighbours().into_iter().flatten());
                for n in around {
                    distances.entry(n).or_insert_with(|| {
                        next.push(n);
                        step
                    });
                }
            }
            ring = next;
        }
        distances
    }

    fn sample_bits(s: HeightSample) -> [u64; 4] {
        [
            bits(s.height_m),
            bits(s.gradient[0]),
            bits(s.gradient[1]),
            bits(s.gradient[2]),
        ]
    }

    /// The bits of `eval` along `edge` of `key`'s vertices, in increasing x or y, with their
    /// directions' bits.
    fn edge_samples(
        key: PatchKey,
        edge: Edge,
        eval: &impl Fn([f64; 3]) -> [u64; 4],
    ) -> Vec<([u64; 3], [u64; 4])> {
        let quads = u8::try_from(PATCH_QUADS).unwrap();
        (0..=quads)
            .map(|n| match edge {
                Edge::UMin => key.vertex_dir(0, n),
                Edge::UMax => key.vertex_dir(quads, n),
                Edge::VMin => key.vertex_dir(n, 0),
                Edge::VMax => key.vertex_dir(n, quads),
            })
            .map(|d| (d.map(bits), eval(d)))
            .collect()
    }

    /// Asserts that `eval` gives the same bits from either side's patches of `level` along every
    /// face edge (at up to six patches an edge, the ends included) and from all three patches at
    /// every cube corner.
    fn assert_edges_and_corners_agree(level: u8, eval: impl Fn([f64; 3]) -> [u64; 4]) {
        let last = (1_u32 << level) - 1;
        let mut picks = vec![
            0,
            1.min(last),
            last / 3,
            last / 2,
            last.saturating_sub(1),
            last,
        ];
        picks.sort_unstable();
        picks.dedup();
        let mut crossings = 0;
        for face in Face::ALL {
            for edge in Edge::ALL {
                for &k in &picks {
                    let (i, j) = match edge {
                        Edge::UMin => (0, k),
                        Edge::UMax => (last, k),
                        Edge::VMin => (k, 0),
                        Edge::VMax => (k, last),
                    };
                    let key = PatchKey::new(face, level, i, j).unwrap();
                    let (neighbour, back) = key.edge_neighbour_and_back(edge);
                    assert_ne!(neighbour.face(), face, "{key:?} {edge:?}");
                    crossings += 1;
                    let ours = edge_samples(key, edge, &eval);
                    let mut theirs = edge_samples(neighbour, back, &eval);
                    if theirs[0].0 != ours[0].0 {
                        theirs.reverse();
                    }
                    assert_eq!(ours, theirs, "{key:?} {edge:?} and {neighbour:?} {back:?}");
                }
            }
        }
        assert_eq!(crossings, 6 * 4 * picks.len());
        // The eight cube corners, each the corner vertex of three patches.
        let quads = u8::try_from(PATCH_QUADS).unwrap();
        let mut corners: Vec<([i8; 3], [u64; 4])> = Vec::new();
        for face in Face::ALL {
            for (ci, cj, x, y) in [
                (0, 0, 0, 0),
                (last, 0, quads, 0),
                (0, last, 0, quads),
                (last, last, quads, quads),
            ] {
                let dir = PatchKey::new(face, level, ci, cj).unwrap().vertex_dir(x, y);
                let signs = dir.map(|c| if c > 0.0 { 1_i8 } else { -1 });
                corners.push((signs, eval(dir)));
            }
        }
        corners.sort_unstable();
        for group in corners.chunks(3) {
            assert!(group.iter().all(|c| c == &group[0]), "{group:?}");
        }
    }

    #[test]
    fn values_and_gradients_agree_bit_for_bit_along_every_face_edge_and_at_every_cube_corner() {
        for field in [earth(), ceres()] {
            let coarse = field.header().level().get();
            for level in [coarse, coarse + 1, coarse + 4] {
                assert_edges_and_corners_agree(level, |d| sample_bits(height(field, d)));
            }
        }
    }

    /// Values of one level's cells, a function of each cell's centre, recording the cells read:
    /// the interpolant at levels a coarse field never has, as T14.b's multigrid reads it.
    struct Values {
        level: u8,
        at: fn([f64; 3]) -> f64,
        reads: RefCell<Vec<f64>>,
    }

    impl Values {
        fn new(level: u8, at: fn([f64; 3]) -> f64) -> Self {
            Self {
                level,
                at,
                reads: RefCell::new(Vec::new()),
            }
        }
    }

    impl CellValues for Values {
        fn level(&self) -> u8 {
            self.level
        }

        fn value(&self, cell: PatchKey) -> Option<f64> {
            assert_eq!(cell.level(), self.level);
            let value = (self.at)(cell.vertex_dir(32, 32));
            self.reads.borrow_mut().push(value);
            Some(value)
        }
    }

    fn interpolated_bits(i: Interpolated) -> [u64; 4] {
        [
            bits(i.value),
            bits(i.gradient[0]),
            bits(i.gradient[1]),
            bits(i.gradient[2]),
        ]
    }

    /// A rough value in ±8,000 from a cell's centre: a mix of its direction's millionths.
    fn rough_value(dir: [f64; 3]) -> f64 {
        let mut word = 0x5eed_u64;
        for c in dir {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a unit vector's millionths fit an i64"
            )]
            let millionths = (c * 1e6).round() as i64;
            word = mix(word ^ millionths.cast_unsigned());
        }
        #[expect(clippy::cast_precision_loss, reason = "a 53-bit fraction")]
        let unit = (word >> 11) as f64 / (1_u64 << 53) as f64;
        16_000.0 * unit - 8_000.0
    }

    #[test]
    fn the_interpolant_holds_its_properties_at_levels_a_coarse_field_never_has() {
        for level in [1, 2, 4, 12] {
            let values = Values::new(level, rough_value);
            assert_edges_and_corners_agree(level + 1, |d| {
                interpolated_bits(interpolate(&values, d).unwrap())
            });
            let mut points = random_points(0x6c76_6c00 + u64::from(level), 300);
            points.extend(band_points(level));
            for &dir in &points {
                values.reads.take();
                let value = interpolate(&values, dir).unwrap().value;
                let reads = values.reads.take();
                let low = reads.iter().copied().fold(f64::INFINITY, num::min);
                let high = reads.iter().copied().fold(f64::NEG_INFINITY, num::max);
                assert!(
                    low <= value && value <= high,
                    "level {level} {dir:?}: {value}"
                );
            }
            let constant = Values::new(level, |_| -2_345.678);
            for &dir in &points {
                let i = interpolate(&constant, dir).unwrap();
                assert_same_bits(i.value, -2_345.678);
                for g in i.gradient {
                    assert_same_bits(g, 0.0);
                }
            }
        }
    }

    #[test]
    fn the_base_elevation_does_not_depend_on_the_order_of_queries() {
        // The whole height function's order and cache independence is R09.T8's
        // (`synth::tests`), with the bands that fill the cache.
        for field in [earth(), rough()] {
            let level = field.header().level().get();
            let mut points = random_points(0x6f72_6465, 300);
            points.extend(band_points(level).into_iter().step_by(7));
            assert_order_independent(&points, |&d| sample_bits(height(field, d)));
        }
    }

    #[test]
    fn a_synthesiser_is_clone_over_any_view() {
        fn clone<T: Clone>(t: &T) -> T {
            t.clone()
        }
        let synth = synthesiser(ceres());
        assert_eq!(clone(&synth).seed(), synth.seed());
        let held = Held {
            inner: ceres(),
            held: BTreeMap::new(),
        };
        let _ = clone(&synthesiser_of(&held));
    }

    /// The finite difference of H(P) = h(M⁻¹P ÷ |M⁻¹P|) along each body-fixed axis at the
    /// spheroid point of `dir`, `step` metres each way.
    fn central_difference(field: &CoarseField, dir: [f64; 3], step: f64) -> [f64; 3] {
        let figure = field.header().figure();
        let p = figure.point(dir);
        let radii = [
            figure.equatorial_radius_m,
            figure.equatorial_radius_m,
            figure.polar_radius_m,
        ];
        [0, 1, 2].map(|k| {
            let at = |sign: f64| {
                let mut q = p;
                q[k] += sign * step;
                height(field, unit_dir([0, 1, 2].map(|a| q[a] / radii[a]))).height_m
            };
            (at(1.0) - at(-1.0)) / (2.0 * step)
        })
    }

    fn norm(a: [f64; 3]) -> f64 {
        dot(a, a).sqrt()
    }

    #[test]
    fn the_gradient_matches_a_central_difference() {
        for (field, seed) in [(earth(), 0x6772_6164), (rough(), 0x726f_7567)] {
            let level = field.header().level().get();
            let mut points = random_points(seed, 1_500);
            points.extend(band_points(level));
            let mut checked = 0;
            for &dir in &points {
                let g = height(field, dir).gradient;
                let size = norm(g);
                // Slopes under 1 cm a kilometre are left out: a height sums 16 to 48 rounded
                // terms of up to 8 × 10⁶ mm, each rounding by up to 10⁻⁹ mm, so its rounding
                // reaches about 2 × 10⁻¹¹ m, 10⁻¹¹ a metre over the difference's 2 m: 10⁻⁶ of a
                // slope of 10⁻⁵.
                if size < 1e-5 {
                    continue;
                }
                checked += 1;
                let fd = central_difference(field, dir, 1.0);
                let error = norm([0, 1, 2].map(|k| fd[k] - g[k]));
                assert!(
                    error <= 1e-6 * size,
                    "at {dir:?}: gradient {g:?}, difference {fd:?}, relative {}",
                    error / size
                );
            }
            assert!(
                checked * 10 >= points.len() * 9,
                "{checked} of {} checked",
                points.len()
            );
        }
    }

    /// The height of `field` along the line of `face` at v = `v0`, at u = `u`, metres.
    fn along_u(field: &CoarseField, face: Face, u: f64, v0: f64) -> f64 {
        height(field, unit_dir(face_uv_to_xyz(FaceUv { face, u, v: v0 }))).height_m
    }

    /// The one-sided second differences of `phi` on each side of `at`, step `h`, and the central
    /// first difference.
    fn second_differences(phi: impl Fn(f64) -> f64, at: f64, h: f64) -> (f64, f64, f64) {
        let (m2, m1, z, p1, p2) = (
            phi(at - 2.0 * h),
            phi(at - h),
            phi(at),
            phi(at + h),
            phi(at + 2.0 * h),
        );
        (
            (z - 2.0 * m1 + m2) / (h * h),
            (p2 - 2.0 * p1 + z) / (h * h),
            (p1 - m1) / (2.0 * h),
        )
    }

    #[test]
    fn the_field_is_c1_but_not_c2_across_face_centre_lines() {
        let field = tilted();
        for face in Face::ALL {
            for v0 in [-0.55, -0.2, 0.35, 0.6] {
                // C¹: the gradients either side of u = 0 close in proportion to their distance.
                let gradient = |u: f64| {
                    height(field, unit_dir(face_uv_to_xyz(FaceUv { face, u, v: v0 }))).gradient
                };
                let gap = |d: f64| {
                    let (a, b) = (gradient(d), gradient(-d));
                    norm([0, 1, 2].map(|k| a[k] - b[k]))
                };
                let (wide, narrow) = (gap(1e-4), gap(1e-6));
                assert!(
                    narrow <= 0.02 * wide + 1e-18,
                    "{face:?} v = {v0}: gradient gaps {wide} at 1e-4, {narrow} at 1e-6"
                );
                // Not C²: the second derivative in u jumps by −3 φ′(0) at the warp's kink, s″
                // going from 9 ÷ 8 to −9 ÷ 8 while s′ = ¾, and is continuous away from it.
                let phi = |u: f64| along_u(field, face, u, v0);
                let (below, above, slope) = second_differences(phi, 0.0, 1e-4);
                let jump = above - below;
                assert!(
                    slope.abs() > 100.0,
                    "{face:?} v = {v0}: slope {slope} m per unit of u"
                );
                assert!(
                    (jump + 3.0 * slope).abs() <= 0.02 * (3.0 * slope).abs(),
                    "{face:?} v = {v0}: jump {jump}, expected {}",
                    -3.0 * slope
                );
                let (below, above, slope) = second_differences(phi, 0.3, 1e-4);
                assert!(
                    (above - below).abs() <= 0.02 * (3.0 * slope).abs(),
                    "{face:?} v = {v0}: a jump of {} at u = 0.3",
                    above - below
                );
            }
        }
    }

    #[test]
    fn the_field_is_c2_across_face_edges() {
        for field in [tilted(), rough()] {
            for face in Face::ALL {
                for v0 in [-0.7, -0.15, 0.4, 0.85] {
                    for edge_u in [-1.0, 1.0] {
                        let gradient = |d: f64| {
                            let u = edge_u + d;
                            height(field, unit_dir(face_uv_to_xyz(FaceUv { face, u, v: v0 })))
                                .gradient
                        };
                        let gap = |d: f64| {
                            let (a, b) = (gradient(d), gradient(-d));
                            norm([0, 1, 2].map(|k| a[k] - b[k]))
                        };
                        let (wide, narrow) = (gap(1e-4), gap(1e-6));
                        assert!(
                            narrow <= 0.02 * wide + 1e-15,
                            "{face:?} u = {edge_u}, v = {v0}: gaps {wide} and {narrow}"
                        );
                        let value_gap = (along_u(field, face, edge_u + 1e-9, v0)
                            - along_u(field, face, edge_u - 1e-9, v0))
                        .abs();
                        assert!(value_gap < 1e-3, "{face:?} {edge_u} {v0}: {value_gap} m");
                    }
                }
            }
        }
        // And C² there, unlike on the centre lines: no jump in the second derivative across an
        // edge, by the standard the centre-line test holds a point off the line to.
        let field = tilted();
        for face in Face::ALL {
            for v0 in [-0.7, -0.15, 0.4, 0.85] {
                for edge_u in [-1.0, 1.0] {
                    let phi = |u: f64| along_u(field, face, u, v0);
                    let (below, above, slope) = second_differences(phi, edge_u, 1e-4);
                    assert!(
                        (above - below).abs() <= 0.02 * (3.0 * slope).abs(),
                        "{face:?} u = {edge_u}, v = {v0}: a jump of {} against a slope of {slope}",
                        above - below
                    );
                }
            }
        }
    }

    #[test]
    fn a_constant_field_interpolates_to_itself() {
        let field = FieldBuilder::new(Metres::new(CERES_M))
            .elevation(|_| Metres::new(1_234.567))
            .build();
        let level = field.header().level().get();
        let elevation = field.synthesis()[0].elevation().value();
        let mut points = random_points(0xc0_57a7, 1_000);
        points.extend(band_points(level));
        for dir in points {
            let s = height(&field, dir);
            assert_same_bits(s.height_m, elevation);
            for g in s.gradient {
                assert_same_bits(g, 0.0);
            }
        }
    }

    #[test]
    fn no_value_leaves_the_range_of_the_cells_it_read() {
        for field in [rough(), earth()] {
            let level = field.header().level().get();
            let view = Recorder::new(field);
            let mut points = random_points(0x7261_6e67, 3_000);
            points.extend(band_points(level));
            for dir in points {
                let h = base_elevation(&view, dir).unwrap().height_m;
                let reads = view.take();
                let elevations = reads
                    .iter()
                    .map(|&c| field.cell(c).unwrap().elevation().value());
                let low = elevations.clone().fold(f64::INFINITY, num::min);
                let high = elevations.fold(f64::NEG_INFINITY, num::max);
                assert!(
                    low <= h && h <= high,
                    "{dir:?}: {h} outside [{low}, {high}] of {} cells",
                    reads.len()
                );
            }
        }
    }

    /// The flat fields of an Earth, a Mars, the Moon and Ceres: levels 8, 7, 6 and 5.
    fn flat_fields() -> &'static [CoarseField] {
        static FIELDS: OnceLock<Vec<CoarseField>> = OnceLock::new();
        FIELDS.get_or_init(|| {
            [EARTH_M, MARS_M, MOON_M, CERES_M]
                .map(|r| FieldBuilder::new(Metres::new(r)).build())
                .into()
        })
    }

    #[test]
    fn the_read_set_stays_within_the_synthesis_margin() {
        let margin = SYNTHESIS_MARGIN_CELLS;
        let mut farthest = 0;
        for (field, expected_level) in flat_fields().iter().zip([8, 7, 6, 5]) {
            let level = field.header().level().get();
            assert_eq!(level, expected_level);
            let view = Recorder::new(field);
            let mut points = random_points(0x7265_6164 + u64::from(level), 500);
            points.extend(band_points(level));
            let coarse = field.header().level();
            let mut rings: BTreeMap<PatchKey, (BTreeMap<PatchKey, u8>, Cover)> = BTreeMap::new();
            for dir in points {
                let home = PatchKey::containing(level, dir).unwrap();
                base_elevation(&view, dir).unwrap();
                // The distances by this test's count, and the margin a payload sends about the
                // cell (T3's `Cover::with_margin`), which must agree.
                let (distances, sent) = rings.entry(home).or_insert_with(|| {
                    let sent = Cover::from_cells([cell_index(home)]).with_margin(coarse, margin);
                    (king_distances(home, margin), sent)
                });
                for cell in view.take() {
                    let d = distances.get(&cell).copied().unwrap_or_else(|| {
                        panic!("{dir:?} in {home:?} read {cell:?}, beyond {margin} cells")
                    });
                    assert!(
                        sent.contains(cell_index(cell)),
                        "{cell:?} is not in the margin sent"
                    );
                    farthest = farthest.max(d);
                }
            }
        }
        // The whole margin, near the ends of the face edges: a ghost three cells past an edge
        // drifts up to three cells along it from the query's row, where the two faces' grids meet
        // obliquely, the spline reaches two more along the edge, and the ghost's stencil one more
        // (Design note 15's "about five at a corner"). A sweep of every face edge at 0.001 of its
        // length and 0.1 cell across, out to 2.6 cells either side, at levels 5 to 8 (T4, about
        // 6 × 10⁵ points a level), found 5 at levels 6 to 8, 4 at level 5, and never 6.
        assert_eq!(
            farthest, margin,
            "the farthest read is {farthest} cells away"
        );
    }

    #[test]
    fn a_cell_not_held_is_not_surveyed() {
        let field = ceres();
        let level = field.header().level().get();
        let mut cache = SynthCache::new();
        let band = BandLevel::new(level).unwrap();
        // A cell at a cube corner, one on a face edge, and one inside a face.
        let last = (1_u32 << level) - 1;
        for (face, i, j) in [
            (Face::PosX, 0, 0),
            (Face::NegY, last, 9),
            (Face::PosZ, 13, 17),
        ] {
            let cell = PatchKey::new(face, level, i, j).unwrap();
            let points = [
                (1, 1),
                (32, 32),
                (63, 1),
                (1, 63),
                (63, 63),
                (0, 0),
                (64, 64),
            ]
            .map(|(x, y)| cell.vertex_dir(x, y));
            // With the margin held, every point of the cell is answered, as from the whole field.
            let held = Held {
                inner: field,
                held: king_distances(cell, SYNTHESIS_MARGIN_CELLS),
            };
            for &dir in &points {
                let from_held = synthesiser_of(&held).height_at(&mut cache, dir, band);
                let from_whole = synthesiser(field).height_at(&mut cache, dir, band);
                assert_eq!(from_held, from_whole, "{cell:?} at {dir:?}");
                assert!(from_held.is_ok());
            }
            // With only the cell and its neighbours, a corner of it reads beyond them.
            let held = Held {
                inner: field,
                held: king_distances(cell, 1),
            };
            let dir = cell.vertex_dir(0, 0);
            match synthesiser_of(&held).height_at(&mut cache, dir, band) {
                Err(QueryHeightError::NotSurveyed(missing)) => {
                    assert!(!held.held.contains_key(&missing), "{missing:?} is held");
                    let whole = Recorder::new(field);
                    base_elevation(&whole, dir).unwrap();
                    assert!(whole.take().contains(&missing), "{missing:?} is not read");
                }
                other => panic!("{cell:?}: {other:?}"),
            }
        }
    }

    fn synthesiser_of<'a>(held: &'a Held<'a>) -> Synthesiser<'a, Held<'a>> {
        Synthesiser::new(held, DetailSeed::new(0x7465_7374))
    }

    #[test]
    fn with_nothing_finer_than_its_cells_the_height_is_the_base_elevation_at_every_level() {
        // The rough field has no fine relief, no craters and no drainage, so every level's bands
        // are zero; on the synthetic worlds the bands join it (R09.T8).
        let field = rough();
        let synth = synthesiser(field);
        let mut cache = SynthCache::new();
        for dir in random_points(0x6c65_7665, 200) {
            let base = sample_bits(height(field, dir));
            for level in [0, 8, 13, 19, MAX_LEVEL] {
                let level = BandLevel::new(level).unwrap();
                let s = synth.height_at(&mut cache, dir, level).unwrap();
                assert_eq!(sample_bits(s), base, "{dir:?} at {level:?}");
            }
        }
    }

    #[test]
    fn categorical_reads_take_the_cell_containing_the_direction() {
        let field = earth();
        let level = field.header().level().get();
        let synth = synthesiser(field);
        let mut points = random_points(0x6361_7465, 500);
        points.extend(band_points(level));
        for dir in points {
            let (cell, record) = synth.cell_at(dir).unwrap();
            assert_eq!(cell, PatchKey::containing(level, dir).unwrap());
            assert_eq!(record, field.cell(cell).unwrap());
        }
        let held = Held {
            inner: field,
            held: BTreeMap::new(),
        };
        let dir = [0.0, 0.6, 0.8];
        let cell = PatchKey::containing(level, dir).unwrap();
        assert_eq!(
            synthesiser_of(&held).cell_at(dir),
            Err(QueryHeightError::NotSurveyed(cell))
        );
    }

    #[test]
    fn a_cell_centre_takes_a_weighted_mean_of_its_neighbourhood() {
        // One raised cell in the middle of face 2 of a flat field: the spline is a low-pass, whose
        // basis at a knot is (1/6, 2/3, 1/6), so the cell's centre rises by (2/3)² of the cell's
        // height, an edge neighbour's by 2/3 × 1/6, and a cell two away not at all.
        let level = 5;
        let raised = PatchKey::new(Face::PosZ, level, 16, 16).unwrap();
        let field = FieldBuilder::new(Metres::new(CERES_M))
            .elevation(move |site: &CellSite| {
                if PatchKey::containing(level, site.dir).unwrap() == raised {
                    Metres::new(900.0)
                } else {
                    Metres::ZERO
                }
            })
            .build();
        let centre = |key: PatchKey| key.vertex_dir(32, 32);
        assert!((height(&field, centre(raised)).height_m - 400.0).abs() < 1e-9);
        let beside = PatchKey::new(Face::PosZ, level, 17, 16).unwrap();
        assert!((height(&field, centre(beside)).height_m - 100.0).abs() < 1e-9);
        let away = height(
            &field,
            centre(PatchKey::new(Face::PosZ, level, 18, 16).unwrap()),
        );
        assert!((0.0..1e-9).contains(&away.height_m), "{away:?}");
    }

    #[test]
    fn the_query_error_is_a_height_source_error() {
        fn is_source_error<E: std::error::Error + Send + Sync + 'static>() {}
        is_source_error::<QueryHeightError>();
        is_source_error::<ReadCellError>();
        let cell = PatchKey::new(Face::NegZ, 6, 3, 40).unwrap();
        assert_eq!(
            QueryHeightError::NotSurveyed(cell).to_string(),
            "cell (3, 40) of face 5 at level 6 is not surveyed or in a survey's margin"
        );
        assert_eq!(
            ReadCellError { cell }.to_string(),
            "cell (3, 40) of face 5 at level 6 is not held"
        );
    }
}
