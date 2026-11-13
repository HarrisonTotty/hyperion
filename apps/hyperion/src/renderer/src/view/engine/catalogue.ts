/**
 * Every material, post-process and compute kernel the engine can create, which the smoke harness
 * renders one by one, offline, on SwiftShader.
 *
 * @remarks
 * Later plans register theirs here, so that the offline test covers every shader (R01's Generator
 * version section reserves it). An entry may name the settings it renders at; R01 has one setting,
 * `default`, and R12.T7.a adds the ladder's (R01 Design note 18).
 */

import {
  AERIAL_PERSPECTIVE_BELOW_DECK_KERNEL,
  AERIAL_PERSPECTIVE_KERNEL,
  AERIAL_PERSPECTIVE_THERMAL_KERNEL,
  AERIAL_PERSPECTIVE_THICK_KERNEL,
  AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL,
  COMPOSITE_BEHIND_KERNEL,
  COMPOSITE_MATERIAL,
  LIMB_KERNELS,
  MARCH_TABLE_ONLY_KERNELS,
  RAY_MARCH_BELOW_DECK_KERNEL,
  RAY_MARCH_CENTRE_KERNEL,
  RAY_MARCH_CENTRE_ONE_BAND_KERNEL,
  RAY_MARCH_CENTRE_SPECIAL_KERNEL,
  RAY_MARCH_DECK_KERNEL,
  RAY_MARCH_DECK_SPECIAL_KERNEL,
  RAY_MARCH_DECK_THERMAL_KERNEL,
  RAY_MARCH_FILL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_ONE_BAND_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_ONE_BAND_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_DECK_KERNEL,
  RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  RAY_MARCH_SLOTS_ONE_BAND_KERNEL,
  RAY_MARCH_SLOTS_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_THERMAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_ROW_KERNEL,
  RAY_MARCH_SLOTS_THICK_ROW_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_THERMAL_KERNEL,
  RAY_MARCH_SPECIAL_KERNEL,
  RAY_MARCH_THERMAL_KERNEL,
  RAY_MARCH_THICK_ROW_KERNEL,
  RAY_MARCH_THICK_ROW_SPECIAL_KERNEL,
  RAY_MARCH_THICK_TABLE_KERNEL,
  RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL,
  RAY_MARCH_THICK_THERMAL_KERNEL,
  SKY_VIEW_BELOW_DECK_KERNEL,
  SKY_VIEW_KERNEL,
  SKY_VIEW_THERMAL_KERNEL,
  SKY_VIEW_THICK_KERNEL,
  SKY_VIEW_THICK_THERMAL_KERNEL,
  THICK_LIMB_KERNELS,
} from "../atmosphere/hillaire";
import { MARCH_CENSUS_KERNEL } from "../atmosphere/marchProbe";
import { LIT_BODY_PROBE } from "../appearance/litBodyProbe";
import {
  BODY_DISC_AIR_MATERIALS,
  BODY_DISC_CELLS_AIR_KERNEL,
  BODY_DISC_CELLS_EMISSIVE_AIR_KERNEL,
  BODY_DISC_CELLS_EMISSIVE_KERNEL,
  BODY_DISC_CELLS_KERNEL,
  BODY_DISC_CELLS_PACKED_AIR_KERNEL,
  BODY_DISC_CELLS_PACKED_EMISSIVE_AIR_KERNEL,
  BODY_DISC_CELLS_PACKED_EMISSIVE_KERNEL,
  BODY_DISC_CELLS_PACKED_KERNEL,
  BODY_DISC_EMISSIVE_AIR_MATERIALS,
  BODY_DISC_EMISSIVE_MATERIALS,
  BODY_DISC_MATERIALS,
  FIGURE_RESOLVE_MATERIALS,
  SMOOTH_MESH_AIR_CLUSTER_MATERIAL,
  SMOOTH_MESH_AIR_MATERIAL,
  SMOOTH_MESH_CLUSTER_MATERIAL,
  SMOOTH_MESH_EMISSIVE_AIR_CLUSTER_MATERIAL,
  SMOOTH_MESH_EMISSIVE_AIR_MATERIAL,
  SMOOTH_MESH_EMISSIVE_CLUSTER_MATERIAL,
  SMOOTH_MESH_EMISSIVE_MATERIAL,
  SMOOTH_MESH_IDS_CLUSTER_MATERIAL,
  SMOOTH_MESH_IDS_MATERIAL,
  SMOOTH_MESH_MATERIAL,
} from "../bodies/draw";
import {
  IRRADIANCE_KERNEL,
  IRRADIANCE_THICK_KERNEL,
  MULTI_SCATTERING_KERNEL,
  SLOT_DEPTHS_KERNEL,
  SOURCE_TRANSMITTANCE_KERNEL,
  TRANSMITTANCE_KERNEL,
} from "../atmosphere/tables";
import { tableOnlyKernel, tableOnlyMaterial } from "../atmosphere/surfaceLighting";
import type { KernelPair } from "./kernels";
import { BLOOM_DOWN_MATERIAL, BLOOM_UP_MATERIAL } from "../post/bloomChain";
import { HISTOGRAM_KERNEL } from "../post/histogram";
import { OVERLAY_SCENE_DEPTH_MATERIAL } from "../photoreal/overlayDepth";
import { TONEMAP_MATERIAL } from "../post/tonemap";
import { BAKE_KERNELS, BAKE_SPLAT } from "../sky/bake";
import { BAND_MATERIAL } from "../sky/band";
import { CUBE_DISPLAY_MATERIAL, CUBE_HDR_MATERIAL } from "../sky/cubeLayer";
import { DISC_MATERIAL } from "../sky/disc";
import { SKY_SPRITE_HDR_MATERIAL } from "../sky/spriteHdr";
import { LIT_AGX_MATERIAL } from "../spike/litView";
import { TERRAIN_MATERIALS } from "../terrain/gpu/material";
import { CLUSTER_COMPACT } from "../clusters/compact";
import { CLUSTER_CULL_KERNEL, CLUSTER_CULL_OCCLUSION_KERNEL } from "../clusters/cull";
import { DEPTH_PYRAMID_KERNEL } from "../clusters/hzb";
import { CLUSTER_STATS_KERNEL } from "../clusters/stats";
import {
  TERRAIN_CLUSTER_DEPTH_MATERIALS,
  TERRAIN_CLUSTER_ID_MATERIALS,
  TERRAIN_CLUSTER_SHADOW_MATERIALS,
  TERRAIN_DEPTH_MATERIALS,
  TERRAIN_FIN_DEPTH_MATERIALS,
  TERRAIN_FIN_SHADOW_MATERIALS,
  TERRAIN_LIT_CLUSTER_MATERIALS,
  TERRAIN_LIT_EMISSIVE_CLUSTER_MATERIALS,
  TERRAIN_LIT_EMISSIVE_FIN_MATERIALS,
  TERRAIN_LIT_EMISSIVE_INTERIOR_MATERIALS,
  TERRAIN_LIT_EMISSIVE_MATERIALS,
  TERRAIN_LIT_FIN_MATERIALS,
  TERRAIN_LIT_INTERIOR_MATERIALS,
  TERRAIN_LIT_MATERIALS,
  TERRAIN_RESOLVE_MATERIALS,
  TERRAIN_SHADOW_MATERIALS,
} from "../terrain/terrainMaterial";
import { TERRAIN_WIRE_CLUSTER_MATERIALS, TERRAIN_WIRE_MATERIALS } from "../terrain/terrainWire";
import {
  HULL_CLUSTER_MATERIALS,
  MESH_OCCLUDER_MATERIAL,
  WIREFRAME_MATERIALS,
} from "../wireframe/submit";
import { ID_READ_MATERIAL, ID_WRITE_MATERIAL } from "./idTarget";
import { SUBGROUP_TWINS } from "./twins";
import type {
  PointSplatSpec,
  RenderTargetFormat,
  WgslMaterialSpec,
  WgslPostProcessSpec,
} from "./types";

