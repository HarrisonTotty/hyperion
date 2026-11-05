//! The hold on plan 15's binary class tables refitted on the generator's own pairs (P15.T10.b's
//! refit at [`PUBLISHED_CENSUS_VERSION`]; train 27's finding).
//!
//! Train 27's refit fitted the accreting white dwarfs' and the X-ray binaries' tables to a census
//! (`stellar::binary::testing::class_census_in`) that drew every record's first attempt under
//! `Free` multiplicity on its primary's own draws. The generator shows a displaced record single
//! and runs a retained one's pair on its primary's kept attempt of plan 08's kick loop, so the
//! census counted pairs the grid never holds: of a probe census of 9,409 young-thin-disc records
//! of layer E, 52 of its 113 members were displaced remnants and 14 of the other 61 differed from
//! the generator's. The census now draws the generator's first attempt, and tasks `binary_awd` and
//! `binary_xrb` refit on it into their blocks, the `_29` items of `tables::binary`.
//!
//! Below 29 every galaxy reads the version's tables, train 27's refit as train 28 ran it again with
//! its bodies unchanged, frozen in `tables::binary`'s `refit_27` module
//! ([`BinaryTableRules::RawCensus`]), so that every generated value keeps its bits; from 29 it
//! reads the refit's ([`BinaryTableRules::Published`]). The tests reach the new arm through
//! `GalaxyRules::every_29_hold()`. The bump to 29 deletes this hold: this module with its tripwire,
//! the galaxy rules' `binary_tables` field, `tables::binary`'s `refit_27` module and the items that
//! read it (`AWD_FAST_SAMPLER`, `AWD_SLOW_SAMPLER`, `XRB_SAMPLER`, `CLASS_SHARES` and their marks),
//! whose `_29` items it renames over them, and re-pins what moves.

use crate::GeneratorVersion;
use crate::galaxy::catalogue_classes::binary::AwdSide;
use crate::tables::binary::{
    AWD_FAST_SAMPLER, AWD_FAST_SAMPLER_29, AWD_SLOW_SAMPLER, AWD_SLOW_SAMPLER_29, AwdMark, AwdPick,
    CLASS_SHARES, CLASS_SHARES_29, ClassSamplerTable, ClassShareTable, XRB_SAMPLER, XRB_SAMPLER_29,
    XrbMark, XrbPick,
};

/// The generator version from which a galaxy reads plan 15's binary class tables refitted on the
/// census of the generator's own first attempts (P15.T10.b's refit at 29).
pub(crate) const PUBLISHED_CENSUS_VERSION: GeneratorVersion = GeneratorVersion::new(29);

/// Which of plan 15's fitted binary class tables a galaxy reads (the galaxy rules'
/// `binary_tables` field, the hold of [`PUBLISHED_CENSUS_VERSION`]): its share matrix's class
/// shares where it is built, and its classes' samplers and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum BinaryTableRules {
    /// Versions 27 and 28's: the tables train 27 fitted, and train 28 ran again unchanged, on a
    /// census of every record's first attempt under `Free` multiplicity on its primary's own draws.
    RawCensus,
    /// Version 29's: the tables refitted on the census of the pairs the generator itself draws at
    /// a record's first attempt.
    Published,
}

impl BinaryTableRules {
    /// The arm at `version`: [`Published`](Self::Published) from [`PUBLISHED_CENSUS_VERSION`].
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        if version.get() >= PUBLISHED_CENSUS_VERSION.get() {
            Self::Published
        } else {
            Self::RawCensus
        }
    }

    /// The share rows: [`CLASS_SHARES`] or [`CLASS_SHARES_29`].
    #[must_use]
    pub(crate) const fn class_shares(self) -> &'static ClassShareTable {
        match self {
            Self::RawCensus => &CLASS_SHARES,
            Self::Published => &CLASS_SHARES_29,
        }
    }

    /// The accreting white dwarfs' sampler of `side`.
    #[must_use]
    pub(crate) const fn awd_sampler(
        self,
        side: AwdSide,
    ) -> &'static ClassSamplerTable<AwdMark, AwdPick> {
        match (self, side) {
            (Self::RawCensus, AwdSide::Fast) => &AWD_FAST_SAMPLER,
            (Self::RawCensus, AwdSide::Slow) => &AWD_SLOW_SAMPLER,
            (Self::Published, AwdSide::Fast) => &AWD_FAST_SAMPLER_29,
            (Self::Published, AwdSide::Slow) => &AWD_SLOW_SAMPLER_29,
        }
    }

    /// The X-ray binaries' sampler.
    #[must_use]
    pub(crate) const fn xrb_sampler(self) -> &'static ClassSamplerTable<XrbMark, XrbPick> {
        match self {
            Self::RawCensus => &XRB_SAMPLER,
            Self::Published => &XRB_SAMPLER_29,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::galaxy::Galaxy;
    use crate::galaxy::catalogue_classes::binary::{AwdProcess, XrayBinaries};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::rules::GalaxyRules;
    use crate::rng::Seed;

    /// The hold is deleted by the bump to 29, with everything its module documentation lists.
    #[test]
    fn the_binary_tables_are_held_only_until_the_29_bump() {
        assert!(
            GENERATOR_VERSION.get() < PUBLISHED_CENSUS_VERSION.get(),
            "the bump to 29 deletes the binary tables' hold (module documentation)"
        );
        assert_eq!(
            BinaryTableRules::of_version(GENERATOR_VERSION),
            BinaryTableRules::RawCensus
        );
        assert_eq!(
            BinaryTableRules::of_version(PUBLISHED_CENSUS_VERSION),
            BinaryTableRules::Published
        );
        assert_eq!(
            GalaxyRules::every_29_hold().binary_tables,
            BinaryTableRules::Published
        );
    }

    /// A galaxy's classes read its rules' tables: the version's galaxy 27's, and one under every
    /// 29 hold the refit's, which differ from them in every sampler and in the share rows.
    #[test]
    fn a_galaxy_reads_its_rules_binary_tables() {
        let seed = Seed::new(0x0b17_29a0_0000_0001);
        for (rules, tables) in [
            (
                GalaxyRules::of_version(GENERATOR_VERSION),
                BinaryTableRules::RawCensus,
            ),
            (GalaxyRules::every_29_hold(), BinaryTableRules::Published),
        ] {
            let galaxy =
                Galaxy::from_params_with_rules(seed, GalaxyParams::milky_way_like(), rules)
                    .expect("the Milky Way fixture builds");
            assert_eq!(galaxy.rules().binary_tables, tables);
            assert_eq!(
                XrayBinaries::new(&galaxy).sampler().table(),
                tables.xrb_sampler()
            );
            for side in [AwdSide::Fast, AwdSide::Slow] {
                assert_eq!(
                    AwdProcess::new(&galaxy, side).sampler().table(),
                    tables.awd_sampler(side)
                );
            }
        }
        let [raw, published] = [BinaryTableRules::RawCensus, BinaryTableRules::Published];
        assert_ne!(raw.xrb_sampler(), published.xrb_sampler());
        assert_ne!(raw.class_shares(), published.class_shares());
        for side in [AwdSide::Fast, AwdSide::Slow] {
            assert_ne!(raw.awd_sampler(side), published.awd_sampler(side));
        }
    }
}
