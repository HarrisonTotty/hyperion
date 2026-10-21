//! Horizon maps, the terrain's shadows on itself (plan R10, T8.a; Design note 10).
//!
//! A patch's horizon map holds the elevation of the horizon in eight azimuths: the highest angle
//! above the local horizontal, against the datum's normal ν, at which the ground meets the sky in
//! that direction (Max 1988; Sloan and Cohen 2000). It holds it at each of the patch's 65 × 65
//! vertices, from the patch's own heights (the near field), and at each point of a 9 × 9 lattice
//! over the patch, from the ground beyond it (the far field). Nothing in it depends on the light,
//! so a patch never rebakes for the sun or the body's rotation: R10.T8.b's shader compares the
//! sun's elevation in its azimuth against the larger of the near field and the far field, each
//! interpolated linearly in azimuth and the far field bilinearly between its points, and turns the
//! difference into the share of the star's disc above the horizon.
//!
//! # The azimuths
//!
//! The eight azimuths are the patch grid's own directions, [`AZIMUTH_STEPS`]: azimuth k looks along
//! the grid step (dx, dy), from +x (k = 0) through the diagonal (1, 1) to +y (k = 2) and on round,
//! counterclockwise seen from outside the body, as S2's (u, v) axes turn. A direction's *grid
//! azimuth* ([`grid_azimuth`]) is the angle of its components (a, b) on the grid's tangents at the
//! point, a ∂p/∂x + b ∂p/∂y, as atan2(b, a): k × 45° exactly on the grid's directions, and in
//! between linear in that angle, whatever the grid's skew. On a face's diagonal near a cube corner,
//! where the grid's lines meet at 120°, the eight directions are 30° to 60° apart on the ground
//! rather than 45°; over most of a face they are within about 12° of it. The grid's own directions
//! let the near field read only the patch's vertices, along its grid lines, with no interpolation,
//! and sweep each line once (below). A tangent frame that is the same across every face edge does
//! not exist (no continuous tangent field covers a sphere), so the azimuths are each face's own.
//!
//! # The encoding
//!
//! An elevation e is stored as the signed normalised 16-bit value of e ÷ 90° over [−90°, 90°]
//! ([`encode_horizon`]), two azimuths to a `u32` word as WGSL's core `pack2x16snorm` packs them
//! (azimuth 2w in the low half of word w, 2w + 1 in the high half), four words a vertex, which
//! `unpack2x16snorm` reads back ([`decode_horizon`]). The step is 180° ÷ 65,534, 9.9″, and the
//! worst round trip 4.94″ at every elevation (Design note 10). The words are laid out vertex by
//! vertex, row by row, four to a vertex: an `array<vec4u>` in a storage buffer, the slot field
//! `horizon-map` the shader binds (decision-r10-t10b-r09-t2e §5), the near field's 65 × 65 vertices
//! and then the far field's 9 × 9 points.
//!
//! # The near field
//!
//! At each vertex and azimuth, the highest ground along the patch's grid line through it, ahead in
//! that direction, from the patch's own heights and, beyond its edges, a margin of eight of its
//! level's vertices on its neighbours: the drawn mesh, whose edges along a row or column are the
//! segments between its vertices, so that the line's highest point seen from the vertex is one of
//! them. It is found by Timonen and Westerholm 2010's line sweep, O(n) a line: each line is walked
//! from its far end back, and a stack keeps the convex hull of the vertices passed, in the plane
//! through the body's centre that holds the line's two ends (a row or a column of S2's grid is a
//! great circle, in that plane; a diagonal, which S2's warp bends a little off it, and a
//! spheroid's heights along ν, are projected onto it). The hull's tangent from a vertex is its
//! horizon, since the horizon is the most raised
//! direction from the vertex in that plane whatever the planet's curvature; its elevation is then
//! taken exactly, from the two points' body-fixed positions and the datum's normal, so that the
//! curvature drop, d² ÷ 2R on a sphere, is included and no small-angle form is needed.
//!
//! The margin's heights are those the neighbours' own vertices have, the height function's at the
//! same lattice vertex of the same level, read as the far field reads its ground (below): from the
//! caller's cache, where a neighbour baked before keeps them, or asked, and the reference surface
//! where the view cannot make them, the map then not complete. So a vertex on a patch's edge,
//! looking out, sees the ground its neighbour's vertex there sees within the margin, and the two
//! sides of an edge differ only by the ground beyond it, which each takes from the far field
//! (R10.T8.c). The margin stops at the face's edge, where the grid's lines turn onto another face's
//! grid: a vertex on a face's edge, looking off the face, has no near horizon, [`NO_HORIZON`].
//!
//! # The far field
//!
//! At each point of the patch's 9 × 9 lattice (every eighth vertex), the highest ground in each
//! azimuth beyond: a march along the great circle that leaves the point along the azimuth's grid
//! direction, at angular distances θₖ = θ₀ × 2^(k/4), from θ₀ one of the patch's vertex spacings
//! and growing by 2^¼ ≈ 1.19 a step (Design note 10's 1.15–1.25), to the reach √(2 Δh ÷ c) at
//! most, c the datum's polar radius and Δh the field's realised relief, beyond which the curvature
//! drop exceeds every height difference the body has. Each sample's height is the bilinear
//! interpolation of four vertex heights of a quadtree level m chosen so that the level's vertex
//! spacing is about the step's length, θₖ (2^¼ − 1), and never finer than the patch's own: the
//! patch's level for the first twelve steps, to 2^(11/4) θ₀, and one coarser every four steps
//! after. The point stands at its own height, the patch's vertex's. The patch's own vertex heights
//! and its parent level's at its even vertices are the bake's; the others are kept in the caller's
//! [`SynthCache`] by face, level and lattice vertex, with the band of its own that its neighbours'
//! near fields and marches read, so that a patch's neighbours, and its children, read them again
//! for free.
//!
//! The lattice is the face's ([`far_field_point`]): its points are the same points, with the same
//! values, in every patch of a level on the face that holds them, so the far field is the same bits
//! on both sides of a patch edge. It is not the same across levels, since the march starts at the
//! level's own vertex spacing and reads the level's own ground: a finer level's far field at a
//! shared point starts nearer and reads finer ground (R10's Risks, "Deviations in T8.a, as
//! built"). Nor is it across a face's edge, where the azimuths turn with the face's grid.
//!
//! # The survey
//!
//! The near field reads the bake's own heights, where a vertex the view cannot make already stands
//! at the reference surface. A vertex of its margin, or a far sample's, that the view cannot make
//! takes the reference surface too, height 0, the datum the client draws there (Design note 12),
//! and the map is then not [complete](HorizonMap::is_complete): a later block may bring the ground
//! that casts the shadow. The map names the cells it was refused for
//! ([`HorizonMap::refused_cells`]): each the first cell a refused height's reads lacked, the one
//! whose arrival alone can change that height, so that a patch is baked again exactly on a block
//! that brings one of them (decision-r10-t8a §3, R10.T8.b; the margin's refusals since R10.T8.c).
//!
//! # Determinism
//!
//! Every value is a pure function of the view's cells, the detail seed and the patch: the same
//! bits in any order of bakes and from any cache whose views only grow, on every target. The cache
//! keeps only answered heights, each the height function's own; a refused one is asked again by the
//! next bake. The map is presentation (the plan's Generator version section): goldens pin it at
//! the generator version in force, and a change to it re-blesses them with no bump; nothing is
//! drawn from a stream.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, SQRT_2, TAU};

