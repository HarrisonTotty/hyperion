//! Test helpers shared by every HYPERION crate that generates content.
//!
//! This crate is a dev-dependency only. It holds what the galaxy generator's tests need and the
//! generator itself must not contain:
//!
//! - [`golden`](mod@golden): the golden-file harness that pins generated output bit for bit.
//! - [`order`]: the order-independence check ("A then B equals B then A equals B alone").
//! - [`stats`]: hand-written statistical tests (chi-square, Kolmogorov–Smirnov, Poisson counts).
//! - [`lcg`]: a small fixed generator for test inputs, independent of the sim's own streams.
//! - [`float`]: bit-exact float comparison, which `hyperion-sim` may not do itself because its
//!   Clippy configuration forbids `f64::to_bits`.
//!
//! It depends on the exactly pinned `libm` and on nothing else. It never depends on
//! `hyperion-sim`: that cycle would compile the sim's types twice inside the sim's own unit tests.

pub mod float;
pub mod golden;
pub mod lcg;
pub mod order;
pub mod stats;
