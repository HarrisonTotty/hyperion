//! The channel network: a body's river channels between its coarse cells and its band limit, one
//! Dendry-style network whose first level is the coarse flow network itself (plan R09, T6.a;
//! Design note 13).
//!
//! The network follows Gaillard, Benes, Guérin, Galin, Rohmer and Cani 2019 (Dendry: a procedural
//! model for dendritic patterns, I3D, doi:10.1145/3306131.3317020) and the behaviour of its
//! reference code (github.com/mgaillard/Noise, `NoiseLib/include/noise.h`), implemented from their
//! description; no code is taken from it (GPL-3.0). Its geometry is procedural and labelled so; its
//! profiles are physics. Its levels are the quadtree's, from the field's coarse level L down,
//! each level's cells a quarter of the one above.
//!
//! # Key points
//!
//! Every cell of every level from L down has one **key point**. A cell of level L has its own,
//! drawn on `surface.channel` keyed by the cell ([`ObjectKey::surface_cell`], instance 0, words 0
//! and 1): its face coordinates (s, t), each uniform in [ε, 1 − ε) of the cell with ε =
//! [`KEY_POINT_MARGIN`], 0.25, so that it lies in the cell's central half (Dendry's
//! `GeneratePoint`, whose ε keeps points from the cell's borders). A cell of a finer level
//! **inherits** its parent's key point when that point lies in it (Dendry's
//! `ReplaceNeighboringPoints`), and draws its own otherwise, so every coarser key point is a key
//! point of every finer level and three of a parent's four children have new ones. The coordinates
//! are integers in units of 2⁻⁵⁰ of the face's side, so that which child holds a point is exact and
//! an inherited point is the same bits at every level. A key point never lies on a face edge, so
//! it has one face; its direction is computed from that face alone ([`KeyPoint::dir`]), and a
//! query from either side of a face edge reads the same bits for it.
//!
//! # Segments
//!
//! Each key point heads at most one **segment**, down which its channel flows ([`Segment`]): a
//! straight 3D chord between spheroid points in the body-fixed frame (metres), or two for a trunk.
//!
//! - **The first level** is the coarse flow network itself: the key point of a coarse cell whose
//!   flow direction names a receiver is joined to the receiver's key point (a *trunk*), through a
//!   **crossing** on the edge the two cells share: where the straight chord between the key points
//!   meets the edge's great circle, held to the edge's central half ([`CROSSING_MARGIN`]). A coarse
//!   cell is geodesically convex (its edges are great circles: a line of constant u on a cube face
//!   is one), so the trunk's first chord lies in its own cell and its second in the receiver, and
//!   it leaves its cell at its **exit** parameter, the crossing. The hold matters only at a face
//!   edge near a cube corner: two cells across a face edge are not together convex, since a row of
//!   cells bends there, and the straight chord between their key points can pass through a third
//!   cell; on the synthetic worlds the hold binds on under 0.2% of the trunks, nearly all of them
//!   across face edges (R09.T6.a's test).
//! - **Every later level's** new key point joins the nearest point of the segments of all coarser
//!   levels (a *tributary*), found at each coarser level in the 3 × 3 graph neighbourhood of the
//!   key point's cell there (its cell, its four edge neighbours and its corner neighbours, by the
//!   cube's neighbour rule, across face edges), by the 3D chord distance between spheroid points:
//!   the brainstorm's rule (its per-query evaluation, step 4). Ties go to the coarser level, then
//!   to the lower packed cell key ([`PatchKey::to_u64`]), then to the lower parameter along the
//!   segment, so that no tie depends on the order the candidates are found in. Dendry's paper needs
//!   at least 5 × 5 cells for a key point to find its true nearest segment (§4.1.2); the 3 × 3 is
//!   the plan's (Design note 13), so a join is the nearest within it. The neighbourhoods do not
//!   stop at coarse cells, so a tributary may join a neighbouring cell's network: the sub-cell
//!   divides fall between the channels, where the coarse field cannot place them, and not on the
//!   coarse cells' edges, so the coarse drainage areas hold for the trunks, not for every
//!   tributary's water.
//!
//! A coarse cell carries tributaries only where it is dry and has a channel (a flow direction and a
//! steepness index above zero). A key point that is inherited heads no segment of its own, and nor
//! does one that lies on the segment it would join.
//!
//! # Profiles
//!
//! Each segment carries its bed's height above the datum along it ([`Segment::bed_at`]). A trunk's
//! runs straight from its cell's water surface to its receiver's (the ground on dry land, and the
//! sea's or a lake's level where a channel reaches one): it carries the coarse field's own fall
//! between the two cells over its own length. The field's slope S = `k_s` A^−θ is that fall over
//! the distance between the cells' centres ([`LogSteepness`]), and a trunk's key points, each in
//! its cell's central half, are 0.5 to 1.6 of that distance apart, so a trunk's slope is about 0.6
//! to 2 times the field's, and at a confluence it can be steeper than its own tributaries. A
//! tributary's follows the steady-state stream-power slope S = `k_s` A^−θ (θ =
//! [`LogSteepness::THETA`], 0.45, the reference concavity; Design note 9, after Kirby and Whipple
//! 2012, J. Struct. Geol. 44, 54, doi:10.1016/j.jsg.2012.07.009, not yet checked against the paper)
//! with its coarse cell's `k_s`, which a steady state shares among a region's channels (with n = 1,
//! `k_s` = u ÷ k wherever uplift u and erodibility k are uniform: Tzathas et al. 2024, eq. 9), held
//! to at least [`MINIMUM_SLOPE`] (Dendry's minimum slope), and the drainage area from **Hack's
//! law** in SI, L = C A^h with C = [`HACK_COEFFICIENT`] and h = [`HACK_EXPONENT`] ([`hack_area`]),
//! L being the main stream's length from the divide: at the key point, [`head_length`], the length
//! Hack's law gives one mean cell of the key point's level, and growing by the distance along the
//! segment. Since θ ÷ h = ¾, the bed rises from the join upstream by ∫ `k_s` C^¾ L^−¾ dL = 4 `k_s`
//! C^¾ (L₂^¼ − L₁^¼) in closed form, the stream-power part through square roots alone. A
//! tributary's bed at its join is its parent's bed there, bit for bit, so every segment descends to
//! its parent at no less than the minimum slope.
//!
//! Hack's constants are Earth's (Virginia and Maryland's rivers), and overstate how integrated a
//! network is on a world whose wet epoch was short (the brainstorm's per-query evaluation). Hack's
//! basins span 0.31 to 971 km² (Hack 1957, p. 45), so the law is extrapolated at both ends: a
//! continental trunk's drainage is far above it, and from level 15 on an Earth a key point's head
//! drains under its smallest (where Hack's L falls below √A, under 9 × 10⁴ m², a main stream would
//! be shorter than its basin is wide). The stream-power law itself holds only below slopes of about
//! 0.03 to 0.1, above which debris flows cut the valleys (Stock and Dietrich 2003, WRR 39(4), 1089,
//! doi:10.1029/2001WR001057), which a head at level 11 on an Earth reaches where `k_s` is 300
//! m^0.9: where the network ends, at the channel heads, is R09.T6.b's level cut
//! (`decision-r09-t5.md`, adjacent finding C).
//!
//! The profile reads the field's drainage and `k_s` as geometric: Hack's law relates a stream's
//! length to its planimetric area (Hack 1957, p. 47). Where the coarse pass replaces the area by
//! Hergarten's runoff-weighted equivalent area (Design note 9; [`LogArea`](crate::field::LogArea)'s
//! documentation), a tributary's slope from that `k_s` and a geometric area is off by the runoff,
//! in metres a year, to the power θ, and a trunk's main stream from that area by its power h: asked
//! of "main", for R09.T14 (the plan's Risks, "Deviations in T6.a, as built"). The synthetic worlds'
//! areas are geometric. The network reads the liquid that cut it only through the field's `k_s` and
//! drainage, which the coarse pass computes with that liquid's erodibility (Design note 9's K,
//! R09.T0.b), so it is the same for water, methane–ethane or any other: nothing here names a
//! liquid.
//!
//! # What it reads, and what is left to R09.T6.b
//!
//! The network reads the coarse cells through the view: their flow directions, steepness indices,
//! drainage areas and water surfaces. A tributary's join reads the trunks of the 3 × 3 coarse cells
//! about its own, each with its receiver, and the finer segments about it, each with its own join,
//! so the segments below a coarse cell read cells up to three king moves from it (R09.T6.a's test),
//! inside the margin's five (Design note 15 budgets four for the first level); [`ReadCellError`]
//! names the first that is missing. Its relief, the incision a valley cut about each channel makes,
//! with each level's mean over its parent cell removed, is R09.T6.b's, as are the network's full
//! depth to the band limit, its cut by channel width, the per-patch cache in the caller's
//! [`SynthCache`](super::SynthCache), and the channels' share of the unresolved variance: a
//! contribution of its own beside the spectrum's structural share, which the coarse pass books in
//! `σ_h`'s budget once erosion has run (R09.T14.d's second rescale; R09.T12.e and
//! `decision-r09-t5.md` item 4, by which it is zero before erosion), from the per-level rise
//! [`tributary_rise`] gives. [`ChannelNetwork`] memoises what it computes, per network, and no
//! result depends on what it has computed before.

