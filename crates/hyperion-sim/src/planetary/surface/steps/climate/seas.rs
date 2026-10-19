//! The seas' liquids (R09.T13.d; the lane's research-d §3): each liquid of the inputs placed on
//! the flooded cells to its share of the ocean fraction (plan 14's `surface_liquids`).
//!
//! # The sea
//!
//! The sea is the flood the relief step writes (decision-r09-t12-coarse R8): R09.T12.e floods the
//! connected ocean itself, each flooded cell's water surface the sea level ℓ and its liquid entry
//! the main liquid's, and leaves the enclosed depressions below ℓ dry, until R09.T14's lakes
//! flood them to their own levels. A cell is under liquid where its water surface lies strictly
//! above its ground ([`under_sea`]), whatever flooded it; every reader takes that one predicate,
//! the energy-balance models' sea shares, the liquids and, through each cell's water surface, the
//! lapse term the ices' keys are read at. This step writes no water surface. Where no cell is
//! flooded (every world before R09.T12.e, whose water surfaces are all their ground), it places
//! nothing.
//!
//! # Several liquids ([`place_liquids`])
//!
//! By basin, not by depth: a depth order would put the first liquid in every basin's middle and
//! the second on every shore, which neither miscible nor immiscible liquids do (research-d §3).
//!
//! 1. The basins are the flooded cells' connected components over the cells' edge neighbours,
//!    found in cell-index order.
//! 2. They go coldest first, by their area-weighted annual mean surface temperature, ties by their
//!    least cell index.
//! 3. The liquids go most volatile first, by their saturation pressure at the body's mean surface
//!    temperature on the registry's diagrams, ties by key (Titan's methane before its ethane).
//! 4. The flooded cells in (basin, annual mean surface temperature, cell index) are dealt to the
//!    liquids in turn, each until its share of the ocean fraction is met to one cell; the last
//!    takes the rest. A world of one liquid gives it every flooded cell.
//!
//! The shares are of the flooded area: each liquid's target is its area fraction over the ocean
//! fraction ([`CoarseInputs::ocean_fraction`]), times the area the relief step flooded, not its
//! own fraction of the sphere, so that where the flood's area differs from the ocean fraction (R8's
//! depressions that join the ocean at ℓ, or T14's lakes) each liquid keeps its proportion of it.
//! The rule reads only which cells are flooded, so it works over any flooded set, R09.T12.e's one
//! ocean as well as T14's lakes, and needs no copy of R09.T12.e's flood function.
//!
//! Inside one basin larger than a liquid's share (R09.T12.e's one ocean, or a Titan whose seas
//! join), the liquids divide it by the cells' annual mean surface temperature: the most volatile
//! takes the basin's coldest cells to its share, the next the warmer ones, so the boundary between
//! them is an isotherm of the annual mean, and one liquid's cells need not be connected (on a
//! Titan of one ocean, methane at both cold ends and ethane between them).
//!
//! The physical motive is that the more volatile liquid lasts where evaporation is weakest:
//! ethane has been identified in Ontario Lacus, at 72° S (Brown et al. 2008, Nature 454, 607;
//! title read); that Titan's north-polar seas are the richer in methane is from memory
//! (research-d §3). It is a heuristic, labelled, and the order of the liquids a closed rule over
//! any registry rows, for the composition audit.

use hyperion_surface::field::PaletteRole;
use hyperion_surface::substance_key::SubstanceKey;

use super::super::CellState;
use super::finite_k;
use crate::planetary::surface::grid::CellGraph;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::substance::by_key;

/// Whether field cell `cell` lies under liquid: where the relief step flooded it, its water
/// surface strictly above its ground (the [module documentation](self); decision-r09-t12-coarse
/// R8), whatever flooded it, R09.T12.e's ocean or R09.T14's lakes. A cell whose heights are not
/// numbers reads as dry.
#[must_use]
pub(crate) fn under_sea(cell: &CellState) -> bool {
    cell.water_surface.value() > cell.elevation.value()
}

/// One liquid a placement placed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlacedLiquid {
    /// The substance.
    pub(crate) substance: SubstanceKey,
    /// Its share of the ocean fraction, plan 14's.
    pub(crate) share: f64,
    /// The share of the flooded area it covers.
    pub(crate) placed: f64,
    /// The cells it covers.
    pub(crate) cells: u32,
}

