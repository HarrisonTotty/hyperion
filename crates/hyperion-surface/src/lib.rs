//! The shared height function of HYPERION's planets, computed bit for bit alike by the server and
//! the client.
//!
//! The server needs a body's surface to answer what the ship collides with and measures; the
//! client needs the same surface to draw it, from orbit to a metre above the ground, and cannot
//! wait for the server for every vertex. So both run this crate: natively in the server, and as a
//! WebAssembly module in the client's workers. It holds, as their plans land, the shared height
//! function (the rendering plans' R05 writes a provisional one for a hand-made test planet, R09 the
//! real one), the material classes (R10) and the rocks (R11). Today it holds R05's: the cube
//! sphere ([`cube`], [`geometry`]), the provisional [`test_planet`] and its noise basis
//! ([`noise`]), the patch bake and the collision interpolant ([`patch`]), and the datum heights
//! are measured from, the reference [`spheroid`] (plan 14's P14.T46.e).
//!
//! # The contract of the height function
//!
//! The height at a point is a pure function of the body's coarse field, the point, the detail level
//! asked for and the body's detail seed: the same inputs give the same `f64` on every target,
//! x86-64, `wasm32-wasip1` and the browser's `wasm32-unknown-unknown`, whichever call came first.
//! The point is a direction in the body-fixed frame, as a plain `[f64; 3]`, since this crate cannot
//! see the sim's coordinate types; R05 and R09 fix its exact form (a unit vector, in their plans).
//! Any cache is passed in by the caller, so the function keeps none of its own.
//!
//! # Boundary and determinism rules
//!
//! - No I/O, no clocks, no threads, and no caches of its own: a cache belongs to the caller.
//! - It depends on `hyperion-base` alone (for `math`, `rng`, `units` and the generator version),
//!   and never on the sim, which depends on it; on `wasm32-unknown-unknown` only, it also depends
//!   on `wasm-bindgen`, for the client module's exports (`src/wasm.rs`).
//! - Every transcendental goes through `hyperion_base::math`; this crate's own `clippy.toml` bans
//!   the platform's methods, `mul_add`, the `algebraic_*` methods and reading float bits, and must
//!   not fall back to the workspace root's, which allows the last.
//! - No `f32` in the authoritative path, octave sums in a fixed order, and keys and counters in
//!   `u64`, never `usize`.
//! - Randomness only from `hyperion_base::rng` streams, under tags of this crate's registry,
//!   [`tags`].
//! - Relaxed SIMD is banned: a build that enables it fails here, and `target_feature` attributes
//!   are rejected by a source test (plan R04, Design note 10). Fixed-width `simd128` is allowed,
//!   provided no sum is reassociated.

#[cfg(target_feature = "relaxed-simd")]
compile_error!(
    "relaxed SIMD is banned in hyperion-surface: its results are implementation-defined, so the \
     client's terrain would not match the server's bit for bit"
);

pub mod cube;
pub mod geometry;
pub mod noise;
pub mod num;
pub mod patch;
pub mod spheroid;
pub mod tags;
pub mod test_planet;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod wasm;

/// The version of the provisional test planet, written into the header of every golden file of
/// this crate in place of the generator version (plan R05, Design note 13).
///
/// The test planet (R05) belongs to no universe, so a change to it moves no universe's output and
/// does not bump `GENERATOR_VERSION`; it bumps this instead, as does any change to the cube
/// sphere's mapping or a patch bake's bytes. Where a golden test of this crate fails with the
/// testkit's hint to "bump `GENERATOR_VERSION`", read "bump `TEST_PLANET_VERSION`". R09's real height
/// function writes `GENERATOR_VERSION` into its own goldens.
pub const TEST_PLANET_VERSION: u32 = 1;

/// The generator version this build of the crate computes surfaces for.
///
/// It is `hyperion_base`'s `GENERATOR_VERSION`, the same number the sim and the server report. The
/// client's WebAssembly module reports it, so that a stale build of the module, which would draw
/// terrain the server does not collide with, is caught when it loads (plan R04, T10).
#[must_use]
pub const fn generator_version() -> u32 {
    hyperion_base::GENERATOR_VERSION.get()
}
