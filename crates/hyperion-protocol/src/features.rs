//! Feature messages (galaxy plan 09, P09.T39): the large features within range of a point, the
//! features the galaxy map draws, one feature's structure, and the central black hole; and what a
//! range row adds for a system that a feature or a catalogue class placed.
//!
//! These are wire types only, as every type of this crate is: the server converts plan 09's
//! simulation types to them (P09.T40), and nothing here depends on the simulation crate. Until a
//! server answers the four kinds, it answers each `unsupported`, as plan 04's design note 15
//! has a server answer a kind it does not serve, and leaves the range row's three new fields off
//! the wire. A type's shape is fixed by the first build that sends a value of it
//! (`PROTOCOL_VERSION`'s documentation), which is P09.T40's; until then a shape here may still
//! change without a bump. Every enum here is closed, carrying all its variants from that build, but
//! the three plan 04's reserved list names as growing (ruled 2026-10-30, before any build sent a
//! value of them): [`SystemOriginWire`] (plan 10's members and pinned content),
//! [`CatalogueClassWire`] (classes registered later) and [`MemberClassWire`], each of which takes
//! a new variant without a bump.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::galaxy::MassLayer;
use crate::primitives::{FeatureIdHex, GalacticPosition, SystemIdHex, UniverseIdHex, UniverseTime};

/// What a large feature is at a time (plan 09's `FeatureKind`).
///
/// The catalogue places the first five. A nursery is a star-forming region while it is embedded,
/// then a bound open cluster or an association until it dissolves, so a feature's kind depends on
/// the time it is asked at. The galactic centre is the first entry of plan 10's global list, and
/// streams and dwarf cores are the global list's too, which plan 10 places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FeatureKindWire {
    /// A globular cluster.
    Globular,
    /// A bound open cluster, young or old.
    OpenCluster,
    /// An unbound OB association, expanding until it dissolves.
    Association,
    /// A star-forming region: an embedded cluster still forming, with its gas.
    StarFormingRegion,
    /// A molecular cloud or dark nebula: gas and dust, with no members.
    MolecularCloud,
    /// The galactic centre: its nuclear cluster and black hole.
    GalacticCentre,
    /// A stellar stream (plan 10).
    Stream,
    /// The core of an accreted dwarf galaxy (plan 10).
    DwarfCore,
}

/// What a feature shines as besides its stars (plan 09's `EmissionClass`, Design note 17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum EmissionClassWire {
    /// An H II region: gas ionised by hot young stars, while a nursery expects at least half a
    /// living star above 15 M☉.
    HiiRegion,
    /// A reflection nebula: dust lit by stars too cool to ionise it.
    ReflectionNebula,
    /// A dark cloud: gas and dust seen against what lies behind.
    DarkCloud,
    /// A supernova's expanding shell.
    RemnantShell,
    /// A pulsar's wind nebula inside its shell.
    PulsarWindNebula,
}

/// One large feature as it is at a time: what a list, a mark or a readout shows of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FeatureSummary {
    /// The feature's ID.
    pub id: FeatureIdHex,
    /// Its designation, derived from the ID, such as `H7K F212`: never a proper name.
    pub designation: String,
    /// What it is at the time.
    pub kind: FeatureKindWire,
    /// Its centre at the time, in the `GALACTIC` frame.
    pub position: GalacticPosition,
    /// Its radius at the time, in light-years: the sphere about `position` that a chart draws it
    /// as.
    pub extent_ly: f64,
    /// Its age at the time, in megayears; `null` for a molecular cloud, which has none.
    pub age_myr: Option<f64>,
    /// Its present mass, in M☉: its stars' for a cluster, an association or the centre, its gas's
    /// for a molecular cloud.
    pub mass_msun: f64,
    /// The expected number of its member systems at the time: zero for a molecular cloud.
    pub expected_members: f64,
    /// What it shines as besides its stars; `null` for a feature that shines as nothing but its
    /// stars, such as an old cluster.
    pub emission: Option<EmissionClassWire>,
    /// The radius of a nursery's superbubble at the time, in light-years; `null` for a feature with
    /// no bubble.
    pub bubble_radius_ly: Option<f64>,
}

/// Asks for the large features within `radius_ly` of `centre` at `time` (`features_in_range`),
/// answered with [`FeaturesInRange`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FeaturesInRangeRequest {
    /// The universe to search.
    pub universe: UniverseIdHex,
    /// The centre of the sphere, in the `GALACTIC` frame, inside the root cube.
    pub centre: GalacticPosition,
    /// The sphere's radius in light-years: finite, above zero and at most 131,072, as a range
    /// query's.
    pub radius_ly: f64,
    /// The instant at which the features are placed, aged and given their kind, within 1,000 years
    /// of the epoch.
    pub time: UniverseTime,
    /// The kinds wanted: at least one, each at most once. The census has a line for each, in this
    /// order.
    pub kinds: Vec<FeatureKindWire>,
}

/// The large features within range of a point at a time, with the census that says which kinds
/// are complete.
///
/// `centre`, `radius_ly` and `time` echo the request. A feature is within range when its sphere of
/// `extent_ly` about its position at `time` meets the query's sphere. The features come in the
/// server's order, and clients sort for themselves; finding none is a valid answer, not an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FeaturesInRange {
    /// The universe searched.
    pub universe: UniverseIdHex,
    /// The centre of the sphere, in the `GALACTIC` frame.
    pub centre: GalacticPosition,
    /// The sphere's radius in light-years.
    pub radius_ly: f64,
    /// The instant at which the features were placed.
    pub time: UniverseTime,
    /// One line for each requested kind, in the request's order.
    pub census: Vec<FeatureKindCensus>,
    /// Every feature of the included kinds within range.
    pub features: Vec<FeatureSummary>,
}

/// One kind's line in a features-in-range census.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FeatureKindCensus {
    /// The kind.
    pub kind: FeatureKindWire,
    /// The number of the kind's features returned: 0 unless the kind is included.
    pub returned: u32,
    /// Whether the kind is included, and why not otherwise.
    pub status: FeatureKindStatus,
}

