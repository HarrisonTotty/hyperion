//! Streams: the words drawn for one (seed, domain tag, object).

use super::threefry::threefry2x64_20;
use super::{DomainTag, ObjectKey, Seed, TagScope};

/// Blocks in a stream: `block` is the low 48 bits of counter word 1.
const BLOCKS: u64 = 1 << 48;

/// Bit position of `sub` in counter word 1.
const SUB_SHIFT: u32 = 48;

/// A random stream: the words drawn for one object under one domain tag, in a universe.
///
/// A stream is a window onto the Threefry2x64-20 block function ([`threefry2x64_20`]) with its key
/// and counter fixed by what it is for:
///
/// | Word           | Contents                                                          |
/// | -------------- | ----------------------------------------------------------------- |
/// | Key word 0     | The universe [`Seed`]                                             |
/// | Key word 1     | The domain tag's hash, [`DomainTag::hash`]                        |
/// | Counter word 0 | The object's word, [`ObjectKey::word`]                            |
/// | Counter word 1 | `sub << 48 \| block`: [`ObjectKey::sub`] and the 48-bit block number |
///
/// Word `n` of the stream is output `n & 1` of block `n >> 1`, so a stream holds 2⁴⁹ words
/// ([`Stream::WORDS`]). Because the block function is a bijection on the counter for a fixed key,
/// two streams that differ in the object's word or `sub` never share a block, and streams under
/// different seeds or tags are unrelated. The generator version is not in the key, so a version
/// bump moves only what its code change moves.
///
/// The words are what the samplers consume: [`uniform`](Self::uniform), [`normal`](Self::normal),
/// [`poisson`](Self::poisson) and the rest each document how many they take. A stream is a plain
/// value, its position and nothing else of consequence, so cloning one forks it: both copies draw
/// the same words from there on.
///
/// # Examples
///
/// Random access and sequential draws agree:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
///
/// let mut stream = Stream::open(Seed::new(42), tags::SELFTEST_STREAM, ObjectKey::galaxy());
/// let third = stream.word_at(2);
/// stream.seek(2);
/// assert_eq!(stream.next_u64(), third);
/// assert_eq!(stream.position(), 3);
/// ```
///
/// The central promise: drawing a new property under a new tag leaves an existing property's
/// values unchanged. A later version of a generator adds a star's flares under their own event
/// tag and draws them first; the star's mass, drawn from its own stream, does not move. Drawn from
/// the mass's stream instead, as one random sequence would, the flares would have moved it.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{EventBin, SystemId, event_tags};
/// use hyperion_sim::rng::{EventKey, PowerLaw, Stream, tags};
///
/// let (seed, star) = (Seed::new(42), SystemId::from_raw(0x0200_0800_2000_0007)?);
/// let masses = PowerLaw::new(2.3, 0.5, 150.0)?;
/// let mass_stream = || Stream::open(seed, tags::SELFTEST_STREAM, star.into());
///
/// // Version 1 draws the mass.
/// let mass_v1 = mass_stream().power_law(&masses);
///
/// // Version 2 draws flares under a new tag first, then the mass.
/// let flare_key = EventKey::derive(seed, event_tags::SELF_TEST, star.into());
/// let flares = flare_key.bin_stream(EventBin::new(0)?).poisson(0.7);
/// let mass_v2 = mass_stream().power_law(&masses);
/// assert!(mass_v2.total_cmp(&mass_v1).is_eq());
/// assert!(flares < 10);
///
/// // One sequence for both would have moved the mass.
/// let mut sequence = mass_stream();
/// sequence.poisson(0.7);
/// assert!(sequence.power_law(&masses).total_cmp(&mass_v1).is_ne());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stream {
    /// `(seed, tag hash)`.
    key: [u64; 2],
    /// Counter word 0: the object's word.
    object: u64,
    /// The high bits of counter word 1: `sub << 48`.
    sub_bits: u64,
    /// The number of the next word `next_u64` returns.
    position: u64,
    /// When `position` is odd, the second word of block `position >> 1`, which the previous call
    /// computed; otherwise 0, so that two streams in the same place compare equal.
    pending: u64,
}

impl Stream {
    /// The number of words in a stream: 2⁴⁸ blocks of two.
    pub const WORDS: u64 = 1 << 49;

