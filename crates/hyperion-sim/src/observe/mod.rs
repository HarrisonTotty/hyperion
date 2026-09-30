//! What a sensor sees: every reading of an object at distance d is its state at the retarded time
//! t − d ÷ c (plan 12; brainstorm, "Orbits and time": "What a sensor sees is the past").
//!
//! In a model where every position and state is a function of time, a retarded reading costs one
//! evaluation. This module gives it:
//!
//! - [`Observer`]: a position inside the root cube and a time inside the clock window. A sensor is
//!   an observer; so is a navigation computer asking where a star appears.
//! - [`Trajectory`]: anything whose position is a pure function of time, and [`Drift`], the
//!   straight line every system follows outside the central black hole's Kepler regime.
//! - [`retarded`]: one fixed-point step on the light cone (Design note 1), giving a
//!   [`Retardation`]: the emitted time, the light's age, the apparent position and the velocity
//!   then. [`retarded_from`] is the same step from a present position the caller already has.
//!   [`retarded_exact_linear`] is the drift's closed form, the step's test oracle.
//! - [`curvature_error`]: the stated bound on the error from treating the source's path as
//!   straight over the light's age (Design note 3).
//! - [`extrapolate_to_present`]: the observed position and velocity carried forward to the
//!   observer's present, which for a drifting source lands on the star (the brainstorm's "an
//!   observed position and velocity, extrapolated by the navigation computer and jumped to, land
//!   exactly on the star").
//! - [`observe_hit`] and [`summary_observed`]: a range query's system as its light shows it
//!   ([`ObservedSystem`]), and [`bearing`], the local direction from one point to another. A
//!   member of the galactic centre is refused ([`TraceMotionError`]) until plan 09's P09.T28
//!   builds its orbit.
//! - [`QueryMode`] and [`StarsCache`]: a range query asked in observed mode
//!   ([`range_query_observed`](crate::galaxy::query::range_query_observed)) reports each system
//!   found as its light shows it, taking the systems' stars from the caller's cache
//!   ([`NoStarsCache`] keeps none).
//! - [`retarded_in_system`]: what an observer inside a system ([`SystemObserver`]) sees of a body
//!   or star on a [`SystemTrajectory`], light time and aberration together, iterated to a
//!   nanosecond (rendering plan R03, Design note 7; [`InSystemRetardation`]), with the tracks of a
//!   system's bodies ([`BodyTrack`]) and stars ([`StarTrack`]).
//!
//! Spatial searches are untouched: they find systems on their present positions, and observed
//! mode changes what is reported, never what is found (plan 12, Design note 4). Retarded
//! evaluation is defined back to the start of the source horizon,
//! [`SourceHorizon::START`](crate::time::SourceHorizon::START) = −(H + L), which is the earliest
//! emitted time any observer inside the cube can receive during play.
//!
//! Nothing here changes generated output: observation only reads.

mod bearing;
mod error;
mod in_system;
mod retarded;
mod stars_cache;
mod system;

pub use crate::galaxy::query::QueryMode;
pub use bearing::{AXIS_FRAME_RADIUS_LY, Bearing, BearingFrame, bearing};
pub use error::{CurvatureError, curvature_error};
pub use in_system::{
    BodyTrack, BuildSystemObserverError, IN_SYSTEM_LIGHT_TIME_TOLERANCE, IN_SYSTEM_MAX_CORRECTIONS,
    InSystemRetardation, StarTrack, SystemObserver, SystemTrajectory, TraceInSystemError,
    retarded_in_system,
};
pub use retarded::{
    BuildObserverError, Drift, Motion, Observer, Retardation, TraceMotionError, Trajectory,
    extrapolate_to_present, light_time, retarded, retarded_exact_linear, retarded_from,
};
pub use stars_cache::{NoStarsCache, StarsCache, stars_of};
pub(crate) use system::observe_on_line;
pub use system::{ObservedSystem, observe_hit, summary_observed};
