//! An oxygen–neon dwarf's coalescence past the capture mass (plan 11, P11.T4's fix with P11.T17.b;
//! research-leans-45 item 1).
//!
//! As built before the fix, an oxygen–neon dwarf of 1.345–1.363 M☉ that coalesces in a common
//! envelope with its Hertzsprung-gap or giant companion leaves a product placed around a helium
//! core of 1.50–1.55 M☉, the dwarf's and the donor's together (`Engine::coalesced_source` gives it
//! the dwarf's kind). Stripped to a helium giant, its placed track's carbon–oxygen core restarts at
//! about 1.06 M☉, below the dwarf, and the giant shines at `M_V` −6 to −7.6 for 0.1 Myr until that
//! core reaches the capture mass (`detached::Stop::CoreLimit`): layer E's 8.0–8.15 M☉ primaries
//! past their pair-light table (research-leans-45; P11.T17.b's slow tests at 29). A degenerate core
//! keeps its mass (research-leans-34 §3, research-leans-35 §7–§8): a merged core over an
//! oxygen–neon dwarf that already holds the capture mass passes it at once.
//!
//! A coalescence whose core comes from an oxygen–neon
//! dwarf and holds its limit (`supernova::core_limit`: the capture mass, 1.37 M☉, under the default
//! recipe, from electron captures on ²⁴Mg and ²⁰Ne, Miyaji et al. 1980, PASJ 32, 303, and Nomoto
//! 1984, ApJ 277, 791; the Chandrasekhar mass under `Hurley2000`) collapses at once by electron
//! capture, as an unplaced product's oxygen–neon core does (`common_envelope::Product::
//! ElectronCapture`, `Engine::capture_collapse`), with no Type Ia candidate (`Engine::product_member`);
//! a pinned primary's product is left to its pin (design note 16).

#[cfg(test)]
thread_local! {
    /// This thread's coalescences collapsed at once by electron capture since it started
    /// ([`merged_captures`]).
    static MERGED_CAPTURES: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Counts a coalescence collapsed at once on this thread ([`merged_captures`]).
#[cfg(test)]
pub(super) fn count_merged_capture() {
    MERGED_CAPTURES.with(|c| c.set(c.get() + 1));
}

/// The coalescences this thread's engines have collapsed at once by electron capture since it
/// started: a test build's counter for the dwarf-limit census.
#[cfg(test)]
#[must_use]
pub(crate) fn merged_captures() -> usize {
    MERGED_CAPTURES.with(core::cell::Cell::get)
}