use std::collections::BTreeMap;

use hyperion_base::math;
use hyperion_base::rng::{DetailSeed, ObjectKey};
use hyperion_base::units::{Metres, SquareMetres};

use super::interp::ReadCellError;
use crate::cube::{Edge, Face, FaceUv, MAX_LEVEL, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use crate::field::{FieldHeader, FieldView, LogSteepness, SynthesisCell};
use crate::num;
use crate::spheroid::Spheroid;
use crate::tags::SURFACE_CHANNEL;

/// ε, the share of a cell's side by which a drawn key point keeps from each of its edges: 0.25,
/// so that it lies in the cell's central half (Design note 13; Dendry's `GeneratePoint` draws
/// each coordinate uniform in [ε, 1 − ε)).
pub const KEY_POINT_MARGIN: f64 = 0.25;

/// The share of a coarse cell's edge by which a trunk's crossing keeps from each of the edge's
/// ends: the crossing is where the straight chord between the two key points meets the edge, held
/// to the edge's central half, as the key points are held to their cells' (R09.T6.a's own).
pub const CROSSING_MARGIN: f64 = 0.25;

/// Hack's coefficient C in SI, m^−0.2: a basin's main stream is L = C A^h long, metres, from the
/// divide to the point where its drainage area is A, square metres (Design note 13).
///
/// Hack 1957 (Studies of longitudinal stream profiles in Virginia and Maryland, USGS Professional
/// Paper 294-B, eq. 3, p. 63) gives L = 1.4 A^0.6 with L in miles and A in square miles, L measured
/// along the channel, following its meanders, to the divide at the head of the longest stream
/// above (pp. 47–48). In SI, C = 1.4 × 1,609.344^(1 − 2h) = 0.319 741 (the international mile,
/// exact), the plan's 0.320. The 1.5 quoted with h = 0.6 by Tzathas et al. 2024 (Table 1) is a
/// mile-based figure, as Hack's own Christians Creek line, L = 1.5 A^0.62 in miles (p. 66), is,
/// and in SI would make streams 4.7 times too long. Earth's: Hack's coefficient "averages around
/// 1.4 but ranges between 1 and 2.5" (p. 65), and the law is "valid only for the region under
/// discussion" (p. 64), basins of 0.31 to 971 km² (p. 45); labelled so on every other world. Read
/// from the paper (pubs.usgs.gov/pp/0294b/report.pdf) by R09.T6.a's science check.
pub const HACK_COEFFICIENT: f64 = 0.319_741;

/// Hack's exponent h, 0.6: L = C A^h ([`HACK_COEFFICIENT`]; Hack 1957, eq. 3, p. 63).
///
/// Earth's, as the coefficient is: Arizona's and the Black Hills' basins give 0.7 (Hack 1957,
/// p. 64), and h ranges 0.5 to 0.7 across studies (Dodds and Rothman 2000, arXiv:physics/0005047,
/// §IX).
pub const HACK_EXPONENT: f64 = 0.6;

/// The least slope a tributary's bed falls at, metres a metre: 10⁻⁵, a centimetre a kilometre.
///
/// R09.T6.a's own floor, after Dendry's minimum slope (Gaillard et al. 2019's
/// `elevationWithMinSlope`), which keeps every tributary descending to its parent. The stream-power
/// slope it floors is positive wherever a coarse cell has channels, so the floor binds only on the
/// gentlest: two thirds of the lowest water-surface gradient measured on the Amazon, 1.5 cm a
/// kilometre 800 to 1,020 km from its mouth (Birkett et al. 2002, JGR 107(D20), 8059,
/// doi:10.1029/2001JD000609, as Fassoni-Andrade et al. 2021, Rev. Geophys. 59, e2020RG000728,
/// report it), and on sub-cell tributaries only where the cell's steepness index is near its
/// smallest code, beyond a main stream of about a kilometre at 0.0043 m^0.9. One floor at every
/// level, since the profile itself steepens with level: the brainstorm's "held to a minimum slope
/// by level" read as each level held to its parent.
pub const MINIMUM_SLOPE: f64 = 1e-5;

/// A key point's face coordinates are integers in units of 2⁻⁵⁰ of the face's side: 24 bits for the
/// cell of the deepest level, [`MAX_LEVEL`], and [`CELL_BITS`] within it.
const POSITION_BITS: u32 = 50;

/// 2⁵⁰, a key point coordinate's unit as a fraction of the face's side.
const POSITION_SCALE: f64 = 1_125_899_906_842_624.0;

/// The bits of a drawn key point's place within its own cell: 2⁻²⁶ of the cell's side.
const CELL_BITS: u32 = 26;

/// The random bits of each drawn coordinate: the top 24 of its word, so that a coordinate is
/// ε + (1 − 2ε) r ÷ 2²⁴ of the cell, in steps of 2⁻²⁵.
const JITTER_BITS: u32 = 24;

/// ε in units of 2⁻²⁶ of a cell: 2²⁴.
const MARGIN_UNITS: u64 = 1 << 24;

/// A drawn coordinate's step in units of 2⁻²⁶ of a cell, (1 − 2ε) 2²⁶ ÷ 2²⁴: 2.
const JITTER_STEP: u64 = 2;

/// The main stream's length from the divide of a basin draining `area`, by Hack's law in SI,
/// L = C A^h ([`HACK_COEFFICIENT`], [`HACK_EXPONENT`]): 320 km for 10⁴ km².
///
/// # Panics
///
/// If `area` is not finite and non-negative.
#[must_use]
pub fn main_stream_length(area: SquareMetres) -> Metres {
    let a = area.value();
    assert!(
        a.is_finite() && a >= 0.0,
        "a drainage area must be finite and non-negative, got {a} m²"
    );
    Metres::new(HACK_COEFFICIENT * math::powf(a, HACK_EXPONENT))
}

/// The drainage area at the end of a main stream of `length` from the divide, by Hack's law in
/// SI, A = (L ÷ C)^(1 ÷ h), the inverse of [`main_stream_length`].
///
/// # Panics
///
/// If `length` is not finite and non-negative.
#[must_use]
pub fn hack_area(length: Metres) -> SquareMetres {
    let l = length.value();
    assert!(
        l.is_finite() && l >= 0.0,
        "a stream's length must be finite and non-negative, got {l} m"
    );
    SquareMetres::new(math::powf(l / HACK_COEFFICIENT, 1.0 / HACK_EXPONENT))
}

/// The main stream's length above a key point of `level` on a body of mean radius `radius`: what
/// Hack's law gives one mean cell of the level, 4πR² ÷ (6 · 4^level), as if the key point drained
/// one cell at its head (the plan's own reading of where a level's channels begin).
///
/// 41 km at level 9 on an Earth, 3.4 km at level 12, 4.3 m at level 20. From level 15 on an Earth
/// (a mean cell of 7.9 × 10⁴ m²; level 14's, 3.2 × 10⁵ m², is just above it) the head drains less
/// than Hack's smallest basin, 0.31 km² (Hack 1957, p. 45), so the law is extrapolated there (see
/// the module documentation).
///
/// # Panics
///
/// If `radius` is not finite and positive, or `level` is above [`MAX_LEVEL`].
#[must_use]
pub fn head_length(radius: Metres, level: u8) -> Metres {
    let r = radius.value();
    assert!(
        r.is_finite() && r > 0.0,
        "a body's radius must be finite and positive, got {r} m"
    );
    assert!(
        level <= MAX_LEVEL,
        "a level is at most {MAX_LEVEL}, got {level}"
    );
    // 6 · 4^level cells, exact in f64.
    let side = f64::from(1_u32 << level);
    let cells = 6.0 * side * side;
    let area = 4.0 * core::f64::consts::PI * r * r / cells;
    main_stream_length(SquareMetres::new(area))
}

/// The rise of a tributary's bed from its join up to its key point: a tributary of `length` whose
/// coarse cell's steepness index is `steepness_m0_9` (m^0.9), heading a main stream of `head`
/// above its key point ([`head_length`] of its level), rises by ∫ max(`k_s` C^¾ L^−¾,
/// [`MINIMUM_SLOPE`]) dL from L = `head` to `head` + `length` (see the module documentation), in
/// closed form: the per-level relief that R09.T6.b's closed-form incision reads.
///
/// A segment's [`Segment::bed_from`] is its [`Segment::bed_to`] plus this, bit for bit.
///
/// # Panics
///
/// If `steepness_m0_9`, `head` or `length` is not finite and non-negative, or the rise is not
/// finite (a steepness beyond about 10²³⁵ m^0.9, far past the field's codes).
#[must_use]
pub fn tributary_rise(steepness_m0_9: f64, head: Metres, length: Metres) -> Metres {
    let (h, l) = (head.value(), length.value());
    assert!(
        [steepness_m0_9, h, l]
            .iter()
            .all(|x| x.is_finite() && *x >= 0.0),
        "a tributary's steepness, head and length must be finite and non-negative, got \
         {steepness_m0_9} m^0.9, {h} m and {l} m"
    );
    Metres::new(num::assert_finite(rise(steepness_m0_9, h, h + l)))
}

/// C^¾, m^−0.15: Hack's coefficient as the profile's integral reads it, through square roots
/// alone.
#[must_use]
fn hack_coefficient_three_quarters() -> f64 {
    let root = HACK_COEFFICIENT.sqrt();
    root * root.sqrt()
}

/// x^¼, through two correctly rounded square roots.
#[must_use]
fn fourth_root(x: f64) -> f64 {
    x.sqrt().sqrt()
}

/// The main-stream length, metres, beyond which a tributary of steepness index `steepness`
/// (m^0.9) falls at [`MINIMUM_SLOPE`]: where `k_s` C^¾ L^−¾ meets it.
#[must_use]
fn gentle_from(steepness: f64) -> f64 {
    let a = steepness * hack_coefficient_three_quarters();
    math::powf(a / MINIMUM_SLOPE, 4.0 / 3.0)
}

/// The rise of a tributary's bed between the main-stream lengths `from` and `to` (metres,
/// `from` ≤ `to`): ∫ max(`k_s` C^¾ L^−¾, `S_min`) dL, the stream-power part
/// 4 `k_s` C^¾ (L₂^¼ − L₁^¼) up to the length where the slope meets [`MINIMUM_SLOPE`] and the floor
/// beyond it. Exactly zero when `from` equals `to`.
#[must_use]
fn rise(steepness: f64, from: f64, to: f64) -> f64 {
    let a = steepness * hack_coefficient_three_quarters();
    let gentle = gentle_from(steepness);
    let steep = 4.0 * a * (fourth_root(num::min(to, gentle)) - fourth_root(num::min(from, gentle)));
    let flat = MINIMUM_SLOPE * (num::max(to, gentle) - num::max(from, gentle));
    steep + flat
}

/// A tributary's slope at the main-stream length `length` (metres): max(`k_s` C^¾ L^−¾,
/// [`MINIMUM_SLOPE`]), which is `k_s` A^−θ with A from Hack's law wherever the floor does not bind.
/// Unbounded above: the stream-power law holds only below about 0.03 to 0.1 (Stock and Dietrich
/// 2003; see the module documentation), which R09.T6.b's level cut respects.
#[must_use]
fn tributary_slope(steepness: f64, length: f64) -> f64 {
    let a = steepness * hack_coefficient_three_quarters();
    let f = fourth_root(length);
    num::max(a / (f * f * f), MINIMUM_SLOPE)
}

/// The dot product of two vectors.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product a × b.
#[must_use]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The point (1 − t) a + t b, which is a at t = 0 and b at t = 1 exactly (a zero's sign aside).
#[must_use]
fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [0, 1, 2].map(|k| (1.0 - t) * a[k] + t * b[k])
}

