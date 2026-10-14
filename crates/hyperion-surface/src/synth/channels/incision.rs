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
//!   down to it ([`valley_depth`]); its top runs that far above its bed all along it.
//! - A tributary inherits the top where it joins, so its bed rises towards the top upstream, and its
//!   groove shallows to its channel alone where it has risen to it.
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
//! Level k > L is evaluated below a coarse cell only where the cell carries tributaries and the
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

use hyperion_base::math;
use hyperion_base::units::{Metres, MetresPerSecondSquared, SquareMetres};

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
/// natural angle of repose in many granular materials (∼ 30°)"; Grieve, Hurst and Mudd 2016
/// (Earth Surf. Dynam. 4, 309, `doi:10.5194/esurf-4-309-2016`) find 0.57 at Coweeta and 0.79–0.8 in the Oregon
/// Coast Range (as this task's science check read them). Low confidence as a valley-wall figure.
pub const VALLEY_WALL_GRADIENT: f64 = 0.6;

/// The narrowest channel a level is evaluated for: two band limits, 4 m, below which the band
/// limit cannot resolve a channel (Design note 13).
pub const CUT_WIDTH: Metres = Metres::new(2.0 * BAND_LIMIT_M);

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

/// [`deepest_level`] with channels narrower than `cut_m` metres cut, rather than [`CUT_WIDTH`]:
/// `f64::INFINITY` cuts every finer level, and 0 ends the network at the channel heads alone.
#[must_use]
fn deepest_level_cut(cell: &SynthesisCell, body: ChannelBody, cut_m: f64) -> u8 {
    body.check();
    let level = body.level;
    let Some(steepness) = tributary_steepness_of(cell) else {
        return level;
    };
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

/// How a groove's depth at its axis varies along its segment.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Depth {
    /// A trunk's: the same all along it.
    Constant(f64),
    /// A tributary's: min(max(top − bed(t), `floor_m`), `cap_m`), with the bed `bed_to_m` plus its
    /// stream-power rise from the join ([`rise`], as [`Segment::bed_at`] computes it).
    Tributary {
        top_m: f64,
        bed_to_m: f64,
        rise: RiseTo,
        head_m: f64,
        length_m: f64,
        floor_m: f64,
        cap_m: f64,
    },
}

impl Depth {
    /// The depth at parameter `t`, metres, and its derivative in t.
    #[must_use]
    fn at(&self, t: f64) -> (f64, f64) {
        match *self {
            Self::Constant(depth) => (depth, 0.0),
            Self::Tributary {
                top_m,
                bed_to_m,
                rise,
                head_m,
                length_m,
                floor_m,
                cap_m,
            } => {
                let main_stream = head_m + t * length_m;
                let below = top_m - (bed_to_m + rise.from(main_stream));
                if below <= floor_m {
                    (floor_m, 0.0)
                } else if below >= cap_m {
                    (cap_m, 0.0)
                } else {
                    // The bed falls by S ℓ a unit of t, so the depth below the top grows by it.
                    (below, rise.slope_at(main_stream) * length_m)
                }
            }
        }
    }
}

