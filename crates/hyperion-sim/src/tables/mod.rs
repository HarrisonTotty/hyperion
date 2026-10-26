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

pub mod binary;
pub mod binary_cooling_floor;
// The three `binary_pair_light` tables are written whole by `hyperion-fit`, whose dimensions'
// counts reach six digits, so their one lint allowance is on their modules.
#[allow(
    clippy::unreadable_literal,
    reason = "a fitted table's counts are data, written whole"
)]
pub mod binary_pair_light_c;
#[allow(
    clippy::unreadable_literal,
    reason = "a fitted table's counts are data, written whole"
)]
pub mod binary_pair_light_d;
#[allow(
    clippy::unreadable_literal,
    reason = "a fitted table's counts are data, written whole"
)]
pub mod binary_pair_light_e;
pub mod binary_reach;
pub mod chabrier;
pub mod cluster_dynamics;
pub mod cluster_retention;
// The seven `condensate_extinction` tables are written whole by `hyperion-fit`, so their one lint
// allowance is on their modules.
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod condensate_extinction;
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod condensate_extinction_2;
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod condensate_extinction_3;
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod condensate_extinction_4;
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod condensate_extinction_5;
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod condensate_extinction_6;
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod condensate_extinction_7;
pub mod displaced_forms;
pub mod gauss_legendre;
pub mod giant_cooling;
pub mod helium;
pub mod kick_defaults;
pub mod kick_rank;
pub mod lifetime_bracket;
pub mod limb_darkening;
pub mod liquid_optics;
pub mod mge;
pub mod period_correction;
pub mod saturation;
// The three `sky_binary_light` tables are written whole by `hyperion-fit`, so their one lint
// allowance is on their modules.
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod sky_binary_light_c;
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod sky_binary_light_d;
#[allow(
    clippy::approx_constant,
    reason = "fitted values that may land near a constant are data"
)]
pub mod sky_binary_light_e;
pub mod sky_envelope;
pub mod sky_phase_envelope;
pub mod star_colour;
pub mod star_colour_reddening;
pub mod star_colour_reddening_av02_05;
pub mod star_colour_reddening_av07p5;
pub mod star_colour_reddening_av10_15;
pub mod star_colour_reddening_av20_30;
pub mod stellar_fates_high;
pub mod stellar_fates_low;
pub mod stellar_fates_mid;
pub mod streams;
pub mod stripping;
pub mod substance_albedo;
pub mod thermo;
pub mod type_ia_delay;
pub mod wd_cooling;

/// What the fitting toolchain records of one fitted table, shared with the surface crate's
/// `tables::MANIFEST` (R09.T7.f).
pub use hyperion_base::version::TableInfo;

/// The fitted tables, in name order; hand-entered constants such as [`gauss_legendre`] are not
/// listed.
// @begin-manifest: written by hyperion-fit from tables.lock. Do not edit.
pub const MANIFEST: &[TableInfo] = &[
    TableInfo {
        name: "binary_cooling_floor",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "binary_pair_light_c",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "binary_pair_light_d",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "binary_pair_light_e",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "binary_reach",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "chabrier",
        revision: 1,
        since_generator_version: 14,
        provisional: false,
    },
    TableInfo {
        name: "cluster_bh",
        revision: 1,
        since_generator_version: 15,
        provisional: false,
    },
    TableInfo {
        name: "cluster_retention",
        revision: 0,
        since_generator_version: 16,
        provisional: false,
    },
    TableInfo {
        name: "condensate_extinction",
        revision: 0,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "condensate_extinction_2",
        revision: 0,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "condensate_extinction_3",
        revision: 0,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "condensate_extinction_4",
        revision: 0,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "condensate_extinction_5",
        revision: 0,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "condensate_extinction_6",
        revision: 0,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "condensate_extinction_7",
        revision: 0,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "displaced_forms",
        revision: 0,
        since_generator_version: 22,
        provisional: true,
    },
    TableInfo {
        name: "equipartition",
        revision: 1,
        since_generator_version: 15,
        provisional: false,
    },
    TableInfo {
        name: "giant_cooling",
        revision: 0,
        since_generator_version: 11,
        provisional: false,
    },
    TableInfo {
        name: "helium",
        revision: 0,
        since_generator_version: 23,
        provisional: true,
    },
    TableInfo {
        name: "kick_defaults",
        revision: 0,
        since_generator_version: 23,
        provisional: false,
    },
    TableInfo {
        name: "kick_rank",
        revision: 1,
        since_generator_version: 16,
        provisional: true,
    },
    TableInfo {
        name: "lifetime_bracket",
        revision: 0,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "limb_darkening",
        revision: 0,
        since_generator_version: 19,
        provisional: false,
    },
    TableInfo {
        name: "liquid_optics",
        revision: 2,
        since_generator_version: 22,
        provisional: false,
    },
    TableInfo {
        name: "mge",
        revision: 1,
        since_generator_version: 11,
        provisional: true,
    },
    TableInfo {
        name: "period_correction",
        revision: 0,
        since_generator_version: 14,
        provisional: false,
    },
    TableInfo {
        name: "pulsars",
        revision: 1,
        since_generator_version: 15,
        provisional: false,
    },
    TableInfo {
        name: "saturation_curves",
        revision: 0,
        since_generator_version: 23,
        provisional: false,
    },
    TableInfo {
        name: "sky_binary_light_c",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "sky_binary_light_d",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "sky_binary_light_e",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "sky_envelope",
        revision: 0,
        since_generator_version: 19,
        provisional: false,
    },
    TableInfo {
        name: "sky_phase_envelope",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "star_colour",
        revision: 0,
        since_generator_version: 19,
        provisional: false,
    },
    TableInfo {
        name: "star_colour_reddening",
        revision: 1,
        since_generator_version: 20,
        provisional: false,
    },
    TableInfo {
        name: "star_colour_reddening_av02_05",
        revision: 1,
        since_generator_version: 20,
        provisional: false,
    },
    TableInfo {
        name: "star_colour_reddening_av07p5",
        revision: 1,
        since_generator_version: 20,
        provisional: false,
    },
    TableInfo {
        name: "star_colour_reddening_av10_15",
        revision: 1,
        since_generator_version: 20,
        provisional: false,
    },
    TableInfo {
        name: "star_colour_reddening_av20_30",
        revision: 1,
        since_generator_version: 20,
        provisional: false,
    },
    TableInfo {
        name: "stellar_fates_high",
        revision: 0,
        since_generator_version: 16,
        provisional: false,
    },
    TableInfo {
        name: "stellar_fates_low",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "stellar_fates_mid",
        revision: 0,
        since_generator_version: 21,
        provisional: false,
    },
    TableInfo {
        name: "streams",
        revision: 0,
        since_generator_version: 23,
        provisional: true,
    },
    TableInfo {
        name: "stripping",
        revision: 0,
        since_generator_version: 16,
        provisional: false,
    },
    TableInfo {
        name: "substance_albedos",
        revision: 2,
        since_generator_version: 23,
        provisional: false,
    },
    TableInfo {
        name: "thermo_equilibrium",
        revision: 0,
        since_generator_version: 23,
        provisional: false,
    },
    TableInfo {
        name: "type_ia_delay",
        revision: 0,
        since_generator_version: 15,
        provisional: false,
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
