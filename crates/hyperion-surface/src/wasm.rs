//! What the client's WebAssembly module exports, on `wasm32-unknown-unknown` only.
//!
//! `just gen-surface` builds this crate for the browser target and runs `wasm-bindgen --target
//! web` over it, into the renderer's `generated/surface/`, where a module worker loads it (plan
//! R04, T10.b and T10.c). `#[wasm_bindgen]` writes the exports, which compiles under the
//! workspace's `forbid(unsafe_code)` where a raw `#[unsafe(no_mangle)]` export would not (R04
//! Design notes 12 and 16). The exports are thin: each calls the crate's own function, which the
//! native and wasip1 tests already check.
//!
//! # The bake's layout (plan R05, T5), which the TypeScript side mirrors
//!
//! `bakePatch(face, level, i, j, vertexPath, normals, ridged, skirtM)` bakes the test planet's
//! patch (`face` 0–5 in S2's order, `level` 0–24, cell `i`, `j`; `vertexPath` 0 for
//! `BakedOffsets` and 1 for `FaceDifferences`; `normals` 0 for the mesh's 65 × 65 and 1 for
//! 129 × 129; `ridged` for the ridged octaves; `skirtM` an extra skirt margin in metres) and
//! returns a `BakedPatch` whose getters each copy one array out of linear memory into a new typed
//! array the worker owns and may transfer:
//!
//! - `heights()`: `Float32Array` of 65 × 65 × 2, vertex (x, y) at `2 (65 y + x)`: its own height,
//!   then its morph target, metres above the spheroid;
//! - `offsets()`: `Float32Array` of 65 × 65 × 6 for `BakedOffsets`, vertex (x, y) at
//!   `6 (65 y + x)`: q₀ then q₁, metres from the origin, body-fixed; `undefined` for
//!   `FaceDifferences`;
//! - `normals()`: `Float32Array` of N × N × 2, N = 65 or 129, sample (x, y) at `2 (N y + x)`: the
//!   octahedral pair of the unit normal, body-fixed, which the worker packs into a `Float16Array`;
//! - `origin()`: `Float64Array` of 3, the origin, body-fixed metres; `originHeightM`,
//!   `heightRangeM()` (`Float32Array` of 2, lowest then highest), `boundingRadiusM` and
//!   `skirtDepthM`, metres.
//!
//! `levelTable(ridged)` returns a `Float64Array` of 25 × 4, level n at `4 n`: the level bound `ε_n`,
//! the lowest and highest height, and the largest vertex spacing, metres (T6).

use wasm_bindgen::JsError;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::cube::{Face, PatchKey};
use crate::geometry::{BAND_LIMIT_M, FINEST_SPACING_M};
use crate::noise::LatticeCache;
use crate::patch::{BakeOptions, NormalScale, PatchBake, VertexPath};
use crate::test_planet::{Ridges, TEST_PLANET};

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

/// The terrain's band limit, metres ([`BAND_LIMIT_M`]).
#[wasm_bindgen(js_name = bandLimitM)]
#[must_use]
pub fn band_limit_m() -> f64 {
    BAND_LIMIT_M
}

/// The brainstorm's finest vertex spacing, metres ([`FINEST_SPACING_M`]).
#[wasm_bindgen(js_name = finestSpacingM)]
#[must_use]
pub fn finest_spacing_m() -> f64 {
    FINEST_SPACING_M
}

/// The test planet with ridges on or off.
fn planet(ridged: bool) -> crate::test_planet::TestPlanet {
    TEST_PLANET.with_ridges(if ridged { Ridges::On } else { Ridges::Off })
}

/// The level table of the test planet (see the module documentation).
#[wasm_bindgen(js_name = levelTable)]
#[must_use]
pub fn level_table(ridged: bool) -> Vec<f64> {
    planet(ridged).level_table()
}

/// A baked patch, whose getters copy its arrays out (see the module documentation).
#[wasm_bindgen]
#[derive(Debug)]
pub struct BakedPatch {
    bake: PatchBake,
}

#[wasm_bindgen]
impl BakedPatch {
    /// Own heights and morph targets, interleaved.
    #[must_use]
    pub fn heights(&self) -> Vec<f32> {
        self.bake.heights.clone()
    }

