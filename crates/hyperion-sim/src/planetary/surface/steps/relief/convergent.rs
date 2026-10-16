//! The convergent landforms (R09.T12.b; Design note 7, `decision-r09-t5.md` item 6 and
//! `decision-r09-t12-coarse.md` items 2 and 5d): collisional belts, trenches, and arcs with their
//! volcanic fronts, on a mobile lid's boundaries as R09.T11 classed them.
//!
//! - **Belts**, on continental crust whose nearest boundary is a collision: the population lifted
//!   by a plateau profile of the distance |d| to the boundary over a half-width of 100–250 km (a
//!   full width of 200–500 km), so that its mean stands at the plateau's [`BELT_PLATEAU`] times h.
//! - **Trenches**, on the subducting side of a subduction (where [`CellState::subducts`] holds):
//!   the floor deepened by 2–4 km times h at the boundary, falling off as (1 − |d| ÷ 100 km)².
//! - **Arcs**, on the overriding plate behind a subduction: a band from the volcanic front, G
//!   behind the trench's axis, to G + W, by the overriding crust, with G varied along strike
//!   (`decision-r09-t5.md` item 6). Since a cell 260 km behind a trench may have another boundary
//!   nearer, the arcs read their own distance from the trench: a shortest path over the overriding
//!   plate's cells ([`reach_from`]) from each overriding boundary cell of a subduction, started at
//!   half its link to its subducting neighbour across, the axis being the edge the two share; and
//!   the distance an arc reads is the great circle from the cell to its source's axis point, the
//!   two centres' normalised midpoint, so that the path's 8-connected excess does not reach G.
//!
//! The heights scale by the height factor h = g⊕ ÷ g ([`height_factor`], ruled), the distances and
//! widths not at all: Earth's, unscaled (labelled). The along-strike draw n is a level-5 octave of
//! the landform noise's lattice on instance [`STRIKE_INSTANCE`], made unit-RMS over the sphere and
//! clamped to ±[`STRIKE_CLAMP`]; it sets a belt's half-width and a trench's depth at the cell, and
//! an arc's G at its source trench cell, so that each profile across a trench takes one G.

use hyperion_surface::field::{BoundaryKind, Crust};
use hyperion_surface::num;

use super::super::plates::{cell_of, reach_from, slot};
use super::super::{CellState, Pass, Working};
use super::{CONTINENTAL_CORE_MEAN, noise};
use crate::math;
use crate::units::{Metres, MetresPerSecondSquared};

/// The reference Earth's surface gravity g⊕, GM ÷ R² = 3.986 004 418 × 10¹⁴ m³ s⁻² (IERS
/// Conventions 2010, TN 36, Table 1.1) over (6,371.0 km)², the reference Earth's radius: 9.820 25 m
/// s⁻², not standard gravity's 9.806 65. `reference::earth_like` reads it, so that its height
/// factor is 1 to the bit (`decision-r09-t12-coarse.md` item 2).
pub(crate) const EARTH_GRAVITY: MetresPerSecondSquared =
    MetresPerSecondSquared::new(3.986_004_418e14 / (6_371.0e3 * 6_371.0e3));

/// The height factor h = g⊕ ÷ g of a body of surface gravity `gravity`, 1 on the reference Earth
/// (`decision-r09-t12-coarse.md` item 2): collisional belts are crustal roots in Airy balance,
/// their height limited by the crust's strength against gravity, so 1 ÷ g, which scales the belts'
/// plateau and peaks, the trenches' depth below their floor and the arcs' heights, never distances
/// or widths. Design note 7's "× the lithosphere factor" is a plan error (ruled): that factor,
/// min(1, `T_e` ÷ 70 km), gates the volcanic loads alone (Design note 3), and would cut the
/// reference Earth's plateaus to 1.4 km. R09.T12.e multiplies the mobile lid's structural anchor by
/// it (`decision-r09-t5.md` item 3).
#[must_use]
pub(crate) fn height_factor(gravity: MetresPerSecondSquared) -> f64 {
    EARTH_GRAVITY.value() / gravity.value()
}

/// The along-strike draw's instance on `surface.coarse.relief`, 1.
pub(super) const STRIKE_INSTANCE: u16 = 1;

