//! The model grids' hypsometry (R09.T13.g; decision-r09-t13a-relief §1.2, decision-r09-t13ge
//! §1.4): each grid point's surface heights above the sea level as equal-area quantiles of the
//! field cells it holds, and its flooded cells' mean water-surface height, which the
//! energy-balance model's relief terms read ([`ebm`](super::ebm)).
//!
//! # The heights
//!
//! A field cell's height is its surface's above the sea level, z = `water_surface` −
//! `sea_level`: the ground's on dry land and the water's surface over water, zero over R09.T12.e's
//! ocean, whose water surface is the sea level (decision-r09-t12-coarse R8). Until R09.T12.e every
//! world is dry and its sea level is its lowest elevation, so every height is at least zero; an
//! enclosed depression left dry below the sea level has a negative one. Every height is asserted
//! finite before it is sorted.
//!
//! # The quantiles
//!
//! Each cell belongs to the point of its centre's latitude and longitude in the model's coordinates
//! ([`ModelGrid::point_of`]), as the sea shares bin it. A point's cells, sorted by (height, cell
//! index), give its [`QUANTILES`] heights: quantile q is the height of the first cell at which the
//! cumulative solid angle reaches (q + ½) ÷ 8 of the point's, the midpoint rule in probability
//! (decision-r09-t13ge §1.4, g3: eight, at which Antarctica's own quadrature is within 0.03 K of
//! sixty-four's). Each point also keeps `z̄_w`, the solid-angle mean height of the water's surface
//! over its flooded cells ([`seas::under_sea`]), zero where it has none (g2): the sea's ice reads
//! the water's own surface, not the land's heights.
//!
//! # Points that hold no cell
//!
//! On a grid of more than one column (the geographic 72 × 72 and the locked 72 × 144) a point that
//! holds no cell's centre takes the cell that holds its own centre, every quantile that cell's
//! height (R09.T13.b's sea-share rule, `point_sea_shares`); on the zonal grid a band that holds none
//! takes the nearest band's that does, equatorward first (R09.T13.a's, `sea_shares`), as g5 rules.
//! A field of no cells is at the sea level.
//!
//! # Shapes
//!
//! A point whose quantiles and `z̄_w` are all zero is at the sea level ([`Shape::SeaLevel`]): the
//! model takes T13.a's arithmetic there, so a world whose every height is zero gives T13.a's output
//! to the bit. A point whose quantiles are all exactly equal is [`Shape::Level`], evaluated once.

use hyperion_surface::cube::PatchKey;
use hyperion_surface::field::cell_index;
use hyperion_surface::num::assert_finite;

use super::super::{Pass, Working};
use super::coordinates::Coordinates;
use super::grid::{LATITUDE_BANDS, ModelGrid, ZONAL_COLUMNS};
use super::seas;

/// The equal-area quantiles a grid point's heights are held at: eight (decision-r09-t13a-relief
/// §1.2, the lane's floor; decision-r09-t13ge §1.4, g3, sixteen not required).
pub(super) const QUANTILES: usize = 8;

/// How a point's heights are spread, which sets how the model evaluates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Shape {
    /// Every quantile and `z̄_w` zero: T13.a's arithmetic at the sea-level temperature.
    SeaLevel,
    /// Every quantile exactly equal, but not every height zero: evaluated once, at that height.
    Level,
    /// The quantiles differ: evaluated at each and averaged.
    Varied,
}

/// One grid point's heights above the sea level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PointHeights {
    /// The [`QUANTILES`] equal-area heights, m, ascending.
    pub(super) quantiles_m: [f64; QUANTILES],
    /// `z̄_w`, the solid-angle mean height of the water's surface over the point's flooded cells, m;
    /// zero where it has none.
    pub(super) water_m: f64,
    /// How the heights are spread.
    pub(super) shape: Shape,
}

