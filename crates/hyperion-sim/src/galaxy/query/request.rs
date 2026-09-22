//! A range query's request: where, how far, when, and the limits its census works under.

use std::num::NonZeroU32;

use super::BuildRangeQueryError;
use crate::coords::{GalacticPosition, ROOT_HALF_WIDTH_LY};
use crate::id::Layer;
use crate::time::{ClockWindow, UniverseTime};
use crate::units::LightYears;

/// The default census limit: 4,096 expected systems (plan 03, Design note 9).
///
/// The limit bounds the _expected_ count of the layers a query admits, never the realised count,
/// which fluctuates about it and is never truncated. Callers size buffers with headroom.
pub const DEFAULT_CENSUS_LIMIT: NonZeroU32 = NonZeroU32::new(4_096).expect("4,096 is not zero");

/// The default cell budget: 2²⁰ = 1,048,576 cells over the admitted layers (plan 03, Design note
/// 10), about what a 500 ly sphere meets in layer A alone.
pub const DEFAULT_CELL_BUDGET: NonZeroU32 = NonZeroU32::new(1 << 20).expect("2²⁰ is not zero");

/// The square of the root cube's diagonal, ly²: 3 × 131,072², exact in `f64`.
///
/// The largest radius a query may have is the diagonal, 227,023 ly, compared in squares so that no
/// square root is needed.
#[must_use]
fn root_diagonal_squared_ly2() -> f64 {
    let edge = 2.0 * f64::from(ROOT_HALF_WIDTH_LY);
    3.0 * edge * edge
}

/// The finest stellar layer a query may walk: a floor on primary initial mass.
///
/// A long-range query needs a floor ("navigation beacons only", brainstorm, "The range query"),
/// since a 500 ly sphere meets a million layer-A cells. The steps run from the coarsest; plan 13
/// adds two below [`MassFloor::LayerA`] for the substellar layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum MassFloor {
    /// Layer E only: primaries of 8 M☉ and up.
    LayerE,
    /// Down to layer D: 2.5 M☉ and up.
    LayerD,
    /// Down to layer C: 0.75 M☉ and up, every layer that can hold an evolved star.
    LayerC,
    /// Down to layer B: 0.5 M☉ and up.
    LayerB,
    /// Every stellar layer, down to 0.08 M☉: the default.
    #[default]
    LayerA,
}

impl MassFloor {
    /// The finest layer the floor admits.
    #[must_use]
    pub const fn layer(self) -> Layer {
        match self {
            Self::LayerE => Layer::E,
            Self::LayerD => Layer::D,
            Self::LayerC => Layer::C,
            Self::LayerB => Layer::B,
            Self::LayerA => Layer::A,
        }
    }
}

/// Which substellar layers a query asks for, after layer A (brainstorm, "The range query": they
/// come after layer A in the walk and only when the caller asks for them).
///
/// The enum exists so that plan 13 changes no signature; until then
/// [`RangeQueryBuilder::build`] rejects anything but [`SubstellarRequest::None`] (plan 03,
/// Design note 17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum SubstellarRequest {
    /// Stellar layers only: the default.
    #[default]
    None,
    /// The free-floating brown dwarfs too.
    BrownDwarfs,
    /// The free-floating brown dwarfs and the rogue planets too.
    BrownDwarfsAndRoguePlanets,
}

/// "Which systems lie within R light-years of this point at time t?", with the limits its census
/// works under.
///
/// Built and validated by [`RangeQuery::builder`].
///
/// # Examples
///
/// ```
/// use std::num::NonZeroU32;
///
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::query::{BuildRangeQueryError, MassFloor, RangeQuery};
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::LightYears;
///
/// // Navigation beacons within 500 ly of a point in the disc, a century from now.
/// let centre = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).expect("in range");
/// let century = UniverseTime::from_julian_years(100).expect("in range");
/// let query = RangeQuery::builder(centre, LightYears::new(500.0))
///     .time(century)
///     .mass_floor(MassFloor::LayerD)
///     .limit(NonZeroU32::new(2_000).expect("not zero"))
///     .build()?;
/// assert_eq!(query.mass_floor(), MassFloor::LayerD);
///
/// // A time outside the clock window is refused: positions there are not guaranteed.
/// let too_late = UniverseTime::from_julian_years(1_001).expect("in range");
/// assert_eq!(
///     RangeQuery::builder(centre, LightYears::new(50.0)).time(too_late).build(),
///     Err(BuildRangeQueryError::TimeOutsideClockWindow(too_late))
/// );
/// # Ok::<(), BuildRangeQueryError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RangeQuery {
    centre: GalacticPosition,
    radius: LightYears,
    time: UniverseTime,
    limit: NonZeroU32,
    mass_floor: MassFloor,
    substellar: SubstellarRequest,
    cell_budget: NonZeroU32,
}

