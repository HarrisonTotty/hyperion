//! The surface requests (rendering plan R09, R09.T19): the checks of a `survey_pass`'s and a
//! `surface_field`'s fields, the refusals of a body whose surface cannot be surveyed or served,
//! and a field's header, cover and revisions as the wire carries them.
//!
//! The order is `body_detail`'s (plan 14, P14.T36.b), the universe being looked up before this:
//! the times, which need nothing, then the body ID's two parts, both named `body`. A pass's cells
//! are checked against the field's cell count only once the body is resolved, since its level
//! follows from the body's radius ([`SurveyRequest::into_pass`]), and a field's `have_revision`
//! against the body's revision only once its coverage is read ([`FieldRequest::since`]).

use std::ops::Range;

use hyperion_protocol::{
    BodyIdHex, ClimateModelDto, CoverDto, CoverRangeDto, ErrorCode, LiquidBasisDto, LiquidDto,
    Modelled, PaletteEntryDto, PaletteRoleDto, PrecipitationSourceDto, RecordSectionDto,
    RequestError, SURFACE_LIQUID_BINS, SurfaceFieldRequest, SurfaceHeaderDto, SurfaceRevisionDto,
    SurveyPassRequest, SurveySourceDto,
};
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::planetary::record::RecordSection;
use hyperion_sim::planetary::surface::{ReadCoarseInputsError, SurfaceInputsError};
use hyperion_sim::planetary::{BodyIndex, ResolveBodyError};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Metres;
use hyperion_surface::field::{
    ClimateModelKind, CoarseLevel, Cover, FieldHeader, LIQUID_BINS, LiquidBasis, LiquidProperties,
    PaletteEntry, PaletteRole, PrecipitationSource, ResolutionCode,
};

use super::planetary::{body_refusal, unknown_body_system};
use super::{ConvertRequestError, clock_time};
use crate::knowledge::LoadKnowledgeError;
use crate::knowledge::surveys::{
    BuildSurveyPassError, CoverageRevision, RecordSurveyError, SurveyPass, SurveyPassParts,
    SurveySource,
};
use crate::limits::MAX_SURVEY_RANGES;
use crate::surface::SurfaceJobError;

/// A `survey_pass` request, checked as far as it can be before its body is resolved: the times
/// inside the clock window and in order, a resolution that has a code, at most
/// [`MAX_SURVEY_RANGES`] ranges, and the body ID decoded.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SurveyRequest {
    body: BodyId,
    from: UniverseTime,
    to: UniverseTime,
    source: SurveySource,
    resolution: Metres,
    cells: Vec<Range<u32>>,
}

impl SurveyRequest {
    /// The body surveyed, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn body(&self) -> BodyId {
        self.body
    }

    /// The system of the body, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn system(&self) -> SystemId {
        self.body.system()
    }

    /// The pass, its cells at `level`, the level of the body's field.
    ///
    /// # Errors
    ///
    /// `bad_request` naming `cells` for runs that are none, empty, out of order, overlapping or
    /// past the field's last cell. The times and the resolution were checked already.
    pub(crate) fn into_pass(self, level: CoarseLevel) -> Result<SurveyPass, RequestError> {
        SurveyPass::new(SurveyPassParts {
            body: self.body,
            from: self.from,
            to: self.to,
            source: self.source,
            resolution: self.resolution,
            level,
            cells: self.cells,
        })
        .map_err(|error| {
            let field = match error {
                BuildSurveyPassError::Span { .. } => "to",
                BuildSurveyPassError::Resolution { .. } => "resolution_m",
                BuildSurveyPassError::NoCells
                | BuildSurveyPassError::EmptyRun { .. }
                | BuildSurveyPassError::Unsorted { .. }
                | BuildSurveyPassError::OutsideField { .. } => "cells",
            };
            ConvertRequestError::new(field, error).into()
        })
    }
}

impl TryFrom<&SurveyPassRequest> for SurveyRequest {
    type Error = RequestError;

    /// Checks `from` and `to`, each inside the clock window and `to` no earlier than `from`, then
    /// `resolution_m` and the number of `cells`, and decodes `body` last: bits that are not a
    /// system ID name no system (`unknown_system`), and an index outside plan 14's layout is
    /// malformed (`bad_request`), both naming `body`.
    fn try_from(request: &SurveyPassRequest) -> Result<Self, Self::Error> {
        let from = clock_time("from", &request.from)?;
        let to = clock_time("to", &request.to)?;
        // Checked here as `SurveyPass::new` checks them, in its words, so that they are refused
        // before the body is resolved.
        if to < from {
            return Err(
                ConvertRequestError::new("to", BuildSurveyPassError::Span { from, to }).into(),
            );
        }
        let resolution = Metres::new(request.resolution_m);
        ResolutionCode::from_resolution(resolution).map_err(|_| {
            ConvertRequestError::new(
                "resolution_m",
                BuildSurveyPassError::Resolution {
                    resolution_m: request.resolution_m,
                },
            )
        })?;
        if request.cells.len() > MAX_SURVEY_RANGES {
            return Err(ConvertRequestError::new(
                "cells",
                format!(
                    "{} ranges are more than a pass may carry, {MAX_SURVEY_RANGES}: send a longer \
                     pass as several requests sharing its time span",
                    request.cells.len()
                ),
            )
            .into());
        }
        Ok(Self {
            body: decode_body(&request.body)?,
            from,
            to,
            source: survey_source(request.source),
            resolution,
            cells: request
                .cells
                .iter()
                .map(|range| range.start..range.end)
                .collect(),
        })
    }
}

