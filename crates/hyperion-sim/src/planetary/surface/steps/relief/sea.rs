//! R09.T12.e's sea: the connected ocean that covers plan 14's ocean fraction, its level, its fill
//! and its liquid (plan R09, Design note 7; main's rulings R8 and R10, 2026-10-19).
//!
//! The sea is the connected ocean, so an enclosed depression does not fill to the ocean's level
//! (the Dead Sea and the Caspian lie below it, dry or at their own levels):
//!
//! - **The quantile** `ℓ_q`: the cells sorted by (elevation, cell index), each asserted finite,
//!   their solid angles walked in that order; `ℓ_q` is the elevation of the cell at which the walk
//!   reaches plan 14's ocean fraction f of the sphere (the last cell's, should rounding keep the
//!   walk short of it).
//! - **The seed**: the largest connected region, through edge neighbours, of the cells at or below
//!   `ℓ_q` (the lane's lean: "at or below", so that a fraction under one cell still has a seed),
//!   ties to the region of least cell index, so that the ocean is the largest water body, not the
//!   deepest basin's.
//! - **The flood**: a priority flood from the seed, each frontier cell keyed by its filled level,
//!   the larger of its elevation and the filled level of the cell it was reached from, then its
//!   cell index. The sea level ℓ is the lowest filled level at which the area the flood took
//!   reaches f × the sphere, every cell of that filled level taken; where the flood takes every
//!   cell first (f = 1, to rounding), the last cell's.
//! - **The fill**: every cell the flood took whose ground lies below ℓ takes the water surface ℓ
//!   and the main liquid's palette entry ([`CoarseInputs::main_liquid`]). "Below" is read on the
//!   field's codes, the ground's millimetres under the sea level's, so that a cell is under water
//!   in the quantised field exactly where it was flooded, a ground a fraction of a millimetre below
//!   ℓ included, which stays dry with the cells at ℓ: the ocean's shore, which the flood took.
//!   Every other cell stays dry, the enclosed depressions below ℓ among them, until R09.T14.a's
//!   priority flood and R09.T14.d's water surfaces place lakes there by their water balance
//!   (labelled).
//!
//! The ocean's area is then f, or more by the cells of the final filled level. f = 0 floods
//! nothing, and ℓ is the lowest cell's elevation; f = 1 floods every cell but the highest, ℓ the
//! highest's elevation. The areas are the cells' solid angles, a share of the sphere of
//! directions, where plan 14's fraction is one of the body's surface area (Eakins and Sharman
//! 2010's 0.7095 is the WGS84 ellipsoid's): the two differ by about 10⁻⁴ on an Earth and up to
//! about 10⁻² on a strongly flattened ocean world (labelled; R09.T12.e's science check). One liquid, the main liquid, fills the whole ocean fraction, the liquids'
//! shares summed ([`CoarseInputs::ocean_fraction`]): R09.T13.d places each liquid's own share
//! (ruling R10).
//!
//! [`CoarseInputs::main_liquid`]: crate::planetary::surface::inputs::CoarseInputs::main_liquid
//! [`CoarseInputs::ocean_fraction`]: crate::planetary::surface::inputs::CoarseInputs::ocean_fraction

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use hyperion_surface::field::SynthesisCell;
use hyperion_surface::num;

use super::super::plates::slot;
use super::super::{Pass, Working};
use crate::planetary::surface::grid::CellGraph;
use crate::units::Metres;

/// What the sea's placement found: the record its tests and the plan read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sea {
    /// `ℓ_q`, the area-weighted quantile at the ocean fraction, metres: the lowest elevation where
    /// the fraction is zero.
    pub(crate) quantile_m: f64,
    /// ℓ, the sea level, metres above the datum.
    pub(crate) level_m: f64,
    /// The seed's cells, ascending: none where the fraction is zero.
    pub(crate) seed: Vec<u32>,
    /// The cells the flood took, in pop order: the ocean, its shore at the sea level included.
    pub(crate) taken: Vec<u32>,
    /// The share of the sphere the flood took.
    pub(crate) ocean_share: f64,
    /// The share of the sphere the flood took at the final filled level, the sea level: the cells
    /// and depressions that joined there, by which the ocean may exceed the fraction.
    pub(crate) final_level_share: f64,
    /// The share of the sphere under water.
    pub(crate) flooded_share: f64,
    /// The cells under water.
    pub(crate) flooded_cells: usize,
    /// The share of the sphere of dry ground below the sea level, on the field's codes.
    pub(crate) dry_below_share: f64,
}

/// Where the flood's level comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SeaLevel {
    /// The lowest filled level at which the ocean covers the ocean fraction: the sea's placement.
    FromFraction,
    /// A level already placed, which the climate is reduced to: the reflood.
    Kept(Metres),
}

