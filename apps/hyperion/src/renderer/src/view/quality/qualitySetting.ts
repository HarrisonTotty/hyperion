/**
 * The quality settings of the view, gathered in one place (plan R05, Design note 26).
 *
 * @remarks
 * There are two settings, `high` (the recommended specification) and `low` (the UHD 620 class), and
 * one list from each to its {@link ViewSettings}. Later plans extend this module only by adding a
 * field to {@link ViewSettings}, with its high and low values in {@link SETTINGS}: R05.T12.c the
 * atmosphere's table sizes, R06 the sky, R07 its styles' passes, R08 the atmosphere's view, R11
 * decoration, R12 the ladder, R14 the cluster path's geometry. They never add a setting name or a second list. Each feature builds its
 * low form beside its high form (the budget's third rule), so a field is never added for one setting
 * alone.
 *
 * The names exported here are stable: {@link QualitySetting}, {@link QUALITY_SETTINGS},
 * {@link ViewSettings}, {@link SETTINGS}, {@link TerrainSettings} and {@link TERRAIN_SETTINGS}.
 */

import { TABLE_SIZES, type TableSizes } from "../atmosphere/hillaire";
import {
  ATMOSPHERE_SETTINGS,
  ATMOSPHERE_VIEW_SETTINGS,
  type AtmosphereSettings,
  type AtmosphereViewSettings,
} from "../atmosphere/settings";
import { DISC_ANNULI_HIGH, DISC_ANNULI_LOW } from "../lighting/annuli";
import { PLANETSHINE_SOURCES_HIGH, PLANETSHINE_SOURCES_LOW } from "../lighting/planetshine";
import { BLOOM_LEVELS, type BloomLevels } from "../post/bloom";
import { HIGH_SKY, LOW_SKY, type SkySettings } from "../sky/setting";
import type { GeometryOverride, MeshTolerancePx } from "../../../../preload/api";
import type { Stride } from "../clusters/templates";
import type { FarCascadeRate } from "../terrain/shadows/cascades";
import type { R11Ladder } from "../surface/decoration/decorationSetting";
import type { TerrainNormals, TerrainVertexPath } from "./terrainKinds";

/** The view's quality setting: `high` on the recommended specification, `low` on the UHD 620. */
export type QualitySetting = "high" | "low";

/** Every {@link QualitySetting}, in order from the highest, for tests and settings menus. */
export const QUALITY_SETTINGS: ReadonlyArray<QualitySetting> = ["high", "low"];

/**
 * Each {@link QualitySetting} as the console reads it: the guide's `QUALITY` values, on `VIEW`'s
 * label block and the `LINK` display's `Graphics` panel (R07.T17).
 */
export const QUALITY_NAMES = {
  high: "HIGH",
  low: "LOW",
} as const satisfies Record<QualitySetting, string>;

/** Where the terrain's normals are evaluated, and how its vertices are formed: in
 * `terrainKinds.ts`, which the height workers read too. */
export type { TerrainNormals, TerrainVertexPath } from "./terrainKinds";

/**
 * How a handed-over body's terrain draws its own sub-cell shading near the hand-over (plan R10,
 * R10.T15.b.2): `faded`, the disc's at the hand-over and the hand-back, each texel lit at the
 * disc's point along its ray on the disc's normal map's normal, its star cut by the datum's tangent
 * plane, rising to its own place, normals and horizon map as the camera closes in
 * (`terrain/handover.ts`' `handoverDetailShare`); `whole`, its own at every range.
 */
export type HandoverDetail = "faded" | "whole";

/** The terrain's settings for one {@link QualitySetting} (R05 Design notes 4, 7, 10 and 26). */
export interface TerrainSettings {
  /**
   * The screen-space error tolerance τ in pixels at the presented resolution's CSS px
   * (`selectionViewport`): the largest a level's error bound may subtend before a finer level is
   * selected (R05 Design note 7).
   */
  readonly tauPx: number;
  /**
   * The height in device pixels at which the view renders before it is presented upscaled, or
   * `null` to render at the canvas's own size.
   */
  readonly renderHeightPx: number | null;
  /** The resolution at which normals are baked and stored. */
  readonly normals: TerrainNormals;
  /** The vertex path, provisional until R05.T18 chooses from the spike's measurements. */
  readonly vertexPath: TerrainVertexPath;
  /**
   * The patch cache's byte budget, from which its fixed slot count follows (R05 Design note 10),
   * held by R10.T14.b (decision-r10-t14b-constants) on the hard bound, re-sized after R10.T4.b.
   */
  readonly cacheBytes: number;
  /**
   * How its sub-cell shading stands near the hand-over: `faded` on the high setting, whose τ of
   * 1 px puts its hand-over twice as far out as the low setting's on one display (three times on the
   * hand-over check's windows), so that a view there takes in the terminator, where the terrain's
   * sub-cell slopes, its relief's parallax and its cast shadows, none of which the disc draws, move
   * a cell by more than 1/3 stop (the served airless world's 90° look, R10.T15.b.2); `whole` on the
   * low setting, as before.
   */
  readonly handoverDetail: HandoverDetail;
}

