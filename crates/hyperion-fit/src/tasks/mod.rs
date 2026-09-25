//! The fitting tasks, one module each. Each has a `fit()` that returns its result, a `render()`
//! that turns the result into its table's contents, and a unit struct implementing
//! [`FitTask`](crate::task::FitTask), which [`registry`](crate::task::registry) lists.

pub mod chabrier;
pub mod giant_cooling;
pub mod kick_rank;
pub mod mge;
pub mod wd_cooling;