use hyperion_base::math;
use hyperion_base::units::Radians;

use crate::cube::{
    Face, FaceUv, MAX_LEVEL, PATCH_QUADS, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir, uv_on_face,
    uv_to_st, xyz_to_face_uv,
};
use crate::field::{FieldView, cell_index};
use crate::geometry::vertex_spacing;
use crate::num;
use crate::patch::PATCH_VERTICES;
use crate::spheroid::Spheroid;
use crate::synth::channels::NetworkId;
use crate::synth::{BandLevel, PatchHeights, QueryHeightError, SynthCache, Synthesiser};

/// The azimuths a horizon map holds.
pub const AZIMUTHS: usize = 8;

/// Each azimuth's grid step (dx, dy) over a patch's vertices, azimuth k at k × 45° of grid
/// azimuth ([`grid_azimuth`]): +x, the diagonal (1, 1), +y and on round, counterclockwise seen from
/// outside the body.
pub const AZIMUTH_STEPS: [[i8; 2]; AZIMUTHS] = [
    [1, 0],
    [1, 1],
    [0, 1],
    [-1, 1],
    [-1, 0],
    [-1, -1],
    [0, -1],
    [1, -1],
];

/// The far field's points along a patch's side: every [`FAR_STRIDE`]th vertex, 9.
pub const FAR_SIDE: usize = 9;

/// The patch's vertices from one far-field point to the next along a side.
pub const FAR_STRIDE: u8 = 8;

/// The `u32` words a vertex or a far-field point takes: two azimuths to a word.
pub const WORDS_PER_TEXEL: usize = 4;

/// The code of no horizon, −90°: a vertex with no ground ahead of it on its face (on the face's
/// edge, looking off it), or a far-field point with no sample within the reach (a body with no
/// relief).
pub const NO_HORIZON: i16 = -32767;

/// The azimuths of a turn, as a number of 45° steps.
const TURN: f64 = 8.0;

/// The largest code, 2¹⁵ − 1, that WGSL's `pack2x16snorm` scales by.
const SNORM16_MAX: f64 = 32_767.0;

/// The far march's growth by step within an octave, 2^(k ÷ 4) for k = 0 to 3, so that four steps
/// double the distance exactly and a level's steps are its parent's halved, bit for bit.
const GROWTH: [f64; 4] = [1.0, 1.189_207_115_002_721, SQRT_2, 1.681_792_830_507_429];

/// The steps an octave of distance takes.
const STEPS_PER_OCTAVE: usize = 4;

/// The octaves of the march read at the patch's own level before coarser ones: over these three,
/// from θ₀ to 2^(11/4) θ₀, the step's length, θ (2^¼ − 1), stays under about 1.3 θ₀, the patch's
/// vertex spacing θ₀, which no step reads finer than.
const OWN_LEVEL_OCTAVES: usize = 3;

/// The far field's vertex heights a cache keeps before it starts afresh: about 4 MB, the far
/// fields of some 30 to 60 patches.
const FAR_HEIGHTS_HELD: usize = 1 << 17;

/// The cells a bake's far march keeps the four vertex heights of, direct-mapped by key: the
/// samples of neighbouring points and of successive steps fall in the same cell, and a kept cell
/// spares four look-ups in the cache's map. Against one cell kept, the march with every height
/// kept took 5.07 ms against 5.98 on the Earth-like world's L + 4 patch, 9.21 against 10.05 at its
/// finest level, and 5.11 against 5.15 on the Moon-like rim (2026-10-15, release, one thread, the
/// least of five, under a shared load of 8 to 18: provisional).
const CELLS_KEPT: u64 = 1 << 12;

/// Whether a bake makes the horizon map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HorizonLayer {
    /// The near and far fields, for the lit style's shadows (R10.T8.b) on both settings.
    Baked,
    /// None: the map is empty, for a style that draws no shadow, as the wireframe's depth-only
    /// terrain (R10.T11) and R05's spike shading.
    Skipped,
}

/// A patch's horizon map (see the [module documentation](self)): each vertex's and each far-field
/// point's horizon elevation in [`AZIMUTHS`] azimuths, encoded by [`encode_horizon`], two to a
/// word.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct HorizonMap {
    near: Vec<u32>,
    far: Vec<u32>,
    refused: bool,
    refused_cells: Vec<u32>,
}

impl HorizonMap {
    /// The near field's words: [`WORDS_PER_TEXEL`] a vertex, vertex by vertex, row by row (y
    /// outer, x inner), azimuth 2w in word w's low half and 2w + 1 in its high half, as
    /// `pack2x16snorm` packs them; 65 × 65 × 4 words, none where the map is not baked.
    #[must_use]
    pub fn near_words(&self) -> &[u32] {
        &self.near
    }

    /// The far field's words, laid out as the near field's over its [`FAR_SIDE`] × [`FAR_SIDE`]
    /// points, point (a, b) at vertex (8a, 8b); none where the map is not baked.
    #[must_use]
    pub fn far_words(&self) -> &[u32] {
        &self.far
    }