/**
 * How the terrain's geometry is drawn (plan R14, the cluster path, terrain its first consumer): the
 * way back to the geometry drawn before plan R14, kept compiled and selectable at every setting,
 * and each lever of R14's first phase.
 */
export interface GeometrySettings {
  /**
   * Whether the terrain draws through plan R14's path: its interior patches by a mesh of their own,
   * without the survey discard (R14.T1.a), and their casters through the cluster cull (R14.T1.b).
   * `false` is the way back: every patch drawn and cast by the lit mesh, every other field of these
   * settings unread.
   */
  readonly clusterPath: boolean;
  /** R14.T1.c: casters culled by the view depths their receivers sample each cascade at. */
  readonly casterDepthCull: boolean;
  /** R14.T1.d: the coarsest stride a caster may take at its cascade's texel; 1 keeps stride 1. */
  readonly casterStrideCap: Stride;
  /**
   * R14.T1.e: where the symbology's pass over the photorealistic image takes the terrain's depth
   * for its marks' test. `redraw`, as before: each of the terrain's draws again with the depth
   * material. `cluster`, both settings': the overlay's own depth, the edge patches' draws and the
   * interior patches' clusters at stride 1, culled to the camera's frustum, which keeps every
   * mark's visibility and is the one to keep once plan R14's T3.c strides the camera pass.
   * `scene-depth`, the plan's lean: the scene target's depth where its size is the canvas's, and
   * elsewhere as `cluster`; it carries the reference surfaces' tessellated depth, which hides marks
   * on their datum that the redraw leaves in sight (R14.T1.e's smoke), so no setting takes it.
   * Unread by the way back, which redraws.
   */
  readonly overlayDepth: "redraw" | "scene-depth" | "cluster";
  /**
   * R14.T2.a: the lit pass's interior patches drawn through the cluster path, culled to the
   * camera's frustum and the body's horizon per cluster and compacted in the draw set's order, at
   * stride 1, forward; `false` draws them by R14.T1.a's interior mesh. Both settings take it.
   */
  readonly litClusters: boolean;
  /**
   * R14.T6.b: the frames between re-renders of the far shadow cascades, 2 and 3 of 0 to 3, which
   * are held between while the sun and the view's slice stay within their holds' bounds
   * (`FarCascadeHolds`); 1 renders all four every frame. 4 on both settings, a lean (the low
   * setting draws no cascades).
   */
  readonly farCascadeRate: FarCascadeRate;
  /**
   * R14.T4.a: the mesh regime's figures drawn through the cluster path, each patch's clusters culled
   * to its view's frustum, at stride 1; `false` draws each figure by its instanced draw of R05's
   * patch mesh. Both settings take it.
   */
  readonly figureClusters: boolean;
  /**
   * R14.T4.b: the hulls' occluder faces, in the wireframe and as the silhouette over the image,
   * drawn through the cluster path as one-level DAG-cut pages, culled to the view's frustum;
   * `false` draws each hull by its instanced draw of its faces. Both settings take it.
   */
  readonly hullClusters: boolean;
  /**
   * R14.T4.c: the wireframe's interior patches drawn through the cluster path at stride 1, culled
   * to the view's frustum and the body's horizon per cluster, compacted in the draw set's order
   * and drawn without the survey discard before the edge patches; `false` draws every patch by the
   * lit mesh. Both settings take it.
   */
  readonly wireClusters: boolean;
  /**
   * R14.T5.b: the mesh page pool's bytes, its pages and their records together, category
   * `geometry-pages`: 128 MiB high and 32 MiB low (Design note 15's leans), 10% of it for the
   * meshes' pinned roots. 0 on the way back, which has no pool.
   */
  readonly pageBytes: number;
  /**
   * R14.T5.f: a mesh's DAG-cut tolerance, px: `camera`, its camera pass's (T5.e's), 1 high and 2
   * low; `wireframeOccluder`, its occluder depth's in the wireframe, 0.5, half the thin stroke at a
   * stroke scale of 1, so that a cut's silhouette moves under every stroke's half-width and hidden-
   * line removal holds (Design note 13; both leans). 0 on the way back: the finest cut.
   */
  readonly meshTolerancePx: MeshTolerancePx;
  /**
   * R14.T1.g: the lit pass's edge patches drawn through the cluster path beside the interior
   * patches, in the lit pass, the casters and the overlay's depth, at stride 1: each block and strip
   * the survey keeps whole without the survey discard, each it keeps in part, and the skirt heads
   * (`terrain-head`) and survey fins (`terrain-fin`), with it, and each it keeps nowhere not at all;
   * `false` draws them by the lit mesh. Read where `litClusters` is on. Both settings take it.
   */
  readonly finClusters: boolean;
  /**
   * R14.T3.c: the lit pass's camera strides. `off` draws every cluster at stride 1; `measured` at
   * the coarsest stride whose shared band the cluster lies beyond by (1 + β), the band the distance
   * past which its block, the blocks on its sides and its edges' neighbours all draw that stride's
   * stated error plus e_s within the view's τ′ (`clusters/strides.ts`), each vertex the stride
   * drops moved onto the coarser mesh by a distance morph of its own, and stride 1 in the forced
   * region. Read where `litClusters` is on; the overlay's depth then takes its own pass at stride
   * 1, whatever `overlayDepth` says. On each setting where its acceptance passes on the pod's
   * seamless probe, else `off` (the brief's ruling, 2026-11-02): `measured` on low, `off` on high,
   * where a frame's change moved the ground past its 1 px.
   */
  readonly strides: "off" | "measured";
  /**
   * R14.T3.d: two-pass occlusion against the depth pyramid. `hzb` draws the lit pass's clusters
   * visible last frame first, builds the pyramid from that depth, and draws after it only the rest
   * the pyramid does not refuse (`clusters/hzb.ts`); `off` draws every cluster the camera's tests
   * keep in the one raster. Exact: the image differs only where two surfaces meet at equal depth,
   * whose order it changes. Read where `litClusters` is on. `off` on both settings until its
   * acceptance on the pod (the brief's ruling, 2026-11-02; the plan writes `none`).
   */
  readonly occlusion: "off" | "hzb";
}

