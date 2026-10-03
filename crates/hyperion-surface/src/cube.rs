//! The cube sphere: S2's six faces, its per-face (u, v) axes and its quadratic warp.
//!
//! A body's surface is addressed through a cube projected onto the sphere, as Google's S2 geometry
//! library does (`s2coords.h`), so that S2's published cell statistics apply unchanged (plan R05,
//! Design note 2). A direction maps to one face and a point (u, v) on it, with u and v in
//! [−1, 1]; the warp between (u, v) and the cell coordinates (s, t) in [0, 1] is S2's quadratic
//! projection, which evens the cells' areas to a largest-to-smallest ratio of about 2.09 (Design
//! note 3), against 5.2 for the unwarped cube.
//!
//! # Conventions
//!
//! - Faces 0 to 5 are +x, +y, +z, −x, −y, −z, with S2's axes: a face's point is
//!   face 0 (1, u, v), face 1 (−u, 1, v), face 2 (−u, −v, 1), face 3 (−1, −v, −u),
//!   face 4 (v, −1, −u), face 5 (v, u, −1) (`S2::FaceUVtoXYZ`).
//! - The warp is u = (4s² − 1) ÷ 3 for s ≥ ½ and (1 − 4(1 − s)²) ÷ 3 otherwise, whose inverse is
//!   s = ½√(1 + 3u) for u ≥ 0 and 1 − ½√(1 − 3u) otherwise (`S2_QUADRATIC_PROJECTION`'s
//!   `STtoUV` and `UVtoST`). S2 multiplies by the rounded `1.0 / 3`; this crate divides by 3,
//!   one correctly rounded operation, so the TypeScript mirror reproduces it bit for bit (Design
//!   note 1). Only `+ − × ÷` and `sqrt` appear, which IEEE 754 rounds exactly on every target.
//! - For the TypeScript mirror, the operations that decide the bits: `4.0 * s * s - 1.0` in that
//!   order (`(4s)s`, then the subtraction); a division by 3, never a product with `1 / 3`; the
//!   norm as `sqrt(x * x + y * y + z * z)` summed left to right, never `Math.hypot`; and the face
//!   axes' negations as unary minus, which turns +0 into −0 (`0 - u` would not). The golden pins
//!   those −0s, so the mirror must compare bits, not numbers.
//! - A point on a face edge or a cube corner belongs to the face of largest |axis|, ties going to
//!   the **lowest face index** (S2 sends ties to the highest axis instead), so that every direction
//!   has exactly one face and the rule agrees with the canonical face of [`PatchKey::vertex_dir`]
//!   (Design note 2).
//!
//! Sources: S2 Geometry, `src/s2/s2coords.h` (the face axes, `STtoUV`, `UVtoST`, `GetFace`,
//! `ValidFaceXYZtoUV`) and `src/s2/s2metrics.cc` (`kMinArea` = 8√2 ÷ 9 and
//! `kMaxArea` = 2.635799256963161491, for the quadratic projection), github.com/google/s2geometry.

/// One of the cube's six faces, in S2's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Face {
    /// Face 0, centred on +x.
    PosX,
    /// Face 1, centred on +y.
    PosY,
    /// Face 2, centred on +z.
    PosZ,
    /// Face 3, centred on −x.
    NegX,
    /// Face 4, centred on −y.
    NegY,
    /// Face 5, centred on −z.
    NegZ,
}

impl Face {
    /// The six faces in index order.
    pub const ALL: [Self; 6] = [
        Self::PosX,
        Self::PosY,
        Self::PosZ,
        Self::NegX,
        Self::NegY,
        Self::NegZ,
    ];

    /// The face's S2 index, 0 to 5.
    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Self::PosX => 0,
            Self::PosY => 1,
            Self::PosZ => 2,
            Self::NegX => 3,
            Self::NegY => 4,
            Self::NegZ => 5,
        }
    }

    /// The face of S2 index `index`, or `None` above 5.
    #[must_use]
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::PosX),
            1 => Some(Self::PosY),
            2 => Some(Self::PosZ),
            3 => Some(Self::NegX),
            4 => Some(Self::NegY),
            5 => Some(Self::NegZ),
            _ => None,
        }
    }
}

impl TryFrom<u8> for Face {
    type Error = DecodeFaceError;

    fn try_from(index: u8) -> Result<Self, Self::Error> {
        Self::from_index(index).ok_or(DecodeFaceError(index))
    }
}

/// Why a number is not a face's index: it is above 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecodeFaceError(pub u8);

impl std::fmt::Display for DecodeFaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "face index {} is above 5", self.0)
    }
}

impl std::error::Error for DecodeFaceError {}

/// A point on a face: the face and its (u, v), each in [−1, 1] for a point on the cube.
///
/// Plain data with public fields, as S2's own `(face, u, v)` triples are: any (u, v) names a
/// point on the face's plane, and the range is where the face's square lies, not an invariant
/// that functions here rely on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceUv {
    /// The face.
    pub face: Face,
    /// The face's first coordinate, in [−1, 1].
    pub u: f64,
    /// The face's second coordinate, in [−1, 1].
    pub v: f64,
}

/// S2's quadratic warp from a cell coordinate s in [0, 1] to a face coordinate u in [−1, 1].
#[must_use]
pub fn st_to_uv(s: f64) -> f64 {
    if s >= 0.5 {
        (4.0 * s * s - 1.0) / 3.0
    } else {
        let r = 1.0 - s;
        (1.0 - 4.0 * r * r) / 3.0
    }
}

/// The inverse of [`st_to_uv`], from u in [−1, 1] back to s in [0, 1], with a square root alone.
#[must_use]
pub fn uv_to_st(u: f64) -> f64 {
    if u >= 0.0 {
        0.5 * (1.0 + 3.0 * u).sqrt()
    } else {
        1.0 - 0.5 * (1.0 - 3.0 * u).sqrt()
    }
}

