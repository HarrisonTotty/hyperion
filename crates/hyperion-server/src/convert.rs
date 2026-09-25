//! Conversion between the wire's types and the server's, and the validation of every request
//! field on the way in (plan 04, P04.T14).
//!
//! A request's fields are checked here, once, by a `TryFrom` from its wire type; a field that
//! cannot be used becomes a [`ConvertRequestError`], answered `bad_request` with the field named.
//! Answers are built here too, by `From` from the server's types to the wire's, as are the request
//! errors that the server's own errors become. The `system_summary` request and its answer are in
//! [`stellar`], and plan 14's `system_bodies` and `body_detail` in [`planetary`].

mod planetary;
mod stellar;

use std::error::Error;
use std::fmt;
use std::num::NonZeroU32;
use std::sync::Arc;

use hyperion_protocol::{
    CreateUniverseRequest, DensityMap, DensityMapRequest, ErrorCode, GalaxyParameters, LayerCensus,
    LayerStatus, MassLayer, Parameter, ParameterGroup, ParameterOrigin, ParameterValue,
    RequestError, SeedHex, SystemIdHex, SystemsInRange, SystemsInRangeRequest, Unit, UniverseInfo,
    UniverseList,
};
use hyperion_sim::coords::{GalacticPosition, LyCell};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::{
    ArmParams, GalaxyParams, HaloComponentKind, HaloComponentParams, HaloParams,
};
use hyperion_sim::galaxy::placement::layer_spec;
use hyperion_sim::galaxy::potential::PotentialTables;
use hyperion_sim::galaxy::query::{
    BuildRangeQueryError, Census, CensusStop, MassFloor, RangeQuery, RangeResult, SystemHit,
};
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::time::{CLOCK_WINDOW_H, ClockWindow, UniverseTime};
use hyperion_sim::units::{
    Degrees, Gigayears, KilometresPerSecond, LightYears, Megayears, PerYear, Radians, SolarMasses,
    Years,
};
use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion};

pub(crate) use self::planetary::{
    BodiesRequest, DetailRequest, body_detail, body_refusal, hosts_request, system_bodies,
};
pub(crate) use self::stellar::{SummaryRequest, system_summary, unknown_system};
use crate::compute::{CodeDepth, GalaxyKey, MapKey, MapResolution, QuantisedMap, RawDensityMap};
use crate::limits::{MAX_CENSUS_LIMIT, MAX_QUERY_CELLS, MAX_QUERY_RADIUS_LY};
use crate::universe::{
    CreateUniverseError, OpenUniverseError, ParseUniverseNameError, Universe, UniverseName,
};

/// A request field that cannot be used, answered `bad_request` with the field named.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ConvertRequestError {
    field: &'static str,
    reason: String,
}

impl ConvertRequestError {
    /// The field `field`, as the wire names it, cannot be used because of `reason`.
    #[must_use]
    pub(crate) fn new(field: &'static str, reason: impl fmt::Display) -> Self {
        Self {
            field,
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for ConvertRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {}: {}", self.field, self.reason)
    }
}

impl Error for ConvertRequestError {}

impl From<ConvertRequestError> for RequestError {
    fn from(error: ConvertRequestError) -> Self {
        Self {
            code: ErrorCode::BadRequest,
            message: error.to_string(),
            field: Some(error.field.to_owned()),
        }
    }
}

/// A `create_universe` request, checked: a valid name and the seed, if one was given.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NewUniverse {
    name: UniverseName,
    seed: Option<u64>,
}

impl NewUniverse {
    /// The name, trimmed and checked, and the seed asked for, or `None` for one drawn by the
    /// server.
    #[must_use]
    pub(crate) fn into_parts(self) -> (UniverseName, Option<u64>) {
        (self.name, self.seed)
    }
}

impl TryFrom<CreateUniverseRequest> for NewUniverse {
    type Error = ConvertRequestError;

    /// Checks the name by the rules of [`UniverseName`]. The seed's form was checked as it was
    /// parsed.
    fn try_from(request: CreateUniverseRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: request.name.parse().map_err(invalid_name)?,
            seed: request.seed.as_ref().map(SeedHex::to_u64),
        })
    }
}

/// A name that broke a rule of [`UniverseName`], as the error of the `name` field.
#[must_use]
fn invalid_name(error: ParseUniverseNameError) -> ConvertRequestError {
    ConvertRequestError::new("name", error)
}

impl From<&Universe> for UniverseInfo {
    fn from(universe: &Universe) -> Self {
        Self {
            id: universe.id().into(),
            name: universe.name().as_str().to_owned(),
            seed: SeedHex::from_u64(universe.seed()),
            generator_version: universe.generator_version().get(),
            status: universe.status(),
        }
    }
}

/// The answer to `list_universes`: `universes`, in the order given, and the server's generator
/// version, against which each one's status was judged.
#[must_use]
pub(crate) fn universe_list(universes: &[Arc<Universe>]) -> UniverseList {
    UniverseList {
        universes: universes
            .iter()
            .map(|universe| UniverseInfo::from(universe.as_ref()))
            .collect(),
        server_generator_version: GENERATOR_VERSION.get(),
    }
}

/// A `density_map` request, checked: the map asked for and the depth its codes are wanted in.
///
/// The universe is looked up before this, as every galaxy handler does (P04.T14's intro), so what is
/// left to check is the resolution and the bit depth, each of which names its own field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MapRequest {
    view: hyperion_protocol::MapView,
    population: hyperion_protocol::MapPopulation,
    resolution: MapResolution,
    depth: CodeDepth,
}

impl MapRequest {
    /// The map to compute, over the galaxy `galaxy` names.
    #[must_use]
    pub(crate) fn key(self, galaxy: GalaxyKey) -> MapKey {
        MapKey::new(galaxy, self.view, self.population, self.resolution)
    }

    /// The depth the codes are quantised to.
    #[must_use]
    pub(crate) fn depth(self) -> CodeDepth {
        self.depth
    }
}

impl TryFrom<&DensityMapRequest> for MapRequest {
    type Error = ConvertRequestError;

    /// Checks `resolution` against the four widths M1 serves and `bits` against 8 and 16.
    fn try_from(request: &DensityMapRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            view: request.view,
            population: request.population,
            resolution: MapResolution::try_from(request.resolution)
                .map_err(|error| ConvertRequestError::new("resolution", error))?,
            depth: CodeDepth::try_from(request.bits)
                .map_err(|error| ConvertRequestError::new("bits", error))?,
        })
    }
}

/// The answer to `density_map`: the raster's geometry, the codes' range, and the codes themselves.
///
/// The geometry is the raw map's, so the client reads the pixel-to-light-year rule off the response
/// alone (the `DensityMap` doc comment), and the floor and ceiling are the quantiser's for the depth
/// the request asked for. It takes the request by value because it moves the universe's ID into the
/// answer, and because the base64 of up to 2.8 MiB that
/// [`QuantisedMap::to_base64`](crate::compute::QuantisedMap::to_base64) makes means this belongs in
/// a pool job, which owns what it is given.
#[must_use]
pub(crate) fn density_map(
    request: DensityMapRequest,
    raw: &RawDensityMap,
    quantised: &QuantisedMap,
) -> DensityMap {
    DensityMap {
        universe: request.universe,
        view: request.view,
        population: request.population,
        width_px: raw.width_px(),
        height_px: raw.height_px(),
        centre_ly: raw.centre_ly(),
        ly_per_px: raw.ly_per_px(),
        bits: quantised.depth().bits(),
        floor_log10_per_ly2: quantised.floor_log10_per_ly2(),
        ceiling_log10_per_ly2: quantised.ceiling_log10_per_ly2(),
        data_base64: quantised.to_base64(),
    }
}

/// A `systems_in_range` request, checked: the query the sim is to run.
///
/// Every field is checked here, in design note 24's order, so that the answer names the first field
/// at fault: the universe (by `requests::universe::openable_universe`, before this), then `time`,
/// `centre`, `radius_ly` and `limit`. The query carries the server's cell budget,
/// [`MAX_QUERY_CELLS`], rather than plan 03's own default, which is sized for a caller with no
/// clients to protect.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RangeRequest(RangeQuery);

impl RangeRequest {
    /// The query, to run on the CPU pool.
    #[must_use]
    pub(crate) fn into_query(self) -> RangeQuery {
        self.0
    }
}

impl TryFrom<&SystemsInRangeRequest> for RangeRequest {
    type Error = ConvertRequestError;

    /// Checks `time`, `centre`, `radius_ly` and `limit`, in that order, and builds the query.
    fn try_from(request: &SystemsInRangeRequest) -> Result<Self, Self::Error> {
        let time = query_time(&request.time)?;
        let centre = query_centre(&request.centre)?;
        let radius = query_radius(request.radius_ly)?;
        let limit = query_limit(request.limit)?;
        let query = RangeQuery::builder(centre, radius)
            .time(time)
            .limit(limit)
            .mass_floor(mass_floor(request.min_layer))
            .cell_budget(MAX_QUERY_CELLS)
            .build()
            .map_err(refused_query)?;
        Ok(Self(query))
    }
}

