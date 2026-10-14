//! Sample sizes for the tests that are heavy only for their sample: the whole sample natively and
//! on `wasm32-wasip1`, and a smaller one on `wasm32-unknown-unknown`, the browser target.
//!
//! The browser target runs a test binary's tests one at a time, in one V8 instance on one thread,
//! and the justfile stops a binary there after `browser_timeout` (600 s). The library's binary
//! grew to 502 s there (2026-10-14, Electron 44.4.3, under the heavy-test lock at a 1-minute load
//! of about 1), most of it in a few tests that pool a statistic, check a bound or agree with a
//! brute-force reference over many points: R10.T2's weights against the classes counted, the
//! classifier's and the weights' queries of partial ice (each a cell's cut, 256 heights at the
//! finest level), R09's level bound and unresolved variance, the crater inversion's draws. Those
//! tests check the algorithm, which is the same code on every target; what could differ between
//! the targets, the bits, is pinned by the goldens, which run whole on the browser target too. So
//! there each runs the same assertions over a smaller sample, and natively and on wasip1, where
//! nextest gives each test a process of its own across every core, over the whole one: 349 s on
//! the browser target with these sizes, in the same setting. A golden keeps its whole sample on
//! every target. An order- or cache-independence test on the browser target asks fewer points,
//! each through every path the whole sample takes (each query kind, level, cell and cache), so
//! that the property is still checked there.

/// `full` natively and on wasip1, `browser` on the browser target (the module documentation).
#[must_use]
pub(crate) const fn per_target<T: Copy>(full: T, browser: T) -> T {
    if cfg!(all(target_arch = "wasm32", target_os = "unknown")) {
        browser
    } else {
        full
    }
}
