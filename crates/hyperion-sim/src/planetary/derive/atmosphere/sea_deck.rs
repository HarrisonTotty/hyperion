//! No acid deck over a sea (research-census-25 §4): a runaway's Venus deck needs a dry surface,
//! behind its hold at generator version 26 ([`SEA_DECK_VERSION`]).
//!
//! An observable H₂SO₄ haze is not sustained over more than 10⁻³ Earth oceans: the SO₂ dissolves
//! and rains out (Loftus, Wordsworth and Morley 2019, ApJ 887, 231, for an ocean pH of about 6 or
//! more). The rule here is stricter than its source, any listed surface liquid water dropping the
//! deck, since the census's seas hold 0.23 to 4,000 Earth oceans. To
//! version 25 P14.T53.b part 1 (c) re-solves under the deck a first solve whose state is
//! `Temperate`, even where part 3 labels it a steam envelope, and the deck's albedo then cools a
//! kept ocean's water below its critical temperature into a global sea under the acid deck: 29
//! runaways with a sea in P14.T33.c's census of 10⁶ bodies at 25. From 26:
//!
//! - (c) reads the published label
//!   ([`Atmosphere::published_state`](super::Atmosphere::published_state)), so a first solve
//!   labelled `SteamEnvelope` is published as it stands;
//! - (b)'s deck test gains Loftus's condition: a decked runaway solve that lists surface liquid
//!   water is solved once more without the deck, (b)'s deckless solve;
//! - and a runaway that still lists liquid water is labelled by rules 6–7 on its own figures
//!   (`present::published_label`), every figure kept.
//!
//! That is at most one solve more, each reading only the solve before it.

use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which a runaway's Venus deck needs a dry surface: 26, the planetary
/// bump after 25's (research-census-25 §4, ruled 2026-10-31).
///
/// It is this rule's one hold. Below it [`SeaDeckRules::in_force`] is [`SeaDeckRules::Held`]:
/// (c) reads the state and the deck is (b)'s cycling test's alone, and every figure is version
/// 24's and 25's bit for bit. 26's bump deletes this hold, [`SeaDeckRules`] with its `Held` arm,
/// the tests' switch and its tripwire (`the_sea_deck_is_held_only_until_the_26_bump`).
pub(crate) const SEA_DECK_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// Whether a runaway's deck needs a dry surface (its hold, [`SEA_DECK_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SeaDeckRules {
    /// Version 25's and before: (c) reads the state, and the deck stays above the acid's cycling
    /// temperature whatever lies under it.
    Held,
    /// Version 26's: (c) reads the published label, and a decked runaway over liquid water is
    /// solved without the deck.
    Dry,
}

impl SeaDeckRules {
    /// The rules this generator version gives: the dry deck's from [`SEA_DECK_VERSION`], none
    /// below it; in the tests, the rules a test's switch sets on its thread ([`with_sea_deck`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= SEA_DECK_VERSION {
            Self::Dry
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The sea deck's rules a test sets on its thread, `None` for the version's.
    static RULES: core::cell::Cell<Option<SeaDeckRules>> = const { core::cell::Cell::new(None) };
}

/// `run`'s result with the sea deck's rules `rules` on this thread, for the tests to reach version
/// 26's ahead of the bump; the version's rules are set back after, a panic in `run` included.
/// 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_sea_deck<R>(rules: SeaDeckRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<SeaDeckRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// Whether a runaway's deck needs a dry surface ([`SeaDeckRules::Dry`]); never under its hold.
#[must_use]
pub(crate) fn sea_deck_in_force() -> bool {
    SeaDeckRules::in_force() == SeaDeckRules::Dry
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sea deck is held until the 25 → 26 bump: the rules in force are version 25's.
    #[test]
    fn the_sea_deck_is_held_only_until_the_26_bump() {
        assert!(
            GENERATOR_VERSION < SEA_DECK_VERSION,
            "the 25 → 26 bump puts the dry deck on the record (research-census-25 §4): delete \
             `SEA_DECK_VERSION`, `SeaDeckRules` with its `Held` arm, `sea_deck_in_force` and \
             its held arms in `solved_without_the_deck`, `solved_as_a_hothouse` and \
             `published_label`, the tests' `with_sea_deck`, its switch in `with_every_26_hold` \
             and this test"
        );
        assert_eq!(SeaDeckRules::in_force(), SeaDeckRules::Held);
        assert!(!sea_deck_in_force());
        assert!(with_sea_deck(SeaDeckRules::Dry, sea_deck_in_force));
        assert!(crate::planetary::testing::with_every_26_hold(
            sea_deck_in_force
        ));
        assert!(!sea_deck_in_force());
    }
}
