/**
 * Loads the engine only when a display draws a scene.
 *
 * @remarks
 * This is the one place that imports the Babylon implementation, by a dynamic `import()`, which
 * splits it into its own chunk; the renderer build names that chunk `babylon`, and
 * `scripts/checkChunks.mjs` fails if the entry chunk holds any Babylon code (R01 Design note 14).
 */

import type { AdapterOutcome } from "./platform";
import type { GraphicsStatusStore } from "./status";
import type { LoadEngineOptions, RenderEngine } from "./types";

/**
 * Imports the engine and creates it on the vetted adapter.
 *
 * @param outcome - A fresh adapter, vetted by `requestAdapterOutcome`: an adapter is consumed by its
 * first device (R01 Design note 7).
 * @param status - The store the engine reports its faults and device losses to.
 */
export async function loadRenderEngine(
  outcome: AdapterOutcome & { readonly kind: "adapter" },
  status: GraphicsStatusStore,
  options: LoadEngineOptions = {},
): Promise<RenderEngine> {
  const importEngine = options.importEngine ?? (() => import("./babylon/engine"));
  const { createBabylonEngine } = await importEngine();
  return createBabylonEngine(outcome, status, options.overrides);
}
