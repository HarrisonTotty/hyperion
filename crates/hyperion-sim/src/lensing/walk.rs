//! The lens walk: every system that passes close to one line of sight during a window (plan 12,
//! P12.T4.b; Design note 13).

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;

use super::point_lens::{einstein_angle, point_lens_magnification};
use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::events::TimeWindow;
use crate::galaxy::Galaxy;
use crate::galaxy::features::members::{NoInteriorCache, resolve_member};
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{
    CellCache, CellKey, Existence, ResolveSystemError, SystemKind, SystemOrigin, SystemRecord,
    resolve,
};
use crate::galaxy::query::{
    LayerSet, MassFloor, PAD_SPEED, QuerySphere, SubstellarRequest, SystemHit, SystemSource,
    UNBOUND_PAD_SPEED, cells_along_segment, pad_speed,
};
use crate::id::{Layer, SystemId, SystemIdKind};
use crate::math;
use crate::observe::{Drift, Observer, TraceMotionError, retarded};
use crate::stellar::multiplicity::MAX_COMPANIONS;
use crate::stellar::system::SystemStars;
use crate::time::{ClockWindow, Span, UniverseTime};
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR, SPEED_OF_LIGHT};
use crate::units::{
    KilometresPerSecond, LightYears, Metres, MetresPerSecond, Seconds, SolarMasses,
};

/// The default reach of a lens query, in Einstein radii: 3, where a point lens magnifies by
/// 1.017, a generous reach beyond the u₀ ≤ 1 (A ≥ 1.34) that survey samples are defined by.
pub const DEFAULT_MAX_IMPACT: f64 = 3.0;

/// The default budget of cells a lens query may walk: 2²⁰, as the range query's.
pub const DEFAULT_LENS_CELL_BUDGET: NonZeroU32 =
    NonZeroU32::new(1 << 20).expect("2^20 is not zero");

/// The number of pieces a sightline's tube is cut into, each with the radius its far end needs.
const PIECES: u32 = 64;

/// The span over which a source's hits are differenced to read its velocity: one Julian year.
const DIFFERENCE_YEARS: i64 = 1;

/// One monitored line of sight: an observer and a background system (plan 12's Provides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensSightline {
    observer: GalacticPosition,
    source: SystemId,
}

impl LensSightline {
    /// The line of sight from `observer` to the system `source`.
    #[must_use]
    pub const fn new(observer: GalacticPosition, source: SystemId) -> Self {
        Self { observer, source }
    }

    /// Where the observer is.
    #[must_use]
    pub const fn observer(&self) -> &GalacticPosition {
        &self.observer
    }

    /// The background system whose light is watched.
    #[must_use]
    pub const fn source(&self) -> SystemId {
        self.source
    }
}

/// A [`LensQuery`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildLensQueryError {
    /// The observer lies outside the root cube.
    ObserverOutsideRootCube,
    /// The window reaches outside the clock window: observer times are in play, ±H.
    WindowOutsideClockWindow(TimeWindow),
    /// The reach in Einstein radii is not finite and positive.
    MaxImpactNotPositive(f64),
}

impl fmt::Display for BuildLensQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObserverOutsideRootCube => f.write_str("the observer lies outside the root cube"),
            Self::WindowOutsideClockWindow(w) => write!(
                f,
                "the window from {} to {} reaches outside the clock window",
                w.start(),
                w.end()
            ),
            Self::MaxImpactNotPositive(u) => {
                write!(
                    f,
                    "the reach of {u} einstein radii is not finite and positive"
                )
            }
        }
    }
}

impl Error for BuildLensQueryError {}

/// A lens query: which systems lens one source, as one observer sees it, during a window (plan
/// 12's Provides).
///
/// Built by [`LensQuery::builder`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensQuery {
    sightline: LensSightline,
    window: TimeWindow,
    max_impact: f64,
    mass_floor: MassFloor,
    substellar: SubstellarRequest,
    cell_budget: NonZeroU32,
}

/// Builds a [`LensQuery`]: the reach defaults to [`DEFAULT_MAX_IMPACT`], the grid layers to every
/// stellar layer, the substellar layers to none and the budget to [`DEFAULT_LENS_CELL_BUDGET`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensQueryBuilder {
    query: LensQuery,
}

impl LensQuery {
    /// A builder for the query along `sightline` over the observer times `window`.
    #[must_use]
    pub fn builder(sightline: LensSightline, window: TimeWindow) -> LensQueryBuilder {
        LensQueryBuilder {
            query: Self {
                sightline,
                window,
                max_impact: DEFAULT_MAX_IMPACT,
                mass_floor: MassFloor::default(),
                substellar: SubstellarRequest::default(),
                cell_budget: DEFAULT_LENS_CELL_BUDGET,
            },
        }
    }

    /// The line of sight.
    #[must_use]
    pub const fn sightline(&self) -> &LensSightline {
        &self.sightline
    }

    /// The observer times watched.
    #[must_use]
    pub const fn window(&self) -> TimeWindow {
        self.window
    }

    /// How close, in Einstein radii, a lens must come to count.
    #[must_use]
    pub const fn max_impact(&self) -> f64 {
        self.max_impact
    }

    /// The finest stellar layer walked.
    #[must_use]
    pub const fn mass_floor(&self) -> MassFloor {
        self.mass_floor
    }

    /// Which substellar layers are walked.
    #[must_use]
    pub const fn substellar(&self) -> SubstellarRequest {
        self.substellar
    }

    /// The most cells the walk may take.
    #[must_use]
    pub const fn cell_budget(&self) -> NonZeroU32 {
        self.cell_budget
    }

    /// The layers walked, stellar down to the floor and the substellar ones asked for.
    #[must_use]
    pub fn layers(&self) -> LayerSet {
        let floor = self.mass_floor.layer().value();
        let substellar: &[Layer] = match self.substellar {
            SubstellarRequest::None => &[],
            SubstellarRequest::BrownDwarfs => &[Layer::BrownDwarf],
            SubstellarRequest::BrownDwarfsAndRoguePlanets => {
                &[Layer::BrownDwarf, Layer::RoguePlanet]
            }
        };
        [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
            .into_iter()
            .filter(|layer| layer.value() >= floor)
            .chain(substellar.iter().copied())
            .collect()
    }
}

impl LensQueryBuilder {
    /// How close, in Einstein radii, a lens must come to count.
    #[must_use]
    pub const fn max_impact(mut self, max_impact: f64) -> Self {
        self.query.max_impact = max_impact;
        self
    }

    /// The finest stellar layer walked.
    #[must_use]
    pub const fn mass_floor(mut self, floor: MassFloor) -> Self {
        self.query.mass_floor = floor;
        self
    }

    /// Which substellar layers are walked.
    #[must_use]
    pub const fn substellar(mut self, substellar: SubstellarRequest) -> Self {
        self.query.substellar = substellar;
        self
    }

    /// The most cells the walk may take.
    #[must_use]
    pub const fn cell_budget(mut self, budget: NonZeroU32) -> Self {
        self.query.cell_budget = budget;
        self
    }

    /// The query.
    ///
    /// # Errors
    ///
    /// [`BuildLensQueryError::ObserverOutsideRootCube`],
    /// [`BuildLensQueryError::WindowOutsideClockWindow`] for a window that starts before −H or ends
    /// after +H, and [`BuildLensQueryError::MaxImpactNotPositive`].
    pub fn build(self) -> Result<LensQuery, BuildLensQueryError> {
        let q = self.query;
        if !q.sightline.observer.in_root_cube() {
            return Err(BuildLensQueryError::ObserverOutsideRootCube);
        }
        if q.window.start() < ClockWindow::START || q.window.end() > ClockWindow::END {
            return Err(BuildLensQueryError::WindowOutsideClockWindow(q.window));
        }
        if !(q.max_impact.is_finite() && q.max_impact > 0.0) {
            return Err(BuildLensQueryError::MaxImpactNotPositive(q.max_impact));
        }
        Ok(q)
    }
}

/// One lens: a system that passes within the query's reach of the source's direction during the
/// window, and the light curve it causes (plan 12's Provides).
///
/// The lens and the source move in straight lines over the window, so the curve is the point lens's
/// of u(t) = √(u₀² + ((t − t₀) ÷ `t_E`)²), with u₀ and t₀ the closest approach, which may lie
/// outside the window; [`peak`](Self::peak) and [`impact`](Self::impact) are the closest within it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensEvent {
    lens: SystemId,
    peak: UniverseTime,
    impact: f64,
    einstein_radius: Metres,
    einstein_time: Seconds,
    peak_magnification: f64,
    /// The closest approach's time, seconds since the epoch, unclamped.
    closest_s: f64,
    /// The closest approach's impact, Einstein radii, unclamped.
    closest_impact: f64,
}

impl LensEvent {
    /// The lensing system.
    #[must_use]
    pub const fn lens(&self) -> SystemId {
        self.lens
    }

    /// The observer time of the brightest point within the window: the closest approach, or the
    /// window's end nearer it.
    #[must_use]
    pub const fn peak(&self) -> UniverseTime {
        self.peak
    }

    /// The least impact within the window, in Einstein radii.
    #[must_use]
    pub const fn impact(&self) -> f64 {
        self.impact
    }

    /// The Einstein radius in the lens plane.
    #[must_use]
    pub const fn einstein_radius(&self) -> Metres {
        self.einstein_radius
    }

    /// The Einstein time: the Einstein angle over the relative angular speed; infinite for a lens
    /// and source that do not move across each other.
    #[must_use]
    pub const fn einstein_time(&self) -> Seconds {
        self.einstein_time
    }

    /// The magnification at the peak.
    #[must_use]
    pub const fn peak_magnification(&self) -> f64 {
        self.peak_magnification
    }

    /// The magnification of the source at observer time `t`.
    #[must_use]
    pub fn magnification_at(&self, t: UniverseTime) -> f64 {
        let tau = t.since_epoch().as_seconds_f64() - self.closest_s;
        let along = if self.einstein_time.value().is_finite() {
            tau / self.einstein_time.value()
        } else {
            0.0
        };
        point_lens_magnification((self.closest_impact * self.closest_impact + along * along).sqrt())
    }
}

/// The magnification `event` gives its source at observer time `t` (plan 12's Provides):
/// [`LensEvent::magnification_at`].
///
/// The plan's sketch takes the galaxy and the sightline too; an event carries its own straight
/// lines, so it needs neither (P12.T4.b as built).
#[must_use]
pub fn magnification_at(event: &LensEvent, t: UniverseTime) -> f64 {
    event.magnification_at(t)
}

/// What a lens walk walked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LensCensus {
    layers: LayerSet,
    cells: u64,
    systems_examined: u64,
    spheres: u32,
}

impl LensCensus {
    /// The grid layers walked.
    #[must_use]
    pub const fn layers(&self) -> LayerSet {
        self.layers
    }

    /// The grid cells walked.
    #[must_use]
    pub const fn cells(&self) -> u64 {
        self.cells
    }

    /// The systems looked at, from the grid and the sources.
    #[must_use]
    pub const fn systems_examined(&self) -> u64 {
        self.systems_examined
    }