/// Whether a kind is in a features-in-range answer.
///
/// Each kind is returned whole or not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FeatureKindStatus {
    /// Every feature of the kind within range is returned.
    Included,
    /// Left out because the query would have visited too many feature cells.
    OverCellBudget,
    /// Left out because this server places none of the kind: streams and dwarf cores, which plan
    /// 10's global list holds.
    NotGenerated,
}

/// What the galaxy map's feature overlay can draw (plan 09, P09.T41.b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MapFeatureKindWire {
    /// Every globular cluster.
    Globular,
    /// The galactic centre.
    GalacticCentre,
    /// Every supernova remnant whose shell is still non-radiative, bright in X-rays and radio.
    BrightShell,
}

/// Asks for the features the galaxy map draws at a time (`galaxy_features`), answered with
/// [`GalaxyFeatures`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GalaxyFeaturesRequest {
    /// The universe whose galaxy is drawn.
    pub universe: UniverseIdHex,
    /// The kinds wanted: at least one, each at most once. The answer has a list for each, in this
    /// order.
    pub kinds: Vec<MapFeatureKindWire>,
    /// The instant they are drawn at, within 1,000 years of the epoch.
    pub time: UniverseTime,
}

/// The features the galaxy map draws at a time, each kind complete or withheld with the reason.
///
/// `time` echoes the request. Plan 10's global list (`global_features`) is another request, which
/// the map asks for streams and dwarf cores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GalaxyFeatures {
    /// The universe drawn.
    pub universe: UniverseIdHex,
    /// The instant they were drawn at.
    pub time: UniverseTime,
    /// One list for each requested kind, in the request's order.
    pub lists: Vec<MapFeatureList>,
}

/// One kind's list for the galaxy map, tagged by `kind` with the strings of
/// [`MapFeatureKindWire`], its entries' type fixed by the kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum MapFeatureList {
    /// The globular clusters.
    Globular(MapFeatures),
    /// The galactic centre.
    GalacticCentre(MapFeatures),
    /// The bright shells.
    BrightShell(MapShells),
}

/// A kind's features for the galaxy map, tagged by `status`: all of them, or none with the reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export)]
pub enum MapFeatures {
    /// Every feature of the kind at the time, in the server's order.
    Complete {
        /// The features.
        features: Vec<FeatureSummary>,
    },
    /// Withheld because this server places none of the kind.
    NotGenerated,
}

/// The bright shells for the galaxy map, tagged by `status`: all of them, or none with the reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export)]
pub enum MapShells {
    /// Every bright shell at the time, in the server's order.
    Complete {
        /// The shells.
        shells: Vec<BrightShell>,
    },
    /// Withheld because this server places none: until the catalogue classes are served.
    NotGenerated,
}

/// A supernova remnant's shell as the galaxy map draws it: the system whose explosion made it, and
/// the shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BrightShell {
    /// The catalogue system that exploded.
    pub system: SystemIdHex,
    /// Its designation, derived from the ID.
    pub designation: String,
    /// The explosion site, the shell's centre, in the `GALACTIC` frame.
    pub site: GalacticPosition,
    /// The time since the explosion, in Julian years.
    pub age_yr: f64,
    /// The shell.
    pub shell: ShellSummary,
}

/// Asks for one feature's structure at a time (`feature_detail`), answered with
/// [`FeatureDetail`].
///
/// The server resolves the ID first: a well-formed ID that names no feature of the universe is
/// refused, naming `feature`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FeatureDetailRequest {
    /// The universe the feature is in.
    pub universe: UniverseIdHex,
    /// The feature.
    pub feature: FeatureIdHex,
    /// The instant it is described at, within 1,000 years of the epoch.
    pub time: UniverseTime,
}

/// One feature at a time: its summary and its structure.
///
/// `universe` and `time` echo the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FeatureDetail {
    /// The universe the feature is in.
    pub universe: UniverseIdHex,
    /// The instant it is described at.
    pub time: UniverseTime,
    /// The feature, as a range answer lists it.
    pub feature: FeatureSummary,
    /// Its structure, by what it is.
    pub structure: FeatureStructure,
}

/// A feature's structure, tagged by `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum FeatureStructure {
    /// A cluster of stars, bound or not: a globular, an open cluster, an association or a
    /// star-forming region (plan 09's `ClusterModel`).
    Cluster(ClusterStructure),
    /// The galactic centre's nuclear cluster about its black hole (plan 09's `CentreModel`).
    Centre(CentreStructure),
    /// A molecular cloud: gas and dust, with no members.
    Cloud(CloudStructure),
}

/// A cluster's structure today.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClusterStructure {
    /// Its core radius, in light-years.
    pub core_radius_ly: f64,
    /// The radius holding half its mass, in light-years.
    pub half_mass_radius_ly: f64,
    /// Its tidal radius, in light-years: where the galaxy's tide strips its stars.
    pub tidal_radius_ly: f64,
    /// The escape speed from its centre, in km/s.
    pub central_escape_speed_km_s: f64,
    /// Its half-mass relaxation time, in megayears.
    pub relaxation_time_myr: f64,
    /// Whether its core has collapsed: it holds no black holes and is older than 14 half-mass
    /// relaxation times (plan 09, P09.T9.d).
    pub core_collapsed: bool,
    /// The expected number of stellar black holes it still holds.
    pub black_holes: f64,
    /// Its member classes with their expected counts, in the server's order.
    pub classes: Vec<ClassCountWire>,
}

/// The galactic centre's structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CentreStructure {
    /// The radius of the black hole's sphere of influence, in light-years: where the cluster's
    /// stars inside it weigh as much as the black hole.
    pub influence_radius_ly: f64,
    /// The radius of the black hole's loss cone, in light-years: a member whose pericentre falls
    /// inside it is torn apart or swallowed.
    pub loss_cone_radius_ly: f64,
    /// Its member classes with their expected counts in the whole cluster, in the server's order.
    pub classes: Vec<ClassCountWire>,
}

/// A molecular cloud's structure.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CloudStructure {
    /// The core radius of its Plummer profile, in light-years.
    pub core_radius_ly: f64,
    /// The density of hydrogen at its centre, per cubic centimetre.
    pub central_density_per_cm3: f64,
}