/// The along-strike draw's lattice level, 5 (6,144 cells, about 290 km across on an Earth), one
/// octave: arc segments of some 200–500 km vary on their own, and the medians gather some hundred
/// independent segments.
pub(super) const STRIKE_LEVEL: u8 = 5;

/// The along-strike draw's clamp, ±1.5 (`decision-r09-t5.md` item 6).
pub(super) const STRIKE_CLAMP: f64 = 1.5;

/// A collisional belt's plateau, 5 km over sea level on the reference Earth: the Tibetan plateau's
/// interior, "a mean elevation of 5023 m above sea level" (Fielding, Isacks, Barazangi and Duncan
/// 1994, Geology 22, 163–167, the abstract, as R09.T12's science check read it). Design note 7's
/// peaks near 9 km (Everest's 8,848.86 m) are its summits, which the coarse cells' means reach only
/// where the noise rides high on the plateau, the rest being the synthesis's belt octaves. Medium
/// confidence on Earth.
pub(crate) const BELT_PLATEAU: Metres = Metres::new(5_000.0);

/// A belt's mean half-width, 175 km, varied along strike by 50 km × n, so that its full width
/// spans Design note 7's 200–500 km over n's ±1.5 (an illustration within the plan's range, for
/// which R09.T12's science check found no primary source: the Himalaya are some 150–350 km wide).
pub(crate) const BELT_HALF_WIDTH: Metres = Metres::new(175e3);

/// A belt's half-width's along-strike spread, 50 km a unit of n.
const BELT_HALF_WIDTH_SPREAD: Metres = Metres::new(50e3);

/// The share of a belt's half-width over which it holds its plateau, a half: the rest is a cosine
/// shoulder down to the population (an illustration).
const BELT_FLAT_SHARE: f64 = 0.5;

/// A trench's mean depth below the sea floor it cuts, 3 km, varied along strike by 2 ÷ 3 km × n,
/// so that it spans Design note 7's 2–4 km over n's ±1.5, times h. Design note 7 names no source
/// and R09.T12's science check found no primary one (plate age sets a trench's depth, Stern 2002,
/// Rev. Geophys. 40, 1012, after Grellet and Dubois 1982, not read): low confidence, labelled.
pub(crate) const TRENCH_DEPTH: Metres = Metres::new(3_000.0);

/// A trench's depth's along-strike spread, 2 ÷ 3 km a unit of n.
const TRENCH_DEPTH_SPREAD: Metres = Metres::new(2_000.0 / 3.0);

/// A trench's half-width on its subducting side, 100 km: its depth falls as (1 − |d| ÷ 100 km)²
/// from its boundary cells (an illustration of a trench's inner 50–100 km).
const TRENCH_HALF_WIDTH: Metres = Metres::new(100e3);

/// The volcanic front's median distance behind the trench's axis where the overriding plate is
/// oceanic, 190 km (interquartile 170–220 km, 10–90% 140–250 km), and continental, 260 km
/// (205–295, 180–330 km): Syracuse and Abers 2006's Table S2a (G³ 7, Q05017), 527 front volcanoes,
/// classified by overriding crust by `decision-r09-t5.md` item 6's split of their arc segments, not
/// a split the paper prints (recomputed 2026-10-16: oceanic n = 201, continental n = 326; all
/// fronts 220 km, 180–278, range 85–470; the paper's ¶26 prints the middle half of all fronts at
/// 180–275 km and their range from 85–90 km at Scotia and Vanuatu to 470 km in Alaska). Oceanic, 21 segments: `Central_Aleut`, `West_Aleut`,
/// `Izu`, `Bonin`, `North_Marianas`, `South_Marianas`, `Tonga`, `Kermadec`, `North_Vanuatu`,
/// `South_Vanuatu`, `Solomon`, `New_Britain`, `North_Lesser_Antilles`, `South_Lesser_Antilles`,
/// `Scotia`, `North_Philippines`, `South_Philippines`, `East_Banda_Sea`, `West_Banda_Sea`,
/// `North_Kurile`, `South_Kurile`. Continental, 27: `Colombia`, `Ecuador`, `Peru`, `North_Chile`,
/// `North-Central_Chile`, `Central_Chile`, `South-Central_Chile`, `South_Chile`, `Alaska`,
/// `Alaska_Peninsula`, `Kamchatka`, `Hokkaido`, `North_Honshu`, `Central_Honshu`, `Kyushu`,
/// `Ryukyu`, `North_Sumatra`, `Central_Sumatra`, `South_Sumatra`, `Sunda_Strait`, `Java`,
/// `Bali/Lombok`, `New_Zealand`, `Guatemala/El_Salvador`, `Nicaragua`, `Costa_Rica`, `East_Aleut`.
/// Bird 2003's "200–250 km is more typical" (G³ 4, 1027, ¶105) brackets the median; the slab lies
/// 101 ± 16 km beneath the front (England, Engdahl and Thatcher 2004, §2.3). Earth's distances,
/// unscaled off Earth (labelled; the arc–trench gap off Earth is a lean of the plan's Risks).
pub(crate) const FRONT_OCEANIC: Metres = Metres::new(190e3);

