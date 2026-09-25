//! The fitting tasks, one module each. A task has a `fit()` that returns its result and a
//! `render()` that turns the result into the committed Rust source.

pub mod giant_cooling;
pub mod kick_rank;
pub mod mge;
mod render;
pub mod wd_cooling;