/// The body ID `body` decoded, not yet resolved: bits that are not a system ID name no system
/// (`unknown_system`), and an index outside plan 14's layout is malformed (`bad_request`), both
/// naming `body`, as `body_detail` refuses them.
fn decode_body(body: &BodyIdHex) -> Result<BodyId, RequestError> {
    let (system, index) = body.to_parts();
    let system = SystemId::from_raw(system).map_err(|error| unknown_body_system(body, error))?;
    let index = BodyIndex::try_from(index)
        .map_err(|error| body_refusal(body, ResolveBodyError::MalformedIndex(error)))?;
    Ok(index.body_id(system))
}

/// A `surface_field` request with its body decoded (R09.T19.b); its `have_revision` is checked
/// against the body's coverage once that is read ([`since`](Self::since)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FieldRequest {
    body: BodyId,
    have_revision: Option<u32>,
}

impl FieldRequest {
    /// The body asked for, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn body(self) -> BodyId {
        self.body
    }

    /// The system of the body, decoded but not yet resolved.
    #[must_use]
    pub(crate) fn system(self) -> SystemId {
        self.body.system()
    }

    /// The revision the client holds, against which the answer is a delta, given the body's
    /// revision `revision` (`None` before its first pass): `None` for the whole field, asked with
    /// no `have_revision` or with 0.
    ///
    /// # Errors
    ///
    /// `bad_request` naming `have_revision` for a revision above the body's, which no answer
    /// gave.
    pub(crate) fn since(
        self,
        revision: Option<CoverageRevision>,
    ) -> Result<Option<CoverageRevision>, RequestError> {
        let revision = revision.map_or(0, CoverageRevision::get);
        match self.have_revision {
            Some(held) if held > revision => Err(ConvertRequestError::new(
                "have_revision",
                format!(
                    "revision {held} is above the body's coverage revision, {revision}: ask \
                     again without `have_revision` for the whole field"
                ),
            )
            .into()),
            Some(held) => Ok(CoverageRevision::new(held)),
            None => Ok(None),
        }
    }
}

impl TryFrom<&SurfaceFieldRequest> for FieldRequest {
    type Error = RequestError;

    /// Decodes `body`, as a `survey_pass`'s is decoded.
    fn try_from(request: &SurfaceFieldRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            body: decode_body(&request.body)?,
            have_revision: request.have_revision,
        })
    }
}

// The wire's bins are the field's: a liquid record's by-bin members cross as they are, and a new
// bin set is a new payload format and a new protocol (decision-r11-palette-liquids §1.2).
const _: () = assert!(SURFACE_LIQUID_BINS == LIQUID_BINS);

/// A field's header as the main thread reads it: the scalars of `header`, its classes, its
/// palette's substances and roles with each entry's normal albedo, phase row and liquid record
/// (ruled by "main", 2026-10-13, R09.T19.a's record; decision-r11-palette-liquids §1.5 and §2),
/// its main liquid, and the two fractions plan 14 fills later, absent until it does.
///
/// The body is the header's own, which on the reference worlds is theirs, not the body asked for
/// (R09.T17's deferred finding).
#[must_use]
pub(crate) fn surface_header(header: &FieldHeader) -> SurfaceHeaderDto {
    let body = header.body();
    let figure = header.figure();
    SurfaceHeaderDto {
        body: BodyIdHex::from_parts(body.raw_system_id(), body.body_index()),
        level: header.level().get(),
        radius_m: header.radius().value(),
        equatorial_radius_m: figure.equatorial_radius_m,
        polar_radius_m: figure.polar_radius_m,
        boundary_diameter_m: header.boundary_diameter().value(),
        sea_level_m: header.sea_level().value(),
        lapse_rate_k_per_m: header.lapse_rate_k_per_m(),
        realised_sigma_h_m: header.realised_sigma_h().value(),
        realised_relief_m: header.realised_relief().value(),
        months: header.months(),
        season_eccentricity: header.season_eccentricity(),
        reference_temperature_k: header.reference_temperature().value(),
        surface_age_gyr: header.surface_age().value(),
        surface_pressure_pa: header.surface_pressure().value(),
        albedo_scale: header.albedo_scale(),
        climate_model: climate_model(header.climate_model()),
        precipitation: precipitation_source(header.precipitation()),
        palette: header
            .palette()
            .entries()
            .iter()
            .map(palette_entry)
            .collect(),
        main_liquid: header.main_liquid(),
        maturity: modelled(header.maturity()),
        organic_cover: modelled(header.organic_cover()),
    }
}

