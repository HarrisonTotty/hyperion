//! What a range query returns besides its systems: the census, the expected counts per layer, the
//! set of layers walked and the query's statistics.

use std::ops::Add;

use crate::coords::GalacticPosition;
use crate::galaxy::placement::{STELLAR_LAYERS, SystemRecord, layer_spec};
use crate::id::{Layer, SystemId};
use crate::units::{LightYears, SolarMasses};

/// An expected number of systems per layer, indexed by [`Layer::value`]: the five stellar layers
/// and the two substellar ones.
///
/// Every entry is a finite, non-negative number of systems, which [`from_array`](Self::from_array)
/// and [`set`](Self::set) enforce. Every count M1 produces leaves the two substellar entries
/// exactly zero, so that plan 13 fills two entries that were always zero and changes neither this
/// type nor any census taken before it (plan 03, Design note 17); the census rule's tests pin it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayerCounts([f64; 7]);

impl LayerCounts {
    /// Zero in every layer.
    pub const ZERO: Self = Self([0.0; 7]);

    /// The counts `counts[layer.value()]`.
    ///
    /// # Panics
    ///
    /// If a count is NaN, infinite or negative: an expected count that is not a number of systems
    /// is a bug in whoever computed it.
    #[must_use]
    pub fn from_array(counts: [f64; 7]) -> Self {
        for count in counts {
            assert_expected_count(count);
        }
        Self(counts)
    }

    /// The counts as an array indexed by [`Layer::value`].
    #[must_use]
    pub const fn to_array(self) -> [f64; 7] {
        self.0
    }

    /// The expected count in `layer`.
    #[must_use]
    pub fn get(&self, layer: Layer) -> f64 {
        self.0[usize::from(layer.value())]
    }

    /// Sets the expected count in `layer`.
    ///
    /// # Panics
    ///
    /// If `count` is NaN, infinite or negative, as [`from_array`](Self::from_array).
    pub fn set(&mut self, layer: Layer, count: f64) {
        assert_expected_count(count);
        self.0[usize::from(layer.value())] = count;
    }
}

/// Panics unless `count` is a finite, non-negative expected number of systems.
fn assert_expected_count(count: f64) {
    assert!(
        count.is_finite() && count >= 0.0,
        "an expected count of systems must be finite and non-negative, not {count}"
    );
}

impl Add for LayerCounts {
    type Output = Self;

    /// The layers' counts added one by one; a sum too large for an `f64` is infinite.
    fn add(self, rhs: Self) -> Self {
        let mut sum = self.0;
        for (total, count) in sum.iter_mut().zip(rhs.0) {
            *total += count;
        }
        Self(sum)
    }
}

/// A set of layers: which ones a query walks, or which ones a source is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LayerSet(u8);

impl LayerSet {
    /// No layer.
    pub const EMPTY: Self = Self(0);

    /// This set with `layer` added.
    #[must_use]
    pub const fn with(self, layer: Layer) -> Self {
        Self(self.0 | 1 << layer.value())
    }

    /// Whether `layer` is in the set.
    #[must_use]
    pub const fn contains(self, layer: Layer) -> bool {
        self.0 & 1 << layer.value() != 0
    }

    /// Whether the set holds no layer.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The layers in the set, in [`Layer::ALL`] order: A to E, then the substellar layers.
    pub fn iter(self) -> impl Iterator<Item = Layer> {
        Layer::ALL
            .into_iter()
            .filter(move |&layer| self.contains(layer))
    }
}

impl FromIterator<Layer> for LayerSet {
    fn from_iter<I: IntoIterator<Item = Layer>>(layers: I) -> Self {
        layers.into_iter().fold(Self::EMPTY, Self::with)
    }
}

/// Which rule ended a census's walk down the layers (plan 03, Design notes 8 and 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CensusStop {
    /// Every layer down to the query's mass floor fits.
    MassFloor,
    /// The next layer's expected count would take the running total past the query's limit.
    Limit,
    /// The next layer's cells would take the running count of cells past the query's cell
    /// budget.
    CellBudget,
}

/// The order a query walks the layers in: E to A ([`STELLAR_LAYERS`]), then the brown dwarfs and the
/// rogue planets, which come after layer A and only when asked for (brainstorm, "The range query").
const WALK_ORDER: [Layer; 7] = [
    STELLAR_LAYERS[0].layer(),
    STELLAR_LAYERS[1].layer(),
    STELLAR_LAYERS[2].layer(),
    STELLAR_LAYERS[3].layer(),
    STELLAR_LAYERS[4].layer(),
    Layer::BrownDwarf,
    Layer::RoguePlanet,
];

/// What a range query's result is complete for: every layer from E down to one layer, or none.
///
/// The brainstorm's "a complete census or nothing, per layer": whether a layer fits is decided
/// before anything is generated, from expected counts that do not depend on the cache, and the
/// walk stops at the first layer that does not fit, so the result can always be stated as
/// "complete above m" ([`complete_above`](Self::complete_above)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Census {
    complete_down_to: Option<Layer>,
    stopped_by: CensusStop,
    expected: LayerCounts,
}