/// One member class of a feature with its expected count.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClassCountWire {
    /// The class.
    pub class: MemberClassWire,
    /// The lower edge of the range its primaries' initial masses are drawn from, in M☉.
    pub initial_mass_min_msun: f64,
    /// The upper edge of that range, in M☉.
    pub initial_mass_max_msun: f64,
    /// The mean present mass of its systems, in M☉.
    pub mean_mass_msun: f64,
    /// Its expected count.
    pub expected: f64,
}

/// The class of a feature's member, tagged by `type`: what it is, and where in its feature's
/// tables it was drawn from.
///
/// The enum grows (plan 04's reserved list): a later tag, such as one of plan 10's own, needs no
/// bump, and a client that does not know it reads the class as Missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum MemberClassWire {
    /// A class of a catalogue cluster's interior (plan 09's `MemberClass`, P09.T9).
    Cluster {
        /// What its members are.
        kind: ClassKindWire,
        /// The layer whose band its primaries' initial masses fall in, `a` to `e`.
        band: MassLayer,
        /// Which of a massive globular's populations it belongs to.
        generation: GenerationWire,
        /// Whether its systems are single or binary.
        multiplicity: MultiplicityWire,
    },
    /// A class of the galactic centre's nuclear cluster (plan 09's `CentreClass`), never a tail.
    Centre {
        /// What its members are.
        kind: ClassKindWire,
        /// The layer whose band its primaries' initial masses fall in, `a` to `e`.
        band: MassLayer,
        /// The star-formation episode its members were born in, an index into the centre's
        /// history (plan 09's `age_components`, after Schödel et al. 2020): 0 the old stars of
        /// 10–13 Gyr, 1 the intermediate of 2.5–3.5 Gyr, 2 the recent of 150–500 Myr, 3 the
        /// continuous since, 4 and 5 the burst of 3–8 Myr on the clockwise disc and off it.
        episode: u8,
        /// The distribution function its members' orbits are drawn from.
        tracer: CentreTracerWire,
    },
    /// An entry of a feature's feature-level list, which holds its catalogue classes' hosts in
    /// the feature, such as a cluster's core-collapse progenitors or the centre's disrupted stars
    /// (plan 09, Design note 16).
    Listed {
        /// The entry's catalogue class.
        class: CatalogueClassWire,
    },
}

/// What a member class's systems are (plan 09's `ClassKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ClassKindWire {
    /// Living stars below the turn-off.
    Living,
    /// White dwarfs, by their progenitors' band.
    WhiteDwarf,
    /// Neutron stars retained at birth.
    NeutronStar,
    /// Black holes still bound.
    BlackHole,
    /// A cluster's near tidal tail, of its first population.
    Tail,
}

/// Which of a massive globular's populations a class belongs to (plan 09's `Generation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum GenerationWire {
    /// The first population: every open cluster's, and every globular's born below 10⁵ M☉.
    First,
    /// The enriched second population of a globular born above 10⁵ M☉.
    Second,
}

/// Whether a class's systems are single or binary (plan 09's `Multiplicity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MultiplicityWire {
    /// Single systems.
    Single,
    /// Binaries, profiled by their system mass.
    Binary,
}

/// The distribution function a centre class's orbits are drawn from (plan 09's `CentreTracer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CentreTracerWire {
    /// The nuclear cluster's own profile.
    Stars,
    /// The black holes' steeper cusp.
    BlackHoles,
    /// The young clockwise disc.
    YoungDisc,
    /// The young stars off the disc, in an isotropic cusp.
    YoungIsotropic,
}

/// A catalogue class: a kind of rare host placed on a grid of its own (plan 09's `ClassId`
/// registry, in its value order).
///
/// Every class the registry allocates is here, those plan 11 fills among them. A tidal-disruption
/// victim is only ever an entry of the centre's feature-level list, never a catalogue system. The
/// registry leaves values 9–63 free for classes registered later. The enum grows (plan 04's
/// reserved list): a class registered later takes a new string without a bump, and a client that
/// does not know it reads it as Missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CatalogueClassWire {
    /// A core-collapse supernova's host.
    CoreCollapse,
    /// A Type Ia supernova's host.
    TypeIa,
    /// A stellar merger (plan 11).
    StellarMerger,
    /// A neutron-star merger (plan 11).
    NeutronStarMerger,
    /// A luminous blue variable.
    LuminousBlueVariable,
    /// An X-ray binary (plan 11).
    XrayBinary,
    /// An accreting white dwarf, of the fast hosts (plan 11).
    AccretingWhiteDwarf,
    /// A star the central black hole tore apart or swallowed.
    TidalDisruptionVictim,
    /// An accreting white dwarf, of the slow hosts (plan 11).
    AccretingWhiteDwarfSlow,
}

/// What placed a system, tagged by `type` (plan 03's `SystemOrigin`, as plan 09 extends it).
///
/// Plan 10's members of streams and dwarf cores have no value here yet: P10.T10.a adds them. The
/// enum grows (plan 04's reserved list), so they, and pinned content's, take new strings without a
/// bump, and a client that does not know one reads the origin as Missing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum SystemOriginWire {
    /// A layer's grid, in the population the row names.
    Grid,
    /// A catalogue feature, as one of its members.
    FeatureMember {
        /// The feature.
        feature: FeatureIdHex,
    },
    /// The galactic centre, as one of its members or as its black hole.
    CentreMember,
    /// A catalogue class's grid, as one of its hosts.
    CatalogueSystem {
        /// The class.
        class: CatalogueClassWire,
    },
}

/// What a supernova catalogue entry is at a time, tagged by `type` (plan 09's `SupernovaState`).
///
/// An entry has one ID on both sides of its explosion: before it, the living progenitor, whose
/// stars `system_summary` describes; from it, a supernova with its shell; once the shell has merged
/// with the gas, the bare remnant.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum SupernovaStateWire {
    /// Before the explosion.
    Progenitor {
        /// The time left until the explosion, in Julian years.
        until_explosion_yr: f64,
        /// The orbital period of a Type Ia's double white dwarf spiralling together, in seconds;
        /// `null` for any other progenitor.
        pair_period_s: Option<f64>,
    },
    /// A supernova and its expanding shell.
    Supernova {
        /// The time since the explosion, in Julian years.
        age_yr: f64,
        /// The shell.
        shell: ShellSummary,
        /// What the explosion left, and where.
        remnant: RemnantSummary,
    },
    /// The shell has merged with the gas; what the explosion left remains.
    BareRemnant {
        /// The time since the explosion, in Julian years.
        age_yr: f64,
        /// What the explosion left, and where.
        remnant: RemnantSummary,
    },
}

