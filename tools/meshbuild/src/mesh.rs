//! The builder's input, a triangle mesh in metres, and its quantised form on one grid.
//!
//! Every position is quantised first onto one grid for the whole mesh: 21 bits an axis about the
//! mesh's least corner, with a step that is a power of two, the least that spans the mesh's
//! largest extent in 2²¹ − 1 steps (1 mm on a 2 km mesh, 7.8 mm on a 10 km one). A page's vertex
//! is then its grid coordinates, which every page dequantises to the same `f32` (an integer below
//! 2²⁴ times a power of two is exact), so that a vertex two clusters share lands on the same bits
//! whichever page draws it, and the build works on the positions that are drawn: positions that
//! quantise to one grid point are welded, and triangles left with two corners on one point are
//! dropped. A leaf's error of 0 is relative to the quantised mesh, within half a step of the
//! source an axis. Normals are quantised to 16-bit octahedral coordinates and UVs to 16 bits on a
//! grid of their own, before the build, so that the vertices it tells apart are the ones a page can
//! tell apart.

use std::collections::HashMap;

use thiserror::Error;

use crate::num::{f32_down, f64_of, i64_of_integral, ix, u32_of};

/// The bits of a grid coordinate.
pub(crate) const GRID_BITS: u32 = 21;

/// The largest grid coordinate.
const GRID_MAX: f64 = 2_097_151.0;

/// The largest UV grid coordinate: 16 bits.
const UV_GRID_MAX: f64 = 65_535.0;

/// Why a [`SourceMesh`] was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NewSourceMeshError {
    /// The mesh has no triangle.
    #[error("the mesh has no triangle")]
    NoTriangles,
    /// A triangle names a vertex the mesh does not have.
    #[error("triangle {triangle} names vertex {vertex}, of {vertices}")]
    IndexOutOfRange {
        /// The triangle's index.
        triangle: usize,
        /// The vertex it names.
        vertex: u32,
        /// The mesh's vertex count.
        vertices: usize,
    },
    /// The normals or UVs are not one a vertex.
    #[error("{what} has {found} entries for {vertices} vertices")]
    AttributeCount {
        /// `"normals"` or `"uvs"`.
        what: &'static str,
        /// Its entries.
        found: usize,
        /// The mesh's vertex count.
        vertices: usize,
    },
    /// A position, normal or UV is NaN or infinite.
    #[error("vertex {vertex} has a non-finite {what}")]
    NotFinite {
        /// The vertex.
        vertex: usize,
        /// `"position"`, `"normal"` or `"uv"`.
        what: &'static str,
    },
    /// More vertices or triangles than [`MAX_ELEMENTS`].
    #[error("the mesh has more than {MAX_ELEMENTS} vertices or triangles")]
    TooLarge,
}

/// Why a mesh could not be quantised.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum QuantiseMeshError {
    /// The positions span nothing, or a range whose grid step `f32` cannot hold.
    #[error("the positions' extent, {extent_m} m, gives no 21-bit grid with an f32 step")]
    Extent {
        /// The largest extent over the three axes, m.
        extent_m: f64,
    },
    /// Every triangle has two corners on one grid point.
    #[error("every triangle is degenerate at the grid's step of {step_m} m")]
    AllDegenerate {
        /// The grid's step, m.
        step_m: f64,
    },
}

/// The most vertices, and the most triangles, a mesh may have: 2²⁸, so that every count the build
/// and the pages keep (three corners a triangle, about two triangles of DAG a source triangle) fits
/// 32 bits.
pub const MAX_ELEMENTS: usize = 1 << 28;

/// A triangle mesh as authored: positions in metres in the mesh's own frame, counter-clockwise
/// triangles seen from outside, and optionally a normal and a UV a vertex.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceMesh {
    positions: Vec<[f64; 3]>,
    normals: Option<Vec<[f64; 3]>>,
    uvs: Option<Vec<[f64; 2]>>,
    triangles: Vec<[u32; 3]>,
}