impl Census {
    /// A census complete down to `complete_down_to`, stopped by `stopped_by`. Only the census
    /// rule builds one.
    ///
    /// # Panics
    ///
    /// In debug builds, if nothing is complete yet the mass floor is named as the stop: every
    /// floor admits layer E, so a census that admits nothing was stopped by the limit or the
    /// budget.
    #[must_use]
    pub(super) fn new(
        complete_down_to: Option<Layer>,
        stopped_by: CensusStop,
        expected: LayerCounts,
    ) -> Self {
        debug_assert!(
            complete_down_to.is_some() || stopped_by != CensusStop::MassFloor,
            "a census that admits nothing was stopped by the limit or the cell budget"
        );
        Self {
            complete_down_to,
            stopped_by,
            expected,
        }
    }

    /// The finest layer the result holds every system of, or `None` if even layer E does not fit
    /// and the query returned nothing (the caller must shrink the radius).
    #[must_use]
    pub const fn complete_down_to(&self) -> Option<Layer> {
        self.complete_down_to
    }

    /// The primary initial mass above which the result is complete, or `None` when nothing fits.
    ///
    /// It is the lower edge of [`complete_down_to`](Self::complete_down_to)'s band, such as
    /// 0.5 M☉ for layer B.
    #[must_use]
    pub fn complete_above(&self) -> Option<SolarMasses> {
        self.complete_down_to
            .and_then(layer_spec)
            .map(|spec| SolarMasses::new(spec.band().lo()))
    }

    /// Which rule ended the walk.
    #[must_use]
    pub const fn stopped_by(&self) -> CensusStop {
        self.stopped_by
    }

    /// The expected number of systems in the unpadded sphere per layer, the grid's and the
    /// sources' together, for every layer, walked or not.
    #[must_use]
    pub const fn expected(&self) -> &LayerCounts {
        &self.expected
    }

    /// The layers the query walks: every layer of the walk's order, E to A and then the
    /// substellar layers, down to [`complete_down_to`](Self::complete_down_to); none if nothing
    /// fits.
    #[must_use]
    pub fn layers(&self) -> LayerSet {
        let Some(finest) = self.complete_down_to else {
            return LayerSet::EMPTY;
        };
        WALK_ORDER
            .into_iter()
            .take_while(|&layer| layer != finest)
            .chain([finest])
            .collect()
    }
}

/// One system a range query found: the system as placed, and where it was at the query's time.
///
/// The record is the state at the epoch, which is what a cache holds and what an ID resolves to;
/// [`position`](Self::position) and [`distance`](Self::distance) are the only parts that depend on
/// the query's time. The position is the epoch position moved by plan 08's velocity to that time,
/// and the distance is the distance from the sphere's centre to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemHit {
    record: SystemRecord,
    position: GalacticPosition,
    distance: LightYears,
}

impl SystemHit {
    /// A hit from the system, where it was at the query's time, and its distance from the query's
    /// centre there.
    ///
    /// A [`SystemSource`](super::SystemSource) builds its own hits; the grid's come from
    /// [`hit_at`](super::hit_at), which is also what decides whether there is one.
    #[must_use]
    pub const fn new(
        record: SystemRecord,
        position: GalacticPosition,
        distance: LightYears,
    ) -> Self {
        Self {
            record,
            position,
            distance,
        }
    }

    /// The system as placed: its state at the epoch.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The system's ID.
    #[must_use]
    pub fn id(&self) -> SystemId {
        self.record.id()
    }

    /// Where the system was at the query's time, in the galactic frame.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// How far the system was from the query's centre at the query's time.
    #[must_use]
    pub const fn distance(&self) -> LightYears {
        self.distance
    }
}

/// What a range query found: the systems, what the result is complete for, and what it cost.
///
/// The systems are ordered by distance at the query's time, then by ID, so the order is the same
/// whatever the query walked first and whatever a cache held (plan 03, Design note 14). Their count
/// is not bounded by the query's limit: the limit bounds the expected count of a layer, and the
/// realised count fluctuates around it and is never truncated, because truncation would break the
/// census (Design note 9).
#[derive(Debug, Clone, PartialEq)]
pub struct RangeResult {
    systems: Vec<SystemHit>,
    census: Census,
    stats: QueryStats,
}

impl RangeResult {
    /// A result from its parts. Only the query assembly builds one.
    #[must_use]
    pub(super) const fn new(systems: Vec<SystemHit>, census: Census, stats: QueryStats) -> Self {
        Self {
            systems,
            census,
            stats,
        }
    }

    /// The systems found, nearest first at the query's time and by ID on a tie.
    #[must_use]
    pub fn systems(&self) -> &[SystemHit] {
        &self.systems
    }

    /// The systems found, taken out of the result.
    #[must_use]
    pub fn into_systems(self) -> Vec<SystemHit> {
        self.systems
    }

    /// What the result is complete for.
    #[must_use]
    pub const fn census(&self) -> &Census {
        &self.census
    }

    /// What the query cost.
    #[must_use]
    pub const fn stats(&self) -> &QueryStats {
        &self.stats
    }
}

