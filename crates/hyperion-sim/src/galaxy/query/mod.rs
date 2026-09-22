//! The range query: which systems lie within R light-years of a point at time t (plan 03).
//!
//! The brainstorm's "The range query". The query walks the stellar layers from the coarsest to the
//! finest, visiting only the cells that meet the sphere, and returns a complete census or nothing
//! per layer: whether a layer fits under the caller's limit is decided before anything is
//! generated, from its expected count over the sphere, so the answer never depends on what a
//! caller happens to have cached. Cells are chosen by epoch position, so the sphere is padded by
//! the largest speed times |t|; distances are then tested at t and systems not yet born are
//! dropped. Sources other than the grid (features, the global list, catalogue classes, pinned
//! content) are merged through a hook that later plans fill.
//!
//! Queries take a time inside the [`ClockWindow`](crate::time::ClockWindow), within
//! ±[`CLOCK_WINDOW_H`](crate::time::CLOCK_WINDOW_H) = ±1,000 Julian years of the epoch, where
//! present positions are guaranteed (plan 03, Design note 12).

mod census;
mod request;
mod result;
mod walk;

use std::error::Error;
use std::fmt;

pub use census::decide_census;
pub use request::{
    DEFAULT_CELL_BUDGET, DEFAULT_CENSUS_LIMIT, MassFloor, RangeQuery, RangeQueryBuilder,
    SubstellarRequest,
};
pub use result::{Census, CensusStop, LayerCounts, LayerSet, QueryStats};
pub use walk::{BuildQuerySphereError, QuerySphere, cells_in_sphere, count_cells_in_sphere};

use crate::time::UniverseTime;

/// A range query's parameters were rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildRangeQueryError {
    /// The radius is NaN or infinite.
    RadiusNotFinite,
    /// The radius is zero or negative.
    RadiusNotPositive,
    /// The radius exceeds the root cube's diagonal, 131,072 × √3 ≈ 227,023 ly, beyond which a
    /// sphere inside the cube can hold nothing more.
    RadiusBeyondRootCube,
    /// The centre lies outside the root cube.
    CentreOutsideRootCube,
    /// The time lies outside the [`ClockWindow`](crate::time::ClockWindow), ±1,000 Julian years
    /// about the epoch, where present positions are guaranteed.
    TimeOutsideClockWindow(UniverseTime),
    /// Substellar layers were asked for; plan 13 places them.
    SubstellarLayersUnavailable,
}

impl fmt::Display for BuildRangeQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RadiusNotFinite => f.write_str("the query radius is not finite"),
            Self::RadiusNotPositive => f.write_str("the query radius is not positive"),
            Self::RadiusBeyondRootCube => {
                f.write_str("the query radius exceeds the root cube's diagonal")
            }
            Self::CentreOutsideRootCube => {
                f.write_str("the query centre lies outside the root cube")
            }
            Self::TimeOutsideClockWindow(t) => {
                write!(f, "the query time {t} lies outside the clock window")
            }
            Self::SubstellarLayersUnavailable => {
                f.write_str("substellar layers are not generated yet")
            }
        }
    }
}

impl Error for BuildRangeQueryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        let t = UniverseTime::from_julian_years(2_000).unwrap();
        for error in [
            BuildRangeQueryError::RadiusNotFinite,
            BuildRangeQueryError::RadiusNotPositive,
            BuildRangeQueryError::RadiusBeyondRootCube,
            BuildRangeQueryError::CentreOutsideRootCube,
            BuildRangeQueryError::TimeOutsideClockWindow(t),
            BuildRangeQueryError::SubstellarLayersUnavailable,
        ] {
            let message = error.to_string();
            let first = message.chars().next().unwrap();
            assert!(!first.is_uppercase(), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
    }
}