    /// The spheres each source was asked about, twice each.
    #[must_use]
    pub const fn spheres(&self) -> u32 {
        self.spheres
    }
}

/// The lenses of one query, by peak and then by lens, and what was walked.
#[derive(Debug, Clone, PartialEq)]
pub struct LensResult {
    events: Vec<LensEvent>,
    walked: LensCensus,
}

impl LensResult {
    /// The lenses, by peak and then by ID.
    #[must_use]
    pub fn events(&self) -> &[LensEvent] {
        &self.events
    }

    /// What the walk walked.
    #[must_use]
    pub const fn walked(&self) -> &LensCensus {
        &self.walked
    }
}

/// A lens query could not be answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindLensesError {
    /// The source does not resolve.
    SourceNotFound(ResolveSystemError),
    /// The source's motion cannot be traced yet: a member of the galactic centre, whose orbit
    /// waits for plan 09's P09.T28.
    SourceMotionNotTraced(TraceMotionError),
    /// The walk would take more cells than the budget allows; nothing was generated.
    OverCellBudget {
        /// The cells the walk needs.
        cells: u64,
        /// The budget.
        budget: NonZeroU32,
    },
    /// The query is not inside what the candidates were walked for ([`LensCandidates::covers`]).
    NotCoveredByCandidates,
}

impl fmt::Display for FindLensesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceNotFound(e) => write!(f, "the lensed source does not resolve: {e}"),
            Self::SourceMotionNotTraced(e) => write!(f, "the lensed source cannot be traced: {e}"),
            Self::OverCellBudget { cells, budget } => write!(
                f,
                "the lens walk needs {cells} cells, over its budget of {budget}"
            ),
            Self::NotCoveredByCandidates => {
                f.write_str("the lens query is not inside what its candidates were walked for")
            }
        }
    }
}

impl Error for FindLensesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SourceNotFound(e) => Some(e),
            Self::SourceMotionNotTraced(e) => Some(e),
            Self::OverCellBudget { .. } | Self::NotCoveredByCandidates => None,
        }
    }
}

/// The mass of `record`'s system at `t`, when the light passes it, or `None` before it is born:
/// a free-floating object's mass mark, or the sum of its stars' masses then, remnants included
/// (Design note 13). A member of the galactic centre has `None` too until plan 09's P09.T28 gives
/// it an orbit to be seen on, since no lens can be placed without one (P12.T4.b as built).
///
/// # Panics
///
/// If `record` is a feature member that does not resolve in `galaxy`.
#[must_use]
pub fn lens_mass_at(
    galaxy: &Galaxy,
    record: &SystemRecord,
    t: UniverseTime,
) -> Option<SolarMasses> {
    if record.existence_at(t) == Existence::NoSystemYet {
        return None;
    }
    match record.kind() {
        SystemKind::BrownDwarf | SystemKind::RoguePlanet => Some(record.primary_initial_mass()),
        SystemKind::Stellar => {
            let stars = match record.origin() {
                SystemOrigin::Grid(_) => SystemStars::generate(galaxy, record),
                SystemOrigin::FeatureMember { .. } => {
                    let SystemIdKind::FeatureMember(id) = record.id().kind() else {
                        unreachable!("a feature member's record is built with a member ID")
                    };
                    resolve_member(galaxy, &NoInteriorCache, id)
                        .expect("a feature member's record resolves in the galaxy that placed it")
                        .stars(galaxy)
                }
                SystemOrigin::CentreMember { .. } => return None,
            };
            let mass = stars
                .stars()
                .iter()
                .filter_map(|star| star.state_at(t))
                .fold(0.0, |sum, state| sum + state.mass().value());
            (mass > 0.0).then_some(SolarMasses::new(mass))
        }
    }
}

/// The heaviest system a layer can hold: its band's upper mass for every star it can have.
fn layer_mass_bound(layer: Layer) -> SolarMasses {
    let hi = MassBand::of_layer(layer).hi();
    SolarMasses::new(match layer {
        Layer::BrownDwarf | Layer::RoguePlanet => hi,
        Layer::A | Layer::B | Layer::C | Layer::D | Layer::E => {
            hi * f64::from(u8::try_from(1 + MAX_COMPANIONS).expect("a handful of stars"))
        }
    })
}

/// The geometry of the sightline at the window's middle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Geometry {
    observer: Observer,
    /// The unit vector from the observer to the source.
    n: [f64; 3],
    /// The source's distance, metres.
    d_s_m: f64,
    /// The source's velocity across the line, m/s.
    source_across: [f64; 3],
    /// The source's speed, m/s.
    source_speed: f64,
    /// Seconds from the window's middle to its start and end.
    tau: (f64, f64),
}

impl Geometry {
    /// The part of `v` across the line of sight.
    fn across(&self, v: [f64; 3]) -> [f64; 3] {
        let along = dot(v, self.n);
        [0, 1, 2].map(|a| v[a] - along * self.n[a])
    }
}

/// A lens candidate: its record and its line.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Candidate {
    record: SystemRecord,
    line: Drift,
}

/// Every system that lenses the query's source during its window, as its observer sees it (plan
/// 12's Provides; Design note 13).
///
/// A lens is a point mass of its system's mass when the light passes it ([`lens_mass_at`]), and
/// its position is taken then, t − `D_l` ÷ c ([`retarded`] at the window's middle, the lens and the
/// source moving in straight lines across the window). The walk takes the grid's layers by the
/// query's mass floor and the substellar layers it asks for, cell by cell along the sightline
/// ([`cells_along_segment`]), and each source through its sphere method on a chain of spheres
/// along the sightline, asked twice a year apart to read each hit's velocity.
///
/// It is the cold walk and the evaluation in one: [`lens_candidates`] for the query's own window,
/// then [`lenses_among`] them. A caller that watches one sightline from one place keeps the
/// candidates and renews the window from them, which costs milliseconds against the walk's
/// seconds (ruling 143.3).
///
/// **The tube** (P12.T4.b as built; ruling 143.3). Design note 13 gives its radius as the reach
/// times the largest Einstein radius on the sightline plus
/// [`PAD_SPEED`](crate::galaxy::query::PAD_SPEED) times the window. Cells hold epoch positions,
/// and a lens is taken at its retarded time, up to the source's light time before the window, so a
/// tube of that radius would hold the lenses of epoch positions and miss the ones on the line then.
/// The radius here adds, for a galaxy whose systems move, the farthest a lens can have moved
/// between the epoch and its retarded time, which grows along the sightline: the tube is a cone,
/// cut into 64 pieces each as wide as its far end needs. The speed is the local escape speed's
/// bound over the piece's reach ([`LensWalkPlan`]), because plan 08 cuts every grid velocity
/// below the escape speed at its epoch position, and layer E keeps
/// [`UNBOUND_PAD_SPEED`] for the exempt classes plan 08 will place there. A galaxy built without
/// its kinematic tables has no motion and needs no pad.
///
/// The result does not depend on the cache or on the order of the sources: every candidate is
/// taken once, by ID, and the events are sorted by peak and then by lens.
///
/// # Errors
///
/// [`FindLensesError::SourceNotFound`] if the sightline's source does not resolve,
/// [`FindLensesError::SourceMotionNotTraced`] if it is a member of the galactic centre, and
/// [`FindLensesError::OverCellBudget`] if the tube's cells exceed the budget, which is decided
/// before any cell is generated.
///
/// # Panics
///
/// If a source's hit or the source's record is a feature member that does not resolve in
/// `galaxy`, which only a record of another galaxy can be.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, NoCache, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::lensing::{LensQuery, LensSightline, lenses_along};
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(4));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 790, 0])?, &mut cell);
/// let source = cell.first().ok_or("a layer-C cell of the disc holds a system")?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let year = UniverseTime::from_julian_years(1).ok_or("in range")?;
/// let window = TimeWindow::new(UniverseTime::EPOCH, year)?;
/// let query = LensQuery::builder(LensSightline::new(sun, source.id()), window).build()?;
/// let lenses = lenses_along(&galaxy, &mut NoCache::new(), &[], &query)?;
/// // Seven hundred light-years of the disc: almost never a lens in a year.
/// assert!(lenses.events().len() <= 1);
/// assert!(lenses.walked().cells() > 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn lenses_along<C: CellCache>(
    galaxy: &Galaxy,
    cache: &mut C,
    sources: &[&dyn SystemSource],
    query: &LensQuery,
) -> Result<LensResult, FindLensesError> {
    let candidates = lens_candidates(galaxy, cache, query)?;
    lenses_among(galaxy, &candidates, sources, query)
}

/// The grid's lens candidates of one sightline over a span of observer times: the cold walk, on
/// one thread (ruling 143.3).
///
/// Every grid system that could lens the source during any window inside `query`'s, as its
/// observer sees it, is kept, with its line: those whose lines come within the reach of the
/// source's direction, widened by a slack ([`LensCandidates`]). [`lenses_among`] then evaluates
/// any window inside it from the short list alone. It is [`LensWalkPlan::new`], one
/// [`LensWalkPlan::walk`] over every cell and [`LensWalkPlan::finish`]; a caller with a pool of
/// threads walks the cells in chunks instead, with the same result.
///
/// # Errors
///
/// As [`lenses_along`].
///
/// # Panics
///
/// If the source's record is a feature member that does not resolve in `galaxy`.
///
/// # Examples
///
/// A sensor watches a disc source for a century; each year's lenses come from one walk.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, NoCache, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::lensing::{LensQuery, LensSightline, lens_candidates, lenses_along, lenses_among};
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(4));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 790, 0])?, &mut cell);
/// let source = cell.first().ok_or("a layer-C cell of the disc holds a system")?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let sightline = LensSightline::new(sun, source.id());
/// let year = |y| UniverseTime::from_julian_years(y).ok_or("in range");
/// let century = LensQuery::builder(sightline, TimeWindow::new(year(0)?, year(100)?)?).build()?;
/// let candidates = lens_candidates(&galaxy, &mut NoCache::new(), &century)?;
/// let one_year = LensQuery::builder(sightline, TimeWindow::new(year(41)?, year(42)?)?).build()?;
/// assert_eq!(
///     lenses_among(&galaxy, &candidates, &[], &one_year)?.events(),
///     lenses_along(&galaxy, &mut NoCache::new(), &[], &one_year)?.events()
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn lens_candidates<C: CellCache>(
    galaxy: &Galaxy,
    cache: &mut C,
    query: &LensQuery,
) -> Result<LensCandidates, FindLensesError> {
    let plan = LensWalkPlan::new(galaxy, query)?;
    let chunk = plan.walk(galaxy, cache, plan.cells());
    Ok(plan.finish([chunk]))
}

