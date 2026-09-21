//! Constant tables: numbers the generator reads but never computes at run time.
//!
//! A table is either exact mathematics written out to the last bit ([`gauss_legendre`]) or the
//! committed output of the offline fitting crate, `hyperion-fit`, under a header naming the tool,
//! its inputs and its version. Every table belongs to the generator version: changing one moves
//! generated output.

pub mod gauss_legendre;
pub mod mge;
