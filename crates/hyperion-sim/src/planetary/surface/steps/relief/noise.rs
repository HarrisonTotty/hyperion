//! The coarse landform noise (R09.T12.a–c): smooth random fields over a pass's cells, drawn on
//! `surface.coarse.relief`, which carry no power finer than the coarse cell's Nyquist wavelength
//! (`decision-r09-t5.md` item 3: the synthesis's structural share owns what is finer).
//!
//! A field is a sum of octaves, one per cube-sphere lattice level ℓ from its first to its last,
//! at most the field's own. Each lattice cell draws one standard normal (two words) from its own
//! stream, keyed `surface_cell(face, ℓ, i, j, instance)`, so that the octaves are disjoint by
//! level and the fields by instance. An octave's value at a cell's centre x is the Shepard mean
//! of the lattice values within the chord h = [`KERNEL_REACH`] × 2⁻ℓ, under the compact kernel
//! (1 − |x − c|² ÷ h²)³ of the lattice centres c, as R09.T11's warp is (C² wherever it is not zero,
//! and across face edges, since it reads directions, not face coordinates). The candidates are the
//! lattice cells within [`RINGS`] rings of the one holding x, summed in ascending lattice index.
//!
//! Each octave is made zero-mean and unit-RMS over the sphere (area-weighted, in cell-index
//! order), weighted by 2^(−(ℓ − first) ÷ 2), so that its variance halves an octave finer, a
//! per-degree variance ∝ l⁻², the silicate structural spectrum's β₁ = 2.0 below the coarse cell
//! (`decision-r09-t5.md` item 2), carried above it; and the sum is made zero-mean and unit-RMS in
//! turn. The spectrum's continuation above the coarse cell is an illustration (labelled), chosen
//! so that the coarse field meets the synthesis's at the cell without a step in its slope.

use hyperion_surface::tags::SURFACE_COARSE_RELIEF;

use super::super::Pass;
use super::super::plates::slot;
use crate::math;
use crate::planetary::surface::grid::CellGraph;
use crate::rng::ObjectKey;

/// The kernel's reach as a chord on the unit sphere, over the lattice's 2⁻ℓ: 1.8, about twice the
/// level's smallest cell edge (0.947 × 2⁻ℓ radians, Design note 4), so that every centre a kernel
/// reaches lies within [`RINGS`] rings of the point's own lattice cell, and at least one lies
/// within it (a cell's circumradius is under 0.75 × 2⁻ℓ).
const KERNEL_REACH: f64 = 1.8;

/// The rings of lattice cells about the one holding a point that its kernel can reach: 2.
const RINGS: usize = 2;

/// The coarse landform noise's lattice instance (`surface.coarse.relief`), instance 0: the crust's
/// populations (R09.T12.a).
pub(super) const LANDFORM_INSTANCE: u16 = 0;

/// The coarsest lattice level of the landform noise, 2 (96 cells, about 2,300 km across on an
/// Earth): continents rise and fall as wholes, as Africa and Australia do.
pub(super) const LANDFORM_FIRST_LEVEL: u8 = 2;

/// A unit field over `pass`'s cells, instance `instance`, from lattice level `first` to `last`
/// (held to the field's level): zero-mean and unit-RMS over the sphere, area-weighted (the module
/// documentation).
///
/// # Panics
///
/// If `first` is above the held `last`, or a field has no area, which no grid has.
#[must_use]
pub(super) fn unit_field(pass: &Pass<'_>, instance: u16, first: u8, last: u8) -> Vec<f64> {
    let grid = pass.grid();
    let level = grid.level();
    let last = last.min(level);
    assert!(first <= last, "a noise's levels {first}..={last} are empty");
    let weights = grid.solid_angles_sr();
    let mut sum = vec![0.0; weights.len()];
    for lattice_level in first..=last {
        let octave = octave(pass, instance, lattice_level);
        let amplitude = math::powf(0.5, 0.5 * f64::from(lattice_level - first));
        for (s, o) in sum.iter_mut().zip(&octave) {
            *s += amplitude * o;
        }
    }
    standardise(&mut sum, weights);
    sum
}

