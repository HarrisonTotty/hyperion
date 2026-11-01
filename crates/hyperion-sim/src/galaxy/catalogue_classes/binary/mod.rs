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
//! features too (P11.T8.f, [`feature`]): a candidate is drawn at a [`BinaryHost`], a catalogue
//! cell's site or a feature's list entry, and the catalogue gives up the features' share of its
//! hosts.
//!
//! # The fitted tables' hold
//!
//! Plan 15's P15.T10.b fits the accreting white dwarfs' and the X-ray binaries' tables against the
//! engine: `tables::binary`'s `_26` items, a sampler for each accreting class and the X-ray
//! binaries', and their rows of the share table.
//! They move the classes' members, so they are held until generator version 27
//! (`BINARY_TABLES_VERSION`): a galaxy below it reads the scratch tables of P11.T7–T8, and one
//! under `BinaryTableRules::Fitted` the fitted ones, chosen at one place, `BinaryTableRules`' two
//! readers, and read where the galaxy is built (the share matrix's class shares) and where a
//! class's process is built.

mod awd;
pub(crate) mod feature;
mod hosts;
mod merger;
mod nsm;
#[cfg(test)]
mod tables_hold;
mod xrb;

use crate::GeneratorVersion;
use crate::tables::binary::{
    AWD_FAST_SAMPLER_26, AWD_SAMPLER, AWD_SLOW_SAMPLER_26, AwdMark, AwdPick, CLASS_SHARES,
    CLASS_SHARES_26, ClassSamplerTable, ClassShareTable, XRB_SAMPLER, XRB_SAMPLER_26, XrbMark,
    XrbPick,
};

pub use awd::{
    AccretingWdFast, AccretingWdSlow, AwdBinary, AwdCandidate, AwdComponent, AwdEntry, AwdHistory,
    AwdProcess, AwdSide, GIANT_DONOR_CORE_MASS, GIANT_DONOR_TEMPERATURE_K,
    HELIUM_DONOR_TEMPERATURE_K, LIFETIME_MARGIN, OXYGEN_NEON_MIN_MASS,
};
pub use feature::{BinaryHost, HostPlace, ListedHost};
pub use hosts::{ClassHosts, HostComponent};
pub(crate) use merger::{
    MERGER_CHANNELS_VERSION, MERGER_CLASS_VERSION, MergerChannelRules, MergerClassRules,
};
pub use merger::{MergerCandidate, MergerEntry, MergerPair, StellarMergers};
pub use nsm::{Inspiral, NeutronStarMergers, NsmCandidate, NsmEntry};
pub use xrb::{
    BE_DONOR_ATTEMPTS, DONOR_TURN_OFF_SHARE, EVOLVED_DONOR_LOBE_SHARE, EVOLVED_DONOR_MIN_CORE_MASS,
    SUPERGIANT_DONOR_CORE_SHARE, SUPERGIANT_DONOR_LIFE_SHARE, XrayBinaries, XrbBinary,
    XrbCandidate, XrbComponent, XrbEntry, XrbHistory, XrbSide,
};

/// The generator version from which the binary classes read plan 15's fitted tables (P15.T10.b):
/// 27, by main's ruling of 2026-11-01, since their nova rate, black-hole transients and high-mass
/// hosts are wrong states that research reviews first. Below it every galaxy takes
/// [`BinaryTableRules::Scratch`], the code of version 26 verbatim. The bump to 27 deletes this
/// hold: the constant, [`BinaryTableRules`] with its
/// `Scratch` arm, `GalaxyRules::binary_tables` and the tripwire in `tables_hold`, renames the
/// fitted items in place of the scratch ones and refits them at 27.
pub(crate) const BINARY_TABLES_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Which tables the binary classes read (the hold of [`BINARY_TABLES_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum BinaryTableRules {
    /// The scratch tables of P11.T7–T8 (plan 11's design note 13): [`CLASS_SHARES`],
    /// [`AWD_SAMPLER`] for both accreting white dwarf classes and [`XRB_SAMPLER`].
    Scratch,
    /// Plan 15's fitted tables (P15.T10.b): [`CLASS_SHARES_26`], a sampler a side,
    /// [`AWD_FAST_SAMPLER_26`] and [`AWD_SLOW_SAMPLER_26`], and [`XRB_SAMPLER_26`].
    Fitted,
}

impl BinaryTableRules {
    /// The share table the galaxy's class shares and the classes' processes read.
    #[must_use]
    pub(crate) const fn class_shares(self) -> &'static ClassShareTable {
        match self {
            Self::Scratch => &CLASS_SHARES,
            Self::Fitted => &CLASS_SHARES_26,
        }
    }

    /// The sampler the accreting white dwarfs of `side` draw from.
    #[must_use]
    pub(crate) const fn awd_sampler(
        self,
        side: AwdSide,
    ) -> &'static ClassSamplerTable<AwdMark, AwdPick> {
        match (self, side) {
            (Self::Scratch, AwdSide::Fast | AwdSide::Slow) => &AWD_SAMPLER,
            (Self::Fitted, AwdSide::Fast) => &AWD_FAST_SAMPLER_26,
            (Self::Fitted, AwdSide::Slow) => &AWD_SLOW_SAMPLER_26,
        }
    }

    /// The sampler the X-ray binaries draw from.
    #[must_use]
    pub(crate) const fn xrb_sampler(self) -> &'static ClassSamplerTable<XrbMark, XrbPick> {
        match self {
            Self::Scratch => &XRB_SAMPLER,
            Self::Fitted => &XRB_SAMPLER_26,
        }
    }
}