/// The point (u, v) of `f`'s face on the cube of half-side 1, in body-fixed axes; not normalised
/// ([`unit_dir`] normalises).
#[must_use]
pub fn face_uv_to_xyz(f: FaceUv) -> [f64; 3] {
    let FaceUv { face, u, v } = f;
    match face {
        Face::PosX => [1.0, u, v],
        Face::PosY => [-u, 1.0, v],
        Face::PosZ => [-u, -v, 1.0],
        Face::NegX => [-1.0, -v, -u],
        Face::NegY => [v, -1.0, -u],
        Face::NegZ => [v, u, -1.0],
    }
}

/// The face of the direction `p` and its (u, v) there.
///
/// The face is that of `p`'s largest |component|, ties going to the lowest face index (see the
/// module documentation); `p` need not be normalised.
///
/// # Panics
///
/// If `p` is zero or has a component that is not finite: such a vector has no direction.
#[must_use]
pub fn xyz_to_face_uv(p: [f64; 3]) -> FaceUv {
    let face = face_of(p);
    let [x, y, z] = p;
    let (first, second) = match face {
        Face::PosX => (y / x, z / x),
        Face::PosY => (-x / y, z / y),
        Face::PosZ => (-x / z, -y / z),
        Face::NegX => (z / x, y / x),
        Face::NegY => (z / y, -x / y),
        Face::NegZ => (-y / z, -x / z),
    };
    FaceUv {
        face,
        u: first,
        v: second,
    }
}

/// The face of the direction `p`: its largest |component|, ties to the lowest face index.
///
/// # Panics
///
/// If `p` is zero or has a component that is not finite.
#[must_use]
#[expect(
    clippy::float_cmp,
    reason = "the tie rule needs exact equality: a magnitude ties the largest or it does not"
)]
pub(crate) fn face_of(p: [f64; 3]) -> Face {
    assert!(
        p.iter().all(|c| c.is_finite()) && p.iter().any(|&c| c != 0.0),
        "a direction must be finite and non-zero, got {p:?}"
    );
    let largest = p[0].abs().max(p[1].abs()).max(p[2].abs());
    // Among the axes that reach the largest magnitude, the face of lowest index. The axis's face
    // is its index for a positive component and its index plus 3 for a negative one (a component
    // of largest magnitude is never zero here).
    let mut best = u8::MAX;
    for (axis, &c) in (0_u8..).zip(p.iter()) {
        if c.abs() == largest {
            let face = if c > 0.0 { axis } else { axis + 3 };
            best = best.min(face);
        }
    }
    Face::from_index(best).expect("a face of largest magnitude exists for a non-zero direction")
}

/// `p` scaled to unit length.
///
/// # Panics
///
/// If the squared norm x² + y² + z² is not a finite, non-zero `f64`: `p` is zero or not finite,
/// or so large (|p| above about 10¹⁵⁴) or so small (below about 10⁻¹⁶²) that it overflows or
/// underflows. Every point of the cube, where the crate's directions come from, is far inside.
#[must_use]
pub fn unit_dir(p: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = p;
    let norm = (x * x + y * y + z * z).sqrt();
    assert!(
        norm.is_finite() && norm > 0.0,
        "a direction must be finite and non-zero, got {p:?}"
    );
    [x / norm, y / norm, z / norm]
}

/// The deepest quadtree level of any body: a mean vertex spacing of about 9 mm on an Earth (plan
/// R05, Design note 2), finer than any body's finest level ([`crate::geometry::finest_level`]).
pub const MAX_LEVEL: u8 = 24;

/// Quads along a patch's side: a patch is 65 × 65 vertices (plan R05, Goal).
pub const PATCH_QUADS: u32 = 64;

/// A grid of samples over a patch: its 65 × 65 vertices, or the 129 × 129 of normals at twice
/// the mesh's resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleGrid {
    /// 64 quads a side, the mesh's vertices.
    Mesh,
    /// 128 quads a side.
    Double,
}

impl SampleGrid {
    /// The grid's quads a side.
    #[must_use]
    pub const fn quads(self) -> u32 {
        match self {
            Self::Mesh => PATCH_QUADS,
            Self::Double => 2 * PATCH_QUADS,
        }
    }
}

/// One of a patch's four edges, named by the face coordinate that is constant along it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Edge {
    /// The edge at the patch's smallest u (vertex column x = 0), towards i − 1.
    UMin,
    /// The edge at the patch's largest u (x = 64), towards i + 1.
    UMax,
    /// The edge at the patch's smallest v (vertex row y = 0), towards j − 1.
    VMin,
    /// The edge at the patch's largest v (y = 64), towards j + 1.
    VMax,
}

impl Edge {
    /// The four edges.
    pub const ALL: [Self; 4] = [Self::UMin, Self::UMax, Self::VMin, Self::VMax];

    /// The step in (i, j) that crosses the edge.
    const fn step(self) -> (i64, i64) {
        match self {
            Self::UMin => (-1, 0),
            Self::UMax => (1, 0),
            Self::VMin => (0, -1),
            Self::VMax => (0, 1),
        }
    }
}

/// A patch of the quadtree on a cube face: the face, the level and the cell (i, j), with i along u
/// and j along v and both below 2^level.
///
/// Cell (i, j) of level n covers s in [i ÷ 2ⁿ, (i + 1) ÷ 2ⁿ] and t likewise in j, before the warp
/// ([`st_to_uv`]). Its vertex (x, y), for x and y from 0 to [`PATCH_QUADS`], sits at
/// s = (64 i + x) ÷ 2ⁿ⁺⁶, an exact binary fraction, and likewise t.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PatchKey {
    face: Face,
    level: u8,
    i: u32,
    j: u32,
}

/// Why [`PatchKey::new`] refused a level or cell: it is out of range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NewPatchKeyError {
    /// The level is above [`MAX_LEVEL`].
    LevelAboveMax(u8),
    /// A cell index is not below 2^level.
    IndexOutOfRange {
        /// The level.
        level: u8,
        /// The cell's index along u.
        i: u32,
        /// The cell's index along v.
        j: u32,
    },
}

