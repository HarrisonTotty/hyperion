/**
 * The Babylon engine's fixed options (R01 Design note 11).
 *
 * @remarks
 * Researched in `@babylonjs/core` 9.28.0. `useLargeWorldRendering` is a read-only constructor option,
 * off by default, and on it would force 64-bit matrices and a floating origin in every scene, so it
 * is passed as `false` explicitly (open question 1). `stencil: false` gives a render target
 * `depth32float` rather than `depth24plus-stencil8`. `doNotHandleContextLost: true` keeps Babylon's
 * own restore, which would re-run `initAsync` unawaited, out of the way: the adapter handles loss
 * itself (Design note 9). The engine's own canvas is never shown; every view draws into its own.
 */

import type { WebGPUEngineOptions } from "@babylonjs/core/Engines/webgpuEngine.pure";

/** Options the engine is always made with, apart from the features it asks for. */
export const FIXED_ENGINE_OPTIONS = {
  stencil: false,
  antialias: false,
  doNotHandleContextLost: true,
  useLargeWorldRendering: false,
  powerPreference: "high-performance",
} as const satisfies WebGPUEngineOptions;

/**
 * The options of one engine creation.
 *
 * @param requiredFeatures - `requiredFeatures`' answer for this creation's adapter.
 */
export function babylonEngineOptions(
  requiredFeatures: ReadonlyArray<GPUFeatureName>,
): WebGPUEngineOptions {
  return {
    ...FIXED_ENGINE_OPTIONS,
    enableAllFeatures: false,
    deviceDescriptor: { requiredFeatures: [...requiredFeatures] },
  };
}