/**
 * `setting`'s geometry with a launch's development override over it (`--geometry`, plan R14,
 * Design note 17): each field the override gives replaces the setting's.
 */
export function geometryOf(setting: QualitySetting, override: GeometryOverride): GeometrySettings {
  return { ...SETTINGS[setting].geometry, ...override };
}

/** The way back (plan R14): the terrain's geometry as it was drawn before plan R14. */
export const GEOMETRY_BEFORE_R14: GeometrySettings = {
  clusterPath: false,
  casterDepthCull: false,
  casterStrideCap: 1,
  overlayDepth: "redraw",
  litClusters: false,
  farCascadeRate: 1,
  figureClusters: false,
  hullClusters: false,
  wireClusters: false,
  pageBytes: 0,
  meshTolerancePx: { camera: 0, wireframeOccluder: 0 },
  finClusters: false,
  strides: "off",
  occlusion: "off",
};

/**
 * Everything the view's quality setting decides, one field per feature.
 *
 * @remarks
 * Later plans add fields here and their values to {@link SETTINGS} (see the module's remarks).
 * Plan R11's ladder entries are its fields directly ({@link R11Ladder}: `decoration`,
 * `scatterDensity`, `clouds`, `cloudShadows`, `oceanComponents`, `oceanRefraction`, `whitecaps`,
 * `rings` and `ringShadowOnBody`; R11.T1, Design note 18), as R12's `SettingSnapshot` reads them.
 */
