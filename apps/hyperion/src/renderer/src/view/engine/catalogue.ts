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
import { BLOOM_DOWN_MATERIAL, BLOOM_UP_MATERIAL } from "../post/bloomChain";
import { HISTOGRAM_KERNEL } from "../post/histogram";
import { TONEMAP_MATERIAL } from "../post/tonemap";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import { SUBGROUP_TWINS } from "./twins";
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

/** R01.T10's subgroup twins, run on both capability paths. */
const TWIN_ENTRIES: ReadonlyArray<CatalogueEntry> = SUBGROUP_TWINS.map((spec) => ({
  kind: "compute",
  spec,
}));

/**
 * R02's wireframe materials (plan R02, R02.T14.c): the lines, the two occluders and the star
 * sprites, each as `WireframeRenderer` creates it.
 */
const WIREFRAME_ENTRIES: ReadonlyArray<CatalogueEntry> = Object.values(WIREFRAME_MATERIALS).map(
  (spec) => ({ kind: "material", spec }),
);

/** R07's post-processing kernels and passes (plan R07, T12–T15). */
const POST_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "compute", spec: HISTOGRAM_KERNEL },
  { kind: "material", spec: BLOOM_DOWN_MATERIAL },
  { kind: "material", spec: BLOOM_UP_MATERIAL },
  { kind: "material", spec: TONEMAP_MATERIAL },
];

/** Every shader the engine can create; later plans add theirs here. */
export const WGSL_CATALOGUE: ReadonlyArray<CatalogueEntry> = [
  ...TWIN_ENTRIES,
  ...WIREFRAME_ENTRIES,
  ...POST_ENTRIES,
];
