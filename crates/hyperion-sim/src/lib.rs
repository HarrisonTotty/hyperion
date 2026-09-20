//! Deterministic simulation and procedural generation core for HYPERION.
//!
//! This crate performs no I/O and reads no clocks: given the same seed and the
//! same sequence of inputs it must always produce the same universe. Transport,
//! sessions and persistence live in `hyperion-server`.

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