export interface ViewSettings extends R11Ladder {
  /** The terrain's settings (R05). */
  readonly terrain: TerrainSettings;
  /** The atmosphere's table sizes and sample counts (R05.T12.c, Design note 16; R08 widens them). */
  readonly atmosphere: TableSizes;
  /**
   * The atmosphere's view: the sky's sources and the stated quality limits (plan R08, R08.T7;
   * `view/atmosphere/settings.ts`).
   */
  readonly atmosphereView: AtmosphereViewSettings;
  /**
   * The atmosphere's frame-cost levers, each with its way back (the owner's priority, 2026-11-01;
   * `view/atmosphere/settings.ts`).
   */
  readonly atmosphereLevers: AtmosphereSettings;
  /** The sky's cube, sprites, census size and re-bake cadence (R06.T13.f, Design note 22). */
  readonly sky: SkySettings;
  /**
   * The least and greatest internal scale at which the resolution controller holds the
   * photorealistic view, as a fraction of its render resolution on each axis: [0.5, 1.0] on both
   * settings (R07 Design note 14). R12.T11 may move them.
   */
  readonly internalScaleBounds: readonly [min: number, max: number];
  /** The several views' budget: the photorealistic rate and how many such views (R07.T18). */
  readonly budget: ViewBudgetSettings;
  /** The photorealistic style's passes: the histogram, bloom and the lit bodies (R07.T17). */
  readonly photoreal: PhotorealSettings;
  /** How the terrain's geometry is drawn (plan R14). */
  readonly geometry: GeometrySettings;
}

/**
 * What the photorealistic style's passes read of a setting (R07 Design note 18): the low setting's
 * forms, built beside the high ones.
 *
 * @remarks
 * Design note 18's other two items are settings already: the view renders at most
 * {@link TerrainSettings.renderHeightPx} rows (1,080 on high, 720 on low), presented upscaled by
 * the tone-mapping pass, the render resolution that `ViewBudget.renderScale` is a fraction of; and
 * the one photorealistic view is {@link ViewBudgetSettings.photorealisticViews}.
 */
export interface PhotorealSettings {
  /**
   * The exposure histogram's stride, px: 1 reads every pixel of the scene target, 2 every second
   * pixel of every second row, a quarter-resolution input (R07.T12).
   */
  readonly histogramStride: 1 | 2;
  /**
   * The bloom chain's weighted mip levels (R07.T14): 7 from full resolution, or 5 from quarter
   * resolution.
   */
  readonly bloom: BloomLevels;
  /** The eclipse term's annuli K per star (R07 Design note 6): 4, or 3. */
  readonly discAnnuli: number;
  /** The most lit neighbours that light a body by planetshine (R07 Design note 7): 2, or 1. */
  readonly planetshineSources: number;
  /**
   * The cells per axis a disc of 32 px or more takes in its limb pixels while a photorealistic
   * primary's frame-rate floor stands at its lowest step (R07.T8.i, the owner's priority; a lean,
   * 2026-11-02): 2, a quarter of `LIMB_SAMPLES`' cells, within 1/3 stop of them on at least 99.5%
   * of the limb's pixels; `LIMB_SAMPLES` (4) is the way back. The low setting never steps its floor
   * (`aloneScaleBounds`), so its value is never read; it is the same.
   */
  readonly floorLimbSamples: number;
}

/** What the per-view budget reads of a setting (R07 Design notes 14 and 18). */
export interface ViewBudgetSettings {
  /**
   * The rate a photorealistic primary view is budgeted at: 60 Hz for the 1080p60 target, 30 Hz for
   * the low setting's 30 fps at 720p, paced to every second vsync (R05 Design note 21).
   */
  readonly photorealisticRateHz: 60 | 30;
  /**
   * The most views drawn in the photorealistic style at once, or `null` for no limit: one on the
   * low setting (R07 Design note 18), the only limit the refusal's wording allows.
   */
  readonly photorealisticViews: 1 | null;
}

const MIB = 1024 * 1024;

/**
 * The high setting's patch cache budget, 400,000,000 B, held by R10.T14.b
 * (decision-r10-t14b-constants) on the hard bound, re-sized after R10.T4.b (the rule in its union
 * form asked 8,666 high lit slots, 2,996 low lit and 3,764 wireframe on TERRAIN TEST's moon, about
 * seven times the held 1,286, 422 and 520): R10 Design note 15's research figure, 1.3 × the
 * all-round peak at 100 m (about 1,900 slots, 390 MB at its 206 kB a slot). As built it holds 1,208
 * lit slots of 330,996 B (`cacheBudget.ts`' `bytesPerPatch`, R10.T14.a, whose count model's figures
 * plan 10 records beside it), 1,286 of 311,024 B before R10.T1.c's maturity layer grew each slot's
 * class layers by its codes and nodes (decision-r10-t1c-client-scope §2.2: recorded, not re-ruled).
 */
const HIGH_CACHE_BYTES = 400_000_000;