impl SourceMesh {
    /// A mesh from its positions and triangles, with no normals or UVs.
    ///
    /// # Errors
    ///
    /// [`NewSourceMeshError`] for no triangles, an index out of range, a non-finite position, or
    /// more than [`MAX_ELEMENTS`] vertices or triangles.
    pub fn new(
        positions: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    ) -> Result<Self, NewSourceMeshError> {
        let vertices = positions.len();
        if vertices > MAX_ELEMENTS || triangles.len() > MAX_ELEMENTS {
            return Err(NewSourceMeshError::TooLarge);
        }
        if triangles.is_empty() {
            return Err(NewSourceMeshError::NoTriangles);
        }
        for (triangle, corners) in triangles.iter().enumerate() {
            if let Some(&vertex) = corners.iter().find(|&&v| ix(v) >= vertices) {
                return Err(NewSourceMeshError::IndexOutOfRange {
                    triangle,
                    vertex,
                    vertices,
                });
            }
        }
        if let Some(vertex) = positions.iter().position(|p| !finite(p)) {
            return Err(NewSourceMeshError::NotFinite {
                vertex,
                what: "position",
            });
        }
        Ok(Self {
            positions,
            normals: None,
            uvs: None,
            triangles,
        })
    }

    /// The mesh with a normal a vertex. Normals need not be unit length; a zero normal takes the
    /// area-weighted mean of the faces' normals at the vertex's grid point, as every vertex does
    /// without normals.
    ///
    /// # Errors
    ///
    /// [`NewSourceMeshError::AttributeCount`] if they are not one a vertex, and
    /// [`NewSourceMeshError::NotFinite`] for a non-finite one.
    pub fn with_normals(mut self, normals: Vec<[f64; 3]>) -> Result<Self, NewSourceMeshError> {
        self.check_attribute(
            "normals",
            &normals.iter().map(|n| finite(n)).collect::<Vec<_>>(),
        )?;
        self.normals = Some(normals);
        Ok(self)
    }

    /// The mesh with a UV a vertex.
    ///
    /// # Errors
    ///
    /// [`NewSourceMeshError::AttributeCount`] if they are not one a vertex, and
    /// [`NewSourceMeshError::NotFinite`] for a non-finite one.
    pub fn with_uvs(mut self, uvs: Vec<[f64; 2]>) -> Result<Self, NewSourceMeshError> {
        self.check_attribute("uvs", &uvs.iter().map(|uv| finite(uv)).collect::<Vec<_>>())?;
        self.uvs = Some(uvs);
        Ok(self)
    }

    /// Checks an attribute's count and, by vertex, whether each value is finite.
    fn check_attribute(
        &self,
        what: &'static str,
        finite: &[bool],
    ) -> Result<(), NewSourceMeshError> {
        let vertices = self.positions.len();
        if finite.len() != vertices {
            return Err(NewSourceMeshError::AttributeCount {
                what,
                found: finite.len(),
                vertices,
            });
        }
        match finite.iter().position(|ok| !ok) {
            Some(vertex) => Err(NewSourceMeshError::NotFinite {
                vertex,
                what: if what == "uvs" { "uv" } else { "normal" },
            }),
            None => Ok(()),
        }
    }

    /// Its vertex positions, m.
    #[must_use]
    pub fn positions(&self) -> &[[f64; 3]] {
        &self.positions
    }

    /// Its triangles, as indices into [`Self::positions`].
    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// A vertex of the quantised mesh: a grid point and the attributes a page stores for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Vertex {
    /// The welded position, an index into [`QuantisedMesh::grid`].
    pub(crate) position: u32,
    /// The octahedral normal, as signed 16-bit normalised coordinates.
    pub(crate) normal: [i16; 2],
    /// The UV's grid coordinates.
    pub(crate) uv: [u16; 2],
}

/// The mesh on its grid: welded positions, deduplicated vertices and non-degenerate triangles.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QuantisedMesh {
    /// The grid's origin, the source positions' least corner, m.
    pub(crate) origin_m: [f64; 3],
    /// The grid's step, m: a power of two.
    pub(crate) step_m: f64,
    /// The UV grid's origin.
    pub(crate) uv_origin: [f32; 2],
    /// The UV grid's step: a power of two.
    pub(crate) uv_step: f32,
    /// Whether the source had UVs (else every UV is the origin).
    pub(crate) has_uvs: bool,
    /// Each welded position's grid coordinates, each below 2²¹.
    pub(crate) grid: Vec<[u32; 3]>,
    /// The vertices, in the order the source first uses them.
    pub(crate) vertices: Vec<Vertex>,
    /// The triangles, as indices into `vertices`.
    pub(crate) triangles: Vec<[u32; 3]>,
}

