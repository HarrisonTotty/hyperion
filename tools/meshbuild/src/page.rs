//! The mesh page format (plan R14's `MeshPage`, version 1), its writer and its reader.
//!
//! A mesh file is a header, a page table and the pages, each page starting on 16 bytes and holding
//! at most [`MAX_PAGE_CLUSTERS`] clusters in at most [`MAX_PAGE_BYTES`], so that the client's page
//! pool can keep fixed slots. Everything is little-endian.
//!
//! **The mesh file**: the header, [`MESH_HEADER_BYTES`]: the magic `HMSH`, the version, the page
//! count, the cluster count, the group count, the DAG's levels, the root pages (the first so many
//! pages hold every root cluster, which residency pins) and 0. Then the page table, an entry of
//! [`PAGE_ENTRY_BYTES`] a page, which says what a page holds before it is fetched: its offset and
//! length; its clusters; its levels, the finest (bits 0–15) and the coarsest (bits 16–31); the
//! span of its clusters' own errors and of their parent errors, m, four `f32` (least own, greatest
//! own, least parent, greatest parent); and a sphere about the page's origin holding its clusters'
//! own spheres, four `f32`. Then the pages.
//!
//! **A page**: a header of [`PAGE_HEADER_BYTES`], then five sections, each starting on 16 bytes,
//! at the offsets the header gives:
//!
//! | Bytes | Header field                                                                    |
//! | ----- | ------------------------------------------------------------------------------- |
//! | 0–3   | the magic `HMPG`                                                                |
//! | 4–7   | the version, 1                                                                  |
//! | 8–11  | the page's index in its mesh                                                    |
//! | 12–23 | its clusters, vertices and triangles                                            |
//! | 24–27 | its bytes                                                                       |
//! | 28–31 | flags: bit 0 the mesh had UVs                                                   |
//! | 32–55 | the page's origin in the mesh's frame, m, three `f64`: the grid's least corner  |
//! | 56–59 | the grid's step, m, `f32`, a power of two                                       |
//! | 60–63 | the UV grid's step, `f32`, a power of two                                       |
//! | 64–71 | the UV grid's origin, two `f32`                                                 |
//! | 72–91 | the sections' offsets: records, LOD spheres, cluster table, vertices, triangles |
//! | 92–95 | 0                                                                               |
//!
//! - **Records**, 32 B a cluster, as the client's `ClusterRecord` (`clusters/record.ts`) lays them
//!   out in the DAG-cut mode: the own sphere's centre about the page's origin and its radius, m,
//!   four `f32`; the own and parent errors, m, two `f32` (the parent +∞ at a root); the template
//!   reference, the cluster's first triangle in the page (bits 0–23) and its triangle count (bits
//!   24–31); and the flags, the mesh shape (2) with the DAG-cut bit (bit 4), opaque and material 0.
//! - **LOD spheres**, 16 B a cluster, the client's `ClusterLod`: the parent group's sphere, four
//!   `f32` (a root's own).
//! - **The cluster table**, 16 B a cluster: its first vertex in the page and its vertex count, and
//!   the group it was made from and the group it was simplified in, [`NO_GROUP`] for none: what
//!   residency reads to keep a group's pages together.
//! - **Vertices**, 16 B, four words: the grid coordinates, 21 bits each, x | y ≪ 21 and
//!   y ≫ 11 | z ≪ 10 (so the vertex is the coordinates times the step about the page's origin,
//!   exactly in `f32`); the octahedral normal as WGSL's `pack2x16snorm` packs it; and the UV's grid
//!   coordinates u | v ≪ 16 (the UV is the UV origin plus the coordinates times the UV step).
//! - **Triangles**, 4 B, a word each: the three 8-bit indices into the cluster's vertices, i₀ |
//!   i₁ ≪ 8 | i₂ ≪ 16.
//!
//! A cluster's own sphere bounds it and is its culling sphere as well (plan R14, Design note 3).
//! Every page of a mesh has the same origin and steps, so a vertex two clusters share is the same
//! bits whichever page holds it, and a group's sphere and error are the same bits in each record
//! that holds them, whichever page.
//!
//! **Packing**: each cluster belongs to a unit: the clusters one group made, or, for leaves, the
//! leaves one group simplified. A unit is never split across pages while it fits one, so that
//! what one group's replacement brings in or takes out arrives and leaves together. The units
//! holding a root come first, then the rest by level, coarsest first, so that "a level and every
//! coarser one" is a prefix of the pages.

