//! Interacting binaries (plan 11, P11.T4): the binary evolution engine of Hurley, Tout and Pols
//! (2002, MNRAS 329, 897, "BSE"), run forward once into a timeline from which the pair's state at
//! any age is read.
//!
//! A pair close enough to interact ([`can_interact`], plan 11's design note 7) is evolved by
//! [`evolve`] from zero age to the age asked for, event to event: detached evolution under winds
//! and their accretion, tides, magnetic braking and gravitational radiation (`detached.rs`, BSE
//! sections 2.1–2.4); Roche-lobe overflow, stable or dynamical (`rlof.rs`, section 2.6); common
//! envelopes, coalescence and collisions (`common_envelope.rs`, section 2.7); supernovae and their
//! kicks (`supernova.rs`, section 2.5 and appendix A1). The result, a [`BinaryTimeline`], is an
//! ordered list of [`Segment`]s whose state at any age is a lookup and closed forms (design note
//! 6): each star's own single-star closed forms (plan 06) at the mass and age the binary gives it,
//! and the orbit on nodes.
//!
//! Every star is plan 06's: a star that has not interacted follows its own track bit for bit, a
//! star whose mass the binary changes keeps its track's closed forms (HPT section 7.1), and a star
//! stripped to its helium core follows the helium-star track ruling 34.1 gave plan 06
//! ([`Track::helium_star`](crate::stellar::sse::Track::helium_star)). A primary heavy enough for
//! its death to be read by placement dies when plan 06 says (design note 16).
//!
//! The engine's parameters are [`BinaryParams`], BSE's table 3 with the generator's defaults
//! (design note 14). Nothing generated calls the engine yet: plan 11's P11.T6–T11 wire it into the
//! system stage.

mod common_envelope;
mod detached;
mod evolve;
mod params;
mod rlof;
mod star;
mod supernova;
mod timeline;

#[cfg(test)]
mod tests;

pub use crate::stellar::multiplicity::{DRAWS_PER_ATTEMPT, MAX_REDRAWS, RedrawAttempt};
pub use evolve::{MAX_SEGMENTS, can_interact, evolve};
pub use params::BinaryParams;
pub use timeline::{
    BinaryInput, BinaryState, BinaryTimeline, BuildBinaryInputError, Component, IaPoolChannel,
    PooledIaEvent, Segment, SegmentKind, SupernovaRecord,
};

/// What becomes of a merged binary in a cluster (plan 11, design note 14): the fourth of the kick
/// law's defaults that the brainstorm's "Open questions" lists, which P15.T5.c reviews.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MergedBinaryFate {
    /// The merged pair stays a member, judged on the pair's velocity.
    StaysJudgedOnPairVelocity,
    /// It stays or leaves by its own kick.
    StaysJudgedOnKick,
    /// It is ejected.
    Ejected,
}

/// The generator's default for a merged binary in a cluster (plan 11, design note 14): it stays a
/// member and is judged on the pair's velocity, until P15.T5.c's review.
pub const CLUSTER_MERGED_BINARY_FATE: MergedBinaryFate =
    MergedBinaryFate::StaysJudgedOnPairVelocity;
