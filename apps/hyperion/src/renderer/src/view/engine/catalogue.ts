/**
 * Every material, post-process and compute kernel the engine can create, which the smoke harness
 * renders one by one, offline, on SwiftShader.
 *
 * @remarks
 * Later plans register theirs here, so that the offline test covers every shader (R01's Generator
 * version section reserves it). An entry may name the settings it renders at; R01 has one setting,
 * `default`, and R12.T7.a adds the ladder's (R01 Design note 18).
 */

import { MULTI_SCATTERING_KERNEL, TRANSMITTANCE_KERNEL } from "../atmosphere/tables";
import type { KernelPair } from "./kernels";
import { WIREFRAME_MATERIALS } from "../wireframe/submit";
import { SUBGROUP_TWINS } from "./twins";
import type { PointSplatSpec, WgslMaterialSpec, WgslPostProcessSpec } from "./types";

/** A named rendering setting at which the harness renders an entry. */
export type CatalogueSetting = "default";

/** One shader the harness renders. */
export type CatalogueEntry = (
  | { readonly kind: "material"; readonly spec: WgslMaterialSpec }
  | { readonly kind: "post-process"; readonly spec: WgslPostProcessSpec }
  | { readonly kind: "compute"; readonly spec: KernelPair }
  // R06.T13.h: R06's bake splat, compiled by the harness through `createPointSplatAsync`.
  | { readonly kind: "point-splat"; readonly spec: PointSplatSpec }
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

/** R05.T12.b's per-planet atmosphere tables: transmittance and multiple scattering. */
const ATMOSPHERE_TABLE_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  TRANSMITTANCE_KERNEL,
  MULTI_SCATTERING_KERNEL,
].map((spec) => ({ kind: "compute", spec }));

/** Every shader the engine can create; later plans add theirs here. */
export const WGSL_CATALOGUE: ReadonlyArray<CatalogueEntry> = [
  ...TWIN_ENTRIES,
  ...WIREFRAME_ENTRIES,
  ...ATMOSPHERE_TABLE_ENTRIES,
];
