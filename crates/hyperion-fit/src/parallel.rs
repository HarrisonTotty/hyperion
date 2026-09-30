//! Deterministic parallel map-reduce (plan 15, P15.T1 and Design note 2).
//!
//! Work is cut into chunks whose boundaries depend only on the item count and the chunk size, each
//! chunk's result is a pure function of its range, and the results are handed to the reduction in
//! index order. The thread count therefore cannot change a bit of the output. `rayon` is used here
//! and nowhere else.

use std::num::NonZeroUsize;
use std::ops::Range;

use rayon::prelude::*;

/// How many chunks each thread is given per wave: enough to balance uneven chunks, few enough that
/// a wave's results stay small in memory.
const CHUNKS_PER_THREAD_PER_WAVE: usize = 8;

/// Maps the items `0..n_items`, cut into consecutive ranges of `chunk` items (the last shorter),
/// through `map` on `threads` threads, and hands each result to `reduce` in index order.
///
/// Chunks are mapped in waves of a few per thread, and each wave is reduced before the next is
/// mapped, so at most a wave's results are held at once. The output is the same for every thread
/// count.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If `chunk` is zero, or `map` or `reduce` panics.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroUsize;
///
/// use hyperion_fit::parallel::map_reduce_chunks;
///
/// let mut total = 0_u64;
/// let four = NonZeroUsize::new(4).expect("four is not zero");
/// map_reduce_chunks(1_000, 64, four, |range| range.sum::<u64>(), |sum| total += sum)?;
/// assert_eq!(total, 499_500);
/// # Ok::<(), hyperion_fit::parallel::BuildThreadPoolError>(())
/// ```
pub fn map_reduce_chunks<T, M, R>(
    n_items: u64,
    chunk: u64,
    threads: NonZeroUsize,
    map: M,
    mut reduce: R,
) -> Result<(), BuildThreadPoolError>
where
    T: Send,
    M: Fn(Range<u64>) -> T + Sync,
    R: FnMut(T),
{
    assert!(chunk > 0, "a chunk holds at least one item");
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.get())
        .build()
        .map_err(BuildThreadPoolError)?;
    let n_chunks = n_items.div_ceil(chunk);
    let wave =
        u64::try_from(threads.get().saturating_mul(CHUNKS_PER_THREAD_PER_WAVE)).unwrap_or(u64::MAX);
    let range_of = |i: u64| i * chunk..((i + 1) * chunk).min(n_items);
    let mut first = 0;
    while first < n_chunks {
        let last = first.saturating_add(wave).min(n_chunks);
        let results: Vec<T> = pool.install(|| {
            (first..last)
                .into_par_iter()
                .map(|i| map(range_of(i)))
                .collect()
        });
        results.into_iter().for_each(&mut reduce);
        first = last;
    }
    Ok(())
}

/// The thread pool of [`map_reduce_chunks`] could not be built.
#[derive(Debug, thiserror::Error)]
#[error("cannot build the fit's thread pool")]
pub struct BuildThreadPoolError(#[source] rayon::ThreadPoolBuildError);

#[cfg(test)]
mod tests {
    use super::*;

    /// A pseudo-random `f64` in [0, 1) from item `i`: `SplitMix64`'s output, top 53 bits.
    fn value(i: u64) -> f64 {
        let mut z = i.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        #[expect(clippy::cast_precision_loss, reason = "53 bits are exact in an f64")]
        let v = (z >> 11) as f64 / (1_u64 << 53) as f64;
        v
    }

    /// The sum of 10⁶ pseudo-random values, summed within each chunk and then chunk by chunk in
    /// index order: floating-point addition does not associate, so any other order would show in
    /// the bits.
    fn sum(threads: usize) -> f64 {
        let mut total = 0.0;
        map_reduce_chunks(
            1_000_000,
            1_000,
            NonZeroUsize::new(threads).unwrap(),
            |range| range.map(value).sum::<f64>(),
            |partial| total += partial,
        )
        .unwrap();
        total
    }

    #[test]
    fn map_reduce_gives_identical_bytes_for_one_and_four_threads() {
        let one = sum(1);
        let four = sum(4);
        assert!(
            one.total_cmp(&four).is_eq(),
            "{one:?} against {four:?}: the bit patterns differ"
        );
        assert!((one - 500_000.0).abs() < 2_000.0, "{one}");
    }

    #[test]
    fn chunks_cover_every_item_once_in_order() {
        let mut ranges = Vec::new();
        map_reduce_chunks(
            10,
            3,
            NonZeroUsize::new(2).unwrap(),
            |range| range,
            |range| ranges.push(range),
        )
        .unwrap();
        assert_eq!(ranges, [0..3, 3..6, 6..9, 9..10]);
        let mut none = 0;
        map_reduce_chunks(
            0,
            3,
            NonZeroUsize::new(2).unwrap(),
            |range| range,
            |_| none += 1,
        )
        .unwrap();
        assert_eq!(none, 0);
    }
}
