//! The channels' incision: the valleys the network cuts, level by level, each level's mean over
//! its parent cell removed (plan R09, T6.b; Design notes 13 and 14).
//!
//! # The valleys
//!
//! Every segment the network evaluates cuts a **groove** about its chords: a valley whose floor is
//! its channel, half its bankfull width either side of the axis, and whose walls rise from the
//! floor's edge to the surface over a wall width W, with the cross-profile 1 − S(x), S the cubic
//! smoothstep 3x² − 2x³ of x = (distance − half the width) ÷ W, so that the valley meets its floor
//! and the ground beyond it with no kink. W is [`VALLEY_WALL_SHARE`] of the level's mean cell, so
//! that a level's valleys, one a cell, fill its cells as Dendry's levels do. The groove's depth at
//! its axis is G = min(max(β, d), `G_max`), with d the channel's bankfull depth, `G_max` the depth
//! whose steepest wall, 1.5 G ÷ W, is [`VALLEY_WALL_GRADIENT`], and β how far the channel's bed lies
//! below its trunk's **valley top**:
//!
//! - A trunk's valley is as deep as the relief its cell's tributaries cut, from the channel heads
//!   down to it ([`valley_depth`]), but no deeper than [`VALLEY_RELIEF_SHARE`] of what its cell
//!   stands above its **base level**: its receiver's water where the receiver is under water, its
//!   ground where it is dry and terminal and above the sea, and the sea level otherwise (R09.T6.f,
//!   R09.T6.g;
//!   decision-r09-channel-term §4(b) and §9); its top runs that far above its bed all along it.
//! - A tributary inherits the top where it joins, so its bed rises towards the top upstream, and its
//!   groove shallows to its channel alone where it has risen to it.
//! - On a world dry now a trunk's valley is its epoch's ([`EpochValley`]; R09.T6.c): no deeper than
//!   the epoch's incision at its cell and with no floor at the channel's depth, its wall width W
//!   [`WALL_RUN_PER_DEPTH`] times its depth, at most the level's, so that it is ten times as wide
//!   as deep (each wall 5:1); and no
//!   tributary is drawn below it.
//!
//! So the network's relief below a coarse cell is the stream-power relief of its own channels, from
//! the coarse cell's `k_s` and runoffs and Hack's law (Design note 13), and a tributary's valley
//! is a notch in its parent's wall that deepens away from the join. The grooves of every level up
//! to k combine by their maximum, the deepest at each point, into the envelope `E_k` ≥ 0, so that
//! a groove inside a deeper one changes nothing and two valleys meet at a confluence without a pit;
//! level k's incision is −(`E_k` − `E_{k−1}`), what it cuts below every coarser level, and incision
//! only cuts down.
//!
//! Distances are 3D, from the spheroid point to a segment's chords. A trunk's chord sags below the
//! surface by up to L² ÷ 8R, about 25 m over a 36 km cell, which is accepted (R09.T6.a's note): it
//! is nothing against the wall's kilometres, and narrows only a floor narrower than the sag.
//!
//! # Water
//!
//! A coarse cell under water ([`SynthesisCell::is_under_water`], whatever its liquid) carries no
//! incision at any level, neither a groove nor a correction, and cuts no valley of its own, not
//! even its trunk's past its crossing into a dry receiver (R09.T6.f; decision-r09-channel-term
//! §4(a)). Rivers are "highly depositional near their mouths" (Lamb, Nittrouer, Mohrig and Shaw
//! 2012, JGR Earth Surface 117, F01002, doi:10.1029/2011JF002079, abstract), so a sea's or a lake's
//! bed is not dissected by the rivers that reach it now, and the level's mean removal does not
//! lift it above its water. Shelf valleys that a lower sea once exposed, and submarine canyons, are
//! other processes, not modelled (the field carries no history of the sea level).
//!
//! The fade is a device for continuity, not a law: each level's raw incision in a dry cell is
//! faded towards every edge and corner the cell shares with a cell under water, times the product
//! of S(d ÷ w) over them, S the cubic smoothstep, d the distance to the edge or corner in the
//! cell's face coordinates, and w the level's wall width as a share of the coarse cell's side,
//! ½ ÷ 2^(k − L). So a trunk running into water cuts nothing past its crossing, and its valley
//! shallows over the last half cell to the shore. Each level's correction is zero at every corner
//! a cell under water shares, so that it is zero along every shore and over every wet cell, and
//! the dry cell's bubble carries the rest of its mean. A dry cell with no wet neighbour is not
//! faded and keeps every corner, though its values still move with its neighbours' means and
//! grooves.
//!
//! # Each level's mean
//!
//! Level k's incision has its mean over its parent cell removed (Design note 13), the first level's
//! over its coarse cell, whose mean elevation the coarse pass gave: the mean is the incision's area
//! mean over a [`MEAN_SAMPLES`] × [`MEAN_SAMPLES`] grid at the centres of the cell's equal steps in
//! face coordinates, each weighted by the solid angle it stands for. A constant per parent cell
//! would leave a step at every cell's edge, so what is removed is a correction with exactly the
//! cell's mean that is continuous across cells: bilinear in the cell's face coordinates between its
//! four corners, each the mean of the cells that share it (three at a cube corner, four elsewhere,
//! summed in packed-key order), plus a bubble 256 x²(1 − x)² y²(1 − y)², zero with its gradient on
//! every edge, scaled so that the correction's mean on the same grid is the cell's mean. The level's
//! mean over every parent cell on that grid is then zero to rounding; on a finer grid it is zero to
//! the grid's quadrature error, up to 5.6% of the level's RMS over a parent cell and 2.2% over a
//! coarse cell, which the parents tile ([`MEAN_SAMPLES`]). The correction is C⁰ across cells, its
//! gradient changing at an edge by the difference of its neighbours' corner slopes.
//!
//! # The cut
//!
//! Level k > L is evaluated below a coarse cell only where the cell carries tributaries (never on a
//! world dry now's, whose epoch's level cut is L; [`EpochValley`]) and the
//! bankfull width of a channel draining one mean cell of the level at the cell's runoff is at least
//! [`CUT_WIDTH`], and only to the channel heads, a mean cell of at least [`channel_head_area`]
//! ([`deepest_level`]); a segment of a level its coarse cell cuts cuts no groove, and the trunks,
//! the first level, are always evaluated. The cut is a function of the coarse cell alone, so each
//! level's set is fixed per cell, the same at every level of detail.
//!
//! # The search
//!
//! Level k's grooves at a point are found among the segments headed within two king moves of the
//! point's cell at that level, each with its bounding box, the box of its chords grown by its floor
//! and wall, cached with it: a cell's list holds the grooves whose box can reach the cell's
//! bounding ball, and a query tests its own point against each box before it measures. A groove is
//! zero outside its box, so the pruning changes no bit. The network's joins search the 3 × 3
//! (Design note 13), but a groove reaches half a cell beyond its segment and a trunk ends at its
//! receiver's key point, so from a 3 × 3 a groove would be cut off at the neighbourhood's edge
//! wherever its segment reaches more than three quarters of a cell towards it. From the 5 × 5 that
//! happens only to a segment more than 1.75 cells long (the plan's Risks record how often).
//!
//! # What it reads, and the cost
//!
//! A query reads the coarse cells within two king moves of its own for the cut, each level's grooves (the network's segments
//! within two king moves, each with its own reads), and the means of its parent cell and the eight
//! about it at each level, each a grid of queries of its own. The caller's [`SynthCache`] keeps all of it: the
//! network's memo, each cell's grooves, and each parent cell's mean and correction, by packed
//! cell key and level, so a patch builds each cell once.
//!
//! The gradient is analytic: the deepest groove's, through its nearest point and its depth along
//! the segment, as a function of the spheroid point, and the correction's, through the face
//! coordinates as R09.T4's base elevation's is, both returned as the base elevation returns its own.

use std::collections::BTreeMap;
use std::sync::Arc;

use hyperion_base::math;
use hyperion_base::units::{Metres, MetresPerSecondSquared, SquareMetres};

use super::bend::Bend;
use super::{
    Channels, Chord, Kind, NetworkId, NetworkMemo, RiseTo, Segment, ancestor, channel_size,
    head_length, main_stream_length, neighbourhood, read, rise, tributary_steepness_of,
};
use crate::cube::{
    Edge, FaceUv, MAX_LEVEL, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir, uv_on_face, uv_to_st,
};
use crate::field::{FieldHeader, FieldView, SynthesisCell};
use crate::geometry::BAND_LIMIT_M;
use crate::height::HeightSample;
use crate::num;
use crate::synth::SynthCache;
use crate::synth::interp::{ReadCellError, axes, warp_slope};
use crate::synth::relief::as_function_of_direction;

/// The network's least channel-head drainage area, 10⁴ m²: a design floor (`decision-r09-t5.md`,
/// adjacent finding C), since a channel finer than its head over-dissects hillslopes that are
/// smooth on real land, where the land's spectral rollover at 180–250 m is the first-order valley
/// spacing (Perron, Kirchner and Dietrich 2008, JGR 113, F04003, ¶36). Field-mapped heads drain
/// 10³ to 10⁶ m² (Clubb et al. 2014, WRR 50, 4283, doi:10.1002/2013WR015167, §5.3; site means 413
/// to 20,816 m², §4.2). On steep ground the head is where the stream-power slope reaches
/// [`DEBRIS_FLOW_SLOPE`] ([`channel_head_area`]).
pub const CHANNEL_HEAD_AREA: SquareMetres = SquareMetres::new(1e4);

/// The steepest bed the stream-power profile holds for, metres a metre: 0.1, the upper end of the
/// range above which debris flows, not rivers, cut valleys: fluvial incision "rarely extends to
/// slopes greater than ∼0.03 to 0.10" (Stock and Dietrich 2003, WRR 39(4), 1089,
/// doi:10.1029/2001WR001057, their abstract), so that the network ends only where it is surely no
/// longer fluvial.
pub const DEBRIS_FLOW_SLOPE: f64 = 0.1;

/// The drainage area at the channel heads of a coarse cell whose tributaries' steepness index is
/// `steepness_m0_9` (m^0.9): [`CHANNEL_HEAD_AREA`], or where the stream-power slope `k_s` A^−θ
/// falls to [`DEBRIS_FLOW_SLOPE`] if that is larger, A = (`k_s` ÷ S)^(1 ÷ θ): 4.7 × 10⁴ m² at
/// `k_s` 12.7 m^0.9, the synthetic Earth's median, and 10⁸ m² at 405.
///
/// # Panics
///
/// If `steepness_m0_9` is not finite and non-negative.
#[must_use]
pub fn channel_head_area(steepness_m0_9: f64) -> SquareMetres {
    assert!(
        steepness_m0_9.is_finite() && steepness_m0_9 >= 0.0,
        "a steepness index must be finite and non-negative, got {steepness_m0_9} m^0.9"
    );
    let steep = math::powf(
        steepness_m0_9 / DEBRIS_FLOW_SLOPE,
        1.0 / crate::field::LogSteepness::THETA,
    );
    SquareMetres::new(num::max(CHANNEL_HEAD_AREA.value(), steep))
}

/// A valley's wall width, as a share of its level's mean cell: 0.5, so that the valleys of a level,
/// one a cell, meet between their channels (a design choice, after Dendry's levels, whose valleys
/// fill their cells).
pub const VALLEY_WALL_SHARE: f64 = 0.5;

/// The steepest gradient a valley's wall may have, metres a metre: 0.6, a design cap at the low
/// end of measured critical hillslope gradients. Godard et al. 2020 (Earth Surf. Dynam. 8, 221,
/// `doi:10.5194/esurf-8-221-2020`, §4.1) take a critical gradient of 0.6 m m⁻¹, "close to the
/// natural angle of repose in many granular materials (∼ 30°)"; Grieve, Mudd, Hurst and
/// Milodowski 2016 (Earth Surf. Dynam. 4, 309, `doi:10.5194/esurf-4-309-2016`) use 0.57 at Coweeta
/// (Fig. 6, lower than any published before) and fit 0.83 ± 0.01 in the Oregon Coast Range
/// (Table 1; 0.79 in Grieve, Mudd and Hurst 2016, Earth Surf. Process. Landforms 41, 1039,
/// `doi:10.1002/esp.3884`). Low confidence as a valley-wall figure. It is a design cap, not the critical gradient itself (Roering, Kirchner
/// and Dietrich 1999, Water Resour. Res. 35, 853, `doi:10.1029/1998WR900090`, Table 1: `S_c` = 1.25
/// in basin MR1 of the Oregon Coast Range, 1.27 ± 0.16 over five basins): under R09.T6.i a world
/// wet now's walls are as steep as it allows, 2.5 G across.
pub const VALLEY_WALL_GRADIENT: f64 = 0.6;

/// The narrowest channel a level is evaluated for: two band limits, 4 m, below which the band
/// limit cannot resolve a channel (Design note 13).
pub const CUT_WIDTH: Metres = Metres::new(2.0 * BAND_LIMIT_M);

/// Each wall of a valley of a short wet epoch runs this far across per metre of the valley's
/// depth: 5, the walls' mean gradient of 0.2 inverted (R09.T6.c; decision-r09-t14-mars-ceiling §5).
///
/// A valley of depth G on its bankfull floor `w_bf` is `w_bf` + 10G wide at its top, each wall
/// running 5G as the groove's 1 − smoothstep, so that its cross-section's mean width is `w_bf` + 5G,
/// its void a unit length G (`w_bf` + 5G) and its steepest wall 1.5 G ÷ 5G = 0.3, under
/// [`VALLEY_WALL_GRADIENT`]. The width is "about ten times the depth" (Luo, Cang and Howard 2017,
/// Nat. Commun. 8, 15766, doi:10.1038/ncomms15766, Methods, after Williams and Phillips 2001, JGR
/// 106(E10), 23737, doi:10.1029/2000JE001409, whose top width is linear in depth below 125 m and
/// whose width-to-depth ratio falls as the depth grows, which the floor term gives; their walls'
/// steepening with depth is not modelled), medium-low confidence; the 5:1 walls are Mars's rock,
/// and an ice or methane epoch is an extrapolation (labelled). The coarse pass's width closure on a
/// world dry now lowers each cell by the same cross-section's void ([`EpochValley`]).
pub const WALL_RUN_PER_DEPTH: f64 = 5.0;

/// The samples a side of the grid on which a level's mean over a parent cell is taken: 8, 64 in
/// all, four a side of each of the level's own cells. A design choice: on an independent 24 × 24
/// grid, a level's corrected incision has a mean up to 5.6% of its RMS over a parent cell, and on
/// a 48 × 48 up to 2.2% over a coarse cell, which the parents tile (R09.T6.b's measurement on the
/// Earth- and Mars-like worlds; `the_mean_holds_on_a_finer_grid_to_its_quadrature_error`).
pub const MEAN_SAMPLES: u32 = 8;

/// The wall width's share at which a smoothstep wall is steepest: its gradient is at most 1.5 G ÷ W.
const STEEPEST_WALL: f64 = 1.5;

/// The mean cell width of `level` on a body of mean radius `radius_m`, metres: √(4πR² ÷ (6 · 4^level)),
/// Design note 4's measure.
#[must_use]
pub(crate) fn mean_cell_width(radius_m: f64, level: u8) -> f64 {
    let side = f64::from(1_u32 << level);
    radius_m * (4.0 * core::f64::consts::PI / 6.0).sqrt() / side
}

/// The mean cell area of `level` on a body of mean radius `radius_m`, square metres:
/// 4πR² ÷ (6 · 4^level).
#[must_use]
fn mean_cell_area(radius_m: f64, level: u8) -> f64 {
    let side = f64::from(1_u32 << level);
    4.0 * core::f64::consts::PI * radius_m * radius_m / (6.0 * side * side)
}

/// The deepest a groove of `level` may be on a body of mean radius `radius_m`, metres: `G_max`,
/// the depth whose steepest wall, 1.5 G ÷ W, is [`VALLEY_WALL_GRADIENT`], W being the level's
/// wall width, [`VALLEY_WALL_SHARE`] of its mean cell. Every groove of the level is capped at it,
/// a trunk's and a tributary's alike ([`Groove`]'s depth), so the level's raw incision lies in
/// [−`G_max`, 0] whatever the cell: the level bound's channel term (R09.T8).
#[must_use]
pub(crate) fn groove_cap_m(radius_m: f64, level: u8) -> f64 {
    let wall_m = VALLEY_WALL_SHARE * mean_cell_width(radius_m, level);
    VALLEY_WALL_GRADIENT * wall_m / STEEPEST_WALL
}

/// The deepest level the network can evaluate below any coarse cell of a body of mean radius
/// `radius_m` whose coarse level is `level`: the deepest whose mean cell is at least
/// [`CHANNEL_HEAD_AREA`], which [`deepest_level`] never passes whatever the cell's steepness and
/// runoff, and never above the coarse level itself, which every network has.
#[must_use]
pub(crate) fn deepest_possible_level(radius_m: f64, level: u8) -> u8 {
    let mut deepest = level;
    for k in level + 1..=MAX_LEVEL {
        if mean_cell_area(radius_m, k) < CHANNEL_HEAD_AREA.value() {
            break;
        }
        deepest = k;
    }
    deepest
}

/// A floor under the solid-angle-weighted mean of the bubble 256 x²(1 − x)² y²(1 − y)² over a
/// parent cell's mean grid ([`mean_grid`]) at every level a parent cell may have, the coarse
/// levels 5 and deeper: 0.284. On the 8 × 8 grid alone the mean is 0.284 566; the weights, the
/// warp's rates and the gnomonic face's solid angle, keep every cell's from level 5 down within
/// 0.284 565 to 0.284 713 (R09.T8's one-off sweep of every cell to level 8, and of sampled cells
/// to 11; `the_bubble_s_weighted_mean_stays_above_its_floor` holds every cell at levels 5 and 6
/// and sampled cells to 24). A level's correction scales its bubble by
/// the cell's mean less the bilinear part's, divided by this mean ([`Correction`]), so the floor
/// bounds the correction, and with it the level bound's channel term (R09.T8).
pub(crate) const BUBBLE_MEAN_FLOOR: f64 = 0.284;