    /// The baked offsets q₀ and q₁, or none on the `FaceDifferences` path.
    #[must_use]
    pub fn offsets(&self) -> Option<Vec<f32>> {
        self.bake.offsets.clone()
    }

    /// The octahedral normal pairs.
    #[must_use]
    pub fn normals(&self) -> Vec<f32> {
        self.bake.normals.clone()
    }

    /// The origin, body-fixed metres.
    #[must_use]
    pub fn origin(&self) -> Vec<f64> {
        self.bake.origin.to_vec()
    }

    /// The origin's height, metres.
    #[wasm_bindgen(getter, js_name = originHeightM)]
    #[must_use]
    pub fn origin_height_m(&self) -> f64 {
        self.bake.origin_height_m
    }

    /// The lowest and highest baked height, metres.
    #[wasm_bindgen(js_name = heightRangeM)]
    #[must_use]
    pub fn height_range_m(&self) -> Vec<f32> {
        vec![self.bake.height_range_m.0, self.bake.height_range_m.1]
    }

    /// The radius about the origin holding the patch, metres.
    #[wasm_bindgen(getter, js_name = boundingRadiusM)]
    #[must_use]
    pub fn bounding_radius_m(&self) -> f64 {
        self.bake.bounding_radius_m
    }

    /// The skirts' depth, metres.
    #[wasm_bindgen(getter, js_name = skirtDepthM)]
    #[must_use]
    pub fn skirt_depth_m(&self) -> f64 {
        self.bake.skirt_depth_m
    }
}

/// Bakes the test planet's patch (see the module documentation).
///
/// # Errors
///
/// A `JsError` naming the problem if the face, level, cell, path or normal scale is out of range,
/// or the skirt margin is negative or not finite.
#[wasm_bindgen(js_name = bakePatch)]
#[expect(
    clippy::too_many_arguments,
    reason = "a flat call is the binding's cheapest shape; the TypeScript side wraps it"
)]
pub fn bake_patch(
    face: u8,
    level: u8,
    i: u32,
    j: u32,
    vertex_path: u8,
    normals: u8,
    ridged: bool,
    skirt_m: f64,
) -> Result<BakedPatch, JsError> {
    let face = Face::try_from(face).map_err(|e| JsError::new(&e.to_string()))?;
    let key = PatchKey::new(face, level, i, j).map_err(|e| JsError::new(&e.to_string()))?;
    let vertex_path = match vertex_path {
        0 => VertexPath::BakedOffsets,
        1 => VertexPath::FaceDifferences,
        n => return Err(JsError::new(&format!("vertex path {n} is not 0 or 1"))),
    };
    let normals = match normals {
        0 => NormalScale::Mesh,
        1 => NormalScale::Double,
        n => return Err(JsError::new(&format!("normal scale {n} is not 0 or 1"))),
    };
    if !(skirt_m.is_finite() && skirt_m >= 0.0) {
        return Err(JsError::new(&format!(
            "skirt margin {skirt_m} is not finite and non-negative"
        )));
    }
    let opts = BakeOptions {
        vertex_path,
        normals,
        skirt_m,
    };
    let mut cache = LatticeCache::new();
    let bake = match crate::patch::bake_patch(&planet(ridged), key, &opts, &mut cache) {
        Ok(bake) => bake,
        Err(never) => match never {},
    };
    Ok(BakedPatch { bake })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn the_exported_constants_are_the_crates() {
        assert!((band_limit_m() - BAND_LIMIT_M).abs() <= 0.0);
        assert!((finest_spacing_m() - FINEST_SPACING_M).abs() <= 0.0);
    }

    #[test]
    fn a_bake_has_the_documented_layout() {
        let Ok(baked) = bake_patch(2, 7, 3, 4, 0, 1, false, 0.0) else {
            panic!("a valid patch bakes");
        };
        assert_eq!(baked.heights().len(), 65 * 65 * 2);
        assert_eq!(baked.offsets().map(|o| o.len()), Some(65 * 65 * 6));
        assert_eq!(baked.normals().len(), 129 * 129 * 2);
        assert_eq!(baked.origin().len(), 3);
        assert_eq!(level_table(false).len(), 25 * 4);
    }
}