    /// Whether the map holds nothing: [`HorizonLayer::Skipped`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.near.is_empty() && self.far.is_empty()
    }

    /// Whether the near field's margin and the far field read only ground the view could make.
    ///
    /// It is false where a vertex of the margin or a far sample took the reference surface for
    /// want of a cell the view does not hold, or the bake's own heights stood at it: a later block
    /// can bring ground that raises the horizon, so the map can change while the bake's survey mask
    /// does not ([`TerrainLayers::is_complete`](super::TerrainLayers::is_complete) does not count
    /// it).
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        !self.refused
    }

    /// The cells the near field's margin and the far field were refused for, as rising
    /// [`cell_index`]es at the field's level.
    ///
    /// Each is, for a height the view could not make, the first cell its reads lacked
    /// ([`QueryHeightError::NotSurveyed`]), each once. Every read before that cell was held and is
    /// final, so the height can change only once that cell is held: a block that brings none of
    /// these cells, surveyed or margin, leaves the map's words as they are (decision-r10-t8a §3).
    /// Empty for a map whose margin and far field read only ground the view holds, and for one not
    /// baked; a map whose bake was not complete may be incomplete with none, its own heights
    /// standing at the reference surface.
    #[must_use]
    pub fn refused_cells(&self) -> &[u32] {
        &self.refused_cells
    }

    /// The near field's horizon at vertex (`x`, `y`) in azimuth `k` (the index of
    /// [`AZIMUTH_STEPS`]).
    ///
    /// # Panics
    ///
    /// If the map is empty, `x` or `y` is above 64, or `k` is not below [`AZIMUTHS`].
    #[must_use]
    pub fn near(&self, x: u8, y: u8, k: usize) -> Radians {
        assert!(x <= 64 && y <= 64, "vertex ({x}, {y}) is outside a patch");
        let texel = usize::from(y) * PATCH_VERTICES + usize::from(x);
        decode_horizon(code_of(&self.near, texel, k))
    }

    /// The far field's horizon at point (`a`, `b`), over vertex (8a, 8b), in azimuth `k` (the
    /// index of [`AZIMUTH_STEPS`]).
    ///
    /// # Panics
    ///
    /// If the map is empty, `a` or `b` is above 8, or `k` is not below [`AZIMUTHS`].
    #[must_use]
    pub fn far(&self, a: u8, b: u8, k: usize) -> Radians {
        assert!(a <= 8 && b <= 8, "far point ({a}, {b}) is outside a patch");
        let texel = usize::from(b) * FAR_SIDE + usize::from(a);
        decode_horizon(code_of(&self.far, texel, k))
    }

    /// The horizon at vertex (`x`, `y`) towards the grid azimuth `azimuth` ([`grid_azimuth`]), as
    /// R10.T8.b's shader takes it there: the near field's and the far field's, each linear in
    /// azimuth between the two azimuths about it, the far field bilinear between the four points
    /// about the vertex, and the larger of the two.
    ///
    /// # Panics
    ///
    /// If the map is empty, `x` or `y` is above 64, or `azimuth` is not finite.
    #[must_use]
    pub fn elevation_at(&self, x: u8, y: u8, azimuth: Radians) -> Radians {
        let (k0, k1, between) = azimuth_knots(azimuth);
        let along = |at: &dyn Fn(usize) -> f64| lerp(at(k0), at(k1), between);
        let near = along(&|k| self.near(x, y, k).value());
        let (column, across) = far_cell(x);
        let (row, up) = far_cell(y);
        let far_at = |c: u8, r: u8| along(&|k| self.far(c, r, k).value());
        let far = lerp(
            lerp(far_at(column, row), far_at(column + 1, row), across),
            lerp(far_at(column, row + 1), far_at(column + 1, row + 1), across),
            up,
        );
        Radians::new(num::max(near, far))
    }
}

/// The code of azimuth `k` at texel `texel` of `words`.
///
/// # Panics
///
/// If `texel` is past the words or `k` is not below [`AZIMUTHS`].
#[must_use]
fn code_of(words: &[u32], texel: usize, k: usize) -> i16 {
    assert!(k < AZIMUTHS, "azimuth {k} is not below {AZIMUTHS}");
    let word = words[texel * WORDS_PER_TEXEL + k / 2];
    let half = if k.is_multiple_of(2) {
        word
    } else {
        word >> 16
    };
    u16::try_from(half & 0xffff)
        .expect("a half word fits a u16")
        .cast_signed()
}

/// The two azimuths about the grid azimuth `azimuth` and its fraction of the way from the first to
/// the second.
///
/// # Panics
///
/// If `azimuth` is not finite.
#[must_use]
fn azimuth_knots(azimuth: Radians) -> (usize, usize, f64) {
    let phi = azimuth.value();
    assert!(phi.is_finite(), "an azimuth must be finite, got {phi}");
    let q = (phi / FRAC_PI_4).rem_euclid(TURN);
    let floor = q.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the floor of a value in [0, 8] is an integer from 0 to 8"
    )]
    let k = (floor as usize) % AZIMUTHS;
    (k, (k + 1) % AZIMUTHS, q - floor)
}

/// The far-field point at or before vertex index `x` along a side and the fraction of the way to
/// the next, the last pair's end at the side's last vertex.
#[must_use]
fn far_cell(x: u8) -> (u8, f64) {
    let a = (x / FAR_STRIDE).min(FAR_STRIDE - 1);
    (a, f64::from(x - a * FAR_STRIDE) / f64::from(FAR_STRIDE))
}

/// `a` + (`b` − `a`) `f`, in that form.
#[must_use]
fn lerp(a: f64, b: f64, f: f64) -> f64 {
    a + (b - a) * f
}

/// The code of an elevation in WGSL's `pack2x16snorm` rounding: ⌊½ + 32,767 × clamp(e ÷ 90°, −1,
/// 1)⌋, from −32,767 at −90° to 32,767 at 90° (WGSL, "Data Packing Built-in Functions").
///
/// The product 32,767 × (e ÷ 90°) is taken exactly, as two `f64`s ([`math::two_product`]), and the
/// floor decided on their sum, so that the code is the formula's in real arithmetic for every
/// `f64` quotient: for an `f32` input, as WGSL takes it, the formula's own.
///
/// # Panics
///
/// If `elevation` is a NaN: an elevation the bake made is a number.
#[must_use]
pub fn encode_horizon(elevation: Radians) -> i16 {
    snorm16(elevation.value() / FRAC_PI_2)
}

/// `x`'s signed normalised 16-bit code, ⌊½ + 32,767 × clamp(x, −1, 1)⌋, exactly (see
/// [`encode_horizon`]).
///
/// # Panics
///
/// If `x` is a NaN.
#[must_use]
pub(crate) fn snorm16(x: f64) -> i16 {
    assert!(!x.is_nan(), "a horizon's elevation is a number");
    if x >= 1.0 {
        return 32_767;
    }
    if x <= -1.0 {
        return -32_767;
    }
    // p = hi + lo exactly. ⌊hi + ½⌋ can be one off only where p + ½ is within a rounding of an
    // integer n, where hi lies within a factor of two of n ∓ ½ and so hi − (n ∓ ½) is exact
    // (Sterbenz's lemma), and the sign of that plus lo is the sign of p + ½ − n.
    let (hi, lo) = math::two_product(SNORM16_MAX, x);
    let mut n = (hi + 0.5).floor();
    if (hi - (n - 0.5)) + lo < 0.0 {
        n -= 1.0;
    } else if (hi - (n + 0.5)) + lo >= 0.0 {
        n += 1.0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an integer in [−32,767, 32,767], exact in i16"
    )]
    let code = n as i16;
    code
}

