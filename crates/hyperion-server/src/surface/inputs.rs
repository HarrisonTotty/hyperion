//! The seam between the surface service and plan 14: where a body's coarse-pass inputs come from
//! (rendering plan R09, Provides: `InputsSource`).

use std::fmt;

use hyperion_sim::planetary::SystemContext;
use hyperion_sim::planetary::record::BodyRecord;
use hyperion_sim::planetary::surface::{CoarseInputs, ReadCoarseInputsError};

/// Where the surface service takes a body's coarse-pass inputs from.
///
/// [`SurfaceService`](super::SurfaceService) calls it on a pool worker, once for each field it
/// computes, with the body's full record at [`RECORD_TIME`](super::RECORD_TIME), a moon's
/// planet's full record taken with it, and its system's context: the arguments of
/// [`CoarseInputs::for_body`] (R09.T10.f), so that every source reads records into inputs.
/// Production uses [`RecordInputs`]; the server's tests use the coarse pass's reference worlds, so
/// that the service and the wire are tested before plan 14's surface section lands.
pub trait InputsSource: fmt::Debug + Send + Sync {
    /// The coarse-pass inputs of the body of `record`, in the system of `ctx`, with `planet` the
    /// record of the planet it orbits if it is a moon, `Some` exactly where
    /// [`CoarseInputs::planet_of`] is.
    ///
    /// # Errors
    ///
    /// [`ReadCoarseInputsError::Section`] for the body's own state: with
    /// [`NoSolidSurface`](hyperion_sim::planetary::surface::SurfaceInputsError::NoSolidSurface)
    /// for a body with no solid surface, and
    /// [`NotModelled`](hyperion_sim::planetary::surface::SurfaceInputsError::NotModelled) or
    /// [`NotResolved`](hyperion_sim::planetary::surface::SurfaceInputsError::NotResolved) naming a
    /// section the pass reads that the record does not model or withholds. The service passes
    /// full records, so the last is a bug there, as is every other variant, a fault of the
    /// service's own reading (a planet's record not given or not the moon's, a record not taken
    /// at the record's time, inputs the builder refuses).
    fn inputs(
        &self,
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError>;
}

/// The production source: the body's record, with a moon's planet's, read by
/// [`CoarseInputs::for_body`].
///
/// Until plan 14's surface section is inhabited (P14.T24, P14.T48 and P14.T54.a), every body with a
/// solid surface answers
/// [`SurfaceInputsError::NotModelled`](hyperion_sim::planetary::surface::SurfaceInputsError::NotModelled)
/// naming
/// [`RecordSection::Surface`](hyperion_sim::planetary::record::RecordSection::Surface), a moon
/// once its planet's record is checked, and a body whose surface section is not applicable, a
/// giant's,
/// [`SurfaceInputsError::NoSolidSurface`](hyperion_sim::planetary::surface::SurfaceInputsError::NoSolidSurface),
/// each in [`ReadCoarseInputsError::Section`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordInputs;

impl InputsSource for RecordInputs {
    fn inputs(
        &self,
        record: &BodyRecord,
        planet: Option<&BodyRecord>,
        ctx: &SystemContext,
    ) -> Result<CoarseInputs, ReadCoarseInputsError> {
        CoarseInputs::for_body(record, planet, ctx)
    }
}