use thiserror::Error;

use crate::build::Dag;
use crate::cluster::{local_triangles, local_vertices};
use crate::mesh::GRID_BITS;
use crate::num::{f32_nearest, ix, u32_of};
use crate::sphere::{Sphere, enclosing_spheres};

/// `HMSH`, the mesh file's magic.
pub const MESH_MAGIC: u32 = u32::from_le_bytes(*b"HMSH");
/// `HMPG`, a page's magic.
pub const PAGE_MAGIC: u32 = u32::from_le_bytes(*b"HMPG");
/// The format's version.
pub const FORMAT_VERSION: u32 = 1;
/// The mesh file's header.
pub const MESH_HEADER_BYTES: usize = 32;
/// A page table entry.
pub const PAGE_ENTRY_BYTES: usize = 48;
/// A page's header.
pub const PAGE_HEADER_BYTES: usize = 96;
/// A cluster's record, as the client's `CLUSTER_RECORD_BYTES`.
pub const CLUSTER_RECORD_BYTES: usize = 32;
/// A cluster's parent LOD sphere.
pub const CLUSTER_LOD_BYTES: usize = 16;
/// A cluster's table entry.
pub const CLUSTER_TABLE_BYTES: usize = 16;
/// A vertex.
pub const VERTEX_BYTES: usize = 16;
/// A triangle.
pub const TRIANGLE_BYTES: usize = 4;
/// The most clusters a page holds.
pub const MAX_PAGE_CLUSTERS: usize = 128;
/// The most bytes a page holds: Nanite's streaming page (Karis 2021).
pub const MAX_PAGE_BYTES: usize = 128 * 1024;
/// The cluster table's group where there is none.
pub const NO_GROUP: u32 = u32::MAX;
/// The flags of every mesh cluster: the client's `CLUSTER_SHAPE_CODE.mesh` (2) and
/// `CLUSTER_FLAG_DAG` (bit 4), opaque, material 0.
pub const MESH_CLUSTER_FLAGS: u32 = 2 | (1 << 4);
/// A page's flag: the mesh had UVs.
pub const PAGE_FLAG_UVS: u32 = 1;

/// The mask of one grid coordinate.
const GRID_MASK: u32 = (1 << GRID_BITS) - 1;

/// Why a mesh file could not be written.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WriteMeshError {
    /// The file would pass the 4 GiB its 32-bit offsets reach.
    #[error("the mesh file would hold {bytes} bytes, past its 32-bit offsets")]
    TooLarge {
        /// The bytes it would hold.
        bytes: usize,
    },
}

/// Why a mesh file could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReadMeshError {
    /// The bytes end before what they describe.
    #[error("the mesh file is truncated at {at} of {len} bytes")]
    Truncated {
        /// Where the read needed more.
        at: usize,
        /// The bytes there are.
        len: usize,
    },
    /// A magic or version is not this format's.
    #[error("not a version {FORMAT_VERSION} mesh file: {what}")]
    NotThisFormat {
        /// What was wrong.
        what: &'static str,
    },
}

/// One cluster's place in its page, before serialising.
#[derive(Debug)]
struct Placed {
    cluster: u32,
    vertices: Vec<u32>,
    triangles: Vec<[u8; 3]>,
}

