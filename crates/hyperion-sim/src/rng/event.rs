//! Event keys: the streams of a system's or body's events in time, keyed in two steps.
//!
//! A system's own events (flares, glitches, outbursts, eruptions) are generated as a pure function
//! of seed, ID and time, without stored state: time is cut into bins, or into the cycles of a
//! monotone phase, and the count and contents of bin `k` are "a pure function of (seed, ID, tag,
//! k)" (brainstorm, "Events in time"). The streams are keyed in two steps (brainstorm, "Random
//! streams, not a random sequence"), so that any bin can be asked for directly, in any order.
//!
//! **Step 1**, [`EventKey::derive`]: one block of the block function [`threefry2x64_20`] gives the
//! subject's event key under an event tag.
//!
//! | Word           | Contents                                                                         |
//! | -------------- | -------------------------------------------------------------------------------- |
//! | Key word 0     | The universe seed                                                                |
//! | Key word 1     | The hash of the event tag's domain tag, which has scope [`TagScope::Event`]      |
//! | Counter word 0 | The subject's system's raw ID                                                    |
//! | Counter word 1 | `sub << 48 \| block`: a system has `sub` 0 and block 0; a body has `sub` = its body index and block 1 |
//!
//! The two output words are the event key. A body's block is 1 so that body 0 does not share its
//! system's key: an event tag, unlike a domain tag, may name events of systems and of bodies
//! alike. No stream is ever opened under a tag of scope `Event`, so these blocks are shared with
//! nothing.
//!
//! **Step 2**, [`EventKey::bin_stream`] and [`EventKey::event_stream`]: an ordinary [`Stream`]
//! whose key words are the event key.
//!
//! | Word           | Contents                                                                                 |
//! | -------------- | ---------------------------------------------------------------------------------------- |
//! | Counter word 0 | The bin or cycle number k, its `i64` reinterpreted as `u64` (two's complement)           |
//! | Counter word 1 | `slot << 48 \| block`: slot 0 is the bin's own stream (count, times, thinning); slot j + 1 is event j's own stream, j = 0–255 |
//!
//! An [`EventId`]'s word (tag, k, j) therefore names exactly one stream. Every sampler works on
//! these streams, since they are ordinary.
//!
//! [`EventId`]: crate::id::EventId
//! [`TagScope::Event`]: super::TagScope::Event

use super::threefry::threefry2x64_20;
use super::{Seed, Stream};
use crate::id::{EventBin, EventSubject, EventTag};

/// The block of step 1 that holds a body's event key; a system's is block 0.
const BODY_BLOCK: u64 = 1;

/// Bit position of `sub` in counter word 1.
const SUB_SHIFT: u32 = 48;

/// A system's or body's event key under one event tag: the key words of its event streams.
///
/// # Examples
///
/// The events of bin 12 of a star, counted on the bin's stream, and the first event's own draws
/// on its own stream, asked for directly by its [`EventId`](crate::id::EventId):
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{EventBin, EventId, EventWord, SystemId, event_tags};
/// use hyperion_sim::rng::EventKey;
///
/// let star = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let key = EventKey::derive(Seed::new(7), event_tags::SELF_TEST, star.into());
///
/// let bin = EventBin::new(12)?;
/// let flares = key.bin_stream(bin).poisson(2.5);
/// assert!(flares < 20);
///
/// let first = EventId::new(star.into(), EventWord::new(event_tags::SELF_TEST, bin, 0));
/// let word = first.word();
/// let when = key.event_stream(word.bin(), word.number()).uniform();
/// assert!((0.0..1.0).contains(&when));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventKey([u64; 2]);