/// The depth of a trunk's valley, metres: the relief a coarse cell's tributaries of steepness index
/// `steepness_m0_9` (m^0.9, [`super::tributary_steepness_m0_9`]) cut between its channel heads and its
/// trunk, on a body of mean radius `radius` whose coarse cells are of `level`.
///
/// It is the stream-power bed's rise ([`super::tributary_rise`]'s closed form, with the minimum
/// slope) from the main stream of a channel head, Hack's length of [`channel_head_area`], to that of
/// one mean coarse cell ([`head_length`]): the relief of a basin the size of the cell from its
/// outlet to its heads, which the network's tributaries share out level by level. Zero where the
/// cell is smaller than a channel head's drainage.
///
/// # Panics
///
/// If `steepness_m0_9` is not finite and non-negative, `radius` is not finite and positive, or
/// `level` is above the deepest.
#[must_use]
pub fn valley_depth(steepness_m0_9: f64, radius: Metres, level: u8) -> Metres {
    let heads = main_stream_length(channel_head_area(steepness_m0_9)).value();
    let cell = head_length(radius, level).value();
    if cell <= heads {
        return Metres::ZERO;
    }
    Metres::new(num::assert_finite(rise(steepness_m0_9, heads, cell)))
}

/// The share of its cell's height above its base level that a steady-state trunk's valley may
/// take: 0.5 (R09.T6.g; decision-r09-channel-term §9.2).
///
/// Earth's relief in windows of about 37 km, the coarse cell's size, is a share of the window's
/// height above the sea: Earth2014's 5′ surface over ice-free land (Hirt and Rexer 2015, Int. J.
/// Appl. Earth Obs. Geoinf. 39, 103, doi:10.1016/j.jag.2015.03.001) gives relief ÷ mean height a
/// median of 0.38, and its 5 × 5 box means keep 0.46–0.64 of four canyons' real depths, so the real
/// share is about 0.4 to 0.8 (the ruling's census). The stream-power relief from the channel heads
/// at the coarse field's own steepness index ([`valley_depth`]) is about 2.3 times its cell's
/// height above its base level at the median on the reference Earth: that index, about 70 m^0.9
/// at every drainage from 10³ to 10⁶ km², is the coarse grid's steady state, an active mountain
/// range's u ÷ K at K⊕ (0.44 mm a⁻¹), not the sub-cell channels'. The share is Earth's, labelled on
/// every other world.
pub const VALLEY_RELIEF_SHARE: f64 = 0.5;

/// The height above the datum, metres, below which a trunk draining into `receiver` cuts no valley:
/// its **base level** (R09.T6.f, R09.T6.g; decision-r09-channel-term §4(b) and §9.4).
///
/// It is the receiver's water surface where the receiver is under water, a lake's or the sea's;
/// its ground where it is dry and terminal, a playa's pit, or the sea level if that is higher; and
/// otherwise the sea level, `sea_level`: "the level of the sea [is] a grand base level, below which
/// the dry lands cannot be eroded", and a lake's is a local one (Powell 1875, Exploration of the
/// Colorado River of the West and its Tributaries, Washington: GPO, p. 203). A dry basin below the
/// sea is held to the sea, as `Synthesiser::height_at`'s floor holds it (labelled: no dry point is
/// cut below the sea, R09.T6.f's P2). A basin that drains through dry cells to a lake or a playa
/// above the sea is held to the sea, a weaker hold than its own outlet's (labelled). On a world
/// with no sea the header's sea level is its lowest cell's ground, so every basin is held to the
/// world's lowest ground, a weak but harmless bound.
#[must_use]
pub(crate) fn base_level_m(receiver: &SynthesisCell, sea_level: Metres) -> f64 {
    if receiver.is_under_water() {
        receiver.water_surface().value()
    } else if receiver.flow.edge().is_none() {
        num::max(receiver.water_surface().value(), sea_level.value())
    } else {
        sea_level.value()
    }
}

/// The valley depth of the trunk of the coarse cell `cell`, whose flow names `receiver`, on a
/// world that is not dry now, metres (R09.T6.f; decision-r09-channel-term §4(b)).
///
/// It is [`valley_depth`] at the cell's tributaries' steepness, zero where the cell carries none,
/// held to [`VALLEY_RELIEF_SHARE`] of the height the cell's ground stands above its base level
/// ([`base_level_m`]; R09.T6.g), and zero where it stands at or below it. That holds the valley at
/// the cell's mean ground, not along its whole length: the fade towards water and
/// `Synthesiser::height_at`'s floor keep the cut above the sea and a playa's pit (trunks into lakes
/// above the sea excepted). The trunk's segment carries it ([`Segment::below_top`]), so that its
/// groove, the synthesis's closed form and its tributaries, through the valley top they inherit,
/// all read the same depth. Where the hold does not bind it is [`valley_depth`] to the bit. A world
/// dry now's trunks are its epoch's ([`EpochValley`]) and are not held.
#[must_use]
pub(crate) fn trunk_valley_m(
    cell: &SynthesisCell,
    receiver: &SynthesisCell,
    sea_level: Metres,
    body: ChannelBody,
) -> f64 {
    let valley = tributary_steepness_of(cell)
        .map_or(0.0, |k| valley_depth(k, body.radius, body.level).value());
    let room = cell.water_surface().value() - base_level_m(receiver, sea_level);
    num::min(valley, VALLEY_RELIEF_SHARE * num::max(room, 0.0))
}

/// A steady-state trunk's groove depth at its axis, G = min(max(`valley_m`, `bankfull_m`),
/// `cap_m`), metres.
///
/// It is at least its bankfull channel and at most its level's cap ([`groove_cap_m`]): the one
/// form its groove, its closed form and a world dry now's steady-state bound ([`EpochValley`]) all
/// take.
#[must_use]
fn trunk_depth_m(valley_m: f64, bankfull_m: f64, cap_m: f64) -> f64 {
    num::min(num::max(valley_m, bankfull_m), cap_m)
}

/// A trunk's valley on a world dry now: the valley of a short wet epoch, whose cross-section the
/// synthesis cuts and whose void the coarse pass's width closure takes from the cell (R09.T6.c;
/// decision-r09-t14-choices F2; decision-r09-t14-mars-ceiling §2.2 and §5).
///
/// Its depth G is the lesser of R09.T6.b's steady-state groove, min(max(`valley`, `d_bf`), `G_max`),
/// and the epoch's incision at the cell, with no floor at the channel's bankfull depth, so that a
/// valley the epoch cut less deep than its channel is as shallow as its incision; its floor is the
/// bankfull channel, half its width either side of the axis (T6.b's); and each wall rises over
/// W = min([`WALL_RUN_PER_DEPTH`] G, T6.b's wall) as T6.b's 1 − smoothstep. The cap at T6.b's wall,
/// half the coarse cell's mean width, keeps every groove within the 5 × 5 search's reach, and binds
/// only where G exceeds a tenth of the cell (3.8 km on the reference Mars); its steepest wall is
/// then 1.5 G ÷ W ≤ [`VALLEY_WALL_GRADIENT`], since G ≤ `G_max`.
///
/// A dry-now cell carries this trunk alone: the epoch's level cut is the coarse level, so its
/// network draws no sub-cell tributaries. The ruled tributary incision, the cell's at the same K t
/// in the short-time regime, (`r_t` `A_t`)^m `S_t` ÷ (r̄ A)^m S, is identically 1 on T6.b's
/// tributary profiles, `S_t` = `k_s` (r̄ ÷ `r_t`)^θ `A_t`^−θ with m = θ, except where the minimum
/// slope binds, so it cuts no level; and the tributaries T6.b's inherited valley top keeps below
/// the top, the epoch's knickpoints' reach, would give the reference Mars 0.177 km⁻¹ of valleys
/// (15 times Hynek, Beach and Hoke 2010's 0.0115 km⁻¹ for Noachian units), 6.14 m of global layer
/// and a ceiling at 3.3 times its bound (main's ruling, option A; the plan's "Deviations in T6.c,
/// as built"). A channel-initiation area that is sourced and independent of the grid is a later
/// item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EpochValley {
    depth: Metres,
    floor_width: Metres,
    wall: Metres,
    /// T6.b's wall, half the coarse cell's mean width, at which a wall stops widening.
    wall_cap: Metres,
}

impl EpochValley {
    /// The valley of a trunk whose steady-state valley depth is `valley` ([`valley_depth`], zero
    /// where the cell has no channels), whose bankfull channel is `bankfull_width` wide and
    /// `bankfull_depth` deep, cut by the epoch's `incision`, on a coarse cell of `body`.
    ///
    /// # Panics
    ///
    /// If an input is negative or not finite, or `body` is not a body.
    #[must_use]
    pub fn of(
        valley: Metres,
        bankfull_width: Metres,
        bankfull_depth: Metres,
        incision: Metres,
        body: ChannelBody,
    ) -> Self {
        body.check();
        let [valley, width, depth, incision] = [valley, bankfull_width, bankfull_depth, incision]
            .map(|m| {
                let v = m.value();
                assert!(
                    v.is_finite() && v >= 0.0,
                    "a valley's depths and widths must be finite and non-negative, got {v} m"
                );
                v
            });
        let r = body.radius.value();
        let steady = trunk_depth_m(valley, depth, groove_cap_m(r, body.level));
        let g = num::min(steady, incision);
        let cap = VALLEY_WALL_SHARE * mean_cell_width(r, body.level);
        let wall = num::min(WALL_RUN_PER_DEPTH * g, cap);
        Self {
            depth: Metres::new(g),
            floor_width: Metres::new(width),
            wall: Metres::new(wall),
            wall_cap: Metres::new(cap),
        }
    }

    /// The valley's depth below the upland, G.
    #[must_use]
    pub const fn depth(&self) -> Metres {
        self.depth
    }

    /// The valley's floor's width, its bankfull channel's, `w_bf`.
    #[must_use]
    pub const fn floor_width(&self) -> Metres {
        self.floor_width
    }

    /// Each wall's width, W.
    #[must_use]
    pub const fn wall(&self) -> Metres {
        self.wall
    }

    /// Its void a unit length of valley, G (`w_bf` + W), square metres: the floor's G `w_bf` and
    /// the two smoothstep walls' G W ÷ 2 each.
    #[must_use]
    pub fn void_m2(&self) -> f64 {
        self.depth.value() * (self.floor_width.value() + self.wall.value())
    }

    /// Its mean void a unit length over its trunk's second chord, square metres, where its depth
    /// grades by a smoothstep from its own G to its receiver's valley depth `receiver` (R09.T6.h;
    /// decision-r09-t14-mars-ceiling §5 as amended, main's ruling 1 of 2026-10-30), its floor its
    /// own bankfull channel and each wall 5 G at every point.
    ///
    /// Below the wall's cap it is the closed form `w_bf` (G + Δ ÷ 2) + 5 (G² + G Δ + 13Δ² ÷ 35),
    /// Δ the receiver's depth less its own, from ∫₀¹ S = ½ and ∫₀¹ S² = 13 ÷ 35 of the smoothstep
    /// S; where a wall reaches the cap, the mean of G (`w_bf` + W) over [`GRADED_SAMPLES`] points
    /// along the chord, in order.
    ///
    /// # Panics
    ///
    /// If `receiver` is negative or not finite.
    #[must_use]
    pub fn graded_void_m2(&self, receiver: Metres) -> f64 {
        let to = receiver.value();
        assert!(
            to.is_finite() && to >= 0.0,
            "a receiver's valley depth must be finite and non-negative, got {to} m"
        );
        let (from, floor) = (self.depth.value(), self.floor_width.value());
        graded_mean(
            from,
            to,
            self.wall_cap.value(),
            |g, wall| g * (floor + wall),
            |m| floor * m.mean + WALL_RUN_PER_DEPTH * m.mean_square,
        )
    }

    /// Its trunk's mean void a unit length, square metres, its second chord graded to its
    /// receiver's valley depth `receiver` (R09.T6.h): half its own [`void_m2`](Self::void_m2), over
    /// its first chord, and half its [`graded_void_m2`](Self::graded_void_m2), over its second,
    /// each chord half its trunk's mean length, as the closed forms count a trunk's halves.
    ///
    /// # Panics
    ///
    /// If `receiver` is negative or not finite.
    #[must_use]
    pub fn trunk_void_m2(&self, receiver: Metres) -> f64 {
        0.5 * self.void_m2() + 0.5 * self.graded_void_m2(receiver)
    }
}

/// The points along a graded chord over which a mean is taken where its wall reaches its cap: 64,
/// at the middles of equal steps of s (a fixed-order midpoint rule, within about 10⁻⁴ of the
/// integral).
pub const GRADED_SAMPLES: u32 = 64;

/// The moments of a graded chord's depth G(s) = G₀ + Δ S(s) over s in [0, 1], S the cubic
/// smoothstep: ⟨G⟩ = G₀ + Δ ÷ 2, ⟨G²⟩ = G₀² + G₀ Δ + 13Δ² ÷ 35 and ⟨G³⟩ = G₀³ + 3G₀²Δ ÷ 2 +
/// 39 G₀ Δ² ÷ 35 + 43Δ³ ÷ 140, from ∫ S = ½, ∫ S² = 13 ÷ 35 and ∫ S³ = 43 ÷ 140.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Moments {
    mean: f64,
    mean_square: f64,
    mean_cube: f64,
}

impl Moments {
    /// A depth linear along the chord from `from` to `to`: ⟨Gⁿ⟩ = (G₁ⁿ⁺¹ − G₀ⁿ⁺¹) ÷ ((n + 1)(G₁ −
    /// G₀)), in its expanded form, exact where the two are equal (R09.T6.i's tributary profile, a
    /// linear stand-in for the concave stream-power one, whose ⟨G⟩ it puts low by 2–6% of the
    /// rise, which [`BENT_TRIBUTARY_VARIANCE_SHARE`] absorbs).
    #[must_use]
    fn linear(from: f64, to: f64) -> Self {
        let (a, b) = (from, to);
        Self {
            mean: f64::midpoint(a, b),
            mean_square: (a * a + a * b + b * b) / 3.0,
            mean_cube: 0.25 * (a + b) * (a * a + b * b),
        }
    }

    #[must_use]
    fn of(from: f64, to: f64) -> Self {
        let step = to - from;
        Self {
            mean: from + 0.5 * step,
            mean_square: from * from + from * step + 13.0 * step * step / 35.0,
            mean_cube: from * from * from
                + 1.5 * from * from * step
                + 39.0 * from * step * step / 35.0
                + 43.0 * step * step * step / 140.0,
        }
    }
}

/// The mean over a chord whose depth grades from `from` to `to` (metres; [`graded_depth_m`]) of a
/// cross-section's `integrand` of its depth G and wall W = min([`WALL_RUN_PER_DEPTH`] G, `cap`):
/// `uncapped` of the depth's [`Moments`] where no wall reaches the cap, and otherwise the mean over
/// [`GRADED_SAMPLES`] points in order.
#[must_use]
fn graded_mean(
    from: f64,
    to: f64,
    cap: f64,
    integrand: impl Fn(f64, f64) -> f64,
    uncapped: impl Fn(Moments) -> f64,
) -> f64 {
    if WALL_RUN_PER_DEPTH * num::max(from, to) <= cap {
        return uncapped(Moments::of(from, to));
    }
    let n = f64::from(GRADED_SAMPLES);
    let mut sum = 0.0;
    for k in 0..GRADED_SAMPLES {
        let s = (f64::from(k) + 0.5) / n;
        let g = from + (to - from) * (s * s * (3.0 - 2.0 * s));
        sum += integrand(g, num::min(WALL_RUN_PER_DEPTH * g, cap));
    }
    sum / n
}

/// The valley of the trunk of the coarse cell `cell` of `body` on a world dry now
/// ([`EpochValley`]): `None` where the cell carries no epoch's valley, its code
/// [`LogIncision::NONE`](crate::field::LogIncision::NONE), a world wet now or never wet, whose
/// trunk is T6.b's, or where it has no trunk. A cell the epoch did not cut
/// ([`LogIncision::UNCUT`](crate::field::LogIncision::UNCUT)) has a valley of depth zero.
///
/// The steady-state depth reads the cell's tributaries' steepness, or its own `k_s` where it sheds
/// no runoff of its own (a cell renewed after part of the epoch), and its bankfull channel its
/// catchment's runoff over its drainage, as its trunk's segment does.
///
/// # Panics
///
/// If `body` is not a body.
#[must_use]
pub fn epoch_valley(cell: &SynthesisCell, body: ChannelBody) -> Option<EpochValley> {
    let incision = cell.epoch_incision.incision()?;
    cell.flow.edge()?;
    if cell.catchment_runoff == crate::field::LogRunoff::NONE {
        return None;
    }
    let (_, width, depth) = channel_size(
        cell.catchment_runoff.metres_per_year(),
        cell.drainage.area(),
        body.gravity,
    );
    let valley = epoch_valley_steepness(cell)
        .map_or(Metres::ZERO, |k| valley_depth(k, body.radius, body.level));
    Some(EpochValley::of(
        valley,
        Metres::new(width),
        Metres::new(depth),
        incision,
        body,
    ))
}

/// The valley depth, metres, to which a world dry now's trunk grades along its second chord into
/// the coarse cell `receiver` of `body` (R09.T6.h): its epoch valley's depth, and zero where it
/// has none or is under water, so that the valley tapers to nothing at its key point.
#[must_use]
pub(crate) fn receiver_valley_m(receiver: &SynthesisCell, body: ChannelBody) -> f64 {
    if receiver.is_under_water() {
        0.0
    } else {
        epoch_valley(receiver, body).map_or(0.0, |v| v.depth().value())
    }
}