/// The DAG's mesh file (see the module's documentation).
///
/// # Errors
///
/// [`WriteMeshError::TooLarge`] if the file would pass 4 GiB.
pub fn write_mesh(dag: &Dag) -> Result<Vec<u8>, WriteMeshError> {
    let pages = pack(dag);
    let root_pages = pages
        .iter()
        .rposition(|page| {
            page.iter()
                .any(|p| dag.clusters()[ix(p.cluster)].consuming_group().is_none())
        })
        .map_or(0, |last| last + 1);
    let bodies: Vec<Vec<u8>> = pages
        .iter()
        .enumerate()
        .map(|(i, page)| write_page(dag, u32_of(i), page))
        .collect();
    let table_end = align16(MESH_HEADER_BYTES + PAGE_ENTRY_BYTES * pages.len());
    let total = bodies
        .iter()
        .fold(table_end, |end, body| align16(end) + body.len());
    let offset_of =
        |bytes: usize| u32::try_from(bytes).map_err(|_| WriteMeshError::TooLarge { bytes: total });
    offset_of(total)?;
    let mut out = Vec::with_capacity(total);
    for v in [
        MESH_MAGIC,
        FORMAT_VERSION,
        u32_of(pages.len()),
        u32_of(dag.clusters().len()),
        u32_of(dag.groups().len()),
        dag.levels(),
        u32_of(root_pages),
        0,
    ] {
        put_u32(&mut out, v);
    }
    let mut offset = table_end;
    for (page, body) in pages.iter().zip(&bodies) {
        put_u32(&mut out, offset_of(offset)?);
        put_u32(&mut out, offset_of(body.len())?);
        put_page_summary(&mut out, dag, page);
        offset = align16(offset + body.len());
    }
    for body in &bodies {
        pad16(&mut out);
        out.extend_from_slice(body);
    }
    debug_assert_eq!(out.len(), total, "the file's parts add up to its size");
    Ok(out)
}

/// A page table entry's summary of `page`: its clusters, levels, error spans and sphere.
fn put_page_summary(out: &mut Vec<u8>, dag: &Dag, page: &[Placed]) {
    let clusters: Vec<_> = page
        .iter()
        .map(|p| &dag.clusters()[ix(p.cluster)])
        .collect();
    let levels = clusters.iter().map(|c| c.level());
    let (finest, coarsest) = (levels.clone().min(), levels.max());
    let level =
        |l: Option<u32>| u32::from(u16::try_from(l.unwrap_or(0)).expect("at most 64 levels"));
    put_u32(out, u32_of(clusters.len()));
    put_u32(out, level(finest) | (level(coarsest) << 16));
    let span = |values: Vec<f32>| {
        let least = values
            .iter()
            .copied()
            .fold(f32::INFINITY, |a, b| if b < a { b } else { a });
        let greatest = values
            .iter()
            .copied()
            .fold(0.0_f32, |a, b| if b > a { b } else { a });
        (least, greatest)
    };
    let own = span(clusters.iter().map(|c| c.own_error_m()).collect());
    let parent = span(clusters.iter().map(|c| c.parent_error_m()).collect());
    for v in [own.0, own.1, parent.0, parent.1] {
        put_f32(out, v);
    }
    let spheres: Vec<Sphere> = clusters.iter().map(|c| c.own_sphere()).collect();
    put_sphere(out, enclosing_spheres(&spheres));
}

fn align16(n: usize) -> usize {
    n.div_ceil(16) * 16
}

fn pad16(out: &mut Vec<u8>) {
    out.resize(align16(out.len()), 0);
}

fn page_bytes(clusters: usize, vertices: usize, triangles: usize) -> usize {
    PAGE_HEADER_BYTES
        + (CLUSTER_RECORD_BYTES + CLUSTER_LOD_BYTES + CLUSTER_TABLE_BYTES) * clusters
        + VERTEX_BYTES * vertices
        + align16(TRIANGLE_BYTES * triangles)
}

/// A cluster's unit, ordered coarsest first: (the level reversed, the unit's kind, its group or
/// cluster).
type UnitKey = (std::cmp::Reverse<u32>, u8, u32);

