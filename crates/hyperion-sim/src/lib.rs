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
//! The crate's only runtime dependency is `libm`.

pub mod coords;
pub mod galaxy;
pub mod id;
pub mod math;
pub mod rng;
pub mod tables;
pub mod time;
pub mod units;
pub mod version;

pub use rng::Seed;
pub use version::{GENERATOR_VERSION, GeneratorVersion};

use std::time::Duration;

/// Top-level simulation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simulation {
    seed: u64,
    tick: u64,
    elapsed: Duration,
}

impl Simulation {
    /// Creates a simulation of the galaxy generated from `seed`.
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

    /// The seed the galaxy is generated from.
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
    fn identical_inputs_produce_identical_state() {
        let run = || {
            let mut sim = Simulation::new(7);
            sim.step(Duration::from_millis(16));
            sim
        };
        assert_eq!(run(), run());
    }
}
