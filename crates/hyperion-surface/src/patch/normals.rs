//! A patch's normals: from the height's analytic gradient, body-fixed, as octahedral pairs (plan
//! R05, Design note 5).
//!
//! The gradient g of the height field at the spheroid point is taken into the tangent plane,
//! `g_t = g − (g · ν) ν`, and scaled by ρ ÷ (ρ + h), where ρ is the spheroid's normal-section
//! radius of curvature in the direction of `g_t`, 1/ρ = cos²α ÷ M + sin²α ÷ N with α the
//! direction's azimuth from north and M and N the meridional and prime-vertical radii (Euler's
//! theorem; R ÷ (R + h) on a sphere): a height gradient at height h is spread over a surface ρ + h
//! from the centre of curvature. The normal is `(ν − g_s) ÷ |ν − g_s|`, `g_s` the scaled gradient.
//! The scalar factor is exact along the meridian and the prime vertical; in between, the inverse of
//! (I + h S), S the shape operator, is not a multiple of `g_t`, an error of about (h ÷ R) e², 10⁻⁵
//! at 10 km, which the normals do not see.
//!
//! Octahedral encoding (Meyer et al. 2010, "On floating-point normal vectors", Computer Graphics
//! Forum 29, 1405; Cigolle et al. 2014, "A survey of efficient representations for independent
//! unit vectors", JCGT 3, 2): n ÷ (|x| + |y| + |z|) projected onto the octahedron, the lower
//! hemisphere folded over the diagonals, with the sign of a zero taken as +1. The pair is returned
//! as `f32`; the worker packs it into half floats for the GPU, outside this crate, which never
//! reads float bits.

use super::{HeightSource, NormalScale, PATCH_VERTICES};
use crate::cube::PatchKey;
use crate::height::HeightSample;
use crate::num::assert_finite;
use crate::spheroid::Spheroid;

/// +1 for x ≥ 0 (zeros of either sign included), −1 otherwise.
#[must_use]
fn sign_not_zero(x: f64) -> f64 {
    if x < 0.0 { -1.0 } else { 1.0 }
}

/// The octahedral pair of the unit vector `n`.
#[must_use]
pub fn encode_octahedral(n: [f64; 3]) -> [f64; 2] {
    let l1 = n[0].abs() + n[1].abs() + n[2].abs();
    let (x, y) = (n[0] / l1, n[1] / l1);
    if n[2] >= 0.0 {
        [x, y]
    } else {
        [
            (1.0 - y.abs()) * sign_not_zero(x),
            (1.0 - x.abs()) * sign_not_zero(y),
        ]
    }
}

/// The octahedral pair of the unit vector `n` ([`encode_octahedral`]) narrowed to `f32`, as a
/// bake stores its normals and the worker's normal map its pairs (R10.T10.g).
#[must_use]
pub(crate) fn encode_octahedral_f32(n: [f64; 3]) -> [f32; 2] {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an octahedral coordinate in [-1, 1] narrows to f32 by design"
    )]
    let narrowed = encode_octahedral(n).map(|c| c as f32);
    narrowed
}

/// The unit vector of the octahedral pair `e`.
#[must_use]
pub fn decode_octahedral(e: [f64; 2]) -> [f64; 3] {
    let (x, y) = (e[0], e[1]);
    let z = 1.0 - x.abs() - y.abs();
    let (x, y) = if z < 0.0 {
        (
            (1.0 - y.abs()) * sign_not_zero(x),
            (1.0 - x.abs()) * sign_not_zero(y),
        )
    } else {
        (x, y)
    };
    let len = (x * x + y * y + z * z).sqrt();
    [x / len, y / len, z / len]
}

/// The surface normal at the unit direction `dir` for a height `h_m` metres and its gradient
/// `gradient` (body-fixed, per metre) on `figure` (see the module documentation).
#[must_use]
pub fn surface_normal(figure: &Spheroid, dir: [f64; 3], h_m: f64, gradient: [f64; 3]) -> [f64; 3] {
    let nu = figure.normal(dir);
    let along = gradient[0] * nu[0] + gradient[1] * nu[1] + gradient[2] * nu[2];
    let tangent = [0, 1, 2].map(|a| gradient[a] - along * nu[a]);
    let size = (tangent[0] * tangent[0] + tangent[1] * tangent[1] + tangent[2] * tangent[2]).sqrt();
    let scaled = if size > 0.0 {
        let unit = tangent.map(|t| t / size);
        let rho = figure.section_radius_m(dir, unit);
        let factor = rho / (rho + h_m);
        tangent.map(|t| t * factor)
    } else {
        tangent
    };
    let n = [0, 1, 2].map(|a| nu[a] - scaled[a]);
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    n.map(|c| c / len)
}

