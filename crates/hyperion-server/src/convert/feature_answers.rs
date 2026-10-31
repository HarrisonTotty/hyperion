//! Plan 09's four feature requests, checked, and their answers (P09.T40.b): `features_in_range`,
//! `galaxy_features`, `feature_detail` and `black_hole_state`.
//!
//! Every field is checked in design note 24's order once the universe has been (by
//! `requests::universe::openable_universe`): `time` first, as a range query checks it, then the
//! request's own fields in their order, each failure a `bad_request` naming the field. A feature is
//! then described at the request's time from the sim's own models (P09.T39's table of sources):
//!
//! - **The kind** is the record's at the time ([`FeatureRecord::kind_at`]); a feature unborn or
//!   dissolved then is not listed, and a detail of one is `unknown_feature`.
//! - **The position** is its epoch position moved by its bulk velocity for the time since the epoch
//!   ([`bulk_velocity`]); a cloud has none and stays where it is.
//! - **The extent** is the sphere a chart draws: a cluster's tidal radius
//!   ([`ClusterModel::tidal_radius`]), a nursery's half size at its age when it is no cluster, a
//!   cloud's half-mass radius ([`CloudMarks::radius`]), never its reach, which bounds it over all
//!   time.
//! - **The mass** is its stars' present mass (a globular's today, an open cluster's at the time, a
//!   nursery's at its age) or a cloud's gas.
//! - **The expected members** are its class counts' sum, tails included
//!   ([`class_counts`]), from its cluster model at the epoch, which is what its members are drawn
//!   from; none for a feature with no cluster model.
//! - **The emission** is [`EmissionClass::of`] at the time, and **the bubble** a nursery's
//!   [`Superbubble`] at its age.
//!
//! The galactic centre is a feature of its own, named on the wire by the word
//! [`CENTRE_FEATURE_WORD`] (P09.T39): its nuclear cluster inside the radius its mass is normalised
//! to, about its black hole at the galactic origin, which does not move.

use hyperion_protocol::{
    AssociationStructure, BlackHoleLuminosityWire, BlackHoleState, BlackHoleStateRequest,
    CentreStructure, CentreTracerWire, ClassCountWire, CloudStructure, ClusterStructure,
    DisruptionOutcomeWire, EmissionClassWire, ErrorCode, FeatureDetail, FeatureDetailRequest,
    FeatureIdHex, FeatureKindCensus, FeatureKindStatus, FeatureKindWire, FeatureStructure,
    FeatureSummary, FeaturesInRange, FeaturesInRangeRequest, GalaxyFeatures, GalaxyFeaturesRequest,
    MapFeatureKindWire, MapFeatureList, MapFeatures, MapShells, MemberClassWire, RequestError,
    SystemIdHex, TidalDisruptionWire,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::features::catalogue::{
    FeatureCatalogue, FeatureCellCache, FeatureMarks, FeatureRecord, MAX_FEATURE_REACH,
    bulk_velocity,
};
use hyperion_sim::galaxy::features::centre::events::{
    CentreLuminosityModel, DisruptionOutcome, DisruptionTail, luminosity_at,
};
use hyperion_sim::galaxy::features::centre::{
    BuildCentreError, CentreClass, CentreClassKind, CentreModel, CentreTracer,
};
use hyperion_sim::galaxy::features::cluster::ClusterModel;
use hyperion_sim::galaxy::features::emission::EmissionClass;
use hyperion_sim::galaxy::features::interior::counts::{ClassCounts, class_counts};
use hyperion_sim::galaxy::features::interior::{ClassKind, MemberClass};
use hyperion_sim::galaxy::features::kinds::cloud::{
    PLUMMER_HALF_MASS_RATIO, plummer_central_density,
};
use hyperion_sim::galaxy::features::kinds::{NurseryMarks, NurseryStage, Superbubble};
use hyperion_sim::galaxy::features::{FeatureId, FeatureKind};
use hyperion_sim::galaxy::params::NuclearClusterParams;
use hyperion_sim::galaxy::query::{UNBOUND_PAD_SPEED, pad_for};
use hyperion_sim::id::{CentreMemberId, FeatureCell, SystemId, SystemIdKind};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, Megayears, Seconds, SolarMasses, Years};

use super::features::{class_kind, feature_id_hex, member_class};
use super::stellar::wire_time;
use super::{ConvertRequestError, galactic_position, mass_layer, query_centre, query_radius};
use crate::compute::{MapGlobulars, counted_members, expected_members_of};
use crate::limits::MAX_FEATURE_CELLS;

/// The wire's word for the galactic centre as a feature, `f000000000000000` (P09.T39): its
/// members' common word with their member bits zeroed, as a catalogue feature's is, though the sim
/// names no such feature.
pub(crate) const CENTRE_FEATURE_WORD: u64 = 0xF000_0000_0000_0000;

/// The galactic centre's designation: its feature-level members' (`CENTRE M0` for its black hole)
/// without their slot, as a catalogue feature's is its members' without theirs.
const CENTRE_DESIGNATION: &str = "CENTRE";

/// A feature a client named: decoded, not yet resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeatureTarget {
    /// A feature of the catalogue.
    Catalogue(FeatureId),
    /// The galactic centre.
    Centre,
}

/// The feature `hex` names, refused naming `field` unless its word is a feature's: the galactic
/// centre's word exactly, or a catalogue feature's, the word of a feature member's ID with its
/// member bits zeroed.
///
/// A catalogue feature's word is also a well-formed member ID (a zero slot is band A's member 0
/// of nested cell 0), so a word is decoded as a member's and refused unless its feature gives it
/// back.
///
/// # Errors
///
/// [`ConvertRequestError`] naming `field` for a word that is no feature's.
pub(crate) fn feature_target(
    field: &'static str,
    hex: &FeatureIdHex,
) -> Result<FeatureTarget, ConvertRequestError> {
    let word = hex.to_u64();
    if word == CENTRE_FEATURE_WORD {
        return Ok(FeatureTarget::Centre);
    }
    let refused = || {
        ConvertRequestError::new(
            field,
            format!(
                "{} is not a feature's ID: a feature's is its members' word with their member \
                 bits zeroed, or the galactic centre's {CENTRE_FEATURE_WORD:016x}",
                hex.as_str()
            ),
        )
    };
    let Ok(id) = SystemId::from_raw(word) else {
        return Err(refused());
    };
    let SystemIdKind::FeatureMember(member) = id.kind() else {
        return Err(refused());
    };
    let feature = FeatureId::new(member.feature());
    if feature.object_word() != word {
        return Err(refused());
    }
    Ok(FeatureTarget::Catalogue(feature))
}