/// Places the sea on `working` for the body of `pass` (see the module documentation): finds its
/// level from plan 14's ocean fraction, floods the ocean and writes the sea level.
///
/// # Panics
///
/// Never on valid inputs: every elevation is finite and within its code.
pub(crate) fn place(pass: &Pass<'_>, working: &mut Working) -> Sea {
    flood(pass, working, SeaLevel::FromFraction)
}

/// Refloods the ocean on `working` against the sea level it holds, which the climate is reduced
/// to, after the craters that waited for the erosion step (main's ruling R4): the sea's cells,
/// those of the main liquid, are drained, and the flood runs again from a fresh seed with the
/// level kept.
///
/// # Panics
///
/// As [`place`].
pub(crate) fn reflood(pass: &Pass<'_>, working: &mut Working) -> Sea {
    if let Some(main) = pass.inputs().main_liquid() {
        for cell in &mut working.cells {
            if cell.liquid_entry == Some(main) {
                cell.water_surface = cell.elevation;
                cell.liquid_entry = None;
            }
        }
    }
    let kept = working.sea_level;
    flood(pass, working, SeaLevel::Kept(kept))
}

/// A frontier cell of the flood: its filled level and its index, ordered so that the heap pops the
/// lowest level first, then the least index.
#[derive(Debug, Clone, Copy)]
struct Entry {
    /// The filled level, metres.
    level_m: f64,
    cell: u32,
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .level_m
            .total_cmp(&self.level_m)
            .then_with(|| other.cell.cmp(&self.cell))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Entry {}

/// The code of a height, millimetres, the field's ([`SynthesisCell::quantise_height`]).
///
/// # Panics
///
/// If the height has no code: a fault of the step that wrote it.
#[must_use]
fn code(height_m: f64) -> i32 {
    SynthesisCell::quantise_height(Metres::new(height_m))
        .unwrap_or_else(|e| panic!("a coarse height has a code, but {e} ({e:?})"))
}

/// The largest connected region of `grid`'s cells that `inside` marks, through edge neighbours,
/// its area summed in ascending cell order; ties to the region of least cell index. Its cells
/// ascend.
#[must_use]
fn largest_region(grid: &CellGraph, inside: &[bool]) -> Vec<u32> {
    let omega = grid.solid_angles_sr();
    let mut seen = vec![false; inside.len()];
    let (mut best, mut best_area) = (Vec::new(), 0.0);
    for start in 0..grid.len() {
        if !inside[slot(start)] || seen[slot(start)] {
            continue;
        }
        seen[slot(start)] = true;
        let mut region = vec![start];
        let mut next = 0;
        while let Some(&c) = region.get(next) {
            next += 1;
            for m in grid.edge_neighbours(c) {
                if inside[slot(m)] && !seen[slot(m)] {
                    seen[slot(m)] = true;
                    region.push(m);
                }
            }
        }
        region.sort_unstable();
        let area: f64 = region.iter().map(|&c| omega[slot(c)]).sum();
        if best.is_empty() || area > best_area {
            best = region;
            best_area = area;
        }
    }
    best
}

/// `ℓ_q`, metres: the elevation of the cell at which the cells of `grid`, sorted by (elevation in
/// `z`, cell index), reach the area `target_sr` steradians, their solid angles walked in that
/// order; the highest cell's, should rounding keep the walk short of it.
#[must_use]
fn quantile(grid: &CellGraph, z: &[f64], target_sr: f64) -> f64 {
    let omega = grid.solid_angles_sr();
    let mut order: Vec<u32> = (0..grid.len()).collect();
    order.sort_by(|&a, &b| z[slot(a)].total_cmp(&z[slot(b)]).then(a.cmp(&b)));
    let mut walked = 0.0;
    for &c in &order {
        walked += omega[slot(c)];
        if walked >= target_sr {
            return z[slot(c)];
        }
    }
    z[slot(*order.last().expect("a grid has cells"))]
}

/// What a priority flood took ([`priority_flood`]).
struct Flood {
    /// The cells it took, in pop order, each with its filled level.
    taken: Vec<Entry>,
    /// The sea level, metres.
    level_m: f64,
    /// The area it took, steradians, summed in pop order.
    area_sr: f64,
}

/// The priority flood of `grid`'s cells of elevations `z` from the cells `seed` (see the module
/// documentation). Under [`SeaLevel::Kept`] it takes every cell of a filled level up to the kept
/// level; under [`SeaLevel::FromFraction`] the level is the lowest filled level at which the area
/// reaches `target_sr` steradians, every cell of that level taken, or the last cell's where the
/// flood takes every cell first.
#[must_use]
fn priority_flood(
    grid: &CellGraph,
    z: &[f64],
    seed: &[u32],
    target_sr: f64,
    level: SeaLevel,
) -> Flood {
    // At least one cell lies at or below the quantile, a cell's own elevation, so the seed is a
    // region of one cell or more and the flood pops at least one.
    debug_assert!(!seed.is_empty(), "a flood from no cell");
    let omega = grid.solid_angles_sr();
    let mut visited = vec![false; z.len()];
    let mut frontier = BinaryHeap::with_capacity(seed.len());
    for &c in seed {
        visited[slot(c)] = true;
        frontier.push(Entry {
            level_m: z[slot(c)],
            cell: c,
        });
    }
    let mut found = match level {
        SeaLevel::Kept(kept) => Some(kept.value()),
        SeaLevel::FromFraction => None,
    };
    let mut last = f64::NEG_INFINITY;
    let (mut taken, mut area_sr): (Vec<Entry>, f64) = (Vec::new(), 0.0);
    while let Some(entry) = frontier.pop() {
        if found.is_some_and(|l| entry.level_m.total_cmp(&l).is_gt()) {
            break;
        }
        taken.push(entry);
        area_sr += omega[slot(entry.cell)];
        last = entry.level_m;
        if found.is_none() && area_sr >= target_sr {
            found = Some(entry.level_m);
        }
        for m in grid.edge_neighbours(entry.cell) {
            if !visited[slot(m)] {
                visited[slot(m)] = true;
                frontier.push(Entry {
                    level_m: num::max(z[slot(m)], entry.level_m),
                    cell: m,
                });
            }
        }
    }
    Flood {
        taken,
        level_m: found.unwrap_or(last),
        area_sr,
    }
}

/// The sea on `working`, its level from `level` (see the module documentation).
fn flood(pass: &Pass<'_>, working: &mut Working, level: SeaLevel) -> Sea {
    let inputs = pass.inputs();
    let grid = pass.grid();
    let omega = grid.solid_angles_sr();
    let total_sr: f64 = omega.iter().sum();
    let f = inputs.ocean_fraction();
    let z: Vec<f64> = working
        .cells
        .iter()
        .map(|c| num::assert_finite(c.elevation.value()))
        .collect();
    let dry_below = |level_m: f64, wet: &[bool]| -> f64 {
        let sea = code(level_m);
        let mut area = 0.0;
        for ((&h, &w), &wet) in z.iter().zip(omega).zip(wet) {
            if !wet && code(h) < sea {
                area += w;
            }
        }
        area / total_sr
    };
    // The builder holds the fraction to [+0, 1].
    if f <= 0.0 {
        let lowest = z.iter().copied().fold(f64::INFINITY, num::min);
        let level_m = match level {
            SeaLevel::Kept(kept) => kept.value(),
            SeaLevel::FromFraction => lowest,
        };
        working.sea_level = Metres::new(level_m);
        return Sea {
            quantile_m: lowest,
            level_m,
            seed: Vec::new(),
            taken: Vec::new(),
            ocean_share: 0.0,
            final_level_share: 0.0,
            flooded_share: 0.0,
            flooded_cells: 0,
            dry_below_share: dry_below(level_m, &vec![false; z.len()]),
        };
    }
    let target_sr = f * total_sr;
    let quantile_m = quantile(grid, &z, target_sr);
    let inside: Vec<bool> = z.iter().map(|&h| h <= quantile_m).collect();
    let seed = largest_region(grid, &inside);
    let ocean = priority_flood(grid, &z, &seed, target_sr, level);
    let sea = code(ocean.level_m);
    let main = inputs.main_liquid().expect(
        "an ocean fraction above zero is the liquids' shares, and the inputs' builder gives every \
         liquid a palette entry of the liquid role",
    );
    let mut wet = vec![false; z.len()];
    let (mut flooded, mut flooded_cells, mut final_level) = (0.0, 0, 0.0);
    for entry in &ocean.taken {
        let c = slot(entry.cell);
        if entry.level_m.total_cmp(&ocean.level_m).is_eq() {
            final_level += omega[c];
        }
        if code(z[c]) < sea {
            let cell = &mut working.cells[c];
            cell.water_surface = Metres::new(ocean.level_m);
            cell.liquid_entry = Some(main);
            wet[c] = true;
            flooded += omega[c];
            flooded_cells += 1;
        }
    }
    working.sea_level = Metres::new(ocean.level_m);
    Sea {
        quantile_m,
        level_m: ocean.level_m,
        seed,
        taken: ocean.taken.iter().map(|e| e.cell).collect(),
        ocean_share: ocean.area_sr / total_sr,
        final_level_share: final_level / total_sr,
        flooded_share: flooded / total_sr,
        flooded_cells,
        dry_below_share: dry_below(ocean.level_m, &wet),
    }
}
