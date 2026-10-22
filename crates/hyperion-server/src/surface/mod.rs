//! The surface service: each body's coarse field, computed once on the CPU pool and kept in memory
//! (rendering plan R09, R09.T17; Design note 19).
//!
//! A body's coarse field is the sim's
//! [`coarse_pass`](hyperion_sim::planetary::surface::coarse_pass) of its surface seed and its
//! inputs, plan 14's global figures for the body, so it is a function of the universe and the body
//! alone. [`SurfaceService`] keys it by [`FieldKey`], the universe's
//! [`GalaxyKey`](crate::compute::GalaxyKey) and the body's ID, and keeps each field as an
//! `Arc<CoarseField>` in a [`FieldCache`], a byte-bounded LRU of its own budget,
//! `HYPERION_SURFACE_CACHE_MB` (128 MiB by default, `--surface-cache`; [`config`](crate::config)).
//! Callers that ask for one body's field at once share one pass through a
//! [`SingleFlight`](crate::compute::SingleFlight), and the pass runs as one bulk job on the pool,
//! never on the runtime, under a [`CancelToken`](crate::compute::CancelToken) raised when every one
//! of them has given up: a pass still queued is then skipped, and a field nobody waits for any
//! more is not kept. A field is never persisted (the plan's open question 4): after an eviction or
//! a restart it is computed again, to the bit.
//!
//! # Where the inputs come from
//!
//! The pass's inputs come through the [`InputsSource`] seam, which reads them from the body's
//! record, a moon's planet's record and its system's context. The service fetches the planet's
//! record, the one
//! [`CoarseInputs::planet_of`](hyperion_sim::planetary::surface::CoarseInputs::planet_of) names,
//! beside the body's, so that both come from one place at one time (R09.T10.f). In production the
//! source is [`RecordInputs`], plan 14's records read by
//! [`CoarseInputs::for_body`](hyperion_sim::planetary::surface::CoarseInputs::for_body), behind
//! the serve gate: it answers every body that reads
//! [`SurfaceInputsError::NotModelled`](hyperion_sim::planetary::surface::SurfaceInputsError::NotModelled)
//! naming the surface section until the gate opens (`GENERATED_SURFACES_FROM`, 24; P14.T54.a).
//! Until then the service and the wire are tested on the coarse pass's reference worlds, given as
//! the source through
//! [`ServerConfigBuilder::surface_inputs`](crate::ServerConfigBuilder::surface_inputs). A source's
//! refusal other than a section's state
//! ([`ReadCoarseInputsError::Section`](hyperion_sim::planetary::surface::ReadCoarseInputsError::Section))
//! is a fault of the server's own reading, which a request answers `internal`.
//!
//! The record is read at [`RECORD_TIME`], the epoch, whoever asks first and whenever: the field is
//! a function of its key, and a record read at the asker's time would make the field kept depend on
//! who asked first. P14.T54.a, which settles the time the record's surface section is read at (the
//! hosts' light and a despinning body's spin change with it), settles this one too (R09's Risks).
//!
//! # The albedo scale
//!
//! Each field leaves the pass with R10's albedo scale in its header
//! ([`field_albedo_scale`], R10.T10.a): the one scale on the body's class albedos that makes its
//! materials, over their area fractions on every cell, give the body's geometric albedo in V from
//! its record's photometry, so that the terrain's brightness never shifts as the survey grows. It
//! is `None`, the palette's albedos unscaled, wherever the ground is not what the body's p
//! measures: where the photometry is not modelled, where the air hides the ground (30 kPa and up),
//! where the photometry's law is a cloud deck's at any pressure, and where the palette names no
//! substance.
//!
//! # What starts a field
//!
//! The first `surface_field` or `survey_pass` for the body (R09.T19, which wires them), and
//! [`SurfaceService::prefetch`], which session code will call as the ship approaches a body. A
//! `survey_pass` first reads the field's level on an interactive job
//! (`SurfaceService::field_level`) and starts the field only where the body's inputs are
//! modelled.
//!
//! # The survey logs
//!
//! Nothing holds a universe's Knowledge yet (P12.T8 will), so the service opens each universe's
//! [`SurveyLog`](crate::knowledge::surveys::SurveyLog) on its first use and keeps it
//! (`SurfaceService::survey_log`): a universe's survey file has one writer (R09.T18).
//!
//! # The floating-point mode
//!
//! The pass runs on R04's probed pool. A worker whose floating-point mode is found to flush
//! subnormals withholds the field
//! ([`JobError::FloatingPointMode`](crate::compute::JobError::FloatingPointMode)), and the service
//! logs the refusal at `error`, naming the body; a request answers it `internal`, as every job's
//! refusal is ([`ComputeError`](crate::compute::ComputeError)'s conversion).

mod cache;
mod inputs;
mod service;

pub use cache::{FieldCache, FieldKey};
pub use inputs::{InputsSource, RecordInputs};
pub(crate) use service::FieldLevel;
pub use service::{
    RECORD_TIME, SurfaceCounters, SurfaceJobError, SurfaceService, field_albedo_scale,
};
