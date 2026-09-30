/**
 * Every material, post-process and compute kernel the engine can create, which the smoke harness
 * renders one by one, offline, on SwiftShader.
 *
 * @remarks
 * Later plans register theirs here, so that the offline test covers every shader (R01's Generator
 * version section reserves it). An entry may name the settings it renders at; R01 has one setting,
 * `default`, and R12.T7.a adds the ladder's (R01 Design note 18).
 */

import type { KernelPair } from "./kernels";
import type { WgslMaterialSpec, WgslPostProcessSpec } from "./types";

/** A named rendering setting at which the harness renders an entry. */
export type CatalogueSetting = "default";

/** One shader the harness renders. */
export type CatalogueEntry = (
  | { readonly kind: "material"; readonly spec: WgslMaterialSpec }
  | { readonly kind: "post-process"; readonly spec: WgslPostProcessSpec }
  | { readonly kind: "compute"; readonly spec: KernelPair }
) & {
  /** The settings it renders at; `default` alone when absent. */
  readonly settings?: ReadonlyArray<CatalogueSetting>;
};

/** Every shader the engine can create. Empty until the first shaders are registered. */
export const WGSL_CATALOGUE: ReadonlyArray<CatalogueEntry> = [];