/** A named rendering setting at which the harness renders an entry. */
export type CatalogueSetting = "default";

/** One shader the harness renders. */
export type CatalogueEntry = (
  | {
      readonly kind: "material";
      readonly spec: WgslMaterialSpec;
      /**
       * The targets its pipelines are compiled for; `rgba16float` alone when absent. A caster into
       * a depth-only target (R10.T9.b's shadow cascades) names `depth-only`, which has no colour.
       */
      readonly targets?: ReadonlyArray<RenderTargetFormat>;
    }
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
 * sprites, with R10.T11's lines and occluder sphere over a body's surveyed terrain, each as
 * `WireframeRenderer` creates it.
 */
const WIREFRAME_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  ...Object.values(WIREFRAME_MATERIALS),
  // The hulls' faces on the cluster path (plan R14, R14.T4.b).
  ...Object.values(HULL_CLUSTER_MATERIALS),
  MESH_OCCLUDER_MATERIAL,
].map((spec) => ({ kind: "material", spec }));

/**
 * R05.T12.b's per-planet atmosphere tables: transmittance and multiple scattering, since plan
 * R08's R08.T9.b the sky's irradiance (`irradiance.wgsl`), since R08.T8.e each light slot's
 * optical depth (`slotDepths.wgsl`), and since R08.T9.b's lever the sources' transmittance
 * (`sourceTransmittance.wgsl`). R08's `surfaceLighting.wgsl`, a library, is catalogued inside the
 * lit-body probe, the disc materials and the sources' kernel, which compose it.
 */
