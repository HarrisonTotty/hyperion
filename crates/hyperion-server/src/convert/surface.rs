//! The surface requests (rendering plan R09, R09.T19.a): the checks of a `survey_pass`'s fields,
//! and the refusals of a body whose surface cannot be surveyed or served.
//!
//! The order is `body_detail`'s (plan 14, P14.T36.b), the universe being looked up before this:
//! the times, which need nothing, then the body ID's two parts, both named `body`. A pass's cells
//! are checked against the field's cell count only once the body is resolved, since its level
//! follows from the body's radius ([`SurveyRequest::into_pass`]).

use std::ops::Range;

use hyperion_protocol::{BodyIdHex, ErrorCode, RequestError, SurveyPassRequest, SurveySourceDto};
use hyperion_sim::id::{BodyId, SystemId};
use hyperion_sim::planetary::surface::SurfaceInputsError;
use hyperion_sim::planetary::{BodyIndex, ResolveBodyError};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::Metres;
use hyperion_surface::field::{CoarseLevel, ResolutionCode};

use super::planetary::{body_refusal, unknown_body_system};
use super::{ConvertRequestError, clock_time};
use crate::knowledge::LoadKnowledgeError;
use crate::knowledge::surveys::{
    BuildSurveyPassError, RecordSurveyError, SurveyPass, SurveyPassParts, SurveySource,
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
        let (system, index) = request.body.to_parts();
        let system = SystemId::from_raw(system)
            .map_err(|error| unknown_body_system(&request.body, error))?;
        let index = BodyIndex::try_from(index).map_err(|error| {
            body_refusal(&request.body, ResolveBodyError::MalformedIndex(error))
        })?;
        Ok(Self {
            body: index.body_id(system),
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
/// server's own bug (`internal`): the service reads full records. A pass that did not run fails as
/// a pool job does. A caller that answers a body whose inputs are not modelled in its own way, as
/// `surface_field` answers `not_modelled` (R09.T19.b), takes that case first.
#[must_use]
pub(crate) fn surface_refusal(body: &BodyIdHex, error: SurfaceJobError) -> RequestError {
    let message = match error {
        SurfaceJobError::Body(error) => return body_refusal(body, error),
        SurfaceJobError::Compute(error) => return error.into(),
        SurfaceJobError::Inputs(SurfaceInputsError::NoSolidSurface) => format!(
            "body {} has no solid surface: it is a giant, a ring, a belt or the halo, or is not \
             present at the epoch",
            body.as_str()
        ),
        SurfaceJobError::Inputs(SurfaceInputsError::NotModelled(section)) => format!(
            "body {}'s {section} section is not modelled by this generator version, so its \
             surface has no cells yet",
            body.as_str()
        ),
        SurfaceJobError::Inputs(SurfaceInputsError::NotResolved(section)) => {
            tracing::error!(
                body = body.as_str(),
                %section,
                "a body's full record withheld a section the coarse pass reads"
            );
            return RequestError {
                code: ErrorCode::Internal,
                message: "the server failed while reading the body's record".to_owned(),
                field: None,
            };
        }
    };
    RequestError {
        code: ErrorCode::BadRequest,
        message,
        field: Some("body".to_owned()),
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

    use hyperion_sim::id::SystemId;
    use hyperion_sim::planetary::DecodeBodyIndexError;
    use hyperion_sim::planetary::record::RecordSection;

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
                SurfaceJobError::Inputs(SurfaceInputsError::NoSolidSurface),
                (ErrorCode::BadRequest, Some("body")),
            ),
            (
                SurfaceJobError::Inputs(SurfaceInputsError::NotModelled(RecordSection::Figure)),
                (ErrorCode::BadRequest, Some("body")),
            ),
            (
                SurfaceJobError::Inputs(SurfaceInputsError::NotResolved(RecordSection::Figure)),
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
            SurfaceJobError::Inputs(SurfaceInputsError::NotModelled(RecordSection::Figure)),
        );
        assert!(
            figure.message.contains("figure section is not modelled"),
            "{figure:?}"
        );
        assert!(figure.message.contains(body.as_str()), "{figure:?}");
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