/// Makes `values` zero-mean and unit-RMS under the area weights `weights`, summed in index order;
/// a field of no spread is left at zero.
///
/// # Panics
///
/// If the weights sum to zero.
pub(super) fn standardise(values: &mut [f64], weights: &[f64]) {
    let (mean, rms) = mean_and_spread(values, weights);
    for v in values.iter_mut() {
        *v = if rms > 0.0 { (*v - mean) / rms } else { 0.0 };
    }
}

/// The area-weighted mean and standard deviation of `values` under `weights`, summed in index
/// order.
///
/// # Panics
///
/// If the weights sum to zero.
#[must_use]
pub(super) fn mean_and_spread(values: &[f64], weights: &[f64]) -> (f64, f64) {
    let total: f64 = weights.iter().sum();
    assert!(total > 0.0, "a field's cells have an area");
    let mean = values.iter().zip(weights).map(|(v, w)| v * w).sum::<f64>() / total;
    let variance = values
        .iter()
        .zip(weights)
        .map(|(v, w)| (v - mean) * (v - mean) * w)
        .sum::<f64>()
        / total;
    (mean, variance.sqrt())
}

/// One octave over `pass`'s cells, at lattice level `lattice_level`, instance `instance`:
/// zero-mean and unit-RMS over the sphere.
///
/// # Panics
///
/// If a cell's kernel reaches no lattice centre, which [`KERNEL_REACH`] rules out.
#[must_use]
fn octave(pass: &Pass<'_>, instance: u16, lattice_level: u8) -> Vec<f64> {
    let grid = pass.grid();
    let owned;
    let lattice = if lattice_level == grid.level() {
        grid
    } else {
        owned = CellGraph::new(lattice_level).expect("a lattice level is a coarse grid's");
        &owned
    };
    let values: Vec<f64> = lattice
        .cells()
        .iter()
        .map(|cell| {
            pass.seed()
                .stream(
                    SURFACE_COARSE_RELIEF,
                    ObjectKey::surface_cell(
                        cell.face().index(),
                        cell.level(),
                        cell.i(),
                        cell.j(),
                        instance,
                    )
                    .expect("a lattice cell is a surface cell"),
                )
                .standard_normal()
        })
        .collect();
    let reach = KERNEL_REACH * math::powf(0.5, f64::from(lattice_level));
    let reach2 = reach * reach;
    let shift = 2 * u32::from(grid.level() - lattice_level);
    let mut home = u32::MAX;
    let mut candidates: Vec<u32> = Vec::new();
    let mut field: Vec<f64> = grid
        .centres()
        .iter()
        .enumerate()
        .map(|(n, &point)| {
            let own = u32::try_from(n).expect("a coarse level has under 2^32 cells") >> shift;
            if own != home {
                home = own;
                neighbourhood(lattice, home, &mut candidates);
            }
            let (mut sum, mut weight) = (0.0, 0.0);
            for &m in &candidates {
                let centre = lattice.centre(m);
                let offset = [
                    point[0] - centre[0],
                    point[1] - centre[1],
                    point[2] - centre[2],
                ];
                let chord2 = offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2];
                if chord2 >= reach2 {
                    continue;
                }
                let fall = 1.0 - chord2 / reach2;
                let kernel = fall * fall * fall;
                sum += kernel * values[slot(m)];
                weight += kernel;
            }
            assert!(weight > 0.0, "a noise kernel reaches a lattice centre");
            sum / weight
        })
        .collect();
    standardise(&mut field, grid.solid_angles_sr());
    field
}

/// The lattice cells within [`RINGS`] rings of `home` on `lattice`, through edge and vertex
/// neighbours, `home` included, ascending, written into `out`.
fn neighbourhood(lattice: &CellGraph, home: u32, out: &mut Vec<u32>) {
    out.clear();
    out.push(home);
    let mut frontier_start = 0;
    for _ in 0..RINGS {
        let frontier_end = out.len();
        for k in frontier_start..frontier_end {
            let cell = out[k];
            for m in lattice
                .edge_neighbours(cell)
                .into_iter()
                .chain(lattice.vertex_neighbours(cell))
            {
                if !out.contains(&m) {
                    out.push(m);
                }
            }
        }
        frontier_start = frontier_end;
    }
    out.sort_unstable();
}