const ATMOSPHERE_TABLE_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  TRANSMITTANCE_KERNEL,
  MULTI_SCATTERING_KERNEL,
  IRRADIANCE_KERNEL,
  SLOT_DEPTHS_KERNEL,
  SOURCE_TRANSMITTANCE_KERNEL,
].map((spec) => ({ kind: "compute", spec }));

/** R07's post-processing kernels and passes (plan R07, T12–T15). */
const POST_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "compute", spec: HISTOGRAM_KERNEL },
  { kind: "material", spec: BLOOM_DOWN_MATERIAL },
  { kind: "material", spec: BLOOM_UP_MATERIAL },
  { kind: "material", spec: TONEMAP_MATERIAL },
  // The scene's depth into the symbology's pass (plan R14, R14.T1.e).
  { kind: "material", spec: OVERLAY_SCENE_DEPTH_MATERIAL, targets: ["canvas"] },
];

/**
 * R05.T12.c's per-frame atmosphere: the sky view, the aerial perspective and the ray march, since
 * R08.T8.e in both its forms (the shared read, and by slot), since R08.T11.d each with its two entry
 * points (the centre ray's and the footprint's), the footprint's specialised for one band and no
 * layer edge, and the fill of its target outside the air's area,
 * and the composite that lays them over the terrain, with what lies behind its sky from above at the
 * march's resolution (the owner's priority, 2026-11-01).
 */
const ATMOSPHERE_VIEW_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  ...[
    SKY_VIEW_KERNEL,
    AERIAL_PERSPECTIVE_KERNEL,
    RAY_MARCH_KERNEL,
    RAY_MARCH_SLOTS_KERNEL,
    RAY_MARCH_CENTRE_KERNEL,
    RAY_MARCH_SLOTS_CENTRE_KERNEL,
    RAY_MARCH_FILL_KERNEL,
    RAY_MARCH_SPECIAL_KERNEL,
    RAY_MARCH_SLOTS_SPECIAL_KERNEL,
    COMPOSITE_BEHIND_KERNEL,
    // The inspection profile's census of the march's far limb (R08.T8.d), check-only.
    MARCH_CENSUS_KERNEL,
  ].map((spec): CatalogueEntry => ({ kind: "compute", spec })),
  { kind: "material", spec: COMPOSITE_MATERIAL },
];

/**
 * R08.T11.g's ray march for one band, edged, over both reads (`hillaire.ts`'s `withMarchOneBand`).
 * The smoke page compiles them in a part of their own (`smoke/page.ts`), for the watchdog of the
 * part that compiles the rest.
 */