/// The elevation of a code as WGSL's `unpack2x16snorm` reads it, max(code ÷ 32,767, −1) × 90°: −90°
/// at −32,767 and −32,768, 0 at 0 and 90° at 32,767, rising with the code.
#[must_use]
pub fn decode_horizon(code: i16) -> Radians {
    Radians::new(num::max(f64::from(code) / SNORM16_MAX, -1.0) * FRAC_PI_2)
}

/// The far-field lattice's point (`i`, `j`) at `level` on `face`: s = i ÷ (8 × 2^level) and t =
/// j ÷ (8 × 2^level), exact binary fractions, through S2's warp (Design note 10).
///
/// The lattice is the face's, a patch side's spacing ÷ [`FAR_STRIDE`], and a patch of `level` holds
/// its points (8 × patch i + a, 8 × patch j + b), a and b from 0 to 8, so points on a shared edge
/// are the same point. A point is the same bits at every level that holds it: (`i`, `j`) at
/// `level` is (2`i`, 2`j`) at `level` + 1.
///
/// # Panics
///
/// If `level` is above [`MAX_LEVEL`], or `i` or `j` above 8 × 2^level.
#[must_use]
pub fn far_field_point(face: Face, level: u8, i: u32, j: u32) -> FaceUv {
    assert!(level <= MAX_LEVEL, "level {level} is above the maximum");
    let intervals = u32::from(FAR_STRIDE) << level;
    assert!(
        i <= intervals && j <= intervals,
        "far-field point ({i}, {j}) is past level {level}'s {intervals} intervals"
    );
    let side = f64::from(intervals);
    FaceUv {
        face,
        u: st_to_uv(f64::from(i) / side),
        v: st_to_uv(f64::from(j) / side),
    }
}

/// The grid azimuth of the direction `towards` at the direction `at`, both body-fixed, on
/// `face`'s grid (see the [module documentation](self)): with `towards`' tangential part
/// a ∂p/∂x + b ∂p/∂y on the grid's tangents at `at`, the angle atan2(b, a) in [0, 2π), at which
/// azimuth k of a horizon map lies at k × 45°; `None` where `towards` has no tangential part, its
/// part normal to `at` under [`ZENITH_TOLERANCE`] of its length (the zenith, the nadir, or a zero
/// `towards`).
///
/// The tangents are those of S2's grid, ∂p/∂x ∝ (U − p (p · U)) du/ds and ∂p/∂y ∝ (V − p (p · V))
/// dv/dt, with U and V the face's axes, p the unit direction of `at`, and du/ds = 8s ÷ 3 for s at
/// least ½ and 8 (1 − s) ÷ 3 below it, the derivative of S2's quadratic warp ([`st_to_uv`]); the
/// factors they share, 1 ÷ |q| on the face's plane and the patch's 2^−(level + 6), cancel. `at`
/// need not be unit, and may lie beyond the face's edges, where its coordinates extend the face's.
///
/// # Panics
///
/// If `at` or `towards` is not finite, or `at` is not in front of `face` (its component along the
/// face's normal is not positive).
#[must_use]
pub fn grid_azimuth(face: Face, at: [f64; 3], towards: [f64; 3]) -> Option<Radians> {
    assert!(
        at.iter().chain(&towards).all(|c| c.is_finite()),
        "a grid azimuth's directions must be finite, got {at:?} and {towards:?}"
    );
    let normal = face_uv_to_xyz(FaceUv {
        face,
        u: 0.0,
        v: 0.0,
    });
    assert!(
        dot(at, normal) > 0.0,
        "{at:?} is not in front of face {face:?}"
    );
    let unit = unit_dir(at);
    let tangential = sub(towards, scale(unit, dot(towards, unit)));
    let length = dot(towards, towards).sqrt();
    if dot(tangential, tangential).sqrt() <= ZENITH_TOLERANCE * length {
        return None;
    }
    let (fu, fv) = uv_on_face(face, at);
    let [g_x, g_y] = grid_tangents(face, unit, uv_to_st(fu), uv_to_st(fv));
    let (xx, xy, yy) = (dot(g_x, g_x), dot(g_x, g_y), dot(g_y, g_y));
    let (wx, wy) = (dot(g_x, tangential), dot(g_y, tangential));
    // The solve of [xx xy; xy yy] (a, b) = (wx, wy), times its positive determinant.
    let across = yy * wx - xy * wy;
    let along = xx * wy - xy * wx;
    let phi = math::atan2(along, across);
    Some(Radians::new(if phi < 0.0 { phi + TAU } else { phi }))
}

/// The share of a direction's length below which its part normal to the point's direction is
/// taken as none, so that [`grid_azimuth`] has no azimuth: 10⁻¹², some 4,500 `f64` roundings of a
/// unit vector, under any tangent a shader's `f32` resolves.
pub const ZENITH_TOLERANCE: f64 = 1e-12;

/// S2's grid tangents at the unit direction `p` of cell coordinates (`s`, `t`) on `face`, each
/// up to the factor they share (see [`grid_azimuth`]).
#[must_use]
fn grid_tangents(face: Face, p: [f64; 3], s: f64, t: f64) -> [[f64; 3]; 2] {
    let origin = face_uv_to_xyz(FaceUv {
        face,
        u: 0.0,
        v: 0.0,
    });
    let axis = |u: f64, v: f64| sub(face_uv_to_xyz(FaceUv { face, u, v }), origin);
    let tangent = |a: [f64; 3], rate: f64| scale(sub(a, scale(p, dot(p, a))), rate);
    [
        tangent(axis(1.0, 0.0), warp_rate(s)),
        tangent(axis(0.0, 1.0), warp_rate(t)),
    ]
}

/// du/ds of S2's quadratic warp at s: 8s ÷ 3 for s at least ½, 8 (1 − s) ÷ 3 below it.
#[must_use]
fn warp_rate(s: f64) -> f64 {
    if s >= 0.5 {
        8.0 * s / 3.0
    } else {
        8.0 * (1.0 - s) / 3.0
    }
}