impl QuantisedMesh {
    /// Quantises `source` (see the module's documentation).
    pub(crate) fn new(source: &SourceMesh) -> Result<Self, QuantiseMeshError> {
        let (origin_m, step_m) = position_grid(&source.positions)?;
        let mut grid = Vec::new();
        let mut welded: HashMap<[u32; 3], u32> = HashMap::new();
        let position_of: Vec<u32> = source
            .positions
            .iter()
            .map(|p| {
                let q =
                    std::array::from_fn(|axis| grid_coordinate(p[axis], origin_m[axis], step_m));
                *welded.entry(q).or_insert_with(|| {
                    grid.push(q);
                    u32_of(grid.len() - 1)
                })
            })
            .collect();
        let corners: Vec<[u32; 3]> = source
            .triangles
            .iter()
            .copied()
            .filter(|t| {
                let p = t.map(|v| position_of[ix(v)]);
                p[0] != p[1] && p[1] != p[2] && p[2] != p[0]
            })
            .collect();
        if corners.is_empty() {
            return Err(QuantiseMeshError::AllDegenerate { step_m });
        }
        let face_normals = position_normals(&grid, &position_of, &corners);
        let (uv_origin, uv_step) = source.uvs.as_deref().map_or(([0.0; 2], 1.0), uv_grid);
        let mut vertices = Vec::new();
        let mut unique: HashMap<Vertex, u32> = HashMap::new();
        let mut vertex_of = vec![u32::MAX; source.positions.len()];
        let mut triangles = Vec::with_capacity(corners.len());
        for t in &corners {
            triangles.push(t.map(|v| {
                let v = ix(v);
                if vertex_of[v] == u32::MAX {
                    let position = position_of[v];
                    let authored = source.normals.as_ref().map(|n| n[v]);
                    let normal = octahedral(match authored {
                        Some(n) if length_squared(n) > 0.0 => n,
                        _ => face_normals[ix(position)],
                    });
                    let uv = source.uvs.as_ref().map_or([0, 0], |uvs| {
                        std::array::from_fn(|axis| {
                            let q =
                                (uvs[v][axis] - f64::from(uv_origin[axis])) / f64::from(uv_step);
                            u16::try_from(i64_of_integral(q.round().clamp(0.0, UV_GRID_MAX)))
                                .expect("clamped to 16 bits")
                        })
                    });
                    let vertex = Vertex {
                        position,
                        normal,
                        uv,
                    };
                    vertex_of[v] = *unique.entry(vertex).or_insert_with(|| {
                        vertices.push(vertex);
                        u32_of(vertices.len() - 1)
                    });
                }
                vertex_of[v]
            }));
        }
        Ok(Self {
            origin_m,
            step_m,
            uv_origin,
            uv_step,
            has_uvs: source.uvs.is_some(),
            grid,
            vertices,
            triangles,
        })
    }

    /// A vertex's welded position.
    #[must_use]
    pub(crate) fn position_of(&self, vertex: u32) -> u32 {
        self.vertices[ix(vertex)].position
    }

    /// A welded position's grid coordinates, as integers.
    #[must_use]
    pub(crate) fn grid_i64(&self, position: u32) -> [i64; 3] {
        self.grid[ix(position)].map(i64::from)
    }

    /// A welded position about the grid's origin, m, as a page dequantises it (exactly).
    #[must_use]
    pub(crate) fn local_m(&self, position: u32) -> [f64; 3] {
        self.grid[ix(position)].map(|q| f64::from(q) * self.step_m)
    }
}

