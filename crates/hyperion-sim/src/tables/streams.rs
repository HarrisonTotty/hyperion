//! The streams' multiplier: how many globular-cluster streams a galaxy has per globular (plan 15,
//! P15.T11; read by plan 10, Design note 10).
//!
//! @provisional by hyperion-fit 0.1.0, task `streams` revision 0 (P15.T11.a). Do not edit.
//! inputs-sha256: 5b9e18c781c760ab1b96fe7f7e19b781802b810454c2427d96417ac2a8bd9921
//! manifest: crates/hyperion-fit/manifests/streams.toml
//! data: none
//! sim-fingerprint: none
//! since-generator-version: 23
//! source: the brainstorm's value, entered by plan 10's P10.T3.b ("Streams and accreted
//!   structure", `docs/agent/brainstorming/galaxy-generation.md`: "globular streams number about
//!   1.5 per globular, most of them orphans whose cluster is gone")
//! acceptance: scratch: the brainstorm's 1.5, unfitted until P15.T11.b
//!
//! Provisional (P15.T11.a): the brainstorm's value, which plan 10's P10.T3.b entered here by
//! hand as its first reader, before plan 15's P15.T11 (as plan 10's Consumes allows), and which
//! P15.T11.a registered with the fitting toolchain at the same value. P15.T11.b fits it against
//! the thin, globular-like streams of the known census: over 120 streams, wide and thin
//! together, and far from complete (Bonaca and Price-Whelan 2025, New Astron. Rev. 100, 101713,
//! §2.2; Mateu 2023, MNRAS 520, 5225, lists 95). Plan 15 holds the value uncertain threefold.
//! It is a parameter of the generator version whatever the fit says.

/// Globular-cluster streams per globular cluster of the untruncated system, living clusters'
/// tubes and orphans together: 1.5 (brainstorm, "Streams and accreted structure": "globular
/// streams number about 1.5 per globular, most of them orphans whose cluster is gone").
pub const ORPHAN_STREAMS_PER_GLOBULAR: f64 = 1.5;
