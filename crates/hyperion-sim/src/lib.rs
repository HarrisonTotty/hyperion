//! Deterministic simulation and procedural generation core for HYPERION.
//!
//! This crate performs no I/O, reads no clocks, spawns no threads and holds no caches: given the
//! same seed and the same inputs it must always produce the same universe, on every platform.
//! Transport, sessions, caches and persistence live in `hyperion-server`.
//!
//! The determinism foundation, which everything else is built on:
//!
//! - [`math`]: every transcendental function, on the exactly pinned `libm`.
//! - [`version`]: [`GENERATOR_VERSION`], half of what identifies a universe.
//! - [`units`]: unit newtypes over `f64` and the physical constants between them.
//! - [`time`]: the universe clock, the clock window H and the light-crossing bound L.
//! - [`coords`]: the galactic, system and body frames, generation cells, the galactic axes and
//!   the named directions.
//! - [`id`]: the 64-bit system ID with every layout, body IDs, event words, their text forms and
//!   designations.
//! - [`rng`]: random streams keyed by seed, domain tag and object, the domain-tag registry, and
//!   the samplers.
//!
//! On top of it, the galaxy model (plan 02):
//!
//! - [`galaxy`]: the galaxy's parameters drawn from the seed, the mass function, age
//!   distributions and the mean mass of a system, the mass model and its potential tables, with
//!   the numerical helpers they share.
//! - [`tables`]: constant tables, exact or fitted offline.
//!
//! The stars (plan 06):
//!
//! - [`stellar`]: each star's state at any age from its mass, composition and draws, its remnant
//!   and kick, its classes, and its events in time.
//!
//! Events in time (plan 06):
//!
//! - [`events`]: the two constructions of a system's or body's own events, Poisson bins and the
//!   monotone phase, the event tags of the event kinds, and time windows.
//!
//! The crate's only runtime dependency is `libm`.

pub mod coords;
pub mod events;
pub mod galaxy;
pub mod id;
pub mod math;
pub mod rng;
pub mod stellar;
pub mod tables;
pub mod time;
pub mod units;
pub mod version;

pub use rng::Seed;
pub use version::{GENERATOR_VERSION, GeneratorVersion};

use std::time::Duration;

/// The mutable, stepped state of one session inside a universe: the ship, the clock, and later the
/// deltas play makes.
///
/// A universe is the immutable `(seed, generator_version)` and is shared by every session in it;
/// a `Simulation` is what one session changes. The first milestone has no session, so the server
/// constructs none yet. The session clock runs on the universe clock: [`Simulation::now`] is the
/// epoch plus the elapsed time, so that a session already knows "now" in the terms every query
/// takes (plan 04, design note 20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simulation {
    seed: u64,
    tick: u64,
    elapsed: Duration,
}

impl Simulation {
    /// Creates a session in the universe whose seed is `seed`, at the epoch.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            tick: 0,
            elapsed: Duration::ZERO,
        }
    }

    /// Advances the simulation by one step of length `dt`.
    pub fn step(&mut self, dt: Duration) {
        self.tick += 1;
        self.elapsed += dt;
    }

    /// The universe's seed: the seed its galaxy is generated from.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Number of steps taken so far.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Total simulated time.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// The session's present on the universe clock: the epoch plus [`Simulation::elapsed`].
    ///
    /// # Panics
    ///
    /// If more than 2⁶³ − 1 seconds (2.9 × 10¹¹ years) have elapsed, which the universe clock
    /// cannot represent. No session comes near it.
    #[must_use]
    pub fn now(&self) -> time::UniverseTime {
        let seconds = i64::try_from(self.elapsed.as_secs())
            .expect("a session's elapsed time is below 2⁶³ seconds");
        let elapsed = time::Span::new(seconds, self.elapsed.subsec_nanos())
            .expect("a Duration's subsecond nanoseconds are below 10⁹");
        time::UniverseTime::EPOCH
            .checked_add(elapsed)
            .expect("the epoch plus at most 2⁶³ − 1 seconds is a universe time")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_advances_tick_and_elapsed_time() {
        let mut sim = Simulation::new(42);
        for _ in 0..3 {
            sim.step(Duration::from_millis(50));
        }
        assert_eq!(sim.tick(), 3);
        assert_eq!(sim.elapsed(), Duration::from_millis(150));
    }

    #[test]
    fn now_is_the_epoch_plus_the_elapsed_time() {
        let mut sim = Simulation::new(42);
        assert_eq!(sim.now(), time::UniverseTime::EPOCH);
        for _ in 0..3 {
            sim.step(Duration::from_millis(50));
        }
        assert_eq!(sim.now(), time::UniverseTime::new(0, 150_000_000).unwrap());
        sim.step(Duration::from_secs(2));
        assert_eq!(sim.now(), time::UniverseTime::new(2, 150_000_000).unwrap());
    }

    #[test]
    fn identical_inputs_produce_identical_state() {
        let run = || {
            let mut sim = Simulation::new(7);
            sim.step(Duration::from_millis(16));
            sim
        };
        assert_eq!(run(), run());
    }
}
