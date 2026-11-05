//! Plan 11's binary catalogue classes (plan 11, P11.T8): hosts of interacting binaries carved out
//! of the cells and placed on class grids of their own under the `111` prefix.
//!
//! The grid redraws every binary that falls into one of plan 11's carved classes (P11.T7,
//! [`carved_class`](crate::stellar::binary::carved_class)), so the catalogue holds them, each class
//! with its own [`ClassProcess`](super::ClassProcess): a bound from its row of the share table, a
//! candidate drawn present first through a
//! [`ClassSampler`](crate::stellar::binary::sampler::ClassSampler), its scan marks before the rest
//! (`stellar::binary::scan`), a timeline from its marks, and the test that the grid applies
//! too. Built so far: the accreting white dwarfs, fast and slow (P11.T8.a, [`AccretingWdFast`],
//! [`AccretingWdSlow`]), the X-ray binaries (P11.T8.b, [`XrayBinaries`]), and the stellar mergers
//! (P11.T8.c, [`StellarMergers`]) and the neutron-star mergers (P11.T8.d, [`NeutronStarMergers`]),
//! which thin on [`ClassHosts`]. From generator version 27 the same classes are listed inside
//! features too (P11.T8.f, the crate's `feature` module): a candidate is drawn at a [`BinaryHost`],
//! a catalogue cell's site or a feature's list entry ([`ListedBinary`]), and the catalogue gives
//! up the features' share of its hosts.
//!
//! # The fitted tables
//!
//! Plan 15's P15.T10.b fits the accreting white dwarfs' and the X-ray binaries' tables against the
//! engine, on the census of the pairs the generator itself draws (refitted at generator version
//! 29): `tables::binary`'s samplers, one for each accreting class and the X-ray binaries', and
//! their rows of the share table, read where the galaxy is built (the share matrix's class shares)
//! and where a class's process is built.

mod awd;
pub(crate) mod feature;
#[cfg(test)]
mod fitted_tables;
mod hosts;
mod merger;
mod mergers_hold;
mod nsm;
mod xrb;

pub use awd::{
    AccretingWdFast, AccretingWdSlow, AwdBinary, AwdCandidate, AwdComponent, AwdEntry, AwdHistory,
    AwdProcess, AwdSide, GIANT_DONOR_CORE_MASS, GIANT_DONOR_TEMPERATURE_K,
    HELIUM_DONOR_TEMPERATURE_K, LIFETIME_MARGIN, OXYGEN_NEON_MIN_MASS,
};
pub use feature::{BinaryHost, HostPlace, ListedBinary, ListedHost, is_listed_binary};
pub use hosts::{ClassHosts, HostComponent};
pub use merger::{MergerCandidate, MergerEntry, MergerPair, StellarMergers};
pub(crate) use mergers_hold::MergerTableRules;
pub use nsm::{Inspiral, NeutronStarMergers, NsmCandidate, NsmEntry};
pub use xrb::{
    BE_DONOR_ATTEMPTS, DONOR_TURN_OFF_SHARE, EVOLVED_DONOR_LOBE_SHARE, EVOLVED_DONOR_MIN_CORE_MASS,
    SUPERGIANT_DONOR_CORE_SHARE, SUPERGIANT_DONOR_LIFE_SHARE, XrayBinaries, XrbBinary,
    XrbCandidate, XrbComponent, XrbEntry, XrbHistory, XrbSide,
};
pub(crate) use xrb::{compact_radius, symbiotic_giant_state};