/// The steepness index a dry-now cell's trunk valley's steady-state depth reads, m^0.9: its
/// tributaries' ([`tributary_steepness_of`]), or its own `k_s` where it has channels but sheds no
/// runoff of its own; `None` without channels.
#[must_use]
pub(crate) fn epoch_valley_steepness(cell: &SynthesisCell) -> Option<f64> {
    if !super::has_channels(cell) {
        return None;
    }
    Some(tributary_steepness_of(cell).unwrap_or_else(|| cell.steepness.index_m0_9()))
}

/// A body as the channels' closed forms read it: its mean radius, its coarse level and its surface
/// gravity, which a field's header gives ([`ChannelBody::of`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelBody {
    /// The body's volumetric mean radius.
    pub radius: Metres,
    /// The coarse field's level L, the network's first level.
    pub level: u8,
    /// The surface gravity, by which the hydraulic geometry scales.
    pub gravity: MetresPerSecondSquared,
}

impl ChannelBody {
    /// The body of `header`: its radius, level and gravity
    /// ([`CraterParams::gravity`](crate::craters::CraterParams::gravity)).
    #[must_use]
    pub fn of(header: &FieldHeader) -> Self {
        Self {
            radius: header.radius(),
            level: header.level().get(),
            gravity: header.craters().gravity(),
        }
    }

    /// Asserts it is a body: a radius and a gravity finite and positive, and a level a cube's.
    fn check(&self) {
        let (r, g) = (self.radius.value(), self.gravity.value());
        assert!(
            r.is_finite() && r > 0.0,
            "a body's radius must be finite and positive, got {r} m"
        );
        assert!(
            g.is_finite() && g > 0.0,
            "a body's gravity must be finite and positive, got {g} m s⁻²"
        );
        assert!(
            self.level <= MAX_LEVEL,
            "a level is at most {MAX_LEVEL}, got {}",
            self.level
        );
    }
}

/// The deepest level whose grooves are evaluated below the coarse cell `cell` of `body`: its
/// coarse level itself where the cell carries no tributaries, and otherwise the deepest finer level
/// whose mean cell is at least the channel heads' drainage ([`channel_head_area`] of its
/// tributaries' steepness) and whose channel draining one mean cell at the cell's own runoff is at
/// least [`CUT_WIDTH`] wide (the [module documentation](self), "The cut").
///
/// # Panics
///
/// If `body`'s radius or gravity is not finite and positive, or its level is above
/// [`MAX_LEVEL`].
#[must_use]
pub fn deepest_level(cell: &SynthesisCell, body: ChannelBody) -> u8 {
    deepest_level_cut(cell, body, CUT_WIDTH.value())
}

/// The continuous cut of the coarse cell `cell` of `body` under R09.T6.i (its (e)): the level, not
/// held to a whole one, whose mean cell is the larger of its channel heads' drainage
/// ([`channel_head_area`] of its tributaries' steepness) and the area whose channel at its own
/// runoff is [`CUT_WIDTH`] wide ([`super::cut_area_m2`]), L + log₄ of the coarse mean cell over
/// it, and never above the coarse level L itself; L where the cell carries no tributaries or is a
/// world dry now's. [`deepest_level`] is about its whole part.
#[must_use]
pub(crate) fn cut_level(cell: &SynthesisCell, body: ChannelBody) -> f64 {
    let first = f64::from(body.level);
    let Some(steepness) = tributary_steepness_of(cell) else {
        return first;
    };
    if cell.epoch_incision.is_epoch() {
        return first;
    }
    let heads = channel_head_area(steepness).value();
    let cut = super::cut_area_m2(cell.runoff.metres_per_year(), CUT_WIDTH, body.gravity);
    let coarse = mean_cell_area(body.radius.value(), body.level);
    num::max(
        first,
        first + 0.5 * math::log2(coarse / num::max(heads, cut)),
    )
}

/// The deepest level the closed forms count below the coarse cell `cell` of `body`: the deepest its
/// continuous cut weights above zero, the first level below [`cut_level`] + 1, at most
/// [`deepest_possible_level`] (R09.T6.i's (e)).
#[must_use]
fn drawn_deepest_level(cell: &SynthesisCell, body: ChannelBody) -> u8 {
    levels_below(body, cut_level(cell, body) + 1.0)
}

/// The deepest level below `reach` (a continuous level) of a network of `body`, from its coarse
/// level, at most [`deepest_possible_level`].
#[must_use]
fn levels_below(body: ChannelBody, reach: f64) -> u8 {
    let deepest = deepest_possible_level(body.radius.value(), body.level);
    let mut level = body.level;
    while level < deepest && f64::from(level + 1) < reach {
        level += 1;
    }
    level
}

/// [`deepest_level`] with channels narrower than `cut_m` metres cut, rather than [`CUT_WIDTH`]:
/// `f64::INFINITY` cuts every finer level, and 0 ends the network at the channel heads alone.
#[must_use]
fn deepest_level_cut(cell: &SynthesisCell, body: ChannelBody, cut_m: f64) -> u8 {
    body.check();
    let level = body.level;
    let Some(steepness) = tributary_steepness_of(cell) else {
        return level;
    };
    // A world dry now's epoch cut its trunks alone ([`EpochValley`]).
    if cell.epoch_incision.is_epoch() {
        return level;
    }
    let heads = channel_head_area(steepness).value();
    let runoff = cell.runoff.metres_per_year();
    let mut deepest = level;
    for k in level + 1..=MAX_LEVEL {
        let area = mean_cell_area(body.radius.value(), k);
        if area < heads {
            break;
        }
        let (_, width, _) = channel_size(runoff, SquareMetres::new(area), body.gravity);
        if width < cut_m {
            break;
        }
        deepest = k;
    }
    deepest
}

/// The cubic smoothstep's complement f(x) = 1 − x²(3 − 2x) on [0, 1], 1 below and 0 above, and its
/// derivative −6x(1 − x): a valley's cross-profile.
#[must_use]
fn profile(x: f64) -> (f64, f64) {
    if x <= 0.0 {
        (1.0, 0.0)
    } else if x >= 1.0 {
        (0.0, 0.0)
    } else {
        (1.0 - x * x * (3.0 - 2.0 * x), -6.0 * x * (1.0 - x))
    }
}

/// A graded trunk's depth at parameter `t`, metres, and its derivative in t: `from_m` over its first
/// chord, to its crossing at `exit`, and over its second G = `from_m` + (`to_m` − `from_m`) S(s),
/// S the cubic smoothstep 3s² − 2s³ of s = (t − exit) ÷ (1 − exit), from 0 at the crossing to 1
/// at the receiver's key point, held between the two (R09.T6.h; decision-r09-t14-mars-ceiling §5
/// as amended by main's ruling 1 of 2026-10-30).
#[must_use]
pub(crate) fn graded_depth_m(from_m: f64, to_m: f64, exit: f64, t: f64) -> (f64, f64) {
    if t <= exit {
        return (from_m, 0.0);
    }
    let run = 1.0 - exit;
    let s = num::min((t - exit) / run, 1.0);
    let step = to_m - from_m;
    let g = from_m + step * (s * s * (3.0 - 2.0 * s));
    let held = num::max(num::min(from_m, to_m), num::min(g, num::max(from_m, to_m)));
    (held, step * 6.0 * s * (1.0 - s) / run)
}

/// How a groove's depth at its axis varies along its segment.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Depth {
    /// A trunk's: the same all along it.
    Constant(f64),
    /// A world dry now's trunk's: graded along its second chord to its receiver's
    /// ([`graded_depth_m`]; R09.T6.h).
    Graded { from_m: f64, to_m: f64, exit: f64 },
    /// A world wet now's trunk's under R09.T6.i: its valley graded along its second chord to its
    /// receiver's ([`graded_depth_m`]), at least its channel's `floor_m` and at most the level's
    /// `cap_m`.
    GradedValley {
        from_m: f64,
        to_m: f64,
        exit: f64,
        floor_m: f64,
        cap_m: f64,
    },
    /// A tributary's: min(max(top − bed(t), `floor_m`), `cap_m`), with the bed `bed_to_m` plus its
    /// stream-power rise from the join scaled by `scale` ([`rise`], as [`Segment::bed_at`] computes
    /// it; φ under R09.T6.i, 1 below it).
    Tributary {
        top_m: f64,
        bed_to_m: f64,
        rise: RiseTo,
        head_m: f64,
        length_m: f64,
        floor_m: f64,
        cap_m: f64,
        scale: f64,
    },
}

impl Depth {
    /// The depth at parameter `t`, metres, and its derivative in t.
    #[must_use]
    fn at(&self, t: f64) -> (f64, f64) {
        match *self {
            Self::Constant(depth) => (depth, 0.0),
            Self::Graded { from_m, to_m, exit } => graded_depth_m(from_m, to_m, exit, t),
            Self::GradedValley {
                from_m,
                to_m,
                exit,
                floor_m,
                cap_m,
            } => {
                let (valley, d_valley) = graded_depth_m(from_m, to_m, exit, t);
                if valley <= floor_m {
                    (floor_m, 0.0)
                } else if valley >= cap_m {
                    (cap_m, 0.0)
                } else {
                    (valley, d_valley)
                }
            }
            Self::Tributary {
                top_m,
                bed_to_m,
                rise,
                head_m,
                length_m,
                floor_m,
                cap_m,
                scale,
            } => {
                let main_stream = head_m + t * length_m;
                let below = top_m - (bed_to_m + scale * rise.from(main_stream));
                if below <= floor_m {
                    (floor_m, 0.0)
                } else if below >= cap_m {
                    (cap_m, 0.0)
                } else {
                    // The bed falls by φ S ℓ a unit of t, so the depth below the top grows by it.
                    (below, scale * rise.slope_at(main_stream) * length_m)
                }
            }
        }
    }
}

/// One segment's groove, ready for queries (see the module documentation, "The valleys").
#[derive(Debug, Clone, PartialEq)]
struct Groove {
    chords: [Chord; 2],
    chord_count: usize,
    /// A bent trunk's course, `None` for straight chords (R09.T6.h).
    bent: Option<BentGroove>,
    depth: Depth,
    /// Half the channel's bankfull width, metres.
    floor_m: f64,
    /// The wall width W, metres: a bent groove's widest.
    wall_m: f64,
    /// The box beyond which the groove is zero, body-fixed, metres.
    low: [f64; 3],
    high: [f64; 3],
}

/// A bent trunk's groove: its course, the box beyond which each half is zero, and how its wall's
/// width follows its depth (R09.T6.h).
#[derive(Debug, Clone, PartialEq)]
struct BentGroove {
    bend: Arc<Bend>,
    /// Each half's box, low and high corners, body-fixed, metres; a tributary's second is empty.
    boxes: [([f64; 3], [f64; 3]); 2],
    wall: Wall,
    /// The continuous cut's weight on its depth, 1 but at a tributary's partial level (R09.T6.i).
    weight: f64,
    /// Where a sub-chord reads its depth beyond its ends.
    ends: Ends,
}

/// Where a bent groove's sub-chord reads its depth for a point beyond its ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ends {
    /// At its end, its nearest point (R09.T6.h's, a world dry now's): each sub-chord's end cap is
    /// its end's depth.
    Nearest,
    /// At the point's projection on its line, held to the course's ends (R09.T6.i's, a world wet
    /// now's): where the depth changes quickly along a course, as a tributary's does, a cap at its
    /// end's depth would stand a scallop above the next sub-chord's floor and walls at every
    /// vertex; read on its line, a sub-chord's depth beyond its end is its neighbour's there.
    OnLine,
}

/// How a bent groove's wall width follows its depth: W = min(`per_depth` G, `cap_m`) at every
/// point, a valley of a short wet epoch's [`WALL_RUN_PER_DEPTH`] ([`EpochValley`]), or under
/// R09.T6.i a world wet now's [`WET_WALL_PER_DEPTH`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct Wall {
    /// Metres across a metre of depth.
    per_depth: f64,
    /// The widest wall, metres: T6.b's, half the level's mean cell.
    cap_m: f64,
}

/// A world wet now's walls under R09.T6.i, in metres across per metre of depth:
/// [`STEEPEST_WALL`] ÷ [`VALLEY_WALL_GRADIENT`], 2.5, so that every wall's steepest gradient,
/// 1.5 G ÷ W, is the design cap [`VALLEY_WALL_GRADIENT`] wherever the level's half cell does not
/// cap it (its (b)).
const WET_WALL_PER_DEPTH: f64 = STEEPEST_WALL / VALLEY_WALL_GRADIENT;

impl Wall {
    /// The wall's width at a depth `depth` changing at `d_depth` a unit of t, metres, and its
    /// change a unit of t.
    #[must_use]
    fn at(self, depth: f64, d_depth: f64) -> (f64, f64) {
        let wall = self.per_depth * depth;
        if wall < self.cap_m {
            (wall, self.per_depth * d_depth)
        } else {
            (self.cap_m, 0.0)
        }
    }
}

/// A bent groove's deepest sub-chord at a point so far: its value there, the parameter of its
/// nearest point, the distance and the profile's x, the gap from the nearest point and the
/// parameter's gradient.
#[derive(Debug, Clone, Copy)]
struct Nearest {
    value: f64,
    t: f64,
    d: f64,
    x: f64,
    gap: [f64; 3],
    along: [f64; 3],
}

/// The box of `chords` grown by `reach` metres, low and high corners.
#[must_use]
fn chords_box(chords: impl IntoIterator<Item = Chord>, reach: f64) -> ([f64; 3], [f64; 3]) {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for chord in chords {
        for k in 0..3 {
            low[k] = num::min(low[k], num::min(chord.from[k], chord.to[k]) - reach);
            high[k] = num::max(high[k], num::max(chord.from[k], chord.to[k]) + reach);
        }
    }
    (low, high)
}

/// How far a groove reaches beyond its chords, metres: its floor and its wall, with a margin that
/// keeps a rounded distance from reaching inside.
#[must_use]
fn reach_m(floor_m: f64, wall_m: f64) -> f64 {
    (floor_m + wall_m) * (1.0 + 1e-9) + 1e-6
}

/// Whether `q` lies inside the box from `low` to `high`.
#[must_use]
fn in_box(q: [f64; 3], (low, high): &([f64; 3], [f64; 3])) -> bool {
    (0..3).all(|k| q[k] >= low[k] && q[k] <= high[k])
}

/// The nearest point to `q` of `chord`: its squared distance, its parameter along the segment, the
/// gap from it to `q`, and the parameter's gradient as a function of `q` (zero where the nearest
/// point is an end).
#[must_use]
fn nearest_of(q: [f64; 3], chord: Chord) -> (f64, f64, [f64; 3], [f64; 3]) {
    let Chord { from, to, span } = chord;
    let ab = [0, 1, 2].map(|k| to[k] - from[k]);
    let aq = [0, 1, 2].map(|k| q[k] - from[k]);
    let ab2 = dot(ab, ab);
    let raw = dot(aq, ab) / ab2;
    let u = num::max(0.0, num::min(raw, 1.0));
    let near = [0, 1, 2].map(|k| from[k] + u * ab[k]);
    let gap = [0, 1, 2].map(|k| q[k] - near[k]);
    let d2 = dot(gap, gap);
    // d t ÷ d q along the chord where the nearest point is inside it, else zero.
    let rate = if raw > 0.0 && raw < 1.0 {
        (span.1 - span.0) / ab2
    } else {
        0.0
    };
    let along = [0, 1, 2].map(|k| rate * ab[k]);
    let t = span.0 + (span.1 - span.0) * u;
    (d2, t, gap, along)
}

/// The parameter along the course of `q`'s projection on `chord`'s line, held to the course's
/// ends, [0, 1], and its gradient as a function of `q` (zero where it is held).
#[must_use]
fn on_line_of(q: [f64; 3], chord: Chord) -> (f64, [f64; 3]) {
    let Chord { from, to, span } = chord;
    let ab = [0, 1, 2].map(|k| to[k] - from[k]);
    let aq = [0, 1, 2].map(|k| q[k] - from[k]);
    let ab2 = dot(ab, ab);
    let raw = span.0 + (span.1 - span.0) * (dot(aq, ab) / ab2);
    let t = num::max(0.0, num::min(raw, 1.0));
    let rate = if raw > 0.0 && raw < 1.0 {
        (span.1 - span.0) / ab2
    } else {
        0.0
    };
    (t, [0, 1, 2].map(|k| rate * ab[k]))
}

/// A groove's value at a point and its gradient there, as a function of the point (metres a metre).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Value {
    value: f64,
    gradient: [f64; 3],
}

impl Value {
    const ZERO: Self = Self {
        value: 0.0,
        gradient: [0.0; 3],
    };
}

/// What a bent groove's segment gives it ([`Groove::bent`]): its own depth and its deepest, metres,
/// how its depth runs along it, its wall, its weight and where its sub-chords read their depth.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BentDepth {
    own_m: f64,
    deepest_m: f64,
    depth: Depth,
    wall: Wall,
    weight: f64,
    ends: Ends,
}

