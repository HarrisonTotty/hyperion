//! What the client's WebAssembly module exports, on `wasm32-unknown-unknown` only.
//!
//! `just gen-surface` builds this crate for the browser target and runs `wasm-bindgen --target
//! web` over it, into the renderer's `generated/surface/`, where a module worker loads it (plan
//! R04, T10.b and T10.c). `#[wasm_bindgen]` writes the exports, which compiles under the
//! workspace's `forbid(unsafe_code)` where a raw `#[unsafe(no_mangle)]` export would not (R04
//! Design notes 12 and 16). The exports are thin: each calls the crate's own function, which the
//! native and wasip1 tests already check.

use wasm_bindgen::prelude::wasm_bindgen;

/// The generator version this module computes surfaces for, the crate's `generator_version`.
///
/// The client compares it with the server's generator version when the module loads, so that a
/// stale build of the module, which would draw terrain the server does not collide with, is
/// reported as a fault of the client's module.
#[wasm_bindgen(js_name = generatorVersion)]
#[must_use]
pub fn generator_version() -> u32 {
    crate::generator_version()
}
