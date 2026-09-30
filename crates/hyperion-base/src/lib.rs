//! HYPERION's determinism foundation, beneath the simulation and the surface crate.
//!
//! What every deterministic crate of the workspace builds on, and what the client's WebAssembly
//! module shares with the server bit for bit:
//!
//! - [`math`]: every transcendental function, on the exactly pinned `libm`.
//! - [`units`]: unit newtypes over `f64` and the physical constants between them.
//! - [`version`]: [`GENERATOR_VERSION`], half of what identifies a universe.
//! - [`rng`]: random streams keyed by seed, domain tag and object, the samplers and decisions,
//!   raw event keys, and the foundation's own registry of domain tags, with the
//!   [`domain_tags!`] macro every registry is declared with.
//! - [`hex`]: the parser behind every 16-digit text form, a [`Seed`]'s among them.
//!
//! `hyperion-sim` re-exports each of these at its old path (its crate root's `math`, `rng` and the
//! rest), so code above the sim names them as it always did; code in this crate and in
//! `hyperion-surface`, which cannot see the sim, names them here.
//!
//! # Boundary
//!
//! This crate does no I/O, reads no clocks, spawns no threads and holds no caches: its functions
//! are pure. `libm`, pinned with `=`, is its only runtime dependency, and a new one is a reviewed
//! change, since a dependency could bring platform maths or relaxed SIMD with it. It never depends
//! on another workspace crate.
//!
//! # Relaxed SIMD
//!
//! WebAssembly's relaxed-SIMD instructions leave some results to the implementation, so the same
//! module could compute different bits on different machines. A build that enables the feature
//! fails here (plan R04, Design note 10).

#[cfg(target_feature = "relaxed-simd")]
compile_error!(
    "relaxed SIMD is banned in hyperion-base: its results are implementation-defined, so a build \
     with `+relaxed-simd` would not give the same bits on every machine"
);

pub mod hex;
pub mod math;
pub mod rng;
pub mod units;
pub mod version;

pub use rng::Seed;
pub use version::{GENERATOR_VERSION, GeneratorVersion};
