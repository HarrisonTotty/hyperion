//! Raw event keys: the key derivation of a system's or body's event streams, over plain words.
//!
//! The event streams are keyed in two steps (the sim's `EventKey` documents the layout). Step 1 is
//! one block of [`threefry2x64_20`] under a domain tag of scope [`TagScope::Event`], and step 2
//! opens ordinary streams whose key words are that block's output. This module does both over
//! plain words, so that the mechanism needs nothing of the IDs: the counter of step 1 and the bin
//! word of step 2 are integers the caller has already formed from its subject and bin. The sim's
//! `EventKey` is a newtype over [`RawEventKey`] that forms them from its ID types.
//!
//! Only a registered tag of scope `Event` derives a key, as only a registered tag of any other
//! scope opens an ordinary stream, so the key discipline is no wider than [`Stream::open`]'s.

use super::threefry::threefry2x64_20;
use super::{DomainTag, Seed, Stream, TagScope};

/// An event key of step 1: the two key words of a subject's event streams under one event tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawEventKey([u64; 2]);

impl RawEventKey {
    /// Step 1: one block, key `(seed, tag's hash)` and counter `counter`.
    ///
    /// # Panics
    ///
    /// If `tag`'s scope is not [`TagScope::Event`], as [`Stream::open`] panics on an `Event` tag:
    /// an event key and an ordinary stream never share a tag, so they never share a block.
    #[must_use]
    pub fn derive(seed: Seed, tag: DomainTag, counter: [u64; 2]) -> Self {
        assert!(
            tag.scope() == TagScope::Event,
            "domain tag {} has scope {:?}, but an event key needs a tag of scope Event",
            tag.name(),
            tag.scope(),
        );
        Self(threefry2x64_20([seed.get(), tag.hash()], counter))
    }

    /// Step 2: the stream whose key words are this key and whose counter is
    /// `(bin_word, slot << 48 | block)`, at word 0.
    #[must_use]
    pub fn stream(&self, bin_word: u64, slot: u16) -> Stream {
        Stream::from_words(self.0, bin_word, slot)
    }

    /// The two key words.
    #[must_use]
    pub const fn words(self) -> [u64; 2] {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::tags;

    const SEED: Seed = Seed::new(0x0e7e_0000_0000_0002);

    /// A tag of scope `Event`, minted here: base's registry holds none, so the positive path is
    /// covered by the sim's `EventKey` tests and its `rng/events` golden.
    const EVENT_TAG: DomainTag = DomainTag::registered("selftest.raw_event", TagScope::Event);

    #[test]
    fn step_one_is_one_block_under_the_tag() {
        let key = RawEventKey::derive(SEED, EVENT_TAG, [7, 1]);
        assert_eq!(
            key.words(),
            threefry2x64_20([SEED.get(), EVENT_TAG.hash()], [7, 1])
        );
    }

    #[test]
    fn step_two_counters_hold_the_bin_word_and_the_slot() {
        let key = RawEventKey::derive(SEED, EVENT_TAG, [7, 0]);
        let stream = key.stream(u64::MAX, 3);
        assert_eq!(
            [stream.word_at(2), stream.word_at(3)],
            threefry2x64_20(key.words(), [u64::MAX, (3 << 48) | 1])
        );
    }

    #[test]
    #[should_panic(expected = "an event key needs a tag of scope Event")]
    fn a_raw_event_key_refuses_a_tag_of_another_scope() {
        let _ = RawEventKey::derive(SEED, tags::SELFTEST_STREAM, [0, 0]);
    }
}