    /// Opens the stream of `object` under `tag` in the universe of `seed`, at word 0.
    ///
    /// # Panics
    ///
    /// If the key's scope is not the tag's (Design note 3 of the determinism plan): a tag fixes
    /// what its counter word names, which is what lets a cell's word equal its candidate 0's ID
    /// and body 0 share its system's word. A tag of scope [`TagScope::SelfTest`] accepts a key of
    /// any scope, because tests exercise every kind of key; no key has scope [`TagScope::Event`],
    /// so an event tag is never opened here. The check holds in release builds too: a mismatch
    /// would silently alias two objects' streams and change a galaxy, and it costs two byte
    /// comparisons.
    #[must_use]
    pub fn open(seed: Seed, tag: DomainTag, object: ObjectKey) -> Self {
        assert!(
            tag.scope() == object.scope() || tag.scope() == TagScope::SelfTest,
            "domain tag {} has scope {:?}, but the object key has scope {:?}",
            tag.name(),
            tag.scope(),
            object.scope(),
        );
        Self::from_words([seed.get(), tag.hash()], object.word(), object.sub())
    }

    /// A stream from its raw key words, counter word 0 and `sub`, at word 0.
    ///
    /// [`open`](Self::open) keys a stream by seed and domain tag; an event's streams are keyed by
    /// its subject's event key instead ([`EventKey`](super::EventKey)), which is why this exists.
    #[must_use]
    pub(super) fn from_words(key: [u64; 2], object: u64, sub: u16) -> Self {
        Self {
            key,
            object,
            sub_bits: u64::from(sub) << SUB_SHIFT,
            position: 0,
            pending: 0,
        }
    }

    /// The counter of block `block`: `(object word, sub << 48 | block)`.
    ///
    /// # Panics
    ///
    /// If `block` is 2⁴⁸ or more, where it would run into the next `sub`.
    #[inline]
    fn counter(&self, block: u64) -> [u64; 2] {
        assert!(
            block < BLOCKS,
            "a stream holds 2^49 words; block {block} is past its end"
        );
        [self.object, self.sub_bits | block]
    }

    /// The two words of block `block`.
    #[inline]
    fn block(&self, block: u64) -> [u64; 2] {
        threefry2x64_20(self.key, self.counter(block))
    }