/// The instant to query at: a well-formed time inside the clock window.
fn query_time(time: &hyperion_protocol::UniverseTime) -> Result<UniverseTime, ConvertRequestError> {
    let t = UniverseTime::new(time.seconds, time.nanos)
        .map_err(|error| ConvertRequestError::new("time", error))?;
    if !ClockWindow::contains(t) {
        return Err(ConvertRequestError::new(
            "time",
            format!(
                "{t} lies outside the {} Julian years either side of the epoch where positions are \
                 guaranteed",
                CLOCK_WINDOW_H.as_julian_years_f64()
            ),
        ));
    }
    Ok(t)
}

/// The sphere's centre: a canonical position inside the root cube.
fn query_centre(
    centre: &hyperion_protocol::GalacticPosition,
) -> Result<GalacticPosition, ConvertRequestError> {
    let position = GalacticPosition::new(LyCell::new(centre.cell_ly), centre.offset_m)
        .map_err(|error| ConvertRequestError::new("centre", error))?;
    if !position.in_root_cube() {
        return Err(ConvertRequestError::new(
            "centre",
            "the centre lies outside the galaxy's root cube",
        ));
    }
    Ok(position)
}

/// The sphere's radius: finite, above zero and at most [`MAX_QUERY_RADIUS_LY`].
fn query_radius(radius_ly: f64) -> Result<LightYears, ConvertRequestError> {
    let refused = |reason: &str| Err(ConvertRequestError::new("radius_ly", reason));
    if !radius_ly.is_finite() {
        return refused("the radius is not a finite number of light-years");
    }
    if radius_ly <= 0.0 {
        return refused("the radius must be above zero");
    }
    if radius_ly > MAX_QUERY_RADIUS_LY {
        return Err(ConvertRequestError::new(
            "radius_ly",
            format!("the radius must be at most {MAX_QUERY_RADIUS_LY} light-years"),
        ));
    }
    Ok(LightYears::new(radius_ly))
}

/// The census limit: from 1 to [`MAX_CENSUS_LIMIT`] expected systems.
fn query_limit(limit: u32) -> Result<NonZeroU32, ConvertRequestError> {
    let limit = NonZeroU32::new(limit).ok_or_else(|| {
        ConvertRequestError::new("limit", "the limit must be at least one system")
    })?;
    if limit.get() > MAX_CENSUS_LIMIT {
        return Err(ConvertRequestError::new(
            "limit",
            format!("the limit must be at most {MAX_CENSUS_LIMIT} systems"),
        ));
    }
    Ok(limit)
}

/// The sim's mass floor for the lightest layer the client asked for.
#[must_use]
fn mass_floor(layer: MassLayer) -> MassFloor {
    match layer {
        MassLayer::A => MassFloor::LayerA,
        MassLayer::B => MassFloor::LayerB,
        MassLayer::C => MassFloor::LayerC,
        MassLayer::D => MassFloor::LayerD,
        MassLayer::E => MassFloor::LayerE,
    }
}

/// A query the sim refused, as the error of the field it belongs to.
///
/// The checks above catch each of these first, with a message of the server's own, so this is the
/// belt to their braces: plan 03's `build` enforces the same rules (its design note 12), and if it
/// ever refuses something they let through, the client still learns which field to mend.
#[must_use]
fn refused_query(error: BuildRangeQueryError) -> ConvertRequestError {
    let field = match error {
        BuildRangeQueryError::RadiusNotFinite
        | BuildRangeQueryError::RadiusNotPositive
        | BuildRangeQueryError::RadiusBeyondRootCube => "radius_ly",
        BuildRangeQueryError::CentreOutsideRootCube => "centre",
        BuildRangeQueryError::TimeOutsideClockWindow(_) => "time",
        BuildRangeQueryError::SubstellarLayersUnavailable => "min_layer",
    };
    ConvertRequestError::new(field, error)
}

/// The answer to `systems_in_range`: the request's own terms, the census, and the systems found.
///
/// `centre`, `radius_ly` and `time` are echoed from the request, which is why it is taken by value:
/// the job that ran the query owns it and moves them into the answer. The records are in
/// [`RangeResult`]'s order, nearest first, and each is the system at the query's time.
#[must_use]
pub(crate) fn systems_in_range(
    request: SystemsInRangeRequest,
    query: &RangeQuery,
    result: &RangeResult,
) -> SystemsInRange {
    let systems = result
        .systems()
        .iter()
        .map(|hit| system_record(hit, query.time()))
        .collect();
    SystemsInRange {
        universe: request.universe,
        centre: request.centre,
        radius_ly: request.radius_ly,
        time: request.time,
        census: census(query, result),
        systems,
    }
}

/// One system found, as the wire carries it: the state at the epoch but for the position and the
/// age, which are at the query's time.
///
/// The row never carries a stellar brief yet, whatever the request's `include_stellar` says, and
/// every row is plan 04's. A brief builds the primary's track, 1–2 ms for an evolved star (ruling
/// 46 of 2026-09-22), so a brief on every row of a 20,000-system answer would cost seconds; the
/// briefs are the part of plan 06's P06.T34 that waits for the track integrator's optimisation,
/// which ruling 46 puts first. The client reads an absent `stellar` as no brief sent.
#[must_use]
fn system_record(hit: &SystemHit, time: UniverseTime) -> hyperion_protocol::SystemRecord {
    let record = hit.record();
    hyperion_protocol::SystemRecord {
        id: SystemIdHex::from_u64(record.id().raw()),
        designation: record.id().designation().to_string(),
        position: galactic_position(hit.position()),
        layer: mass_layer(record.layer()),
        initial_mass_msun: record.primary_initial_mass().value(),
        age_myr: Megayears::from(record.age_at(time)).value(),
        population: wire_population(record.population()),
        stellar: None,
    }
}

/// The census as the wire carries it: all five layers, A to E, each with its band, its expected
/// count, what it returned and why it is in or out (design note 13).
#[must_use]
fn census(query: &RangeQuery, result: &RangeResult) -> hyperion_protocol::Census {
    let census = result.census();
    // Keyed by `Layer::value`, so a layer plan 13 adds counts into an entry of its own rather than
    // past the end.
    let mut returned = [0_u32; Layer::ALL.len()];
    for hit in result.systems() {
        let count = &mut returned[usize::from(hit.record().layer().value())];
        *count = count.saturating_add(1);
    }
    let layers = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E]
        .into_iter()
        .map(|layer| {
            let band = layer_spec(layer)
                .expect("every stellar layer has a row in plan 03's layer table")
                .band();
            LayerCensus {
                layer: mass_layer(layer),
                mass_min_msun: band.lo(),
                mass_max_msun: band.hi(),
                expected: census.expected().get(layer),
                returned: returned[usize::from(layer.value())],
                status: layer_status(layer, census, query.mass_floor()),
            }
        })
        .collect();
    hyperion_protocol::Census {
        limit: query.limit().get(),
        complete_above_msun: census.complete_above().map(SolarMasses::value),
        layers,
    }
}

/// Whether a layer is in the result, and why not otherwise.
///
/// A layer the census admits is `included`; one lighter than the floor the client asked for is
/// `below_mass_floor`; anything else was left out by the rule that stopped the walk, and the census
/// is all-or-nothing per layer, so its `returned` is zero.
#[must_use]
fn layer_status(layer: Layer, census: &Census, floor: MassFloor) -> LayerStatus {
    if census.layers().contains(layer) {
        LayerStatus::Included
    } else if below_mass_floor(layer, floor) {
        LayerStatus::BelowMassFloor
    } else {
        match census.stopped_by() {
            CensusStop::Limit => LayerStatus::OverLimit,
            CensusStop::CellBudget => LayerStatus::OverCellBudget,
            // Every floor admits layer E, so a walk that stopped at the floor left out only the
            // layers below it, which the arm above answers.
            CensusStop::MassFloor => LayerStatus::BelowMassFloor,
        }
    }
}

/// Whether `layer` is lighter than the finest layer the query asked for.
///
/// Compared as mass floors rather than as [`Layer`]s: `Layer`'s own order puts the substellar layers
/// above E although they are lighter, while [`MassFloor`] runs from the coarsest to the finest and
/// plan 13 extends it at the finer end, so this answer holds when those floors arrive.
#[must_use]
fn below_mass_floor(layer: Layer, floor: MassFloor) -> bool {
    let admits = match layer {
        Layer::A => MassFloor::LayerA,
        Layer::B => MassFloor::LayerB,
        Layer::C => MassFloor::LayerC,
        Layer::D => MassFloor::LayerD,
        Layer::E => MassFloor::LayerE,
        // No floor names a substellar layer until plan 13, and the census lists none of them.
        Layer::BrownDwarf | Layer::RoguePlanet => return false,
    };
    admits > floor
}

/// A position as the wire carries it: the light-year cell and the metre offset, exactly as the sim
/// holds them (design note 10).
#[must_use]
fn galactic_position(position: &GalacticPosition) -> hyperion_protocol::GalacticPosition {
    hyperion_protocol::GalacticPosition {
        cell_ly: position.cell().to_array(),
        offset_m: position.offset_metres(),
    }
}

