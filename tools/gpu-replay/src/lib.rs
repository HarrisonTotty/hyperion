//! The descent spike's native replayer (plan R05, T15.b and T15.c): reads a capture of the
//! client's WebGPU calls, validates its WGSL with naga and replays its frames in wgpu, so that the
//! browser's cost can be priced against native (Design note 22).

pub mod capture;
mod clocks;
pub mod replay;
pub mod results;
pub mod run;
pub mod validate;
mod window;
