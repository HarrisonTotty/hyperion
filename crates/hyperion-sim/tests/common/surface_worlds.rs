//! The sample worlds of R10's world tests (rendering plan R10, `sample_worlds()`; plan R09, T16):
//! R09's reference Earth, Mars, Moon and Ceres, with an icy moon and a locked world, each run
//! through the coarse pass under `reference::SEED` once per test binary.
//!
//! The worlds are `planetary::surface::reference`'s (`earth_like`, `mars_like`, `moon_like`,
//! `ceres_like`, `europa_like` and `locked_earth_like`), behind the sim's `testing` feature. The
//! sim's library goldens (`planetary::surface::coarse_golden`, the files
//! `tests/golden/planetary/surface/<world>.golden`) pin each one's field; the integration test
//! `surface_coarse_golden.rs` checks that the field a world here gives is the one its golden pins,
//! by the golden's payload digest.
//!
//! Under nextest each test runs in a process of its own (`.config/nextest.toml`), so "once per
//! binary" holds within one test: the checks that share a world's pass are one test
//! (decision-r09-t13-test-cost §1.2). A test of a [`Tier::Slow`] world is `#[ignore = "slow: …"]`.

use std::sync::OnceLock;

use hyperion_sim::planetary::surface::{CoarseInputs, coarse_pass, reference};
use hyperion_surface::field::CoarseField;

/// How long a sample world's coarse pass takes, which tier of tests may run it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// A pass of seconds at most: the fast tests may run it.
    Fast,
    /// A pass at level 8 with a water cycle, tens of seconds or more: only slow tests run it.
    Slow,
}

/// The number of sample worlds.
const COUNT: usize = 6;

/// A sample world: its name, its inputs' constructor and its tier, built only here, so that a
/// world's name and inputs always agree.
#[derive(Debug, Clone, Copy)]
pub struct SampleWorld {
    /// Its place in [`sample_worlds`], which keys its field.
    index: usize,
    /// Its name, the name of its `reference` function and of its golden.
    name: &'static str,
    /// Its inputs to the coarse pass.
    inputs: fn() -> CoarseInputs,
    /// Which tier of tests may run its pass.
    tier: Tier,
}

/// The sample worlds, in R09's order: Earth-like, Mars-like, Moon-like, Ceres-like, Europa-like
/// and the locked Earth-like.
static WORLDS: [SampleWorld; COUNT] = [
    SampleWorld {
        index: 0,
        name: "earth_like",
        inputs: reference::earth_like,
        tier: Tier::Slow,
    },
    SampleWorld {
        index: 1,
        name: "mars_like",
        inputs: reference::mars_like,
        tier: Tier::Fast,
    },
    SampleWorld {
        index: 2,
        name: "moon_like",
        inputs: reference::moon_like,
        tier: Tier::Fast,
    },
    SampleWorld {
        index: 3,
        name: "ceres_like",
        inputs: reference::ceres_like,
        tier: Tier::Fast,
    },
    SampleWorld {
        index: 4,
        name: "europa_like",
        inputs: reference::europa_like,
        tier: Tier::Fast,
    },
    SampleWorld {
        index: 5,
        name: "locked_earth_like",
        inputs: reference::locked_earth_like,
        tier: Tier::Slow,
    },
];

/// The sample worlds, in R09's order.
#[must_use]
pub fn sample_worlds() -> &'static [SampleWorld] {
    &WORLDS
}

impl SampleWorld {
    /// Its name, the name of its `reference` function and of its golden.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Its inputs to the coarse pass, built anew.
    #[must_use]
    pub fn inputs(&self) -> CoarseInputs {
        (self.inputs)()
    }

    /// Which tier of tests may run its pass.
    #[must_use]
    pub fn tier(&self) -> Tier {
        self.tier
    }

    /// The world's coarse field under `reference::SEED`, computed on first call and kept for the
    /// rest of the binary's run (one pass a world a process).
    #[must_use]
    pub fn field(&self) -> &'static CoarseField {
        static FIELDS: [OnceLock<CoarseField>; COUNT] = [const { OnceLock::new() }; COUNT];
        FIELDS[self.index].get_or_init(|| coarse_pass(reference::SEED, &self.inputs()))
    }
}