/// A supernova remnant's shell at one age (plan 09's `ShellState`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ShellSummary {
    /// The radius of its outer shock from the explosion site, in light-years.
    pub radius_ly: f64,
    /// The speed of its outer shock, in km/s.
    pub shock_speed_km_s: f64,
    /// Its phase.
    pub phase: ShellPhaseWire,
    /// What it shines in.
    pub emission: ShellEmissionWire,
}

/// A supernova remnant's shell's phase (plan 09's `ShellPhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ShellPhaseWire {
    /// The ejecta coast: the swept-up mass is still below theirs.
    FreeExpansion,
    /// The adiabatic blast wave.
    SedovTaylor,
    /// A cooled, dense shell pushed by the hot interior's pressure.
    PressureDrivenSnowplough,
    /// A dense shell coasting on its momentum.
    MomentumConservingSnowplough,
}

/// What a shell shines in, by its phase and its shock (plan 09's `ShellEmission`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ShellEmissionWire {
    /// Bright in X-rays and radio: free expansion and Sedov–Taylor.
    pub x_ray_and_radio: bool,
    /// Optical filaments: the shock is faster than plan 09's `OPTICAL_SHOCK_SPEED`, 70 km/s (the
    /// brainstorm's "Supernova remnants").
    pub optical: bool,
    /// A shell of neutral hydrogen seen at 21 cm: the two snowplough phases.
    pub neutral_hydrogen: bool,
}

/// What a supernova left, at one time after it (plan 09's `RemnantState`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RemnantSummary {
    /// How far the compact remnant is from the explosion site, its kick times the age, in
    /// light-years; `null` if the explosion left none, as a Type Ia leaves none.
    pub offset_ly: Option<f64>,
    /// Whether the compact remnant drives a bow shock, past plan 09's `BOW_SHOCK_SHARE` of its
    /// shell's radius, 0.68 (van der Swaluw et al. 2003, as P09.T16.b cites them); always `false`
    /// once the shell has merged.
    pub bow_shock: bool,
    /// Its pulsar's wind nebula, while the pulsar's spin-down powers one; `null` otherwise.
    pub nebula: Option<PulsarWindNebulaWire>,
    /// How far a Type Ia's surviving companion is from the site, its speed times the age, in
    /// light-years; `null` for any other explosion or one with no survivor.
    pub survivor_offset_ly: Option<f64>,
}

/// A pulsar's wind nebula (plan 09's `PulsarWindNebula`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PulsarWindNebulaWire {
    /// The pulsar's spin-down luminosity that powers it, in watts.
    pub power_w: f64,
}

/// Asks for the central black hole's state at a time (`black_hole_state`), answered with
/// [`BlackHoleState`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BlackHoleStateRequest {
    /// The universe whose black hole is described.
    pub universe: UniverseIdHex,
    /// The instant it is described at, within 1,000 years of the epoch.
    pub time: UniverseTime,
}

/// The central black hole at a time: its mass, its luminosity and the stars it has disrupted.
///
/// `time` echoes the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BlackHoleState {
    /// The universe described.
    pub universe: UniverseIdHex,
    /// The instant it is described at.
    pub time: UniverseTime,
    /// The black hole's ID, the galactic centre's member zero.
    pub id: SystemIdHex,
    /// Its mass, in M☉.
    pub mass_msun: f64,
    /// Its luminosity at the time, unabsorbed in the 2–10 keV X-ray band.
    ///
    /// Optional on the wire: a server that does not compute it leaves the key out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub luminosity: Option<BlackHoleLuminosityWire>,
    /// Its tidal disruptions at or before the time, newest first, back to the start of the source
    /// horizon.
    pub disruptions: Vec<TidalDisruptionWire>,
}

/// The central black hole's luminosity at one time, by part, in watts, unabsorbed in the 2–10 keV
/// X-ray band (plan 09's `CentreLuminosity`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BlackHoleLuminosityWire {
    /// Its quiescent emission, in watts.
    pub quiescent_w: f64,
    /// The flares running at the time, in watts.
    pub flares_w: f64,
    /// The debris tails of its earlier disruptions, in watts.
    pub tails_w: f64,
    /// The whole, in watts: the quiescence, the flares and the tails, added in that order.
    pub total_w: f64,
}

/// One tidal disruption by the central black hole (plan 09's `TidalDisruptionVictim`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TidalDisruptionWire {
    /// The star that was disrupted, an entry of the centre's feature-level list.
    pub victim: SystemIdHex,
    /// The instant of the disruption, its pericentre passage. It may lie before the clock window,
    /// back to the start of the source horizon.
    pub time: UniverseTime,
    /// The star's mass when it was disrupted, in M☉.
    pub star_mass_msun: f64,
    /// What became of it.
    pub outcome: DisruptionOutcomeWire,
    /// The peak luminosity of its debris tail, unabsorbed in the 2–10 keV X-ray band, in watts;
    /// `null` for a star swallowed whole, which leaves no debris.
    pub peak_luminosity_w: Option<f64>,
}