impl RangeQuery {
    /// A builder for a query of the sphere of `radius` about `centre`, with every other setting
    /// at its default: at the epoch, [`DEFAULT_CENSUS_LIMIT`], [`MassFloor::LayerA`], no
    /// substellar layers and [`DEFAULT_CELL_BUDGET`].
    pub fn builder(centre: GalacticPosition, radius: LightYears) -> RangeQueryBuilder {
        RangeQueryBuilder {
            query: Self {
                centre,
                radius,
                time: UniverseTime::EPOCH,
                limit: DEFAULT_CENSUS_LIMIT,
                mass_floor: MassFloor::default(),
                substellar: SubstellarRequest::default(),
                cell_budget: DEFAULT_CELL_BUDGET,
            },
        }
    }

    /// The sphere's centre, in the galactic frame; inside the root cube.
    #[must_use]
    pub const fn centre(&self) -> &GalacticPosition {
        &self.centre
    }

    /// The sphere's radius: finite, positive and at most the root cube's diagonal.
    #[must_use]
    pub const fn radius(&self) -> LightYears {
        self.radius
    }

    /// The time at which distances are tested, inside the clock window.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// The census limit on the admitted layers' expected count.
    #[must_use]
    pub const fn limit(&self) -> NonZeroU32 {
        self.limit
    }

    /// The finest layer the census may admit.
    #[must_use]
    pub const fn mass_floor(&self) -> MassFloor {
        self.mass_floor
    }

    /// The substellar layers asked for; [`SubstellarRequest::None`] until plan 13.
    #[must_use]
    pub const fn substellar(&self) -> SubstellarRequest {
        self.substellar
    }

    /// The cell budget over the admitted layers.
    #[must_use]
    pub const fn cell_budget(&self) -> NonZeroU32 {
        self.cell_budget
    }
}

/// Builds a [`RangeQuery`]: start from [`RangeQuery::builder`], change any setting, then
/// [`build`](Self::build).
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct RangeQueryBuilder {
    query: RangeQuery,
}

impl RangeQueryBuilder {
    /// The time at which distances are tested.
    ///
    /// Default: the epoch.
    pub const fn time(mut self, t: UniverseTime) -> Self {
        self.query.time = t;
        self
    }

    /// The census limit on expected systems.
    ///
    /// Default: [`DEFAULT_CENSUS_LIMIT`].
    pub const fn limit(mut self, limit: NonZeroU32) -> Self {
        self.query.limit = limit;
        self
    }

    /// The finest layer the census may admit.
    ///
    /// Default: [`MassFloor::LayerA`].
    pub const fn mass_floor(mut self, floor: MassFloor) -> Self {
        self.query.mass_floor = floor;
        self
    }

    /// The substellar layers to add after layer A.
    ///
    /// Default: [`SubstellarRequest::None`], the only value [`build`](Self::build) accepts until
    /// plan 13.
    pub const fn substellar(mut self, request: SubstellarRequest) -> Self {
        self.query.substellar = request;
        self
    }

    /// The cell budget over the admitted layers.
    ///
    /// Default: [`DEFAULT_CELL_BUDGET`].
    pub const fn cell_budget(mut self, cells: NonZeroU32) -> Self {
        self.query.cell_budget = cells;
        self
    }