/// A palette entry as the main thread reads it: its key, its role, its normal albedo before the
/// albedo scale, its phase row, and its liquid record, `None` where the entry is not liquid on the
/// body.
#[must_use]
fn palette_entry(entry: &PaletteEntry) -> PaletteEntryDto {
    PaletteEntryDto {
        substance: entry.substance.as_str().to_owned(),
        role: palette_role(entry.role),
        normal_albedo_bvr: entry.normal_albedo_bvr,
        phase_row: entry.phase_row,
        liquid: entry.liquid.as_ref().map(liquid),
    }
}

/// A liquid record as the wire carries it: each member as the field holds it, and each member the
/// server does not model absent (decision-r11-palette-liquids §1.4).
#[must_use]
pub(crate) fn liquid(record: &LiquidProperties) -> LiquidDto {
    LiquidDto {
        temperature_k: record.temperature.value(),
        density_kg_m3: record.density_kg_m3,
        basis: liquid_basis(record.basis),
        refractive_index: record.refractive_index,
        absorption_per_m: modelled(record.absorption_per_m),
        scattering_per_m: modelled(record.scattering_per_m),
        surface_tension_n_per_m: modelled(record.surface_tension_n_per_m),
        viscosity_pa_s: modelled(record.viscosity_pa_s),
    }
}

/// A value the field holds or does not model, as the wire carries it: absent when it is `None`.
#[must_use]
fn modelled<T>(value: Option<T>) -> Modelled<T> {
    value.map_or(Modelled::NotModelled, Modelled::Value)
}

/// A liquid record's basis as the wire names it.
#[must_use]
pub(crate) fn liquid_basis(basis: LiquidBasis) -> LiquidBasisDto {
    match basis {
        LiquidBasis::Measured => LiquidBasisDto::Measured,
        LiquidBasis::Derived => LiquidBasisDto::Derived,
        LiquidBasis::StandIn => LiquidBasisDto::StandIn,
    }
}

/// A climate model's class as the wire names it.
#[must_use]
pub(crate) fn climate_model(kind: ClimateModelKind) -> ClimateModelDto {
    match kind {
        ClimateModelKind::EnergyBalance => ClimateModelDto::EnergyBalance,
        ClimateModelKind::LockedEnergyBalance => ClimateModelDto::LockedEnergyBalance,
        ClimateModelKind::RadiativeEquilibrium => ClimateModelDto::RadiativeEquilibrium,
        ClimateModelKind::Isothermal => ClimateModelDto::Isothermal,
    }
}

/// Where a field's precipitation came from, as the wire names it.
#[must_use]
pub(crate) fn precipitation_source(source: PrecipitationSource) -> PrecipitationSourceDto {
    match source {
        PrecipitationSource::Heuristic => PrecipitationSourceDto::Heuristic,
    }
}

/// A palette entry's role as the wire names it.
#[must_use]
pub(crate) fn palette_role(role: PaletteRole) -> PaletteRoleDto {
    match role {
        PaletteRole::PrimaryCrust => PaletteRoleDto::PrimaryCrust,
        PaletteRole::SecondaryCrust => PaletteRoleDto::SecondaryCrust,
        PaletteRole::TertiaryCrust => PaletteRoleDto::TertiaryCrust,
        PaletteRole::Province => PaletteRoleDto::Province,
        PaletteRole::Ice => PaletteRoleDto::Ice,
        PaletteRole::Liquid => PaletteRoleDto::Liquid,
        PaletteRole::Cover => PaletteRoleDto::Cover,
        PaletteRole::Deposit => PaletteRoleDto::Deposit,
        PaletteRole::Bulk => PaletteRoleDto::Bulk,
    }
}

/// A record section as a `not_modelled` answer names it.
#[must_use]
pub(crate) fn record_section(section: RecordSection) -> RecordSectionDto {
    match section {
        RecordSection::Label => RecordSectionDto::Label,
        RecordSection::Mass => RecordSectionDto::Mass,
        RecordSection::Orbit => RecordSectionDto::Orbit,
        RecordSection::Moons => RecordSectionDto::Moons,
        RecordSection::Rings => RecordSectionDto::Rings,
        RecordSection::Population => RecordSectionDto::Population,
        RecordSection::Bulk => RecordSectionDto::Bulk,
        RecordSection::Surface => RecordSectionDto::Surface,
        RecordSection::Hooks => RecordSectionDto::Hooks,
        RecordSection::Photometry => RecordSectionDto::Photometry,
        RecordSection::Figure => RecordSectionDto::Figure,
        RecordSection::Rotation => RecordSectionDto::Rotation,
        RecordSection::Envelope => RecordSectionDto::Envelope,
    }
}