/// How much work a range query did.
///
/// A bag of counters with no invariant between them, which the query assembly adds to in place.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct QueryStats {
    pub(super) cells_visited: u64,
    pub(super) systems_examined: u64,
    pub(super) padded_radius: LightYears,
}

impl QueryStats {
    /// Cells walked, over every admitted layer.
    #[must_use]
    pub const fn cells_visited(&self) -> u64 {
        self.cells_visited
    }

    /// Systems the query looked at: every record of every cell it walked, plus every hit a source
    /// returned, since a source tests its own members and reports only those that are in.
    #[must_use]
    pub const fn systems_examined(&self) -> u64 {
        self.systems_examined
    }

    /// The widest radius any layer's cells were chosen by: the query's radius plus the largest pad
    /// for motion over |t|.
    ///
    /// Each layer is walked with its own pad: layer E's is plan 08's unbound class's, 3,000 km/s,
    /// and every other layer's plan 03's 1,000 km/s, so this is layer E's whenever it is walked.
    #[must_use]
    pub const fn padded_radius(&self) -> LightYears {
        self.padded_radius
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn census(complete_down_to: Option<Layer>) -> Census {
        Census::new(complete_down_to, CensusStop::Limit, LayerCounts::ZERO)
    }

    #[test]
    fn complete_above_is_the_lower_edge_of_the_finest_complete_layer() {
        let above = |layer| census(Some(layer)).complete_above().unwrap().value();
        assert_same_bits(above(Layer::B), 0.5);
        assert_same_bits(above(Layer::A), 0.08);
        assert_same_bits(above(Layer::C), 0.75);
        assert_same_bits(above(Layer::D), 2.5);
        assert_same_bits(above(Layer::E), 8.0);
        assert_eq!(census(None).complete_above(), None);
    }

    #[test]
    fn a_census_walks_from_e_down_to_its_finest_complete_layer() {
        assert_eq!(census(None).layers(), LayerSet::EMPTY);
        assert_eq!(
            census(Some(Layer::E)).layers().iter().collect::<Vec<_>>(),
            [Layer::E]
        );
        assert_eq!(
            census(Some(Layer::C)).layers().iter().collect::<Vec<_>>(),
            [Layer::C, Layer::D, Layer::E]
        );
        assert_eq!(
            census(Some(Layer::A)).layers(),
            [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn layer_counts_are_indexed_by_layer_value_and_add_layer_by_layer() {
        let mut grid = LayerCounts::ZERO;
        grid.set(Layer::A, 1_000.0);
        grid.set(Layer::E, 2.5);
        assert_same_bits(grid.get(Layer::A), 1_000.0);
        assert_same_bits(grid.to_array()[4], 2.5);
        let sources = LayerCounts::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 0.0]);
        let sum = grid + sources;
        let expected = [1_001.0, 2.0, 3.0, 4.0, 7.5, 0.0, 0.0];
        for (layer, value) in Layer::ALL.into_iter().zip(expected) {
            assert_same_bits(sum.get(layer), value);
        }
        assert_eq!(LayerCounts::default(), LayerCounts::ZERO);
    }

    #[test]
    #[should_panic(expected = "finite and non-negative")]
    fn a_negative_count_is_refused() {
        let mut counts = LayerCounts::ZERO;
        counts.set(Layer::C, -1.0);
    }

    #[test]
    #[should_panic(expected = "finite and non-negative")]
    fn a_nan_count_is_refused() {
        let _ = LayerCounts::from_array([0.0, 0.0, f64::NAN, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn the_walk_order_is_the_stellar_layers_then_the_substellar_ones() {
        assert_eq!(
            WALK_ORDER,
            [
                Layer::E,
                Layer::D,
                Layer::C,
                Layer::B,
                Layer::A,
                Layer::BrownDwarf,
                Layer::RoguePlanet
            ]
        );
        // A census reaching the rogue planets, as plan 13's may, walks every layer.
        assert_eq!(census(Some(Layer::RoguePlanet)).layers().iter().count(), 7);
    }

    #[test]
    fn a_layer_set_holds_exactly_the_layers_put_in_it() {
        let set = LayerSet::EMPTY.with(Layer::D).with(Layer::BrownDwarf);
        for layer in Layer::ALL {
            assert_eq!(
                set.contains(layer),
                matches!(layer, Layer::D | Layer::BrownDwarf),
                "{layer:?}"
            );
        }
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            [Layer::D, Layer::BrownDwarf]
        );
        assert!(!set.is_empty());
        assert!(LayerSet::EMPTY.is_empty());
        assert_eq!(LayerSet::default(), LayerSet::EMPTY);
        let all: LayerSet = Layer::ALL.into_iter().collect();
        assert_eq!(all.iter().count(), 7);
    }

    #[test]
    fn query_stats_start_at_zero() {
        let stats = QueryStats::default();
        assert_eq!(stats.cells_visited(), 0);
        assert_eq!(stats.systems_examined(), 0);
        assert_same_bits(stats.padded_radius().value(), 0.0);
    }
}
