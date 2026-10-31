//! Special hosts: the closed forms behind a body's fate on a young or evolved host, and the
//! substellar hosts (plan 14, phase F).
//!
//! - [`young`]: when each planet forms, below its disc's lifetime (design note 12, P14.T28.a).
//! - [`evolved`]: circularisation, adiabatic expansion, engulfment by an expanding giant, and what
//!   a supernova leaves of an orbit (design note 11, P14.T28.b and c).
//! - [`substellar`]: a free-floating brown dwarf's planets (design note 13, P14.T27.a), behind
//!   their hold at generator version 26.
//!
//! [`fate`](crate::planetary::fate) strings them together into a body's state at a time. The
//! rogue planets of P14.T27.b and the stripping of P14.T29 will join them here.

pub mod evolved;
pub mod substellar;
pub mod young;
