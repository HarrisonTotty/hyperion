//! Threads for the tests that run their own.
//!
//! nextest runs the tests on a pool of slots, its test threads (`--test-threads`, or
//! `NEXTEST_TEST_THREADS`, by default the machine's logical CPUs), and a test that runs threads of
//! its own reserves that many slots with `threads-required` in `.config/nextest.toml`. nextest caps
//! a reservation at the pool: its scheduler (future-queue 0.4.0, under nextest 0.9.150) sets a
//! test's weight above the pool's size to the pool's size, so a test reserving 8 slots on a run of
//! 4 test threads holds all 4 and runs alone, and running 8 threads it would put 8 on 4 slots
//! (checked on 2026-10-29: two 8-slot tests and four 1-slot tests at `--test-threads 4` ran one
//! 8-slot test at a time, then the four). nextest gives each test process the run's test threads as
//! `NEXTEST_TEST_THREADS` (since 0.9.130), but not the slots the test holds, so [`granted`] works
//! them out from the reservation the test states: the fewer of the two.
//!
//! [`in_shares`] runs a test's shares of work on that many threads. A test whose shares are fixed
//! (share k the records k, k + n, …, merged in share order) gets the same results on any number of
//! threads.

use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The variable through which nextest gives each test process the run's test threads.
const TEST_THREADS_VAR: &str = "NEXTEST_TEST_THREADS";

/// How many threads a test that reserves `reserved` slots may run, at least one.
///
/// That is `reserved` (the test's `threads-required` in `.config/nextest.toml`), or the run's test
/// threads where nextest runs fewer, which is all nextest grants it; `reserved` outside nextest
/// (under `cargo test`); one on WebAssembly, which has no threads.
#[must_use]
pub fn granted(reserved: usize) -> usize {
    if cfg!(target_family = "wasm") {
        return 1;
    }
    granted_of(reserved, std::env::var(TEST_THREADS_VAR).ok().as_deref())
}

/// [`granted`] on a run whose `NEXTEST_TEST_THREADS` is `test_threads` (`None` outside nextest): an
/// unreadable or zero count is taken as no count.
fn granted_of(reserved: usize, test_threads: Option<&str>) -> usize {
    let pool = test_threads
        .and_then(|t| t.trim().parse::<usize>().ok())
        .filter(|&n| n > 0);
    pool.map_or(reserved, |n| reserved.min(n)).max(1)
}

/// Runs `share(0)` to `share(shares − 1)` on at most [`granted`]`(reserved)` threads and returns
/// their results in share order.
///
/// The threads take the next share as they finish one. Where that is one thread (on WebAssembly
/// among others), the shares run on this thread, one after another; otherwise on threads of their
/// own. So a share must not read or write the caller's thread-local state (a test's switch of the
/// sim's rules, a counter), which it would see in one case and not in the other.
///
/// # Panics
///
/// If a share panics: the panic of the lowest share that does, on any number of threads, is
/// resumed here, once the shares before it have run.
pub fn in_shares<T: Send>(
    shares: usize,
    reserved: usize,
    share: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    in_shares_on(granted(reserved).min(shares), shares, share)
}

/// [`in_shares`] on `threads` threads.
fn in_shares_on<T: Send>(
    threads: usize,
    shares: usize,
    share: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    if threads <= 1 {
        return (0..shares).map(share).collect();
    }
    let next = AtomicUsize::new(0);
    // Once a share has panicked, no thread takes another: every share before it has been taken, so
    // the lowest that panics is among those run.
    let failed = AtomicBool::new(false);
    let worker = || {
        let mut mine = Vec::new();
        while !failed.load(Ordering::Relaxed) {
            let k = next.fetch_add(1, Ordering::Relaxed);
            if k >= shares {
                break;
            }
            let result = panic::catch_unwind(AssertUnwindSafe(|| share(k)));
            failed.fetch_or(result.is_err(), Ordering::Relaxed);
            mine.push((k, result));
        }
        mine
    };
    let mut done: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads).map(|_| scope.spawn(worker)).collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_else(|e| panic::resume_unwind(e)))
            .collect()
    });
    done.sort_unstable_by_key(|(k, _)| *k);
    done.into_iter()
        .map(|(_, result)| result.unwrap_or_else(|e| panic::resume_unwind(e)))
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn a_reservation_is_capped_at_the_runs_test_threads() {
        assert_eq!(granted_of(8, Some("4")), 4);
        assert_eq!(granted_of(4, Some("16")), 4);
        assert_eq!(granted_of(8, Some(" 2\n")), 2);
        assert_eq!(granted_of(8, None), 8, "under cargo test");
        assert_eq!(granted_of(8, Some("0")), 8, "no count");
        assert_eq!(granted_of(8, Some("num-cpus")), 8, "no count");
        assert_eq!(granted_of(0, Some("4")), 1, "at least one");
    }

    #[test]
    fn granted_reads_the_run_it_is_in() {
        let expected = if cfg!(target_family = "wasm") {
            1
        } else {
            granted_of(10_000, std::env::var(TEST_THREADS_VAR).ok().as_deref())
        };
        assert_eq!(granted(10_000), expected);
    }

    #[test]
    fn shares_come_back_in_their_order() {
        assert_eq!(in_shares(7, 3, |k| k * k), vec![0, 1, 4, 9, 16, 25, 36]);
        assert_eq!(in_shares(0, 4, |k| k), Vec::<usize>::new());
    }

    /// The tests that run threads, which WebAssembly has none of (compiled out there, as the
    /// browser suites' check expects of a module of this name).
    #[cfg(not(target_family = "wasm"))]
    mod native_only {
        use super::*;

        /// Work that takes about `rounds` multiplications, so that shares finish out of order.
        fn busy(seed: usize, rounds: usize) -> u64 {
            let mut x = u64::try_from(seed).expect("a share number fits 64 bits");
            for _ in 0..rounds {
                x = x
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
            }
            x
        }

        /// Shares of uneven cost come back the same on one, two, three and seven threads.
        #[test]
        fn any_number_of_threads_gives_the_same_results() {
            let share = |k: usize| (k, busy(k, (k % 3) * 20_000));
            let one = in_shares_on(1, 11, share);
            for threads in [2, 3, 7] {
                assert_eq!(in_shares_on(threads, 11, share), one, "{threads} threads");
            }
            let order: Vec<usize> = one.iter().map(|&(k, _)| k).collect();
            assert_eq!(order, (0..11).collect::<Vec<_>>());
        }

        /// Of two shares that panic, the lower's panic is resumed on any number of threads, though
        /// the higher, quicker one panics first on several.
        #[test]
        fn the_lowest_failing_shares_panic_is_resumed() {
            for threads in [1, 2, 4] {
                let caught = panic::catch_unwind(|| {
                    in_shares_on(threads, 8, |k| {
                        assert_ne!(k, 5, "share 5 fails");
                        if k == 2 {
                            assert_ne!(busy(k, 2_000_000), 0, "share 2's work");
                            panic!("share 2 fails");
                        }
                        k
                    })
                });
                let payload = caught.expect_err("a share panics");
                let message = payload.downcast_ref::<&str>().copied().unwrap_or_default();
                assert_eq!(message, "share 2 fails", "{threads} threads");
            }
        }
    }
}
