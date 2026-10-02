//! Random streams: the sim's event keys and its registry of domain tags, over the foundation's
//! mechanism.
//!
//! Streams, keys, samplers, decisions and the block function live in `hyperion_base::rng`, and
//! every item of it is re-exported here, so that `hyperion_sim::rng::Stream` and the rest name
//! what they always named. This module adds what needs the sim's ID types: [`EventKey`], over
//! base's [`RawEventKey`], and [`tags`], the registry of every stage's domain tags, which re-exports
//! base's `SELFTEST_STREAM` and asserts itself disjoint from base's registry and the surface
//! crate's.
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

mod event;
pub mod tags;

pub use event::EventKey;
pub use hyperion_base::rng::*;

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