/// The wire's layer for a stellar layer.
///
/// # Panics
///
/// For the brown-dwarf and rogue-planet layers, which the first milestone never places:
/// `RangeQueryBuilder::build` refuses a substellar request, and the wire has no value for them
/// until plan 13 sends one.
#[must_use]
fn mass_layer(layer: Layer) -> MassLayer {
    match layer {
        Layer::A => MassLayer::A,
        Layer::B => MassLayer::B,
        Layer::C => MassLayer::C,
        Layer::D => MassLayer::D,
        Layer::E => MassLayer::E,
        Layer::BrownDwarf | Layer::RoguePlanet => {
            panic!("the substellar layer {layer:?} has no wire form until plan 13 places it")
        }
    }
}

/// The wire's population.
#[must_use]
fn wire_population(population: Population) -> hyperion_protocol::Population {
    match population {
        Population::YoungThinDisc => hyperion_protocol::Population::YoungThinDisc,
        Population::OldThinDisc => hyperion_protocol::Population::OldThinDisc,
        Population::ThickDisc => hyperion_protocol::Population::ThickDisc,
        Population::Bulge => hyperion_protocol::Population::Bulge,
        Population::LongBar => hyperion_protocol::Population::LongBar,
        Population::NuclearDisc => hyperion_protocol::Population::NuclearDisc,
        Population::Halo => hyperion_protocol::Population::Halo,
    }
}

/// The radius at which the `rotation` group reports the galaxy's rotation: 26,000 ly.
///
/// 7.97 kpc, which is the round 8 kpc at which the brainstorm quotes its rotation curves ("210–270
/// km/s at 8 kpc"); the Sun's own galactocentric radius is 8.2 ± 0.1 kpc (Bland-Hawthorn and Gerhard
/// 2016). A fictional galaxy has no Sun, so the radius is fixed by plan 04 rather than drawn, and is
/// sent so that the client need not know it.
const ROTATION_RADIUS: LightYears = LightYears::new(26_000.0);

/// The galaxy parameters M1 does not send, each with the reason (plan 04, P04.T14.b).
///
/// Every quantity `GalaxyParams` and the potential tables expose is accounted for: it appears once
/// in [`galaxy_parameters`], or here. Whoever adds a getter to plan 02's parameters sends it or adds
/// it to this list, and raises [`PARAMETERS_ACCOUNTED_FOR`]; a test holds the two to each other.
///
/// Compiled for the tests alone, because the test is what enforces it: nothing on the wire depends
/// on the list.
#[cfg(test)]
const EXCLUDED_PARAMETERS: &[(&str, &str)] = &[
    (
        "metallicity_gradient",
        "dex per kiloparsec is not a wire unit, and no unit is added until a plan displays a \
         metallicity",
    ),
    (
        "halo.<component>.feh_mean",
        "a metallicity, as above: it waits for the plan that displays one",
    ),
    (
        "black_hole.scatter",
        "a scatter draw: it is already folded into the black hole's mass, which is sent",
    ),
    (
        "black_hole.bulge_dispersion",
        "an intermediate of the M–sigma relation rather than a structural parameter; the mass it \
         gives is sent",
    ),
    (
        "accretion.progenitors",
        "the per-progenitor list is plan 10's to display; the lesser progenitors' combined halo \
         share is sent",
    ),
    (
        "halo.<component>.ages",
        "the components' age distributions; the `history` group stands for them in M1",
    ),
    (
        "halo.discrete_share",
        "held at zero until plan 10, which places the streams and dwarf cores it counts",
    ),
    (
        "dark_halo.scale_radius",
        "r200 divided by c200, both of which are sent",
    ),
    (
        "gas.scale_height",
        "a constant of the generator, the same for every seed",
    ),
    (
        "nuclear_cluster.inner_slope",
        "a constant of the generator, as above",
    ),
    (
        "nuclear_cluster.break_radius",
        "a constant of the generator, as above",
    ),
    (
        "nuclear_cluster.outer_slope",
        "a constant of the generator, as above",
    ),
    (
        "halo.<component>.cut_radius",
        "a constant of the generator, as above",
    ),
    (
        "halo.globular_debris.flattening",
        "a constant of the generator: the debris is spherical",
    ),
    (
        "potential.radial_profiles",
        "the potential's tables are profiles of radius, not parameters; only the `rotation` \
         group's radius and the two speeds there are sent",
    ),
];

/// Galaxy parameters accounted for: those [`galaxy_parameters`] sends, plus the
/// [`EXCLUDED_PARAMETERS`].
///
/// Raised by whoever adds a parameter, which is what makes them decide whether it is sent.
#[cfg(test)]
const PARAMETERS_ACCOUNTED_FOR: usize = 95;

/// The galaxy's parameters as the wire carries them: grouped, keyed, and in display units.
///
/// The groups, their keys, their units and their order are plan 04's P04.T14.b table, which the
/// client's glossary copies; a key is never renamed once shipped. Units are those of design note 11,
/// the ones the client can format without converting: the pattern speed in degrees per megayear,
/// times in gigayears, lengths in light-years, and nothing per kiloparsec. What is not sent is
/// listed in [`EXCLUDED_PARAMETERS`].
///
/// # Panics
///
/// If the galaxy's halo has no in-situ, dominant-merger or globular-debris component, or its
/// dominant merger has no break. Plan 02 draws all four for every seed, so their absence is a bug.
#[must_use]
pub(crate) fn galaxy_parameters(universe: &Universe, galaxy: &Galaxy) -> GalaxyParameters {
    let params = galaxy.params();
    let seed = SeedHex::from_u64(universe.seed());
    let groups = vec![
        group(
            "identity",
            identity(&seed, universe.generator_version(), params),
        ),
        group("mass", mass(params)),
        group("populations", population_shares(params)),
        group("population_masses", population_masses(params)),
        group("population_mean_masses", population_mean_masses(params)),
        group("discs", discs(params)),
        group("bulge_and_bar", bulge_and_bar(params, galaxy.potential())),
        group("nuclear_disc", nuclear_disc(params)),
        group("halo", halo(params.halo())),
        group("arms", arms(params.arms())),
        group("history", history(params)),
        group("rotation", rotation(galaxy.potential())),
    ];
    GalaxyParameters {
        universe: universe.id().into(),
        seed,
        generator_version: universe.generator_version().get(),
        groups,
    }
}

#[must_use]
fn identity(seed: &SeedHex, version: GeneratorVersion, params: &GalaxyParams) -> Vec<Parameter> {
    let mass_function = match params.mass_function() {
        MassFunctionKind::Kroupa => "kroupa",
        MassFunctionKind::Chabrier => "chabrier",
    };
    vec![
        text("seed", seed.as_str()),
        fixed("generator_version", Unit::Count, f64::from(version.get())),
        text("mass_function", mass_function),
    ]
}

#[must_use]
fn mass(params: &GalaxyParams) -> Vec<Parameter> {
    let dark = params.dark_halo();
    vec![
        drawn("stellar_mass", Unit::Msun, msun(params.stellar_mass())),
        derived("system_count", Unit::Count, params.system_count()),
        derived(
            "mean_system_mass",
            Unit::Msun,
            msun(params.stellar_mass()) / params.system_count(),
        ),
        derived(
            "mean_formed_mass",
            Unit::Msun,
            msun(params.mean_formed_mass()),
        ),
        derived("gas.mass", Unit::Msun, msun(params.gas_disc().mass())),
        derived(
            "black_hole.mass",
            Unit::Msun,
            msun(params.black_hole().mass()),
        ),
        derived(
            "nuclear_cluster.mass",
            Unit::Msun,
            msun(params.nuclear_cluster().mass()),
        ),
        derived("dark_halo.mass", Unit::Msun, msun(dark.m200())),
        derived("dark_halo.concentration", Unit::None, dark.concentration()),
        derived("dark_halo.virial_radius", Unit::Ly, ly(dark.r200())),
        drawn("dark_halo.f_star", Unit::None, dark.f_star()),
    ]
}

#[must_use]
fn population_shares(params: &GalaxyParams) -> Vec<Parameter> {
    POPULATIONS
        .iter()
        .map(|&population| {
            number(
                &population_key(population, "share"),
                share_origin(population),
                Unit::None,
                params.population_share(population),
            )
        })
        .collect()
}

#[must_use]
fn population_masses(params: &GalaxyParams) -> Vec<Parameter> {
    POPULATIONS
        .iter()
        .map(|&population| {
            derived(
                &population_key(population, "mass"),
                Unit::Msun,
                msun(params.population_mass(population)),
            )
        })
        .collect()
}

#[must_use]
fn population_mean_masses(params: &GalaxyParams) -> Vec<Parameter> {
    POPULATIONS
        .iter()
        .map(|&population| {
            derived(
                &population_key(population, "mean_system_mass"),
                Unit::Msun,
                msun(params.mean_system_mass(population)),
            )
        })
        .collect()
}