/// [`BentDepth`] of the bent segment `segment` at its level's T6.b wall `wall_m` and cap `cap_m`.
#[must_use]
fn bent_depth(segment: &Segment, wall_m: f64, cap_m: f64) -> BentDepth {
    let floor_depth = segment.depth_m;
    let wet = |own_m: f64, deepest_m: f64, depth: Depth, weight: f64| BentDepth {
        own_m,
        deepest_m,
        depth,
        wall: Wall {
            per_depth: WET_WALL_PER_DEPTH,
            cap_m: wall_m,
        },
        weight,
        ends: Ends::OnLine,
    };
    match segment.kind {
        Kind::Trunk {
            valley_m,
            epoch_wall_m: Some(_),
            receiver_valley_m,
            exit,
            ..
        } => BentDepth {
            own_m: valley_m,
            deepest_m: receiver_valley_m.map_or(valley_m, |to_m| num::max(valley_m, to_m)),
            depth: receiver_valley_m.map_or(Depth::Constant(valley_m), |to_m| Depth::Graded {
                from_m: valley_m,
                to_m,
                exit,
            }),
            wall: Wall {
                per_depth: WALL_RUN_PER_DEPTH,
                cap_m: wall_m,
            },
            weight: 1.0,
            ends: Ends::Nearest,
        },
        Kind::Trunk {
            valley_m,
            epoch_wall_m: None,
            receiver_valley_m,
            exit,
            ..
        } => {
            let own = trunk_depth_m(valley_m, floor_depth, cap_m);
            match receiver_valley_m {
                Some(to_m) => wet(
                    own,
                    num::max(own, trunk_depth_m(to_m, floor_depth, cap_m)),
                    Depth::GradedValley {
                        from_m: valley_m,
                        to_m,
                        exit,
                        floor_m: num::min(floor_depth, cap_m),
                        cap_m,
                    },
                    1.0,
                ),
                None => wet(own, own, Depth::Constant(own), 1.0),
            }
        }
        Kind::Tributary {
            steepness,
            head_m,
            length_m,
            top_m,
            scale,
            weight,
            ..
        } => {
            let floor = num::min(floor_depth, cap_m);
            let depth = Depth::Tributary {
                top_m,
                bed_to_m: segment.bed_to_m,
                rise: RiseTo::new(steepness, head_m + length_m),
                head_m,
                length_m,
                floor_m: floor,
                cap_m,
                scale,
            };
            // Its bed rises from its join to its head, so it is deepest at its join, and its walls
            // widest there.
            let deepest = depth.at(1.0).0;
            wet(deepest, deepest, depth, weight)
        }
    }
}

impl Groove {
    /// The groove of `segment` of a body of mean radius `radius_m`, or `None` where it is
    /// nowhere deeper than zero.
    #[must_use]
    fn of(segment: &Segment, radius_m: f64) -> Option<Self> {
        let wall_m = VALLEY_WALL_SHARE * mean_cell_width(radius_m, segment.level());
        let cap_m = groove_cap_m(radius_m, segment.level());
        let floor_depth = segment.depth_m;
        if let Some(bend) = segment.bend() {
            return Self::bent(segment, bend, wall_m, cap_m);
        }
        let (chords, chord_count, depth, wall_m) = match segment.kind {
            Kind::Trunk {
                valley_m,
                epoch_wall_m,
                ..
            } => {
                let chords = segment.chords().expect("a trunk has two chords");
                match epoch_wall_m {
                    // T6.b's steady state: at least the channel, at most the level's cap, its
                    // valley held to its base level (R09.T6.f; [`trunk_valley_m`]).
                    None => {
                        let depth = trunk_depth_m(valley_m, floor_depth, cap_m);
                        (chords, 2, Depth::Constant(depth), wall_m)
                    }
                    // A world dry now's: the epoch's valley as it is ([`EpochValley`]).
                    Some(epoch_wall) => (chords, 2, Depth::Constant(valley_m), epoch_wall),
                }
            }
            Kind::Tributary {
                steepness,
                head_m,
                length_m,
                top_m,
                scale,
                ..
            } => {
                let chord = Chord {
                    from: segment.from_m,
                    to: segment.to_m,
                    span: (0.0, 1.0),
                };
                let depth = Depth::Tributary {
                    top_m,
                    bed_to_m: segment.bed_to_m,
                    rise: RiseTo::new(steepness, head_m + length_m),
                    head_m,
                    length_m,
                    floor_m: num::min(floor_depth, cap_m),
                    cap_m,
                    scale,
                };
                ([chord, chord], 1, depth, wall_m)
            }
        };
        // A tributary's depth is at least its channel's, so only a groove with no channel and no
        // valley is empty.
        let deepest = match depth {
            Depth::Constant(d) => d,
            Depth::Graded { from_m, to_m, .. } => num::max(from_m, to_m),
            Depth::GradedValley { cap_m, .. } => cap_m,
            Depth::Tributary { floor_m, cap_m, .. } => num::max(floor_m, cap_m),
        };
        if deepest <= 0.0 {
            return None;
        }
        let floor_m = 0.5 * segment.width_m;
        // A point farther than the floor and the wall from every chord is outside the groove; the
        // margin keeps a rounded distance from reaching inside.
        let reach = (floor_m + wall_m) * (1.0 + 1e-9) + 1e-6;
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for chord in &chords[..chord_count] {
            for k in 0..3 {
                low[k] = num::min(low[k], num::min(chord.from[k], chord.to[k]) - reach);
                high[k] = num::max(high[k], num::max(chord.from[k], chord.to[k]) + reach);
            }
        }
        Some(Self {
            chords,
            chord_count,
            bent: None,
            depth,
            floor_m,
            wall_m,
            low,
            high,
        })
    }

    /// The groove of the bent segment `segment`, whose course is `bend`, at its level's T6.b wall
    /// `wall_m` and cap `cap_m`, or `None` where it is nowhere deeper than zero: a world dry now's
    /// trunk's, its depth graded along its second chord where it has its receiver's and its wall
    /// [`WALL_RUN_PER_DEPTH`] times its depth at every point, at most T6.b's (R09.T6.h); and under
    /// R09.T6.i a world wet now's trunk's, its valley graded so, and a tributary's, each wall
    /// [`WET_WALL_PER_DEPTH`] times its depth, at most T6.b's, a tributary's depth weighted by its
    /// continuous cut.
    #[must_use]
    fn bent(segment: &Segment, bend: &Arc<Bend>, wall_m: f64, cap_m: f64) -> Option<Self> {
        let BentDepth {
            own_m,
            deepest_m: deepest,
            depth,
            wall,
            weight,
            ends,
        } = bent_depth(segment, wall_m, cap_m);
        if own_m <= 0.0 || weight <= 0.0 {
            return None;
        }
        let floor_m = 0.5 * segment.width_m;
        let widest = wall.at(deepest, 0.0).0;
        let boxes = if bend.halves() == 2 {
            [
                chords_box(bend.chords(0), reach_m(floor_m, wall.at(own_m, 0.0).0)),
                chords_box(bend.chords(1), reach_m(floor_m, widest)),
            ]
        } else {
            [
                chords_box(bend.chords(0), reach_m(floor_m, widest)),
                chords_box(std::iter::empty(), 0.0),
            ]
        };
        let low = [0, 1, 2].map(|k| num::min(boxes[0].0[k], boxes[1].0[k]));
        let high = [0, 1, 2].map(|k| num::max(boxes[0].1[k], boxes[1].1[k]));
        let chord = Chord {
            from: segment.from_m,
            to: segment.to_m,
            span: (0.0, 1.0),
        };
        let chords = segment.chords().unwrap_or([chord, chord]);
        Some(Self {
            chords,
            chord_count: 2,
            bent: Some(BentGroove {
                bend: Arc::clone(bend),
                boxes,
                wall,
                weight,
                ends,
            }),
            depth,
            floor_m,
            wall_m: widest,
            low,
            high,
        })
    }

    /// Whether `q` (metres) is inside the groove's box.
    #[must_use]
    fn may_reach(&self, q: [f64; 3]) -> bool {
        (0..3).all(|k| q[k] >= self.low[k] && q[k] <= self.high[k])
    }

    /// Whether `q` (metres) lies on the groove's floor, its channel's bed: within half the
    /// channel's bankfull width of the nearest of its chords, or of its bent course's sub-chords
    /// (R10.T1.b's channel floor).
    #[must_use]
    fn floor_holds(&self, q: [f64; 3]) -> bool {
        if let Some(bent) = &self.bent {
            return (0..2).any(|half| {
                in_box(q, &bent.boxes[half])
                    && bent.bend.chords(half).any(|chord| {
                        let (d2, ..) = nearest_of(q, chord);
                        d2 <= self.floor_m * self.floor_m
                    })
            });
        }
        self.chords[..self.chord_count].iter().any(|chord| {
            let Chord { from, to, .. } = *chord;
            let ab = [0, 1, 2].map(|k| to[k] - from[k]);
            let aq = [0, 1, 2].map(|k| q[k] - from[k]);
            let u = num::max(0.0, num::min(dot(aq, ab) / dot(ab, ab), 1.0));
            let gap = [0, 1, 2].map(|k| q[k] - (from[k] + u * ab[k]));
            dot(gap, gap) <= self.floor_m * self.floor_m
        })
    }

    /// Whether the groove's box reaches the ball of `radius` about `centre` (metres).
    #[must_use]
    fn may_reach_ball(&self, centre: [f64; 3], radius: f64) -> bool {
        let mut gap2 = 0.0;
        for ((low, high), c) in self.low.iter().zip(&self.high).zip(&centre) {
            let gap = num::max(num::max(low - c, c - high), 0.0);
            gap2 += gap * gap;
        }
        gap2 <= radius * radius
    }

    /// The groove's depth at `q` (metres), positive, and its gradient there.
    #[must_use]
    #[expect(
        clippy::many_single_char_names,
        reason = "the point, the chord's parameter, distance and the profile's x and f, as the \
                  module documentation names them"
    )]
    fn at(&self, q: [f64; 3]) -> Value {
        if let Some(bent) = &self.bent {
            return self.bent_at(bent, q);
        }
        // The nearest point over its chords, the first of equals.
        let mut best: Option<(f64, f64, [f64; 3], [f64; 3])> = None;
        for chord in &self.chords[..self.chord_count] {
            let Chord { from, to, span } = *chord;
            let ab = [0, 1, 2].map(|k| to[k] - from[k]);
            let aq = [0, 1, 2].map(|k| q[k] - from[k]);
            let ab2 = dot(ab, ab);
            let raw = dot(aq, ab) / ab2;
            let u = num::max(0.0, num::min(raw, 1.0));
            let near = [0, 1, 2].map(|k| from[k] + u * ab[k]);
            let gap = [0, 1, 2].map(|k| q[k] - near[k]);
            let d2 = dot(gap, gap);
            // d t ÷ d q along the chord where the nearest point is inside it, else zero.
            let rate = if raw > 0.0 && raw < 1.0 {
                (span.1 - span.0) / ab2
            } else {
                0.0
            };
            let along = [0, 1, 2].map(|k| rate * ab[k]);
            let t = span.0 + (span.1 - span.0) * u;
            if best.is_none_or(|b| d2 < b.0) {
                best = Some((d2, t, gap, along));
            }
        }
        let (d2, t, gap, along) = best.expect("a groove has a chord");
        let d = d2.sqrt();
        let x = (d - self.floor_m) / self.wall_m;
        if x >= 1.0 {
            return Value::ZERO;
        }
        let (f, df) = profile(x);
        let (depth, d_depth) = self.depth.at(t);
        let across = if d > 0.0 {
            [0, 1, 2].map(|k| gap[k] / d)
        } else {
            [0.0; 3]
        };
        Value {
            value: depth * f,
            gradient: [0, 1, 2]
                .map(|k| f * d_depth * along[k] + depth * df / self.wall_m * across[k]),
        }
    }

    /// A bent groove's depth at `q` (metres), positive, and its gradient there: the deepest of its
    /// sub-chords' grooves, each its depth G(t) and wall W(t) at its own nearest point (on a world
    /// wet now, at `q`'s projection on its line, [`Ends::OnLine`]), the first of equals, so that a
    /// depth varying along the course leaves no step across a bend's bisector where the nearest
    /// sub-chord changes (R09.T6.h).
    #[must_use]
    fn bent_at(&self, bent: &BentGroove, q: [f64; 3]) -> Value {
        let mut deepest: Option<Nearest> = None;
        // A sub-chord farther than the floor and the widest wall from `q` cuts nothing there.
        let reach = reach_m(self.floor_m, self.wall_m);
        for half in 0..2 {
            if !in_box(q, &bent.boxes[half]) {
                continue;
            }
            for chord in bent.bend.chords(half) {
                let near = (0..3).all(|k| {
                    q[k] >= num::min(chord.from[k], chord.to[k]) - reach
                        && q[k] <= num::max(chord.from[k], chord.to[k]) + reach
                });
                if !near {
                    continue;
                }
                let (d2, nearest_t, gap, nearest_along) = nearest_of(q, chord);
                let d = d2.sqrt();
                // Beyond the widest wall no depth reaches.
                if d - self.floor_m >= self.wall_m {
                    continue;
                }
                let (t, along) = match bent.ends {
                    Ends::Nearest => (nearest_t, nearest_along),
                    Ends::OnLine => on_line_of(q, chord),
                };
                let (depth, _) = self.depth.at(t);
                if depth <= 0.0 {
                    continue;
                }
                let (wall, _) = bent.wall.at(depth, 0.0);
                let x = (d - self.floor_m) / wall;
                if x >= 1.0 {
                    continue;
                }
                let value = depth * profile(x).0;
                if deepest.is_none_or(|b| value > b.value) {
                    deepest = Some(Nearest {
                        value,
                        t,
                        d,
                        x,
                        gap,
                        along,
                    });
                }
            }
        }
        let Some(Nearest {
            value,
            t,
            d,
            x,
            gap,
            along,
        }) = deepest
        else {
            return Value::ZERO;
        };
        let (f, df) = profile(x);
        let (depth, d_depth) = self.depth.at(t);
        let (wall, d_wall) = bent.wall.at(depth, d_depth);
        let across = if d > 0.0 {
            [0, 1, 2].map(|k| gap[k] / d)
        } else {
            [0.0; 3]
        };
        // x = (d − h) ÷ W(t): its gradient is the across direction ÷ W less x W′ ÷ W along t; the
        // continuous cut weights the whole.
        let w = bent.weight;
        Value {
            value: w * value,
            gradient: [0, 1, 2].map(|k| {
                w * (f * d_depth * along[k]
                    + depth * df * (across[k] / wall - x * d_wall / wall * along[k]))
            }),
        }
    }
}

/// The dot product, in index order.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A level's correction over one parent cell: its four corners' values, in the order (0, 0),
/// (1, 0), (1, 1), (0, 1) of the cell's face coordinates, and its bubble's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Correction {
    corners: [f64; 4],
    bubble: f64,
}

/// The bubble B(x) = 16 x²(1 − x)² on [0, 1], 1 at the middle, and its derivative.
#[must_use]
fn bubble(x: f64) -> (f64, f64) {
    let y = x * (1.0 - x);
    (16.0 * y * y, 32.0 * y * (1.0 - 2.0 * x))
}

impl Correction {
    /// Its value at the cell's face coordinates (`x`, `y`), each in [0, 1], and its derivatives in
    /// them.
    #[must_use]
    fn at(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let [c00, c10, c11, c01] = self.corners;
        let (bx, dbx) = bubble(x);
        let (by, dby) = bubble(y);
        let low = c00 + x * (c10 - c00);
        let high = c01 + x * (c11 - c01);
        let value = low + y * (high - low) + self.bubble * bx * by;
        let d_x = (1.0 - y) * (c10 - c00) + y * (c11 - c01) + self.bubble * dbx * by;
        let d_y = high - low + self.bubble * bx * dby;
        (value, d_x, d_y)
    }
}

/// The cubic smoothstep S(x) = x²(3 − 2x) on [0, 1], 0 below and 1 above, and its derivative
/// 6x(1 − x): a groove's fade towards water.
#[must_use]
fn smoothstep(x: f64) -> (f64, f64) {
    let (f, df) = profile(x);
    (1.0 - f, -df)
}

/// A coarse cell's water: whether it is under water, and which of the cells across its edges and
/// corners are (R09.T6.f; the [module documentation](self), "Water").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shore {
    /// Whether the cell itself is under water.
    wet: bool,
    /// Whether the cell across each edge is, in [`Edge::ALL`]'s order.
    edges: [bool; 4],
    /// Whether the cell across each corner is, in [`Correction`]'s corner order; never at a cube
    /// corner, where there is no such cell and its two edge neighbours meet.
    corners: [bool; 4],
}

impl Shore {
    /// Whether the cell is dry and shares no edge or corner with a cell under water, so that no
    /// level's incision is faded there.
    #[must_use]
    fn is_inland(&self) -> bool {
        !self.wet && !self.edges.contains(&true) && !self.corners.contains(&true)
    }

    /// The fade of a level's incision at the face coordinates (`x`, `y`) of the dry cell, each in
    /// [0, 1], over the width `width` (a share of the cell's side), and its derivatives in x and
    /// y: the product of S(d ÷ `width`) over each edge and corner the cell shares with a cell under
    /// water, d the distance to that edge or corner in face coordinates and S the cubic
    /// [`smoothstep`], so zero on the shared edge or at the shared corner and one from `width` away.
    #[must_use]
    fn fade(&self, x: f64, y: f64, width: f64) -> (f64, f64, f64) {
        let (mut f, mut f_x, mut f_y) = (1.0, 0.0, 0.0);
        let mut times = |d: f64, d_x: f64, d_y: f64| {
            let (s, ds) = smoothstep(d / width);
            let rate = ds / width;
            f_x = f_x * s + f * rate * d_x;
            f_y = f_y * s + f * rate * d_y;
            f *= s;
        };
        // The distance to an edge's neighbour, from inside the cell, is the distance to the edge.
        let edges = [
            (x, 1.0, 0.0),
            (1.0 - x, -1.0, 0.0),
            (y, 0.0, 1.0),
            (1.0 - y, 0.0, -1.0),
        ];
        for (&wet, (d, d_x, d_y)) in self.edges.iter().zip(edges) {
            if wet {
                times(d, d_x, d_y);
            }
        }
        for (&wet, (cx, cy)) in
            self.corners
                .iter()
                .zip([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
        {
            if wet {
                let (dx, dy) = (x - cx, y - cy);
                let d = (dx * dx + dy * dy).sqrt();
                let (d_x, d_y) = if d > 0.0 {
                    (dx / d, dy / d)
                } else {
                    (0.0, 0.0)
                };
                times(d, d_x, d_y);
            }
        }
        (f, f_x, f_y)
    }
}

/// The width over which level `level` of a network whose first level is `first` fades towards
/// water, as a share of a coarse cell's side: the level's wall width, [`VALLEY_WALL_SHARE`] of its
/// own cell, ½ ÷ 2^(`level` − `first`), exact.
#[must_use]
fn fade_width(first: u8, level: u8) -> f64 {
    VALLEY_WALL_SHARE / f64::from(1_u32 << (level - first))
}

/// One sample of a parent cell's mean grid: its unit direction, its weight (the solid angle it
/// stands for, to a constant) and its face coordinates in the cell.
#[derive(Debug, Clone, Copy)]
struct Sample {
    dir: [f64; 3],
    weight: f64,
    x: f64,
    y: f64,
}

/// du ÷ ds of S2's warp ([`st_to_uv`]): 8s ÷ 3 from the middle up and 8(1 − s) ÷ 3 below it.
#[must_use]
fn warp_rate(s: f64) -> f64 {
    8.0 * num::max(s, 1.0 - s) / 3.0
}

/// The [`MEAN_SAMPLES`]² samples of `cell`'s mean grid, in (x, y) order, x fastest.
#[must_use]
#[expect(
    clippy::many_single_char_names,
    reason = "the cube's (s, t) and (u, v) and the cell's (x, y), as the crate names them"
)]
fn mean_grid(cell: PatchKey) -> Vec<Sample> {
    let n = MEAN_SAMPLES;
    let side = f64::from(1_u32 << cell.level());
    let mut out = Vec::with_capacity(usize::try_from(n * n).expect("the grid is small"));
    for b in 0..n {
        for a in 0..n {
            // (2a + 1) ÷ 2n, an exact binary fraction, as is its place on the face.
            let x = f64::from(2 * a + 1) / f64::from(2 * n);
            let y = f64::from(2 * b + 1) / f64::from(2 * n);
            let s = (f64::from(cell.i()) + x) / side;
            let t = (f64::from(cell.j()) + y) / side;
            let (u, v) = (st_to_uv(s), st_to_uv(t));
            let along = 1.0 + u * u + v * v;
            let weight = warp_rate(s) * warp_rate(t) / (along * along.sqrt());
            let dir = unit_dir(face_uv_to_xyz(FaceUv {
                face: cell.face(),
                u,
                v,
            }));
            out.push(Sample { dir, weight, x, y });
        }
    }
    out
}