/**
 * The low setting's patch cache budget, 64 MiB, held by R10.T14.b (decision-r10-t14b-constants) on
 * the hard bound, re-sized after R10.T4.b and the owner's UHD 620 run (the rule in its union form
 * asked 8,666 high lit slots, 2,996 low lit and 3,764 wireframe on TERRAIN TEST's moon, about seven
 * times the held 1,286, 422 and 520): 1,296 slots of R05's spike layout; as built, 375 lit slots of
 * 178,908 B (R10.T14.a), 422 of 158,936 B before R10.T1.c's maturity layer.
 */
const LOW_CACHE_BYTES = 64 * MIB;

/**
 * The one list from each {@link QualitySetting} to its {@link ViewSettings} (R05 Design note 26).
 *
 * @remarks
 * The low setting renders at 720 rows presented upscaled with τ = 2 px, mesh-resolution normals,
 * the `face-differences` vertex path (`baked-offsets` does not fit its 64 MiB cache) and a 64 MiB
 * cache. The high setting renders at the canvas's size, at most 1,080 rows presented upscaled (lean
 * 2026-11-01: the brainstorm states the budget at 1080p, a larger display rendering below native
 * and upscaling, "Two deployments, one scene"; R07 Design note 14), with τ = 1 px, doubled normals
 * and `baked-offsets`, its precision holding by construction until R05.T18 chooses, and its
 * sub-cell shading faded in below the hand-over, where the low setting's stands whole
 * (R10.T15.b.2). Under a
 * grounded body every setting draws the finest level whatever these say (R05 Design note 9). Both
 * settings hold the photorealistic view's internal scale in [0.5, 1.0]; the low setting budgets a
 * photorealistic primary view at 30 Hz and allows one photorealistic view (R07 Design notes 14 and
 * 18). The low setting's photorealistic style takes its histogram at a stride of 2, blooms over 5
 * levels from quarter resolution, eclipses with 3 annuli and lights each body by planetshine from
 * one neighbour, against 1, 7 levels from full resolution, 4 annuli and two neighbours (R07 Design
 * note 18, R07.T17). R11's ladder entries are Design note 18's (high; low): decoration on and off,
 * the decorative scatter's density 1 and 0, the clouds' volume and layer with and without their
 * shadows on the terrain, 32 and 8 Gerstner components with and without refraction and whitecaps,
 * and the rings' particles and annulus with and without their shadow on the body, so that the low
 * setting draws no decoration of any kind (R11.T1).
 */
export const SETTINGS: Readonly<Record<QualitySetting, ViewSettings>> = {
  high: {
    terrain: {
      tauPx: 1,
      // A Retina or 4K canvas renders at 1080p and is upscaled, as low's at 720p (lean 2026-11-01).
      renderHeightPx: 1080,
      normals: "double",
      vertexPath: "baked-offsets",
      cacheBytes: HIGH_CACHE_BYTES,
      handoverDetail: "faded",
    },
    atmosphere: TABLE_SIZES.high,
    atmosphereView: ATMOSPHERE_VIEW_SETTINGS.high,
    atmosphereLevers: ATMOSPHERE_SETTINGS.high,
    sky: HIGH_SKY,
    internalScaleBounds: [0.5, 1],
    budget: { photorealisticRateHz: 60, photorealisticViews: null },
    photoreal: {
      histogramStride: 1,
      bloom: BLOOM_LEVELS.high,
      discAnnuli: DISC_ANNULI_HIGH,
      planetshineSources: PLANETSHINE_SOURCES_HIGH,
      floorLimbSamples: 2,
    },
    geometry: {
      clusterPath: true,
      casterDepthCull: true,
      casterStrideCap: 8,
      overlayDepth: "cluster",
      litClusters: true,
      farCascadeRate: 4,
      figureClusters: true,
      hullClusters: true,
      wireClusters: true,
      pageBytes: 128 * 2 ** 20,
      meshTolerancePx: { camera: 1, wireframeOccluder: 0.5 },
      finClusters: true,
      strides: "off",
      occlusion: "off",
    },
    decoration: true,
    scatterDensity: 1,
    clouds: "volume",
    cloudShadows: true,
    oceanComponents: 32,
    oceanRefraction: true,
    whitecaps: true,
    rings: "particles",
    ringShadowOnBody: true,
  },
  low: {
    terrain: {
      tauPx: 2,
      renderHeightPx: 720,
      normals: "mesh",
      vertexPath: "face-differences",
      cacheBytes: LOW_CACHE_BYTES,
      handoverDetail: "whole",
    },
    atmosphere: TABLE_SIZES.low,
    atmosphereView: ATMOSPHERE_VIEW_SETTINGS.low,
    atmosphereLevers: ATMOSPHERE_SETTINGS.low,
    sky: LOW_SKY,
    internalScaleBounds: [0.5, 1],
    budget: { photorealisticRateHz: 30, photorealisticViews: 1 },
    photoreal: {
      histogramStride: 2,
      bloom: BLOOM_LEVELS.low,
      discAnnuli: DISC_ANNULI_LOW,
      planetshineSources: PLANETSHINE_SOURCES_LOW,
      floorLimbSamples: 2,
    },
    geometry: {
      clusterPath: true,
      casterDepthCull: true,
      casterStrideCap: 8,
      overlayDepth: "cluster",
      litClusters: true,
      farCascadeRate: 4,
      figureClusters: true,
      hullClusters: true,
      wireClusters: true,
      pageBytes: 32 * 2 ** 20,
      meshTolerancePx: { camera: 2, wireframeOccluder: 0.5 },
      finClusters: true,
      strides: "measured",
      occlusion: "off",
    },
    decoration: false,
    scatterDensity: 0,
    clouds: "layer",
    cloudShadows: false,
    oceanComponents: 8,
    oceanRefraction: false,
    whitecaps: false,
    rings: "annulus",
    ringShadowOnBody: false,
  },
};

