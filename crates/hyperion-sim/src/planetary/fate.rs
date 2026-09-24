//! What has become of a body at a time: the states that plan 14's fate transform produces (design
//! notes 1, 11 and 12).
//!
//! A system is generated as it was born, and time enters only through the fate transform:
//! `fate::state_at(body, ctx, t)`, P14.T28, turns a body's primordial elements into its state and
//! elements at a time. That transform is not built yet. This module holds the states it will
//! produce, which a body's record carries (P14.T34), so that the record and its wire form (T35)
//! have every state from the start. Until T28, every generated body is [`BodyState::Present`].

use crate::time::UniverseTime;

/// A body's state at a time: a prefix of not yet formed → present → destroyed or unbound
/// (P14.T28's test).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BodyState {
    /// The body has not formed yet: a giant before its formation age, a small planet before its
    /// disc's lifetime, or any body of a system not yet born (design note 12).
    NotYetFormed,
    /// The body exists and orbits as its elements say.
    Present,
    /// The body was destroyed at `at`.
    Destroyed {
        /// What destroyed it.
        cause: DestructionCause,
        /// When.
        at: UniverseTime,
    },
    /// The body was unbound from its system at `at`, and is no longer tracked (design note 11:
    /// the rogue-planet layer counts escapers statistically).
    Unbound {
        /// When.
        at: UniverseTime,
    },
}

/// What destroyed a body (design notes 11 and 12).
///
/// A moon destroyed with its planet takes its planet's cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DestructionCause {
    /// A protoplanetary disc dispersed at the end of its lifetime (P14.T28.a).
    Dispersed,
    /// The host expanded over the body's orbit: a planet is engulfed once its semi-major axis is
    /// inside 2–3 host radii (Mustill and Villaver 2012; P14.T28.b).
    Engulfed,
    /// The body's pericentre fell inside its primary's Roche limit, as after a supernova that
    /// leaves a planet on a plunging orbit about the remnant (P14.T28.c).
    TidallyDisrupted,
}