/// The cells that share each of `cell`'s corners, in the order of [`Correction`]'s corners: the
/// cell, its two edge neighbours there and the cell across the corner, which a cube corner lacks.
#[must_use]
pub(crate) fn corner_cells(cell: PatchKey) -> [Vec<PatchKey>; 4] {
    let across = cell.corner_neighbours();
    let edge = |e: Edge| cell.edge_neighbour(e);
    let corners = [
        (Edge::UMin, Edge::VMin, across[0]),
        (Edge::UMax, Edge::VMin, across[1]),
        (Edge::UMax, Edge::VMax, across[2]),
        (Edge::UMin, Edge::VMax, across[3]),
    ];
    corners.map(|(a, b, diagonal)| {
        let mut cells = vec![cell, edge(a), edge(b)];
        cells.extend(diagonal);
        cells.sort_unstable_by_key(|c| c.to_u64());
        cells
    })
}

/// The face coordinates (x, y) of the unit direction `dir` in `cell`, each in [0, 1], and their
/// gradients on the unit sphere, per radian.
#[must_use]
fn cell_coordinates(cell: PatchKey, dir: [f64; 3]) -> (f64, f64, [f64; 3], [f64; 3]) {
    let side = f64::from(1_u32 << cell.level());
    let (normal, e_u, e_v) = axes(cell.face());
    let along = dot(dir, normal);
    let (u, v) = uv_on_face(cell.face(), dir);
    let x = num::max(0.0, num::min(uv_to_st(u) * side - f64::from(cell.i()), 1.0));
    let y = num::max(0.0, num::min(uv_to_st(v) * side - f64::from(cell.j()), 1.0));
    let dx_du = side * warp_slope(u);
    let dy_dv = side * warp_slope(v);
    let grad_x = [0, 1, 2].map(|k| dx_du * ((e_u[k] - u * normal[k]) / along));
    let grad_y = [0, 1, 2].map(|k| dy_dv * ((e_v[k] - v * normal[k]) / along));
    (x, y, grad_x, grad_y)
}

/// The cells within two king moves of `cell` on the cube's graph, itself included, in packed-key
/// order: the 3 × 3 graph neighbourhoods of its own 3 × 3, merged, so 25 cells, 21 about a cube
/// corner.
#[must_use]
fn two_moves(cell: PatchKey) -> Vec<PatchKey> {
    let mut cells: Vec<PatchKey> = neighbourhood(cell)
        .into_iter()
        .flatten()
        .flat_map(|c| neighbourhood(c).into_iter().flatten())
        .collect();
    cells.sort_unstable_by_key(|c| c.to_u64());
    cells.dedup();
    cells
}

/// The cell of the deepest level that holds the unit direction `dir`, whose ancestors are the
/// cells of every level that hold it.
#[must_use]
fn deepest_cell(dir: [f64; 3]) -> PatchKey {
    PatchKey::containing(crate::cube::MAX_LEVEL, dir).expect("a unit direction is in a cell")
}

/// The channels' part of a [`SynthCache`]: the network's memo and, by packed cell key and level,
/// each level's grooves by cell and its mean and correction by parent cell, for one body and seed
/// (see the module documentation).
#[derive(Debug, Clone, Default)]
pub(crate) struct ChannelCache {
    id: Option<NetworkId>,
    network: NetworkMemo,
    /// Per coarse cell, its deepest evaluated level and the deepest within two king moves.
    deepest: BTreeMap<u64, (u8, u8)>,
    /// Per cell of any level, the index in `lists` of the grooves of that level that may reach it.
    groove_index: BTreeMap<u64, usize>,
    /// The groove lists, in the order they were built (an order no result reads).
    lists: Vec<Vec<Groove>>,
    /// Per level, the cell whose list was last read and its index: a patch's points share their
    /// coarser cells, so most reads find it here.
    recent_list: [Option<(u64, usize)>; LEVELS],
    /// Per (level, parent cell), the level's mean over it.
    means: BTreeMap<(u8, u64), f64>,
    /// Per (level, parent cell), the level's correction over it.
    corrections: BTreeMap<(u8, u64), Correction>,
    /// Per level, the parent cell whose correction was last read, and it.
    recent_correction: [Option<(u64, Correction)>; LEVELS],
    /// Per coarse cell, its water and its neighbours' (R09.T6.f).
    shores: BTreeMap<u64, Shore>,
    /// The coarse cell whose shore was last read, and it.
    recent_shore: Option<(u64, Shore)>,
    /// Per coarse cell, its floor's corners' heights above the datum, metres, `None` where each
    /// is the sea level (R09.T6.g).
    floor_corners_m: BTreeMap<u64, Option<[f64; 4]>>,
}

/// Every level a cell may have, 0 to [`MAX_LEVEL`].
const LEVELS: usize = MAX_LEVEL as usize + 1;

impl ChannelCache {
    /// An empty cache.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            id: None,
            network: NetworkMemo::new(),
            deepest: BTreeMap::new(),
            groove_index: BTreeMap::new(),
            lists: Vec::new(),
            recent_list: [None; LEVELS],
            means: BTreeMap::new(),
            corrections: BTreeMap::new(),
            recent_correction: [None; LEVELS],
            shores: BTreeMap::new(),
            recent_shore: None,
            floor_corners_m: BTreeMap::new(),
        }
    }

    /// Empties the cache unless it holds `id`'s, then keeps `id`.
    fn keep(&mut self, id: NetworkId) {
        if self.id != Some(id) {
            *self = Self::new();
            self.id = Some(id);
        }
    }

    /// The network's memo for `id`, emptied first unless it holds `id`'s.
    pub(crate) fn network_memo(&mut self, id: NetworkId) -> &mut NetworkMemo {
        self.keep(id);
        &mut self.network
    }
}

/// One level's raw incision at a point, −(`E_k` − `E_{k−1}`), and its gradient as a function of
/// the spheroid point.
type Raw = Value;

/// One level's incision at a point less its correction there.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LevelSample {
    /// The value, metres.
    value: f64,
    /// The raw incision's gradient as a function of the spheroid point, metres a metre.
    of_point: [f64; 3],
    /// The correction's gradient, negated, on the unit sphere, metres a radian.
    of_direction: [f64; 3],
}

impl LevelSample {
    const ZERO: Self = Self {
        value: 0.0,
        of_point: [0.0; 3],
        of_direction: [0.0; 3],
    };
}

/// A value for each level from the first, on the stack: a query's levels, at most [`LEVELS`].
#[derive(Debug, Clone, Copy)]
struct PerLevel<T> {
    len: usize,
    items: [T; LEVELS],
}

impl<T: Copy> PerLevel<T> {
    /// None yet, `zero` filling the rest.
    const fn new(zero: T) -> Self {
        Self {
            len: 0,
            items: [zero; LEVELS],
        }
    }

    fn push(&mut self, item: T) {
        self.items[self.len] = item;
        self.len += 1;
    }

    /// The levels held, from the first.
    fn as_slice(&self) -> &[T] {
        &self.items[..self.len]
    }
}

/// The incision's working state over one field and one cache.
struct Incision<'a, F> {
    channels: &'a Channels,
    field: &'a F,
    cache: &'a mut ChannelCache,
}

impl<F: FieldView> Incision<'_, F> {
    /// The first level, the field's coarse level.
    #[must_use]
    fn first(&self) -> u8 {
        self.channels.level
    }

    /// The segment `cell`'s key point heads, through the cache's memo of the network.
    fn segment(&mut self, cell: PatchKey) -> Result<Option<Segment>, ReadCellError> {
        self.network().segment(cell)
    }

    /// The deepest evaluated level of the coarse cell `coarse`, and the deepest within two king
    /// moves of it: every groove a query's level reads, and every groove the means of its parent
    /// cell and the eight about it read, is headed within that, so a level beyond it has no groove
    /// and no correction there.
    fn deepest(&mut self, coarse: PatchKey) -> Result<(u8, u8), ReadCellError> {
        let key = coarse.to_u64();
        if let Some(&d) = self.cache.deepest.get(&key) {
            return Ok(d);
        }
        let own = self.deepest_of(coarse)?;
        let mut around = own;
        for cell in two_moves(coarse) {
            around = around.max(self.deepest_of(cell)?);
        }
        self.cache.deepest.insert(key, (own, around));
        Ok((own, around))
    }

    /// The deepest level whose grooves are evaluated below the coarse cell `coarse`: the deepest
    /// level its continuous cut weights above zero anywhere in it ([`super::Continuity::deepest`],
    /// its corners'; R09.T6.i), never past the deepest a cell of the body can reach
    /// ([`deepest_possible_level`]).
    fn deepest_of(&mut self, coarse: PatchKey) -> Result<u8, ReadCellError> {
        let body = self.channels.body();
        let record = read(self.field, coarse)?;
        let first = self.first();
        if tributary_steepness_of(record).is_none() || record.epoch_incision.is_epoch() {
            return Ok(first);
        }
        let reach = self.network().continuity(coarse)?.deepest() + 1.0;
        Ok(levels_below(body, reach))
    }

    /// The network over the field, through the cache's memo of it.
    #[must_use]
    fn network(&mut self) -> super::ChannelNetwork<'_, F> {
        super::ChannelNetwork {
            channels: self.channels,
            field: self.field,
            memo: super::Memo::Cache(&mut self.cache.network),
        }
    }

    /// The water of the coarse cell `coarse` and of the cells across its edges and corners
    /// ([`Shore`]); a wet cell's neighbours are not read.
    fn shore(&mut self, coarse: PatchKey) -> Result<Shore, ReadCellError> {
        let key = coarse.to_u64();
        if let Some((recent, shore)) = self.cache.recent_shore
            && recent == key
        {
            return Ok(shore);
        }
        let shore = if let Some(&shore) = self.cache.shores.get(&key) {
            shore
        } else {
            let wet = read(self.field, coarse)?.is_under_water();
            let (mut edges, mut corners) = ([false; 4], [false; 4]);
            if !wet {
                for (edge, side) in edges.iter_mut().zip(Edge::ALL) {
                    *edge = read(self.field, coarse.edge_neighbour(side))?.is_under_water();
                }
                for (corner, across) in corners.iter_mut().zip(coarse.corner_neighbours()) {
                    if let Some(cell) = across {
                        *corner = read(self.field, cell)?.is_under_water();
                    }
                }
            }
            let shore = Shore {
                wet,
                edges,
                corners,
            };
            self.cache.shores.insert(key, shore);
            shore
        };
        self.cache.recent_shore = Some((key, shore));
        Ok(shore)
    }

    /// The level the floor of the coarse cell `coarse` holds the channels' cut to, metres: the
    /// ground of a dry, terminal cell, a playa's pit, where the cell is one or drains into one, and
    /// otherwise the sea level; never below the sea level (R09.T6.g; decision-r09-channel-term
    /// §9.4).
    fn pit_level_m(&self, coarse: PatchKey) -> Result<f64, ReadCellError> {
        let sea = self.channels.sea_level().value();
        let record = read(self.field, coarse)?;
        let is_pit = |cell: &SynthesisCell| !cell.is_under_water() && cell.flow.edge().is_none();
        let pit = if record.is_under_water() {
            None
        } else if is_pit(record) {
            Some(record)
        } else if let Some(edge) = record.flow.edge() {
            Some(read(self.field, coarse.edge_neighbour(edge))?).filter(|r| is_pit(r))
        } else {
            None
        };
        Ok(pit.map_or(sea, |p| num::max(p.water_surface().value(), sea)))
    }

    /// The heights above the datum, metres, of the corners of the floor of the coarse cell
    /// `coarse`, in [`Correction`]'s corner order, each
    /// the highest [`pit_level_m`](Self::pit_level_m) of the cells that share it, so that the floor is
    /// continuous and at least each cell's own pit level all over it; `None` where every corner is
    /// the sea level.
    fn floor_corners_m(&mut self, coarse: PatchKey) -> Result<Option<[f64; 4]>, ReadCellError> {
        let key = coarse.to_u64();
        if let Some(&corners) = self.cache.floor_corners_m.get(&key) {
            return Ok(corners);
        }
        let sea = self.channels.sea_level().value();
        let mut corners = [sea; 4];
        let mut above = false;
        for (corner, cells) in corners.iter_mut().zip(corner_cells(coarse)) {
            for cell in cells {
                *corner = num::max(*corner, self.pit_level_m(cell)?);
            }
            above |= *corner > sea;
        }
        let corners = above.then_some(corners);
        self.cache.floor_corners_m.insert(key, corners);
        Ok(corners)
    }

    /// Whether `cell`, of the first level or finer, lies in a coarse cell under water.
    fn is_wet(&self, cell: PatchKey) -> Result<bool, ReadCellError> {
        Ok(read(self.field, ancestor(cell, self.first()))?.is_under_water())
    }

    /// The index in the cache's lists of the grooves of `cell`'s level that may reach `cell`: those
    /// of the segments headed within two king moves of it ([`two_moves`]), in that order, whose
    /// levels their coarse cells evaluate and whose boxes reach the cell's bounding ball.
    fn grooves(&mut self, cell: PatchKey) -> Result<usize, ReadCellError> {
        let key = cell.to_u64();
        let level = usize::from(cell.level());
        if let Some((recent, index)) = self.cache.recent_list[level]
            && recent == key
        {
            return Ok(index);
        }
        let index = if let Some(&index) = self.cache.groove_index.get(&key) {
            index
        } else {
            let list = self.build_grooves(cell)?;
            let index = self.cache.lists.len();
            self.cache.lists.push(list);
            self.cache.groove_index.insert(key, index);
            index
        };
        self.cache.recent_list[level] = Some((key, index));
        Ok(index)
    }

    fn build_grooves(&mut self, cell: PatchKey) -> Result<Vec<Groove>, ReadCellError> {
        let first = self.first();
        let level = cell.level();
        let figure = self.channels.figure;
        let centre = figure.point(cell.vertex_dir(32, 32));
        let mut ball = 0.0_f64;
        for (x, y) in [(0, 0), (64, 0), (64, 64), (0, 64)] {
            let corner = figure.point(cell.vertex_dir(x, y));
            let gap = [0, 1, 2].map(|k| corner[k] - centre[k]);
            ball = num::max(ball, dot(gap, gap).sqrt());
        }
        // A geodesically convex cell's farthest point from its centre is a corner; the margin
        // covers the spheroid's departure from a sphere and rounding.
        let ball = ball * 1.01 + 1.0;
        let radius_m = self.channels.radius.value();
        let mut list = Vec::new();
        for around in two_moves(cell) {
            if level > first {
                let (own, _) = self.deepest(ancestor(around, first))?;
                if level > own {
                    continue;
                }
            }
            let Some(segment) = self.segment(around)? else {
                continue;
            };
            // A cell under water cuts no valley of its own, not even past its crossing into a dry
            // receiver (R09.T6.f); only trunks can be a wet cell's, its tributaries none.
            if level == first && self.is_wet(around)? {
                continue;
            }
            if let Some(groove) = Groove::of(&segment, radius_m)
                && groove.may_reach_ball(centre, ball)
            {
                list.push(groove);
            }
        }
        Ok(list)
    }

    /// The raw incision of every level from the first to `last` at the unit direction `dir`,
    /// level by level ([`Raw`]).
    fn raw(&mut self, dir: [f64; 3], last: u8) -> Result<PerLevel<Raw>, ReadCellError> {
        let first = self.first();
        let q = self.channels.figure.point(dir);
        let finest = deepest_cell(dir);
        let mut envelope = Value::ZERO;
        let mut out = PerLevel::new(Value::ZERO);
        for level in first..=last {
            let index = self.grooves(ancestor(finest, level))?;
            let mut deepest = envelope;
            for groove in &self.cache.lists[index] {
                if !groove.may_reach(q) {
                    continue;
                }
                let v = groove.at(q);
                if v.value > deepest.value {
                    deepest = v;
                }
            }
            out.push(Value {
                value: envelope.value - deepest.value,
                gradient: [0, 1, 2].map(|k| envelope.gradient[k] - deepest.gradient[k]),
            });
            envelope = deepest;
        }
        Ok(out)
    }

    /// Whether the unit direction `dir` lies on the floor of a groove of any level from the first
    /// to `through` that [`levels`](Self::levels) evaluates there: on a channel's bed.
    fn on_floor(&mut self, dir: [f64; 3], through: u8) -> Result<bool, ReadCellError> {
        let first = self.first();
        if through < first {
            return Ok(false);
        }
        let finest = deepest_cell(dir);
        let (_, around) = self.deepest(ancestor(finest, first))?;
        let q = self.channels.figure.point(dir);
        for level in first..=through.min(around) {
            let index = self.grooves(ancestor(finest, level))?;
            if self.cache.lists[index]
                .iter()
                .any(|groove| groove.may_reach(q) && groove.floor_holds(q))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The parent cell over which level `level`'s mean at a point of `cell` (of that level) is
    /// taken: its parent, or the coarse cell itself for the first level.
    #[must_use]
    fn parent_of(&self, cell: PatchKey) -> PatchKey {
        if cell.level() == self.first() {
            cell
        } else {
            cell.parent().expect("a finer level's cell has a parent")
        }
    }

    /// Level `level`'s mean raw incision over the parent cell `parent`, metres (see the module
    /// documentation, "Each level's mean").
    fn mean(&mut self, level: u8, parent: PatchKey) -> Result<f64, ReadCellError> {
        let key = (level, parent.to_u64());
        if let Some(&m) = self.cache.means.get(&key) {
            return Ok(m);
        }
        let first = self.first();
        let at = usize::from(level - first);
        // A parent cell under water carries no incision (R09.T6.f), and one by water its faded
        // incision.
        let coarse = ancestor(parent, first);
        let shore = self.shore(coarse)?;
        if shore.wet {
            self.cache.means.insert(key, 0.0);
            return Ok(0.0);
        }
        let width = fade_width(first, level);
        let mut sum = 0.0;
        let mut weights = 0.0;
        for sample in mean_grid(parent) {
            let raw = self.raw(sample.dir, level)?;
            let value = raw.as_slice()[at].value;
            let value = if shore.is_inland() {
                value
            } else {
                let (x, y, _, _) = cell_coordinates(coarse, sample.dir);
                shore.fade(x, y, width).0 * value
            };
            sum += sample.weight * value;
            weights += sample.weight;
        }
        let mean = sum / weights;
        self.cache.means.insert(key, mean);
        Ok(mean)
    }

    /// Level `level`'s correction over the parent cell `parent`.
    fn correction(&mut self, level: u8, parent: PatchKey) -> Result<Correction, ReadCellError> {
        let key = (level, parent.to_u64());
        let slot = usize::from(level);
        if let Some((recent, c)) = self.cache.recent_correction[slot]
            && recent == key.1
        {
            return Ok(c);
        }
        if let Some(&c) = self.cache.corrections.get(&key) {
            self.cache.recent_correction[slot] = Some((key.1, c));
            return Ok(c);
        }
        let mut corners = [0.0; 4];
        for (corner, cells) in corners.iter_mut().zip(corner_cells(parent)) {
            // A corner a cell under water shares is zero, as that cell's correction is all over,
            // so that the correction is continuous onto it and the dry cell's bubble carries the
            // rest of its mean (R09.T6.f).
            let mut by_water = false;
            for &cell in &cells {
                by_water |= self.is_wet(cell)?;
            }
            if by_water {
                continue;
            }
            let mut sum = 0.0;
            for &cell in &cells {
                sum += self.mean(level, cell)?;
            }
            #[expect(
                clippy::cast_precision_loss,
                reason = "three or four cells, exact in f64"
            )]
            let count = cells.len() as f64;
            *corner = sum / count;
        }
        let plain = Correction {
            corners,
            bubble: 0.0,
        };
        let (mut linear, mut bump, mut weights) = (0.0, 0.0, 0.0);
        for sample in mean_grid(parent) {
            let (bx, _) = bubble(sample.x);
            let (by, _) = bubble(sample.y);
            linear += sample.weight * plain.at(sample.x, sample.y).0;
            bump += sample.weight * bx * by;
            weights += sample.weight;
        }
        let mean = self.mean(level, parent)?;
        let correction = Correction {
            corners,
            bubble: (mean - linear / weights) / (bump / weights),
        };
        self.cache.corrections.insert(key, correction);
        self.cache.recent_correction[slot] = Some((key.1, correction));
        Ok(correction)
    }

    /// Each level's incision with its correction removed, from the first to `through` (and never
    /// past the deepest any coarse cell about the point evaluates), at the unit direction `dir`.
    fn levels(
        &mut self,
        dir: [f64; 3],
        through: u8,
    ) -> Result<PerLevel<LevelSample>, ReadCellError> {
        let first = self.first();
        let mut out = PerLevel::new(LevelSample::ZERO);
        if through < first {
            return Ok(out);
        }
        let finest = deepest_cell(dir);
        let coarse = ancestor(finest, first);
        // A cell under water carries no incision at any level (R09.T6.f).
        let shore = self.shore(coarse)?;
        if shore.wet {
            return Ok(out);
        }
        let (_, around) = self.deepest(coarse)?;
        let last = through.min(around);
        let raw = self.raw(dir, last)?;
        let by_water = (!shore.is_inland()).then(|| cell_coordinates(coarse, dir));
        for (level, &r) in (first..=last).zip(raw.as_slice()) {
            let parent = self.parent_of(ancestor(finest, level));
            let correction = self.correction(level, parent)?;
            let (x, y, grad_x, grad_y) = cell_coordinates(parent, dir);
            let (c, d_x, d_y) = correction.at(x, y);
            out.push(match by_water {
                None => LevelSample {
                    value: r.value - c,
                    of_point: r.gradient,
                    of_direction: [0, 1, 2].map(|k| -(d_x * grad_x[k] + d_y * grad_y[k])),
                },
                // The raw incision faded towards the water, f r, its gradient f ∇r through the
                // point and r ∇f through the direction.
                Some((cx, cy, fade_x, fade_y)) => {
                    let (f, f_x, f_y) = shore.fade(cx, cy, fade_width(first, level));
                    LevelSample {
                        value: f * r.value - c,
                        of_point: r.gradient.map(|g| f * g),
                        of_direction: [0, 1, 2].map(|k| {
                            r.value * (f_x * fade_x[k] + f_y * fade_y[k])
                                - (d_x * grad_x[k] + d_y * grad_y[k])
                        }),
                    }
                }
            });
        }
        Ok(out)
    }
}

