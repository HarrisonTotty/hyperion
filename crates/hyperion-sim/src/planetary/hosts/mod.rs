//! Special hosts: the closed forms behind a body's fate on a young or evolved host (plan 14, phase
//! F).
//!
//! - [`young`]: when each planet forms, below its disc's lifetime (design note 12, P14.T28.a).
//! - [`evolved`]: circularisation, adiabatic expansion, engulfment by an expanding giant, and what
//!   a supernova leaves of an orbit (design note 11, P14.T28.b and c).
//!
//! [`fate`](crate::planetary::fate) strings them together into a body's state at a time. The
//! substellar hosts of P14.T27 and the stripping of P14.T29 will join them here.

pub mod evolved;
pub mod young;