/// The lenses of `query` among `candidates`, and among `sources` as [`lenses_along`] asks them:
/// a renewal of a watched sightline without a walk (ruling 143.3).
///
/// The result's events are exactly those [`lenses_along`] finds for `query`; its census is the
/// cold walk's, with this query's layers and spheres.
///
/// # Errors
///
/// [`FindLensesError::NotCoveredByCandidates`] if `query` is not inside what `candidates` were
/// walked for: another sightline, a window reaching outside theirs, a wider reach, or a layer they
/// did not walk.
///
/// # Panics
///
/// If a source's hit is a feature member that does not resolve in `galaxy`.
pub fn lenses_among(
    galaxy: &Galaxy,
    candidates: &LensCandidates,
    sources: &[&dyn SystemSource],
    query: &LensQuery,
) -> Result<LensResult, FindLensesError> {
    if !candidates.covers(query) {
        return Err(FindLensesError::NotCoveredByCandidates);
    }
    let geometry = geometry(&candidates.source_line, query);
    let layers = query.layers();
    let mut walked = LensCensus {
        layers,
        ..candidates.walked
    };
    let mut events = Vec::new();
    for candidate in &candidates.candidates {
        if layers.contains(candidate.record.layer()) {
            events.extend(event_of(galaxy, &geometry, query, candidate, sources));
        }
    }
    let (from_sources, spheres) = source_candidates(galaxy, &geometry, query, sources);
    walked.spheres = spheres;
    for candidate in &from_sources {
        if candidate.record.id() == query.sightline.source {
            continue;
        }
        walked.systems_examined += 1;
        events.extend(event_of(galaxy, &geometry, query, candidate, &[]));
    }
    events.sort_by(|a, b| {
        a.peak
            .cmp(&b.peak)
            .then_with(|| a.lens.raw().cmp(&b.lens.raw()))
    });
    Ok(LensResult { events, walked })
}

/// How far, beyond the reach, a candidate's least approach may pass and still be kept: 0.05 ly,
/// which is fifty to a thousand Einstein radii, so that a window inside the candidates' own is
/// judged on the same line to far better than that (ruling 143.3 as built).
const KEEP_SLACK_LY: f64 = 0.05;

/// The share of the lens's and the source's motion over the candidates' half-span added to the
/// slack: what linearising their paths at the span's middle, instead of the window's, can move a
/// least approach by, with room to spare (the retarded time runs at 1 ÷ (1 + `v_r` ÷ c) of the
/// observer's, 1% at 3,000 km/s).
const KEEP_MOTION_SHARE: f64 = 0.05;

/// The kept candidates of one sightline over a span of observer times, walked once (ruling 143.3).
///
/// Built by [`lens_candidates`] or, in chunks, by a [`LensWalkPlan`]. A candidate is a grid system
/// whose line passes within the reach of the source's direction during the span, the reach being
/// [`LensQuery::max_impact`] times the largest Einstein radius on the sightline, plus a slack of
/// 0.05 ly and a twentieth of the lens's and the source's motion over the span, so that every
/// window inside the span finds the same lenses from the candidates as a walk would. Sources
/// (features) are not candidates: they are asked again for each window, since their lines are read
/// from each window's own spheres.
///
/// The candidates belong to one observer position and one source: the caller keys a cache of them
/// by those (Design note 8's "the sim holds no cache"). Walking them is a pure function of the
/// query, so any cache hit gives the same bits as a fresh walk.
#[derive(Debug, Clone, PartialEq)]
pub struct LensCandidates {
    query: LensQuery,
    source_line: Drift,
    candidates: Vec<Candidate>,
    walked: LensCensus,
}

impl LensCandidates {
    /// The query the candidates were walked for: its window is the span they serve.
    #[must_use]
    pub const fn query(&self) -> &LensQuery {
        &self.query
    }

    /// The number of candidates kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Whether no candidate was kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// What the cold walk walked.
    #[must_use]
    pub const fn walked(&self) -> &LensCensus {
        &self.walked
    }

    /// Whether `query` lies inside what the candidates serve: the same sightline, a window inside
    /// theirs, a reach no wider, and layers they walked.
    #[must_use]
    pub fn covers(&self, query: &LensQuery) -> bool {
        let own = &self.query;
        let layers = own.layers();
        query.sightline == own.sightline
            && query.window.start() >= own.window.start()
            && query.window.end() <= own.window.end()
            && query.max_impact <= own.max_impact
            && query.layers().iter().all(|layer| layers.contains(layer))
    }
}

/// A part of a cold walk: the candidates found in some of a [`LensWalkPlan`]'s cells.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LensCandidateChunk {
    candidates: Vec<Candidate>,
    cells: u64,
    systems_examined: u64,
}

impl LensCandidateChunk {
    /// The cells walked.
    #[must_use]
    pub const fn cells(&self) -> u64 {
        self.cells
    }

    /// The systems looked at.
    #[must_use]
    pub const fn systems_examined(&self) -> u64 {
        self.systems_examined
    }
}

/// A cold lens walk laid out before any cell is generated, so that its cells can be walked in
/// chunks on a caller's threads (ruling 143.3; the sim spawns none, plan 12's Design note 8).
///
/// [`new`](Self::new) resolves the source, fixes the geometry at the window's middle and lists the
/// cone's cells, checking them against the budget. [`walk`](Self::walk) is a pure function of the
/// plan and the cells it is given; [`finish`](Self::finish) merges the chunks, whatever their order
/// or split, into the [`LensCandidates`] a single [`walk`](Self::walk) of every cell gives.
///
/// **The pad** (ruling 143.3). For a galaxy whose systems move, each piece of the cone is widened
/// by the farthest a lens can have moved, at a speed bounded by the escape speed over the region
/// its epoch positions can lie in, not by plan 03's 1,000 km/s: plan 08 draws every grid velocity
/// below the lesser of that speed and 1,000 km/s at the system's epoch position. The bound is read
/// from a table of the potential's escape speed on 177 spherical radii, eight an octave from 2⁻⁴
/// to 2¹⁸ ly, at 13 polar angles each, maximised over every radius beyond each and raised by 2%
/// for what lies between the samples. It is a bound on the model's own escape speed at the
/// table's resolution, which a slow test checks at 10⁵ random points; nothing under it is dropped
/// between samples unless the escape speed rises by more than 2% within an eighth of an octave
/// outward. Layer E keeps [`UNBOUND_PAD_SPEED`].
///
/// **The prefilter** (ruling 143.3). Each system is tested first by its epoch position against the
/// cone at its own distance, with its own speed bound; the survivors' velocities are drawn and
/// their retarded positions estimated in plain `f64`, two light-time steps, and tested with a
/// margin for that estimate; only the survivors of both take the exact retarded step.
///
/// # Examples
///
/// Four threads walk a quarter of the cells each; the merged candidates are the one-thread walk's.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::events::TimeWindow;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, NoCache, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::lensing::{LensQuery, LensSightline, LensWalkPlan, lens_candidates};
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(4));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 790, 0])?, &mut cell);
/// let source = cell.first().ok_or("a layer-C cell of the disc holds a system")?;
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let year = UniverseTime::from_julian_years(1).ok_or("in range")?;
/// let window = TimeWindow::new(UniverseTime::EPOCH, year)?;
/// let query = LensQuery::builder(LensSightline::new(sun, source.id()), window).build()?;
/// let plan = LensWalkPlan::new(&galaxy, &query)?;
/// let quarter = plan.cells().len().div_ceil(4);
/// # // wasm32-wasip1 has no threads: there the quarters are walked in turn on this one.
/// # #[cfg(target_family = "wasm")]
/// # let chunks: Vec<_> = plan
/// #     .cells()
/// #     .chunks(quarter)
/// #     .map(|cells| plan.walk(&galaxy, &mut NoCache::new(), cells))
/// #     .collect();
/// # #[cfg(not(target_family = "wasm"))]
/// let chunks = std::thread::scope(|scope| {
///     let handles: Vec<_> = plan
///         .cells()
///         .chunks(quarter)
///         .map(|cells| scope.spawn(|| plan.walk(&galaxy, &mut NoCache::new(), cells)))
///         .collect();
///     handles.into_iter().map(|h| h.join()).collect::<Result<Vec<_>, _>>()
/// })
/// .map_err(|_| "a walker panicked")?;
/// assert_eq!(plan.finish(chunks), lens_candidates(&galaxy, &mut NoCache::new(), &query)?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LensWalkPlan {
    query: LensQuery,
    source_line: Drift,
    geometry: Geometry,
    envelope: Option<EscapeEnvelope>,
    cells: Vec<CellKey>,
}

impl LensWalkPlan {
    /// The walk for `query`'s sightline and reach over its window, its cells listed.
    ///
    /// # Errors
    ///
    /// As [`lenses_along`].
    ///
    /// # Panics
    ///
    /// If the source's record is a feature member that does not resolve in `galaxy`.
    pub fn new(galaxy: &Galaxy, query: &LensQuery) -> Result<Self, FindLensesError> {
        let source_record =
            resolve(galaxy, query.sightline.source).map_err(FindLensesError::SourceNotFound)?;
        let source_line = Drift::of_record(galaxy, &source_record)
            .map_err(FindLensesError::SourceMotionNotTraced)?;
        let geometry = geometry(&source_line, query);
        let envelope = EscapeEnvelope::of(galaxy);
        let cells = grid_cells(&geometry, query, query.layers(), envelope.as_ref())?;
        Ok(Self {
            query: *query,
            source_line,
            geometry,
            envelope,
            cells,
        })
    }

    /// The query walked for.
    #[must_use]
    pub const fn query(&self) -> &LensQuery {
        &self.query
    }

    /// The cells to walk, every layer's, each once: layer by layer, and by key within a layer.
    #[must_use]
    pub fn cells(&self) -> &[CellKey] {
        &self.cells
    }

    /// The candidates among `cells`, which should be some of [`cells`](Self::cells), each walked
    /// in one chunk only.
    ///
    /// # Panics
    ///
    /// If a cell's system is a feature member that does not resolve in `galaxy`, which a grid cell
    /// cannot hold.
    #[must_use]
    pub fn walk<C: CellCache>(
        &self,
        galaxy: &Galaxy,
        cache: &mut C,
        cells: &[CellKey],
    ) -> LensCandidateChunk {
        let mut chunk = LensCandidateChunk::default();
        for &key in cells {
            cache.with_cell(galaxy, key, |records| {
                for record in records {
                    if record.id() == self.query.sightline.source {
                        continue;
                    }
                    chunk.systems_examined += 1;
                    if let Some(candidate) = self.keep(galaxy, record) {
                        chunk.candidates.push(candidate);
                    }
                }
            });
            chunk.cells += 1;
        }
        chunk
    }

    /// The candidates of every chunk, by ID, each once.
    #[must_use]
    pub fn finish(self, chunks: impl IntoIterator<Item = LensCandidateChunk>) -> LensCandidates {
        let mut walked = LensCensus {
            layers: self.query.layers(),
            ..LensCensus::default()
        };
        let mut candidates = Vec::new();
        for mut chunk in chunks {
            walked.cells += chunk.cells;
            walked.systems_examined += chunk.systems_examined;
            candidates.append(&mut chunk.candidates);
        }
        candidates.sort_unstable_by_key(|c: &Candidate| c.record.id().raw());
        candidates.dedup_by_key(|c| c.record.id().raw());
        LensCandidates {
            query: self.query,
            source_line: self.source_line,
            candidates,
            walked,
        }
    }