export const ATMOSPHERE_ONE_BAND_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  RAY_MARCH_ONE_BAND_KERNEL,
  RAY_MARCH_SLOTS_ONE_BAND_KERNEL,
].map((spec): CatalogueEntry => ({ kind: "compute", spec }));

/**
 * R08.T8.d's far limb's node rays packed (`hillaire.ts`'s `LIMB_KERNELS`: each read's packed entry
 * point and node pass in each form, the resolve and the dispatches) and the tile cull's centre ray
 * in the specialised forms. The smoke page compiles them in a part of their own (`smoke/page.ts`),
 * for the watchdog of the part that compiles the rest.
 */
export const ATMOSPHERE_LIMB_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  ...LIMB_KERNELS,
  RAY_MARCH_CENTRE_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_SPECIAL_KERNEL,
  RAY_MARCH_CENTRE_ONE_BAND_KERNEL,
  RAY_MARCH_SLOTS_CENTRE_ONE_BAND_KERNEL,
].map((spec): CatalogueEntry => ({ kind: "compute", spec }));

/**
 * R08.T14.c's per-frame atmosphere over the thick multiple-scattering table (`hillaire.ts`'s
 * `withThickRead`): the sky view, the aerial perspective and both marches' footprints, each with
 * its form specialised for one band and no layer edge, the marches with their altitude row once a
 * sample (perf-thick-march); and R08.T14.d.4's sky-irradiance build over
 * it (`tables.ts`'s `IRRADIANCE_THICK_KERNEL`). The smoke page compiles them in a part of their own
 * (`smoke/page.ts`), for the watchdog of the part that compiles the rest.
 */
export const ATMOSPHERE_THICK_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  SKY_VIEW_THICK_KERNEL,
  AERIAL_PERSPECTIVE_THICK_KERNEL,
  RAY_MARCH_THICK_ROW_KERNEL,
  RAY_MARCH_SLOTS_THICK_ROW_KERNEL,
  RAY_MARCH_THICK_ROW_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_ROW_SPECIAL_KERNEL,
  IRRADIANCE_THICK_KERNEL,
].map((spec): CatalogueEntry => ({ kind: "compute", spec }));

/**
 * The thick read's march levers (perf-thick-march; `hillaire.ts`'s `thickFromTable` and
 * `thickPerStep`): the shared read's footprints from the sources' table alone, general and
 * specialised, and the packed limb's kernels, each read's per step and the shared read's from the
 * table, in both forms
 * (`THICK_LIMB_KERNELS`; the resolve and the dispatches are {@link ATMOSPHERE_LIMB_ENTRIES}'). The
 * smoke page compiles them in a part of their own (`smoke/page.ts`), for the watchdog of the part
 * that compiles the rest.
 */
export const ATMOSPHERE_THICK_LEVER_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  RAY_MARCH_THICK_TABLE_KERNEL,
  RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL,
  ...THICK_LIMB_KERNELS,
].map((spec): CatalogueEntry => ({ kind: "compute", spec }));

/**
 * R08.T18.b's per-frame atmosphere with the medium's own light (`hillaire.ts`' `withThermalSource`):
 * the sky view, the aerial perspective and both marches' footprints, of the thin read and of the
 * thick, made only for a self-luminous body. The smoke page compiles them in the thick read's part
 * (`smoke/page.ts`), for the watchdog of the part that compiles the rest.
 */
export const ATMOSPHERE_THERMAL_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  SKY_VIEW_THERMAL_KERNEL,
  AERIAL_PERSPECTIVE_THERMAL_KERNEL,
  RAY_MARCH_THERMAL_KERNEL,
  RAY_MARCH_SLOTS_THERMAL_KERNEL,
  SKY_VIEW_THICK_THERMAL_KERNEL,
  AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL,
  RAY_MARCH_THICK_THERMAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_THERMAL_KERNEL,
].map((spec): CatalogueEntry => ({ kind: "compute", spec }));