/// The grid's origin (the least corner) and step (the least power of two that spans the largest
/// extent in 2²¹ − 1 steps).
fn position_grid(positions: &[[f64; 3]]) -> Result<([f64; 3], f64), QuantiseMeshError> {
    let mut lo = positions[0];
    let mut hi = positions[0];
    for p in positions {
        for axis in 0..3 {
            lo[axis] = crate::num::min(lo[axis], p[axis]);
            hi[axis] = crate::num::max(hi[axis], p[axis]);
        }
    }
    let extent_m = (0..3)
        .map(|axis| hi[axis] - lo[axis])
        .fold(0.0, crate::num::max);
    let step =
        power_of_two_step(extent_m, GRID_MAX).ok_or(QuantiseMeshError::Extent { extent_m })?;
    Ok((lo, step))
}

/// The least power of two `s`, an `f32` normal number, with `extent / s` ≤ `most`.
fn power_of_two_step(extent: f64, most: f64) -> Option<f64> {
    let (least, greatest) = (f64::from(f32::MIN_POSITIVE), f64::from(f32::MAX));
    if !(extent > 0.0 && extent.is_finite()) {
        return None;
    }
    let mut s = 1.0_f64;
    while extent / s > most && s <= greatest {
        s *= 2.0;
    }
    while extent / (s * 0.5) <= most && s * 0.5 >= least {
        s *= 0.5;
    }
    (extent / s <= most && (least..=greatest).contains(&s)).then_some(s)
}

/// A coordinate's grid index about `origin`, rounded to the nearest point.
fn grid_coordinate(value: f64, origin: f64, step: f64) -> u32 {
    let q = ((value - origin) / step).round().clamp(0.0, GRID_MAX);
    u32::try_from(i64_of_integral(q)).expect("clamped to 21 bits")
}

/// The UV grid: an `f32` origin at or below the least UV and the least power-of-two step that
/// spans the rest in 2¹⁶ − 1 steps (1 if the UVs span nothing).
fn uv_grid(uvs: &[[f64; 2]]) -> ([f32; 2], f32) {
    let mut lo = uvs[0];
    let mut hi = uvs[0];
    for uv in uvs {
        for axis in 0..2 {
            lo[axis] = crate::num::min(lo[axis], uv[axis]);
            hi[axis] = crate::num::max(hi[axis], uv[axis]);
        }
    }
    let origin = lo.map(f32_down);
    let extent = (0..2)
        .map(|axis| hi[axis] - f64::from(origin[axis]))
        .fold(0.0, crate::num::max);
    let step = power_of_two_step(extent, UV_GRID_MAX).unwrap_or(1.0);
    (origin, crate::num::f32_nearest(step))
}

/// Each welded position's normal: the sum of its faces' cross products (area-weighted), unit
/// length, or +z where they cancel.
fn position_normals(grid: &[[u32; 3]], position_of: &[u32], corners: &[[u32; 3]]) -> Vec<[f64; 3]> {
    let mut sums = vec![[0_i64; 3]; grid.len()];
    for t in corners {
        let p = t.map(|v| position_of[ix(v)]);
        let [first, second, third] = p.map(|p| grid[ix(p)].map(i64::from));
        let normal = crate::geometry::cross_i64(
            crate::geometry::sub_i64(second, first),
            crate::geometry::sub_i64(third, first),
        );
        for corner in p {
            for axis in 0..3 {
                sums[ix(corner)][axis] += normal[axis];
            }
        }
    }
    sums.iter()
        .map(|s| {
            let n = s.map(f64_of);
            if length_squared(n) > 0.0 {
                n
            } else {
                [0.0, 0.0, 1.0]
            }
        })
        .collect()
}

fn length_squared(v: [f64; 3]) -> f64 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

