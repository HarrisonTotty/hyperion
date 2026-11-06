//! The hold on an oxygen–neon dwarf's coalescence past the capture mass (plan 11, P11.T4's fix
//! with P11.T17.b, at [`MERGED_CAPTURE_VERSION`]; research-leans-45 item 1).
//!
//! As built under every 29 hold, an oxygen–neon dwarf of 1.345–1.363 M☉ that coalesces in a common
//! envelope with its Hertzsprung-gap or giant companion leaves a product placed around a helium
//! core of 1.50–1.55 M☉, the dwarf's and the donor's together (`Engine::coalesced_source` gives it
//! the dwarf's kind). Stripped to a helium giant, its placed track's carbon–oxygen core restarts at
//! about 1.06 M☉, below the dwarf, and the giant shines at `M_V` −6 to −7.6 for 0.1 Myr until that
//! core reaches the capture mass (`detached::Stop::CoreLimit`): layer E's 8.0–8.15 M☉ primaries
//! past their pair-light table (research-leans-45; P11.T17.b's slow tests at 29). A degenerate core
//! keeps its mass (research-leans-34 §3, research-leans-35 §7–§8): a merged core over an
//! oxygen–neon dwarf that already holds the capture mass passes it at once.
//!
//! From 30 ([`MergedCaptureRules::Captured`]) a coalescence whose core comes from an oxygen–neon
//! dwarf and holds its limit (`supernova::core_limit`: the capture mass, 1.37 M☉, under the default
//! recipe, from electron captures on ²⁴Mg and ²⁰Ne, Miyaji et al. 1980, PASJ 32, 303, and Nomoto
//! 1984, ApJ 277, 791; the Chandrasekhar mass under `Hurley2000`) collapses at once by electron
//! capture, as an unplaced product's oxygen–neon core does (`common_envelope::Product::
//! ElectronCapture`, `Engine::capture_collapse`), with no Type Ia candidate; a pinned primary's
//! product is left to its pin (design note 16).

use crate::GeneratorVersion;
use crate::stellar::rules::StellarRules;

/// The generator version whose stellar rules collapse an oxygen–neon dwarf's coalescence that holds
/// the capture mass at once (P11.T4's fix): `Engine::product_member` takes
/// [`MergedCaptureRules::Captured`].
///
/// Below it every pair takes the held arm, [`MergedCaptureRules::Held`], the code of version 29
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_30_hold()`. The bump to 30 deletes the hold, not the rule: this module with
/// [`MergedCaptureRules`], the stellar rules' `merged_capture` field and its read in
/// `Engine::product_member`, the brief's read of [`MergedCaptureRules::fitted_tables_hold`] and the
/// tripwire (`the_merged_capture_is_held_only_until_the_30_bump`); it keeps the collapse applied
/// always, and refits the pair-light and reach tables, which the engine's runs fit.
pub(crate) const MERGED_CAPTURE_VERSION: GeneratorVersion = GeneratorVersion::new(30);

/// Whether an oxygen–neon dwarf's coalescence that holds the capture mass collapses at once (the
/// stellar rules' `merged_capture` field, the hold of [`MERGED_CAPTURE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MergedCaptureRules {
    /// Version 29's and before: the product is placed around its core, whose track's own core
    /// reaches the limit later.
    Held,
    /// Version 30's: it collapses at once by electron capture.
    Captured,
}

impl MergedCaptureRules {
    /// The arm at `version`: [`MergedCaptureRules::Captured`] from [`MERGED_CAPTURE_VERSION`],
    /// [`MergedCaptureRules::Held`] below it.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        if version.get() >= MERGED_CAPTURE_VERSION.get() {
            Self::Captured
        } else {
            Self::Held
        }
    }

    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold for a pair under this arm: under the version's arm only, until the bump refits
    /// them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).merged_capture
    }
}

#[cfg(test)]
thread_local! {
    /// This thread's coalescences collapsed at once by this hold's arm since it started
    /// ([`merged_captures`]).
    static MERGED_CAPTURES: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Counts a coalescence collapsed at once on this thread ([`merged_captures`]).
#[cfg(test)]
pub(super) fn count_merged_capture() {
    MERGED_CAPTURES.with(|c| c.set(c.get() + 1));
}

/// The coalescences this thread's engines have collapsed at once by electron capture since it
/// started ([`MergedCaptureRules::Captured`]): a test build's counter for the dwarf-limit census.
#[cfg(test)]
#[must_use]
pub(crate) fn merged_captures() -> usize {
    MERGED_CAPTURES.with(core::cell::Cell::get)
}