impl EventKey {
    /// Step 1: the event key of `subject` under `tag` in the universe of `seed`.
    ///
    /// One block: key `(seed, tag's domain-tag hash)`, counter `(system's raw ID, sub << 48 |
    /// block)`, with `sub` 0 and block 0 for a system, and `sub` the body index and block 1 for a
    /// body.
    #[must_use]
    pub fn derive(seed: Seed, tag: EventTag, subject: EventSubject) -> Self {
        let counter = match subject {
            EventSubject::System(system) => [system.raw(), 0],
            EventSubject::Body(body) => [
                body.system().raw(),
                (u64::from(body.body_index()) << SUB_SHIFT) | BODY_BLOCK,
            ],
        };
        Self(threefry2x64_20(
            [seed.get(), tag.domain_tag().hash()],
            counter,
        ))
    }

    /// The two key words.
    #[must_use]
    pub const fn words(self) -> [u64; 2] {
        self.0
    }

    /// Step 2, slot 0: the stream of bin (or cycle) `bin` itself, for its count, its times and its
    /// thinning, at word 0.
    #[must_use]
    pub fn bin_stream(&self, bin: EventBin) -> Stream {
        Stream::from_words(self.0, bin.get().cast_unsigned(), 0)
    }

    /// Step 2, slot `j + 1`: event `j`'s own stream in bin `bin`, at word 0.
    #[must_use]
    pub fn event_stream(&self, bin: EventBin, j: u8) -> Stream {
        Stream::from_words(self.0, bin.get().cast_unsigned(), u16::from(j) + 1)
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId, event_tags};
    use crate::rng::tags;

    const SEED: Seed = Seed::new(0x0e7e_0000_0000_0001);

    fn star() -> SystemId {
        SystemId::from_parts(
            Layer::A,
            GenCell::new(CellSize::Ly8, [652, -4_584, 2_047]).unwrap(),
            7,
        )
        .unwrap()
    }

    fn key(subject: EventSubject) -> EventKey {
        EventKey::derive(SEED, event_tags::SELF_TEST, subject)
    }

    fn bin(k: i64) -> EventBin {
        EventBin::new(k).unwrap()
    }

    fn first_words(mut stream: Stream, n: usize) -> Vec<u64> {
        (0..n).map(|_| stream.next_u64()).collect()
    }

    #[test]
    fn step_one_is_one_block_under_the_event_tag() {
        let star = star();
        let tag_key = [SEED.get(), tags::EVENT_SELFTEST.hash()];
        assert_eq!(
            key(star.into()).words(),
            threefry2x64_20(tag_key, [star.raw(), 0])
        );
        assert_eq!(
            key(BodyId::new(star, 0x0102).into()).words(),
            threefry2x64_20(tag_key, [star.raw(), (0x0102 << 48) | 1])
        );
    }

    #[test]
    fn step_two_counters_hold_the_bin_and_the_slot() {
        let key = key(star().into());
        for k in [EventBin::MIN.get(), -1, 0, 5, EventBin::MAX.get()] {
            let counter_0 = k.cast_unsigned();
            for n in 0..4_u64 {
                let bin_block = threefry2x64_20(key.words(), [counter_0, n]);
                let bin_stream = key.bin_stream(bin(k));
                assert_eq!(
                    [bin_stream.word_at(2 * n), bin_stream.word_at(2 * n + 1)],
                    bin_block
                );
                for j in [0_u8, 1, 255] {
                    let slot = u64::from(j) + 1;
                    let event_block = threefry2x64_20(key.words(), [counter_0, (slot << 48) | n]);
                    let event_stream = key.event_stream(bin(k), j);
                    assert_eq!(
                        [event_stream.word_at(2 * n), event_stream.word_at(2 * n + 1)],
                        event_block
                    );
                }
            }
        }
    }

