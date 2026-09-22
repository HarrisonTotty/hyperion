//! Why a computation the server needed did not arrive.

use std::error::Error;
use std::fmt;

use hyperion_sim::galaxy::placement::ExceedIndexCapacityError;

use super::{JobError, SubmitJobError};

/// A value the server could not compute: the CPU pool would not take the job, the job did not
/// finish, or the galaxy it needed cannot be played.
///
/// Every cached computation ([`GalaxyCache`](super::GalaxyCache) and the map and cell caches that
/// follow it) fails this way and no other, because the work itself is pure: a galaxy or a map is a
/// function of its key. It is `Clone` so that the waiters on one
/// [`SingleFlight`](super::SingleFlight) can share it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComputeError {
    /// The pool refused the job.
    Submit(SubmitJobError),
    /// The job was refused, cancelled or lost before it produced a value.
    Job(JobError),
    /// The galaxy was built, and a layer of it is too dense for its system IDs, so nothing may be
    /// generated in it (plan 03, design note 6).
    UnplayableGalaxy(ExceedIndexCapacityError),
}

impl fmt::Display for ComputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Submit(_) => f.write_str("the computation could not be queued"),
            Self::Job(_) => f.write_str("the computation did not finish"),
            Self::UnplayableGalaxy(_) => f.write_str("the galaxy cannot be generated"),
        }
    }
}

impl Error for ComputeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Submit(source) => Some(source),
            Self::Job(source) => Some(source),
            Self::UnplayableGalaxy(source) => Some(source),
        }
    }
}

impl From<ExceedIndexCapacityError> for ComputeError {
    fn from(error: ExceedIndexCapacityError) -> Self {
        Self::UnplayableGalaxy(error)
    }
}

impl From<SubmitJobError> for ComputeError {
    fn from(error: SubmitJobError) -> Self {
        Self::Submit(error)
    }
}

impl From<JobError> for ComputeError {
    fn from(error: JobError) -> Self {
        Self::Job(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_cause_keeps_its_source() {
        let submit = ComputeError::from(SubmitJobError::QueueFull);
        assert_eq!(submit, ComputeError::Submit(SubmitJobError::QueueFull));
        assert_eq!(
            submit.source().map(ToString::to_string).as_deref(),
            Some("the cpu pool's queue is full")
        );
        let job = ComputeError::from(JobError::Cancelled);
        assert_eq!(job.to_string(), "the computation did not finish");
        assert_eq!(
            job.source().map(ToString::to_string).as_deref(),
            Some("the job was cancelled")
        );
        let too_dense = ComputeError::from(ExceedIndexCapacityError::LayerTooDense {
            layer: hyperion_sim::id::Layer::A,
            largest_mean: 65_000.0,
            capacity: 65_536,
        });
        assert_eq!(too_dense.to_string(), "the galaxy cannot be generated");
        assert!(
            too_dense
                .source()
                .is_some_and(|source| source.to_string().contains("layer A's densest cell")),
            "{too_dense:?}"
        );
    }
}
