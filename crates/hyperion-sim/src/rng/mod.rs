//! Random streams, not a random sequence.
//!
//! Each value the generator draws comes from a stream keyed by what it is for:
//! `(universe seed, domain tag; object)`. Adding a draw for a new property then moves nothing
//! already generated, which is what makes lazy, out-of-order and parallel generation safe
//! (brainstorm, "Random streams, not a random sequence"; Salmon et al. 2011).
//!
//! A [`DomainTag`] names the property group (`"star.mass"`, `"planet.orbits"`). Its name is hashed
//! to a `u64` at compile time with [`hash_tag_name`], and it carries a [`TagScope`] fixing what
//! kind of object its streams name. Every tag is an entry of the single registry, [`tags`], whose
//! `const` assertion rejects a malformed name, a duplicate or a hash collision at compile time.
//!
//! # Keying
//!
//! A [`Stream`] is opened with [`Stream::open`]`(`[`Seed`]`, `[`DomainTag`]`, `[`ObjectKey`]`)`,
//! and draws its words from the block function [`threefry2x64_20`]:
//!
//! | Word           | Contents                                                                                                                         |
//! | -------------- | -------------------------------------------------------------------------------------------------------------------------------- |
//! | Key word 0     | The universe seed                                                                                                                |
//! | Key word 1     | The domain tag's hash                                                                                                            |
//! | Counter word 0 | The object's word: a system's raw ID, a body's system's raw ID, a cell word (an ID with its index zeroed), a feature word, or 0 or an item number for the galaxy |
//! | Counter word 1 | `sub << 48 \| block`: `sub` is 0, or the body index for a body; `block` is the 48-bit draw number                                |
//!
//! Word `n` of a stream is output `n & 1` of block `n >> 1`. The generator version is not in the
//! key.
//!
//! # Samplers
//!
//! The samplers are hand-written, because a dependency's distributions may change their output in
//! a minor release and a dependency bump must never move a star. Each is a fixed sequence of IEEE
//! operations and [`math`](crate::math) calls on a documented number of words, so its output is
//! part of the generator version:
//!
//! | Sampler                                                   | Words                    |
//! | --------------------------------------------------------- | ------------------------ |
//! | [`uniform`](Stream::uniform) and its open forms, [`uniform_in`](Stream::uniform_in) | 1 |
//! | [`below`](Stream::below)                                  | 1 per attempt, under 2 on average |
//! | [`standard_normal`](Stream::standard_normal), [`standard_normal_pair`](Stream::standard_normal_pair), [`normal`](Stream::normal), [`log_normal`](Stream::log_normal), [`log_normal_dex`](Stream::log_normal_dex) | 2 |
//! | [`poisson`](Stream::poisson) below [`POISSON_PTRS_MIN_MEAN`] | 1 (0 at a mean of 0)  |
//! | [`poisson`](Stream::poisson) from [`POISSON_PTRS_MIN_MEAN`] | 2 per attempt         |
//! | [`power_law`](Stream::power_law)                          | 1                        |
//! | [`PiecewisePowerLaw::sample`], [`PiecewiseLinear::sample`] | 2                       |
//! | [`mark`](Stream::mark), [`decide`](Stream::decide), [`pick`](Stream::pick) | 1       |
//!
//! # Decisions
//!
//! A random decision compares a [`Mark`], the top 53 bits of one word, against a 53-bit
//! [`Threshold`], `ceil(p × 2⁵³)`: exactly `uniform() < p` on the same word, with `p = 1` always
//! accepting. [`Thresholds`] and [`Mark::pick_weighted`] pick a class, or reject, with one mark.
//! This is a convention, not a second line of defence: a threshold is computed from floats, so
//! reproducibility still rests on the pinned `libm`.
//!
//! # Events
//!
//! A system's or body's events in time are keyed in two steps. [`EventKey::derive`] takes one
//! block under the event tag's domain tag, counter `(system's raw ID, sub << 48 | block)`, where a
//! system has `sub` 0 and block 0 and a body `sub` = its body index and block 1. The event key is
//! then the key of ordinary streams whose counter is `(k, slot << 48 | block)`: slot 0 is bin `k`'s
//! own stream ([`EventKey::bin_stream`]) and slot `j + 1` event `j`'s
//! ([`EventKey::event_stream`]).
//!
//! # The central promise
//!
//! Drawing a new property under a new tag leaves an existing property's values unchanged. A later
//! version of a generator adds a star's flares under their own event tag and draws them first; the
//! star's mass, drawn from its own stream, does not move. Drawn from the mass's stream instead, as
//! one random sequence would, the flares would have moved it.
//!
//! ```
//! use hyperion_sim::Seed;
//! use hyperion_sim::id::{EventBin, SystemId, event_tags};
//! use hyperion_sim::rng::{EventKey, PowerLaw, Stream, tags};
//!
//! let (seed, star) = (Seed::new(42), SystemId::from_raw(0x0200_0800_2000_0007)?);
//! let masses = PowerLaw::new(2.3, 0.5, 150.0)?;
//! let mass_stream = || Stream::open(seed, tags::SELFTEST_STREAM, star.into());
//!
//! // Version 1 draws the mass.
//! let mass_v1 = mass_stream().power_law(&masses);
//!
//! // Version 2 draws flares under a new tag first, then the mass.
//! let flare_key = EventKey::derive(seed, event_tags::SELF_TEST, star.into());
//! let flares = flare_key.bin_stream(EventBin::new(0)?).poisson(0.7);
//! let mass_v2 = mass_stream().power_law(&masses);
//! assert!(mass_v2.total_cmp(&mass_v1).is_eq());
//! assert!(flares < 10);
//!
//! // One sequence for both would have moved the mass.
//! let mut sequence = mass_stream();
//! sequence.poisson(0.7);
//! assert!(sequence.power_law(&masses).total_cmp(&mass_v1).is_ne());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod decide;
mod domain_tag;
mod event;
mod key;
mod raw_event;
mod sample;
mod stream;
pub mod tags;
mod threefry;