/**
 * R08.T15.a.2's ray marches over a cloud deck (`hillaire.ts`'s `withDeckRead`), both reads'
 * footprints, each with its form specialised for one band and no layer edge, and their thermal
 * forms (`withThermalSource`, R08.T15.b.2's route), made only for a self-luminous body over a deck;
 * and R08.T15.b.2's kernels beneath a deck (`hillaire.ts`'s `belowDeckKernel`): the sky view, the
 * aerial perspective and the march. The smoke page compiles them in a part of their own
 * (`smoke/page.ts`), beside the deck's twins.
 */
export const ATMOSPHERE_DECK_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  RAY_MARCH_DECK_KERNEL,
  RAY_MARCH_SLOTS_DECK_KERNEL,
  RAY_MARCH_DECK_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL,
  RAY_MARCH_DECK_THERMAL_KERNEL,
  RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL,
  SKY_VIEW_BELOW_DECK_KERNEL,
  AERIAL_PERSPECTIVE_BELOW_DECK_KERNEL,
  RAY_MARCH_BELOW_DECK_KERNEL,
].map((spec): CatalogueEntry => ({ kind: "compute", spec }));

/**
 * R05.T11.b's terrain pass, one material per vertex path, T11.c's lit view's display pass,
 * R10.T10.b's lit terrain, one material per vertex path, and its interior patches' form without the
 * survey discard (plan R14, R14.T1.a), R10.T16.c's lit terrain as depth alone for the symbology's
 * pass, one per vertex path, and its interior patches' on the cluster path (plan R14, R14.T1.e),
 * R10.T11's wireframe terrain, one per vertex path, and R10.T9.b's shadow casters, one per vertex
 * path, and plan R14's cluster casters (R14.T1.b), for the cascades' `depth-only` layers, and the
 * cluster path's cull and counters; the edge patches' fin forms of the lit terrain, its depth
 * and its caster on the cluster path (plan R14, R14.T1.g); and the visibility buffer's ID raster
 * and resolve (plan R14, R14.T2.c and T2.d).
 */
const TERRAIN_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  ...[
    ...TERRAIN_MATERIALS,
    LIT_AGX_MATERIAL,
    ...TERRAIN_LIT_MATERIALS,
    ...TERRAIN_LIT_INTERIOR_MATERIALS,
    ...TERRAIN_LIT_CLUSTER_MATERIALS,
    ...TERRAIN_LIT_FIN_MATERIALS,
    ...TERRAIN_DEPTH_MATERIALS,
    ...TERRAIN_CLUSTER_DEPTH_MATERIALS,
    ...TERRAIN_FIN_DEPTH_MATERIALS,
    ...TERRAIN_WIRE_MATERIALS,
    ...TERRAIN_WIRE_CLUSTER_MATERIALS,
  ].map((spec): CatalogueEntry => ({ kind: "material", spec })),
  ...[
    ...TERRAIN_SHADOW_MATERIALS,
    ...TERRAIN_CLUSTER_SHADOW_MATERIALS,
    ...TERRAIN_FIN_SHADOW_MATERIALS,
  ].map((spec): CatalogueEntry => ({
    kind: "material",
    spec,
    targets: ["depth-only"],
  })),
  // The visibility buffer's ID raster on the de-indexed path (plan R14, R14.T2.c), into its
  // `r32uint`; its `primitive_index` forms need a feature SwiftShader does not offer.
  ...TERRAIN_CLUSTER_ID_MATERIALS.map((spec): CatalogueEntry => ({
    kind: "material",
    spec,
    targets: ["r32uint"],
  })),
  // Its resolve and the resolve's emissive form (plan R14, R14.T2.d), into the scene target.
  ...TERRAIN_RESOLVE_MATERIALS.map((spec): CatalogueEntry => ({ kind: "material", spec })),
  // Plan R14's cluster path (R14.T1.b): its cull, the copy of its counters and its stable
  // compaction (R14.T2.a); the cull with the two-pass occlusion and the depth pyramid (R14.T3.d).
  { kind: "compute", spec: CLUSTER_CULL_KERNEL },
  { kind: "compute", spec: CLUSTER_CULL_OCCLUSION_KERNEL },
  { kind: "compute", spec: DEPTH_PYRAMID_KERNEL },
  { kind: "compute", spec: CLUSTER_STATS_KERNEL },
  { kind: "compute", spec: CLUSTER_COMPACT },
];

