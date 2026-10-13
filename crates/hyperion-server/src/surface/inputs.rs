//! The seam between the surface service and plan 14: where a body's coarse-pass inputs come from
//! (rendering plan R09, Provides: `InputsSource`).

use std::fmt;

use hyperion_sim::planetary::SystemContext;
use hyperion_sim::planetary::record::BodyRecord;
use hyperion_sim::planetary::surface::{CoarseInputs, SurfaceInputsError};

/// Where the surface service takes a body's coarse-pass inputs from.
///
/// [`SurfaceService`](super::SurfaceService) calls it on a pool worker, once for each field it
/// computes, with the body's full record at [`RECORD_TIME`](super::RECORD_TIME) and its system's
/// context. Production uses [`RecordInputs`]; the server's tests use the coarse pass's reference
/// worlds, so that the service and the wire are tested before plan 14's surface section lands.
pub trait InputsSource: fmt::Debug + Send + Sync {
    /// The coarse-pass inputs of the body of `record`, in the system of `ctx`.
    ///
    /// # Errors
    ///
    /// [`SurfaceInputsError::NoSolidSurface`] for a body with no solid surface, and
    /// [`SurfaceInputsError::NotModelled`] or [`SurfaceInputsError::NotResolved`] naming a section
    /// the pass reads that the record does not model or withholds. The service passes full
    /// records, so the last is a bug there.
    fn inputs(
        &self,
        record: &BodyRecord,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, SurfaceInputsError>;
}

/// The production source: the body's record, read by [`CoarseInputs::for_body`].
///
/// Until plan 14's surface section is inhabited (P14.T24, P14.T48 and P14.T54.a), every body with a
/// solid surface answers [`SurfaceInputsError::NotModelled`] naming
/// [`RecordSection::Surface`](hyperion_sim::planetary::record::RecordSection::Surface), and a body
/// whose surface section is not applicable, a giant's, [`SurfaceInputsError::NoSolidSurface`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordInputs;

impl InputsSource for RecordInputs {
    fn inputs(
        &self,
        record: &BodyRecord,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, SurfaceInputsError> {
        CoarseInputs::for_body(record, ctx)
    }
}