/// A coarse cell's channel incision variance as the synthesis realises it and as its closed forms
/// count it ([`Channels::variance_check`]; feature `testing`): what R09.T6.b's and R09.T6.i's
/// constants were measured from.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone, PartialEq)]
pub struct VarianceCheck {
    /// The whole incision's variance over the cell, square metres.
    pub whole_m2: f64,
    /// Each evaluated level's own realised variance, from the coarse level, square metres.
    pub realised_m2: Vec<(u8, f64)>,
    /// Each level's closed-form variance, as the synthesis's terms count it (each finer level's
    /// already its variance share's), square metres.
    pub closed_m2: Vec<(u8, f64)>,
    /// The share of the levels' summed variances the whole realises in the closed form.
    pub whole_share: f64,
}

#[cfg(any(test, feature = "testing"))]
impl Channels {
    /// [`VarianceCheck`] of the coarse cell `coarse` of `field`: its incision sampled on an `n` ×
    /// `n` grid even in its face coordinates, weighted by area, against the closed forms
    /// [`drawn_channel_variance`] sums; `None` where `field` lacks a cell those read.
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] naming the first cell read that `field` does not hold.
    ///
    /// # Panics
    ///
    /// If `field`'s header is not of the body these channels were made for, `coarse` is not of the
    /// field's level, or `n` is zero.
    pub fn variance_check<F: FieldView>(
        &self,
        field: &F,
        coarse: PatchKey,
        n: u32,
        cache: &mut SynthCache,
    ) -> Result<Option<VarianceCheck>, ReadCellError> {
        self.check_field(field);
        assert!(n > 0, "a grid has a point");
        let Some((record, terms)) = synthesised_terms(field, coarse) else {
            return Ok(None);
        };
        let side = f64::from(1_u32 << coarse.level());
        let (mut sum, mut square, mut weights) = (0.0, 0.0, 0.0);
        let mut by_level = [(0.0, 0.0); LEVELS];
        let mut count = 0;
        let across = f64::from(n);
        for row in 0..n {
            for column in 0..n {
                let s = (f64::from(coarse.i()) + (f64::from(column) + 0.5) / across) / side;
                let t = (f64::from(coarse.j()) + (f64::from(row) + 0.5) / across) / side;
                let uv = FaceUv {
                    face: coarse.face(),
                    u: st_to_uv(s),
                    v: st_to_uv(t),
                };
                let along = 1.0 + uv.u * uv.u + uv.v * uv.v;
                // The area element of the warp and the gnomonic face.
                let weight = warp_rate(s) * warp_rate(t) / (along * along.sqrt());
                let levels = self
                    .incision(field, cache)
                    .levels(unit_dir(face_uv_to_xyz(uv)), MAX_LEVEL)?;
                let values = levels.as_slice();
                count = count.max(values.len());
                let total: f64 = values.iter().map(|sample| sample.value).sum();
                sum += weight * total;
                square += weight * total * total;
                weights += weight;
                for (level, sample) in by_level.iter_mut().zip(values) {
                    level.0 += weight * sample.value;
                    level.1 += weight * sample.value * sample.value;
                }
            }
        }
        let mean = sum / weights;
        let first = self.first_level();
        let realised_m2 = (first..)
            .zip(&by_level[..count])
            .map(|(level, (s, s2))| (level, s2 / weights - (s / weights) * (s / weights)))
            .collect();
        Ok(Some(VarianceCheck {
            whole_m2: square / weights - mean * mean,
            realised_m2,
            closed_m2: terms.iter().map(|t| (t.level, t.variance)).collect(),
            whole_share: whole_share(record),
        }))
    }
}

impl Channels {
    /// The channels' incision at the unit direction `dir`, levels [`first_level`](Self::first_level)
    /// to `through` (inclusive, and never past the deepest the cut evaluates about the point): its
    /// height, metres, and its gradient in body-fixed space at the spheroid point, metres per metre,
    /// as [`interp::base_elevation`](crate::synth::interp::base_elevation)'s is (the model is the
    /// [`incision`](self) module's).
    ///
    /// The levels are summed in level order, each its incision less its correction. `cache` is the
    /// caller's; the result is independent of what it holds (see [`SynthCache`]). With `through`
    /// below the first level the incision is zero and reads nothing.
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] naming the first cell read that `field` does not hold.
    ///
    /// # Panics
    ///
    /// If `field`'s header is not of the body these channels were made for, `dir` is zero or has a
    /// component that is not finite, or a height is not finite (a bug).
    pub fn incision_at<F: FieldView>(
        &self,
        field: &F,
        dir: [f64; 3],
        through: u8,
        cache: &mut SynthCache,
    ) -> Result<HeightSample, ReadCellError> {
        self.check_field(field);
        let levels = self.incision(field, cache).levels(dir, through)?;
        Ok(summed(field.header(), dir, levels.as_slice()))
    }

    /// [`incision_at`](Self::incision_at) through `through` and through `parent_through`, no
    /// finer, from one evaluation of the levels: each level's raw incision reads only coarser
    /// levels' and its correction none, and the sum takes them coarsest first, so the coarser sum
    /// is the finer's first levels', bit for bit.
    ///
    /// # Errors
    ///
    /// As [`incision_at`](Self::incision_at) through `through`.
    ///
    /// # Panics
    ///
    /// As [`incision_at`](Self::incision_at); and, in debug builds, if `parent_through` is above
    /// `through`.
    pub(crate) fn incision_pair_at<F: FieldView>(
        &self,
        field: &F,
        dir: [f64; 3],
        through: u8,
        parent_through: u8,
        cache: &mut SynthCache,
    ) -> Result<(HeightSample, HeightSample), ReadCellError> {
        debug_assert!(
            parent_through <= through,
            "a parent's levels through {parent_through} are not a prefix of those through {through}"
        );
        self.check_field(field);
        let levels = self.incision(field, cache).levels(dir, through)?;
        let levels = levels.as_slice();
        // The parent's levels: the first to `parent_through`, none below the first, of those
        // `levels` holds, which stop at the deepest the cut evaluates about the point.
        let parent_count = parent_through
            .checked_sub(self.level)
            .map_or(0, |n| (usize::from(n) + 1).min(levels.len()));
        let header = field.header();
        Ok((
            summed(header, dir, levels),
            summed(header, dir, &levels[..parent_count]),
        ))
    }

    /// Whether the unit direction `dir` lies on a channel's bed: within half its bankfull width of
    /// the axis of a segment whose groove the incision evaluates there, levels
    /// [`first_level`](Self::first_level) to `through` (R10.T1.b's channel floor, where the
    /// classifier lays `Sediment`). It reads what [`incision_at`](Self::incision_at) reads of the
    /// network, and nothing of the means; `cache` is the caller's, and the answer is independent of
    /// what it holds.
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] naming the first cell read that `field` does not hold.
    ///
    /// # Panics
    ///
    /// As [`incision_at`](Self::incision_at).
    pub(crate) fn on_channel_floor<F: FieldView>(
        &self,
        field: &F,
        dir: [f64; 3],
        through: u8,
        cache: &mut SynthCache,
    ) -> Result<bool, ReadCellError> {
        self.check_field(field);
        self.incision(field, cache).on_floor(dir, through)
    }

    /// The floor below which the channels take no point of the dry coarse cell `coarse` at the unit
    /// direction `dir` (R09.T6.g; decision-r09-channel-term §9.4): `None` where it is the sea level
    /// all over the cell, R09.T6.f's floor, and otherwise its height above the datum, metres, and
    /// its gradient in body-fixed space, metres per metre.
    ///
    /// It is bilinear in the cell's face coordinates between its corners, each the highest pit
    /// level of the cells that share it: a playa's pit's ground in the pit and in each cell that
    /// drains into it, and the sea level elsewhere and never below it. So it is continuous, the
    /// pit's ground or higher all over the pit and its donors, and it falls to the sea level
    /// across their other neighbours. `cache` is the caller's, and the answer is independent of
    /// what it holds.
    ///
    /// # Errors
    ///
    /// [`ReadCellError`] naming the first cell read that `field` does not hold: the cells sharing
    /// `coarse`'s corners and their receivers, within two king moves of it.
    pub(crate) fn floor_at<F: FieldView>(
        &self,
        field: &F,
        coarse: PatchKey,
        dir: [f64; 3],
        cache: &mut SynthCache,
    ) -> Result<Option<HeightSample>, ReadCellError> {
        self.check_field(field);
        let Some(corners) = self.incision(field, cache).floor_corners_m(coarse)? else {
            return Ok(None);
        };
        let (x, y, grad_x, grad_y) = cell_coordinates(coarse, dir);
        let (value, d_x, d_y) = Correction {
            corners,
            bubble: 0.0,
        }
        .at(x, y);
        let of_direction = [0, 1, 2].map(|k| d_x * grad_x[k] + d_y * grad_y[k]);
        Ok(Some(HeightSample {
            height_m: num::assert_finite(value),
            gradient: combine(field.header(), dir, [0.0; 3], of_direction),
        }))
    }

    /// The incision's working state over `field` and `cache`.
    #[must_use]
    fn incision<'a, F: FieldView>(
        &'a self,
        field: &'a F,
        cache: &'a mut SynthCache,
    ) -> Incision<'a, F> {
        let cache = cache.channels_mut();
        cache.keep(self.id());
        Incision {
            channels: self,
            field,
            cache,
        }
    }
}

/// The incision at the unit direction `dir` of a body of header `header` from its `levels`, summed
/// in level order, each its incision less its correction ([`Channels::incision_at`]).
#[must_use]
fn summed(header: &FieldHeader, dir: [f64; 3], levels: &[LevelSample]) -> HeightSample {
    let mut height = 0.0;
    let mut of_point = [0.0; 3];
    let mut of_direction = [0.0; 3];
    for level in levels {
        height += level.value;
        for k in 0..3 {
            of_point[k] += level.of_point[k];
            of_direction[k] += level.of_direction[k];
        }
    }
    HeightSample {
        height_m: num::assert_finite(height),
        gradient: combine(header, dir, of_point, of_direction),
    }
}

/// The body-fixed gradient per metre at the spheroid point of `dir` of a sum of a function of the
/// point, of gradient `of_point`, and a function of direction, of gradient `of_direction` on the
/// unit sphere per radian.
#[must_use]
fn combine(
    header: &FieldHeader,
    dir: [f64; 3],
    of_point: [f64; 3],
    of_direction: [f64; 3],
) -> [f64; 3] {
    let figure = header.figure();
    let radii = [
        figure.equatorial_radius_m,
        figure.equatorial_radius_m,
        figure.polar_radius_m,
    ];
    let point = as_function_of_direction(header, dir, of_point);
    [0, 1, 2].map(|k| num::assert_finite(point[k] + of_direction[k] / radii[k]))
}

/// The share of a finer level's cells whose key point heads a tributary where its coarse cell
/// carries tributaries, 0.75: three of a parent's four children draw a key point of their own, and
/// on the Earth- and Mars-like worlds none of those lay on the segment it would join (R09.T6.b's
/// measurement, levels L + 1 to L + 4 below 12 coarse cells each).
pub const TRIBUTARY_SHARE: f64 = 0.75;

/// A tributary's mean length, in mean cells of its level, from its key point to its join: 0.65,
/// measured as [`TRIBUTARY_SHARE`] was (0.58 to 0.70 by level and world).
pub const MEAN_TRIBUTARY_LENGTH: f64 = 0.65;

/// A trunk's mean length, in mean coarse cells, through its crossing to its receiver's key point:
/// 1.0, measured as [`TRIBUTARY_SHARE`] was (1.00 on the Earth-like world, 1.12 on the Mars-like).
pub const MEAN_TRUNK_LENGTH: f64 = 1.0;

