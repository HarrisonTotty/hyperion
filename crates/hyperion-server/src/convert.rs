//! Conversion between the wire's types and the server's, and the validation of every request
//! field on the way in (plan 04, P04.T14).
//!
//! A request's fields are checked here, once, by a `TryFrom` from its wire type; a field that
//! cannot be used becomes a [`ConvertRequestError`], answered `bad_request` with the field named.
//! Answers are built here too, by `From` from the server's types to the wire's, as are the request
//! errors that the server's own errors become.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use hyperion_protocol::{
    CreateUniverseRequest, DensityMap, DensityMapRequest, ErrorCode, GalaxyParameters, Parameter,
    ParameterGroup, ParameterOrigin, ParameterValue, RequestError, SeedHex, Unit, UniverseInfo,
    UniverseList,
};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::{
    ArmParams, GalaxyParams, HaloComponentKind, HaloComponentParams, HaloParams,
};
use hyperion_sim::galaxy::potential::PotentialTables;
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, Population};
use hyperion_sim::units::{
    Degrees, Gigayears, KilometresPerSecond, LightYears, PerYear, Radians, SolarMasses, Years,
};
use hyperion_sim::{GENERATOR_VERSION, GeneratorVersion};

use crate::compute::{CodeDepth, GalaxyKey, MapKey, MapResolution, QuantisedMap, RawDensityMap};
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
        GALAXY.get_or_init(|| Galaxy::from_params(Seed::new(0x4d2), GalaxyParams::milky_way_like()))
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

    #[test]
    fn the_groups_are_the_plans_table_in_its_order() {
        let response = fixture();
        let groups: Vec<&str> = response
            .groups
            .iter()
            .map(|group| group.key.as_str())
            .collect();
        assert_eq!(
            groups,
            [
                "identity",
                "mass",
                "populations",
                "population_masses",
                "population_mean_masses",
                "discs",
                "bulge_and_bar",
                "nuclear_disc",
                "halo",
                "arms",
                "history",
                "rotation",
            ]
        );
        let per_population: Vec<&str> = response.groups[2]
            .parameters
            .iter()
            .map(|parameter| parameter.key.as_str())
            .collect();
        assert_eq!(
            per_population,
            [
                "population.young_thin_disc.share",
                "population.old_thin_disc.share",
                "population.thick_disc.share",
                "population.bulge.share",
                "population.long_bar.share",
                "population.nuclear_disc.share",
                "population.halo.share",
            ]
        );
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
            6.0e10, // Licquia and Newman 2015.
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
}