#[must_use]
fn discs(params: &GalaxyParams) -> Vec<Parameter> {
    let (thin, young, thick) = (params.thin_disc(), params.young_disc(), params.thick_disc());
    vec![
        derived("disc.thin.scale_length", Unit::Ly, ly(thin.length())),
        drawn("disc.thin.scale_height", Unit::Ly, ly(thin.height())),
        derived("disc.young.scale_length", Unit::Ly, ly(young.length())),
        drawn("disc.young.scale_height", Unit::Ly, ly(young.height())),
        derived("disc.thick.scale_length", Unit::Ly, ly(thick.length())),
        derived("disc.thick.scale_height", Unit::Ly, ly(thick.height())),
        derived(
            "disc.gas.scale_length",
            Unit::Ly,
            ly(params.gas_disc().length()),
        ),
    ]
}

#[must_use]
fn bulge_and_bar(params: &GalaxyParams, potential: &PotentialTables) -> Vec<Parameter> {
    let (bulge, bar) = (params.bulge(), params.bar());
    vec![
        derived("bulge.scale_x", Unit::Ly, ly(bulge.scale_x())),
        derived("bulge.scale_y", Unit::Ly, ly(bulge.scale_y())),
        derived("bulge.scale_z", Unit::Ly, ly(bulge.scale_z())),
        drawn("bulge.boxiness", Unit::None, bulge.boxiness()),
        drawn(
            "bar.share_of_bulge",
            Unit::None,
            params.bar_share_of_bulge(),
        ),
        derived("bar.half_length", Unit::Ly, ly(bar.half_length())),
        derived("bar.width", Unit::Ly, ly(bar.width())),
        drawn("bar.height", Unit::Ly, ly(bar.height())),
        drawn("bar.corotation_ratio", Unit::None, bar.corotation_ratio()),
        derived(
            "bar.corotation_radius",
            Unit::Ly,
            ly(potential.bar_corotation()),
        ),
        derived(
            "bar.pattern_speed",
            Unit::DegPerMyr,
            deg_per_myr(potential.bar_pattern_speed()),
        ),
    ]
}

#[must_use]
fn nuclear_disc(params: &GalaxyParams) -> Vec<Parameter> {
    let nuclear = params.nuclear_disc();
    vec![
        derived("nuclear_disc.scale_length", Unit::Ly, ly(nuclear.length())),
        derived("nuclear_disc.scale_height", Unit::Ly, ly(nuclear.height())),
    ]
}

#[must_use]
fn halo(halo: &HaloParams) -> Vec<Parameter> {
    let in_situ = component(halo, HaloComponentKind::InSitu);
    let dominant = component(halo, HaloComponentKind::DominantMerger);
    let debris = component(halo, HaloComponentKind::GlobularDebris);
    // In the components' fixed order, so that the sum is the same for every caller.
    let lesser: f64 = halo
        .components()
        .iter()
        .filter(|c| matches!(c.kind(), HaloComponentKind::Lesser(_)))
        .map(HaloComponentParams::share)
        .sum();
    let outer_break = dominant
        .outer_break()
        .expect("plan 02 gives the dominant merger's halo component a break");
    vec![
        derived("halo.in_situ.share", Unit::None, in_situ.share()),
        derived("halo.dominant_merger.share", Unit::None, dominant.share()),
        derived("halo.lesser.share", Unit::None, lesser),
        derived("halo.globular_debris.share", Unit::None, debris.share()),
        drawn("halo.in_situ.slope", Unit::None, in_situ.slope()),
        drawn("halo.dominant_merger.slope", Unit::None, dominant.slope()),
        drawn("halo.globular_debris.slope", Unit::None, debris.slope()),
        drawn("halo.in_situ.core", Unit::Ly, ly(in_situ.core())),
        drawn("halo.dominant_merger.core", Unit::Ly, ly(dominant.core())),
        drawn("halo.globular_debris.core", Unit::Ly, ly(debris.core())),
        drawn("halo.in_situ.flattening", Unit::None, in_situ.flattening()),
        drawn(
            "halo.dominant_merger.flattening",
            Unit::None,
            dominant.flattening(),
        ),
        drawn(
            "halo.dominant_merger.break_radius",
            Unit::Ly,
            ly(outer_break.radius()),
        ),
        drawn(
            "halo.dominant_merger.break_steepening",
            Unit::None,
            outer_break.steepening(),
        ),
    ]
}

#[must_use]
fn arms(arms: &ArmParams) -> Vec<Parameter> {
    vec![
        drawn("arms.count", Unit::Count, f64::from(arms.count().get())),
        drawn("arms.pitch", Unit::Deg, deg(arms.pitch())),
        drawn("arms.young_width", Unit::Ly, ly(arms.young_width())),
        drawn("arms.young_fraction", Unit::None, arms.young_fraction()),
        drawn("arms.old_amplitude", Unit::None, arms.old_amplitude()),
    ]
}

#[must_use]
fn history(params: &GalaxyParams) -> Vec<Parameter> {
    let accretion = params.accretion();
    vec![
        drawn(
            "history.formation_timescale",
            Unit::Gyr,
            gyr(params.sfh_timescale()),
        ),
        drawn(
            "history.last_major_merger",
            Unit::Gyr,
            gyr(accretion.last_major_merger()),
        ),
        derived(
            "history.globular_clusters",
            Unit::Count,
            f64::from(accretion.globular_count()),
        ),
    ]
}

#[must_use]
fn rotation(potential: &PotentialTables) -> Vec<Parameter> {
    vec![
        fixed("rotation.radius", Unit::Ly, ly(ROTATION_RADIUS)),
        derived(
            "rotation.circular_speed",
            Unit::KmPerS,
            km_per_s(potential.v_circ(ROTATION_RADIUS)),
        ),
        derived(
            "rotation.escape_speed",
            Unit::KmPerS,
            km_per_s(potential.escape_speed_in_plane(ROTATION_RADIUS)),
        ),
    ]
}

/// The component of `kind`, which plan 02 draws for every galaxy.
#[must_use]
fn component(halo: &HaloParams, kind: HaloComponentKind) -> &HaloComponentParams {
    halo.components()
        .iter()
        .find(|component| component.kind() == kind)
        .unwrap_or_else(|| panic!("plan 02 gives every halo a {kind:?} component"))
}

/// `population.<population>.<leaf>`, the key of a per-population parameter.
#[must_use]
fn population_key(population: Population, leaf: &str) -> String {
    format!("population.{}.{leaf}", population.name())
}

/// Whether a population's share is drawn or follows from the others (plan 02, design note 3).
#[must_use]
fn share_origin(population: Population) -> ParameterOrigin {
    match population {
        Population::ThickDisc | Population::NuclearDisc | Population::Halo => {
            ParameterOrigin::Drawn
        }
        Population::YoungThinDisc
        | Population::OldThinDisc
        | Population::Bulge
        | Population::LongBar => ParameterOrigin::Derived,
    }
}

#[must_use]
fn group(key: &str, parameters: Vec<Parameter>) -> ParameterGroup {
    ParameterGroup {
        key: key.to_owned(),
        parameters,
    }
}

#[must_use]
fn number(key: &str, origin: ParameterOrigin, unit: Unit, value: f64) -> Parameter {
    Parameter {
        key: key.to_owned(),
        origin,
        value: ParameterValue::Number { value, unit },
    }
}

/// A parameter drawn from the seed.
#[must_use]
fn drawn(key: &str, unit: Unit, value: f64) -> Parameter {
    number(key, ParameterOrigin::Drawn, unit, value)
}

/// A parameter computed from drawn values.
#[must_use]
fn derived(key: &str, unit: Unit, value: f64) -> Parameter {
    number(key, ParameterOrigin::Derived, unit, value)
}

/// A parameter the generator version fixes.
#[must_use]
fn fixed(key: &str, unit: Unit, value: f64) -> Parameter {
    number(key, ParameterOrigin::Fixed, unit, value)
}

/// A parameter whose value is a name, which the generator version fixes.
#[must_use]
fn text(key: &str, value: &str) -> Parameter {
    Parameter {
        key: key.to_owned(),
        origin: ParameterOrigin::Fixed,
        value: ParameterValue::Text {
            value: value.to_owned(),
        },
    }
}

/// A length in light-years, the wire's unit for every length.
#[must_use]
fn ly(length: LightYears) -> f64 {
    length.value()
}

/// A mass in solar masses, the wire's unit for every mass.
#[must_use]
fn msun(mass: SolarMasses) -> f64 {
    mass.value()
}

/// A speed in kilometres per second.
#[must_use]
fn km_per_s(speed: KilometresPerSecond) -> f64 {
    speed.value()
}

/// An angle in degrees, from the sim's radians.
#[must_use]
fn deg(angle: Radians) -> f64 {
    Degrees::from(angle).value()
}

/// A time in gigayears, from the sim's Julian years.
#[must_use]
fn gyr(time: Years) -> f64 {
    Gigayears::from(time).value()
}