/// The clusters by page (see the module's documentation).
fn pack(dag: &Dag) -> Vec<Vec<Placed>> {
    let clusters = dag.clusters();
    let mut keyed: Vec<(UnitKey, u32)> = clusters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let (kind, id) = match (c.generating_group(), c.consuming_group()) {
                (Some(g), _) => (0, g),
                (None, Some(g)) => (1, g),
                (None, None) => (2, u32_of(i)),
            };
            ((std::cmp::Reverse(c.level()), kind, id), u32_of(i))
        })
        .collect();
    keyed.sort_unstable();
    let mut units: Vec<Vec<u32>> = Vec::new();
    let mut last = None;
    for (key, i) in keyed {
        let unit = (key.1, key.2);
        if last == Some(unit) {
            units.last_mut().expect("a unit was opened").push(i);
        } else {
            units.push(vec![i]);
            last = Some(unit);
        }
    }
    let holds_root = |unit: &Vec<u32>| {
        unit.iter()
            .any(|&c| clusters[ix(c)].consuming_group().is_none())
    };
    units.sort_by_key(|unit| !holds_root(unit));
    let mut pages: Vec<Vec<Placed>> = Vec::new();
    let mut page: Vec<Placed> = Vec::new();
    let (mut vertices, mut triangles) = (0, 0);
    let fits = |n: usize, v: usize, t: usize| {
        n <= MAX_PAGE_CLUSTERS && page_bytes(n, v, t) <= MAX_PAGE_BYTES
    };
    for unit in units {
        let placed: Vec<Placed> = unit
            .iter()
            .map(|&c| {
                let t = clusters[ix(c)].triangles();
                let v = local_vertices(t);
                Placed {
                    cluster: c,
                    triangles: local_triangles(t, &v),
                    vertices: v,
                }
            })
            .collect();
        let (uv, ut) = placed.iter().fold((0, 0), |(v, t), p| {
            (v + p.vertices.len(), t + p.triangles.len())
        });
        if !page.is_empty() && !fits(page.len() + placed.len(), vertices + uv, triangles + ut) {
            pages.push(std::mem::take(&mut page));
            (vertices, triangles) = (0, 0);
        }
        for p in placed {
            if !page.is_empty()
                && !fits(
                    page.len() + 1,
                    vertices + p.vertices.len(),
                    triangles + p.triangles.len(),
                )
            {
                pages.push(std::mem::take(&mut page));
                (vertices, triangles) = (0, 0);
            }
            vertices += p.vertices.len();
            triangles += p.triangles.len();
            page.push(p);
        }
    }
    if !page.is_empty() {
        pages.push(page);
    }
    pages
}

fn write_page(dag: &Dag, index: u32, page: &[Placed]) -> Vec<u8> {
    let mesh = dag.mesh();
    let n = page.len();
    let vertex_count: usize = page.iter().map(|p| p.vertices.len()).sum();
    let triangle_count: usize = page.iter().map(|p| p.triangles.len()).sum();
    let records = PAGE_HEADER_BYTES;
    let lods = records + CLUSTER_RECORD_BYTES * n;
    let table = lods + CLUSTER_LOD_BYTES * n;
    let vertex_offset = table + CLUSTER_TABLE_BYTES * n;
    let triangle_offset = vertex_offset + VERTEX_BYTES * vertex_count;
    let total = page_bytes(n, vertex_count, triangle_count);
    let mut out = Vec::with_capacity(total);
    for v in [
        PAGE_MAGIC,
        FORMAT_VERSION,
        index,
        u32_of(n),
        u32_of(vertex_count),
        u32_of(triangle_count),
        u32_of(total),
        if mesh.has_uvs { PAGE_FLAG_UVS } else { 0 },
    ] {
        put_u32(&mut out, v);
    }
    for v in mesh.origin_m {
        out.extend_from_slice(&v.to_le_bytes());
    }
    put_f32(&mut out, f32_nearest(mesh.step_m));
    put_f32(&mut out, mesh.uv_step);
    put_f32(&mut out, mesh.uv_origin[0]);
    put_f32(&mut out, mesh.uv_origin[1]);
    for v in [records, lods, table, vertex_offset, triangle_offset, 0] {
        put_u32(&mut out, u32_of(v));
    }
    let mut first_triangle = 0;
    for p in page {
        let c = &dag.clusters()[ix(p.cluster)];
        put_sphere(&mut out, c.own_sphere());
        put_f32(&mut out, c.own_error_m());
        put_f32(&mut out, c.parent_error_m());
        put_u32(
            &mut out,
            u32_of(first_triangle) | (u32_of(p.triangles.len()) << 24),
        );
        put_u32(&mut out, MESH_CLUSTER_FLAGS);
        first_triangle += p.triangles.len();
    }
    for p in page {
        put_sphere(&mut out, dag.clusters()[ix(p.cluster)].parent_sphere());
    }
    let mut first_vertex = 0;
    for p in page {
        let c = &dag.clusters()[ix(p.cluster)];
        put_u32(&mut out, u32_of(first_vertex));
        put_u32(&mut out, u32_of(p.vertices.len()));
        put_u32(&mut out, c.generating_group().unwrap_or(NO_GROUP));
        put_u32(&mut out, c.consuming_group().unwrap_or(NO_GROUP));
        first_vertex += p.vertices.len();
    }
    for p in page {
        for &v in &p.vertices {
            let vertex = mesh.vertices[ix(v)];
            let [x, y, z] = mesh.grid[ix(vertex.position)];
            put_u32(&mut out, x | (y << GRID_BITS));
            put_u32(
                &mut out,
                (y >> (32 - GRID_BITS)) | (z << (2 * GRID_BITS - 32)),
            );
            let [nx, ny] = vertex.normal.map(i16::cast_unsigned);
            put_u32(&mut out, u32::from(nx) | (u32::from(ny) << 16));
            put_u32(
                &mut out,
                u32::from(vertex.uv[0]) | (u32::from(vertex.uv[1]) << 16),
            );
        }
    }
    for p in page {
        for t in &p.triangles {
            put_u32(
                &mut out,
                u32::from(t[0]) | (u32::from(t[1]) << 8) | (u32::from(t[2]) << 16),
            );
        }
    }
    pad16(&mut out);
    debug_assert_eq!(out.len(), total, "the page's sections add up to its size");
    out
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_sphere(out: &mut Vec<u8>, s: Sphere) {
    for v in s.centre_m() {
        put_f32(out, v);
    }
    put_f32(out, s.radius_m());
}

/// A mesh file as read back.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshFile {
    group_count: u32,
    levels: u32,
    root_pages: u32,
    pages: Vec<Page>,
}

