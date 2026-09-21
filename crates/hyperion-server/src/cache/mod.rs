//! Caches the server owns, bounded by bytes.
//!
//! The sim holds no caches: its generators are pure functions and the caller keeps what it wants
//! to keep (brainstorm, "Runtime and code shape"). The server keeps one byte budget per kind of
//! cached thing for the whole process, with the galaxy's `(seed, generator_version)` in every key,
//! so that two saves of one seed share their entries (plan 04, design note 23).

mod byte_lru;

pub use byte_lru::{
    ByteLru, ENTRY_OVERHEAD_BYTES, HeapBytes, Insertion, LruCounters, SharedByteLru,
};