/// Bakes `key`'s horizon map from its own heights `heights` (R05's bake's, the reference surface
/// where the view could not make a vertex, the bake then not `complete`) and, for the far field,
/// the ground `synth` makes beyond it, its vertex heights kept in `cache` (see the
/// [module documentation](self)).
#[must_use]
pub(super) fn bake_horizon<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    heights: &PatchHeights,
    complete: bool,
) -> HorizonMap {
    let header = synth.field().header();
    let figure = header.figure();
    let steps = far_steps(key.level(), &figure, header.realised_relief().value());
    let mut ground = FarGround {
        synth,
        id: synth.channels().id(),
        own: heights,
        cells: vec![None; usize::try_from(CELLS_KEPT).expect("4,096 cells fit a usize")],
        refused_here: BTreeSet::new(),
        refused_cells: BTreeSet::new(),
        // The bake's own heights stand at the reference surface where it was not complete.
        refused: !complete,
    };
    // The margin's heights are the far field's ground at the patch's level: the height function's
    // at the lattice vertex, as the neighbour's own vertex there has it.
    let near = near_codes(key, &figure, heights, |i, j| {
        ground.vertex_height_m(cache, key.face(), key.level(), i, j)
    });
    let far = far_codes(&mut ground, cache, key, &figure, &steps);
    if complete {
        keep_own_band(cache, ground.id, heights);
    }
    HorizonMap {
        near: pack(&near),
        far: pack(&far),
        refused: ground.refused,
        refused_cells: ground.refused_cells.into_iter().collect(),
    }
}

/// The vertices within this many of a patch's edge, at its level and its parent's, that its
/// neighbours' far marches and near fields read: the march reads the patch's own level to
/// 2^(11/4) ≈ 6.7 vertex spacings from a far point and its parent's to as many of the parent's, a
/// sample's cell reaches one vertex further, and the near field's margin is [`NEAR_MARGIN`].
const OWN_BAND: u8 = 8;

/// The vertices of its level the near field reads beyond each of a patch's edges, on its
/// neighbours: the band a complete bake keeps of its own for them (`OWN_BAND`), so that a patch
/// baked beside baked neighbours reads its margin from the cache, and the far field's stride, so
/// that from every edge vertex the near field reaches the next far point's distance out.
const NEAR_MARGIN: u8 = OWN_BAND;

const _: () = assert!(
    NEAR_MARGIN == FAR_STRIDE,
    "the near field's margin reaches the next far point's distance out from every edge vertex"
);

/// Keeps in `cache` the heights of `heights`' patch that its neighbours' near fields' margins and
/// far fields read ([`OWN_BAND`]): its vertices near its edges at its level, and its even ones at
/// its parent's.
/// They are the height function's own, every one answered (the caller's `complete`), so the
/// cache's rule, answered heights only, holds.
fn keep_own_band(cache: &mut SynthCache, id: NetworkId, heights: &PatchHeights) {
    let key = heights.key();
    let quads = u8::try_from(PATCH_QUADS).expect("64 quads fit a u8");
    let near_edge = |x: u8, y: u8, last: u8| x.min(last - x).min(y).min(last - y) <= OWN_BAND;
    let kept = cache.far_heights_mut();
    kept.keep(id);
    if kept.heights.len() >= FAR_HEIGHTS_HELD {
        kept.heights.clear();
    }
    let (i0, j0) = (key.i() * PATCH_QUADS, key.j() * PATCH_QUADS);
    for y in 0..=quads {
        for x in (0..=quads).filter(|&x| near_edge(x, y, quads)) {
            let at = memo_key(
                key.face(),
                key.level(),
                i0 + u32::from(x),
                j0 + u32::from(y),
            );
            kept.heights
                .entry(at)
                .or_insert(heights.sample(x, y).height_m);
        }
    }
    if let Some(parent) = key.level().checked_sub(1) {
        let half = quads / 2;
        for y in 0..=half {
            for x in (0..=half).filter(|&x| near_edge(x, y, half)) {
                let at = memo_key(
                    key.face(),
                    parent,
                    i0 / 2 + u32::from(x),
                    j0 / 2 + u32::from(y),
                );
                let h = heights
                    .parent_height_m(2 * x, 2 * y)
                    .expect("a patch above level 0 carries its parent's heights");
                kept.heights.entry(at).or_insert(h);
            }
        }
    }
}