impl MeshFile {
    /// Its groups.
    #[must_use]
    pub const fn group_count(&self) -> u32 {
        self.group_count
    }

    /// The DAG's levels.
    #[must_use]
    pub const fn levels(&self) -> u32 {
        self.levels
    }

    /// How many pages, from the first, hold every root.
    #[must_use]
    pub const fn root_pages(&self) -> u32 {
        self.root_pages
    }

    /// Its pages, in order.
    #[must_use]
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }
}

/// A page as read back, with its page table entry's summary.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    index: u32,
    bytes: u32,
    levels: (u16, u16),
    own_error_span_m: (f32, f32),
    parent_error_span_m: (f32, f32),
    bounds: Sphere,
    origin_m: [f64; 3],
    step_m: f32,
    uv_origin: [f32; 2],
    uv_step: f32,
    flags: u32,
    clusters: Vec<PageCluster>,
}

impl Page {
    /// Its index in the mesh.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Its bytes.
    #[must_use]
    pub const fn bytes(&self) -> u32 {
        self.bytes
    }

    /// Its clusters' levels, the finest and the coarsest (the table's summary).
    #[must_use]
    pub const fn levels(&self) -> (u16, u16) {
        self.levels
    }

    /// The least and greatest own error of its clusters, m (the table's summary).
    #[must_use]
    pub const fn own_error_span_m(&self) -> (f32, f32) {
        self.own_error_span_m
    }

    /// The least and greatest parent error of its clusters, m (the table's summary).
    #[must_use]
    pub const fn parent_error_span_m(&self) -> (f32, f32) {
        self.parent_error_span_m
    }

    /// A sphere holding its clusters' own spheres, about its origin (the table's summary).
    #[must_use]
    pub const fn bounds(&self) -> Sphere {
        self.bounds
    }

    /// Its origin in the mesh's frame, m.
    #[must_use]
    pub const fn origin_m(&self) -> [f64; 3] {
        self.origin_m
    }

    /// The position grid's step, m.
    #[must_use]
    pub const fn step_m(&self) -> f32 {
        self.step_m
    }

    /// The UV grid's origin.
    #[must_use]
    pub const fn uv_origin(&self) -> [f32; 2] {
        self.uv_origin
    }

    /// The UV grid's step.
    #[must_use]
    pub const fn uv_step(&self) -> f32 {
        self.uv_step
    }