/// The distance between two points, the norm of their difference.
#[must_use]
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [0, 1, 2].map(|k| b[k] - a[k]);
    dot(d, d).sqrt()
}

/// The cell of level `level` that holds `cell`, a cell of that level or finer.
#[must_use]
fn ancestor(cell: PatchKey, level: u8) -> PatchKey {
    let up = cell.level() - level;
    PatchKey::new(cell.face(), level, cell.i() >> up, cell.j() >> up)
        .expect("an ancestor of a patch is a patch")
}

/// `cell`'s 3 × 3 graph neighbourhood: the cell, its four edge neighbours in [`Edge::ALL`]'s order
/// and its corner neighbours in [`PatchKey::corner_neighbours`]' order, `None` where a corner is
/// the cube's.
#[must_use]
fn neighbourhood(cell: PatchKey) -> [Option<PatchKey>; 9] {
    let [c0, c1, c2, c3] = cell.corner_neighbours();
    let [e0, e1, e2, e3] = Edge::ALL.map(|edge| Some(cell.edge_neighbour(edge)));
    [Some(cell), e0, e1, e2, e3, c0, c1, c2, c3]
}

/// The record of `cell`, or the error naming it.
fn read<F: FieldView>(field: &F, cell: PatchKey) -> Result<&SynthesisCell, ReadCellError> {
    field.cell(cell).ok_or(ReadCellError { cell })
}