    /// The validated query.
    ///
    /// # Errors
    ///
    /// The first rule broken, in this order:
    ///
    /// - [`BuildRangeQueryError::RadiusNotFinite`] for a NaN or infinite radius.
    /// - [`BuildRangeQueryError::RadiusNotPositive`] for a radius of zero or less.
    /// - [`BuildRangeQueryError::RadiusBeyondRootCube`] for a radius above the root cube's
    ///   diagonal, 227,023 ly.
    /// - [`BuildRangeQueryError::CentreOutsideRootCube`] unless the centre lies in the root cube.
    /// - [`BuildRangeQueryError::TimeOutsideClockWindow`] unless `|t| ≤ H` (plan 03, Design
    ///   note 12).
    /// - [`BuildRangeQueryError::SubstellarLayersUnavailable`] for any substellar request but
    ///   [`SubstellarRequest::None`] (Design note 17).
    pub fn build(self) -> Result<RangeQuery, BuildRangeQueryError> {
        let query = self.query;
        let radius = query.radius.value();
        if !radius.is_finite() {
            return Err(BuildRangeQueryError::RadiusNotFinite);
        }
        if radius <= 0.0 {
            return Err(BuildRangeQueryError::RadiusNotPositive);
        }
        if radius * radius > root_diagonal_squared_ly2() {
            return Err(BuildRangeQueryError::RadiusBeyondRootCube);
        }
        if !query.centre.in_root_cube() {
            return Err(BuildRangeQueryError::CentreOutsideRootCube);
        }
        if !ClockWindow::contains(query.time) {
            return Err(BuildRangeQueryError::TimeOutsideClockWindow(query.time));
        }
        match query.substellar {
            SubstellarRequest::None => Ok(query),
            SubstellarRequest::BrownDwarfs | SubstellarRequest::BrownDwarfsAndRoguePlanets => {
                Err(BuildRangeQueryError::SubstellarLayersUnavailable)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::LyCell;
    use crate::time::CLOCK_WINDOW_H;

    fn sunlike() -> GalacticPosition {
        GalacticPosition::new(LyCell::new([0, 26_000, 0]), [0.0; 3]).unwrap()
    }

    #[test]
    fn defaults_are_as_documented() {
        let query = RangeQuery::builder(sunlike(), LightYears::new(50.0))
            .build()
            .unwrap();
        assert_eq!(query.centre().cell(), sunlike().cell());
        for (stored, given) in query.centre().offset_metres().into_iter().zip([0.0; 3]) {
            assert_same_bits(stored, given);
        }
        assert_same_bits(query.radius().value(), 50.0);
        assert_eq!(query.time(), UniverseTime::EPOCH);
        assert_eq!(query.limit().get(), 4_096);
        assert_eq!(query.limit(), DEFAULT_CENSUS_LIMIT);
        assert_eq!(query.mass_floor(), MassFloor::LayerA);
        assert_eq!(query.substellar(), SubstellarRequest::None);
        assert_eq!(query.cell_budget().get(), 1_048_576);
        assert_eq!(query.cell_budget(), DEFAULT_CELL_BUDGET);
    }

    #[test]
    fn every_setting_reaches_the_query() {
        let t = UniverseTime::from_julian_years(-250).unwrap();
        let limit = NonZeroU32::new(100).unwrap();
        let budget = NonZeroU32::new(5_000).unwrap();
        let query = RangeQuery::builder(sunlike(), LightYears::new(12.5))
            .time(t)
            .limit(limit)
            .mass_floor(MassFloor::LayerC)
            .cell_budget(budget)
            .substellar(SubstellarRequest::None)
            .build()
            .unwrap();
        assert_eq!(
            (
                query.time(),
                query.limit(),
                query.mass_floor(),
                query.cell_budget()
            ),
            (t, limit, MassFloor::LayerC, budget)
        );
    }

    #[test]
    fn each_bad_radius_is_rejected_with_its_variant() {
        let build = |radius: f64| RangeQuery::builder(sunlike(), LightYears::new(radius)).build();
        assert_eq!(build(f64::NAN), Err(BuildRangeQueryError::RadiusNotFinite));
        assert_eq!(
            build(f64::INFINITY),
            Err(BuildRangeQueryError::RadiusNotFinite)
        );
        assert_eq!(
            build(f64::NEG_INFINITY),
            Err(BuildRangeQueryError::RadiusNotFinite)
        );
        assert_eq!(build(0.0), Err(BuildRangeQueryError::RadiusNotPositive));
        assert_eq!(build(-0.0), Err(BuildRangeQueryError::RadiusNotPositive));
        assert_eq!(build(-5.0), Err(BuildRangeQueryError::RadiusNotPositive));
        // The diagonal is 131,072 × √3 = 227,023.35 ly.
        build(227_023.0).unwrap();
        assert_eq!(
            build(227_024.0),
            Err(BuildRangeQueryError::RadiusBeyondRootCube)
        );
        build(f64::MIN_POSITIVE).unwrap();
    }

    #[test]
    fn a_centre_outside_the_root_cube_is_rejected() {
        let inside = GalacticPosition::new(LyCell::new([65_535, -65_536, 0]), [0.0; 3]).unwrap();
        RangeQuery::builder(inside, LightYears::new(1.0))
            .build()
            .unwrap();
        for cell in [[65_536, 0, 0], [0, -65_537, 0], [0, 0, 70_000]] {
            let outside = GalacticPosition::new(LyCell::new(cell), [0.0; 3]).unwrap();
            assert_eq!(
                RangeQuery::builder(outside, LightYears::new(1.0)).build(),
                Err(BuildRangeQueryError::CentreOutsideRootCube),
                "{cell:?}"
            );
        }
    }

    #[test]
    fn a_time_outside_the_clock_window_is_rejected() {
        let h = UniverseTime::EPOCH.checked_add(CLOCK_WINDOW_H).unwrap();
        let minus_h = UniverseTime::EPOCH.checked_sub(CLOCK_WINDOW_H).unwrap();
        let build = |t| {
            RangeQuery::builder(sunlike(), LightYears::new(50.0))
                .time(t)
                .build()
        };
        assert_eq!(build(h).map(|query| query.time()), Ok(h));
        assert_eq!(build(minus_h).map(|query| query.time()), Ok(minus_h));
        for t in [
            UniverseTime::new(h.seconds(), 1).unwrap(),
            UniverseTime::new(minus_h.seconds() - 1, 999_999_999).unwrap(),
            UniverseTime::from_julian_years(1_000_000).unwrap(),
        ] {
            assert_eq!(
                build(t),
                Err(BuildRangeQueryError::TimeOutsideClockWindow(t)),
                "{t}"
            );
        }
    }

    #[test]
    fn the_first_rule_broken_names_the_error() {
        let outside = GalacticPosition::new(LyCell::new([70_000, 0, 0]), [0.0; 3]).unwrap();
        let too_late = UniverseTime::from_julian_years(5_000).unwrap();
        let everything_wrong = |radius: f64| {
            RangeQuery::builder(outside, LightYears::new(radius))
                .time(too_late)
                .substellar(SubstellarRequest::BrownDwarfs)
                .build()
        };
        assert_eq!(
            everything_wrong(f64::NAN),
            Err(BuildRangeQueryError::RadiusNotFinite)
        );
        assert_eq!(
            everything_wrong(-1.0),
            Err(BuildRangeQueryError::RadiusNotPositive)
        );
        assert_eq!(
            everything_wrong(300_000.0),
            Err(BuildRangeQueryError::RadiusBeyondRootCube)
        );
        assert_eq!(
            everything_wrong(50.0),
            Err(BuildRangeQueryError::CentreOutsideRootCube)
        );
        assert_eq!(
            RangeQuery::builder(sunlike(), LightYears::new(50.0))
                .time(too_late)
                .substellar(SubstellarRequest::BrownDwarfs)
                .build(),
            Err(BuildRangeQueryError::TimeOutsideClockWindow(too_late))
        );
    }

    #[test]
    fn substellar_requests_are_refused_until_plan_13() {
        for request in [
            SubstellarRequest::BrownDwarfs,
            SubstellarRequest::BrownDwarfsAndRoguePlanets,
        ] {
            assert_eq!(
                RangeQuery::builder(sunlike(), LightYears::new(50.0))
                    .substellar(request)
                    .build(),
                Err(BuildRangeQueryError::SubstellarLayersUnavailable)
            );
        }
    }

    #[test]
    fn mass_floors_name_their_finest_layer_from_the_coarsest() {
        assert_eq!(
            [
                MassFloor::LayerE,
                MassFloor::LayerD,
                MassFloor::LayerC,
                MassFloor::LayerB,
                MassFloor::LayerA
            ]
            .map(MassFloor::layer),
            [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A]
        );
        assert!(MassFloor::LayerE < MassFloor::LayerA);
    }
}