/// The volcanic front's median distance behind the trench's axis under continental crust, 260 km
/// ([`FRONT_OCEANIC`] gives its source and classification).
pub(crate) const FRONT_CONTINENTAL: Metres = Metres::new(260e3);

/// The front's along-strike spread: G × (1 + 0.2 n) (`decision-r09-t5.md` item 6).
const FRONT_SPREAD: f64 = 0.2;

/// An island arc's width behind its front, 60 km (30–100 km; `decision-r09-t5.md` item 6).
pub(crate) const ARC_WIDTH_OCEANIC: Metres = Metres::new(60e3);

/// A continental arc's width behind its front, 175 km (100–250 km; `decision-r09-t5.md` item 6).
pub(crate) const ARC_WIDTH_CONTINENTAL: Metres = Metres::new(175e3);

/// An island arc's rise above the sea floor it stands on, 3 km times h: an arc crust about three
/// times the oceanic crust's thickness, in isostatic balance, stands some 2.5–3 km above the
/// floor beside it, its crest a kilometre or two deep between its volcanic islands (the
/// Izu–Bonin–Mariana arc, whose volcanoes stand on a platform 1–4 km deep; from memory, and no
/// primary source found by R09.T12's science check). Low confidence: an illustration.
pub(crate) const ARC_RISE_OCEANIC: Metres = Metres::new(3_000.0);

/// A continental arc's rise above the continental population, 2.5 km times h: its crest near
/// 3 km (the Cascades' and Japan's arcs lower, the Central Andes' Altiplano about 3.75 km; from
/// memory, and no primary source found by R09.T12's science check). Low confidence: an
/// illustration.
pub(crate) const ARC_RISE_CONTINENTAL: Metres = Metres::new(2_500.0);

/// The arcs' distances from the trench, and the fronts' G, as [`run`] laid them out: what the
/// tests read.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Arcs {
    /// Each overriding cell's great-circle distance from its source's axis point, metres, or
    /// `None` where no trench of its plate reaches it.
    pub(super) distance_m: Vec<Option<f64>>,
    /// Each such cell's path distance from the axis, metres (the 8-connected transform's).
    pub(super) path_m: Vec<Option<f64>>,
    /// Each such cell's front distance G, metres, its source's.
    pub(super) front_m: Vec<Option<f64>>,
    /// Each such cell's arc width W, metres, by its source's crust.
    pub(super) width_m: Vec<Option<f64>>,
    /// Each such cell's source: the overriding boundary cell its path starts from.
    pub(super) source: Vec<Option<u32>>,
    /// Each source's axis point, a unit vector, by cell.
    pub(super) axis: Vec<Option<[f64; 3]>>,
}