/// One segment's groove, ready for queries (see the module documentation, "The valleys").
#[derive(Debug, Clone, Copy, PartialEq)]
struct Groove {
    chords: [Chord; 2],
    chord_count: usize,
    depth: Depth,
    /// Half the channel's bankfull width, metres.
    floor_m: f64,
    /// The wall width W, metres.
    wall_m: f64,
    /// The box beyond which the groove is zero, body-fixed, metres.
    low: [f64; 3],
    high: [f64; 3],
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

impl Groove {
    /// The groove of `segment` of a body of mean radius `radius_m`, or `None` where it is
    /// nowhere deeper than zero.
    #[must_use]
    fn of(segment: &Segment, radius_m: f64) -> Option<Self> {
        let wall_m = VALLEY_WALL_SHARE * mean_cell_width(radius_m, segment.level());
        let cap_m = groove_cap_m(radius_m, segment.level());
        let floor_depth = segment.depth_m;
        let (chords, chord_count, depth) = match segment.kind {
            Kind::Trunk { valley_m, .. } => {
                let depth = num::min(num::max(valley_m, floor_depth), cap_m);
                let chords = segment.chords().expect("a trunk has two chords");
                (chords, 2, Depth::Constant(depth))
            }
            Kind::Tributary {
                steepness,
                head_m,
                length_m,
                top_m,
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
                };
                ([chord, chord], 1, depth)
            }
        };
        // A tributary's depth is at least its channel's, so only a groove with no channel and no
        // valley is empty.
        let deepest = match depth {
            Depth::Constant(d) => d,
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
            depth,
            floor_m,
            wall_m,
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
    /// channel's bankfull width of the nearest of its chords (R10.T1.b's channel floor).
    #[must_use]
    fn floor_holds(&self, q: [f64; 3]) -> bool {
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
fn corner_cells(cell: PatchKey) -> [Vec<PatchKey>; 4] {
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
        super::ChannelNetwork {
            channels: self.channels,
            field: self.field,
            memo: super::Memo::Cache(&mut self.cache.network),
        }
        .segment(cell)
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
        let body = self.channels.body();
        let own = deepest_level(read(self.field, coarse)?, body);
        let mut around = own;
        for cell in two_moves(coarse) {
            around = around.max(deepest_level(read(self.field, cell)?, body));
        }
        self.cache.deepest.insert(key, (own, around));
        Ok((own, around))
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
        let mut sum = 0.0;
        let mut weights = 0.0;
        for sample in mean_grid(parent) {
            let raw = self.raw(sample.dir, level)?;
            sum += sample.weight * raw.as_slice()[at].value;
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
        let (_, around) = self.deepest(ancestor(finest, first))?;
        let last = through.min(around);
        let raw = self.raw(dir, last)?;
        for (level, &r) in (first..=last).zip(raw.as_slice()) {
            let parent = self.parent_of(ancestor(finest, level));
            let correction = self.correction(level, parent)?;
            let (x, y, grad_x, grad_y) = cell_coordinates(parent, dir);
            let (c, d_x, d_y) = correction.at(x, y);
            out.push(LevelSample {
                value: r.value - c,
                of_point: r.gradient,
                of_direction: [0, 1, 2].map(|k| -(d_x * grad_x[k] + d_y * grad_y[k])),
            });
        }
        Ok(out)
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
        let mut height = 0.0;
        let mut of_point = [0.0; 3];
        let mut of_direction = [0.0; 3];
        for level in levels.as_slice() {
            height += level.value;
            for k in 0..3 {
                of_point[k] += level.of_point[k];
                of_direction[k] += level.of_direction[k];
            }
        }
        Ok(HeightSample {
            height_m: num::assert_finite(height),
            gradient: combine(field.header(), dir, of_point, of_direction),
        })
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

/// Each evaluated level's closed-form statistics below the coarse cell `cell` of `body` (see
/// [`channel_variance`]).
#[must_use]
fn level_terms(cell: &SynthesisCell, body: ChannelBody) -> Vec<LevelTerm> {
    level_terms_to(cell, body, deepest_level(cell, body))
}

/// [`level_terms`] through the tributary level `last` rather than the cut's deepest.
#[must_use]
fn level_terms_to(cell: &SynthesisCell, body: ChannelBody, last: u8) -> Vec<LevelTerm> {
    body.check();
    let (r, level) = (body.radius.value(), body.level);
    let mut out = Vec::new();
    if cell.flow.edge().is_none() {
        return out;
    }
    let tributaries = tributary_steepness_of(cell);
    // The trunk: one a coarse cell, its valley's depth all along it.
    let wall = VALLEY_WALL_SHARE * mean_cell_width(r, level);
    let cap = VALLEY_WALL_GRADIENT * wall / STEEPEST_WALL;
    let (_, width, depth) = channel_size(
        cell.catchment_runoff.metres_per_year(),
        cell.drainage.area(),
        body.gravity,
    );
    let valley = tributaries.map_or(0.0, |k| valley_depth(k, body.radius, level).value());
    let g = num::min(num::max(valley, depth), cap);
    if g > 0.0 {
        let density = MEAN_TRUNK_LENGTH / mean_cell_width(r, level);
        out.push(level_term(level, density, g, 0.5 * width, wall));
    }
    let Some(steepness) = tributaries else {
        return out;
    };
    let runoff = cell.runoff.metres_per_year();
    let heads = main_stream_length(channel_head_area(steepness)).value();
    for k in level + 1..=last.min(MAX_LEVEL) {
        let cell_width = mean_cell_width(r, k);
        let wall = VALLEY_WALL_SHARE * cell_width;
        let cap = VALLEY_WALL_GRADIENT * wall / STEEPEST_WALL;
        let length = MEAN_TRIBUTARY_LENGTH * cell_width;
        let head = head_length(body.radius, k).value();
        // Its bed's depth below the top at its middle: the relief from its heads down to it.
        let middle = head + 0.5 * length;
        let below = if middle > heads {
            rise(steepness, heads, middle)
        } else {
            0.0
        };
        let area = super::hack_area(Metres::new(head + length));
        let (_, width, depth) = channel_size(runoff, area, body.gravity);
        let g = num::min(num::max(below, depth), cap);
        let density = TRIBUTARY_SHARE * length / (cell_width * cell_width);
        let term = level_term(k, density, g, 0.5 * width, wall);
        out.push(LevelTerm {
            variance: TRIBUTARY_VARIANCE_SHARE * term.variance,
            slope2: TRIBUTARY_VARIANCE_SHARE * term.slope2,
            ..term
        });
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
/// than `finer_than`, square metres: the closed form of each evaluated level's grooves, summed in
/// level order (the [module documentation](self) gives the model it estimates).
///
/// Level k's grooves, [`TRIBUTARY_SHARE`] × [`MEAN_TRIBUTARY_LENGTH`] mean cells of groove a cell
/// (one trunk of [`MEAN_TRUNK_LENGTH`] coarse cells at the first level), of depth
/// G = min(max(β, d), `G_max`) with β the stream-power rise from the channel heads to the main
/// stream at a tributary's middle, carry the variance 2G² ρ (h + 13W ÷ 35) − (2G ρ (h + W ÷ 2))²,
/// ρ their length a unit area, h half the bankfull width and W the wall width. Each level is
/// counted in the band from its mean cell's width to twice it. A finer level realises
/// [`TRIBUTARY_VARIANCE_SHARE`] of its own closed form, and their sum's variance is
/// [`WHOLE_VARIANCE_SHARE`] of the sum of theirs. The result is an estimate for the coarse pass's
/// budget, within a factor of two of the whole incision's realised variance below a coarse cell
/// (R09.T6.b's test on the Earth- and Mars-like worlds), not a pinned law.
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
    let w = finer_than.value();
    assert!(
        w.is_finite() && w > 0.0,
        "a wavelength must be finite and positive, got {w}"
    );
    let r = body.radius.value();
    let mut variance = 0.0;
    for term in level_terms(cell, body) {
        variance += band_share(r, term.level, w) * term.variance;
    }
    SquareMetres::new(WHOLE_VARIANCE_SHARE * variance)
}

/// The channels' share of the structure function of `field`'s ground at the coarse cell `cell` in
/// wavelengths shorter than `finer_than`, at a lag of `lag`, square metres: E[(h(p + r) − h(p))²]
/// of their incision; `None` where `field` does not hold the cell.
///
/// Each level contributes its band share of 2V (1 − exp(−s r² ÷ 2V)), with V its variance
/// ([`channel_variance`]'s terms) and s the mean square of its slope along a random direction,
/// ρ G² 1.2 ÷ W for its smoothstep walls: the small-lag law s r² and the large-lag 2V of two
/// independent samples; the sum is scaled by [`WHOLE_VARIANCE_SHARE`] as the variance is, so that it
/// saturates at twice [`channel_variance`]. The small-lag slope is the levels' sum scaled alike, an
/// approximation (the plan's Risks).
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
    let record = field.cell(cell)?;
    let body = ChannelBody::of(header);
    let mut d = 0.0;
    for term in level_terms(record, body) {
        if term.variance <= 0.0 {
            continue;
        }
        let share = band_share(body.radius.value(), term.level, w);
        let saturation = 2.0 * term.variance;
        d += share * saturation * (1.0 - math::exp(-term.slope2 * r * r / saturation));
    }
    Some(SquareMetres::new(WHOLE_VARIANCE_SHARE * d))
}

/// [`channel_variance`] of `field`'s coarse cell `cell`, or `None` where `field` does not hold it.
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
    assert_eq!(
        cell.level(),
        header.level().get(),
        "the cell is not of the field's level"
    );
    let record = field.cell(cell)?;
    Some(channel_variance(
        record,
        ChannelBody::of(header),
        finer_than,
    ))
}

#[cfg(test)]
mod tests;
