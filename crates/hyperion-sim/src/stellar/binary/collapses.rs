//! The hold on the core collapses that change no orbit (plan 11, P11.T10's fix, at
//! [`COLLAPSES_VERSION`]).
//!
//! As built, a pair the pre-test passes over ([`can_interact`](super::can_interact)) is one
//! detached segment of its stars' own models on the drawn orbit, with no supernova record, even
//! where a star of it collapses by the age it is run to, though `run_pairs` (`stellar/system.rs`)
//! runs it because it holds a remnant. So a wide pair keeps a neutron star its 265 km/s kick
//! would have freed bound on its drawn orbit for ever: of 10⁴ massive systems of the binaries'
//! census sampler (P11.T10's), 575 primary pairs with a companion of 2.5 M☉ or more, all at
//! log P > 3.5 (days).
//!
//! From 27 ([`CollapseRules::Applied`]) a pair a star of which dies suddenly by the age it is run
//! to (a core's collapse, an electron capture or a pair instability: the pinned primary's at plan
//! 06's age, any other star's on its own track) is run through the engine, not passed over, whose
//! `Engine::explode` applies Hurley, Tout and Pols's (2002, "BSE") appendix A1 to it and unbinds
//! it where the orbit comes out so ([`CollapseRules::runs_pair`]). A white dwarf's birth runs no
//! pair: in an old galaxy it would run most of the low-mass pairs, and it never brings the
//! periastron in (`light.rs`, item 3).
//!
//! **A lean** (2026-11-01, `fix-p11-t10-released`): a pair the engine now runs for its collapse is
//! stepped from the first star's arrival, as any other pair it runs, so its orbit before the
//! collapse is the engine's, widened by the stars' winds, where a run to an age before the
//! collapse is passed over on the drawn orbit: the pre-collapse orbit depends on the age the pair
//! is run to. `SystemStars` runs every pair to the system's age at +H, so a system shows one
//! history. P11.T4.l's closed form, the drawn orbit up to the collapse, is not on this tree; the
//! ruling (p11-supernova-pins §4.3) found the two routes' statistics the same.

use crate::GeneratorVersion;
use crate::stellar::rules::StellarRules;

/// The generator version from which every core collapse acts on the orbit it unbinds (P11.T10's
/// fix): `evolve`'s pre-test runs a pair a star of which collapses by its age
/// ([`CollapseRules::Applied`]).
///
/// Below it every pair takes the held arm, [`CollapseRules::Gated`], the code of version 24
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_27_hold()`. The bump to 27 deletes this hold: this constant with
/// [`CollapseRules`], the stellar rules' `collapses` field, `run_pair`'s read of it, the brief's
/// read of [`CollapseRules::fitted_tables_hold`], `stellar/rules/collapses_hold.rs` with its
/// tripwire (`the_collapses_are_held_only_until_the_27_bump`), and re-pins the movers plan 11's
/// records list; the bump refits the pair-light and reach tables, which the engine's runs fit.
pub(crate) const COLLAPSES_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether a core collapse acts on the orbits the binary engine does not otherwise run (the
/// stellar rules' `collapses` field, the hold of [`COLLAPSES_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CollapseRules {
    /// Version 26's and before: a pair the pre-test passes over is its stars' own models on the
    /// drawn orbit, whatever dies in it.
    Gated,
    /// Version 27's: a pair a star of which collapses by its age is run through the engine.
    Applied,
}

impl CollapseRules {
    /// Whether the engine runs a pair the pre-test passes over, given the earliest sudden death of
    /// its stars, years (`None` for none), and the age it is run to, `until_years`: under
    /// [`CollapseRules::Applied`], where that death falls by then.
    #[must_use]
    pub(super) fn runs_pair(self, first_collapse_years: Option<f64>, until_years: f64) -> bool {
        match self {
            Self::Gated => false,
            Self::Applied => first_collapse_years.is_some_and(|at| at <= until_years),
        }
    }

    /// Whether the tables fitted offline from the binary engine's runs, which `hyperion-fit` builds
    /// under the generator version's rules (the pair-light and reach tables the range brief's gate
    /// reads), hold under this arm: under the version's arm only, until the bump refits them.
    #[must_use]
    pub(crate) fn fitted_tables_hold(self) -> bool {
        self == StellarRules::of_version(crate::GENERATOR_VERSION).collapses
    }
}