/// Whether a coarse cell carries tributaries: it is dry, its water goes somewhere, and it has a
/// channel steepness above zero.
#[must_use]
fn has_channels(cell: &SynthesisCell) -> bool {
    cell.flow.edge().is_some() && cell.steepness != LogSteepness::ZERO && !cell.is_under_water()
}

/// Where a trunk from the unit direction `from` (in `cell`) to `to` (across `edge`) crosses the
/// edge: the point of the edge's great circle, between its two corners A and B as `cell` gives
/// them, where the straight chord between `from` and `to` meets the circle's plane, held to the
/// edge's central half ([`CROSSING_MARGIN`]); a unit direction.
///
/// The chord meets the plane at m₀; its place along the edge is λ = α ÷ (α + β), with
/// α = (A × m₀) · n and β = (m₀ × B) · n for the plane's normal n = A × B, since a point of the
/// circle between the corners is (1 − λ) A + λ B, radially projected. The crossing is that point at
/// λ held to [ε, 1 − ε].
#[must_use]
fn crossing(cell: PatchKey, edge: Edge, from: [f64; 3], to: [f64; 3]) -> [f64; 3] {
    let ((x0, y0), (x1, y1)) = match edge {
        Edge::UMin => ((0, 0), (0, 64)),
        Edge::UMax => ((64, 0), (64, 64)),
        Edge::VMin => ((0, 0), (64, 0)),
        Edge::VMax => ((0, 64), (64, 64)),
    };
    let (a, b) = (cell.vertex_dir(x0, y0), cell.vertex_dir(x1, y1));
    let normal = cross(a, b);
    let (inside, outside) = (dot(normal, from), dot(normal, to));
    let t = inside / (inside - outside);
    assert!(
        t.is_finite(),
        "a trunk's key points lie on either side of its cell's edge"
    );
    let met = lerp(from, to, t);
    let alpha = dot(cross(a, met), normal);
    let beta = dot(cross(met, b), normal);
    let along = alpha / (alpha + beta);
    assert!(along.is_finite(), "a trunk crosses its cell's edge");
    let held = num::max(CROSSING_MARGIN, num::min(along, 1.0 - CROSSING_MARGIN));
    unit_dir(lerp(a, b, held))
}

