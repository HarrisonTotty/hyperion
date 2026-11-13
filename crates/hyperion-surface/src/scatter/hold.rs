//! The authoritative rocks' hold: they are placed from generator version 27 (plan R11's Generator
//! version; the owner's rule B, a hold and not a bump).
//!
//! Below [`ROCKS_VERSION`] the crate places no rock: [`rocks_in_cell`](super::rocks_in_cell)
//! adds none, [`rocks_near`](super::rocks_near) finds none and
//! [`surface_height_with_rocks`](super::surface_height_with_rocks) is the ground's height, so no
//! output moves at 24 to 26 and train 27 folds the rocks in. The tests reach the rules of 27
//! through [`with_rock_rules`]. 27's bump deletes this module: [`ROCKS_VERSION`], [`RockRules`]
//! with its `Held` arm, [`with_rock_rules`] and the tripwire
//! (`the_rocks_are_held_only_until_the_27_bump`).

use hyperion_base::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which the authoritative rocks are placed (R11.T3): 27.
pub const ROCKS_VERSION: GeneratorVersion = GeneratorVersion::new(33);

/// Whether the authoritative rocks are placed (their hold, [`ROCKS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RockRules {
    /// Version 26's and before: no rock is placed.
    Held,
    /// Version 27's: the rocks of R11.T3.
    Placed,
}

impl RockRules {
    /// The rules this generator version gives: placed from [`ROCKS_VERSION`], held below it; in
    /// the tests, the rules a test's switch sets on its thread ([`with_rock_rules`]).
    #[must_use]
    pub fn in_force() -> Self {
        #[cfg(any(test, feature = "testing"))]
        if let Some(rules) = ROCK_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= ROCKS_VERSION {
            Self::Placed
        } else {
            Self::Held
        }
    }
}

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// The rocks' rules a test sets on its thread, `None` for the version's.
    static ROCK_RULES: core::cell::Cell<Option<RockRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the rocks' rules `rules` on this thread, for the tests to reach R11.T3
/// ahead of the bump (the `testing` feature's, for other crates' tests); the version's rules are
/// set back after, a panic in `run` included. 27's bump deletes it with the hold.
#[cfg(any(test, feature = "testing"))]
pub fn with_rock_rules<R>(rules: RockRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<RockRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ROCK_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(ROCK_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// `run`'s result under the rules of 27, [`RockRules::Placed`]: the tests' reading of the rocks.
#[cfg(any(test, feature = "testing"))]
pub fn with_rocks_placed<R>(run: impl FnOnce() -> R) -> R {
    with_rock_rules(RockRules::Placed, run)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// R11.T3's hold: 27's bump deletes it, and until then the version holds the rocks and only the
    /// tests' switch places them.
    #[test]
    fn the_rocks_are_held_only_until_the_27_bump() {
        use RockRules::{Held, Placed};
        assert!(
            GENERATOR_VERSION < ROCKS_VERSION,
            "the 26 → 27 bump places the authoritative rocks (R11.T3): delete `scatter::hold` \
             (`ROCKS_VERSION`, `RockRules` with its `Held` arm, `with_rock_rules`, \
             `with_rocks_placed` and this test) and the `Held` paths of `rocks_in_cell`, \
             `rocks_near` and `surface_height_with_rocks`, unwrap the tests' switches, and bless \
             the scatter goldens at 27"
        );
        assert_eq!(RockRules::in_force(), Held);
        assert_eq!(with_rock_rules(Placed, RockRules::in_force), Placed);
        assert_eq!(with_rocks_placed(RockRules::in_force), Placed);
        assert_eq!(
            with_rocks_placed(|| with_rock_rules(Held, RockRules::in_force)),
            Held
        );
        assert_eq!(RockRules::in_force(), Held);
    }
}