/// A world dry now's bent trunks' mean length over their straight chords', `s_net`: 1.05, the mean
/// over the reference Mars's and the Mars-like world's trunks, 1.0535 and 1.0505 (R09.T6.h's
/// measurement, as [`MEAN_TRUNK_LENGTH`] was taken), by which a dry-now cell's trunk is longer than
/// [`MEAN_TRUNK_LENGTH`] in the closed forms and in the coarse pass's closure (the bend's
/// [`BEND_FACTOR`](super::BEND_FACTOR) and spline).
pub const EPOCH_TRUNK_SINUOSITY: f64 = 1.05;

/// The share of a finer level's closed-form groove variance that its own incision realises, 0.46:
/// a groove inside a deeper, coarser one cuts nothing there, which the closed form, taking each
/// level's grooves alone, does not see. It is the net of that and the closed form's shallower β (the
/// rise from the heads to a tributary's middle, where the realised bed lies deeper below its top by
/// the rise from its join to the coarse cell's main stream). The median of the ratio over 24 levels
/// of seven coarse cells of the Earth- and Mars-like worlds, 0.25 to 0.93 (R09.T6.b's measurement;
/// with the heads' slope limit and the 5 × 5 search, the levels' summed variance came within
/// −10% to +17% of the closed form's on nine of ten Earth-like cells). Provisional.
pub const TRIBUTARY_VARIANCE_SHARE: f64 = 0.46;

/// The variance of the levels' summed incision as a share of the sum of their own variances, 0.52:
/// the levels anticorrelate, since a finer level cuts most where the coarser ones cut least and
/// little inside their deeper grooves. The median of the ratio over 20 coarse cells of the Earth-
/// and Mars-like worlds, 0.27 to 1.38 (R09.T6.b's measurement on 32 × 32 grids). Provisional.
pub const WHOLE_VARIANCE_SHARE: f64 = 0.52;

/// A tributary's mean length under R09.T6.i's rules, in mean cells of its level, from its key point
/// along its bent course to its join: 0.71, the mean over levels L + 1 to L + 4 below 12 coarse
/// cells each of the synthetic Earth-like world, the reference Earth and the locked Earth, 0.706,
/// 0.700 and 0.715 (0.58 to 0.77 by level and world; R09.T6.i's measurement, as
/// [`MEAN_TRIBUTARY_LENGTH`] was taken): the join moved along its parent by the junction angle
/// ([`JUNCTION_ANGLE_COT`](super::JUNCTION_ANGLE_COT)) and the bend add about 1 ÷ sin 72° and its
/// `s_net`.
pub const MEAN_BENT_TRIBUTARY_LENGTH: f64 = 0.71;

/// A world wet now's bent trunks' mean length over their straight chords', `s_net`, under
/// R09.T6.i's rules: 1.05, over every dry trunk of the synthetic Earth-like world, the reference
/// Earth and the locked Earth, 1.0471, 1.0539 and 1.0539 (R09.T6.i's measurement), the same bend as
/// a world dry now's ([`EPOCH_TRUNK_SINUOSITY`]).
pub const WET_TRUNK_SINUOSITY: f64 = 1.05;

/// [`TRIBUTARY_VARIANCE_SHARE`] under R09.T6.i's rules, 0.82: the median over the three wet worlds'
/// tributary levels, 204 of them below 20 coarse cells each on 32 × 32 grids, of the realised
/// variance over the closed form's (0.77, 0.68 and 1.06 by world, p10 0.12 and p90 1.85 pooled;
/// R09.T6.i's measurement, as R09.T6.b's was, the coarse passes booked under it). Nearer 1 than
/// T6.b's: the accordant grooves cut below their parents' walls where T6.b's, half a cell wide,
/// buried them. Provisional.
pub const BENT_TRIBUTARY_VARIANCE_SHARE: f64 = 0.82;

/// [`WHOLE_VARIANCE_SHARE`] under R09.T6.i's rules, 0.91: the median over 58 coarse cells of the
/// three wet worlds of the whole incision's realised variance over the sum of its levels' (0.91,
/// 0.91 and 0.93 by world; R09.T6.i's measurement). The levels anticorrelate far less than T6.b's,
/// their walls 2.5 G across rather than half a cell, so that a finer level's grooves seldom lie
/// inside a coarser one's. Provisional.
pub const BENT_WHOLE_VARIANCE_SHARE: f64 = 0.91;

/// The share of the sum of a coarse cell's levels' variances that their summed incision realises:
/// [`BENT_WHOLE_VARIANCE_SHARE`] where the levels anticorrelate (R09.T6.i), and 1 on a world dry
/// now's cell, whose one level, its epoch's trunk, has no other to anticorrelate with (R09.T6.c).
#[must_use]
fn whole_share(cell: &SynthesisCell) -> f64 {
    if cell.epoch_incision.is_epoch() {
        1.0
    } else {
        BENT_WHOLE_VARIANCE_SHARE
    }
}

/// One level's closed-form incision statistics over a coarse cell.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LevelTerm {
    level: u8,
    /// The incision's variance, square metres.
    variance: f64,
    /// The mean square of its derivative along a random direction, metres² a metre².
    slope2: f64,
}

/// The closed-form statistics of a level's grooves: `density` metres of groove a square metre, of
/// depth `depth`, floor `floor` and wall `wall` (metres).
#[must_use]
fn level_term(level: u8, density: f64, depth: f64, floor: f64, wall: f64) -> LevelTerm {
    // Across a groove, each side, ∫ f dx = h + W ÷ 2, ∫ f² dx = h + 13W ÷ 35 and ∫ f'² dx =
    // 6 ÷ (5W) for the smoothstep wall.
    let mean = density * 2.0 * depth * (floor + 0.5 * wall);
    let square = density * 2.0 * depth * depth * (floor + 13.0 * wall / 35.0);
    // The gradient is across the groove; along a random direction its square averages a half.
    let slope2 = density * depth * depth * 1.2 / wall;
    LevelTerm {
        level,
        variance: num::max(square - mean * mean, 0.0),
        slope2,
    }
}

/// The valley a closed form gives a coarse cell's steady-state trunk.
#[derive(Debug, Clone, PartialEq)]
enum TrunkValley {
    /// R09.T6.b's, from the cell alone and not held to its base level: [`channel_variance`]'s,
    /// kept to the bit (R09.T6.f).
    FromTheCell,
    /// As the synthesis cuts it (R09.T6.f, R09.T6.g): the halves of the trunks in the cell, its
    /// own and its donors', each held to its base level ([`trunk_valley_m`]).
    Drawn {
        /// The trunks' halves in the cell, its own first, then its donors' in [`Edge::ALL`]'s
        /// order.
        halves: Vec<TrunkHalf>,
        /// How much the cell's own trunk's valley is held, metres: what its tributaries' depths
        /// below their inherited top lose.
        held_by_m: f64,
        /// The cell's own trunk's groove depth G, metres, zero where it heads none: the depth at
        /// which its first level of tributaries joins it under R09.T6.i's rules.
        own_m: f64,
    },
    /// A world dry now's, as the synthesis cuts it (R09.T6.h): the cell's own epoch valley, its
    /// second chord graded to its receiver's valley depth, metres.
    Epoch { receiver_m: f64 },
}

/// One half of a steady-state trunk's groove in a coarse cell, as the synthesis's closed forms
/// count it (R09.T6.g): the cell's own trunk from its key point to its crossing, or a donor's from
/// its crossing to the cell's key point, each [`MEAN_TRUNK_LENGTH`] ÷ 2 mean cells long.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TrunkHalf {
    /// Its groove's depth G at its axis, metres ([`trunk_depth_m`]).
    depth_m: f64,
    /// The depth its groove grades to along it, metres: `depth_m` but for a donor's half under
    /// R09.T6.i's rules, graded to the cell's own trunk's valley (its (d)).
    end_m: f64,
    /// Half its channel's bankfull width, metres.
    floor_m: f64,
    /// The fade along it: none, or [`INTO_WATER_FADE`] for the cell's own trunk into water.
    fade: Fade,
}

/// A fade's mean along a groove, ⟨S⟩, and its mean square, ⟨S²⟩, by which a closed form scales
/// the groove's mean and its square.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fade {
    /// ⟨S⟩.
    mean: f64,
    /// ⟨S²⟩.
    mean_square: f64,
}

/// No fade: ⟨S⟩ = ⟨S²⟩ = 1.
const UNFADED: Fade = Fade {
    mean: 1.0,
    mean_square: 1.0,
};

/// The fade's mean and mean square along the cell's own trunk into a cell under water, ⟨S⟩ = ½ and
/// ⟨S²⟩ = 13 ÷ 35: S(x) the cubic smoothstep of x = d ÷ w from its key point, taken at the cell's
/// centre (x = 1, d half the cell and the trunk level's fade width w half the cell), to its
/// crossing on the shore (x = 0); ∫₀¹ S = ½ and ∫₀¹ S² = 13 ÷ 35.
const INTO_WATER_FADE: Fade = Fade {
    mean: 0.5,
    mean_square: 13.0 / 35.0,
};

/// Each evaluated level's closed-form statistics below the coarse cell `cell` of `body`, its trunk's
/// valley from the cell alone (see [`channel_variance`]).
#[must_use]
fn level_terms(cell: &SynthesisCell, body: ChannelBody) -> Vec<LevelTerm> {
    level_terms_to(
        cell,
        body,
        drawn_deepest_level(cell, body),
        &TrunkValley::FromTheCell,
    )
}

/// Each evaluated level's closed-form statistics below the coarse cell `key` of `field` as the
/// synthesis cuts them, with the cell's record ([`drawn_terms`] over the field's records and its
/// header's sea level and body).
#[must_use]
fn synthesised_terms(
    field: &impl FieldView,
    key: PatchKey,
) -> Option<(&SynthesisCell, Vec<LevelTerm>)> {
    let header = field.header();
    drawn_terms(
        &|cell| field.cell(cell),
        key,
        header.sea_level(),
        ChannelBody::of(header),
    )
}

/// Each evaluated level's closed-form statistics below the coarse cell `key`, read from `cells`, as
/// the synthesis cuts them on a body of `body` and sea level `sea_level`, with the cell's record
/// (R09.T6.f, R09.T6.g; decision-r09-channel-term §4 and §9).
///
/// There are none where the cell is under water. Its trunk level counts the trunks that run in it:
/// the first half of its own, faded towards a receiver under water ([`INTO_WATER_FADE`]), and the
/// second half of each dry edge neighbour's whose flow names it, each held to its own base level
/// ([`trunk_valley_m`]); its tributaries' depths below their inherited top follow its own trunk's
/// hold. A world dry now's cell takes its own epoch trunk whole, its course bent
/// ([`EPOCH_TRUNK_SINUOSITY`]) and its second half graded to its receiver's valley
/// ([`epoch_trunk_term`]; R09.T6.h). `None` where `cells` lacks the cell, its receiver, or, where
/// water drains through it, one of its edge neighbours, one king move away, inside the synthesis's
/// margin.
#[must_use]
fn drawn_terms<'a>(
    cells: &impl Fn(PatchKey) -> Option<&'a SynthesisCell>,
    key: PatchKey,
    sea_level: Metres,
    body: ChannelBody,
) -> Option<(&'a SynthesisCell, Vec<LevelTerm>)> {
    let record = cells(key)?;
    if record.is_under_water() {
        return Some((record, Vec::new()));
    }
    let last = drawn_deepest_level(record, body);
    if record.epoch_incision.is_epoch() {
        // Its trunk's depth grades to its receiver's valley (R09.T6.h).
        let receiver_m = match record.flow.edge() {
            Some(edge) => receiver_valley_m(cells(key.edge_neighbour(edge))?, body),
            None => 0.0,
        };
        return Some((
            record,
            level_terms_to(record, body, last, &TrunkValley::Epoch { receiver_m }),
        ));
    }
    let cap = groove_cap_m(body.radius.value(), body.level);
    let mut halves = Vec::new();
    let mut held_by_m = 0.0;
    // The valley a donor's trunk grades to under R09.T6.i's rules: the cell's own, or none where it
    // heads no trunk (its (d)).
    let mut own_valley_m = 0.0;
    let mut own_m = 0.0;
    if let Some(edge) = record.flow.edge() {
        let receiver = cells(key.edge_neighbour(edge))?;
        let valley = trunk_valley_m(record, receiver, sea_level, body);
        held_by_m = tributary_steepness_of(record)
            .map_or(0.0, |k| valley_depth(k, body.radius, body.level).value())
            - valley;
        let fade = if receiver.is_under_water() {
            INTO_WATER_FADE
        } else {
            UNFADED
        };
        let own = trunk_half(record, valley, Grade::Level, cap, body, fade);
        own_m = own.map_or(0.0, |half| half.depth_m);
        halves.extend(own);
        if record.catchment_runoff != crate::field::LogRunoff::NONE {
            own_valley_m = valley;
        }
    }
    let donors_grade = Grade::To {
        valley_m: own_valley_m,
    };
    // A cell no water drains through, on a field never routed, has no donor to read.
    let donors: &[Edge] = if record.drainage == crate::field::LogArea::ZERO {
        &[]
    } else {
        &Edge::ALL
    };
    for &edge in donors {
        let (neighbour, back) = key.edge_neighbour_and_back(edge);
        let donor = cells(neighbour)?;
        if donor.is_under_water()
            || donor.epoch_incision.is_epoch()
            || donor.flow.edge() != Some(back)
        {
            continue;
        }
        let valley = trunk_valley_m(donor, record, sea_level, body);
        halves.extend(trunk_half(donor, valley, donors_grade, cap, body, UNFADED));
    }
    Some((
        record,
        level_terms_to(
            record,
            body,
            last,
            &TrunkValley::Drawn {
                halves,
                held_by_m,
                own_m,
            },
        ),
    ))
}

/// How a trunk's half runs along it in a cell's closed forms.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Grade {
    /// At its own valley's depth all along it.
    Level,
    /// Graded by the smoothstep to the valley `valley_m` deep (metres): a donor's half under
    /// R09.T6.i's rules, into the cell's own valley (its (d)).
    To {
        /// The depth of the valley it grades to, metres.
        valley_m: f64,
    },
}

/// The half in a cell of the steady-state trunk of the dry coarse cell `cell`, whose valley is
/// `valley_m` deep and runs along it as `grade` says, on a body of `body` whose trunks' grooves are
/// capped at `cap_m`, faded by `fade`; `None` where it cuts nothing, its catchment shedding no
/// runoff.
#[must_use]
fn trunk_half(
    cell: &SynthesisCell,
    valley_m: f64,
    grade: Grade,
    cap_m: f64,
    body: ChannelBody,
    fade: Fade,
) -> Option<TrunkHalf> {
    let (_, width, depth) = channel_size(
        cell.catchment_runoff.metres_per_year(),
        cell.drainage.area(),
        body.gravity,
    );
    let g = trunk_depth_m(valley_m, depth, cap_m);
    (g > 0.0).then_some(TrunkHalf {
        depth_m: g,
        end_m: match grade {
            Grade::Level => g,
            Grade::To { valley_m: to_m } => trunk_depth_m(to_m, depth, cap_m),
        },
        floor_m: 0.5 * width,
        fade,
    })
}

/// The trunk level's closed-form statistics of `halves`, each [`MEAN_TRUNK_LENGTH`] ÷ 2 mean cells
/// of groove in a cell of `cell_m` mean width whose walls are `wall_m` wide, at `level`: the sums
/// of each half's ∫ g and ∫ g² across it and of its slope's mean square, as [`level_term`] takes
/// one groove's, each times its fade's mean or mean square, summed in their order.
#[must_use]
fn trunk_term(level: u8, halves: &[TrunkHalf], cell_m: f64, wall_m: f64) -> LevelTerm {
    let density = 0.5 * MEAN_TRUNK_LENGTH / cell_m;
    let (mut mean, mut square, mut slope2) = (0.0, 0.0, 0.0);
    for half in halves {
        let (g, h) = (half.depth_m, half.floor_m);
        let Fade {
            mean: fade,
            mean_square: fade2,
        } = half.fade;
        // [`level_term`]'s integrals across a groove, each side.
        mean += fade * density * 2.0 * g * (h + 0.5 * wall_m);
        square += fade2 * density * 2.0 * g * g * (h + 13.0 * wall_m / 35.0);
        slope2 += fade2 * density * g * g * 1.2 / wall_m;
    }
    LevelTerm {
        level,
        variance: num::max(square - mean * mean, 0.0),
        slope2,
    }
}

/// The trunk level's closed-form statistics of `halves` under R09.T6.i's rules, each
/// [`MEAN_TRUNK_LENGTH`] × [`WET_TRUNK_SINUOSITY`] ÷ 2 mean cells of bent groove in a cell of
/// `cell_m` mean width, whose walls are 2.5 G across ([`WET_WALL_PER_DEPTH`]; its (b)) and whose
/// depth grades from its `depth_m` to its `end_m` by the smoothstep ([`Moments`]; its (d)): with
/// W = aG each side, ∫ g = 2h⟨G⟩ + a⟨G²⟩, ∫ g² = 2h⟨G²⟩ + 26a⟨G³⟩ ÷ 35 and the slope's mean square
/// 1.2⟨G⟩ ÷ a, each times its fade's mean or mean square, summed in their order. Every G is at
/// most the trunk's cap, where 2.5 G meets the half cell, so no wall is capped.
#[must_use]
fn wet_trunk_term(level: u8, halves: &[TrunkHalf], cell_m: f64) -> LevelTerm {
    let density = 0.5 * MEAN_TRUNK_LENGTH * WET_TRUNK_SINUOSITY / cell_m;
    let (mut mean, mut square, mut slope2) = (0.0, 0.0, 0.0);
    for half in halves {
        let m = Moments::of(half.depth_m, half.end_m);
        let h = half.floor_m;
        let Fade {
            mean: fade,
            mean_square: fade2,
        } = half.fade;
        let across = wet_across(h, m);
        mean += fade * density * across.mean;
        square += fade2 * density * across.square;
        slope2 += fade2 * density * across.slope2;
    }
    LevelTerm {
        level,
        variance: num::max(square - mean * mean, 0.0),
        slope2,
    }
}