/// A cell's key point: a place on one cube face, in units of 2⁻⁵⁰ of the face's side (see the
/// module documentation).
///
/// A drawn key point lies in its cell's central half, never on a face edge, so it has one face, and
/// its direction is computed from that face alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyPoint {
    face: Face,
    s: u64,
    t: u64,
}

impl KeyPoint {
    /// The face it lies on.
    #[must_use]
    pub const fn face(self) -> Face {
        self.face
    }

    /// Its face coordinates (s, t), each in [0, 1] before S2's warp ([`st_to_uv`]), exact.
    #[must_use]
    pub fn st(self) -> (f64, f64) {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a coordinate is below 2^50, exact in f64"
        )]
        let at = |c: u64| c as f64 / POSITION_SCALE;
        (at(self.s), at(self.t))
    }

    /// Its unit direction in the body-fixed frame, from its own face.
    #[must_use]
    pub fn dir(self) -> [f64; 3] {
        let (s, t) = self.st();
        unit_dir(face_uv_to_xyz(FaceUv {
            face: self.face,
            u: st_to_uv(s),
            v: st_to_uv(t),
        }))
    }

    /// Whether it lies in `cell`, exactly.
    #[must_use]
    pub fn is_in(self, cell: PatchKey) -> bool {
        let shift = POSITION_BITS - u32::from(cell.level());
        self.face == cell.face()
            && self.s >> shift == u64::from(cell.i())
            && self.t >> shift == u64::from(cell.j())
    }
}

/// Where a segment's channel goes at its downstream end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outlet {
    /// A trunk's, the first level's: into the coarse cell `receiver` its cell's flow names, at the
    /// receiver's key point.
    ///
    /// It leaves its own cell at the parameter `exit`, in (0, 1), its crossing.
    Cell {
        /// The coarse cell the flow names.
        receiver: PatchKey,
        /// Its crossing's parameter.
        exit: f64,
    },
    /// A tributary's: into the segment headed by `parent`'s key point, at that segment's
    /// parameter `at`.
    Join {
        /// The cell whose key point heads the parent segment, of a coarser level.
        parent: PatchKey,
        /// The parameter along the parent segment, 0 at its key point and 1 at its outlet.
        at: f64,
    },
}

/// What a segment is: where it goes, and how its bed and drainage vary along it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// A trunk: its receiver, its crossing's parameter and spheroid point (metres), and the coarse
    /// cell's drainage area along it, square metres; its bed runs straight between its ends.
    Trunk {
        receiver: PatchKey,
        exit: f64,
        crossing_m: [f64; 3],
        area_m2: f64,
    },
    /// A tributary: the cell heading the segment it joins and the parameter there, its coarse
    /// cell's steepness index (m^0.9), the main stream's length above its key point and its own
    /// length (metres).
    Tributary {
        parent: PatchKey,
        at: f64,
        steepness: f64,
        head_m: f64,
        length_m: f64,
    },
}

/// One segment of the network: the chord from a key point down to where its channel goes (two
/// chords, through its crossing, for a trunk), with its bed's profile along it (see the module
/// documentation).
///
/// A point along it is given by a parameter t, 0 at the key point and 1 at the outlet, in
/// proportion to the length along it. Plain data, read through getters; `Clone`, not `Copy`, at
/// 136 bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    cell: PatchKey,
    from_m: [f64; 3],
    to_m: [f64; 3],
    bed_from_m: f64,
    bed_to_m: f64,
    kind: Kind,
}

impl Segment {
    /// The cell whose key point heads it: a coarse cell for a trunk.
    #[must_use]
    pub const fn cell(&self) -> PatchKey {
        self.cell
    }