/// A cover as the wire carries it: each range with its resolution code, 0 for a margin cell.
#[must_use]
pub(crate) fn cover_dto(cover: &Cover) -> CoverDto {
    CoverDto {
        ranges: cover
            .ranges()
            .iter()
            .map(|range| CoverRangeDto {
                start: range.start(),
                end: range.end(),
                resolution: range.code().get(),
            })
            .collect(),
    }
}

/// A body's raised revision as the scene topic tells it.
#[must_use]
pub(crate) fn surface_revision(body: BodyId, revision: CoverageRevision) -> SurfaceRevisionDto {
    SurfaceRevisionDto {
        body: BodyIdHex::from_parts(body.system().raw(), body.body_index()),
        revision: revision.get(),
    }
}

/// How a pass measured, as the survey log names it.
#[must_use]
pub(crate) fn survey_source(source: SurveySourceDto) -> SurveySource {
    match source {
        SurveySourceDto::Orbital => SurveySource::Orbital,
        SurveySourceDto::CloseRange => SurveySource::CloseRange,
        SurveySourceDto::Landed => SurveySource::Landed,
    }
}

/// Why the surface of the body `body` names cannot be surveyed, as the wire says it.
///
/// A body that does not resolve is refused as `body_detail` refuses it ([`body_refusal`]); one
/// with no solid surface (a giant, a ring, a belt, the halo, or a body not yet formed or gone at
/// the record's time) is `bad_request` naming `body`, and so is one whose field's level cannot be
/// had because the record's figure is not modelled either. A section the record withholds is the
/// server's own bug (`internal`): the service reads full records. So is every refusal of the
/// inputs other than a section's state (R09.T10.f; decision-backlog-2 §3.3), each a fault of the
/// server's own reading of its records: a moon's planet's record not given or not its planet's, a
/// seasonal orbit that does not hold at the record's time, or inputs the builder refuses; each is
/// logged. A pass that did not run fails as a pool job does. A caller that answers a body whose
/// inputs are not modelled in its own way, as `surface_field` answers `not_modelled` (R09.T19.b),
/// takes that case first.
#[must_use]
pub(crate) fn surface_refusal(body: &BodyIdHex, error: SurfaceJobError) -> RequestError {
    let message = match error {
        SurfaceJobError::Body(error) => return body_refusal(body, error),
        SurfaceJobError::Compute(error) => return error.into(),
        SurfaceJobError::Inputs(ReadCoarseInputsError::Section(
            SurfaceInputsError::NoSolidSurface,
        )) => format!(
            "body {} has no solid surface: it is a giant, a ring, a belt or the halo, or is not \
             present at the epoch",
            body.as_str()
        ),
        SurfaceJobError::Inputs(ReadCoarseInputsError::Section(
            SurfaceInputsError::NotModelled(section),
        )) => format!(
            "body {}'s {section} section is not modelled by this generator version, so its \
             surface has no cells yet",
            body.as_str()
        ),
        SurfaceJobError::Inputs(ReadCoarseInputsError::Section(
            SurfaceInputsError::NotResolved(section),
        )) => {
            tracing::error!(
                body = body.as_str(),
                %section,
                "a body's full record withheld a section the coarse pass reads"
            );
            return reading_failed();
        }
        SurfaceJobError::Inputs(
            error @ (ReadCoarseInputsError::PlanetMissing(_)
            | ReadCoarseInputsError::NotItsPlanet { .. }
            | ReadCoarseInputsError::NotAtRecordTime
            | ReadCoarseInputsError::SeasonalOrbit(_)
            | ReadCoarseInputsError::Build(_)),
        ) => {
            tracing::error!(
                body = body.as_str(),
                %error,
                cause = ?error,
                "the server could not read a body's coarse-pass inputs from its records"
            );
            return reading_failed();
        }
    };
    RequestError {
        code: ErrorCode::BadRequest,
        message,
        field: Some("body".to_owned()),
    }
}

/// The answer to a body whose records the server failed to read into its coarse-pass inputs, a
/// fault of its own: `internal`, naming nothing of the server's.
#[must_use]
fn reading_failed() -> RequestError {
    RequestError {
        code: ErrorCode::Internal,
        message: "the server failed while reading the body's record".to_owned(),
        field: None,
    }
}