/// The unit normal at `dir` of `sample`, its height and gradient asserted finite: every baked
/// normal's, whichever of [`bake_normals`], [`draft_normals`] and [`normals_between`] makes it.
#[must_use]
fn sample_normal(figure: &Spheroid, dir: [f64; 3], sample: HeightSample) -> [f64; 3] {
    surface_normal(
        figure,
        dir,
        assert_finite(sample.height_m),
        sample.gradient.map(assert_finite),
    )
}

/// The index of vertex (`x`, `y`) in a patch's row-by-row vertices.
#[must_use]
fn vertex_index(x: u32, y: u32) -> usize {
    vertex_index_on(PATCH_VERTICES, x, y)
}

/// The patch's normals at `scale`, row by row, as octahedral pairs narrowed to `f32`.
///
/// `vertices` are the patch's 65 × 65 vertices' heights and gradients at its level, row by row
/// ([`PatchHeights::samples`](crate::synth::PatchHeights::samples)): a normal at a vertex reads
/// its sample, since [`NormalScale::Double`]'s even samples are the vertices bit for bit
/// ([`PatchKey::sample_dir`]), and the source is asked only at the samples between them, in row
/// order, as [`normals_between`] asks it.
///
/// # Errors
///
/// The source's own.
///
/// # Panics
///
/// If `vertices` is not the patch's 65 × 65.
pub(crate) fn bake_normals<S: HeightSource>(
    source: &S,
    key: PatchKey,
    scale: NormalScale,
    vertices: &[HeightSample],
    cache: &mut S::Cache,
) -> Result<Vec<f32>, S::Error> {
    assert_eq!(
        vertices.len(),
        PATCH_VERTICES * PATCH_VERTICES,
        "a patch's normals read its 65 × 65 vertices"
    );
    let figure = source.figure();
    let per_patch = scale.grid().quads();
    let side = scale.side();
    let mut out = Vec::with_capacity(2 * side * side);
    for y in 0..=per_patch {
        for x in 0..=per_patch {
            let dir = key.sample_dir(x, y, scale.grid());
            let sample = match vertex_of(scale, x, y) {
                Some((vx, vy)) => vertices[vertex_index(vx, vy)],
                None => source.height(cache, dir, key.level())?,
            };
            out.extend(encode_octahedral_f32(sample_normal(&figure, dir, sample)));
        }
    }
    Ok(out)
}

/// The vertex that sample (`x`, `y`) of `scale`'s grid is, or `None` for a sample between the
/// vertices.
#[must_use]
const fn vertex_of(scale: NormalScale, x: u32, y: u32) -> Option<(u32, u32)> {
    match scale {
        NormalScale::Mesh => Some((x, y)),
        NormalScale::Double if x.is_multiple_of(2) && y.is_multiple_of(2) => Some((x / 2, y / 2)),
        NormalScale::Double => None,
    }
}

/// The samples of [`NormalScale::Double`]'s grid between the vertices, three in four: 12,416.
pub const NORMALS_BETWEEN: usize =
    (2 * PATCH_VERTICES - 1) * (2 * PATCH_VERTICES - 1) - PATCH_VERTICES * PATCH_VERTICES;

/// The patch's normals at `scale` as a draft ([`NormalsLayer::Drafted`](super::NormalsLayer::Drafted)),
/// row by row, from its `vertices` alone, asking the source nothing: each vertex's
/// [`bake_normals`]' bit for bit, and each sample between them the mean of the unit normals of the
/// vertices about it (two along a row or a column, the quad's four within it), normalised. At
/// [`NormalScale::Mesh`] every sample is a vertex, so the draft is [`bake_normals`]' whole.
///
/// # Panics
///
/// If `vertices` is not the patch's 65 × 65.
#[must_use]
pub(crate) fn draft_normals(
    figure: &Spheroid,
    key: PatchKey,
    scale: NormalScale,
    vertices: &[HeightSample],
) -> Vec<f32> {
    assert_eq!(
        vertices.len(),
        PATCH_VERTICES * PATCH_VERTICES,
        "a patch's normals read its 65 × 65 vertices"
    );
    let grid = scale.grid();
    let per_patch = grid.quads();
    // A vertex's sample on the scale's grid: (x, y) on the mesh's, (2x, 2y) on the double's.
    let stride = per_patch / u32::try_from(PATCH_VERTICES - 1).expect("64 fits a u32");
    let mesh: Vec<[f64; 3]> = (0..PATCH_VERTICES * PATCH_VERTICES)
        .map(|n| {
            let at = |k: usize| u32::try_from(k).expect("a vertex index fits a u32") * stride;
            let dir = key.sample_dir(at(n % PATCH_VERTICES), at(n / PATCH_VERTICES), grid);
            sample_normal(figure, dir, vertices[n])
        })
        .collect();
    let side = scale.side();
    let mut out = Vec::with_capacity(2 * side * side);
    for y in 0..=per_patch {
        for x in 0..=per_patch {
            let n = vertex_of(scale, x, y).map_or_else(
                || mean_about(&mesh, x, y),
                |(vx, vy)| mesh[vertex_index(vx, vy)],
            );
            out.extend(encode_octahedral_f32(n));
        }
    }
    out
}