/// An angular speed in degrees per megayear, from the sim's radians per Julian year.
///
/// Design note 11: the bar's pattern speed is the one angular speed on the wire, and the client
/// formats `deg_per_myr` without converting. 38 km/s per kpc is 2.23 °/Myr.
#[must_use]
fn deg_per_myr(speed: PerYear) -> f64 {
    Degrees::from(Radians::new(speed.value())).value() * 1e6
}

impl From<CreateUniverseError> for RequestError {
    /// A taken name names the `name` field. Failures to draw, to find a free ID or to finish are
    /// the server's own (`internal`); a failed write is `storage_failed`.
    fn from(error: CreateUniverseError) -> Self {
        let message = error.to_string();
        let (code, field) = match error {
            CreateUniverseError::NameTaken { .. } => (ErrorCode::NameTaken, Some("name")),
            CreateUniverseError::LimitReached { .. } => (ErrorCode::UniverseLimitReached, None),
            CreateUniverseError::Storage(_) => (ErrorCode::StorageFailed, None),
            CreateUniverseError::Entropy(_)
            | CreateUniverseError::NoFreeId
            | CreateUniverseError::Interrupted => (ErrorCode::Internal, None),
        };
        Self {
            code,
            message,
            field: field.map(str::to_owned),
        }
    }
}