/// What became of a star that entered the black hole's loss cone (plan 09's `DisruptionOutcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DisruptionOutcomeWire {
    /// Torn apart at its tidal radius, outside the horizon: its debris falls back as a tail.
    Disrupted,
    /// The black hole's mass is at or above the star's Hills mass, so the star crossed the
    /// horizon whole.
    SwallowedWhole,
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::envelope::{RequestBody, ResponseBody};
    use crate::testing::{assert_wire_form, assert_wire_strings};

    const UNIVERSE: u64 = 0x0123_4567_89ab_cdef;

    /// Candidate 5 of feature cell (0, 0, 0), plan 01's example of a feature's word.
    pub(crate) const FEATURE: u64 = 0xe842_0002_8000_0000;

    fn universe() -> UniverseIdHex {
        UniverseIdHex::from_u64(UNIVERSE)
    }

    fn epoch_plus_a_century() -> UniverseTime {
        UniverseTime {
            seconds: 3_155_760_000,
            nanos: 0,
        }
    }

    fn near_the_sun() -> GalacticPosition {
        GalacticPosition {
            cell_ly: [26_000, 0, 50],
            offset_m: [0.0, 0.0, 4_730_365_236_290_400.0],
        }
    }

    fn near_the_sun_json() -> Value {
        json!({ "cell_ly": [26_000, 0, 50], "offset_m": [0.0, 0.0, 4_730_365_236_290_400.0] })
    }

    /// An embedded nursery with its bubble: every nullable field has a value.
    pub(crate) fn star_forming_region() -> FeatureSummary {
        FeatureSummary {
            id: FeatureIdHex::from_u64(FEATURE),
            designation: "AAA F5".to_owned(),
            kind: FeatureKindWire::StarFormingRegion,
            position: near_the_sun(),
            extent_ly: 42.5,
            age_myr: Some(2.25),
            mass_msun: 3_150.0,
            expected_members: 5_712.5,
            emission: Some(EmissionClassWire::HiiRegion),
            bubble_radius_ly: Some(61.0),
        }
    }

    fn star_forming_region_json() -> Value {
        json!({
            "id": "e842000280000000",
            "designation": "AAA F5",
            "kind": "star_forming_region",
            "position": near_the_sun_json(),
            "extent_ly": 42.5,
            "age_myr": 2.25,
            "mass_msun": 3_150.0,
            "expected_members": 5_712.5,
            "emission": "hii_region",
            "bubble_radius_ly": 61.0,
        })
    }

    /// A molecular cloud: no age, no members, no bubble.
    fn cloud() -> FeatureSummary {
        FeatureSummary {
            id: FeatureIdHex::from_u64(FEATURE + (1 << 31)),
            designation: "AAA F6".to_owned(),
            kind: FeatureKindWire::MolecularCloud,
            position: near_the_sun(),
            extent_ly: 180.0,
            age_myr: None,
            mass_msun: 2.5e5,
            expected_members: 0.0,
            emission: Some(EmissionClassWire::DarkCloud),
            bubble_radius_ly: None,
        }
    }

    /// The galactic centre as the map draws it, which shines as nothing but its stars.
    fn centre() -> FeatureSummary {
        FeatureSummary {
            id: FeatureIdHex::from_u64(0xf000_0000_0000_0000),
            designation: "AAA C0".to_owned(),
            kind: FeatureKindWire::GalacticCentre,
            position: GalacticPosition::default(),
            extent_ly: 1_000.0,
            age_myr: Some(10_000.0),
            mass_msun: 2.5e7,
            expected_members: 5.0e7,
            emission: None,
            bubble_radius_ly: None,
        }
    }

    fn sedov_shell() -> ShellSummary {
        ShellSummary {
            radius_ly: 12.5,
            shock_speed_km_s: 1_500.0,
            phase: ShellPhaseWire::SedovTaylor,
            emission: ShellEmissionWire {
                x_ray_and_radio: true,
                optical: true,
                neutral_hydrogen: false,
            },
        }
    }

    fn sedov_shell_json() -> Value {
        json!({
            "radius_ly": 12.5,
            "shock_speed_km_s": 1_500.0,
            "phase": "sedov_taylor",
            "emission": { "x_ray_and_radio": true, "optical": true, "neutral_hydrogen": false },
        })
    }

    /// A young pulsar's remnant, kicked off the site and blowing a nebula.
    fn pulsar_remnant() -> RemnantSummary {
        RemnantSummary {
            offset_ly: Some(2.5),
            bow_shock: false,
            nebula: Some(PulsarWindNebulaWire { power_w: 1.0e31 }),
            survivor_offset_ly: None,
        }
    }

    fn pulsar_remnant_json() -> Value {
        json!({
            "offset_ly": 2.5,
            "bow_shock": false,
            "nebula": { "power_w": 1.0e31 },
            "survivor_offset_ly": null,
        })
    }

    /// A core collapse 5,000 years after its explosion.
    pub(crate) fn young_supernova() -> SupernovaStateWire {
        SupernovaStateWire::Supernova {
            age_yr: 5_000.0,
            shell: sedov_shell(),
            remnant: pulsar_remnant(),
        }
    }

    pub(crate) fn young_supernova_json() -> Value {
        json!({
            "type": "supernova",
            "age_yr": 5_000.0,
            "shell": sedov_shell_json(),
            "remnant": pulsar_remnant_json(),
        })
    }

    #[test]
    fn features_in_range_request_wire_form() {
        assert_wire_form(
            &RequestBody::FeaturesInRange(FeaturesInRangeRequest {
                universe: universe(),
                centre: near_the_sun(),
                radius_ly: 1_000.0,
                time: epoch_plus_a_century(),
                kinds: vec![
                    FeatureKindWire::OpenCluster,
                    FeatureKindWire::MolecularCloud,
                ],
            }),
            json!({
                "kind": "features_in_range",
                "universe": "0123456789abcdef",
                "centre": near_the_sun_json(),
                "radius_ly": 1_000.0,
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "kinds": ["open_cluster", "molecular_cloud"],
            }),
        );
    }

    #[test]
    fn features_in_range_response_wire_form() {
        assert_wire_form(
            &ResponseBody::FeaturesInRange(FeaturesInRange {
                universe: universe(),
                centre: near_the_sun(),
                radius_ly: 1_000.0,
                time: epoch_plus_a_century(),
                census: vec![
                    FeatureKindCensus {
                        kind: FeatureKindWire::StarFormingRegion,
                        returned: 1,
                        status: FeatureKindStatus::Included,
                    },
                    FeatureKindCensus {
                        kind: FeatureKindWire::MolecularCloud,
                        returned: 1,
                        status: FeatureKindStatus::Included,
                    },
                    FeatureKindCensus {
                        kind: FeatureKindWire::Stream,
                        returned: 0,
                        status: FeatureKindStatus::NotGenerated,
                    },
                ],
                features: vec![star_forming_region(), cloud()],
            }),
            json!({
                "kind": "features_in_range",
                "universe": "0123456789abcdef",
                "centre": near_the_sun_json(),
                "radius_ly": 1_000.0,
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "census": [
                    { "kind": "star_forming_region", "returned": 1, "status": "included" },
                    { "kind": "molecular_cloud", "returned": 1, "status": "included" },
                    { "kind": "stream", "returned": 0, "status": "not_generated" },
                ],
                "features": [
                    star_forming_region_json(),
                    {
                        "id": "e842000300000000",
                        "designation": "AAA F6",
                        "kind": "molecular_cloud",
                        "position": near_the_sun_json(),
                        "extent_ly": 180.0,
                        "age_myr": null,
                        "mass_msun": 2.5e5,
                        "expected_members": 0.0,
                        "emission": "dark_cloud",
                        "bubble_radius_ly": null,
                    },
                ],
            }),
        );
    }

    #[test]
    fn features_in_range_response_wire_form_when_nothing_fits() {
        assert_wire_form(
            &ResponseBody::FeaturesInRange(FeaturesInRange {
                universe: universe(),
                centre: near_the_sun(),
                radius_ly: 131_072.0,
                time: UniverseTime::default(),
                census: vec![FeatureKindCensus {
                    kind: FeatureKindWire::Globular,
                    returned: 0,
                    status: FeatureKindStatus::OverCellBudget,
                }],
                features: Vec::new(),
            }),
            json!({
                "kind": "features_in_range",
                "universe": "0123456789abcdef",
                "centre": near_the_sun_json(),
                "radius_ly": 131_072.0,
                "time": { "seconds": 0, "nanos": 0 },
                "census": [{ "kind": "globular", "returned": 0, "status": "over_cell_budget" }],
                "features": [],
            }),
        );
    }

    #[test]
    fn galaxy_features_request_wire_form() {
        assert_wire_form(
            &RequestBody::GalaxyFeatures(GalaxyFeaturesRequest {
                universe: universe(),
                kinds: vec![
                    MapFeatureKindWire::Globular,
                    MapFeatureKindWire::GalacticCentre,
                    MapFeatureKindWire::BrightShell,
                ],
                time: UniverseTime::default(),
            }),
            json!({
                "kind": "galaxy_features",
                "universe": "0123456789abcdef",
                "kinds": ["globular", "galactic_centre", "bright_shell"],
                "time": { "seconds": 0, "nanos": 0 },
            }),
        );
    }

    /// Each list status, and each kind's entries: feature summaries, and shells.
    #[test]
    fn galaxy_features_response_wire_form() {
        assert_wire_form(
            &ResponseBody::GalaxyFeatures(GalaxyFeatures {
                universe: universe(),
                time: UniverseTime::default(),
                lists: vec![
                    MapFeatureList::GalacticCentre(MapFeatures::Complete {
                        features: vec![centre()],
                    }),
                    MapFeatureList::BrightShell(MapShells::Complete {
                        shells: vec![BrightShell {
                            system: SystemIdHex::from_u64(0xfc00_0000_0000_0003),
                            designation: "AAA X0".to_owned(),
                            site: near_the_sun(),
                            age_yr: 5_000.0,
                            shell: sedov_shell(),
                        }],
                    }),
                    MapFeatureList::Globular(MapFeatures::NotGenerated),
                    MapFeatureList::BrightShell(MapShells::NotGenerated),
                ],
            }),
            json!({
                "kind": "galaxy_features",
                "universe": "0123456789abcdef",
                "time": { "seconds": 0, "nanos": 0 },
                "lists": [
                    {
                        "kind": "galactic_centre",
                        "status": "complete",
                        "features": [{
                            "id": "f000000000000000",
                            "designation": "AAA C0",
                            "kind": "galactic_centre",
                            "position": { "cell_ly": [0, 0, 0], "offset_m": [0.0, 0.0, 0.0] },
                            "extent_ly": 1_000.0,
                            "age_myr": 10_000.0,
                            "mass_msun": 2.5e7,
                            "expected_members": 5.0e7,
                            "emission": null,
                            "bubble_radius_ly": null,
                        }],
                    },
                    {
                        "kind": "bright_shell",
                        "status": "complete",
                        "shells": [{
                            "system": "fc00000000000003",
                            "designation": "AAA X0",
                            "site": near_the_sun_json(),
                            "age_yr": 5_000.0,
                            "shell": sedov_shell_json(),
                        }],
                    },
                    { "kind": "globular", "status": "not_generated" },
                    { "kind": "bright_shell", "status": "not_generated" },
                ],
            }),
        );
    }

    #[test]
    fn feature_detail_request_wire_form() {
        assert_wire_form(
            &RequestBody::FeatureDetail(FeatureDetailRequest {
                universe: universe(),
                feature: FeatureIdHex::from_u64(FEATURE),
                time: epoch_plus_a_century(),
            }),
            json!({
                "kind": "feature_detail",
                "universe": "0123456789abcdef",
                "feature": "e842000280000000",
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
            }),
        );
    }

    /// A young cluster's detail, with a class of each member type: a cluster's, the centre's and a
    /// feature-level list's.
    #[test]
    fn feature_detail_response_wire_form_for_a_cluster() {
        let living = ClassCountWire {
            class: MemberClassWire::Cluster {
                kind: ClassKindWire::Living,
                band: MassLayer::A,
                generation: GenerationWire::First,
                multiplicity: MultiplicityWire::Binary,
            },
            initial_mass_min_msun: 0.08,
            initial_mass_max_msun: 0.5,
            mean_mass_msun: 0.375,
            expected: 1_250.5,
        };
        let listed = ClassCountWire {
            class: MemberClassWire::Listed {
                class: CatalogueClassWire::CoreCollapse,
            },
            initial_mass_min_msun: 8.0,
            initial_mass_max_msun: 150.0,
            mean_mass_msun: 12.5,
            expected: 0.25,
        };
        assert_wire_form(
            &ResponseBody::FeatureDetail(Box::new(FeatureDetail {
                universe: universe(),
                time: epoch_plus_a_century(),
                feature: star_forming_region(),
                structure: FeatureStructure::Cluster(ClusterStructure {
                    core_radius_ly: 1.5,
                    half_mass_radius_ly: 4.0,
                    tidal_radius_ly: 37.5,
                    central_escape_speed_km_s: 2.5,
                    relaxation_time_myr: 125.0,
                    core_collapsed: false,
                    black_holes: 0.75,
                    classes: vec![living, listed],
                }),
            })),
            json!({
                "kind": "feature_detail",
                "universe": "0123456789abcdef",
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
                "feature": star_forming_region_json(),
                "structure": {
                    "type": "cluster",
                    "core_radius_ly": 1.5,
                    "half_mass_radius_ly": 4.0,
                    "tidal_radius_ly": 37.5,
                    "central_escape_speed_km_s": 2.5,
                    "relaxation_time_myr": 125.0,
                    "core_collapsed": false,
                    "black_holes": 0.75,
                    "classes": [
                        {
                            "class": {
                                "type": "cluster",
                                "kind": "living",
                                "band": "a",
                                "generation": "first",
                                "multiplicity": "binary",
                            },
                            "initial_mass_min_msun": 0.08,
                            "initial_mass_max_msun": 0.5,
                            "mean_mass_msun": 0.375,
                            "expected": 1_250.5,
                        },
                        {
                            "class": { "type": "listed", "class": "core_collapse" },
                            "initial_mass_min_msun": 8.0,
                            "initial_mass_max_msun": 150.0,
                            "mean_mass_msun": 12.5,
                            "expected": 0.25,
                        },
                    ],
                },
            }),
        );
    }

    #[test]
    fn feature_detail_structure_wire_forms_for_the_centre_and_a_cloud() {
        assert_wire_form(
            &FeatureStructure::Centre(CentreStructure {
                influence_radius_ly: 10.0,
                loss_cone_radius_ly: 3.25e-5,
                classes: vec![ClassCountWire {
                    class: MemberClassWire::Centre {
                        kind: ClassKindWire::BlackHole,
                        band: MassLayer::E,
                        episode: 0,
                        tracer: CentreTracerWire::BlackHoles,
                    },
                    initial_mass_min_msun: 20.0,
                    initial_mass_max_msun: 150.0,
                    mean_mass_msun: 10.0,
                    expected: 2.0e4,
                }],
            }),
            json!({
                "type": "centre",
                "influence_radius_ly": 10.0,
                "loss_cone_radius_ly": 3.25e-5,
                "classes": [{
                    "class": {
                        "type": "centre",
                        "kind": "black_hole",
                        "band": "e",
                        "episode": 0,
                        "tracer": "black_holes",
                    },
                    "initial_mass_min_msun": 20.0,
                    "initial_mass_max_msun": 150.0,
                    "mean_mass_msun": 10.0,
                    "expected": 2.0e4,
                }],
            }),
        );
        assert_wire_form(
            &FeatureStructure::Cloud(CloudStructure {
                core_radius_ly: 25.0,
                central_density_per_cm3: 125.0,
            }),
            json!({ "type": "cloud", "core_radius_ly": 25.0, "central_density_per_cm3": 125.0 }),
        );
    }

    #[test]
    fn black_hole_state_request_wire_form() {
        assert_wire_form(
            &RequestBody::BlackHoleState(BlackHoleStateRequest {
                universe: universe(),
                time: epoch_plus_a_century(),
            }),
            json!({
                "kind": "black_hole_state",
                "universe": "0123456789abcdef",
                "time": { "seconds": 3_155_760_000_i64, "nanos": 0 },
            }),
        );
    }

    fn black_hole_state(luminosity: Option<BlackHoleLuminosityWire>) -> ResponseBody {
        ResponseBody::BlackHoleState(BlackHoleState {
            universe: universe(),
            time: UniverseTime::default(),
            id: SystemIdHex::from_u64(0xf000_0007_0000_0000),
            mass_msun: 4.25e6,
            luminosity,
            disruptions: vec![
                TidalDisruptionWire {
                    victim: SystemIdHex::from_u64(0xf000_0007_0000_0002),
                    time: UniverseTime {
                        seconds: -315_576_000_000,
                        nanos: 0,
                    },
                    star_mass_msun: 0.75,
                    outcome: DisruptionOutcomeWire::Disrupted,
                    peak_luminosity_w: Some(1.5e37),
                },
                TidalDisruptionWire {
                    victim: SystemIdHex::from_u64(0xf000_0007_0000_0001),
                    time: UniverseTime {
                        seconds: -3_155_760_000_000,
                        nanos: 0,
                    },
                    star_mass_msun: 12.5,
                    outcome: DisruptionOutcomeWire::SwallowedWhole,
                    peak_luminosity_w: None,
                },
            ],
        })
    }

    fn black_hole_state_json() -> Value {
        json!({
            "kind": "black_hole_state",
            "universe": "0123456789abcdef",
            "time": { "seconds": 0, "nanos": 0 },
            "id": "f000000700000000",
            "mass_msun": 4.25e6,
            "disruptions": [
                {
                    "victim": "f000000700000002",
                    "time": { "seconds": -315_576_000_000_i64, "nanos": 0 },
                    "star_mass_msun": 0.75,
                    "outcome": "disrupted",
                    "peak_luminosity_w": 1.5e37,
                },
                {
                    "victim": "f000000700000001",
                    "time": { "seconds": -3_155_760_000_000_i64, "nanos": 0 },
                    "star_mass_msun": 12.5,
                    "outcome": "swallowed_whole",
                    "peak_luminosity_w": null,
                },
            ],
        })
    }

    /// The luminosity is optional: a server that does not compute it leaves the key out.
    #[test]
    fn black_hole_state_response_wire_form_without_a_luminosity() {
        assert_wire_form(&black_hole_state(None), black_hole_state_json());
    }

    #[test]
    fn black_hole_state_response_wire_form_with_a_luminosity() {
        let mut wire = black_hole_state_json();
        wire["luminosity"] = json!({
            "quiescent_w": 2.5e26,
            "flares_w": 1.25e27,
            "tails_w": 0.0,
            "total_w": 1.5e27,
        });
        assert_wire_form(
            &black_hole_state(Some(BlackHoleLuminosityWire {
                quiescent_w: 2.5e26,
                flares_w: 1.25e27,
                tails_w: 0.0,
                total_w: 1.5e27,
            })),
            wire,
        );
    }

    #[test]
    fn system_origin_wire_forms() {
        assert_wire_form(&SystemOriginWire::Grid, json!({ "type": "grid" }));
        assert_wire_form(
            &SystemOriginWire::FeatureMember {
                feature: FeatureIdHex::from_u64(FEATURE),
            },
            json!({ "type": "feature_member", "feature": "e842000280000000" }),
        );
        assert_wire_form(
            &SystemOriginWire::CentreMember,
            json!({ "type": "centre_member" }),
        );
        assert_wire_form(
            &SystemOriginWire::CatalogueSystem {
                class: CatalogueClassWire::TypeIa,
            },
            json!({ "type": "catalogue_system", "class": "type_ia" }),
        );
    }

    #[test]
    fn supernova_state_wire_forms() {
        assert_wire_form(
            &SupernovaStateWire::Progenitor {
                until_explosion_yr: 1_250.5,
                pair_period_s: None,
            },
            json!({ "type": "progenitor", "until_explosion_yr": 1_250.5, "pair_period_s": null }),
        );
        assert_wire_form(
            &SupernovaStateWire::Progenitor {
                until_explosion_yr: 12.5,
                pair_period_s: Some(37.5),
            },
            json!({ "type": "progenitor", "until_explosion_yr": 12.5, "pair_period_s": 37.5 }),
        );
        assert_wire_form(&young_supernova(), young_supernova_json());
        // A Type Ia's: no compact remnant, and its survivor running from the site.
        assert_wire_form(
            &SupernovaStateWire::BareRemnant {
                age_yr: 4.5e6,
                remnant: RemnantSummary {
                    offset_ly: None,
                    bow_shock: false,
                    nebula: None,
                    survivor_offset_ly: Some(4_600.0),
                },
            },
            json!({
                "type": "bare_remnant",
                "age_yr": 4.5e6,
                "remnant": {
                    "offset_ly": null,
                    "bow_shock": false,
                    "nebula": null,
                    "survivor_offset_ly": 4_600.0,
                },
            }),
        );
    }

    #[test]
    fn feature_kind_strings() {
        assert_wire_strings(&[
            (FeatureKindWire::Globular, "globular"),
            (FeatureKindWire::OpenCluster, "open_cluster"),
            (FeatureKindWire::Association, "association"),
            (FeatureKindWire::StarFormingRegion, "star_forming_region"),
            (FeatureKindWire::MolecularCloud, "molecular_cloud"),
            (FeatureKindWire::GalacticCentre, "galactic_centre"),
            (FeatureKindWire::Stream, "stream"),
            (FeatureKindWire::DwarfCore, "dwarf_core"),
        ]);
        assert_wire_strings(&[
            (FeatureKindStatus::Included, "included"),
            (FeatureKindStatus::OverCellBudget, "over_cell_budget"),
            (FeatureKindStatus::NotGenerated, "not_generated"),
        ]);
        assert_wire_strings(&[
            (MapFeatureKindWire::Globular, "globular"),
            (MapFeatureKindWire::GalacticCentre, "galactic_centre"),
            (MapFeatureKindWire::BrightShell, "bright_shell"),
        ]);
    }

    #[test]
    fn emission_and_shell_strings() {
        assert_wire_strings(&[
            (EmissionClassWire::HiiRegion, "hii_region"),
            (EmissionClassWire::ReflectionNebula, "reflection_nebula"),
            (EmissionClassWire::DarkCloud, "dark_cloud"),
            (EmissionClassWire::RemnantShell, "remnant_shell"),
            (EmissionClassWire::PulsarWindNebula, "pulsar_wind_nebula"),
        ]);
        assert_wire_strings(&[
            (ShellPhaseWire::FreeExpansion, "free_expansion"),
            (ShellPhaseWire::SedovTaylor, "sedov_taylor"),
            (
                ShellPhaseWire::PressureDrivenSnowplough,
                "pressure_driven_snowplough",
            ),
            (
                ShellPhaseWire::MomentumConservingSnowplough,
                "momentum_conserving_snowplough",
            ),
        ]);
        assert_wire_strings(&[
            (DisruptionOutcomeWire::Disrupted, "disrupted"),
            (DisruptionOutcomeWire::SwallowedWhole, "swallowed_whole"),
        ]);
    }

    #[test]
    fn member_class_strings() {
        assert_wire_strings(&[
            (ClassKindWire::Living, "living"),
            (ClassKindWire::WhiteDwarf, "white_dwarf"),
            (ClassKindWire::NeutronStar, "neutron_star"),
            (ClassKindWire::BlackHole, "black_hole"),
            (ClassKindWire::Tail, "tail"),
        ]);
        assert_wire_strings(&[
            (GenerationWire::First, "first"),
            (GenerationWire::Second, "second"),
        ]);
        assert_wire_strings(&[
            (MultiplicityWire::Single, "single"),
            (MultiplicityWire::Binary, "binary"),
        ]);
        assert_wire_strings(&[
            (CentreTracerWire::Stars, "stars"),
            (CentreTracerWire::BlackHoles, "black_holes"),
            (CentreTracerWire::YoungDisc, "young_disc"),
            (CentreTracerWire::YoungIsotropic, "young_isotropic"),
        ]);
    }

    /// The registry's classes in value order, 0–8.
    #[test]
    fn catalogue_class_strings() {
        assert_wire_strings(&[
            (CatalogueClassWire::CoreCollapse, "core_collapse"),
            (CatalogueClassWire::TypeIa, "type_ia"),
            (CatalogueClassWire::StellarMerger, "stellar_merger"),
            (CatalogueClassWire::NeutronStarMerger, "neutron_star_merger"),
            (
                CatalogueClassWire::LuminousBlueVariable,
                "luminous_blue_variable",
            ),
            (CatalogueClassWire::XrayBinary, "xray_binary"),
            (
                CatalogueClassWire::AccretingWhiteDwarf,
                "accreting_white_dwarf",
            ),
            (
                CatalogueClassWire::TidalDisruptionVictim,
                "tidal_disruption_victim",
            ),
            (
                CatalogueClassWire::AccretingWhiteDwarfSlow,
                "accreting_white_dwarf_slow",
            ),
        ]);
    }
}