impl PointHeights {
    /// The point of quantiles `quantiles_m`, m, ascending, and `z̄_w` `water_m`, m, its shape found
    /// by exact comparison, a zero of either sign being zero.
    ///
    /// # Panics
    ///
    /// If a height is not finite.
    #[must_use]
    pub(super) fn new(quantiles_m: [f64; QUANTILES], water_m: f64) -> Self {
        let first = assert_finite(quantiles_m[0]);
        let level = quantiles_m.iter().all(|&z| {
            let z = assert_finite(z);
            z.total_cmp(&first).is_eq() || (is_zero(z) && is_zero(first))
        });
        let shape = if level && is_zero(first) && is_zero(assert_finite(water_m)) {
            Shape::SeaLevel
        } else if level {
            Shape::Level
        } else {
            Shape::Varied
        };
        Self {
            quantiles_m,
            water_m,
            shape,
        }
    }

    /// Every height at `z_m`, m, and `z̄_w` `water_m`, m: one cell's heights, or a band's held level.
    #[must_use]
    pub(super) fn level(z_m: f64, water_m: f64) -> Self {
        Self::new([z_m; QUANTILES], water_m)
    }
}

/// Whether `x` is zero, of either sign.
#[must_use]
fn is_zero(x: f64) -> bool {
    x.abs().total_cmp(&0.0).is_eq()
}

/// The heights of every point of a model grid, in its points' order.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Hypsometry {
    points: Vec<PointHeights>,
}

impl Hypsometry {
    /// The hypsometry of `points`, one per grid point in the grid's order.
    #[must_use]
    pub(super) fn new(points: Vec<PointHeights>) -> Self {
        Self { points }
    }

    /// The hypsometry of `working`'s field on `grid` in `coordinates` (the
    /// [module documentation](self)): each point's quantiles of its cells' heights above the sea
    /// level and its flooded cells' mean water-surface height, a point that holds no cell taking
    /// the cell at its centre, or on the zonal grid the nearest band's, equatorward first.
    ///
    /// # Panics
    ///
    /// If a cell's height is not finite, or a point's centre lies in no cell of the field's grid,
    /// which a coarse level rules out.
    #[must_use]
    pub(super) fn of_field(
        pass: &Pass<'_>,
        working: &Working,
        grid: &ModelGrid,
        coordinates: Coordinates,
    ) -> Self {
        let field = pass.grid();
        let sea = working.sea_level.value();
        let heights: Vec<f64> = working
            .cells
            .iter()
            .map(|cell| assert_finite(cell.water_surface.value() - sea))
            .collect();
        let point_of: Vec<usize> = field
            .centres()
            .iter()
            .take(heights.len())
            .map(|centre| grid.point_of(coordinates.to_model(*centre)))
            .collect();
        // Each point's cells, in cell-index order: a counting sort by point.
        let points = grid.points();
        let mut start = vec![0_usize; points + 1];
        for &point in &point_of {
            start[point + 1] += 1;
        }
        for point in 0..points {
            start[point + 1] += start[point];
        }
        let mut next = start.clone();
        let mut order = vec![0_usize; point_of.len()];
        for (cell, &point) in point_of.iter().enumerate() {
            order[next[point]] = cell;
            next[point] += 1;
        }
        let solid_angles = field.solid_angles_sr();
        let held: Vec<Option<PointHeights>> = (0..points)
            .map(|point| {
                let cells = &mut order[start[point]..start[point + 1]];
                (!cells.is_empty()).then(|| {
                    point_heights(cells, &heights, solid_angles, |cell| {
                        seas::under_sea(&working.cells[cell])
                    })
                })
            })
            .collect();
        let zonal = grid.columns() == ZONAL_COLUMNS;
        let fallback = |point: usize| -> PointHeights {
            if zonal {
                let found: Vec<bool> = held.iter().map(Option::is_some).collect();
                return nearest_held(point, &found)
                    .and_then(|band| held[band])
                    .unwrap_or_else(|| PointHeights::level(0.0, 0.0));
            }
            let direction = coordinates.to_body(grid.centre_direction(point));
            let key = PatchKey::containing(field.level(), direction)
                .expect("the field's grid is at a coarse level");
            let index = usize::try_from(cell_index(key)).expect("a cell index fits a usize");
            match working.cells.get(index) {
                Some(cell) => {
                    let z = heights[index];
                    PointHeights::level(z, if seas::under_sea(cell) { z } else { 0.0 })
                }
                None => PointHeights::level(0.0, 0.0),
            }
        };
        Self::new(
            held.iter()
                .enumerate()
                .map(|(point, heights)| heights.unwrap_or_else(|| fallback(point)))
                .collect(),
        )
    }