/// Checks a request's `kinds`: at least one, each at most once, refused naming `kinds`.
fn checked_kinds<T: Copy + PartialEq + std::fmt::Debug>(
    kinds: &[T],
) -> Result<(), ConvertRequestError> {
    if kinds.is_empty() {
        return Err(ConvertRequestError::new(
            "kinds",
            "at least one kind must be asked for",
        ));
    }
    for (i, kind) in kinds.iter().enumerate() {
        if kinds[..i].contains(kind) {
            return Err(ConvertRequestError::new(
                "kinds",
                format!("{kind:?} is asked for twice"),
            ));
        }
    }
    Ok(())
}

/// A `features_in_range` request, checked: the sphere and the time. Its kinds are checked and
/// read from the request itself.
///
/// Checked in design note 24's order, as `systems_in_range` is: `time`, `centre`, `radius_ly` (as a
/// range query's radius), then `kinds`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FeatureRangeQuery {
    time: UniverseTime,
    centre: GalacticPosition,
    radius: LightYears,
}

impl TryFrom<&FeaturesInRangeRequest> for FeatureRangeQuery {
    type Error = ConvertRequestError;

    fn try_from(request: &FeaturesInRangeRequest) -> Result<Self, Self::Error> {
        let time = super::query_time(&request.time)?;
        let centre = query_centre(&request.centre)?;
        let radius = query_radius(request.radius_ly)?;
        checked_kinds(&request.kinds)?;
        Ok(Self {
            time,
            centre,
            radius,
        })
    }
}

/// A `galaxy_features` request, checked: `time`, then `kinds`.
///
/// # Errors
///
/// [`ConvertRequestError`] naming the first field at fault.
pub(crate) fn galaxy_features_time(
    request: &GalaxyFeaturesRequest,
) -> Result<UniverseTime, ConvertRequestError> {
    let time = super::query_time(&request.time)?;
    checked_kinds(&request.kinds)?;
    Ok(time)
}

/// A `feature_detail` request, checked: `time`, then `feature`, decoded but not yet resolved.
///
/// # Errors
///
/// [`ConvertRequestError`] naming the first field at fault.
pub(crate) fn feature_detail_target(
    request: &FeatureDetailRequest,
) -> Result<(UniverseTime, FeatureTarget), ConvertRequestError> {
    let time = super::query_time(&request.time)?;
    let target = feature_target("feature", &request.feature)?;
    Ok((time, target))
}

/// A `black_hole_state` request, checked: its `time`.
///
/// # Errors
///
/// [`ConvertRequestError`] naming `time` for a time outside the clock window.
pub(crate) fn black_hole_time(
    request: &BlackHoleStateRequest,
) -> Result<UniverseTime, ConvertRequestError> {
    super::query_time(&request.time)
}

/// The time `t` as years after the epoch, as plan 09's feature models take it.
#[must_use]
fn years_after_epoch(t: UniverseTime) -> Years {
    Years::new(t.since_epoch().as_julian_years_f64())
}

/// The feature cells [`FeatureCatalogue::near`] may read for a sphere of `radius` about `centre`.
///
/// They are the cells of the box it walks, each axis from the cell of the sphere's least
/// coordinate less [`MAX_FEATURE_REACH`] to that of its greatest plus it, clamped to the root cube.
/// It counts every cell of the box, of which the lookup skips the corners, so it errs high: toward
/// a kind refused as over the budget, never toward a lookup past it.
#[must_use]
pub(crate) fn feature_cells_near(centre: &GalacticPosition, radius: LightYears) -> u64 {
    let edge = f64::from(FeatureCell::EDGE_LY);
    let reach = radius.value() + MAX_FEATURE_REACH.value();
    centre
        .to_light_years_f64()
        .iter()
        .map(|&c| {
            let lo = ((c - reach) / edge).floor().max(-16.0);
            let hi = ((c + reach) / edge).floor().min(15.0);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole number of cells from 0 to 32"
            )]
            let cells = (hi - lo + 1.0).max(0.0) as u64;
            cells
        })
        .product()
}