    /// The brainstorm's "both event constructions return the same events in any order of asking",
    /// at the level of the streams: bins −3 to 3 asked forwards, backwards, shuffled and alone.
    #[test]
    fn bins_give_the_same_words_in_any_order_of_asking() {
        let subjects: [EventSubject; 3] = [
            star().into(),
            BodyId::new(star(), 0).into(),
            BodyId::new(star(), 9).into(),
        ];
        let keys: Vec<(EventSubject, i64)> = subjects
            .into_iter()
            .flat_map(|s| (-3..=3).map(move |k| (s, k)))
            .collect();
        assert_order_independent(&keys, |&(subject, k)| {
            let key = EventKey::derive(SEED, event_tags::SELF_TEST, subject);
            let mut words = first_words(key.bin_stream(bin(k)), 8);
            for j in [0, 1, 255] {
                words.extend(first_words(key.event_stream(bin(k), j), 4));
            }
            words
        });
    }

    /// Bin −1 is stored in an event word as forty ones, 2⁴⁰ − 1, but its counter word is the
    /// sign-extended value, so the stream is not the one a counter word of 2⁴⁰ − 1 would give.
    #[test]
    fn the_counter_uses_the_sign_extended_bin() {
        let key = key(star().into());
        let minus_one = EventBin::from_field((1 << 40) - 1);
        assert_eq!(minus_one, bin(-1));
        let stream = key.bin_stream(minus_one);
        assert_eq!(
            stream.word_at(0),
            threefry2x64_20(key.words(), [u64::MAX, 0])[0]
        );
        let unextended = Stream::from_words(key.words(), (1 << 40) - 1, 0);
        assert_ne!(
            first_words(stream, 16),
            first_words(unextended, 16),
            "bin -1 must not alias counter word 2^40 - 1"
        );
    }

    #[test]
    fn slot_zero_and_event_zero_differ() {
        let key = key(star().into());
        let bin_words = first_words(key.bin_stream(bin(4)), 16);
        let event_words = first_words(key.event_stream(bin(4), 0), 16);
        assert!(bin_words.iter().all(|w| !event_words.contains(w)));
    }

    #[test]
    fn neighbouring_bins_events_and_subjects_share_no_word() {
        let star = star();
        let mut streams = Vec::new();
        for subject in [
            star.into(),
            BodyId::new(star, 0).into(),
            BodyId::new(star, 1).into(),
            BodyId::new(star, u16::MAX).into(),
        ] {
            let key = key(subject);
            for k in [-2, -1, 0, 1] {
                streams.push(key.bin_stream(bin(k)));
                streams.extend((0..3).map(|j| key.event_stream(bin(k), j)));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for (i, stream) in streams.into_iter().enumerate() {
            for word in first_words(stream, 64) {
                assert!(seen.insert(word), "stream {i} repeats a word");
            }
        }
    }

    /// A system and all 65,536 of its possible bodies get 65,537 different keys under one tag:
    /// block 0 against block 1 separates the system from every body, and `sub` separates the
    /// bodies. With `sub << 48` alone, body 0 would repeat its system's key.
    #[test]
    fn a_system_and_every_body_index_have_keys_of_their_own() {
        let star = star();
        let mut keys = std::collections::BTreeSet::new();
        assert!(keys.insert(key(star.into())));
        for index in 0..=u16::MAX {
            assert!(
                keys.insert(key(BodyId::new(star, index).into())),
                "body {index} repeats a key"
            );
        }
        assert_eq!(keys.len(), 65_537);
    }

    /// Body 0 has its system's word and `sub` 0; its key differs because it is block 1.
    #[test]
    fn a_body_key_differs_from_its_system_key() {
        let star = star();
        let system = key(star.into());
        for index in [0, 1, u16::MAX] {
            assert_ne!(key(BodyId::new(star, index).into()), system, "body {index}");
        }
        assert_ne!(
            key(BodyId::new(star, 0).into()),
            key(BodyId::new(star, 1).into())
        );
        let next = SystemId::from_raw(star.raw() + 1).unwrap();
        assert_ne!(key(next.into()), system);
        assert_ne!(
            EventKey::derive(
                Seed::new(SEED.get() ^ 1),
                event_tags::SELF_TEST,
                star.into()
            ),
            system
        );
    }
}