    /// The grid's points.
    #[cfg(test)]
    #[must_use]
    pub(super) fn points(&self) -> usize {
        self.points.len()
    }

    /// Point `point`'s heights.
    #[must_use]
    pub(super) fn point(&self, point: usize) -> &PointHeights {
        &self.points[point]
    }

    /// Every point's heights, in the grid's order.
    #[must_use]
    pub(super) fn all(&self) -> &[PointHeights] {
        &self.points
    }

    /// The least and the greatest of the points' quantiles, m: (0, 0) at the sea level.
    #[must_use]
    pub(super) fn extremes_m(&self) -> (f64, f64) {
        self.points
            .iter()
            .flat_map(|point| point.quantiles_m)
            .fold((0.0, 0.0), |(low, high), z| {
                (
                    if z < low { z } else { low },
                    if z > high { z } else { high },
                )
            })
    }
}

/// The heights of a point of cells `cells` (indices, in cell-index order, which it sorts by
/// (height, index)), each of height `heights[cell]` and solid angle `solid_angles[cell]`, flooded
/// where `flooded(cell)`: its quantiles at cumulative solid angle (q + ½) ÷ [`QUANTILES`] and `z̄_w`,
/// its sums in cell-index order but the quantiles' cumulative one, in height order.
///
/// # Panics
///
/// If `cells` is empty.
#[must_use]
fn point_heights(
    cells: &mut [usize],
    heights: &[f64],
    solid_angles: &[f64],
    flooded: impl Fn(usize) -> bool,
) -> PointHeights {
    assert!(!cells.is_empty(), "a point's heights need a cell");
    let (water, wet) = cells.iter().fold((0.0, 0.0), |(water, wet), &cell| {
        if flooded(cell) {
            let omega = solid_angles[cell];
            (water + omega * heights[cell], wet + omega)
        } else {
            (water, wet)
        }
    });
    let water_m = if wet > 0.0 { water / wet } else { 0.0 };
    cells.sort_by(|&a, &b| heights[a].total_cmp(&heights[b]).then(a.cmp(&b)));
    let total = cells
        .iter()
        .fold(0.0, |sum, &cell| sum + solid_angles[cell]);
    let mut quantiles_m = [heights[cells[cells.len() - 1]]; QUANTILES];
    let (mut q, mut cumulative) = (0, 0.0);
    for &cell in cells.iter() {
        cumulative += solid_angles[cell];
        while q < QUANTILES && cumulative >= quantile_share(q) * total {
            quantiles_m[q] = heights[cell];
            q += 1;
        }
        if q == QUANTILES {
            break;
        }
    }
    PointHeights::new(quantiles_m, water_m)
}

/// The cumulative share of a point's solid angle at which its quantile `q` lies, (q + ½) ÷
/// [`QUANTILES`].
#[must_use]
fn quantile_share(q: usize) -> f64 {
    (super::grid::index(q) + 0.5) / super::grid::index(QUANTILES)
}