/** The terrain's settings per {@link QualitySetting}: `SETTINGS[s].terrain`, for selection. */
export const TERRAIN_SETTINGS: Readonly<Record<QualitySetting, TerrainSettings>> = {
  high: SETTINGS.high.terrain,
  low: SETTINGS.low.terrain,
};

/**
 * The wireframe style's screen-space error tolerance τ, px, on either setting: its depth-only
 * terrain's (plan R10, T11) and the disc's hand-over to it (R10 Design note 13: τ is the style's
 * own, {@link TerrainSettings.tauPx} in the photorealistic style, 1 px high and 2 px low, and
 * 4 px in the wireframe).
 */
export const WIREFRAME_TERRAIN_TAU_PX = 4;

/**
 * The slots of the wireframe's terrain cache on either setting (plan R10, R10.T11), held by
 * R10.T14.b (decision-r10-t14b-constants) on the hard bound, re-sized after R10.T4.b and the
 * owner's UHD 620 run (the rule in its union form asked 8,666 high lit slots, 2,996 low lit and
 * 3,764 wireframe on TERRAIN TEST's moon, about seven times the held 1,286, 422 and 520): Design
 * note 15's sizing rule over its wireframe column (4 px at 1080p), max(1.3 × the all-round peak,
 * 3 × the frustum peak) = max(1.3 × 284, 3 × 76) ≈ 370 at 100 m, and the 20–40% the count model
 * leaves out for relief, the 2:1 rule and the forced region at its upper end, about 520. Sized by
 * its slots, not by the setting's bytes: a wireframe slot holds its heights, offsets on their path
 * and survey mask alone (139,428 B on `baked-offsets`, 42% of a lit slot; 38,028 B on
 * `face-differences`, a fifth), so the high setting's 400 MB would give it 2,868 slots, whose
 * offsets pass a default 128 MiB storage binding.
 */
export const WIREFRAME_TERRAIN_SLOTS = 520;

/**
 * A terrain variant for the spike's runs (T13.c's `--vertex-path` and `--normals`, T17's): the
 * fields given replace the setting's own; nothing else is overridable.
 */
export interface TerrainVariant {
  readonly vertexPath?: TerrainVertexPath;
  readonly normals?: TerrainNormals;
}

/**
 * `SETTINGS[setting].terrain` with the variant's fields replaced; an absent field keeps the
 * setting's (the spike variant flags ruling, decision-r05-spike-ux.md).
 *
 * @throws RangeError for `low` with `baked-offsets`, which does not fit the low cache (Design note 4).
 */
export function terrainSettingsFor(
  setting: QualitySetting,
  variant: TerrainVariant = {},
): TerrainSettings {
  const base = SETTINGS[setting].terrain;
  const vertexPath = variant.vertexPath ?? base.vertexPath;
  if (setting === "low" && vertexPath === "baked-offsets") {
    throw new RangeError("the low setting's cache does not fit the baked-offsets vertex path");
  }
  if (variant.vertexPath === undefined && variant.normals === undefined) {
    return base;
  }
  return { ...base, vertexPath, normals: variant.normals ?? base.normals };
}