/// The answer when a universe's survey log could not be opened, logged with its cause:
/// `unsupported_save_format` for a log of a format this build cannot read, and `internal` for one
/// that could not be read or is damaged. The message names no path of the server's.
#[must_use]
pub(crate) fn open_surveys_refusal(error: &LoadKnowledgeError) -> RequestError {
    tracing::error!(?error, "failed to open a universe's survey log");
    let (code, message) = match error {
        LoadKnowledgeError::UnsupportedFormat { format, .. } => (
            ErrorCode::UnsupportedSaveFormat,
            format!(
                "the universe's survey log is in format {format}, which this server cannot read"
            ),
        ),
        LoadKnowledgeError::Io { .. }
        | LoadKnowledgeError::MalformedLine { .. }
        | LoadKnowledgeError::Interrupted => (
            ErrorCode::Internal,
            "the server could not read the universe's survey log".to_owned(),
        ),
    };
    RequestError {
        code,
        message,
        field: None,
    }
}

/// The answer when a pass could not be recorded, logged with its cause: `storage_failed` when it
/// could not be saved, and `internal` for a pass at another level than the body's earlier ones or
/// past the last revision, which only a fault of the server's own makes.
#[must_use]
pub(crate) fn record_survey_refusal(error: &RecordSurveyError) -> RequestError {
    tracing::error!(?error, "failed to record a survey pass");
    let (code, message) = match error {
        RecordSurveyError::Save(_) => (
            ErrorCode::StorageFailed,
            "the server could not save the survey pass",
        ),
        RecordSurveyError::Level { .. } | RecordSurveyError::Revisions { .. } => (
            ErrorCode::Internal,
            "the server could not record the survey pass",
        ),
    };
    RequestError {
        code,
        message: message.to_owned(),
        field: None,
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use hyperion_protocol::UniverseIdHex;
    use hyperion_sim::Seed;
    use hyperion_sim::orbit::BuildOrbitError;
    use hyperion_sim::planetary::DecodeBodyIndexError;
    use hyperion_sim::planetary::hooks::surface_seed;
    use hyperion_sim::planetary::surface::{BuildCoarseInputsError, coarse_pass, reference};
    use hyperion_sim::units::Kelvin;
    use hyperion_surface::field::{FieldView, MaterialPalette, MechanicsFamily};
    use hyperion_surface::substance_key::SubstanceKey;
    use hyperion_surface::testing::{FieldBuilder, basaltic_melt, liquid_water};

    use super::*;
    use crate::compute::{ComputeError, JobError, SubmitJobError};
    use crate::knowledge::SaveKnowledgeError;

    fn body() -> BodyIdHex {
        BodyIdHex::from_parts(0x0200_0800_2000_0000, 0x0103)
    }

    fn code_and_field(error: &RequestError) -> (ErrorCode, Option<&str>) {
        (error.code, error.field.as_deref())
    }

    /// Each reason a body's surface cannot be had is answered with its code, naming `body` where
    /// the body is at fault, and a withheld section as the server's own failure.
    #[test]
    fn a_surface_refusal_names_the_body_where_it_is_at_fault() {
        let body = body();
        let malformed = DecodeBodyIndexError::ReservedSlot { raw: 0xd000 };
        let planet = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), 0x0100);
        let cases = [
            (
                SurfaceJobError::Body(ResolveBodyError::NoSuchBody),
                (ErrorCode::UnknownBody, Some("body")),
            ),
            (
                SurfaceJobError::Body(ResolveBodyError::MalformedIndex(malformed)),
                (ErrorCode::BadRequest, Some("body")),
            ),
            (
                SurfaceJobError::Inputs(SurfaceInputsError::NoSolidSurface.into()),
                (ErrorCode::BadRequest, Some("body")),
            ),
            (
                SurfaceJobError::Inputs(
                    SurfaceInputsError::NotModelled(RecordSection::Figure).into(),
                ),
                (ErrorCode::BadRequest, Some("body")),
            ),
            (
                SurfaceJobError::Inputs(
                    SurfaceInputsError::NotResolved(RecordSection::Figure).into(),
                ),
                (ErrorCode::Internal, None),
            ),
            // R09.T10.f: a fault of the server's reading of its records, whatever it is.
            (
                SurfaceJobError::Inputs(ReadCoarseInputsError::PlanetMissing(planet)),
                (ErrorCode::Internal, None),
            ),
            (
                SurfaceJobError::Inputs(ReadCoarseInputsError::NotItsPlanet {
                    planet: Some(planet),
                    given: planet,
                }),
                (ErrorCode::Internal, None),
            ),
            (
                SurfaceJobError::Inputs(ReadCoarseInputsError::NotItsPlanet {
                    planet: None,
                    given: planet,
                }),
                (ErrorCode::Internal, None),
            ),
            (
                SurfaceJobError::Inputs(ReadCoarseInputsError::NotAtRecordTime),
                (ErrorCode::Internal, None),
            ),
            (
                SurfaceJobError::Inputs(ReadCoarseInputsError::SeasonalOrbit(
                    BuildOrbitError::EccentricityOutOfRange { value: 1.5 },
                )),
                (ErrorCode::Internal, None),
            ),
            (
                SurfaceJobError::Inputs(ReadCoarseInputsError::Build(
                    BuildCoarseInputsError::Missing("figure"),
                )),
                (ErrorCode::Internal, None),
            ),
            (
                SurfaceJobError::Compute(ComputeError::Submit(SubmitJobError::QueueFull)),
                (ErrorCode::QueueFull, None),
            ),
            (
                SurfaceJobError::Compute(ComputeError::Job(JobError::Cancelled)),
                (ErrorCode::Cancelled, None),
            ),
        ];
        for (error, expected) in cases {
            let refusal = surface_refusal(&body, error);
            assert_eq!(code_and_field(&refusal), expected, "{error:?}: {refusal:?}");
        }
        let figure = surface_refusal(
            &body,
            SurfaceJobError::Inputs(SurfaceInputsError::NotModelled(RecordSection::Figure).into()),
        );
        assert!(
            figure.message.contains("figure section is not modelled"),
            "{figure:?}"
        );
        assert!(figure.message.contains(body.as_str()), "{figure:?}");
    }

    /// R09.T19.b: a field asked with no revision or with 0 is the whole field, one with the
    /// body's revision or below a delta since it, and one above refused naming `have_revision`.
    #[test]
    fn a_field_request_s_revision_is_checked_against_the_body_s() {
        let request = |have_revision| {
            FieldRequest::try_from(&SurfaceFieldRequest {
                universe: UniverseIdHex::from_u64(42),
                body: body(),
                have_revision,
            })
            .unwrap()
        };
        let seventh = CoverageRevision::new(7);
        assert_eq!(request(None).since(None), Ok(None));
        assert_eq!(request(None).since(seventh), Ok(None));
        assert_eq!(request(Some(0)).since(seventh), Ok(None));
        assert_eq!(request(Some(0)).since(None), Ok(None));
        assert_eq!(
            request(Some(3)).since(seventh),
            Ok(CoverageRevision::new(3))
        );
        assert_eq!(request(Some(7)).since(seventh), Ok(seventh));
        for (held, revision) in [(8, seventh), (1, None)] {
            let refused = request(Some(held)).since(revision).unwrap_err();
            assert_eq!(
                code_and_field(&refused),
                (ErrorCode::BadRequest, Some("have_revision")),
                "{refused:?}"
            );
        }
        assert_eq!(
            request(None).body(),
            BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), 0x0103)
        );
        let malformed = FieldRequest::try_from(&SurfaceFieldRequest {
            universe: UniverseIdHex::from_u64(42),
            body: BodyIdHex::from_parts(0x0200_0800_2000_0000, 0xd000),
            have_revision: None,
        })
        .unwrap_err();
        assert_eq!(
            code_and_field(&malformed),
            (ErrorCode::BadRequest, Some("body"))
        );
    }

    /// R09.T19.b: every record section is named on the wire as the record names it, and every
    /// palette role and climate model by its own name, in code order.
    #[test]
    fn surface_classes_map_to_their_wire_names() {
        for section in RecordSection::ALL {
            assert_eq!(
                serde_json::to_value(record_section(section)).unwrap(),
                section.to_string()
            );
        }
        let roles: Vec<_> = PaletteRole::ALL
            .iter()
            .map(|&role| serde_json::to_value(palette_role(role)).unwrap())
            .collect();
        assert_eq!(
            roles,
            [
                "primary_crust",
                "secondary_crust",
                "tertiary_crust",
                "province",
                "ice",
                "liquid",
                "cover",
                "deposit",
                "bulk",
            ]
        );
        let models: Vec<_> = ClimateModelKind::ALL
            .iter()
            .map(|&model| serde_json::to_value(climate_model(model)).unwrap())
            .collect();
        assert_eq!(
            models,
            [
                "energy_balance",
                "locked_energy_balance",
                "radiative_equilibrium",
                "isothermal",
            ]
        );
        let sources: Vec<_> = PrecipitationSource::ALL
            .iter()
            .map(|&source| serde_json::to_value(precipitation_source(source)).unwrap())
            .collect();
        assert_eq!(sources, ["heuristic"]);
    }

    /// R09.T19.b: a field's header reaches the wire with each scalar in its unit, the spheroid's
    /// radii in order, and its palette's keys and roles in the palette's order.
    #[test]
    fn a_field_header_carries_its_scalars_and_palette() {
        let inputs = reference::ceres_like();
        let seed = surface_seed(
            Seed::new(0x4d2),
            BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), 0x0103),
        );
        let field = coarse_pass(seed, &inputs);
        let header = field.header();
        let wire = surface_header(header);
        assert_eq!(
            wire.body.to_parts(),
            (header.body().raw_system_id(), header.body().body_index())
        );
        assert_eq!(wire.level, header.level().get());
        assert_eq!(wire.radius_m.to_bits(), inputs.radius().value().to_bits());
        assert!(wire.equatorial_radius_m >= wire.polar_radius_m, "{wire:?}");
        let figure = header.figure();
        let scalars = [
            (wire.equatorial_radius_m, figure.equatorial_radius_m, "a"),
            (wire.polar_radius_m, figure.polar_radius_m, "c"),
            (
                wire.boundary_diameter_m,
                header.boundary_diameter().value(),
                "D_b",
            ),
            (wire.sea_level_m, header.sea_level().value(), "sea level"),
            (
                wire.lapse_rate_k_per_m,
                header.lapse_rate_k_per_m(),
                "lapse rate",
            ),
            (
                wire.realised_sigma_h_m,
                header.realised_sigma_h().value(),
                "sigma_h",
            ),
            (
                wire.realised_relief_m,
                header.realised_relief().value(),
                "relief",
            ),
            (
                wire.season_eccentricity,
                header.season_eccentricity(),
                "eccentricity",
            ),
            (
                wire.reference_temperature_k,
                header.reference_temperature().value(),
                "temperature",
            ),
            (wire.surface_age_gyr, header.surface_age().value(), "age"),
            (
                wire.surface_pressure_pa,
                header.surface_pressure().value(),
                "pressure",
            ),
        ];
        for (sent, held, name) in scalars {
            assert_eq!(sent.to_bits(), held.to_bits(), "{name}");
        }
        assert_eq!(wire.months, header.months());
        assert_eq!(wire.climate_model, climate_model(header.climate_model()));
        assert_eq!(
            wire.precipitation,
            precipitation_source(header.precipitation())
        );
        assert_eq!(wire.albedo_scale, None);
        assert_eq!(wire.palette.len(), header.palette().len());
        for (entry, held) in wire.palette.iter().zip(header.palette().entries()) {
            assert_eq!(entry.substance, held.substance.as_str());
            assert_eq!(entry.role, palette_role(held.role));
            assert_eq!(
                entry.normal_albedo_bvr.map(f64::to_bits),
                held.normal_albedo_bvr.map(f64::to_bits)
            );
            assert_eq!(entry.phase_row, held.phase_row);
            assert_eq!(entry.liquid, None, "{}", entry.substance);
        }
        assert_eq!(wire.main_liquid, None);
        assert!(wire.maturity.is_not_modelled(), "{wire:?}");
        assert!(wire.organic_cover.is_not_modelled(), "{wire:?}");
    }

    /// Decision-r11-palette-liquids §1.5 and §2: a sea's and a melt's records reach the wire with
    /// every member to the bit, a member the field does not model absent, an entry that is not
    /// liquid with none; the header names its main liquid and carries the two fractions plan 14
    /// fills, once they are filled; and every basis has its wire name.
    #[test]
    fn a_field_header_carries_its_liquid_records() {
        let entry = |key, role, density, liquid| PaletteEntry {
            substance: SubstanceKey::new(key).unwrap(),
            role,
            normal_albedo_bvr: [0.3; 3],
            phase_row: 1,
            grain_density_kg_m3: density,
            transition: Kelvin::new(1_400.0),
            mechanics: MechanicsFamily::Silicate,
            liquid,
        };
        let water = liquid_water();
        let sea = PaletteEntry {
            normal_albedo_bvr: water.fresnel_normal_albedo_bvr(),
            phase_row: 0,
            transition: Kelvin::new(273.15),
            mechanics: MechanicsFamily::WaterIce,
            ..entry("H2O", PaletteRole::Liquid, water.density_kg_m3, Some(water))
        };
        let palette = MaterialPalette::new(vec![
            entry(
                "basalt",
                PaletteRole::PrimaryCrust,
                2_900.0,
                Some(basaltic_melt()),
            ),
            entry("anorthosite", PaletteRole::SecondaryCrust, 2_730.0, None),
            sea,
        ])
        .unwrap();
        let field = FieldBuilder::new(Metres::new(4.697e5))
            .palette(palette)
            .crust_palette([None, None, Some(0), Some(1)])
            .main_liquid(2)
            .maturity(0.75)
            .organic_cover(0.0)
            .build();
        let wire = surface_header(field.header());
        assert_eq!(wire.main_liquid, Some(2));
        assert_eq!(wire.maturity, Modelled::Value(0.75));
        assert_eq!(wire.organic_cover, Modelled::Value(0.0));
        let [melt, rock, sea] = wire.palette.as_slice() else {
            panic!("{:?}", wire.palette);
        };
        assert_eq!(rock.liquid, None);
        let record = liquid_water();
        let sent = sea.liquid.as_ref().unwrap();
        assert_eq!(sent.basis, LiquidBasisDto::Measured);
        assert_eq!(
            sent.temperature_k.to_bits(),
            record.temperature.value().to_bits()
        );
        assert_eq!(sent.density_kg_m3.to_bits(), record.density_kg_m3.to_bits());
        assert_eq!(
            sent.refractive_index.map(f64::to_bits),
            record.refractive_index.map(f64::to_bits)
        );
        assert_eq!(
            sent.absorption_per_m,
            Modelled::Value(record.absorption_per_m.unwrap())
        );
        assert!(sent.scattering_per_m.is_not_modelled(), "{sent:?}");
        assert_eq!(
            sent.surface_tension_n_per_m,
            Modelled::Value(record.surface_tension_n_per_m.unwrap())
        );
        assert_eq!(
            sent.viscosity_pa_s,
            Modelled::Value(record.viscosity_pa_s.unwrap())
        );
        assert_eq!(
            sea.normal_albedo_bvr.map(f64::to_bits),
            record.fresnel_normal_albedo_bvr().map(f64::to_bits)
        );
        let lava = melt.liquid.as_ref().unwrap();
        assert_eq!(lava.basis, LiquidBasisDto::StandIn);
        assert!(lava.surface_tension_n_per_m.is_not_modelled(), "{lava:?}");
        assert!(lava.viscosity_pa_s.is_not_modelled(), "{lava:?}");
        let bases: Vec<_> = LiquidBasis::ALL
            .iter()
            .map(|&basis| serde_json::to_value(liquid_basis(basis)).unwrap())
            .collect();
        assert_eq!(bases, ["measured", "derived", "stand_in"]);
    }

    /// The sim's reference Earth carries the surface crate's synthetic water record, through a
    /// copy of its own (the sim's tests cannot reach the surface crate's `testing`), so that the
    /// two move together when `water.json` moves (the client's
    /// `view/atmosphere/materials/paletteLiquids.test.ts` holds both to the file).
    #[test]
    fn the_reference_earth_s_sea_is_the_synthetic_water_record() {
        let earth = reference::earth_like();
        let sea = earth.palette().get(earth.main_liquid().unwrap()).unwrap();
        assert_eq!(sea.liquid.as_ref(), Some(&liquid_water()));
    }

    /// A survey log of a later format is the save's, and any other failure to open it the
    /// server's; neither message names a path of the server's.
    #[test]
    fn a_survey_log_that_cannot_be_opened_is_refused_by_its_cause() {
        let path =
            PathBuf::from("/srv/hyperion/universes/00000000000000ab/knowledge/surveys.v1.jsonl");
        let cases = [
            (
                LoadKnowledgeError::UnsupportedFormat {
                    path: path.clone(),
                    format: 2,
                },
                ErrorCode::UnsupportedSaveFormat,
            ),
            (
                LoadKnowledgeError::Io {
                    operation: "read",
                    path: path.clone(),
                    source: io::Error::other("disk on fire"),
                },
                ErrorCode::Internal,
            ),
            (
                LoadKnowledgeError::MalformedLine {
                    path: path.clone(),
                    line: 3,
                    reason: "cells",
                    source: None,
                },
                ErrorCode::Internal,
            ),
            (LoadKnowledgeError::Interrupted, ErrorCode::Internal),
        ];
        for (error, code) in cases {
            let refusal = open_surveys_refusal(&error);
            assert_eq!(code_and_field(&refusal), (code, None), "{error:?}");
            assert!(!refusal.message.contains("/srv"), "{refusal:?}");
        }
    }

    /// A pass that cannot be saved is `storage_failed`, and one the log refuses the server's own
    /// failure.
    #[test]
    fn a_survey_pass_that_cannot_be_recorded_is_refused_by_its_cause() {
        let body = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000).unwrap(), 0x0103);
        let cases = [
            (
                RecordSurveyError::Save(SaveKnowledgeError::Interrupted),
                ErrorCode::StorageFailed,
            ),
            (
                RecordSurveyError::Save(SaveKnowledgeError::Poisoned),
                ErrorCode::StorageFailed,
            ),
            (
                RecordSurveyError::Level {
                    body,
                    recorded: CoarseLevel::MIN,
                    pass: CoarseLevel::MAX,
                },
                ErrorCode::Internal,
            ),
            (RecordSurveyError::Revisions { body }, ErrorCode::Internal),
        ];
        for (error, code) in cases {
            let refusal = record_survey_refusal(&error);
            assert_eq!(code_and_field(&refusal), (code, None), "{error:?}");
        }
    }
}