/**
 * R01's point splat as the harness's blending check draws it (R01.T9.i): the catalogue's one
 * `point-splat` entry until R06.T13.g registers the sky's bake splat beside it (R06.T13.h).
 */
export const ENGINE_CHECK_SPLAT: PointSplatSpec = {
  name: "smoke splat",
  format: "rgba32float",
  blend: "additive",
  vertexWgsl: `
@group(0) @binding(0) var<storage, read> points : array<vec2f>;
@vertex fn main(@builtin(vertex_index) index : u32) -> @builtin(position) vec4f {
  return vec4f(points[index], 0.5, 1.0);
}`,
  fragmentWgsl: `@fragment fn main() -> @location(0) vec4f { return vec4f(1.0, 0.0, 0.0, 1.0); }`,
};

/** The sky's passes (plan R06): the HDR twin of the star sprites (T13.c) and the band (T13.d). */
const SKY_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "material", spec: SKY_SPRITE_HDR_MATERIAL },
  { kind: "material", spec: BAND_MATERIAL },
  { kind: "material", spec: DISC_MATERIAL },
  { kind: "material", spec: CUBE_DISPLAY_MATERIAL },
  { kind: "material", spec: CUBE_HDR_MATERIAL },
  ...BAKE_KERNELS.map((spec): CatalogueEntry => ({ kind: "compute", spec })),
];

/** The point splats (R06.T13.h). */
const SPLAT_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "point-splat", spec: ENGINE_CHECK_SPLAT },
  // R06.T13.g: the sky's bake splat.
  { kind: "point-splat", spec: BAKE_SPLAT },
];

/**
 * R07's lit-body shading library (`shaders/litBody.wgsl`, R07.T4.c and T6.c), compiled and run
 * inside its probe kernel, since a library of functions is no material of its own, with R08's
 * surface lighting (`surfaceLighting.wgsl`, R08.T9.b), which every one of these composes.
 */
const LIT_BODY_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "compute", spec: LIT_BODY_PROBE },
  // R07.T8.a's disc regime: its wholly covered pixels and its limb, one source.
  { kind: "material", spec: BODY_DISC_MATERIALS.interior },
  { kind: "material", spec: BODY_DISC_MATERIALS.limb },
  // R07.T9's mesh regime: a body's smooth figure on R05's patches, shaded as the disc is, and its
  // form on the cluster path (plan R14, R14.T4.a).
  { kind: "material", spec: SMOOTH_MESH_MATERIAL },
  { kind: "material", spec: SMOOTH_MESH_CLUSTER_MATERIAL },
  // Its figure shaded once a pixel (`GeometrySettings.figureShading` `screen`): its raster into the
  // view's `r32uint` figure IDs, instanced and on the cluster path, and its resolve.
  { kind: "material", spec: SMOOTH_MESH_IDS_MATERIAL, targets: ["r32uint"] },
  { kind: "material", spec: SMOOTH_MESH_IDS_CLUSTER_MATERIAL, targets: ["r32uint"] },
  { kind: "material", spec: FIGURE_RESOLVE_MATERIALS.plain },
  // R07.T8.d's cell pass: the small discs' cells shaded in parallel for both regimes' draws; its
  // packed kernel is `PACKED_CELL_ENTRIES`'.
  { kind: "compute", spec: BODY_DISC_CELLS_KERNEL },
];

/**
 * R08.T18.a's emissive forms of the lit bodies' kernels (`discReflectance.ts`' `withDiscEmission`):
 * the disc's interior and limb, the mesh regime's figure and the `disc cells` kernel, and the lit
 * terrain's on both vertex paths (`terrainMaterial.ts`' `withTerrainEmission`), each with the
 * ground's thermal emission, made only for a body above the visibility cut. The smoke page compiles
 * them in part 9, beside their twin's group (`smoke/page.ts`), not in part 1, which compiles the
 * rest, for its watchdog.
 */
