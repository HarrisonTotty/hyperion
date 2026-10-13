//! The surface crate's constant tables: numbers it reads but never computes at run time, the
//! committed output of the offline fitting crate, `hyperion-fit` (plan 15; R09.T7.f).
//!
//! A table's file opens with a header in plan 15's grammar (Design note 5): `@generated` by a task
//! at a revision, with the hash of its inputs and the generator version it took effect at. Every
//! table belongs to the generator version: changing one that generated output reads moves that
//! output, and bumps [`GENERATOR_VERSION`].
//!
//! [`MANIFEST`] lists the fitted tables. `hyperion-fit` writes it from its lock file, and
//! `hyperion-fit check` (`just fit-check`) fails if the two disagree, if a table was edited by
//! hand, or if its inputs changed without a rerun.
//!
//! [`GENERATOR_VERSION`]: hyperion_base::GENERATOR_VERSION

// The table is written whole by `hyperion-fit`, so its one lint allowance is on its module.
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod bowl_permanent_shadow;

use hyperion_base::version::TableInfo;

/// The fitted tables, in name order.
// `hyperion-fit` writes one row a `TableInfo` block, as in the sim's, which rustfmt would fold
// while the list holds one.
#[rustfmt::skip]
// @begin-manifest: written by hyperion-fit from tables.lock. Do not edit.
pub const MANIFEST: &[TableInfo] = &[
    TableInfo {
        name: "bowl_permanent_shadow",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
];
// @end-manifest

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_base::GENERATOR_VERSION;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

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