impl std::fmt::Display for NewPatchKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LevelAboveMax(level) => {
                write!(f, "patch level {level} is above the maximum {MAX_LEVEL}")
            }
            Self::IndexOutOfRange { level, i, j } => {
                write!(f, "patch cell ({i}, {j}) is outside level {level}")
            }
        }
    }
}

impl std::error::Error for NewPatchKeyError {}

/// Why a word is not a packed [`PatchKey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecodePatchKeyError {
    /// Bits above the 56 a key packs are set.
    UnusedBitsSet(u64),
    /// The face field is above 5.
    FaceOutOfRange(u8),
    /// The level or cell is out of range.
    Range(NewPatchKeyError),
}

impl std::fmt::Display for DecodePatchKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnusedBitsSet(word) => {
                write!(f, "patch key word {word:#018x} sets bits above bit 55")
            }
            Self::FaceOutOfRange(face) => write!(f, "patch key face {face} is above 5"),
            Self::Range(_) => write!(f, "patch key out of range"),
        }
    }
}

impl std::error::Error for DecodePatchKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Range(e) => Some(e),
            Self::UnusedBitsSet(_) | Self::FaceOutOfRange(_) => None,
        }
    }
}

impl From<NewPatchKeyError> for DecodePatchKeyError {
    fn from(e: NewPatchKeyError) -> Self {
        Self::Range(e)
    }
}

const FACE_SHIFT: u32 = 53;
const LEVEL_SHIFT: u32 = 48;
const I_SHIFT: u32 = 24;
const INDEX_MASK: u64 = (1 << 24) - 1;

impl PatchKey {
    /// The patch (`face`, `level`, `i`, `j`).
    ///
    /// # Errors
    ///
    /// [`NewPatchKeyError::LevelAboveMax`] above [`MAX_LEVEL`], and
    /// [`NewPatchKeyError::IndexOutOfRange`] if `i` or `j` is not below 2^`level`.
    pub fn new(face: Face, level: u8, i: u32, j: u32) -> Result<Self, NewPatchKeyError> {
        if level > MAX_LEVEL {
            return Err(NewPatchKeyError::LevelAboveMax(level));
        }
        let cells = 1_u32 << level;
        if i >= cells || j >= cells {
            return Err(NewPatchKeyError::IndexOutOfRange { level, i, j });
        }
        Ok(Self { face, level, i, j })
    }

    /// The whole face `face`, the patch of level 0.
    #[must_use]
    pub const fn root(face: Face) -> Self {
        Self {
            face,
            level: 0,
            i: 0,
            j: 0,
        }
    }

    /// The face.
    #[must_use]
    pub const fn face(self) -> Face {
        self.face
    }

    /// The level, 0 to [`MAX_LEVEL`].
    #[must_use]
    pub const fn level(self) -> u8 {
        self.level
    }

    /// The cell's index along u, below 2^level.
    #[must_use]
    pub const fn i(self) -> u32 {
        self.i
    }

    /// The cell's index along v, below 2^level.
    #[must_use]
    pub const fn j(self) -> u32 {
        self.j
    }

    /// The key packed into a word, for cache keys: face in bits 53–55, level in 48–52, i in 24–47
    /// and j in 0–23 (plan R05, Design note 2). The client keys its maps by a string instead, since
    /// the word does not fit a JavaScript number.
    #[must_use]
    pub fn to_u64(self) -> u64 {
        (u64::from(self.face.index()) << FACE_SHIFT)
            | (u64::from(self.level) << LEVEL_SHIFT)
            | (u64::from(self.i) << I_SHIFT)
            | u64::from(self.j)
    }

    /// The key a word from [`to_u64`](Self::to_u64) packs.
    ///
    /// # Errors
    ///
    /// [`DecodePatchKeyError`] if bits above 55 are set, the face is above 5, or the level or cell
    /// is out of range.
    ///
    /// # Panics
    ///
    /// Never: each field is masked to its width before it is narrowed.
    pub fn from_u64(word: u64) -> Result<Self, DecodePatchKeyError> {
        if word >> 56 != 0 {
            return Err(DecodePatchKeyError::UnusedBitsSet(word));
        }
        let face_index = u8::try_from(word >> FACE_SHIFT).expect("three bits fit a u8");
        let face =
            Face::from_index(face_index).ok_or(DecodePatchKeyError::FaceOutOfRange(face_index))?;
        let level = u8::try_from((word >> LEVEL_SHIFT) & 0x1f).expect("five bits fit a u8");
        let i = u32::try_from((word >> I_SHIFT) & INDEX_MASK).expect("24 bits fit a u32");
        let j = u32::try_from(word & INDEX_MASK).expect("24 bits fit a u32");
        Ok(Self::new(face, level, i, j)?)
    }

    /// The patch one level up that contains this one, or `None` at level 0.
    #[must_use]
    pub const fn parent(self) -> Option<Self> {
        if self.level == 0 {
            None
        } else {
            Some(Self {
                face: self.face,
                level: self.level - 1,
                i: self.i >> 1,
                j: self.j >> 1,
            })
        }
    }

    /// The four patches one level down, in the order (2i, 2j), (2i + 1, 2j), (2i, 2j + 1),
    /// (2i + 1, 2j + 1).
    ///
    /// # Panics
    ///
    /// At [`MAX_LEVEL`], which has no children.
    #[must_use]
    pub fn children(self) -> [Self; 4] {
        assert!(
            self.level < MAX_LEVEL,
            "a patch at the maximum level {MAX_LEVEL} has no children"
        );
        let child = |di: u32, dj: u32| Self {
            face: self.face,
            level: self.level + 1,
            i: 2 * self.i + di,
            j: 2 * self.j + dj,
        };
        [child(0, 0), child(1, 0), child(0, 1), child(1, 1)]
    }