pub use decide::{Mark, Threshold, Thresholds};
pub use domain_tag::{
    DomainTag, TagScope, assert_registries_disjoint, assert_tag_names, hash_tag_name,
    is_valid_tag_name,
};
pub use event::EventKey;
pub use key::{ObjectKey, ParseSeedError, Seed};
pub use raw_event::RawEventKey;
pub use sample::{
    BuildPiecewiseError, BuildPowerLawError, POISSON_MAX_MEAN, POISSON_PTRS_MIN_MEAN,
    PiecewiseLinear, PiecewisePowerLaw, PowerLaw,
};
pub use stream::Stream;
pub use threefry::threefry2x64_20;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId};

    const SEED: Seed = Seed::new(0x5eed_0000_0000_0001);

    /// A second self-test tag, minted here for the tests that vary the tag. It is not in the
    /// registry because nothing outside these tests opens it.
    const OTHER_TAG: DomainTag = DomainTag::registered("selftest.other", TagScope::SelfTest);

    fn first_words(stream: &Stream, n: u64) -> Vec<u64> {
        let mut stream = stream.clone();
        (0..n).map(|_| stream.next_u64()).collect()
    }

    fn open(object: ObjectKey) -> Stream {
        Stream::open(SEED, tags::SELFTEST_STREAM, object)
    }

    fn grid_id(cell: [i32; 3], index: u32) -> SystemId {
        SystemId::from_parts(Layer::A, GenCell::new(CellSize::Ly8, cell).unwrap(), index).unwrap()
    }

    /// Asserts that no word of `streams`' first 1,000 appears in two of them.
    fn assert_disjoint(what: &str, streams: &[Stream]) {
        let mut seen = BTreeSet::new();
        for (i, stream) in streams.iter().enumerate() {
            for word in first_words(stream, 1_000) {
                assert!(seen.insert(word), "{what}: stream {i} repeats a word");
            }
        }
    }

    /// The structured inputs the brainstorm names: adjacent cells, consecutive candidates and
    /// consecutive body indices, and one-bit changes of seed and tag.
    #[test]
    fn streams_differing_in_any_one_input_share_no_word() {
        assert_disjoint(
            "seeds",
            &[0, 1, 2, 1 << 63]
                .map(|s| Stream::open(Seed::new(s), tags::SELFTEST_STREAM, ObjectKey::galaxy())),
        );
        assert_disjoint(
            "tags",
            &[tags::SELFTEST_STREAM, OTHER_TAG].map(|t| Stream::open(SEED, t, ObjectKey::galaxy())),
        );
        let adjacent_cells = [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1], [-1, 0, 0]]
            .map(|c| open(ObjectKey::cell(grid_id(c, 0).cell_word())));
        assert_disjoint("adjacent cells", &adjacent_cells);
        let candidates = [0, 1, 2, 3].map(|i| open(grid_id([5, -2, 7], i).into()));
        assert_disjoint("consecutive candidates", &candidates);
        let system = grid_id([5, -2, 7], 1);
        let bodies = [0, 1, 2, 3].map(|b| open(BodyId::new(system, b).into()));
        assert_disjoint("consecutive bodies", &bodies);
    }
}