/// The answer to `features_in_range`: the request's terms, a census line per requested kind, and
/// every feature of the included kinds whose sphere meets the query's, nearest first, then by ID.
///
/// A feature's sphere is of `extent_ly` about its position at the time. The catalogue's features
/// are looked up through `cells` about the sphere padded by the furthest any feature's bulk motion
/// carries it by then ([`UNBOUND_PAD_SPEED`]); each is then placed at the time, and only one of a
/// wanted kind that meets the sphere has its classes counted. If the lookup would read more than
/// [`MAX_FEATURE_CELLS`] cells, every kind of the catalogue's is left out as over the cell budget,
/// unread; the galactic centre, which is no cell's, is still answered. Streams and dwarf cores are
/// plan 10's global list's, never placed here: not generated.
///
/// # Errors
///
/// The [`BuildCentreError`] of the galaxy's centre if the galactic centre is asked for and its
/// model cannot be built.
pub(crate) fn features_in_range(
    galaxy: &Galaxy,
    cells: &dyn FeatureCellCache,
    request: FeaturesInRangeRequest,
    query: FeatureRangeQuery,
) -> Result<FeaturesInRange, BuildCentreError> {
    let t = query.time;
    let catalogue_kinds = request.kinds.iter().any(|&kind| is_catalogue_kind(kind));
    let padded = query.radius + pad_for(t, UNBOUND_PAD_SPEED);
    let over_budget =
        catalogue_kinds && feature_cells_near(&query.centre, padded) > MAX_FEATURE_CELLS;
    let mut found: Vec<(f64, u64, FeatureSummary)> = Vec::new();
    if catalogue_kinds && !over_budget {
        for record in FeatureCatalogue::near(galaxy, &query.centre, padded, cells) {
            let Some(placed) = Placed::of(galaxy, &record, t) else {
                continue;
            };
            if !request.kinds.contains(&feature_kind(placed.kind)) {
                continue;
            }
            let away = LightYears::from(query.centre.distance_to(&placed.position)).value();
            if away <= query.radius.value() + placed.extent.value() {
                let expected = placed.model.as_ref().map_or(0.0, |model| {
                    expected_members_of(galaxy, model, record.reach())
                });
                let summary = placed.summary(galaxy, &record, t, expected);
                found.push((away, record.id().object_word(), summary));
            }
        }
    }
    if request.kinds.contains(&FeatureKindWire::GalacticCentre) {
        let summary = centre_summary(galaxy.centre()?, galaxy);
        let away = LightYears::from(query.centre.distance_to(&GalacticPosition::ORIGIN)).value();
        if away <= query.radius.value() + summary.extent_ly {
            found.push((away, CENTRE_FEATURE_WORD, summary));
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let census = request
        .kinds
        .iter()
        .map(|&kind| {
            let status = match kind {
                FeatureKindWire::Stream | FeatureKindWire::DwarfCore => {
                    FeatureKindStatus::NotGenerated
                }
                FeatureKindWire::GalacticCentre => FeatureKindStatus::Included,
                FeatureKindWire::Globular
                | FeatureKindWire::OpenCluster
                | FeatureKindWire::Association
                | FeatureKindWire::StarFormingRegion
                | FeatureKindWire::MolecularCloud => {
                    if over_budget {
                        FeatureKindStatus::OverCellBudget
                    } else {
                        FeatureKindStatus::Included
                    }
                }
            };
            let returned = found.iter().filter(|entry| entry.2.kind == kind).count();
            FeatureKindCensus {
                kind,
                returned: u32::try_from(returned).unwrap_or(u32::MAX),
                status,
            }
        })
        .collect();
    Ok(FeaturesInRange {
        universe: request.universe,
        centre: request.centre,
        radius_ly: request.radius_ly,
        time: request.time,
        census,
        features: found.into_iter().map(|entry| entry.2).collect(),
    })
}

/// Whether the catalogue places features of `kind`: every kind but the galactic centre and plan
/// 10's streams and dwarf cores.
#[must_use]
fn is_catalogue_kind(kind: FeatureKindWire) -> bool {
    match kind {
        FeatureKindWire::Globular
        | FeatureKindWire::OpenCluster
        | FeatureKindWire::Association
        | FeatureKindWire::StarFormingRegion
        | FeatureKindWire::MolecularCloud => true,
        FeatureKindWire::GalacticCentre | FeatureKindWire::Stream | FeatureKindWire::DwarfCore => {
            false
        }
    }
}

/// The wire's kind.
#[must_use]
fn feature_kind(kind: FeatureKind) -> FeatureKindWire {
    match kind {
        FeatureKind::Globular => FeatureKindWire::Globular,
        FeatureKind::OpenCluster => FeatureKindWire::OpenCluster,
        FeatureKind::Association => FeatureKindWire::Association,
        FeatureKind::StarFormingRegion => FeatureKindWire::StarFormingRegion,
        FeatureKind::MolecularCloud => FeatureKindWire::MolecularCloud,
        FeatureKind::GalacticCentre => FeatureKindWire::GalacticCentre,
        FeatureKind::Stream => FeatureKindWire::Stream,
        FeatureKind::DwarfCore => FeatureKindWire::DwarfCore,
    }
}

/// The wire's emission class.
#[must_use]
fn emission_class(class: EmissionClass) -> EmissionClassWire {
    match class {
        EmissionClass::HiiRegion => EmissionClassWire::HiiRegion,
        EmissionClass::ReflectionNebula => EmissionClassWire::ReflectionNebula,
        EmissionClass::DarkCloud => EmissionClassWire::DarkCloud,
        EmissionClass::RemnantShell => EmissionClassWire::RemnantShell,
        EmissionClass::PulsarWindNebula => EmissionClassWire::PulsarWindNebula,
    }
}

/// A catalogue feature at a time, as far as a range's test reads it: what it is, where, how far it
/// reaches, how old it is, and its cluster model at the epoch, if it has one.
///
/// Each part is cheap; the class counts, about a millisecond a cluster, wait for a feature that
/// passes the test ([`summary`](Self::summary)).
#[derive(Debug, Clone, PartialEq)]
struct Placed {
    kind: FeatureKind,
    stage: Option<NurseryStage>,
    position: GalacticPosition,
    extent: LightYears,
    age: Option<Years>,
    model: Option<ClusterModel>,
}

impl Placed {
    /// `record` at `t`, or `None` if it does not exist then: not yet born, or dissolved.
    #[must_use]
    fn of(galaxy: &Galaxy, record: &FeatureRecord, t: UniverseTime) -> Option<Self> {
        let years = years_after_epoch(t);
        let kind = record.kind_at(years)?;
        let stage = match record.marks() {
            FeatureMarks::Nursery(marks) => Some(marks.stage_at(years)),
            FeatureMarks::Globular(_) | FeatureMarks::OpenCluster(_) | FeatureMarks::Cloud(_) => {
                None
            }
        };
        let model = ClusterModel::from_record(galaxy, record);
        let age = record.age_at_epoch().map(|age| age + years);
        Some(Self {
            kind,
            stage,
            position: position_at(galaxy, record, t),
            extent: extent(record, stage, model.as_ref(), age),
            age,
            model,
        })
    }

    /// The feature `record`, placed so at `t`, as a list or a readout shows it (module
    /// documentation), with `expected` members.
    #[must_use]
    fn summary(
        &self,
        galaxy: &Galaxy,
        record: &FeatureRecord,
        t: UniverseTime,
        expected: f64,
    ) -> FeatureSummary {
        let years = years_after_epoch(t);
        FeatureSummary {
            id: feature_id_hex(record.id()),
            designation: record.id().designation().to_string(),
            kind: feature_kind(self.kind),
            position: galactic_position(&self.position),
            extent_ly: self.extent.value(),
            age_myr: self.age.map(|age| Megayears::from(age).value()),
            mass_msun: present_mass(record, years, self.age).value(),
            expected_members: expected,
            emission: EmissionClass::of(galaxy, record, years).map(emission_class),
            bubble_radius_ly: match record.marks() {
                FeatureMarks::Nursery(marks) => self
                    .age
                    .and_then(|age| bubble(galaxy, record, marks, age))
                    .map(|bubble| bubble.radius().value()),
                FeatureMarks::Globular(_)
                | FeatureMarks::OpenCluster(_)
                | FeatureMarks::Cloud(_) => None,
            },
        }
    }
}

/// A catalogue feature at `t`, as a list or a readout shows it (module documentation), or `None`
/// for one that does not exist then: not yet born or dissolved. What the tests hold every answer's
/// summaries to.
#[cfg(test)]
#[must_use]
pub(crate) fn feature_summary(
    galaxy: &Galaxy,
    record: &FeatureRecord,
    t: UniverseTime,
) -> Option<FeatureSummary> {
    let placed = Placed::of(galaxy, record, t)?;
    let expected = placed.model.as_ref().map_or(0.0, |model| {
        expected_members_of(galaxy, model, record.reach())
    });
    Some(placed.summary(galaxy, record, t, expected))
}

/// Where `record` is at `t`: its epoch position moved by its bulk velocity, or its epoch position
/// for a cloud, which has none.
#[must_use]
fn position_at(galaxy: &Galaxy, record: &FeatureRecord, t: UniverseTime) -> GalacticPosition {
    let epoch = *record.position();
    let Some(velocity) = bulk_velocity(galaxy, record) else {
        return epoch;
    };
    let elapsed = Seconds::new(t.since_epoch().as_seconds_f64());
    // A feature moves some ten light-years at most over the clock window, so it stays inside the
    // root cube; were it to leave, it is drawn where it was.
    epoch
        .translated(velocity.displacement_over(elapsed))
        .unwrap_or(epoch)
}

/// The radius a chart draws `record` at, of nursery stage `stage` if it is a nursery: a cluster's
/// tidal radius, a nursery that is no cluster its half size at its age `age`, a cloud its
/// half-mass radius.
#[must_use]
fn extent(
    record: &FeatureRecord,
    stage: Option<NurseryStage>,
    model: Option<&ClusterModel>,
    age: Option<Years>,
) -> LightYears {
    match record.marks() {
        FeatureMarks::Cloud(cloud) => cloud.radius(),
        FeatureMarks::Globular(marks) => {
            model.map_or_else(|| marks.half_mass_radius(), ClusterModel::tidal_radius)
        }
        FeatureMarks::OpenCluster(marks) => {
            model.map_or_else(|| marks.half_mass_radius(), ClusterModel::tidal_radius)
        }
        FeatureMarks::Nursery(marks) => {
            let half_size = || marks.size_at(age.unwrap_or(marks.age_at_epoch())) * 0.5;
            match (stage, model) {
                (Some(NurseryStage::BoundCluster), Some(model)) => model.tidal_radius(),
                (
                    Some(
                        NurseryStage::BoundCluster
                        | NurseryStage::Embedded
                        | NurseryStage::Association
                        | NurseryStage::Unborn
                        | NurseryStage::Dissolved,
                    )
                    | None,
                    _,
                ) => half_size(),
            }
        }
    }
}

/// `record`'s present mass `years` after the epoch: its stars' for a cluster or a nursery, at age
/// `age`, a cloud's gas.
#[must_use]
fn present_mass(record: &FeatureRecord, years: Years, age: Option<Years>) -> SolarMasses {
    match record.marks() {
        FeatureMarks::Globular(marks) => marks.mass(),
        FeatureMarks::OpenCluster(marks) => marks.present_mass(years),
        FeatureMarks::Nursery(marks) => marks.stellar_mass_at(age.unwrap_or(marks.age_at_epoch())),
        FeatureMarks::Cloud(cloud) => cloud.mass(),
    }
}

/// A nursery's superbubble at age `age`, as the features' gas places it.
#[must_use]
fn bubble(
    galaxy: &Galaxy,
    record: &FeatureRecord,
    marks: &NurseryMarks,
    age: Years,
) -> Option<Superbubble> {
    let gas = galaxy.gas();
    Superbubble::of(
        marks,
        age,
        gas.mean_density(record.position()),
        gas.params().neutral_height(),
        galaxy.feature_shares().supernova_clock(),
    )
}

/// The galactic centre, as a list or a readout shows it at any time.
///
/// It is its nuclear cluster inside the radius its mass is normalised to
/// ([`NuclearClusterParams::NORMALISATION_RADIUS`]), 128 ly, about the black hole at the galactic
/// origin, with that mass and the systems its classes expect. It has no single age (its stars are
/// of six episodes, from 10–13 Gyr to a few Myr), so its age is null; and no emission class or
/// bubble of plan 09's.
#[must_use]
pub(crate) fn centre_summary(centre: &CentreModel, galaxy: &Galaxy) -> FeatureSummary {
    FeatureSummary {
        id: FeatureIdHex::from_u64(CENTRE_FEATURE_WORD),
        designation: CENTRE_DESIGNATION.to_owned(),
        kind: FeatureKindWire::GalacticCentre,
        position: galactic_position(&GalacticPosition::ORIGIN),
        extent_ly: NuclearClusterParams::NORMALISATION_RADIUS.value(),
        age_myr: None,
        mass_msun: galaxy.params().nuclear_cluster().mass().value(),
        expected_members: centre.classes().systems(),
        emission: None,
        bubble_radius_ly: None,
    }
}

/// The answer to `feature_detail`: the feature `target` names at `t`.
///
/// # Errors
///
/// `unknown_feature` naming `feature` for a catalogue feature's ID that names none in `galaxy`, or
/// none at `t` (not yet born, or dissolved); `internal` if the galactic centre is asked for and its
/// model cannot be built.
pub(crate) fn detail_of(
    galaxy: &Galaxy,
    request: FeatureDetailRequest,
    target: FeatureTarget,
    t: UniverseTime,
) -> Result<FeatureDetail, RequestError> {
    let (feature, structure) = match target {
        FeatureTarget::Centre => {
            let centre = galaxy.centre().map_err(centre_not_built)?;
            (
                centre_summary(centre, galaxy),
                centre_structure(centre, galaxy),
            )
        }
        FeatureTarget::Catalogue(id) => FeatureCatalogue::resolve(galaxy, id)
            .and_then(|record| catalogue_detail(galaxy, &record, t))
            .ok_or_else(|| unknown_feature(&request.feature))?,
    };
    Ok(FeatureDetail {
        universe: request.universe,
        time: request.time,
        feature,
        structure,
    })
}

/// The refusal of a well-formed feature ID that names no feature of the universe at the time.
#[must_use]
fn unknown_feature(feature: &FeatureIdHex) -> RequestError {
    RequestError {
        code: ErrorCode::UnknownFeature,
        message: format!(
            "feature {} names no feature of this universe at this time",
            feature.as_str()
        ),
        field: Some("feature".to_owned()),
    }
}

/// The failure of a galaxy's centre to build, which is the server's own: `internal`.
#[must_use]
pub(crate) fn centre_not_built(error: BuildCentreError) -> RequestError {
    RequestError {
        code: ErrorCode::Internal,
        message: format!("the galactic centre could not be built: {error}"),
        field: None,
    }
}

/// The catalogue feature `record`'s summary and structure at `t`, or `None` if it does not exist
/// then.
///
/// A globular or an open cluster is a cluster, its classes its class counts. A cloud is its
/// Plummer profile. A nursery is a cluster while it has a cluster model (bound at the epoch) and
/// is not yet an association; without one, while it holds gas it is its gas, the Plummer ball the
/// features' gas places (half its mass within half its size), as a cloud, and otherwise an
/// association, its size and its expansion.
#[must_use]
fn catalogue_detail(
    galaxy: &Galaxy,
    record: &FeatureRecord,
    t: UniverseTime,
) -> Option<(FeatureSummary, FeatureStructure)> {
    let placed = Placed::of(galaxy, record, t)?;
    let counted = placed
        .model
        .as_ref()
        .map(|model| (model, class_counts(galaxy, model, record.reach())));
    let expected = counted
        .as_ref()
        .map_or(0.0, |(_, counts)| counted_members(counts));
    let summary = placed.summary(galaxy, record, t, expected);
    let cluster = |(model, counts): (&ClusterModel, ClassCounts)| {
        FeatureStructure::Cluster(cluster_structure(model, &counts))
    };
    let structure = match (record.marks(), counted) {
        (FeatureMarks::Cloud(cloud), _) => FeatureStructure::Cloud(CloudStructure {
            core_radius_ly: cloud.core_radius().value(),
            central_density_per_cm3: cloud.central_density().value(),
        }),
        (FeatureMarks::Globular(_) | FeatureMarks::OpenCluster(_), Some(counted)) => {
            cluster(counted)
        }
        // A globular or an old open cluster has a model whenever it exists at all.
        (FeatureMarks::Globular(_) | FeatureMarks::OpenCluster(_), None) => return None,
        (FeatureMarks::Nursery(marks), counted) => match (placed.stage, counted) {
            (Some(NurseryStage::Embedded | NurseryStage::BoundCluster), Some(counted)) => {
                cluster(counted)
            }
            (
                Some(
                    NurseryStage::Embedded
                    | NurseryStage::BoundCluster
                    | NurseryStage::Association
                    | NurseryStage::Unborn
                    | NurseryStage::Dissolved,
                )
                | None,
                _,
            ) => unbound_nursery(marks, placed.age, &summary),
        },
    };
    Some((summary, structure))
}

/// A nursery with no cluster model, or one that has become an association: while it holds gas,
/// its gas as a cloud; otherwise an association.
#[must_use]
fn unbound_nursery(
    marks: &NurseryMarks,
    age: Option<Years>,
    summary: &FeatureSummary,
) -> FeatureStructure {
    let age = age.unwrap_or(marks.age_at_epoch());
    let gas = marks.gas_mass_at(age);
    if gas.value() > 0.0 {
        let core = marks.size_at(age) * (0.5 / PLUMMER_HALF_MASS_RATIO);
        FeatureStructure::Cloud(CloudStructure {
            core_radius_ly: core.value(),
            central_density_per_cm3: plummer_central_density(gas, core).value(),
        })
    } else {
        FeatureStructure::Association(AssociationStructure {
            radius_ly: summary.extent_ly,
            expansion_speed_km_s: marks.expansion_speed().value(),
        })
    }
}

/// A cluster's structure: its model's radii and figures, and its class counts `counts` with its
/// tails.
#[must_use]
fn cluster_structure(model: &ClusterModel, counts: &ClassCounts) -> ClusterStructure {
    let classes = counts
        .classes
        .iter()
        .map(|count| class_count(&count.class, count.expected))
        .chain(
            counts
                .tails
                .iter()
                .flatten()
                .map(|tail| class_count(&tail.class, tail.expected)),
        )
        .collect();
    ClusterStructure {
        core_radius_ly: model.core_radius().value(),
        half_mass_radius_ly: model.half_mass_radius().value(),
        tidal_radius_ly: model.tidal_radius().value(),
        central_escape_speed_km_s: model.escape_speed_central().value(),
        relaxation_time_myr: Megayears::from(model.relaxation_time()).value(),
        core_collapsed: model.is_core_collapsed(),
        black_holes: model.black_hole_count(),
        classes,
    }
}

/// A cluster's member class with its expected count, as the wire carries it.
#[must_use]
fn class_count(class: &MemberClass, expected: f64) -> ClassCountWire {
    let (low, high) = class.initial_mass_range;
    ClassCountWire {
        class: member_class(class),
        initial_mass_min_msun: low.value(),
        initial_mass_max_msun: high.value(),
        mean_mass_msun: class.mean_mass.value(),
        expected,
    }
}

/// The galactic centre's structure in `galaxy`: the black hole's sphere of influence and loss
/// cone, and its classes, each with its episode and the distribution function its orbits are
/// drawn from.
///
/// The sphere of influence is the Kepler regime's edge, where every mass inside weighs the black
/// hole's ([`CentreMotion::influence_radius`]), and not the model's, which counts the cluster
/// alone and gives its search's 10⁴ ly bracket where the black hole outweighs the cluster.
///
/// [`CentreMotion::influence_radius`]: hyperion_sim::galaxy::motion::CentreMotion::influence_radius
#[must_use]
fn centre_structure(centre: &CentreModel, galaxy: &Galaxy) -> FeatureStructure {
    FeatureStructure::Centre(CentreStructure {
        influence_radius_ly: galaxy.centre_motion().influence_radius().value(),
        loss_cone_radius_ly: centre.loss_cone_radius().value(),
        classes: centre
            .classes()
            .classes()
            .iter()
            .map(centre_class)
            .collect(),
    })
}

/// One of the centre's classes, as the wire carries it.
#[must_use]
fn centre_class(class: &CentreClass) -> ClassCountWire {
    let (low, high) = class.initial_mass_range;
    ClassCountWire {
        class: MemberClassWire::Centre {
            kind: match class.kind {
                CentreClassKind::Living => class_kind(ClassKind::Living),
                CentreClassKind::WhiteDwarf => class_kind(ClassKind::WhiteDwarf),
                CentreClassKind::NeutronStar => class_kind(ClassKind::NeutronStar),
                CentreClassKind::BlackHole => class_kind(ClassKind::BlackHole),
            },
            band: mass_layer(class.band.layer()),
            episode: class.component,
            tracer: match class.tracer {
                CentreTracer::Stars => CentreTracerWire::Stars,
                CentreTracer::BlackHoles => CentreTracerWire::BlackHoles,
                CentreTracer::YoungDisc => CentreTracerWire::YoungDisc,
                CentreTracer::YoungIsotropic => CentreTracerWire::YoungIsotropic,
            },
        },
        initial_mass_min_msun: low.value(),
        initial_mass_max_msun: high.value(),
        mean_mass_msun: class.mean_mass.value(),
        expected: class.expected,
    }
}

/// The answer to `black_hole_state` at `t`: the black hole's ID and mass, its unabsorbed 2–10 keV
/// luminosity by part, and every tidal disruption at or before `t`, newest first.
///
/// Its luminosity and disruptions are the centre's events' (plan 09, P09.T30). `t` is inside the
/// clock window, which the request's check holds, so every part is defined at every such time. The
/// sim lists the victims by their disruption's time and then their event's ID, so the list,
/// reversed, is newest first, and two at one instant come by event ID, the later first.
///
/// # Errors
///
/// `internal` if the centre's events cannot be built, which no galaxy the sim allows reaches.
pub(crate) fn black_hole_state(
    galaxy: &Galaxy,
    request: BlackHoleStateRequest,
    t: UniverseTime,
) -> Result<BlackHoleState, RequestError> {
    let events = CentreLuminosityModel::of(galaxy).map_err(|error| RequestError {
        code: ErrorCode::Internal,
        message: format!("the galactic centre's events could not be built: {error}"),
        field: None,
    })?;
    let at = luminosity_at(&events, t);
    let disruptions = events
        .victims()
        .victims()
        .iter()
        .filter(|victim| victim.time() <= t)
        .rev()
        .map(|victim| TidalDisruptionWire {
            victim: SystemIdHex::from_u64(SystemId::from(victim.id()).raw()),
            time: wire_time(victim.time()),
            star_mass_msun: victim.star_mass().value(),
            outcome: match victim.outcome() {
                DisruptionOutcome::Disrupted => DisruptionOutcomeWire::Disrupted,
                DisruptionOutcome::SwallowedWhole => DisruptionOutcomeWire::SwallowedWhole,
            },
            peak_luminosity_w: victim.tail().map(DisruptionTail::peak_luminosity_watts),
        })
        .collect();
    Ok(BlackHoleState {
        universe: request.universe,
        time: request.time,
        id: SystemIdHex::from_u64(SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).raw()),
        mass_msun: galaxy.params().black_hole().mass().value(),
        luminosity: Some(BlackHoleLuminosityWire {
            quiescent_w: at.quiescent_watts(),
            flares_w: at.flares_watts(),
            tails_w: at.tails_watts(),
            total_w: at.total_watts(),
        }),
        disruptions,
    })
}

/// The answer to `galaxy_features` at `t`: one list per requested kind, in the request's order.
///
/// The globulars are every globular of `globulars` that is one at `t`, in the catalogue's order,
/// each with its expected members as the list keeps them; the galactic centre is its one summary;
/// the bright shells are withheld, not generated, until the catalogue classes' sources are served
/// (P09.T37).
///
/// # Errors
///
/// The centre's build error if the galactic centre is asked for and its model cannot be built.
pub(crate) fn galaxy_features(
    galaxy: &Galaxy,
    globulars: &MapGlobulars,
    request: GalaxyFeaturesRequest,
    t: UniverseTime,
) -> Result<GalaxyFeatures, BuildCentreError> {
    let mut lists = Vec::with_capacity(request.kinds.len());
    for kind in &request.kinds {
        lists.push(match kind {
            MapFeatureKindWire::Globular => MapFeatureList::Globular(MapFeatures::Complete {
                features: globulars
                    .globulars()
                    .iter()
                    .filter_map(|globular| {
                        let placed = Placed::of(galaxy, globular.record(), t)?;
                        (placed.kind == FeatureKind::Globular).then(|| {
                            placed.summary(
                                galaxy,
                                globular.record(),
                                t,
                                globular.expected_members(),
                            )
                        })
                    })
                    .collect(),
            }),
            MapFeatureKindWire::GalacticCentre => {
                MapFeatureList::GalacticCentre(MapFeatures::Complete {
                    features: vec![centre_summary(galaxy.centre()?, galaxy)],
                })
            }
            MapFeatureKindWire::BrightShell => MapFeatureList::BrightShell(MapShells::NotGenerated),
        });
    }
    Ok(GalaxyFeatures {
        universe: request.universe,
        time: request.time,
        lists,
    })
}

#[cfg(test)]
mod tests {
    use hyperion_sim::id::{FeatureMemberId, FeatureRef, MemberSlot};

    use super::*;

    fn hex(word: u64) -> FeatureIdHex {
        FeatureIdHex::from_u64(word)
    }

    /// A feature's word decodes to its feature, the centre's to the centre, and every other word,
    /// a member's with its slot set, a grid system's or none at all, is refused naming the field.
    #[test]
    fn a_feature_id_is_its_members_word_or_the_centre_s() {
        let feature =
            FeatureId::of(FeatureCell::new([1, -9, 3]).expect("a cell"), 212).expect("an ID");
        assert_eq!(
            feature_target("feature", &hex(feature.object_word())),
            Ok(FeatureTarget::Catalogue(feature))
        );
        assert_eq!(
            feature_target("feature", &hex(CENTRE_FEATURE_WORD)),
            Ok(FeatureTarget::Centre)
        );
        let member = SystemId::from(
            FeatureMemberId::new(
                FeatureRef::new(FeatureCell::new([1, -9, 3]).unwrap(), 212).unwrap(),
                MemberSlot::FeatureLevel { index: 3 },
            )
            .unwrap(),
        );
        let grid = 0x4200_2cb2_0000_0001;
        let black_hole = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE).raw();
        for word in [member.raw(), grid, black_hole, u64::MAX] {
            let refused = feature_target("feature", &hex(word)).unwrap_err();
            assert_eq!(
                RequestError::from(refused).field.as_deref(),
                Some("feature"),
                "{word:016x}"
            );
        }
    }

    /// A list of kinds needs one at least, each once.
    #[test]
    fn kinds_are_one_at_least_and_each_once() {
        assert!(checked_kinds(&[FeatureKindWire::Globular]).is_ok());
        for bad in [
            &[][..],
            &[FeatureKindWire::Globular, FeatureKindWire::Globular][..],
        ] {
            let error = RequestError::from(checked_kinds(bad).unwrap_err());
            assert_eq!(error.field.as_deref(), Some("kinds"), "{bad:?}");
        }
    }

    /// The cells counted are the box the catalogue's lookup walks: three a side for a point
    /// sphere at a cell's centre, clamped to the root cube's 32, so the whole cube is 32,768.
    #[test]
    fn the_cells_counted_are_the_lookup_s_box() {
        let middle = GalacticPosition::from_light_years([2_048.0, 2_048.0, 2_048.0]).unwrap();
        assert_eq!(feature_cells_near(&middle, LightYears::new(1.0)), 27);
        assert_eq!(
            feature_cells_near(&GalacticPosition::ORIGIN, LightYears::new(131_072.0)),
            32_768
        );
        let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        assert!(feature_cells_near(&sun, LightYears::new(10_000.0)) <= MAX_FEATURE_CELLS);
        assert!(feature_cells_near(&sun, LightYears::new(20_000.0)) > MAX_FEATURE_CELLS);
    }

    use hyperion_protocol::UniverseIdHex;
    use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
    use hyperion_sim::galaxy::features::centre::testing::milky_way_galaxy;

    use crate::compute::{GalaxyKey, SharedFeatureCellCache};
    use crate::convert::origins::{SEED, galaxy};

    fn at(years: i64) -> UniverseTime {
        UniverseTime::from_julian_years(years).expect("in the clock window")
    }

    fn sun() -> GalacticPosition {
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the root cube")
    }

    fn range_request(radius_ly: f64, t: UniverseTime) -> FeaturesInRangeRequest {
        FeaturesInRangeRequest {
            universe: UniverseIdHex::from_u64(1),
            centre: galactic_position(&sun()),
            radius_ly,
            time: wire_time(t),
            kinds: vec![
                FeatureKindWire::Globular,
                FeatureKindWire::OpenCluster,
                FeatureKindWire::Association,
                FeatureKindWire::StarFormingRegion,
                FeatureKindWire::MolecularCloud,
                FeatureKindWire::GalacticCentre,
            ],
        }
    }

    /// A range of features is the same through the shared feature cells at every budget, cold and
    /// warm, as through none, and in either order of two overlapping ranges on one cache.
    #[test]
    fn a_range_of_features_does_not_depend_on_the_cell_cache() {
        let key = GalaxyKey::new(SEED, hyperion_sim::GENERATOR_VERSION);
        let answer = |cells: &dyn FeatureCellCache, radius_ly, t| {
            let request = range_request(radius_ly, t);
            let query = FeatureRangeQuery::try_from(&request).expect("a valid request");
            features_in_range(galaxy(), cells, request, query).expect("the centre builds")
        };
        let expected = answer(&NoFeatureCache, 400.0, at(0));
        assert!(!expected.features.is_empty(), "features lie within 400 ly");
        for budget in [0, 4_096, 64 << 20] {
            let cache = SharedFeatureCellCache::new(budget);
            for pass in ["cold", "warm"] {
                assert_eq!(
                    answer(&cache.handle(key), 400.0, at(0)),
                    expected,
                    "{pass}, {budget}"
                );
            }
        }
        let cache = SharedFeatureCellCache::new(64 << 20);
        hyperion_testkit::order::assert_order_independent(
            &[(400.0, 0), (900.0, 1_000)],
            |&(r, y)| answer(&cache.handle(key), r, at(y)),
        );
    }

    /// The galaxy map's kept globulars describe each globular as a summary of it alone does, at the
    /// epoch and a thousand years on, and finding them twice gives the same list.
    #[test]
    fn the_map_s_kept_globulars_are_each_globular_s_own_summary() {
        let globulars = MapGlobulars::of(galaxy());
        assert_eq!(
            MapGlobulars::of(galaxy()),
            globulars,
            "found twice, the same"
        );
        assert!(globulars.globulars().len() > 100);
        for years in [0, 1_000] {
            let request = GalaxyFeaturesRequest {
                universe: UniverseIdHex::from_u64(1),
                kinds: vec![MapFeatureKindWire::Globular],
                time: wire_time(at(years)),
            };
            let map = galaxy_features(galaxy(), &globulars, request, at(years)).expect("built");
            let MapFeatureList::Globular(MapFeatures::Complete { features }) = &map.lists[0] else {
                panic!("the globulars");
            };
            let expected: Vec<_> = globulars
                .globulars()
                .iter()
                .filter_map(|globular| {
                    let id = globular.record().id();
                    let record = FeatureCatalogue::resolve(galaxy(), id).expect("it resolves");
                    feature_summary(galaxy(), &record, at(years))
                })
                .collect();
            assert_eq!(*features, expected, "at {years} years");
        }
    }

    /// The nurseries the catalogue holds within reach of the Sun-like point with no cluster model,
    /// at the epoch: what `feature_detail` describes by their gas or as associations.
    fn unbound_nurseries() -> Vec<FeatureRecord> {
        FeatureCatalogue::near(galaxy(), &sun(), LightYears::new(3_000.0), &NoFeatureCache)
            .filter(|feature| matches!(feature.marks(), FeatureMarks::Nursery(_)))
            .filter(|feature| feature.kind_at(Years::new(0.0)).is_some())
            .filter(|feature| ClusterModel::from_record(galaxy(), feature).is_none())
            .collect()
    }

    /// A nursery with no cluster model is, while it holds gas, the Plummer ball of its gas that the
    /// features' gas places, as a cloud; and once its gas has gone, an association of its extent
    /// and its expansion speed.
    #[test]
    fn an_unbound_nursery_is_its_gas_or_an_association() {
        let (mut clouds, mut associations) = (0, 0);
        for record in unbound_nurseries() {
            let FeatureMarks::Nursery(marks) = record.marks() else {
                unreachable!("filtered to nurseries")
            };
            let (summary, structure) =
                catalogue_detail(galaxy(), &record, at(0)).expect("it exists at the epoch");
            let age = marks.age_at_epoch();
            match structure {
                FeatureStructure::Cloud(cloud) => {
                    let gas = marks.gas_mass_at(age);
                    let core = marks.size_at(age) * (0.5 / PLUMMER_HALF_MASS_RATIO);
                    assert!(gas.value() > 0.0);
                    assert_eq!(cloud.core_radius_ly.to_bits(), core.value().to_bits());
                    assert_eq!(
                        cloud.central_density_per_cm3.to_bits(),
                        plummer_central_density(gas, core).value().to_bits()
                    );
                    clouds += 1;
                }
                FeatureStructure::Association(association) => {
                    assert_eq!(marks.gas_mass_at(age).value(), 0.0);
                    assert_eq!(association.radius_ly.to_bits(), summary.extent_ly.to_bits());
                    assert_eq!(
                        association.expansion_speed_km_s.to_bits(),
                        marks.expansion_speed().value().to_bits()
                    );
                    assert_eq!(summary.kind, FeatureKindWire::Association);
                    assert_eq!(
                        summary.expected_members, 0.0,
                        "an association has no members"
                    );
                    associations += 1;
                }
                other => panic!("{}: {other:?}", summary.designation),
            }
        }
        assert!(
            clouds > 0 && associations > 0,
            "{clouds} embedded regions and {associations} associations near the Sun"
        );
    }

    /// The sphere of influence `centre_structure` serves for `galaxy`'s centre, ly.
    fn served_influence_radius(galaxy: &Galaxy) -> f64 {
        let centre = galaxy.centre().expect("the centre's model builds");
        let FeatureStructure::Centre(structure) = centre_structure(centre, galaxy) else {
            panic!("the centre's structure is a centre's");
        };
        structure.influence_radius_ly
    }

    /// The served sphere of influence at Milky Way values is the Kepler regime's edge, 7.19 ly
    /// (2.20 pc; P09.T28.b.a as built), within 2% of it, against the 2.18 pc that Schödel et al.'s
    /// (2018, A&A 609, A27, §4.5) masses inside 1 and 3 pc give.
    #[test]
    fn the_milky_way_s_centre_is_served_its_sphere_of_influence() {
        let galaxy = milky_way_galaxy();
        let served = served_influence_radius(galaxy);
        assert_eq!(
            served.to_bits(),
            galaxy.centre_motion().influence_radius().value().to_bits()
        );
        assert!((7.05..7.33).contains(&served), "{served} ly");
    }

    /// Where the black hole outweighs the whole nuclear cluster, as in the server tests' galaxy,
    /// the cluster alone has no sphere of influence (its search ends at its 10⁴ ly bracket); the
    /// served one counts the rest of the galaxy too, and lies in the 1.9–177 ly that drawn
    /// galaxies give (seeds 1–300, P09.T28.b.a as built).
    #[test]
    fn a_centre_whose_black_hole_outweighs_its_cluster_is_served_a_finite_sphere() {
        let galaxy = galaxy();
        let params = galaxy.params();
        assert!(
            params.black_hole().mass() > params.nuclear_cluster().mass(),
            "the server tests' black hole outweighs its cluster"
        );
        let centre = galaxy.centre().expect("the centre's model builds");
        assert!(centre.influence_radius().value() > 9_999.0);
        let served = served_influence_radius(galaxy);
        assert_eq!(
            served.to_bits(),
            galaxy.centre_motion().influence_radius().value().to_bits()
        );
        assert!((1.9..177.0).contains(&served), "{served} ly");
    }
}
