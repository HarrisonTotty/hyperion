//! The natal kick a remnant is born with (plan 06, P06.T19).
//!
//! This holds the kick's value type, which the stellar stage's models already return
//! (`StarModel::natal_kick`, P06.T29.a) so that plans 08, 11 and 14 can be written against it. The
//! law that draws it, `KickLaw` with its parameters, draws and rank table, is P06.T19's, and until
//! it lands no remnant has a kick: the models return `None` (ruling 33 of 2026-09-22, and the
//! `SYSTEM` slice's relaxation of T29.a).

use crate::coords::UnitVector;
use crate::units::MetresPerSecond;

/// Which mode of the kick law a remnant's kick came from (plan 06, P06.T19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KickMode {
    /// The ordinary mode: the measured log-normal, ranked by Mandel and Müller's score.
    Ordinary,
    /// The low mode, a Maxwellian of σ = 5 km/s: electron capture, accretion-induced collapse and
    /// companion-stripped progenitors.
    Low,
    /// A black hole of complete fallback, which has no kick.
    FallbackNone,
    /// A white dwarf's Maxwellian of σ = 1 km/s.
    WhiteDwarf,
}

/// A remnant's natal kick: its speed, its direction along the galactic axes, and the mode it came
/// from (plan 06's Provides).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NatalKick {
    speed: MetresPerSecond,
    direction: UnitVector,
    mode: KickMode,
}

impl NatalKick {
    /// A kick of `speed` (m/s, finite and non-negative) along `direction` from `mode`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `speed` is negative or not finite.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "P06.T19's kick law is the first caller")
    )]
    #[must_use]
    pub(crate) fn new(speed: MetresPerSecond, direction: UnitVector, mode: KickMode) -> Self {
        debug_assert!(
            speed.value().is_finite() && speed.value() >= 0.0,
            "a kick's speed is finite and non-negative: {speed:?}"
        );
        Self {
            speed,
            direction,
            mode,
        }
    }

    /// The kick's speed, m/s, in the frame of the progenitor at its death.
    #[must_use]
    pub const fn speed(&self) -> MetresPerSecond {
        self.speed
    }

    /// The kick's direction along the galactic axes.
    #[must_use]
    pub const fn direction(&self) -> UnitVector {
        self.direction
    }

    /// The mode of the law the kick came from.
    #[must_use]
    pub const fn mode(&self) -> KickMode {
        self.mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kick_keeps_its_parts() {
        let kick = NatalKick::new(
            MetresPerSecond::new(2.7e5),
            UnitVector::NORTH,
            KickMode::Ordinary,
        );
        assert!((kick.speed().value() - 2.7e5).abs() < 1e-9);
        assert_eq!(kick.direction(), UnitVector::NORTH);
        assert_eq!(kick.mode(), KickMode::Ordinary);
    }
}