/// The nearest band to `band` of those `held` marks, equatorward first, as the zonal model's sea
/// shares take it (R09.T13.a); `None` where none is held.
#[must_use]
fn nearest_held(band: usize, held: &[bool]) -> Option<usize> {
    if held.get(band) == Some(&true) {
        return Some(band);
    }
    (1..LATITUDE_BANDS).find_map(|distance| {
        let (equatorward, poleward) = if band < LATITUDE_BANDS / 2 {
            (band.checked_add(distance), band.checked_sub(distance))
        } else {
            (band.checked_sub(distance), band.checked_add(distance))
        };
        [equatorward, poleward]
            .into_iter()
            .flatten()
            .find(|&b| held.get(b) == Some(&true))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math;
    use crate::planetary::surface::grid::CellGraph;
    use crate::planetary::surface::reference;
    use crate::planetary::surface::steps::climate::tests::set_ground;
    use crate::units::Metres;
    use hyperion_surface::field::CoarseLevel;
    use hyperion_testkit::float::assert_same_bits;

    /// The grids of a coarse level `level`.
    fn grids(level: u8) -> (CellGraph, CellGraph) {
        let level = CoarseLevel::new(level).expect("a coarse level");
        (CellGraph::for_field(level), CellGraph::for_climate(level))
    }

    /// A hand-set point: sixteen cells, the first twelve of solid angle 1 at heights 0, 100, …,
    /// 1,100 m in reverse index order, and four of solid angle 3 at 2,000 m: the quantiles at
    /// (q + ½) ÷ 8 of 24 are the first cells whose cumulative solid angle reaches 1.5, 4.5, 7.5,
    /// 10.5, 13.5, …; the flooded cells' mean water surface is their solid-angle mean.
    #[test]
    fn a_point_s_quantiles_are_its_cells_at_equal_shares_of_its_solid_angle() {
        let heights: Vec<f64> = (0..12)
            .map(|i| 100.0 * f64::from(11 - i))
            .chain([2_000.0; 4])
            .collect();
        let solid_angles: Vec<f64> = [1.0; 12].into_iter().chain([3.0; 4]).collect();
        let mut cells: Vec<usize> = (0..16).collect();
        let point = point_heights(&mut cells, &heights, &solid_angles, |cell| {
            cell == 11 || cell == 12
        });
        // Sorted, the cumulative solid angle is 1, 2, …, 12 at 0–1,100 m, then 15, 18, 21, 24.
        assert_eq!(
            point.quantiles_m,
            [
                100.0, 400.0, 700.0, 1_000.0, 2_000.0, 2_000.0, 2_000.0, 2_000.0
            ]
        );
        assert_eq!(point.shape, Shape::Varied);
        // Cell 11 at 0 m of solid angle 1 and cell 12 at 2,000 m of 3.
        assert_same_bits(point.water_m, 3.0 * 2_000.0 / 4.0);
        // Ties sort by index, and equal heights make a level point; zero heights the sea level.
        let mut tied: Vec<usize> = vec![3, 1, 2, 0];
        let level = point_heights(&mut tied, &[500.0; 4], &[1.0; 4], |_| false);
        assert_eq!(tied, vec![0, 1, 2, 3]);
        assert_eq!(level.shape, Shape::Level);
        assert_eq!(
            point_heights(&mut [0, 1], &[0.0, -0.0], &[1.0, 1.0], |_| true).shape,
            Shape::SeaLevel
        );
        assert_eq!(PointHeights::level(0.0, 50.0).shape, Shape::Level);
        assert_eq!(
            PointHeights::new([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1e-300], 0.0).shape,
            Shape::Varied
        );
    }

    /// A band that holds no cell takes the nearest that does, equatorward first, in either
    /// hemisphere; with none held there is none.
    #[test]
    fn an_empty_band_takes_the_nearest_held_band_equatorward_first() {
        let mut held = [true; LATITUDE_BANDS];
        held[0] = false;
        held[1] = false;
        held[70] = false;
        held[71] = false;
        held[34] = false;
        held[37] = false;
        assert_eq!(nearest_held(0, &held), Some(2));
        assert_eq!(nearest_held(71, &held), Some(69));
        // Band 34 is southern: equatorward is 35, as far as 33.
        assert_eq!(nearest_held(34, &held), Some(35));
        // Band 37 is northern: equatorward is 36.
        assert_eq!(nearest_held(37, &held), Some(36));
        assert_eq!(nearest_held(5, &held), Some(5));
        assert_eq!(nearest_held(3, &[false; LATITUDE_BANDS]), None);
    }

    /// On the geographic and locked grids at level 5, where about a fifth of the points hold no
    /// cell, each empty point takes the heights of the cell at its centre, its water surface
    /// over it if flooded; a held point's quantiles lie among its own cells' heights.
    #[test]
    fn an_empty_point_takes_the_cell_at_its_centre() {
        let inputs = reference::earth_like();
        let (field, climate) = grids(5);
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
        let mut working = Working::initial(&pass);
        let flood = inputs.main_liquid();
        for (n, (cell, centre)) in working.cells.iter_mut().zip(field.centres()).enumerate() {
            let longitude = math::atan2(centre[1], centre[0]);
            let ground = 3_000.0 * math::sin(3.0 * longitude)
                + 7.0 * f64::from(u32::try_from(n % 13).unwrap());
            set_ground(cell, Metres::new(ground), Metres::new(500.0), flood);
        }
        working.sea_level = Metres::new(500.0);
        for (grid, coordinates) in [
            (ModelGrid::geographic(), Coordinates::Geographic),
            (ModelGrid::locked(), Coordinates::TidallyLocked),
        ] {
            let hypsometry = Hypsometry::of_field(&pass, &working, &grid, coordinates);
            assert_eq!(hypsometry.points(), grid.points());
            let mut holds: Vec<Vec<usize>> = vec![Vec::new(); grid.points()];
            for (cell, centre) in field.centres().iter().enumerate() {
                holds[grid.point_of(coordinates.to_model(*centre))].push(cell);
            }
            let mut empty = 0;
            for (point, cells) in holds.iter().enumerate() {
                let heights = hypsometry.point(point);
                let z = |cell: usize| working.cells[cell].water_surface.value() - 500.0;
                if cells.is_empty() {
                    empty += 1;
                    let key = PatchKey::containing(
                        field.level(),
                        coordinates.to_body(grid.centre_direction(point)),
                    )
                    .unwrap();
                    let cell = usize::try_from(cell_index(key)).unwrap();
                    for q in heights.quantiles_m {
                        assert_same_bits(q, z(cell));
                    }
                    let wet = seas::under_sea(&working.cells[cell]);
                    assert_same_bits(heights.water_m, if wet { z(cell) } else { 0.0 });
                } else {
                    for q in heights.quantiles_m {
                        assert!(cells.iter().any(|&c| z(c).total_cmp(&q).is_eq()));
                    }
                }
            }
            eprintln!(
                "{coordinates:?}: {empty} of {} points hold no cell",
                grid.points()
            );
            assert!(empty > grid.points() / 8, "{empty}");
        }
    }

    /// The zonal grid pools each band's quantiles over all of its cells, every longitude: a
    /// height that rises uniformly with longitude, 0 to 4,000 m round each band, gives each band
    /// the quantiles 4,000 m × (q + ½) ÷ 8 within a cell's span; a field at its sea level, and a
    /// sea flooded to it, is at the sea level everywhere.
    #[test]
    fn the_zonal_grid_pools_each_band_s_cells() {
        let inputs = reference::earth_like();
        let (field, climate) = grids(6);
        let pass = Pass::new(reference::SEED, &inputs, &field, &climate);
        let mut working = Working::initial(&pass);
        let zonal = ModelGrid::zonal();
        let flat = Hypsometry::of_field(&pass, &working, &zonal, Coordinates::Geographic);
        assert!(flat.all().iter().all(|p| p.shape == Shape::SeaLevel));
        assert_eq!(flat.extremes_m(), (0.0, 0.0));
        let flood = inputs.main_liquid();
        for (cell, centre) in working.cells.iter_mut().zip(field.centres()) {
            let longitude = math::atan2(centre[1], centre[0]);
            let ground = 4_000.0 * (longitude + core::f64::consts::PI) / core::f64::consts::TAU;
            set_ground(cell, Metres::new(ground), Metres::ZERO, flood);
        }
        let pooled = Hypsometry::of_field(&pass, &working, &zonal, Coordinates::Geographic);
        let mut worst: f64 = 0.0;
        // Away from the poles' few cells.
        for band in 8..LATITUDE_BANDS - 8 {
            let heights = pooled.point(band);
            for (q, &z) in heights.quantiles_m.iter().enumerate() {
                let expected = 4_000.0 * quantile_share(q);
                worst = worst.max((z - expected).abs());
            }
            assert_eq!(heights.shape, Shape::Varied);
        }
        eprintln!("the pooled quantiles' worst departure: {worst:.1} m");
        // A cell at level 6 spans about 1.4° of longitude at the equator, 16 m of the ramp, and
        // under 3° at 70°.
        assert!(worst < 40.0, "{worst}");
        for cell in &mut working.cells {
            set_ground(cell, Metres::new(-200.0), Metres::ZERO, flood);
        }
        let sea = Hypsometry::of_field(&pass, &working, &zonal, Coordinates::Geographic);
        assert!(sea.all().iter().all(|p| p.shape == Shape::SeaLevel));
    }
}
