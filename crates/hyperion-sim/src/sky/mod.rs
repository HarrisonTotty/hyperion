//! The sky an observer sees: which stars the galaxy holds brighter than a limit, how bright and
//! what colour each appears at its retarded time, the unresolved band of the rest, and the
//! naked eye's limit per direction (rendering plan R06).
//!
//! - [`eye`]: the naked eye's threshold against a background (Crumey 2014, eq. 34, with its colour
//!   corrections) and the veiling glare of bright stars (CIE 146:2002), and [`MAX_CUT_V`], the
//!   deepest cut a sky is asked to.
//!
//! Everything here only reads the galaxy: nothing changes generated output, and nothing draws a
//! random word.

pub mod eye;

pub use eye::{EyeObserver, MAX_CUT_V, REFERENCE_SP_RATIO};