/// The flooded cells' (`sea`'s) connected components over the edge neighbours of `grid`, found
/// in cell-index order: each cell's basin, numbered from 0 in the order found, or `None` where it
/// is dry; and the basins' count.
#[must_use]
fn basins(grid: &CellGraph, sea: &[bool]) -> (Vec<Option<u32>>, u32) {
    let mut basin = vec![None; sea.len()];
    let mut count = 0_u32;
    let mut stack = Vec::new();
    for start in 0..sea.len() {
        if !sea[start] || basin[start].is_some() {
            continue;
        }
        basin[start] = Some(count);
        stack.push(start);
        while let Some(cell) = stack.pop() {
            let index = u32::try_from(cell).expect("a coarse level's cells fit a u32");
            for neighbour in grid.edge_neighbours(index) {
                let n = usize::try_from(neighbour).expect("a cell index fits a usize");
                if sea[n] && basin[n].is_none() {
                    basin[n] = Some(count);
                    stack.push(n);
                }
            }
        }
        count += 1;
    }
    (basin, count)
}

/// Shares the flooded cells of `cells` on `grid` ([`under_sea`], the relief step's flood) among
/// the liquids of `inputs` (the [module documentation](self)), each cell's annual mean surface
/// temperature `annual_k`: each flooded cell's `liquid_entry`, its liquid's
/// [`PaletteRole::Liquid`] entry. Its water surface, and every dry cell, it leaves as they are.
/// What it placed, most volatile first; nothing on a world with no flooded cell or no liquid.
///
/// # Panics
///
/// If a flooded cell's temperature is not finite, a liquid's volatility is not a number, or a
/// liquid has no `Liquid` entry in the palette, which the inputs' builder refuses.
pub(crate) fn place_liquids(
    inputs: &CoarseInputs,
    grid: &CellGraph,
    annual_k: &[f64],
    cells: &mut [CellState],
) -> Vec<PlacedLiquid> {
    let sea: Vec<bool> = cells.iter().map(under_sea).collect();
    if inputs.liquids().is_empty() || !sea.iter().any(|&s| s) {
        return Vec::new();
    }
    let omega = grid.solid_angles_sr();
    let (basin, count) = basins(grid, &sea);
    let slots = usize::try_from(count).expect("the basins fit a usize");
    let (mut heat, mut area) = (vec![0.0; slots], vec![0.0; slots]);
    let mut least: Vec<Option<usize>> = vec![None; slots];
    for (i, b) in basin.iter().enumerate() {
        if let Some(b) = *b {
            let b = usize::try_from(b).expect("a basin's number fits a usize");
            least[b].get_or_insert(i);
            heat[b] += omega[i] * finite_k(annual_k[i]);
            area[b] += omega[i];
        }
    }
    let mut rank: Vec<usize> = (0..slots).collect();
    rank.sort_by(|&a, &b| {
        (heat[a] / area[a])
            .total_cmp(&(heat[b] / area[b]))
            .then(least[a].cmp(&least[b]))
    });
    let mut order_of = vec![0_usize; slots];
    for (position, &b) in rank.iter().enumerate() {
        order_of[b] = position;
    }
    let mut dealt: Vec<(usize, f64, usize)> = basin
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            b.map(|b| {
                let b = usize::try_from(b).expect("a basin's number fits a usize");
                (order_of[b], annual_k[i], i)
            })
        })
        .collect();
    dealt.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mean_k = inputs.mean_surface_temperature();
    // Above the critical point a saturation pressure may be +∞, which sorts as the most volatile;
    // a NaN would sort by its sign bit, so none may reach the sort.
    let volatility = |key: SubstanceKey| {
        let pressure = by_key(key.as_str())
            .and_then(|id| id.substance().phase_diagram())
            .map_or(0.0, |diagram| diagram.saturation_pressure(mean_k).value());
        assert!(
            !pressure.is_nan(),
            "a liquid's saturation pressure must be a number, got {pressure} Pa for {}",
            key.as_str()
        );
        pressure
    };
    let mut liquids: Vec<(f64, SubstanceKey, f64)> = inputs
        .liquids()
        .iter()
        .map(|l| (volatility(l.substance), l.substance, l.area_fraction))
        .collect();
    liquids.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let total_share = inputs.ocean_fraction();
    let sea_area = area.iter().fold(0.0, |sum, a| sum + a);
    let palette = inputs.palette();
    let final_liquid = liquids.len() - 1;
    let mut queue = dealt.into_iter().peekable();
    let mut placed = Vec::with_capacity(liquids.len());
    for (n, (_, substance, fraction)) in liquids.into_iter().enumerate() {
        let entry = palette
            .find(PaletteRole::Liquid, substance)
            .expect("the inputs' builder gives every liquid a Liquid entry");
        let share = if total_share > 0.0 {
            fraction / total_share
        } else {
            0.0
        };
        let target = share * sea_area;
        let (mut held, mut taken) = (0.0, 0_u32);
        while let Some(&(_, _, i)) = queue.peek() {
            if n != final_liquid && held + 0.5 * omega[i] > target {
                break;
            }
            cells[i].liquid_entry = Some(entry);
            held += omega[i];
            taken += 1;
            queue.next();
        }
        placed.push(PlacedLiquid {
            substance,
            share,
            placed: held / sea_area,
            cells: taken,
        });
    }
    placed
}
