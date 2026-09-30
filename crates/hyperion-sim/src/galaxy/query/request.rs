//! A range query's request: where, how far, when, and the limits its census works under.

use std::num::NonZeroU32;

use super::BuildRangeQueryError;
use super::mode::QueryMode;
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

/// The finest layer a query may walk: a floor on primary initial mass, or on object mass below the
/// stars.
///
/// A long-range query needs a floor ("navigation beacons only", brainstorm, "The range query"),
/// since a 500 ly sphere meets a million layer-A cells. The steps run from the coarsest, so a finer
/// floor compares greater. The two steps below [`MassFloor::LayerA`] are the substellar layers'
/// (plan 13, Design note 9): the brainstorm's "the mass floor gains two steps". A query may take
/// one only with the matching [`SubstellarRequest`], which [`RangeQueryBuilder::build`] checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum MassFloor {
    /// Layer E only: primaries of 8 M☉ and up.
    LayerE,
    /// Down to layer D: 2.5 M☉ and up.
    LayerD,
    /// Down to layer C: 0.75 M☉ and up, every layer that can hold an evolved star.
    ///
    /// The floor reads the primary's initial mass, not the system's light. The rare bright
    /// exception, a layer-A or -B system whose binary has made it brighter than ten 0.75 M☉ stars
    /// of its age (a merger's blue straggler, an accretor), is held under 10⁻³ of those layers'
    /// systems near the solar circle by population (plan 11, P11.T11): the slow test
    /// `binary_system_bright_exception_and_contact_binaries` measures it over 9 × 10⁴ of them, and
    /// plan 11's Risks record the figure.
    LayerC,
    /// Down to layer B: 0.5 M☉ and up.
    LayerB,
    /// Every stellar layer, down to 0.08 M☉: the default.
    #[default]
    LayerA,
    /// The stars and the free-floating brown dwarfs, down to 13 Jupiter masses (0.0124 M☉).
    BrownDwarfs,
    /// The stars, the brown dwarfs and the rogue planets, down to a third of an Earth mass
    /// (1.0 × 10⁻⁶ M☉).
    RoguePlanets,
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
            Self::BrownDwarfs => Layer::BrownDwarf,
            Self::RoguePlanets => Layer::RoguePlanet,
        }
    }
}

/// Which substellar layers a query asks for, after layer A (brainstorm, "The range query": they
/// come after layer A in the walk and only when the caller asks for them).
///
/// The request and the [`MassFloor`] are tied so that they cannot disagree (plan 13, Design note
/// 9): a request lowers a floor of [`MassFloor::LayerA`] to its own step
/// ([`floor`](Self::floor)), a floor below layer A needs the request that asks for its layer, and
/// a request with a floor that would cut its layers off is refused. [`RangeQueryBuilder::build`]
/// applies the three rules.
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

impl SubstellarRequest {
    /// The floor step that walks exactly the layers asked for: [`MassFloor::LayerA`] for none,
    /// then one step per substellar layer.
    #[must_use]
    pub const fn floor(self) -> MassFloor {
        match self {
            Self::None => MassFloor::LayerA,
            Self::BrownDwarfs => MassFloor::BrownDwarfs,
            Self::BrownDwarfsAndRoguePlanets => MassFloor::RoguePlanets,
        }
    }

    /// Whether the request asks for `layer`'s objects: every stellar layer always, and a
    /// substellar layer when the request names it.
    #[must_use]
    pub const fn admits(self, layer: Layer) -> bool {
        match layer {
            Layer::A | Layer::B | Layer::C | Layer::D | Layer::E => true,
            Layer::BrownDwarf => !matches!(self, Self::None),
            Layer::RoguePlanet => matches!(self, Self::BrownDwarfsAndRoguePlanets),
        }
    }
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
    mode: QueryMode,
}