    /// The speed bound, over c, of a lens of `layer` whose epoch position is `radius_ly` from the
    /// centre.
    fn beta_at(&self, layer: Layer, radius_ly: f64) -> f64 {
        let Some(envelope) = &self.envelope else {
            return 0.0;
        };
        let speed = match layer {
            Layer::E => pad_speed(layer).value(),
            Layer::A | Layer::B | Layer::C | Layer::D | Layer::BrownDwarf | Layer::RoguePlanet => {
                envelope.bound(radius_ly).min(pad_speed(layer).value())
            }
        };
        MetresPerSecond::from(KilometresPerSecond::new(speed)).value() / SPEED_OF_LIGHT
    }

    /// `record` as a candidate, if its line passes within the kept reach during the span: the
    /// epoch position's test, the `f64` estimate's and the exact one's, in that order.
    fn keep(&self, galaxy: &Galaxy, record: &SystemRecord) -> Option<Candidate> {
        let g = &self.geometry;
        let layer = record.layer();
        let w = g
            .observer
            .position()
            .displacement_to(record.epoch_position())
            .metres();
        let w_norm = norm(w);
        let beta = self.beta_at(layer, norm(record.epoch_position().to_light_years_f64()));
        let half = g.tau.1;
        let t_abs = g.observer.time().since_epoch().as_seconds_f64().abs() + half;
        // The farthest the retarded position at any time of the span lies from the epoch
        // position: |v| (|t| + D ÷ c) with D ≤ |w| + that distance.
        let moved = beta * (SPEED_OF_LIGHT * t_abs + w_norm) / (1.0 - beta);
        let lens_speed = beta * SPEED_OF_LIGHT;
        let d_e = dot(w, g.n);
        let along_slack = along_slack(g, lens_speed) + moved;
        if d_e <= -along_slack || d_e >= g.d_s_m + along_slack {
            return None;
        }
        let across = norm(g.across(w));
        let limit = keep_reach(g, &self.query, layer, lens_speed)
            + moved
            + g.source_speed * half * (d_e.abs() + moved) / g.d_s_m
            + 1e-6 * METRES_PER_LIGHT_YEAR;
        if across > limit {
            return None;
        }
        let line = Drift::of_record(galaxy, record)
            .expect("a grid cell holds grid records, whose lines are traced");
        let v = line.velocity().metres_per_second();
        let middle = g.observer.time().since_epoch().as_seconds_f64();
        // Two light-time steps from the epoch position, in plain f64.
        let first = middle - w_norm / SPEED_OF_LIGHT;
        let guess = [0, 1, 2].map(|a| w[a] + v[a] * first);
        let second = middle - norm(guess) / SPEED_OF_LIGHT;
        let estimate = [0, 1, 2].map(|a| w[a] + v[a] * second);
        let beta_sq = beta * beta;
        let estimate_error = 0.01 * METRES_PER_LIGHT_YEAR
            + 2.0 * (beta_sq * moved + beta_sq * beta * (w_norm + moved));
        if !passes(g, &self.query, layer, estimate, v, estimate_error) {
            return None;
        }
        let seen = retarded(&g.observer, &line);
        let offset = g
            .observer
            .position()
            .displacement_to(seen.apparent_position())
            .metres();
        passes(
            g,
            &self.query,
            layer,
            offset,
            seen.velocity_then().metres_per_second(),
            0.0,
        )
        .then_some(Candidate {
            record: *record,
            line,
        })
    }
}

/// How far beyond either end of the sightline a lens moving at `lens_speed` m/s may lie at the
/// span's middle and still be between the observer and the source during the span, metres.
fn along_slack(g: &Geometry, lens_speed: f64) -> f64 {
    2.0 * (lens_speed + g.source_speed) * g.tau.1 + METRES_PER_LIGHT_YEAR
}

/// The kept reach at the lens for `layer` and a lens moving at `lens_speed` m/s, metres: the
/// largest Einstein radius on the sightline times the reach, plus the slack ([`LensCandidates`]).
fn keep_reach(g: &Geometry, query: &LensQuery, layer: Layer, lens_speed: f64) -> f64 {
    let half = g.tau.1;
    // The source's distance over the span, at its farthest.
    let d_s = g.d_s_m + 2.0 * g.source_speed * half + METRES_PER_LIGHT_YEAR;
    // D (D_s − D) ÷ D_s is largest at D_s ÷ 2: r_E = √(G M D_s) ÷ c.
    let einstein = (crate::units::consts::GM_SUN * layer_mass_bound(layer).value() * d_s).sqrt()
        / SPEED_OF_LIGHT;
    query.max_impact * einstein
        + KEEP_SLACK_LY * METRES_PER_LIGHT_YEAR
        + KEEP_MOTION_SHARE * (lens_speed + g.source_speed) * half
        + along_slack(g, lens_speed) / g.d_s_m * g.source_speed * half
}

/// Whether a lens at `offset` metres from the observer at the span's middle, moving at `v` m/s,
/// comes within the kept reach, widened by `extra` metres, of the source's moving line during the
/// span.
fn passes(
    g: &Geometry,
    query: &LensQuery,
    layer: Layer,
    offset: [f64; 3],
    v: [f64; 3],
    extra: f64,
) -> bool {
    let speed = norm(v);
    let d_l = dot(offset, g.n);
    let slack = along_slack(g, speed);
    if d_l <= -slack || d_l >= g.d_s_m + slack {
        return false;
    }
    let start = g.across(offset);
    let lens_across = g.across(v);
    let rate = [0, 1, 2].map(|a| lens_across[a] - g.source_across[a] * d_l / g.d_s_m);
    let rate_sq = dot(rate, rate);
    let half = g.tau.1;
    let closest = if rate_sq > 0.0 {
        (-dot(start, rate) / rate_sq).clamp(-half, half)
    } else {
        0.0
    };
    let least = norm([0, 1, 2].map(|a| start[a] + rate[a] * closest));
    least <= keep_reach(g, query, layer, speed) + extra
}

/// The grid's cells of every walked layer along the cone, piece by piece, each once.
///
/// Before any cell is listed, the cone's volume in cells, which its kept cells cover and so at
/// least number, is checked against the budget, so that a reach too wide for the budget costs
/// nothing to refuse.
fn grid_cells(
    geometry: &Geometry,
    query: &LensQuery,
    layers: LayerSet,
    envelope: Option<&EscapeEnvelope>,
) -> Result<Vec<CellKey>, FindLensesError> {
    // Each piece's radius, per layer, with the speed bound over the region its lenses' epoch
    // positions can lie in: within the radius plan 03's padding speed gives of the piece.
    let radii: Vec<Vec<LightYears>> = layers
        .iter()
        .map(|layer| {
            let mass = layer_mass_bound(layer);
            pieces(geometry)
                .map(|(near, far)| {
                    let Some(envelope) = envelope else {
                        return tube_radius(geometry, query, mass, 0.0, near, far);
                    };
                    let pad = MetresPerSecond::from(pad_speed(layer)).value() / SPEED_OF_LIGHT;
                    let widest = tube_radius(geometry, query, mass, pad, near, far);
                    let speed = match layer {
                        Layer::E => pad_speed(layer).value(),
                        Layer::A
                        | Layer::B
                        | Layer::C
                        | Layer::D
                        | Layer::BrownDwarf
                        | Layer::RoguePlanet => {
                            let from_centre = segment_distance_from_centre(
                                &point_along(geometry, near),
                                &point_along(geometry, far),
                            );
                            let inner = (from_centre - widest.value()).max(0.0);
                            envelope.bound(inner).min(pad_speed(layer).value())
                        }
                    };
                    let beta = MetresPerSecond::from(KilometresPerSecond::new(speed)).value()
                        / SPEED_OF_LIGHT;
                    tube_radius(geometry, query, mass, beta, near, far)
                })
                .collect()
        })
        .collect();
    let mut least = 0.0;
    for (layer, radii) in layers.iter().zip(&radii) {
        let size = f64::from(layer.cell_size_ly());
        for ((near, far), radius) in pieces(geometry).zip(radii) {
            let radius = radius.value();
            least += core::f64::consts::PI * radius * radius * (far - near) / (size * size * size);
        }
    }
    if least > f64::from(query.cell_budget.get()) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a count over the budget, reported only; an absurd one saturates"
        )]
        let cells = least.ceil() as u64;
        return Err(FindLensesError::OverCellBudget {
            cells,
            budget: query.cell_budget,
        });
    }
    let mut cells: Vec<CellKey> = Vec::new();
    for (layer, radii) in layers.iter().zip(&radii) {
        let first = cells.len();
        for ((near, far), &radius) in pieces(geometry).zip(radii) {
            cells.extend(cells_along_segment(
                layer,
                &point_along(geometry, near),
                &point_along(geometry, far),
                radius,
            ));
        }
        cells[first..].sort_unstable();
        let unique = dedup_tail(&mut cells, first);
        cells.truncate(first + unique);
    }
    let needed = u64::try_from(cells.len()).expect("a cell count fits in 64 bits");
    if needed > u64::from(query.cell_budget.get()) {
        return Err(FindLensesError::OverCellBudget {
            cells: needed,
            budget: query.cell_budget,
        });
    }
    Ok(cells)
}

/// The distance of the segment from `a` to `b` from the galactic centre, light-years.
fn segment_distance_from_centre(a: &GalacticPosition, b: &GalacticPosition) -> f64 {
    let a = a.to_light_years_f64();
    let b = b.to_light_years_f64();
    let ab = [0, 1, 2].map(|k| b[k] - a[k]);
    let length_sq = dot(ab, ab);
    let t = if length_sq > 0.0 {
        (-dot(a, ab) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    norm([0, 1, 2].map(|k| a[k] + ab[k] * t))
}

/// Radii of the escape-speed table per octave.
const ENVELOPE_PER_OCTAVE: u32 = 8;

/// The table's first radius, 2⁻⁴ ly, the potential grid's own first point; inside it the bound is
/// plan 03's padding speed.
const ENVELOPE_FIRST_LY: f64 = 0.0625;

/// The table's radii: 22 octaves, out to the potential grid's last point, 2¹⁸ ly.
const ENVELOPE_RADII: u32 = 22 * ENVELOPE_PER_OCTAVE + 1;

/// Polar angles sampled at each radius, from the plane to the pole.
const ENVELOPE_ANGLES: u32 = 13;

/// What the table's values are raised by, for what lies between its samples.
const ENVELOPE_MARGIN: f64 = 1.02;

/// A bound on the potential's escape speed beyond each spherical radius ([`LensWalkPlan`]'s pad).
#[derive(Debug, Clone, PartialEq)]
struct EscapeEnvelope {
    /// The largest sampled escape speed at or beyond each of the table's radii, km/s.
    beyond: Vec<f64>,
}

impl EscapeEnvelope {
    /// The table of `galaxy`'s potential, or `None` for a galaxy without kinematic tables, whose
    /// systems do not move.
    fn of(galaxy: &Galaxy) -> Option<Self> {
        galaxy.kinematics()?;
        let potential = galaxy.potential();
        let pad = PAD_SPEED.value();
        let mut beyond: Vec<f64> = (0..ENVELOPE_RADII)
            .map(|k| {
                let r =
                    ENVELOPE_FIRST_LY * math::exp2(f64::from(k) / f64::from(ENVELOPE_PER_OCTAVE));
                (0..ENVELOPE_ANGLES)
                    .map(|j| {
                        let theta = core::f64::consts::FRAC_PI_2 * f64::from(j)
                            / f64::from(ENVELOPE_ANGLES - 1);
                        let (sin, cos) = math::sin_cos(theta);
                        // The draw's own reading: none, or NaN, is its padding speed.
                        potential
                            .escape_speed(LightYears::new(r * cos), LightYears::new(r * sin))
                            .map_or(pad, KilometresPerSecond::value)
                    })
                    .map(|v| if v.is_nan() { pad } else { v })
                    .fold(0.0, f64::max)
            })
            .collect();
        for k in (0..beyond.len() - 1).rev() {
            beyond[k] = beyond[k].max(beyond[k + 1]);
        }
        Some(Self { beyond })
    }

    /// The bound at spherical radius `radius_ly`, km/s: the table's value at the nearest radius
    /// inside it, raised by the margin; plan 03's padding speed inside the first radius.
    fn bound(&self, radius_ly: f64) -> f64 {
        let pad = PAD_SPEED.value();
        if radius_ly.is_nan() || radius_ly < ENVELOPE_FIRST_LY {
            return pad;
        }
        let step =
            (math::log2(radius_ly / ENVELOPE_FIRST_LY) * f64::from(ENVELOPE_PER_OCTAVE)).floor();
        let last = self.beyond.len() - 1;
        let index = if step >= f64::from(ENVELOPE_RADII - 1) {
            last
        } else {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole number from 0 to the table's last index"
            )]
            let index = step as usize;
            index
        };
        self.beyond[index] * ENVELOPE_MARGIN
    }
}

