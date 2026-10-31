//! Plan 11's binary catalogue classes (plan 11, P11.T8): hosts of interacting binaries carved out
//! of the cells and placed on class grids of their own under the `111` prefix.
//!
//! The grid redraws every binary that falls into one of plan 11's carved classes (P11.T7,
//! [`carved_class`](crate::stellar::binary::carved_class)), so the catalogue holds them, each class
//! with its own [`ClassProcess`](super::ClassProcess): a bound from its row of the share table, a
//! candidate drawn present first through a
//! [`ClassSampler`](crate::stellar::binary::sampler::ClassSampler), its scan marks before the rest
//! (`stellar::binary::scan`), a timeline from its marks, and the test that the grid applies
//! too. Built so far: the accreting white dwarfs, fast and slow
//! (P11.T8.a, [`AccretingWdFast`], [`AccretingWdSlow`]), and the stellar mergers (P11.T8.c,
//! [`StellarMergers`]), which thin on [`ClassHosts`]. The X-ray binaries and the neutron-star
//! mergers follow in P11.T8.b and T8.d.

mod awd;
mod hosts;
mod merger;

pub use awd::{
    AccretingWdFast, AccretingWdSlow, AwdBinary, AwdCandidate, AwdComponent, AwdEntry, AwdHistory,
    AwdProcess, AwdSide, GIANT_DONOR_CORE_MASS, GIANT_DONOR_TEMPERATURE_K,
    HELIUM_DONOR_TEMPERATURE_K, LIFETIME_MARGIN, OXYGEN_NEON_MIN_MASS,
};
pub use hosts::{ClassHosts, HostComponent};
pub use merger::{ContactPair, MergerCandidate, MergerEntry, StellarMergers};