    /// Its level, its cell's.
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.cell.level()
    }

    /// Whether it is a trunk, of the first level.
    #[must_use]
    pub const fn is_trunk(&self) -> bool {
        matches!(self.kind, Kind::Trunk { .. })
    }

    /// Its key point's spheroid point, body-fixed, metres.
    #[must_use]
    pub const fn from_m(&self) -> [f64; 3] {
        self.from_m
    }

    /// Its outlet's point, body-fixed, metres: the receiver's key point, or the point it joins.
    #[must_use]
    pub const fn to_m(&self) -> [f64; 3] {
        self.to_m
    }

    /// The point at parameter `t`, body-fixed, metres: (1 − t) from + t to on a tributary, and on
    /// a trunk the same along each of its two chords, the crossing at its exit.
    ///
    /// # Panics
    ///
    /// If `t` is not in [0, 1].
    #[must_use]
    pub fn point_m(&self, t: f64) -> [f64; 3] {
        check_parameter(t);
        match self.kind {
            Kind::Trunk {
                exit, crossing_m, ..
            } => {
                if t <= exit {
                    lerp(self.from_m, crossing_m, t / exit)
                } else {
                    lerp(crossing_m, self.to_m, (t - exit) / (1.0 - exit))
                }
            }
            Kind::Tributary { .. } => lerp(self.from_m, self.to_m, t),
        }
    }

    /// Its crossing's spheroid point, body-fixed, metres, where a trunk leaves its cell; `None` for
    /// a tributary.
    #[must_use]
    pub const fn crossing_m(&self) -> Option<[f64; 3]> {
        match self.kind {
            Kind::Trunk { crossing_m, .. } => Some(crossing_m),
            Kind::Tributary { .. } => None,
        }
    }

    /// Its length along it: the chord's, or a trunk's two chords'.
    #[must_use]
    pub fn length(&self) -> Metres {
        Metres::new(match self.kind {
            Kind::Trunk { crossing_m, .. } => {
                distance(self.from_m, crossing_m) + distance(crossing_m, self.to_m)
            }
            Kind::Tributary { .. } => distance(self.from_m, self.to_m),
        })
    }

    /// Where its channel goes.
    #[must_use]
    pub const fn outlet(&self) -> Outlet {
        match self.kind {
            Kind::Trunk { receiver, exit, .. } => Outlet::Cell { receiver, exit },
            Kind::Tributary { parent, at, .. } => Outlet::Join { parent, at },
        }
    }

    /// The bed's height above the datum at its key point.
    #[must_use]
    pub const fn bed_from(&self) -> Metres {
        Metres::new(self.bed_from_m)
    }

    /// The bed's height above the datum at its outlet: the receiver's water surface for a trunk,
    /// the parent's bed at the join for a tributary.
    #[must_use]
    pub const fn bed_to(&self) -> Metres {
        Metres::new(self.bed_to_m)
    }

    /// The bed's height above the datum at parameter `t`: straight along a trunk, and along a
    /// tributary its outlet's height plus the stream-power rise from there (see the module
    /// documentation), so [`bed_from`](Self::bed_from) at 0 and [`bed_to`](Self::bed_to) at 1
    /// exactly.
    ///
    /// # Panics
    ///
    /// If `t` is not in [0, 1].
    #[must_use]
    pub fn bed_at(&self, t: f64) -> Metres {
        check_parameter(t);
        Metres::new(match self.kind {
            Kind::Trunk { .. } => (1.0 - t) * self.bed_from_m + t * self.bed_to_m,
            Kind::Tributary {
                steepness,
                head_m,
                length_m,
                ..
            } => self.bed_to_m + rise(steepness, head_m + t * length_m, head_m + length_m),
        })
    }

    /// The main stream's length from the divide at parameter `t`: Hack's length of the trunk's
    /// drainage area along a trunk, and the head's length plus the distance along a tributary.
    ///
    /// # Panics
    ///
    /// If `t` is not in [0, 1].
    #[must_use]
    pub fn main_stream_at(&self, t: f64) -> Metres {
        check_parameter(t);
        match self.kind {
            Kind::Trunk { area_m2, .. } => main_stream_length(SquareMetres::new(area_m2)),
            Kind::Tributary {
                head_m, length_m, ..
            } => Metres::new(head_m + t * length_m),
        }
    }

    /// The drainage area at parameter `t`: the coarse cell's along a trunk, and Hack's area of the
    /// main stream's length along a tributary ([`hack_area`]).
    ///
    /// # Panics
    ///
    /// If `t` is not in [0, 1].
    #[must_use]
    pub fn area_at(&self, t: f64) -> SquareMetres {
        check_parameter(t);
        match self.kind {
            Kind::Trunk { area_m2, .. } => SquareMetres::new(area_m2),
            Kind::Tributary { .. } => hack_area(self.main_stream_at(t)),
        }
    }

    /// The bed's slope at parameter `t`, metres a metre, positive downstream: the trunk's straight
    /// fall over its length, or the tributary's max(`k_s` A^−θ, [`MINIMUM_SLOPE`]).
    ///
    /// # Panics
    ///
    /// If `t` is not in [0, 1].
    #[must_use]
    pub fn slope_at(&self, t: f64) -> f64 {
        check_parameter(t);
        match self.kind {
            Kind::Trunk { .. } => (self.bed_from_m - self.bed_to_m) / self.length().value(),
            Kind::Tributary { steepness, .. } => {
                tributary_slope(steepness, self.main_stream_at(t).value())
            }
        }
    }

    /// The steepness index `k_s` its profile reads, m^0.9: its coarse cell's for a tributary, and
    /// `None` for a trunk, whose bed is the coarse field's own.
    #[must_use]
    pub const fn steepness_m0_9(&self) -> Option<f64> {
        match self.kind {
            Kind::Trunk { .. } => None,
            Kind::Tributary { steepness, .. } => Some(steepness),
        }
    }

    /// A trunk's two chords, each with the parameters it spans: from its key point to its crossing
    /// over [0, exit], and from its crossing to its receiver's key point over [exit, 1]; `None` for
    /// a tributary.
    #[must_use]
    fn chords(&self) -> Option<[Chord; 2]> {
        match self.kind {
            Kind::Trunk {
                exit, crossing_m, ..
            } => Some([
                Chord {
                    from: self.from_m,
                    to: crossing_m,
                    span: (0.0, exit),
                },
                Chord {
                    from: crossing_m,
                    to: self.to_m,
                    span: (exit, 1.0),
                },
            ]),
            Kind::Tributary { .. } => None,
        }
    }
}

/// Asserts a segment's parameter is in [0, 1].
fn check_parameter(t: f64) {
    assert!(
        (0.0..=1.0).contains(&t),
        "a segment's parameter is in [0, 1], got {t}"
    );
}