    /// The patch of the same level across `edge`, on the neighbouring face where the edge is a
    /// face edge.
    ///
    /// # Panics
    ///
    /// Never: one step across an edge leaves the face by one coordinate at most, so it never
    /// meets a cube corner.
    #[must_use]
    pub fn edge_neighbour(self, edge: Edge) -> Self {
        let (di, dj) = edge.step();
        self.step_cell(di, dj)
            .expect("a step across one edge leaves the face by one coordinate at most")
    }

    /// The patch across `edge` and the edge of that patch which leads back to this one.
    ///
    /// Across a face edge the way back is generally another [`Edge`], since the two faces' axes
    /// differ.
    ///
    /// # Panics
    ///
    /// Never: adjacency on the cube is symmetric.
    #[must_use]
    pub fn edge_neighbour_and_back(self, edge: Edge) -> (Self, Edge) {
        let neighbour = self.edge_neighbour(edge);
        let back = Edge::ALL
            .into_iter()
            .find(|&e| neighbour.edge_neighbour(e) == self)
            .expect("the neighbour across an edge is a neighbour back across one of its edges");
        (neighbour, back)
    }

    /// The patches of the same level across each corner, in the order of the corners (i − 1,
    /// j − 1), (i + 1, j − 1), (i + 1, j + 1), (i − 1, j + 1); `None` where the corner is one of the
    /// cube's, about which only three patches of a level meet (plan R05, Design note 2).
    #[must_use]
    pub fn corner_neighbours(self) -> [Option<Self>; 4] {
        [(-1, -1), (1, -1), (1, 1), (-1, 1)].map(|(di, dj)| self.step_cell(di, dj))
    }

    /// The cell (i + `di`, j + `dj`), folded onto the neighbouring face when it leaves this one, or
    /// `None` when both coordinates leave it (a cube corner's diagonal).
    ///
    /// The fold is exact integer geometry on the cube, in half-cell units: a cell's centre is
    /// (2i + 1 − 2ⁿ, 2j + 1 − 2ⁿ) on a face of half-width 2ⁿ, placed in three dimensions by S2's
    /// axes. One step past an edge puts one coordinate at 2ⁿ + 1, and folding the step over the
    /// edge sets that coordinate to 2ⁿ and the old face's own to 2ⁿ − 1. The warp is the same odd
    /// function on both sides of every edge, so the folded cell is the neighbour on the sphere
    /// too. This is Design note 2's per-face-pair transform, computed rather than tabulated (T1.b
    /// as built); T2's golden writes its table out.
    fn step_cell(self, di: i64, dj: i64) -> Option<Self> {
        let half = 1_i64 << self.level;
        let cu = 2 * i64::from(self.i) + 1 - half + 2 * di;
        let cv = 2 * i64::from(self.j) + 1 - half + 2 * dj;
        let mut p = face_point(self.face, cu, cv, half);
        let mut over = (0..3).filter(|&a| p[a].abs() > half);
        let face = match (over.next(), over.next()) {
            (None, _) => self.face,
            (Some(axis), None) => {
                let own = face_axis(self.face);
                p[axis] = p[axis].signum() * half;
                p[own] = p[own].signum() * (half - 1);
                face_of_axis(axis, p[axis])
            }
            (Some(_), Some(_)) => return None,
        };
        let (u, v) = face_coords(face, p);
        let index = |c: i64| u32::try_from((c + half - 1) / 2).expect("a folded cell is on a face");
        Some(Self {
            face,
            level: self.level,
            i: index(u),
            j: index(v),
        })
    }

    /// The unit direction of vertex (`x`, `y`) of the patch's 65 × 65, in body-fixed axes.
    ///
    /// A vertex on a face edge or a cube corner is evaluated on its **canonical face**, the lowest
    /// index of the faces that contain it, so that every patch sharing it gets the same bits: two
    /// faces would compute the same edge point from different (u, v) and axes, and could differ in
    /// the last bit (plan R05, Design note 2).
    ///
    /// # Panics
    ///
    /// If `x` or `y` is above [`PATCH_QUADS`].
    #[must_use]
    pub fn vertex_dir(self, x: u8, y: u8) -> [f64; 3] {
        self.sample_dir(u32::from(x), u32::from(y), SampleGrid::Mesh)
    }

    /// The unit direction of sample (`x`, `y`) of `grid` over the patch, by
    /// [`vertex_dir`](Self::vertex_dir)'s canonical rule: the mesh's vertices on
    /// [`SampleGrid::Mesh`], and the double-resolution normals' samples on [`SampleGrid::Double`],
    /// whose even samples are the mesh's vertices bit for bit.
    ///
    /// # Panics
    ///
    /// If `x` or `y` is above the grid's quads a side.
    #[must_use]
    pub fn sample_dir(self, x: u32, y: u32, grid: SampleGrid) -> [f64; 3] {
        let (quads, along_u, along_v) = self.sample_lattice(x, y, grid.quads());
        let point = face_point(self.face, 2 * along_u - quads, 2 * along_v - quads, quads);
        let face = canonical_face(point, quads);
        let (u, v) = face_coords(face, point);
        // Both sums are even and non-negative, so the halving is exact.
        lattice_dir(
            face,
            i64::midpoint(u, quads),
            i64::midpoint(v, quads),
            quads,
        )
    }

    /// [`vertex_dir`](Self::vertex_dir) evaluated on the patch's own face, without the canonical
    /// rule: what the tests show the rule is needed against.
    #[cfg(test)]
    fn vertex_dir_on_own_face(self, x: u8, y: u8) -> [f64; 3] {
        let (quads, a, b) = self.sample_lattice(u32::from(x), u32::from(y), PATCH_QUADS);
        lattice_dir(self.face, a, b, quads)
    }