/// The sources' candidates in ID order, through a chain of spheres along the sightline, each asked twice
/// a year apart to read velocities, and how many spheres were asked.
fn source_candidates(
    galaxy: &Galaxy,
    geometry: &Geometry,
    query: &LensQuery,
    sources: &[&dyn SystemSource],
) -> (Vec<Candidate>, u32) {
    let mut found: BTreeMap<u64, (usize, Candidate)> = BTreeMap::new();
    let mut spheres = 0;
    if sources.is_empty() {
        return (Vec::new(), spheres);
    }
    let layers = query.layers();
    let heaviest = SolarMasses::new(
        layers
            .iter()
            .map(|layer| layer_mass_bound(layer).value())
            .fold(0.0, f64::max),
    );
    let beta = MetresPerSecond::from(UNBOUND_PAD_SPEED).value() / SPEED_OF_LIGHT;
    let step = Span::from_julian_years(DIFFERENCE_YEARS).expect("a year fits the clock");
    let mut first = Vec::new();
    let mut second = Vec::new();
    for (near, far) in pieces(geometry) {
        let half = 0.5 * (far - near);
        let middle = f64::midpoint(near, far);
        let reach = tube_radius(geometry, query, heaviest, 0.0, near, far).value()
            + half
            + beta * (half + years(step))
            + 1e-6;
        let at = geometry
            .observer
            .time()
            .checked_sub(light_years_as_span(middle))
            .expect("a light time across the cube keeps a time on the clock");
        let later = at.checked_add(step).expect("a year later is on the clock");
        let centre = point_along(geometry, middle);
        let sphere = |t| {
            QuerySphere::new(centre, LightYears::new(reach), t, LightYears::ZERO)
                .expect("a sphere of a positive radius")
        };
        for (index, source) in sources.iter().enumerate() {
            first.clear();
            second.clear();
            source.systems_in_sphere(galaxy, &sphere(at), layers, &mut first);
            source.systems_in_sphere(galaxy, &sphere(later), layers, &mut second);
            spheres += 1;
            second.sort_by_key(|h| h.id().raw());
            for hit in &first {
                let Ok(k) = second.binary_search_by_key(&hit.id().raw(), |h| h.id().raw()) else {
                    continue;
                };
                let (from, _) = found
                    .entry(hit.id().raw())
                    .or_insert_with(|| (index, difference(hit, &second[k], at, step)));
                debug_assert_eq!(
                    *from,
                    index,
                    "two sources returned the ID {:?}: a source broke the contract that its IDs \
                     are its own",
                    hit.id()
                );
            }
        }
    }
    (found.into_values().map(|(_, c)| c).collect(), spheres)
}

/// Removes the duplicates of the sorted tail of `cells` from `first` on, returning its new length.
fn dedup_tail(cells: &mut Vec<CellKey>, first: usize) -> usize {
    let mut tail = cells.split_off(first);
    tail.dedup();
    let unique = tail.len();
    cells.append(&mut tail);
    unique
}

/// A source's hit read as a line from two positions a step apart.
fn difference(hit: &SystemHit, then: &SystemHit, at: UniverseTime, step: Span) -> Candidate {
    let moved = hit.position().displacement_to(then.position()).metres();
    let seconds = step.as_seconds_f64();
    let velocity = GalacticVelocity::new(moved.map(|m| m / seconds));
    Candidate {
        record: *hit.record(),
        line: Drift::through(hit.position(), at, velocity),
    }
}

/// The sightline's geometry at the window's middle.
fn geometry(source: &Drift, query: &LensQuery) -> Geometry {
    let window = query.window;
    let half = window
        .end()
        .checked_since(window.start())
        .expect("a window's ends differ by a span the clock holds")
        .as_seconds_f64()
        * 0.5;
    let middle = window
        .start()
        .checked_add(Span::from_seconds_f64(half).expect("half a window fits the clock"))
        .expect("the middle of a window is on the clock");
    let observer = Observer::new(query.sightline.observer, middle)
        .expect("a built query's observer and window are in the cube and the clock window");
    let seen = retarded(&observer, source);
    let to_source = observer
        .position()
        .displacement_to(seen.apparent_position());
    let d_s_m = to_source.length().value();
    let n = if d_s_m > 0.0 {
        to_source.metres().map(|m| m / d_s_m)
    } else {
        [1.0, 0.0, 0.0]
    };
    let source_velocity = seen.velocity_then().metres_per_second();
    let mut g = Geometry {
        observer,
        n,
        d_s_m,
        source_across: [0.0; 3],
        source_speed: norm(source_velocity),
        tau: (-half, half),
    };
    g.source_across = g.across(source_velocity);
    g
}

/// The pieces of the sightline, light-years from the observer: [`PIECES`] equal ones.
fn pieces(g: &Geometry) -> impl Iterator<Item = (f64, f64)> {
    let d_s_ly = g.d_s_m / METRES_PER_LIGHT_YEAR;
    (0..PIECES).map(move |k| {
        let near = d_s_ly * f64::from(k) / f64::from(PIECES);
        let far = d_s_ly * f64::from(k + 1) / f64::from(PIECES);
        (near, far)
    })
}

/// The point `ly` light-years from the observer towards the source.
fn point_along(g: &Geometry, ly: f64) -> GalacticPosition {
    let metres = ly * METRES_PER_LIGHT_YEAR;
    g.observer
        .position()
        .translated(GalacticDisplacement::new(g.n.map(|c| c * metres)))
        .expect("a point on a sightline inside the cube is addressable")
}

/// The cone's radius over the piece from `near_ly` to `far_ly` light-years for lenses of up to
/// `mass` moving at up to `beta` (a speed over c): the reach, what the source's line wanders over
/// the window, and the farthest a lens's epoch position can lie from where it is at its retarded
/// time (method documentation).
fn tube_radius(
    g: &Geometry,
    query: &LensQuery,
    mass: SolarMasses,
    beta: f64,
    near_ly: f64,
    far_ly: f64,
) -> LightYears {
    let (near, far) = (near_ly, far_ly);
    let half = g.tau.1;
    let d_s_ly = g.d_s_m / METRES_PER_LIGHT_YEAR;
    // How far the source's distance moves over the window, light-years.
    let source_shift = 2.0 * g.source_speed * half / SPEED_OF_LIGHT / SECONDS_PER_JULIAN_YEAR;
    let d_s_far = d_s_ly + source_shift;
    // D (D_s − D) ÷ D_s is largest at D_s ÷ 2.
    let widest = if near <= 0.5 * d_s_far && 0.5 * d_s_far <= far {
        0.5 * d_s_far
    } else if far < 0.5 * d_s_far {
        far
    } else {
        near
    };
    let lens_plane = widest * (d_s_far - widest) / d_s_far * METRES_PER_LIGHT_YEAR;
    let einstein = (4.0 * crate::units::consts::GM_SUN * mass.value()
        / (SPEED_OF_LIGHT * SPEED_OF_LIGHT)
        * lens_plane)
        .max(0.0)
        .sqrt()
        / METRES_PER_LIGHT_YEAR;
    let wander = norm(g.source_across) * half / SPEED_OF_LIGHT / SECONDS_PER_JULIAN_YEAR * far
        / d_s_ly.max(1.0);
    let reach = query.max_impact * einstein + wander + source_shift + 1e-6;
    // The lens moves at up to β c for |t| + D ÷ c, where its distance D is at most the piece's far
    // end plus the radius, which holds the motion itself: δ ≤ β (|t| + far + reach + δ) in years
    // and light-years.
    let t = g.observer.time().since_epoch().as_seconds_f64().abs() + half;
    let moved = beta * (t / SECONDS_PER_JULIAN_YEAR + far + reach) / (1.0 - beta);
    LightYears::new(reach + moved)
}