/// A body's channel network, prepared once for its field's header and detail seed: what every
/// [`ChannelNetwork`] over the field reads (see the module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Channels {
    seed: DetailSeed,
    level: u8,
    figure: Spheroid,
    radius: Metres,
}

impl Channels {
    /// The channels of the body of `header`, drawn from its detail seed `seed`.
    #[must_use]
    pub fn new(header: &FieldHeader, seed: DetailSeed) -> Self {
        Self {
            seed,
            level: header.level().get(),
            figure: header.figure(),
            radius: header.radius(),
        }
    }

    /// The body's detail seed.
    #[must_use]
    pub const fn seed(&self) -> DetailSeed {
        self.seed
    }

    /// The network's first level, the field's coarse level L.
    #[must_use]
    pub const fn first_level(&self) -> u8 {
        self.level
    }

    /// The network over `field`, with nothing yet computed.
    ///
    /// # Panics
    ///
    /// If `field`'s header is not of the body these channels were made for: another level, radius
    /// or figure.
    #[must_use]
    pub fn network<'a, F: FieldView>(&'a self, field: &'a F) -> ChannelNetwork<'a, F> {
        let header = field.header();
        assert!(
            header.level().get() == self.level
                && header.radius() == self.radius
                && header.figure() == self.figure,
            "a channel network reads the field whose header made its channels"
        );
        ChannelNetwork {
            channels: self,
            field,
            key_points: BTreeMap::new(),
            segments: BTreeMap::new(),
        }
    }

    /// The key point `cell` draws for itself on `surface.channel`, before inheritance.
    #[must_use]
    fn drawn_key_point(&self, cell: PatchKey) -> KeyPoint {
        let stream = self.seed.stream(
            SURFACE_CHANNEL,
            ObjectKey::surface_cell(cell.face().index(), cell.level(), cell.i(), cell.j(), 0)
                .expect("a patch key is a surface cell"),
        );
        let shift = u32::from(MAX_LEVEL - cell.level());
        let coordinate = |index: u32, word: u64| {
            let jitter = MARGIN_UNITS + JITTER_STEP * (word >> (64 - JITTER_BITS));
            ((u64::from(index) << CELL_BITS) | jitter) << shift
        };
        KeyPoint {
            face: cell.face(),
            s: coordinate(cell.i(), stream.word_at(0)),
            t: coordinate(cell.j(), stream.word_at(1)),
        }
    }
}

/// A straight chord of a segment, spheroid points in metres, and the segment's parameters it spans.
#[derive(Debug, Clone, Copy)]
struct Chord {
    from: [f64; 3],
    to: [f64; 3],
    span: (f64, f64),
}

/// A chord a join may reach: of `segment`, headed by `owner`'s key point.
#[derive(Debug, Clone, Copy)]
struct Part<'s> {
    owner: PatchKey,
    segment: &'s Segment,
    chord: Chord,
}

/// The part `chord` of `segment`, headed by `owner`'s key point.
#[must_use]
const fn part(owner: PatchKey, segment: &Segment, chord: Chord) -> Part<'_> {
    Part {
        owner,
        segment,
        chord,
    }
}

/// The nearest point found so far for a key point's join.
#[derive(Debug, Clone)]
struct Nearest {
    distance2: f64,
    level: u8,
    owner: PatchKey,
    segment: Segment,
    at: f64,
    point: [f64; 3],
}

impl Nearest {
    /// Whether this is nearer than `other`, ties to the coarser level, then the lower cell key,
    /// then the lower parameter along the segment: a total order, so the winner does not depend on
    /// the order the candidates are offered in (both parameters are finite, and never −0).
    #[must_use]
    fn beats(&self, other: &Self) -> bool {
        self.distance2
            .total_cmp(&other.distance2)
            .then_with(|| {
                (self.level, self.owner.to_u64()).cmp(&(other.level, other.owner.to_u64()))
            })
            .then_with(|| self.at.total_cmp(&other.at))
            .is_lt()
    }
}

/// The nearest point to `q` (metres) of `part`'s chord, with its parameter along the whole segment,
/// and the point recomputed from that ([`Segment::point_m`]), so that a join's point is its
/// parent's point at the join's parameter, bit for bit.
#[must_use]
fn nearest_on(q: [f64; 3], part: &Part<'_>, level: u8) -> Nearest {
    let Chord { from, to, span } = part.chord;
    let ab = [0, 1, 2].map(|k| to[k] - from[k]);
    let aq = [0, 1, 2].map(|k| q[k] - from[k]);
    let u = num::max(0.0, num::min(dot(aq, ab) / dot(ab, ab), 1.0));
    let at = span.0 + (span.1 - span.0) * u;
    let point = part.segment.point_m(at);
    let gap = [0, 1, 2].map(|k| q[k] - point[k]);
    Nearest {
        distance2: num::assert_finite(dot(gap, gap)),
        level,
        owner: part.owner,
        segment: part.segment.clone(),
        at,
        point,
    }
}

/// A body's channel network over one field view, computed as it is asked for and memoised by cell
/// (see the module documentation).
///
/// It is the caller's: every answer is a pure function of the field, the seed and the cell, the
/// same whatever was asked before; the memo only saves work. R09.T6.b moves the memo into the
/// caller's [`SynthCache`](super::SynthCache).
pub struct ChannelNetwork<'a, F> {
    channels: &'a Channels,
    field: &'a F,
    key_points: BTreeMap<u64, KeyPoint>,
    segments: BTreeMap<u64, Option<Segment>>,
}

impl<F> std::fmt::Debug for ChannelNetwork<'_, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelNetwork")
            .field("channels", self.channels)
            .field("key_points", &self.key_points.len())
            .field("segments", &self.segments.len())
            .finish_non_exhaustive()
    }
}

