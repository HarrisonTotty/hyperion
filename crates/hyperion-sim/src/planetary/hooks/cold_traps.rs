//! Cold traps on airless bodies (plan 14, P14.T51.f; science-r09-drainage.md §4): the share of a
//! body's surface whose annual maximum temperature stays below a condensable's threshold, and the
//! share of that whose ice is exposed.
//!
//! Built so far: the exposed fraction ([`exposed_fraction`]), which nothing on the record reads
//! yet, so no value moves.
//!
//! # The exposed fraction
//!
//! The visible ice is not the cold-trap area (science-r09-drainage.md §4.3 item 5). Of the three
//! airless bodies observed, the exposed share of the cold-trapping or permanently shadowed area
//! spans more than an order of magnitude, and no model of supply and gardening explains the
//! spread:
//!
//! - Ceres: bright deposits in 10 of at least 634 permanently shadowed craters, 0.016 (Platz et al.
//!   2016, Nat. Astron. 1, 0007, abstract);
//! - the Moon: "only ∼3.5% of cold traps exhibit ice exposures" (Li et al. 2018, PNAS 115, 8907,
//!   read in full);
//! - Mercury: 43–46% of the permanently shadowed area hosts radar-bright deposits (Deutsch et al.
//!   2016, Icarus 280, 158, abstract).
//!
//! All three via science-r09-drainage.md §4.1, which read them. So `f_x` is drawn once a body,
//! log-uniform in [[`EXPOSED_FRACTION_FLOOR`], [`EXPOSED_FRACTION_CEILING`]] on its own
//! [`tags::BODY_COLD_TRAPS`] stream. It is empirical until a delivery and gardening budget exists
//! (with P14.T55.c).
//!
//! # Determinism
//!
//! One new tag, [`tags::BODY_COLD_TRAPS`], keyed by the body; word 0 read, words 1–7 reserved
//! ([`EXPOSED_FRACTION_WORDS`]). No existing stream gains a draw.

#[cfg(test)]
mod tests;

use crate::Seed;
use crate::id::BodyId;
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};

/// The least exposed fraction `f_x`: 0.016, Ceres's bright deposits in 10 of at least 634
/// permanently shadowed craters (10 ÷ 634 = 0.0158; Platz et al. 2016, Nat. Astron. 1, 0007,
/// abstract, via science-r09-drainage.md §4.1). Empirical until a delivery and gardening budget
/// exists (with P14.T55.c).
pub const EXPOSED_FRACTION_FLOOR: f64 = 0.016;

/// The greatest exposed fraction `f_x`: 0.45, within Mercury's 43–46% of its permanently shadowed
/// area that hosts radar-bright deposits (Deutsch et al. 2016, Icarus 280, 158, abstract, via
/// science-r09-drainage.md §4.1). Empirical until a delivery and gardening budget exists (with
/// P14.T55.c).
pub const EXPOSED_FRACTION_CEILING: f64 = 0.45;

/// Words of [`tags::BODY_COLD_TRAPS`] a body reads or reserves: word 0, the exposed fraction's
/// rank, then seven reserved.
pub const EXPOSED_FRACTION_WORDS: u64 = 8;

/// The share `f_x` of the body `body`'s cold traps whose ice is exposed, in the universe of `seed`
/// (P14.T51.f): [`EXPOSED_FRACTION_FLOOR`] × ([`EXPOSED_FRACTION_CEILING`] ÷
/// [`EXPOSED_FRACTION_FLOOR`])^u, u the open uniform of word 0 of its [`tags::BODY_COLD_TRAPS`]
/// stream, so log-uniform in [0.016, 0.45] and held to it. No other word is read.
///
/// The bounds are Ceres's 0.016 and Mercury's 0.43–0.46, with the Moon's 0.035 between them (the
/// [module](self) documentation). It is empirical until a delivery and gardening budget exists
/// (with P14.T55.c). Dimensionless.
///
/// # Examples
///
/// The fraction depends on the seed and the body's ID and nothing else:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::hooks::cold_traps::exposed_fraction;
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let moon = BodyId::new(system, 0x1300);
/// let f = exposed_fraction(Seed::new(7), moon);
/// assert!((0.016..=0.45).contains(&f));
/// assert_eq!(f.to_bits(), exposed_fraction(Seed::new(7), moon).to_bits());
/// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
/// ```
#[must_use]
pub fn exposed_fraction(seed: Seed, body: BodyId) -> f64 {
    let mut stream = Stream::open(seed, tags::BODY_COLD_TRAPS, ObjectKey::from(body));
    let u = stream.uniform_open();
    let f =
        EXPOSED_FRACTION_FLOOR * math::powf(EXPOSED_FRACTION_CEILING / EXPOSED_FRACTION_FLOOR, u);
    f.clamp(EXPOSED_FRACTION_FLOOR, EXPOSED_FRACTION_CEILING)
}