    /// Its flags ([`PAGE_FLAG_UVS`]).
    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    /// Its clusters, in order.
    #[must_use]
    pub fn clusters(&self) -> &[PageCluster] {
        &self.clusters
    }
}

/// A cluster as read back from its page.
#[derive(Debug, Clone, PartialEq)]
pub struct PageCluster {
    own_sphere: Sphere,
    own_error_m: f32,
    parent_error_m: f32,
    parent_sphere: Sphere,
    first_triangle: u32,
    flags: u32,
    first_vertex: u32,
    generating_group: Option<u32>,
    consuming_group: Option<u32>,
    vertices: Vec<PageVertex>,
    triangles: Vec<[u8; 3]>,
}

impl PageCluster {
    /// Its own sphere, about the page's origin.
    #[must_use]
    pub const fn own_sphere(&self) -> &Sphere {
        &self.own_sphere
    }

    /// Its own error, m.
    #[must_use]
    pub const fn own_error_m(&self) -> f32 {
        self.own_error_m
    }

    /// Its parent error, m (+∞ at a root).
    #[must_use]
    pub const fn parent_error_m(&self) -> f32 {
        self.parent_error_m
    }

    /// Its parent group's sphere (a root's own).
    #[must_use]
    pub const fn parent_sphere(&self) -> &Sphere {
        &self.parent_sphere
    }

    /// Its first triangle in the page.
    #[must_use]
    pub const fn first_triangle(&self) -> u32 {
        self.first_triangle
    }

    /// Its record's flags.
    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    /// Its first vertex in the page.
    #[must_use]
    pub const fn first_vertex(&self) -> u32 {
        self.first_vertex
    }

    /// The group it was made from, `None` for a leaf.
    #[must_use]
    pub const fn generating_group(&self) -> Option<u32> {
        self.generating_group
    }

    /// The group it was simplified in, `None` for a root.
    #[must_use]
    pub const fn consuming_group(&self) -> Option<u32> {
        self.consuming_group
    }

    /// Its vertices.
    #[must_use]
    pub fn vertices(&self) -> &[PageVertex] {
        &self.vertices
    }

    /// Its triangles, as indices into [`Self::vertices`].
    #[must_use]
    pub fn triangles(&self) -> &[[u8; 3]] {
        &self.triangles
    }
}

/// A vertex as read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageVertex {
    grid: [u32; 3],
    normal: [i16; 2],
    uv: [u16; 2],
}

impl PageVertex {
    /// Its grid coordinates: the position about the page's origin is these times the step.
    #[must_use]
    pub const fn grid(&self) -> [u32; 3] {
        self.grid
    }

    /// Its octahedral normal, signed 16-bit normalised.
    #[must_use]
    pub const fn normal(&self) -> [i16; 2] {
        self.normal
    }