/// A wet-now groove's integrals across it, both sides, averaged along it, under R09.T6.i's rules:
/// its walls W = aG ([`WET_WALL_PER_DEPTH`]), its floor `h` either side of the axis and its depth's
/// moments `m` along it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Across {
    /// ⟨∫ g⟩ = 2h⟨G⟩ + a⟨G²⟩, square metres.
    mean: f64,
    /// ⟨∫ g²⟩ = 2h⟨G²⟩ + 26a⟨G³⟩ ÷ 35, cubic metres.
    square: f64,
    /// ⟨∫ g′²⟩ ÷ 2, the slope's mean square along a random direction, 1.2⟨G⟩ ÷ a, metres.
    slope2: f64,
}

/// [`Across`] of a groove of floor `h` and depth moments `m`: [`level_term`]'s integrals with
/// W = aG, polynomials in G.
#[must_use]
fn wet_across(h: f64, m: Moments) -> Across {
    let a = WET_WALL_PER_DEPTH;
    Across {
        mean: 2.0 * h * m.mean + a * m.mean_square,
        square: 2.0 * h * m.mean_square + 26.0 * a * m.mean_cube / 35.0,
        slope2: 1.2 * m.mean / a,
    }
}

/// The trunk level's closed-form statistics of a world dry now's trunk of `density` metres a
/// square metre, its valley `valley` over its first half and graded to its receiver's valley depth
/// `receiver_m` over its second (R09.T6.h): each half [`level_term`]'s integrals across it, the
/// second's mean along it ([`graded_mean`]), at half the density each, summed in that order. The
/// slope's mean square is across the groove alone: the depth's change along the course adds about
/// 8 (Δ ÷ ℓ₂)² of it, under 10⁻³ for 200 m over a 20 km chord.
#[must_use]
fn epoch_trunk_term(level: u8, density: f64, valley: &EpochValley, receiver_m: f64) -> LevelTerm {
    let (g, h, wall) = (
        valley.depth().value(),
        0.5 * valley.floor_width().value(),
        valley.wall().value(),
    );
    let cap = valley.wall_cap.value();
    let half = 0.5 * density;
    // [`level_term`]'s integrals across a groove, each side.
    let mean_across = |g: f64, w: f64| 2.0 * g * (h + 0.5 * w);
    let square_across = |g: f64, w: f64| 2.0 * g * g * (h + 13.0 * w / 35.0);
    let slope_across = |g: f64, w: f64| g * g * 1.2 / w;
    let graded = |integrand: &dyn Fn(f64, f64) -> f64, uncapped: &dyn Fn(Moments) -> f64| {
        graded_mean(g, receiver_m, cap, integrand, uncapped)
    };
    // With W = 5G: 2G (h + 2.5G), 2G² (h + 13G ÷ 7) and 1.2 G ÷ 5.
    let mean = half * mean_across(g, wall)
        + half
            * graded(&|g, w| mean_across(g, w), &|m| {
                2.0 * h * m.mean + WALL_RUN_PER_DEPTH * m.mean_square
            });
    let square = half * square_across(g, wall)
        + half
            * graded(&|g, w| square_across(g, w), &|m| {
                2.0 * h * m.mean_square + 26.0 * m.mean_cube / 7.0
            });
    let slope2 = half * slope_across(g, wall)
        + half
            * graded(
                &|g, w| if g > 0.0 { slope_across(g, w) } else { 0.0 },
                &|m| 1.2 * m.mean / WALL_RUN_PER_DEPTH,
            );
    LevelTerm {
        level,
        variance: num::max(square - mean * mean, 0.0),
        slope2,
    }
}

/// [`level_terms`] through the tributary level `last` rather than the cut's deepest, with the
/// trunk's valley `valley`.
#[must_use]
fn level_terms_to(
    cell: &SynthesisCell,
    body: ChannelBody,
    last: u8,
    valley: &TrunkValley,
) -> Vec<LevelTerm> {
    body.check();
    if !cell.epoch_incision.is_epoch() {
        return bent_level_terms_to(cell, body, last, valley);
    }
    let (r, level) = (body.radius.value(), body.level);
    let mut out = Vec::new();
    let wall = VALLEY_WALL_SHARE * mean_cell_width(r, level);
    // The trunk level: its donors' halves count in a cell with no trunk of its own (R09.T6.g).
    if let TrunkValley::Drawn { halves, .. } = valley
        && cell.flow.edge().is_none()
    {
        if !halves.is_empty() {
            out.push(trunk_term(level, halves, mean_cell_width(r, level), wall));
        }
        return out;
    }
    if cell.flow.edge().is_none() {
        return out;
    }
    // A world dry now's: its trunk's epoch valley alone ([`EpochValley`]), its course bent
    // ([`EPOCH_TRUNK_SINUOSITY`]) and, as the synthesis cuts it, graded to its receiver's
    // (R09.T6.h).
    if let Some(epoch) = epoch_valley(cell, body)
        && epoch.depth().value() > 0.0
    {
        let density = MEAN_TRUNK_LENGTH * EPOCH_TRUNK_SINUOSITY / mean_cell_width(r, level);
        out.push(match valley {
            TrunkValley::Epoch { receiver_m } => {
                epoch_trunk_term(level, density, &epoch, *receiver_m)
            }
            TrunkValley::FromTheCell | TrunkValley::Drawn { .. } => level_term(
                level,
                density,
                epoch.depth().value(),
                0.5 * epoch.floor_width().value(),
                epoch.wall().value(),
            ),
        });
    }
    out
}

/// [`level_terms_to`] under R09.T6.i's rules on a world wet now: the trunk level's bent grooves,
/// their walls 2.5 G and the donors' halves graded to the cell's own valley ([`wet_trunk_term`]);
/// and each tributary level's grooves along their accordant profile, from the depth at their join,
/// the parent level's mean G (the trunk's own at L + 1), down to their heads' relief, the rise from
/// the cell's channel heads to the level's main stream (or their own rise less, where that is the
/// less: φ = 1), each held to the channel's depth and the level's cap, with ⟨Gⁿ⟩ of a linear
/// profile ([`Moments::linear`]) and walls 2.5 G ([`wet_across`]); [`MEAN_BENT_TRIBUTARY_LENGTH`]
/// mean cells of groove each, weighted by the continuous cut, clamp(ℓ − k + 1, 0, 1) of the cell's
/// own [`cut_level`] ℓ (its (e)), and [`BENT_TRIBUTARY_VARIANCE_SHARE`] of their variance realised.
#[must_use]
fn bent_level_terms_to(
    cell: &SynthesisCell,
    body: ChannelBody,
    last: u8,
    valley: &TrunkValley,
) -> Vec<LevelTerm> {
    let (r, level) = (body.radius.value(), body.level);
    let cell_m = mean_cell_width(r, level);
    let mut out = Vec::new();
    let tributaries = tributary_steepness_of(cell);
    // The trunk level, and the depth its first tributaries join it at.
    let trunk_m = match valley {
        TrunkValley::FromTheCell => {
            if cell.flow.edge().is_none() {
                return out;
            }
            let (_, width, depth) = channel_size(
                cell.catchment_runoff.metres_per_year(),
                cell.drainage.area(),
                body.gravity,
            );
            let valley = tributaries.map_or(0.0, |k| valley_depth(k, body.radius, level).value());
            let g = trunk_depth_m(valley, depth, groove_cap_m(r, level));
            if g > 0.0 {
                let half = TrunkHalf {
                    depth_m: g,
                    end_m: g,
                    floor_m: 0.5 * width,
                    fade: UNFADED,
                };
                out.push(wet_trunk_term(level, &[half, half], cell_m));
            }
            g
        }
        TrunkValley::Drawn { halves, own_m, .. } => {
            if !halves.is_empty() {
                out.push(wet_trunk_term(level, halves, cell_m));
            }
            if cell.flow.edge().is_none() {
                return out;
            }
            *own_m
        }
        TrunkValley::Epoch { .. } => unreachable!("a world dry now's cell takes R09.T6.h's forms"),
    };
    let Some(steepness) = tributaries else {
        return out;
    };
    let runoff = cell.runoff.metres_per_year();
    let cut = cut_level(cell, body);
    let mut join_m = trunk_m;
    for k in level + 1..=last.min(MAX_LEVEL) {
        let cell_width = mean_cell_width(r, k);
        let cap = groove_cap_m(r, k);
        let length = MEAN_BENT_TRIBUTARY_LENGTH * cell_width;
        let head = head_length(body.radius, k).value();
        let relief = super::head_relief_m(steepness, head);
        let area = super::hack_area(Metres::new(head + length));
        let (_, width, depth) = channel_size(runoff, area, body.gravity);
        let kept = |g: f64| num::min(num::max(g, depth), cap);
        let at_head = num::min(relief, join_m - rise(steepness, head, head + length));
        let profile = Moments::linear(kept(at_head), kept(join_m));
        let weight = super::cut_weight(cut, k);
        let density = TRIBUTARY_SHARE * length / (cell_width * cell_width);
        let across = wet_across(0.5 * width, profile);
        let mean = weight * density * across.mean;
        let square = weight * weight * density * across.square;
        let slope2 = weight * weight * density * across.slope2;
        out.push(LevelTerm {
            level: k,
            variance: BENT_TRIBUTARY_VARIANCE_SHARE * num::max(square - mean * mean, 0.0),
            slope2: BENT_TRIBUTARY_VARIANCE_SHARE * slope2,
        });
        join_m = profile.mean;
    }
    out
}

/// A level's share of the band of wavelengths shorter than `finer_than` metres: none at its mean
/// cell's width and below, all from twice it, log-linear between.
#[must_use]
fn band_share(radius_m: f64, level: u8, finer_than: f64) -> f64 {
    let width = mean_cell_width(radius_m, level);
    num::max(0.0, num::min(math::log2(finer_than / width), 1.0))
}

/// The channels' incision variance below the coarse cell `cell` of `body`, in wavelengths shorter
/// than `finer_than`, square metres, from the cell's record alone.
///
/// It is the closed form of each evaluated level's grooves, summed in level order (the
/// [module documentation](self) gives the model it estimates). The coarse pass's `σ_h` budget read
/// it until R09.T6.g, and books [`drawn_channel_variance`] since.
///
/// Level k's grooves, [`TRIBUTARY_SHARE`] × [`MEAN_TRIBUTARY_LENGTH`] mean cells of groove a cell
/// (one trunk of [`MEAN_TRUNK_LENGTH`] coarse cells at the first level), of depth G = min(max(β,
/// d), `G_max`) with β the stream-power rise from the channel heads to the main stream at a
/// tributary's middle, carry the variance 2G² ρ (h + 13W ÷ 35) − (2G ρ (h + W ÷ 2))², ρ their
/// length a unit area, h half the bankfull width and W the wall width. Each level is counted in the
/// band from its mean cell's width to twice it. A finer level realises [`TRIBUTARY_VARIANCE_SHARE`]
/// of its own closed form, and their sum's variance is [`WHOLE_VARIANCE_SHARE`] of the sum of
/// theirs. On a world dry now the cell's one term is its trunk's epoch valley ([`EpochValley`]: G
/// its depth, h half its bankfull width and W its walls), taken whole (R09.T6.c), its course
/// [`EPOCH_TRUNK_SINUOSITY`] times as long (R09.T6.h); the cell alone does not give its receiver,
/// so its depth is G all along it. The result is an estimate for the coarse pass's budget, not a
/// pinned law. The synthesis's own form, [`drawn_channel_variance`], is within a factor of two of
/// the whole incision's realised variance below a coarse cell (R09.T6.b's test on the Earth- and
/// Mars-like worlds, which reads that form).
///
/// It is R09.T6.b's form, from the cell alone: its trunk's valley is the cell's [`valley_depth`],
/// not held to its base level, its trunk counted whole in its own cell, and a cell under water
/// carries its trunk's bankfull channel. The synthesis cuts none of that
/// ([`Channels::incision_at`]); its own closed forms, which read the receiver and the donors, are
/// [`drawn_channel_variance`], [`channel_structure_function`]'s and the band levels' omitted
/// variance ([`Synthesiser::omitted_variance`](crate::synth::Synthesiser::omitted_variance)).
///
/// # Panics
///
/// If `body`'s radius or gravity is not finite and positive, its level is above [`MAX_LEVEL`], or
/// `finer_than` is not finite and positive.
#[must_use]
pub fn channel_variance(
    cell: &SynthesisCell,
    body: ChannelBody,
    finer_than: Metres,
) -> SquareMetres {
    variance_of(cell, body, &level_terms(cell, body), finer_than)
}

/// The variance in wavelengths shorter than `finer_than` of `terms`, the closed-form level terms
/// of the coarse cell `cell` of `body` ([`channel_variance`]).
///
/// # Panics
///
/// If `finer_than` is not finite and positive.
#[must_use]
fn variance_of(
    cell: &SynthesisCell,
    body: ChannelBody,
    terms: &[LevelTerm],
    finer_than: Metres,
) -> SquareMetres {
    let w = finer_than.value();
    assert!(
        w.is_finite() && w > 0.0,
        "a wavelength must be finite and positive, got {w}"
    );
    let r = body.radius.value();
    let mut variance = 0.0;
    for term in terms {
        variance += band_share(r, term.level, w) * term.variance;
    }
    SquareMetres::new(whole_share(cell) * variance)
}

/// The channels' share of the structure function of `field`'s ground at the coarse cell `cell` in
/// wavelengths shorter than `finer_than`, at a lag of `lag`, square metres: E[(h(p + r) − h(p))²]
/// of their incision as the synthesis cuts it; `None` where `field` does not hold what
/// [`drawn_channel_variance`] reads.
///
/// Each level contributes its band share of 2V (1 − exp(−s r² ÷ 2V)), with V its variance (the
/// synthesis's terms, below; [`channel_variance`]'s form) and s the mean square of its slope along
/// a random direction, ρ G² 1.2 ÷ W for its smoothstep walls: the small-lag law s r² and the
/// large-lag 2V of two independent samples; the sum is scaled by [`WHOLE_VARIANCE_SHARE`] as the
/// variance is (taken whole on a world dry now's cell, its epoch's trunk alone; R09.T6.c), so that
/// it saturates at twice the variance. The small-lag slope is the levels' sum scaled alike, an
/// approximation (the plan's Risks). The terms are the synthesis's, [`drawn_channel_variance`]'s
/// (R09.T6.f, R09.T6.g), where [`channel_variance`] reads the cell alone.
///
/// # Panics
///
/// If `finer_than` is not finite and positive, `lag` is negative or not finite, or `cell` is not of
/// the field's level.
#[must_use]
pub fn channel_structure_function(
    field: &impl FieldView,
    cell: PatchKey,
    finer_than: Metres,
    lag: Metres,
) -> Option<SquareMetres> {
    let (w, r) = (finer_than.value(), lag.value());
    assert!(
        w.is_finite() && w > 0.0 && r.is_finite() && r >= 0.0,
        "a wavelength must be finite and positive and a lag finite and non-negative, got {w} and \
         {r} m"
    );
    let header = field.header();
    assert_eq!(
        cell.level(),
        header.level().get(),
        "the cell is not of the field's level"
    );
    let (record, terms) = synthesised_terms(field, cell)?;
    let body = ChannelBody::of(header);
    let mut d = 0.0;
    for term in terms {
        if term.variance <= 0.0 {
            continue;
        }
        let share = band_share(body.radius.value(), term.level, w);
        let saturation = 2.0 * term.variance;
        d += share * saturation * (1.0 - math::exp(-term.slope2 * r * r / saturation));
    }
    Some(SquareMetres::new(whole_share(record) * d))
}

/// The channels' incision variance below `field`'s coarse cell `cell` in wavelengths shorter than
/// `finer_than` as the synthesis cuts it: [`drawn_channel_variance`] over the field's records, sea
/// level and body; `None` where `field` does not hold what that reads.
///
/// # Panics
///
/// If `finer_than` is not finite and positive, or `cell` is not of the field's level.
#[must_use]
pub(crate) fn unresolved_channel_variance(
    field: &impl FieldView,
    cell: PatchKey,
    finer_than: Metres,
) -> Option<SquareMetres> {
    let header = field.header();
    drawn_channel_variance(
        |key| field.cell(key),
        cell,
        header.sea_level(),
        ChannelBody::of(header),
        finer_than,
    )
}

/// The channels' incision variance below the coarse cell `cell` in wavelengths shorter than
/// `finer_than`, square metres, as the synthesis cuts it on a body of `body` whose sea level is
/// `sea_level`, read from the coarse records `cells`: [`channel_variance`]'s form over the
/// synthesis's own terms (R09.T6.f, R09.T6.g), which the coarse pass's `σ_h` budget books.
///
/// The terms are none in a cell under water; the trunk level counts the trunks that run in the
/// cell, its own trunk's first half and each dry donor's second half, each held to its base level
/// (`trunk_valley_m`), its own faded towards a receiver under water; and the tributary levels
/// are its own, below a top as much lower as its trunk is held. A world dry now's cell takes its
/// epoch trunk whole, its course [`EPOCH_TRUNK_SINUOSITY`] times as long and its second half
/// graded to its receiver's valley (R09.T6.h). `None` where `cells` lacks the cell, its receiver,
/// or, where water drains through it, one of its four edge neighbours.
///
/// # Panics
///
/// If `finer_than` is not finite and positive, `cell` is not of `body`'s coarse level, or `body` is
/// not a body.
#[must_use]
pub fn drawn_channel_variance<'a>(
    cells: impl Fn(PatchKey) -> Option<&'a SynthesisCell>,
    cell: PatchKey,
    sea_level: Metres,
    body: ChannelBody,
    finer_than: Metres,
) -> Option<SquareMetres> {
    assert_eq!(
        cell.level(),
        body.level,
        "the cell is not of the body's coarse level"
    );
    let (record, terms) = drawn_terms(&cells, cell, sea_level, body)?;
    Some(variance_of(record, body, &terms, finer_than))
}

#[cfg(test)]
mod tests;
