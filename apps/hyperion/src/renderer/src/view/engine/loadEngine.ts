/**
 * Loads the engine only when a display draws a scene.
 *
 * @remarks
 * This is the one place that imports the WebGPU implementation, by a dynamic `import()`, which
 * splits it into a chunk of its own, loaded only when a display draws (R01 Design notes 14 and 24);
 * `engineBoundary.test.ts` refuses a static import of it anywhere.
 *
 * The engine it returns survives a lost device: it re-creates the engine on a fresh
 * adapter through the same import, and its views with it (`ResilientEngine`, Design note 9).
 */

import type { AdapterOutcome } from "./platform";
import { ResilientEngine } from "./resilientEngine";
import { type GraphicsStatusStore, navigatorGpu } from "./status";
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
  const importEngine = options.importEngine ?? (() => import("./webgpu/engine"));
  const { createWebGpuEngine } = await importEngine();
  const create = (fresh: AdapterOutcome & { readonly kind: "adapter" }): Promise<RenderEngine> =>
    createWebGpuEngine(fresh, status, options.overrides);
  const engine = await create(outcome);
  return new ResilientEngine(engine, status, options.gpu ?? navigatorGpu(), create);
}