/// The mean of the unit normals `mesh` holds (the vertices', row by row) at the vertices about
/// sample (`x`, `y`) of the double grid, between them: two along a row or a column, the quad's
/// four within it; normalised.
#[must_use]
fn mean_about(mesh: &[[f64; 3]], x: u32, y: u32) -> [f64; 3] {
    let corners = [
        (x / 2, y / 2),
        (x.div_ceil(2), y / 2),
        (x / 2, y.div_ceil(2)),
        (x.div_ceil(2), y.div_ceil(2)),
    ];
    let mut sum = [0.0; 3];
    for (vx, vy) in corners {
        let m = mesh[vertex_index(vx, vy)];
        sum = [0, 1, 2].map(|a| sum[a] + m[a]);
    }
    let len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    // Neighbouring normals of one surface are never opposite; the guard keeps a degenerate mean
    // finite.
    if len > 0.0 {
        sum.map(|c| c / len)
    } else {
        mesh[vertex_index(x / 2, y / 2)]
    }
}

/// The normals of a [`draft_normals`]' samples between the vertices, row by row, each skipping
/// the vertices: [`bake_normals`]' at those samples bit for bit, asked of the source in the same
/// order, [`NORMALS_BETWEEN`] pairs at [`NormalScale::Double`] and none at [`NormalScale::Mesh`].
/// [`merge_between`] puts them in the draft's place.
///
/// # Errors
///
/// The source's own.
pub(crate) fn normals_between<S: HeightSource>(
    source: &S,
    key: PatchKey,
    scale: NormalScale,
    cache: &mut S::Cache,
) -> Result<Vec<f32>, S::Error> {
    let figure = source.figure();
    let per_patch = scale.grid().quads();
    let mut out = Vec::new();
    match scale {
        NormalScale::Mesh => {}
        NormalScale::Double => out.reserve(2 * NORMALS_BETWEEN),
    }
    for y in 0..=per_patch {
        for x in 0..=per_patch {
            if vertex_of(scale, x, y).is_none() {
                let dir = key.sample_dir(x, y, scale.grid());
                let sample = source.height(cache, dir, key.level())?;
                out.extend(encode_octahedral_f32(sample_normal(&figure, dir, sample)));
            }
        }
    }
    Ok(out)
}

/// `normals`, a bake's at `scale` row by row, with each sample between the vertices replaced, in
/// order, by `between`'s ([`refine_normals`](crate::bake::refine_normals)'): a draft's made the
/// bake's whole.
///
/// # Panics
///
/// If `normals` is not `scale`'s grid or `between` not its samples between the vertices.
#[must_use]
pub fn merge_between(normals: &[f32], between: &[f32], scale: NormalScale) -> Vec<f32> {
    let side = scale.side();
    assert_eq!(
        normals.len(),
        2 * side * side,
        "a bake's normals at {scale:?}"
    );
    let expected = match scale {
        NormalScale::Mesh => 0,
        NormalScale::Double => 2 * NORMALS_BETWEEN,
    };
    assert_eq!(
        between.len(),
        expected,
        "the samples between the vertices at {scale:?}"
    );
    let per_patch = scale.grid().quads();
    let mut merged = normals.to_vec();
    let mut next = between.as_chunks::<2>().0.iter();
    for y in 0..=per_patch {
        for x in 0..=per_patch {
            if vertex_of(scale, x, y).is_none() {
                let n = vertex_index_on(side, x, y);
                let pair = next.next().expect("one pair a sample between the vertices");
                merged[2 * n..2 * n + 2].copy_from_slice(pair);
            }
        }
    }
    merged
}

/// The index of sample (`x`, `y`) of a grid `side` samples a side, row by row.
#[must_use]
fn vertex_index_on(side: usize, x: u32, y: u32) -> usize {
    let index = |n: u32| usize::try_from(n).expect("a sample index fits a usize");
    index(y) * side + index(x)
}