export const THERMAL_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "material", spec: BODY_DISC_EMISSIVE_MATERIALS.interior },
  { kind: "material", spec: BODY_DISC_EMISSIVE_MATERIALS.limb },
  { kind: "material", spec: SMOOTH_MESH_EMISSIVE_MATERIAL },
  { kind: "material", spec: SMOOTH_MESH_EMISSIVE_CLUSTER_MATERIAL },
  { kind: "material", spec: FIGURE_RESOLVE_MATERIALS.emissive },
  { kind: "compute", spec: BODY_DISC_CELLS_EMISSIVE_KERNEL },
  // R10.T10.b's lit terrain's emissive form, one material per vertex path, and its interior
  // patches' form (plan R14, R14.T1.a), on the cluster path (T2.a) and its fin form (T1.g).
  ...[
    ...TERRAIN_LIT_EMISSIVE_MATERIALS,
    ...TERRAIN_LIT_EMISSIVE_INTERIOR_MATERIALS,
    ...TERRAIN_LIT_EMISSIVE_CLUSTER_MATERIALS,
    ...TERRAIN_LIT_EMISSIVE_FIN_MATERIALS,
  ].map((spec): CatalogueEntry => ({ kind: "material", spec })),
];

/**
 * R08.T16.b's air forms of the lit bodies' kernels (`discReflectance.ts`' `withDiscAir`): the disc's
 * interior and limb, the mesh regime's figure and the `disc cells` kernel, each reading the body's
 * disc table beyond the 10⁹ m boundary, made only for a body drawn by it. The smoke page compiles
 * them in part 9, beside their twin's group (`smoke/page.ts`), after R08.T18.a's emissive forms.
 */
export const DISC_AIR_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "material", spec: BODY_DISC_AIR_MATERIALS.interior },
  { kind: "material", spec: BODY_DISC_AIR_MATERIALS.limb },
  { kind: "material", spec: SMOOTH_MESH_AIR_MATERIAL },
  { kind: "material", spec: SMOOTH_MESH_AIR_CLUSTER_MATERIAL },
  { kind: "material", spec: FIGURE_RESOLVE_MATERIALS.air },
  { kind: "compute", spec: BODY_DISC_CELLS_AIR_KERNEL },
];

/**
 * R08.T18.b's forms of the lit bodies' kernels for emission and air together
 * (`discReflectance.ts`' `withDiscEmissionAndAir`): the disc's interior and limb, the mesh regime's
 * figure and the `disc cells` kernel, made only for a glowing body drawn by its disc table. The
 * smoke page compiles them in part 9, after the air forms.
 */
export const DISC_EMISSIVE_AIR_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "material", spec: BODY_DISC_EMISSIVE_AIR_MATERIALS.interior },
  { kind: "material", spec: BODY_DISC_EMISSIVE_AIR_MATERIALS.limb },
  { kind: "material", spec: SMOOTH_MESH_EMISSIVE_AIR_MATERIAL },
  { kind: "material", spec: SMOOTH_MESH_EMISSIVE_AIR_CLUSTER_MATERIAL },
  { kind: "material", spec: FIGURE_RESOLVE_MATERIALS.emissiveAir },
  { kind: "compute", spec: BODY_DISC_CELLS_EMISSIVE_AIR_KERNEL },
];

/**
 * Plan R07's packed `disc cells` kernels (`GeometrySettings.discCells` `packed`), the plain one and
 * R08.T18.a's emissive, R08.T16.b's air and R08.T18.b's emissive-with-air forms, each with its
 * table-only twin (R08.T9.b's lever, `sourceTableEngine.ts`). The smoke page compiles them in part
 * 20, alone: the per-cell kernels' parts 1, 9 and 16 had no room for them, nor had part 17.
 */