    /// Its UV's grid coordinates.
    #[must_use]
    pub const fn uv(&self) -> [u16; 2] {
        self.uv
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl Reader<'_> {
    fn word(&self, at: usize) -> Result<[u8; 4], ReadMeshError> {
        self.bytes
            .get(at..at + 4)
            .map(|b| [b[0], b[1], b[2], b[3]])
            .ok_or(ReadMeshError::Truncated {
                at: at + 4,
                len: self.bytes.len(),
            })
    }

    fn u32(&self, at: usize) -> Result<u32, ReadMeshError> {
        self.word(at).map(u32::from_le_bytes)
    }

    fn f32(&self, at: usize) -> Result<f32, ReadMeshError> {
        self.word(at).map(f32::from_le_bytes)
    }

    fn f64(&self, at: usize) -> Result<f64, ReadMeshError> {
        let lo = self.word(at)?;
        let hi = self.word(at + 4)?;
        Ok(f64::from_le_bytes([
            lo[0], lo[1], lo[2], lo[3], hi[0], hi[1], hi[2], hi[3],
        ]))
    }

    fn sphere(&self, at: usize) -> Result<Sphere, ReadMeshError> {
        Ok(Sphere::new(
            [self.f32(at)?, self.f32(at + 4)?, self.f32(at + 8)?],
            self.f32(at + 12)?,
        ))
    }
}

/// Reads a mesh file [`write_mesh`] wrote.
///
/// # Errors
///
/// [`ReadMeshError`] if the bytes are truncated or not this format's.
pub fn read_mesh(bytes: &[u8]) -> Result<MeshFile, ReadMeshError> {
    let r = Reader { bytes };
    if r.u32(0)? != MESH_MAGIC {
        return Err(ReadMeshError::NotThisFormat {
            what: "the mesh magic",
        });
    }
    if r.u32(4)? != FORMAT_VERSION {
        return Err(ReadMeshError::NotThisFormat {
            what: "the mesh version",
        });
    }
    let page_count = ix(r.u32(8)?);
    let mut pages = Vec::with_capacity(page_count.min(bytes.len() / PAGE_HEADER_BYTES));
    for i in 0..page_count {
        let at = MESH_HEADER_BYTES + PAGE_ENTRY_BYTES * i;
        let (offset, len) = (ix(r.u32(at)?), ix(r.u32(at + 4)?));
        let body = bytes
            .get(offset..offset + len)
            .ok_or(ReadMeshError::Truncated {
                at: offset + len,
                len: bytes.len(),
            })?;
        let mut page = read_page(body)?;
        let levels = r.word(at + 12)?;
        page.levels = (
            u16::from_le_bytes([levels[0], levels[1]]),
            u16::from_le_bytes([levels[2], levels[3]]),
        );
        page.own_error_span_m = (r.f32(at + 16)?, r.f32(at + 20)?);
        page.parent_error_span_m = (r.f32(at + 24)?, r.f32(at + 28)?);
        page.bounds = r.sphere(at + 32)?;
        pages.push(page);
    }
    Ok(MeshFile {
        group_count: r.u32(16)?,
        levels: r.u32(20)?,
        root_pages: r.u32(24)?,
        pages,
    })
}

fn read_page(bytes: &[u8]) -> Result<Page, ReadMeshError> {
    let r = Reader { bytes };
    if r.u32(0)? != PAGE_MAGIC {
        return Err(ReadMeshError::NotThisFormat {
            what: "a page's magic",
        });
    }
    if r.u32(4)? != FORMAT_VERSION {
        return Err(ReadMeshError::NotThisFormat {
            what: "a page's version",
        });
    }
    let n = ix(r.u32(12)?);
    let [records, lods, table, vertices, triangles] =
        [72, 76, 80, 84, 88].map(|at| r.u32(at).map(ix));
    let (records, lods, table, vertices, triangles) =
        (records?, lods?, table?, vertices?, triangles?);
    let mut clusters = Vec::with_capacity(n.min(MAX_PAGE_CLUSTERS));
    for i in 0..n {
        clusters.push(read_cluster(
            &r,
            records + CLUSTER_RECORD_BYTES * i,
            lods + CLUSTER_LOD_BYTES * i,
            table + CLUSTER_TABLE_BYTES * i,
            (vertices, triangles),
        )?);
    }
    Ok(Page {
        index: r.u32(8)?,
        bytes: r.u32(24)?,
        levels: (0, 0),
        own_error_span_m: (0.0, 0.0),
        parent_error_span_m: (0.0, 0.0),
        bounds: Sphere::default(),
        origin_m: [r.f64(32)?, r.f64(40)?, r.f64(48)?],
        step_m: r.f32(56)?,
        uv_step: r.f32(60)?,
        uv_origin: [r.f32(64)?, r.f32(68)?],
        flags: r.u32(28)?,
        clusters,
    })
}

/// One cluster from its record, its LOD sphere and its table entry, with its vertices and
/// triangles from the page's sections (`sections`: their offsets).
fn read_cluster(
    r: &Reader<'_>,
    record: usize,
    lod: usize,
    entry: usize,
    sections: (usize, usize),
) -> Result<PageCluster, ReadMeshError> {
    let group = |g: u32| (g != NO_GROUP).then_some(g);
    let template = r.u32(record + 24)?;
    let (first_vertex, vertex_count) = (r.u32(entry)?, r.u32(entry + 4)?);
    let first_triangle = template & 0x00ff_ffff;
    let mut cluster = PageCluster {
        own_sphere: r.sphere(record)?,
        own_error_m: r.f32(record + 16)?,
        parent_error_m: r.f32(record + 20)?,
        parent_sphere: r.sphere(lod)?,
        first_triangle,
        flags: r.u32(record + 28)?,
        first_vertex,
        generating_group: group(r.u32(entry + 8)?),
        consuming_group: group(r.u32(entry + 12)?),
        vertices: Vec::with_capacity(ix(vertex_count).min(256)),
        triangles: Vec::with_capacity(ix(template >> 24)),
    };
    for v in 0..ix(vertex_count) {
        let at = sections.0 + VERTEX_BYTES * (ix(first_vertex) + v);
        let (w0, w1, w2, w3) = (r.u32(at)?, r.u32(at + 4)?, r.u32(at + 8)?, r.u32(at + 12)?);
        let low = |w: u32| u16::try_from(w & 0xffff).expect("16 bits");
        let high = |w: u32| u16::try_from(w >> 16).expect("16 bits");
        cluster.vertices.push(PageVertex {
            grid: [
                w0 & GRID_MASK,
                (w0 >> GRID_BITS) | ((w1 << (32 - GRID_BITS)) & GRID_MASK),
                w1 >> (2 * GRID_BITS - 32),
            ],
            normal: [low(w2).cast_signed(), high(w2).cast_signed()],
            uv: [low(w3), high(w3)],
        });
    }
    for t in 0..ix(template >> 24) {
        let w = r.u32(sections.1 + TRIANGLE_BYTES * (ix(first_triangle) + t))?;
        let byte = |k: u32| u8::try_from((w >> k) & 0xff).expect("8 bits");
        cluster.triangles.push([byte(0), byte(8), byte(16)]);
    }
    Ok(cluster)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procedural;

    fn small() -> Vec<u8> {
        write_mesh(&crate::build(&procedural::terrain(24, 20, 1.0, 2.0, 3)).unwrap()).unwrap()
    }

    #[test]
    fn a_vertex_reads_back_as_written() {
        let dag = crate::build(&procedural::torus_knot(2, 3, 64, 8, 1.0, 0.3)).unwrap();
        let file = read_mesh(&write_mesh(&dag).unwrap()).unwrap();
        let mesh = dag.mesh();
        let mut written: Vec<[u32; 3]> = mesh.grid.clone();
        written.sort_unstable();
        let mut read: Vec<[u32; 3]> = file
            .pages()
            .iter()
            .flat_map(Page::clusters)
            .filter(|c| c.generating_group().is_none())
            .flat_map(|c| c.vertices().iter().map(PageVertex::grid))
            .collect();
        read.sort_unstable();
        read.dedup();
        assert_eq!(
            read, written,
            "the leaves hold every grid point, as written"
        );
        assert!(
            written.iter().flatten().any(|&q| q > 1 << 20),
            "the 21st bit is exercised"
        );
    }

    #[test]
    fn the_page_table_summarises_each_page() {
        let file = read_mesh(&small()).unwrap();
        for page in file.pages() {
            let own: Vec<f32> = page
                .clusters()
                .iter()
                .map(PageCluster::own_error_m)
                .collect();
            let least = own.iter().copied().fold(f32::INFINITY, f32::min);
            assert_eq!(page.own_error_span_m().0.to_bits(), least.to_bits());
            assert!(
                page.clusters()
                    .iter()
                    .all(|c| page.bounds().contains(c.own_sphere(), 0.0))
            );
        }
        let finest = file.pages().iter().map(|p| p.levels().0);
        assert!(
            finest.clone().zip(finest.skip(1)).all(|(a, b)| a >= b),
            "coarsest first"
        );
    }

    #[test]
    fn a_truncated_or_foreign_file_is_refused() {
        let bytes = small();
        assert_eq!(
            read_mesh(&bytes[..24]),
            Err(ReadMeshError::Truncated { at: 36, len: 24 })
        );
        let mut foreign = bytes.clone();
        foreign[0] = b'X';
        assert_eq!(
            read_mesh(&foreign),
            Err(ReadMeshError::NotThisFormat {
                what: "the mesh magic"
            })
        );
        let mut later = bytes;
        later[4] = 2;
        assert_eq!(
            read_mesh(&later),
            Err(ReadMeshError::NotThisFormat {
                what: "the mesh version"
            })
        );
    }
}