/// The event a candidate makes, if it passes within the query's reach during the window and none
/// of `suppressing` replaces it when the light passes it, as a range query drops a grid system a
/// source replaces (plan 03's merge rule).
fn event_of(
    galaxy: &Galaxy,
    g: &Geometry,
    query: &LensQuery,
    candidate: &Candidate,
    suppressing: &[&dyn SystemSource],
) -> Option<LensEvent> {
    // A source's centre member moves on an orbit that is not built (P09.T28): it is no lens yet.
    if matches!(candidate.record.origin(), SystemOrigin::CentreMember { .. }) {
        return None;
    }
    let seen = retarded(&g.observer, &candidate.line);
    let offset = g
        .observer
        .position()
        .displacement_to(seen.apparent_position())
        .metres();
    let d_l = dot(offset, g.n);
    if !(d_l > 0.0 && d_l < g.d_s_m) {
        return None;
    }
    let across = g.across(offset);
    let theta0 = across.map(|c| c / d_l);
    let lens_across = g.across(seen.velocity_then().metres_per_second());
    // The observer is at rest in the galactic frame, so it adds no term of its own.
    let omega = [0, 1, 2].map(|a| lens_across[a] / d_l - g.source_across[a] / g.d_s_m);
    let omega_sq = dot(omega, omega);
    let closest = if omega_sq > 0.0 {
        -dot(theta0, omega) / omega_sq
    } else {
        0.0
    };
    let at_peak = closest.clamp(g.tau.0, g.tau.1);
    let theta_at = |tau: f64| norm([0, 1, 2].map(|a| theta0[a] + omega[a] * tau));
    let least = theta_at(at_peak);
    // First with the heaviest mass the layer can hold, which needs no stars; then with its own.
    let bound = einstein_angle(
        layer_mass_bound(candidate.record.layer()),
        Metres::new(d_l),
        Metres::new(g.d_s_m),
    )
    .value();
    if least > query.max_impact * bound {
        return None;
    }
    if suppressing
        .iter()
        .any(|source| source.suppresses(galaxy, &candidate.record, seen.emitted()))
    {
        return None;
    }
    let mass = lens_mass_at(galaxy, &candidate.record, seen.emitted())?;
    let theta_e = einstein_angle(mass, Metres::new(d_l), Metres::new(g.d_s_m)).value();
    if theta_e <= 0.0 {
        return None;
    }
    let impact = least / theta_e;
    if impact > query.max_impact {
        return None;
    }
    let middle = g.observer.time();
    let peak = middle
        .checked_add(Span::from_seconds_f64(at_peak).expect("inside the window"))
        .expect("inside the window");
    let middle_s = middle.since_epoch().as_seconds_f64();
    Some(LensEvent {
        lens: candidate.record.id(),
        peak,
        impact,
        einstein_radius: Metres::new(theta_e * d_l),
        einstein_time: Seconds::new(if omega_sq > 0.0 {
            theta_e / omega_sq.sqrt()
        } else {
            f64::INFINITY
        }),
        peak_magnification: point_lens_magnification(impact),
        closest_s: middle_s + closest,
        closest_impact: theta_at(closest) / theta_e,
    })
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// A span in Julian years, as a float.
fn years(span: Span) -> f64 {
    span.as_julian_years_f64()
}

/// The light time over `ly` light-years: `ly` Julian years.
fn light_years_as_span(ly: f64) -> Span {
    Span::from_seconds_f64(ly * SECONDS_PER_JULIAN_YEAR).expect("a light time across the cube")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    use super::*;
    use crate::Seed;
    use crate::galaxy::Population;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{NoCache, generate_cell};
    use crate::galaxy::query::LayerCounts;
    use crate::observe::{Trajectory, light_time};

    /// A galaxy without its kinematic tables: nothing on the grid moves, so the only motion is
    /// what a test places.
    fn still_galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x1204_b000), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
        })
    }

    /// A lens placed by hand: a brown dwarf of `mass` on a straight line through `at` at `time`.
    #[derive(Debug)]
    struct Pinned {
        record: SystemRecord,
        line: Drift,
    }

    impl SystemSource for Pinned {
        fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
            LayerCounts::ZERO
        }

        fn systems_in_sphere(
            &self,
            _galaxy: &Galaxy,
            sphere: &QuerySphere,
            layers: LayerSet,
            out: &mut Vec<SystemHit>,
        ) {
            if !layers.contains(self.record.layer()) {
                return;
            }
            let position = self.line.position_at(sphere.time());
            let distance = LightYears::from(sphere.centre().distance_to(&position));
            if distance.value() <= sphere.radius().value() {
                out.push(SystemHit::new(self.record, position, distance));
            }
        }

        fn suppresses(&self, _galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
            false
        }
    }

    /// A brown dwarf's record of `mass` M☉ with an index no cell reaches, so no grid system has it.
    fn brown_dwarf(galaxy: &Galaxy, index: u32, mass: f64) -> SystemRecord {
        let key = CellKey::new(Layer::BrownDwarf, [0, 1_500, 0]).unwrap();
        SystemRecord::from_parts(
            key.candidate_id(index).unwrap(),
            GalacticPosition::ORIGIN,
            SystemOrigin::Grid(galaxy.fields().component_id(0).unwrap()),
            Population::OldThinDisc,
            SolarMasses::new(mass),
            crate::units::Years::new(5e9),
        )
    }

    struct Setup {
        sightline: LensSightline,
        d_s: f64,
        observer: GalacticPosition,
    }

    /// A source 8,000 ly coreward of an observer, both still.
    fn setup() -> Setup {
        let galaxy = still_galaxy();
        let mut cell = Vec::new();
        generate_cell(
            galaxy,
            CellKey::new(Layer::C, [2, 562, 0]).unwrap(),
            &mut cell,
        );
        let source = cell
            .first()
            .expect("a layer-C cell of the disc holds systems");
        let observer = source
            .epoch_position()
            .translated(GalacticDisplacement::new([
                0.0,
                8_000.0 * METRES_PER_LIGHT_YEAR,
                0.0,
            ]))
            .unwrap();
        Setup {
            sightline: LensSightline::new(observer, source.id()),
            d_s: 8_000.0 * METRES_PER_LIGHT_YEAR,
            observer,
        }
    }

    /// A lens half-way along the sightline, `impact` of its Einstein radii to the side at its
    /// closest, crossing at `speed` m/s so that its closest approach is seen at observer time
    /// `peak`.
    fn pinned(
        s: &Setup,
        index: u32,
        mass: f64,
        impact: f64,
        speed: f64,
        peak: UniverseTime,
    ) -> (Pinned, f64) {
        let galaxy = still_galaxy();
        let d_l = 0.5 * s.d_s;
        let r_e = (4.0 * crate::units::consts::GM_SUN * mass / (SPEED_OF_LIGHT * SPEED_OF_LIGHT)
            * d_l
            * (s.d_s - d_l)
            / s.d_s)
            .sqrt();
        // The source lies along −y from the observer; the lens sits off along +x and moves along
        // +z.
        let at = s
            .observer
            .translated(GalacticDisplacement::new([impact * r_e, -d_l, 0.0]))
            .unwrap();
        let emitted = peak
            .checked_sub(light_time(s.observer.distance_to(&at)))
            .unwrap();
        let line = Drift::through(&at, emitted, GalacticVelocity::new([0.0, 0.0, speed]));
        (
            Pinned {
                record: brown_dwarf(galaxy, 60_000 + index, mass),
                line,
            },
            r_e,
        )
    }

    fn days(d: f64) -> Span {
        Span::from_seconds_f64(d * 86_400.0).unwrap()
    }

    fn window_about(t: UniverseTime, half_days: f64) -> TimeWindow {
        TimeWindow::new(
            t.checked_sub(days(half_days)).unwrap(),
            t.checked_add(days(half_days)).unwrap(),
        )
        .unwrap()
    }

    /// P12.T4.b: a pinned lens gives the textbook light curve: 1.34 at u = 1, symmetric about the
    /// peak, with the Einstein time of the relative transverse speed.
    #[test]
    fn lensing_pinned_lens_gives_the_textbook_light_curve() {
        let galaxy = still_galaxy();
        let s = setup();
        let peak_at = UniverseTime::from_julian_years(200).unwrap();
        let speed = 100e3;
        let (lens, r_e) = pinned(&s, 1, 0.05, 1.0, speed, peak_at);
        let query = LensQuery::builder(s.sightline, window_about(peak_at, 40.0))
            .substellar(SubstellarRequest::BrownDwarfs)
            .build()
            .unwrap();
        let result = lenses_along(galaxy, &mut NoCache::new(), &[&lens], &query).unwrap();
        let event = result
            .events()
            .iter()
            .find(|e| e.lens() == lens.record.id())
            .expect("the pinned lens is found");
        assert!(
            (event.impact() - 1.0).abs() < 1e-4,
            "u₀ = {}",
            event.impact()
        );
        assert!(
            (event.peak_magnification() - 1.341_64).abs() < 1e-4,
            "A = {}",
            event.peak_magnification()
        );
        let gap = event
            .peak()
            .checked_since(peak_at)
            .unwrap()
            .as_seconds_f64();
        assert!(
            gap.abs() < 60.0,
            "the peak is {gap} s from where it was placed"
        );
        assert!((event.einstein_radius().value() / r_e - 1.0).abs() < 1e-6);
        let t_e = r_e / speed;
        assert!(
            (event.einstein_time().value() / t_e - 1.0).abs() < 1e-3,
            "t_E = {} s against {t_e} s",
            event.einstein_time().value()
        );
        // Symmetric about the peak, and u = √2 one Einstein time away.
        for k in [0.5, 1.0, 2.0, 3.0] {
            let tau = Span::from_seconds_f64(k * t_e).unwrap();
            let before = event.magnification_at(event.peak().checked_sub(tau).unwrap());
            let after = event.magnification_at(event.peak().checked_add(tau).unwrap());
            assert!((before - after).abs() < 1e-9, "{before} against {after}");
            let expected = point_lens_magnification((1.0 + k * k).sqrt());
            assert!(
                (after - expected).abs() < 1e-4,
                "{after} against {expected} at {k} t_E"
            );
        }
        assert!((magnification_at(event, event.peak()) - event.peak_magnification()).abs() < 1e-9);
        // A source that is not asked for layers F sees nothing of it.
        let stellar = LensQuery::builder(s.sightline, window_about(peak_at, 40.0))
            .build()
            .unwrap();
        let result = lenses_along(galaxy, &mut NoCache::new(), &[&lens], &stellar).unwrap();
        assert!(result.events().iter().all(|e| e.lens() != lens.record.id()));
    }

    /// A lens that passes wide of the reach is not an event; one whose peak lies outside the window
    /// is, at the window's edge, if it is within reach there.
    #[test]
    fn lensing_counts_a_lens_within_reach_and_clamps_its_peak_to_the_window() {
        let galaxy = still_galaxy();
        let s = setup();
        let peak_at = UniverseTime::EPOCH;
        let (wide, _) = pinned(&s, 2, 0.05, 3.5, 100e3, peak_at);
        let (late, _) = pinned(
            &s,
            3,
            0.05,
            0.5,
            100e3,
            peak_at.checked_add(days(12.0)).unwrap(),
        );
        let query = LensQuery::builder(s.sightline, window_about(peak_at, 5.0))
            .substellar(SubstellarRequest::BrownDwarfs)
            .build()
            .unwrap();
        let result = lenses_along(galaxy, &mut NoCache::new(), &[&wide, &late], &query).unwrap();
        assert!(result.events().iter().all(|e| e.lens() != wide.record.id()));
        let edge = result
            .events()
            .iter()
            .find(|e| e.lens() == late.record.id())
            .expect("the late lens is within reach at the window's end");
        assert_eq!(edge.peak(), query.window().end());
        assert!(edge.impact() > 0.5);
        assert!(
            edge.magnification_at(peak_at.checked_add(days(12.0)).unwrap())
                > edge.peak_magnification()
        );
    }

    /// A cache that keeps every cell.
    #[derive(Default)]
    struct Keep(BTreeMap<CellKey, Vec<SystemRecord>>);

    impl CellCache for Keep {
        fn with_cell<R>(
            &mut self,
            galaxy: &Galaxy,
            key: CellKey,
            f: impl FnOnce(&[SystemRecord]) -> R,
        ) -> R {
            let cell = self.0.entry(key).or_insert_with(|| {
                let mut out = Vec::new();
                generate_cell(galaxy, key, &mut out);
                out
            });
            f(cell)
        }
    }

    /// The result does not depend on the order of the sources or on the cache.
    #[test]
    fn lensing_is_order_independent() {
        let galaxy = still_galaxy();
        let s = setup();
        let peak_at = UniverseTime::EPOCH;
        let (a, _) = pinned(&s, 4, 0.05, 0.3, 80e3, peak_at);
        let (b, _) = pinned(
            &s,
            5,
            0.02,
            1.5,
            150e3,
            peak_at.checked_add(days(3.0)).unwrap(),
        );
        let query = LensQuery::builder(s.sightline, window_about(peak_at, 20.0))
            .substellar(SubstellarRequest::BrownDwarfs)
            .build()
            .unwrap();
        let forward = lenses_along(galaxy, &mut NoCache::new(), &[&a, &b], &query).unwrap();
        let backward = lenses_along(galaxy, &mut Keep::default(), &[&b, &a], &query).unwrap();
        assert_eq!(forward, backward);
        assert!(forward.events().len() >= 2);
        assert!(
            forward
                .events()
                .windows(2)
                .all(|p| p[0].peak() <= p[1].peak())
        );
        let mut keep = Keep::default();
        let first = lenses_along(galaxy, &mut keep, &[&a, &b], &query).unwrap();
        let again = lenses_along(galaxy, &mut keep, &[&a, &b], &query).unwrap();
        assert_eq!(first, again);
        hyperion_testkit::order::assert_order_independent(&[0_usize, 1, 2], |&k| {
            let sources: [&dyn SystemSource; 2] = if k % 2 == 0 { [&a, &b] } else { [&b, &a] };
            lenses_along(galaxy, &mut NoCache::new(), &sources, &query).unwrap()
        });
    }

    #[test]
    fn lensing_rejects_a_query_over_budget_and_bad_queries() {
        let galaxy = still_galaxy();
        let s = setup();
        let window = window_about(UniverseTime::EPOCH, 1.0);
        let tight = LensQuery::builder(s.sightline, window)
            .cell_budget(NonZeroU32::new(10).unwrap())
            .build()
            .unwrap();
        assert!(matches!(
            lenses_along(galaxy, &mut NoCache::new(), &[], &tight),
            Err(FindLensesError::OverCellBudget { cells, .. }) if cells > 10
        ));
        let late = TimeWindow::new(
            UniverseTime::from_julian_years(999).unwrap(),
            UniverseTime::from_julian_years(1_001).unwrap(),
        )
        .unwrap();
        assert_eq!(
            LensQuery::builder(s.sightline, late).build(),
            Err(BuildLensQueryError::WindowOutsideClockWindow(late))
        );
        assert_eq!(
            LensQuery::builder(s.sightline, window)
                .max_impact(0.0)
                .build(),
            Err(BuildLensQueryError::MaxImpactNotPositive(0.0))
        );
        let outside = GalacticPosition::from_light_years([70_000.0, 0.0, 0.0]).unwrap();
        assert_eq!(
            LensQuery::builder(LensSightline::new(outside, s.sightline.source()), window).build(),
            Err(BuildLensQueryError::ObserverOutsideRootCube)
        );
        let nobody = CellKey::new(Layer::C, [2, 562, 0])
            .unwrap()
            .candidate_id(4_000)
            .unwrap();
        let missing = LensQuery::builder(LensSightline::new(s.observer, nobody), window)
            .build()
            .unwrap();
        assert!(matches!(
            lenses_along(galaxy, &mut NoCache::new(), &[], &missing),
            Err(FindLensesError::SourceNotFound(_))
        ));
        for message in [
            BuildLensQueryError::ObserverOutsideRootCube.to_string(),
            BuildLensQueryError::MaxImpactNotPositive(0.0).to_string(),
            FindLensesError::OverCellBudget {
                cells: 11,
                budget: NonZeroU32::new(10).unwrap(),
            }
            .to_string(),
        ] {
            assert!(!message.chars().next().unwrap().is_uppercase(), "{message}");
            assert!(!message.ends_with('.'), "{message}");
        }
    }

    /// A source that replaces a grid system also takes it out of the lens walk, as a range query
    /// leaves it out (plan 03's merge rule).
    #[test]
    fn lensing_leaves_out_a_grid_system_a_source_suppresses() {
        #[derive(Debug)]
        struct Everything;
        impl SystemSource for Everything {
            fn expected_in_sphere(&self, _galaxy: &Galaxy, _sphere: &QuerySphere) -> LayerCounts {
                LayerCounts::ZERO
            }
            fn systems_in_sphere(
                &self,
                _galaxy: &Galaxy,
                _sphere: &QuerySphere,
                _layers: LayerSet,
                _out: &mut Vec<SystemHit>,
            ) {
            }
            fn suppresses(
                &self,
                _galaxy: &Galaxy,
                _record: &SystemRecord,
                _t: UniverseTime,
            ) -> bool {
                true
            }
        }
        let galaxy = still_galaxy();
        let s = setup();
        // A reach wide enough that the grid gives some lenses on this sightline.
        let query = LensQuery::builder(s.sightline, window_about(UniverseTime::EPOCH, 1.0))
            .max_impact(30_000.0)
            .build()
            .unwrap();
        let open = lenses_along(galaxy, &mut NoCache::new(), &[], &query).unwrap();
        assert!(
            !open.events().is_empty(),
            "the wide reach finds grid lenses"
        );
        let replaced = lenses_along(galaxy, &mut NoCache::new(), &[&Everything], &query).unwrap();
        assert!(replaced.events().is_empty());
        assert_eq!(replaced.walked().cells(), open.walked().cells());
    }

    /// A centre member as the lensed source is refused with a typed error until its orbit is
    /// built (P09.T28); the black hole resolves without the centre's model.
    #[test]
    fn lensing_refuses_a_centre_member_as_its_source() {
        let galaxy = still_galaxy();
        let s = setup();
        let id = SystemId::from(crate::id::CentreMemberId::CENTRAL_BLACK_HOLE);
        let query = LensQuery::builder(
            LensSightline::new(s.observer, id),
            window_about(UniverseTime::EPOCH, 1.0),
        )
        .build()
        .unwrap();
        assert_eq!(
            lenses_along(galaxy, &mut NoCache::new(), &[], &query),
            Err(FindLensesError::SourceMotionNotTraced(
                TraceMotionError::CentreOrbitNotBuilt(id)
            ))
        );
    }

    /// Ruling 143.3: a renewal from candidates walked for a longer span finds exactly the lenses a
    /// fresh walk of each window finds, pinned moving lenses and grid lenses alike, and refuses a
    /// window outside the span, a wider reach or another layer.
    #[test]
    fn lensing_renewal_from_candidates_equals_a_fresh_walk() {
        let galaxy = still_galaxy();
        let s = setup();
        let peak_at = UniverseTime::EPOCH;
        let (a, _) = pinned(&s, 6, 0.05, 0.3, 80e3, peak_at);
        let (b, _) = pinned(
            &s,
            7,
            0.02,
            1.5,
            150e3,
            peak_at.checked_add(days(30.0)).unwrap(),
        );
        let sources: [&dyn SystemSource; 2] = [&a, &b];
        // A reach wide enough that the grid gives lenses on this sightline.
        let reach = 30_000.0;
        let span = window_about(peak_at, 200.0);
        let wide = LensQuery::builder(s.sightline, span)
            .max_impact(reach)
            .substellar(SubstellarRequest::BrownDwarfs)
            .build()
            .unwrap();
        let candidates = lens_candidates(galaxy, &mut NoCache::new(), &wide).unwrap();
        assert!(!candidates.is_empty());
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x143_3001);
        let mut found = 0;
        for _ in 0..12 {
            let start = days(-200.0 + 380.0 * lcg.next_f64());
            let length = days(1.0 + 19.0 * lcg.next_f64());
            let from = peak_at.checked_add(start).unwrap();
            let to = from.checked_add(length).unwrap().min(span.end());
            let window = TimeWindow::new(from, to).unwrap();
            let query = LensQuery::builder(s.sightline, window)
                .max_impact(reach)
                .substellar(SubstellarRequest::BrownDwarfs)
                .build()
                .unwrap();
            let renewed = lenses_among(galaxy, &candidates, &sources, &query).unwrap();
            let fresh = lenses_along(galaxy, &mut NoCache::new(), &sources, &query).unwrap();
            assert_eq!(renewed.events(), fresh.events());
            found += renewed.events().len();
        }
        assert!(found > 12, "{found} lenses over the windows");
        // The candidates' own window gives the fresh walk's result, census and all.
        assert_eq!(
            lenses_among(galaxy, &candidates, &sources, &wide).unwrap(),
            lenses_along(galaxy, &mut NoCache::new(), &sources, &wide).unwrap()
        );
        let outside = LensQuery::builder(s.sightline, window_about(peak_at, 201.0))
            .max_impact(reach)
            .substellar(SubstellarRequest::BrownDwarfs)
            .build()
            .unwrap();
        let wider = LensQuery::builder(s.sightline, window_about(peak_at, 5.0))
            .max_impact(2.0 * reach)
            .substellar(SubstellarRequest::BrownDwarfs)
            .build()
            .unwrap();
        let rogues = LensQuery::builder(s.sightline, window_about(peak_at, 5.0))
            .max_impact(reach)
            .substellar(SubstellarRequest::BrownDwarfsAndRoguePlanets)
            .build()
            .unwrap();
        for query in [outside, wider, rogues] {
            assert!(!candidates.covers(&query));
            assert_eq!(
                lenses_among(galaxy, &candidates, &sources, &query),
                Err(FindLensesError::NotCoveredByCandidates)
            );
        }
        let message = FindLensesError::NotCoveredByCandidates.to_string();
        assert!(!message.chars().next().unwrap().is_uppercase() && !message.ends_with('.'));
    }

    /// Ruling 143.3: walking the cells in chunks, in any split and order, merges to the one-thread
    /// walk's candidates.
    #[test]
    fn lensing_walk_in_chunks_equals_one_walk() {
        let galaxy = still_galaxy();
        let s = setup();
        let query = LensQuery::builder(s.sightline, window_about(UniverseTime::EPOCH, 10.0))
            .max_impact(30_000.0)
            .build()
            .unwrap();
        let whole = lens_candidates(galaxy, &mut NoCache::new(), &query).unwrap();
        assert!(!whole.is_empty());
        let plan = LensWalkPlan::new(galaxy, &query).unwrap();
        assert_eq!(plan.query(), &query);
        let cells = plan.cells();
        for parts in [2, 3, 7] {
            let size = cells.len().div_ceil(parts);
            let mut chunks: Vec<LensCandidateChunk> = cells
                .chunks(size)
                .map(|part| plan.walk(galaxy, &mut Keep::default(), part))
                .collect();
            chunks.reverse();
            assert_eq!(
                chunks.iter().map(LensCandidateChunk::cells).sum::<u64>(),
                whole.walked().cells()
            );
            assert_eq!(plan.clone().finish(chunks), whole);
        }
        assert_eq!(
            whole.walked().systems_examined(),
            lenses_along(galaxy, &mut NoCache::new(), &[], &query)
                .unwrap()
                .walked()
                .systems_examined()
        );
    }

    /// Ruling 143.3: the kept reach holds every lens that any window inside the candidates' span
    /// finds, for lenses moving at up to 1,000 km/s past a moving source: the linear paths at the
    /// span's middle, and its slack, cover what each window's own geometry finds.
    #[test]
    fn lensing_kept_reach_holds_every_lens_of_a_window_inside_the_span() {
        let galaxy = still_galaxy();
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x143_3002);
        let observer = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let year = |y: f64| Span::from_seconds_f64(y * SECONDS_PER_JULIAN_YEAR).unwrap();
        let middle = UniverseTime::from_julian_years(300).unwrap();
        let span = TimeWindow::new(
            middle.checked_sub(year(100.0)).unwrap(),
            middle.checked_add(year(100.0)).unwrap(),
        )
        .unwrap();
        let direction = [0.3, -0.9, -0.05];
        let unit = direction.map(|c| c / norm(direction));
        let d_s = 6_000.0 * METRES_PER_LIGHT_YEAR;
        let source_at = observer
            .translated(GalacticDisplacement::new(unit.map(|c| c * d_s)))
            .unwrap();
        // Seen at `source_at` from the span's middle.
        let source = Drift::through(
            &source_at,
            middle.checked_sub(light_time(Metres::new(d_s))).unwrap(),
            GalacticVelocity::new([150e3, 40e3, -60e3]),
        );
        let reach = 2_000.0;
        let sightline = LensSightline::new(observer, brown_dwarf(galaxy, 69_999, 0.05).id());
        let wide = LensQuery::builder(sightline, span)
            .max_impact(reach)
            .substellar(SubstellarRequest::BrownDwarfs)
            .build()
            .unwrap();
        let g = geometry(&source, &wide);
        let (mut accepted, mut rejected) = (0, 0);
        for index in 0..3_000 {
            // A window of up to a year at a random time of the span, and a lens whose line
            // crosses near the sightline at a random distance near that window.
            let from = span
                .start()
                .checked_add(year(199.0 * lcg.next_f64()))
                .unwrap();
            let window =
                TimeWindow::new(from, from.checked_add(year(lcg.next_f64())).unwrap()).unwrap();
            let d_l = (0.05 + 0.9 * lcg.next_f64()) * d_s;
            let crossing = from.checked_add(year(-0.5 + 2.0 * lcg.next_f64())).unwrap();
            let v = [0; 3].map(|_| (2.0 * lcg.next_f64() - 1.0) * 577e3);
            let scale = if index % 3 == 0 { 0.3 } else { 0.03 };
            let side = [0; 3].map(|_| (2.0 * lcg.next_f64() - 1.0) * scale * METRES_PER_LIGHT_YEAR);
            let at = observer
                .translated(GalacticDisplacement::new(
                    [0, 1, 2].map(|a| unit[a] * d_l + side[a]),
                ))
                .unwrap();
            let emitted = crossing.checked_sub(light_time(Metres::new(d_l))).unwrap();
            let candidate = Candidate {
                record: brown_dwarf(galaxy, 70_000 + index, 0.05),
                line: Drift::through(&at, emitted, GalacticVelocity::new(v)),
            };
            let seen = retarded(&g.observer, &candidate.line);
            let offset = g
                .observer
                .position()
                .displacement_to(seen.apparent_position())
                .metres();
            let kept = passes(
                &g,
                &wide,
                Layer::BrownDwarf,
                offset,
                seen.velocity_then().metres_per_second(),
                0.0,
            );
            let query = LensQuery::builder(sightline, window)
                .max_impact(reach)
                .substellar(SubstellarRequest::BrownDwarfs)
                .build()
                .unwrap();
            let found = event_of(galaxy, &geometry(&source, &query), &query, &candidate, &[]);
            if found.is_some() {
                assert!(kept, "a lens of {window:?} was not kept: {candidate:?}");
                accepted += 1;
            }
            if !kept {
                rejected += 1;
            }
        }
        assert!(accepted >= 30, "{accepted} lenses found");
        assert!(rejected >= 300, "{rejected} candidates dropped");
    }

    /// A Milky Way galaxy whose systems move: its full potential and kinematic tables, built once
    /// for the slow tests.
    fn moving_galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x143_3100), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
                .with_full_potential()
        })
    }

    /// Every lens of `query` by brute force: every system of every walked layer in a box about
    /// the sightline wider than anything can move, each tested exactly.
    fn brute_force(galaxy: &Galaxy, query: &LensQuery, source: &Drift) -> (Vec<LensEvent>, u64) {
        let g = geometry(source, query);
        let d_s_ly = g.d_s_m / METRES_PER_LIGHT_YEAR;
        let from = g.observer.position().to_light_years_f64();
        let to = point_along(&g, d_s_ly).to_light_years_f64();
        let mut events = Vec::new();
        let mut examined = 0;
        let mut cell = Vec::new();
        for layer in query.layers().iter() {
            let beta = MetresPerSecond::from(pad_speed(layer)).value() / SPEED_OF_LIGHT;
            let pad =
                tube_radius(&g, query, layer_mass_bound(layer), beta, 0.0, d_s_ly).value() + 10.0;
            let size = f64::from(layer.cell_size_ly());
            #[expect(
                clippy::cast_possible_truncation,
                reason = "cell coordinates of the cube"
            )]
            let range = |a: usize| {
                let low = ((from[a].min(to[a]) - pad) / size).floor() as i32;
                let high = ((from[a].max(to[a]) + pad) / size).floor() as i32;
                low..=high
            };
            for x in range(0) {
                for y in range(1) {
                    for z in range(2) {
                        generate_cell(galaxy, CellKey::new(layer, [x, y, z]).unwrap(), &mut cell);
                        for record in &cell {
                            if record.id() == query.sightline().source() {
                                continue;
                            }
                            examined += 1;
                            let candidate = Candidate {
                                record: *record,
                                line: Drift::of_record(galaxy, record).unwrap(),
                            };
                            events.extend(event_of(galaxy, &g, query, &candidate, &[]));
                        }
                    }
                }
            }
        }
        events.sort_by(|a, b| {
            a.peak
                .cmp(&b.peak)
                .then_with(|| a.lens.raw().cmp(&b.lens.raw()))
        });
        (events, examined)
    }

    /// Ruling 143.3's check of the cone in a galaxy whose systems move: from the Sun, at +500
    /// years, towards a source 1,500 ly coreward and 60 ly above the plane, with a reach wide
    /// enough to find dozens of lenses, the cone with its escape-speed pad and prefilter finds
    /// exactly the lenses a brute-force walk of a box about the sightline finds, and so does a
    /// renewal of one-year windows from candidates walked for two centuries.
    #[test]
    #[ignore = "slow: builds the full potential and walks every system in a box around a 1,500 ly \
                sightline"]
    fn lensing_cone_equals_a_brute_force_walk_in_a_moving_galaxy() {
        let galaxy = moving_galaxy();
        let mut cell = Vec::new();
        generate_cell(
            galaxy,
            CellKey::new(Layer::C, [0, 765, 1]).unwrap(),
            &mut cell,
        );
        let source = cell
            .first()
            .expect("a layer-C cell of the disc holds systems");
        let source_line = Drift::of_record(galaxy, source).unwrap();
        assert!(
            source_line.velocity().speed().value() > 1e5,
            "the source moves"
        );
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let sightline = LensSightline::new(sun, source.id());
        let year = |y: i64| UniverseTime::from_julian_years(y).unwrap();
        let reach = 300_000.0;
        let build = |window| {
            LensQuery::builder(sightline, window)
                .max_impact(reach)
                .substellar(SubstellarRequest::BrownDwarfs)
                .build()
                .unwrap()
        };
        let query = build(TimeWindow::new(year(500), year(501)).unwrap());
        let walked = lenses_along(galaxy, &mut NoCache::new(), &[], &query).unwrap();
        let (expected, examined) = brute_force(galaxy, &query, &source_line);
        eprintln!(
            "{} lenses; the cone walked {} cells and examined {} systems, the box {examined}",
            expected.len(),
            walked.walked().cells(),
            walked.walked().systems_examined()
        );
        assert!(expected.len() >= 20, "{} lenses", expected.len());
        assert_eq!(walked.events(), &expected[..]);
        // The escape-speed pad walks fewer cells than plan 03's padding speed would.
        let g = geometry(&source_line, &query);
        let flat = EscapeEnvelope {
            beyond: vec![PAD_SPEED.value(); usize::try_from(ENVELOPE_RADII).unwrap()],
        };
        let padded = grid_cells(&g, &query, query.layers(), Some(&flat)).unwrap();
        let cells = u64::try_from(padded.len()).unwrap();
        eprintln!("at plan 03's padding speed the cone would walk {cells} cells");
        assert!(walked.walked().cells() < cells);
        // Two centuries of candidates, renewed a year at a time.
        let span = build(TimeWindow::new(year(400), year(600)).unwrap());
        let candidates = lens_candidates(galaxy, &mut NoCache::new(), &span).unwrap();
        eprintln!(
            "{} candidates over two centuries from {} systems examined",
            candidates.len(),
            candidates.walked().systems_examined()
        );
        for start in [400, 457, 500, 541, 599] {
            let window = build(TimeWindow::new(year(start), year(start + 1)).unwrap());
            let renewed = lenses_among(galaxy, &candidates, &[], &window).unwrap();
            let fresh = lenses_along(galaxy, &mut NoCache::new(), &[], &window).unwrap();
            assert_eq!(
                renewed.events(),
                fresh.events(),
                "the window from {start} yr"
            );
            if start == 457 {
                let (expected, _) = brute_force(galaxy, &window, &source_line);
                assert_eq!(renewed.events(), &expected[..]);
            }
        }
    }

    /// The escape-speed table bounds the draw's cut, the lesser of the escape speed and plan 03's
    /// padding speed, at 10⁵ random points of the cube and its centre (ruling 143.3's pad).
    #[test]
    #[ignore = "slow: builds the full potential"]
    fn lensing_escape_envelope_bounds_the_draws_cut() {
        let galaxy = moving_galaxy();
        let envelope = EscapeEnvelope::of(galaxy).expect("the galaxy moves");
        let potential = galaxy.potential();
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x143_3101);
        let mut closest = f64::INFINITY;
        for _ in 0..100_000 {
            // Log-uniform radii from 2⁻⁴ ly to the cube's corner, uniform directions.
            let r = ENVELOPE_FIRST_LY * math::exp2(lcg.next_f64() * 20.8);
            let z = r * (2.0 * lcg.next_f64() - 1.0);
            let r_cyl = (r * r - z * z).max(0.0).sqrt();
            let cut = potential
                .escape_speed(LightYears::new(r_cyl), LightYears::new(z))
                .map_or(PAD_SPEED.value(), KilometresPerSecond::value)
                .min(PAD_SPEED.value());
            let bound = envelope.bound(r).min(PAD_SPEED.value());
            assert!(
                cut <= bound,
                "{cut} km/s over the bound {bound} at r = {r} ly, z = {z} ly"
            );
            if cut < PAD_SPEED.value() {
                closest = closest.min(bound / cut - 1.0);
            }
        }
        eprintln!("the bound's least margin over the cut: {closest:e}");
        // At the Sun the bound is near the escape speed, not the padding speed.
        let sun = envelope.bound(26_000.0);
        assert!((500.0..700.0).contains(&sun), "{sun} km/s at the Sun");
        assert!(envelope.bound(0.01) >= PAD_SPEED.value());
    }
}