export const PACKED_CELL_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  BODY_DISC_CELLS_PACKED_KERNEL,
  BODY_DISC_CELLS_PACKED_EMISSIVE_KERNEL,
  BODY_DISC_CELLS_PACKED_AIR_KERNEL,
  BODY_DISC_CELLS_PACKED_EMISSIVE_AIR_KERNEL,
].flatMap((spec): ReadonlyArray<CatalogueEntry> => {
  const twin = tableOnlyKernel(spec);
  const own: CatalogueEntry = { kind: "compute", spec };
  return twin === null ? [own] : [own, { kind: "compute", spec: twin }];
});

/**
 * The engine's ID target's own forms (plan R14, R14.T2.b; `idTarget.ts`): the writer into
 * `r32uint`, and the reader of a `texture_2d<u32>` into the scene's `rgba16float`.
 */
const ID_TARGET_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  { kind: "material", spec: ID_WRITE_MATERIAL, targets: ["r32uint"] },
  { kind: "material", spec: ID_READ_MATERIAL },
];

/**
 * R08.T9.b's lever's table-only twins (`sourceTableEngine.ts`): each lit material and `disc cells`
 * kernel of the entries before them with `surfaceLighting.wgsl`'s `SOURCE_TABLE_ONLY` set, which a
 * lit body of one band is drawn by. The smoke page compiles them in part 16, the lightest.
 */
export const TABLE_ONLY_ENTRIES: ReadonlyArray<CatalogueEntry> = [
  ...TERRAIN_ENTRIES,
  ...LIT_BODY_ENTRIES,
  ...THERMAL_ENTRIES,
  ...DISC_AIR_ENTRIES,
  ...DISC_EMISSIVE_AIR_ENTRIES,
].flatMap((entry): ReadonlyArray<CatalogueEntry> => {
  if (entry.kind === "material") {
    const spec = tableOnlyMaterial(entry.spec);
    return spec === null ? [] : [{ ...entry, spec }];
  }
  // The table's own build and the lit-body probe bind no body's table where a twin would stand in.
  if (entry.kind === "compute" && entry.spec.name.startsWith("BODY DISC CELLS")) {
    const spec = tableOnlyKernel(entry.spec);
    return spec === null ? [] : [{ kind: "compute", spec }];
  }
  return [];
});

/**
 * R08.T8.e's table's table-only twins of the thin read's marches (`hillaire.ts`'s
 * `MARCH_TABLE_ONLY_KERNELS`): each march a view's frame dispatches with a body of one band's
 * sources' table bound, its sliced lookup a step compiled out (`sourceTableEngine.ts`). The smoke
 * page compiles them in a part of their own (`smoke/page.ts`), for the watchdog of the part that
 * compiles the rest.
 */
export const MARCH_TABLE_ONLY_ENTRIES: ReadonlyArray<CatalogueEntry> = MARCH_TABLE_ONLY_KERNELS.map(
  (spec): CatalogueEntry => ({ kind: "compute", spec }),
);

/** Every shader the engine can create; later plans add theirs here. */
export const WGSL_CATALOGUE: ReadonlyArray<CatalogueEntry> = [
  ...TWIN_ENTRIES,
  ...ID_TARGET_ENTRIES,
  ...WIREFRAME_ENTRIES,
  ...ATMOSPHERE_TABLE_ENTRIES,
  ...POST_ENTRIES,
  ...ATMOSPHERE_VIEW_ENTRIES,
  ...ATMOSPHERE_ONE_BAND_ENTRIES,
  ...ATMOSPHERE_LIMB_ENTRIES,
  ...ATMOSPHERE_THICK_ENTRIES,
  ...ATMOSPHERE_THICK_LEVER_ENTRIES,
  ...ATMOSPHERE_THERMAL_ENTRIES,
  ...ATMOSPHERE_DECK_ENTRIES,
  ...TERRAIN_ENTRIES,
  ...SPLAT_ENTRIES,
  ...LIT_BODY_ENTRIES,
  ...THERMAL_ENTRIES,
  ...DISC_AIR_ENTRIES,
  ...DISC_EMISSIVE_AIR_ENTRIES,
  ...PACKED_CELL_ENTRIES,
  ...SKY_ENTRIES,
  ...TABLE_ONLY_ENTRIES,
  ...MARCH_TABLE_ONLY_ENTRIES,
];