    /// The sample lattice of the patch's level at `per_patch` quads a patch: its quads a face
    /// side, `per_patch` × 2^level, and the sample's lattice indices (a, b) on the patch's face,
    /// each 0 to that number.
    #[must_use]
    fn sample_lattice(self, x: u32, y: u32, per_patch: u32) -> (i64, i64, i64) {
        assert!(
            x <= per_patch && y <= per_patch,
            "vertex ({x}, {y}) is outside a patch of {per_patch} quads"
        );
        let per_patch = i64::from(per_patch);
        let quads = per_patch << self.level;
        let a = i64::from(self.i) * per_patch + i64::from(x);
        let b = i64::from(self.j) * per_patch + i64::from(y);
        (quads, a, b)
    }
}

/// The unit direction of lattice vertex (`a`, `b`) of `face`, on a lattice of `quads` quads a
/// side: s = a ÷ quads, an exact binary fraction, then the warp.
fn lattice_dir(face: Face, a: i64, b: i64, quads: i64) -> [f64; 3] {
    #[expect(
        clippy::cast_precision_loss,
        reason = "lattice indices are at most 2^30, exact in f64"
    )]
    let exact = |n: i64| n as f64;
    let side = exact(quads);
    unit_dir(face_uv_to_xyz(FaceUv {
        face,
        u: st_to_uv(exact(a) / side),
        v: st_to_uv(exact(b) / side),
    }))
}

/// The integer point (`u`, `v`) of `face` on the cube of half-width `half`, by S2's axes.
const fn face_point(face: Face, u: i64, v: i64, half: i64) -> [i64; 3] {
    match face {
        Face::PosX => [half, u, v],
        Face::PosY => [-u, half, v],
        Face::PosZ => [-u, -v, half],
        Face::NegX => [-half, -v, -u],
        Face::NegY => [v, -half, -u],
        Face::NegZ => [v, u, -half],
    }
}

/// The inverse of [`face_point`]: the (u, v) of the integer point `p` on `face`.
const fn face_coords(face: Face, p: [i64; 3]) -> (i64, i64) {
    match face {
        Face::PosX => (p[1], p[2]),
        Face::PosY => (-p[0], p[2]),
        Face::PosZ => (-p[0], -p[1]),
        Face::NegX => (-p[2], -p[1]),
        Face::NegY => (-p[2], p[0]),
        Face::NegZ => (p[1], p[0]),
    }
}

/// The axis, 0 to 2, normal to `face`.
const fn face_axis(face: Face) -> usize {
    match face {
        Face::PosX | Face::NegX => 0,
        Face::PosY | Face::NegY => 1,
        Face::PosZ | Face::NegZ => 2,
    }
}

/// The face normal to `axis` on the side of `sign`'s sign.
fn face_of_axis(axis: usize, sign: i64) -> Face {
    let axis = u8::try_from(axis).expect("an axis is 0 to 2");
    let index = if sign > 0 { axis } else { axis + 3 };
    Face::from_index(index).expect("an axis and a side name a face")
}

