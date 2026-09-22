//! The census rule: which layers a query admits, decided before anything is generated (plan 03,
//! P03.T9.d; Design notes 8–10).

use super::request::RangeQuery;
use super::result::{Census, CensusStop, LayerCounts};
use crate::galaxy::placement::STELLAR_LAYERS;
use crate::id::Layer;

/// The census of a query: how far down the layers its result is complete, and why it stops there.
///
/// `grid` is the grid's expected count per layer over the unpadded sphere and `sources` the merged
/// sources' together, summed from [`LayerCounts::ZERO`] in the order the sources are listed; the
/// two are added layer by layer once. The layers are then tried from E to A. Each adds its
/// expected count to a running total that starts at 0 and runs in that order (Design note 8; the
/// order is part of the output, as plan 02's D18 fixes the order of its component sums), and its
/// cells in the padded sphere, from `cells_in_layer`, to a running count of cells. The walk stops
/// at the first of these that holds:
///
/// 1. the layer's addition would take the expected total above the query's
///    [`limit`](RangeQuery::limit) ([`CensusStop::Limit`]); a total equal to the limit fits;
/// 2. the layer's cells would take the running count above the query's
///    [`cell_budget`](RangeQuery::cell_budget) ([`CensusStop::CellBudget`]);
///
/// and that layer is left out with every finer one, even if a finer one alone would fit, because
/// the result must be statable as "complete above m" (Design note 8). Otherwise the walk ends,
/// with the layer admitted, at the layer of the query's [`mass_floor`](RangeQuery::mass_floor)
/// ([`CensusStop::MassFloor`]). If even layer E does not fit, nothing is admitted and nothing is
/// generated.
///
/// The limit bounds expected counts only; the realised count is never truncated (Design note 9).
/// The budget depends only on the query's geometry (Design note 10). Both are decided before any
/// cell is generated, so the census does not depend on what a caller has cached.
/// `cells_in_layer` is called only for the layers the walk reaches, at most once each, so a
/// query stopped at layer D never counts layer A's cells. The layers to walk are
/// [`Census::layers`].
///
/// [`Census::expected`] is `grid + sources` for every layer, walked or not. The substellar
/// entries pass through: they are zero in every M1 query (Design note 17).
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::query::{CensusStop, LayerCounts, RangeQuery, decide_census};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::units::LightYears;
///
/// // A 50 ly query in the bulge, with the default limit of 4,096 systems.
/// let centre = GalacticPosition::from_light_years([1_000.0, 0.0, 0.0]).expect("in range");
/// let query = RangeQuery::builder(centre, LightYears::new(50.0)).build()?;
/// // Expected systems per layer, A to E, then the two substellar layers.
/// let grid = LayerCounts::from_array([90_000.0, 11_000.0, 14_000.0, 2_400.0, 700.0, 0.0, 0.0]);
/// let census = decide_census(&query, &grid, &LayerCounts::ZERO, |_| 1_000);
/// // E and D fit under 4,096; C would not, so the result is complete above 2.5 M☉.
/// assert_eq!(census.complete_down_to(), Some(Layer::D));
/// assert_eq!(census.stopped_by(), CensusStop::Limit);
/// assert!(census.complete_above().is_some_and(|m| m.value() > 2.4 && m.value() < 2.6));
/// # Ok::<(), hyperion_sim::galaxy::query::BuildRangeQueryError>(())
/// ```
#[must_use]
pub fn decide_census(
    query: &RangeQuery,
    grid: &LayerCounts,
    sources: &LayerCounts,
    mut cells_in_layer: impl FnMut(Layer) -> u64,
) -> Census {
    let expected = *grid + *sources;
    let limit = f64::from(query.limit().get());
    let cell_budget = u64::from(query.cell_budget().get());
    let floor = query.mass_floor().layer();
    let mut total = 0.0;
    let mut cells = 0_u64;
    let mut complete_down_to = None;
    let mut stopped_by = CensusStop::MassFloor;
    for spec in &STELLAR_LAYERS {
        let layer = spec.layer();
        let next_total = total + expected.get(layer);
        if next_total > limit {
            stopped_by = CensusStop::Limit;
            break;
        }
        let next_cells = cells.saturating_add(cells_in_layer(layer));
        if next_cells > cell_budget {
            stopped_by = CensusStop::CellBudget;
            break;
        }
        total = next_total;
        cells = next_cells;
        complete_down_to = Some(layer);
        if layer == floor {
            break;
        }
    }
    Census::new(complete_down_to, stopped_by, expected)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::{GalacticPosition, LyCell};
    use crate::galaxy::placement::layer_spec;
    use crate::galaxy::query::request::MassFloor;
    use crate::galaxy::query::result::LayerSet;
    use crate::units::LightYears;

    /// Counts for layers A to E, the substellar entries zero.
    fn counts(stellar: [f64; 5]) -> LayerCounts {
        let mut all = [0.0; 7];
        all[..5].copy_from_slice(&stellar);
        LayerCounts::from_array(all)
    }

    /// A 50 ly query at a point in the disc with the given limits.
    fn query(limit: u32, cell_budget: u32, floor: MassFloor) -> RangeQuery {
        let centre = GalacticPosition::new(LyCell::new([0, 26_000, 0]), [0.0; 3]).unwrap();
        RangeQuery::builder(centre, LightYears::new(50.0))
            .limit(NonZeroU32::new(limit).unwrap())
            .cell_budget(NonZeroU32::new(cell_budget).unwrap())
            .mass_floor(floor)
            .build()
            .unwrap()
    }

    /// The default limit and budget.
    fn default_query(floor: MassFloor) -> RangeQuery {
        query(4_096, 1 << 20, floor)
    }

    /// Cells per layer for A to E.
    fn cells(stellar: [u64; 5]) -> impl Fn(Layer) -> u64 {
        move |layer| {
            assert!(
                layer_spec(layer).is_some(),
                "no substellar layer is walked in M1"
            );
            stellar[usize::from(layer.value())]
        }
    }

    const FEW_CELLS: [u64; 5] = [1_400, 240, 50, 14, 5];

    /// The census under the default limit and budget and no sources.
    fn take(grid: [f64; 5], floor: MassFloor) -> Census {
        decide_census(
            &default_query(floor),
            &counts(grid),
            &LayerCounts::ZERO,
            cells(FEW_CELLS),
        )
    }

    #[test]
    fn every_layer_fits_down_to_the_floor() {
        let census = take([1_100.0, 190.0, 240.0, 40.0, 12.0], MassFloor::LayerA);
        assert_eq!(census.complete_down_to(), Some(Layer::A));
        assert_eq!(census.stopped_by(), CensusStop::MassFloor);
        assert_eq!(
            census.layers(),
            [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn b_fitting_alone_after_c_fails_is_still_excluded() {
        // E and D fit (3,000); C would take the total to 5,000; B alone would bring it to 3,010.
        let census = take(
            [500_000.0, 10.0, 2_000.0, 2_000.0, 1_000.0],
            MassFloor::LayerA,
        );
        assert_eq!(census.complete_down_to(), Some(Layer::D));
        assert_eq!(census.stopped_by(), CensusStop::Limit);
        assert!(!census.layers().contains(Layer::B));
    }

    #[test]
    fn the_floor_stops_before_the_limit() {
        // Layer B would break the limit, but the floor stops the walk at C first.
        let census = take([0.0, 90_000.0, 3_000.0, 500.0, 100.0], MassFloor::LayerC);
        assert_eq!(census.complete_down_to(), Some(Layer::C));
        assert_eq!(census.stopped_by(), CensusStop::MassFloor);
        let census = take([1.0, 1.0, 1.0, 1.0, 1.0], MassFloor::LayerE);
        assert_eq!(census.complete_down_to(), Some(Layer::E));
        assert_eq!(census.stopped_by(), CensusStop::MassFloor);
    }

    #[test]
    fn the_cell_budget_can_stop_first() {
        // A sparse halo sphere: every count is tiny, but layer B's cells overrun the budget, and
        // the limit would never have stopped the walk.
        let census = decide_census(
            &default_query(MassFloor::LayerA),
            &counts([30.0, 4.0, 5.0, 1.0, 0.3]),
            &LayerCounts::ZERO,
            cells([8_000_000, 1_000_000, 125_000, 16_000, 2_000]),
        );
        assert_eq!(census.complete_down_to(), Some(Layer::C));
        assert_eq!(census.stopped_by(), CensusStop::CellBudget);
        // A smaller budget stops the walk earlier.
        let census = decide_census(
            &query(4_096, 17_000, MassFloor::LayerA),
            &counts([30.0, 4.0, 5.0, 1.0, 0.3]),
            &LayerCounts::ZERO,
            cells([8_000_000, 1_000_000, 125_000, 16_000, 2_000]),
        );
        assert_eq!(census.complete_down_to(), Some(Layer::E));
        assert_eq!(census.stopped_by(), CensusStop::CellBudget);
    }

    #[test]
    fn the_limit_is_checked_before_the_budget_at_the_same_layer() {
        let census = decide_census(
            &default_query(MassFloor::LayerA),
            &counts([0.0, 0.0, 0.0, 0.0, 5_000.0]),
            &LayerCounts::ZERO,
            cells([0, 0, 0, 0, 2 << 20]),
        );
        assert_eq!(census.stopped_by(), CensusStop::Limit);
    }

    #[test]
    fn sources_can_tip_a_layer_over_the_limit() {
        let grid = counts([0.0, 0.0, 900.0, 3_000.0, 100.0]);
        let floor_c = default_query(MassFloor::LayerC);
        let alone = decide_census(&floor_c, &grid, &LayerCounts::ZERO, cells(FEW_CELLS));
        assert_eq!(alone.complete_down_to(), Some(Layer::C));
        let sources = counts([0.0, 0.0, 200.0, 0.0, 0.0]);
        let census = decide_census(&floor_c, &grid, &sources, cells(FEW_CELLS));
        assert_eq!(census.complete_down_to(), Some(Layer::D));
        assert_eq!(census.stopped_by(), CensusStop::Limit);
        assert_same_bits(census.expected().get(Layer::C), 1_100.0);
    }

    #[test]
    fn when_nothing_fits_nothing_is_admitted() {
        let census = take([0.0, 0.0, 0.0, 0.0, 30_000_000.0], MassFloor::LayerA);
        assert_eq!(census.complete_down_to(), None);
        assert_eq!(census.complete_above(), None);
        assert_eq!(census.stopped_by(), CensusStop::Limit);
        assert_eq!(census.layers(), LayerSet::EMPTY);
        let census = decide_census(
            &default_query(MassFloor::LayerA),
            &counts([1.0; 5]),
            &LayerCounts::ZERO,
            cells([1; 5].map(|n: u64| n << 21)),
        );
        assert_eq!(census.complete_down_to(), None);
        assert_eq!(census.stopped_by(), CensusStop::CellBudget);
    }

    #[test]
    fn a_total_equal_to_the_limit_fits() {
        let census = take([0.0, 0.0, 0.0, 4_000.0, 96.0], MassFloor::LayerA);
        assert_eq!(census.complete_down_to(), Some(Layer::A));
        let census = take([0.0, 0.0, 1.0, 4_000.0, 96.0], MassFloor::LayerA);
        assert_eq!(census.complete_down_to(), Some(Layer::D));
        assert_eq!(census.stopped_by(), CensusStop::Limit);
    }

    #[test]
    fn the_substellar_entries_are_zero_in_and_out() {
        for floor in [MassFloor::LayerA, MassFloor::LayerC, MassFloor::LayerE] {
            let census = decide_census(
                &default_query(floor),
                &counts([1_000.0, 200.0, 300.0, 40.0, 10.0]),
                &counts([5.0, 1.0, 1.0, 0.5, 0.1]),
                cells(FEW_CELLS),
            );
            let expected = census.expected().to_array();
            assert_same_bits(expected[usize::from(Layer::BrownDwarf.value())], 0.0);
            assert_same_bits(expected[usize::from(Layer::RoguePlanet.value())], 0.0);
            assert!(!census.layers().contains(Layer::BrownDwarf));
            assert!(!census.layers().contains(Layer::RoguePlanet));
        }
    }

    #[test]
    fn expected_holds_every_layer_and_cells_are_counted_only_where_the_walk_reaches() {
        let asked = std::cell::RefCell::new(Vec::new());
        let census = decide_census(
            &default_query(MassFloor::LayerA),
            &counts([90_000.0, 11_000.0, 14_000.0, 2_400.0, 700.0]),
            &LayerCounts::ZERO,
            |layer| {
                asked.borrow_mut().push(layer);
                10
            },
        );
        assert_eq!(census.complete_down_to(), Some(Layer::D));
        assert_eq!(asked.into_inner(), [Layer::E, Layer::D]);
        // Layers left out are still reported.
        assert_same_bits(census.expected().get(Layer::A), 90_000.0);
    }

    #[test]
    fn the_census_is_the_same_on_repeated_calls() {
        let grid = [1_234.5, 210.25, 260.125, 40.062_5, 11.031_25];
        let first = take(grid, MassFloor::LayerA);
        let second = take(grid, MassFloor::LayerA);
        assert_eq!(first.complete_down_to(), second.complete_down_to());
        assert_eq!(first.stopped_by(), second.stopped_by());
        for (a, b) in first
            .expected()
            .to_array()
            .into_iter()
            .zip(second.expected().to_array())
        {
            assert_same_bits(a, b);
        }
    }
}
