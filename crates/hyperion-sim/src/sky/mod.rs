//! The sky as an observer sees it (rendering plan R06): which stars are visible from a place and
//! a time, how bright and what colour they are, the light of those too faint to list, and the
//! discs of the observer's own stars.
//!
//! Everything here reads the galaxy and the committed tables and only reads: no output of the
//! generator depends on it, and it opens no random stream.

pub mod colour;
pub mod disc;