/// The lowest-index face containing the integer point `p` of the cube of half-width `half`.
fn canonical_face(p: [i64; 3], half: i64) -> Face {
    (0..3)
        .filter(|&a| p[a].abs() == half)
        .map(|a| face_of_axis(a, p[a]))
        .min()
        .expect("a point of the cube lies on at least one face")
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_base::math;
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn ulps_apart(a: f64, b: f64) -> u64 {
        hyperion_testkit::float::ulps_apart(a, b)
    }

    #[test]
    fn the_warp_inverts_exactly_at_the_quarters_and_to_one_ulp_elsewhere() {
        for s in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_same_bits(uv_to_st(st_to_uv(s)), s);
        }
        assert_same_bits(st_to_uv(0.0), -1.0);
        assert_same_bits(st_to_uv(0.5), 0.0);
        assert_same_bits(st_to_uv(1.0), 1.0);
        let n = 10_000_u32;
        for i in 0..=n {
            let s = f64::from(i) / f64::from(n);
            let back = uv_to_st(st_to_uv(s));
            // Below ½ the warp works in 1 − s, so its one ulp is the spacing of 1 − s, 2⁻⁵³ in
            // [½, 1), not the far finer spacing of s itself near 0 (plan R05, T1.a as built).
            let within = if s >= 0.5 {
                ulps_apart(back, s) <= 1
            } else {
                (back - s).abs() <= f64::EPSILON / 2.0
            };
            assert!(within, "s = {s}, back = {back}");
        }
    }

    fn random_face_uv(rng: &mut Lcg, margin: f64) -> FaceUv {
        let face = Face::from_index(u8::try_from(rng.next_below(6)).unwrap()).unwrap();
        let span = 2.0 * (1.0 - margin);
        FaceUv {
            face,
            u: rng.next_f64() * span - (1.0 - margin),
            v: rng.next_f64() * span - (1.0 - margin),
        }
    }

    #[test]
    fn face_coordinates_round_trip_away_from_the_edges() {
        let mut rng = Lcg::new(0x5eed_c0be);
        for _ in 0..10_000 {
            let f = random_face_uv(&mut rng, 1e-6);
            let back = xyz_to_face_uv(face_uv_to_xyz(f));
            assert_eq!(back.face, f.face);
            assert!((back.u - f.u).abs() <= 1e-15, "{f:?} -> {back:?}");
            assert!((back.v - f.v).abs() <= 1e-15, "{f:?} -> {back:?}");
            // The scale of the vector does not matter.
            let p = face_uv_to_xyz(f);
            let unit = xyz_to_face_uv(unit_dir(p));
            assert_eq!(unit.face, f.face);
            assert!((unit.u - f.u).abs() <= 1e-15 && (unit.v - f.v).abs() <= 1e-15);
        }
    }

    #[test]
    fn every_face_centre_and_axis_is_s2s() {
        let centres = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, -1.0],
        ];
        for (face, centre) in Face::ALL.into_iter().zip(centres) {
            assert_eq!(Face::from_index(face.index()), Some(face));
            let xyz = face_uv_to_xyz(FaceUv {
                face,
                u: 0.0,
                v: 0.0,
            });
            // Equal as numbers: S2's negated axes give −0 where the centre has +0.
            for (got, want) in xyz.into_iter().zip(centre) {
                assert!((got - want).abs() <= 0.0, "{face:?}: {xyz:?}");
            }
            assert_eq!(face_of(centre), face);
        }
        assert_eq!(Face::from_index(6), None);
    }

    #[test]
    fn edges_and_corners_go_to_the_lowest_face_index() {
        // Each of the twelve edges' midpoints and the eight corners, by sign pattern.
        let signs = [-1.0, 0.0, 1.0];
        for &x in &signs {
            for &y in &signs {
                for &z in &signs {
                    let p = [x, y, z];
                    if p.iter().all(|&c| c == 0.0) {
                        continue;
                    }
                    let mut expected = u8::MAX;
                    for (axis, c) in (0_u8..).zip(p) {
                        if c != 0.0 {
                            expected = expected.min(if c > 0.0 { axis } else { axis + 3 });
                        }
                    }
                    let face = face_of(p);
                    assert_eq!(face.index(), expected, "{p:?}");
                    // The point lies on the face it is given to, with |u|, |v| ≤ 1.
                    let f = xyz_to_face_uv(p);
                    assert!(f.u.abs() <= 1.0 && f.v.abs() <= 1.0, "{p:?} -> {f:?}");
                }
            }
        }
    }

    #[test]
    fn every_direction_has_exactly_one_face() {
        let mut rng = Lcg::new(0xface);
        for _ in 0..10_000 {
            let p = [
                rng.next_f64() - 0.5,
                rng.next_f64() - 0.5,
                rng.next_f64() - 0.5,
            ];
            let f = xyz_to_face_uv(p);
            // The direction is on its face's square and maps back to itself.
            assert!(f.u.abs() <= 1.0 && f.v.abs() <= 1.0);
            let q = face_uv_to_xyz(f);
            let cross = cross(p, q);
            assert!(cross.iter().all(|c| c.abs() < 1e-15), "{p:?} -> {q:?}");
            assert!(dot(p, q) > 0.0);
            // No other face contains it in its interior.
            for other in Face::ALL {
                if other != f.face {
                    let a = face_uv_to_xyz(FaceUv {
                        face: other,
                        u: 0.0,
                        v: 0.0,
                    });
                    assert!(dot(p, a) <= p[0].abs().max(p[1].abs()).max(p[2].abs()));
                }
            }
        }
    }

    #[test]
    fn lines_of_constant_u_are_great_circles() {
        for face in Face::ALL {
            for i in 0..=8 {
                let fixed = f64::from(i) / 4.0 - 1.0;
                let normal_u = cross(
                    face_uv_to_xyz(FaceUv {
                        face,
                        u: fixed,
                        v: -0.9,
                    }),
                    face_uv_to_xyz(FaceUv {
                        face,
                        u: fixed,
                        v: 0.3,
                    }),
                );
                let normal_v = cross(
                    face_uv_to_xyz(FaceUv {
                        face,
                        u: -0.9,
                        v: fixed,
                    }),
                    face_uv_to_xyz(FaceUv {
                        face,
                        u: 0.3,
                        v: fixed,
                    }),
                );
                for j in 0..=10 {
                    let free = f64::from(j) / 5.0 - 1.0;
                    let on_u = unit_dir(face_uv_to_xyz(FaceUv {
                        face,
                        u: fixed,
                        v: free,
                    }));
                    assert!(dot(normal_u, on_u).abs() < 1e-15, "{face:?} u = {fixed}");
                    // And of constant v, through the centre too.
                    let on_v = unit_dir(face_uv_to_xyz(FaceUv {
                        face,
                        u: free,
                        v: fixed,
                    }));
                    assert!(dot(normal_v, on_v).abs() < 1e-15, "{face:?} v = {fixed}");
                }
            }
        }
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    /// The solid angle of the spherical triangle on unit vectors a, b, c, by Van Oosterom and
    /// Strackee (1983): tan(E ÷ 2) = |a · (b × c)| ÷ (1 + a·b + b·c + c·a).
    fn triangle_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
        let num = dot(a, cross(b, c)).abs();
        let den = 1.0 + dot(a, b) + dot(b, c) + dot(c, a);
        2.0 * math::atan2(num, den)
    }

    /// The exact area of cell (i, j) of face 0 at `level`, over two triangles.
    fn cell_area(level: u32, i: u32, j: u32) -> f64 {
        let n = f64::from(1_u32 << level);
        let corner = |a: u32, b: u32| {
            unit_dir(face_uv_to_xyz(FaceUv {
                face: Face::PosX,
                u: st_to_uv(f64::from(a) / n),
                v: st_to_uv(f64::from(b) / n),
            }))
        };
        let (p00, p10, p11, p01) = (
            corner(i, j),
            corner(i + 1, j),
            corner(i + 1, j + 1),
            corner(i, j + 1),
        );
        triangle_area(p00, p10, p11) + triangle_area(p00, p11, p01)
    }

    /// The largest and smallest cell areas of a level and where they lie, over one quadrant of
    /// one face, which the face's symmetries map onto all of it.
    fn area_extremes(level: u32) -> ((f64, u32, u32), (f64, u32, u32)) {
        let half = 1_u32 << (level - 1);
        let mut max = (0.0_f64, 0, 0);
        let mut min = (f64::INFINITY, 0, 0);
        for i in 0..half {
            for j in 0..half {
                let a = cell_area(level, i, j);
                if a > max.0 {
                    max = (a, i, j);
                }
                if a < min.0 {
                    min = (a, i, j);
                }
            }
        }
        (max, min)
    }

    #[test]
    fn the_cell_area_ratio_at_level_10_is_s2s() {
        let ((max, _, _), (min, mi, mj)) = area_extremes(10);
        let ratio = max / min;
        // Design note 3: 2.0917 at level 10, against 2.087 at 9 and 2.094 at 11.
        assert!(
            ((ratio - 2.0917) / 2.0917).abs() < 1e-4,
            "ratio at level 10 = {ratio}"
        );
        // Below S2's limit, kMaxArea ÷ kMinArea = 2.635799256963161491 ÷ (8√2 ÷ 9).
        let limit = 2.635_799_256_963_161_5 / (8.0 * 2.0_f64.sqrt() / 9.0);
        assert!(ratio < limit, "{ratio} ≥ {limit}");
        // The smallest cell is at an edge's midpoint: on the face's edge (i = 0 or j = 0 in this
        // quadrant, whose corner (0, 0) is the cube's) and next to its middle (index 511).
        let mid = (1_u32 << 9) - 1;
        assert!(
            (mi == 0 && mj == mid) || (mj == 0 && mi == mid),
            "smallest cell at ({mi}, {mj})"
        );
    }

    fn random_key(rng: &mut Lcg) -> PatchKey {
        let face = Face::from_index(u8::try_from(rng.next_below(6)).unwrap()).unwrap();
        let level = u8::try_from(rng.next_below(u64::from(MAX_LEVEL) + 1)).unwrap();
        let cells = 1_u64 << level;
        let i = u32::try_from(rng.next_below(cells)).unwrap();
        let j = u32::try_from(rng.next_below(cells)).unwrap();
        PatchKey::new(face, level, i, j).unwrap()
    }

    #[test]
    fn keys_round_trip_through_their_words() {
        let mut rng = Lcg::new(0x6b65_7973);
        for _ in 0..10_000 {
            let k = random_key(&mut rng);
            assert_eq!(PatchKey::from_u64(k.to_u64()), Ok(k));
        }
        let top = PatchKey::new(Face::NegZ, 24, (1 << 24) - 1, (1 << 24) - 1).unwrap();
        assert_eq!(top.to_u64(), 0x00b8_ffff_ffff_ffff);
        assert_eq!(PatchKey::from_u64(top.to_u64()), Ok(top));
        assert_eq!(PatchKey::root(Face::PosX).to_u64(), 0);
    }

    #[test]
    fn out_of_range_words_and_keys_are_refused() {
        assert_eq!(
            PatchKey::from_u64(1 << 56),
            Err(DecodePatchKeyError::UnusedBitsSet(1 << 56))
        );
        assert_eq!(
            PatchKey::from_u64(6 << 53),
            Err(DecodePatchKeyError::FaceOutOfRange(6))
        );
        assert_eq!(
            PatchKey::from_u64(25 << 48),
            Err(DecodePatchKeyError::Range(NewPatchKeyError::LevelAboveMax(
                25
            )))
        );
        // Level 3 has cells 0 to 7.
        assert_eq!(
            PatchKey::from_u64((3 << 48) | (8 << 24)),
            Err(DecodePatchKeyError::Range(
                NewPatchKeyError::IndexOutOfRange {
                    level: 3,
                    i: 8,
                    j: 0
                }
            ))
        );
        assert_eq!(
            PatchKey::new(Face::PosY, 0, 0, 1),
            Err(NewPatchKeyError::IndexOutOfRange {
                level: 0,
                i: 0,
                j: 1
            })
        );
    }

    #[test]
    fn parents_and_children_agree() {
        let mut rng = Lcg::new(0x7061_7265);
        for _ in 0..1_000 {
            let k = random_key(&mut rng);
            if k.level() < MAX_LEVEL {
                for child in k.children() {
                    assert_eq!(child.parent(), Some(k));
                }
            }
            match k.parent() {
                Some(parent) => assert!(parent.children().contains(&k)),
                None => assert_eq!(k.level(), 0),
            }
        }
    }

    /// The patches along a face's border and through its middle at `level`: every cell whose i and
    /// j are each 0, 1, the middle, the last but one or the last.
    fn sample_patches(level: u8) -> Vec<PatchKey> {
        let last = (1_u32 << level) - 1;
        let mut picks = vec![0, 1.min(last), last / 2, last.saturating_sub(1), last];
        picks.sort_unstable();
        picks.dedup();
        let mut keys = Vec::new();
        for face in Face::ALL {
            for &i in &picks {
                for &j in &picks {
                    keys.push(PatchKey::new(face, level, i, j).unwrap());
                }
            }
        }
        keys
    }

    /// The 65 vertices along `edge` of `key`, in increasing x or y, as bits.
    fn edge_bits(
        key: PatchKey,
        edge: Edge,
        dir: fn(PatchKey, u8, u8) -> [f64; 3],
    ) -> Vec<[u64; 3]> {
        let quads = u8::try_from(PATCH_QUADS).unwrap();
        (0..=quads)
            .map(|n| match edge {
                Edge::UMin => dir(key, 0, n),
                Edge::UMax => dir(key, quads, n),
                Edge::VMin => dir(key, n, 0),
                Edge::VMax => dir(key, n, quads),
            })
            .map(|d| d.map(hyperion_testkit::float::bits))
            .collect()
    }

    /// `b` in the order of `a`: `b` itself or reversed, whichever matches `a`'s first vertex.
    fn aligned(a: &[[u64; 3]], mut b: Vec<[u64; 3]>, first_matches: bool) -> Vec<[u64; 3]> {
        if !first_matches {
            b.reverse();
        }
        assert_eq!(a.len(), b.len());
        b
    }

    #[test]
    fn a_neighbours_neighbour_across_the_shared_edge_is_itself() {
        for level in [0, 1, 5, 24] {
            for key in sample_patches(level) {
                for edge in Edge::ALL {
                    let (neighbour, back) = key.edge_neighbour_and_back(edge);
                    assert_ne!(neighbour, key);
                    assert_eq!(neighbour.level(), level);
                    assert_eq!(neighbour.edge_neighbour(back), key, "{key:?} {edge:?}");
                }
            }
        }
    }

    #[test]
    fn shared_edges_are_bitwise_equal_from_both_sides() {
        let mut crossings = 0;
        let mut own_face_differences = 0;
        for level in [0, 1, 5, 19, 24] {
            for key in sample_patches(level) {
                for edge in Edge::ALL {
                    let (neighbour, back) = key.edge_neighbour_and_back(edge);
                    let ours = edge_bits(key, edge, PatchKey::vertex_dir);
                    let theirs = edge_bits(neighbour, back, PatchKey::vertex_dir);
                    let first_matches = theirs[0] == ours[0];
                    let theirs = aligned(&ours, theirs, first_matches);
                    assert_eq!(
                        ours, theirs,
                        "{key:?} {edge:?} against {neighbour:?} {back:?}"
                    );
                    if neighbour.face() != key.face() {
                        crossings += 1;
                        // Without the canonical rule, each side evaluates on its own face.
                        let own = edge_bits(key, edge, PatchKey::vertex_dir_on_own_face);
                        let other = aligned(
                            &own,
                            edge_bits(neighbour, back, PatchKey::vertex_dir_on_own_face),
                            first_matches,
                        );
                        own_face_differences +=
                            own.iter().zip(&other).filter(|(a, b)| a != b).count();
                    }
                }
            }
        }
        // Every face edge was crossed, and the rule is what makes the crossings agree.
        assert!(crossings >= 24 * 4, "{crossings} crossings");
        assert!(
            own_face_differences > 0,
            "evaluating on the patch's own face never differed, so the test cannot fail"
        );
    }

    #[test]
    fn each_cube_corner_has_one_direction_from_its_three_faces() {
        let quads = u8::try_from(PATCH_QUADS).unwrap();
        for level in [0, 5, 24] {
            let last = (1_u32 << level) - 1;
            let mut corners: Vec<([i8; 3], [u64; 3])> = Vec::new();
            for face in Face::ALL {
                for (ci, cj, x, y) in [
                    (0, 0, 0, 0),
                    (last, 0, quads, 0),
                    (0, last, 0, quads),
                    (last, last, quads, quads),
                ] {
                    let dir = PatchKey::new(face, level, ci, cj).unwrap().vertex_dir(x, y);
                    let signs = dir.map(|c| if c > 0.0 { 1_i8 } else { -1 });
                    corners.push((signs, dir.map(hyperion_testkit::float::bits)));
                }
            }
            corners.sort_unstable();
            assert_eq!(corners.len(), 24);
            for group in corners.chunks(3) {
                assert!(group.iter().all(|c| c.0 == group[0].0), "{group:?}");
                assert!(group.iter().all(|c| c.1 == group[0].1), "{group:?}");
            }
        }
    }

    #[test]
    fn the_double_grids_even_samples_are_the_vertices_bit_for_bit() {
        for key in [
            PatchKey::new(Face::PosX, 0, 0, 0).unwrap(),
            PatchKey::new(Face::NegZ, 7, 127, 0).unwrap(),
            PatchKey::new(Face::PosY, 24, 5, (1 << 24) - 1).unwrap(),
        ] {
            for y in 0..=64_u8 {
                for x in 0..=64_u8 {
                    let even =
                        key.sample_dir(2 * u32::from(x), 2 * u32::from(y), SampleGrid::Double);
                    assert_eq!(
                        even.map(hyperion_testkit::float::bits),
                        key.vertex_dir(x, y).map(hyperion_testkit::float::bits)
                    );
                }
            }
        }
    }

    #[test]
    fn a_cube_corner_patch_has_seven_neighbours() {
        for face in Face::ALL {
            for level in [1, 3, 24] {
                let last = (1_u32 << level) - 1;
                for (i, j) in [(0, 0), (last, 0), (0, last), (last, last)] {
                    let key = PatchKey::new(face, level, i, j).unwrap();
                    let corners = key.corner_neighbours();
                    assert_eq!(corners.iter().filter(|c| c.is_none()).count(), 1, "{key:?}");
                    let mut all: Vec<PatchKey> = Edge::ALL
                        .into_iter()
                        .map(|e| key.edge_neighbour(e))
                        .chain(corners.into_iter().flatten())
                        .collect();
                    all.sort_unstable();
                    all.dedup();
                    assert_eq!(all.len(), 7, "{key:?}");
                    assert!(!all.contains(&key));
                }
            }
        }
    }

    #[test]
    fn an_interior_patch_has_eight_neighbours_on_its_face() {
        let key = PatchKey::new(Face::NegY, 4, 7, 9).unwrap();
        let expected = [(6, 8), (8, 8), (8, 10), (6, 10)];
        for (got, (i, j)) in key.corner_neighbours().into_iter().zip(expected) {
            assert_eq!(got, Some(PatchKey::new(Face::NegY, 4, i, j).unwrap()));
        }
        assert_eq!(
            key.edge_neighbour(Edge::UMax),
            PatchKey::new(Face::NegY, 4, 8, 9).unwrap()
        );
    }

    #[test]
    fn a_face_edge_neighbour_is_the_adjacent_cell_on_the_sphere() {
        // The neighbour's centre lies one cell's arc away, across the edge, at every level here.
        let mut rng = Lcg::new(0x6e65_6967);
        for _ in 0..2_000 {
            let key = random_key(&mut rng);
            for edge in Edge::ALL {
                let neighbour = key.edge_neighbour(edge);
                let half = PATCH_QUADS / 2;
                let half = u8::try_from(half).unwrap();
                let a = key.vertex_dir(half, half);
                let b = neighbour.vertex_dir(half, half);
                // Centres of adjacent cells of level n are 0.6 to 2.0 cell widths of the mean
                // (π ÷ 2 ÷ 2ⁿ radians) apart, and never the same point.
                let width = core::f64::consts::FRAC_PI_2 / f64::from(1_u32 << key.level());
                let gap = dot(a, b).clamp(-1.0, 1.0);
                let angle = math::acos(gap);
                assert!(
                    angle > 0.5 * width && angle < 2.0 * width,
                    "{key:?} {edge:?}: {angle} vs {width}"
                );
            }
        }
    }
}
