//! A built table's words, for a caller that keeps it beyond its process (rendering plan R06;
//! perf-sky-tables-on-disk): the server's sky tables, kept on disk across its restarts.
//!
//! The sim does no I/O and never takes a float's bits, as its Clippy bans say. So a table that a
//! caller stores is written word by word into the caller's [`WordSink`] and read back from its
//! [`WordSource`]: integers as they are, and each float as the caller encodes it, which must give
//! back its bits exactly. Every list is written as its length and its capacity, and read back
//! allocated at that capacity, so that a table read back owns the heap the one written did, which a
//! byte-bounded cache charges it. A table's reader checks what its readers index by, so that words
//! it did not write, but that hold together, read as `None`, never as a table that panics later.

/// Where a table's words are written.
pub trait WordSink {
    /// Writes `word`.
    fn word(&mut self, word: u64);

    /// Writes `value`, which the [`WordSource`] reading it back gives back bit for bit.
    fn float(&mut self, value: f64);

    /// Writes a length or an index.
    ///
    /// # Panics
    ///
    /// Never: a `usize` fits 64 bits on every target the workspace builds.
    fn count(&mut self, n: usize) {
        self.word(u64::try_from(n).expect("a length fits 64 bits on every target"));
    }

    /// Writes a list's length and capacity, which [`WordSource::list`] reads back and allocates
    /// it at.
    fn lengths(&mut self, len: usize, capacity: usize) {
        self.count(len);
        self.count(capacity);
    }

    /// Writes `values`' length and capacity ([`lengths`](Self::lengths)), then each value.
    fn floats(&mut self, values: &Vec<f64>) {
        self.lengths(values.len(), values.capacity());
        for &value in values {
            self.float(value);
        }
    }
}

/// Where a table's words are read back from, in the order a [`WordSink`] took them: `None` from any
/// read past their end.
///
/// # Examples
///
/// A caller's words as bytes, eight little-endian bytes to a word, round-tripping a galaxy's
/// tables (here those that hold no star, which make no track). A table's reader reads its own
/// words alone, so the caller checks that none is left over:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::words::{WordSink, WordSource};
///
/// struct Writer(Vec<u8>);
///
/// impl WordSink for Writer {
///     fn word(&mut self, word: u64) {
///         self.0.extend(word.to_le_bytes());
///     }
///
///     fn float(&mut self, value: f64) {
///         self.0.extend(value.to_le_bytes());
///     }
/// }
///
/// struct Reader<'a>(&'a [u8]);
///
/// impl WordSource for Reader<'_> {
///     fn word(&mut self) -> Option<u64> {
///         let (word, rest) = self.0.split_first_chunk::<8>()?;
///         self.0 = rest;
///         Some(u64::from_le_bytes(*word))
///     }
///
///     fn float(&mut self) -> Option<f64> {
///         self.word().map(f64::from_bits)
///     }
///
///     fn words_left(&self) -> usize {
///         self.0.len() / 8
///     }
/// }
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
/// let tables = LuminosityTables::dark(&galaxy);
/// let mut writer = Writer(Vec::new());
/// tables.write_words(&mut writer);
/// let mut reader = Reader(&writer.0);
/// let back = LuminosityTables::read_words(&mut reader, &galaxy).ok_or("the tables written")?;
/// assert_eq!(reader.words_left(), 0);
/// assert_eq!(back.heap_bytes(), tables.heap_bytes());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait WordSource {
    /// The next word.
    fn word(&mut self) -> Option<u64>;

    /// The next value, the bits written.
    fn float(&mut self) -> Option<f64>;

    /// The words left: what bounds a list's length before it is allocated.
    fn words_left(&self) -> usize;

    /// The next length or index.
    fn count(&mut self) -> Option<usize> {
        usize::try_from(self.word()?).ok()
    }

    /// A list's length and capacity ([`WordSink::lengths`]), then that many items, each read by
    /// `item`, which reads at least one word; allocated at the capacity written.
    ///
    /// Before anything is allocated, the length is checked against the words left, and the
    /// capacity against twice the length and some: a vector grown by doubling holds no more. A
    /// list that fails either check reads as none.
    fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>>
    where
        Self: Sized,
    {
        let (len, capacity) = (self.count()?, self.count()?);
        if len > self.words_left()
            || capacity < len
            || capacity > len.saturating_mul(2).saturating_add(64)
        {
            return None;
        }
        let mut items = Vec::with_capacity(capacity);
        for _ in 0..len {
            items.push(item(self)?);
        }
        Some(items)
    }

    /// A length and capacity, then that many values ([`list`](Self::list)).
    fn floats(&mut self) -> Option<Vec<f64>>
    where
        Self: Sized,
    {
        self.list(Self::float)
    }
}

/// Words in memory, for the sim's tests of a table's words.
#[cfg(test)]
pub(crate) mod testing {
    use super::{WordSink, WordSource};

    /// Words in memory, each float kept as its value, read back from the first: what a caller's
    /// sink and source do with bytes, without them.
    #[derive(Debug, Default)]
    pub(crate) struct Words {
        written: Vec<Word>,
        at: usize,
    }

    #[derive(Debug, Clone, Copy)]
    enum Word {
        Integer(u64),
        Float(f64),
    }

    impl Words {
        /// The words written, cut to the first `len`, to be read again from the first.
        #[must_use]
        pub(crate) fn cut(&self, len: usize) -> Self {
            Self {
                written: self.written[..len].to_vec(),
                at: 0,
            }
        }

        /// How many words were written.
        #[must_use]
        pub(crate) fn len(&self) -> usize {
            self.written.len()
        }
    }

    impl WordSink for Words {
        fn word(&mut self, word: u64) {
            self.written.push(Word::Integer(word));
        }

        fn float(&mut self, value: f64) {
            self.written.push(Word::Float(value));
        }
    }

    impl WordSource for Words {
        fn word(&mut self) -> Option<u64> {
            let Word::Integer(word) = *self.written.get(self.at)? else {
                return None;
            };
            self.at += 1;
            Some(word)
        }

        fn float(&mut self) -> Option<f64> {
            let Word::Float(value) = *self.written.get(self.at)? else {
                return None;
            };
            self.at += 1;
            Some(value)
        }

        fn words_left(&self) -> usize {
            self.written.len() - self.at
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Words;
    use super::*;

    /// A list reads back at its length and capacity, and one whose length passes the words left,
    /// or whose capacity is short of its length or far above it, reads as none.
    #[test]
    fn a_list_reads_back_at_its_capacity_and_refuses_what_cannot_be_one() {
        let mut values = Vec::with_capacity(9);
        values.extend([1.5, -0.0, f64::MAX]);
        let mut words = Words::default();
        WordSink::floats(&mut words, &values);
        let back = WordSource::floats(&mut words).expect("the list written");
        assert_eq!(back.len(), 3);
        assert_eq!(back.capacity(), 9);
        let bits = |values: &[f64]| {
            values
                .iter()
                .map(|&v| hyperion_testkit::float::bits(v))
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(&back), bits(&values));
        assert_eq!(words.words_left(), 0);
        for (len, capacity) in [(4, 4), (3, 2), (3, 71)] {
            let mut words = Words::default();
            words.lengths(len, capacity);
            for value in &values {
                WordSink::float(&mut words, *value);
            }
            assert!(
                WordSource::floats(&mut words).is_none(),
                "{len} of {capacity}"
            );
        }
    }
}