/// Runs the convergent landforms on `working`'s mobile lid (see the module documentation); a world
/// without plates is left as it is. Returns the arcs' layout, for the tests.
pub(super) fn run(pass: &Pass<'_>, working: &mut Working) -> Option<Arcs> {
    if working.plates.is_empty() {
        return None;
    }
    let h = height_factor(pass.inputs().gravity());
    let mut strike = noise::unit_field(pass, STRIKE_INSTANCE, STRIKE_LEVEL, STRIKE_LEVEL);
    for n in &mut strike {
        *n = n.clamp(-STRIKE_CLAMP, STRIKE_CLAMP);
    }
    let arcs = arcs(pass, working, &strike);
    for (n, cell) in working.cells.iter_mut().enumerate() {
        let lift = belt_lift(cell, strike[n], h)
            + trench_lift(cell, strike[n], h)
            + arc_lift(cell, &arcs, n, h);
        if lift != 0.0 {
            let z = Metres::new(num::assert_finite(cell.elevation.value() + lift));
            cell.elevation = z;
            cell.water_surface = z;
        }
    }
    Some(arcs)
}

/// A collisional belt's lift at `cell`, metres, under the along-strike draw `strike` (n) and the
/// height factor `factor` (h): on continental crust whose nearest boundary is a collision, the plateau profile of
/// |d| over the half-width 175 km + 50 km × n times h × ([`BELT_PLATEAU`] −
/// [`CONTINENTAL_CORE_MEAN`], the population's own mean);
/// zero elsewhere.
#[must_use]
pub(super) fn belt_lift(cell: &CellState, strike: f64, factor: f64) -> f64 {
    let (BoundaryKind::Collision, Some(distance), Crust::Continental) =
        (cell.boundary, cell.boundary_distance, cell.crust)
    else {
        return 0.0;
    };
    let half_width = BELT_HALF_WIDTH.value() + BELT_HALF_WIDTH_SPREAD.value() * strike;
    let across = distance.value().abs() / half_width;
    let shape = if across <= BELT_FLAT_SHARE {
        1.0
    } else if across < 1.0 {
        let shoulder = (across - BELT_FLAT_SHARE) / (1.0 - BELT_FLAT_SHARE);
        f64::midpoint(1.0, math::cos(core::f64::consts::PI * shoulder))
    } else {
        0.0
    };
    shape * factor * (BELT_PLATEAU.value() - CONTINENTAL_CORE_MEAN.value())
}

/// A trench's lift at `cell`, metres, negative, under the along-strike draw `strike` (n) and the
/// height factor `factor` (h): on the subducting side of a subduction within [`TRENCH_HALF_WIDTH`] of its
/// boundary cells, −h (3 km + 2 ÷ 3 km × n) (1 − |d| ÷ 100 km)²; zero elsewhere.
#[must_use]
fn trench_lift(cell: &CellState, strike: f64, factor: f64) -> f64 {
    let (BoundaryKind::Subduction, Some(distance), true) =
        (cell.boundary, cell.boundary_distance, cell.subducts)
    else {
        return 0.0;
    };
    let across = distance.value().abs() / TRENCH_HALF_WIDTH.value();
    if across >= 1.0 {
        return 0.0;
    }
    let depth = TRENCH_DEPTH.value() + TRENCH_DEPTH_SPREAD.value() * strike;
    -factor * depth * (1.0 - across) * (1.0 - across)
}

/// The arc's profile at u = y − G metres from its front, over its width `width` metres: a cosine
/// rise over the quarter-width before the front, the plateau from the front over half the width,
/// and a cosine fall to the band's back at G + W (an illustration).
#[must_use]
fn arc_profile(u: f64, width: f64) -> f64 {
    let rise = 0.25 * width;
    if u <= -rise || u >= width {
        0.0
    } else if u < 0.0 {
        0.5 * (1.0 - math::cos(core::f64::consts::PI * (u + rise) / rise))
    } else if u <= 0.5 * width {
        1.0
    } else {
        f64::midpoint(
            1.0,
            math::cos(core::f64::consts::PI * (u - 0.5 * width) / (0.5 * width)),
        )
    }
}

/// An arc's lift at cell `n` (state `cell`), metres, from the arcs' layout `arcs` under the height
/// factor `h`: its profile at its distance from the front over its source's band, times h and the
/// rise over the cell's own crust ([`ARC_RISE_OCEANIC`], [`ARC_RISE_CONTINENTAL`]); zero where no
/// trench of its plate reaches it.
#[must_use]
fn arc_lift(cell: &CellState, arcs: &Arcs, n: usize, h: f64) -> f64 {
    let (Some(y), Some(front), Some(width)) =
        (arcs.distance_m[n], arcs.front_m[n], arcs.width_m[n])
    else {
        return 0.0;
    };
    let rise = match cell.crust {
        Crust::Oceanic => ARC_RISE_OCEANIC,
        Crust::Continental | Crust::Lid | Crust::Province => ARC_RISE_CONTINENTAL,
    };
    h * rise.value() * arc_profile(y - front, width)
}