/// The codes, two to a word, the first in the low half.
#[must_use]
fn pack(codes: &[i16]) -> Vec<u32> {
    let (pairs, rest) = codes.as_chunks::<2>();
    assert!(rest.is_empty(), "codes come two to a word");
    pairs
        .iter()
        .map(|&[low, high]| {
            u32::from(low.cast_unsigned()) | (u32::from(high.cast_unsigned()) << 16)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The near field.
// ---------------------------------------------------------------------------------------------

/// The near field's codes, [`AZIMUTHS`] a vertex, vertex by vertex, row by row (see the
/// [module documentation](self)): the line sweep over the patch's own heights `heights` and, ahead
/// of its edges, its margin ([`NearGrid`]), whose heights `margin_m` gives by their lattice indices
/// (i, j) at the patch's level on its face, metres along the datum's normal.
#[must_use]
pub(super) fn near_codes(
    key: PatchKey,
    figure: &Spheroid,
    heights: &PatchHeights,
    mut margin_m: impl FnMut(u32, u32) -> f64,
) -> Vec<i16> {
    let grid = NearGrid::of(key);
    let count = grid.len();
    let mut datum = Vec::with_capacity(count);
    let mut up = Vec::with_capacity(count);
    let mut position = Vec::with_capacity(count);
    for y in grid.y0..=grid.y1 {
        for x in grid.x0..=grid.x1 {
            let (dir, h) = if let Some((vx, vy)) = NearGrid::own(x, y) {
                (key.vertex_dir(vx, vy), heights.sample(vx, vy).height_m)
            } else {
                let (i, j) = grid.lattice(x, y);
                (
                    lattice_vertex_dir(key.face(), key.level(), i, j),
                    margin_m(i, j),
                )
            };
            let base = figure.point(dir);
            let nu = figure.normal(dir);
            position.push(add(base, scale(nu, h)));
            datum.push(base);
            up.push(nu);
        }
    }
    let points = Points {
        position: &position,
        up: &up,
        datum: &datum,
    };
    let mut codes = vec![NO_HORIZON; PATCH_VERTICES * PATCH_VERTICES * AZIMUTHS];
    let mut sweep = Sweep::default();
    let side = i32::try_from(PATCH_VERTICES).expect("65 fits an i32");
    for (k, &[dx, dy]) in AZIMUTH_STEPS.iter().enumerate() {
        let (dx, dy) = (i32::from(dx), i32::from(dy));
        for y in 0..side {
            for x in 0..side {
                // A line starts at the patch's vertex whose step back leaves the patch, since only
                // the ground ahead of a vertex is its horizon, and runs on through the margin.
                if NearGrid::own(x - dx, y - dy).is_some() {
                    continue;
                }
                sweep.line.clear();
                sweep.texels.clear();
                let (mut cx, mut cy) = (x, y);
                while let Some(n) = grid.index(cx, cy) {
                    sweep.line.push(n);
                    if let Some((vx, vy)) = NearGrid::own(cx, cy) {
                        sweep
                            .texels
                            .push(usize::from(vy) * PATCH_VERTICES + usize::from(vx));
                    }
                    cx += dx;
                    cy += dy;
                }
                sweep.run(&points, k, &mut codes);
            }
        }
    }
    codes
}

/// The near field's grid about a patch, in the patch's vertex indices: its own 65 × 65 vertices
/// and, beyond each edge, [`NEAR_MARGIN`] of its level's on its face, fewer where the face's edge
/// comes first, so that x runs from `x0` (−8 to 0) to `x1` (64 to 72), and y likewise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NearGrid {
    /// The lattice indices at the patch's level of the patch's vertex (0, 0).
    i0: u32,
    j0: u32,
    /// The grid's extent in the patch's vertex indices, both ends held.
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
}

impl NearGrid {
    /// `key`'s grid.
    #[must_use]
    fn of(key: PatchKey) -> Self {
        let margin = u32::from(NEAR_MARGIN);
        let last = PATCH_QUADS << key.level();
        let (i0, j0) = (key.i() * PATCH_QUADS, key.j() * PATCH_QUADS);
        let quads = i32::try_from(PATCH_QUADS).expect("64 fits an i32");
        let to = |n: u32| i32::try_from(n.min(margin)).expect("a margin of 8 fits an i32");
        Self {
            i0,
            j0,
            x0: -to(i0),
            x1: quads + to(last - i0 - PATCH_QUADS),
            y0: -to(j0),
            y1: quads + to(last - j0 - PATCH_QUADS),
        }
    }

    /// The vertices it holds.
    #[must_use]
    fn len(&self) -> usize {
        let side = |a: i32, b: i32| usize::try_from(b - a + 1).expect("an extent is positive");
        side(self.x0, self.x1) * side(self.y0, self.y1)
    }

    /// The index, row by row, of vertex (`x`, `y`), or `None` outside the grid.
    #[must_use]
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if !(self.x0..=self.x1).contains(&x) || !(self.y0..=self.y1).contains(&y) {
            return None;
        }
        let width = self.x1 - self.x0 + 1;
        Some(
            usize::try_from((y - self.y0) * width + (x - self.x0))
                .expect("a vertex within the grid has a non-negative index"),
        )
    }

    /// The lattice indices at the patch's level, on its face, of vertex (`x`, `y`) of the grid.
    ///
    /// # Panics
    ///
    /// If (`x`, `y`) lies before the face's first row or column.
    #[must_use]
    fn lattice(&self, x: i32, y: i32) -> (u32, u32) {
        let on = |start: u32, n: i32| {
            start
                .checked_add_signed(n)
                .expect("a vertex of the grid is on the face")
        };
        (on(self.i0, x), on(self.j0, y))
    }

    /// The patch's own vertex (`x`, `y`), or `None` where it is not one of the patch's.
    #[must_use]
    fn own(x: i32, y: i32) -> Option<(u8, u8)> {
        let quads = u8::try_from(PATCH_QUADS).expect("64 quads fit a u8");
        // An index that fits no u8, negative or past 255, is not the patch's: that is the answer.
        let own = |n: i32| u8::try_from(n).ok().filter(|&n| n <= quads);
        Some((own(x)?, own(y)?))
    }
}

/// A near field's vertices in `f64`, row by row over its grid: their body-fixed positions, the
/// datum's normal under each, and the datum's point under each.
struct Points<'a> {
    position: &'a [[f64; 3]],
    up: &'a [[f64; 3]],
    datum: &'a [[f64; 3]],
}

/// One line's sweep: the line's vertices in its azimuth's order, the patch's own first and its
/// margin's after them, the patch's texels of its own, their places in its plane, and the hull's
/// stack, kept between lines for their allocations.
#[derive(Default)]
struct Sweep {
    line: Vec<usize>,
    texels: Vec<usize>,
    plane: Vec<[f64; 2]>,
    hull: Vec<usize>,
}

impl Sweep {
    /// Writes the near horizon in azimuth `azimuth` of each of the patch's own vertices of the
    /// line to `codes` by its texel: Timonen and Westerholm's sweep from the line's far end back,
    /// the hull kept in the plane through the body's centre and the line's two ends, in which the
    /// line runs counterclockwise, so that a point ahead stands higher as seen from a vertex where
    /// it lies clockwise of another. The margin's vertices join the hull and take no code.
    fn run(&mut self, points: &Points<'_>, azimuth: usize, codes: &mut [i16]) {
        let (Some(&first), Some(&last)) = (self.line.first(), self.line.last()) else {
            return;
        };
        if self.line.len() < 2 {
            return;
        }
        let b1 = unit_dir(points.datum[first]);
        let end = points.datum[last];
        let b2 = unit_dir(sub(end, scale(b1, dot(end, b1))));
        self.plane.clear();
        self.plane.extend(
            self.line
                .iter()
                .map(|&n| [dot(points.position[n], b1), dot(points.position[n], b2)]),
        );
        self.hull.clear();
        for at in (0..self.line.len()).rev() {
            let from = self.plane[at];
            while let [.., farther, nearer] = self.hull[..] {
                // The nearer hull point goes when the farther stands at least as high from here.
                if cross(
                    sub2(self.plane[nearer], from),
                    sub2(self.plane[farther], from),
                ) <= 0.0
                {
                    self.hull.pop();
                } else {
                    break;
                }
            }
            // The patch's own vertices are the line's first ones, `texels` theirs in order: a grid
            // line that leaves the patch, a rectangle, does not come back to it.
            if let (Some(&texel), Some(&top)) = (self.texels.get(at), self.hull.last()) {
                let (vertex, horizon) = (self.line[at], self.line[top]);
                let seen = elevation(
                    points.position[vertex],
                    points.up[vertex],
                    points.position[horizon],
                );
                codes[texel * AZIMUTHS + azimuth] = encode_horizon(seen);
            }
            self.hull.push(at);
        }
    }
}

/// The elevation of `target` seen from `from`, against the up direction `up` (unit): the angle of
/// their difference above the plane normal to `up`.
#[must_use]
pub(super) fn elevation(from: [f64; 3], up: [f64; 3], target: [f64; 3]) -> Radians {
    let d = sub(target, from);
    let v = dot(d, up);
    let h = sub(d, scale(up, v));
    Radians::new(math::atan2(v, dot(h, h).sqrt()))
}

// ---------------------------------------------------------------------------------------------
// The far field.
// ---------------------------------------------------------------------------------------------

/// One step of the far march: its angular distance and the level whose vertex heights it reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FarStep {
    /// The angular distance θ along the great circle, radians.
    pub(super) theta_rad: f64,
    /// cos θ.
    cos: f64,
    /// sin θ.
    sin: f64,
    /// The quadtree level whose vertex heights the sample interpolates.
    pub(super) level: u8,
}