/// A direction's octahedral coordinates (Meyer et al. 2010, "On Floating-Point Normal Vectors",
/// Computer Graphics Forum 29(4)), as signed 16-bit normalised values, as WGSL's
/// `unpack2x16snorm` reads them.
#[must_use]
pub(crate) fn octahedral(n: [f64; 3]) -> [i16; 2] {
    let sum = n[0].abs() + n[1].abs() + n[2].abs();
    let (mut x, mut y) = (n[0] / sum, n[1] / sum);
    if n[2] < 0.0 {
        let sign = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
        (x, y) = ((1.0 - y.abs()) * sign(x), (1.0 - x.abs()) * sign(y));
    }
    [x, y].map(|v| {
        let q = i64_of_integral((v.clamp(-1.0, 1.0) * 32_767.0).round());
        i16::try_from(q).expect("clamped to ±32767")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(z: f64) -> SourceMesh {
        SourceMesh::new(
            vec![[0.0, 0.0, z], [2.0, 0.0, z], [2.0, 1.0, z], [0.0, 1.0, z]],
            vec![[0, 1, 2], [0, 2, 3]],
        )
        .unwrap()
    }

    #[test]
    fn the_step_is_the_least_power_of_two_spanning_the_extent() {
        assert_eq!(power_of_two_step(65_535.0, UV_GRID_MAX), Some(1.0));
        assert_eq!(power_of_two_step(65_536.0, UV_GRID_MAX), Some(2.0));
        assert_eq!(power_of_two_step(2_097_151.0, GRID_MAX), Some(1.0));
        assert_eq!(power_of_two_step(1.0, GRID_MAX), Some(1.0 / 1_048_576.0));
        assert_eq!(power_of_two_step(0.0, GRID_MAX), None);
    }

    #[test]
    fn positions_quantise_onto_one_grid_and_weld() {
        let mesh = QuantisedMesh::new(&square(5.0)).unwrap();
        assert_eq!(mesh.step_m.to_bits(), (1.0_f64 / 524_288.0).to_bits());
        assert_eq!(
            mesh.grid,
            vec![
                [0, 0, 0],
                [1_048_576, 0, 0],
                [1_048_576, 524_288, 0],
                [0, 524_288, 0]
            ]
        );
        assert_eq!(mesh.triangles.len(), 2);
    }

    #[test]
    fn a_flat_square_faces_up() {
        let mesh = QuantisedMesh::new(&square(0.0)).unwrap();
        assert!(
            mesh.vertices.iter().all(|v| v.normal == [0, 0]),
            "{:?}",
            mesh.vertices
        );
    }

    #[test]
    fn octahedral_coordinates_fold_the_lower_half() {
        assert_eq!(octahedral([0.0, 0.0, 1.0]), [0, 0]);
        assert_eq!(octahedral([1.0, 0.0, 0.0]), [32_767, 0]);
        assert_eq!(octahedral([0.0, 0.0, -1.0]), [32_767, 32_767]);
        assert_eq!(octahedral([0.0, -1.0, 0.0]), [0, -32_767]);
    }

    #[test]
    fn a_bad_mesh_is_refused_with_its_reason() {
        let three = || vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(
            SourceMesh::new(three(), vec![[0, 1, 3]]),
            Err(NewSourceMeshError::IndexOutOfRange {
                triangle: 0,
                vertex: 3,
                vertices: 3
            })
        );
        assert_eq!(
            SourceMesh::new(three(), vec![]),
            Err(NewSourceMeshError::NoTriangles)
        );
        let mut nan = three();
        nan[1][2] = f64::NAN;
        assert_eq!(
            SourceMesh::new(nan, vec![[0, 1, 2]]),
            Err(NewSourceMeshError::NotFinite {
                vertex: 1,
                what: "position"
            })
        );
        let mesh = SourceMesh::new(three(), vec![[0, 1, 2]]).unwrap();
        assert_eq!(
            mesh.clone().with_uvs(vec![[0.0; 2]; 2]),
            Err(NewSourceMeshError::AttributeCount {
                what: "uvs",
                found: 2,
                vertices: 3
            })
        );
        assert_eq!(
            mesh.with_normals(vec![[0.0, 0.0, f64::INFINITY]; 3]),
            Err(NewSourceMeshError::NotFinite {
                vertex: 0,
                what: "normal"
            })
        );
    }

    #[test]
    fn a_mesh_with_no_extent_or_no_area_is_not_quantised() {
        let point = SourceMesh::new(vec![[1.0; 3]; 3], vec![[0, 1, 2]]).unwrap();
        assert_eq!(
            QuantisedMesh::new(&point),
            Err(QuantiseMeshError::Extent { extent_m: 0.0 })
        );
        let sliver = SourceMesh::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [1e-9, 0.0, 0.0]],
            vec![[0, 1, 2]],
        )
        .unwrap();
        assert_eq!(
            QuantisedMesh::new(&sliver),
            Err(QuantiseMeshError::AllDegenerate {
                step_m: 1.0 / 1_048_576.0
            })
        );
    }
}