/// Whether `cell` is a boundary cell of a subduction (its signed distance +0): on its subducting
/// side where `subducts` holds, on the overriding side where it does not.
#[must_use]
fn subduction_boundary(cell: &CellState) -> bool {
    cell.boundary == BoundaryKind::Subduction
        && cell.boundary_distance.is_some_and(|d| d.value() == 0.0)
}

/// The arcs' layout over `working`'s cells under the along-strike draw `strike` (see the module
/// documentation).
///
/// # Panics
///
/// If a distance is not finite.
#[must_use]
fn arcs(pass: &Pass<'_>, working: &Working, strike: &[f64]) -> Arcs {
    let grid = pass.grid();
    let radius_m = pass.inputs().radius().value();
    let cells = &working.cells;
    let count = cells.len();
    let mut axis: Vec<Option<[f64; 3]>> = vec![None; count];
    let mut sources: Vec<(u32, f64)> = Vec::new();
    for (n, cell) in cells.iter().enumerate() {
        if !subduction_boundary(cell) || cell.subducts {
            continue;
        }
        let here = cell_of(n);
        let centre = grid.centre(here);
        // The nearest subducting boundary cell across, by the link's arc, the first of equal arcs
        // in neighbour order.
        let mut nearest: Option<(f64, u32)> = None;
        for m in grid
            .edge_neighbours(here)
            .into_iter()
            .chain(grid.vertex_neighbours(here))
        {
            let other = &cells[slot(m)];
            if other.plate == cell.plate || !subduction_boundary(other) || !other.subducts {
                continue;
            }
            let link = grid.arc(here, m).value();
            if nearest.is_none_or(|(l, _)| link < l) {
                nearest = Some((link, m));
            }
        }
        let (start, point) = match nearest {
            Some((link, m)) => {
                let across = grid.centre(m);
                let mid = [
                    centre[0] + across[0],
                    centre[1] + across[1],
                    centre[2] + across[2],
                ];
                let norm = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
                (
                    0.5 * link * radius_m,
                    [mid[0] / norm, mid[1] / norm, mid[2] / norm],
                )
            }
            // No subducting boundary cell across: the axis at the cell itself (labelled).
            None => (0.0, centre),
        };
        axis[n] = Some(point.map(num::assert_finite));
        sources.push((here, start));
    }
    let region: Vec<u8> = cells.iter().map(|c| c.plate).collect();
    let reached = reach_from(grid, &region, &sources, radius_m);
    let mut arcs = Arcs {
        distance_m: vec![None; count],
        path_m: vec![None; count],
        front_m: vec![None; count],
        width_m: vec![None; count],
        source: vec![None; count],
        axis,
    };
    for (n, r) in reached.iter().enumerate() {
        let Some(r) = r else { continue };
        let source = slot(r.source);
        let Some(point) = arcs.axis[source] else {
            continue;
        };
        let x = grid.centre(cell_of(n));
        let gap = [x[0] - point[0], x[1] - point[1], x[2] - point[2]];
        let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
        let angle = 2.0 * math::asin(num::min(0.5 * chord, 1.0));
        let (median, width) = match cells[source].crust {
            Crust::Oceanic => (FRONT_OCEANIC, ARC_WIDTH_OCEANIC),
            Crust::Continental | Crust::Lid | Crust::Province => {
                (FRONT_CONTINENTAL, ARC_WIDTH_CONTINENTAL)
            }
        };
        arcs.distance_m[n] = Some(num::assert_finite(angle * radius_m));
        arcs.path_m[n] = Some(r.distance_m);
        arcs.front_m[n] = Some(median.value() * (1.0 + FRONT_SPREAD * strike[source]));
        arcs.width_m[n] = Some(width.value());
        arcs.source[n] = Some(r.source);
    }
    arcs
}
