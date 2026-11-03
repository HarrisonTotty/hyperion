/**
 * The launch's compile budget for the renderer's engine (`--compile-budget`, plan R14;
 * `HyperionApi.compileBudget`), as `loadRenderEngine` takes it.
 */

import type { LoadEngineOptions } from "./types";

/**
 * The launch's budget where it gives one, else nothing, so that the engine takes its default: a
 * page with no preload, a test's, has no `window.hyperion`, and the preload checked the budget.
 */
export function launchCompileBudget(): Pick<LoadEngineOptions, "compileBudget"> {
  const api: unknown = Reflect.get(globalThis, "hyperion");
  const budget: unknown =
    typeof api === "object" && api !== null ? Reflect.get(api, "compileBudget") : undefined;
  return budget === "unlimited" || typeof budget === "number" ? { compileBudget: budget } : {};
}