/// The far march's reach on a datum `figure` for a realised relief of `relief_m` metres: √(2 Δh ÷
/// c), the angle beyond which the curvature drop d² ÷ 2R passes Δh (Design note 10's √(2 R Δh)
/// as an angle), with R the polar radius c, where an oblate datum drops least below its tangent
/// plane, c (1 − cos θ); and π ÷ 2 at most.
#[must_use]
pub(super) fn far_reach(figure: &Spheroid, relief_m: f64) -> Radians {
    Radians::new(num::min(
        (2.0 * relief_m / figure.polar_radius_m).sqrt(),
        FRAC_PI_2,
    ))
}

/// The far march's steps for a patch of `level` on a datum `figure` with a realised relief of
/// `relief_m` metres (see the [module documentation](self)).
#[must_use]
pub(super) fn far_steps(level: u8, figure: &Spheroid, relief_m: f64) -> Vec<FarStep> {
    let first = vertex_spacing(1.0, level).mean_m;
    let reach = far_reach(figure, relief_m).value();
    let mut steps = Vec::new();
    let mut octave = first;
    for k in 0.. {
        if k > 0 && k % STEPS_PER_OCTAVE == 0 {
            octave *= 2.0;
        }
        let theta = octave * GROWTH[k % STEPS_PER_OCTAVE];
        if theta > reach {
            break;
        }
        let coarser = u8::try_from((k / STEPS_PER_OCTAVE + 1).saturating_sub(OWN_LEVEL_OCTAVES))
            .unwrap_or(u8::MAX);
        let (sin, cos) = math::sin_cos(theta);
        steps.push(FarStep {
            theta_rad: theta,
            cos,
            sin,
            level: level.saturating_sub(coarser),
        });
    }
    steps
}

/// The far field's codes, [`AZIMUTHS`] a point, point by point, row by row (see the
/// [module documentation](self)).
#[must_use]
fn far_codes<F: FieldView>(
    ground: &mut FarGround<'_, '_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    figure: &Spheroid,
    steps: &[FarStep],
) -> Vec<i16> {
    let face = key.face();
    let quads = f64::from(PATCH_QUADS << key.level());
    let mut codes = vec![NO_HORIZON; FAR_SIDE * FAR_SIDE * AZIMUTHS];
    for row in 0..=FAR_STRIDE {
        for column in 0..=FAR_STRIDE {
            let (vx, vy) = (column * FAR_STRIDE, row * FAR_STRIDE);
            // The point on the patch's level's vertex lattice, at its own height.
            let gi = key.i() * PATCH_QUADS + u32::from(vx);
            let gj = key.j() * PATCH_QUADS + u32::from(vy);
            let at = key.vertex_dir(vx, vy);
            let own_m = ground.own.sample(vx, vy).height_m;
            let observer = figure.surface_point(at, own_m);
            let up = figure.normal(at);
            let tangents = grid_tangents(face, at, f64::from(gi) / quads, f64::from(gj) / quads);
            let point = usize::from(row) * FAR_SIDE + usize::from(column);
            for (k, &[dx, dy]) in AZIMUTH_STEPS.iter().enumerate() {
                let along = unit_dir(add(
                    scale(tangents[0], f64::from(dx)),
                    scale(tangents[1], f64::from(dy)),
                ));
                let mut best: Option<(f64, f64)> = None;
                for step in steps {
                    // A unit vector to a few parts in 10¹⁶, `at` and `along` being unit and
                    // normal: the datum's point and normal take it as it is.
                    let dir = add(scale(at, step.cos), scale(along, step.sin));
                    let height_m = ground.height_m(cache, dir, step.level);
                    let rel = sub(figure.surface_point(dir, height_m), observer);
                    let sample = (dot(rel, up), dot(rel, rel));
                    if best.is_none_or(|best| higher(sample, best)) {
                        best = Some(sample);
                    }
                }
                if let Some((rise, squared)) = best {
                    let run = num::max(squared - rise * rise, 0.0).sqrt();
                    codes[point * AZIMUTHS + k] =
                        encode_horizon(Radians::new(math::atan2(rise, run)));
                }
            }
        }
    }
    codes
}

/// Whether the sample (v, d²), its height v above the observer's horizontal and its squared
/// distance d², stands higher than `best`: v |v| ÷ d², the signed square of its elevation's sine,
/// is larger, compared by cross-multiplying.
#[must_use]
fn higher((v, d2): (f64, f64), (best_v, best_d2): (f64, f64)) -> bool {
    v * v.abs() * best_d2 > best_v * best_v.abs() * d2
}

/// The far field's ground, and the near field's margin's: the vertex heights of a level, the
/// patch's own and its parent level's from the bake, others from the caller's cache where it keeps
/// them, the reference surface where the view cannot make them.
struct FarGround<'s, 'a, F> {
    synth: &'s Synthesiser<'a, F>,
    /// The field the cache's heights are kept for.
    id: NetworkId,
    /// The bake's heights of the patch: its vertices' at its level and its even vertices' at its
    /// parent's, each the height function's own at that lattice vertex.
    own: &'s PatchHeights,
    /// The cells whose four vertex heights the bake has read, by the key of their first vertex.
    cells: Vec<Option<(u64, [f64; 4])>>,
    /// The vertices the view refused in this bake, so that each is asked once.
    refused_here: BTreeSet<u64>,
    /// The [`cell_index`] of the cell each refusal named, the first cell its reads lacked.
    refused_cells: BTreeSet<u32>,
    /// Whether any vertex took the reference surface.
    refused: bool,
}

impl<F: FieldView> FarGround<'_, '_, F> {
    /// The height at the unit direction `dir`, metres above the datum along its normal: the
    /// bilinear interpolation of the four vertex heights of `level` about it, on the face that
    /// holds it, in the cell coordinates (s, t).
    fn height_m(&mut self, cache: &mut SynthCache, dir: [f64; 3], level: u8) -> f64 {
        let FaceUv { face, u, v } = xyz_to_face_uv(dir);
        let quads = PATCH_QUADS << level;
        let (i, fx) = lattice_cell(uv_to_st(u), quads);
        let (j, fy) = lattice_cell(uv_to_st(v), quads);
        let key = memo_key(face, level, i, j);
        let slot = usize::try_from((key ^ (key >> 28) ^ (key >> 56)) % CELLS_KEPT)
            .expect("a slot index is below the kept cells");
        let [h00, h10, h01, h11] = match self.cells[slot] {
            Some((kept, heights)) if kept == key => heights,
            _ => {
                let heights = [
                    self.vertex_height_m(cache, face, level, i, j),
                    self.vertex_height_m(cache, face, level, i + 1, j),
                    self.vertex_height_m(cache, face, level, i, j + 1),
                    self.vertex_height_m(cache, face, level, i + 1, j + 1),
                ];
                self.cells[slot] = Some((key, heights));
                heights
            }
        };
        lerp(lerp(h00, h10, fx), lerp(h01, h11, fx), fy)
    }