impl From<OpenUniverseError> for RequestError {
    /// An unknown ID names the `universe` field. A universe that exists but cannot be run is not
    /// the field's fault, and its message gives both versions or both formats.
    fn from(error: OpenUniverseError) -> Self {
        let (code, field) = match error {
            OpenUniverseError::UnknownUniverse { .. } => {
                (ErrorCode::UnknownUniverse, Some("universe"))
            }
            OpenUniverseError::GeneratorVersionMismatch { .. } => {
                (ErrorCode::GeneratorVersionMismatch, None)
            }
            OpenUniverseError::UnsupportedSaveFormat { .. } => {
                (ErrorCode::UnsupportedSaveFormat, None)
            }
        };
        Self {
            code,
            message: error.to_string(),
            field: field.map(str::to_owned),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use hyperion_protocol::UniverseStatus;
    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::placement::NoCache;
    use hyperion_sim::galaxy::query::range_query;
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::limits::MAX_UNIVERSE_NAME_CHARS;
    use crate::universe::{DrawEntropyError, SavedUniverse, UniverseId, WriteSaveError};

    fn request(name: &str, seed: Option<u64>) -> CreateUniverseRequest {
        CreateUniverseRequest {
            name: name.to_owned(),
            seed: seed.map(SeedHex::from_u64),
        }
    }

    #[test]
    fn a_create_request_is_trimmed_and_keeps_its_seed() {
        let (name, seed) = NewUniverse::try_from(request("  Kepler Reach ", Some(0x4d2)))
            .unwrap()
            .into_parts();
        assert_eq!(name.as_str(), "Kepler Reach");
        assert_eq!(seed, Some(0x4d2));
        let (_, seed) = NewUniverse::try_from(request("Talos", None))
            .unwrap()
            .into_parts();
        assert_eq!(seed, None);
    }

    #[test]
    fn a_bad_name_is_a_bad_request_naming_the_field() {
        let too_long = "a".repeat(MAX_UNIVERSE_NAME_CHARS + 1);
        for name in ["", "   ", &too_long, "Kepler\u{7}Reach"] {
            let error = RequestError::from(NewUniverse::try_from(request(name, None)).unwrap_err());
            assert_eq!(error.code, ErrorCode::BadRequest, "{name:?}");
            assert_eq!(error.field.as_deref(), Some("name"), "{name:?}");
            assert!(error.message.starts_with("invalid name: "), "{error:?}");
        }
    }

    #[test]
    fn universe_info_carries_the_identity_and_the_status() {
        let saved = crate::universe::SavedUniverse::new(
            UniverseId::new(0x0123_4567_89ab_cdef),
            "Talos".parse().unwrap(),
            0x4d2,
            GENERATOR_VERSION,
        );
        let info = UniverseInfo::from(&Universe::from(saved));
        assert_eq!(
            info,
            UniverseInfo {
                id: hyperion_protocol::UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
                name: "Talos".to_owned(),
                seed: SeedHex::from_u64(0x4d2),
                generator_version: GENERATOR_VERSION.get(),
                status: UniverseStatus::Compatible,
            }
        );
        let list = universe_list(&[Arc::new(Universe::from(
            crate::universe::SavedUniverse::new(
                UniverseId::new(7),
                "Vega".parse().unwrap(),
                1,
                GeneratorVersion::new(GENERATOR_VERSION.get() + 1),
            ),
        ))]);
        assert_eq!(list.server_generator_version, GENERATOR_VERSION.get());
        assert_eq!(list.universes[0].status, UniverseStatus::GeneratorMismatch);
    }

    #[test]
    fn create_errors_become_their_codes() {
        let io_error = || WriteSaveError::Io {
            operation: "write",
            path: PathBuf::from("universe.json"),
            source: io::Error::other("disk full"),
        };
        for (error, code, field) in [
            (
                CreateUniverseError::NameTaken {
                    name: "Talos".to_owned(),
                },
                ErrorCode::NameTaken,
                Some("name"),
            ),
            (
                CreateUniverseError::LimitReached { limit: 256 },
                ErrorCode::UniverseLimitReached,
                None,
            ),
            (
                CreateUniverseError::Storage(io_error()),
                ErrorCode::StorageFailed,
                None,
            ),
            (
                CreateUniverseError::Entropy(DrawEntropyError::Exhausted),
                ErrorCode::Internal,
                None,
            ),
            (CreateUniverseError::NoFreeId, ErrorCode::Internal, None),
            (CreateUniverseError::Interrupted, ErrorCode::Internal, None),
        ] {
            let message = error.to_string();
            let converted = RequestError::from(error);
            assert_eq!(converted.code, code, "{message}");
            assert_eq!(converted.field.as_deref(), field, "{message}");
            assert_eq!(converted.message, message);
        }
    }

    #[test]
    fn open_errors_become_their_codes_and_a_mismatch_names_both_versions() {
        let id = UniverseId::new(0x4d2);
        let unknown = RequestError::from(OpenUniverseError::UnknownUniverse { id });
        assert_eq!(unknown.code, ErrorCode::UnknownUniverse);
        assert_eq!(unknown.field.as_deref(), Some("universe"));

        let mismatch = RequestError::from(OpenUniverseError::GeneratorVersionMismatch {
            id,
            saved: GeneratorVersion::new(3),
            server: GeneratorVersion::new(9),
        });
        assert_eq!(mismatch.code, ErrorCode::GeneratorVersionMismatch);
        assert_eq!(mismatch.field, None);
        assert_eq!(
            mismatch.message,
            "universe 00000000000004d2 was created with generator version 3, and this server runs \
             generator version 9"
        );

        let format = RequestError::from(OpenUniverseError::UnsupportedSaveFormat { id, format: 2 });
        assert_eq!(format.code, ErrorCode::UnsupportedSaveFormat);
        assert_eq!(format.field, None);
    }

    /// The Milky Way fixture's galaxy, built once for the whole test binary: its values are known,
    /// so the units and the origins can be checked against them.
    fn milky_way() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x4d2), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
        })
    }

    fn universe() -> Universe {
        Universe::from(SavedUniverse::new(
            UniverseId::new(0x0123_4567_89ab_cdef),
            "Talos".parse().unwrap(),
            0x4d2,
            GENERATOR_VERSION,
        ))
    }

    fn fixture() -> GalaxyParameters {
        galaxy_parameters(&universe(), milky_way())
    }

    /// Every parameter of the response, in the order it is sent, with its group.
    fn sent(response: &GalaxyParameters) -> Vec<(&str, &Parameter)> {
        response
            .groups
            .iter()
            .flat_map(|group| {
                group
                    .parameters
                    .iter()
                    .map(move |parameter| (group.key.as_str(), parameter))
            })
            .collect()
    }

    fn find<'a>(response: &'a GalaxyParameters, key: &str) -> &'a Parameter {
        sent(response)
            .into_iter()
            .find(|(_, parameter)| parameter.key == key)
            .unwrap_or_else(|| panic!("no parameter {key}"))
            .1
    }

    fn number_of(response: &GalaxyParameters, key: &str) -> (f64, Unit) {
        match find(response, key).value {
            ParameterValue::Number { value, unit } => (value, unit),
            ParameterValue::Text { ref value } => panic!("{key} is the text {value}"),
        }
    }

    /// The value of `key`, once its unit is what it should be.
    fn value_in(response: &GalaxyParameters, key: &str, unit: Unit) -> f64 {
        let (value, sent) = number_of(response, key);
        assert_eq!(sent, unit, "the unit of {key}");
        value
    }

    /// Asserts that a converted value is `expected` to within a relative tolerance, for the values
    /// that pass through a unit conversion and so need not be bit-exact.
    fn assert_close(key: &str, actual: f64, expected: f64) {
        let tolerance = 1e-12 * expected.abs();
        assert!(
            (actual - expected).abs() <= tolerance,
            "{key} is {actual}, not {expected} to within {tolerance}"
        );
    }

    #[test]
    fn every_galaxy_parameter_is_sent_once_or_excluded_with_a_reason() {
        let response = fixture();
        let sent = sent(&response);
        assert_eq!(
            sent.len() + EXCLUDED_PARAMETERS.len(),
            PARAMETERS_ACCOUNTED_FOR,
            "a parameter was added or removed: send it or add it to EXCLUDED_PARAMETERS, and \
             raise PARAMETERS_ACCOUNTED_FOR"
        );
        for (key, reason) in EXCLUDED_PARAMETERS {
            assert!(!reason.is_empty(), "{key} is excluded without a reason");
        }
    }

    #[test]
    fn parameter_keys_are_unique_and_dotted_snake_case() {
        let response = fixture();
        let mut keys: Vec<&str> = sent(&response)
            .iter()
            .map(|(_, parameter)| parameter.key.as_str())
            .collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "two parameters share a key");
        for key in keys {
            // `^[a-z0-9_]+(\.[a-z0-9_]+)*$`, the form the plan fixes, without a regex crate.
            let well_formed = !key.is_empty()
                && key.split('.').all(|segment| {
                    !segment.is_empty()
                        && segment
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                });
            assert!(well_formed, "the key {key} is not dotted snake_case");
        }
        for group in &response.groups {
            assert!(
                group
                    .key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_'),
                "the group key {} is not snake_case",
                group.key
            );
        }
    }

    /// Plan 04's P04.T14.b table, group by group and key by key, in its order, written out
    /// independently of the builder. The parameters panel shows groups and keys in this order, so
    /// the order is part of the contract with plan 05's glossary (ruling 11 of 2026-09-22), and a
    /// golden blessed from the builder would not catch a key moved within its group.
    const PLAN_TABLE: [(&str, &[&str]); 12] = [
        ("identity", &["seed", "generator_version", "mass_function"]),
        (
            "mass",
            &[
                "stellar_mass",
                "system_count",
                "mean_system_mass",
                "mean_formed_mass",
                "gas.mass",
                "black_hole.mass",
                "nuclear_cluster.mass",
                "dark_halo.mass",
                "dark_halo.concentration",
                "dark_halo.virial_radius",
                "dark_halo.f_star",
            ],
        ),
        (
            "populations",
            &[
                "population.young_thin_disc.share",
                "population.old_thin_disc.share",
                "population.thick_disc.share",
                "population.bulge.share",
                "population.long_bar.share",
                "population.nuclear_disc.share",
                "population.halo.share",
            ],
        ),
        (
            "population_masses",
            &[
                "population.young_thin_disc.mass",
                "population.old_thin_disc.mass",
                "population.thick_disc.mass",
                "population.bulge.mass",
                "population.long_bar.mass",
                "population.nuclear_disc.mass",
                "population.halo.mass",
            ],
        ),
        (
            "population_mean_masses",
            &[
                "population.young_thin_disc.mean_system_mass",
                "population.old_thin_disc.mean_system_mass",
                "population.thick_disc.mean_system_mass",
                "population.bulge.mean_system_mass",
                "population.long_bar.mean_system_mass",
                "population.nuclear_disc.mean_system_mass",
                "population.halo.mean_system_mass",
            ],
        ),
        (
            "discs",
            &[
                "disc.thin.scale_length",
                "disc.thin.scale_height",
                "disc.young.scale_length",
                "disc.young.scale_height",
                "disc.thick.scale_length",
                "disc.thick.scale_height",
                "disc.gas.scale_length",
            ],
        ),
        (
            "bulge_and_bar",
            &[
                "bulge.scale_x",
                "bulge.scale_y",
                "bulge.scale_z",
                "bulge.boxiness",
                "bar.share_of_bulge",
                "bar.half_length",
                "bar.width",
                "bar.height",
                "bar.corotation_ratio",
                "bar.corotation_radius",
                "bar.pattern_speed",
            ],
        ),
        (
            "nuclear_disc",
            &["nuclear_disc.scale_length", "nuclear_disc.scale_height"],
        ),
        (
            "halo",
            &[
                "halo.in_situ.share",
                "halo.dominant_merger.share",
                "halo.lesser.share",
                "halo.globular_debris.share",
                "halo.in_situ.slope",
                "halo.dominant_merger.slope",
                "halo.globular_debris.slope",
                "halo.in_situ.core",
                "halo.dominant_merger.core",
                "halo.globular_debris.core",
                "halo.in_situ.flattening",
                "halo.dominant_merger.flattening",
                "halo.dominant_merger.break_radius",
                "halo.dominant_merger.break_steepening",
            ],
        ),
        (
            "arms",
            &[
                "arms.count",
                "arms.pitch",
                "arms.young_width",
                "arms.young_fraction",
                "arms.old_amplitude",
            ],
        ),
        (
            "history",
            &[
                "history.formation_timescale",
                "history.last_major_merger",
                "history.globular_clusters",
            ],
        ),
        (
            "rotation",
            &[
                "rotation.radius",
                "rotation.circular_speed",
                "rotation.escape_speed",
            ],
        ),
    ];

    #[test]
    fn the_groups_are_the_plans_table_in_its_order() {
        let response = fixture();
        let sent: Vec<(&str, Vec<&str>)> = response
            .groups
            .iter()
            .map(|group| {
                let keys = group.parameters.iter().map(|p| p.key.as_str()).collect();
                (group.key.as_str(), keys)
            })
            .collect();
        let table: Vec<(&str, Vec<&str>)> = PLAN_TABLE
            .iter()
            .map(|(group, keys)| (*group, keys.to_vec()))
            .collect();
        assert_eq!(sent, table);
    }

    #[test]
    fn the_identity_group_carries_the_seed_the_version_and_the_mass_function() {
        let response = fixture();
        assert_eq!(response.seed, SeedHex::from_u64(0x4d2));
        assert_eq!(response.generator_version, GENERATOR_VERSION.get());
        assert_eq!(
            find(&response, "seed").value,
            ParameterValue::Text {
                value: "00000000000004d2".to_owned()
            }
        );
        assert_eq!(
            find(&response, "mass_function").value,
            ParameterValue::Text {
                value: "chabrier".to_owned()
            },
            "the fixture takes the default mass function"
        );
        assert_same_bits(
            value_in(&response, "generator_version", Unit::Count),
            f64::from(GENERATOR_VERSION.get()),
        );
        for key in ["seed", "mass_function", "rotation.radius"] {
            assert_eq!(find(&response, key).origin, ParameterOrigin::Fixed, "{key}");
        }
    }

    #[test]
    fn values_are_in_the_units_the_client_displays() {
        let response = fixture();
        let galaxy = milky_way();
        let params = galaxy.params();
        // Masses in M☉ and lengths in light-years, as the fixture sets them: sent as they are, so
        // to the bit.
        assert_same_bits(
            value_in(&response, "stellar_mass", Unit::Msun),
            5.12e10, // Bland-Hawthorn and Gerhard 2016 (plan 02, P02.T12.d).
        );
        assert_same_bits(
            value_in(&response, "disc.thin.scale_length", Unit::Ly),
            ly(params.thin_disc().length()),
        );
        assert_same_bits(value_in(&response, "arms.count", Unit::Count), 4.0);
        assert_same_bits(value_in(&response, "rotation.radius", Unit::Ly), 26_000.0);
        // Times in gigayears, not the sim's Julian years, and angles in degrees, not radians. Both
        // pass through a conversion, so they are compared to a tolerance.
        assert_close(
            "history.formation_timescale",
            value_in(&response, "history.formation_timescale", Unit::Gyr),
            7.0,
        );
        let merger = value_in(&response, "history.last_major_merger", Unit::Gyr);
        assert!((6.0..=11.0).contains(&merger), "{merger} Gyr");
        assert_close(
            "arms.pitch",
            value_in(&response, "arms.pitch", Unit::Deg),
            12.0,
        );
        // The pattern speed in degrees per megayear: 38 km/s per kpc is 2.23 °/Myr.
        let pattern = value_in(&response, "bar.pattern_speed", Unit::DegPerMyr);
        let radians_per_year = galaxy.potential().bar_pattern_speed().value();
        assert_close(
            "bar.pattern_speed",
            pattern,
            radians_per_year * 180.0 / std::f64::consts::PI * 1e6,
        );
        assert!((1.0..=4.0).contains(&pattern), "{pattern} °/Myr");
        // The rotation curve at the fixed radius, in km/s, against the brainstorm's Milky Way
        // figures: 210–270 km/s at 8 kpc, and an escape speed of 574 km/s there (500–580 measured).
        let circular = value_in(&response, "rotation.circular_speed", Unit::KmPerS);
        assert!(
            (210.0..=270.0).contains(&circular),
            "{circular} km/s at 26,000 ly is outside the brainstorm's 210–270 km/s at 8 kpc"
        );
        let escape = value_in(&response, "rotation.escape_speed", Unit::KmPerS);
        assert!(
            (500.0..=580.0).contains(&escape),
            "{escape} km/s is outside the brainstorm's 500–580 km/s at the Sun's radius"
        );
        // Every share is dimensionless, and the populations' shares sum to one.
        let shares: f64 = response.groups[2]
            .parameters
            .iter()
            .map(|parameter| match parameter.value {
                ParameterValue::Number { value, unit } => {
                    assert_eq!(unit, Unit::None, "{}", parameter.key);
                    value
                }
                ParameterValue::Text { .. } => panic!("a share is a number"),
            })
            .sum();
        assert!((shares - 1.0).abs() < 1e-12, "the shares sum to {shares}");
    }

    #[test]
    fn a_share_is_drawn_or_derived_as_plan_02_draws_it() {
        let response = fixture();
        for (population, origin) in [
            (Population::ThickDisc, ParameterOrigin::Drawn),
            (Population::NuclearDisc, ParameterOrigin::Drawn),
            (Population::Halo, ParameterOrigin::Drawn),
            (Population::YoungThinDisc, ParameterOrigin::Derived),
            (Population::OldThinDisc, ParameterOrigin::Derived),
            (Population::Bulge, ParameterOrigin::Derived),
            (Population::LongBar, ParameterOrigin::Derived),
        ] {
            let key = population_key(population, "share");
            assert_eq!(find(&response, &key).origin, origin, "{key}");
        }
    }

    #[test]
    fn the_halo_group_sums_the_lesser_progenitors_and_lists_the_others() {
        let response = fixture();
        let halo = milky_way().params().halo();
        let lesser: Vec<f64> = halo
            .components()
            .iter()
            .filter(|c| matches!(c.kind(), HaloComponentKind::Lesser(_)))
            .map(HaloComponentParams::share)
            .collect();
        assert!(
            (2..=5).contains(&lesser.len()),
            "{} lesser progenitors",
            lesser.len()
        );
        let (sent_share, unit) = number_of(&response, "halo.lesser.share");
        assert_eq!(unit, Unit::None);
        // The sum in the components' order, to the bit: that order is part of the response.
        assert_same_bits(sent_share, lesser.iter().sum::<f64>());
        let named: f64 = [
            "halo.in_situ.share",
            "halo.dominant_merger.share",
            "halo.lesser.share",
            "halo.globular_debris.share",
        ]
        .iter()
        .map(|key| number_of(&response, key).0)
        .sum();
        assert!(
            (named - 1.0).abs() < 1e-12,
            "the halo's shares sum to {named}"
        );
        assert_eq!(
            number_of(&response, "halo.dominant_merger.break_radius").1,
            Unit::Ly
        );
    }

    #[test]
    fn the_mass_group_is_consistent_with_the_populations() {
        let response = fixture();
        let (stellar, _) = number_of(&response, "stellar_mass");
        let (count, _) = number_of(&response, "system_count");
        let (mean, _) = number_of(&response, "mean_system_mass");
        assert!((mean - stellar / count).abs() < 1e-12 * mean);
        let masses: f64 = POPULATIONS
            .iter()
            .map(|&population| number_of(&response, &population_key(population, "mass")).0)
            .sum();
        assert!(
            (masses / stellar - 1.0).abs() < 1e-9,
            "the populations' masses sum to {masses}, not {stellar}"
        );
        let (virial, _) = number_of(&response, "dark_halo.virial_radius");
        let (concentration, _) = number_of(&response, "dark_halo.concentration");
        assert!(
            (virial / concentration - ly(milky_way().params().dark_halo().scale_radius())).abs()
                < 1e-6,
            "the scale radius is excluded because it follows from these two"
        );
    }

    /// A `systems_in_range` request over the fixture, at a cell of the galactic frame.
    fn range_request(
        cell_ly: [i32; 3],
        radius_ly: f64,
        limit: u32,
        min_layer: MassLayer,
    ) -> SystemsInRangeRequest {
        SystemsInRangeRequest {
            universe: hyperion_protocol::UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
            centre: hyperion_protocol::GalacticPosition {
                cell_ly,
                offset_m: [0.0; 3],
            },
            radius_ly,
            time: hyperion_protocol::UniverseTime {
                seconds: 0,
                nanos: 0,
            },
            min_layer,
            limit,
            include_stellar: false,
        }
    }

    /// The Sun-like point: in the plane, 26,000 ly out on the +y axis, clear of the bar.
    fn sunlike(radius_ly: f64, limit: u32, min_layer: MassLayer) -> SystemsInRangeRequest {
        range_request([0, 26_000, 0], radius_ly, limit, min_layer)
    }

    /// The answer to `request` over the fixture with no cache: what the handler's pool job builds.
    fn answer(request: &SystemsInRangeRequest) -> SystemsInRange {
        let query = RangeRequest::try_from(request)
            .expect("a valid request")
            .into_query();
        let result = range_query(milky_way(), &mut NoCache::new(), &[], &query);
        systems_in_range(request.clone(), &query, &result)
    }

    #[test]
    fn a_request_for_briefs_gets_rows_without_them_until_the_integrator_is_fast() {
        // The flag is the protocol's (P06.T33), but no brief is built until the track integrator's
        // optimisation lands (ruling 46), so every row is still plan 04's.
        let mut request = sunlike(50.0, 5_000, MassLayer::A);
        request.include_stellar = true;
        let answer = answer(&request);
        assert!(
            !answer.systems.is_empty(),
            "a 50 ly sphere at the Sun is not empty"
        );
        assert!(answer.systems.iter().all(|row| row.stellar.is_none()));
    }

    /// The field `bad_request` names, for a request that is refused.
    fn refused_field(request: &SystemsInRangeRequest) -> String {
        let error = RequestError::from(RangeRequest::try_from(request).unwrap_err());
        assert_eq!(error.code, ErrorCode::BadRequest, "{error:?}");
        error.field.expect("a bad field names itself")
    }

    /// One layer's line in a census, by layer.
    fn line(census: &hyperion_protocol::Census, layer: MassLayer) -> &LayerCensus {
        census
            .layers
            .iter()
            .find(|line| line.layer == layer)
            .unwrap_or_else(|| panic!("no census line for layer {layer:?}"))
    }

    #[test]
    fn the_fields_of_a_range_query_are_checked_in_design_note_24s_order() {
        // Every field at fault at once: the answer names the first of them, and mending each in
        // turn walks the order time, centre, radius, limit.
        let mut request = sunlike(200_000.0, 0, MassLayer::A);
        request.time.nanos = 1_000_000_000;
        request.centre.cell_ly = [200_000, 0, 0];
        assert_eq!(refused_field(&request), "time");
        request.time.nanos = 0;
        request.time.seconds = 2_000 * 31_557_600;
        assert_eq!(refused_field(&request), "time", "a time outside the window");
        request.time.seconds = 0;
        assert_eq!(refused_field(&request), "centre");
        request.centre.cell_ly = [0, 26_000, 0];
        assert_eq!(refused_field(&request), "radius_ly");
        request.radius_ly = 50.0;
        assert_eq!(refused_field(&request), "limit");
        request.limit = 1;
        RangeRequest::try_from(&request).expect("every field is now in range");
    }

    #[test]
    fn each_field_that_cannot_be_used_names_itself() {
        let field = |request: SystemsInRangeRequest| refused_field(&request);
        // A time whose nanoseconds are not below a second, and one outside the clock window.
        let mut nanos = sunlike(50.0, 100, MassLayer::A);
        nanos.time.nanos = 1_000_000_000;
        assert_eq!(field(nanos), "time");
        let mut late = sunlike(50.0, 100, MassLayer::A);
        late.time.seconds = 1_001 * 31_557_600;
        assert_eq!(field(late), "time");
        // A centre outside the root cube, and one whose offset is not canonical.
        let mut outside = sunlike(50.0, 100, MassLayer::A);
        outside.centre.cell_ly = [65_536, 0, 0];
        assert_eq!(field(outside), "centre");
        let mut uncanonical = sunlike(50.0, 100, MassLayer::A);
        uncanonical.centre.offset_m = [1e17, 0.0, 0.0];
        assert_eq!(field(uncanonical), "centre");
        // A radius that is not finite, not positive, or wider than the root cube.
        for radius in [
            f64::INFINITY,
            f64::NAN,
            0.0,
            -1.0,
            MAX_QUERY_RADIUS_LY + 1.0,
        ] {
            let request = sunlike(radius, 100, MassLayer::A);
            assert_eq!(field(request), "radius_ly", "{radius}");
        }
        // A limit of none, or beyond what one response may hold.
        for limit in [0, MAX_CENSUS_LIMIT + 1] {
            let request = sunlike(50.0, limit, MassLayer::A);
            assert_eq!(field(request), "limit", "{limit}");
        }
        // The edges hold: the window's ends, the widest radius, and the largest limit.
        for seconds in [-1_000 * 31_557_600, 1_000 * 31_557_600] {
            let mut edge = sunlike(MAX_QUERY_RADIUS_LY, MAX_CENSUS_LIMIT, MassLayer::A);
            edge.time.seconds = seconds;
            RangeRequest::try_from(&edge).expect("the clock window includes its ends");
        }
    }

    #[test]
    fn a_checked_request_carries_the_servers_cell_budget_and_its_own_terms() {
        let request = sunlike(50.0, 4_000, MassLayer::D);
        let query = RangeRequest::try_from(&request).unwrap().into_query();
        assert_eq!(query.cell_budget(), MAX_QUERY_CELLS);
        assert_eq!(query.limit().get(), 4_000);
        assert_eq!(query.mass_floor(), MassFloor::LayerD);
        assert_eq!(query.time(), UniverseTime::EPOCH);
        assert_eq!(query.centre().cell(), LyCell::new([0, 26_000, 0]));
        assert_same_bits(query.radius().value(), 50.0);
    }

    #[test]
    fn every_min_layer_becomes_its_mass_floor() {
        for (layer, floor) in [
            (MassLayer::A, MassFloor::LayerA),
            (MassLayer::B, MassFloor::LayerB),
            (MassLayer::C, MassFloor::LayerC),
            (MassLayer::D, MassFloor::LayerD),
            (MassLayer::E, MassFloor::LayerE),
        ] {
            assert_eq!(mass_floor(layer), floor, "{layer:?}");
        }
    }

    #[test]
    fn a_layer_is_below_the_floor_when_the_floor_does_not_reach_it() {
        // The stellar layers from the lightest, and the floor that admits each as its finest: a
        // floor admits its own layer and every coarser one, so what lies below it is the lighter
        // layers, whatever order `Layer` itself sorts in.
        let stellar = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];
        let floors = [
            MassFloor::LayerA,
            MassFloor::LayerB,
            MassFloor::LayerC,
            MassFloor::LayerD,
            MassFloor::LayerE,
        ];
        for (finest, floor) in floors.into_iter().enumerate() {
            for (index, layer) in stellar.into_iter().enumerate() {
                assert_eq!(
                    below_mass_floor(layer, floor),
                    index < finest,
                    "{layer:?} under {floor:?}"
                );
            }
        }
        // A substellar layer is in no census this milestone builds, and no floor names one yet.
        for layer in [Layer::BrownDwarf, Layer::RoguePlanet] {
            assert!(!below_mass_floor(layer, MassFloor::LayerA), "{layer:?}");
        }
    }

    #[test]
    fn the_census_lists_all_five_layers_with_plan_02s_bands_in_order() {
        let request = sunlike(50.0, MAX_CENSUS_LIMIT, MassLayer::A);
        let answer = answer(&request);
        let census = &answer.census;
        assert_eq!(census.limit, MAX_CENSUS_LIMIT);
        let layers: Vec<MassLayer> = census.layers.iter().map(|line| line.layer).collect();
        assert_eq!(
            layers,
            [
                MassLayer::A,
                MassLayer::B,
                MassLayer::C,
                MassLayer::D,
                MassLayer::E
            ]
        );
        // Plan 02's band edges, and each layer's own expected count.
        let bands = [
            (MassLayer::A, 0.08, 0.5),
            (MassLayer::B, 0.5, 0.75),
            (MassLayer::C, 0.75, 2.5),
            (MassLayer::D, 2.5, 8.0),
            (MassLayer::E, 8.0, 150.0),
        ];
        let mut returned = 0;
        for (layer, lo, hi) in bands {
            let line = line(census, layer);
            assert_same_bits(line.mass_min_msun, lo);
            assert_same_bits(line.mass_max_msun, hi);
            assert!(line.expected > 0.0, "{layer:?}: {line:?}");
            assert_eq!(line.status, LayerStatus::Included, "{line:?}");
            returned += line.returned;
        }
        // The whole sphere fits under the limit, so the result is complete to the lightest layer
        // and every system found is counted in its layer's line.
        assert_same_bits(census.complete_above_msun.expect("layer A fits"), 0.08);
        assert_eq!(usize::try_from(returned).unwrap(), answer.systems.len());
        assert!(!answer.systems.is_empty(), "the solar circle is not empty");
    }

    #[test]
    fn the_layers_below_the_requested_floor_say_so_and_return_nothing() {
        let request = sunlike(50.0, MAX_CENSUS_LIMIT, MassLayer::C);
        let answer = answer(&request);
        for (layer, status) in [
            (MassLayer::A, LayerStatus::BelowMassFloor),
            (MassLayer::B, LayerStatus::BelowMassFloor),
            (MassLayer::C, LayerStatus::Included),
            (MassLayer::D, LayerStatus::Included),
            (MassLayer::E, LayerStatus::Included),
        ] {
            let line = line(&answer.census, layer);
            assert_eq!(line.status, status, "{line:?}");
            // A layer that is out is out whole: the census is all-or-nothing per layer.
            if status != LayerStatus::Included {
                assert_eq!(line.returned, 0, "{line:?}");
            }
            // Its expected count is reported whether it is included or not.
            assert!(line.expected > 0.0, "{line:?}");
        }
        assert_same_bits(
            answer.census.complete_above_msun.expect("layer C fits"),
            0.75,
        );
        for record in &answer.systems {
            assert!(
                matches!(record.layer, MassLayer::C | MassLayer::D | MassLayer::E),
                "{record:?}"
            );
        }
    }

    #[test]
    fn a_census_that_admits_nothing_is_an_answer_with_no_systems() {
        // Fifty light-years of the galactic centre expects tens of thousands of layer-E systems
        // alone, so nothing fits under a limit of 5,000 (design note 13).
        let request = range_request([0, 0, 0], 50.0, 5_000, MassLayer::A);
        let answer = answer(&request);
        assert_eq!(answer.census.complete_above_msun, None);
        assert!(answer.systems.is_empty());
        for line in &answer.census.layers {
            assert_eq!(line.status, LayerStatus::OverLimit, "{line:?}");
            assert_eq!(line.returned, 0);
        }
    }

    #[test]
    fn the_answer_echoes_the_request_and_carries_each_record_at_the_querys_time() {
        let mut request = sunlike(50.0, MAX_CENSUS_LIMIT, MassLayer::D);
        // A century before the epoch: every age is 100 years lower and no position has moved, since
        // plan 08 draws the velocities.
        request.time.seconds = -100 * 31_557_600;
        let answer = answer(&request);
        assert_eq!(answer.universe, request.universe);
        assert_eq!(answer.centre, request.centre);
        assert_same_bits(answer.radius_ly, request.radius_ly);
        assert_eq!(answer.time, request.time);
        assert!(!answer.systems.is_empty());
        let query = RangeRequest::try_from(&request).unwrap().into_query();
        let galaxy = milky_way();
        for record in &answer.systems {
            let id = hyperion_sim::id::SystemId::from_raw(record.id.to_u64())
                .expect("a record's ID is a system ID");
            // The sim resolves the ID to the very record the query returned.
            let resolved = hyperion_sim::galaxy::placement::resolve(galaxy, id)
                .expect("a returned system resolves");
            assert_eq!(record.designation, id.designation().to_string());
            assert_eq!(record.layer, mass_layer(resolved.layer()));
            assert_eq!(record.population, wire_population(resolved.population()));
            assert_same_bits(
                record.initial_mass_msun,
                resolved.primary_initial_mass().value(),
            );
            assert_same_bits(
                record.age_myr,
                Megayears::from(resolved.age_at(query.time())).value(),
            );
            assert_eq!(
                record.position,
                galactic_position(resolved.epoch_position())
            );
        }
    }

    #[test]
    fn every_population_carries_its_own_name_to_the_wire() {
        for population in POPULATIONS {
            let wire = serde_json::to_value(wire_population(population)).unwrap();
            assert_eq!(wire, serde_json::json!(population.name()), "{population:?}");
        }
    }

    #[test]
    fn every_stellar_layer_carries_its_letter_to_the_wire() {
        for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
            let wire = serde_json::to_value(mass_layer(layer)).unwrap();
            let letter = layer.letter().to_ascii_lowercase().to_string();
            assert_eq!(wire, serde_json::json!(letter), "{layer:?}");
        }
    }

    #[test]
    #[should_panic(expected = "has no wire form until plan 13")]
    fn a_substellar_layer_has_no_wire_layer_yet() {
        let _ = mass_layer(Layer::BrownDwarf);
    }

    #[test]
    fn a_query_the_sim_refuses_still_names_a_field() {
        // Nothing the checks above let through reaches this, so it is exercised directly.
        for (error, field) in [
            (BuildRangeQueryError::RadiusNotFinite, "radius_ly"),
            (BuildRangeQueryError::RadiusNotPositive, "radius_ly"),
            (BuildRangeQueryError::RadiusBeyondRootCube, "radius_ly"),
            (BuildRangeQueryError::CentreOutsideRootCube, "centre"),
            (
                BuildRangeQueryError::TimeOutsideClockWindow(UniverseTime::EPOCH),
                "time",
            ),
            (
                BuildRangeQueryError::SubstellarLayersUnavailable,
                "min_layer",
            ),
        ] {
            let converted = RequestError::from(refused_query(error));
            assert_eq!(converted.code, ErrorCode::BadRequest);
            assert_eq!(converted.field.as_deref(), Some(field), "{error:?}");
        }
    }
}
