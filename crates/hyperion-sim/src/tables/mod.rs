//! Constant tables: numbers the generator reads but never computes at run time.
//!
//! A table is either exact mathematics written out to the last bit ([`gauss_legendre`]) or the
//! committed output of the offline fitting crate, `hyperion-fit` (plan 15). A fitted table's file
//! opens with a header in plan 15's grammar (Design note 5): `@generated` by a task at a revision,
//! or `@provisional` while a scratch stand-in is in place, with the hash of its inputs and the
//! generator version it took effect at. Every table belongs to the generator version: changing one
//! that generated output reads moves that output, and bumps [`GENERATOR_VERSION`].
//!
//! [`MANIFEST`] lists the fitted tables. `hyperion-fit` writes it from its lock file, and
//! `hyperion-fit check` (`just fit-check`) fails if the two disagree, if a table was edited by
//! hand, or if its inputs or the generator code it depends on changed without a rerun.
//!
//! [`GENERATOR_VERSION`]: crate::GENERATOR_VERSION

pub mod chabrier;
pub mod gauss_legendre;
pub mod giant_cooling;
pub mod kick_rank;
pub mod mge;
pub mod wd_cooling;

/// What the fitting toolchain records of one fitted table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableInfo {
    /// The table's name: the task that makes it, and in general its module's.
    pub name: &'static str,
    /// The table's own revision, from its header.
    pub revision: u32,
    /// The [`GENERATOR_VERSION`](crate::GENERATOR_VERSION) at which this revision took effect.
    pub since_generator_version: u32,
    /// Whether a scratch stand-in is in place.
    pub provisional: bool,
}

/// The fitted tables, in name order; hand-entered constants such as [`gauss_legendre`] are not
/// listed.
// @begin-manifest: written by hyperion-fit from tables.lock. Do not edit.
pub const MANIFEST: &[TableInfo] = &[
    TableInfo {
        name: "chabrier",
        revision: 0,
        since_generator_version: 11,
        provisional: true,
    },
    TableInfo {
        name: "giant_cooling",
        revision: 0,
        since_generator_version: 11,
        provisional: false,
    },
    TableInfo {
        name: "kick_rank",
        revision: 0,
        since_generator_version: 11,
        provisional: true,
    },
    TableInfo {
        name: "mge",
        revision: 1,
        since_generator_version: 11,
        provisional: true,
    },
    TableInfo {
        name: "wd_cooling",
        revision: 0,
        since_generator_version: 11,
        provisional: false,
    },
];
// @end-manifest

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GENERATOR_VERSION;

    #[test]
    fn the_manifest_is_in_name_order_and_no_table_is_ahead_of_the_generator() {
        assert!(MANIFEST.windows(2).all(|w| w[0].name < w[1].name));
        assert!(
            MANIFEST
                .iter()
                .all(|t| t.since_generator_version <= GENERATOR_VERSION.get()),
            "{MANIFEST:?}"
        );
    }
}