impl RangeQuery {
    /// A builder for a query of the sphere of `radius` about `centre`, with every other setting
    /// at its default: at the epoch, [`DEFAULT_CENSUS_LIMIT`], [`MassFloor::LayerA`], no
    /// substellar layers, [`DEFAULT_CELL_BUDGET`] and [`QueryMode::Now`].
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
                mode: QueryMode::Now,
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

    /// The finest layer the census may admit: the builder's floor, or the request's own step when
    /// the builder's was [`MassFloor::LayerA`] and substellar layers were asked for.
    #[must_use]
    pub const fn mass_floor(&self) -> MassFloor {
        self.mass_floor
    }

    /// The substellar layers asked for.
    #[must_use]
    pub const fn substellar(&self) -> SubstellarRequest {
        self.substellar
    }

    /// The cell budget over the admitted layers.
    #[must_use]
    pub const fn cell_budget(&self) -> NonZeroU32 {
        self.cell_budget
    }

    /// What the answer reports: the present alone, or also what an observer's sensors receive
    /// (plan 12, P12.T3). The systems found and the census do not depend on it (Design note 4).
    #[must_use]
    pub const fn mode(&self) -> QueryMode {
        self.mode
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

    /// The finest layer the census may admit. A step below [`MassFloor::LayerA`] needs the
    /// matching [`substellar`](Self::substellar) request.
    ///
    /// Default: [`MassFloor::LayerA`].
    pub const fn mass_floor(mut self, floor: MassFloor) -> Self {
        self.query.mass_floor = floor;
        self
    }

    /// The substellar layers to add after layer A. With the default floor, the request lowers it
    /// to its own step ([`SubstellarRequest::floor`]).
    ///
    /// Default: [`SubstellarRequest::None`].
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

    /// What the answer reports ([`QueryMode`]). Only
    /// [`range_query_observed`](super::range_query_observed) reads it;
    /// [`range_query`](super::range_query) finds the same systems in either mode and reports the
    /// present.
    ///
    /// Default: [`QueryMode::Now`].
    pub const fn mode(mut self, mode: QueryMode) -> Self {
        self.query.mode = mode;
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
    /// - [`BuildRangeQueryError::SubstellarNotRequested`] for a floor below
    ///   [`MassFloor::LayerA`] whose layers the substellar request does not ask for (plan 13,
    ///   Design note 9).
    /// - [`BuildRangeQueryError::SubstellarBelowFloor`] for a substellar request with a floor that
    ///   would cut its layers off: above layer A, or the brown dwarfs' step with the rogue planets
    ///   asked for.
    /// - [`BuildRangeQueryError::ObserverOutsideRootCube`] for an observed mode whose observer lies
    ///   outside the root cube.
    ///
    /// With a floor of [`MassFloor::LayerA`] and substellar layers asked for, the built query's
    /// floor is the request's step.
    pub fn build(self) -> Result<RangeQuery, BuildRangeQueryError> {
        let mut query = self.query;
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
        query.mass_floor = tie_floor(query.mass_floor, query.substellar)?;
        if let QueryMode::ObservedFrom(observer) = query.mode
            && !observer.in_root_cube()
        {
            return Err(BuildRangeQueryError::ObserverOutsideRootCube);
        }
        Ok(query)
    }
}

/// The floor a query walks to, from the builder's floor and substellar request (plan 13, Design
/// note 9): the request's own step in place of [`MassFloor::LayerA`], the floor itself when it is
/// that step or a stellar floor with nothing asked for, and an error when the two disagree.
fn tie_floor(
    floor: MassFloor,
    request: SubstellarRequest,
) -> Result<MassFloor, BuildRangeQueryError> {
    let asked = request.floor();
    match floor {
        MassFloor::LayerE | MassFloor::LayerD | MassFloor::LayerC | MassFloor::LayerB => {
            if request == SubstellarRequest::None {
                Ok(floor)
            } else {
                Err(BuildRangeQueryError::SubstellarBelowFloor)
            }
        }
        MassFloor::LayerA => Ok(asked),
        MassFloor::BrownDwarfs | MassFloor::RoguePlanets => match floor.cmp(&asked) {
            std::cmp::Ordering::Equal => Ok(floor),
            // The floor reaches a layer nobody asked for.
            std::cmp::Ordering::Greater => Err(BuildRangeQueryError::SubstellarNotRequested),
            // The request reaches below the floor.
            std::cmp::Ordering::Less => Err(BuildRangeQueryError::SubstellarBelowFloor),
        },
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

    /// Every floor against every request (plan 13, Design note 9): the request lowers layer A's
    /// floor to its step, the matching step stands, and every other pairing is refused with the
    /// variant that names the disagreement.
    #[test]
    fn the_floor_and_the_substellar_request_are_tied() {
        use BuildRangeQueryError::{SubstellarBelowFloor, SubstellarNotRequested};
        use MassFloor::{BrownDwarfs, LayerA, LayerB, LayerC, LayerD, LayerE, RoguePlanets};
        use SubstellarRequest::{BrownDwarfsAndRoguePlanets as Both, None as Nothing};
        let bd = SubstellarRequest::BrownDwarfs;
        let cases = [
            (LayerE, Nothing, Ok(LayerE)),
            (LayerB, Nothing, Ok(LayerB)),
            (LayerA, Nothing, Ok(LayerA)),
            (BrownDwarfs, Nothing, Err(SubstellarNotRequested)),
            (RoguePlanets, Nothing, Err(SubstellarNotRequested)),
            (LayerA, bd, Ok(BrownDwarfs)),
            (BrownDwarfs, bd, Ok(BrownDwarfs)),
            (RoguePlanets, bd, Err(SubstellarNotRequested)),
            (LayerA, Both, Ok(RoguePlanets)),
            (RoguePlanets, Both, Ok(RoguePlanets)),
            (BrownDwarfs, Both, Err(SubstellarBelowFloor)),
            (LayerE, bd, Err(SubstellarBelowFloor)),
            (LayerD, Both, Err(SubstellarBelowFloor)),
            (LayerC, bd, Err(SubstellarBelowFloor)),
            (LayerB, Both, Err(SubstellarBelowFloor)),
        ];
        for (floor, request, expected) in cases {
            let built = RangeQuery::builder(sunlike(), LightYears::new(50.0))
                .mass_floor(floor)
                .substellar(request)
                .build();
            assert_eq!(
                built.as_ref().map(RangeQuery::mass_floor).map_err(|e| *e),
                expected,
                "{floor:?} with {request:?}"
            );
            if let Ok(query) = built {
                assert_eq!(query.substellar(), request);
            }
        }
    }

    #[test]
    fn a_request_admits_the_stars_and_the_layers_it_names() {
        for request in [
            SubstellarRequest::None,
            SubstellarRequest::BrownDwarfs,
            SubstellarRequest::BrownDwarfsAndRoguePlanets,
        ] {
            for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
                assert!(request.admits(layer), "{request:?} {layer:?}");
            }
            assert_eq!(
                request.admits(Layer::BrownDwarf),
                request != SubstellarRequest::None
            );
            assert_eq!(
                request.admits(Layer::RoguePlanet),
                request == SubstellarRequest::BrownDwarfsAndRoguePlanets
            );
            // The request's step walks exactly the layers it admits.
            assert!(request.admits(request.floor().layer()));
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
                MassFloor::LayerA,
                MassFloor::BrownDwarfs,
                MassFloor::RoguePlanets
            ]
            .map(MassFloor::layer),
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
        assert!(MassFloor::LayerE < MassFloor::LayerA);
        assert!(MassFloor::LayerA < MassFloor::BrownDwarfs);
        assert!(MassFloor::BrownDwarfs < MassFloor::RoguePlanets);
    }
}