    /// The height of vertex (`i`, `j`) of `level`'s lattice on `face` at band level `level`,
    /// metres above the datum along its normal, kept in `cache`; the reference surface, 0, where
    /// the view cannot make it.
    fn vertex_height_m(
        &mut self,
        cache: &mut SynthCache,
        face: Face,
        level: u8,
        i: u32,
        j: u32,
    ) -> f64 {
        if let Some(own_m) = self.own_height_m(face, level, i, j) {
            return own_m;
        }
        let key = memo_key(face, level, i, j);
        let kept = cache.far_heights_mut();
        kept.keep(self.id);
        if let Some(&h) = kept.heights.get(&key) {
            return h;
        }
        if self.refused_here.contains(&key) {
            self.refused = true;
            return 0.0;
        }
        let band = BandLevel::new(level).expect("a far step's level is at most its patch's");
        match self
            .synth
            .height_at(cache, lattice_vertex_dir(face, level, i, j), band)
        {
            Ok(sample) => {
                let kept = cache.far_heights_mut();
                if kept.heights.len() >= FAR_HEIGHTS_HELD {
                    kept.heights.clear();
                }
                kept.heights.insert(key, sample.height_m);
                sample.height_m
            }
            Err(QueryHeightError::NotSurveyed(cell)) => {
                // A cell the view does not hold: the reference surface, until a block brings it,
                // and the cell named so that the block that does is known.
                self.refused_here.insert(key);
                self.refused_cells.insert(cell_index(cell));
                self.refused = true;
                0.0
            }
        }
    }
}

impl<F> FarGround<'_, '_, F> {
    /// The bake's height of vertex (`i`, `j`) of `level`'s lattice on `face`, metres, where it is
    /// one of the patch's vertices at its level or an even one at its parent's: the same bits as
    /// [`Synthesiser::height_at`] there, which the bake asked at the same direction, so that no
    /// value depends on whether it is read here or asked again (and a vertex the bake could not
    /// make is the reference surface either way).
    #[must_use]
    fn own_height_m(&self, face: Face, level: u8, i: u32, j: u32) -> Option<f64> {
        let key = self.own.key();
        if face != key.face() {
            return None;
        }
        let quads = u8::try_from(PATCH_QUADS).expect("64 quads fit a u8");
        let index = |n: u32, start: u32, last: u8| {
            n.checked_sub(start)
                .and_then(|k| u8::try_from(k).ok())
                .filter(|&k| k <= last)
        };
        if level == key.level() {
            let x = index(i, key.i() * PATCH_QUADS, quads)?;
            let y = index(j, key.j() * PATCH_QUADS, quads)?;
            Some(self.own.sample(x, y).height_m)
        } else if key.level().checked_sub(1) == Some(level) {
            let half = PATCH_QUADS / 2;
            let x = index(i, key.i() * half, quads / 2)?;
            let y = index(j, key.j() * half, quads / 2)?;
            self.own.parent_height_m(2 * x, 2 * y)
        } else {
            None
        }
    }
}

/// The cell of a lattice of `quads` quads a side holding the cell coordinate `s` in [0, 1], and
/// `s`'s fraction of the way across it; the last cell for s = 1.
#[must_use]
fn lattice_cell(s: f64, quads: u32) -> (u32, f64) {
    let x = s * f64::from(quads);
    let floor = num::max(x.floor(), 0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the floor of a value in [0, quads], at most 2^30, is a non-negative integer"
    )]
    let i = (floor as u32).min(quads - 1);
    (i, x - f64::from(i))
}

/// The unit direction of vertex (`i`, `j`) of `level`'s lattice on `face`, by
/// [`PatchKey::vertex_dir`]'s canonical rule, so that a vertex on a face edge is the same bits
/// from either face.
#[must_use]
fn lattice_vertex_dir(face: Face, level: u8, i: u32, j: u32) -> [f64; 3] {
    let last = (1_u32 << level) - 1;
    let (pi, pj) = ((i / PATCH_QUADS).min(last), (j / PATCH_QUADS).min(last));
    let index = |n: u32, p: u32| u8::try_from(n - p * PATCH_QUADS).expect("a vertex of the patch");
    PatchKey::new(face, level, pi, pj)
        .expect("a lattice vertex's patch is on the face")
        .vertex_dir(index(i, pi), index(j, pj))
}

/// A lattice vertex's key in the far field's memo: the face, the level and the vertex's indices,
/// 28 bits each. A level's indices reach 64 × 2^level, below 2^28 to level 21; the march's first
/// steps and the near field's margin read the patch's own level, whose indices need 29 bits from
/// level 22 (of [`MAX_LEVEL`]'s 24), where keys would collide. Patches stop near levels 19 to 20,
/// at the body's finest level (`finest_level`: 19 for an Earth, one finer for each doubling of the
/// radius), so no rocky world's bake reaches it (R10.T8.c's deferred finding).
#[must_use]
fn memo_key(face: Face, level: u8, i: u32, j: u32) -> u64 {
    (u64::from(face.index()) << 61) | (u64::from(level) << 56) | (u64::from(i) << 28) | u64::from(j)
}

/// The far field's vertex heights a [`SynthCache`] keeps (see the [module documentation](self)):
/// by face, level and lattice vertex, for one field at a time, as the channels' part is kept
/// ([`NetworkId`]); it starts afresh at [`FAR_HEIGHTS_HELD`]. Each is the height function's own,
/// so no map depends on what it holds.
#[derive(Debug, Clone, Default)]
pub(crate) struct FarHeights {
    of: Option<NetworkId>,
    heights: BTreeMap<u64, f64>,
}

impl FarHeights {
    /// No heights.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            of: None,
            heights: BTreeMap::new(),
        }
    }

    /// Keeps the heights of the field `of` only: starts afresh for another.
    fn keep(&mut self, of: NetworkId) {
        if self.of != Some(of) {
            self.of = Some(of);
            self.heights.clear();
        }
    }

    /// How many heights it holds.
    #[cfg(test)]
    #[must_use]
    pub(super) fn len(&self) -> usize {
        self.heights.len()
    }
}

// ---------------------------------------------------------------------------------------------
// Vectors.
// ---------------------------------------------------------------------------------------------

#[must_use]
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[must_use]
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[must_use]
fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[must_use]
fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// The z-component of the 2D cross product a × b: positive where b lies counterclockwise of a.
#[must_use]
fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

#[cfg(test)]
mod tests;
