//! The streams' multiplier: how many globular-cluster streams a galaxy has per globular (plan 15,
//! P15.T11; read by plan 10, Design note 10).
//!
//! **Provisional: the brainstorm's value, hand-entered by plan 10** (P10.T3.b, its first reader,
//! runs before plan 15's P15.T11, as plan 10's Consumes allows). It is not a fitted table yet, so
//! no fit header and no [`MANIFEST`](super::MANIFEST) entry: P15.T11.a takes this module over
//! unchanged, and P15.T11.b fits the value against the census of thin streams (over 120 known and
//! far from complete; Bonaca and Price-Whelan 2025, Mateu 2023), uncertain threefold. It is a
//! parameter of the generator version whatever the fit says.

/// Globular-cluster streams per globular cluster of the untruncated system, living clusters'
/// tubes and orphans together: 1.5 (brainstorm, "Streams and accreted structure": "globular
/// streams number about 1.5 per globular, most of them orphans whose cluster is gone").
pub const ORPHAN_STREAMS_PER_GLOBULAR: f64 = 1.5;