    /// The next word, advancing the stream by one.
    ///
    /// A call at an even position computes a block and keeps its second word for the next call.
    ///
    /// # Panics
    ///
    /// If the stream is exhausted, after 2⁴⁹ words. That is unreachable in practice: at a
    /// nanosecond a word it takes six days.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let n = self.position;
        let word = if n & 1 == 0 {
            let [first, second] = self.block(n >> 1);
            self.pending = second;
            first
        } else {
            std::mem::take(&mut self.pending)
        };
        // `block` rejected n ≥ 2⁴⁹ above, and an odd n is below 2⁴⁹ too, so this cannot overflow.
        self.position = n + 1;
        word
    }

    /// Word `n` of the stream, without moving it.
    ///
    /// # Panics
    ///
    /// If `n` is [`Stream::WORDS`] or more.
    #[inline]
    #[must_use]
    pub fn word_at(&self, n: u64) -> u64 {
        let [first, second] = self.block(n >> 1);
        if n & 1 == 0 { first } else { second }
    }

    /// The number of the word the next draw returns: how many words have been drawn, unless the
    /// stream was moved with [`seek`](Self::seek).
    #[must_use]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Moves the stream so that the next draw returns word `n`.
    ///
    /// Seeking is what lets a draw be addressed by number: a property drawn from words `4k` to
    /// `4k + 3` can be read for any `k` without drawing the ones before it.
    ///
    /// # Panics
    ///
    /// If `n` is above [`Stream::WORDS`]. Seeking to exactly `WORDS` is allowed; drawing there
    /// panics.
    pub fn seek(&mut self, n: u64) {
        assert!(
            n <= Self::WORDS,
            "a stream holds 2^49 words; cannot seek to word {n}"
        );
        self.position = n;
        self.pending = if n & 1 == 0 { 0 } else { self.block(n >> 1)[1] };
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::{BodyId, Layer, SystemId};
    use crate::rng::tags;

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

    #[test]
    fn word_at_equals_the_sequential_draws() {
        let stream = open(ObjectKey::galaxy_item(3));
        let mut sequential = stream.clone();
        for n in 0..=1_000 {
            assert_eq!(sequential.position(), n);
            assert_eq!(stream.word_at(n), sequential.next_u64(), "word {n}");
        }
    }

    #[test]
    fn words_are_the_block_outputs_in_order() {
        let mut stream = open(ObjectKey::cell(0xabc));
        for block in 0..4 {
            let expected =
                threefry2x64_20([SEED.get(), tags::SELFTEST_STREAM.hash()], [0xabc, block]);
            assert_eq!([stream.next_u64(), stream.next_u64()], expected);
        }
    }

    #[test]
    fn seek_lands_on_the_word_asked_for_at_either_parity() {
        let stream = open(ObjectKey::feature(99));
        let words = first_words(&stream, 41);
        for target in [0, 1, 2, 17, 38, 39] {
            let mut moved = open(ObjectKey::feature(99));
            moved.next_u64();
            moved.seek(target);
            assert_eq!(moved.position(), target);
            let n = usize::try_from(target).unwrap();
            assert_eq!(moved.next_u64(), words[n], "seek to {target}");
            assert_eq!(moved.next_u64(), words[n + 1], "after seek to {target}");
        }
    }

    #[test]
    fn two_streams_at_the_same_word_are_equal() {
        let mut a = open(ObjectKey::galaxy());
        let mut b = a.clone();
        a.next_u64();
        a.next_u64();
        b.seek(2);
        assert_eq!(a, b);
        a.next_u64();
        b.seek(3);
        assert_eq!(a, b);
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

    /// Body 1's first block and body 0's last block are neighbours in counter word 1, one apart;
    /// they are different counters, and body 0 cannot draw past its last block into body 1's.
    #[test]
    fn sub_and_block_do_not_alias() {
        let system = grid_id([0, 0, 0], 0);
        let body0 = open(BodyId::new(system, 0).into());
        let body1 = open(BodyId::new(system, 1).into());
        let last = body0.counter(BLOCKS - 1);
        let first = body1.counter(0);
        assert_eq!(last, [system.raw(), 0x0000_ffff_ffff_ffff]);
        assert_eq!(first, [system.raw(), 0x0001_0000_0000_0000]);
        assert_ne!(last, first);
        assert_ne!(
            body0.word_at(Stream::WORDS - 1),
            body1.word_at(0),
            "the last word of body 0 is not body 1's first"
        );
    }

    #[test]
    #[should_panic(expected = "past its end")]
    fn drawing_past_the_last_block_panics() {
        let mut stream = open(ObjectKey::galaxy());
        stream.seek(Stream::WORDS - 1);
        stream.next_u64();
        stream.next_u64();
    }

    #[test]
    #[should_panic(expected = "cannot seek")]
    fn seeking_past_the_end_panics() {
        open(ObjectKey::galaxy()).seek(Stream::WORDS + 1);
    }

    #[test]
    fn keys_hold_seed_and_tag_and_counters_hold_the_object() {
        let system = grid_id([3, 4, 5], 6);
        let body = BodyId::new(system, 0x0102);
        let stream = open(body.into());
        assert_eq!(stream.key, [SEED.get(), tags::SELFTEST_STREAM.hash()]);
        assert_eq!(stream.counter(7), [system.raw(), (0x0102 << 48) | 7]);
        assert_eq!(open(system.into()).counter(7), [system.raw(), 7]);
    }

    #[test]
    #[should_panic(expected = "scope")]
    fn a_scope_mismatch_panics() {
        const CELL_TAG: DomainTag = DomainTag::registered("selftest.cell", TagScope::Cell);
        let _ = Stream::open(SEED, CELL_TAG, ObjectKey::galaxy());
    }

    #[test]
    #[should_panic(expected = "scope")]
    fn an_event_tag_is_never_opened_as_a_stream() {
        let _ = Stream::open(SEED, tags::EVENT_SELFTEST, ObjectKey::galaxy());
    }

    #[test]
    fn a_matching_scope_opens() {
        const CELL_TAG: DomainTag = DomainTag::registered("selftest.cell", TagScope::Cell);
        let mut stream = Stream::open(SEED, CELL_TAG, ObjectKey::cell(8));
        assert_eq!(stream.next_u64(), stream.word_at(0));
    }
}