impl<F: FieldView> ChannelNetwork<'_, F> {
    /// The key point of `cell`, a cell of the first level or finer: its parent's if that lies in
    /// it, or its own drawn one (see the module documentation).
    ///
    /// # Panics
    ///
    /// If `cell` is of a level coarser than the first.
    #[must_use]
    pub fn key_point(&mut self, cell: PatchKey) -> KeyPoint {
        let first = self.channels.level;
        assert!(
            cell.level() >= first,
            "a key point is of level {first} or finer, not {}",
            cell.level()
        );
        let key = cell.to_u64();
        if let Some(&point) = self.key_points.get(&key) {
            return point;
        }
        let point = match cell.parent() {
            Some(parent) if cell.level() > first => {
                let above = self.key_point(parent);
                if above.is_in(cell) {
                    above
                } else {
                    self.channels.drawn_key_point(cell)
                }
            }
            Some(_) | None => self.channels.drawn_key_point(cell),
        };
        self.key_points.insert(key, point);
        point
    }

    /// Whether `cell`'s key point is its parent's: a cell finer than the first level that holds its
    /// parent's key point.
    ///
    /// # Panics
    ///
    /// If `cell` is of a level coarser than the first.
    #[must_use]
    pub fn is_inherited(&mut self, cell: PatchKey) -> bool {
        let first = self.channels.level;
        assert!(
            cell.level() >= first,
            "a key point is of level {first} or finer, not {}",
            cell.level()
        );
        match cell.parent() {
            Some(parent) if cell.level() > first => self.key_point(parent).is_in(cell),
            Some(_) | None => false,
        }
    }

    /// The segment `cell`'s key point heads, or `None` where it heads none: a coarse cell's trunk
    /// to its receiver, `None` where its flow is terminal; a finer cell's tributary to its join,
    /// `None` where the key point is inherited, lies on the segment it would join, or lies in a
    /// coarse cell without channels (see the module documentation).
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] naming the first cell read that the view does not hold, in a fixed order:
    /// the coarse cell, its receiver, then for a tributary each candidate's cells, level by level
    /// and in each level's 3 × 3 order.
    ///
    /// # Panics
    ///
    /// If `cell` is of a level coarser than the first, or a height is not finite (a bug).
    pub fn segment(&mut self, cell: PatchKey) -> Result<Option<Segment>, ReadCellError> {
        let first = self.channels.level;
        assert!(
            cell.level() >= first,
            "a segment is of level {first} or finer, not {}",
            cell.level()
        );
        let key = cell.to_u64();
        if let Some(segment) = self.segments.get(&key) {
            return Ok(segment.clone());
        }
        let segment = if cell.level() == first {
            self.trunk(cell)?
        } else {
            self.tributary(cell)?
        };
        self.segments.insert(key, segment.clone());
        Ok(segment)
    }

    /// The trunk of coarse cell `coarse`, from its key point to its receiver's.
    fn trunk(&mut self, coarse: PatchKey) -> Result<Option<Segment>, ReadCellError> {
        let own = read(self.field, coarse)?;
        let Some(edge) = own.flow.edge() else {
            return Ok(None);
        };
        let receiver = coarse.edge_neighbour(edge);
        let into = read(self.field, receiver)?;
        let from = self.key_point(coarse).dir();
        let to = self.key_point(receiver).dir();
        let figure = self.channels.figure;
        let (from_m, to_m) = (figure.point(from), figure.point(to));
        let crossing_m = figure.point(crossing(coarse, edge, from, to));
        let before = distance(from_m, crossing_m);
        let exit = before / (before + distance(crossing_m, to_m));
        Ok(Some(Segment {
            cell: coarse,
            from_m,
            to_m,
            bed_from_m: num::assert_finite(own.water_surface().value()),
            bed_to_m: num::assert_finite(into.water_surface().value()),
            kind: Kind::Trunk {
                receiver,
                exit: num::assert_finite(exit),
                crossing_m,
                area_m2: own.drainage.area().value(),
            },
        }))
    }

    /// The tributary of `cell`, finer than the first level.
    fn tributary(&mut self, cell: PatchKey) -> Result<Option<Segment>, ReadCellError> {
        let first = self.channels.level;
        let coarse = ancestor(cell, first);
        let own = *read(self.field, coarse)?;
        if !has_channels(&own) || self.is_inherited(cell) {
            return Ok(None);
        }
        let figure = self.channels.figure;
        let q = figure.point(self.key_point(cell).dir());
        let mut best: Option<Nearest> = None;
        let mut offer = |candidate: Nearest| {
            if best.as_ref().is_none_or(|b| candidate.beats(b)) {
                best = Some(candidate);
            }
        };
        for level in first..cell.level() {
            for around in neighbourhood(ancestor(cell, level)).into_iter().flatten() {
                let Some(segment) = self.segment(around)? else {
                    continue;
                };
                if let Some(chords) = segment.chords() {
                    // A trunk's two chords, the first offered first.
                    for chord in chords {
                        offer(nearest_on(q, &part(around, &segment, chord), level));
                    }
                } else {
                    let chord = Chord {
                        from: segment.from_m,
                        to: segment.to_m,
                        span: (0.0, 1.0),
                    };
                    offer(nearest_on(q, &part(around, &segment, chord), level));
                }
            }
        }
        let best = best
            .expect("a coarse cell with channels has a trunk, so its key points find a segment");
        if best.distance2 <= 0.0 {
            return Ok(None);
        }
        let length_m = distance(q, best.point);
        let mut segment = Segment {
            cell,
            from_m: q,
            to_m: best.point,
            bed_from_m: 0.0,
            bed_to_m: best.segment.bed_at(best.at).value(),
            kind: Kind::Tributary {
                parent: best.owner,
                at: best.at,
                steepness: own.steepness.index_m0_9(),
                head_m: head_length(self.channels.radius, cell.level()).value(),
                length_m,
            },
        };
        segment.bed_from_m = num::assert_finite(segment.bed_at(0.0).value());
        Ok(Some(segment))
    }
}

#[cfg(test)]
mod tests;
