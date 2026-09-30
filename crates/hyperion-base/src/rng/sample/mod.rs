//! The samplers: words of a [`Stream`](super::Stream) turned into values of a distribution.
//!
//! Every sampler is a method of `Stream` or takes one, consumes a documented number of words, and
//! is a fixed sequence of IEEE operations and [`math`](crate::math) calls. The order of those
//! operations is part of the generator version, so none of them is ever reordered, fused or
//! replaced by an "equivalent" expression without a version bump.

mod normal;
mod piecewise;
mod poisson;
mod power_law;
mod uniform;

pub use piecewise::{BuildPiecewiseError, PiecewiseLinear, PiecewisePowerLaw};
pub use poisson::{POISSON_MAX_MEAN, POISSON_PTRS_MIN_MEAN};
pub use power_law::{BuildPowerLawError, PowerLaw};
#[cfg(test)]
pub(super) use uniform::uniform_of;
